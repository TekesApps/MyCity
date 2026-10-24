//! Pure control identities for exact, ordered air pages (issue #103).
//!
//! This module executes no air work, measures no cost and owns no CPU budget,
//! admission, pool, lease, memory refund or resource allocation authority.
//! Counters are reports, not proof of execution. The caller binds the complete
//! original Ticket, preserves numerical iteration order, and pauses only at a
//! real page boundary while retaining its cursor, scratch and grant. Exhausted
//! quota means unfinished work, not a demonstrated resource shortage.
//! Transport failure is an uncertain result: never replay or refund on that
//! basis. A rejected control message requires rejecting the connection; these
//! pure methods leave state unchanged so the caller can retain evidence.

pub const PLAN_VERSION: u16 = 1;
pub const PLAN_ID: &str = "air-exact-pages-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidQuota,
    InvalidInstance,
    VersionMismatch,
    InstanceMismatch,
    TurnMismatch,
    WrongPhase,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quota {
    pub source_limit: u32,
    pub cell_limit: u32,
    pub pages: u16,
}

impl Quota {
    pub fn validate(self) -> Result<Self, Error> {
        if !(1..=8192).contains(&self.source_limit)
            || !(1..=1024).contains(&self.cell_limit)
            || !(1..=16).contains(&self.pages)
        {
            return Err(Error::InvalidQuota);
        }
        Ok(self)
    }
}

/// Independent units, deliberately without a combined work-cost metric.
/// `integrated_cells` includes the full grid integrated at segment end, even
/// when that exceeds the next page's cell limit. Completed no-op calls report
/// ZERO. The actual loop, not this module, defines and increments each count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkCounters {
    pub sources: u64,
    pub cells: u64,
    pub integrated_cells: u64,
    pub segments: u64,
    pub pages: u64,
}

impl WorkCounters {
    pub const ZERO: Self = Self {
        sources: 0,
        cells: 0,
        integrated_cells: 0,
        segments: 0,
        pages: 0,
    };

    pub fn checked_add(self, rhs: Self) -> Result<Self, Error> {
        Ok(Self {
            sources: self.sources.checked_add(rhs.sources).ok_or(Error::Overflow)?,
            cells: self.cells.checked_add(rhs.cells).ok_or(Error::Overflow)?,
            integrated_cells: self.integrated_cells.checked_add(rhs.integrated_cells).ok_or(Error::Overflow)?,
            segments: self.segments.checked_add(rhs.segments).ok_or(Error::Overflow)?,
            pages: self.pages.checked_add(rhs.pages).ok_or(Error::Overflow)?,
        })
    }
}

/// Boundary report. `work` is this turn's delta, never a cumulative cost.
/// The outer event distinguishes terminal reports, which must use `finish`
/// after outer identity validation, never `accept_boundary`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub plan_version: u16,
    pub turn: u64,
    pub process_instance: String,
    pub work: WorkCounters,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Continue {
    pub plan_version: u16,
    pub turn: u64,
    pub process_instance: String,
    pub quota: Quota,
}

/// Canonical nonzero decimal u128: no sign, whitespace or leading zero.
pub fn validate_process_instance(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > 39 || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.parse::<u128>().is_err()
    {
        return Err(Error::InvalidInstance);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Running,
    Waiting,
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Turns {
    instance: String,
    turn: u64,
    quota: Quota,
    total: WorkCounters,
    phase: Phase,
}

impl Turns {
    fn new(instance: String, quota: Quota) -> Result<Self, Error> {
        validate_process_instance(&instance)?;
        quota.validate()?;
        Ok(Self { instance, turn: 1, quota, total: WorkCounters::ZERO, phase: Phase::Running })
    }

    fn identity(&self, version: u16, turn: u64, instance: &str) -> Result<(), Error> {
        if version != PLAN_VERSION { return Err(Error::VersionMismatch); }
        validate_process_instance(instance)?;
        if instance != self.instance { return Err(Error::InstanceMismatch); }
        if turn != self.turn { return Err(Error::TurnMismatch); }
        Ok(())
    }

    fn boundary(&mut self, work: WorkCounters) -> Result<(), Error> {
        if self.phase != Phase::Running { return Err(Error::WrongPhase); }
        // A nonterminal boundary must have a representable following turn.
        self.turn.checked_add(1).ok_or(Error::Overflow)?;
        let total = self.total.checked_add(work)?;
        self.total = total;
        self.phase = Phase::Waiting;
        Ok(())
    }

    fn resume(&mut self, quota: Quota) -> Result<(), Error> {
        if self.phase != Phase::Waiting { return Err(Error::WrongPhase); }
        quota.validate()?;
        let turn = self.turn.checked_add(1).ok_or(Error::Overflow)?;
        self.turn = turn;
        self.quota = quota;
        self.phase = Phase::Running;
        Ok(())
    }

    fn finish(&mut self, work: WorkCounters) -> Result<(), Error> {
        if self.phase != Phase::Running { return Err(Error::WrongPhase); }
        let total = self.total.checked_add(work)?;
        self.total = total;
        self.phase = Phase::Finished;
        Ok(())
    }
}

/// Child-side bookkeeping. Starts running turn 1 under the initial quota.
/// Call `reach_boundary` only after actual nonterminal page work is paused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildTurns(Turns);

impl ChildTurns {
    pub fn new(process_instance: String, quota: Quota) -> Result<Self, Error> {
        Turns::new(process_instance, quota).map(Self)
    }

    pub fn reach_boundary(&mut self, work: WorkCounters) -> Result<Boundary, Error> {
        self.0.boundary(work)?;
        Ok(Boundary { plan_version: PLAN_VERSION, turn: self.0.turn,
            process_instance: self.0.instance.clone(), work })
    }

    pub fn accept_continue(&mut self, message: &Continue) -> Result<Quota, Error> {
        self.0.identity(message.plan_version, message.turn, &message.process_instance)?;
        self.0.resume(message.quota)?;
        Ok(message.quota)
    }

    /// Final page bypasses a Boundary/Continue exchange. Repeated finish is
    /// rejected; an executor's later completed no-op must return ZERO itself.
    pub fn finish(&mut self, work: WorkCounters) -> Result<(), Error> { self.0.finish(work) }
    pub fn turn(&self) -> u64 { self.0.turn }
    pub fn quota(&self) -> Quota { self.0.quota }
    pub fn total(&self) -> WorkCounters { self.0.total }
    pub fn is_waiting(&self) -> bool { self.0.phase == Phase::Waiting }
    pub fn is_finished(&self) -> bool { self.0.phase == Phase::Finished }
}

/// Owner-side bookkeeping with exactly one pending boundary. Preparing a
/// Continue consumes it once; this is NOT acknowledgement of send/delivery or
/// execution. Caller retains the prepared message and uncertain result on
/// transport failure, and must not blindly retry on a new connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerTurns(Turns);

impl OwnerTurns {
    pub fn new(process_instance: String, quota: Quota) -> Result<Self, Error> {
        Turns::new(process_instance, quota).map(Self)
    }

    pub fn accept_boundary(&mut self, message: &Boundary) -> Result<(), Error> {
        self.0.identity(message.plan_version, message.turn, &message.process_instance)?;
        self.0.boundary(message.work)
    }

    pub fn prepare_continue(&mut self, quota: Quota) -> Result<Continue, Error> {
        let turn = self.0.turn;
        self.0.resume(quota)?;
        Ok(Continue { plan_version: PLAN_VERSION, turn,
            process_instance: self.0.instance.clone(), quota })
    }

    /// Outer completion envelope must already bind original Ticket, version,
    /// current turn and instance; `work` is the final turn's delta.
    pub fn finish(&mut self, work: WorkCounters) -> Result<(), Error> { self.0.finish(work) }
    pub fn turn(&self) -> u64 { self.0.turn }
    pub fn quota(&self) -> Quota { self.0.quota }
    pub fn total(&self) -> WorkCounters { self.0.total }
    pub fn is_waiting(&self) -> bool { self.0.phase == Phase::Waiting }
    pub fn is_finished(&self) -> bool { self.0.phase == Phase::Finished }
}

#[cfg(test)]
#[path = "air_turns_tests.rs"]
mod tests;
