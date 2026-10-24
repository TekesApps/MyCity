//! Bounded selection for the real continuous-supply timeline lane.
//!
//! This module is read-only. A selection binds an existing authenticated source
//! and accepted Timeline responsibility; only the original Store may reserve,
//! execute, receipt, and recover an Advance.

use super::DueError;
use mycity_core::{
    arena::Scope,
    continuous_supply::{self, DueView},
    schedule::Phase,
    task_registry::{
        evaluate, CachedOutput, ClockInstant, ClockKind, ConsumerDemand,
        RequestCursor, ScopeIdentity, Snapshot as RegistrySnapshot, Source, TaskKey,
        WakeCandidate, WorkWindow,
    },
    timeline::Progress,
};
use mycity_storage::{Continuation, Store, CONTINUATION_MAX_PAGES, CONTINUATION_PAGE_BYTES};

pub const SUPPLY_ADMISSION_POLICY_VERSION: u32 = 1;
pub const SUPPLY_ADMISSION_PAGE_LIMIT: u16 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplySelectionMode {
    RegistrySelected,
    MandatoryFallback,
    MandatoryZeroBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyPlanKind {
    FundedContinuation,
    NextBoundaryAdmission,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyZeroReason {
    ZeroWidthBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SupplySelection {
    NoWork,
    Busy,
    Selected(SelectedSupplyContinuation),
}

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

impl From<&Continuation> for ContinuationBinding {
    fn from(value: &Continuation) -> Self {
        Self {
            start: value.start,
            at: value.at,
            target: value.target,
            next_serial: value.next_serial,
            page_limit: value.page_limit,
            remaining_pages: value.remaining_pages,
            remaining_bytes: value.remaining_bytes,
        }
    }
}

/// Private fields make this a snapshot-bound intent, not authority to create
/// funding or execute a page. The owning Store must still accept the original
/// Timeline envelope and preserve its receipt/recovery rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedSupplyContinuation {
    key: Option<TaskKey>,
    mode: SupplySelectionMode,
    plan_kind: SupplyPlanKind,
    policy_version: Option<u32>,
    zero_reason: Option<SupplyZeroReason>,
    target: u64,
    page_limit: u16,
    sequence: u64,
    scope: Scope,
    progress: Progress,
    due: DueView,
    continuation: Option<ContinuationBinding>,
}

impl SelectedSupplyContinuation {
    pub fn target(&self) -> u64 { self.target }
    pub fn page_limit(&self) -> u16 { self.page_limit }
    pub fn key(&self) -> Option<&TaskKey> { self.key.as_ref() }
    pub fn mode(&self) -> SupplySelectionMode { self.mode }
    pub fn plan_kind(&self) -> SupplyPlanKind { self.plan_kind }
    pub fn policy_version(&self) -> Option<u32> { self.policy_version }
    pub fn zero_reason(&self) -> Option<SupplyZeroReason> { self.zero_reason }
    pub fn at(&self) -> u64 { self.progress.at }
    pub fn sequence(&self) -> u64 { self.sequence }
}

#[derive(Clone)]
struct SupplySnapshot {
    sequence: u64,
    scope: Scope,
    progress: Progress,
    due: DueView,
    continuation: Option<ContinuationBinding>,
    key: Option<TaskKey>,
    target: u64,
    page_limit: u16,
    plan_kind: SupplyPlanKind,
    policy_version: Option<u32>,
    zero_reason: Option<SupplyZeroReason>,
}

fn nonzero_digest(value: &[u8; 32]) -> bool {
    value.iter().any(|byte| *byte != 0)
}

fn valid_phase(phase: Option<Phase>) -> bool {
    matches!(phase, Some(Phase::P0 | Phase::P1 | Phase::P2 | Phase::P3 | Phase::P4 | Phase::P5))
}

fn supply_key(due: DueView, progress: Progress) -> Option<TaskKey> {
    let window = WorkWindow { start: progress.last_complete, end: progress.at };
    if !window.is_valid() { return None; }
    let scope = ScopeIdentity {
        world_id: due.scope.world(),
        branch_id: due.scope.branch(),
        city_id: Some(due.city_id),
    };
    // The authenticated continuous-supply opening fixes this profile to a
    // Development clock for its city; its registration/fingerprint bind that
    // configuration rather than accepting a caller-provided clock.
    Some(TaskKey {
        scope,
        clock: ClockKind::Development { city_id: due.city_id },
        source_id: due.source_id,
        source_version: u64::from(due.version),
        window,
        cursor: due.cursor,
    })
}

fn read_snapshot(store: &Store) -> Result<Option<SupplySnapshot>, DueError> {
    let engine = store.engine().map_err(|_| DueError::StoreUnavailable)?;
    if !engine.continuous_supply_is_authenticated() {
        return Err(DueError::InvalidSupplyDueView);
    }
    let Some(due) = engine.continuous_supply_due_state().map_err(|_| DueError::InvalidSupplyDueView)? else {
        return Ok(None);
    };
    let (scope, _) = engine.world().scope_revision();
    if due.version != continuous_supply::VERSION
        || due.scope != scope
        || due.city_id == 0
        || due.source_id == 0
        || !nonzero_digest(&due.fingerprint)
        || !nonzero_digest(&due.registration)
        || engine.continuous_supply_registration() != Some(due.registration)
    {
        return Err(DueError::InvalidSupplyDueView);
    }

    let sequence = store.committed_sequence();
    let Some(timeline) = engine.timeline() else { return Ok(None); };
    let progress = timeline.progress;
    let raw_continuation = store.continuation_status().map_err(|_| DueError::StoreUnavailable)?;

    if let Some(phase) = progress.phase {
        if !valid_phase(Some(phase)) || progress.target.is_none() {
            return Err(DueError::InvalidSupplySnapshot);
        }
        let Some(continuation) = raw_continuation.as_ref() else {
            return Err(DueError::MissingSupplyContinuation);
        };
        if continuation.remaining_pages == 0 || continuation.remaining_bytes == 0 {
            return Err(DueError::SupplyContinuationExhausted);
        }
        if continuation.remaining_pages > CONTINUATION_MAX_PAGES
            || u64::from(continuation.remaining_pages).checked_mul(CONTINUATION_PAGE_BYTES)
                != Some(continuation.remaining_bytes)
            || continuation.start != progress.last_complete
            || continuation.at != progress.at
            || Some(continuation.target) != progress.target
            || continuation.next_serial != progress.serial
            || continuation.at < continuation.start
            || continuation.at - continuation.start > 3600
            || continuation.target < continuation.at
            || continuation.page_limit == 0
            || continuation.page_limit > mycity_core::timeline::PAGE_LIMIT
        {
            return Err(DueError::InvalidSupplySnapshot);
        }
        if due.cursor < progress.last_complete || due.cursor > progress.at {
            return Err(DueError::InvalidSupplyDueView);
        }
        let zero = progress.last_complete == progress.at;
        let key = if zero { None } else { supply_key(due, progress) };
        if !zero && key.is_none() { return Err(DueError::InvalidSupplySnapshot); }
        return Ok(Some(SupplySnapshot {
            sequence,
            scope,
            progress,
            due,
            continuation: Some(ContinuationBinding::from(continuation)),
            key,
            target: continuation.target,
            page_limit: continuation.page_limit,
            plan_kind: SupplyPlanKind::FundedContinuation,
            policy_version: None,
            zero_reason: zero.then_some(SupplyZeroReason::ZeroWidthBoundary),
        }));
    }

    match (progress.target, raw_continuation) {
        (None, None) => {
            if due.cursor != progress.last_complete || progress.at != progress.last_complete {
                return Err(DueError::InvalidSupplyDueView);
            }
            Ok(None)
        }
        (Some(target), None) => {
            if progress.at != progress.last_complete || target <= progress.at || due.cursor != progress.at {
                return Err(DueError::InvalidSupplySnapshot);
            }
            Ok(Some(SupplySnapshot {
                sequence,
                scope,
                progress,
                due,
                continuation: None,
                key: None,
                target,
                page_limit: SUPPLY_ADMISSION_PAGE_LIMIT,
                plan_kind: SupplyPlanKind::NextBoundaryAdmission,
                policy_version: Some(SUPPLY_ADMISSION_POLICY_VERSION),
                zero_reason: Some(SupplyZeroReason::ZeroWidthBoundary),
            }))
        }
        (_, Some(_)) => Err(DueError::InvalidSupplySnapshot),
    }
}

fn registry_candidate(snapshot: &SupplySnapshot) -> Option<WakeCandidate> {
    let key = snapshot.key?;
    let token = snapshot.sequence.checked_add(1).filter(|value| *value != 0)?;
    let source = [Source {
        scope: key.scope,
        id: key.source_id,
        version: key.source_version,
        clock: key.clock,
        due: Some(snapshot.progress.at),
        dirty: snapshot.due.cursor < snapshot.progress.at,
        necessary: true,
        window: key.window,
        cursor: key.cursor,
    }];
    let demand = [ConsumerDemand {
        scope: key.scope,
        consumer_id: key.source_id,
        source_id: key.source_id,
        clock: key.clock,
        required_version: key.source_version,
        window: key.window,
        max_age_seconds: 0,
        necessary: true,
    }];
    let now = ClockInstant { kind: key.clock, seconds: snapshot.progress.at };
    evaluate(
        RegistrySnapshot {
            scope: key.scope,
            token,
            sources: &source,
            demands: &demand,
            caches: &[] as &[CachedOutput],
            outstanding: &[],
        },
        now,
        RequestCursor::begin(key.scope, token, now),
        1,
    ).ok()?.items.into_iter().next()
}

fn selected(snapshot: SupplySnapshot, mode: SupplySelectionMode) -> SupplySelection {
    SupplySelection::Selected(SelectedSupplyContinuation {
        key: snapshot.key,
        mode,
        plan_kind: snapshot.plan_kind,
        policy_version: snapshot.policy_version,
        zero_reason: snapshot.zero_reason,
        target: snapshot.target,
        page_limit: snapshot.page_limit,
        sequence: snapshot.sequence,
        scope: snapshot.scope,
        progress: snapshot.progress,
        due: snapshot.due,
        continuation: snapshot.continuation,
    })
}

pub fn select_supply_continuation(
    store: &Store,
    worker_active_execution: bool,
) -> Result<SupplySelection, DueError> {
    select_supply_inner(store, worker_active_execution, true)
}

pub fn select_supply_continuation_with_missing_hint(
    store: &Store,
    worker_active_execution: bool,
) -> Result<SupplySelection, DueError> {
    select_supply_inner(store, worker_active_execution, false)
}

fn select_supply_inner(
    store: &Store,
    worker_active_execution: bool,
    observe: bool,
) -> Result<SupplySelection, DueError> {
    let Some(snapshot) = read_snapshot(store)? else { return Ok(SupplySelection::NoWork); };
    if worker_active_execution { return Ok(SupplySelection::Busy); }
    if snapshot.plan_kind == SupplyPlanKind::NextBoundaryAdmission {
        return Ok(selected(snapshot, SupplySelectionMode::MandatoryZeroBoundary));
    }
    if snapshot.key.is_none() {
        return Ok(selected(snapshot, SupplySelectionMode::MandatoryZeroBoundary));
    }
    let key = snapshot.key;
    let mode = if observe && key.is_some_and(|key| registry_candidate(&snapshot).is_some_and(|candidate| {
        candidate.key == key
            && candidate.clock == key.clock
            && candidate.due == Some(snapshot.progress.at)
            && candidate.necessary
    })) {
        SupplySelectionMode::RegistrySelected
    } else {
        SupplySelectionMode::MandatoryFallback
    };
    Ok(selected(snapshot, mode))
}

pub fn validate_supply_continuation(
    store: &Store,
    selected: &SelectedSupplyContinuation,
    worker_active_execution: bool,
) -> Result<(), DueError> {
    if worker_active_execution { return Err(DueError::SelectionChanged); }
    let current = read_snapshot(store).map_err(|error| match error {
        DueError::StoreUnavailable => DueError::StoreUnavailable,
        _ => DueError::SelectionChanged,
    })?.ok_or(DueError::SelectionChanged)?;
    if current.sequence != selected.sequence
        || current.scope != selected.scope
        || current.progress != selected.progress
        || current.due != selected.due
        || current.continuation != selected.continuation
        || current.key != selected.key
        || current.target != selected.target
        || current.page_limit != selected.page_limit
        || current.plan_kind != selected.plan_kind
        || current.policy_version != selected.policy_version
        || current.zero_reason != selected.zero_reason
    {
        return Err(DueError::SelectionChanged);
    }
    Ok(())
}
