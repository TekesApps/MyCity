//! Finite read-only selection for the existing required-job owner.
//!
//! This module only revalidates a real Store snapshot and returns a hint about
//! which required occurrence the existing owner should continue. It does not
//! create a queue, worker, clock, receipt, lease, or acknowledgement.

use crate::task_registry_adapter::{
    observe_required, ActualJobBinding, HintState, Observation, OwnerView,
};
use mycity_core::{
    arena::Scope,
    background::Failure,
    schedule::Phase,
    task_registry::{ScopeIdentity, TaskKey},
};
use mycity_storage::Store;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    RegistrySelected,
    MandatoryFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequiredBlock {
    AttemptLimitReached,
    InvalidInputFailure,
}

impl RequiredBlock {
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::AttemptLimitReached => "attempt_limit_reached",
            Self::InvalidInputFailure => "invalid_input_failure",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    NoWork,
    Busy,
    Backpressure(RequiredBlock),
    Selected(SelectedRequired),
}

/// Private fields make the selection a snapshot-bound capability, not an
/// owner-supplied job/token DTO. The job accessor is only for the existing
/// required path; it does not authorize dispatch by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedRequired {
    job: u64,
    scope: Scope,
    committed_sequence: u64,
    at: u64,
    last_complete: u64,
    phase: Phase,
    binding: ActualJobBinding,
    key: TaskKey,
    mode: SelectionMode,
}

impl SelectedRequired {
    pub fn job(&self) -> u64 {
        self.job
    }

    pub fn mode(&self) -> SelectionMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DueError {
    StoreUnavailable,
    MissingTimeline,
    InvalidPendingSnapshot,
    SelectionChanged,
    InvalidDueView,
    MissingContinuation,
    ContinuationExhausted,
    InvalidServiceSnapshot,
    InvalidSupplyDueView,
    InvalidSupplySnapshot,
    MissingSupplyContinuation,
    SupplyContinuationExhausted,
}

impl DueError {
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::StoreUnavailable => "store_unavailable",
            Self::MissingTimeline => "missing_timeline",
            Self::InvalidPendingSnapshot => "invalid_pending_snapshot",
            Self::SelectionChanged => "selection_changed",
            Self::InvalidDueView => "invalid_due_view",
            Self::MissingContinuation => "missing_continuation",
            Self::ContinuationExhausted => "continuation_exhausted",
            Self::InvalidServiceSnapshot => "invalid_service_snapshot",
            Self::InvalidSupplyDueView => "invalid_supply_due_view",
            Self::InvalidSupplySnapshot => "invalid_supply_snapshot",
            Self::MissingSupplyContinuation => "missing_supply_continuation",
            Self::SupplyContinuationExhausted => "supply_continuation_exhausted",
        }
    }
}

mod supply;
pub use supply::{
    select_supply_continuation, select_supply_continuation_with_missing_hint,
    validate_supply_continuation, SelectedSupplyContinuation, SupplyPlanKind,
    SupplySelection, SupplySelectionMode, SupplyZeroReason,
    SUPPLY_ADMISSION_PAGE_LIMIT, SUPPLY_ADMISSION_POLICY_VERSION,
};

impl std::fmt::Display for DueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_code())
    }
}

impl std::error::Error for DueError {}

#[derive(Clone)]
struct Snapshot {
    job: u64,
    scope: Scope,
    scope_identity: ScopeIdentity,
    committed_sequence: u64,
    at: u64,
    last_complete: u64,
    phase: Phase,
    binding: ActualJobBinding,
    key: TaskKey,
}

fn read_snapshot(store: &Store) -> Result<Option<Snapshot>, DueError> {
    let engine = store.engine().map_err(|_| DueError::StoreUnavailable)?;
    let Some(background) = engine.background() else {
        return Ok(None);
    };
    let Some(job) = background.pending() else {
        return Ok(None);
    };
    let timeline = engine.timeline().ok_or(DueError::MissingTimeline)?;
    let progress = timeline.progress;
    let phase = match progress.phase {
        Some(phase @ (Phase::P1 | Phase::P2 | Phase::P3 | Phase::P4)) => phase,
        _ => return Err(DueError::InvalidPendingSnapshot),
    };
    if job.id == 0
        || job.start >= job.end
        || u64::from(job.start) != progress.last_complete
        || u64::from(job.end) != progress.at
        || progress.at <= progress.last_complete
        || job.input.target != job.end
        || job.generation != u32::from(job.attempt)
        || (job.running && job.failure.is_some())
    {
        return Err(DueError::InvalidPendingSnapshot);
    }
    let (scope, _) = engine.world().scope_revision();
    let scope_identity = ScopeIdentity {
        world_id: scope.world(),
        branch_id: scope.branch(),
        // The current Background Job is world-scoped and carries no city.
        city_id: None,
    };
    if scope_identity.world_id == 0 || scope_identity.branch_id == 0 {
        return Err(DueError::InvalidPendingSnapshot);
    }
    let observation = observe_required(
        background,
        OwnerView {
            scope,
            city_id: None,
            world_time_seconds: progress.at,
            snapshot_token: store.committed_sequence().checked_add(1),
        },
        &[],
    );
    let (Some(binding), Some(key)) = (observation.attempt, observation.business_key) else {
        return Err(DueError::InvalidPendingSnapshot);
    };
    Ok(Some(Snapshot {
        job: job.id,
        scope,
        scope_identity,
        committed_sequence: store.committed_sequence(),
        at: progress.at,
        last_complete: progress.last_complete,
        phase,
        binding,
        key,
    }))
}

fn classify(
    snapshot: Snapshot,
    worker_active_execution: bool,
    hint: Option<&Observation>,
    expected_token: Option<u64>,
) -> Selection {
    if worker_active_execution {
        return Selection::Busy;
    }
    let observation = hint.cloned().unwrap_or_else(|| {
        // Missing advisory state is a mandatory fallback, never an absence of
        // required work. This observation is generated from this real Job.
        Observation {
            state: HintState::Unknown(
                crate::task_registry_adapter::UnknownReason::MissingOwnerSnapshotToken,
            ),
            business_key: Some(snapshot.key),
            attempt: Some(snapshot.binding.clone()),
            candidate: None,
            snapshot_token: None,
            snapshot_token_durable: false,
        }
    });
    let registry_selected = matches!(observation.state, HintState::Candidate)
        && observation.business_key == Some(snapshot.key)
        && observation.attempt.as_ref() == Some(&snapshot.binding)
        && observation.candidate.is_some_and(|candidate| {
            candidate.key == snapshot.key
                && candidate.clock == snapshot.key.clock
                && candidate.due == Some(snapshot.key.window.end)
                && candidate.necessary
        })
        && observation.snapshot_token == expected_token
        && !observation.snapshot_token_durable;
    Selection::Selected(SelectedRequired {
        job: snapshot.job,
        scope: snapshot.scope,
        committed_sequence: snapshot.committed_sequence,
        at: snapshot.at,
        last_complete: snapshot.last_complete,
        phase: snapshot.phase,
        binding: snapshot.binding,
        key: snapshot.key,
        mode: if registry_selected {
            SelectionMode::RegistrySelected
        } else {
            SelectionMode::MandatoryFallback
        },
    })
}

pub fn select_required(
    store: &Store,
    worker_active_execution: bool,
) -> Result<Selection, DueError> {
    select_required_inner(store, worker_active_execution, true)
}

/// Explicit no-hint path for proving a missing advisory cannot hide required
/// work. It never changes the actual Store snapshot or dispatch authority.
pub fn select_required_with_missing_hint(
    store: &Store,
    worker_active_execution: bool,
) -> Result<Selection, DueError> {
    select_required_inner(store, worker_active_execution, false)
}

fn select_required_inner(
    store: &Store,
    worker_active_execution: bool,
    observe_current: bool,
) -> Result<Selection, DueError> {
    let Some(snapshot) = read_snapshot(store)? else {
        return Ok(Selection::NoWork);
    };
    if worker_active_execution {
        return Ok(Selection::Busy);
    }
    if let Some(block) = if snapshot.binding.attempt >= 3 {
        Some(RequiredBlock::AttemptLimitReached)
    } else if snapshot.binding.failure == Some(Failure::InvalidInput) {
        Some(RequiredBlock::InvalidInputFailure)
    } else {
        None
    } {
        return Ok(Selection::Backpressure(block));
    }
    let owned_observation = if observe_current {
        let engine = store.engine().map_err(|_| DueError::StoreUnavailable)?;
        let background = engine.background().ok_or(DueError::InvalidPendingSnapshot)?;
        Some(observe_required(
            background,
            OwnerView {
                scope: snapshot.scope,
                city_id: None,
                world_time_seconds: snapshot.at,
                snapshot_token: store.committed_sequence().checked_add(1),
            },
            &[],
        ))
    } else {
        None
    };
    Ok(classify(
        snapshot,
        worker_active_execution,
        owned_observation.as_ref(),
        store.committed_sequence().checked_add(1),
    ))
}

/// Verify that the private snapshot capability still describes the same
/// Store/Timeline/Job. Diagnostic mode is not a snapshot field and is not
/// recomputed here; a missing hint may have caused a valid fallback selection.
pub fn validate_required(
    store: &Store,
    selected: &SelectedRequired,
    worker_active_execution: bool,
) -> Result<(), DueError> {
    if worker_active_execution {
        return Err(DueError::SelectionChanged);
    }
    let current = match read_snapshot(store) {
        Ok(Some(current)) => current,
        Ok(None) | Err(DueError::MissingTimeline | DueError::InvalidPendingSnapshot) => {
            return Err(DueError::SelectionChanged)
        }
        Err(error) => return Err(error),
    };
    if current.job != selected.job
        || current.scope != selected.scope
        || current.scope_identity != selected.key.scope
        || current.committed_sequence != selected.committed_sequence
        || current.at != selected.at
        || current.last_complete != selected.last_complete
        || current.phase != selected.phase
        || current.binding != selected.binding
        || current.key != selected.key
    {
        return Err(DueError::SelectionChanged);
    }
    Ok(())
}

// Service-needs P5 continuation is a separate required lane from Background.
// Keep this typed API independent so the existing Background selection and
// dispatch contract above remains unchanged.

use mycity_core::{
    service_needs::{self, Subphase},
    service_needs_authority,
    task_registry::{
        evaluate, CachedOutput, ClockInstant, ClockKind, ConsumerDemand,
        Snapshot as RegistrySnapshot, Source, WakeCandidate, WorkWindow,
    },
    timeline::Progress,
};

use mycity_storage::{CONTINUATION_MAX_PAGES, CONTINUATION_PAGE_BYTES};

pub const SERVICE_NEEDS_SOURCE_ID: u64 = 0x534e_0000_0000_0001;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ContinuationBinding {
    start: u64,
    at: u64,
    target: u64,
    next_serial: u64,
    page_limit: u16,
    remaining_pages: u32,
    remaining_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceSelection {
    NoWork,
    Busy,
    Selected(SelectedServiceContinuation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedServiceContinuation {
    key: TaskKey,
    mode: SelectionMode,
    sequence: u64,
    scope: Scope,
    progress: Progress,
    due: service_needs_authority::DueView,
    continuation: ContinuationBinding,
}

impl SelectedServiceContinuation {
    pub fn target(&self) -> u64 { self.continuation.target }
    pub fn page_limit(&self) -> u16 { self.continuation.page_limit }
    pub fn key(&self) -> &TaskKey { &self.key }
    pub fn mode(&self) -> SelectionMode { self.mode }
    pub fn at(&self) -> u64 { self.progress.at }
    pub fn sequence(&self) -> u64 { self.sequence }
}

#[derive(Clone)]
struct ServiceSnapshot {
    scope: Scope,
    identity: ScopeIdentity,
    sequence: u64,
    progress: Progress,
    due: service_needs_authority::DueView,
    continuation: ContinuationBinding,
    key: TaskKey,
}

fn nonzero_digest(value: &[u8; 32]) -> bool { value.iter().any(|byte| *byte != 0) }

/// Cursor zero means the accepted P5 page has not started its service run.
/// Started cursors encode subphase and row, so Deadlines row zero differs from
/// both the unstarted state and Occurrences row zero.
fn service_cursor(run: Option<service_needs::DueRun>) -> u64 {
    match run {
        None => 0,
        Some(run) => {
            let subphase = match run.subphase { Subphase::Deadlines => 0, Subphase::Occurrences => 1 };
            1 + u64::from(run.next_household) * 2 + subphase
        }
    }
}

fn service_snapshot(store: &Store) -> Result<Option<ServiceSnapshot>, DueError> {
    let engine = store.engine().map_err(|_| DueError::StoreUnavailable)?;
    let Some(due) = engine.service_needs_due_state().map_err(|_| DueError::InvalidDueView)? else {
        return Ok(None);
    };
    if due.version != service_needs::VERSION
        || due.city_id == 0
        || !nonzero_digest(&due.fingerprint)
        || !nonzero_digest(&due.registration)
        || engine.service_needs_registration() != Some(due.registration)
    {
        return Err(DueError::InvalidDueView);
    }
    let Some(timeline) = engine.timeline() else {
        if due.pending.is_some() || due.covered_through != 0 { return Err(DueError::MissingTimeline); }
        return Ok(None);
    };
    let progress = timeline.progress;
    let p5 = progress.phase == Some(Phase::P5);
    if due.covered_through < progress.last_complete || due.covered_through > progress.at {
        return Err(DueError::InvalidDueView);
    }
    let due_owned = due.pending.is_some()
        || due.covered_through < progress.at
        || due.next_due == Some(progress.at);
    if !p5 {
        if due.pending.is_some() || due.covered_through != progress.last_complete {
            return Err(DueError::InvalidServiceSnapshot);
        }
        return Ok(None);
    }
    if !due_owned { return Ok(None); }
    if progress.at <= progress.last_complete
        || progress.target.is_none()
        || due.covered_through != progress.last_complete && due.covered_through != progress.at
        || due.pending.is_some_and(|run| run.at != progress.at)
    {
        return Err(DueError::InvalidDueView);
    }
    let Some(raw_continuation) = store.continuation_status().map_err(|_| DueError::StoreUnavailable)? else {
        return Err(DueError::MissingContinuation);
    };
    if raw_continuation.remaining_pages == 0 || raw_continuation.remaining_bytes == 0 {
        return Err(DueError::ContinuationExhausted);
    }
    if raw_continuation.remaining_pages > CONTINUATION_MAX_PAGES
        || raw_continuation.remaining_bytes
            != u64::from(raw_continuation.remaining_pages) * CONTINUATION_PAGE_BYTES
        || raw_continuation.at < raw_continuation.start
        || raw_continuation.at - raw_continuation.start > 3600
        || raw_continuation.target < raw_continuation.at
        || raw_continuation.start != progress.last_complete
        || raw_continuation.at != progress.at
        || Some(raw_continuation.target) != progress.target
        || raw_continuation.next_serial != progress.serial
        || raw_continuation.page_limit == 0
        || raw_continuation.page_limit > mycity_core::timeline::PAGE_LIMIT
    {
        return Err(DueError::InvalidServiceSnapshot);
    }
    let continuation = ContinuationBinding {
        start: raw_continuation.start,
        at: raw_continuation.at,
        target: raw_continuation.target,
        next_serial: raw_continuation.next_serial,
        page_limit: raw_continuation.page_limit,
        remaining_pages: raw_continuation.remaining_pages,
        remaining_bytes: raw_continuation.remaining_bytes,
    };
    let (scope, _) = engine.world().scope_revision();
    let identity = ScopeIdentity {
        world_id: scope.world(),
        branch_id: scope.branch(),
        city_id: Some(due.city_id),
    };
    if identity.world_id == 0 || identity.branch_id == 0 {
        return Err(DueError::InvalidServiceSnapshot);
    }
    let window = WorkWindow { start: progress.last_complete, end: progress.at };
    if !window.is_valid() { return Err(DueError::InvalidServiceSnapshot); }
    let key = TaskKey {
        scope: identity,
        clock: ClockKind::Development { city_id: due.city_id },
        source_id: SERVICE_NEEDS_SOURCE_ID,
        // Schema version for the stable city service occurrence, not a model version.
        source_version: 1,
        window,
        cursor: service_cursor(due.pending),
    };
    Ok(Some(ServiceSnapshot {
        scope,
        identity,
        sequence: store.committed_sequence(),
        progress,
        due,
        continuation,
        key,
    }))
}

fn registry_candidate(snapshot: &ServiceSnapshot) -> Option<WakeCandidate> {
    let token = snapshot.sequence.checked_add(1).filter(|token| *token != 0)?;
    let key = snapshot.key;
    let source = [Source {
        scope: snapshot.identity,
        id: key.source_id,
        version: key.source_version,
        clock: key.clock,
        due: Some(snapshot.progress.at),
        dirty: false,
        necessary: true,
        window: key.window,
        cursor: key.cursor,
    }];
    let demand = [ConsumerDemand {
        scope: snapshot.identity,
        consumer_id: key.source_id,
        source_id: key.source_id,
        clock: key.clock,
        required_version: key.source_version,
        window: key.window,
        max_age_seconds: 0,
        necessary: true,
    }];
    let now = ClockInstant { kind: key.clock, seconds: snapshot.progress.at };
    let page = evaluate(
        RegistrySnapshot {
            scope: snapshot.identity,
            token,
            sources: &source,
            demands: &demand,
            caches: &[] as &[CachedOutput],
            outstanding: &[],
        },
        now,
        mycity_core::task_registry::RequestCursor::begin(snapshot.identity, token, now),
        1,
    ).ok()?;
    page.items.into_iter().next()
}

fn service_selection(snapshot: ServiceSnapshot, mode: SelectionMode) -> ServiceSelection {
    ServiceSelection::Selected(SelectedServiceContinuation {
        key: snapshot.key,
        mode,
        sequence: snapshot.sequence,
        scope: snapshot.scope,
        progress: snapshot.progress,
        due: snapshot.due,
        continuation: snapshot.continuation,
    })
}

pub fn select_service_continuation(
    store: &Store,
    worker_active_execution: bool,
) -> Result<ServiceSelection, DueError> {
    let Some(snapshot) = service_snapshot(store)? else { return Ok(ServiceSelection::NoWork); };
    if worker_active_execution { return Ok(ServiceSelection::Busy); }
    let mode = registry_candidate(&snapshot).filter(|candidate| {
        candidate.key == snapshot.key
            && candidate.clock == snapshot.key.clock
            && candidate.due == Some(snapshot.progress.at)
            && candidate.necessary
    }).map_or(SelectionMode::MandatoryFallback, |_| SelectionMode::RegistrySelected);
    Ok(service_selection(snapshot, mode))
}

/// Explicitly simulate a missing registry hint; the accepted Store obligation
/// remains selected and can only proceed through the original Advance receipt.
pub fn select_service_continuation_with_missing_hint(
    store: &Store,
    worker_active_execution: bool,
) -> Result<ServiceSelection, DueError> {
    let Some(snapshot) = service_snapshot(store)? else { return Ok(ServiceSelection::NoWork); };
    if worker_active_execution { return Ok(ServiceSelection::Busy); }
    Ok(service_selection(snapshot, SelectionMode::MandatoryFallback))
}

pub fn validate_service_continuation(
    store: &Store,
    selected: &SelectedServiceContinuation,
    worker_active_execution: bool,
) -> Result<(), DueError> {
    if worker_active_execution { return Err(DueError::SelectionChanged); }
    let Some(current) = service_snapshot(store).map_err(|error| match error {
        DueError::MissingTimeline | DueError::MissingContinuation | DueError::ContinuationExhausted
        | DueError::InvalidDueView | DueError::InvalidServiceSnapshot => DueError::SelectionChanged,
        other => other,
    })? else { return Err(DueError::SelectionChanged); };
    if current.scope != selected.scope
        || current.identity != selected.key.scope
        || current.sequence != selected.sequence
        || current.progress != selected.progress
        || current.due != selected.due
        || current.continuation != selected.continuation
        || current.key != selected.key
    {
        return Err(DueError::SelectionChanged);
    }
    Ok(())
}

#[cfg(test)]
mod service_cursor_tests {
    use super::service_cursor;
    use mycity_core::service_needs::{DueRun, Subphase};

    #[test]
    fn service_cursor_separates_unstarted_and_subphase_row_zero() {
        assert_eq!(service_cursor(None), 0);
        assert_eq!(service_cursor(Some(DueRun { at: 28_800, next_household: 0, subphase: Subphase::Deadlines })), 1);
        assert_eq!(service_cursor(Some(DueRun { at: 28_800, next_household: 0, subphase: Subphase::Occurrences })), 2);
    }
}
