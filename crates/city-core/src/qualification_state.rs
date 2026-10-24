//! Bounded source-bound qualification facts and course-result responsibility.
//!
//! This module is a detached candidate only. The original World/Store writer owns
//! event authentication and must atomically CAS/apply `Candidate::next` with its
//! existing Prepared transaction. It is not a second education, population, or
//! money ledger.
use crate::{arena::Scope, member_qualification::{Background, BaseQualification, CapabilityRow, ReadSnapshot, ReadSource, Skill, MAX_READ_ROWS}, member_activity::Members as MemberRange, units::GameTime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const OPENING_VERSION: u16 = 1;
pub const MEMBER_VERSION: u32 = 1;
pub const MAX_BASELINES: usize = 16;
pub const MAX_COURSE_RESULTS: usize = 4;
pub const MAX_STATE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Profile { ImmutableGameOpeningV1 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Course { Electric, Water }
impl Course { pub fn result_bit(self) -> u8 { match self { Self::Electric => BaseQualification::Electric.bit(), Self::Water => BaseQualification::Water.bit() } } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningAuthority {
    pub id: [u8; 16], pub scope: Scope, pub population_origin: [u8; 32],
    pub member_version: u32, pub profile: Profile, pub issued_at: GameTime,
    pub valid_until: GameTime, pub permitted_results: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuerRegistration {
    pub id: [u8; 16], pub authority_id: [u8; 16], pub scope: Scope,
    pub population_origin: [u8; 32], pub member_version: u32,
    pub valid_from: GameTime, pub valid_until: GameTime, pub permitted_results: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineSourceRegistration {
    pub id: [u8; 16], pub authority_id: [u8; 16], pub scope: Scope,
    pub population_origin: [u8; 32], pub member_version: u32,
    pub valid_from: GameTime, pub valid_until: GameTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleRegistration {
    pub id: [u8; 16], pub issuer_id: [u8; 16], pub course: Course,
    pub syllabus_mask: u32, pub minimum_covered_parts: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineFact {
    pub members: MemberRange, pub source_id: [u8; 16],
    /// A positive fact mask. Missing bits are known absent only when `base_known`.
    pub base_known: bool, pub base_mask: u8,
    pub background: Option<Background>, pub skill: Option<Skill>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseResultRegistration {
    pub course: Course, pub cohort_id: u64, pub generation: u64,
    pub members: MemberRange, pub roster_hash: [u8; 32],
    pub rule_id: [u8; 16], pub issuer_id: [u8; 16],
    /// The separately registered free entitlement. This is not the old paid course.
    pub free_entitlement_id: [u8; 16],
}

/// Opening directories are immutable and fixed-size. Every baseline and course
/// responsibility resolves through these exact source/issuer/rule entries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version: u16, pub profile: Profile, pub scope: Scope,
    pub member_version: u32, pub population_origin: [u8; 32], pub total_people: u64,
    pub authority: OpeningAuthority,
    pub issuers: [Option<IssuerRegistration>; MAX_COURSE_RESULTS],
    pub baseline_sources: [Option<BaselineSourceRegistration>; MAX_BASELINES],
    pub rules: [Option<RuleRegistration>; MAX_COURSE_RESULTS],
    pub baselines: [Option<BaselineFact>; MAX_BASELINES],
    /// All four responsibility slots exist at opening. `None` is a reserved,
    /// unavailable slot; result registration is never assigned at graduation.
    pub course_results: [Option<CourseResultRegistration>; MAX_COURSE_RESULTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidOpening, InvalidRange, UnauthorizedSource, InvalidPolicy, InvalidEvent,
    InvalidAttempt, AttemptReplay, CourseAlreadyExamined, CourseNotPending,
    ExamMissing, ExamFailed, ProgressChanged, AlreadyGranted, NotReady,
    StaleSource, StaleCandidate, InvalidReceipt, Capacity, InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ExamOutcome { Pass, Fail }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExamRecord {
    pub attempt: u32, pub operation_id: [u8; 16], pub rule_id: [u8; 16],
    pub issuer_id: [u8; 16], pub occurred_at: GameTime, pub source: [u8; 32],
    pub covered_content: u32, pub syllabus_mask: u32, pub outcome: ExamOutcome,
    pub progress_binding: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub operation_id: [u8; 16], pub source: [u8; 32], pub occurred_at: GameTime, pub valid_until: GameTime,
    pub members: MemberRange, pub result: Course, pub progress_binding: [u8; 32],
    pub exam_source: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocation {
    pub operation_id: [u8; 16], pub source: [u8; 32], pub occurred_at: GameTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CourseStatus { Pending, Completed }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseSlot {
    pub registration: Option<CourseResultRegistration>, pub status: CourseStatus,
    pub latest_exam: Option<ExamRecord>, pub grant: Option<Grant>,
    pub revocation: Option<Revocation>,
}

/// State deserialization never restores authority. Call `activate` only after the
/// original Store chain has authenticated all still-needed source receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct State {
    pub opening: Opening, pub slots: [CourseSlot; MAX_COURSE_RESULTS],
    #[serde(skip)] ready: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateWire { opening: Opening, slots: [CourseSlot; MAX_COURSE_RESULTS] }
impl<'de> Deserialize<'de> for State {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = StateWire::deserialize(d)?;
        let state = Self { opening: raw.opening, slots: raw.slots, ready: false };
        state.validate().map_err(|_| serde::de::Error::custom("invalid_qualification_state"))?;
        if state.terminal_state_upper_bound_bytes().map_or(true, |n| n > MAX_STATE_BYTES) {
            return Err(serde::de::Error::custom("qualification_state_capacity"));
        }
        Ok(state)
    }
}

/// Non-serde proof constructed by the original root from the actual opening,
/// resolved education population, roster and fixed cohort identities.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedGenesis {
    scope: Scope, population_origin: [u8; 32], member_version: u32,
    total_people: u64, resolved_opening_hash: [u8; 32],
    validated_members: [Option<MemberRange>; MAX_COURSE_RESULTS],
    validated_entitlements: [Option<[u8; 16]>; MAX_COURSE_RESULTS],
    cohort_ids: [u64; MAX_COURSE_RESULTS], generations: [u64; MAX_COURSE_RESULTS],
    roster_hashes: [[u8; 32]; MAX_COURSE_RESULTS],
}
impl ValidatedGenesis {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_root(scope: Scope, population_origin: [u8; 32], member_version: u32,
        total_people: u64, resolved_opening_hash: [u8; 32],
        validated_members: [Option<MemberRange>; MAX_COURSE_RESULTS],
        validated_entitlements: [Option<[u8; 16]>; MAX_COURSE_RESULTS],
        cohort_ids: [u64; MAX_COURSE_RESULTS], generations: [u64; MAX_COURSE_RESULTS],
        roster_hashes: [[u8; 32]; MAX_COURSE_RESULTS]) -> Self {
        Self { scope, population_origin, member_version, total_people, resolved_opening_hash,
            validated_members, validated_entitlements, cohort_ids, generations, roster_hashes }
    }
}

/// Root-authenticated ingress fact. Its constructor is crate-private and must only
/// be called after System/tick/source/current progress and stable event validation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedExamEvent {
    pub(crate) slot: u8, pub(crate) occurred_at: GameTime, pub(crate) source: [u8; 32],
    pub(crate) members: MemberRange, pub(crate) cohort_id: u64, pub(crate) generation: u64,
    pub(crate) roster_hash: [u8; 32], pub(crate) course: Course,
    pub(crate) rule_id: [u8; 16], pub(crate) issuer_id: [u8; 16],
    pub(crate) covered_content: u32, pub(crate) outcome: ExamOutcome,
    pub(crate) progress_binding: [u8; 32],
}
impl ValidatedExamEvent {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_root(slot: u8, occurred_at: GameTime, source: [u8; 32], members: MemberRange,
        cohort_id: u64, generation: u64, roster_hash: [u8; 32], course: Course,
        rule_id: [u8; 16], issuer_id: [u8; 16], covered_content: u32,
        outcome: ExamOutcome, progress_binding: [u8; 32]) -> Self {
        Self { slot, occurred_at, source, members, cohort_id, generation, roster_hash, course,
            rule_id, issuer_id, covered_content, outcome, progress_binding }
    }
}

/// Root-approved completion is created only after the original evaluator has
/// rechecked actual cohort, progress, prerequisites, exam and original source.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RootApprovedCompletion {
    slot: u8, at: GameTime, source: [u8; 32], valid_until: GameTime,
    exam_source: [u8; 32], progress_binding: [u8; 32],
    semantic_key: [u8; 32], operation_id: [u8; 16],
}
impl RootApprovedCompletion {
    pub(crate) fn from_root(slot: u8, at: GameTime, source: [u8; 32], valid_until: GameTime,
        exam_source: [u8; 32], progress_binding: [u8; 32], semantic_key: [u8; 32],
        operation_id: [u8; 16]) -> Self {
        Self { slot, at, source, valid_until, exam_source, progress_binding, semantic_key, operation_id }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RootApprovedRevocation { slot: u8, at: GameTime, operation_id: [u8; 16], source: [u8; 32] }
impl RootApprovedRevocation {
    pub(crate) fn from_root(slot: u8, at: GameTime, operation_id: [u8; 16], source: [u8; 32]) -> Result<Self, Error> {
        if operation_id == [0; 16] || source == [0; 32] { return Err(Error::InvalidEvent); }
        Ok(Self { slot, at, operation_id, source })
    }
}

/// Store verifier supplies this only after source receipts and expected fingerprint
/// have been checked against the original receipt chain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VerifiedActivation { expected_fingerprint: [u8; 32] }
impl VerifiedActivation { pub(crate) fn from_root(expected_fingerprint: [u8; 32]) -> Self { Self { expected_fingerprint } } }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate { pub expected_fingerprint: [u8; 32], pub next: Box<State>, pub outcome: CandidateOutcome }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateOutcome { ExamRecorded, GrantCreated, AlreadyGranted, Revoked }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiptRef { pub kind: ReceiptKind, pub slot: u8, pub operation_id: [u8; 16], pub source: [u8; 32] }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptKind { Examination, Grant, Revocation }

impl Opening {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != OPENING_VERSION || self.profile != Profile::ImmutableGameOpeningV1
            || self.member_version != MEMBER_VERSION || self.total_people == 0
            || self.population_origin == [0; 32] || self.authority.id == [0; 16]
            || self.authority.scope != self.scope || self.authority.population_origin != self.population_origin
            || self.authority.member_version != self.member_version || self.authority.profile != self.profile
            || self.authority.issued_at > self.authority.valid_until || self.authority.permitted_results & !RESULT_MASK != 0
        { return Err(Error::InvalidOpening); }
        let mut seen_source_ids = [[0u8; 16]; MAX_BASELINES];
        let mut baseline_count = 0;
        for source in self.baseline_sources.iter().flatten() {
            if source.id == [0; 16] || source.authority_id != self.authority.id || source.scope != self.scope
                || source.population_origin != self.population_origin || source.member_version != self.member_version
                || source.valid_from > source.valid_until || !self.authority_covers(source.valid_from, source.valid_until)
                || seen_source_ids[..baseline_count].contains(&source.id) { return Err(Error::UnauthorizedSource); }
            seen_source_ids[baseline_count] = source.id; baseline_count += 1;
        }
        let mut issuer_ids = [[0u8; 16]; MAX_COURSE_RESULTS]; let mut issuer_count = 0;
        for issuer in self.issuers.iter().flatten() {
            if issuer.id == [0; 16] || issuer.authority_id != self.authority.id || issuer.scope != self.scope
                || issuer.population_origin != self.population_origin || issuer.member_version != self.member_version
                || issuer.valid_from > issuer.valid_until || issuer.permitted_results == 0
                || issuer.permitted_results & !self.authority.permitted_results != 0
                || !self.authority_covers(issuer.valid_from, issuer.valid_until)
                || issuer_ids[..issuer_count].contains(&issuer.id) { return Err(Error::UnauthorizedSource); }
            issuer_ids[issuer_count] = issuer.id; issuer_count += 1;
        }
        let mut rule_ids = [[0u8; 16]; MAX_COURSE_RESULTS]; let mut rule_count = 0;
        for rule in self.rules.iter().flatten() {
            let syllabus_parts = rule.syllabus_mask.count_ones();
            if rule.id == [0; 16] || rule.syllabus_mask == 0 || rule.minimum_covered_parts == 0
                || rule.minimum_covered_parts as u32 > rule.syllabus_mask.count_ones()
                || (rule.minimum_covered_parts as u32) * 100 < syllabus_parts * 90
                || self.issuer(rule.issuer_id).is_none()
                || rule_ids[..rule_count].contains(&rule.id) { return Err(Error::InvalidPolicy); }
            rule_ids[rule_count] = rule.id; rule_count += 1;
        }
        let mut prior: [Option<MemberRange>; MAX_BASELINES] = [None; MAX_BASELINES]; let mut n = 0;
        for baseline in self.baselines.iter().flatten() {
            if !valid_members(baseline.members, self.total_people) || baseline.base_mask & !BASE_MASK != 0
                || self.baseline_source(baseline.source_id).is_none() { return Err(Error::InvalidRange); }
            for range in prior[..n].iter().flatten() { if overlaps(*range, baseline.members) { return Err(Error::InvalidRange); } }
            prior[n] = Some(baseline.members); n += 1;
        }
        let mut course_keys = [[0u8; 32]; MAX_COURSE_RESULTS]; let mut course_count = 0;
        for (idx, registration) in self.course_results.iter().enumerate() {
            let Some(r) = registration else { continue };
            let rule = self.rule(r.rule_id).ok_or(Error::InvalidPolicy)?;
            let issuer = self.issuer(r.issuer_id).ok_or(Error::UnauthorizedSource)?;
            if r.cohort_id == 0 || r.generation == 0 || r.roster_hash == [0; 32] || r.free_entitlement_id == [0; 16]
                || !valid_members(r.members, self.total_people) || rule.issuer_id != r.issuer_id || rule.course != r.course
                || issuer.permitted_results & r.course.result_bit() == 0 || self.authority.permitted_results & r.course.result_bit() == 0
                || self.validated_slot(idx, r).is_err() { return Err(Error::InvalidPolicy); }
            let key = semantic_key(self.population_origin, r.members, r.cohort_id, r.generation, r.course);
            if course_keys[..course_count].contains(&key) { return Err(Error::InvalidPolicy); }
            course_keys[course_count] = key; course_count += 1;
        }
        Ok(())
    }
    fn authority_covers(&self, from: GameTime, until: GameTime) -> bool { from >= self.authority.issued_at && until <= self.authority.valid_until }
    fn issuer(&self, id: [u8; 16]) -> Option<&IssuerRegistration> { self.issuers.iter().flatten().find(|x| x.id == id) }
    fn baseline_source(&self, id: [u8; 16]) -> Option<&BaselineSourceRegistration> { self.baseline_sources.iter().flatten().find(|x| x.id == id) }
    fn rule(&self, id: [u8; 16]) -> Option<&RuleRegistration> { self.rules.iter().flatten().find(|x| x.id == id) }
    fn validated_slot(&self, idx: usize, r: &CourseResultRegistration) -> Result<(), Error> {
        if idx >= MAX_COURSE_RESULTS || r.members.start >= r.members.end { return Err(Error::InvalidRange); }
        Ok(())
    }
}

const RESULT_MASK: u8 = (1 << 1) | (1 << 2);
const BASE_MASK: u8 = u8::MAX;
fn valid_members(m: MemberRange, people: u64) -> bool { m.start < m.end && m.end <= people }
fn overlaps(a: MemberRange, b: MemberRange) -> bool { a.start < b.end && b.start < a.end }
fn semantic_key(pop: [u8; 32], m: MemberRange, cohort: u64, generation: u64, course: Course) -> [u8; 32] {
    let mut h = Sha256::new(); h.update(b"mycity-qualification-semantic-v1"); h.update(pop);
    h.update(m.start.to_le_bytes()); h.update(m.end.to_le_bytes()); h.update(cohort.to_le_bytes());
    h.update(generation.to_le_bytes()); h.update([match course { Course::Electric => 1, Course::Water => 2 }]); h.finalize().into()
}

impl State {
    pub(crate) fn new(opening: Opening, genesis: &ValidatedGenesis) -> Result<Self, Error> {
        opening.validate()?;
        if opening.scope != genesis.scope || opening.population_origin != genesis.population_origin
            || opening.member_version != genesis.member_version || opening.total_people != genesis.total_people
            || opening_fingerprint(&opening)? != genesis.resolved_opening_hash { return Err(Error::InvalidOpening); }
        for (idx, r) in opening.course_results.iter().enumerate() {
            if let Some(r) = r {
                if genesis.validated_members[idx] != Some(r.members) || genesis.cohort_ids[idx] != r.cohort_id
                    || genesis.generations[idx] != r.generation || genesis.roster_hashes[idx] != r.roster_hash
                    || genesis.validated_entitlements[idx] != Some(r.free_entitlement_id)
                    || genesis.validated_members[idx].map_or(true, |m| !valid_members(m, genesis.total_people))
                { return Err(Error::InvalidOpening); }
            }
        }
        let slots = std::array::from_fn(|i| CourseSlot { registration: opening.course_results[i], status: CourseStatus::Pending,
            latest_exam: None, grant: None, revocation: None });
        let state = Self { opening, slots, ready: true }; state.validate()?;
        if state.terminal_state_upper_bound_bytes()? > MAX_STATE_BYTES { return Err(Error::Capacity); }
        Ok(state)
    }
    pub fn is_ready(&self) -> bool { self.ready }
    /// Stable course-result identity owned by this module. Root reads it only after
    /// matching every field against the original completion candidate.
    pub(crate) fn completion_key(&self, slot: u8) -> Result<[u8; 32], Error> {
        let r = self.slots.get(slot as usize).ok_or(Error::InvalidEvent)?.registration.ok_or(Error::InvalidEvent)?;
        Ok(semantic_key(self.opening.population_origin, r.members, r.cohort_id, r.generation, r.course))
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.opening.validate()?;
        for (idx, s) in self.slots.iter().enumerate() {
            if s.registration != self.opening.course_results[idx] { return Err(Error::InvalidState); }
            if s.latest_exam.is_some() && s.registration.is_none() { return Err(Error::InvalidState); }
            if s.status == CourseStatus::Completed && s.grant.is_none() { return Err(Error::InvalidState); }
            if s.status == CourseStatus::Pending && (s.grant.is_some() || s.revocation.is_some()) { return Err(Error::InvalidState); }
            if let Some(e) = s.latest_exam {
                let r = s.registration.ok_or(Error::InvalidState)?;
                let rule = self.opening.rule(r.rule_id).ok_or(Error::InvalidState)?;
                let issuer = self.opening.issuer(r.issuer_id).ok_or(Error::InvalidState)?;
                if e.attempt == 0 || e.operation_id == [0; 16] || e.source == [0; 32]
                    || e.rule_id != r.rule_id || e.issuer_id != r.issuer_id || e.syllabus_mask != rule.syllabus_mask
                    || e.covered_content & !rule.syllabus_mask != 0
                    || e.occurred_at < issuer.valid_from || e.occurred_at > issuer.valid_until {
                    return Err(Error::InvalidState);
                }
            }
            if let Some(g) = s.grant {
                let r = s.registration.ok_or(Error::InvalidState)?;
                let e = s.latest_exam.ok_or(Error::InvalidState)?;
                let issuer = self.opening.issuer(r.issuer_id).ok_or(Error::InvalidState)?;
                if g.operation_id == [0; 16] || e.outcome != ExamOutcome::Pass
                    || e.covered_content.count_ones() < self.opening.rule(r.rule_id).ok_or(Error::InvalidState)?.minimum_covered_parts as u32
                    || g.members != r.members || g.result != r.course
                    || g.exam_source != e.source || g.progress_binding != e.progress_binding
                    || g.occurred_at < e.occurred_at || g.occurred_at > g.valid_until
                    || g.occurred_at < issuer.valid_from || g.valid_until > issuer.valid_until { return Err(Error::InvalidState); }
                if g.source == [0; 32] { return Err(Error::InvalidState); }
            }
            if let Some(v) = s.revocation { if s.grant.is_none() || v.operation_id == [0;16] || v.source == [0; 32] || v.occurred_at < s.grant.unwrap().occurred_at { return Err(Error::InvalidState); } }
        }
        Ok(())
    }
    pub fn max_bytes(&self) -> usize { MAX_STATE_BYTES }
    /// Conservative JSON byte upper bound after every registered course slot has
    /// an exam, grant and revocation. Opening data remains its immutable exact value.
    pub fn terminal_state_upper_bound_bytes(&self) -> Result<usize, Error> {
        self.validate()?;
        let mut worst = self.clone();
        let max_time = GameTime::new(i64::MAX).map_err(|_| Error::Capacity)?;
        for slot in &mut worst.slots {
            let Some(reg) = slot.registration else { continue };
            let rule = worst.opening.rule(reg.rule_id).ok_or(Error::InvalidPolicy)?;
            let issuer = worst.opening.issuer(reg.issuer_id).ok_or(Error::UnauthorizedSource)?;
            let at = issuer.valid_until;
            slot.status = CourseStatus::Completed;
            slot.latest_exam = Some(ExamRecord { attempt: u32::MAX, operation_id: [u8::MAX; 16],
                rule_id: reg.rule_id, issuer_id: reg.issuer_id, occurred_at: at, source: [u8::MAX; 32],
                covered_content: rule.syllabus_mask, syllabus_mask: rule.syllabus_mask,
                outcome: ExamOutcome::Pass, progress_binding: [u8::MAX; 32] });
            slot.grant = Some(Grant { operation_id: [u8::MAX; 16], source: [u8::MAX; 32], occurred_at: at, valid_until: issuer.valid_until,
                members: reg.members, result: reg.course, progress_binding: [u8::MAX; 32], exam_source: [u8::MAX; 32] });
            slot.revocation = Some(Revocation { operation_id: [u8::MAX; 16], source: [u8::MAX; 32], occurred_at: max_time });
        }
        worst.validate()?;
        let mut writer = CountWriter { count: 0, max: usize::MAX };
        serde_json::to_writer(&mut writer, &worst).map_err(|_| Error::Capacity)?;
        Ok(writer.count)
    }
    /// Typed in-process extra-memory estimate for one detached candidate, excluding
    /// live State, allocator metadata and root Prepared/Journal/Envelope storage.
    pub fn candidate_workspace_extra_estimate_bytes() -> usize {
        std::mem::size_of::<State>() * 2 + std::mem::size_of::<Candidate>()
            + std::mem::size_of::<HashWriter>().max(std::mem::size_of::<CountWriter>())
    }
    /// Typed in-process extra-memory estimate for cold decode/activation, excluding
    /// stored bytes, existing State, allocator metadata and JSON parser buffers.
    pub fn restore_workspace_extra_estimate_bytes() -> usize {
        std::mem::size_of::<StateWire>() + std::mem::size_of::<State>() * 2
            + std::mem::size_of::<[Option<ReceiptRef>; 12]>()
            + std::mem::size_of::<HashWriter>().max(std::mem::size_of::<CountWriter>())
    }
    pub fn serialized_len(&self) -> Result<usize, Error> {
        self.validate()?; let mut w = CountWriter { count: 0, max: MAX_STATE_BYTES };
        serde_json::to_writer(&mut w, self).map_err(|_| Error::Capacity)?; Ok(w.count)
    }
    pub fn fingerprint(&self) -> Result<[u8; 32], Error> {
        self.validate()?; let mut w = HashWriter { hash: Sha256::new(), count: 0, max: MAX_STATE_BYTES };
        serde_json::to_writer(&mut w, self).map_err(|_| Error::Capacity)?; Ok(w.hash.finalize().into())
    }
    pub fn needed_receipts(&self) -> [Option<ReceiptRef>; 12] {
        let mut out = [None; 12]; let mut i = 0;
        for (slot, s) in self.slots.iter().enumerate() {
            for (kind, receipt) in [(ReceiptKind::Examination, s.latest_exam.map(|x| (x.operation_id, x.source))),
                (ReceiptKind::Grant, s.grant.map(|x| (x.operation_id, x.source))),
                (ReceiptKind::Revocation, s.revocation.map(|x| (x.operation_id, x.source)))] {
                if let Some((operation_id, source)) = receipt { out[i] = Some(ReceiptRef { kind, slot: slot as u8, operation_id, source }); i += 1; }
            }
        }
        out
    }
    pub(crate) fn activate(&self, proof: VerifiedActivation) -> Result<Box<Self>, Error> {
        if self.fingerprint()? != proof.expected_fingerprint { return Err(Error::StaleCandidate); }
        let mut next = self.clone(); next.ready = true; Ok(Box::new(next))
    }
    pub(crate) fn prepare_record_examination(&self, event: ValidatedExamEvent, attempt: u32, operation_id: [u8; 16]) -> Result<Candidate, Error> {
        self.ensure_ready()?; self.check_event(&event)?;
        if attempt == 0 || operation_id == [0; 16] { return Err(Error::InvalidAttempt); }
        let idx = event.slot as usize; let old = self.slots.get(idx).ok_or(Error::InvalidEvent)?;
        if old.status != CourseStatus::Pending { return Err(Error::CourseAlreadyExamined); }
        if let Some(prev) = old.latest_exam {
            if attempt == prev.attempt { return Err(Error::AttemptReplay); }
            if attempt != prev.attempt.checked_add(1).ok_or(Error::InvalidAttempt)? || event.occurred_at <= prev.occurred_at
                || event.source == prev.source || operation_id == prev.operation_id { return Err(Error::InvalidAttempt); }
        } else if attempt != 1 { return Err(Error::InvalidAttempt); }
        let reg = old.registration.ok_or(Error::InvalidEvent)?;
        let rule = self.opening.rule(reg.rule_id).ok_or(Error::InvalidPolicy)?;
        if event.covered_content & !rule.syllabus_mask != 0 { return Err(Error::InvalidPolicy); }
        let mut next = self.clone(); next.slots[idx].latest_exam = Some(ExamRecord { attempt, operation_id, rule_id: event.rule_id,
            issuer_id: event.issuer_id, occurred_at: event.occurred_at, source: event.source,
            covered_content: event.covered_content, syllabus_mask: rule.syllabus_mask, outcome: event.outcome,
            progress_binding: event.progress_binding });
        self.candidate(next, CandidateOutcome::ExamRecorded)
    }
    pub(crate) fn prepare_grant(&self, approved: RootApprovedCompletion) -> Result<Candidate, Error> {
        self.ensure_ready()?; let idx = approved.slot as usize; let slot = self.slots.get(idx).ok_or(Error::InvalidEvent)?;
        let reg = slot.registration.ok_or(Error::InvalidEvent)?; let exam = slot.latest_exam.ok_or(Error::ExamMissing)?;
        if slot.status == CourseStatus::Completed { return Err(Error::AlreadyGranted); }
        if exam.outcome != ExamOutcome::Pass { return Err(Error::ExamFailed); }
        if exam.source != approved.exam_source || exam.progress_binding != approved.progress_binding
            || approved.semantic_key != semantic_key(self.opening.population_origin, reg.members, reg.cohort_id, reg.generation, reg.course)
            || approved.at < exam.occurred_at || approved.at > approved.valid_until || approved.source == [0; 32]
            || approved.operation_id == [0; 16] { return Err(Error::ProgressChanged); }
        let issuer = self.opening.issuer(reg.issuer_id).ok_or(Error::UnauthorizedSource)?;
        let rule = self.opening.rule(reg.rule_id).ok_or(Error::InvalidPolicy)?;
        if exam.covered_content.count_ones() < rule.minimum_covered_parts as u32 { return Err(Error::InvalidPolicy); }
        if approved.at < issuer.valid_from || approved.at > issuer.valid_until || approved.valid_until > issuer.valid_until { return Err(Error::UnauthorizedSource); }
        let mut next = self.clone(); next.slots[idx].grant = Some(Grant { operation_id: approved.operation_id, source: approved.source, occurred_at: approved.at,
            valid_until: approved.valid_until, members: reg.members, result: reg.course,
            progress_binding: approved.progress_binding, exam_source: approved.exam_source });
        next.slots[idx].status = CourseStatus::Completed;
        self.candidate(next, CandidateOutcome::GrantCreated)
    }
    pub(crate) fn prepare_revoke(&self, proof: RootApprovedRevocation) -> Result<Candidate, Error> {
        self.ensure_ready()?; let idx = proof.slot as usize; let slot = self.slots.get(idx).ok_or(Error::InvalidEvent)?;
        let grant = slot.grant.ok_or(Error::AlreadyGranted)?;
        if slot.revocation.is_some() || proof.operation_id == [0; 16] || proof.source == [0; 32] || proof.at < grant.occurred_at { return Err(Error::InvalidEvent); }
        let mut next = self.clone(); next.slots[idx].revocation = Some(Revocation { operation_id: proof.operation_id, source: proof.source, occurred_at: proof.at });
        self.candidate(next, CandidateOutcome::Revoked)
    }
    pub fn query(&self, source: ReadSource, requested: MemberRange) -> Result<ReadSnapshot, Error> {
        self.ensure_ready()?;
        if source.scope != self.opening.scope || source.population_origin != self.opening.population_origin
            || source.member_version != self.opening.member_version || !valid_members(requested, self.opening.total_people) { return Err(Error::StaleSource); }
        let mut points = [0u64; 42]; let mut n = 0usize; points[n] = requested.start; n += 1; points[n] = requested.end; n += 1;
        for b in self.opening.baselines.iter().flatten() { add_clipped_points(&mut points, &mut n, requested, b.members); }
        for s in &self.slots { if let Some(g) = s.grant { add_clipped_points(&mut points, &mut n, requested, g.members); } }
        points[..n].sort_unstable(); let mut unique = 0;
        for i in 0..n { if unique == 0 || points[i] != points[unique - 1] { points[unique] = points[i]; unique += 1; } }
        let mut rows = [None; MAX_READ_ROWS]; let mut count = 0usize; let mut complete = true;
        for pair in points[..unique].windows(2) {
            let range = MemberRange { start: pair[0], end: pair[1] }; if range.start >= range.end { continue; }
            let mut mask = 0u8; let mut background = None; let mut skill = None; let mut base_known = false;
            for b in self.opening.baselines.iter().flatten().filter(|b| covers(b.members, range)) {
                let authority = self.opening.baseline_source(b.source_id).ok_or(Error::InvalidState)?;
                if source.as_of < authority.valid_from || source.as_of > authority.valid_until { continue; }
                mask |= b.base_mask; base_known |= b.base_known; choose_background(&mut background, b.background); choose_skill(&mut skill, b.skill);
            }
            for s in &self.slots {
                let Some(g) = s.grant else { continue };
                if covers(g.members, range) && source.as_of >= g.occurred_at && source.as_of < g.valid_until
                    && s.revocation.map_or(true, |r| source.as_of < r.occurred_at) { mask |= g.result.result_bit(); }
            }
            if count < MAX_READ_ROWS { rows[count] = Some(CapabilityRow { members: range, background, skill, base_known,
                base_mask: mask, endorsements_known: false, endorsement_mask: 0 }); count += 1; }
            else { complete = false; }
        }
        Ok(ReadSnapshot { source, requested, rows, complete })
    }
    fn check_event(&self, e: &ValidatedExamEvent) -> Result<(), Error> {
        if e.source == [0; 32] || e.slot as usize >= MAX_COURSE_RESULTS { return Err(Error::InvalidEvent); }
        let r = self.slots[e.slot as usize].registration.ok_or(Error::InvalidEvent)?;
        let rule = self.opening.rule(r.rule_id).ok_or(Error::InvalidPolicy)?;
        let issuer = self.opening.issuer(r.issuer_id).ok_or(Error::UnauthorizedSource)?;
        if e.members != r.members || e.cohort_id != r.cohort_id || e.generation != r.generation || e.roster_hash != r.roster_hash
            || e.course != r.course || e.rule_id != r.rule_id || e.issuer_id != r.issuer_id
            || rule.issuer_id != issuer.id || issuer.permitted_results & e.course.result_bit() == 0
            || e.occurred_at < issuer.valid_from || e.occurred_at > issuer.valid_until { return Err(Error::InvalidEvent); }
        Ok(())
    }
    fn ensure_ready(&self) -> Result<(), Error> { if self.ready { Ok(()) } else { Err(Error::NotReady) } }
    fn candidate(&self, next: State, outcome: CandidateOutcome) -> Result<Candidate, Error> {
        next.validate()?; let bytes = next.serialized_len()?; if bytes > MAX_STATE_BYTES { return Err(Error::Capacity); }
        Ok(Candidate { expected_fingerprint: self.fingerprint()?, next: Box::new(next), outcome })
    }
}

fn covers(outer: MemberRange, inner: MemberRange) -> bool { outer.start <= inner.start && outer.end >= inner.end }
fn add_clipped_points(points: &mut [u64; 42], n: &mut usize, request: MemberRange, interval: MemberRange) {
    let start = request.start.max(interval.start); let end = request.end.min(interval.end);
    if start < end && *n + 2 <= points.len() { points[*n] = start; points[*n + 1] = end; *n += 2; }
}
fn choose_background(dst: &mut Option<Background>, value: Option<Background>) {
    if let Some(v) = value { if dst.map_or(true, |old| v.rank() > old.rank()) { *dst = Some(v); } }
}
fn choose_skill(dst: &mut Option<Skill>, value: Option<Skill>) {
    if let Some(v) = value { if dst.map_or(true, |old| v.rank() > old.rank()) { *dst = Some(v); } }
}
pub(crate) fn opening_fingerprint(opening: &Opening) -> Result<[u8; 32], Error> {
    let mut w = HashWriter { hash: Sha256::new(), count: 0, max: MAX_STATE_BYTES };
    serde_json::to_writer(&mut w, opening).map_err(|_| Error::Capacity)?; Ok(w.hash.finalize().into())
}
struct CountWriter { count: usize, max: usize }
impl Write for CountWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> { self.count = self.count.checked_add(b.len()).filter(|n| *n <= self.max).ok_or_else(|| io::Error::new(io::ErrorKind::Other, "bounded_state_bytes"))?; Ok(b.len()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
struct HashWriter { hash: Sha256, count: usize, max: usize }
impl Write for HashWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> { self.count = self.count.checked_add(b.len()).filter(|n| *n <= self.max).ok_or_else(|| io::Error::new(io::ErrorKind::Other, "bounded_state_bytes"))?; self.hash.update(b); Ok(b.len()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn opening() -> Opening {
        let scope = Scope::new(11, 3).unwrap();
        Opening { version: 1, profile: Profile::ImmutableGameOpeningV1, scope, member_version: 1,
            population_origin: [8; 32], total_people: 8,
            authority: OpeningAuthority { id: [1;16], scope, population_origin: [8;32], member_version: 1,
                profile: Profile::ImmutableGameOpeningV1, issued_at: GameTime::new(0).unwrap(), valid_until: GameTime::new(1000).unwrap(), permitted_results: RESULT_MASK },
            issuers: [Some(IssuerRegistration { id: [2;16], authority_id: [1;16], scope, population_origin: [8;32], member_version: 1,
                valid_from: GameTime::new(0).unwrap(), valid_until: GameTime::new(1000).unwrap(), permitted_results: RESULT_MASK }), None, None, None],
            baseline_sources: [Some(BaselineSourceRegistration { id: [3;16], authority_id: [1;16], scope, population_origin: [8;32], member_version: 1,
                valid_from: GameTime::new(0).unwrap(), valid_until: GameTime::new(1000).unwrap() }), None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            rules: [Some(RuleRegistration { id: [4;16], issuer_id: [2;16], course: Course::Electric, syllabus_mask: 0b1111, minimum_covered_parts: 4 }), None, None, None],
            baselines: [Some(BaselineFact { members: MemberRange { start: 0, end: 2 }, source_id: [3;16], base_known: true,
                base_mask: 0, background: Some(Background::General), skill: Some(Skill::Technical) }), None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            course_results: [Some(CourseResultRegistration { course: Course::Electric, cohort_id: 5, generation: 1,
                members: MemberRange { start: 2, end: 4 }, roster_hash: [9;32], rule_id: [4;16], issuer_id: [2;16], free_entitlement_id: [5;16] }), None, None, None] }
    }
    fn genesis(o: &Opening) -> ValidatedGenesis {
        ValidatedGenesis::from_root(o.scope, o.population_origin, o.member_version, o.total_people, opening_fingerprint(o).unwrap(),
            [Some(MemberRange { start: 2, end: 4 }), None, None, None], [Some([5;16]),None,None,None],
            [5,0,0,0], [1,0,0,0], [[9;32],[0;32],[0;32],[0;32]])
    }
    fn state() -> State { let o = opening(); State::new(o.clone(), &genesis(&o)).unwrap() }
    fn exam(s: &State, tick: i64, outcome: ExamOutcome, progress: [u8;32]) -> Candidate {
        let e = ValidatedExamEvent::from_root(0, GameTime::new(tick).unwrap(), [7;32], MemberRange { start: 2, end: 4 }, 5, 1, [9;32],
            Course::Electric, [4;16], [2;16], 0b111, outcome, progress);
        s.prepare_record_examination(e, 1, [6;16]).unwrap()
    }
    #[test] fn rejects_bad_baseline_range_and_unmapped_authority() {
        let mut o = opening(); o.baselines[0].as_mut().unwrap().members.end = 9; assert_eq!(o.validate(), Err(Error::InvalidRange));
        let mut o = opening(); o.baselines[0].as_mut().unwrap().source_id = [99;16]; assert_eq!(o.validate(), Err(Error::InvalidRange));
        let mut o = opening(); o.baselines[1] = Some(BaselineFact { members: MemberRange { start: 1, end: 3 }, source_id: [3;16], base_known: true, base_mask: 0, background: None, skill: None });
        assert_eq!(o.validate(), Err(Error::InvalidRange));
    }
    #[test] fn rejects_issuer_scope_and_authority_result_escalation() {
        let mut o = opening(); o.issuers[0].as_mut().unwrap().scope = Scope::new(12,3).unwrap(); assert_eq!(o.validate(), Err(Error::UnauthorizedSource));
        let mut o = opening(); o.authority.permitted_results = BaseQualification::Teaching.bit(); assert_eq!(o.validate(), Err(Error::InvalidOpening));
        let mut o = opening(); o.issuers[0].as_mut().unwrap().permitted_results = BaseQualification::Teaching.bit(); assert_eq!(o.validate(), Err(Error::UnauthorizedSource));
        let mut o = opening(); o.rules[0].as_mut().unwrap().minimum_covered_parts = 3; assert_eq!(o.validate(), Err(Error::InvalidPolicy));
    }
    #[test] fn positive_base_mask_does_not_make_missing_bits_known() {
        let mut s = state(); let b = s.opening.baselines[0].as_mut().unwrap(); b.base_known = false; b.base_mask = BaseQualification::Electric.bit();
        assert_eq!(s.opening.validate(), Ok(()));
        let source = ReadSource { scope:s.opening.scope, population_origin:s.opening.population_origin, member_version:1,
            root:[3;32], revision:1, as_of:GameTime::new(1).unwrap() };
        let row = s.query(source, MemberRange { start:0, end:2 }).unwrap().rows[0].unwrap();
        assert!(!row.base_known); assert_eq!(row.base_mask, BaseQualification::Electric.bit());
    }
    #[test] fn all_four_slots_exist_before_completion() { let s = state(); assert_eq!(s.slots.len(), 4); assert!(s.slots.iter().all(|x| x.status == CourseStatus::Pending)); }
    #[test] fn failed_exam_is_recorded_but_cannot_grant() {
        let s = state(); let c = exam(&s, 10, ExamOutcome::Fail, [1;32]); assert_eq!(c.next.slots[0].latest_exam.unwrap().outcome, ExamOutcome::Fail);
        let p = RootApprovedCompletion::from_root(0, GameTime::new(11).unwrap(), [6;32], GameTime::new(100).unwrap(), [7;32], [1;32], semantic_key([8;32], MemberRange {start:2,end:4},5,1,Course::Electric), [1;16]);
        assert_eq!(c.next.prepare_grant(p).unwrap_err(), Error::ExamFailed);
    }
    #[test] fn low_coverage_pass_and_zero_coverage_fail_are_preserved_as_actual_exams() {
        let s = state();
        let low = ValidatedExamEvent::from_root(0,GameTime::new(10).unwrap(),[18;32],MemberRange{start:2,end:4},5,1,[9;32],
            Course::Electric,[4;16],[2;16],0b0111,ExamOutcome::Pass,[1;32]);
        let pass = s.prepare_record_examination(low,1,[17;16]).unwrap();
        assert_eq!(pass.next.slots[0].latest_exam.unwrap().covered_content,0b0111);
        let approval = RootApprovedCompletion::from_root(0,GameTime::new(11).unwrap(),[16;32],GameTime::new(100).unwrap(),[18;32],[1;32],
            pass.next.completion_key(0).unwrap(),[15;16]);
        assert_eq!(pass.next.prepare_grant(approval).unwrap_err(),Error::InvalidPolicy);
        let failed = ValidatedExamEvent::from_root(0,GameTime::new(10).unwrap(),[14;32],MemberRange{start:2,end:4},5,1,[9;32],
            Course::Electric,[4;16],[2;16],0,ExamOutcome::Fail,[2;32]);
        let fail = s.prepare_record_examination(failed,1,[13;16]).unwrap();
        let record=fail.next.slots[0].latest_exam.unwrap(); assert_eq!(record.outcome,ExamOutcome::Fail); assert_eq!(record.covered_content,0);
    }
    #[test] fn duplicate_attempt_cannot_rewrite_result_and_retake_is_later() {
        let s = state(); let c = exam(&s, 10, ExamOutcome::Fail, [1;32]);
        let e = ValidatedExamEvent::from_root(0, GameTime::new(11).unwrap(), [8;32], MemberRange {start:2,end:4},5,1,[9;32],Course::Electric,[4;16],[2;16],0b111,ExamOutcome::Pass,[1;32]);
        assert_eq!(c.next.prepare_record_examination(e, 1, [7;16]).unwrap_err(), Error::AttemptReplay);
        assert_eq!(c.next.prepare_record_examination(e, 2, [7;16]).unwrap().next.slots[0].latest_exam.unwrap().outcome, ExamOutcome::Pass);
    }
    #[test] fn max_attempt_does_not_prevent_grant_for_existing_exam() {
        let mut s = state(); s.slots[0].latest_exam = Some(ExamRecord { attempt:u32::MAX,operation_id:[1;16],rule_id:[4;16],issuer_id:[2;16],occurred_at:GameTime::new(10).unwrap(),source:[7;32],covered_content:15,syllabus_mask:15,outcome:ExamOutcome::Pass,progress_binding:[1;32] });
        let p = RootApprovedCompletion::from_root(0,GameTime::new(11).unwrap(),[6;32],GameTime::new(100).unwrap(),[7;32],[1;32],semantic_key([8;32],MemberRange{start:2,end:4},5,1,Course::Electric),[2;16]);
        assert_eq!(s.prepare_grant(p).unwrap().outcome, CandidateOutcome::GrantCreated);
    }
    #[test] fn different_operation_regrant_is_semantic_already_granted() {
        let mut s = state(); s.slots[0].latest_exam = Some(ExamRecord { attempt:1,operation_id:[1;16],rule_id:[4;16],issuer_id:[2;16],occurred_at:GameTime::new(10).unwrap(),source:[7;32],covered_content:15,syllabus_mask:15,outcome:ExamOutcome::Pass,progress_binding:[1;32] });
        let p = RootApprovedCompletion::from_root(0,GameTime::new(11).unwrap(),[6;32],GameTime::new(100).unwrap(),[7;32],[1;32],semantic_key([8;32],MemberRange{start:2,end:4},5,1,Course::Electric),[2;16]);
        let c = s.prepare_grant(p).unwrap(); assert_eq!(c.outcome,CandidateOutcome::GrantCreated); assert_eq!(c.next.prepare_grant(p).unwrap_err(),Error::AlreadyGranted);
        let receipts=c.next.needed_receipts();
        assert_eq!(receipts[0].unwrap().operation_id,[1;16]); assert_eq!(receipts[1].unwrap().operation_id,[2;16]);
        let revoke=RootApprovedRevocation::from_root(0,GameTime::new(12).unwrap(),[3;16],[4;32]).unwrap();
        let revoked=c.next.prepare_revoke(revoke).unwrap();
        assert_eq!(revoked.next.needed_receipts()[2].unwrap().operation_id,[3;16]);
        assert_eq!(RootApprovedRevocation::from_root(0,GameTime::new(12).unwrap(),[0;16],[4;32]).unwrap_err(),Error::InvalidEvent);
    }
    #[test] fn candidates_are_detached_and_stale_cas_is_detectable() {
        let s = state(); let c = exam(&s,10,ExamOutcome::Pass,[1;32]); assert_eq!(s.slots[0].latest_exam,None);
        let mut changed = s.clone(); changed.slots[0].latest_exam = Some(c.next.slots[0].latest_exam.unwrap());
        assert_ne!(s.fingerprint().unwrap(), changed.fingerprint().unwrap());
        assert_eq!(c.expected_fingerprint,s.fingerprint().unwrap());
        let bad = ValidatedExamEvent::from_root(0,GameTime::new(10).unwrap(),[7;32],MemberRange{start:2,end:4},5,1,[9;32],Course::Electric,[4;16],[2;16],0b1_0000,ExamOutcome::Pass,[1;32]);
        assert_eq!(s.prepare_record_examination(bad,1,[6;16]).unwrap_err(),Error::InvalidPolicy);
        assert_eq!(s.slots[0].latest_exam,None);
    }
    #[test] fn read_segments_keep_unknown_gaps_expiry_revoke_and_endpoint_boundaries() {
        let mut s = state(); s.slots[0].latest_exam = Some(ExamRecord { attempt:1,operation_id:[1;16],rule_id:[4;16],issuer_id:[2;16],occurred_at:GameTime::new(10).unwrap(),source:[7;32],covered_content:15,syllabus_mask:15,outcome:ExamOutcome::Pass,progress_binding:[1;32] });
        let p = RootApprovedCompletion::from_root(0,GameTime::new(11).unwrap(),[6;32],GameTime::new(20).unwrap(),[7;32],[1;32],semantic_key([8;32],MemberRange{start:2,end:4},5,1,Course::Electric),[2;16]);
        s = *s.prepare_grant(p).unwrap().next;
        let src = |at| ReadSource {scope:s.opening.scope,population_origin:[8;32],member_version:1,root:[3;32],revision:1,as_of:GameTime::new(at).unwrap()};
        let q = s.query(src(12),MemberRange{start:0,end:6}).unwrap(); assert_eq!(q.rows[0].unwrap().members,MemberRange{start:0,end:2});
        assert!(q.rows[0].unwrap().base_known); assert!(!q.rows[1].unwrap().base_known);
        assert_eq!(q.rows[1].unwrap().base_mask,BaseQualification::Electric.bit());
        assert_eq!(s.query(src(20),MemberRange{start:2,end:4}).unwrap().rows[0].unwrap().base_mask,0);
        let mut s2 = s.clone(); s2.slots[0].revocation = Some(Revocation{operation_id:[4;16],source:[4;32],occurred_at:GameTime::new(13).unwrap()});
        assert_eq!(s2.query(src(14),MemberRange{start:2,end:4}).unwrap().rows[0].unwrap().base_mask,0);
    }
    #[test] fn rejects_cross_source_and_unready_query_and_serde_drops_authority() {
        let s=state(); let bytes=serde_json::to_vec(&s).unwrap(); let restored:State=serde_json::from_slice(&bytes).unwrap();
        assert!(!restored.is_ready()); assert_eq!(restored.query(ReadSource{scope:s.opening.scope,population_origin:[8;32],member_version:1,root:[1;32],revision:1,as_of:GameTime::new(1).unwrap()},MemberRange{start:0,end:1}),Err(Error::NotReady));
        let mut wrong=restored; wrong.ready=true; assert_eq!(wrong.query(ReadSource{scope:s.opening.scope,population_origin:[9;32],member_version:1,root:[1;32],revision:1,as_of:GameTime::new(1).unwrap()},MemberRange{start:0,end:1}),Err(Error::StaleSource));
        assert_eq!(s.query(ReadSource{scope:s.opening.scope,population_origin:[8;32],member_version:2,root:[1;32],revision:1,as_of:GameTime::new(1).unwrap()},MemberRange{start:0,end:1}),Err(Error::StaleSource));
        assert!(s.serialized_len().unwrap() <= s.max_bytes());
    }
    #[test] fn too_many_read_segments_are_explicitly_incomplete_unknown() {
        let mut s = state(); s.opening.total_people = 40;
        s.opening.baselines = std::array::from_fn(|i| Some(BaselineFact { members: MemberRange { start: (i * 2) as u64, end: (i * 2 + 1) as u64 },
            source_id: [3;16], base_known: true, base_mask: 0, background: None, skill: None }));
        let source = ReadSource { scope:s.opening.scope, population_origin:s.opening.population_origin, member_version:1,
            root:[3;32], revision:1, as_of:GameTime::new(1).unwrap() };
        let snapshot=s.query(source,MemberRange{start:0,end:33}).unwrap(); assert!(!snapshot.complete);
        assert!(snapshot.rows.iter().all(Option::is_some));
    }
    #[test] fn max_width_opening_and_receipts_finish_or_fail_at_admission() {
        let mut o=opening(); let scope=Scope::new(u64::MAX,u64::MAX-1).unwrap(); let max=GameTime::new(i64::MAX).unwrap();
        let members=MemberRange{start:u64::MAX-2,end:u64::MAX};
        o.scope=scope; o.population_origin=[u8::MAX;32]; o.total_people=u64::MAX;
        o.authority.id=[u8::MAX;16]; o.authority.scope=scope; o.authority.population_origin=o.population_origin; o.authority.valid_until=max;
        let issuer=o.issuers[0].as_mut().unwrap(); issuer.id=[254;16]; issuer.authority_id=[u8::MAX;16]; issuer.scope=scope;
        issuer.population_origin=o.population_origin; issuer.valid_until=max;
        let source_ids:[[u8;16];MAX_BASELINES]=std::array::from_fn(|i| { let mut id=[u8::MAX;16]; id[0]=(i+1) as u8; id });
        o.baseline_sources=std::array::from_fn(|i| Some(BaselineSourceRegistration { id:source_ids[i], authority_id:[u8::MAX;16], scope,
            population_origin:o.population_origin, member_version:1, valid_from:GameTime::new(0).unwrap(), valid_until:max }));
        let courses=[Course::Electric,Course::Water,Course::Electric,Course::Water];
        let rule_ids=[[252;16],[251;16],[250;16],[249;16]];
        let entitlements=[[248;16],[247;16],[246;16],[245;16]];
        o.rules=std::array::from_fn(|i| Some(RuleRegistration { id:rule_ids[i], issuer_id:[254;16], course:courses[i], syllabus_mask:15, minimum_covered_parts:4 }));
        o.baselines=std::array::from_fn(|i| { let end=u64::MAX-3-(i as u64)*2;
            Some(BaselineFact { members:MemberRange{start:end-1,end}, source_id:source_ids[i], base_known:false,
                base_mask:BaseQualification::Electric.bit(), background:None, skill:None }) });
        o.course_results=std::array::from_fn(|i| Some(CourseResultRegistration { course:courses[i], cohort_id:u64::MAX-i as u64,
            generation:u64::MAX, members, roster_hash:[u8::MAX;32], rule_id:rule_ids[i], issuer_id:[254;16], free_entitlement_id:entitlements[i] }));
        let hash=opening_fingerprint(&o).unwrap();
        let genesis=ValidatedGenesis::from_root(scope,o.population_origin,1,u64::MAX,hash,[Some(members);4],
            [Some(entitlements[0]),Some(entitlements[1]),Some(entitlements[2]),Some(entitlements[3])],
            [u64::MAX,u64::MAX-1,u64::MAX-2,u64::MAX-3],[u64::MAX;4],[[u8::MAX;32];4]);
        match State::new(o,&genesis) {
            Err(Error::Capacity)=>{},
            Err(e)=>panic!("unexpected admission error: {e:?}"),
            Ok(s)=>{
                assert!(s.terminal_state_upper_bound_bytes().unwrap()<=MAX_STATE_BYTES);
                let event=ValidatedExamEvent::from_root(0,max,[250;32],members,u64::MAX,u64::MAX,[u8::MAX;32],Course::Electric,
                    [252;16],[254;16],15,ExamOutcome::Pass,[249;32]);
                let exam=s.prepare_record_examination(event,1,[248;16]).unwrap();
                let grant=RootApprovedCompletion::from_root(0,max,[247;32],max,[250;32],[249;32],exam.next.completion_key(0).unwrap(),[246;16]);
                let completed=exam.next.prepare_grant(grant).unwrap();
                let revoke=RootApprovedRevocation::from_root(0,max,[245;16],[244;32]).unwrap();
                let revoked=completed.next.prepare_revoke(revoke).unwrap();
                assert!(revoked.next.serialized_len().unwrap()<=MAX_STATE_BYTES);
                let refs=revoked.next.needed_receipts();
                assert_eq!(refs[0].unwrap().operation_id,[248;16]);
                assert_eq!(refs[1].unwrap().operation_id,[246;16]);
                assert_eq!(refs[2].unwrap().operation_id,[245;16]);
                assert!(refs[3].is_none());
            }
        }
    }
}
