//! Full frozen input through the existing sole-writer Sim and worker, not an 8h gate.
use crate::{
    scale_model_task::{self, Mode},
    scale_reference::{self, Step, Tier},
    sim_client::Client,
    sim_host::{Command, Event},
};
use mycity_core::{
    read_bounded,
    schedule::Phase,
    sha256,
    timeline::Command as Timeline,
    transaction::{Action, Envelope},
};
use serde::{Deserialize, Serialize};
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
struct LimitedBytes(Vec<u8>, usize);
impl LimitedBytes {
    #[cfg(test)]
    fn new() -> Self {
        Self::bounded(128 * 1024, 128 * 1024)
    }
    fn bounded(capacity: usize, limit: usize) -> Self {
        Self(Vec::with_capacity(capacity.min(limit)), limit)
    }
}
impl Write for LimitedBytes {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(b.len()) > self.1 {
            return Err(std::io::Error::other("scale_sim_log_record_bound"));
        }
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
const MAX_SIM_OPEN_DIAGNOSTIC_BYTES: usize = 4096;
const MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticArenaPoint {
    clock_begin: crate::heap_meter::ClockReading,
    role_arena: mycity_role_allocator::Snapshot,
    clock_end: crate::heap_meter::ClockReading,
}

#[derive(Deserialize)]
struct DiagnosticOpenWire {
    schema: String,
    operation: String,
    pre_open: DiagnosticArenaPoint,
    post_open: DiagnosticArenaPoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticSlotBytes {
    owned_model_bytes: usize,
    retained_prepare_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticWorkspaceWire {
    active_slot: DiagnosticSlotBytes,
    spare_restore_slot: DiagnosticSlotBytes,
    candidate_owned_bytes: usize,
    codec_backing_bytes: usize,
    workspace_owned_bytes: usize,
}

#[derive(Debug, Serialize)]
struct SimOpenDiagnosticReceipt {
    schema: &'static str,
    response_id: u64,
    response_kind: &'static str,
    sim_pid: u32,
    sim_birth_unix_ns: u128,
    event_clock: crate::heap_meter::ClockReading,
    operation: &'static str,
    pre_open: DiagnosticArenaPoint,
    post_open: DiagnosticArenaPoint,
    model_workspace: DiagnosticWorkspaceWireReceipt,
    store_owned_bytes: usize,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct DiagnosticWorkspaceWireReceipt {
    active_slot: DiagnosticSlotBytes,
    spare_restore_slot: DiagnosticSlotBytes,
    candidate_owned_bytes: usize,
    codec_backing_bytes: usize,
    workspace_owned_bytes: usize,
}

fn same_sim_instance(clock: crate::heap_meter::ClockReading, pid: u32, birth: u128) -> bool {
    clock.pid == pid && clock.instance_unix_ns == birth
}

fn remember_sim_instance_once(
    seen: &mut [Option<(u32, u128)>; MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES],
    count: &mut usize,
    key: (u32, u128),
) -> Result<bool, String> {
    if seen.iter().flatten().any(|prior| *prior == key) {
        return Ok(false);
    }
    if *count >= MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES {
        return Err("scale_sim_open_diagnostic_instance_limit".into());
    }
    seen[*count] = Some(key);
    *count += 1;
    Ok(true)
}

fn sim_open_diagnostic_receipt(
    runtime: &Value,
    event: &Event,
    response_id: u64,
    response_kind: &'static str,
) -> Result<Option<SimOpenDiagnosticReceipt>, String> {
    let Some(open_value) = runtime.get("sim_open_memory_diagnostic") else {
        // Old protocol/source revisions have no diagnostic. Preserve their existing behavior.
        return Ok(None);
    };
    let open: DiagnosticOpenWire = serde_json::from_value(open_value.clone()).map_err(err)?;
    if open.schema != "mycity-sim-open-memory-v1" {
        return Err("scale_sim_open_diagnostic_schema".into());
    }
    let operation = match open.operation.as_str() {
        "open" => "open",
        "create" => "create",
        _ => return Err("scale_sim_open_diagnostic_operation".into()),
    };
    let workspace: DiagnosticWorkspaceWire = serde_json::from_value(
        runtime
            .get("sim_model_workspace")
            .cloned()
            .ok_or("scale_sim_open_diagnostic_workspace_missing")?,
    )
    .map_err(err)?;
    let store_owned_bytes = runtime
        .get("store_owned_bytes")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("scale_sim_open_diagnostic_store_capacity_missing")?;
    let event_clock = event
        .clock
        .ok_or("scale_sim_open_diagnostic_event_clock_missing")?;
    let pid = event.sim_pid;
    let birth = open.pre_open.clock_begin.instance_unix_ns;
    let clocks = [
        open.pre_open.clock_begin,
        open.pre_open.clock_end,
        open.post_open.clock_begin,
        open.post_open.clock_end,
        event_clock,
    ];
    if pid <= 1
        || clocks.iter().any(|c| !same_sim_instance(*c, pid, birth))
        || open.pre_open.clock_begin.elapsed_ns > open.pre_open.clock_end.elapsed_ns
        || open.pre_open.clock_end.elapsed_ns > open.post_open.clock_begin.elapsed_ns
        || open.post_open.clock_begin.elapsed_ns > open.post_open.clock_end.elapsed_ns
        || open.post_open.clock_end.elapsed_ns > event_clock.elapsed_ns
    {
        return Err("scale_sim_open_diagnostic_clock_identity_or_order".into());
    }
    Ok(Some(SimOpenDiagnosticReceipt {
        schema: "mycity-sim-open-diagnostic-receipt-v1",
        response_id,
        response_kind,
        sim_pid: pid,
        sim_birth_unix_ns: birth,
        event_clock,
        operation,
        pre_open: open.pre_open,
        post_open: open.post_open,
        model_workspace: DiagnosticWorkspaceWireReceipt {
            active_slot: workspace.active_slot,
            spare_restore_slot: workspace.spare_restore_slot,
            candidate_owned_bytes: workspace.candidate_owned_bytes,
            codec_backing_bytes: workspace.codec_backing_bytes,
            workspace_owned_bytes: workspace.workspace_owned_bytes,
        },
        store_owned_bytes,
    }))
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
    sim_open_diagnostic_instances: [Option<(u32, u128)>; MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES],
    sim_open_diagnostic_count: usize,
}
impl Experiment {
    pub(crate) fn clock_origin(&self) -> Instant {
        self.began
    }
    pub(crate) fn record(&mut self, value: Value) -> Result<(), String> {
        self.record_bounded(value, 128 * 1024, 128 * 1024)
    }
    fn record_bounded(
        &mut self,
        value: Value,
        initial_capacity: usize,
        maximum_row_bytes: usize,
    ) -> Result<(), String> {
        #[derive(Serialize)]
        struct Row<'a> {
            elapsed_seconds: f64,
            data: &'a Value,
        }
        let row = Row {
            elapsed_seconds: self.began.elapsed().as_secs_f64(),
            data: &value,
        };
        let mut bytes = LimitedBytes::bounded(initial_capacity, maximum_row_bytes);
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
    fn record_open_diagnostic(
        &mut self,
        event: &Event,
        runtime: Option<&Value>,
        response_id: u64,
        response_kind: &'static str,
    ) -> Result<(), String> {
        let Some(runtime) = runtime else {
            return Ok(());
        };
        if runtime.get("sim_open_memory_diagnostic").is_none() {
            // Missing field is an older source/protocol and remains compatible.
            return Ok(());
        }
        let event_clock = event
            .clock
            .ok_or("scale_sim_open_diagnostic_event_clock_missing")?;
        let key = (event.sim_pid, event_clock.instance_unix_ns);
        if event.sim_pid <= 1 || event_clock.pid != event.sim_pid {
            return Err("scale_sim_open_diagnostic_event_identity".into());
        }
        if self
            .sim_open_diagnostic_instances
            .iter()
            .flatten()
            .any(|seen| *seen == key)
        {
            return Ok(());
        }
        let Some(receipt) =
            sim_open_diagnostic_receipt(runtime, event, response_id, response_kind)?
        else {
            return Ok(());
        };
        if key != (receipt.sim_pid, receipt.sim_birth_unix_ns) {
            return Err("scale_sim_open_diagnostic_event_birth_mismatch".into());
        }
        if self.sim_open_diagnostic_count >= MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES {
            return Err("scale_sim_open_diagnostic_instance_limit".into());
        }
        let row = json!({"sim_open_memory_diagnostic":receipt});
        self.record_bounded(row, 1024, MAX_SIM_OPEN_DIAGNOSTIC_BYTES)?;
        if !remember_sim_instance_once(
            &mut self.sim_open_diagnostic_instances,
            &mut self.sim_open_diagnostic_count,
            key,
        )? {
            return Err("scale_sim_open_diagnostic_duplicate_record_race".into());
        }
        Ok(())
    }
    pub(crate) fn call(&mut self, command: Command) -> Result<Value, String> {
        let status_requested = matches!(&command, Command::Status);
        self.id = self.id.checked_add(1).ok_or("scale_sim_id")?;
        self.record(json!({"request":self.id,"command":command}))?;
        let request_sent_ns = self.began.elapsed().as_nanos();
        self.client.submit(self.id, command)?;
        let started = Instant::now();
        for _ in 0..16384 {
            let mut event = self.client.next(Duration::from_secs(35))?;
            let response_received_ns = self.began.elapsed().as_nanos();
            if event.kind == "attempt_failed" {
                self.failed_attempts += 1;
            }
            let worker_identity = (event.kind == "worker_started").then_some(&event.payload);
            let (payload_sha256, payload_bytes) = payload_digest(&event.payload)?;
            // Preserve only the fixed-size timing summary for successful Status
            // results. The full payload still has its original hash/byte witness;
            // the existing 128 KiB row and 128 MiB transcript bounds also apply.
            let sim_host_timing = (status_requested && event.kind == "result")
                .then(|| event.payload.pointer("/Ok/runtime/sim_host_timing"))
                .flatten();
            if status_requested && event.kind == "result" && event.payload.get("Ok").is_some() {
                self.record_open_diagnostic(
                    &event,
                    event.payload.pointer("/Ok/runtime"),
                    self.id,
                    "result",
                )?;
            }
            self.record(json!({"response":self.id,"kind":event.kind,"seconds":started.elapsed().as_secs_f64(),"payload_sha256":payload_sha256,"payload_bytes":payload_bytes,"request_sent_ns":request_sent_ns,"response_received_ns":response_received_ns,"sim_clock":event.clock,"worker_identity":worker_identity,"sim_host_timing":sim_host_timing,"result_ok":if event.kind=="result" {Some(event.payload.get("Ok").is_some())} else {None}}))?;
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
            &json!({"external_owned_charge_bytes":crate::diagnostic_contract::WORKER+crate::diagnostic_contract::DRIVER,
            "component_caps":crate::diagnostic_contract::caps(),
            "scope":"standalone: external worker1 + driver1 MiB; Sim/helper controls inside fixed full-role Rust arenas; native/stack separate",
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
    let ready = client.next(Duration::from_secs(35))?;
    if ready.kind != "ready" {
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
        sim_open_diagnostic_instances: [None; MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES],
        sim_open_diagnostic_count: 0,
    };
    e.record_open_diagnostic(&ready, ready.payload.get("runtime"), 0, "ready")?;
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
    independent_experiment_for_tier(dir, create, measured, probe, Tier::Full)
}
/// Same real experiment setup, using the explicitly selected scale profile.
pub(crate) fn independent_experiment_for_tier(
    dir: &Path,
    create: bool,
    measured: bool,
    probe: bool,
    tier: Tier,
) -> Result<Experiment, String> {
    let exe = std::env::current_exe().map_err(err)?;
    let mut client = if measured {
        Client::spawn_scale_measured(&exe, &dir.join("world"), create, probe, tier)?
    } else {
        Client::spawn_scale(&exe, &dir.join("world"), create, probe, tier)?
    };
    let ready = client.next(Duration::from_secs(35))?;
    if ready.kind != "ready" {
        return Err("rare_not_ready".into());
    }
    let mut experiment = Experiment {
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
        sim_open_diagnostic_instances: [None; MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES],
        sim_open_diagnostic_count: 0,
    };
    experiment.record_open_diagnostic(&ready, ready.payload.get("runtime"), 0, "ready")?;
    Ok(experiment)
}
impl Experiment {
    pub(crate) fn counts(&self) -> (u64, u64, u64) {
        (self.windows, self.verified, self.failed_attempts)
    }
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
    fn clock(pid: u32, birth: u128, elapsed_ns: u64) -> crate::heap_meter::ClockReading {
        crate::heap_meter::ClockReading {
            pid,
            instance_unix_ns: birth,
            elapsed_ns,
        }
    }

    fn diagnostic_event() -> (Event, Value) {
        let pid = 71;
        let birth = 9001;
        let arena = json!({
            "activation": null,
            "current_requested_bytes": 100,
            "lifetime_peak_requested_bytes": 200,
            "bootstrap_live_bytes": 0,
            "bootstrap_peak_bytes": 0,
            "arena_used_bytes": 100,
            "arena_usable_bytes": 4096,
            "bootstrap_quarantined_bytes": 0,
            "arena_free_bytes": 3996,
            "allocation_failures": 0
        });
        let point = |begin, end| {
            json!({
                "clock_begin":clock(pid, birth, begin),
                "role_arena":arena.clone(),
                "clock_end":clock(pid, birth, end)
            })
        };
        let runtime = json!({
            "sim_open_memory_diagnostic": {
                "schema":"mycity-sim-open-memory-v1",
                "operation":"open",
                "pre_open":point(10, 11),
                "post_open":point(20, 21)
            },
            "sim_model_workspace": {
                "active_slot":{"owned_model_bytes":1000,"retained_prepare_bytes":300},
                "spare_restore_slot":{"owned_model_bytes":900,"retained_prepare_bytes":250},
                "candidate_owned_bytes":400,
                "codec_backing_bytes":500,
                "workspace_owned_bytes":600
            },
            "store_owned_bytes":700
        });
        let event = Event {
            protocol: "mycity-sim-v1".into(),
            id: 0,
            sim_pid: pid,
            clock: Some(clock(pid, birth, 30)),
            kind: "ready".into(),
            payload: json!({"runtime":runtime.clone()}),
        };
        (event, runtime)
    }

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
    fn diagnostic_receipt_writer_enforces_its_small_row_limit() {
        let mut b = LimitedBytes::bounded(1024, MAX_SIM_OPEN_DIAGNOSTIC_BYTES);
        b.write_all(&vec![3; MAX_SIM_OPEN_DIAGNOSTIC_BYTES])
            .unwrap();
        let capacity = b.0.capacity();
        assert!(capacity <= MAX_SIM_OPEN_DIAGNOSTIC_BYTES);
        assert!(b.write_all(&[4]).is_err());
        assert_eq!(b.0.len(), MAX_SIM_OPEN_DIAGNOSTIC_BYTES);
        assert_eq!(b.0.capacity(), capacity);
        assert!(b.0.iter().all(|byte| *byte == 3));
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

    #[test]
    fn sim_open_diagnostic_extracts_bounded_first_instance_receipt() {
        let (event, runtime) = diagnostic_event();
        let receipt = sim_open_diagnostic_receipt(&runtime, &event, 0, "ready")
            .unwrap()
            .unwrap();
        assert_eq!(receipt.sim_pid, event.sim_pid);
        assert_eq!(receipt.sim_birth_unix_ns, 9001);
        assert_eq!(receipt.operation, "open");
        assert_eq!(receipt.model_workspace.active_slot.owned_model_bytes, 1000);
        assert_eq!(
            receipt
                .model_workspace
                .spare_restore_slot
                .retained_prepare_bytes,
            250
        );
        assert_eq!(receipt.store_owned_bytes, 700);
        assert!(serde_json::to_vec(&receipt).unwrap().len() < MAX_SIM_OPEN_DIAGNOSTIC_BYTES);
    }

    #[test]
    fn old_protocol_without_open_diagnostic_is_ignored_without_zero_fill() {
        let (event, _) = diagnostic_event();
        assert!(sim_open_diagnostic_receipt(&json!({}), &event, 1, "result")
            .unwrap()
            .is_none());
    }

    #[test]
    fn sim_open_diagnostic_rejects_identity_clock_and_missing_capacity() {
        let (mut event, runtime) = diagnostic_event();
        event.sim_pid += 1;
        assert_eq!(
            sim_open_diagnostic_receipt(&runtime, &event, 0, "ready").unwrap_err(),
            "scale_sim_open_diagnostic_clock_identity_or_order"
        );

        let (event, mut runtime) = diagnostic_event();
        runtime["sim_open_memory_diagnostic"]["post_open"]["clock_begin"]["elapsed_ns"] = 5.into();
        assert_eq!(
            sim_open_diagnostic_receipt(&runtime, &event, 0, "ready").unwrap_err(),
            "scale_sim_open_diagnostic_clock_identity_or_order"
        );

        let (event, mut runtime) = diagnostic_event();
        runtime["sim_model_workspace"]
            .as_object_mut()
            .unwrap()
            .remove("codec_backing_bytes");
        assert!(sim_open_diagnostic_receipt(&runtime, &event, 0, "ready").is_err());
    }

    #[test]
    fn sim_open_diagnostic_has_fixed_instance_limit_and_deduplicates_birth() {
        let mut seen = [None; MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES];
        let mut count = 0;
        assert!(remember_sim_instance_once(&mut seen, &mut count, (71, 9001)).unwrap());
        assert!(!remember_sim_instance_once(&mut seen, &mut count, (71, 9001)).unwrap());
        for n in 1..MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES {
            assert!(
                remember_sim_instance_once(&mut seen, &mut count, (71, 9001 + n as u128)).unwrap()
            );
        }
        assert_eq!(count, MAX_SIM_OPEN_DIAGNOSTIC_INSTANCES);
        assert_eq!(
            remember_sim_instance_once(&mut seen, &mut count, (72, 9002)).unwrap_err(),
            "scale_sim_open_diagnostic_instance_limit"
        );
    }
}
