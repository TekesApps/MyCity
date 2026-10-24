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
}

impl DueError {
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::StoreUnavailable => "store_unavailable",
            Self::MissingTimeline => "missing_timeline",
            Self::InvalidPendingSnapshot => "invalid_pending_snapshot",
            Self::SelectionChanged => "selection_changed",
        }
    }
}

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
