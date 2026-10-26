use crate::{arena,model_rows};
#[path = "../../../../tests/fixtures/issue-57-needs/literal.rs"]
mod literal;
use super::*;

fn opening() -> Opening {
    let mut households = model_rows::Rows::new().unwrap();
    for household_id in 1..=3 { households.try_push(Household { household_id,
        source_id: 100 + u64::from(household_id), generation: 1, people: 2 }).unwrap(); }
    Opening { version: 1, profile: Profile::LocalDevelopmentNaturalNeedsV1,
        scope: arena::Scope::new(1, 2).unwrap(), city_id: 7, seed: 0, households }
}
fn commit(runtime: &mut Runtime, candidate: Candidate) -> Page {
    assert!(candidate.matches(runtime).unwrap());
    let Candidate { next, page, .. } = candidate;
    *runtime = *next; page
}
fn enter(runtime: &mut Runtime, id: u32, op: u64) {
    let c = runtime.record_legal_entry(id, runtime.covered_through(), op).unwrap(); commit(runtime, c);
}
fn drain(runtime: &mut Runtime, at: u64, page_size: usize, op: u64) -> Vec<Effect> {
    let mut events = Vec::new();
    for _ in 0..128 {
        let c = runtime.prepare_due(at, page_size, op).unwrap();
        let page = commit(runtime, c); events.extend_from_slice(&page.effects);
        if page.complete { return events; }
        assert_eq!(runtime.next_due().unwrap(), Some(at));
    }
    panic!("bounded page count");
}
#[test]
fn literal_checked_phase_and_exact_current_layout() {
    assert_eq!(std::mem::size_of::<Current>(), 16);
    for (i, kind) in Kind::ALL.into_iter().enumerate() {
        assert_eq!(phase(0, 0, kind).unwrap(), literal::ZERO_PHASES[i]);
        assert_eq!(phase(u64::MAX, u64::MAX, kind).unwrap(), literal::MAX_PHASES[i]);
    }
}
#[test]
fn no_entry_has_no_need_and_advancing_empty_coverage_allows_actual_later_entry() {
    let mut runtime = Runtime::new(opening(), 0).unwrap();
    assert_eq!(runtime.next_due().unwrap(), None);
    assert!(drain(&mut runtime, 12_345, 128, 10).is_empty());
    assert_eq!(runtime.covered_through(), 12_345);
    enter(&mut runtime, 1, 11);
    let first = runtime.next_due().unwrap().unwrap();
    assert!(first > 12_345);
    assert_eq!(runtime.household(1).unwrap().entry.unwrap().operation_id, 11);
    assert!(runtime.household(2).unwrap().entry.is_none());
}
#[test]
fn entry_is_strictly_before_occurrence_and_never_reset_by_later_lease() {
    let o = opening(); let reg = o.households[0];
    let d = u64::from(phase(stable_token(o.scope, reg), o.seed, Kind::Police).unwrap());
    let at = d * literal::DAY + literal::EIGHT_AM;
    let mut runtime = Runtime::new(o, at).unwrap(); enter(&mut runtime, 1, 10);
    let row = *runtime.household(1).unwrap();
    assert_eq!(row.next_days[Kind::Police.index()], Some(u32::try_from(d).unwrap() + 180));
    // Re-leasing is a no-op for source, first operation and original occurrence coverage.
    let c = runtime.record_legal_entry(1, at, 11).unwrap(); commit(&mut runtime, c);
    assert_eq!(*runtime.household(1).unwrap(), row);
    assert!(matches!(runtime.record_legal_entry(1, at + 1, 12), Err(Error::Stale)));
}
#[test]
fn generated_need_and_deadline_are_original_zero_delivery_events_once() {
    let mut runtime = Runtime::new(opening(), 0).unwrap(); enter(&mut runtime, 1, 10);
    let at = runtime.next_due().unwrap().unwrap();
    let generated = drain(&mut runtime, at, 1, 11);
    assert!(!generated.is_empty());
    for e in &generated {
        assert_eq!(e.event, Event::Generated);
        assert_eq!(e.generation, 1);
        let current = runtime.current(e.household_id, e.kind).unwrap();
        assert!(current.is_waiting()); assert_eq!(current.delivery_ref, 0);
        assert_eq!(u64::from(current.deadline_day) * literal::DAY + literal::EIGHT_AM, at + literal::DAY);
    }
    // The next earliest event is at most the original next-day deadline, never skipped.
    while runtime.next_due().unwrap().unwrap() < at + literal::DAY {
        let next = runtime.next_due().unwrap().unwrap(); drain(&mut runtime, next, 128, 12);
    }
    let expired = drain(&mut runtime, at + literal::DAY, 1, 13);
    for e in generated {
        assert!(expired.iter().any(|x| x.kind == e.kind && x.event == Event::ExpiredUnadmittedZeroDelivery));
        assert!(runtime.current(1, e.kind).unwrap().is_expired());
        assert_eq!(runtime.summary(1, e.kind).unwrap().expired, 1);
        assert_eq!(runtime.household(1).unwrap().deadline_operations[e.kind.index()], 13);
    }
    assert!(drain(&mut runtime, at + literal::DAY, 128, 14).is_empty());
}
#[test]
fn partial_pages_restart_without_duplicate_generation_and_match_full_page_effects() {
    let mut a = Runtime::new(opening(), 0).unwrap();
    for id in 1..=3 { enter(&mut a, id, 10 + u64::from(id)); }
    let mut b = a.clone(); let at = a.next_due().unwrap().unwrap();
    let first = a.prepare_due(at, 1, 20).unwrap(); assert!(!first.page.complete); commit(&mut a, first);
    assert!(a.pending().is_some());
    assert_eq!(a.covered_through(), 0);
    assert!(matches!(a.record_legal_entry(1, 0, 21), Err(Error::Pending)));
    let serialized = serde_json::to_vec(&a).unwrap();
    let mut a: Runtime = serde_json::from_slice(&serialized).unwrap(); a.validate().unwrap();
    assert!(!a.is_ready());
    assert!(matches!(a.prepare_due(at, 1, 20), Err(Error::Unauthenticated)));
    // Simulated root receipt-authentication bridge; not a real Store recovery acceptance.
    a.mark_authenticated().unwrap();
    let events_a = drain(&mut a, at, 1, 20);
    let events_b = drain(&mut b, at, 128, 20);
    assert_eq!(events_a, events_b); // the first page was deadlines only, without effects
    assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
    assert!(a.pending().is_none());
}
#[test]
fn no_budget_skipped_boundary_wrong_registration_and_future_responsibility_are_rejected() {
    let mut runtime = Runtime::new(opening(), 0).unwrap(); enter(&mut runtime, 1, 10);
    let due = runtime.next_due().unwrap().unwrap();
    let fingerprint = runtime.fingerprint().unwrap();
    assert!(matches!(runtime.prepare_due(due, 0, 11), Err(Error::Budget)));
    assert!(matches!(runtime.prepare_due(due + 1, 128, 11), Err(Error::SkippedDue)));
    assert_eq!(runtime.fingerprint().unwrap(), fingerprint);
    let mut bad = opening(); bad.households[0].generation = 2;
    assert!(matches!(Runtime::new(bad, 0), Err(Error::Registration)));
    drain(&mut runtime, due, 128, 11);
    let mut json = serde_json::to_value(&runtime).unwrap();
    let kind = runtime.household(1).unwrap().current.iter().position(|c| c.is_waiting()).unwrap();
    json["households"][0]["current"][kind]["delivery_ref"] = 1.into();
    let invalid: Runtime = serde_json::from_value(json).unwrap();
    assert_eq!(invalid.validate(), Err(Error::UnsupportedResponsibility));
}
#[test]
fn source_key_token_ignores_people_but_separates_world_branch_household_source_generation() {
    let o = opening(); let h = o.households[0]; let token = stable_token(o.scope, h);
    let mut altered = h; altered.people = 5; assert_eq!(stable_token(o.scope, altered), token);
    altered = h; altered.household_id = 99; assert_ne!(stable_token(o.scope, altered), token);
    altered = h; altered.source_id = 999; assert_ne!(stable_token(o.scope, altered), token);
    altered = h; altered.generation = 2; assert_ne!(stable_token(o.scope, altered), token);
    assert_ne!(stable_token(arena::Scope::new(2, 2).unwrap(), h), token);
    assert_ne!(stable_token(arena::Scope::new(1, 3).unwrap(), h), token);
}
#[test]
fn candidate_cas_and_tampered_coverage_fail_without_runtime_mutation() {
    let mut runtime = Runtime::new(opening(), 0).unwrap();
    let first = runtime.record_legal_entry(1, 0, 10).unwrap();
    let stale = runtime.record_legal_entry(2, 0, 11).unwrap();
    commit(&mut runtime, first); assert!(!stale.matches(&runtime).unwrap());
    let due = runtime.next_due().unwrap().unwrap();
    let mut json = serde_json::to_value(&runtime).unwrap(); json["covered_through"] = due.into();
    let invalid: Runtime = serde_json::from_value(json).unwrap(); assert_eq!(invalid.validate(), Err(Error::Invalid));
}
#[test]
fn latest_deadline_survives_next_cycle_slot_reuse_without_repeat_entry() {
    let mut runtime = Runtime::new(opening(), 0).unwrap(); enter(&mut runtime, 1, 10);
    let mut first_medical = None;
    for operation in 11..100 {
        let at = runtime.next_due().unwrap().unwrap();
        drain(&mut runtime, at, 128, operation);
        if let Some(deadline) = runtime.latest_deadline(1, Kind::Medical) {
            first_medical.get_or_insert(deadline);
            if runtime.current(1, Kind::Medical).unwrap().is_waiting() {
                let prior = first_medical.unwrap();
                assert_eq!(deadline, prior);
                assert_eq!(runtime.current(1, Kind::Medical).unwrap().occurrence_day, prior.occurrence_day + 90);
                assert_eq!(runtime.summary(1, Kind::Medical).unwrap(), Summary { generated: 2, expired: 1, reminders: 0 });
                return;
            }
        }
    }
    panic!("two bounded medical cycles should be reachable");
}
#[test]
fn checkpoint_in_occurrence_subphase_requires_auth_and_keeps_effect_cursors() {
    let mut a = Runtime::new(opening(), 0).unwrap();
    for id in 1..=3 { enter(&mut a, id, 10 + u64::from(id)); }
    let mut b = a.clone(); let at = a.next_due().unwrap().unwrap();
    let c = a.prepare_due(at, 128, 20).unwrap(); commit(&mut a, c); // full deadline phase
    let c = a.prepare_due(at, 1, 20).unwrap();
    let first_events = c.page.effects.to_vec(); commit(&mut a, c);
    let bytes = serde_json::to_vec(&a).unwrap();
    assert!(bytes.len() <= MAX_SERIALIZED_BYTES);
    let mut reopened: Runtime = serde_json::from_slice(&bytes).unwrap(); reopened.validate().unwrap();
    assert!(matches!(reopened.record_legal_entry(1, 0, 21), Err(Error::Unauthenticated)));
    reopened.mark_authenticated().unwrap();
    let mut all = first_events; all.extend(drain(&mut reopened, at, 1, 20));
    assert_eq!(all, drain(&mut b, at, 128, 20));
    assert_eq!(reopened.fingerprint().unwrap(), b.fingerprint().unwrap());
}
#[test]
fn horizon_and_invalid_page_limits_are_bounded_without_erasing_state() {
    assert!(matches!(Runtime::new(opening(), u64::MAX), Err(Error::Overflow)));
    let mut runtime = Runtime::new(opening(), u64::from(u32::MAX) * DAY + EIGHT_AM).unwrap();
    assert!(matches!(runtime.record_legal_entry(1, runtime.covered_through(), 10), Err(Error::Overflow)));
    assert!(runtime.household(1).unwrap().entry.is_none());
    assert!(matches!(runtime.prepare_due(runtime.covered_through(), 129, 10), Err(Error::Budget)));
    // The source registration itself rejects duplicate/unsorted identity and missing city.
    let mut o = opening(); o.city_id = 0; assert!(matches!(Runtime::new(o, 0), Err(Error::Registration)));
    let mut o = opening(); o.households[1].source_id = o.households[0].source_id;
    assert!(matches!(Runtime::new(o, 0), Err(Error::Registration)));
    let mut o = opening(); o.households[1].household_id = 1;
    assert!(matches!(Runtime::new(o, 0), Err(Error::Registration)));
}
