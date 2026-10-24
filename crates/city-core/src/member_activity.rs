//! Root-private mutual exclusion of actually admitted member activities.
//! This is not capacity, teaching proof, or a public fact-ingress API. The root must
//! authenticate population/clock/source and commit this candidate with its business
//! progress and receipt atomically. Domain adapters never own a second ledger.
use crate::{arena::Scope, model_rows::{Record, Rows}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MAX_ACTIVE: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Members { pub start: u64, pub end: u64 }
impl Members {
    fn valid(self, people: u64) -> bool { self.start < self.end && self.end <= people }
    fn overlaps(self, other: Self) -> bool { self.start < other.end && other.start < self.end }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ledger(people: u64) -> Ledger { Ledger::new(Scope::new(1, 2).unwrap(), [7; 32], people, 3, 0).unwrap() }
    fn members(start: u64, end: u64) -> [Option<Members>; 2] { [Some(Members { start, end }), None] }
    fn source(l: &Ledger, tick: u64) -> Source { Source { scope: l.scope, population_origin: l.population_origin,
        epoch: l.epoch, ordinal: l.next_ordinal().unwrap(), operation_id: l.next_ordinal().unwrap(), occurred_at: tick } }
    fn commit(l: &mut Ledger, c: Candidate) { *l = c.into_next(l).unwrap(); }
    fn begin(l: &mut Ledger, start: u64, end: u64, tick: u64) -> u64 {
        let s = source(l, tick);
        let c = l.prepare_begin(s, tick, 9, Kind::EducationInstruction, members(start, end)).unwrap();
        commit(l, c); s.ordinal
    }
    fn close(l: &mut Ledger, id: u64, tick: u64) -> Settlement {
        let c = l.prepare_settle(source(l, tick), tick, id, 9, Kind::EducationInstruction, true).unwrap();
        let result = c.settlement.unwrap(); commit(l, c); result
    }
    #[test]
    fn ongoing_members_exclude_every_admitted_domain_and_candidate_is_atomic() {
        let mut l = ledger(100);
        begin(&mut l, 10, 20, 0);
        let before = l.binding();
        for kind in [Kind::EducationPreparation, Kind::EducationInstruction, Kind::ServiceWork] {
            assert!(matches!(l.prepare_begin(source(&l, 1), 1, 10, kind, members(19, 30)), Err(Error::Conflict)));
            assert_eq!(l.binding(), before);
        }
        assert_eq!(l.active().len(), 1);
    }
    #[test]
    fn teacher_and_learners_are_one_atomic_nonoverlapping_claim() {
        let mut l = ledger(100);
        let bad = [Some(Members { start: 0, end: 10 }), Some(Members { start: 9, end: 11 })];
        assert!(matches!(l.prepare_begin(source(&l, 0), 0, 9, Kind::EducationInstruction, bad), Err(Error::Invalid)));
        assert!(l.active().is_empty()); assert_eq!(l.next_ordinal(), Ok(1));
        let good = [Some(Members { start: 0, end: 10 }), Some(Members { start: 10, end: 11 })];
        let c = l.prepare_begin(source(&l, 0), 0, 9, Kind::EducationInstruction, good).unwrap(); commit(&mut l, c);
        assert_eq!(l.active()[0].members, good);
    }
    #[test]
    fn rejected_full_begin_does_not_block_necessary_end_or_consume_stream_number() {
        let mut l = ledger(100);
        let mut first = 0;
        for n in 0..MAX_ACTIVE as u64 { let id = begin(&mut l, n, n + 1, 0); if n == 0 { first = id; } }
        let sequence = l.next_ordinal().unwrap();
        assert!(matches!(l.prepare_begin(source(&l, 1), 1, 9, Kind::ServiceWork, members(90, 91)), Err(Error::UnresolvedBudget)));
        assert_eq!(l.next_ordinal(), Ok(sequence));
        let ended = close(&mut l, first, 1);
        assert_eq!((ended.from, ended.through), (0, 1));
        assert_eq!(l.active().len(), MAX_ACTIVE - 1);
        begin(&mut l, 90, 91, 1);
    }
    #[test]
    fn more_than_table_size_lifetime_events_keep_replay_watermark() {
        let mut l = ledger(2);
        let original = source(&l, 0);
        for n in 0..200 {
            let id = begin(&mut l, 0, 1, n * 2);
            let interval = close(&mut l, id, n * 2 + 1);
            assert_eq!(interval.through - interval.from, 1);
            assert!(l.active().is_empty());
        }
        assert_eq!(l.last_ordinal, 400);
        let mut replay = original; replay.occurred_at = 400; replay.operation_id = 999;
        assert!(matches!(l.prepare_begin(replay, 400, 9, Kind::EducationInstruction, members(0, 1)), Err(Error::Stale)));
    }
    #[test]
    fn midnight_settlement_preserves_original_exclusion_and_consumption_cursor() {
        let mut l = ledger(100);
        let id = begin(&mut l, 0, 3, 86399);
        let c = l.prepare_settle(source(&l, 86400), 86400, id, 9, Kind::EducationInstruction, false).unwrap();
        assert_eq!(c.settlement.unwrap().from, 86399); commit(&mut l, c);
        assert_eq!(l.active()[0].id, id); assert_eq!(l.active()[0].started, 86399);
        assert!(matches!(l.prepare_begin(source(&l, 86400), 86400, 20, Kind::ServiceWork, members(2, 4)), Err(Error::Conflict)));
        let interval = close(&mut l, id, 86401);
        assert_eq!((interval.from, interval.through), (86400, 86401));
        begin(&mut l, 0, 3, 86401); // [begin,end): same-tick handoff is legal.
    }
    #[test]
    fn wrong_owner_kind_clock_population_epoch_and_stale_candidate_rejected() {
        let mut l = ledger(100); let id = begin(&mut l, 0, 10, 1);
        for (owner, kind) in [(10, Kind::EducationInstruction), (9, Kind::ServiceWork)] {
            assert!(matches!(l.prepare_settle(source(&l, 2), 2, id, owner, kind, true), Err(Error::Binding)));
        }
        let mut wrong = source(&l, 2); wrong.population_origin = [8; 32];
        assert!(matches!(l.prepare_begin(wrong, 2, 9, Kind::ServiceWork, members(20, 21)), Err(Error::Binding)));
        wrong = source(&l, 2); wrong.epoch = 4;
        assert!(matches!(l.prepare_begin(wrong, 2, 9, Kind::ServiceWork, members(20, 21)), Err(Error::Binding)));
        assert!(matches!(l.prepare_begin(source(&l, 3), 2, 9, Kind::ServiceWork, members(20, 21)), Err(Error::Binding)));
        let stale = l.prepare_begin(source(&l, 2), 2, 9, Kind::ServiceWork, members(20, 21)).unwrap();
        close(&mut l, id, 2);
        assert!(!stale.matches(&l)); assert!(matches!(stale.into_next(&l), Err(Error::Stale)));
    }
    #[test]
    fn checkpoint_reopen_keeps_unfinished_responsibility_and_rejects_tampering() {
        let mut l = ledger(100); let id = begin(&mut l, 2, 5, 10);
        let bytes = serde_json::to_vec(&l).unwrap();
        let mut reopened: Ledger = serde_json::from_slice(&bytes).unwrap();
        reopened.validate_binding(l.scope, [7; 32], 100, 3, 10).unwrap();
        assert_eq!(reopened.binding(), l.binding());
        assert!(matches!(reopened.prepare_begin(source(&reopened, 11), 11, 10, Kind::ServiceWork, members(4, 8)), Err(Error::Conflict)));
        assert_eq!(close(&mut reopened, id, 12).from, 10);
        let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        bad["active"][0]["settled_through"] = 11.into();
        let bad: Ledger = serde_json::from_value(bad).unwrap();
        assert_eq!(bad.validate_binding(l.scope, [7; 32], 100, 3, 10), Err(Error::Invalid));
    }
    #[test]
    fn zero_duration_close_gives_no_credit_and_epoch_or_ordinal_never_reset() {
        let mut l = ledger(1); let id = begin(&mut l, 0, 1, 5);
        let interval = close(&mut l, id, 5); assert_eq!(interval.from, interval.through);
        assert_eq!(l.last_ordinal, 2);
        l.last_ordinal = u64::MAX;
        assert_eq!(l.next_ordinal(), Err(Error::Overflow));
    }
    #[test]
    fn exhausted_source_reserves_each_admitted_close_including_multiple_open_activities() {
        let mut l = ledger(3); l.last_ordinal = u64::MAX - 1;
        assert!(matches!(l.prepare_begin(source(&l, 0), 0, 9, Kind::EducationInstruction, members(0, 1)), Err(Error::UnresolvedBudget)));
        assert!(l.active().is_empty());
        l.last_ordinal = u64::MAX - 5;
        let a = begin(&mut l, 0, 1, 0);
        let b = begin(&mut l, 1, 2, 0);
        assert!(matches!(l.prepare_begin(source(&l, 1), 1, 9, Kind::EducationInstruction, members(2, 3)), Err(Error::UnresolvedBudget)));
        let c = l.prepare_settle(source(&l, 1), 1, a, 9, Kind::EducationInstruction, false).unwrap();
        commit(&mut l, c);
        assert!(matches!(l.prepare_settle(source(&l, 2), 2, b, 9, Kind::EducationInstruction, false), Err(Error::UnresolvedBudget)));
        let bytes = serde_json::to_vec(&l).unwrap();
        let mut l: Ledger = serde_json::from_slice(&bytes).unwrap();
        l.validate_binding(l.scope, [7; 32], 3, 3, 1).unwrap();
        assert_eq!((close(&mut l, a, 2).from, l.active().len()), (1, 1));
        assert_eq!((close(&mut l, b, 2).from, l.active().len()), (0, 0));
        assert_eq!(l.last_ordinal, u64::MAX);
        assert_eq!(l.next_ordinal(), Err(Error::Overflow));
        let mut invalid: Ledger = serde_json::from_slice(&bytes).unwrap();
        invalid.last_ordinal = u64::MAX;
        assert_eq!(invalid.validate_binding(invalid.scope, [7; 32], 3, 3, 1), Err(Error::Invalid));
    }
    #[test]
    fn layout_budget_reports_backing_and_concurrent_candidate_separately() {
        let l = ledger(100);
        assert_eq!(Ledger::owned_bytes(), std::mem::size_of_val(&l) + MAX_ACTIVE * std::mem::size_of::<Active>());
        assert!(Ledger::candidate_bytes() > Ledger::owned_bytes());
        println!("member_activity layout ledger={} candidate={} active={} capacity={}",
            Ledger::owned_bytes(), Ledger::candidate_bytes(), std::mem::size_of::<Active>(), MAX_ACTIVE);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind { EducationPreparation, EducationInstruction, ServiceWork }

/// Identity of a successfully ADMITTED ledger event, not an arbitrary provider ID.
/// The serial root host uses the next ordinal only after resolving the previous
/// original Envelope/receipt. Rejected requests do not consume an ordinal. An
/// uncertain commit must retry the identical original Envelope, not change its body.
/// Epoch changes and historical/future ingress are unsupported in this first version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub scope: Scope,
    pub population_origin: [u8; 32],
    pub epoch: u64,
    pub ordinal: u64,
    pub operation_id: u64,
    pub occurred_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Active {
    pub id: u64,
    pub owner: u64,
    pub kind: Kind,
    pub members: [Option<Members>; 2],
    pub started: u64,
    pub settled_through: u64,
    pub start_operation_id: u64,
}
impl Record for Active {
    fn inactive() -> Self { Self { id: 0, owner: 0, kind: Kind::EducationPreparation,
        members: [None; 2], started: 0, settled_through: 0, start_operation_id: 0 } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ledger {
    version: u32,
    scope: Scope,
    population_origin: [u8; 32],
    people: u64,
    epoch: u64,
    last_ordinal: u64,
    last_tick: u64,
    active: Rows<Active, MAX_ACTIVE>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error { Binding, Invalid, Stale, Conflict, UnknownActivity, UnresolvedBudget, Overflow }

/// This interval is a transient settlement proposal, not a reusable fulfillment
/// credential. Root consumes it together with its candidate exactly once. A
/// nonclosing settlement keeps the SAME original activity and member exclusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settlement {
    pub activity: Active,
    pub from: u64,
    pub through: u64,
    pub closed: bool,
    pub operation_id: u64,
}
pub(crate) struct Candidate {
    expected: [u8; 32],
    next: Ledger,
    pub settlement: Option<Settlement>,
}
impl Candidate {
    pub fn matches(&self, current: &Ledger) -> bool { self.expected == current.binding() }
    pub fn into_next(self, current: &Ledger) -> Result<Ledger, Error> {
        if !self.matches(current) { return Err(Error::Stale); }
        Ok(self.next)
    }
}

impl Ledger {
    pub fn new(scope: Scope, population_origin: [u8; 32], people: u64, epoch: u64, tick: u64) -> Result<Self, Error> {
        if population_origin == [0; 32] || people == 0 || epoch == 0 { return Err(Error::Binding); }
        Ok(Self { version: 1, scope, population_origin, people, epoch,
            last_ordinal: 0, last_tick: tick, active: Rows::new().map_err(|_| Error::UnresolvedBudget)? })
    }
    pub const fn owned_bytes() -> usize {
        std::mem::size_of::<Self>() + MAX_ACTIVE * std::mem::size_of::<Active>()
    }
    pub const fn candidate_bytes() -> usize {
        std::mem::size_of::<Candidate>() + MAX_ACTIVE * std::mem::size_of::<Active>()
    }
    pub fn active(&self) -> &[Active] { &self.active }
    pub fn next_ordinal(&self) -> Result<u64, Error> { self.last_ordinal.checked_add(1).ok_or(Error::Overflow) }
    pub(crate) fn fingerprint(&self) -> [u8;32] { self.binding() }
    /// Integrity is necessary but not source authentication. Root must separately
    /// compare all binding fields with the actual World's immutable population.
    pub fn validate_binding(&self, scope: Scope, origin: [u8; 32], people: u64, epoch: u64, completed: u64) -> Result<(), Error> {
        if self.version != 1 || self.scope != scope || self.population_origin != origin
            || origin == [0; 32] || self.people != people || people == 0 || self.epoch != epoch
            || epoch == 0 || self.last_tick > completed { return Err(Error::Binding); }
        // Every admitted open responsibility owns one remaining CLOSE ordinal.
        // Neither a new begin nor an intermediate settlement may consume it.
        if u64::MAX - self.last_ordinal < self.active.len() as u64 { return Err(Error::Invalid); }
        for (i, row) in self.active.iter().enumerate() {
            if row.id == 0 || row.id > self.last_ordinal || row.owner == 0 || row.start_operation_id == 0
                || row.started > row.settled_through || row.settled_through > self.last_tick {
                return Err(Error::Invalid);
            }
            Self::validate_members(row.members, people)?;
            if self.active[..i].iter().any(|old| old.id == row.id || old.start_operation_id == row.start_operation_id) {
                return Err(Error::Invalid);
            }
            if self.active[..i].iter().any(|old| Self::member_overlap(old.members, row.members)) {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }
    fn validate_members(members: [Option<Members>; 2], people: u64) -> Result<(), Error> {
        if members[0].is_none() || members.into_iter().flatten().any(|m| !m.valid(people))
            || matches!(members, [Some(a), Some(b)] if a.overlaps(b)) { return Err(Error::Invalid); }
        Ok(())
    }
    fn member_overlap(a: [Option<Members>; 2], b: [Option<Members>; 2]) -> bool {
        a.into_iter().flatten().any(|a| b.into_iter().flatten().any(|b| a.overlaps(b)))
    }
    fn check_source(&self, source: Source, completed: u64) -> Result<(), Error> {
        self.validate_binding(self.scope, self.population_origin, self.people, self.epoch, completed)?;
        if source.scope != self.scope || source.population_origin != self.population_origin
            || source.epoch != self.epoch || source.operation_id == 0
            || source.occurred_at != completed || completed < self.last_tick { return Err(Error::Binding); }
        if source.ordinal != self.next_ordinal()? { return Err(Error::Stale); }
        Ok(())
    }
    fn next(&self, source: Source) -> Result<Self, Error> {
        let mut active = Rows::new().map_err(|_| Error::UnresolvedBudget)?;
        active.copy_active_from(&self.active);
        Ok(Self { version: self.version, scope: self.scope, population_origin: self.population_origin,
            people: self.people, epoch: self.epoch, last_ordinal: source.ordinal,
            last_tick: source.occurred_at, active })
    }
    pub fn prepare_begin(&self, source: Source, completed: u64, owner: u64, kind: Kind,
        members: [Option<Members>; 2]) -> Result<Candidate, Error> {
        self.check_source(source, completed)?;
        Self::validate_members(members, self.people)?;
        if owner == 0 { return Err(Error::Invalid); }
        if self.active.iter().any(|old| Self::member_overlap(old.members, members)) { return Err(Error::Conflict); }
        if self.active.len() == MAX_ACTIVE { return Err(Error::UnresolvedBudget); }
        if u64::MAX - source.ordinal < self.active.len() as u64 + 1 { return Err(Error::UnresolvedBudget); }
        if self.active.iter().any(|old| old.start_operation_id == source.operation_id) { return Err(Error::Invalid); }
        let mut next = self.next(source)?;
        next.active.try_push(Active { id: source.ordinal, owner, kind, members,
            started: completed, settled_through: completed, start_operation_id: source.operation_id })
            .map_err(|_| Error::UnresolvedBudget)?;
        Ok(Candidate { expected: self.binding(), next, settlement: None })
    }
    /// End/settle never needs a FREE active slot. Zero-duration close is permitted
    /// to finish an aborted responsibility, but provides zero creditable seconds.
    /// Root may settle at a day boundary without releasing or inventing a restart.
    pub fn prepare_settle(&self, source: Source, completed: u64, id: u64, owner: u64,
        kind: Kind, close: bool) -> Result<Candidate, Error> {
        self.check_source(source, completed)?;
        let index = self.active.iter().position(|a| a.id == id).ok_or(Error::UnknownActivity)?;
        let row = self.active[index];
        if row.owner != owner || row.kind != kind || completed < row.settled_through { return Err(Error::Binding); }
        if !close && completed == row.settled_through { return Err(Error::Invalid); }
        if !close && u64::MAX - source.ordinal < self.active.len() as u64 { return Err(Error::UnresolvedBudget); }
        let mut next = self.next(source)?;
        if close {
            next.active.clear();
            for (i, old) in self.active.iter().enumerate() {
                if i != index { next.active.try_push(*old).map_err(|_| Error::UnresolvedBudget)?; }
            }
        } else { next.active[index].settled_through = completed; }
        Ok(Candidate { expected: self.binding(), next, settlement: Some(Settlement {
            activity: row, from: row.settled_through, through: completed, closed: close,
            operation_id: source.operation_id }) })
    }
    fn binding(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"mycity-member-activity-v1");
        h.update(self.population_origin);
        for word in [u64::from(self.version), self.scope.world(), self.scope.branch(), self.people,
            self.epoch, self.last_ordinal, self.last_tick, self.active.len() as u64] { h.update(word.to_le_bytes()); }
        for row in self.active.iter() {
            for word in [row.id, row.owner, row.kind as u64, row.started, row.settled_through, row.start_operation_id] { h.update(word.to_le_bytes()); }
            for member in row.members {
                match member {
                    None => h.update([0]),
                    Some(m) => { h.update([1]); h.update(m.start.to_le_bytes()); h.update(m.end.to_le_bytes()); }
                }
            }
        }
        h.finalize().into()
    }
}
