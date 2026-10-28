//! Bounded facility-work responsibility and wage progress, not a second writer.
//! Root must authenticate original receipts, exact employment/account membership,
//! qualification, venue presence, device operation and the UNIQUE member ledger
//! before making the crate-private adapter inputs below. These inputs are trusted
//! host data, not externally secure capabilities. No bool/ID authenticates labor.
use crate::{arena::Scope, member_qualification::{BaseQualification, MemberRange, ReadSource}};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_CONTRACTS: usize = 4;
pub(crate) const MAX_OWNED_BYTES: usize = 8 * 1024;
pub(crate) const MAX_CANDIDATE_BYTES: usize = 32 * 1024;
const DENOMINATOR: u128 = 3600;
const MAX_CENTS: u64 = i64::MAX as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role { Electric, Water, ElectricalMaintenance }
impl Role {
    pub fn required_base(self) -> BaseQualification {
        match self { Self::Electric => BaseQualification::Electric, Self::Water => BaseQualification::Water, Self::ElectricalMaintenance => BaseQualification::Electric }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub scope: Scope,
    pub population_origin: [u8; 32],
    pub member_version: u32,
    pub people: u64,
    pub epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: u64,
    pub version: u32,
    pub device_id: u64,
    pub role: Role,
    pub members: MemberRange,
    pub employer_account: u32,
    pub employee_account: u32,
    pub venue_use_id: u64,
    pub hourly_cents_per_member: u64,
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub binding: Binding,
    pub contracts: [Option<Contract>; MAX_CONTRACTS],
}

impl Contract {
    /// Integer-cent upper projection for the entire explicit operation window.
    /// This reserves/pays nothing and cannot certify that any labor occurred.
    pub fn maximum_earned_cents(self) -> Result<u64, Error> {
        if self.start >= self.end || self.members.start >= self.members.end
            || self.hourly_cents_per_member == 0 || self.hourly_cents_per_member > MAX_CENTS {
            return Err(Error::Invalid);
        }
        let seconds = self.end.checked_sub(self.start).ok_or(Error::Invalid)?;
        wage(self, seconds, 0).map(|(cents, _)| cents)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Binding, Invalid, UnknownContract, State, Stale, Overflow, UnresolvedBudget }

impl Opening {
    pub fn validate(&self) -> Result<(), Error> {
        let b = self.binding;
        if b.scope.world() == 0 || b.scope.branch() == 0 || b.population_origin == [0; 32]
            || b.member_version == 0 || b.people == 0 || b.epoch == 0 { return Err(Error::Binding); }
        let mut ended = false;
        for (index, slot) in self.contracts.iter().enumerate() {
            let Some(c) = slot else { ended = true; continue; };
            if ended || c.id == 0 || c.version == 0 || (c.role == Role::ElectricalMaintenance && c.version != 2) || c.device_id == 0 || c.venue_use_id == 0
                || c.employer_account == 0 || c.employee_account == 0
                || c.employer_account == c.employee_account || c.hourly_cents_per_member == 0
                || c.hourly_cents_per_member > MAX_CENTS || c.start >= c.end
                || c.members.start >= c.members.end || c.members.end > b.people {
                return Err(Error::Invalid);
            }
            for other in self.contracts[..index].iter().flatten() {
                if other.id == c.id { return Err(Error::Invalid); }
            }
            // Do not admit a promise whose full explicit window cannot fit the
            // original Money range; never wait until real work to discover this.
            c.maximum_earned_cents()?;
        }
        Ok(())
    }
}

/// Root serializes this stream only AFTER original receipt retry handling.
/// Ordinal is facility responsibility sequence, not a second world clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    pub binding: Binding,
    pub ordinal: u64,
    pub operation_id: u64,
    pub occurred_at: u64,
}

/// Compact original event reference. Its immutable population/scope binding is
/// stored ONCE in Runtime::opening, not repeated in every durable event slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventRef {
    pub ordinal: u64,
    pub operation_id: u64,
    pub occurred_at: u64,
}
impl EventRef {
    pub fn source(self, binding: Binding) -> Source {
        Source { binding, ordinal: self.ordinal, operation_id: self.operation_id, occurred_at: self.occurred_at }
    }
}
impl From<Source> for EventRef {
    fn from(s: Source) -> Self { Self { ordinal: s.ordinal, operation_id: s.operation_id, occurred_at: s.occurred_at } }
}

/// Construct only in the original root adapter AFTER employment, exact source
/// qualification, venue-enter and member-ledger Begin have all been verified.
/// This constructor checks no real-world authority; root cannot expose it as ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Admission {
    contract: Contract,
    activity_id: u64,
    qualification_source: ReadSource,
}
impl Admission {
    pub(crate) fn from_root(contract: Contract, activity_id: u64, qualification_source: ReadSource) -> Self {
        Self { contract, activity_id, qualification_source }
    }
}

/// Exact interval checked against the original member settlement and either
/// device RUN or a real maintenance order, at a completed Timeline boundary.
/// This private provenance tag is not a durable fact or external JSON ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObservedWorkKind { OperatingDevice, ElectricalMaintenance }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ObservedInterval {
    kind: ObservedWorkKind,
    contract: Contract,
    activity_id: u64,
    from: u64,
    until: u64,
    ledger_operation_id: u64,
    device_operation_id: u64,
    timeline_operation_id: u64,
}
impl ObservedInterval {
    pub(crate) fn from_root(contract: Contract, activity_id: u64, from: u64, until: u64,
        ledger_operation_id: u64, device_operation_id: u64, timeline_operation_id: u64) -> Self {
        Self { kind:ObservedWorkKind::OperatingDevice, contract, activity_id, from, until, ledger_operation_id, device_operation_id, timeline_operation_id }
    }
    // Only the owning maintenance adapter may call this after actual member
    // settlement while its original asset is stopped for this work order.
    pub(crate) fn from_maintenance_root(contract: Contract, activity_id: u64, from: u64, until: u64,
        ledger_operation_id: u64, order_operation_id: u64, timeline_operation_id: u64) -> Self {
        Self { kind:ObservedWorkKind::ElectricalMaintenance, contract, activity_id, from, until,
            ledger_operation_id, device_operation_id:order_operation_id, timeline_operation_id }
    }
}

/// Root calls only after original cash debit/credit has been prepared atomically
/// for these exact accounts and this contract. Acknowledgement is not an asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Payment {
    contract: Contract,
    cents: u64,
    cash_operation_id: u64,
}
impl Payment {
    pub(crate) fn from_root(contract: Contract, cents: u64, cash_operation_id: u64) -> Self {
        Self { contract, cents, cash_operation_id }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Begin(Admission),
    Stop { contract_id: u64, activity_id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Phase { Pending, Active, Stopped }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IntervalRecord {
    pub source: EventRef,
    pub from: u64,
    pub until: u64,
    pub ledger_operation_id: u64,
    pub device_operation_id: u64,
    pub timeline_operation_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PaymentRecord {
    pub source: EventRef,
    pub cents: u64,
    pub cash_operation_id: u64,
}

/// One reserved responsibility per immutable contract, never evicted on Stop.
/// Only the last detail is retained; original full history remains in Store receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub contract_id: u64,
    pub phase: Phase,
    pub activity_id: u64,
    pub begin: Option<EventRef>,
    pub qualification_source: Option<ReadSource>,
    pub started: u64,
    pub settled: u64,
    pub work_seconds: u64,
    pub earned: u64,
    pub paid: u64,
    pub remainder: u16,
    pub last_interval: Option<IntervalRecord>,
    pub last_payment: Option<PaymentRecord>,
    pub stopped: Option<EventRef>,
}
impl Record {
    fn pending(contract_id: u64) -> Self {
        Self { contract_id, phase: Phase::Pending, activity_id: 0, begin: None,
            qualification_source: None, started: 0, settled: 0, work_seconds: 0,
            earned: 0, paid: 0, remainder: 0, last_interval: None, last_payment: None, stopped: None }
    }
    pub fn due(&self) -> u64 { self.earned - self.paid }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    version: u32,
    opening: Opening,
    records: [Option<Record>; MAX_CONTRACTS],
    last_ordinal: u64,
    last_operation_id: u64,
    last_tick: u64,
}

pub(crate) struct Candidate { expected: Runtime, next: Runtime }
impl Candidate {
    pub fn next(&self) -> &Runtime { &self.next }
    pub fn matches(&self, current: &Runtime) -> bool { &self.expected == current }
    pub fn into_next(self, current: &Runtime) -> Result<Runtime, Error> {
        if !self.matches(current) { return Err(Error::Stale); }
        Ok(self.next)
    }
}

fn wage(c: Contract, seconds: u64, carry: u16) -> Result<(u64, u16), Error> {
    let people = c.members.end.checked_sub(c.members.start).ok_or(Error::Invalid)?;
    let numerator = u128::from(c.hourly_cents_per_member).checked_mul(u128::from(people))
        .and_then(|n| n.checked_mul(u128::from(seconds)))
        .and_then(|n| n.checked_add(u128::from(carry))).ok_or(Error::Overflow)?;
    let earned = numerator / DENOMINATOR;
    if earned > u128::from(MAX_CENTS) { return Err(Error::Overflow); }
    Ok((earned as u64, (numerator % DENOMINATOR) as u16))
}

/// Discards all bytes while counting exactly the compact serde_json encoding.
/// No Vec, output file or length-dependent allocation is used by this writer.
#[derive(Default)]
struct CountingWriter { bytes: usize, overflow: bool }
impl std::io::Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.bytes.checked_add(bytes.len()) {
            Some(total) => { self.bytes = total; Ok(bytes.len()) }
            None => { self.overflow = true; Err(std::io::ErrorKind::Other.into()) }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
fn encoded_bytes<T: Serialize>(value: &T) -> Result<usize, Error> {
    let mut writer = CountingWriter::default();
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(if writer.overflow { Error::Overflow } else { Error::Invalid });
    }
    Ok(writer.bytes)
}

impl Runtime {
    pub fn new(opening: Opening) -> Result<Self, Error> {
        opening.validate()?;
        if Self::owned_bytes() > MAX_OWNED_BYTES || Self::prepare_extra_bytes() > MAX_CANDIDATE_BYTES
            || Self::terminal_encoded_bound_bytes(&opening)? > MAX_OWNED_BYTES {
            return Err(Error::UnresolvedBudget);
        }
        let records = opening.contracts.map(|c| c.map(|c| Record::pending(c.id)));
        Ok(Self { version: 1, opening, records, last_ordinal: 0, last_operation_id: 0, last_tick: 0 })
    }
    /// Opening-time wire reservation, not raw-layout evidence. Bounds compact
    /// serde_json bytes of THIS Runtime, excluding any root-owned outer wrapper.
    /// Root must budget wrappers/escaping/other encodings separately. Reject an
    /// over-cap Opening before admitting labor, never shrink the four-slot table.
    pub fn terminal_encoded_bound_bytes(opening: &Opening) -> Result<usize, Error> {
        opening.validate()?;
        encoded_bytes(&Self::terminal_size_envelope(opening)?)
    }
    fn terminal_size_envelope(opening: &Opening) -> Result<Self, Error> {
        let last = EventRef { ordinal: u64::MAX, operation_id: u64::MAX, occurred_at: u64::MAX };
        let q = ReadSource { scope: opening.binding.scope, population_origin: opening.binding.population_origin,
            member_version: opening.binding.member_version, root: [u8::MAX; 32], revision: u64::MAX,
            as_of: crate::units::GameTime::new(i64::MAX).map_err(|_| Error::Overflow)? };
        // Conservative envelope of individually legal maximum-width fields, NOT
        // a causally consistent restored state or labor evidence. Some maxima
        // cannot coexist (e.g. equal event ordinals); that only OVER-reserves.
        // Immutable fields remain exact Opening values. All Options are Some,
        // and Stopped ties Pending for the longest serialized phase name.
        let records = opening.contracts.map(|c| c.map(|c| Record {
            contract_id: c.id, phase: Phase::Stopped, activity_id: u64::MAX,
            begin: Some(last), qualification_source: Some(q), started: u64::MAX,
            settled: u64::MAX, work_seconds: u64::MAX, earned: MAX_CENTS, paid: MAX_CENTS,
            remainder: (DENOMINATOR - 1) as u16,
            last_interval: Some(IntervalRecord { source: last, from: u64::MAX, until: u64::MAX,
                ledger_operation_id: u64::MAX, device_operation_id: u64::MAX, timeline_operation_id: u64::MAX }),
            last_payment: Some(PaymentRecord { source: last, cents: MAX_CENTS, cash_operation_id: u64::MAX }),
            stopped: Some(last),
        }));
        Ok(Self { version: 1, opening: *opening, records, last_ordinal: u64::MAX,
            last_operation_id: u64::MAX, last_tick: u64::MAX })
    }
    pub const fn owned_bytes() -> usize { std::mem::size_of::<Self>() }
    /// Includes complete expected + next fixed states, without heap allocations.
    pub const fn candidate_bytes() -> usize { std::mem::size_of::<Candidate>() }
    /// Candidate plus a separate fixed working clone; excludes original owner.
    /// Native compiler stack frames and serializer overlap need root measurement.
    pub const fn prepare_extra_bytes() -> usize { Self::candidate_bytes() + Self::owned_bytes() }
    pub const fn restore_bytes() -> usize { std::mem::size_of::<Self>() }
    pub fn records(&self) -> &[Option<Record>; MAX_CONTRACTS] { &self.records }
    pub fn opening(&self) -> &Opening { &self.opening }
    pub fn next_ordinal(&self) -> Result<u64, Error> { self.last_ordinal.checked_add(1).ok_or(Error::Overflow) }
    /// Original last admitted event reference for root receipt authentication.
    /// This accessor authenticates nothing and never advances the local stream.
    pub fn last_source(&self) -> Option<Source> {
        if self.last_ordinal == 0 { None }
        else { Some(Source { binding: self.opening.binding, ordinal: self.last_ordinal,
            operation_id: self.last_operation_id, occurred_at: self.last_tick }) }
    }

    fn index(&self, id: u64) -> Result<usize, Error> {
        self.opening.contracts.iter().position(|c| c.is_some_and(|c| c.id == id)).ok_or(Error::UnknownContract)
    }
    fn source(&self, s: Source) -> Result<(), Error> {
        self.validate_binding(&self.opening, self.last_tick)?;
        if s.binding != self.opening.binding || s.operation_id == 0 || s.occurred_at < self.last_tick {
            return Err(Error::Binding);
        }
        // One original Envelope may produce multiple local events atomically.
        // Original operation retry/dedup belongs to Store, not an ID set here.
        if s.ordinal != self.next_ordinal()? { return Err(Error::Stale); }
        Ok(())
    }
    fn finish(&self, mut next: Self, s: Source) -> Result<Candidate, Error> {
        // Reserve both final Stop and final INTEGER arrears payment. A partial
        // payment cannot consume the last payment ordinal and leave a balance.
        if u64::MAX - s.ordinal < next.reserved_ordinals() { return Err(Error::UnresolvedBudget); }
        next.last_ordinal = s.ordinal;
        next.last_operation_id = s.operation_id;
        next.last_tick = s.occurred_at;
        next.validate_binding(&self.opening, s.occurred_at)?;
        Ok(Candidate { expected: self.clone(), next })
    }
    fn reserved_ordinals(&self) -> u64 {
        // At most 2 * MAX_CONTRACTS, independent of amount or event history.
        // A stopped fractional remainder stays in its permanent record. It is
        // not a payable integer cent and does not invent another payment event.
        self.records.iter().flatten().map(|r|
            u64::from(r.phase == Phase::Active) + u64::from(r.earned > r.paid)).sum()
    }
    pub fn prepare(&self, s: Source, command: Command) -> Result<Candidate, Error> {
        self.source(s)?;
        let mut next = self.clone();
        match command {
            Command::Begin(a) => {
                let i = self.index(a.contract.id)?;
                let c = self.opening.contracts[i].ok_or(Error::Invalid)?;
                if a.contract != c { return Err(Error::Binding); }
                if self.records[i].ok_or(Error::Invalid)?.phase != Phase::Pending { return Err(Error::State); }
                if a.activity_id == 0 || s.occurred_at < c.start || s.occurred_at >= c.end {
                    return Err(Error::Invalid);
                }
                if self.records.iter().flatten().any(|r| r.activity_id == a.activity_id) { return Err(Error::Binding); }
                check_qualification_binding(a.qualification_source, s)?;
                let r = next.records[i].as_mut().ok_or(Error::Invalid)?;
                r.phase = Phase::Active; r.activity_id = a.activity_id;
                r.begin = Some(s.into()); r.qualification_source = Some(a.qualification_source);
                r.started = s.occurred_at; r.settled = s.occurred_at;
            }
            Command::Stop { contract_id, activity_id } => {
                let i = self.index(contract_id)?;
                let r = next.records[i].as_mut().ok_or(Error::Invalid)?;
                if r.phase != Phase::Active || r.activity_id != activity_id { return Err(Error::State); }
                // Safety exit may be after contract expiry; it adds NO labor.
                // Root must settle any actual work first in the same transaction.
                r.phase = Phase::Stopped; r.stopped = Some(s.into());
            }
        }
        self.finish(next, s)
    }
    pub fn record_interval(&self, s: Source, observed: ObservedInterval) -> Result<Candidate, Error> {
        self.source(s)?;
        let i = self.index(observed.contract.id)?;
        let c = self.opening.contracts[i].ok_or(Error::Invalid)?;
        let mut next = self.clone();
        let r = next.records[i].as_mut().ok_or(Error::Invalid)?;
        if c != observed.contract || r.activity_id != observed.activity_id
            || (c.role == Role::ElectricalMaintenance) != (observed.kind == ObservedWorkKind::ElectricalMaintenance) {
            return Err(Error::Binding);
        }
        if r.phase != Phase::Active { return Err(Error::State); }
        if observed.from != r.settled || observed.until <= observed.from
            || observed.until > c.end || observed.until > s.occurred_at
            || observed.ledger_operation_id == 0 || observed.device_operation_id == 0
            || observed.timeline_operation_id == 0 { return Err(Error::Invalid); }
        let seconds = observed.until - observed.from;
        let (earned, remainder) = wage(c, seconds, r.remainder)?;
        r.earned = r.earned.checked_add(earned).filter(|n| *n <= MAX_CENTS).ok_or(Error::Overflow)?;
        r.work_seconds = r.work_seconds.checked_add(seconds).ok_or(Error::Overflow)?;
        r.remainder = remainder; r.settled = observed.until;
        r.last_interval = Some(IntervalRecord { source: s.into(), from: observed.from, until: observed.until,
            ledger_operation_id: observed.ledger_operation_id, device_operation_id: observed.device_operation_id,
            timeline_operation_id: observed.timeline_operation_id });
        self.finish(next, s)
    }
    pub fn confirm_payment(&self, s: Source, payment: Payment) -> Result<Candidate, Error> {
        self.source(s)?;
        let i = self.index(payment.contract.id)?;
        if Some(payment.contract) != self.opening.contracts[i] { return Err(Error::Binding); }
        let mut next = self.clone();
        let r = next.records[i].as_mut().ok_or(Error::Invalid)?;
        if r.phase == Phase::Pending || payment.cents == 0 || payment.cents > r.due()
            || payment.cash_operation_id == 0 { return Err(Error::Invalid); }
        if r.last_payment.is_some_and(|p| p.cash_operation_id == payment.cash_operation_id) { return Err(Error::Stale); }
        r.paid = r.paid.checked_add(payment.cents).ok_or(Error::Overflow)?;
        r.last_payment = Some(PaymentRecord { source: s.into(), cents: payment.cents, cash_operation_id: payment.cash_operation_id });
        self.finish(next, s)
    }

    /// Structural checks only. Restore must ALSO authenticate original Store
    /// receipts and compare Active records one-for-one with the original ledger.
    /// This method does not repair state or accrue a missing interval on load.
    pub fn validate_binding(&self, original: &Opening, completed: u64) -> Result<(), Error> {
        original.validate()?;
        if self.version != 1 || self.opening != *original || self.last_tick > completed
            || (self.last_ordinal == 0) != (self.last_operation_id == 0) { return Err(Error::Binding); }
        if u64::MAX - self.last_ordinal < self.reserved_ordinals() { return Err(Error::Invalid); }
        let mut max_ordinal = 0;
        for (i, slot) in self.records.iter().enumerate() {
            let Some(c) = original.contracts[i] else {
                if slot.is_some() { return Err(Error::Invalid); }
                continue;
            };
            let r = slot.ok_or(Error::Invalid)?;
            if r.contract_id != c.id { return Err(Error::Binding); }
            if r.phase == Phase::Pending {
                if r != Record::pending(c.id) { return Err(Error::Invalid); }
                continue;
            }
            let begin = r.begin.ok_or(Error::Invalid)?;
            self.validate_source(begin)?;
            check_qualification_binding(r.qualification_source.ok_or(Error::Invalid)?, begin.source(original.binding))?;
            if r.activity_id == 0 || r.started != begin.occurred_at || r.started < c.start
                || r.started >= c.end || r.settled < r.started || r.settled > c.end
                || r.settled > self.last_tick || r.work_seconds != r.settled - r.started
                || r.paid > r.earned || r.earned > MAX_CENTS || u128::from(r.remainder) >= DENOMINATOR {
                return Err(Error::Invalid);
            }
            for other in self.records[..i].iter().flatten() {
                if other.activity_id == r.activity_id { return Err(Error::Binding); }
            }
            let (earned, remainder) = wage(c, r.work_seconds, 0)?;
            if earned != r.earned || remainder != r.remainder { return Err(Error::Invalid); }
            let mut sources = [Some(begin), None, None, None];
            match r.last_interval {
                None if r.work_seconds != 0 => return Err(Error::Invalid),
                Some(v) => {
                    if v.from < r.started || v.from >= v.until || v.until != r.settled
                        || v.until > v.source.occurred_at || v.source.ordinal <= begin.ordinal
                        || v.ledger_operation_id == 0 || v.device_operation_id == 0 || v.timeline_operation_id == 0 {
                        return Err(Error::Invalid);
                    }
                    sources[1] = Some(v.source);
                }
                None => {}
            }
            match r.last_payment {
                None if r.paid != 0 => return Err(Error::Invalid),
                Some(p) => {
                    if p.cents == 0 || p.cents > r.paid || p.cash_operation_id == 0 || p.source.ordinal <= begin.ordinal {
                        return Err(Error::Invalid);
                    }
                    let paid_through = p.source.occurred_at.min(r.settled);
                    if paid_through < r.started || wage(c, paid_through - r.started, 0)?.0 < r.paid {
                        return Err(Error::Invalid);
                    }
                    sources[2] = Some(p.source);
                }
                None => {}
            }
            match (r.phase, r.stopped) {
                (Phase::Active, None) => {}
                (Phase::Stopped, Some(stop)) if stop.occurred_at >= r.settled && stop.ordinal > begin.ordinal => {
                    if r.last_interval.is_some_and(|v| v.source.ordinal >= stop.ordinal) { return Err(Error::Invalid); }
                    sources[3] = Some(stop);
                }
                _ => return Err(Error::Invalid),
            }
            for (n, source) in sources.iter().enumerate() {
                if let Some(source) = source {
                    self.validate_source(*source)?;
                    if source.occurred_at < begin.occurred_at { return Err(Error::Invalid); }
                    for prior in sources[..n].iter().flatten() {
                        if prior.ordinal == source.ordinal { return Err(Error::Invalid); }
                        if (prior.ordinal < source.ordinal && prior.occurred_at > source.occurred_at)
                            || (prior.ordinal > source.ordinal && prior.occurred_at < source.occurred_at) {
                            return Err(Error::Invalid);
                        }
                    }
                    // An original stream ordinal cannot belong to two records.
                    for other in self.records[..i].iter().flatten() {
                        let other_sources = [other.begin, other.last_interval.map(|v| v.source),
                            other.last_payment.map(|p| p.source), other.stopped];
                        for p in other_sources.iter().flatten() {
                            if p.ordinal == source.ordinal
                                || (p.ordinal < source.ordinal && p.occurred_at > source.occurred_at)
                                || (p.ordinal > source.ordinal && p.occurred_at < source.occurred_at) {
                                return Err(Error::Invalid);
                            }
                        }
                    }
                    max_ordinal = max_ordinal.max(source.ordinal);
                }
            }
        }
        if max_ordinal != self.last_ordinal || (self.last_ordinal == 0 && self.last_tick != 0) { return Err(Error::Invalid); }
        Ok(())
    }
    fn validate_source(&self, s: EventRef) -> Result<(), Error> {
        if s.ordinal == 0 || s.ordinal > self.last_ordinal
            || s.operation_id == 0 || s.occurred_at > self.last_tick
            || (s.ordinal == self.last_ordinal && (s.operation_id != self.last_operation_id || s.occurred_at != self.last_tick)) {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

fn check_qualification_binding(q: ReadSource, s: Source) -> Result<(), Error> {
    if q.scope != s.binding.scope || q.population_origin != s.binding.population_origin
        || q.member_version != s.binding.member_version || q.root == [0; 32]
        || q.as_of.value() < 0 || q.as_of.value() as u64 != s.occurred_at { return Err(Error::Binding); }
    // Matching source data is necessary, NOT proof that qualification was Eligible.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::GameTime;

    fn opening(rate: u64, people: u64) -> Opening {
        let binding = Binding { scope: Scope::new(1, 2).unwrap(), population_origin: [3; 32],
            member_version: 1, people: 100, epoch: 1 };
        let c = Contract { id: 1, version: 1, device_id: 9, role: Role::Electric,
            members: MemberRange { start: 0, end: people }, employer_account: 10,
            employee_account: 11, venue_use_id: 12, hourly_cents_per_member: rate, start: 0, end: 10000 };
        Opening { binding, contracts: [Some(c), None, None, None] }
    }
    fn source(r: &Runtime, at: u64) -> Source {
        Source { binding: r.opening.binding, ordinal: r.next_ordinal().unwrap(),
            operation_id: r.next_ordinal().unwrap(), occurred_at: at }
    }
    fn qualification(s: Source) -> ReadSource {
        ReadSource { scope: s.binding.scope, population_origin: s.binding.population_origin,
            member_version: s.binding.member_version, root: [7; 32], revision: 0,
            as_of: GameTime::new(s.occurred_at as i64).unwrap() }
    }
    fn commit(r: &mut Runtime, c: Candidate) { *r = c.into_next(r).unwrap(); }
    fn begin(r: &mut Runtime, id: u64, activity: u64, at: u64) {
        let s = source(r, at); let c = r.opening.contracts[r.index(id).unwrap()].unwrap();
        let candidate = r.prepare(s, Command::Begin(Admission::from_root(c, activity, qualification(s)))).unwrap();
        commit(r, candidate);
    }
    fn interval(r: &Runtime, id: u64, from: u64, until: u64, at: u64) -> ObservedInterval {
        let i = r.index(id).unwrap(); let c = r.opening.contracts[i].unwrap();
        ObservedInterval::from_root(c, r.records[i].unwrap().activity_id, from, until, at + 100, at + 200, at + 300)
    }
    fn accrue(r: &mut Runtime, id: u64, from: u64, until: u64) {
        let candidate = r.record_interval(source(r, until), interval(r, id, from, until, until)).unwrap();
        commit(r, candidate);
    }
    fn stop(r: &mut Runtime, id: u64, at: u64) {
        let i = r.index(id).unwrap();
        let c = r.prepare(source(r, at), Command::Stop { contract_id: id, activity_id: r.records[i].unwrap().activity_id }).unwrap();
        commit(r, c);
    }
    fn pay(r: &mut Runtime, cents: u64, cash: u64, at: u64) {
        let c = r.opening.contracts[0].unwrap();
        let candidate = r.confirm_payment(source(r, at), Payment::from_root(c, cents, cash)).unwrap();
        commit(r, candidate);
    }

    #[test]
    fn maintenance_uses_its_real_order_source_and_original_wage_curve() {
        let mut o = opening(2000, 4);
        o.contracts[0].as_mut().unwrap().role = Role::ElectricalMaintenance;
        o.contracts[0].as_mut().unwrap().version = 2;
        let mut whole = Runtime::new(o).unwrap(); begin(&mut whole, 1, 50, 0);
        let contract = whole.opening.contracts[0].unwrap();
        // An electrical production proof is not a maintenance work order.
        assert!(matches!(whole.record_interval(source(&whole, 3600), interval(&whole, 1, 0, 3600, 3600)),Err(Error::Binding)));
        assert_eq!(whole.records[0].unwrap().earned,0);
        let mut split = whole.clone();
        let event = |from, until| ObservedInterval::from_maintenance_root(contract,50,from,until,101,201,301);
        let candidate = whole.record_interval(source(&whole,3600),event(0,3600)).unwrap();commit(&mut whole,candidate);
        for (from,until) in [(0,3599),(3599,3600)] {
            let candidate = split.record_interval(source(&split,until),event(from,until)).unwrap();commit(&mut split,candidate);
        }
        // Four actual members x 3600s = 14400 person-seconds, at 2000 cents/person-hour.
        for r in [&whole,&split] { let row = r.records[0].unwrap();
            assert_eq!((row.work_seconds,row.earned,row.remainder),(3600,8000,0)); }
        stop(&mut split,1,3600);assert_eq!(split.records[0].unwrap().due(),8000);
        pay(&mut split,3000,401,3600);assert_eq!(split.records[0].unwrap().due(),5000);
        let restored:Runtime=serde_json::from_slice(&serde_json::to_vec(&split).unwrap()).unwrap();
        restored.validate_binding(&split.opening,3600).unwrap();
        assert_eq!(restored.records[0].unwrap(),split.records[0].unwrap());
        // Conversely an ordinary production contract cannot claim a maintenance proof.
        let mut old=Runtime::new(opening(2000,4)).unwrap();begin(&mut old,1,50,0);
        let c=old.opening.contracts[0].unwrap();
        assert!(matches!(old.record_interval(source(&old,1),ObservedInterval::from_maintenance_root(c,50,0,1,101,201,301)),Err(Error::Binding)));
    }

    #[test]
    fn independent_integer_oracle_and_split_merge() {
        for (rate, people) in [(1u64, 1u64), (1001, 3), (3600, 2)] {
            let mut whole = Runtime::new(opening(rate, people)).unwrap();
            begin(&mut whole, 1, 50, 0);
            let mut split = whole.clone();
            accrue(&mut whole, 1, 0, 4);
            accrue(&mut split, 1, 0, 3); accrue(&mut split, 1, 3, 4);
            // Independent small-fixture oracle: integer repeated addition for
            // member-seconds, without the production wage()/carry helper.
            let mut numerator = 0u64;
            for _member in 0..people { for _second in 0..4 { numerator += rate; } }
            let a = whole.records[0].unwrap(); let b = split.records[0].unwrap();
            assert_eq!((a.earned, a.remainder), (numerator / 3600, (numerator % 3600) as u16));
            assert_eq!((a.earned, a.remainder, a.work_seconds), (b.earned, b.remainder, b.work_seconds));
        }
    }

    #[test]
    fn no_future_prefill_or_zero_gapped_overlapping_duplicate_interval() {
        let mut r = Runtime::new(opening(1001, 2)).unwrap(); begin(&mut r, 1, 50, 0);
        assert_eq!(r.records[0].unwrap().earned, 0);
        let before = r.clone();
        for (from, until, at) in [(0, 0, 0), (1, 2, 2), (0, 3, 2), (0, 10001, 10001)] {
            assert!(r.record_interval(source(&r, at), interval(&r, 1, from, until, at)).is_err());
            assert_eq!(r, before);
        }
        let c = r.record_interval(source(&r, 3), interval(&r, 1, 0, 3, 3)).unwrap();
        let stale = r.record_interval(source(&r, 3), interval(&r, 1, 0, 3, 3)).unwrap();
        commit(&mut r, c);
        assert!(matches!(stale.into_next(&r), Err(Error::Stale)));
        assert!(r.record_interval(source(&r, 4), interval(&r, 1, 0, 3, 4)).is_err());
        assert!(r.record_interval(source(&r, 4), interval(&r, 1, 2, 4, 4)).is_err());
    }

    #[test]
    fn source_contract_and_original_activity_must_match() {
        let mut r = Runtime::new(opening(100, 1)).unwrap();
        let s = source(&r, 0); let c = r.opening.contracts[0].unwrap();
        let mut q = qualification(s); q.population_origin = [8; 32];
        assert!(r.prepare(s, Command::Begin(Admission::from_root(c, 50, q))).is_err());
        q = qualification(s); q.root = [0; 32];
        assert!(r.prepare(s, Command::Begin(Admission::from_root(c, 50, q))).is_err());
        let mut changed = c; changed.hourly_cents_per_member -= 1;
        assert!(r.prepare(s, Command::Begin(Admission::from_root(changed, 50, qualification(s)))).is_err());
        assert!(r.prepare(s, Command::Begin(Admission::from_root(c, 0, qualification(s)))).is_err());
        begin(&mut r, 1, 50, 0);
        let s = source(&r, 1); let mut observed = interval(&r, 1, 0, 1, 1);
        observed.activity_id += 1;
        assert!(r.record_interval(s, observed).is_err());
        observed = interval(&r, 1, 0, 1, 1); observed.device_operation_id = 0;
        assert!(r.record_interval(s, observed).is_err());
        let mut bad = s; bad.binding.member_version += 1;
        assert!(r.record_interval(bad, interval(&r, 1, 0, 1, 1)).is_err());
        bad = s; bad.ordinal += 1;
        assert!(matches!(r.record_interval(bad, interval(&r, 1, 0, 1, 1)), Err(Error::Stale)));
        // Unknown eligibility cannot be settled by this module: no Eligible bool
        // exists. Matching q source alone is NOT a qualification test. Root must
        // call labor_qualification and refuse Unknown before constructing Admission.
    }

    #[test]
    fn stop_preserves_unpaid_fraction_and_does_not_fill_gap() {
        let mut r = Runtime::new(opening(1001, 3)).unwrap(); begin(&mut r, 1, 50, 0);
        accrue(&mut r, 1, 0, 4);
        let before = r.records[0].unwrap(); stop(&mut r, 1, 9000);
        let stopped = r.records[0].unwrap();
        assert_eq!((stopped.earned, stopped.paid, stopped.remainder, stopped.work_seconds),
            (before.earned, before.paid, before.remainder, before.work_seconds));
        assert_eq!(stopped.due(), 3); assert_eq!(stopped.remainder, 1212);
        assert!(r.record_interval(source(&r, 9001), interval(&r, 1, 4, 5, 9001)).is_err());
        let c = r.opening.contracts[0].unwrap(); let s = source(&r, 9001);
        assert!(r.prepare(s, Command::Begin(Admission::from_root(c, 51, qualification(s)))).is_err());
        pay(&mut r, 1, 70, 9001);
        assert_eq!(r.records[0].unwrap().due(), 2);
        assert_eq!(r.records[0].unwrap().remainder, 1212);
    }

    #[test]
    fn partial_payment_duplicate_overpay_and_same_envelope_events() {
        let mut r = Runtime::new(opening(3600, 1)).unwrap(); begin(&mut r, 1, 50, 0);
        let original_op = 90;
        let mut s = source(&r, 4); s.operation_id = original_op;
        let candidate = r.record_interval(s, interval(&r, 1, 0, 4, 4)).unwrap(); commit(&mut r, candidate);
        let c = r.opening.contracts[0].unwrap();
        s = source(&r, 4); s.operation_id = original_op; // same Envelope, next local ordinal
        let candidate = r.confirm_payment(s, Payment::from_root(c, 1, original_op)).unwrap();
        commit(&mut r, candidate);
        assert_eq!(r.last_source(), Some(s));
        assert_eq!(r.records[0].unwrap().due(), 3);
        assert!(matches!(r.confirm_payment(s, Payment::from_root(c, 1, original_op)), Err(Error::Stale)));
        assert!(matches!(r.confirm_payment(source(&r, 4), Payment::from_root(c, 1, original_op)), Err(Error::Stale)));
        for cents in [0, 4] { assert!(r.confirm_payment(source(&r, 4), Payment::from_root(c, cents, 91)).is_err()); }
        pay(&mut r, 2, 91, 4); pay(&mut r, 1, 92, 4);
        assert_eq!(r.records[0].unwrap().due(), 0);
    }

    #[test]
    fn overflow_rejects_atomically_instead_of_erasing_work() {
        let mut o = opening(MAX_CENTS, 100); o.contracts[0].as_mut().unwrap().end = u64::MAX;
        assert!(matches!(Runtime::new(o), Err(Error::Overflow)));
        assert!(matches!(wage(o.contracts[0].unwrap(), u64::MAX, 0), Err(Error::Overflow)));
        let mut r = Runtime::new(opening(1001, 3)).unwrap(); begin(&mut r, 1, 50, 0);
        let before = r.clone();
        assert!(r.record_interval(source(&r, u64::MAX), interval(&r, 1, 0, u64::MAX, 1)).is_err());
        assert_eq!(r, before);
        stop(&mut r, 1, 3600); // closing never computes unobserved future wages
    }

    #[test]
    fn restore_structure_tampering_is_rejected_without_catchup() {
        let mut r = Runtime::new(opening(1001, 3)).unwrap(); begin(&mut r, 1, 50, 0);
        accrue(&mut r, 1, 0, 4); pay(&mut r, 1, 70, 4); stop(&mut r, 1, 5);
        let before = r.clone();
        assert_eq!(r.validate_binding(&r.opening, 10000), Ok(()));
        assert_eq!(r, before);
        let mut variants = [r.clone(), r.clone(), r.clone(), r.clone(), r.clone(), r.clone(), r.clone(), r.clone()];
        variants[0].records[0].as_mut().unwrap().earned += 1;
        variants[1].records[0].as_mut().unwrap().paid = 100;
        variants[2].records[0].as_mut().unwrap().remainder += 1;
        variants[3].records[0].as_mut().unwrap().work_seconds += 1;
        variants[4].records[0].as_mut().unwrap().activity_id = 0;
        variants[5].records[0].as_mut().unwrap().last_interval.as_mut().unwrap().source.ordinal = 1;
        variants[6].records[0].as_mut().unwrap().last_payment = None;
        variants[7].last_ordinal += 1;
        for v in variants { assert!(v.validate_binding(&r.opening, 10000).is_err()); }
        let mut changed = r.opening; changed.contracts[0].as_mut().unwrap().hourly_cents_per_member -= 1;
        assert!(r.validate_binding(&changed, 10000).is_err());
        assert!(r.validate_binding(&r.opening, 4).is_err());
    }

    #[test]
    fn all_reserved_records_can_close_and_duplicate_activity_is_rejected() {
        let mut o = opening(1, 1); let base = o.contracts[0].unwrap();
        for (i, slot) in o.contracts.iter_mut().enumerate() {
            let mut c = base; c.id = i as u64 + 1; c.members = MemberRange { start: i as u64, end: i as u64 + 1 };
            c.role = if i % 2 == 0 { Role::Electric } else { Role::Water }; *slot = Some(c);
        }
        let mut r = Runtime::new(o).unwrap();
        for id in 1..=4 { begin(&mut r, id, id + 50, 0); }
        assert_eq!(r.records.iter().flatten().filter(|r| r.phase == Phase::Active).count(), 4);
        for id in 1..=4 { stop(&mut r, id, 1); }
        assert_eq!(r.records.iter().flatten().filter(|r| r.phase == Phase::Stopped).count(), 4);
        let mut r = Runtime::new(o).unwrap(); begin(&mut r, 1, 51, 0);
        let s = source(&r, 0); let c = o.contracts[1].unwrap();
        assert!(r.prepare(s, Command::Begin(Admission::from_root(c, 51, qualification(s)))).is_err());
        assert!(Runtime::owned_bytes() <= MAX_OWNED_BYTES);
        assert!(Runtime::candidate_bytes() <= MAX_CANDIDATE_BYTES);
        assert!(Runtime::prepare_extra_bytes() <= MAX_CANDIDATE_BYTES);
    }

    #[test]
    fn malformed_opening_is_not_silently_repaired() {
        let base = opening(100, 1);
        assert_eq!(Runtime::new(base).unwrap().last_source(), None);
        let mut variants = [base; 5];
        variants[0].contracts[0].as_mut().unwrap().hourly_cents_per_member = 0;
        variants[1].contracts[0].as_mut().unwrap().members.end = 101;
        variants[2].contracts[0].as_mut().unwrap().end = 0;
        variants[3].contracts[1] = variants[3].contracts[0];
        variants[4].contracts[2] = variants[4].contracts[0]; variants[4].contracts[0] = None;
        for v in variants { assert!(Runtime::new(v).is_err()); }
    }

    #[test]
    fn sequence_reservation_blocks_new_work_but_allows_every_stop() {
        let mut o = opening(1, 1); let base = o.contracts[0].unwrap();
        for (i, slot) in o.contracts.iter_mut().enumerate() {
            let mut c = base; c.id = i as u64 + 1;
            c.members = MemberRange { start: i as u64, end: i as u64 + 1 }; *slot = Some(c);
        }
        let mut r = Runtime::new(o).unwrap();
        for id in 1..=4 { begin(&mut r, id, id + 50, 0); }
        // Hypothetical authenticated high-watermark restore, not a real receipt.
        for (i, row) in r.records.iter_mut().flatten().enumerate() {
            row.begin.as_mut().unwrap().ordinal = u64::MAX - 7 + i as u64;
        }
        r.last_ordinal = u64::MAX - 4;
        assert_eq!(r.validate_binding(&o, 0), Ok(()));
        assert!(matches!(r.record_interval(source(&r, 1), interval(&r, 1, 0, 1, 1)), Err(Error::UnresolvedBudget)));
        for id in 1..=4 { stop(&mut r, id, 1); }
        assert_eq!(r.last_ordinal, u64::MAX);
        assert_eq!(r.validate_binding(&o, 1), Ok(()));
    }

    /// Synthetic authenticated-prefix fixture only; no real receipt is produced.
    fn move_watermark(r: &mut Runtime, last: u64) {
        let delta = last.checked_sub(r.last_ordinal).unwrap();
        for row in r.records.iter_mut().flatten() {
            if let Some(v) = row.begin.as_mut() { v.ordinal += delta; }
            if let Some(v) = row.last_interval.as_mut() { v.source.ordinal += delta; }
            if let Some(v) = row.last_payment.as_mut() { v.source.ordinal += delta; }
            if let Some(v) = row.stopped.as_mut() { v.ordinal += delta; }
        }
        r.last_ordinal = last;
    }

    #[test]
    fn near_limit_stop_restore_and_final_arrears_payment_remain_possible() {
        let mut r = Runtime::new(opening(3600, 1)).unwrap(); begin(&mut r, 1, 50, 0);
        accrue(&mut r, 1, 0, 1);
        move_watermark(&mut r, u64::MAX - 2);
        assert_eq!(r.reserved_ordinals(), 2);
        assert_eq!(r.validate_binding(&r.opening, 1), Ok(()));
        let mut dead = r.clone(); move_watermark(&mut dead, u64::MAX - 1);
        assert_eq!(dead.validate_binding(&r.opening, 1), Err(Error::Invalid));
        stop(&mut r, 1, 1);
        assert_eq!(r.last_ordinal, u64::MAX - 1);
        let bytes = serde_json::to_vec(&r).unwrap();
        let mut restored: Runtime = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.validate_binding(&r.opening, 1), Ok(()));
        assert_eq!(restored.records[0].unwrap().due(), 1);
        dead = restored.clone(); move_watermark(&mut dead, u64::MAX);
        assert_eq!(dead.validate_binding(&r.opening, 1), Err(Error::Invalid));
        pay(&mut restored, 1, 900, 1);
        assert_eq!(restored.last_ordinal, u64::MAX);
        assert_eq!(restored.records[0].unwrap().due(), 0);
        assert_eq!(restored.validate_binding(&r.opening, 1), Ok(()));
    }

    #[test]
    fn final_payment_slot_refuses_partial_payment_without_mutation() {
        let mut r = Runtime::new(opening(7200, 1)).unwrap(); begin(&mut r, 1, 50, 0);
        accrue(&mut r, 1, 0, 1); stop(&mut r, 1, 1);
        move_watermark(&mut r, u64::MAX - 1);
        assert_eq!(r.validate_binding(&r.opening, 1), Ok(()));
        let before = r.clone(); let c = r.opening.contracts[0].unwrap();
        assert!(matches!(r.confirm_payment(source(&r, 1), Payment::from_root(c, 1, 901)), Err(Error::UnresolvedBudget)));
        assert_eq!(r, before);
        pay(&mut r, 2, 902, 1);
        assert_eq!(r.last_ordinal, u64::MAX);
        assert_eq!(r.records[0].unwrap().due(), 0);
    }

    #[test]
    fn every_contract_reserves_stop_and_final_payment_independently() {
        let mut o = opening(3600, 1); let base = o.contracts[0].unwrap();
        for (i, slot) in o.contracts.iter_mut().enumerate() {
            let mut c = base; c.id = i as u64 + 1;
            c.members = MemberRange { start: i as u64, end: i as u64 + 1 }; *slot = Some(c);
        }
        let mut r = Runtime::new(o).unwrap();
        for id in 1..=4 { begin(&mut r, id, id + 50, 0); }
        for id in 1..=4 { accrue(&mut r, id, 0, 1); }
        move_watermark(&mut r, u64::MAX - 8);
        assert_eq!(r.reserved_ordinals(), 8);
        assert_eq!(r.validate_binding(&o, 1), Ok(()));
        for id in 1..=4 { stop(&mut r, id, 1); }
        let bytes = serde_json::to_vec(&r).unwrap();
        let mut restored: Runtime = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.validate_binding(&o, 1), Ok(()));
        for id in 1..=4 {
            let c = o.contracts[restored.index(id).unwrap()].unwrap();
            let next = restored.confirm_payment(source(&restored, 1), Payment::from_root(c, 1, 910 + id)).unwrap();
            commit(&mut restored, next);
        }
        assert_eq!(restored.last_ordinal, u64::MAX);
        assert_eq!(restored.reserved_ordinals(), 0);
        assert!(restored.records.iter().flatten().all(|r| r.due() == 0 && r.phase == Phase::Stopped));
    }

    #[test]
    fn new_integer_wages_require_new_payment_reserve_before_real_work() {
        let mut r = Runtime::new(opening(3600, 1)).unwrap(); begin(&mut r, 1, 50, 0);
        move_watermark(&mut r, u64::MAX - 2);
        assert_eq!(r.validate_binding(&r.opening, 0), Ok(()));
        let before = r.clone();
        assert!(matches!(r.record_interval(source(&r, 1), interval(&r, 1, 0, 1, 1)), Err(Error::UnresolvedBudget)));
        assert_eq!(r, before);
        let mut joint = Runtime::new(opening(3600, 1)).unwrap(); begin(&mut joint, 1, 50, 0);
        move_watermark(&mut joint, u64::MAX - 3);
        accrue(&mut joint, 1, 0, 1); pay(&mut joint, 1, 920, 1); stop(&mut joint, 1, 1);
        assert_eq!(joint.last_ordinal, u64::MAX);
        assert_eq!(joint.validate_binding(&joint.opening, 1), Ok(()));
    }

    #[test]
    fn stopped_subcent_responsibility_survives_restore_without_fake_payment() {
        let mut r = Runtime::new(opening(1, 1)).unwrap(); begin(&mut r, 1, 50, 0);
        accrue(&mut r, 1, 0, 1); move_watermark(&mut r, u64::MAX - 1); stop(&mut r, 1, 1);
        let bytes = serde_json::to_vec(&r).unwrap();
        let restored: Runtime = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.validate_binding(&r.opening, 1), Ok(()));
        let row = restored.records[0].unwrap();
        assert_eq!((row.earned, row.paid, row.remainder), (0, 0, 1));
        assert_eq!(restored.reserved_ordinals(), 0);
        assert_eq!(restored.last_ordinal, u64::MAX);
        assert!(row.last_payment.is_none());
    }

    #[test]
    fn counting_writer_matches_compact_json_and_checks_overflow() {
        let o = opening(1001, 3);
        let envelope = Runtime::terminal_size_envelope(&o).unwrap();
        assert_eq!(encoded_bytes(&envelope).unwrap(), serde_json::to_vec(&envelope).unwrap().len());
        let mut writer = CountingWriter { bytes: usize::MAX, overflow: false };
        assert!(std::io::Write::write(&mut writer, &[1]).is_err());
        assert!(writer.overflow);
        assert_eq!(writer.bytes, usize::MAX);
    }

    #[test]
    fn terminal_bound_covers_pristine_active_partial_paid_stopped_and_fraction() {
        let o = opening(1001, 3);
        let bound = Runtime::terminal_encoded_bound_bytes(&o).unwrap();
        let mut r = Runtime::new(o).unwrap();
        assert!(serde_json::to_vec(&r).unwrap().len() <= bound);
        begin(&mut r, 1, u64::MAX, 0);
        assert!(serde_json::to_vec(&r).unwrap().len() <= bound);
        accrue(&mut r, 1, 0, 4); pay(&mut r, 1, u64::MAX, 4);
        assert!(serde_json::to_vec(&r).unwrap().len() <= bound);
        stop(&mut r, 1, 4);
        assert!(serde_json::to_vec(&r).unwrap().len() <= bound);
        assert_eq!(r.records[0].unwrap().remainder, 1212);
        pay(&mut r, 2, u64::MAX - 1, 4);
        assert!(serde_json::to_vec(&r).unwrap().len() <= bound);
        // Options stay Some, qualification hash bytes and revision use their
        // largest width in the envelope, and all three phase names are bounded.
        for phase in [Phase::Pending, Phase::Active, Phase::Stopped] {
            assert!(serde_json::to_vec(&phase).unwrap().len() <= serde_json::to_vec(&Phase::Stopped).unwrap().len());
        }
    }

    #[test]
    fn high_width_four_contract_registration_is_subject_to_wire_gate() {
        let mut o = opening(1, 1);
        o.binding = Binding { scope: Scope::new(u64::MAX, u64::MAX).unwrap(),
            population_origin: [u8::MAX; 32], member_version: u32::MAX,
            people: u64::MAX, epoch: u64::MAX };
        let base = o.contracts[0].unwrap();
        for (i, slot) in o.contracts.iter_mut().enumerate() {
            let n = i as u64;
            *slot = Some(Contract { id: u64::MAX - n, version: u32::MAX, device_id: u64::MAX,
                members: MemberRange { start: u64::MAX - n - 1, end: u64::MAX - n },
                employer_account: u32::MAX, employee_account: u32::MAX - 1, venue_use_id: u64::MAX,
                hourly_cents_per_member: MAX_CENTS, start: 0, end: 1, ..base });
        }
        assert_eq!(o.validate(), Ok(()));
        let bound = Runtime::terminal_encoded_bound_bytes(&o).unwrap();
        assert_eq!(bound, serde_json::to_vec(&Runtime::terminal_size_envelope(&o).unwrap()).unwrap().len());
        if bound > MAX_OWNED_BYTES {
            assert!(matches!(Runtime::new(o), Err(Error::UnresolvedBudget)));
        } else {
            assert!(Runtime::new(o).is_ok());
        }
        // Never silently drop a fourth contract or increase the original cap.
        assert_eq!(o.contracts.iter().flatten().count(), MAX_CONTRACTS);
        assert_eq!(MAX_OWNED_BYTES, 8192);
    }
}
