//! Advisory bridge from the real Sim-owned pending air job to the core registry.
//!
//! This module never leases, dispatches, acknowledges, accepts, or mutates a job.
//! Its result must not gate the existing `sim_host::required` path.

use mycity_core::{
    arena::Scope,
    background::{Background, Failure, Job},
    task_registry::{
        evaluate, CachedOutput, ClockInstant, ClockKind, ConsumerDemand, Outstanding,
        RegistryError, ScopeIdentity, Snapshot, Source, TaskKey, WakeCandidate, WorkWindow,
    },
};

/// The owner supplies the actual current world time and page token. The token is
/// ephemeral and only binds one immutable evaluation snapshot; it is not durable.
#[derive(Clone, Copy, Debug)]
pub struct OwnerView {
    pub scope: Scope,
    pub city_id: Option<u32>,
    pub world_time_seconds: u64,
    pub snapshot_token: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    MissingPendingJob,
    MissingOwnerSnapshotToken,
    InvalidOwnerContext,
    InvalidJobWindow,
    InvalidJobBinding,
    RegistryRejected(RegistryError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardBlockReason {
    AttemptLimitReached,
    InvalidInputFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoHintReason {
    AlreadyRunning,
    NotYetDue,
    NoCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActualJobBinding {
    pub scope: ScopeIdentity,
    pub clock: ClockKind,
    pub id: u64,
    pub base_revision: u64,
    pub field_version: u64,
    pub start: u32,
    pub end: u32,
    pub input_identity: String,
    pub checkpoint_sha256: [u8; 32],
    pub incarnation: u64,
    pub generation: u32,
    pub attempt: u8,
    pub running: bool,
    pub failure: Option<Failure>,
}

impl ActualJobBinding {
    fn from_job(scope: ScopeIdentity, job: &Job) -> Self {
        Self {
            scope,
            clock: ClockKind::World,
            id: job.id,
            base_revision: job.base_revision,
            field_version: job.field_version,
            start: job.start,
            end: job.end,
            input_identity: job.input.identity(),
            checkpoint_sha256: job.checkpoint_sha256,
            incarnation: job.incarnation,
            generation: job.generation,
            attempt: job.attempt,
            running: job.running,
            failure: job.failure,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HintState {
    Unknown(UnknownReason),
    HardBlocked(HardBlockReason),
    Candidate,
    NoHint(NoHintReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub state: HintState,
    /// Stable required-job identity; attempt fields are carried separately.
    pub business_key: Option<TaskKey>,
    pub attempt: Option<ActualJobBinding>,
    pub candidate: Option<WakeCandidate>,
    /// The owner token binds this advisory page to its immutable input snapshot.
    /// It is not part of the durable or business attempt identity.
    pub snapshot_token: Option<u64>,
    /// The core cursor token is owner supplied and ephemeral. No durable token
    /// or recovery authority is created by this adapter.
    pub snapshot_token_durable: bool,
}

fn observation(state: HintState, key: Option<TaskKey>, attempt: Option<ActualJobBinding>, candidate: Option<WakeCandidate>, snapshot_token: Option<u64>) -> Observation {
    Observation { state, business_key: key, attempt, candidate, snapshot_token, snapshot_token_durable: false }
}

fn stable_key(scope: ScopeIdentity, job: &Job) -> TaskKey {
    TaskKey {
        scope,
        clock: ClockKind::World,
        source_id: job.id,
        // The registry version identifies this immutable required occurrence.
        // It is deliberately independent of valid field_version == 0.
        source_version: 1,
        window: WorkWindow { start: u64::from(job.start), end: u64::from(job.end) },
        // Background retries recompute the whole frozen input; there is no
        // durable subtask cursor in Job.
        cursor: 0,
    }
}

/// Read only the real Background::pending() record and propose whether its
/// required interval should be visible to the registry. A missing token, cache,
/// duplicate hint, or hard failure never consumes the background obligation.
pub fn observe_required(
    background: &Background,
    owner: OwnerView,
    caches: &[CachedOutput],
) -> Observation {
    let Some(job) = background.pending() else {
        return observation(HintState::Unknown(UnknownReason::MissingPendingJob), None, None, None, owner.snapshot_token);
    };
    let scope = ScopeIdentity {
        world_id: owner.scope.world(),
        branch_id: owner.scope.branch(),
        city_id: owner.city_id,
    };
    let binding = ActualJobBinding::from_job(scope, job);
    let key = stable_key(scope, job);
    let Some(token) = owner.snapshot_token.filter(|token| *token != 0) else {
        return observation(HintState::Unknown(UnknownReason::MissingOwnerSnapshotToken), Some(key), Some(binding), None, owner.snapshot_token);
    };
    if scope.world_id == 0
        || scope.branch_id == 0
        || scope.city_id == Some(0)
    {
        return observation(HintState::Unknown(UnknownReason::InvalidOwnerContext), Some(key), Some(binding), None, owner.snapshot_token);
    }
    if job.id == 0
        || job.generation != u32::from(job.attempt)
        || (job.running && job.failure.is_some())
    {
        return observation(HintState::Unknown(UnknownReason::InvalidJobBinding), Some(key), Some(binding), None, owner.snapshot_token);
    }
    if job.start >= job.end || job.input.target != job.end {
        return observation(HintState::Unknown(UnknownReason::InvalidJobWindow), Some(key), Some(binding), None, owner.snapshot_token);
    }
    if job.attempt >= 3 {
        return observation(HintState::HardBlocked(HardBlockReason::AttemptLimitReached), Some(key), Some(binding), None, owner.snapshot_token);
    }
    if job.failure == Some(Failure::InvalidInput) {
        return observation(HintState::HardBlocked(HardBlockReason::InvalidInputFailure), Some(key), Some(binding), None, owner.snapshot_token);
    }

    let now = ClockInstant { kind: ClockKind::World, seconds: owner.world_time_seconds };
    let window = WorkWindow { start: u64::from(job.start), end: u64::from(job.end) };
    let source = Source {
        scope,
        id: job.id,
        version: 1,
        clock: ClockKind::World,
        due: Some(u64::from(job.end)),
        dirty: false,
        necessary: true,
        window,
        cursor: 0,
    };
    let demand = ConsumerDemand {
        scope,
        consumer_id: job.id,
        source_id: job.id,
        clock: ClockKind::World,
        required_version: 1,
        window,
        max_age_seconds: 0,
        necessary: true,
    };
    let demands: &[ConsumerDemand] = if owner.world_time_seconds >= u64::from(job.end) {
        std::slice::from_ref(&demand)
    } else {
        &[]
    };
    let key = stable_key(scope, job);
    let in_flight = [Outstanding { key, necessary: true }];
    let outstanding: &[Outstanding] = if job.running { &in_flight } else { &[] };
    let result = evaluate(
        Snapshot { scope, token, sources: std::slice::from_ref(&source), demands, caches, outstanding },
        now,
        mycity_core::task_registry::RequestCursor::begin(scope, token, now),
        1,
    );
    let page = match result {
        Ok(page) => page,
        Err(error) => return observation(HintState::Unknown(UnknownReason::RegistryRejected(error)), Some(key), Some(binding), None, owner.snapshot_token),
    };
    if let Some(candidate) = page.items.first().copied() {
        return observation(HintState::Candidate, Some(key), Some(binding), Some(candidate), owner.snapshot_token);
    }
    let reason = if job.running {
        NoHintReason::AlreadyRunning
    } else if owner.world_time_seconds < u64::from(job.end) {
        NoHintReason::NotYetDue
    } else {
        NoHintReason::NoCandidate
    };
    observation(HintState::NoHint(reason), Some(key), Some(binding), None, owner.snapshot_token)
}

/// A result may be associated with the current required attempt only when its
/// world/branch/city scope, World clock, and every actual Job binding field,
/// including attempt/incarnation/generation, match the current owner snapshot.
pub fn result_matches_pending(
    binding: &ActualJobBinding,
    current_scope: ScopeIdentity,
    current: Option<&Job>,
) -> bool {
    current.is_some_and(|job| *binding == ActualJobBinding::from_job(current_scope, job))
}
