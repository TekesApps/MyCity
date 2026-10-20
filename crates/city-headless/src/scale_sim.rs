//! Full frozen input through the existing sole-writer Sim and worker, not an 8h gate.
use crate::{
    joint_reference::business_digest,
    scale_reference::{self, Step},
    sim_client::Client,
    sim_host::Command,
};
use mycity_core::{
    read_bounded,
    schedule::Phase,
    sha256,
    timeline::Command as Timeline,
    transaction::{Action, Engine, Envelope},
};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    read_bounded(File::open(path).map_err(err)?, limit).map_err(err)
}
fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut f = File::create_new(path).map_err(err)?;
    serde_json::to_writer_pretty(&mut f, value).map_err(err)?;
    f.write_all(b"\n").map_err(err)?;
    f.sync_all().map_err(err)
}
struct Experiment {
    client: Client,
    id: u64,
    log: BufWriter<File>,
    log_bytes: usize,
    began: Instant,
    windows: u64,
    verified: u64,
    max_boundary_seconds: f64,
    failed_attempts: u64,
}
impl Experiment {
    fn record(&mut self, value: Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(
            &json!({"elapsed_seconds":self.began.elapsed().as_secs_f64(),"data":value}),
        )
        .map_err(err)?;
        if bytes.len() > 128 * 1024 || self.log_bytes + bytes.len() + 1 > 128 * 1024 * 1024 {
            return Err("scale_sim_log_bound".into());
        }
        self.log.write_all(&bytes).map_err(err)?;
        self.log.write_all(b"\n").map_err(err)?;
        self.log.flush().map_err(err)?;
        self.log_bytes += bytes.len() + 1;
        Ok(())
    }
    fn call(&mut self, command: Command) -> Result<Value, String> {
        self.id = self.id.checked_add(1).ok_or("scale_sim_id")?;
        self.record(json!({"request":self.id,"command":command}))?;
        self.client.submit(self.id, command)?;
        let started = Instant::now();
        for _ in 0..16384 {
            let event = self.client.next(Duration::from_secs(35))?;
            if event.kind == "attempt_failed" {
                self.failed_attempts += 1;
            }
            let raw = serde_json::to_vec(&event.payload).map_err(err)?;
            self.record(json!({"response":self.id,"kind":event.kind,"seconds":started.elapsed().as_secs_f64(),"payload_sha256":sha256(&raw),"payload_bytes":raw.len()}))?;
            if event.kind == "result" {
                return event
                    .payload
                    .get("Ok")
                    .cloned()
                    .ok_or_else(|| format!("scale_sim_result:{}", event.payload));
            }
        }
        Err("scale_sim_event_bound".into())
    }
    fn raw(&mut self, actions: Vec<Action>) -> Result<Envelope, String> {
        let state = self.call(Command::Status)?;
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: state["sequence"]
                .as_u64()
                .ok_or("scale_sim_sequence")?
                .checked_add(1)
                .ok_or("scale_sim_operation")?,
            expected_revision: state["revision"].as_u64().ok_or("scale_sim_revision")?,
            actions,
        };
        let outcome = self.call(Command::Execute {
            envelope: envelope.clone(),
        })?;
        if outcome["has_persistent_receipt"] != true || outcome["model"].get("Ok").is_none() {
            return Err(format!("scale_sim_business_failed:{outcome}"));
        }
        Ok(envelope)
    }
    fn drive(&mut self, target: u64, expected: &Value) -> Result<(), String> {
        let started = Instant::now();
        for _ in 0..10000 {
            let state = self.call(Command::Status)?;
            if state["timeline"]["target"].is_null() && state["timeline"]["last_complete"] == target
            {
                if expected["target"] != target
                    || state["business_sha256"] != expected["business_sha256"]
                    || state["background"]["state_sha256"] != expected["air_sha256"]
                    || state["background"]["version"] != expected["windows"]
                    || state["background"]["time"] != target
                    || !state["background"]["pending"].is_null()
                {
                    return Err(format!(
                        "scale_sim_reference_mismatch:{target}:expected={expected}:actual={state}"
                    ));
                }
                let seconds = started.elapsed().as_secs_f64();
                self.max_boundary_seconds = self.max_boundary_seconds.max(seconds);
                self.verified += 1;
                self.record(json!({"verified_target":target,"boundary_seconds":seconds,"expected":expected,"sequence":state["sequence"],"capacity":state["capacity"]}))?;
                return Ok(());
            }
            let phase: Option<Phase> =
                serde_json::from_value(state["timeline"]["phase"].clone()).map_err(err)?;
            if phase == Some(Phase::P4) && !state["background"]["pending"].is_null() {
                let job = state["background"]["pending"]["id"]
                    .as_u64()
                    .ok_or("scale_sim_job")?;
                self.call(Command::RunRequired { job })?;
                self.windows += 1;
            } else {
                self.raw(vec![Action::Timeline {
                    command: Timeline::Advance {
                        target,
                        limit: 8192,
                    },
                }])?;
            }
        }
        Err("scale_sim_drive_bound".into())
    }
}
/// Read only after the Sim has stopped, verifying the exact CURRENT -> root -> image chain.
fn image(world: &Path) -> Result<(Vec<u8>, Value), String> {
    let current: Value =
        serde_json::from_slice(&read(&world.join("CURRENT.json"), 32768)?).map_err(err)?;
    let slot = current["slot"]
        .as_u64()
        .filter(|n| *n <= 1)
        .ok_or("scale_sim_root_slot")?;
    let root_bytes = read(&world.join(format!("root-{slot}.json")), 65536)?;
    if current["root_sha256"] != sha256(&root_bytes) {
        return Err("scale_sim_root_hash".into());
    }
    let root: Value = serde_json::from_slice(&root_bytes).map_err(err)?;
    if root["checkpoint_layout"] != 2 {
        return Err("scale_sim_layout".into());
    }
    let point = &root["checkpoint_v2"];
    let generation = point["generation"].as_u64().ok_or("scale_sim_generation")?;
    let bytes = read(&world.join(format!("world-index-{generation}.json")), 32768)?;
    if point["sha256"] != sha256(&bytes) {
        return Err("scale_sim_manifest_hash".into());
    }
    let manifest: Value = serde_json::from_slice(&bytes).map_err(err)?;
    let point = &manifest["current"];
    if point["generation"] != generation || point["sequence"] != root["sequence"] {
        return Err("scale_sim_not_at_saved_boundary".into());
    }
    let bytes = read(
        &world.join(format!("world-{generation}.json")),
        4 * 1024 * 1024,
    )?;
    if point["bytes"] != bytes.len() || point["sha256"] != sha256(&bytes) {
        return Err("scale_sim_world_hash".into());
    }
    Ok((bytes, root))
}
pub fn run(dir: &Path) -> Result<Value, String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    let result = run_inner(&dir);
    if let Err(error) = &result {
        write_json(
            &dir.join("failure.json"),
            &json!({"error":error,"performance_acceptance":"not_run","eight_hour_gate":"not_run","memory_16gb_gate":"not_run"}),
        )?;
    }
    result
}
fn run_inner(dir: &Path) -> Result<Value, String> {
    // CPU and persisted branches run separately; no joint reference residency claim.
    let mut golden = BufWriter::new(File::create_new(dir.join("cpu-targets.jsonl")).map_err(err)?);
    let (reference, oracle) = scale_reference::run_observed(|r, step| {
        if let Step::Drive(target) = step {
            let air = r
                .engine
                .background()
                .and_then(|b| b.field())
                .ok_or("scale_sim_cpu_field")?;
            serde_json::to_writer(&mut golden,&json!({"target":target,"business_sha256":business_digest(&r.engine)?,"air_sha256":sha256(&air.checkpoint()?),"windows":r.windows})).map_err(err)?;
            golden.write_all(b"\n").map_err(err)?;
        }
        Ok(())
    })?;
    golden.flush().map_err(err)?;
    golden.get_ref().sync_all().map_err(err)?;
    let cpu_bytes = reference.engine.export_checkpoint()?;
    let mut f = File::create_new(dir.join("cpu-final-engine.json")).map_err(err)?;
    f.write_all(&cpu_bytes).map_err(err)?;
    f.sync_all().map_err(err)?;
    write_json(&dir.join("cpu-report.json"), &oracle)?;
    drop(reference);
    drop(cpu_bytes);
    drop(golden);
    let exe = std::env::current_exe().map_err(err)?;
    let world: PathBuf = dir.join("world");
    let mut client = Client::spawn_scale_full(&exe, &world, true, false)?;
    if client.next(Duration::from_secs(35))?.kind != "ready" {
        return Err("scale_sim_not_ready".into());
    }
    let mut e = Experiment {
        client,
        id: 0,
        log: BufWriter::with_capacity(
            65536,
            File::create_new(dir.join("transcript.jsonl")).map_err(err)?,
        ),
        log_bytes: 0,
        began: Instant::now(),
        windows: 0,
        verified: 0,
        max_boundary_seconds: 0.0,
        failed_attempts: 0,
    };
    let mut lines = BufReader::new(File::open(dir.join("cpu-targets.jsonl")).map_err(err)?).lines();
    let mut first = None;
    let mut first_proof = None;
    let mut hours = 0;
    scale_reference::walk(|step| {
        match step {
            Step::Action(action) => {
                let envelope = e.raw(vec![action])?;
                if first.is_none() {
                    first_proof = Some(e.call(Command::ReceiptProof {
                        operation: envelope.operation_id,
                    })?);
                    first = Some(envelope);
                }
            }
            Step::Drive(target) => {
                let line = lines
                    .next()
                    .ok_or("scale_sim_missing_cpu_target")?
                    .map_err(err)?;
                if line.len() > 4096 {
                    return Err("scale_sim_cpu_line_bound".into());
                }
                e.drive(target, &serde_json::from_str::<Value>(&line).map_err(err)?)?;
            }
            Step::HourComplete => hours += 1,
            Step::Initialized | Step::PeakShipments => (),
        }
        Ok(())
    })?;
    if lines.next().is_some() || hours != 720 || e.verified != 723 || e.windows != 724 {
        return Err(format!(
            "scale_sim_target_counts:{hours}:{}:{}",
            e.verified, e.windows
        ));
    }
    let work_seconds = e.began.elapsed().as_secs_f64();
    e.call(Command::Checkpoint)?;
    let saved = e.call(Command::Status)?;
    e.client.stop()?;
    let (before_bytes, root) = image(&world)?;
    let before_sha = sha256(&before_bytes);
    drop(before_bytes);
    let cold_started = Instant::now();
    e.client = Client::spawn_scale_full(&exe, &world, false, false)?;
    let ready = e.client.next(Duration::from_secs(35))?;
    if ready.kind != "ready" {
        return Err("scale_sim_cold_ready".into());
    }
    let cold_seconds = cold_started.elapsed().as_secs_f64();
    let reopened = e.call(Command::Status)?;
    for key in [
        "sequence",
        "business_sha256",
        "timeline",
        "background",
        "state_sha256",
    ] {
        if saved[key] != reopened[key] {
            return Err(format!("scale_sim_cold_changed:{key}"));
        }
    }
    let first = first.ok_or("scale_sim_first")?;
    if e.call(Command::ReceiptProof {
        operation: first.operation_id,
    })? != first_proof.ok_or("scale_sim_first_proof")?
    {
        return Err("scale_sim_original_changed".into());
    }
    let retry = e.call(Command::Execute { envelope: first })?;
    let after = e.call(Command::Status)?;
    if retry["has_persistent_receipt"] != true
        || retry["model"].get("Ok").is_none()
        || after["sequence"] != saved["sequence"]
        || after["state_sha256"] != saved["state_sha256"]
    {
        return Err("scale_sim_retry_changed".into());
    }
    e.call(Command::Checkpoint)?;
    e.client.stop()?;
    let (actual_bytes, final_root) = image(&world)?;
    if sha256(&actual_bytes) != before_sha {
        return Err("scale_sim_saved_image_changed".into());
    }
    let actual = Engine::restore_checkpoint(&actual_bytes)?;
    let expected =
        Engine::restore_checkpoint(&read(&dir.join("cpu-final-engine.json"), 4 * 1024 * 1024)?)?;
    if actual.business_comparison_checkpoint()? != expected.business_comparison_checkpoint()?
        || actual
            .background()
            .and_then(|b| b.field())
            .ok_or("scale_sim_actual_field")?
            .checkpoint()?
            != expected
                .background()
                .and_then(|b| b.field())
                .ok_or("scale_sim_expected_field")?
                .checkpoint()?
    {
        return Err("scale_sim_full_final_comparison".into());
    }
    e.log.flush().map_err(err)?;
    e.log.get_ref().sync_all().map_err(err)?;
    let report = json!({"schema":"mycity-t13a-full-sim-assembly-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"opening_sha256":sha256(scale_reference::OPENING),"policy_sha256":sha256(scale_reference::POLICY),"oracle_sha256":sha256(scale_reference::ORACLE),"scope":"full_real_30day_sim_worker_v2_store_diagnostic_only","cpu_oracle":oracle,"completed_hour_targets":hours,"verified_drive_targets":e.verified,"actual_worker_windows":e.windows,"attempt_failures":e.failed_attempts,"business_sha256":business_digest(&actual)?,"final_air_sha256":after["background"]["state_sha256"],"sim_work_wall_seconds_including_ipc_and_persistence":work_seconds,"maximum_complete_drive_seconds":e.max_boundary_seconds,"cold_ready_seconds":cold_seconds,"oldest_receipt_retry":"passed_without_side_effect","complete_final_business_and_air":"passed","cold_complete_image_unchanged":true,"saved_sequence":root["sequence"],"final_root":final_root,"transcript_bytes":e.log_bytes,"performance_acceptance":"not_run","steady_three_runs":"not_run","eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    write_json(&dir.join("report.json"), &report)?;
    Ok(report)
}
