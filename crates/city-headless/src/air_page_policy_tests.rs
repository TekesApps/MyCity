//! Literal arithmetic/state oracles only; not measured performance or auto-execution.
use super::*;
use crate::air_page_metrics::{Axis, Group, Turn, UnknownReason};
use crate::air_turns::WorkCounters;

fn quota(pages: u16) -> Quota { Quota { source_limit: 16, cell_limit: 32, pages } }
fn config() -> Config { Config { target_ns: 100 } }
fn ns(value: u64) -> Nanoseconds { Nanoseconds::Known { ns: value } }
fn axis(total: u64, max: u64) -> Axis { Axis { total: ns(total), max_page: ns(max) } }
fn empty() -> Group { Group { pages: 0, wall: axis(0, 0), cpu: axis(0, 0) } }
fn summary(pages: u16, total: u64, phase: Phase) -> Summary {
    // Exact distribution fixture: max=ceil(total/pages) except terminal group
    // where one real integration page has the entire declared duration.
    let count = u64::from(pages);
    let max = total / count + u64::from(total % count != 0);
    let mut work = WorkCounters { sources: 0, cells: 0, integrated_cells: 0, segments: 0, pages: count };
    let mut ordinary = Group { pages: count, wall: axis(total, max), cpu: axis(count, 1) };
    let mut integration = empty();
    match phase {
        Phase::Sources => work.sources = count,
        Phase::Cells => work.cells = count,
        Phase::Mixed => { work.sources = count; work.cells = count; }
        Phase::Integration => {
            work.cells = count; work.integrated_cells = 1024; work.segments = 1;
            ordinary = Group { pages: count - 1, wall: axis(0, 0),
                cpu: if count > 1 { axis(count - 1, 1) } else { axis(0, 0) } };
            integration = Group { pages: 1, wall: axis(total, total), cpu: axis(1, 1) };
        }
    }
    let wall = if phase == Phase::Integration { axis(total, total) } else { axis(total, max) };
    let turn = Turn { work, ordinary, integration, wall, cpu: axis(count, 1) };
    let s = Summary { version: 1, work, turn: Some(turn), unavailable: None };
    assert_eq!(s.validate(), Ok(()));
    s
}

#[test]
fn literal_up_hold_down_are_single_rung_with_two_qualifying_turns() {
    let mut p = Policy::new(quota(2), config()).unwrap();
    let low = summary(2, 40, Phase::Sources);
    let d = p.decide(Some(&low));
    assert_eq!((d.reason, d.next_quota.pages), (Reason::PhaseChanged, 2));
    let d = p.decide(Some(&low));
    assert_eq!((d.reason, d.current_quota.pages, d.next_quota.pages), (Reason::StepUp, 2, 4));
    assert_eq!(p.decide(Some(&summary(4, 80, Phase::Sources))).reason, Reason::Hold);
    let high = summary(4, 120, Phase::Sources);
    assert_eq!(p.decide(Some(&high)).next_quota.pages, 4);
    let d = p.decide(Some(&high));
    assert_eq!((d.reason, d.next_quota.pages), (Reason::StepDown, 2));
    assert!(d.turn_overrun && d.still_unresolved);
    assert_eq!((d.next_quota.source_limit, d.next_quota.cell_limit), (16, 32));
}

#[test]
fn new_quota_restarts_hold_and_middle_band_breaks_streak() {
    let mut p = Policy::new(quota(1), config()).unwrap();
    p.decide(Some(&summary(1, 20, Phase::Sources)));
    assert_eq!(p.decide(Some(&summary(1, 20, Phase::Sources))).next_quota.pages, 2);
    assert_eq!(p.decide(Some(&summary(2, 40, Phase::Sources))).next_quota.pages, 2);
    assert_eq!(p.decide(Some(&summary(2, 50, Phase::Sources))).reason, Reason::Hold);
    assert_eq!(p.decide(Some(&summary(2, 40, Phase::Sources))).reason, Reason::LowStreak);
    assert_eq!(p.decide(Some(&summary(2, 40, Phase::Sources))).next_quota.pages, 4);
}

#[test]
fn phase_change_discards_old_trend_including_mixed_and_integration() {
    let mut p = Policy::new(quota(2), config()).unwrap();
    for phase in [Phase::Sources, Phase::Cells, Phase::Integration, Phase::Mixed] {
        let d = p.decide(Some(&summary(2, 40, phase)));
        assert_eq!(d.reason, Reason::PhaseChanged);
        assert_eq!(d.next_quota.pages, 2);
    }
    assert_eq!(p.decide(Some(&summary(2, 40, Phase::Mixed))).next_quota.pages, 4);
}

#[test]
fn missing_unknown_zero_and_invalid_are_explicit_conservative_one_page() {
    let mut p = Policy::new(quota(16), config()).unwrap();
    let d = p.decide(None);
    assert_eq!((d.reason, d.next_quota.pages, d.known_sample), (Reason::MissingSummary, 1, None));
    let d = p.decide(Some(&summary(1, 0, Phase::Sources)));
    assert_eq!((d.reason, d.next_quota.pages), (Reason::ZeroWall, 1));
    let mut unknown = summary(1, 20, Phase::Sources);
    let value = Nanoseconds::Unknown { reason: UnknownReason::ClockWentBackwards };
    let t = unknown.turn.as_mut().unwrap();
    t.wall = Axis { total: value, max_page: value }; t.ordinary.wall = t.wall;
    assert_eq!(unknown.validate(), Ok(()));
    assert_eq!(p.decide(Some(&unknown)).reason, Reason::UnknownWall);
    unknown.version = 99;
    assert_eq!(p.decide(Some(&unknown)).reason, Reason::InvalidSummary);
    assert_eq!(p.current_quota(), quota(1));
}

#[test]
fn cpu_unknown_does_not_replace_wall_or_block_needed_continuation() {
    let mut p = Policy::new(quota(1), config()).unwrap();
    let mut s = summary(1, 20, Phase::Sources);
    let value = Nanoseconds::Unknown { reason: UnknownReason::ClockWentBackwards };
    let t = s.turn.as_mut().unwrap(); t.cpu = Axis { total: value, max_page: value }; t.ordinary.cpu = t.cpu;
    assert_eq!(s.validate(), Ok(()));
    p.decide(Some(&s));
    let d = p.decide(Some(&s));
    assert_eq!(d.next_quota.pages, 2);
    assert_eq!(d.known_sample.unwrap().cpu_total, value);
}

#[test]
fn one_page_overrun_and_integration_overrun_do_not_claim_a_hard_deadline() {
    let mut p = Policy::new(quota(1), config()).unwrap();
    let s = summary(1, 101, Phase::Integration);
    let d = p.decide(Some(&s));
    assert!(d.turn_overrun && d.integration_overrun && d.irreducible_overrun);
    assert_eq!(d.known_sample.unwrap().integration_max_page_wall_ns, 101);
    let d = p.decide(Some(&s));
    assert_eq!((d.reason, d.next_quota.pages), (Reason::AtMinimum, 1));
    assert!(d.still_unresolved);
}

#[test]
fn integration_cost_uses_real_max_not_ordinary_average() {
    let mut p = Policy::new(quota(4), config()).unwrap();
    let s = summary(4, 120, Phase::Integration); // ordinary total0, one integral page120.
    let d = p.decide(Some(&s));
    assert!(d.integration_overrun && d.irreducible_overrun);
    assert_eq!(d.known_sample.unwrap().max_page_wall_ns, 120);
    assert_eq!(p.decide(Some(&s)).next_quota.pages, 2);
}

#[test]
fn retarget_is_explicit_versioned_atomic_and_resets_trend() {
    let mut p = Policy::new(quota(2), config()).unwrap();
    p.decide(Some(&summary(2, 40, Phase::Sources)));
    p.retarget(Config { target_ns: 200 }).unwrap();
    let d = p.decide(Some(&summary(2, 40, Phase::Sources)));
    assert_eq!((d.target_version, d.reason, d.next_quota.pages), (2, Reason::PhaseChanged, 2));
    assert_eq!(d.config.target_ns, 200);
    assert_eq!(p.retarget(Config { target_ns: 0 }), Err(Error::InvalidTarget));
    assert_eq!(p.target_version(), 2);
    p.target_version = u64::MAX;
    assert_eq!(p.retarget(config()), Err(Error::TargetVersionOverflow));
    assert_eq!(p.config.target_ns, 200);
    assert_eq!(p.target_version(), u64::MAX);
}

#[test]
fn targets_ladder_endpoints_and_huge_positive_sample_do_not_overflow() {
    assert!(matches!(Policy::new(quota(1), Config { target_ns: 0 }), Err(Error::InvalidTarget)));
    assert!(matches!(Policy::new(quota(1), Config { target_ns: 1_000_000_001 }), Err(Error::InvalidTarget)));
    assert!(Policy::new(quota(1), Config { target_ns: 1 }).is_ok());
    assert!(Policy::new(quota(16), Config { target_ns: 1_000_000_000 }).is_ok());
    assert!(matches!(Policy::new(quota(3), config()), Err(Error::InvalidQuota)));
    assert!(matches!(Policy::new(quota(0), config()), Err(Error::InvalidQuota)));
    let mut p = Policy::new(quota(16), config()).unwrap();
    let low = summary(16, 16, Phase::Cells);
    p.decide(Some(&low));
    assert_eq!(p.decide(Some(&low)).reason, Reason::AtMaximum);
    let mut p = Policy::new(quota(1), config()).unwrap();
    let huge = summary(1, u64::MAX, Phase::Cells);
    p.decide(Some(&huge));
    let d = p.decide(Some(&huge));
    assert_eq!(d.reason, Reason::AtMinimum);
    assert!(d.irreducible_overrun);
}

#[test]
fn nonterminal_quota_mismatch_and_empty_turn_cannot_drive_upgrade() {
    let mut p = Policy::new(quota(4), config()).unwrap();
    assert_eq!(p.decide(Some(&summary(2, 20, Phase::Cells))).reason, Reason::QuotaMismatch);
    let z = Axis { total: ns(0), max_page: ns(0) };
    let t = Turn { work: WorkCounters::ZERO, ordinary: empty(), integration: empty(), wall: z, cpu: z };
    let s = Summary { version: 1, work: t.work, turn: Some(t), unavailable: None };
    assert_eq!(p.decide(Some(&s)).reason, Reason::EmptySummary);
    assert_eq!(p.current_quota().pages, 1);
    let mut narrow = quota(1); narrow.cell_limit = 1;
    let mut p = Policy::new(narrow, config()).unwrap();
    let mut s = summary(1, 20, Phase::Cells);
    s.work.cells = 2; s.turn.as_mut().unwrap().work = s.work;
    assert_eq!(p.decide(Some(&s)).reason, Reason::QuotaMismatch);
}

#[test]
fn decision_identity_and_serde_are_fixed_size_structural_evidence_only() {
    let mut p = Policy::new(quota(1), config()).unwrap();
    let d = p.decide(None);
    assert_eq!(d.policy, PolicyName::ExactAutoPagesV1);
    assert_eq!(d.policy_version, 1);
    let mut json = serde_json::to_value(d).unwrap();
    assert_eq!(serde_json::from_value::<Decision>(json.clone()).unwrap(), d);
    json["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Decision>(json).is_err());
}

#[test]
fn pending_clone_retarget_and_final_preview_do_not_consume_base_sample() {
    let mut base = Policy::new(quota(4), config()).unwrap();
    let s = summary(4, 40, Phase::Cells);
    let mut pending = base.clone();
    assert_eq!(pending.decide(Some(&s)).next_quota.pages, 4);
    assert_eq!(base.phase, None);
    assert_eq!(base.low, 0);
    base.retarget(Config { target_ns: 200 }).unwrap();
    let mut revised_pending = base.clone();
    let decision = revised_pending.decide(Some(&s));
    assert_eq!((decision.target_version, decision.reason), (2, Reason::PhaseChanged));
    assert_eq!(base.low, 0);
    let final_s = summary(1, 201, Phase::Integration);
    let final_d = base.preview_final(Some(&final_s));
    assert_eq!(final_d.reason, Reason::FinalObservation);
    assert!(final_d.integration_overrun && final_d.irreducible_overrun);
    assert_eq!((final_d.current_quota.pages, final_d.next_quota.pages), (4, 4));
    assert_eq!(base.phase, None);
    assert_eq!(base.low, 0);
}
