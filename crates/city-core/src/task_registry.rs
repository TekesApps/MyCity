//! Pure, bounded task eligibility and wakeup candidates.
//!
//! This module does not own a clock, queue, worker, persistence boundary, or
//! acknowledgement. Its caller supplies current source/cache/demand snapshots;
//! returned candidates remain proposals until the existing owner accepts them.

pub const SOURCE_LIMIT: usize = 256;
pub const DEMAND_LIMIT: usize = 256;
pub const CACHE_LIMIT: usize = 512;
pub const OUTSTANDING_LIMIT: usize = 512;
pub const MAX_PAGE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClockKind {
    /// Seconds on the current world's simulation clock.
    World,
    /// One city's development clock, distinct from world and interaction time.
    Development { city_id: u32 },
    /// Monotonic inter-city interaction seconds.
    Interaction,
}

/// Identity supplied by the current owner. These are identifiers only; this
/// module does not create or advance world, branch, or city state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeIdentity {
    pub world_id: u64,
    pub branch_id: u64,
    pub city_id: Option<u32>,
}

impl ScopeIdentity {
    fn is_valid(self) -> bool {
        self.world_id != 0 && self.branch_id != 0 && self.city_id != Some(0)
    }
}

fn clock_matches_scope(clock: ClockKind, scope: ScopeIdentity) -> bool {
    !matches!(clock, ClockKind::Development { city_id } if scope.city_id != Some(city_id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockInstant {
    pub kind: ClockKind,
    pub seconds: u64,
}

impl ClockInstant {
    /// Age is defined only on the same clock and only for non-future observations.
    pub fn age_since(self, observed: Self) -> Option<u64> {
        (self.kind == observed.kind)
            .then(|| self.seconds.checked_sub(observed.seconds))
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkWindow {
    pub start: u64,
    pub end: u64,
}

impl WorkWindow {
    pub fn is_valid(self) -> bool {
        self.start < self.end
    }

    pub fn covers(self, requested: Self) -> bool {
        self.is_valid() && requested.is_valid() && self.start <= requested.start && self.end >= requested.end
    }
}

/// Stable identity across retries. Attempt number is deliberately absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskKey {
    pub scope: ScopeIdentity,
    pub clock: ClockKind,
    pub source_id: u64,
    pub source_version: u64,
    pub window: WorkWindow,
    pub cursor: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    pub scope: ScopeIdentity,
    pub id: u64,
    pub version: u64,
    pub clock: ClockKind,
    pub due: Option<u64>,
    pub dirty: bool,
    pub necessary: bool,
    pub window: WorkWindow,
    pub cursor: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsumerDemand {
    pub scope: ScopeIdentity,
    pub consumer_id: u64,
    pub source_id: u64,
    pub clock: ClockKind,
    pub required_version: u64,
    pub window: WorkWindow,
    pub max_age_seconds: u64,
    pub necessary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedOutput {
    pub scope: ScopeIdentity,
    pub source_id: u64,
    pub source_version: u64,
    pub clock: ClockKind,
    pub observed_at: u64,
    pub coverage: WorkWindow,
    pub valid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outstanding {
    pub key: TaskKey,
    pub necessary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WakeReason {
    Due,
    Dirty,
    MissingOrStaleForConsumer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitingReason {
    MissingSource,
    ContextMismatch,
    ClockMismatch,
    EvaluationClockMismatch,
    VersionUnavailable,
    CoverageUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitingDemand {
    pub consumer_id: Option<u64>,
    pub source_id: u64,
    pub reason: WaitingReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WakeCandidate {
    pub key: TaskKey,
    pub clock: ClockKind,
    pub due: Option<u64>,
    pub necessary: bool,
    pub reason: WakeReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestCursor {
    pub scope: ScopeIdentity,
    pub snapshot_token: u64,
    pub now: ClockInstant,
    pub offset: usize,
}

impl RequestCursor {
    pub fn begin(scope: ScopeIdentity, snapshot_token: u64, now: ClockInstant) -> Self {
        Self { scope, snapshot_token, now, offset: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WakePage {
    pub items: Vec<WakeCandidate>,
    pub waiting: Vec<WaitingDemand>,
    pub next: RequestCursor,
    pub more: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    Limit,
    InvalidInput,
    InvalidCursor,
    Allocation,
}

/// Immutable per-evaluation view supplied by the authoritative owner.
#[derive(Clone, Copy)]
pub struct Snapshot<'a> {
    pub scope: ScopeIdentity,
    /// Owner-monotonic token changed on any source, demand, cache, or
    /// outstanding-snapshot mutation. Persistence remains the owner's job.
    pub token: u64,
    pub sources: &'a [Source],
    pub demands: &'a [ConsumerDemand],
    pub caches: &'a [CachedOutput],
    pub outstanding: &'a [Outstanding],
}

fn cache_satisfies(
    cache: CachedOutput,
    source: Source,
    demand: ConsumerDemand,
    now: ClockInstant,
) -> bool {
    let age = now.age_since(ClockInstant {
        kind: cache.clock,
        seconds: cache.observed_at,
    });
    cache.valid
        && !source.dirty
        && cache.scope == source.scope
        && cache.source_id == source.id
        && cache.source_version == source.version
        && cache.source_version >= demand.required_version
        && cache.clock == demand.clock
        && cache.coverage.covers(demand.window)
        && age.is_some_and(|seconds| seconds <= demand.max_age_seconds)
}

/// Build bounded, idempotent wakeup proposals. The source and outstanding
/// snapshots remain authoritative; a returned item is never an acknowledgement.
pub fn evaluate(
    snapshot: Snapshot<'_>,
    now: ClockInstant,
    cursor: RequestCursor,
    page_limit: usize,
) -> Result<WakePage, RegistryError> {
    if snapshot.sources.len() > SOURCE_LIMIT
        || snapshot.demands.len() > DEMAND_LIMIT
        || snapshot.caches.len() > CACHE_LIMIT
        || snapshot.outstanding.len() > OUTSTANDING_LIMIT
        || page_limit > MAX_PAGE
        || !snapshot.scope.is_valid()
        || !clock_matches_scope(now.kind, snapshot.scope)
        || snapshot.token == 0
        || snapshot.sources.iter().any(|s| s.scope != snapshot.scope || !clock_matches_scope(s.clock, s.scope) || s.id == 0 || s.version == 0 || !s.window.is_valid())
        || snapshot.caches.iter().any(|c| c.scope != snapshot.scope || !clock_matches_scope(c.clock, c.scope) || c.source_id == 0 || c.source_version == 0 || !c.coverage.is_valid())
        || snapshot.demands.iter().any(|d| d.consumer_id == 0 || d.source_id == 0 || !d.scope.is_valid() || !clock_matches_scope(d.clock, d.scope) || !d.window.is_valid())
        || snapshot.sources.windows(2).any(|w| w[0].id >= w[1].id)
        || snapshot.outstanding.iter().any(|item| item.key.scope != snapshot.scope || !clock_matches_scope(item.key.clock, item.key.scope))
    {
        return Err(RegistryError::InvalidInput);
    }
    if cursor.scope != snapshot.scope || cursor.snapshot_token != snapshot.token || cursor.now != now {
        return Err(RegistryError::InvalidCursor);
    }

    let mut candidates = Vec::new();
    let mut waiting = Vec::new();
    candidates.try_reserve_exact(snapshot.sources.len()).map_err(|_| RegistryError::Allocation)?;
    waiting.try_reserve_exact(snapshot.demands.len().saturating_add(snapshot.sources.len())).map_err(|_| RegistryError::Allocation)?;
    let mut demand_wakes = Vec::new();
    demand_wakes.try_reserve_exact(snapshot.sources.len()).map_err(|_| RegistryError::Allocation)?;
    demand_wakes.resize(snapshot.sources.len(), false);
    for demand in snapshot.demands {
        if demand.scope != snapshot.scope {
            waiting.push(WaitingDemand { consumer_id: Some(demand.consumer_id), source_id: demand.source_id, reason: WaitingReason::ContextMismatch });
            continue;
        }
        let Some((source_index, source)) = snapshot.sources.iter().enumerate().find(|(_, source)| source.id == demand.source_id) else {
            waiting.push(WaitingDemand { consumer_id: Some(demand.consumer_id), source_id: demand.source_id, reason: WaitingReason::MissingSource });
            continue;
        };
        let waiting_reason = if source.clock != demand.clock {
            Some(WaitingReason::ClockMismatch)
        } else if demand.clock != now.kind {
            Some(WaitingReason::EvaluationClockMismatch)
        } else if source.version < demand.required_version {
            Some(WaitingReason::VersionUnavailable)
        } else if !source.window.covers(demand.window) {
            Some(WaitingReason::CoverageUnavailable)
        } else {
            None
        };
        if let Some(reason) = waiting_reason {
            waiting.push(WaitingDemand { consumer_id: Some(demand.consumer_id), source_id: demand.source_id, reason });
            continue;
        }
        let source = *source;
        if !snapshot.caches.iter().any(|cache| cache_satisfies(*cache, source, *demand, now)) {
            demand_wakes[source_index] = true;
        }
    }

    for (source_index, source) in snapshot.sources.iter().enumerate() {
        if source.clock != now.kind {
            if source.due.is_some() || source.dirty {
                waiting.push(WaitingDemand { consumer_id: None, source_id: source.id, reason: WaitingReason::EvaluationClockMismatch });
            }
            continue;
        }
        let due_now = source.due.is_some_and(|due| due <= now.seconds);
        let demand_wake = demand_wakes[source_index];
        let reason = if source.dirty {
            Some(WakeReason::Dirty)
        } else if due_now {
            Some(WakeReason::Due)
        } else if demand_wake {
            Some(WakeReason::MissingOrStaleForConsumer)
        } else {
            None
        };
        let Some(reason) = reason else { continue };
        let key = TaskKey {
            scope: source.scope,
            clock: source.clock,
            source_id: source.id,
            source_version: source.version,
            window: source.window,
            cursor: source.cursor,
        };
        if snapshot.outstanding.iter().any(|item| item.key == key) {
            continue;
        }
        let necessary = source.necessary || snapshot.demands.iter().any(|d| d.scope == source.scope && d.source_id == source.id && d.necessary);
        candidates.push(WakeCandidate { key, clock: source.clock, due: source.due, necessary, reason });
    }
    candidates.sort_unstable_by_key(|candidate| (!candidate.necessary, candidate.due.unwrap_or(u64::MAX), candidate.key));
    if cursor.offset > candidates.len() {
        return Err(RegistryError::InvalidCursor);
    }
    let end = cursor.offset.saturating_add(page_limit).min(candidates.len());
    let more = end < candidates.len();
    let mut items = Vec::new();
    items.try_reserve_exact(end - cursor.offset).map_err(|_| RegistryError::Allocation)?;
    items.extend_from_slice(&candidates[cursor.offset..end]);
    Ok(WakePage {
        items,
        waiting,
        next: RequestCursor { scope: snapshot.scope, snapshot_token: snapshot.token, now, offset: end },
        more,
    })
}

/// A late result is current only when source version and resume cursor still match.
/// Callers must still validate and commit it through their existing owner.
pub fn result_matches_current(key: TaskKey, source: Source) -> bool {
    key.scope == source.scope
        && key.clock == source.clock
        && key.source_id == source.id
        && key.source_version == source.version
        && key.window == source.window
        && key.cursor == source.cursor
}
