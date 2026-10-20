//! Full frozen input through the existing sole-writer Sim and worker, not an 8h gate.
use crate::{
    scale_model_task::{self, Mode},
    scale_reference::{self, Step, Tier},
    sim_client::Client,
    sim_host::Command,
};
use mycity_core::{
    read_bounded,
    schedule::Phase,
    sha256,
    timeline::Command as Timeline,
    transaction::{Action, Envelope},
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
/// Reject before growing the encoded diagnostic buffer beyond 128KiB.
struct LimitedBytes(Vec<u8>);
impl LimitedBytes {
    fn new() -> Self {
        Self(Vec::with_capacity(128 * 1024))
    }
}
impl Write for LimitedBytes {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(b.len()) > 128 * 1024 {
            return Err(std::io::Error::other("scale_sim_log_record_bound"));
        }
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Hash the same serde wire bytes without retaining a second payload buffer.
fn payload_digest(value: &Value) -> Result<(String, usize), String> {
    struct Sink {
        hash: Sha256,
        bytes: usize,
    }
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.hash.update(b);
            self.bytes += b.len();
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink {
        hash: Sha256::new(),
        bytes: 0,
    };
    serde_json::to_writer(&mut sink, value).map_err(err)?;
    Ok((format!("{:x}", sink.hash.finalize()), sink.bytes))
}
/// Unlike BufRead::lines(), enforce the bound before allocating the whole line.
pub(crate) fn target_line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut line = Vec::with_capacity(4096);
    loop {
        let available = reader.fill_buf().map_err(err)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err("scale_sim_truncated_cpu_line".into())
            };
        }
        let n = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |n| n + 1);
        if line.len().saturating_add(n) > 4096 {
            return Err("scale_sim_cpu_line_bound".into());
        }
        let done = available[n - 1] == b'\n';
        line.extend_from_slice(&available[..n]);
        reader.consume(n);
        if done {
            return Ok(Some(line));
        }
    }
}
pub(crate) struct Experiment {
    pub(crate) client: Client,
    pub(crate) id: u64,
    log: BufWriter<File>,
    log_bytes: usize,
    began: Instant,
    windows: u64,
    verified: u64,
    max_boundary_seconds: f64,
    failed_attempts: u64,
}
impl Experiment {
    pub(crate) fn record(&mut self, value: Value) -> Result<(), String> {
        #[derive(Serialize)]
        struct Row<'a> {
            elapsed_seconds: f64,
            data: &'a Value,
        }
        let row = Row {
            elapsed_seconds: self.began.elapsed().as_secs_f64(),
            data: &value,
        };
        let mut bytes = LimitedBytes::new();
        serde_json::to_writer(&mut bytes, &row).map_err(err)?;
        if self.log_bytes + bytes.0.len() + 1 > 128 * 1024 * 1024 {
            return Err("scale_sim_log_bound".into());
        }
        self.log.write_all(&bytes.0).map_err(err)?;
        self.log.write_all(b"\n").map_err(err)?;
        self.log.flush().map_err(err)?;
        self.log_bytes += bytes.0.len() + 1;
        Ok(())
    }
    pub(crate) fn call(&mut self, command: Command) -> Result<Value, String> {
        self.id = self.id.checked_add(1).ok_or("scale_sim_id")?;
        self.record(json!({"request":self.id,"command":command}))?;
        self.client.submit(self.id, command)?;
        let started = Instant::now();
        for _ in 0..16384 {
            let mut event = self.client.next(Duration::from_secs(35))?;
            if event.kind == "attempt_failed" {
                self.failed_attempts += 1;
            }
            let (payload_sha256, payload_bytes) = payload_digest(&event.payload)?;
            self.record(json!({"response":self.id,"kind":event.kind,"seconds":started.elapsed().as_secs_f64(),"payload_sha256":payload_sha256,"payload_bytes":payload_bytes}))?;
            if event.kind == "result" {
                return event
                    .payload
                    .as_object_mut()
                    .and_then(|object| object.remove("Ok"))
                    .ok_or_else(|| format!("scale_sim_result:{}", event.payload));
            }
        }
        Err("scale_sim_event_bound".into())
    }
    pub(crate) fn raw(&mut self, actions: Vec<Action>) -> Result<Envelope, String> {
        let state = self.call(Command::Progress)?;
        self.raw_at(&state, actions)
    }
    pub(crate) fn raw_at(
        &mut self,
        state: &Value,
        actions: Vec<Action>,
    ) -> Result<Envelope, String> {
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
    pub(crate) fn drive(&mut self, target: u64, expected: &Value) -> Result<(), String> {
        let started = Instant::now();
        for _ in 0..10000 {
            let state = self.call(Command::Progress)?;
            if state["timeline"]["target"].is_null() && state["timeline"]["last_complete"] == target
            {
                let state = self.call(Command::Status)?;
                if !state["timeline"]["target"].is_null()
                    || state["timeline"]["last_complete"] != target
                {
                    return Err("scale_sim_complete_changed".into());
                }
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
                self.record(json!({"verified_target":target,"boundary_seconds":seconds,"expected":expected,"actual_business_sha256":state["business_sha256"],"actual_air_sha256":state["background"]["state_sha256"],"sequence":state["sequence"],"capacity":state["capacity"]}))?;
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
                self.raw_at(
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
        Err("scale_sim_drive_bound".into())
    }
}
/// Read only after the Sim has stopped, verifying the exact CURRENT -> root -> image chain.
pub(crate) fn image(world: &Path) -> Result<(Vec<u8>, Value), String> {
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
    run_tier(dir, Tier::Full)
}
pub fn run_tier(dir: &Path, tier: Tier) -> Result<Value, String> {
    run_measured(dir, tier, false)
}
pub fn run_measured(dir: &Path, tier: Tier, measured: bool) -> Result<Value, String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    if measured {
        write_json(
            &dir.join("manifest.json"),
            &json!({"external_owned_charge_bytes":crate::diagnostic_contract::SIM+crate::diagnostic_contract::WORKER+crate::diagnostic_contract::DRIVER+crate::diagnostic_contract::HELPER,
            "component_caps":crate::diagnostic_contract::caps(),
            "scope":"standalone: Sim2.5 + worker1 + driver1 + helper0.25 MiB; observer separate; model/native/stack separate",
            "budget_acceptance":"not_accepted_until_measured_component_contracts"}),
        )?;
    }
    let result = run_inner(&dir, tier, measured);
    if let Err(error) = &result {
        write_json(
            &dir.join("failure.json"),
            &json!({"error":error,"performance_acceptance":"not_run","eight_hour_gate":"not_run","memory_16gb_gate":"not_run"}),
        )?;
    }
    result
}
fn spawn(
    exe: &Path,
    world: &Path,
    create: bool,
    tier: Tier,
    measured: bool,
) -> Result<Client, String> {
    if measured {
        Client::spawn_scale_measured(exe, world, create, false, tier)
    } else {
        Client::spawn_scale(exe, world, create, false, tier)
    }
}
fn run_inner(dir: &Path, tier: Tier, measured: bool) -> Result<Value, String> {
    // Model proof children remain in the observed process tree and budget.
    scale_model_task::run(dir, tier, Mode::Reference)?;
    let oracle: Value =
        serde_json::from_slice(&read(&dir.join("cpu-report.json"), 65536)?).map_err(err)?;
    let exe = std::env::current_exe().map_err(err)?;
    let world: PathBuf = dir.join("world");
    let mut client = spawn(&exe, &world, true, tier, measured)?;
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
    let mut lines = BufReader::new(File::open(dir.join("cpu-targets.jsonl")).map_err(err)?);
    let mut first = None;
    let mut first_proof = None;
    let mut hours = 0;
    scale_reference::walk_tier(tier, |step| {
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
                let line = target_line(&mut lines)?.ok_or("scale_sim_missing_cpu_target")?;
                e.drive(
                    target,
                    &serde_json::from_slice::<Value>(&line).map_err(err)?,
                )?;
            }
            Step::HourComplete => hours += 1,
            Step::Initialized | Step::PeakShipments => (),
        }
        Ok(())
    })?;
    if target_line(&mut lines)?.is_some() || hours != 720 || e.verified != 723 || e.windows != 724 {
        return Err(format!(
            "scale_sim_target_counts:{hours}:{}:{}",
            e.verified, e.windows
        ));
    }
    let work_seconds = e.began.elapsed().as_secs_f64();
    e.call(Command::Checkpoint)?;
    let saved = e.call(Command::Status)?;
    e.client.stop()?;
    let before = scale_model_task::run(dir, tier, Mode::BeforeReopen)?;
    let saved_sequence = before.root.ok_or("scale_sim_before_root")?.sequence;
    let cold_started = Instant::now();
    e.client = spawn(&exe, &world, false, tier, measured)?;
    let ready = e.client.next(Duration::from_secs(35))?;
    if ready.kind != "ready" {
        return Err("scale_sim_cold_ready".into());
    }
    drop(ready);
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
    drop(reopened);
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
    drop(retry);
    drop(saved);
    let final_air_sha256 = after["background"]["state_sha256"]
        .as_str()
        .ok_or("scale_sim_final_air_hash")?
        .to_owned();
    drop(after);
    e.call(Command::Checkpoint)?;
    e.client.stop()?;
    let compared = scale_model_task::run(dir, tier, Mode::Compare)?;
    let stamp = compared.root.ok_or("scale_sim_final_root")?;
    let root_bytes = read(&dir.join("compare/root.json"), 8192)?;
    if root_bytes.len() as u64 != stamp.bytes || sha256(&root_bytes) != stamp.sha256.as_str() {
        return Err("scale_sim_final_root_asset".into());
    }
    let final_root: Value = serde_json::from_slice(&root_bytes).map_err(err)?;
    drop(root_bytes);
    let final_business_sha256 = compared.business_sha256.ok_or("scale_sim_final_business")?;
    e.log.flush().map_err(err)?;
    e.log.get_ref().sync_all().map_err(err)?;
    let report = json!({"schema":"mycity-t13a-full-sim-assembly-v1","compute_diagnostics_enabled":measured,"source_sha256":env!("MYCITY_SOURCE_SHA256"),"opening_sha256":sha256(tier.opening()),"policy_sha256":sha256(tier.policy()),"oracle_sha256":sha256(tier.oracle()),"tier":tier.name(),"scope":"named_tier_real_30day_sim_worker_v2_store_diagnostic_only","cpu_oracle":oracle,"completed_hour_targets":hours,"verified_drive_targets":e.verified,"actual_worker_windows":e.windows,"attempt_failures":e.failed_attempts,"business_sha256":final_business_sha256,"final_air_sha256":final_air_sha256,"sim_work_wall_seconds_including_ipc_and_persistence":work_seconds,"maximum_complete_drive_seconds":e.max_boundary_seconds,"cold_ready_seconds":cold_seconds,"oldest_receipt_retry":"passed_without_side_effect","complete_final_business_and_air":"passed","cold_complete_image_unchanged":true,"saved_sequence":saved_sequence,"final_root":final_root,"transcript_bytes":e.log_bytes,"performance_acceptance":"not_run","steady_three_runs":"not_run","eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    write_json(&dir.join("report.json"), &report)?;
    Ok(report)
}

/// Real full-profile process experiment for independently restored diagnostic branches.
pub(crate) fn independent_experiment(
    dir: &Path,
    create: bool,
    measured: bool,
) -> Result<Experiment, String> {
    independent_experiment_options(dir, create, measured, false)
}
pub(crate) fn independent_experiment_options(
    dir: &Path,
    create: bool,
    measured: bool,
    probe: bool,
) -> Result<Experiment, String> {
    let exe = std::env::current_exe().map_err(err)?;
    let mut client = if measured {
        Client::spawn_scale_measured(&exe, &dir.join("world"), create, probe, Tier::Full)?
    } else {
        Client::spawn_scale(&exe, &dir.join("world"), create, probe, Tier::Full)?
    };
    if client.next(Duration::from_secs(35))?.kind != "ready" {
        return Err("rare_not_ready".into());
    }
    Ok(Experiment {
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
    })
}
impl Experiment {
    pub(crate) fn save_stop(&mut self) -> Result<(), String> {
        self.call(Command::Checkpoint)?;
        self.call(Command::Stop)?;
        self.client.stop()?;
        self.log.flush().map_err(err)?;
        self.log.get_ref().sync_all().map_err(err)?;
        if self.failed_attempts != 0 {
            return Err("rare_failed_worker_attempt".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;
    #[test]
    fn streaming_payload_digest_preserves_exact_old_wire_hash_and_length() {
        for value in [
            Value::Null,
            json!({"nested":["中文\n\"",{ "number": -31415, "flag":true}],"array":[0,1,2]}),
            json!("x".repeat(60000)),
        ] {
            let bytes = serde_json::to_vec(&value).unwrap();
            assert_eq!(
                payload_digest(&value).unwrap(),
                (sha256(&bytes), bytes.len())
            );
        }
    }
    #[test]
    fn diagnostic_buffer_refuses_growth_before_limit_and_keeps_previous_bytes() {
        let mut b = LimitedBytes::new();
        b.write_all(&vec![7; 70000]).unwrap();
        b.write_all(&vec![7; 60000]).unwrap();
        assert_eq!(b.0.capacity(), 128 * 1024);
        for _ in 0..1072 {
            b.write_all(&[7]).unwrap();
        }
        assert_eq!(b.0.capacity(), 128 * 1024);
        let cap = b.0.capacity();
        assert_eq!(b.0.len(), 128 * 1024);
        assert!(b.write_all(&[8]).is_err());
        assert_eq!(b.0.capacity(), cap);
        assert!(b.0.iter().all(|v| *v == 7));
    }
    #[test]
    fn golden_line_reader_is_bounded_before_allocation_and_rejects_truncation() {
        let mut reader = BufReader::with_capacity(
            17,
            std::io::Cursor::new(b"{\"target\":60}\n{\"target\":180}\n"),
        );
        assert_eq!(
            target_line(&mut reader).unwrap().unwrap(),
            b"{\"target\":60}\n"
        );
        assert_eq!(
            target_line(&mut reader).unwrap().unwrap(),
            b"{\"target\":180}\n"
        );
        assert!(target_line(&mut reader).unwrap().is_none());
        let mut reader = BufReader::with_capacity(17, std::io::Cursor::new(vec![b'x'; 65536]));
        assert_eq!(
            target_line(&mut reader).unwrap_err(),
            "scale_sim_cpu_line_bound"
        );
        assert!(std::io::Seek::stream_position(&mut reader).unwrap() <= 4096);
        let mut reader = std::io::Cursor::new(b"no newline");
        assert_eq!(
            target_line(&mut reader).unwrap_err(),
            "scale_sim_truncated_cpu_line"
        );
        for chunk in [3, 17, 2051, 4000, 7000] {
            let mut input = vec![b'x'; 4095];
            input.push(b'\n');
            let mut reader = BufReader::with_capacity(chunk, std::io::Cursor::new(input));
            let line = target_line(&mut reader).unwrap().unwrap();
            assert_eq!(line.len(), 4096);
            assert_eq!(line.capacity(), 4096);
        }
        let mut edge = vec![b'x'; 4095];
        edge.push(b'\n');
        assert_eq!(
            target_line(&mut std::io::Cursor::new(edge))
                .unwrap()
                .unwrap()
                .len(),
            4096
        );
    }
}
