//! Independent literal arithmetic oracles; injections do not certify actual clock readings.
use super::*;

fn known(ns: u64) -> Nanoseconds { Nanoseconds::Known { ns } }
fn ordinary() -> WorkCounters {
    WorkCounters { sources: 3, cells: 0, integrated_cells: 0, segments: 0, pages: 1 }
}
fn integration() -> WorkCounters {
    WorkCounters { sources: 0, cells: 8, integrated_cells: 1024, segments: 1, pages: 1 }
}

#[test]
fn literal_sum_max_and_terminal_groups_keep_distinct_work_units() {
    let mut s = Sampler::new();
    s.record(ordinary(), known(7), known(3));
    s.record(ordinary(), known(11), known(5));
    s.record(integration(), known(29), known(17));
    let summary = s.summary();
    assert_eq!(summary.validate(), Ok(()));
    assert_eq!(summary.work, WorkCounters { sources: 6, cells: 8, integrated_cells: 1024,
        segments: 1, pages: 3 });
    let t = summary.turn.unwrap();
    assert_eq!(t.ordinary.pages, 2);
    assert_eq!(t.integration.pages, 1);
    assert_eq!(t.wall, Axis { total: known(47), max_page: known(29) });
    assert_eq!(t.cpu, Axis { total: known(25), max_page: known(17) });
    assert_eq!(t.ordinary.wall, Axis { total: known(18), max_page: known(11) });
    assert_eq!(t.ordinary.cpu, Axis { total: known(8), max_page: known(5) });
    assert_eq!(t.integration.wall, Axis { total: known(29), max_page: known(29) });
}

#[test]
fn missing_cpu_does_not_erase_wall_or_other_group() {
    let missing = Nanoseconds::Unknown { reason: UnknownReason::CpuUnavailable(CpuUnknown::ClockReadFailed) };
    let mut s = Sampler::new();
    s.record(ordinary(), known(7), missing);
    s.record(integration(), known(29), known(17));
    let summary = s.summary();
    assert_eq!(summary.validate(), Ok(()));
    let t = summary.turn.unwrap();
    assert_eq!(t.wall, Axis { total: known(36), max_page: known(29) });
    assert_eq!(t.cpu.total, missing);
    assert_eq!(t.cpu.max_page, missing);
    assert_eq!(t.integration.cpu.total, known(17));
    assert_eq!(t.work.pages, 2);
}

#[test]
fn wall_unknown_does_not_erase_cpu_and_no_later_page_repairs_unknown() {
    let missing = Nanoseconds::Unknown { reason: UnknownReason::ClockWentBackwards };
    let mut s = Sampler::new();
    s.record(ordinary(), missing, known(3));
    s.record(ordinary(), known(11), known(5));
    let summary = s.summary();
    assert_eq!(summary.validate(), Ok(()));
    let t = summary.turn.unwrap();
    assert_eq!(t.wall.total, missing);
    assert_eq!(t.wall.max_page, missing);
    assert_eq!(t.cpu, Axis { total: known(8), max_page: known(5) });
}

#[test]
fn backward_cpu_failed_end_and_duration_overflow_are_explicit() {
    assert_eq!(cpu_difference(Ok(20), Ok(19)), Nanoseconds::Unknown {
        reason: UnknownReason::ClockWentBackwards });
    assert_eq!(cpu_difference(Ok(20), Ok(27)), known(7));
    assert_eq!(cpu_difference(Err(CpuUnknown::UnsupportedPlatform), Ok(27)),
        Nanoseconds::Unknown { reason: UnknownReason::CpuUnavailable(CpuUnknown::UnsupportedPlatform) });
    assert_eq!(cpu_difference(Ok(20), Err(CpuUnknown::ClockReadFailed)),
        Nanoseconds::Unknown { reason: UnknownReason::CpuUnavailable(CpuUnknown::ClockReadFailed) });
    assert_eq!(nanos(u128::from(u64::MAX) + 1), Nanoseconds::Unknown {
        reason: UnknownReason::NanosecondOverflow });
}

#[test]
fn sum_overflow_keeps_known_max_and_other_axis() {
    let mut s = Sampler::new();
    s.record(ordinary(), known(u64::MAX), known(3));
    s.record(ordinary(), known(1), known(5));
    let summary = s.summary();
    assert_eq!(summary.validate(), Ok(()));
    let t = summary.turn.unwrap();
    assert_eq!(t.wall.total, Nanoseconds::Unknown { reason: UnknownReason::NanosecondOverflow });
    assert_eq!(t.wall.max_page, known(u64::MAX));
    assert_eq!(t.cpu.total, known(8));
}

#[test]
fn empty_and_completed_noop_are_explicit_empty_sums() {
    let mut s = Sampler::new();
    assert_eq!(s.summary().validate(), Ok(()));
    assert_eq!(s.summary().turn, Some(Turn::EMPTY));
    s.record(WorkCounters::ZERO, known(900), known(800));
    assert_eq!(s.summary().turn, Some(Turn::EMPTY));
    s.record(integration(), known(29), known(17));
    let t = s.summary().turn.unwrap();
    assert_eq!(t.ordinary, Group::EMPTY);
    assert_eq!(t.integration.pages, 1);
    assert_eq!(s.summary().validate(), Ok(()));
}

#[test]
fn reset_does_not_cross_turn_and_frozen_final_copy_does_not_change() {
    let mut s = Sampler::new();
    s.record(integration(), known(29), known(17));
    let frozen = s.summary();
    s.reset();
    assert_eq!(s.summary().turn, Some(Turn::EMPTY));
    s.record(ordinary(), known(7), known(3));
    assert_eq!(s.summary().work.pages, 1);
    assert_eq!(s.summary().turn.unwrap().wall.total, known(7));
    assert_eq!(frozen.work.segments, 1);
    assert_eq!(frozen.turn.unwrap().wall.total, known(29));
}

#[test]
fn validation_rejects_count_group_max_sum_and_false_overflow_shapes() {
    let mut s = Sampler::new(); s.record(ordinary(), known(7), known(3));
    let good = s.summary();
    let mut bad = good; bad.work.pages = 2;
    assert_eq!(bad.validate(), Err(ValidationError::Shape));
    let mut bad = good; bad.turn.as_mut().unwrap().ordinary.pages = 2;
    assert_eq!(bad.validate(), Err(ValidationError::Shape));
    let mut bad = good; bad.turn.as_mut().unwrap().wall.max_page = known(8);
    assert_eq!(bad.validate(), Err(ValidationError::Shape));
    let mut bad = good;
    let t = bad.turn.as_mut().unwrap();
    t.ordinary.wall = Axis { total: known(14), max_page: known(7) }; t.rebuild_axes();
    assert_eq!(bad.validate(), Err(ValidationError::Shape));
    let mut bad = good;
    let t = bad.turn.as_mut().unwrap();
    t.ordinary.wall.total = Nanoseconds::Unknown { reason: UnknownReason::NanosecondOverflow };
    t.rebuild_axes();
    assert_eq!(bad.validate(), Err(ValidationError::Shape));
    let mut bad = good; bad.version = 2;
    assert_eq!(bad.validate(), Err(ValidationError::Version));
    let mut bad = good; bad.unavailable = Some(UnavailableReason::InvalidPageWork);
    assert_eq!(bad.validate(), Err(ValidationError::Shape));
}

#[test]
fn invalid_page_counts_terminal_shape_and_turn_bound_are_unavailable_not_business_errors() {
    let mut s = Sampler::new();
    let mut bad = ordinary(); bad.pages = 2;
    s.record(bad, known(7), known(3));
    assert_eq!(s.summary().unavailable, Some(UnavailableReason::InvalidPageWork));
    assert_eq!(s.summary().turn, None);
    assert_eq!(s.summary().validate(), Err(ValidationError::Unavailable));
    s.reset();
    let mut bad = ordinary(); bad.integrated_cells = 1024;
    s.record(bad, known(7), known(3));
    assert_eq!(s.summary().unavailable, Some(UnavailableReason::InvalidPageWork));
    s.reset();
    for _ in 0..16 { s.record(ordinary(), known(7), known(3)); }
    assert_eq!(s.summary().validate(), Ok(()));
    s.record(ordinary(), known(7), known(3));
    assert_eq!(s.summary().unavailable, Some(UnavailableReason::TurnLimit));
}

#[test]
fn checked_count_overflow_and_reset_overflow_fail_closed_for_observation_only() {
    let mut s = Sampler::new(); s.turn.work.sources = u64::MAX;
    s.record(ordinary(), known(7), known(3));
    assert_eq!(s.summary().unavailable, Some(UnavailableReason::CountOverflow));
    s.epoch = u64::MAX; s.reset();
    assert_eq!(s.summary().unavailable, Some(UnavailableReason::ResetOverflow));
}

#[test]
fn token_cannot_append_after_reset() {
    // Exercises production clock capture only for stale-token rejection; no elapsed
    // measurement is asserted and this test is not timing/calibration acceptance.
    let mut s = Sampler::new();
    let token = s.begin_page(); s.reset(); s.end_page(token, ordinary());
    assert_eq!(s.summary().unavailable, Some(UnavailableReason::StaleToken));
}

#[test]
fn serde_rejects_unknown_fields_and_summary_roundtrip_is_structural_only() {
    let mut s = Sampler::new(); s.record(integration(), known(29), known(17));
    let mut json = serde_json::to_value(s.summary()).unwrap();
    let restored: Summary = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(restored, s.summary());
    json["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Summary>(json).is_err());
    let mut json = serde_json::to_value(s.summary()).unwrap();
    json["turn"]["wall"]["max_page"]["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Summary>(json).is_err());
}
