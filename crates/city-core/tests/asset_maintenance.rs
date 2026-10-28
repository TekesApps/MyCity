//! Source-only contract fixtures. NOT_RUN; no actual provider certification.
pub use mycity_core::units;
#[path = "../src/asset_maintenance.rs"]
mod asset_maintenance;
use asset_maintenance::*;
use units::{Carry, GameTime, Quantity};
fn r(id: u32) -> Source { Source { id, revision: 1 } }
fn q(n: i64) -> Quantity { Quantity::new(n).unwrap() }
const DAY: u64 = 86_400;
fn t(n: i64) -> GameTime { GameTime::new(n * DAY as i64).unwrap() }
fn profile(kind: Kind) -> Profile {
    // Synthetic, finite rates, not bridge engineering standards or #32 output.
    Profile { revision: REVISION, kind, facility: r(31), maintenance: r(36),
        clock: ClockKind::InteractionObligation, inspection_period_seconds: match kind { Kind::Bridge => 10 * DAY, Kind::OrdinaryEquipment => 15 * DAY },
        service_period_seconds: 30 * DAY, inspection_worker_seconds: 8 * 144,
        service_worker_seconds: 100 * 144, material: r(8), service_material: q(20),
        service_improvement_bps: 1000, replacement_worker_seconds: 500 * 144,
        replacement_material: q(200) }
}
fn initial(kind: Kind) -> Snapshot {
    Snapshot::commissioned(1, 147, t(0), profile(kind), &[
        (1, Carry::new(1, 3 * DAY).unwrap(), Carry::new(1, 2 * DAY).unwrap()),
        (2, Carry::new(0, 1).unwrap(), Carry::new(0, 1).unwrap()),
    ], Carry::new(7, 3 * DAY).unwrap()).unwrap()
}
fn elapsed(s: &Snapshot, to: i64, development: u64, obligation: u64,
    operating: u64, damage: u16) -> Candidate {
    advance(s, Exposure { to: t(to), development_seconds: development * DAY,
        obligation_seconds: obligation * DAY, parts: &[(1, operating * DAY, damage), (2, 0, 0)] }).unwrap()
}
fn delivery(serial: u64, action: Action, ordinal: u64, work: u64, material: i64) -> Delivery {
    Delivery { receipt_serial: serial, source_snapshot: 147, component: 1,
        generation: 0, action, due_ordinal: ordinal, worker_seconds: work * 144,
        material: r(8), quantity: q(material) }
}
fn apply(s: &Snapshot, d: &Delivery) -> Snapshot {
    deliver(s, d).unwrap().against(s).unwrap().clone()
}
#[test]
fn literal_damage_age_operating_and_no_input_for_two_profiles() {
    for kind in [Kind::Bridge, Kind::OrdinaryEquipment] {
        let s = initial(kind);
        let candidate = elapsed(&s, 30, 30, 30, 6, 2000);
        assert_eq!(candidate.fee_cents(), 70); // 30 * 7/3 cents, not cash spent
        let s = candidate.against(&s).unwrap().clone();
        assert_eq!(s.condition_bps(1), Some(7987)); // 10000 - 2000 - 10 - 3
        assert_eq!(s.age_seconds(), 30 * DAY);
        let no_input = apply(&s, &delivery(1, Action::Service, 1, 0, 0));
        assert_eq!(no_input.condition_bps(1), Some(7987));
        assert_eq!(no_input.due_page(None, 16).unwrap().orders[1].count, 1);
        let labor_only = apply(&no_input, &delivery(2, Action::Service, 1, 100, 0));
        assert_eq!(labor_only.condition_bps(1), Some(7987));
        let partial = apply(&labor_only, &delivery(3, Action::Service, 1, 0, 5));
        assert_eq!(partial.condition_bps(1), Some(8237)); // exactly 250 bps
        let complete = apply(&partial, &delivery(4, Action::Service, 1, 0, 15));
        assert_eq!(complete.condition_bps(1), Some(8987));
        assert_eq!(complete.age_seconds(), 30 * DAY);
        assert_eq!(complete.due_page(None, 16).unwrap().orders.len(), 3);
    }
}
#[test]
fn stale_replayed_receipt_and_old_candidate_rejected() {
    let s0 = initial(Kind::Bridge);
    let a = elapsed(&s0, 30, 30, 30, 0, 2000);
    let s = a.against(&s0).unwrap().clone();
    let d = delivery(1, Action::Service, 1, 50, 10);
    let c = deliver(&s, &d).unwrap();
    let n = c.against(&s).unwrap().clone();
    assert_eq!(deliver(&n, &d), Err(Error::Stale));
    assert_eq!(c.against(&n), Err(Error::Stale));
    assert_eq!(a.against(&n), Err(Error::Stale));
    let mut old = delivery(2, Action::Service, 1, 50, 10);
    old.source_snapshot = 146;
    assert_eq!(deliver(&n, &old), Err(Error::Stale));
    old.source_snapshot = 147;
    old.material = r(999);
    assert_eq!(deliver(&n, &old), Err(Error::Unknown));
}
#[test]
fn interval_split_and_json_restore_preserve_literal_remainders() {
    let s = initial(Kind::OrdinaryEquipment);
    let a = elapsed(&s, 1, 1, 1, 1, 0);
    assert_eq!(a.fee_cents(), 2);
    let one = a.against(&s).unwrap().clone();
    let encoded = serde_json::to_vec(&one).unwrap();
    let restored: Snapshot = serde_json::from_slice(&encoded).unwrap();
    restored.validate().unwrap();
    let b = elapsed(&restored, 3, 2, 2, 2, 0);
    assert_eq!(b.fee_cents(), 5);
    let whole = elapsed(&s, 3, 3, 3, 3, 0);
    assert_eq!(whole.fee_cents(), 7);
    assert_eq!(b.against(&restored).unwrap().condition_bps(1), Some(9998));
    // Revision differs by number of accepted pages; compare accrued facts/carries.
    let split = serde_json::to_value(b.against(&restored).unwrap()).unwrap();
    let full = serde_json::to_value(whole.against(&s).unwrap()).unwrap();
    assert_eq!(split["components"], full["components"]);
    assert_eq!(split["annual_fee"], full["annual_fee"]);
}
#[test]
fn replacement_only_resets_delivered_component_and_keeps_obligations() {
    let a = initial(Kind::Bridge);
    let c = elapsed(&a, 60, 60, 60, 0, 2000);
    let s = c.against(&a).unwrap().clone();
    let d = delivery(1, Action::Replace, 0, 500, 200);
    let n = apply(&s, &d);
    assert_eq!(n.age_seconds(), 60 * DAY);
    assert_eq!(n.component_age(1), Some(0));
    assert_eq!(n.component_age(2), Some(60 * DAY));
    assert_eq!(n.condition_bps(1), Some(10000));
    assert_eq!(n.due_page(None, 16).unwrap().orders, s.due_page(None, 16).unwrap().orders
        .into_iter().map(|mut due| { due.base_revision += 1; due }).collect::<Vec<_>>());
    assert_eq!(deliver(&n, &delivery(2, Action::Replace, 0, 500, 200)), Err(Error::Stale));
}
#[test]
fn sleeping_worker_and_backpressure_preserve_oldest_due_and_resume() {
    let a = initial(Kind::Bridge);
    // Closed city: interaction-bound calendar aging/obligations still advance.
    let c = elapsed(&a, 90, 0, 90, 0, 0);
    let s = c.against(&a).unwrap().clone();
    assert_eq!(s.age_seconds(), 90 * DAY);
    let full = s.due_page(None, 16).unwrap();
    assert_eq!(full.orders[0].count, 9);
    assert_eq!(full.orders[1].count, 3);
    let blocked = s.due_page(None, 0).unwrap();
    assert_eq!(blocked.cursor, s.due_page(None, 0).unwrap().cursor);
    assert!(!blocked.complete);
    let first = s.due_page(Some(blocked.cursor), 1).unwrap();
    let rest = s.due_page(Some(first.cursor), 16).unwrap();
    let mut combined = first.orders;
    combined.extend(rest.orders);
    assert_eq!(combined, full.orders);
    assert_eq!(s.due_page(None, 16).unwrap(), full);
    let partial = apply(&s, &delivery(1, Action::Inspect, 1, 4, 0));
    let bytes = serde_json::to_vec(&partial).unwrap();
    let recovered: Snapshot = serde_json::from_slice(&bytes).unwrap();
    let done = apply(&recovered, &delivery(2, Action::Inspect, 1, 4, 0));
    let due = done.due_page(None, 16).unwrap();
    assert_eq!(due.orders[0].first_ordinal, 2);
    assert_eq!(due.orders[0].count, 8);
    assert_eq!(due.orders[1].count, 3);
}
#[test]
fn invalid_overdelivery_missing_coverage_and_clock_fail_without_changes() {
    let a = initial(Kind::Bridge);
    let c = elapsed(&a, 30, 30, 30, 0, 2000);
    let s = c.against(&a).unwrap().clone();
    let before = s.clone();
    assert_eq!(deliver(&s, &delivery(1, Action::Service, 1, 101, 20)), Err(Error::Invalid));
    assert_eq!(deliver(&s, &delivery(1, Action::Service, 2, 100, 20)), Err(Error::Invalid));
    assert_eq!(advance(&s, Exposure { to: t(60), development_seconds: 0,
        obligation_seconds: 0, parts: &[(1, 0, 0), (2, 0, 0)] }), Err(Error::Invalid));
    assert_eq!(advance(&s, Exposure { to: t(60), development_seconds: 30,
        obligation_seconds: 30, parts: &[(1, 0, 0)] }), Err(Error::Invalid));
    assert_eq!(s, before);
    let retry = apply(&s, &delivery(1, Action::Service, 1, 100, 20));
    assert_eq!(retry.condition_bps(1), Some(8990));
}
#[test]
fn partial_service_restore_keeps_inputs_and_rounding_credit() {
    let a = initial(Kind::Bridge);
    let c = elapsed(&a, 30, 30, 30, 0, 2000);
    let s = c.against(&a).unwrap().clone();
    let partial = apply(&s, &delivery(1, Action::Service, 1, 33, 7));
    assert_eq!(partial.condition_bps(1), Some(8320)); // 7990 + min(330,350)
    let bytes = serde_json::to_vec(&partial).unwrap();
    let restored: Snapshot = serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();
    let complete = apply(&restored, &delivery(2, Action::Service, 1, 67, 13));
    assert_eq!(complete.condition_bps(1), Some(8990));
    assert_eq!(complete.due_page(None, 16).unwrap().orders.len(), 3);
    let old_cursor = s.due_page(None, 1).unwrap().cursor;
    assert_eq!(complete.due_page(Some(old_cursor), 1), Err(Error::Stale));
    // Inputs rejected together on excess; same serial can retry corrected input.
    assert_eq!(deliver(&restored, &delivery(2, Action::Service, 1, 68, 13)), Err(Error::Invalid));
    assert_eq!(apply(&restored, &delivery(2, Action::Service, 1, 67, 13)), complete);
}
#[test]
fn local_clock_is_frozen_but_accepted_interaction_work_can_age_closed_assets() {
    let mut p = profile(Kind::Bridge);
    p.clock = ClockKind::LocalDevelopment;
    let a = Snapshot::commissioned(2, 147, t(0), p,
        &[(1, Carry::new(1, 3 * DAY).unwrap(), Carry::new(1, 2 * DAY).unwrap())],
        Carry::new(7, 3 * DAY).unwrap()).unwrap();
    let c = advance(&a, Exposure { to: t(30), development_seconds: 0,
        obligation_seconds: 0, parts: &[(1, 0, 0)] }).unwrap();
    let n = c.against(&a).unwrap();
    assert_eq!(n.age_seconds(), 0);
    assert!(n.due_page(None, 16).unwrap().orders.is_empty());
    let b = initial(Kind::Bridge);
    let active_delivery = elapsed(&b, 30, 0, 30, 6, 0);
    let next = active_delivery.against(&b).unwrap();
    assert_eq!(next.age_seconds(), 30 * DAY);
    assert_eq!(next.condition_bps(1), Some(9987));
}
#[test]
fn unknown_profiles_and_arithmetic_failure_are_not_shortage_evidence() {
    let mut p = profile(Kind::OrdinaryEquipment);
    p.service_period_seconds = 0;
    assert!(Snapshot::commissioned(1, 147, t(0), p,
        &[(1, Carry::new(0, 1).unwrap(), Carry::new(0, 1).unwrap())],
        Carry::new(0, 1).unwrap()).is_err());
    let a = Snapshot::commissioned(1, 147, t(0), profile(Kind::Bridge),
        &[(1, Carry::new(u64::MAX, 1).unwrap(), Carry::new(0, 1).unwrap())],
        Carry::new(0, 1).unwrap()).unwrap();
    let old = a.clone();
    assert_eq!(advance(&a, Exposure { to: t(1), development_seconds: DAY,
        obligation_seconds: DAY, parts: &[(1, 0, 0)] }), Err(Error::Overflow));
    assert_eq!(a, old);
}
#[test]
fn malformed_restored_carry_and_credit_are_refused() {
    let a = initial(Kind::Bridge);
    let mut value = serde_json::to_value(&a).unwrap();
    value["annual_fee"]["denominator"] = serde_json::json!(0);
    assert!(serde_json::from_value::<Snapshot>(value).is_err());
    let mut value = serde_json::to_value(&a).unwrap();
    value["components"][0]["credited_bps"] = serde_json::json!(1);
    let bad: Snapshot = serde_json::from_value(value).unwrap();
    assert_eq!(bad.validate(), Err(Error::Invalid));
}
