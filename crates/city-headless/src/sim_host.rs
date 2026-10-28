//! Independent sole-writer Sim. IPC carries bounded commands/results, never world files.
use crate::{
    compute_worker::{self as wire, Request as Work, Stage, Ticket, Worker},
    worker_cache,
};
use mycity_core::{
    background::{self, Command as B, Failure},
    sha256,
    transaction::{Action, Caller, Envelope},
};
use mycity_storage::Store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    cell::Cell,
    io::{BufRead, BufReader, Write},
    path::Path,
    time::{Duration, Instant},
};
#[path = "sim_task_observation.rs"]
mod task_observation;
#[path = "task_execution_protocol.rs"]
mod execution_protocol;
pub use execution_protocol::{BoundTaskRequest,TaskExecutionIntent,TaskExecutionLocation};
const PROTOCOL: &str = "mycity-sim-v1";
// required_registry_v2 service continuation; old worlds may already contain C
// operations. Detect them explicitly instead of treating a historical ticket as
// this page. The original manual Advance remains the collision recovery path.
const SERVICE_CONTINUATION_PREFIX: u64 = 0xc000_0000_0000_0000;

// Fixed-size counters make timing visible without adding a per-request log.
// `Cell` lets scoped timers update the report across early returns and retries.
const TIMING_PHASES: usize = 10;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
struct TimingSample {
    count: u64,
    total_ns: u64,
    max_ns: u64,
}
struct SimTimings {
    samples: [Cell<TimingSample>; TIMING_PHASES],
}
#[derive(Clone, Copy)]
#[repr(usize)]
enum TimingPhase {
    RunRequired = 0,
    CacheRecoverRebindReset = 1,
    InputEncodePublish = 2,
    WorkerSpawnRegisterToFirstResponse = 3,
    WorkerProgressWaitAfterFirst = 4,
    CandidateReadAndImport = 5,
    PersistentAccept = 6,
    StatusAirDigest = 7,
    StatusBusinessDigest = 8,
    StatusStateDigest = 9,
}
impl SimTimings {
    fn new() -> Self {
        Self {
            samples: std::array::from_fn(|_| Cell::new(TimingSample::default())),
        }
    }
    fn timer(&self, phase: TimingPhase) -> TimingGuard<'_> {
        TimingGuard {
            owner: self,
            phase,
            started: Instant::now(),
        }
    }
    fn record(&self, phase: TimingPhase, elapsed: Duration) {
        let index = phase as usize;
        let ns = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
        let mut sample = self.samples[index].get();
        sample.count = sample.count.saturating_add(1);
        sample.total_ns = sample.total_ns.saturating_add(ns);
        sample.max_ns = sample.max_ns.max(ns);
        self.samples[index].set(sample);
    }
    fn report(&self) -> Value {
        let sample = |phase: TimingPhase| self.samples[phase as usize].get();
        json!({
            "schema":"mycity-sim-timing-v1",
            "clock":"elapsed Instant nanoseconds; includes scheduling and synchronous filesystem work",
            "scope_notes":{
                "run_required":"one required command from entry through result construction; excludes writing the outer IPC result frame",
                "cache_recover_rebind_reset":"includes cache preparation, exclusive recovery, RebindLease transaction and reset; excludes prior worker reap",
                "input_encode_publish":"frozen input conversion through checkpoint publication and request construction",
                "worker_spawn_register_submit_to_first_validated_response":"includes spawn, registration, request submission and worker work/wait until first validated progress; not pure spawn time",
                "worker_progress_wait_after_first_response":"later next() waits plus event identity/shape validation; combines worker execution, scheduling and IPC wait",
                "candidate_read_and_import":"candidate asset read/validation and import transaction path",
                "persistent_accept_execute":"Accept execute through model outcome validation and durable receipt check",
                "status_digests":"separate Air checkpoint, business and full-state digest calls"
            },
            "run_required":sample(TimingPhase::RunRequired),
            "cache_recover_rebind_reset":sample(TimingPhase::CacheRecoverRebindReset),
            "input_encode_publish":sample(TimingPhase::InputEncodePublish),
            "worker_spawn_register_submit_to_first_validated_response":sample(TimingPhase::WorkerSpawnRegisterToFirstResponse),
            "worker_progress_wait_after_first_response":sample(TimingPhase::WorkerProgressWaitAfterFirst),
            "candidate_read_and_import":sample(TimingPhase::CandidateReadAndImport),
            "persistent_accept_execute":sample(TimingPhase::PersistentAccept),
            "status_air_checkpoint_digest":sample(TimingPhase::StatusAirDigest),
            "status_business_digest":sample(TimingPhase::StatusBusinessDigest),
            "status_state_digest":sample(TimingPhase::StatusStateDigest)
        })
    }
}
struct TimingGuard<'a> {
    owner: &'a SimTimings,
    phase: TimingPhase,
    started: Instant,
}
impl Drop for TimingGuard<'_> {
    fn drop(&mut self) {
        self.owner.record(self.phase, self.started.elapsed());
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub id: u64,
    pub command: Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Status,
    Progress,
    Execute { envelope: Envelope },
    /// Issue the original Store-owned road proof and settle the complete local day.
    /// The client supplies only its original operation identity, never a proof or quote.
    SettleTownMarket { operation: u64 },
    RunRequired { job: u64 },
    /// Select one real mandatory job or one page of an already accepted P5
    /// boundary. It never accepts a new future Timeline target.
    RunDue,
    DescribeTaskExecution,
    RunBoundTask { binding:BoundTaskRequest },
    RunRequiredSliced { job: u64, plan: String, quota: crate::air_turns::Quota },
    PageControl { control: PageControl },
    ProbeRequired { job: u64, point: ProbePoint },
    ProbeAck { point: ProbePoint, cancel: bool },
    Checkpoint,
    ReceiptProof { operation: u64 },
    ProbeExhaust { envelope: Envelope },
    Stop,
}
/// Production page control, never a fault-probe ACK. All messages retain the
/// original request id and sole Worker owner while the computation is paused.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PageControl {
    Observe,
    ReceiptProof { operation: u64 },
    Continue { next: crate::air_turns::Continue },
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbePoint {
    Leased,
    WorkerSources,
    WorkerCells,
    CandidatePublished,
    Committed,
}
fn gate(
    point: ProbePoint,
    pending: &mut Option<ProbePoint>,
    r: &mut impl BufRead,
    w: &mut impl Write,
    id: u64,
    payload: Value,
) -> Result<bool, String> {
    if *pending != Some(point) {
        return Ok(false);
    }
    *pending = None;
    event(
        w,
        id,
        "probe_barrier",
        json!({"point":point,"details":payload}),
    )?;
    let bytes = wire::frame(r)?.ok_or("sim_probe_ack_eof")?;
    let ack: Request = serde_json::from_slice(&bytes).map_err(err)?;
    if ack.protocol != PROTOCOL || ack.id != id {
        return Err("sim_probe_ack_identity".into());
    }
    match ack.command {
        Command::ProbeAck { point: p, cancel } if p == point => Ok(cancel),
        _ => Err("sim_probe_ack_point".into()),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub protocol: String,
    pub id: u64,
    pub sim_pid: u32,
    #[serde(default)]
    pub clock: Option<crate::heap_meter::ClockReading>,
    pub kind: String,
    pub payload: Value,
}
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn event(w: &mut impl Write, id: u64, kind: &str, payload: Value) -> Result<(), String> {
    wire::write_frame(
        w,
        &Event {
            protocol: PROTOCOL.into(),
            id,
            sim_pid: std::process::id(),
            clock: Some(crate::heap_meter::clock_reading()),
            kind: kind.into(),
            payload,
        },
    )
}
#[derive(Clone, Copy)]
pub(crate) enum Profile {
    Base,
    Joint,
    Scale(crate::scale_reference::Tier),
}
impl Profile {
    pub(crate) fn command(self, probe: bool) -> &'static str {
        match (self, probe) {
            (Self::Base, false) => "sim-host",
            (Self::Base, true) => "sim-host-probe",
            (Self::Joint, false) => "sim-host-joint",
            (Self::Joint, true) => "sim-host-joint-probe",
            (Self::Scale(crate::scale_reference::Tier::Full), false) => "sim-host-scale-full",
            (Self::Scale(crate::scale_reference::Tier::Full), true) => "sim-host-scale-full-probe",
            (Self::Scale(crate::scale_reference::Tier::Small), false) => "sim-host-scale-small",
            (Self::Scale(crate::scale_reference::Tier::Small), true) => {
                "sim-host-scale-small-probe"
            }
            (Self::Scale(crate::scale_reference::Tier::Medium), false) => "sim-host-scale-medium",
            (Self::Scale(crate::scale_reference::Tier::Medium), true) => {
                "sim-host-scale-medium-probe"
            }
        }
    }
    fn rules(self) -> String {
        let name = match self {
            Self::Base => "sim-w3",
            Self::Joint => "sim-w4",
            Self::Scale(crate::scale_reference::Tier::Full) => "sim-t13a-full-v2",
            Self::Scale(crate::scale_reference::Tier::Small) => "sim-t13a-small-v2",
            Self::Scale(crate::scale_reference::Tier::Medium) => "sim-t13a-medium-v2",
        };
        format!("{name}:{}", env!("MYCITY_SOURCE_SHA256"))
    }
}
struct Codec {
    epoch: u64,
    slots: crate::worker_slots::Owner,
    air_input: mycity_core::air::BackgroundStateSlot,
}
impl Codec {
    fn new(epoch: u64) -> Result<Self, String> {
        let slots = crate::worker_slots::Owner::new(epoch, 1, 1, 1)?;
        Ok(Self {
            epoch,
            slots,
            air_input: mycity_core::air::BackgroundStateSlot::new(),
        })
    }
    fn log(&self, diag: &mut Option<crate::cpu_log::CpuLog>) -> Result<(), String> {
        if let Some(log) = diag.as_mut() {
            log.sim_controlled_slots(&self.slots.report()?)?;
        }
        Ok(())
    }
}
fn progress(store: &Store) -> Result<Value, String> {
    let e = store.engine().map_err(err)?;
    Ok(
        json!({"revision":e.world().view().revision,"sequence":store.committed_sequence(),
        "timeline":e.timeline().map(|t|t.progress),
        "background":e.background().map(|b|json!({"version":b.version(),"time":b.time(),
            "pending":b.pending().map(|p|json!({"id":p.id,"generation":p.generation,"incarnation":p.incarnation,"attempt":p.attempt,"failure":p.failure}))}))}),
    )
}
#[derive(Serialize)]
struct SimArenaPoint {
    /// Brackets the allocator snapshot; both readings must identify the same process.
    clock_begin: crate::heap_meter::ClockReading,
    role_arena: mycity_role_allocator::Snapshot,
    clock_end: crate::heap_meter::ClockReading,
}

fn sim_arena_point() -> Result<SimArenaPoint, String> {
    let clock_begin = crate::heap_meter::clock_reading();
    let role_arena = crate::heap_meter::arena_snapshot();
    let clock_end = crate::heap_meter::clock_reading();
    if clock_begin.pid != clock_end.pid
        || clock_begin.instance_unix_ns != clock_end.instance_unix_ns
        || clock_begin.elapsed_ns > clock_end.elapsed_ns
    {
        return Err("sim_arena_snapshot_clock_identity".into());
    }
    Ok(SimArenaPoint {
        clock_begin,
        role_arena,
        clock_end,
    })
}

#[derive(Serialize)]
struct SimOpenMemoryDiagnostic {
    operation: &'static str,
    pre_open: SimArenaPoint,
    post_open: SimArenaPoint,
}

fn status(
    store: &Store,
    worker: &Worker,
    codec: &Codec,
    diag: &Option<crate::cpu_log::CpuLog>,
    timings: &SimTimings,
    open_memory: &SimOpenMemoryDiagnostic,
    task_observations: &task_observation::Recorder,
) -> Result<Value, String> {
    // Same-phase current occupancy, before allocating the status report.
    let allocator_clock_begin = crate::heap_meter::clock_reading();
    let allocator_boundary = crate::heap_meter::arena_snapshot();
    let allocator_clock_end = crate::heap_meter::clock_reading();
    let e = store.engine().map_err(err)?;
    let b = e.background();
    let view = e.world().view();
    let air_sha = {
        let _timer = timings.timer(TimingPhase::StatusAirDigest);
        b.and_then(|b| b.field())
            .map(|f| f.checkpoint_sha256())
            .transpose()?
    };
    let model_workspace = store.model_workspace_identity().map_err(err)?;
    let store_owned_bytes = store.owned_capacity_bytes().map_err(err)?;
    let runtime = json!({"allocator_boundary":{"phase":"status_entry_before_report_allocations","clock_begin":allocator_clock_begin,"clock_end":allocator_clock_end,"snapshot":allocator_boundary},"sim_open_memory_diagnostic":{"schema":"mycity-sim-open-memory-v1","operation":open_memory.operation,"pre_open":open_memory.pre_open,"post_open":open_memory.post_open,"native_phys_footprint_bytes":null,"native_phys_footprint_source":"external_native_observer_required; null means unavailable here, not zero"},"sim_model_workspace":{"active_slot":{"owned_model_bytes":model_workspace.initial.owned_model_bytes,"retained_prepare_bytes":model_workspace.initial.retained_prepare_bytes},"spare_restore_slot":{"owned_model_bytes":model_workspace.restore.owned_model_bytes,"retained_prepare_bytes":model_workspace.restore.retained_prepare_bytes},"candidate_owned_bytes":model_workspace.candidate.owned_bytes,"codec_backing_bytes":model_workspace.codec.backing_bytes,"workspace_owned_bytes":model_workspace.owned_bytes},"store_owned_bytes":store_owned_bytes,"accounts":view.accounts,"boot":view.boot,"active_city":view.region.as_ref().map(|r|r.active_city),"obligations":view.region.as_ref().map(|r|r.obligations),"payroll":e.timeline().map(|t|t.payroll.iter().take(8).collect::<Vec<_>>()),"claims":e.timeline().map(|t|t.claims.iter().take(8).collect::<Vec<_>>()),"claim_count":e.timeline().map(|t|t.claims.len()),"payroll_count":e.timeline().map(|t|t.payroll.len()),"resources":e.resources(),"sim_codec":codec.slots.report()? ,"sim_air_input_owned_bytes":codec.air_input.owned_bytes(),"sim_timing_state_bytes":std::mem::size_of::<SimTimings>(),"diagnostics_owned_bytes":diag.as_ref().map_or(0,crate::cpu_log::CpuLog::owned_bytes)});

    let business_sha = {
        let _timer = timings.timer(TimingPhase::StatusBusinessDigest);
        store.business_digest().map_err(err)?
    };
    let state_sha = {
        let _timer = timings.timer(TimingPhase::StatusStateDigest);
        store.state_hash().map_err(err)?
    };
    let mut runtime = runtime;
    runtime["worker_task_observation"] = worker.task_observation();
    runtime["task_execution_observation"] = task_observations.report(worker);
    runtime["sim_host_timing"] = timings.report();
    Ok(
        json!({"revision":view.revision,"sequence":store.committed_sequence(),"business_sha256":business_sha,"runtime":runtime,"timeline":e.timeline().map(|t|t.progress),
 "background":b.map(|b|json!({"version":b.version(),"time":b.time(),"incarnation":b.incarnation(),"pending":b.pending().map(|j|json!({"id":j.id,"generation":j.generation,"attempt":j.attempt,"running":j.running,"failure":j.failure,"start":j.start,"end":j.end})),"field":b.field().map(|f|f.report()),"state_sha256":air_sha})),
 "state_sha256":state_sha,"capacity":store.capacity_status().map_err(err)?,"worker_pid":worker.pid_opt(),"source_sha256":env!("MYCITY_SOURCE_SHA256"),"memory_16gb_gate":"not_run"}),
    )
}
/// Same original internal operation constructor for disclosure and all handlers.
fn continuation_envelope(store:&Store,target:u64,limit:u16)->Result<Envelope,String> {
    let engine=store.engine().map_err(err)?;let(scope,revision)=engine.world().scope_revision();
    let operation=SERVICE_CONTINUATION_PREFIX|store.committed_sequence().checked_add(1)
        .filter(|value|*value<=background::JOB_ID_LIMIT).ok_or("sim_control_id")?;
    Ok(Envelope{world:scope.world(),branch:scope.branch(),operation_id:operation,expected_revision:revision,
        actions:vec![Action::Timeline{command:mycity_core::timeline::Command::Advance{target,limit}}]})
}
/// Continue only the original Store's already-funded Timeline reservation.
/// A lost response is NOT permission to send another RunDue: retain the selected
/// envelope, query its original receipt, and inspect recovered outcome/serial.
/// Continue an original accepted Timeline even with no optional domain provider.
fn timeline_run_due(
    store:&mut Store,active_worker:bool,diag:&mut Option<crate::cpu_log::CpuLog>,
    id:u64,writer:&mut impl Write,selected:&crate::task_registry_directory::SelectedTimeline,
    task_observations:&mut task_observation::Recorder,
    entry:&crate::task_registry_directory::Entry,
) -> Result<Value,String> {
    crate::task_registry_directory::validate_timeline(store,selected,active_worker).map_err(err)?;
    let e=store.engine().map_err(err)?;
    let before=e.timeline().ok_or("timeline_due_missing_clock")?.progress;
    let sequence=store.committed_sequence();
    let envelope=continuation_envelope(store,selected.target(),selected.page_limit())?;
    let operation=envelope.operation_id;
    if store.receipt(operation).map_err(err)?.is_some(){return Err(format!("timeline_due_namespace_collision:operation={operation};use_original_reserved_manual_advance"));}
    event(writer,id,"timeline_continuation_selected",json!({"dispatch":"required_registry_v3","envelope":envelope,
        "before_sequence":sequence,"before_serial":before.serial,"zero_width":before.at==before.last_complete,
        "plan":if selected.is_admission(){"original_next_boundary_admission"}else{"original_funded_continuation"},
        "recovery":"original_receipt_and_recovered_outcome_and_serial; no automatic RunDue retry"}))?;
    let outcome=task_observations.execute(store,Caller::System,envelope,diag,entry)
        .map_err(|e|format!("timeline_due_recovery_required:operation={operation}:{e}"))?;
    outcome.model.as_ref().map_err(|e|format!("timeline_due_model_rejected:operation={operation}:{e:?}"))?;
    if !outcome.has_persistent_receipt{return Err(format!("timeline_due_receipt_unconfirmed:operation={operation}"));}
    let after=store.engine().map_err(err)?.timeline().ok_or("timeline_due_missing_clock")?.progress;
    Ok(json!({"dispatch":"required_registry_v3","dispatch_state":"page_committed","selected_operation":operation,
        "before_sequence":sequence,"before_serial":before.serial,"after_serial":after.serial,"timeline":after,"outcome":outcome}))
}

/// ReceiptProof presence alone does not distinguish a success from a failed ticket.
fn service_run_due(
    store: &mut Store,
    active_worker: bool,
    diag: &mut Option<crate::cpu_log::CpuLog>,
    id: u64,
    writer: &mut impl Write,
    selected: &crate::required_dispatch::SelectedServiceContinuation,
    task_observations:&mut task_observation::Recorder,
    entry:&crate::task_registry_directory::Entry,
) -> Result<Value, String> {
    use crate::required_dispatch::{self as due, SelectionMode};
    if active_worker { return Err("sim_dispatch_busy".into()); }
    due::validate_service_continuation(store, &selected, active_worker).map_err(err)?;
    let engine = store.engine().map_err(err)?;
    let before = engine.timeline().ok_or("service_due_missing_timeline")?.progress;
    let before_sequence = store.committed_sequence();
    let envelope=continuation_envelope(store,selected.target(),selected.page_limit())?;
    let operation=envelope.operation_id;
    let mode = match selected.mode() {
        SelectionMode::RegistrySelected => "registry_selected",
        SelectionMode::MandatoryFallback => "mandatory_fallback",
    };
    let key = selected.key();
    let task_key = json!({"scope":{"world_id":key.scope.world_id,
        "branch_id":key.scope.branch_id,"city_id":key.scope.city_id},
        "clock":{"kind":"development","city_id":key.scope.city_id},
        "source_id":key.source_id,"source_version":key.source_version,
        "window":{"start":key.window.start,"end":key.window.end},"cursor":key.cursor});
    if store.receipt(operation)
        .map_err(|e|format!("service_due_recovery_required:operation={operation}:{e:?}"))?.is_some() {
        return Err(format!("service_due_namespace_collision:operation={operation};use_original_reserved_manual_advance"));
    }
    // Frame failure means no execution. Successful write does not prove the peer
    // received it: without this exact identity the peer must recover, never retry
    // a fresh selection blindly after an uncertain transport failure.
    event(writer,id,"service_continuation_selected",json!({
        "dispatch":"required_registry_v2", "envelope":envelope,
        "before_sequence":before_sequence, "before_serial":before.serial,
        "task_key":task_key, "selection_mode":mode,
        "recovery":"original_receipt_and_recovered_outcome_and_serial; no automatic RunDue retry"
    }))?;
    let outcome = task_observations.execute(store,Caller::System,envelope,diag,entry)
        .map_err(|e|format!("service_due_recovery_required:operation={operation}:{e}"))?;
    outcome.model.as_ref().map_err(|e|format!("service_due_model_rejected:operation={operation}:{e:?}"))?;
    if !outcome.has_persistent_receipt { return Err(format!("service_due_receipt_unconfirmed:operation={operation}")); }
    let after = store.engine().map_err(err)?.timeline().ok_or("service_due_missing_timeline")?.progress;
    let boundary_completed = after.last_complete == before.at && after.phase.is_none();
    Ok(json!({"dispatch":"required_registry_v2",
        "dispatch_state":if boundary_completed {"boundary_completed"} else {"page_committed"},
        "selection_mode":mode, "selected_operation":operation,
        "before_sequence":before_sequence, "before_serial":before.serial,
        "after_serial":after.serial, "boundary_at":before.at,
        "timeline":after, "outcome":outcome, "service_delivery_claimed":false}))
}

/// Source lane follows true NoWork from both earlier mandatory lanes.
fn supply_run_due(
    store: &mut Store,
    active_worker: bool,
    diag: &mut Option<crate::cpu_log::CpuLog>,
    id: u64,
    writer: &mut impl Write,
    selected: &crate::required_dispatch::SelectedSupplyContinuation,
    task_observations:&mut task_observation::Recorder,
    entry:&crate::task_registry_directory::Entry,
) -> Result<Value, String> {
    use crate::required_dispatch::{self as due, SupplySelectionMode};
    if active_worker { return Err("sim_dispatch_busy".into()); }
    due::validate_supply_continuation(store, &selected, active_worker).map_err(err)?;
    let engine = store.engine().map_err(err)?;
    let before = engine.timeline().ok_or("supply_due_missing_timeline")?.progress;
    let before_sequence = store.committed_sequence();
    let envelope=continuation_envelope(store,selected.target(),selected.page_limit())?;
    let operation=envelope.operation_id;
    let mode = match selected.mode() {
        SupplySelectionMode::RegistrySelected => "registry_selected",
        SupplySelectionMode::MandatoryFallback => "mandatory_fallback",
        SupplySelectionMode::MandatoryZeroBoundary => "mandatory_zero_boundary",
    };
    let task_key = selected.key().map(|key| json!({"scope":{"world_id":key.scope.world_id,
        "branch_id":key.scope.branch_id,"city_id":key.scope.city_id},
        "clock":{"kind":"development","city_id":key.scope.city_id},
        "source_id":key.source_id,"source_version":key.source_version,
        "window":{"start":key.window.start,"end":key.window.end},"cursor":key.cursor}));
    let source_before = engine.continuous_supply_due_state().map_err(err)?;
    if store.receipt(operation)
        .map_err(|e|format!("supply_due_recovery_required:operation={operation}:{e:?}"))?.is_some() {
        return Err(format!("supply_due_namespace_collision:operation={operation};use_original_reserved_manual_advance"));
    }
    // Frame failure means no execution. Successful write does not prove the peer
    // received it: without this exact identity the peer must recover, never retry
    // a fresh selection blindly after an uncertain transport failure.
    event(writer,id,"supply_continuation_selected",json!({
        "dispatch":"required_registry_v3", "envelope":envelope,
        "before_sequence":before_sequence, "before_serial":before.serial,
        "task_key":task_key, "selection_mode":mode,
        "plan_kind":format!("{:?}",selected.plan_kind()), "policy_version":selected.policy_version(),
        "zero_reason":format!("{:?}",selected.zero_reason()),
        "recovery":"original_receipt_and_recovered_outcome_and_serial; no automatic RunDue retry"
    }))?;
    let outcome = task_observations.execute(store,Caller::System,envelope,diag,entry)
        .map_err(|e|format!("supply_due_recovery_required:operation={operation}:{e}"))?;
    outcome.model.as_ref().map_err(|e|format!("supply_due_model_rejected:operation={operation}:{e:?}"))?;
    if !outcome.has_persistent_receipt { return Err(format!("supply_due_receipt_unconfirmed:operation={operation}")); }
    let after = store.engine().map_err(err)?.timeline().ok_or("supply_due_missing_timeline")?.progress;
    let source_after = store.engine().map_err(err)?.continuous_supply_due_state().map_err(err)?;
    // A serial increment alone must not turn a zero-width loop into useful work.
    if selected.zero_reason().is_some() && source_before == source_after
        && before.target == after.target && before.last_complete == after.last_complete
        && before.phase == after.phase && before.lane == after.lane && before.cursor == after.cursor {
        return Err(format!("supply_due_zero_progress:operation={operation};recover_original_outcome"));
    }
    let boundary_completed = after.phase.is_none() && after.last_complete >= before.at;
    Ok(json!({"dispatch":"required_registry_v3",
        "dispatch_state":if boundary_completed {"boundary_completed"} else {"page_committed"},
        "selection_mode":mode, "selected_operation":operation,
        "plan_kind":format!("{:?}",selected.plan_kind()), "policy_version":selected.policy_version(),
        "before_sequence":before_sequence, "before_serial":before.serial,
        "after_serial":after.serial, "boundary_at":if boundary_completed {after.last_complete} else {after.at},
        "timeline":after, "outcome":outcome, "supply_delivery_claimed":false}))
}

fn control(
    store: &mut Store,
    command: B,
    diag: &mut Option<crate::cpu_log::CpuLog>,
) -> Result<(), String> {
    let e = store.engine().map_err(err)?;
    let view = e.world().view();
    let envelope = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: 0xd000_0000_0000_0000
            | store
                .committed_sequence()
                .checked_add(1)
                .filter(|n| *n <= background::JOB_ID_LIMIT)
                .ok_or("sim_control_id")?,
        expected_revision: view.revision,
        actions: vec![Action::Background { command }],
    };
    crate::cpu_log::execute(store, Caller::System, envelope, diag)?
        .model
        .map_err(err)?;
    Ok(())
}
struct Required<'a> {
    id: u64,
    job: u64,
    quota: Option<crate::air_turns::Quota>,
    rules: &'a str,
    diag: &'a mut Option<crate::cpu_log::CpuLog>,
    timings: &'a SimTimings,
    selection: Option<&'a crate::required_dispatch::SelectedRequired>,
}
// Advisory only. Failure to produce a hint never removes or gates the existing
// mandatory job. This token binds one local read, not a persistent source identity.
fn required_registry_observation(store: &Store) -> Value {
    use crate::task_registry_adapter::{observe_required, OwnerView};
    let engine = match store.engine() {
        Ok(engine) => engine,
        Err(_) => {
            return json!({"schema":"mycity-task-registry-observation-v1",
            "state":"store_unavailable", "snapshot_token_durable":false,
            "required_preserved":true, "clock":"world", "now":null})
        }
    };
    let (scope, _) = engine.world().scope_revision();
    let now = engine.timeline().map(|timeline| timeline.progress.at);
    let background = engine.background();
    let pending = background.and_then(|background| background.pending());
    let state = match (background, now) {
        (Some(background), Some(now)) => format!(
            "{:?}",
            observe_required(
                background,
                OwnerView {
                    scope,
                    city_id: None,
                    world_time_seconds: u64::from(now),
                    snapshot_token: store.committed_sequence().checked_add(1)
                },
                &[]
            )
            .state
        ),
        (_, None) => "owner_clock_unavailable".into(),
        (None, _) => "background_unavailable".into(),
    };
    json!({"schema":"mycity-task-registry-observation-v1", "state":state,
        "snapshot_token_durable":false, "required_preserved":true,
        "world":scope.world(), "branch":scope.branch(), "clock":"world", "now":now,
        "job":pending.map(|job|job.id), "start":pending.map(|job|job.start),
        "end":pending.map(|job|job.end), "generation":pending.map(|job|job.generation),
        "attempt":pending.map(|job|job.attempt)})
}
fn required(
    store: &mut Store,
    worker: &mut Worker,
    codec: &mut Codec,
    context: Required<'_>,
    w: &mut impl Write,
    reader: &mut impl BufRead,
    mut probe: Option<ProbePoint>,
) -> Result<Value, String> {
    let Required {
        id,
        job,
        quota,
        rules,
        diag,
        timings,
        mut selection,
    } = context;
    let _request_timer = timings.timer(TimingPhase::RunRequired);
    let operation = background::accept_operation_id(job)?;
    // Return the ORIGINAL persisted record; never rebuild an Envelope after lost ACK.
    if let Some(record) = store.receipt(operation).map_err(err)? {
        return Ok(json!({"replayed":true,"record":record}));
    }
    loop {
        let pending = store
            .engine()
            .map_err(err)?
            .background()
            .and_then(|b| b.pending())
            .ok_or("sim_no_pending")?;
        if pending.id != job
            || pending.attempt >= 3
            || pending.failure == Some(Failure::InvalidInput)
        {
            return Err("sim_retry_exhausted_or_wrong_job".into());
        }
        let mut registry = required_registry_observation(store);
        // Old process is reaped before clearing any file or binding a new execution epoch.
        worker.retire(Duration::from_secs(1))?;
        let (cache, cache_fence) = {
            let _timer = timings.timer(TimingPhase::CacheRecoverRebindReset);
            let cache = store.prepare_worker_cache().map_err(err)?;
            let exe = std::env::current_exe().map_err(err)?;
            let cache_fence = worker_cache::recover_exclusive(&cache, &exe)?;
            // RunDue selections are ephemeral and never become authority merely
            // because a hint was produced. Retirement retains its original owner;
            // before the first rebind, reread the actual Store identity and job.
            if let Some(selected) = selection.take() {
                crate::required_dispatch::validate_required(
                    store, selected, worker.has_active_execution(),
                ).map_err(err)?;
            }
            control(store, B::RebindLease { job }, diag)?;
            worker_cache::reset_after_worker_stopped(&cache)?;
            // Keep the exclusive fence alive through input publication and spawn.
            (cache, cache_fence)
        };
        // RebindLease changed execution identity; refresh from the actual new job.
        registry["rebound"] = required_registry_observation(store);
        let _input_timer = timings.timer(TimingPhase::InputEncodePublish);
        let b = store
            .engine()
            .map_err(err)?
            .background()
            .ok_or("sim_no_background")?;
        let pending = b.pending().ok_or("sim_no_pending")?;
        b.frozen_input_into(&mut codec.air_input)?;
        let input = codec.air_input.state()?;
        codec
            .slots
            .rebind(codec.epoch, job, pending.generation, pending.attempt)?;
        let checkpoint = codec
            .slots
            .with_slot(crate::worker_slots::Kind::Input, |bytes| {
                input.checkpoint_to(bytes)?;
                worker_cache::publish(&cache, "inputs", bytes.as_slice())
            });
        codec.log(diag)?;
        let checkpoint = checkpoint?;
        let view = store.engine().map_err(err)?.world().view();
        let request = Work {
            protocol: wire::PROTOCOL.into(),
            strategy: mycity_core::air::STRATEGY.into(),
            controlled: quota,
            ticket: Ticket {
                world: view.scope.world(),
                branch: view.scope.branch(),
                job,
                incarnation: pending.incarnation,
                generation: pending.generation,
                attempt: pending.attempt,
                base_revision: pending.base_revision,
                field_version: pending.field_version,
                start: pending.start,
                end: pending.end,
                rules_sha256: sha256(rules.as_bytes()),
                source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
                input_sha256: pending.input.identity(),
                checkpoint_sha256: checkpoint,
            },
        };
        let generation = pending.generation;
        let attempt_number = pending.attempt;
        drop(_input_timer);
        if gate(
            ProbePoint::Leased,
            &mut probe,
            reader,
            w,
            id,
            json!({"job":job,"attempt":attempt_number}),
        )? {
            control(
                store,
                B::Fail {
                    job,
                    generation,
                    failure: Failure::Cancelled,
                },
                diag,
            )?;
            return Err("sim_attempt_cancelled".into());
        }
        let gated = probe.is_some();
        let mut cancelled = false;
        // A sliced attempt never implicitly restarts after an ambiguous control/transport outcome.
        let control_failure = quota.is_some();
        let attempt = (|| -> Result<_, String> {
            let exe = std::env::current_exe().map_err(err)?;
            let mut first_response_timer =
                Some(timings.timer(TimingPhase::WorkerSpawnRegisterToFirstResponse));
            worker.start_bounded(&exe, &cache, None, gated)?;
            let child = &mut *worker;
            worker_cache::register(&cache, child.pid(), &exe)?;
            drop(cache_fence);
            event(
                w,
                id,
                "worker_started",
                json!({"worker_pid":child.pid(),"broker_lease":child.broker_lease(),"ticket":request.ticket,"registry":registry}),
            )?;
            child.submit(&request)?;
            let mut index = 0u64;
            loop {
                if quota.is_none() && index >= 4 { return Err("sim_worker_incomplete".into()); }
                let wait_timer =
                    (index != 0).then(|| timings.timer(TimingPhase::WorkerProgressWaitAfterFirst));
                let progress = child.next(Duration::from_secs(30))?;
                drop(wait_timer);
                drop(first_response_timer.take());
                index = index.checked_add(1).ok_or("sim_worker_event_counter")?;
                event(
                    w,
                    id,
                    "worker_progress",
                    {
                        let mut progress_value = serde_json::to_value(&progress).map_err(err)?;
                        progress_value["task_observation"] = child.task_observation();
                        progress_value
                    },
                )?;
                if progress.stage == Stage::SliceBoundary {
                    let boundary = progress.turn.as_ref().ok_or("sim_missing_turn_boundary")?;
                    event(w,id,"page_boundary",json!({"ticket":request.ticket,"boundary":boundary,
                        "plan":crate::air_turns::PLAN_ID,"publication":"not_complete",
                        "task_observation":child.task_observation()}))?;
                    // Stream one bounded control at a time. While paused, the same Sim can
                    // answer observations/proofs or cancel; no unrelated mutation/second writer.
                    loop {
                        let bytes = wire::frame(reader)?.ok_or("sim_page_control_eof")?;
                        let control_request: Request = serde_json::from_slice(&bytes).map_err(err)?;
                        if control_request.protocol != PROTOCOL || control_request.id != id {
                            return Err("sim_page_control_identity".into());
                        }
                        match control_request.command {
                            Command::PageControl { control: PageControl::Continue { next } } => {
                                child.continue_pages(next)?;
                                break;
                            }
                            Command::PageControl { control: PageControl::Cancel } => {
                                cancelled = true;
                                return Err("sim_attempt_cancelled".into());
                            }
                            Command::PageControl { control: PageControl::Observe } => {
                                event(w,id,"page_control_result",json!({"Ok":{
                                    "progress":crate::sim_host::progress(store)?,
                                    "worker_owner":child.retirement_state(),
                                    "task_observation":child.task_observation(),
                                    "still_unresolved":true}}))?;
                            }
                            Command::PageControl { control: PageControl::ReceiptProof { operation } } => {
                                let record=store.receipt(operation).map_err(err)?;
                                event(w,id,"page_control_result",json!({"Ok":{
                                    "operation":operation,"present":record.is_some(),
                                    "record":record}}))?;
                            }
                            _ => return Err("sim_unexpected_page_control".into()),
                        }
                    }
                    continue;
                }
                let point = match progress.stage {
                    Stage::Sources => Some(ProbePoint::WorkerSources),
                    Stage::Cells => Some(ProbePoint::WorkerCells),
                    _ => None,
                };
                if let Some(point) = point {
                    if gate(
                        point,
                        &mut probe,
                        reader,
                        w,
                        id,
                        json!({"worker_pid":child.pid(),"cursor":progress.cursor}),
                    )? {
                        cancelled = true;
                        return Err("sim_attempt_cancelled".into());
                    }
                }
                if progress.stage != Stage::Complete {
                    child.acknowledge_probe(&progress)?;
                }
                if progress.stage == Stage::Complete {
                    let candidate_hash = progress
                        .candidate_sha256
                        .as_deref()
                        .ok_or("sim_candidate_hash")?;
                    let _result_timer = timings.timer(TimingPhase::CandidateReadAndImport);
                    return codec
                        .slots
                        .with_slot(crate::worker_slots::Kind::Candidate, |bytes| {
                            wire::read_asset_into(
                                &cache.join("candidates"),
                                candidate_hash,
                                bytes,
                            )?;
                            Ok(crate::cpu_log::import(
                                store,
                                job,
                                generation,
                                bytes.as_slice(),
                                |point| {
                                    if point == mycity_storage::BackgroundPoint::AssetPublished {
                                        let cancelled = gate(
                                            ProbePoint::CandidatePublished,
                                            &mut probe,
                                            reader,
                                            w,
                                            id,
                                            json!({"job":job}),
                                        )
                                        .map_err(std::io::Error::other)?;
                                        if cancelled {
                                            return Err(std::io::Error::other(
                                                "sim_probe_cancel_at_import_unsupported",
                                            ));
                                        }
                                    }
                                    Ok(())
                                },
                                diag,
                            ))
                        });
                }
            }
        })();
        codec.log(diag)?;
        let imported = match attempt {
            Ok(b) => b,
            Err(error) => {
                worker.retire(Duration::ZERO)?;
                control(
                    store,
                    B::Fail {
                        job,
                        generation,
                        failure: if cancelled {
                            Failure::Cancelled
                        } else {
                            Failure::Transient
                        },
                    },
                    diag,
                )?;
                event(
                    w,
                    id,
                    "attempt_failed",
                    json!({"attempt":attempt_number,"reason":error}),
                )?;
                if cancelled {
                    return Err("sim_attempt_cancelled".into());
                }
                if control_failure {
                    // A rejected/uncertain control terminates this command after actual
                    // retirement. It must never auto-start another numerical attempt.
                    return Err(format!("sim_sliced_attempt_terminated:{error}"));
                }
                continue;
            }
        };
        // The pure worker's files are copied/validated into Sim-owned immutable assets.
        let hash = match imported {
            Ok(h) => h,
            Err(error) => {
                // Validation failure remains a required obligation. IO uncertainty must reopen.
                if matches!(
                    error,
                    mycity_storage::Error::Invalid(_) | mycity_storage::Error::Model(_)
                ) {
                    control(
                        store,
                        B::Fail {
                            job,
                            generation,
                            failure: Failure::InvalidInput,
                        },
                        diag,
                    )?;
                }
                return Err(err(error));
            }
        };
        let view = store.engine().map_err(err)?.world().view();
        let envelope = Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: operation,
            expected_revision: view.revision,
            actions: vec![Action::Background {
                command: B::Accept {
                    job,
                    generation,
                    candidate_sha256: hash,
                },
            }],
        };
        {
            let _timer = timings.timer(TimingPhase::PersistentAccept);
            let outcome = crate::cpu_log::execute(store, Caller::System, envelope, diag)?;
            outcome.model.map_err(err)?;
            if !outcome.has_persistent_receipt {
                return Err("sim_admission_not_durable".into());
            }
        }
        if gate(
            ProbePoint::Committed,
            &mut probe,
            reader,
            w,
            id,
            json!({"job":job,"operation_id":operation}),
        )? {
            return Err("sim_probe_cancel_after_commit_unsupported".into());
        }
        return Ok(
            json!({"replayed":false,"record":store.receipt(operation).map_err(err)?.ok_or("sim_receipt_missing")?}),
        );
    }
}
pub fn run(world: &Path, create: bool) -> Result<(), String> {
    run_inner(world, create, false, Profile::Base)
}
pub fn run_probe(world: &Path, create: bool) -> Result<(), String> {
    run_inner(world, create, true, Profile::Base)
}
pub fn run_joint(world: &Path, create: bool, probe: bool) -> Result<(), String> {
    run_inner(world, create, probe, Profile::Joint)
}
pub fn run_scale_full(world: &Path, create: bool, probe: bool) -> Result<(), String> {
    run_scale(world, create, probe, crate::scale_reference::Tier::Full)
}
pub fn run_scale(
    world: &Path,
    create: bool,
    probe: bool,
    tier: crate::scale_reference::Tier,
) -> Result<(), String> {
    run_inner(world, create, probe, Profile::Scale(tier))
}

fn run_inner(
    world: &Path,
    create: bool,
    probe_enabled: bool,
    profile: Profile,
) -> Result<(), String> {
    crate::compute_worker::with_worker(|worker| run_owned(world, create, probe_enabled, profile, worker))
}
fn run_owned(
    world: &Path, create: bool, probe_enabled: bool, profile: Profile, worker: &mut Worker,
) -> Result<(), String> {
    let (epoch, role_admission) =
        crate::normal_pool::broker::activate_role_observed(crate::normal_pool::workspace::Kind::SimCodec)?;
    let mut codec = Codec::new(epoch)?;
    let timings = SimTimings::new();
    let workspace = mycity_storage::StoreWorkspace::new().map_err(err)?;
    let rules = profile.rules();
    let (mut store, pre_open_memory) = if create {
        let (opening, resources) = match profile {
            Profile::Scale(tier) => (tier.opening(), crate::scale_reference::resources()?),
            Profile::Joint => (
                crate::joint_reference::OPENING,
                crate::joint_reference::resources()?,
            ),
            Profile::Base => (
                include_bytes!("../../../fixtures/m0-exchange-opening.json").as_slice(),
                crate::joint_reference::resources()?,
            ),
        };
        let pre_open = sim_arena_point()?;
        let store = Store::create_spatial_in_prepared_workspace(
            world,
            opening,
            resources,
            &rules,
            (
                include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
                include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
            ),
            matches!(profile, Profile::Scale(_)),
            workspace,
        )
        .map_err(err)?;
        (store, pre_open)
    } else {
        let pre_open = sim_arena_point()?;
        let store = Store::open_in_prepared_workspace(world, &rules, workspace).map_err(err)?;
        (store, pre_open)
    };
    let open_memory = SimOpenMemoryDiagnostic {
        operation: if create { "create" } else { "open" },
        pre_open: pre_open_memory,
        post_open: sim_arena_point()?,
    };
    crate::normal_pool::broker::acknowledge_role(
        epoch,
        &(
            store.model_workspace_identity().map_err(err)?,
            store.metadata_workspace_identity().map_err(err)?,
            store
                .spatial()
                .map_err(err)?
                .ok_or("sim_spatial_missing")?
                .normal_backing_identity()
                .map_err(err)?,
            store
                .spatial()
                .map_err(err)?
                .ok_or("sim_spatial_missing")?
                .terrain()
                .grid_backing_identity(),
            codec.slots.report()?,
            codec.air_input.identity(),
        ),
    )?;
    let mut task_observations = task_observation::Recorder::new(epoch, role_admission);
    let mut diag = if crate::cpu_log::enabled() {
        Some(crate::cpu_log::CpuLog::create(
            &world
                .parent()
                .ok_or("diagnostic_world_parent")?
                .join("diagnostics"),
            "sim",
        )?)
    } else {
        None
    };
    if let Some(log) = diag.as_mut() {
        let owned_bytes = std::mem::size_of::<Codec>()
            .checked_add(std::mem::size_of::<SimTimings>())
            .and_then(|bytes|bytes.checked_add(task_observation::METADATA_PLAN_BYTES))
            .and_then(|bytes|bytes.checked_add(execution_protocol::METADATA_PLAN_BYTES))
            .ok_or("sim_diagnostics_bytes")?;
        log.set_external_diagnostics_owned_bytes(owned_bytes)?;
    }
    codec.log(&mut diag)?;
    let stdin = std::io::stdin();
    let mut reader = BufReader::with_capacity(8192, stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    event(
        &mut writer,
        0,
        "ready",
        status(&store, worker, &codec, &diag, &timings, &open_memory, &task_observations)?,
    )?;
    while let Some(bytes) = wire::frame(&mut reader)? {
        let request: Request = serde_json::from_slice(&bytes).map_err(err)?;
        if request.protocol != PROTOCOL || request.id == 0 {
            return Err("sim_request_identity".into());
        }
        let(command,bound_request)=match request.command {
            Command::RunBoundTask{binding}=>(Command::RunDue,Some(binding)),other=>(other,None),
        };
        let stop = matches!(command, Command::Stop);
        let outcome = match command {
            Command::Status => status(&store, worker, &codec, &diag, &timings, &open_memory, &task_observations),
            Command::Progress => progress(&store),
            Command::DescribeTaskExecution=>execution_protocol::describe(&store,worker),
            Command::RunBoundTask{..}=>Err("task_request_not_normalized".into()),
            Command::ReceiptProof { operation } => store.receipt(operation).map_err(err).and_then(|record| {
                let bytes = serde_json::to_vec(&record).map_err(err)?;
                Ok(json!({"operation":operation,"present":record.is_some(),"sha256":sha256(&bytes),"bytes":bytes.len()}))
            }),
            Command::Execute { envelope } => {
                let prefix = envelope.operation_id & 0xf000_0000_0000_0000;
                if matches!(prefix, SERVICE_CONTINUATION_PREFIX | 0xd000_0000_0000_0000 | 0xe000_0000_0000_0000)
                    || envelope
                        .actions
                        .iter()
                        .any(|a| matches!(a,Action::Background{command} if *command!=B::Enable))
                {
                    Err("sim_internal_control_reserved".into())
                } else {
                    crate::cpu_log::execute(&mut store,Caller::System,envelope,&mut diag)
                        .and_then(|v| serde_json::to_value(v).map_err(err))
                }
            }
            Command::SettleTownMarket { operation } => {
                let prefix = operation & 0xf000_0000_0000_0000;
                if matches!(prefix, SERVICE_CONTINUATION_PREFIX | 0xd000_0000_0000_0000 | 0xe000_0000_0000_0000) {
                    Err("sim_internal_control_reserved".into())
                } else {
                    store.settle_town_market(operation).map_err(err)
                        .and_then(|v| serde_json::to_value(v).map_err(err))
                }
            }
            Command::RunDue => (|| -> Result<Value,String> {
                let requested_quota=match bound_request.as_ref(){Some(binding)=>execution_protocol::validate_supported_request(binding)?,None=>None};
                use crate::task_registry_directory::{self as directory, Route, Action as Selected};
                let directory=directory::collect(&store,worker.has_active_execution());
                match directory {
                    Err(error)=>Err(err(error)),
                    Ok(directory)=>{
                        let stamp=if let Some(binding)=bound_request.as_ref(){
                            let prepared=execution_protocol::Prepared::new(&store,worker,&directory)?.ok_or("task_current_route_not_ready")?;
                            prepared.validate_binding(binding)?;let stamp=prepared.stamp(binding)?;
                            event(&mut writer,request.id,"bound_task_selected",stamp.clone())?;Some(stamp)
                        }else{None};
                        let report=directory.report().map_err(err);
                        // No Store write intervenes here. Each selected original handler
                        // validates its own complete context at the commit/rebind boundary;
                        // avoid a redundant whole-directory collection and fingerprint pass.
                        match report {
                            Err(error)=>Err(error),
                            Ok(report)=>{
                                let result=match directory.route().clone() {
                                    Route::NoWork=>Ok(json!({"dispatch":"required_registry_v1","state":"no_work","required_preserved":true})),
                                    Route::Busy=>Err("sim_dispatch_busy".into()),
                                    Route::Blocked(reason)=>Err(format!("sim_dispatch_backpressure:{}",reason.as_code())),
                                    Route::Selected(Selected::Air(selected))=>{
                                        let job=selected.job();
                                        let mode=match selected.mode(){crate::required_dispatch::SelectionMode::RegistrySelected=>"registry_selected",crate::required_dispatch::SelectionMode::MandatoryFallback=>"mandatory_fallback"};
                                        required(&mut store,worker,&mut codec,Required{id:request.id,job,quota:requested_quota,rules:&rules,diag:&mut diag,timings:&timings,selection:Some(&selected)},&mut writer,&mut reader,None).map(|mut v|{
                                            v["dispatch"]=json!("required_registry_v1");v["dispatch_state"]=json!("completed");v["selection_mode"]=json!(mode);v["selected_job"]=json!(job);v
                                        })
                                    },
                                    Route::Selected(Selected::Service(selected))=>service_run_due(&mut store,worker.has_active_execution(),&mut diag,request.id,&mut writer,&selected,&mut task_observations,&directory.entries()[1]),
                                    Route::Selected(Selected::Supply(selected))=>supply_run_due(&mut store,worker.has_active_execution(),&mut diag,request.id,&mut writer,&selected,&mut task_observations,&directory.entries()[2]),
                                    Route::Selected(Selected::Timeline(selected))=>timeline_run_due(&mut store,worker.has_active_execution(),&mut diag,request.id,&mut writer,&selected,&mut task_observations,&directory.entries()[3]),
                                };
                                result.map(|mut v|{v["task_directory"]=report;v["execution_observation"]=task_observations.report(worker);if let Some(stamp)=stamp{v["bound_request"]=stamp;}v})
                            }
                        }
                    }
                }
            })(),
            Command::RunRequired { job } => required(
                &mut store,
                worker,
                &mut codec,
                Required {
                    id: request.id,
                    job,
                    quota: None,
                    rules: &rules,
                    diag:&mut diag,
                    timings:&timings,
                    selection:None,
                },
                &mut writer,
                &mut reader,
                None,
            ),
            Command::RunRequiredSliced { job, plan, quota } => {
                if plan != crate::air_turns::PLAN_ID || quota.validate().is_err() {
                    Err("sim_page_plan".into())
                } else {
                    required(&mut store,worker,&mut codec,Required {
                        id:request.id,job,quota:Some(quota),rules:&rules,diag:&mut diag,timings:&timings,selection:None,
                    },&mut writer,&mut reader,None)
                }
            }
            Command::PageControl { .. } => Err("sim_page_control_not_waiting".into()),
            Command::ProbeRequired { job, point } if probe_enabled => required(
                &mut store,
                worker,
                &mut codec,
                Required {
                    id: request.id,
                    job,
                    quota: None,
                    rules: &rules,
                    diag:&mut diag,
                    timings:&timings,
                    selection:None,
                },
                &mut writer,
                &mut reader,
                Some(point),
            ),
            Command::ProbeRequired { .. } | Command::ProbeAck { .. } => {
                Err("sim_probe_command_disabled_or_unexpected".into())
            }
            Command::ProbeExhaust {envelope} if probe_enabled => {
                let receipt=crate::cpu_log::execute(&mut store,Caller::System,envelope,&mut diag)?;
                receipt.model.map_err(err)?;
                // Test-only protocol, exactly after durable commit and before reply.
                // This real allocation must fail; no fallback or caught fake OOM.
                let bytes=vec![0u8;2*crate::normal_pool::workspace::Kind::SimCodec.bytes()];
                std::hint::black_box(bytes);
                Err("sim_oom_probe_unexpected_success".into())
            }
            Command::ProbeExhaust {..} => Err("sim_probe_command_disabled_or_unexpected".into()),
            Command::Checkpoint => store
                .checkpoint()
                .map_err(err)
                .map(|_| json!({"checkpointed":true})),
            Command::Stop => Ok(json!({"stopped":true})),
        };
        event(
            &mut writer,
            request.id,
            "result",
            serde_json::to_value(outcome).map_err(err)?,
        )?;
        // Start may already have created a broker-owned Child without a receipt.
        // No local Lease exists to Stop it: end THIS Sim session so its original
        // supervisor can actually wait and revoke. Do not wait for another command.
        if worker.requires_session_exit() {
            return Err("sim_worker_session_exit_required".into());
        }
        if stop {
            break;
        }
    }
    worker.retire(Duration::from_secs(1))?;
    if let Some(log) = diag.as_mut() {
        log.finish()?;
    }
    Ok(())
}

#[cfg(test)]
mod timing_tests {
    use super::{SimTimings, TimingPhase};
    use std::time::Duration;

    #[test]
    fn timing_summary_is_fixed_and_accumulates_count_total_and_max() {
        let timings = SimTimings::new();
        timings.record(TimingPhase::RunRequired, Duration::from_nanos(7));
        timings.record(TimingPhase::RunRequired, Duration::from_nanos(11));

        let report = timings.report();
        assert_eq!(report["schema"], "mycity-sim-timing-v1");
        assert_eq!(report["run_required"]["count"], 2);
        assert_eq!(report["run_required"]["total_ns"], 18);
        assert_eq!(report["run_required"]["max_ns"], 11);
        assert_eq!(report["status_state_digest"]["count"], 0);
        assert_eq!(report.as_object().unwrap().len(), 13);
    }
}
