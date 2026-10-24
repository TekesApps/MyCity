//! Source-bound, bounded, read-only job hard gates for exact members.
//! This result is not a credential, hiring authority, or durable eligibility cache.
//! The caller must obtain both expected source and population size from the current
//! original writer, and recheck at the actual transaction boundary.
use crate::member_qualification::{Background, MemberRange, ReadSnapshot, ReadSource, Skill};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Request {
    pub members: MemberRange,
    /// Original population bound, not a count inferred from projection rows.
    pub total_members: u64,
    pub minimum_background: Option<Background>,
    pub minimum_skill: Option<Skill>,
    /// Uses the original C48 eight-bit numbering without reinterpretation.
    pub required_base_mask: u8,
    /// Opaque finite provider bits. No industry meaning is invented here.
    pub required_endorsement_mask: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status { Eligible, Ineligible, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceField { Scope, PopulationOrigin, MemberVersion, Root, Revision, AsOf }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    Satisfied,
    InvalidSource(SourceField),
    SourceMismatch(SourceField),
    InvalidRequest,
    RequestedMembersMismatch,
    InvalidRow { row: usize },
    NonContiguousRows { row: usize },
    OverlapOrUnsorted { row: usize },
    CoverageGap { members: MemberRange },
    IncompleteProjection,
    UnknownBackground { members: MemberRange },
    UnknownSkill { members: MemberRange },
    UnknownBase { members: MemberRange, required: u8 },
    UnknownEndorsement { members: MemberRange, required: u64 },
    BackgroundBelowMinimum { members: MemberRange, actual: Background, required: Background },
    SkillBelowMinimum { members: MemberRange, actual: Skill, required: Skill },
    MissingBase { members: MemberRange, missing: u8 },
    MissingEndorsement { members: MemberRange, missing: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CheckResult {
    pub source: ReadSource,
    pub expected_source: ReadSource,
    pub request: Request,
    pub status: Status,
    /// Deterministic first witness: row order, then background/skill/base/endorsement.
    /// A known failure takes precedence over unknown capabilities only after exact
    /// source and full coverage validation. This is never a worker shortage count.
    pub reason: Reason,
}

fn invalid_source(source: ReadSource) -> Option<SourceField> {
    if source.scope.world() == 0 || source.scope.branch() == 0 { return Some(SourceField::Scope); }
    if source.population_origin == [0; 32] { return Some(SourceField::PopulationOrigin); }
    if source.member_version == 0 { return Some(SourceField::MemberVersion); }
    if source.root == [0; 32] { return Some(SourceField::Root); }
    // Revision zero and the epoch tick are legitimate initial state values.
    None
}

fn mismatch(actual: ReadSource, expected: ReadSource) -> Option<SourceField> {
    if actual.scope != expected.scope { return Some(SourceField::Scope); }
    if actual.population_origin != expected.population_origin { return Some(SourceField::PopulationOrigin); }
    if actual.member_version != expected.member_version { return Some(SourceField::MemberVersion); }
    if actual.root != expected.root { return Some(SourceField::Root); }
    if actual.revision != expected.revision { return Some(SourceField::Revision); }
    if actual.as_of != expected.as_of { return Some(SourceField::AsOf); }
    None
}

/// At most MAX_READ_ROWS rows and four capability checks per row; no allocation,
/// per-person iteration, mutation, clock advance, or persistent cache.
pub(crate) fn check(snapshot: &ReadSnapshot, expected: ReadSource, request: Request) -> CheckResult {
    let result = |status, reason| CheckResult {
        source: snapshot.source, expected_source: expected, request, status, reason,
    };
    let unknown = |reason| result(Status::Unknown, reason);
    if let Some(field) = invalid_source(expected).or_else(|| invalid_source(snapshot.source)) {
        return unknown(Reason::InvalidSource(field));
    }
    if let Some(field) = mismatch(snapshot.source, expected) {
        return unknown(Reason::SourceMismatch(field));
    }
    if request.members.start >= request.members.end || request.members.end > request.total_members {
        return unknown(Reason::InvalidRequest);
    }
    if snapshot.requested != request.members { return unknown(Reason::RequestedMembersMismatch); }

    // Validate the entire projection before any negative capability conclusion.
    // `complete` is an assertion to verify, never a substitute for exact coverage.
    let mut cursor = request.members.start;
    let mut ended = false;
    for (index, slot) in snapshot.rows.iter().enumerate() {
        let Some(row) = slot else { ended = true; continue; };
        if ended { return unknown(Reason::NonContiguousRows { row: index }); }
        let members = row.members;
        if members.start >= members.end || members.start < request.members.start
            || members.end > request.members.end || members.end > request.total_members {
            return unknown(Reason::InvalidRow { row: index });
        }
        if members.start < cursor { return unknown(Reason::OverlapOrUnsorted { row: index }); }
        if members.start > cursor {
            return unknown(Reason::CoverageGap { members: MemberRange { start: cursor, end: members.start } });
        }
        cursor = members.end;
    }
    if cursor != request.members.end {
        return unknown(Reason::CoverageGap { members: MemberRange { start: cursor, end: request.members.end } });
    }
    if !snapshot.complete { return unknown(Reason::IncompleteProjection); }

    let mut failure = None;
    let mut unresolved = None;
    for row in snapshot.rows.iter().flatten() {
        let members = row.members;
        if let Some(required) = request.minimum_background {
            match row.background {
                None => { unresolved.get_or_insert(Reason::UnknownBackground { members }); }
                Some(actual) if actual.rank() < required.rank() => {
                    failure.get_or_insert(Reason::BackgroundBelowMinimum { members, actual, required });
                }
                Some(_) => {}
            }
        }
        if let Some(required) = request.minimum_skill {
            match row.skill {
                None => { unresolved.get_or_insert(Reason::UnknownSkill { members }); }
                Some(actual) if actual.rank() < required.rank() => {
                    failure.get_or_insert(Reason::SkillBelowMinimum { members, actual, required });
                }
                Some(_) => {}
            }
        }
        if request.required_base_mask != 0 {
            let missing = request.required_base_mask & !row.base_mask;
            if missing != 0 {
                if row.base_known { failure.get_or_insert(Reason::MissingBase { members, missing }); }
                else { unresolved.get_or_insert(Reason::UnknownBase { members, required: missing }); }
            }
        }
        if request.required_endorsement_mask != 0 {
            // Set bits are sourced positive facts. The known flag describes only
            // absent bits; unrelated unknown endorsements do not defeat positives.
            let missing = request.required_endorsement_mask & !row.endorsement_mask;
            if missing != 0 {
                if row.endorsements_known { failure.get_or_insert(Reason::MissingEndorsement { members, missing }); }
                else { unresolved.get_or_insert(Reason::UnknownEndorsement { members, required: missing }); }
            }
        }
    }
    if let Some(reason) = failure { result(Status::Ineligible, reason) }
    else if let Some(reason) = unresolved { unknown(reason) }
    else { result(Status::Eligible, Reason::Satisfied) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arena::Scope, member_qualification::{BaseQualification, CapabilityRow, MAX_READ_ROWS}, units::GameTime};

    fn source() -> ReadSource {
        ReadSource { scope: Scope::new(1, 2).unwrap(), population_origin: [3; 32], member_version: 1,
            root: [4; 32], revision: 0, as_of: GameTime::ZERO }
    }
    fn request() -> Request {
        Request { members: MemberRange { start: 10, end: 20 }, total_members: 100,
            minimum_background: Some(Background::General), minimum_skill: Some(Skill::Technical),
            required_base_mask: BaseQualification::Electric.bit(), required_endorsement_mask: 0 }
    }
    fn row(start: u64, end: u64, base_mask: u8) -> CapabilityRow {
        CapabilityRow { members: MemberRange { start, end }, background: Some(Background::General),
            skill: Some(Skill::Technical), base_known: true, base_mask,
            endorsements_known: false, endorsement_mask: 0 }
    }
    fn snapshot() -> ReadSnapshot {
        let mut rows = [None; MAX_READ_ROWS];
        rows[0] = Some(row(10, 20, BaseQualification::Electric.bit()));
        ReadSnapshot { source: source(), requested: request().members, rows, complete: true }
    }
    fn status(snapshot: &ReadSnapshot) -> Status { check(snapshot, source(), request()).status }

    #[test]
    fn synthetic_before_after_electric_water_and_teacher_are_distinct() {
        let mut s = snapshot();
        for mask in [0, BaseQualification::Water.bit(), BaseQualification::Teaching.bit()] {
            s.rows[0].as_mut().unwrap().base_mask = mask;
            assert_eq!(check(&s, source(), request()).reason,
                Reason::MissingBase { members: request().members, missing: BaseQualification::Electric.bit() });
            assert_eq!(status(&s), Status::Ineligible);
        }
        s.rows[0].as_mut().unwrap().base_mask = BaseQualification::Electric.bit();
        assert_eq!(status(&s), Status::Eligible);
        // No actual Store/grant is exercised by this synthetic fixture.
    }

    #[test]
    fn every_member_and_every_required_bit_must_pass() {
        let mut s = snapshot();
        s.rows[0] = Some(row(10, 19, u8::MAX));
        s.rows[1] = Some(row(19, 20, BaseQualification::Water.bit()));
        assert_eq!(check(&s, source(), request()).reason,
            Reason::MissingBase { members: MemberRange { start: 19, end: 20 }, missing: BaseQualification::Electric.bit() });
        let mut r = request();
        r.required_base_mask |= BaseQualification::Water.bit();
        s.rows[1] = Some(row(19, 20, BaseQualification::Electric.bit()));
        assert_eq!(check(&s, source(), r).reason,
            Reason::MissingBase { members: MemberRange { start: 19, end: 20 }, missing: BaseQualification::Water.bit() });
    }

    #[test]
    fn endorsements_need_original_provider_and_do_not_follow_base_bits() {
        let mut s = snapshot();
        let mut r = request();
        // Opaque synthetic bits representing two future provider requirements.
        for required in [1, 2, 1u64 << 63] {
            r.required_endorsement_mask = required;
            s.rows[0].as_mut().unwrap().endorsement_mask = 0;
            assert!(matches!(check(&s, source(), r).reason, Reason::UnknownEndorsement { .. }));
            s.rows[0].as_mut().unwrap().endorsement_mask = required;
            assert_eq!(check(&s, source(), r).status, Status::Eligible);
            s.rows[0].as_mut().unwrap().endorsements_known = true;
            s.rows[0].as_mut().unwrap().endorsement_mask = 0;
            assert!(matches!(check(&s, source(), r).reason, Reason::MissingEndorsement { .. }));
            s.rows[0].as_mut().unwrap().endorsements_known = false;
        }
    }

    #[test]
    fn source_mismatch_including_stale_and_future_is_unknown() {
        let mut s = snapshot();
        let mut variants = [source(); 7];
        variants[0].scope = Scope::new(1, 3).unwrap();
        variants[1].population_origin = [5; 32];
        variants[2].member_version += 1;
        variants[3].root = [6; 32];
        variants[4].revision += 1;
        variants[5].as_of = GameTime::new(1).unwrap();
        variants[6].population_origin = [0; 32];
        for wrong in variants { s.source = wrong; assert_eq!(status(&s), Status::Unknown); }
        s.source = source();
        for wrong in variants { assert_eq!(check(&s, wrong, request()).status, Status::Unknown); }
        s.source.root = [0; 32];
        assert_eq!(status(&s), Status::Unknown);
        s = snapshot(); s.source.member_version = 0;
        assert_eq!(status(&s), Status::Unknown);
    }

    #[test]
    fn malformed_or_incomplete_coverage_never_becomes_shortage() {
        let mut cases = [snapshot(); 10];
        cases[0].rows[0] = Some(row(10, 10, 0)); // empty
        cases[1].rows[0] = Some(row(20, 10, 0)); // reversed
        cases[2].rows[0] = Some(row(9, 20, 0)); // outside request
        cases[3].rows[0] = Some(row(10, 101, 0)); // outside population
        cases[4].rows[0] = Some(row(10, 15, 0)); // truncated despite complete=true
        cases[5].rows[0] = Some(row(10, 15, 0));
        cases[5].rows[1] = Some(row(16, 20, 0)); // interior gap
        cases[6].rows[1] = cases[6].rows[0]; // duplicate
        cases[7].rows[0] = Some(row(10, 16, 0));
        cases[7].rows[1] = Some(row(15, 20, 0)); // overlap
        cases[8].rows[2] = cases[8].rows[0]; // non-packed rows
        cases[9].complete = false; // budget flag, even if rows cover
        for s in cases { assert_eq!(status(&s), Status::Unknown); }
        let mut s = snapshot();
        s.rows[0] = Some(row(15, 20, 0));
        s.rows[1] = Some(row(10, 15, 0)); // unsorted
        assert_eq!(status(&s), Status::Unknown);
    }

    #[test]
    fn full_row_budget_is_accepted_only_with_exact_coverage() {
        let mut s = snapshot();
        let mut r = request();
        r.members.end = 26;
        s.requested = r.members;
        for (index, slot) in s.rows.iter_mut().enumerate() {
            let start = 10 + index as u64;
            *slot = Some(row(start, start + 1, BaseQualification::Electric.bit()));
        }
        assert_eq!(check(&s, source(), r).status, Status::Eligible);
        s.complete = false;
        assert_eq!(check(&s, source(), r).status, Status::Unknown);
        s.complete = true; s.rows[MAX_READ_ROWS - 1] = None;
        assert!(matches!(check(&s, source(), r).reason, Reason::CoverageGap { .. }));
    }

    #[test]
    fn empty_requirements_still_require_source_and_full_members() {
        let mut s = snapshot();
        let mut r = request();
        r.required_base_mask = 0; r.minimum_background = None; r.minimum_skill = None;
        let row = s.rows[0].as_mut().unwrap();
        row.background = None; row.skill = None; row.base_known = false;
        assert_eq!(check(&s, source(), r).status, Status::Eligible);
        s.rows[0] = None;
        assert_eq!(check(&s, source(), r).status, Status::Unknown);
        s = snapshot(); s.source.root = [7; 32];
        assert_eq!(check(&s, source(), r).status, Status::Unknown);
        s = snapshot(); r.members.end = r.members.start;
        assert_eq!(check(&s, source(), r).reason, Reason::InvalidRequest);
    }

    #[test]
    fn background_skill_and_unknown_facts_remain_separate() {
        let mut s = snapshot();
        let mut r = request();
        r.minimum_background = Some(Background::Higher);
        assert!(matches!(check(&s, source(), r).reason, Reason::BackgroundBelowMinimum { .. }));
        r = request(); s.rows[0].as_mut().unwrap().skill = Some(Skill::Basic);
        assert!(matches!(check(&s, source(), r).reason, Reason::SkillBelowMinimum { .. }));
        s = snapshot(); s.rows[0].as_mut().unwrap().background = None;
        assert_eq!(status(&s), Status::Unknown);
        s = snapshot(); s.rows[0].as_mut().unwrap().skill = None;
        assert_eq!(status(&s), Status::Unknown);
        s = snapshot(); s.rows[0].as_mut().unwrap().base_known = false;
        assert_eq!(status(&s), Status::Eligible);
        s.rows[0].as_mut().unwrap().base_mask = 0;
        assert_eq!(status(&s), Status::Unknown);
        s.rows[0].as_mut().unwrap().base_known = true;
        s.rows[0].as_mut().unwrap().base_mask = 0;
        assert_eq!(status(&s), Status::Ineligible);
    }

    #[test]
    fn deterministic_bound_result_and_no_side_effects() {
        let s = snapshot(); let before = s;
        let r = request();
        let first = check(&s, source(), r);
        assert_eq!(check(&s, source(), r), first);
        assert_eq!(s, before);
        assert_eq!(first.source, s.source);
        assert_eq!(first.expected_source, source());
        assert_eq!(first.request, r);
    }

    #[test]
    fn exact_boundaries_request_binding_and_population_bound() {
        let mut s = snapshot(); let mut r = request();
        r.members = MemberRange { start: 0, end: 100 };
        s.requested = r.members;
        s.rows[0] = Some(row(0, 50, BaseQualification::Electric.bit()));
        s.rows[1] = Some(row(50, 100, BaseQualification::Electric.bit()));
        assert_eq!(check(&s, source(), r).status, Status::Eligible);
        r.total_members = 99;
        assert_eq!(check(&s, source(), r).reason, Reason::InvalidRequest);
        r = request();
        assert_eq!(check(&s, source(), r).reason, Reason::RequestedMembersMismatch);
    }

    #[test]
    fn known_failure_witness_requires_complete_projection() {
        let mut s = snapshot();
        s.rows[0] = Some(row(10, 15, BaseQualification::Electric.bit()));
        s.rows[0].as_mut().unwrap().skill = None;
        s.rows[1] = Some(row(15, 20, 0));
        assert_eq!(check(&s, source(), request()).reason,
            Reason::MissingBase { members: MemberRange { start: 15, end: 20 }, missing: BaseQualification::Electric.bit() });
        assert_eq!(status(&s), Status::Ineligible);
        s.complete = false;
        assert_eq!(check(&s, source(), request()).reason, Reason::IncompleteProjection);
        assert_eq!(status(&s), Status::Unknown);
    }

    #[test]
    fn sourced_positive_bits_satisfy_only_requested_gates() {
        let mut s = snapshot(); let mut r = request();
        let electric = BaseQualification::Electric.bit();
        let water = BaseQualification::Water.bit();
        s.rows[0].as_mut().unwrap().base_known = false;
        assert_eq!(status(&s), Status::Eligible);
        r.required_base_mask = electric | water;
        assert_eq!(check(&s, source(), r).reason,
            Reason::UnknownBase { members: r.members, required: water });
        s.rows[0].as_mut().unwrap().endorsement_mask = 1;
        r.required_base_mask = electric;
        r.required_endorsement_mask = 3;
        assert_eq!(check(&s, source(), r).reason,
            Reason::UnknownEndorsement { members: r.members, required: 2 });
        r.required_endorsement_mask = 1;
        assert_eq!(check(&s, source(), r).status, Status::Eligible);
        s.complete = false;
        assert_eq!(check(&s, source(), r).status, Status::Unknown);
    }
}
