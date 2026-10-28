//! Pure component checks. All providers/receipts below are synthetic contracts.
use mycity_core::{asset_cost::*, units};
use units::{GameTime, Money, Quantity};

fn m(n: i64) -> Money { Money::new(n).unwrap() }
fn q(n: i64) -> Quantity { Quantity::new(n).unwrap() }
fn t(n: i64) -> GameTime { GameTime::new(n).unwrap() }
fn r(id: u64) -> Source { Source { id, revision: 1 } }
fn delivered() -> ActualDelivery {
    ActualDelivery { lot: 1, asset: 7, owner: 10, capital: r(100), delivery: r(101),
        source_snapshot: 147, quantity: q(1), basis: m(101), at: t(0),
        profile: Profile { revision: REVISION, source: r(117), method: Method::StraightLine,
            clock: ClockKind::InteractionObligation, life_seconds: 7,
            cost_quantum: q(1), residual: m(2) }, lease: None }
}
fn initial() -> Snapshot { Snapshot::new_delivery(delivered()).unwrap() }
fn expected(s: &Snapshot, receipt: u64) -> Expected {
    Expected { revision: s.revision(), source_snapshot: s.source_snapshot(), receipt: r(receipt) }
}
fn accrue(s: &Snapshot, until: i64, development: u64, receipt: u64) -> Candidate {
    plan(s, expected(s, receipt), Action::Accrue { to: t(until), development_seconds: development }).unwrap()
}
fn next(s: &Snapshot, c: &Candidate) -> Snapshot { c.against(s).unwrap().clone() }
fn restore(s: &Snapshot) -> Snapshot {
    let restored = parse_snapshot(&serde_json::to_vec(s).unwrap()).unwrap();
    restored.validate().unwrap();
    restored
}
fn amount(c: &Candidate, slot: usize) -> i64 { c.costs[slot].unwrap().amount.value() }
fn partial_scope() -> Snapshot {
    let mut d = delivered();
    d.basis = m(102); d.quantity = q(3); d.profile.residual = m(12);
    d.profile.life_seconds = 5;
    Snapshot::new_delivery(d).unwrap()
}
fn repair(source: u64, kind: ExpenseUse, liable: u32) -> Action {
    Action::Expense(ActualExpense { source: r(source), maintenance_delivery: r(36),
        amount: m(17), liable, use_kind: kind })
}

#[test]
fn literal_period_split_remainder_final_residual_and_no_extra_after_life() {
    // Independent literal oracle: (101-2)/7 = 14 + 1/7 cents per second.
    let s = initial();
    let first = accrue(&s, 1, 0, 1);
    assert_eq!(amount(&first, 0), 14);
    let a = restore(&next(&s, &first));
    assert_eq!(a.time_remainder().unwrap(), 1);
    let second = accrue(&a, 3, 0, 2);
    assert_eq!(amount(&second, 0), 28);
    let b = next(&a, &second);
    assert_eq!(b.time_remainder().unwrap(), 3);
    assert_eq!(amount(&accrue(&s, 3, 0, 3), 0), 42);
    let last = accrue(&b, 7, 0, 4);
    assert_eq!(amount(&last, 0), 57);
    let end = next(&b, &last);
    assert_eq!(end.carrying().unwrap(), m(2));
    assert_eq!(end.time_remainder().unwrap(), 0);
    assert_eq!(amount(&accrue(&end, 100, 0, 5), 0), 0);
}

#[test]
fn every_fixed_period_partition_matches_literal_total() {
    let s = initial();
    for cut in 1..7 {
        let first = accrue(&s, cut, 0, 10);
        let restored = restore(&next(&s, &first));
        let second = accrue(&restored, 7, 0, 11);
        assert_eq!(amount(&first, 0) + amount(&second, 0), 99);
        assert_eq!(next(&restored, &second).carrying().unwrap(), m(2));
    }
}

#[test]
fn rent_and_depreciation_belong_to_different_parties_not_double_operator_cost() {
    let mut d = delivered();
    d.lease = Some(Lease { source: r(200), operator: 20, start: t(0), end: t(7), total: m(101) });
    let s = Snapshot::new_delivery(d).unwrap();
    let c = accrue(&s, 3, 0, 10);
    assert_eq!(c.costs[0].unwrap(), CostDelta { party: 10, kind: CostKind::Depreciation,
        amount: m(42), source: r(100) });
    assert_eq!(c.costs[1].unwrap(), CostDelta { party: 20, kind: CostKind::Rent,
        amount: m(43), source: r(200) });
    let mid = restore(&next(&s, &c));
    let tail = accrue(&mid, 7, 0, 11);
    assert_eq!(amount(&tail, 1), 58);
    assert_eq!(amount(&tail, 0), 57);
    assert!(!tail.establishes_distributable_profit());
    assert_eq!(plan(&mid, expected(&mid, 12), repair(200, ExpenseUse::Repair, 20)), Err(Error::Duplicate));
    assert_eq!(plan(&mid, expected(&mid, 12), Action::Exit { quantity: q(1),
        kind: ExitKind::Sale, protection: Protection::Clear }), Err(Error::Unsupported));
}

#[test]
fn no_self_lease_and_no_unsourced_or_missing_lifetime_default() {
    let mut d = delivered();
    d.lease = Some(Lease { source: r(200), operator: 10, start: t(0), end: t(7), total: m(101) });
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Invalid));
    d.lease = None; d.profile.life_seconds = 0;
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Unknown));
    d = delivered(); d.profile.method = Method::Unknown;
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Unknown));
    d = delivered(); d.capital.id = 0;
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Unknown));
    d = delivered(); d.profile.revision = 2;
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Stale));
}

#[test]
fn ordinary_repair_does_not_reset_basis_age_or_make_payment_a_repair() {
    let s = initial();
    let aged = next(&s, &accrue(&s, 3, 0, 1));
    let c = plan(&aged, expected(&aged, 2), repair(300, ExpenseUse::Repair, 10)).unwrap();
    assert_eq!(amount(&c, 0), 17);
    assert_eq!(c.exclusive_cost_source, Some(r(300)));
    assert_eq!(c.new_capital, None);
    assert_eq!(c.additional_lot, None);
    let repaired = next(&aged, &c);
    assert_eq!(repaired.age_seconds(), 3);
    assert_eq!(repaired.carrying().unwrap(), m(59));
    assert_eq!(repaired.origin_basis(), m(101));
    for kind in [ExpenseUse::QuoteOrPayment, ExpenseUse::Unknown] {
        assert_eq!(plan(&aged, expected(&aged, 2), repair(301, kind, 10)), Err(Error::Unknown));
    }
    assert_eq!(plan(&aged, expected(&aged, 2), repair(301, ExpenseUse::AlreadyCapitalized, 10)), Err(Error::Duplicate));
    assert_eq!(plan(&aged, expected(&aged, 2), repair(100, ExpenseUse::Repair, 10)), Err(Error::Duplicate));
    assert_eq!(plan(&aged, expected(&aged, 2), repair(301, ExpenseUse::Repair, 99)), Err(Error::Invalid));
}

#[test]
fn true_replacement_retires_old_remaining_cost_and_only_new_delivery_restarts_life() {
    let s = initial();
    let aged = next(&s, &accrue(&s, 3, 0, 1));
    let mut actual = delivered();
    actual.at = t(3); actual.lot = 2; actual.capital = r(400); actual.delivery = r(401);
    actual.basis = m(70); actual.profile.residual = m(7);
    let action = Action::Replace { actual, maintenance_delivery: r(36), protection: Protection::Clear };
    let c = plan(&aged, expected(&aged, 2), action).unwrap();
    assert_eq!(c.derecognition.unwrap().remaining_cost, m(59));
    assert_eq!(c.new_capital, Some(m(70)));
    assert_eq!(c.costs, [None, None]);
    let new_asset = c.additional_lot.as_ref().unwrap();
    assert_eq!(new_asset.age_seconds(), 0);
    assert_eq!(new_asset.carrying().unwrap(), m(70));
    assert_eq!(next(&aged, &c).quantity(), q(0));
    assert_eq!(amount(&accrue(new_asset, 10, 0, 3), 0), 63);
    actual.capital = r(100);
    assert_eq!(plan(&aged, expected(&aged, 2), Action::Replace { actual,
        maintenance_delivery: r(36), protection: Protection::Clear }), Err(Error::Invalid));
    assert_eq!(plan(&aged, expected(&aged, 2), Action::Replace { actual,
        maintenance_delivery: r(36), protection: Protection::ProtectedOrLiquidating }), Err(Error::Protected));
}

#[test]
fn partial_sale_and_recovery_only_remove_corresponding_remaining_cost() {
    // Three 34-cent quanta, residual 4 each, 5-second life. At second 2,
    // each has depreciated 12 cents and carries 22. No sale price oracle.
    let s = partial_scope();
    let aged = next(&s, &accrue(&s, 2, 0, 1));
    assert_eq!(aged.carrying().unwrap(), m(66));
    let sale = plan(&aged, expected(&aged, 2), Action::Exit { quantity: q(1),
        kind: ExitKind::Sale, protection: Protection::Clear }).unwrap();
    assert_eq!(sale.derecognition.unwrap().remaining_cost, m(22));
    let remaining = restore(&next(&aged, &sale));
    assert_eq!(remaining.carrying().unwrap(), m(44));
    let recovery = plan(&remaining, expected(&remaining, 3), Action::Exit { quantity: q(1),
        kind: ExitKind::Recovery, protection: Protection::Clear }).unwrap();
    assert_eq!(recovery.derecognition.unwrap().remaining_cost, m(22));
    let one = next(&remaining, &recovery);
    let finish = accrue(&one, 5, 0, 4);
    assert_eq!(amount(&finish, 0), 18);
    assert_eq!(next(&one, &finish).carrying().unwrap(), m(4));
}

#[test]
fn related_transfer_preserves_original_basis_age_remainder_and_lineage() {
    let s = partial_scope();
    let aged = next(&s, &accrue(&s, 2, 0, 1));
    let c = plan(&aged, expected(&aged, 2), Action::RelatedTransfer { quantity: q(1),
        destination_lot: 2, buyer: 20, protection: Protection::Clear }).unwrap();
    let transferred = restore(c.additional_lot.as_ref().unwrap());
    assert_eq!(transferred.owner(), 20);
    assert_eq!(transferred.origin_basis(), m(102));
    assert_eq!(transferred.age_seconds(), 2);
    assert_eq!(transferred.related_origin(), Some(r(2)));
    assert_eq!(transferred.carrying().unwrap(), m(22));
    assert_eq!(c.new_capital, None);
    assert_eq!(c.costs, [None, None]);
    let retained = next(&aged, &c);
    assert_eq!(amount(&accrue(&transferred, 5, 0, 3), 0), 18);
    assert_eq!(amount(&accrue(&retained, 5, 0, 4), 0), 36);
    assert_eq!(amount(&accrue(&aged, 5, 0, 5), 0), 54);
    let onward = plan(&transferred, expected(&transferred, 6), Action::RelatedTransfer {
        quantity: q(1), destination_lot: 3, buyer: 30, protection: Protection::Clear }).unwrap();
    assert_eq!(onward.additional_lot.unwrap().related_origin(), Some(r(2)));
    for protection in [Protection::ProtectedOrLiquidating, Protection::Unknown] {
        assert!(plan(&aged, expected(&aged, 2), Action::RelatedTransfer { quantity: q(1),
            destination_lot: 2, buyer: 20, protection }).is_err());
    }
}

#[test]
fn repeat_wrong_version_stale_candidate_and_restore_do_not_reaccrue() {
    let s = initial();
    let c = accrue(&s, 3, 0, 1);
    let done = restore(&next(&s, &c));
    assert_eq!(c.against(&done), Err(Error::Stale));
    assert_eq!(plan(&done, expected(&s, 1), Action::Accrue { to: t(7), development_seconds: 0 }), Err(Error::Stale));
    assert_eq!(plan(&done, expected(&done, 1), repair(300, ExpenseUse::Repair, 10)), Err(Error::Duplicate));
    let mut changed_receipt_version = expected(&done, 1);
    changed_receipt_version.receipt.revision = 2;
    assert_eq!(plan(&done, changed_receipt_version, repair(300, ExpenseUse::Repair, 10)), Err(Error::Duplicate));
    let mut stale = expected(&done, 2); stale.source_snapshot += 1;
    assert_eq!(plan(&done, stale, repair(300, ExpenseUse::Repair, 10)), Err(Error::Stale));
    assert_eq!(plan(&done, expected(&done, 2), Action::Accrue { to: t(3), development_seconds: 0 }), Err(Error::Invalid));
    assert_eq!(amount(&accrue(&done, 7, 0, 2), 0), 57);
}

#[test]
fn local_clock_freezes_depreciation_but_not_established_rent_obligation() {
    let mut d = delivered();
    d.profile.clock = ClockKind::LocalDevelopment;
    d.lease = Some(Lease { source: r(200), operator: 20, start: t(0), end: t(7), total: m(101) });
    let s = Snapshot::new_delivery(d).unwrap();
    let c = accrue(&s, 3, 0, 1);
    assert_eq!(amount(&c, 0), 0);
    assert_eq!(amount(&c, 1), 43);
    let frozen = restore(&next(&s, &c));
    let resume = accrue(&frozen, 7, 4, 2);
    assert_eq!(amount(&resume, 0), 56);
    assert_eq!(amount(&resume, 1), 58);
    assert_eq!(plan(&s, expected(&s, 2), Action::Accrue { to: t(1), development_seconds: 2 }), Err(Error::Invalid));
}

#[test]
fn unknown_expense_keeps_profit_blocked_and_failure_can_retry_same_revision() {
    let s = initial();
    assert_eq!(plan(&s, expected(&s, 1), repair(300, ExpenseUse::Unknown, 10)), Err(Error::Unknown));
    let c = plan(&s, expected(&s, 1), repair(300, ExpenseUse::Repair, 10)).unwrap();
    assert!(!c.establishes_distributable_profit());
    assert_eq!(s, initial());
    assert_eq!(next(&s, &c).revision(), 1);
}

#[test]
fn reject_unsupported_fractional_cost_quantum_and_partial_indivisible_scope() {
    let mut d = delivered(); d.quantity = q(3); // 101/3 is not integer cents per quantum
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Unsupported));
    d.profile.cost_quantum = q(3); // whole indivisible scope is supported
    let s = Snapshot::new_delivery(d).unwrap();
    assert_eq!(plan(&s, expected(&s, 1), Action::Exit { quantity: q(1),
        kind: ExitKind::Sale, protection: Protection::Clear }), Err(Error::Unsupported));
    d.profile.cost_quantum = q(0);
    assert_eq!(Snapshot::new_delivery(d), Err(Error::Unsupported));
}

#[test]
fn fixed_shape_restore_checked_arithmetic_and_maximum_money() {
    let s = initial();
    let mut wire = serde_json::to_value(&s).unwrap();
    wire["revision"] = serde_json::json!(u64::MAX);
    let max_revision: Snapshot = serde_json::from_value(wire).unwrap();
    assert_eq!(plan(&max_revision, expected(&max_revision, 1), repair(300, ExpenseUse::Repair, 10)), Err(Error::Overflow));
    let mut wire = serde_json::to_value(&s).unwrap();
    wire["offset"] = serde_json::json!(i64::MAX);
    let bad: Snapshot = serde_json::from_value(wire).unwrap();
    assert_eq!(bad.validate(), Err(Error::Overflow));
    let mut wire = serde_json::to_value(&s).unwrap();
    wire["age_seconds"] = serde_json::json!(1);
    assert_eq!(serde_json::from_value::<Snapshot>(wire).unwrap().validate(), Err(Error::Invalid));
    let mut wire = serde_json::to_value(&s).unwrap();
    wire["invented_profit"] = serde_json::json!(99);
    assert!(serde_json::from_value::<Snapshot>(wire).is_err());
    let mut d = delivered(); d.basis = m(i64::MAX); d.profile.residual = m(0);
    d.profile.life_seconds = i64::MAX as u64;
    let huge = Snapshot::new_delivery(d).unwrap();
    let c = accrue(&huge, i64::MAX, 0, 1);
    assert_eq!(amount(&c, 0), i64::MAX);
    assert_eq!(next(&huge, &c).carrying().unwrap(), m(0));
}

#[test]
fn input_size_excess_quantity_and_malformed_snapshot_fail_without_consuming_base() {
    let s = initial();
    assert_eq!(parse_snapshot(&vec![b' '; MAX_SNAPSHOT_BYTES + 1]), Err(Error::Limit));
    assert_eq!(parse_snapshot(b"{}"), Err(Error::Invalid));
    assert_eq!(plan(&s, expected(&s, 1), Action::Exit { quantity: q(2),
        kind: ExitKind::Recovery, protection: Protection::Clear }), Err(Error::Invalid));
    let c = plan(&s, expected(&s, 1), Action::Exit { quantity: q(1),
        kind: ExitKind::Recovery, protection: Protection::Clear }).unwrap();
    assert_eq!(c.derecognition.unwrap().remaining_cost, m(101));
    let empty = restore(&next(&s, &c));
    assert_eq!(plan(&empty, expected(&empty, 2), repair(300, ExpenseUse::Repair, 10)), Err(Error::Invalid));
}
