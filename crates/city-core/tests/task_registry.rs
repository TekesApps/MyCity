#[path = "../src/task_registry.rs"]
mod task_registry;

use task_registry::{
    evaluate, result_matches_current, CachedOutput, ClockInstant, ClockKind, ConsumerDemand,
    Outstanding, RequestCursor, ScopeIdentity, Snapshot, Source, TaskKey, WakeReason,
    WaitingReason, WorkWindow,
};

const CONTRACT_FIXTURE: &str = include_str!("../../../tests/fixtures/issue-101/sim-air-contract.json");
const SCOPE: ScopeIdentity = ScopeIdentity { world_id: 7, branch_id: 2, city_id: Some(11) };

fn source(id: u64, version: u64, due: Option<u64>, dirty: bool, necessary: bool, cursor: u64) -> Source {
    Source {
        scope: SCOPE,
        id,
        version,
        clock: ClockKind::World,
        due,
        dirty,
        necessary,
        window: WorkWindow { start: 0, end: 60 },
        cursor,
    }
}

fn snapshot<'a>(sources: &'a [Source], token: u64) -> Snapshot<'a> {
    Snapshot { scope: SCOPE, token, sources, demands: &[], caches: &[], outstanding: &[] }
}

#[test]
fn contract_fixture_is_parsed_and_compared_to_literal_expected_values() {
    let fixture: serde_json::Value = serde_json::from_str(CONTRACT_FIXTURE).unwrap();
    assert_eq!(fixture["fixture_kind"], "contract_only_not_provider_evidence");
    assert_eq!(fixture["input"]["new_air_source_count"], 0);
    let input = &fixture["input"];
    let scope = ScopeIdentity {
        world_id: input["world_id"].as_u64().unwrap(),
        branch_id: input["branch_id"].as_u64().unwrap(),
        city_id: Some(input["city_id"].as_u64().unwrap() as u32),
    };
    let source = Source {
        scope,
        id: input["source_id"].as_u64().unwrap(),
        version: input["source_version"].as_u64().unwrap(),
        clock: ClockKind::World,
        due: Some(input["due_world_seconds"].as_u64().unwrap()),
        dirty: input["dirty"].as_bool().unwrap(),
        necessary: input["necessary"].as_bool().unwrap(),
        window: WorkWindow { start: input["window_start_seconds"].as_u64().unwrap(), end: input["window_end_seconds"].as_u64().unwrap() },
        cursor: input["resume_cursor"].as_u64().unwrap(),
    };
    let token = input["snapshot_token"].as_u64().unwrap();
    let now = ClockInstant { kind: ClockKind::World, seconds: input["now_world_seconds"].as_u64().unwrap() };
    let input_sources = [source];
    let input_snapshot = Snapshot { scope, token, sources: &input_sources, demands: &[], caches: &[], outstanding: &[] };
    let page = evaluate(input_snapshot, now, RequestCursor::begin(scope, token, now), 8).unwrap();
    let expected = &fixture["expected"];
    assert_eq!(page.items.len(), expected["candidate_count"].as_u64().unwrap() as usize);
    assert_eq!(page.items[0].key.source_id, expected["source_id"].as_u64().unwrap());
    assert_eq!(page.items[0].key.source_version, expected["source_version"].as_u64().unwrap());
    assert_eq!(format!("{:?}", page.items[0].reason), expected["reason"].as_str().unwrap());
    assert_eq!(page.items[0].necessary, expected["necessary"].as_bool().unwrap());
    assert!(!expected["acknowledged"].as_bool().unwrap());
    assert!(!expected["committed"].as_bool().unwrap());
}

#[test]
fn different_periods_and_same_instant_are_independently_eligible() {
    let sources = [source(1, 1, Some(10), false, true, 0), source(2, 1, Some(20), false, true, 0), source(3, 1, Some(20), false, true, 0)];
    let now10 = ClockInstant { kind: ClockKind::World, seconds: 10 };
    let at_10 = evaluate(snapshot(&sources, 1), now10, RequestCursor::begin(SCOPE, 1, now10), 8).unwrap();
    assert_eq!(at_10.items.iter().map(|x| x.key.source_id).collect::<Vec<_>>(), [1]);
    let now20 = ClockInstant { kind: ClockKind::World, seconds: 20 };
    let at_20 = evaluate(snapshot(&sources, 1), now20, RequestCursor::begin(SCOPE, 1, now20), 8).unwrap();
    assert_eq!(at_20.items.iter().map(|x| x.key.source_id).collect::<Vec<_>>(), [1, 2, 3]);
}

#[test]
fn zero_new_source_air_interval_remains_dirty_and_snapshot_replay_is_literal() {
    let sources = [source(41, 7, Some(60), true, true, 0)];
    let now = ClockInstant { kind: ClockKind::World, seconds: 60 };
    let first = evaluate(snapshot(&sources, 4), now, RequestCursor::begin(SCOPE, 4, now), 8).unwrap();
    assert_eq!(first.items[0].reason, WakeReason::Dirty);
    let replay = evaluate(snapshot(&sources, 4), now, RequestCursor::begin(SCOPE, 4, now), 8).unwrap();
    assert_eq!(first, replay);
}

#[test]
fn cache_requires_current_version_sufficient_scope_and_fresh_age() {
    let sources = [source(1, 4, None, false, false, 0)];
    let demands = [ConsumerDemand { scope: SCOPE, consumer_id: 5, source_id: 1, clock: ClockKind::World, required_version: 4, window: WorkWindow { start: 10, end: 30 }, max_age_seconds: 5, necessary: true }];
    let good = [CachedOutput { scope: SCOPE, source_id: 1, source_version: 4, clock: ClockKind::World, observed_at: 95, coverage: WorkWindow { start: 0, end: 40 }, valid: true }];
    let now = ClockInstant { kind: ClockKind::World, seconds: 100 };
    let with_good = Snapshot { scope: SCOPE, token: 1, sources: &sources, demands: &demands, caches: &good, outstanding: &[] };
    assert!(evaluate(with_good, now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap().items.is_empty());
    let old = [CachedOutput { observed_at: 94, ..good[0] }];
    let with_old = Snapshot { caches: &old, ..with_good };
    assert_eq!(evaluate(with_old, now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap().items[0].reason, WakeReason::MissingOrStaleForConsumer);
    let narrow = [CachedOutput { coverage: WorkWindow { start: 11, end: 29 }, ..good[0] }];
    let with_narrow = Snapshot { caches: &narrow, ..with_good };
    assert_eq!(evaluate(with_narrow, now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap().items.len(), 1);
}

#[test]
fn world_interaction_and_city_development_clock_scopes_are_distinct() {
    let world_now = ClockInstant { kind: ClockKind::World, seconds: 500 };
    assert_eq!(world_now.age_since(ClockInstant { kind: ClockKind::Interaction, seconds: 499 }), None);
    assert_eq!(ClockInstant { kind: ClockKind::Interaction, seconds: 10 }.age_since(ClockInstant { kind: ClockKind::Interaction, seconds: 12 }), None);
    assert_ne!(ClockKind::Development { city_id: 11 }, ClockKind::Development { city_id: 12 });
}

#[test]
fn hard_dirty_event_invalidates_otherwise_fresh_cache() {
    let sources = [source(1, 4, None, true, true, 0)];
    let demands = [ConsumerDemand { scope: SCOPE, consumer_id: 1, source_id: 1, clock: ClockKind::World, required_version: 4, window: WorkWindow { start: 0, end: 10 }, max_age_seconds: 50, necessary: true }];
    let caches = [CachedOutput { scope: SCOPE, source_id: 1, source_version: 4, clock: ClockKind::World, observed_at: 98, coverage: WorkWindow { start: 0, end: 20 }, valid: true }];
    let now = ClockInstant { kind: ClockKind::World, seconds: 100 };
    let page = evaluate(Snapshot { scope: SCOPE, token: 1, sources: &sources, demands: &demands, caches: &caches, outstanding: &[] }, now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap();
    assert_eq!(page.items[0].reason, WakeReason::Dirty);
}

#[test]
fn impossible_demands_wait_without_masking_independent_due_responsibility() {
    let sources = [source(1, 4, Some(10), false, true, 0)];
    let demands = [
        ConsumerDemand { scope: SCOPE, consumer_id: 1, source_id: 99, clock: ClockKind::World, required_version: 1, window: WorkWindow { start: 0, end: 10 }, max_age_seconds: 10, necessary: true },
        ConsumerDemand { scope: SCOPE, consumer_id: 2, source_id: 1, clock: ClockKind::World, required_version: 5, window: WorkWindow { start: 0, end: 10 }, max_age_seconds: 10, necessary: true },
        ConsumerDemand { scope: SCOPE, consumer_id: 3, source_id: 1, clock: ClockKind::World, required_version: 4, window: WorkWindow { start: 60, end: 120 }, max_age_seconds: 10, necessary: true },
        ConsumerDemand { scope: SCOPE, consumer_id: 4, source_id: 1, clock: ClockKind::Interaction, required_version: 4, window: WorkWindow { start: 0, end: 10 }, max_age_seconds: 10, necessary: true },
    ];
    let now = ClockInstant { kind: ClockKind::World, seconds: 10 };
    let page = evaluate(Snapshot { scope: SCOPE, token: 1, sources: &sources, demands: &demands, caches: &[], outstanding: &[] }, now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].necessary);
    assert_eq!(page.waiting.iter().map(|item| item.reason).collect::<Vec<_>>(), [WaitingReason::MissingSource, WaitingReason::VersionUnavailable, WaitingReason::CoverageUnavailable, WaitingReason::ClockMismatch]);
}

#[test]
fn same_numeric_source_identity_in_other_context_does_not_deduplicate_or_accept_late_result() {
    let a = source(1, 4, Some(10), false, true, 3);
    let mut b = a;
    b.scope = ScopeIdentity { city_id: Some(12), ..SCOPE };
    let now = ClockInstant { kind: ClockKind::World, seconds: 10 };
    let page_a = evaluate(snapshot(&[a], 1), now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap();
    let snap_b = Snapshot { scope: b.scope, token: 1, sources: &[b], demands: &[], caches: &[], outstanding: &[] };
    let scope_b = b.scope;
    let page_b = evaluate(snap_b, now, RequestCursor::begin(scope_b, 1, now), 8).unwrap();
    assert_ne!(page_a.items[0].key, page_b.items[0].key);
    assert!(!result_matches_current(page_a.items[0].key, b));
    assert!(evaluate(snap_b, now, page_a.next, 8).is_err());
}

#[test]
fn clock_identity_is_part_of_task_key_and_wrong_evaluation_clock_is_waiting() {
    let world_source = source(5, 2, Some(10), false, true, 0);
    let mut interaction_source = world_source;
    interaction_source.clock = ClockKind::Interaction;
    let now_world = ClockInstant { kind: ClockKind::World, seconds: 10 };
    let now_interaction = ClockInstant { kind: ClockKind::Interaction, seconds: 10 };
    let page_world = evaluate(snapshot(&[world_source], 3), now_world, RequestCursor::begin(SCOPE, 3, now_world), 8).unwrap();
    let page_interaction = evaluate(snapshot(&[interaction_source], 4), now_interaction, RequestCursor::begin(SCOPE, 4, now_interaction), 8).unwrap();
    assert_ne!(page_world.items[0].key, page_interaction.items[0].key);
    assert!(!result_matches_current(page_world.items[0].key, interaction_source));
    let old_clock_attempt = [Outstanding { key: page_world.items[0].key, necessary: true }];
    let interaction_with_old_attempt = Snapshot { scope: SCOPE, token: 5, sources: &[interaction_source], demands: &[], caches: &[], outstanding: &old_clock_attempt };
    let not_deduplicated = evaluate(interaction_with_old_attempt, now_interaction, RequestCursor::begin(SCOPE, 5, now_interaction), 8).unwrap();
    assert_eq!(not_deduplicated.items.len(), 1);

    let demand = [ConsumerDemand { scope: SCOPE, consumer_id: 9, source_id: 5, clock: ClockKind::Interaction, required_version: 2, window: WorkWindow { start: 0, end: 10 }, max_age_seconds: 5, necessary: true }];
    let wrong_now_page = evaluate(Snapshot { scope: SCOPE, token: 4, sources: &[interaction_source], demands: &demand, caches: &[], outstanding: &[] }, now_world, RequestCursor::begin(SCOPE, 4, now_world), 8).unwrap();
    assert!(wrong_now_page.items.is_empty());
    assert_eq!(wrong_now_page.waiting[0].reason, WaitingReason::EvaluationClockMismatch);
    assert_eq!(wrong_now_page.waiting[1].reason, WaitingReason::EvaluationClockMismatch);

    let invalid_development = Source { clock: ClockKind::Development { city_id: 12 }, ..world_source };
    assert!(evaluate(snapshot(&[invalid_development], 5), now_world, RequestCursor::begin(SCOPE, 5, now_world), 8).is_err());
}

#[test]
fn necessary_due_survives_queue_pressure_pause_and_cancelled_attempt_snapshot_replay() {
    let sources = [source(1, 1, Some(1), false, true, 9), source(2, 1, Some(1), false, false, 0)];
    let now = ClockInstant { kind: ClockKind::World, seconds: 10 };
    let limited = evaluate(snapshot(&sources, 1), now, RequestCursor::begin(SCOPE, 1, now), 1).unwrap();
    assert!(limited.more);
    assert!(limited.items[0].necessary);
    let replay = evaluate(snapshot(&sources, 1), now, RequestCursor::begin(SCOPE, 1, now), 1).unwrap();
    assert_eq!(limited.items, replay.items);
    assert_eq!(limited.items[0].key, TaskKey { scope: SCOPE, clock: ClockKind::World, source_id: 1, source_version: 1, window: sources[0].window, cursor: 9 });
}

#[test]
fn outstanding_attempt_deduplicates_but_cancel_reexposes_same_due_key() {
    let sources = [source(1, 1, Some(1), false, true, 0)];
    let outstanding = [Outstanding { key: TaskKey { scope: SCOPE, clock: ClockKind::World, source_id: 1, source_version: 1, window: sources[0].window, cursor: 0 }, necessary: true }];
    let now = ClockInstant { kind: ClockKind::World, seconds: 5 };
    let page = evaluate(Snapshot { scope: SCOPE, token: 1, sources: &sources, demands: &[], caches: &[], outstanding: &outstanding }, now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap();
    assert!(page.items.is_empty());
    let after_cancel = evaluate(snapshot(&sources, 1), now, RequestCursor::begin(SCOPE, 1, now), 8).unwrap();
    assert_eq!(after_cancel.items[0].key, outstanding[0].key);
}

#[test]
fn cursor_replays_same_page_and_rejects_changed_now_or_snapshot_token() {
    let sources = [source(1, 1, Some(1), false, false, 0), source(2, 1, Some(2), false, true, 0)];
    let now1 = ClockInstant { kind: ClockKind::World, seconds: 1 };
    let frozen = snapshot(&sources, 10);
    let first = evaluate(frozen, now1, RequestCursor::begin(SCOPE, 10, now1), 1).unwrap();
    assert_eq!(first.items[0].key.source_id, 1);
    let replay = evaluate(frozen, now1, RequestCursor::begin(SCOPE, 10, now1), 1).unwrap();
    assert_eq!(first, replay);
    let second = evaluate(frozen, now1, first.next, 1).unwrap();
    assert!(second.items.is_empty());
    assert!(!second.more);
    let now2 = ClockInstant { kind: ClockKind::World, seconds: 2 };
    assert!(evaluate(frozen, now2, first.next, 1).is_err());
    let changed_snapshot = snapshot(&sources, 11);
    assert!(evaluate(changed_snapshot, now1, first.next, 1).is_err());
    let other_scope = ScopeIdentity { city_id: Some(12), ..SCOPE };
    let changed_scope = Snapshot { scope: other_scope, ..changed_snapshot };
    assert!(evaluate(changed_scope, now1, first.next, 1).is_err());
    // Recompute from a new frozen time/snapshot instead of continuing the stale cursor.
    let restarted = evaluate(changed_snapshot, now2, RequestCursor::begin(SCOPE, 11, now2), 2).unwrap();
    assert_eq!(restarted.items.iter().map(|x| x.key.source_id).collect::<Vec<_>>(), [2, 1]);
}

#[test]
fn zero_budget_is_unresolved_and_does_not_generate_replacement_facts() {
    let sources = [source(9, 3, Some(1), false, true, 22)];
    let now = ClockInstant { kind: ClockKind::World, seconds: 2 };
    let page = evaluate(snapshot(&sources, 1), now, RequestCursor::begin(SCOPE, 1, now), 0).unwrap();
    assert!(page.items.is_empty());
    assert!(page.more);
    let retry = evaluate(snapshot(&sources, 1), now, RequestCursor::begin(SCOPE, 1, now), 1).unwrap();
    assert_eq!(retry.items[0].key.source_version, 3);
    assert_eq!(retry.items[0].key.cursor, 22);
}
