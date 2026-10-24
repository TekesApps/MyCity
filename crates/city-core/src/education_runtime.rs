//! Free offline whole-group education proposals. Original root owns commits and authority.
//! No public Fact ingress, autonomous clock, population table, capacity or money pool.
use crate::education::{self as e, Cohort, CohortCheckpoint, Course, Enrollment};
use crate::units::GameTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const MAX_COHORTS: usize = 4;
pub const MAX_ACTIVITIES: usize = 32;
pub const MAX_RECENT_EVENTS: usize = 32;
pub const MAX_SHARED_SCAN: usize = 128;
pub const MAX_JSON_BYTES: usize = 128 * 1024;
const RUNTIME_STRATEGY: u32 = 2;
const HISTORICAL_BATCH_SECONDS: u64 = 86400;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { FreeOfflineWholeGroupV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Members { pub source: u64, pub version: u32, pub start: u64, pub end: u64 }
impl Members {
    fn valid(self, total: u64) -> bool { self.source == 1 && self.version == 1 && self.start < self.end && self.end <= total }
    fn overlaps(self, other: Self) -> bool {
        self.source == other.source && self.version == other.version && self.start < other.end && other.start < self.end
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualificationOrigin { ImmutableGameOpeningV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FreeBasis { ImmutableGameOpeningEntitlementV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationRegistration {
    pub origin: QualificationOrigin,
    pub source_ref: u64, pub course_ref: u64, pub teacher: Members,
    pub valid_from: GameTime, pub valid_until: GameTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreeContractRegistration {
    pub basis: FreeBasis,
    pub source_ref: u64, pub contract_ref: u64, pub course_ref: u64, pub roster_ref: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub enrollment: Enrollment, pub course: Course,
    pub learners: Members, pub teacher: Members, pub use_id: u64,
    pub qualification: QualificationRegistration, pub free_contract: FreeContractRegistration,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version: u32, pub profile: Profile, pub identity: e::Identity,
    pub stream_epoch: u64, pub population_origin: [u8;32],
    pub courses: [Option<Registration>; MAX_COHORTS],
}
/// Root resolves this projection from the ORIGINAL immutable venue offer/resource.
/// It carries no mutable balance and grants no seats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisteredOffer {
    pub use_id: u64, pub venue_id: u64, pub group_id: u64, pub resource_id: u32,
    pub members: Members, pub learners: u32, pub teachers: u32, pub staff: u32,
    pub visitors: u32, pub enrollment: u32, pub window: e::Window,
}
/// Not serializable or externally constructible. Caller has authenticated original
/// Opening/Housing origin, course/qualification and EXPLICIT free contract registrations.
pub(crate) struct VerifiedOpening<'a> {
    identity: e::Identity, input_hash: [u8;32], registration_origin: [u8;32], population_origin: [u8;32], total_people: u64,
    offers: &'a [RegisteredOffer], qualifications: &'a [QualificationRegistration],
    free_contracts: &'a [FreeContractRegistration],
}
impl<'a> VerifiedOpening<'a> {
    pub(crate) fn after_root_opening_validation(identity: e::Identity, input_hash: [u8;32], registration_origin: [u8;32],
        population_origin: [u8;32], total_people: u64, offers: &'a [RegisteredOffer],
        qualifications: &'a [QualificationRegistration], free_contracts: &'a [FreeContractRegistration]) -> Self {
        Self {identity,input_hash,registration_origin,population_origin,total_people,offers,qualifications,free_contracts}
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub identity: e::Identity, pub root: [u8;32], pub revision: u64,
    pub clock: e::Clock,
    pub operation_id: u64, pub event_id: u64,
    /// A root-assigned canonical education-host stream, NOT numeric ordering of event_id.
    pub stream_epoch: u64, pub stream_sequence: u64,
    pub occurred_at: GameTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Action {
    ActivateNoInstruction,
    BeginPreparation,
    SettlePreparation { started_event_id: u64, close: bool },
    BeginLesson,
    SettleLesson { started_event_id: u64, close: bool },
    /// Historical batch, observed at the current tick. Only finalize=true certifies
    /// and consumes the NoInstruction suffix; false retains all remaining responsibility.
    SettleAfterExit { started_event_id: u64, exited_at: GameTime, exit_operation_id: u64, actual_through: GameTime, finalize: bool },
    /// SafeExit hook: cut off physical activity now, without inventing historical hours.
    StopForExit { started_event_id: u64, exited_at: GameTime, exit_operation_id: u64 },
    NoInstruction { window: e::Window },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command { pub cohort: u64, pub action: Action }
/// Permission marker from the original System host transaction boundary, never serde.
/// The caller checks System actor, exact raw game event, canonical stream mapping and
/// stable Timeline (phase/target both None); an Interaction clock cannot be substituted.
pub(crate) struct VerifiedEvent { command: Command, source: Origin }
impl VerifiedEvent {
    pub(crate) fn after_system_host_validation(command: Command, source: Origin,
        completed_development_seconds: GameTime) -> Result<Self, Error> {
        if source.occurred_at != completed_development_seconds { return Err(Error::Clock); }
        Ok(Self {command,source})
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityKind { Preparation, Teaching, Learning, Work, Travel }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role { Teacher, Learners }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimKey { pub epoch: u64, pub sequence: u64, pub cohort: u64, pub role: Role }
/// Borrowed projection of the ONE root-owned shared member activity ledger. Incomplete
/// ends reserve the actual member time from start immediately, not only on completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SharedActivity {
    pub key: ClaimKey, pub members: Members, pub kind: ActivityKind,
    pub start: GameTime, pub settled_through: GameTime, pub end: Option<GameTime>, pub source: Origin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VenueCoverage {
    pub root: [u8;32], pub use_id: u64, pub venue_id: u64, pub group_id: u64,
    pub resource_id: u32, pub token_number: u64, pub members: Members,
    pub offer_window: e::Window, pub entered_at: GameTime, pub exited_at: Option<GameTime>,
}
/// Root resolves the exact original SafeExit and authenticates the explicit host's
/// no-instruction suffix. Persisted copies are provenance DATA, never permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActualCoverage {
    pub identity: e::Identity, pub root: [u8;32], pub cohort: u64, pub generation: u64,
    pub original_begin_event_id: u64, pub kind: ActivityKind, pub window: e::Window,
    pub source_operation_id: u64, pub source_event_id: u64, pub source_revision: u64,
    pub observed_at: GameTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoppedCoverage {
    pub identity: e::Identity, pub root: [u8;32], pub use_id: u64, pub token_number: u64,
    pub exited_at: GameTime, pub exit_operation_id: u64, pub exit_event_id: u64, pub exit_revision: u64,
    pub no_instruction: e::Window,
    /// Independently authenticated whole-group activity coverage; NOT venue presence.
    pub actual: Option<ActualCoverage>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LedgerCoverage { EducationOnlyV1 }
/// Root has revalidated original Resource/Lease/token, current venue lifecycle,
/// qualification/member origin and original host event at this exact root/revision.
/// Presence is only venue coverage; VerifiedEvent separately establishes the activity.
pub(crate) struct Snapshot<'a> {
    event: VerifiedEvent, opening_hash: [u8;32], population_origin: [u8;32],
    venue: Option<VenueCoverage>, activities: &'a [SharedActivity], coverage: LedgerCoverage,
    stopped: Option<StoppedCoverage>,
}
impl<'a> Snapshot<'a> {
    pub(crate) fn after_root_source_validation(event: VerifiedEvent, opening_hash: [u8;32],
        population_origin: [u8;32], venue: Option<VenueCoverage>, activities: &'a [SharedActivity],
        coverage: LedgerCoverage, stopped: Option<StoppedCoverage>) -> Self {
        Self {event,opening_hash,population_origin,venue,activities,coverage,stopped}
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivityDelta {
    Acquire { teacher: SharedActivity, learners: Option<SharedActivity> },
    Settle { teacher: ClaimKey, learners: Option<ClaimKey>, from: GameTime, through: GameTime, close: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Episode { Activation, Preparation, Lesson, NoInstruction }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    cohort: u64, kind: Episode, start: GameTime, end: Option<GameTime>,
    began: Origin, ended: Option<Origin>, settled_through: GameTime, last_settlement: Option<Origin>, token_number: Option<u64>,
    cutoff: Option<StoppedCoverage>,
    /// Monotone known actual-activity endpoint. A smaller later chunk is not permission
    /// to relabel the certified remainder as NoInstruction.
    known_actual_end: GameTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostInput { command: Command, source: Origin, venue_token: Option<u64>, stopped: Option<StoppedCoverage> }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State { cohort: Option<CohortCheckpoint>, prepared_day: i64, prepared_seconds: u64, pending: Option<u8>, awaiting: Option<u8> }
impl State { const EMPTY: Self = Self {cohort:None,prepared_day:-1,prepared_seconds:0,pending:None,awaiting:None}; }
/// A durable original Store receipt/checkpoint anchor, not an event-ID ordering heuristic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptAnchor { pub store_ref: u64, pub root: [u8;32], pub epoch: u64, pub through: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StoppedActivity {
    pub begin: Origin, pub kind: ActivityKind, pub original_start: GameTime,
    pub consumed_through: GameTime, pub known_actual_end: GameTime, pub cutoff: StoppedCoverage,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prefix {
    states: [State; MAX_COHORTS], through: u64, last_source: Option<Origin>, anchor: Option<ReceiptAnchor>,
    /// Original unclosed rows are RETAINED, not merged into an average or dropped.
    open_records: [Option<(u8,Record)>; MAX_COHORTS],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    // Required, no serde default: pre-fix snapshots must not be silently upgraded,
    // including prefixes whose unsupported Stop evidence has already been compacted.
    strategy_version: u32,
    opening: Opening, opening_hash: [u8;32], registration_origin: [u8;32],
    total_people: u64,
    offers: [Option<RegisteredOffer>; MAX_COHORTS],
    prefix: Prefix, states: [State; MAX_COHORTS], records: [Option<Record>; MAX_ACTIVITIES],
    consumed_sequence: u64, last_source: Option<Origin>,
    recent: [Option<HostInput>; MAX_RECENT_EVENTS],
    #[serde(skip)]
    ready: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error { Invalid, Unknown, Clock, Cursor, Duplicate, Conflict, Limit, NeedsCompaction, Pending, Boundary, Restore, Rule(e::Error) }
impl From<e::Error> for Error { fn from(value: e::Error) -> Self { Self::Rule(value) } }
/// Only the changed state/record are copied; immutable Opening is not cloned per plan.
pub(crate) struct Candidate {
    expected_hash: [u8;32], source: Origin, state_slot: usize, record_slot: usize,
    state: State, record: Record, pub(crate) activity_delta: Option<ActivityDelta>,
    input: HostInput, recent_slot: usize,
}
impl Candidate { pub(crate) fn source(&self) -> Origin { self.source } }
pub(crate) struct VerifiedRestore { state_hash: [u8;32], opening_hash: [u8;32], registration_origin: [u8;32], anchor: Option<ReceiptAnchor> }
impl VerifiedRestore {
    /// Root first checks original Store root/hash/receipt chain and shared-ledger recovery.
    pub(crate) fn after_original_store_validation(state_hash: [u8;32], opening_hash: [u8;32], registration_origin: [u8;32], anchor: Option<ReceiptAnchor>) -> Self {
        Self {state_hash,opening_hash,registration_origin,anchor}
    }
}
pub(crate) struct VerifiedCompaction { state_hash: [u8;32], anchor: ReceiptAnchor }
impl VerifiedCompaction {
    /// Root must have durably anchored this exact state and every source receipt through
    /// the selected stream sequence; shared-ledger closed history must remain recoverable.
    pub(crate) fn after_durable_prefix_validation(state_hash: [u8;32], anchor: ReceiptAnchor) -> Self {
        Self {state_hash,anchor}
    }
}
struct HashWriter { hash: Sha256, bytes: usize }
impl Write for HashWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.len() > MAX_JSON_BYTES.saturating_sub(self.bytes) { return Err(io::Error::new(io::ErrorKind::InvalidData,"education_json_bound")); }
        self.hash.update(data); self.bytes += data.len(); Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn fingerprint<T: Serialize>(domain: &[u8], value: &T) -> Result<[u8;32], Error> {
    let mut writer = HashWriter {hash:Sha256::new(),bytes:0}; writer.hash.update(domain);
    serde_json::to_writer(&mut writer,value).map_err(|_| Error::Invalid)?;
    Ok(writer.hash.finalize().into())
}
impl Opening {
    pub fn fingerprint(&self) -> Result<[u8;32], Error> { fingerprint(b"education-opening-v1",self) }
    /// Independent immutable registration identity. No current Store/World hash and
    /// no Runtime field is included, so initialization has no final-root self-reference.
    pub fn registration_origin(&self) -> Result<[u8;32], Error> {
        let mut h=Sha256::new(); h.update(b"education-registration-origin-v1");
        for word in [self.identity.world,self.identity.branch,self.identity.scope] {h.update(word.to_le_bytes());}
        h.update(self.fingerprint()?); h.update(self.population_origin); Ok(h.finalize().into())
    }
}
fn window(start: GameTime,end: GameTime) -> Result<e::Window, Error> {
    if start >= end { return Err(Error::Boundary); } Ok(e::Window {start,end})
}
fn view(source: Origin) -> e::View {
    e::View {identity:source.identity,root:source.root,version:e::VERSION,clock:source.clock,
        as_of:source.occurred_at,max_age_seconds:0,policy_ref:1}
}
fn fact(source: Origin,w: e::Window,kind: e::Kind,subject: u64,binding: [u8;32]) -> Option<e::Fact> {
    Some(e::Fact {view:view(source),source_ref:source.event_id,subject,kind,coverage:w,
        observed_at:source.occurred_at,value:true,binding})
}
fn overlap(a: GameTime,b: Option<GameTime>,c: GameTime,d: Option<GameTime>) -> bool {
    b.is_none_or(|end| c < end) && d.is_none_or(|end| a < end)
}
impl Runtime {
    /// Exact integer reservation for a CANONICAL full-size batch completion path.
    /// Pending may run until GameTime's existing i64::MAX bound: one Stop plus
    /// ceil(unconsumed seconds/day) positive history batches plus one final suffix.
    /// Awaiting retains the remaining batches + final suffix; other work cannot spend it.
    /// Recent slots are reusable only via authenticated, sequence-free prefix compaction.
    fn responsibility_reserve(&self, changed: Option<&Candidate>) -> Result<(u64,usize),Error> {
        let mut ordinals=0u64;
        let mut slots=0usize;
        for i in 0..MAX_COHORTS {
            let state=changed.filter(|c| c.state_slot==i).map_or(self.states[i],|c| c.state);
            if state.pending.is_some() && state.awaiting.is_some() {return Err(Error::Restore);}
            let Some(index)=state.pending.or(state.awaiting) else {continue};
            let index=usize::from(index);
            let row=if let Some(c)=changed.filter(|c| c.record_slot==index) {c.record}
                else {self.records.get(index).copied().flatten().ok_or(Error::Restore)?};
            let checkpoint=state.cohort.ok_or(Error::Restore)?;
            if checkpoint.cursor != row.settled_through {return Err(Error::Restore);}
            let horizon=if state.pending.is_some() {GameTime::new(i64::MAX).map_err(|_| Error::Restore)?}
                else {row.cutoff.ok_or(Error::Restore)?.exited_at};
            if checkpoint.cursor>horizon {return Err(Error::Restore);}
            let seconds=(horizon.value()-checkpoint.cursor.value()) as u64;
            let batches=seconds/HISTORICAL_BATCH_SECONDS+u64::from(seconds%HISTORICAL_BATCH_SECONDS!=0);
            let tail=batches.checked_add(if state.pending.is_some() {2} else {1}).ok_or(Error::Limit)?;
            ordinals=ordinals.checked_add(tail).ok_or(Error::Limit)?;
            slots+=if state.pending.is_some() {2} else {1};
            let domain_steps=batches.checked_mul(2).and_then(|n| n.checked_add(1)).ok_or(Error::Limit)?;
            if u64::MAX-checkpoint.sequence<domain_steps {return Err(Error::Limit);}
        }
        Ok((ordinals,slots))
    }
    pub(crate) fn new(opening: Opening, authority: VerifiedOpening<'_>) -> Result<Self, Error> {
        let hash = opening.fingerprint()?;
        if opening.version != 1 || opening.stream_epoch == 0 || opening.population_origin == [0;32]
            || authority.registration_origin != opening.registration_origin()? || authority.input_hash != hash || authority.identity != opening.identity
            || authority.population_origin != opening.population_origin || authority.total_people == 0
            || authority.offers.len() > 64 || authority.qualifications.len() > MAX_COHORTS || authority.free_contracts.len() > MAX_COHORTS
            || [opening.identity.world,opening.identity.branch,opening.identity.scope].contains(&0)
            || opening.courses.iter().all(Option::is_none) { return Err(Error::Invalid); }
        let mut registered_offers = [None;MAX_COHORTS];
        for (i,registration) in opening.courses.iter().enumerate() {
            let Some(r) = registration else {continue}; let en = r.enrollment;
            if en.started != GameTime::ZERO || en.cohort == 0 || en.generation == 0
                || [en.roster_ref,en.class,en.school,en.fee_contract_ref,en.obligations_ref,r.course.reference,r.use_id].contains(&0)
                || !r.learners.valid(authority.total_people) || !r.teacher.valid(authority.total_people)
                || r.teacher.end-r.teacher.start != 1 || r.learners.overlaps(r.teacher)
                || r.learners.end-r.learners.start != u64::from(en.learners)
                || en.learners == 0 || en.learners > r.course.seats()
                || r.course.cents_per_hour.value() != 0
                || opening.courses[..i].iter().flatten().any(|old| old.enrollment.cohort == en.cohort)
            { return Err(Error::Invalid); }
            let q = r.qualification; let f = r.free_contract;
            if q.source_ref == 0 || q.course_ref != r.course.reference || q.teacher != r.teacher
                || q.valid_from > en.started || q.valid_until <= q.valid_from
                || authority.qualifications.iter().filter(|row| **row == q).count() != 1
                || f.source_ref == 0 || f.contract_ref != en.fee_contract_ref
                || f.course_ref != r.course.reference || f.roster_ref != en.roster_ref
                || authority.free_contracts.iter().filter(|row| **row == f).count() != 1 { return Err(Error::Unknown); }
            let mut offers = authority.offers.iter().filter(|offer| offer.use_id == r.use_id);
            let offer = offers.next().ok_or(Error::Unknown)?;
            if offers.next().is_some() || offer.resource_id == 0 || offer.venue_id != en.school || offer.group_id != en.class
                || offer.learners != u32::from(en.learners) || offer.teachers != 1 || offer.staff != 0 || offer.visitors != 0
                || offer.enrollment < u32::from(en.learners) || !offer.members.valid(authority.total_people)
                || offer.members.start != r.learners.start.min(r.teacher.start)
                || offer.members.end != r.learners.end.max(r.teacher.end)
                || offer.members.end-offer.members.start != u64::from(en.learners)+1
                || offer.window.start >= offer.window.end { return Err(Error::Invalid); }
            registered_offers[i] = Some(*offer);
        }
        Ok(Self {strategy_version:RUNTIME_STRATEGY,opening,opening_hash:hash,registration_origin:authority.registration_origin,total_people:authority.total_people,offers:registered_offers,
            prefix:Prefix {states:[State::EMPTY;MAX_COHORTS],through:0,last_source:None,anchor:None,open_records:[None;MAX_COHORTS]},
            states:[State::EMPTY;MAX_COHORTS],records:[None;MAX_ACTIVITIES],consumed_sequence:0,last_source:None,
            recent:[None;MAX_RECENT_EVENTS],ready:true})
    }
    pub(crate) fn fingerprint(&self) -> Result<[u8;32], Error> { fingerprint(b"education-runtime-v2",self) }
    pub(crate) fn progress(&self, cohort: u64) -> Result<Option<e::Progress>, Error> {
        let slot = self.slot(cohort)?; Ok(self.states[slot].cohort.map(|c| c.progress))
    }
    pub(crate) fn consumed_sequence(&self) -> u64 { self.consumed_sequence }
    pub(crate) fn next_sequence(&self) -> Result<u64, Error> { self.consumed_sequence.checked_add(1).ok_or(Error::Limit) }
    pub(crate) fn last_source(&self) -> Option<Origin> { self.last_source }
    /// No reverse inference from totals. Compacted details must resolve from original
    /// Store receipts by canonical epoch/sequence; absent history is Unknown.
    pub(crate) fn recent_event(&self, epoch: u64, sequence: u64) -> Result<(Command,Origin), Error> {
        if epoch != self.opening.stream_epoch { return Err(Error::Unknown); }
        self.recent.iter().flatten().find(|input| input.source.stream_sequence == sequence)
            .map(|input| (input.command,input.source)).ok_or(Error::Unknown)
    }
    pub(crate) fn opening(&self) -> &Opening { &self.opening }
    /// Projection keys resolve to the root ledger's own activity IDs by original begin
    /// operation/source. These are not extra authoritative activity rows or new IDs.
    pub(crate) fn pending_claims(&self) -> Result<[Option<(SharedActivity,Option<SharedActivity>)>;MAX_COHORTS], Error> {
        let mut result = [None;MAX_COHORTS];
        for (slot,state) in self.states.iter().enumerate() {
            if let Some(index) = state.pending { result[slot] = Some(self.claims(self.records[usize::from(index)].ok_or(Error::Invalid)?)?); }
        }
        Ok(result)
    }
    pub(crate) fn pending(&self, cohort: u64) -> Result<bool, Error> { Ok(self.states[self.slot(cohort)?].pending.is_some()) }
    pub(crate) fn awaiting_settlement(&self, cohort: u64) -> Result<bool, Error> { Ok(self.states[self.slot(cohort)?].awaiting.is_some()) }
    pub(crate) fn stopped_activity(&self, cohort: u64) -> Result<Option<StoppedActivity>, Error> {
        let Some(index)=self.states[self.slot(cohort)?].awaiting else {return Ok(None)};
        let row=self.records[usize::from(index)].ok_or(Error::Invalid)?;
        Ok(Some(StoppedActivity {begin:row.began,kind:if row.kind==Episode::Preparation {ActivityKind::Preparation} else {ActivityKind::Teaching},
            original_start:row.start,consumed_through:row.settled_through,known_actual_end:row.known_actual_end,cutoff:row.cutoff.ok_or(Error::Invalid)?}))
    }
    pub(crate) fn retained_activities(&self) -> usize { self.records.iter().flatten().count() }
    fn slot(&self, cohort: u64) -> Result<usize, Error> {
        self.opening.courses.iter().position(|r| r.is_some_and(|r| r.enrollment.cohort == cohort)).ok_or(Error::Invalid)
    }
    fn registration(&self, slot: usize) -> Result<Registration, Error> { self.opening.courses[slot].ok_or(Error::Invalid) }
    fn key(&self, record: Record, role: Role) -> ClaimKey {
        ClaimKey {epoch:self.opening.stream_epoch,sequence:record.began.stream_sequence,cohort:record.cohort,role}
    }
    fn claims(&self, record: Record) -> Result<(SharedActivity,Option<SharedActivity>), Error> {
        let r = self.registration(self.slot(record.cohort)?)?;
        let teacher = SharedActivity {key:self.key(record,Role::Teacher),members:r.teacher,
            kind:if record.kind == Episode::Preparation {ActivityKind::Preparation} else {ActivityKind::Teaching},
            start:record.start,settled_through:if record.cutoff.is_some() {record.end.ok_or(Error::Invalid)?} else {record.settled_through},end:record.end,source:record.began};
        let learners = if record.kind == Episode::Lesson { Some(SharedActivity {key:self.key(record,Role::Learners),
            members:r.learners,kind:ActivityKind::Learning,start:record.start,settled_through:teacher.settled_through,
            end:record.end,source:record.began}) } else {None};
        Ok((teacher,learners))
    }
    fn validate_pending_ledger(&self, ledger: &[SharedActivity]) -> Result<(), Error> {
        if ledger.len() > MAX_SHARED_SCAN { return Err(Error::Limit); }
        for (i,row) in ledger.iter().enumerate() {
            if !row.members.valid(self.total_people) || row.settled_through < row.start
                || row.end.is_some_and(|end| end < row.start || row.settled_through != end)
                || ledger[..i].iter().any(|old| old.key == row.key) {
                return Err(Error::Unknown);
            }
        }
        for record in self.records.iter().flatten().filter(|row| row.end.is_none()) {
            let (teacher,learners) = self.claims(*record)?;
            for expected in [Some(teacher),learners].into_iter().flatten() {
                if ledger.iter().filter(|row| **row == expected).count() != 1 { return Err(Error::Unknown); }
            }
        }
        Ok(())
    }
    fn validate_venue(&self, slot: usize, w: e::Window, source: Origin, coverage: Option<VenueCoverage>) -> Result<(), Error> {
        let r = self.registration(slot)?; let v = coverage.ok_or(Error::Unknown)?;
        let offer = self.offers[slot].ok_or(Error::Unknown)?;
        if v.root != source.root || v.use_id != r.use_id || v.venue_id != r.enrollment.school
            || v.group_id != r.enrollment.class || v.resource_id != offer.resource_id || v.token_number == 0
            || v.offer_window != offer.window
            || v.members.source != r.learners.source || v.members.version != r.learners.version
            || v.members.start != r.learners.start.min(r.teacher.start) || v.members.end != r.learners.end.max(r.teacher.end)
            || v.exited_at.is_some() || v.entered_at > w.start || v.offer_window.start > w.start
            || v.offer_window.end < w.end || w.end > source.occurred_at
            || r.qualification.valid_from > w.start || r.qualification.valid_until < w.end { return Err(Error::Unknown); }
        Ok(())
    }
    /// Point validation creates an ongoing activity, never a future observed second.
    fn validate_start_venue(&self, slot: usize, at: GameTime, source: Origin, coverage: Option<VenueCoverage>) -> Result<(), Error> {
        let r = self.registration(slot)?; let v = coverage.ok_or(Error::Unknown)?;
        let offer = self.offers[slot].ok_or(Error::Unknown)?;
        if v.root != source.root || v.use_id != r.use_id || v.venue_id != r.enrollment.school
            || v.group_id != r.enrollment.class || v.resource_id != offer.resource_id || v.token_number == 0
            || v.offer_window != offer.window
            || v.members.source != r.learners.source || v.members.version != r.learners.version
            || v.members.start != r.learners.start.min(r.teacher.start) || v.members.end != r.learners.end.max(r.teacher.end)
            || v.exited_at.is_some() || v.entered_at > at || v.offer_window.start > at || v.offer_window.end <= at
            || r.qualification.valid_from > at || r.qualification.valid_until <= at { return Err(Error::Unknown); }
        Ok(())
    }
    pub(crate) fn plan(&self, snapshot: Snapshot<'_>) -> Result<Candidate, Error> {
        if !self.ready || self.strategy_version != RUNTIME_STRATEGY
            || snapshot.opening_hash != self.opening_hash || snapshot.population_origin != self.opening.population_origin {
            return Err(Error::Unknown);
        }
        let _coverage = snapshot.coverage; // Explicitly education-only, not full payroll/travel acceptance.
        self.validate_pending_ledger(snapshot.activities)?;
        let venue_token = if matches!(snapshot.event.command.action,Action::BeginPreparation | Action::BeginLesson) {
            Some(snapshot.venue.ok_or(Error::Unknown)?.token_number)
        } else {None};
        let candidate = self.stage(snapshot.event.command,snapshot.event.source,venue_token,snapshot.stopped)?;
        let record = candidate.record;
        if matches!(record.kind,Episode::Preparation | Episode::Lesson) {
            if matches!(snapshot.event.command.action,Action::BeginPreparation | Action::BeginLesson) {
                self.validate_start_venue(candidate.state_slot,record.start,candidate.source,snapshot.venue)?;
            } else {
                let from = self.records[candidate.record_slot].ok_or(Error::Invalid)?.settled_through;
                if matches!(snapshot.event.command.action,Action::SettleAfterExit {..} | Action::StopForExit {..}) {
                    let stop = snapshot.stopped.ok_or(Error::Unknown)?;
                    let v = snapshot.venue.ok_or(Error::Unknown)?;
                    let offer = self.offers[candidate.state_slot].ok_or(Error::Unknown)?;
                    if v.root != candidate.source.root || v.use_id != stop.use_id || v.use_id != offer.use_id
                        || v.venue_id != offer.venue_id || v.group_id != offer.group_id || v.resource_id != offer.resource_id
                        || v.token_number != stop.token_number || record.token_number != Some(v.token_number)
                        || v.exited_at != Some(stop.exited_at) || v.members != offer.members || v.offer_window != offer.window
                        || v.entered_at > record.start { return Err(Error::Unknown); }
                    let actual_through = match snapshot.event.command.action {
                        Action::SettleAfterExit {actual_through,..} => actual_through,
                        _ => from, // stopping itself NEVER infers historical teaching
                    };
                    if from < actual_through {
                        let mut prefix_venue = v; prefix_venue.exited_at = None;
                        self.validate_venue(candidate.state_slot,window(from,actual_through)?,candidate.source,Some(prefix_venue))?;
                    }
                } else if from < candidate.source.occurred_at {
                    if record.token_number != snapshot.venue.map(|v| v.token_number) { return Err(Error::Unknown); }
                    self.validate_venue(candidate.state_slot,window(from,candidate.source.occurred_at)?,candidate.source,snapshot.venue)?;
                }
            }
            let (teacher,learners) = self.claims(record)?;
            for claim in [Some(teacher),learners].into_iter().flatten() {
                for old in snapshot.activities {
                    if old.key != claim.key && claim.members.overlaps(old.members)
                        && overlap(claim.start,claim.end,old.start,old.end) { return Err(Error::Conflict); }
                }
            }
        }
        Ok(candidate)
    }
    fn stage(&self, command: Command, source: Origin, venue_token: Option<u64>, stopped: Option<StoppedCoverage>) -> Result<Candidate, Error> {
        if source.identity != self.opening.identity || source.root == [0;32]
            || source.operation_id == 0 || source.event_id == 0 || source.stream_epoch != self.opening.stream_epoch {
            return Err(Error::Invalid);
        }
        if source.stream_sequence <= self.consumed_sequence { return Err(Error::Duplicate); }
        if source.stream_sequence != self.consumed_sequence.checked_add(1).ok_or(Error::Limit)? { return Err(Error::Cursor); }
        if self.last_source.is_some_and(|old| source.occurred_at < old.occurred_at || source.revision < old.revision) {
            return Err(Error::Clock);
        }
        // Canonical identity is epoch/sequence. Root binds each admitted sub-command
        // to its original receipt. A SafeExit may fan out to several cohorts in ONE op;
        // op/event numbers are not a second ordered stream or semantic-fact detector.
        let state_slot = self.slot(command.cohort)?;
        let r = self.registration(state_slot)?;
        let mut state = self.states[state_slot];
        let beginning = matches!(command.action,Action::BeginPreparation | Action::BeginLesson);
        if beginning != venue_token.is_some() || venue_token == Some(0) { return Err(Error::Unknown); }
        let recent_count = self.recent.iter().flatten().count();
        let recent_slot = self.recent.iter().position(Option::is_none).ok_or(Error::NeedsCompaction)?;
        let mut action = command.action;
        let mut actual_end = source.occurred_at;
        let originally_active = state.pending.is_some();
        if let Action::SettleAfterExit {started_event_id,exited_at,exit_operation_id,actual_through,finalize} = action {
            let stop = stopped.ok_or(Error::Unknown)?;
            if state.pending.is_some() {return Err(Error::Pending);} // root must cut/release at SafeExit first
            let pending = state.awaiting.ok_or(Error::Pending)?;
            let row = self.records[usize::from(pending)].ok_or(Error::Pending)?;
            if stop.identity != source.identity || stop.root != source.root || stop.use_id != r.use_id
                || Some(stop.token_number) != row.token_number || stop.token_number == 0
                || stop.exited_at != exited_at || stop.exit_operation_id != exit_operation_id || exit_operation_id == 0
                || stop.exit_event_id == 0 || stop.exit_revision > source.revision
                || actual_through < row.settled_through || actual_through > exited_at || exited_at > source.occurred_at
                || (finalize && (stop.no_instruction.start != actual_through || stop.no_instruction.end != source.occurred_at)) {
                return Err(Error::Unknown);
            }
            if let Some(original) = row.cutoff {
                if original.exited_at != exited_at || original.exit_operation_id != exit_operation_id
                    || original.exit_event_id != stop.exit_event_id || original.exit_revision != stop.exit_revision {
                    return Err(Error::Unknown);
                }
            }
            actual_end = actual_through;
            if actual_through > row.settled_through {
                let actual=stop.actual.ok_or(Error::Unknown)?;
                if actual.identity != source.identity || actual.root != source.root || actual.cohort != command.cohort
                    || actual.generation != r.enrollment.generation || actual.original_begin_event_id != row.began.event_id
                    || actual.kind != (if row.kind==Episode::Preparation {ActivityKind::Preparation} else {ActivityKind::Teaching})
                    || actual.window.start > row.settled_through || actual.window.end < actual_through
                    || actual.window.start >= actual.window.end || actual.window.end > exited_at
                    || actual.source_operation_id==0 || actual.source_event_id==0 || actual.source_revision>source.revision
                    || actual.observed_at<actual.window.end || actual.observed_at>source.occurred_at {return Err(Error::Unknown);}
            }
            state.pending = Some(pending); // reuse settlement code; no new shared claim
            action = match row.kind {
                Episode::Preparation => Action::SettlePreparation {started_event_id,close:finalize},
                Episode::Lesson => Action::SettleLesson {started_event_id,close:finalize},
                _ => return Err(Error::Invalid),
            };
        } else if stopped.is_some() && !matches!(action,Action::StopForExit {..}) { return Err(Error::Invalid); }
        let mut delta = None;
        let record_slot;
        let record;
        match action {
            Action::ActivateNoInstruction => {
                if state.cohort.is_some() || state.pending.is_some() || state.awaiting.is_some() { return Err(Error::Invalid); }
                let first_end = GameTime::new(r.enrollment.started.value().checked_add(1).ok_or(Error::Limit)?).map_err(|_| Error::Limit)?;
                if source.occurred_at < first_end { return Err(Error::Clock); }
                let first = window(r.enrollment.started,first_end)?;
                let binding = e::enrollment_binding(r.enrollment,r.course);
                let c = Cohort::begin(view(source),r.enrollment,r.course,
                    fact(source,first,e::Kind::Curriculum,r.course.reference,binding),
                    fact(source,first,e::Kind::Roster,r.enrollment.roster_ref,binding))?;
                let mut input = e::NoInstruction {view:view(source),window:window(r.enrollment.started,source.occurred_at)?,sequence:1,proof:None};
                input.proof = fact(source,input.window,e::Kind::NoInstruction,command.cohort,e::no_instruction_binding(&c,&input));
                state.cohort = Some(e::advance_no_instruction(&c,input)?.next.checkpoint());
                record_slot = self.records.iter().position(Option::is_none).ok_or(Error::Limit)?;
                record = Record {cohort:command.cohort,kind:Episode::Activation,start:r.enrollment.started,
                    end:Some(source.occurred_at),began:source,ended:None,settled_through:source.occurred_at,last_settlement:None,token_number:None,cutoff:None,known_actual_end:r.enrollment.started};
            }
            Action::NoInstruction {window:w} => {
                if state.pending.is_some() || state.awaiting.is_some() || w.end != source.occurred_at { return Err(Error::Pending); }
                let c = Cohort::from_checkpoint(state.cohort.ok_or(Error::Pending)?)?;
                let mut input = e::NoInstruction {view:view(source),window:w,sequence:c.sequence().checked_add(1).ok_or(Error::Limit)?,proof:None};
                input.proof = fact(source,w,e::Kind::NoInstruction,command.cohort,e::no_instruction_binding(&c,&input));
                state.cohort = Some(e::advance_no_instruction(&c,input)?.next.checkpoint());
                record_slot = self.records.iter().position(Option::is_none).ok_or(Error::Limit)?;
                record = Record {cohort:command.cohort,kind:Episode::NoInstruction,start:w.start,end:Some(w.end),began:source,ended:None,
                    settled_through:w.end,last_settlement:None,token_number:None,cutoff:None,known_actual_end:w.start};
            }
            Action::BeginPreparation | Action::BeginLesson => {
                if state.pending.is_some() || state.awaiting.is_some() { return Err(Error::Pending); }
                let c = Cohort::from_checkpoint(state.cohort.ok_or(Error::Pending)?)?;
                if c.cursor() != source.occurred_at || c.status() != e::Status::Active { return Err(Error::Cursor); }
                let kind = if command.action == Action::BeginPreparation {Episode::Preparation} else {Episode::Lesson};
                if kind == Episode::Lesson && (state.prepared_day != source.occurred_at.value()/86400 || state.prepared_seconds < 7200) {
                    return Err(Error::Unknown);
                }
                let checkpoint = c.checkpoint();
                if kind == Episode::Lesson && ((checkpoint.day == source.occurred_at.value()/86400
                    && checkpoint.day_seconds >= r.course.daily_seconds())
                    || (checkpoint.day != source.occurred_at.value()/86400 && checkpoint.progress.actual_teaching_days == u32::MAX)) {
                    return Err(Error::Boundary);
                }
                record_slot = self.records.iter().position(Option::is_none).ok_or(Error::Limit)?;
                record = Record {cohort:command.cohort,kind,start:source.occurred_at,end:None,began:source,ended:None,
                    settled_through:source.occurred_at,last_settlement:None,token_number:venue_token,cutoff:None,known_actual_end:source.occurred_at};
                let (teacher,learners) = self.claims(record)?;
                // Local pending responsibility is checked as well as the ONE shared ledger.
                for old in self.records.iter().flatten().filter(|row| row.end.is_none()) {
                    let (a,b) = self.claims(*old)?;
                    for claim in [Some(teacher),learners].into_iter().flatten() {
                        for other in [Some(a),b].into_iter().flatten() {
                            if claim.members.overlaps(other.members) { return Err(Error::Conflict); }
                        }
                    }
                }
                state.pending = Some(record_slot as u8);
                delta = Some(ActivityDelta::Acquire {teacher,learners});
            }
            Action::SettlePreparation {started_event_id,close} | Action::SettleLesson {started_event_id,close} => {
                record_slot = usize::from(state.pending.ok_or(Error::Pending)?);
                let mut row = self.records[record_slot].ok_or(Error::Invalid)?;
                let expected = if matches!(action,Action::SettlePreparation {..}) {Episode::Preparation} else {Episode::Lesson};
                if row.cohort != command.cohort || row.kind != expected
                    || (row.end.is_some() && !matches!(command.action,Action::SettleAfterExit {..})) || row.began.event_id != started_event_id {
                    return Err(Error::Invalid);
                }
                let from = row.settled_through;
                if actual_end < from || (!close && actual_end == from)
                    || (actual_end.value()-from.value()) as u64 > HISTORICAL_BATCH_SECONDS { return Err(Error::Boundary); }
                let historical=matches!(command.action,Action::SettleAfterExit {..});
                if historical {
                    if let Some(actual)=stopped.and_then(|s| s.actual) {row.known_actual_end=row.known_actual_end.max(actual.window.end);}
                    if close && actual_end<row.known_actual_end {return Err(Error::Boundary);}
                } else {row.known_actual_end=row.known_actual_end.max(actual_end);}
                let mut c = Cohort::from_checkpoint(state.cohort.ok_or(Error::Pending)?)?;
                if c.cursor() != from { return Err(Error::Cursor); }
                let mut cursor = from;
                // At most two calendar pieces for an actual <=1-day settlement. Never
                // release/restart the original activity merely to cross a day boundary.
                while cursor < actual_end {
                    let next_day = (cursor.value()/86400+1).checked_mul(86400).unwrap_or(i64::MAX);
                    let end = GameTime::new(next_day.min(actual_end.value())).map_err(|_| Error::Limit)?;
                    let w = window(cursor,end)?;
                    if expected == Episode::Preparation {
                        let mut input = e::NoInstruction {view:view(source),window:w,sequence:c.sequence().checked_add(1).ok_or(Error::Limit)?,proof:None};
                        input.proof = fact(source,w,e::Kind::NoInstruction,command.cohort,e::no_instruction_binding(&c,&input));
                        c = e::advance_no_instruction(&c,input)?.next;
                        let day = w.start.value()/86400;
                        let previous = if state.prepared_day == day {state.prepared_seconds} else {0};
                        state.prepared_seconds = previous.checked_add((w.end.value()-w.start.value()) as u64).ok_or(Error::Limit)?;
                        if state.prepared_seconds > 86400 { return Err(Error::Boundary); }
                        state.prepared_day = day;
                    } else {
                    if state.prepared_day != w.start.value()/86400 || state.prepared_seconds < 7200 { return Err(Error::Unknown); }
                    let teacher_id = r.teacher.start.checked_add(1).ok_or(Error::Limit)?;
                    let mut input = e::Attendance {view:view(source),window:w,sequence:c.sequence().checked_add(1).ok_or(Error::Limit)?,teacher:teacher_id,remote:false,
                        school_open:None,reservation:None,teacher_qualified:None,teacher_present:None,teacher_exclusive:None,
                        preparation:None,actual_attendance:None,learner_exclusive:None,network:None,fee_service_authorized:None};
                    let binding = e::attendance_binding(&c,&input);
                    input.school_open = fact(source,w,e::Kind::SchoolOpen,r.enrollment.school,binding);
                    input.reservation = fact(source,w,e::Kind::CapacityReservation,r.enrollment.class,binding);
                    input.teacher_qualified = fact(source,w,e::Kind::TeacherQualified,teacher_id,binding);
                    input.teacher_present = fact(source,w,e::Kind::TeacherPresent,teacher_id,binding);
                    input.teacher_exclusive = fact(source,w,e::Kind::TeacherExclusive,teacher_id,binding);
                    input.preparation = fact(source,w,e::Kind::Preparation,teacher_id,binding);
                    input.actual_attendance = fact(source,w,e::Kind::ActualAttendance,r.enrollment.roster_ref,binding);
                    input.learner_exclusive = fact(source,w,e::Kind::LearnerExclusive,r.enrollment.roster_ref,binding);
                    input.fee_service_authorized = fact(source,w,e::Kind::FeeServiceAuthorized,r.enrollment.fee_contract_ref,binding);
                    c = e::advance(&c,input)?.next;
                    }
                    cursor = end;
                }
                if close && actual_end < source.occurred_at {
                    // Only the explicit authenticated stop command can reach this suffix.
                    let w = window(actual_end,source.occurred_at)?;
                    let mut input = e::NoInstruction {view:view(source),window:w,sequence:c.sequence().checked_add(1).ok_or(Error::Limit)?,proof:None};
                    input.proof = fact(source,w,e::Kind::NoInstruction,command.cohort,e::no_instruction_binding(&c,&input));
                    c = e::advance_no_instruction(&c,input)?.next;
                }
                state.cohort = Some(c.checkpoint());
                row.settled_through = if historical && !close {actual_end} else {source.occurred_at}; row.last_settlement = Some(source);
                if close {
                    if row.cutoff.is_none() {row.end = Some(source.occurred_at); row.ended = Some(source);}
                    state.pending = None; state.awaiting = None;
                } else if historical {
                    state.pending=None; state.awaiting=Some(record_slot as u8);
                }
                record = row;
                delta = Some(ActivityDelta::Settle {teacher:self.key(row,Role::Teacher),
                    learners:if expected == Episode::Lesson {Some(self.key(row,Role::Learners))} else {None},from,through:source.occurred_at,close});
                if !originally_active {delta=None;} // SafeExit already closed the shared claim
            }
            Action::StopForExit {started_event_id,exited_at,exit_operation_id} => {
                record_slot=usize::from(state.pending.ok_or(Error::Pending)?);
                if state.awaiting.is_some() {return Err(Error::Pending);}
                let mut row=self.records[record_slot].ok_or(Error::Invalid)?;
                let stop=stopped.ok_or(Error::Unknown)?;
                // Stop is a cutoff-only protocol. Do not accept, ignore or persist
                // actual history here; that evidence belongs to SettleAfterExit.
                if stop.actual.is_some() {return Err(Error::Invalid);}
                if row.began.event_id != started_event_id || row.end.is_some() || row.cutoff.is_some()
                    || stop.identity != source.identity || stop.root != source.root || stop.use_id != r.use_id
                    || row.token_number != Some(stop.token_number) || stop.token_number==0
                    || stop.exited_at != exited_at || exited_at != source.occurred_at || exited_at < row.settled_through
                    || stop.exit_operation_id != exit_operation_id || exit_operation_id != source.operation_id
                    || stop.exit_event_id != source.event_id || stop.exit_revision != source.revision
                    || stop.no_instruction.start != exited_at || stop.no_instruction.end != exited_at {return Err(Error::Unknown);}
                row.end=Some(exited_at); row.ended=Some(source); row.cutoff=Some(stop);
                state.awaiting=state.pending; state.pending=None; record=row;
                delta=Some(ActivityDelta::Settle {teacher:self.key(row,Role::Teacher),
                    learners:if row.kind==Episode::Lesson {Some(self.key(row,Role::Learners))} else {None},
                    from:row.settled_through,through:exited_at,close:true});
            }
            Action::SettleAfterExit {..} => return Err(Error::Invalid), // normalized above
        }
        let candidate=Candidate {expected_hash:self.fingerprint()?,source,state_slot,record_slot,state,record,activity_delta:delta,
            input:HostInput {command,source,venue_token,stopped},recent_slot};
        let (reserved,slots)=self.responsibility_reserve(Some(&candidate))?;
        if u64::MAX-source.stream_sequence<reserved {return Err(Error::Limit);}
        if recent_count+1+slots>MAX_RECENT_EVENTS {return Err(Error::NeedsCompaction);}
        Ok(candidate)
    }
    /// Root calls this only in its prepared transaction image, together with activity_delta.
    /// Exclusive-borrow publication; prospective encoding failure restores every row
    /// before returning. No callback or original-world commit occurs inside this method.
    pub(crate) fn apply_candidate(&mut self, candidate: Candidate) -> Result<(), Error> {
        if !self.ready || candidate.expected_hash != self.fingerprint()?
            || candidate.source.stream_sequence != self.consumed_sequence.checked_add(1).ok_or(Error::Limit)? {
            return Err(Error::Cursor);
        }
        let old_state = self.states[candidate.state_slot];
        let old_record = self.records[candidate.record_slot];
        let old_input = self.recent[candidate.recent_slot];
        let old_sequence = self.consumed_sequence;
        let old_source = self.last_source;
        self.states[candidate.state_slot] = candidate.state;
        self.records[candidate.record_slot] = Some(candidate.record);
        self.recent[candidate.recent_slot] = Some(candidate.input);
        self.consumed_sequence = candidate.source.stream_sequence;
        self.last_source = Some(candidate.source);
        if let Err(error) = self.fingerprint() {
            self.states[candidate.state_slot] = old_state;
            self.records[candidate.record_slot] = old_record;
            self.recent[candidate.recent_slot] = old_input;
            self.consumed_sequence = old_sequence;
            self.last_source = old_source;
            return Err(error);
        }
        Ok(())
    }
    /// Explicit stable-prefix barrier, NOT automatic history retirement. Original Store
    /// keeps exact source receipts; closed local rows become a checkpoint plus watermark.
    pub(crate) fn compact_prefix(&mut self, authority: VerifiedCompaction, ledger: &[SharedActivity]) -> Result<(), Error> {
        if !self.ready || authority.state_hash != self.fingerprint()? || authority.anchor.store_ref == 0
            || authority.anchor.root == [0;32] || authority.anchor.epoch != self.opening.stream_epoch
            || authority.anchor.through != self.consumed_sequence || self.consumed_sequence == self.prefix.through {
            return Err(Error::Invalid);
        }
        if self.last_source.is_none_or(|last| last.operation_id != authority.anchor.store_ref) {return Err(Error::Invalid);}
        if ledger.len() > MAX_SHARED_SCAN { return Err(Error::Limit); }
        self.validate_pending_ledger(ledger)?;
        let mut open_records = [None;MAX_COHORTS];
        let mut retained = [None;MAX_ACTIVITIES];
        for (slot,state) in self.states.iter().enumerate() {
            if state.pending.is_some() && state.awaiting.is_some() {return Err(Error::Invalid);}
            if let Some(index) = state.pending.or(state.awaiting) {
                let row = self.records[usize::from(index)].ok_or(Error::Invalid)?;
                if (state.pending.is_some() && row.end.is_some()) || (state.awaiting.is_some() && row.cutoff.is_none()) {return Err(Error::Invalid);}
                // Preserve ENTIRE active OR stopped-but-unsettled rows and their sources.
                // Only completed/consumed events and closed activity history are retired.
                open_records[slot] = Some((index,row)); retained[usize::from(index)] = Some(row);
            }
        }
        for row in self.records.iter().flatten().filter(|row| row.end.is_some() && matches!(row.kind,Episode::Preparation | Episode::Lesson)) {
            let (teacher,learners) = self.claims(*row)?;
            for claim in [Some(teacher),learners].into_iter().flatten() {
                // Root's active-only ledger normally removed this closed claim in the
                // SAME committed settlement. A retained history projection must match.
                if ledger.iter().any(|old| old.key == claim.key && *old != claim) { return Err(Error::Unknown); }
            }
        }
        // No fallible work after the prefix copy begins. Root still owns atomic persistence.
        self.prefix = Prefix {states:self.states,through:self.consumed_sequence,last_source:self.last_source,
            anchor:Some(authority.anchor),open_records};
        self.records = retained;
        self.recent = [None;MAX_RECENT_EVENTS];
        Ok(())
    }
    /// Deserialized Runtime is unready. Root authenticates Store root/receipts first;
    /// replay then checks recent state against the stable prefix and pending shared claims.
    pub(crate) fn restore(mut self, authority: VerifiedRestore, ledger: &[SharedActivity]) -> Result<Self, Error> {
        if self.strategy_version != RUNTIME_STRATEGY || authority.state_hash != self.fingerprint()? || authority.opening_hash != self.opening_hash
            || self.opening.fingerprint()? != self.opening_hash || authority.anchor != self.prefix.anchor
            || authority.registration_origin != self.registration_origin || self.registration_origin != self.opening.registration_origin()?
            || self.prefix.through > self.consumed_sequence { return Err(Error::Restore); }
        if self.prefix.through == 0 {
            if self.prefix.states != [State::EMPTY;MAX_COHORTS] || self.prefix.anchor.is_some()
                || self.prefix.last_source.is_some() || self.prefix.open_records != [None;MAX_COHORTS] {
                return Err(Error::Restore);
            }
        } else {
            let anchor = self.prefix.anchor.ok_or(Error::Restore)?;
            let last = self.prefix.last_source.ok_or(Error::Restore)?;
            if anchor.epoch != self.opening.stream_epoch || anchor.through != self.prefix.through || anchor.store_ref == 0
                || anchor.root == [0;32] || anchor.store_ref != last.operation_id
                || last.stream_sequence != self.prefix.through || last.stream_epoch != anchor.epoch {
                return Err(Error::Restore);
            }
        }
        for (slot,state) in self.prefix.states.iter().enumerate() {
            if let Some(checkpoint) = state.cohort {
                let c = Cohort::from_checkpoint(checkpoint)?; let r = self.registration(slot)?;
                if c.enrollment() != r.enrollment || checkpoint.course != r.course || checkpoint.identity != self.opening.identity
                    || state.prepared_day < -1 || state.prepared_seconds > 86400
                    || (state.prepared_day == -1) != (state.prepared_seconds == 0)
                    || state.prepared_day > (checkpoint.cursor.value()-1)/86400 { return Err(Error::Restore); }
                if state.pending.is_some() && state.awaiting.is_some() {return Err(Error::Restore);}
            } else if *state != State::EMPTY { return Err(Error::Restore); }
        }
        let mut prefix_records = [None;MAX_ACTIVITIES];
        for (slot,entry) in self.prefix.open_records.iter().enumerate() {
            if let Some((index,row)) = entry {
                let i = usize::from(*index);
                let state=self.prefix.states[slot];
                if i >= MAX_ACTIVITIES || prefix_records[i].is_some() || state.pending.or(state.awaiting) != Some(*index)
                    || !matches!(row.kind,Episode::Preparation | Episode::Lesson)
                    || row.cohort != self.registration(slot)?.enrollment.cohort || row.began.identity != self.opening.identity
                    || row.began.stream_epoch != self.opening.stream_epoch || row.began.stream_sequence == 0
                    || row.began.stream_sequence > self.prefix.through || row.start != row.began.occurred_at
                    || row.start > row.settled_through || row.token_number.is_none_or(|token| token == 0)
                    || self.prefix.states[slot].cohort.ok_or(Error::Restore)?.cursor != row.settled_through {
                    return Err(Error::Restore);
                }
                if state.pending.is_some() {
                    if row.end.is_some() || row.ended.is_some() || row.cutoff.is_some()
                        || row.known_actual_end != row.settled_through {return Err(Error::Restore);}
                } else {
                    let stop=row.cutoff.ok_or(Error::Restore)?; let ended=row.ended.ok_or(Error::Restore)?;
                    if stop.actual.is_some() || row.end != Some(stop.exited_at) || row.settled_through > stop.exited_at
                        || row.known_actual_end < row.settled_through || row.known_actual_end > stop.exited_at
                        || stop.identity != self.opening.identity || stop.root != ended.root || stop.token_number != row.token_number.ok_or(Error::Restore)?
                        || stop.exit_operation_id != ended.operation_id || stop.exit_event_id != ended.event_id || stop.exit_revision != ended.revision
                        || stop.exited_at != ended.occurred_at || ended.stream_epoch != self.opening.stream_epoch
                        || ended.stream_sequence > self.prefix.through || ended.stream_sequence <= row.began.stream_sequence {
                        return Err(Error::Restore);
                    }
                }
                if let Some(last) = row.last_settlement {
                    if last.stream_sequence > self.prefix.through || last.stream_sequence <= row.began.stream_sequence
                        || last.occurred_at < row.settled_through
                        || (state.pending.is_some() && last.occurred_at != row.settled_through)
                        || last.stream_epoch != self.opening.stream_epoch {
                        return Err(Error::Restore);
                    }
                } else if row.settled_through != row.start { return Err(Error::Restore); }
                prefix_records[i] = Some(*row);
            } else if self.prefix.states[slot].pending.is_some() || self.prefix.states[slot].awaiting.is_some() { return Err(Error::Restore); }
        }
        // At most 32 admitted recent host events. Older exact history requires Store.
        let events = self.consumed_sequence.checked_sub(self.prefix.through).ok_or(Error::Restore)?;
        let (reserved,slots)=self.responsibility_reserve(None).map_err(|_| Error::Restore)?;
        if events > MAX_RECENT_EVENTS as u64 || events as usize+slots > MAX_RECENT_EVENTS
            || u64::MAX-self.consumed_sequence < reserved { return Err(Error::Restore); }
        let mut replay = Self {strategy_version:RUNTIME_STRATEGY,opening:self.opening.clone(),opening_hash:self.opening_hash,registration_origin:self.registration_origin,
            offers:self.offers,total_people:self.total_people,
            prefix:self.prefix.clone(),states:self.prefix.states,records:prefix_records,
            consumed_sequence:self.prefix.through,last_source:self.prefix.last_source,recent:[None;MAX_RECENT_EVENTS],ready:true};
        let (prefix_reserved,prefix_slots)=replay.responsibility_reserve(None).map_err(|_| Error::Restore)?;
        if u64::MAX-self.prefix.through<prefix_reserved || prefix_slots>MAX_RECENT_EVENTS {return Err(Error::Restore);}
        if self.recent.iter().flatten().count() != events as usize { return Err(Error::Restore); }
        for input in self.recent.iter().flatten() {
            let candidate = replay.stage(input.command,input.source,input.venue_token,input.stopped)?;
            replay.apply_candidate(candidate)?;
        }
        if replay.states != self.states || replay.records != self.records || replay.last_source != self.last_source
            || replay.recent != self.recent || replay.consumed_sequence != self.consumed_sequence {
            return Err(Error::Restore);
        }
        self.validate_pending_ledger(ledger)?;
        self.ready = true;
        Ok(self)
    }
    pub(crate) fn prefix_anchor(&self) -> Option<ReceiptAnchor> { self.prefix.anchor }
    pub(crate) fn prefix_source(&self) -> Option<Origin> { self.prefix.last_source }
    pub(crate) fn is_ready(&self) -> bool { self.ready }
    pub(crate) fn recent_count(&self) -> usize { self.recent.iter().flatten().count() }
    pub(crate) fn owned_bytes() -> usize { std::mem::size_of::<Self>() }
    pub(crate) fn candidate_bytes() -> usize { std::mem::size_of::<Candidate>() }
    /// Extra typed restore allowance: clone, by-value input/result and replay;
    /// the resident original is already charged by World. This deliberately
    /// conservative container count is not a compiler/native stack peak bound.
    pub(crate) fn restore_workspace_bytes() -> usize {
        4*Self::owned_bytes() + std::mem::size_of::<[Option<Record>;MAX_ACTIVITIES]>()
            + Self::candidate_bytes() + 2*MAX_COHORTS*std::mem::size_of::<SharedActivity>()
    }
}
