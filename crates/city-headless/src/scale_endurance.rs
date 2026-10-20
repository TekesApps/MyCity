//! Absolute-clock controller for the original full trajectory. No layout/business substitution.
//! Functional smoke uses the SAME 720 hours; shortened wall cadence never accepts endurance.
use crate::{
    scale_model_task::{self, Mode},
    scale_reference::{self, Step, Tier},
    scale_sim::{self, Experiment},
    sim_client::Client,
    sim_host::{Command, ProbePoint},
};
use mycity_core::{
    background, schedule::Phase, timeline::Command as Timeline, transaction::Action,
};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub enum Cadence {
    Formal,
    Smoke,
}
impl Cadence {
    fn hour(self) -> Duration {
        match self {
            Self::Formal => Duration::from_secs(40),
            Self::Smoke => Duration::from_millis(100),
        }
    }
    fn query(self) -> Duration {
        self.hour() / 4
    }
    fn save(self) -> Duration {
        self.hour() * 15 / 2
    }
    fn formal(self) -> bool {
        matches!(self, Self::Formal)
    }
}
/// Counters keep all nominal deadlines, including those missed while busy. No catch-up coalescing.
struct Clock {
    cadence: Cadence,
    began: Instant,
    queries: u32,
    saves: u32,
    retry_job: Option<u64>,
    committed_job: Option<u64>,
    cold: bool,
    max_save_lateness: Duration,
    max_query_lateness: Duration,
}
impl Clock {
    fn elapsed(&self) -> Duration {
        self.began.elapsed()
    }
    fn total(&self) -> Duration {
        self.cadence.hour() * 720
    }
    fn query_due(&self) -> Option<Duration> {
        let t = self.cadence.query() * (self.queries + 1);
        (t <= self.total()).then_some(t)
    }
    fn save_due(&self) -> Option<Duration> {
        (self.saves < 96).then(|| self.cadence.save() * (self.saves + 1))
    }
    fn fault_due(&self, hours: u32) -> bool {
        self.elapsed() >= self.cadence.hour() * hours
    }
}
struct Driver<'a> {
    e: Experiment,
    clock: Option<Clock>,
    dir: &'a Path,
    first: Option<(mycity_core::transaction::Envelope, Value)>,
}
impl Driver<'_> {
    fn spawn(&self) -> Result<Client, String> {
        Client::spawn_scale_measured(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &self.dir.join("world"),
            false,
            true,
            Tier::Full,
        )
    }
    fn oldest(&mut self) -> Result<(), String> {
        let (envelope, proof) = self
            .first
            .as_ref()
            .ok_or("endurance_oldest_absent")?
            .clone();
        if self.e.call(Command::ReceiptProof {
            operation: envelope.operation_id,
        })? != proof
        {
            return Err("endurance_oldest_proof".into());
        }
        let before = self.e.call(Command::Status)?;
        let retry = self.e.call(Command::Execute { envelope })?;
        let after = self.e.call(Command::Status)?;
        if retry["has_persistent_receipt"] != true
            || before["sequence"] != after["sequence"]
            || before["state_sha256"] != after["state_sha256"]
        {
            return Err("endurance_oldest_retry_side_effect".into());
        }
        Ok(())
    }
    fn reopen(&mut self, reason: &str) -> Result<(), String> {
        self.e.call(Command::Checkpoint)?;
        let before = self.e.call(Command::Status)?;
        self.e.call(Command::Stop)?;
        self.e.client.stop()?;
        self.e.client = self.spawn()?;
        if self.e.client.next(Duration::from_secs(35))?.kind != "ready" {
            return Err("endurance_reopen_ready".into());
        }
        let after = self.e.call(Command::Status)?;
        for k in [
            "sequence",
            "business_sha256",
            "background",
            "timeline",
            "state_sha256",
        ] {
            if before[k] != after[k] {
                return Err(format!("endurance_cold_changed:{k}"));
            }
        }
        self.oldest()?;
        self.e
            .record(json!({"endurance_cold_reopen":reason,"before":before,"after":after}))
    }
    /// Called only with no outstanding request, between model transaction/result boundaries.
    fn service(&mut self) -> Result<(), String> {
        if self.clock.is_none() {
            return Ok(());
        }
        loop {
            let c = self.clock.as_ref().unwrap();
            let now = c.elapsed();
            let q = c.query_due().filter(|d| *d <= now);
            let s = c.save_due().filter(|d| *d <= now);
            let choice = match (q, s) {
                (Some(q), Some(s)) => {
                    if q <= s {
                        Some((false, q))
                    } else {
                        Some((true, s))
                    }
                }
                (Some(q), None) => Some((false, q)),
                (None, Some(s)) => Some((true, s)),
                _ => None,
            };
            let Some((save, due)) = choice else {
                break;
            };
            let start = self.clock.as_ref().unwrap().elapsed();
            self.e.call(if save {
                Command::Checkpoint
            } else {
                Command::Progress
            })?;
            let c = self.clock.as_mut().unwrap();
            let complete = c.elapsed();
            let late = complete.saturating_sub(due);
            let n = if save {
                c.saves += 1;
                c.max_save_lateness = c.max_save_lateness.max(late);
                c.saves
            } else {
                c.queries += 1;
                c.max_query_lateness = c.max_query_lateness.max(late);
                c.queries
            };
            self.e.record(json!({"periodic":if save {"save"} else {"query"},"ordinal":n,"deadline_seconds":due.as_secs_f64(),"submitted_seconds":start.as_secs_f64(),"completed_seconds":complete.as_secs_f64(),"completion_lateness_seconds":late.as_secs_f64()}))?;
        }
        let c = self.clock.as_ref().unwrap();
        if !c.cold && c.fault_due(540) {
            let submitted = c.elapsed();
            self.reopen("scheduled_6h")?;
            let c = self.clock.as_mut().unwrap();
            c.cold = true;
            let completed = c.elapsed();
            self.e.record(json!({"scheduled_reopen_deadline_seconds":(c.cadence.hour()*540).as_secs_f64(),"submitted_seconds":submitted.as_secs_f64(),"completed_seconds":completed.as_secs_f64()}))?;
        }
        Ok(())
    }
    fn wait_hour(&mut self, hour: u32) -> Result<(), String> {
        let due = self
            .clock
            .as_ref()
            .ok_or("endurance_clock_absent")?
            .cadence
            .hour()
            * hour;
        loop {
            self.service()?;
            let c = self.clock.as_ref().unwrap();
            let now = c.elapsed();
            if now >= due {
                self.e.record(json!({"hour_release":hour,"deadline_seconds":due.as_secs_f64(),"submitted_seconds":now.as_secs_f64()}))?;
                return Ok(());
            }
            std::thread::sleep((due - now).min(Duration::from_millis(100)));
        }
    }
    fn required(&mut self, job: u64) -> Result<(), String> {
        let c = self.clock.as_ref().ok_or("endurance_clock_absent")?;
        let point = if c.retry_job.is_none() && c.fault_due(180) {
            Some(ProbePoint::WorkerCells)
        } else if c.committed_job.is_none() && c.fault_due(360) {
            Some(ProbePoint::Committed)
        } else {
            None
        };
        let Some(point) = point else {
            self.e.call(Command::RunRequired { job })?;
            return Ok(());
        };
        self.e.id = self.e.id.checked_add(1).ok_or("endurance_id")?;
        let id = self.e.id;
        self.e
            .record(json!({"request":id,"command":Command::ProbeRequired{job,point}}))?;
        self.e
            .client
            .submit(id, Command::ProbeRequired { job, point })?;
        let began = Instant::now();
        let mut injected = false;
        for _ in 0..32 {
            let event = self.e.client.next(Duration::from_secs(35))?;
            self.e
                .record(json!({"response":id,"kind":event.kind,"payload":event.payload}))?;
            if event.kind == "probe_barrier" {
                if injected
                    || event.payload["point"]
                        != serde_json::to_value(point).map_err(|e| e.to_string())?
                {
                    return Err("endurance_fault_barrier".into());
                }
                let at = self.clock.as_ref().unwrap().elapsed();
                match point {
                    ProbePoint::WorkerCells => {
                        self.e.client.kill_owned_worker()?;
                        self.e.client.acknowledge_probe(point, false)?;
                        self.clock.as_mut().unwrap().retry_job = Some(job);
                        injected = true;
                    }
                    ProbePoint::Committed => {
                        let operation = background::accept_operation_id(job)?;
                        if event.payload["details"]["operation_id"] != operation {
                            return Err("endurance_committed_identity".into());
                        }
                        self.e.client.kill_sim()?;
                        self.e.client = self.spawn()?;
                        if self.e.client.next(Duration::from_secs(35))?.kind != "ready" {
                            return Err("endurance_fault_ready".into());
                        }
                        let proof = self.e.call(Command::ReceiptProof { operation })?;
                        let before = self.e.call(Command::Status)?;
                        let replay = self.e.call(Command::RunRequired { job })?;
                        let after = self.e.call(Command::Status)?;
                        if replay["replayed"] != true
                            || before["state_sha256"] != after["state_sha256"]
                            || before["sequence"] != after["sequence"]
                            || proof != self.e.call(Command::ReceiptProof { operation })?
                        {
                            return Err("endurance_lost_reply_replay".into());
                        }
                        self.oldest()?;
                        self.clock.as_mut().unwrap().committed_job = Some(job);
                        self.e.record(json!({"endurance_fault":"sim_committed_reply","job":job,"operation":operation,"at_seconds":at.as_secs_f64(),"recovered_seconds":self.clock.as_ref().unwrap().elapsed().as_secs_f64(),"recovery_seconds":began.elapsed().as_secs_f64(),"original_proof":proof}))?;
                        return Ok(());
                    }
                    _ => return Err("endurance_fault_point".into()),
                }
                self.e.record(json!({"endurance_fault":"worker_cells","job":job,"at_seconds":at.as_secs_f64()}))?;
            }
            if event.kind == "result" {
                if !injected || event.payload.get("Ok").is_none() {
                    return Err("endurance_retry_result".into());
                }
                self.e.record(json!({"endurance_worker_recovered":job,"recovery_seconds":began.elapsed().as_secs_f64()}))?;
                return Ok(());
            }
        }
        Err("endurance_fault_event_bound".into())
    }
    fn drive(&mut self, target: u64, expected: &Value) -> Result<(), String> {
        for _ in 0..10000 {
            self.service()?;
            let state = self.e.call(Command::Progress)?;
            if state["timeline"]["target"].is_null() && state["timeline"]["last_complete"] == target
            {
                let state = self.e.call(Command::Status)?;
                if expected["target"] != target
                    || state["business_sha256"] != expected["business_sha256"]
                    || state["background"]["state_sha256"] != expected["air_sha256"]
                    || state["background"]["version"] != expected["windows"]
                    || state["background"]["time"] != target
                    || !state["background"]["pending"].is_null()
                {
                    return Err(format!("endurance_target_reference:{target}"));
                }
                self.e.record(json!({"endurance_verified_target":target,"expected":expected,"state":state,"elapsed_seconds":self.clock.as_ref().unwrap().elapsed().as_secs_f64()}))?;
                return Ok(());
            }
            let phase: Option<Phase> = serde_json::from_value(state["timeline"]["phase"].clone())
                .map_err(|e| e.to_string())?;
            if phase == Some(Phase::P4) && !state["background"]["pending"].is_null() {
                self.required(
                    state["background"]["pending"]["id"]
                        .as_u64()
                        .ok_or("endurance_job")?,
                )?;
            } else {
                self.e.raw_at(
                    &state,
                    vec![Action::Timeline {
                        command: Timeline::Advance {
                            target,
                            limit: 8192,
                        },
                    }],
                )?;
            }
        }
        Err("endurance_drive_bound".into())
    }
}
fn save(path: &Path, v: &Value) -> Result<(), String> {
    let mut f = File::create_new(path).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut f, v).map_err(|e| e.to_string())?;
    f.write_all(b"\n").map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}
pub fn run(dir: &Path, cadence: Cadence) -> Result<Value, String> {
    fs::create_dir(dir).map_err(|e| e.to_string())?;
    let dir = dir.canonicalize().map_err(|e| e.to_string())?;
    save(
        &dir.join("controller-manifest.json"),
        &json!({"source_sha256":env!("MYCITY_SOURCE_SHA256"),"formal":cadence.formal(),"hour_wall_seconds":cadence.hour().as_secs_f64(),"game_hours":720,"periodic_saves":96,"L1":"external_joint_runner_required","eight_hour_gate":"not_accepted_until_independent_joint_audit"}),
    )?;
    let result = run_inner(&dir, cadence);
    if let Err(error) = &result {
        save(
            &dir.join("failure.json"),
            &json!({"error":error,"eight_hour_gate":"failed_or_incomplete","full_M0":"not_accepted"}),
        )?;
    }
    result
}
fn run_inner(dir: &Path, cadence: Cadence) -> Result<Value, String> {
    scale_model_task::run(dir, Tier::Full, Mode::Reference)?;
    let e = scale_sim::independent_experiment_options(dir, true, true, true)?;
    let mut d = Driver {
        e,
        clock: None,
        dir,
        first: None,
    };
    let mut lines =
        BufReader::new(File::open(dir.join("cpu-targets.jsonl")).map_err(|e| e.to_string())?);
    let mut hours = 0;
    let mut targets = 0;
    scale_reference::walk_tier(Tier::Full, |step| {
        match step {
            Step::Action(action) => {
                d.service()?;
                let env = d.e.raw(vec![action])?;
                if d.first.is_none() {
                    let proof = d.e.call(Command::ReceiptProof {
                        operation: env.operation_id,
                    })?;
                    d.first = Some((env, proof));
                }
            }
            Step::Initialized => {
                d.clock = Some(Clock {
                    cadence,
                    began: Instant::now(),
                    queries: 0,
                    saves: 0,
                    retry_job: None,
                    committed_job: None,
                    cold: false,
                    max_save_lateness: Duration::ZERO,
                    max_query_lateness: Duration::ZERO,
                });
                d.e.record(json!({"endurance_clock_started":true}))?;
            }
            Step::Drive(target) => {
                if target % 3600 == 0 {
                    d.wait_hour((target / 3600) as u32)?;
                }
                let line =
                    scale_sim::target_line(&mut lines)?.ok_or("endurance_missing_cpu_target")?;
                d.drive(
                    target,
                    &serde_json::from_slice(&line).map_err(|e| e.to_string())?,
                )?;
                targets += 1;
            }
            Step::HourComplete => hours += 1,
            Step::PeakShipments => (),
        }
        Ok(())
    })?;
    d.service()?;
    let c = d.clock.as_ref().ok_or("endurance_clock")?;
    if hours != 720
        || targets != 723
        || c.saves != 96
        || c.query_due().is_some()
        || !c.cold
        || c.retry_job.is_none()
        || c.committed_job.is_none()
        || c.elapsed() < c.total()
        || scale_sim::target_line(&mut lines)?.is_some()
    {
        return Err("endurance_incomplete_counts".into());
    }
    let retry_job = c.retry_job.unwrap();
    let clock_report = json!({"formal":cadence.formal(),"actual_wall_seconds":c.elapsed().as_secs_f64(),"required_wall_seconds":c.total().as_secs_f64(),"hours":hours,"targets":targets,"queries_completed":c.queries,"saves_completed":c.saves,"retry_job":retry_job,"committed_job":c.committed_job,"scheduled_cold_reopen":c.cold,"max_save_completion_lateness_seconds":c.max_save_lateness.as_secs_f64(),"max_query_completion_lateness_seconds":c.max_query_lateness.as_secs_f64()});
    d.oldest()?;
    d.e.call(Command::Checkpoint)?;
    d.e.call(Command::Stop)?;
    d.e.client.stop()?;
    scale_model_task::run_retry(dir, Tier::Full, Mode::FaultReference, Some(retry_job))?;
    scale_model_task::run(dir, Tier::Full, Mode::BeforeReopen)?;
    d.e.client = d.spawn()?;
    if d.e.client.next(Duration::from_secs(35))?.kind != "ready" {
        return Err("endurance_final_ready".into());
    }
    d.oldest()?;
    d.e.call(Command::Checkpoint)?;
    d.e.call(Command::Stop)?;
    d.e.client.stop()?;
    let compared = scale_model_task::run(dir, Tier::Full, Mode::CompareEndurance)?;
    let report = json!({"schema":"mycity-endurance-controller-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"clock":clock_report,"entire_engine_including_metadata":"equal_independent_fault_reference","image_sha256":compared.image_sha256,"business_sha256":compared.business_sha256,"eight_hour_gate":"not_accepted_until_independent_joint_audit","L1_joint":"not_run_by_this_controller","real_16GB":"not_run","full_M0":"not_accepted"});
    save(&dir.join("report.json"), &report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock(cadence: Cadence) -> Clock {
        Clock {
            cadence,
            began: Instant::now(),
            queries: 0,
            saves: 0,
            retry_job: None,
            committed_job: None,
            cold: false,
            max_save_lateness: Duration::ZERO,
            max_query_lateness: Duration::ZERO,
        }
    }
    #[test]
    fn formal_deadlines_keep_all_2880_queries_and_96_saves_and_last_hour_at_8h() {
        let mut c = clock(Cadence::Formal);
        assert_eq!(c.total(), Duration::from_secs(28800));
        for n in 1..=2880 {
            assert_eq!(c.query_due(), Some(Duration::from_secs(n * 10)));
            c.queries += 1;
        }
        assert!(c.query_due().is_none());
        for n in 1..=96 {
            assert_eq!(c.save_due(), Some(Duration::from_secs(n * 300)));
            c.saves += 1;
        }
        assert!(c.save_due().is_none());
    }
    #[test]
    fn late_scheduler_keeps_old_deadlines_and_sim_recovery_flags_without_clock_reset() {
        let mut c = clock(Cadence::Formal);
        c.began = Instant::now() - Duration::from_secs(21601);
        c.retry_job = Some(182);
        c.committed_job = Some(362);
        assert_eq!(c.query_due(), Some(Duration::from_secs(10)));
        assert_eq!(c.save_due(), Some(Duration::from_secs(300)));
        c.saves += 1;
        assert_eq!(c.save_due(), Some(Duration::from_secs(600)));
        assert!(c.fault_due(540));
        assert_eq!(c.retry_job, Some(182));
        assert_eq!(c.committed_job, Some(362));
        assert!(c.elapsed() >= Duration::from_secs(21601));
    }
    #[test]
    fn smoke_scales_wall_time_only_not_number_of_business_or_periodic_requests() {
        let mut c = clock(Cadence::Smoke);
        assert!(!c.cadence.formal());
        assert_eq!(c.total(), Duration::from_secs(72));
        c.queries = 2879;
        assert_eq!(c.query_due(), Some(c.total()));
        c.saves = 95;
        assert_eq!(c.save_due(), Some(c.total()));
    }
}
