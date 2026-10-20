//! Finite, paced W4 process experiment. Wall=0 is a dry run and never passes endurance.
use crate::{
    group_sampler::{self, Registry, Sampler},
    joint_reference::{self as joint, Policy, Reference},
    sim_client::Client,
    sim_host::{Command, Event, ProbePoint},
};
use mycity_core::{
    boot::{Command as B, Facility},
    schedule::Phase,
    sha256,
    timeline::Command as T,
    transaction::{Action, Envelope},
};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
struct Experiment {
    client: Client,
    reference: Reference,
    policy: Policy,
    world: PathBuf,
    exe: PathBuf,
    id: u64,
    log: File,
    log_bytes: u64,
    started: Instant,
    registry: Registry,
    observed: group_sampler::Observed,
    faults: Vec<Value>,
    checkpoints: Vec<Value>,
    worker_failed: bool,
    sim_failed: bool,
    latencies: Vec<f64>,
    attempt_failures: u64,
}
impl Experiment {
    fn record(&mut self, value: Value) -> Result<(), String> {
        let line = json!({"elapsed_seconds":self.started.elapsed().as_secs_f64(),"data":value});
        let raw = serde_json::to_vec(&line).map_err(err)?;
        if raw.len() > 128 * 1024 || self.log_bytes + raw.len() as u64 + 1 > 32 * 1024 * 1024 {
            return Err("joint_transcript_bytes".into());
        }
        self.log
            .write_all(&raw)
            .and_then(|_| self.log.write_all(b"\n"))
            .map_err(err)?;
        self.log_bytes += raw.len() as u64 + 1;
        Ok(())
    }
    fn next(&mut self) -> Result<Event, String> {
        let event = self.client.next(Duration::from_secs(35))?;
        if event.kind == "worker_started" {
            let pid = u32::try_from(
                event.payload["worker_pid"]
                    .as_u64()
                    .ok_or("joint_worker_pid")?,
            )
            .map_err(err)?;
            group_sampler::declare(&self.registry, pid, "worker", Some(self.client.pid()))?;
        }
        if event.kind == "attempt_failed" {
            self.attempt_failures += 1;
        }
        self.record(json!({"event":event}))?;
        Ok(event)
    }
    fn start(&mut self, command: Command) -> Result<(), String> {
        self.id = self.id.checked_add(1).ok_or("joint_request_id")?;
        self.record(json!({"id":self.id,"command":command}))?;
        self.client.submit(self.id, command)
    }
    fn finish(&mut self) -> Result<Value, String> {
        for _ in 0..24 {
            let event = self.next()?;
            if event.kind == "result" {
                return event
                    .payload
                    .get("Ok")
                    .cloned()
                    .ok_or_else(|| format!("joint_error:{:?}", event.payload));
            }
        }
        Err("joint_frame_count".into())
    }
    fn call(&mut self, command: Command) -> Result<Value, String> {
        self.start(command)?;
        self.finish()
    }
    fn raw(&mut self, actions: Vec<Action>) -> Result<Envelope, String> {
        let s = self.call(Command::Status)?;
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: s["sequence"].as_u64().ok_or("joint_sequence")? + 1,
            expected_revision: s["revision"].as_u64().ok_or("joint_revision")?,
            actions,
        };
        let result = self.call(Command::Execute {
            envelope: envelope.clone(),
        })?;
        if result["model"].get("Ok").is_none() || result["has_persistent_receipt"] != true {
            return Err(format!("joint_model:{result}"));
        }
        Ok(envelope)
    }
    fn execute(&mut self, actions: Vec<Action>) -> Result<Envelope, String> {
        let envelope = self.raw(actions.clone())?;
        self.reference.execute(actions)?;
        Ok(envelope)
    }
    fn barrier(&mut self) -> Result<Value, String> {
        for _ in 0..8 {
            let event = self.next()?;
            if event.kind == "probe_barrier" {
                return Ok(event.payload);
            }
            if event.kind == "result" {
                return Err("joint_barrier_missing".into());
            }
        }
        Err("joint_barrier_frames".into())
    }
    fn required(&mut self, job: u64, before: &Value) -> Result<(), String> {
        let started = Instant::now();
        if !self.worker_failed && job >= 2 {
            self.start(Command::ProbeRequired {
                job,
                point: ProbePoint::WorkerCells,
            })?;
            let barrier = self.barrier()?;
            let killed = self.client.worker_pid().ok_or("joint_worker_missing")?;
            if barrier["details"]["cursor"].as_u64().is_none_or(|n| n == 0) {
                return Err("joint_not_inside_cells".into());
            }
            self.record(
                json!({"fault":"worker_sigkill","job":job,"pid":killed,"barrier":barrier}),
            )?;
            group_sampler::wait_observed(&self.observed, killed, "worker")?;
            self.client.kill_owned_worker()?;
            self.client
                .acknowledge_probe(ProbePoint::WorkerCells, false)?;
            let result = self.finish()?;
            self.faults.push(json!({"kind":"worker_sigkill","job":job,"pid":killed,"barrier":barrier,"result":result}));
            self.worker_failed = true;
        } else if !self.sim_failed
            && before["timeline"]["at"]
                .as_u64()
                .is_some_and(|at| at >= 86400)
        {
            self.start(Command::ProbeRequired {
                job,
                point: ProbePoint::Committed,
            })?;
            let barrier = self.barrier()?;
            let old_sim = self.client.pid();
            let old_worker = self.client.worker_pid();
            self.record(json!({"fault":"sim_sigkill_committed_before_reply","job":job,"sim_pid":old_sim,"worker_pid":old_worker,"barrier":barrier,"before_business_sha256":before["business_sha256"]}))?;
            if !std::process::Command::new("kill")
                .args(["-KILL", &old_sim.to_string()])
                .status()
                .map_err(err)?
                .success()
            {
                return Err("joint_sim_kill".into());
            }
            self.client.wait_for_sim_exit(Duration::from_secs(2))?;
            let next = Client::spawn_joint(&self.exe, &self.world, false, true)?;
            let mut old = std::mem::replace(&mut self.client, next);
            group_sampler::declare(
                &self.registry,
                self.client.pid(),
                "sim",
                Some(std::process::id()),
            )?;
            let ready = self.next()?;
            if ready.kind != "ready"
                || ready.payload["business_sha256"] != before["business_sha256"]
                || ready.payload["runtime"]["accounts"] != before["runtime"]["accounts"]
                || ready.payload["runtime"]["payroll"] != before["runtime"]["payroll"]
                || ready.payload["runtime"]["obligations"] != before["runtime"]["obligations"]
            {
                return Err("joint_reopen_repeated_business".into());
            }
            let result = self.call(Command::RunRequired { job })?;
            if result["replayed"] != true {
                return Err("joint_lost_reply_not_original".into());
            }
            self.faults.push(json!({"kind":"sim_sigkill_committed_before_reply","job":job,"old_sim_pid":old_sim,"new_sim_pid":self.client.pid(),"old_worker_pid":old_worker,"barrier":barrier,"before":before,"reopened":ready.payload,"result":result}));
            old.stop()?;
            self.sim_failed = true;
        } else {
            self.call(Command::RunRequired { job })?;
        }
        if self.latencies.len() >= 512 {
            return Err("joint_latency_limit".into());
        }
        self.latencies.push(started.elapsed().as_secs_f64());
        Ok(())
    }
    fn drive(&mut self, target: u64) -> Result<(), String> {
        self.reference.drive(target)?;
        for _ in 0..10000 {
            let state = self.call(Command::Status)?;
            if state["timeline"]["target"].is_null() && state["timeline"]["last_complete"] == target
            {
                let reference = self.reference.summary(&self.policy)?;
                if state["business_sha256"] != reference["business_sha256"]
                    || state["background"]["state_sha256"] != reference["air_sha256"]
                    || state["background"]["version"] != self.reference.windows
                    || state["runtime"]["accounts"] != reference["accounts"]
                    || state["runtime"]["payroll"] != reference["payroll"]
                    || state["runtime"]["claims"] != reference["claims"]
                {
                    return Err(format!("joint_reference_mismatch_at:{target}"));
                }
                let record = json!({"game_seconds":target,"elapsed_seconds":self.started.elapsed().as_secs_f64(),"business_sha256":state["business_sha256"],"reference_business_sha256":reference["business_sha256"],"air_sha256":state["background"]["state_sha256"],"reference_air_sha256":reference["air_sha256"],"capacity":state["capacity"],"reference":reference});
                self.record(json!({"verified_checkpoint":record}))?;
                self.checkpoints.push(record);
                return Ok(());
            }
            let phase = serde_json::from_value::<Option<Phase>>(state["timeline"]["phase"].clone())
                .map_err(err)?;
            if phase == Some(Phase::P4) && !state["background"]["pending"].is_null() {
                let job = state["background"]["pending"]["id"]
                    .as_u64()
                    .ok_or("joint_job")?;
                self.required(job, &state)?;
            } else {
                self.raw(vec![Action::Timeline {
                    command: T::Advance {
                        target,
                        limit: 8192,
                    },
                }])?;
            }
        }
        Err("joint_drive_limit".into())
    }
}
pub fn run(dir: &Path, wall_seconds: u64) -> Result<Value, String> {
    let policy = joint::policy()?;
    if wall_seconds != 0 && !(policy.minimum_wall_seconds..=2100).contains(&wall_seconds) {
        return Err("joint_wall_must_be_zero_dry_or_1800_to_2100".into());
    }
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    let world = dir.join("world");
    let mut sampler = Sampler::start(&dir.join("group-samples.jsonl"), &world)?;
    let system_before = group_sampler::system_snapshot()?;
    let exe = std::env::current_exe().map_err(err)?;
    let client = Client::spawn_joint(&exe, &world, true, true)?;
    group_sampler::declare(
        &sampler.registry,
        client.pid(),
        "sim",
        Some(std::process::id()),
    )?;
    let mut e = Experiment {
        client,
        reference: Reference::new()?,
        policy,
        world,
        exe,
        id: 0,
        log: File::create(dir.join("transcript.jsonl")).map_err(err)?,
        log_bytes: 0,
        started: Instant::now(),
        registry: sampler.registry.clone(),
        observed: sampler.observed.clone(),
        faults: Vec::new(),
        checkpoints: Vec::new(),
        worker_failed: false,
        sim_failed: false,
        latencies: Vec::new(),
        attempt_failures: 0,
    };
    if e.next()?.kind != "ready" {
        return Err("joint_sim_not_ready".into());
    }
    let work_started = Instant::now();
    let work_started_elapsed_seconds = e.started.elapsed().as_secs_f64();
    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(err)?
        .as_millis();
    e.record(json!({"begin":{"wall_seconds":wall_seconds,"started_unix_ms":started_unix_ms,"policy_sha256":sha256(joint::POLICY),"opening_sha256":sha256(joint::OPENING)}}))?;
    for action in joint::setup(&e.policy)? {
        e.execute(vec![action])?;
    }
    e.drive(60)?;
    e.execute(vec![Action::Boot {
        command: B::Start {
            facility: Facility::Generator,
            reverse_request: false,
        },
    }])?;
    e.drive(120)?;
    for sequence in e.policy.inbox_delivery_sequences.clone() {
        e.execute(vec![Action::Inbox {
            command: mycity_core::inbox::Command::Settle { sequence },
        }])?;
        e.execute(vec![Action::Inbox {
            command: mycity_core::inbox::Command::Archive { sequence },
        }])?;
    }
    let mut arrears_seen = 0i64;
    let mut loan_receipt = None;
    for hour in 1..=168 {
        let due = Duration::from_secs_f64(wall_seconds as f64 * hour as f64 / 168.0);
        while work_started.elapsed() < due {
            std::thread::sleep(
                due.saturating_sub(work_started.elapsed())
                    .min(Duration::from_millis(250)),
            );
        }
        e.drive(hour * 3600)?;
        if hour * 3600 == e.policy.inactive_city.closed_at {
            e.execute(vec![Action::RegionActivate {
                city: e.policy.inactive_city.switch_to,
            }])?;
        }
        if hour * 3600 == e.policy.inactive_city.opened_at {
            e.execute(vec![Action::RegionActivate {
                city: e.policy.inactive_city.city,
            }])?;
        }
        if hour * 3600 == e.policy.loan.issued_at {
            let summary = e.reference.summary(&e.policy)?;
            arrears_seen = summary["tax"]["arrears_cents"]
                .as_i64()
                .ok_or("joint_tax_arrears")?;
            if arrears_seen <= 0 {
                return Err("joint_tax_shortage_not_exercised".into());
            }
            loan_receipt = Some(e.execute(joint::loan(&e.policy)?)?);
            e.execute(vec![Action::RegionSettleDebt {
                obligation: e.policy.tax.obligation_id,
            }])?;
            if e.reference.summary(&e.policy)?["tax"]["arrears_cents"] != 0 {
                return Err("joint_old_tax_not_settled".into());
            }
        }
        if hour % 24 == 0 {
            e.call(Command::Checkpoint)?;
            let progress = json!({"hour":hour,"elapsed_seconds":e.started.elapsed().as_secs_f64(),"windows":e.reference.windows,"faults":e.faults.len()});
            fs::write(
                dir.join("progress.json"),
                serde_json::to_vec_pretty(&progress).map_err(err)?,
            )
            .map_err(err)?;
            e.record(
                json!({"daily_progress":progress,"system":group_sampler::system_snapshot()?}),
            )?;
        }
    }
    let work_wall = work_started.elapsed().as_secs_f64();
    let facts = joint::final_facts(&e.reference.engine, &e.policy)?;
    let final_state = e.call(Command::Status)?;
    let reference = e.reference.summary(&e.policy)?;
    if !e.worker_failed
        || !e.sim_failed
        || e.attempt_failures != 1
        || reference["cash_total"] != 3000000
        || reference["tax"]["paid_cents"] != 1400000
        || reference["tax"]["arrears_cents"] != 0
        || reference["payroll"][0]["paid"] != 336
        || reference["payroll"][1]["paid"] != 528
    {
        return Err("joint_final_semantics".into());
    }
    e.call(Command::Checkpoint)?;
    let last_job = e.reference.windows;
    let cold = e.call(Command::RunRequired { job: last_job })?;
    if cold["replayed"] != true {
        return Err("joint_final_cold_receipt".into());
    }
    e.call(Command::Stop)?;
    e.client.stop()?;
    let reopened = Client::spawn_joint(&e.exe, &e.world, false, false)?;
    group_sampler::declare(
        &sampler.registry,
        reopened.pid(),
        "sim",
        Some(std::process::id()),
    )?;
    let mut old = std::mem::replace(&mut e.client, reopened);
    old.stop()?;
    let ready = e.next()?;
    if ready.kind != "ready"
        || ready.payload["business_sha256"] != reference["business_sha256"]
        || ready.payload["background"]["state_sha256"] != reference["air_sha256"]
    {
        return Err("joint_final_reopen_changed_state".into());
    }
    group_sampler::wait_observed(&e.observed, e.client.pid(), "sim")?;
    let again = e.call(Command::RunRequired { job: last_job })?;
    if cold["record"] != again["record"] {
        return Err("joint_final_original_receipt_changed".into());
    }
    e.call(Command::Stop)?;
    e.client.stop()?;
    fs::write(
        dir.join("reference-engine.json"),
        e.reference.engine.export_checkpoint()?,
    )
    .map_err(err)?;
    e.log.sync_all().map_err(err)?;
    let system_after = group_sampler::system_snapshot()?;
    let samples = sampler.finish()?;
    if samples
        .last_group
        .iter()
        .any(|p| matches!(p.role.as_str(), "sim" | "worker"))
    {
        return Err("joint_processes_still_running".into());
    }
    let maximum_latency = e.latencies.iter().copied().fold(0.0, f64::max);
    let report = json!({"schema":"mycity-joint-probe-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"policy_sha256":sha256(joint::POLICY),"opening_sha256":sha256(joint::OPENING),"requested_wall_seconds":wall_seconds,"work_wall_seconds":work_wall,"started_unix_ms":started_unix_ms,"work_started_elapsed_seconds":work_started_elapsed_seconds,"game_seconds":e.policy.game_seconds,"endurance_1800_seconds":if wall_seconds>=1800 && work_wall>=1800.0{"passed"}else{"not_run_dry"},"memory_16gb_gate":"not_run","host_system_before":system_before,"host_system_after":system_after,"resource_samples":samples,"rss_interpretation":"same ps snapshot sums; shared pages may double count; sub-250ms peaks may be missed; sampler_ps included","faults":e.faults,"attempt_failures":e.attempt_failures,"configured_limits":{"sim_reader_frames":4,"worker_reader_frames":4,"control_bytes":65536,"inflight_jobs":1,"attempts_per_job":3,"queue_peak_occupancy":"not_instrumented"},"maximum_required_roundtrip_seconds":maximum_latency,"air_windows":e.reference.windows,"tax_arrears_observed_cents":arrears_seen,"loan_envelope":loan_receipt,"reference":reference,"realized_facts":facts,"final_state":final_state,"final_original_field_receipt":again,"checkpoints":e.checkpoints,"transcript_bytes":e.log_bytes,"independent_physical_accuracy":"not_verified","full_M0":"not_accepted","final_advisor_review":"pending"});
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(err)?,
    )
    .map_err(err)?;
    Ok(report)
}
