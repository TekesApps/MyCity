//! Fixed-size observations of successful, actual air page calls. No grants or control.
//! Root binds each Summary to the existing Ticket/instance/turn/quota/strategy.
use crate::air_turns::WorkCounters;
use crate::compute_worker::cpu_metrics::{read_process_cpu_ns, UnknownReason as CpuUnknown};
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub const VERSION: u16 = 1;
pub const MAX_TURN_PAGES: u64 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case", deny_unknown_fields)]
pub enum UnknownReason {
    CpuUnavailable(CpuUnknown),
    ClockWentBackwards,
    NanosecondOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Nanoseconds {
    Known { ns: u64 },
    Unknown { reason: UnknownReason },
}

impl Nanoseconds {
    const ZERO: Self = Self::Known { ns: 0 };
    fn sum(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown { .. }, _) => self,
            (_, Self::Unknown { .. }) => other,
            (Self::Known { ns: a }, Self::Known { ns: b }) => a.checked_add(b)
                .map_or(Self::Unknown { reason: UnknownReason::NanosecondOverflow },
                    |ns| Self::Known { ns }),
        }
    }
    fn maximum(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown { .. }, _) => self,
            (_, Self::Unknown { .. }) => other,
            (Self::Known { ns: a }, Self::Known { ns: b }) => Self::Known { ns: a.max(b) },
        }
    }
}

/// An unknown total/max stays unknown; there is no implicit zero or partial-final value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub total: Nanoseconds,
    pub max_page: Nanoseconds,
}

impl Axis {
    const EMPTY: Self = Self { total: Nanoseconds::ZERO, max_page: Nanoseconds::ZERO };
    fn append(&mut self, value: Nanoseconds) {
        self.total = self.total.sum(value);
        self.max_page = self.max_page.maximum(value);
    }
    fn valid(self, count: u64) -> bool {
        if count == 0 { return self == Self::EMPTY; }
        match (self.total, self.max_page) {
            (Nanoseconds::Known { ns: total }, Nanoseconds::Known { ns: max }) => {
                max <= total && u128::from(total) <= u128::from(max) * u128::from(count)
            }
            // Sum can overflow while the actual maximum is still representable.
            (Nanoseconds::Unknown { reason: UnknownReason::NanosecondOverflow },
                Nanoseconds::Known { ns: max }) =>
                    u128::from(max) * u128::from(count) > u128::from(u64::MAX),
            (Nanoseconds::Unknown { .. }, Nanoseconds::Unknown { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub pages: u64,
    pub wall: Axis,
    pub cpu: Axis,
}

impl Group {
    const EMPTY: Self = Self { pages: 0, wall: Axis::EMPTY, cpu: Axis::EMPTY };
    fn valid(self) -> bool { self.wall.valid(self.pages) && self.cpu.valid(self.pages) }
    fn append(&mut self, wall: Nanoseconds, cpu: Nanoseconds) -> Option<()> {
        self.pages = self.pages.checked_add(1)?;
        self.wall.append(wall);
        self.cpu.append(cpu);
        Some(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    /// Distinct units from actual page loops, never a combined work-cost unit.
    pub work: WorkCounters,
    pub ordinary: Group,
    /// A page is terminal-integration work if integrated_cells OR segments is nonzero.
    pub integration: Group,
    pub wall: Axis,
    pub cpu: Axis,
}

impl Turn {
    const EMPTY: Self = Self { work: WorkCounters::ZERO, ordinary: Group::EMPTY,
        integration: Group::EMPTY, wall: Axis::EMPTY, cpu: Axis::EMPTY };
    fn rebuild_axes(&mut self) {
        self.wall = combine(self.ordinary.wall, self.integration.wall);
        self.cpu = combine(self.ordinary.cpu, self.integration.cpu);
    }
    fn valid(self) -> bool {
        let Some(pages) = self.ordinary.pages.checked_add(self.integration.pages) else { return false; };
        if pages != self.work.pages || pages > MAX_TURN_PAGES || !self.ordinary.valid()
            || !self.integration.valid() || !self.wall.valid(pages) || !self.cpu.valid(pages) {
            return false;
        }
        if self.wall != combine(self.ordinary.wall, self.integration.wall)
            || self.cpu != combine(self.ordinary.cpu, self.integration.cpu) { return false; }
        let w = self.work;
        // A segment-commit page has at most one full-grid integral (128x128).
        if w.sources > 8192 * pages || w.cells > 1024 * pages
            || w.segments > self.integration.pages
            || w.integrated_cells > 16384 * w.segments { return false; }
        // Every integration-class page must account for a real segment commit.
        if self.integration.pages != w.segments
            || (w.segments == 0) != (w.integrated_cells == 0) { return false; }
        pages != 0 || w == WorkCounters::ZERO
    }
}

fn combine(a: Axis, b: Axis) -> Axis {
    Axis { total: a.total.sum(b.total), max_page: a.max_page.maximum(b.max_page) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason { InvalidPageWork, CountOverflow, TurnLimit, StaleToken, ResetOverflow }

/// Exactly one of turn/unavailable must be present. Unavailable is never an empty turn.
/// Empty known turn explicitly means no executed pages, so its totals are the empty sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub version: u16,
    /// Exact count snapshot for outer Boundary binding; unavailable summaries must not
    /// be used as successful count evidence (validate returns Unavailable).
    pub work: WorkCounters,
    pub turn: Option<Turn>,
    pub unavailable: Option<UnavailableReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError { Version, Shape, Unavailable }

impl Summary {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != VERSION { return Err(ValidationError::Version); }
        match (self.turn, self.unavailable) {
            (Some(turn), None) if turn.work == self.work && turn.valid() => Ok(()),
            (None, Some(_)) => Err(ValidationError::Unavailable),
            _ => Err(ValidationError::Shape),
        }
    }
}

/// Non-serializable, non-cloneable same-call token. Root must not move it across samplers.
#[must_use]
pub struct PageToken {
    epoch: u64,
    index: u64,
    wall_start: Instant,
    cpu_start: Result<u64, CpuUnknown>,
}

/// One fixed-size accumulator per actual child turn; contains no page Vec or history.
pub struct Sampler {
    epoch: u64,
    turn: Turn,
    unavailable: Option<UnavailableReason>,
}

impl Default for Sampler { fn default() -> Self { Self::new() } }

impl Sampler {
    pub fn new() -> Self { Self { epoch: 0, turn: Turn::EMPTY, unavailable: None } }
    pub fn begin_page(&self) -> PageToken {
        // CPU brackets the wall reads; wall starts after the first CPU syscall.
        let cpu_start = read_process_cpu_ns();
        PageToken { epoch: self.epoch, index: self.turn.work.pages,
            wall_start: Instant::now(), cpu_start }
    }
    /// Call IMMEDIATELY after a successful state.page* invocation, before log.check,
    /// emit, checkpoint or control wait. Failed page calls must not be appended.
    /// No timing/shape failure escapes as a business/control error.
    pub fn end_page(&mut self, token: PageToken, work: WorkCounters) {
        let wall_end = Instant::now();
        let cpu_end = read_process_cpu_ns();
        let wall = wall_end.checked_duration_since(token.wall_start)
            .map(|duration| nanos(duration.as_nanos()))
            .unwrap_or(Nanoseconds::Unknown { reason: UnknownReason::ClockWentBackwards });
        let cpu = cpu_difference(token.cpu_start, cpu_end);
        if self.unavailable.is_some() { return; }
        if token.epoch != self.epoch || token.index != self.turn.work.pages {
            self.unavailable = Some(UnavailableReason::StaleToken); return;
        }
        self.record(work, wall, cpu);
    }
    pub fn summary(&self) -> Summary {
        Summary { version: VERSION, work: self.turn.work,
            turn: self.unavailable.is_none().then_some(self.turn),
            unavailable: self.unavailable }
    }
    /// Reset at a verified new turn; old tokens become invalid. Summary copies stay frozen.
    pub fn reset(&mut self) {
        let Some(epoch) = self.epoch.checked_add(1) else {
            self.unavailable = Some(UnavailableReason::ResetOverflow); return;
        };
        self.epoch = epoch;
        self.turn = Turn::EMPTY;
        self.unavailable = None;
    }
    fn record(&mut self, work: WorkCounters, wall: Nanoseconds, cpu: Nanoseconds) {
        if self.unavailable.is_some() { return; }
        // A completed no-op is explicitly not an executed page or timing sample.
        if work == WorkCounters::ZERO { return; }
        if work.pages != 1 || work.sources > 8192 || work.cells > 1024
            || work.segments > 1 || work.integrated_cells > 16384 * work.segments
            || (work.segments == 0) != (work.integrated_cells == 0) {
            self.unavailable = Some(UnavailableReason::InvalidPageWork); return;
        }
        let Ok(total) = self.turn.work.checked_add(work) else {
            self.unavailable = Some(UnavailableReason::CountOverflow); return;
        };
        if total.pages > MAX_TURN_PAGES { self.unavailable = Some(UnavailableReason::TurnLimit); return; }
        let group = if work.integrated_cells != 0 || work.segments != 0 {
            &mut self.turn.integration
        } else { &mut self.turn.ordinary };
        if group.append(wall, cpu).is_none() {
            self.unavailable = Some(UnavailableReason::CountOverflow); return;
        }
        self.turn.work = total;
        self.turn.rebuild_axes();
    }
}

fn nanos(value: u128) -> Nanoseconds {
    u64::try_from(value).map_or(Nanoseconds::Unknown { reason: UnknownReason::NanosecondOverflow },
        |ns| Nanoseconds::Known { ns })
}

fn cpu_difference(start: Result<u64, CpuUnknown>, end: Result<u64, CpuUnknown>) -> Nanoseconds {
    match (start, end) {
        (Err(reason), _) | (_, Err(reason)) => Nanoseconds::Unknown {
            reason: UnknownReason::CpuUnavailable(reason) },
        (Ok(a), Ok(b)) => b.checked_sub(a).map_or(
            Nanoseconds::Unknown { reason: UnknownReason::ClockWentBackwards },
            |ns| Nanoseconds::Known { ns }),
    }
}

#[cfg(test)]
#[path = "air_page_metrics_tests.rs"]
mod tests;
