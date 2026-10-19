use mycity_core::{
    assembly::{assemble, WorldView},
    economy::{Command, Economy, Phase, Reason, DAY, HOUR},
    transaction::{Action, Caller, Engine, Envelope, TxError},
    units::{Money, Quantity},
};
use serde_json::{json, Value};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-economy-opening.json");
fn engine() -> Engine {
    Engine::new(assemble(INPUT).unwrap(), vec![]).unwrap()
}
fn command(e: &Engine, c: Command) -> Envelope {
    let v = e.world().view();
    Envelope {
        world: v.scope.world(),
        branch: v.scope.branch(),
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: v.revision,
        actions: vec![Action::Economy { command: c }],
    }
}
fn apply(e: &mut Engine, c: Command) {
    let cmd = command(e, c);
    e.execute(Caller::System, cmd).unwrap();
}
fn state(e: &Engine) -> Economy {
    e.world().view().economy.unwrap()
}
fn run_to(e: &mut Engine, t: u64) {
    while state(e).time < t {
        let dt = (t - state(e).time).min(HOUR);
        apply(e, Command::Advance { seconds: dt as u32 });
        invariants(&e.world().view());
    }
}
fn cash(v: &WorldView, id: u32) -> i64 {
    v.accounts.iter().find(|a| a.id == id).unwrap().cash.value()
}
fn invariants(v: &WorldView) {
    let e = v.economy.unwrap();
    assert_eq!(
        v.accounts.iter().map(|a| a.cash.value()).sum::<i64>(),
        e.initial_cash.value()
    );
    let stock = |id| v.stocks.iter().find(|s| s.id == id).unwrap();
    assert_eq!(
        stock(e.supplier_stock).quantity.value()
            + stock(e.raw_stock).quantity.value()
            + e.raw_transit.map_or(0, |f| f.quantity.value())
            + e.raw_used.value(),
        e.initial_raw.value()
    );
    let held = e
        .product_stocks
        .iter()
        .map(|id| stock(*id).quantity.value())
        .sum::<i64>();
    let held_basis = e
        .product_stocks
        .iter()
        .map(|id| stock(*id).basis.value())
        .sum::<i64>();
    let shipping = e
        .sales_transit
        .iter()
        .flatten()
        .map(|s| s.fragment.quantity.value())
        .sum::<i64>();
    let shipping_basis = e
        .sales_transit
        .iter()
        .flatten()
        .map(|s| s.fragment.basis.value())
        .sum::<i64>();
    assert_eq!(
        e.produced.value(),
        held + shipping + e.consumed_f.value() + e.consumed_g.value()
    );
    assert_eq!(
        held_basis + shipping_basis + e.cogs.value(),
        e.raw_used.value() / 1000 * 400 + e.direct_wages.value()
    );
    assert!(e.raw_transit.map_or(0, |f| f.quantity.value()) <= 10000);
    assert!(shipping <= 10000);
    for (i, id) in e.accounts.iter().enumerate() {
        let a = v.accounts.iter().find(|a| a.id == *id).unwrap();
        assert!(a.cash >= a.protected);
        assert!(a.protected >= e.protection[i]);
    }
    for d in e.records {
        assert!(d.worked_hours <= 8);
        assert!(d.freight_q.value() <= 20000);
    }
    for c in e.claims.iter().flatten() {
        assert!(c.paid <= c.principal);
    }
}
fn variant(change: impl FnOnce(&mut Value)) -> Engine {
    let mut v: Value = serde_json::from_slice(INPUT).unwrap();
    change(&mut v);
    Engine::new(assemble(&serde_json::to_vec(&v).unwrap()).unwrap(), vec![]).unwrap()
}
#[test]
fn pe0_reaches_all_independent_thirty_day_oracles_through_actual_hourly_transactions() {
    let mut e = engine();
    run_to(&mut e, 30 * DAY);
    let v = e.world().view();
    let s = state(&e);
    assert_eq!(
        [cash(&v, 1), cash(&v, 2), cash(&v, 3), cash(&v, 4)],
        [808000, 56000, 816000, 1320000]
    );
    assert_eq!(s.phase, Phase::Finished);
    assert_eq!(s.closed_days, 30);
    assert_eq!(s.tax_accrued.value(), 24000);
    assert_eq!(s.claims[2].unwrap().outstanding(), Money::ZERO);
    assert_eq!(
        (
            s.consumed_f.value(),
            s.consumed_g.value(),
            s.produced.value(),
            s.raw_used.value()
        ),
        (180000, 120000, 300000, 300000)
    );
    assert_eq!(
        v.stocks
            .iter()
            .find(|s| s.id == 1)
            .unwrap()
            .quantity
            .value(),
        300000
    );
    assert!(v
        .stocks
        .iter()
        .filter(|s| s.id != 1)
        .all(|s| s.quantity == Quantity::ZERO && s.basis == Money::ZERO));
    assert_eq!(
        (s.sales.value(), s.cogs.value(), s.direct_wages.value()),
        (600000, 360000, 240000)
    );
    assert_eq!(s.supplier_sales, s.supplier_cogs);
    assert_eq!(e.journal().len(), 720);
    assert!(e.owned_capacity_bytes() <= mycity_core::transaction::ENGINE_BYTE_BUDGET);
}
#[test]
fn pe1_low_household_cash_records_failed_whole_order_retains_costed_old_lots_and_keeps_running() {
    let mut e = variant(|v| {
        v["accounts"][1]["cash"] = json!(10000);
        v["sources"][1]["amount"] = json!(10000);
        v["bootstrap"]["branch"] = json!(2);
    });
    run_to(&mut e, DAY);
    assert_eq!(cash(&e.world().view(), 2), 5200);
    run_to(&mut e, 2 * DAY);
    assert_eq!(cash(&e.world().view(), 2), 400);
    run_to(&mut e, 3 * DAY);
    let s = state(&e);
    let r = s.records[2];
    assert_eq!(cash(&e.world().view(), 2), 7600);
    assert_eq!(r.f_result, Reason::NoFunds);
    assert_eq!(r.g_result, Reason::Accepted);
    assert_eq!(
        (
            r.sales.value(),
            r.cogs.value(),
            r.produced.value(),
            s.tax_accrued.value()
        ),
        (8000, 4800, 10000, 1920)
    );
    let v = e.world().view();
    let retained = v.stocks.iter().find(|s| s.id == 102).unwrap();
    assert_eq!(
        (retained.quantity.value(), retained.basis.value()),
        (6000, 7200)
    );
    run_to(&mut e, 4 * DAY);
    assert_eq!(state(&e).records[3].produced.value(), 10000);
    assert!(
        e.world()
            .view()
            .stocks
            .iter()
            .filter(|s| s.id >= 100)
            .map(|s| s.quantity.value())
            .sum::<i64>()
            > 0
    );
    run_to(&mut e, 30 * DAY);
    assert_eq!(state(&e).closed_days, 30);
    invariants(&e.world().view());
}
#[test]
fn pe1_twenty_units_exhaust_without_new_labor_wages_products_or_deleting_existing_tax() {
    let mut e = variant(|v| {
        v["stocks"][0]["quantity"] = json!(20000);
        v["stocks"][0]["basis"] = json!(8000);
        v["sources"][4]["amount"] = json!(8000);
        v["bootstrap"]["branch"] = json!(3);
    });
    run_to(&mut e, 3 * DAY);
    let s = state(&e);
    assert_eq!(s.records[2].reason, Reason::NoInput);
    assert!(!s.records[2].accepted);
    assert_eq!(s.records[2].worked_hours, 0);
    assert_eq!(s.records[2].gross_paid, Money::ZERO);
    assert_eq!(
        (
            s.produced.value(),
            s.direct_wages.value(),
            s.tax_accrued.value()
        ),
        (20000, 16000, 1600)
    );
    assert_eq!(cash(&e.world().view(), 3), 616000);
    run_to(&mut e, 30 * DAY);
    assert_eq!(state(&e).claims[2].unwrap().outstanding(), Money::ZERO);
    assert_eq!(state(&e).produced.value(), 20000);
}
#[test]
fn pe1_existing_debt_is_part_paid_then_handed_off_without_changing_parties_or_disappearing_on_exit()
{
    let mut e = variant(|v| {
        v["economy"]["debt"] = json!({"source":42,"payer":3,"payee":1,"amount":700000,"due":0});
        v["bootstrap"]["branch"] = json!(4);
    });
    apply(&mut e, Command::SettleDebt { claim: 1 });
    assert_eq!(state(&e).claims[0].unwrap().outstanding().value(), 100000);
    assert_eq!(cash(&e.world().view(), 3), 0);
    apply(&mut e, Command::BeginExit);
    let c = command(&e, Command::Close);
    let before = e.state_hash();
    assert!(e.execute(Caller::System, c).is_err());
    assert_eq!(e.state_hash(), before);
    apply(&mut e, Command::HandoffDebt { claim: 1 });
    apply(&mut e, Command::Close);
    let s = state(&e);
    assert!(!s.active);
    let claim = s.claims[0].unwrap();
    assert_eq!(
        (claim.payer, claim.payee, claim.outstanding().value()),
        (3, 1, 100000)
    );
    assert!(claim.registry);
    apply(&mut e, Command::Advance { seconds: 3600 });
    assert_eq!(state(&e).produced.value(), 0);
    assert_eq!(state(&e).claims[0], Some(claim));
    invariants(&e.world().view());
}
#[test]
fn pe1_prepayment_cancel_keeps_delivered_labor_and_creates_only_one_real_refund() {
    let mut e = engine();
    apply(
        &mut e,
        Command::Prepay {
            quantity: Quantity::new(20000).unwrap(),
        },
    );
    run_to(&mut e, DAY);
    assert_eq!(state(&e).prepayment.unwrap().dispatched.value(), 10000);
    let labor = state(&e).direct_wages;
    let consumed = state(&e).consumed_f;
    let c = command(&e, Command::CancelFuture);
    let first = e.execute(Caller::System, c.clone()).unwrap();
    let before = e.state_hash();
    assert_eq!(e.execute(Caller::System, c).unwrap(), first);
    assert_eq!(e.state_hash(), before);
    let debt = state(&e).claims[1].unwrap();
    assert_eq!((debt.principal.value(), debt.paid.value()), (4000, 4000));
    apply(&mut e, Command::CancelFuture);
    assert_eq!(state(&e).claims[1], Some(debt));
    assert_eq!(state(&e).direct_wages, labor);
    assert_eq!(state(&e).consumed_f, consumed);
    invariants(&e.world().view());
}
#[test]
fn pe1_insolvent_supplier_retains_refund_claim_and_cancellation_does_not_mint_money() {
    let mut e = variant(|v| {
        v["economy"]["debt"] = json!({"source":43,"payer":4,"payee":1,"amount":1207000,"due":0});
        v["bootstrap"]["branch"] = json!(5);
    });
    apply(
        &mut e,
        Command::Prepay {
            quantity: Quantity::new(20000).unwrap(),
        },
    );
    run_to(&mut e, DAY);
    apply(&mut e, Command::SettleDebt { claim: 1 });
    assert_eq!(cash(&e.world().view(), 4), 1000);
    apply(&mut e, Command::CancelFuture);
    let c = state(&e).claims[1].unwrap();
    assert_eq!(
        (c.principal.value(), c.paid.value(), c.outstanding().value()),
        (4000, 1000, 3000)
    );
    let money = e
        .world()
        .view()
        .accounts
        .iter()
        .map(|a| a.cash.value())
        .sum::<i64>();
    apply(&mut e, Command::CancelFuture);
    assert_eq!(state(&e).claims[1], Some(c));
    assert_eq!(
        e.world()
            .view()
            .accounts
            .iter()
            .map(|a| a.cash.value())
            .sum::<i64>(),
        money
    );
}
#[test]
fn suspended_guaranteed_wages_are_expense_and_cumulative_tax_can_fall_without_cash_refund() {
    let mut e = engine();
    run_to(&mut e, DAY);
    assert_eq!(state(&e).tax_accrued.value(), 800);
    run_to(&mut e, DAY + HOUR);
    apply(&mut e, Command::SuspendShift);
    run_to(&mut e, 2 * DAY);
    let s = state(&e);
    assert_eq!(s.records[1].worked_hours, 0);
    assert_eq!(s.records[1].gross_paid.value(), 8000);
    assert_eq!(s.records[1].expense.value(), 8000);
    assert_eq!(s.records[1].produced.value(), 0);
    assert_eq!(s.tax_accrued.value(), 0);
    assert_eq!(s.records[1].tax_delta_cents, -800);
    assert!(s.claims[2].is_none());
    assert_eq!(
        e.world()
            .view()
            .stocks
            .iter()
            .find(|s| s.id == 2)
            .unwrap()
            .quantity
            .value(),
        10000
    );
    run_to(&mut e, 3 * DAY);
    assert_eq!(state(&e).tax_accrued.value(), 800);
    assert_eq!(state(&e).records[2].raw_acquired.value(), 0);
    invariants(&e.world().view());
}
#[test]
fn advances_split_at_events_restore_equivalent_business_state_and_never_open_next_day_early() {
    let mut a = engine();
    let mut b = engine();
    run_to(&mut a, DAY);
    for _ in 0..48 {
        apply(&mut b, Command::Advance { seconds: 1800 });
    }
    assert_eq!(a.world().view().accounts, b.world().view().accounts);
    assert_eq!(a.world().view().stocks, b.world().view().stocks);
    assert_eq!(state(&a), state(&b));
    let s = state(&a);
    assert_eq!(s.phase, Phase::Open);
    assert_eq!(s.day, 2);
    assert!(!s.records[1].accepted);
    assert_eq!(s.records[1].raw_acquired, Quantity::ZERO);
    run_to(&mut a, DAY + 1800);
    assert!(state(&a).raw_transit.is_some());
    let v = a.world().view();
    assert_eq!(
        v.stocks.iter().find(|s| s.id == 2).unwrap().quantity,
        Quantity::ZERO
    );
    invariants(&v);
}
#[test]
fn economic_caller_protection_stock_and_disk_failure_gates_leave_no_partial_commit() {
    let mut e = engine();
    let cmd = command(&e, Command::Advance { seconds: 1800 });
    let before = e.state_hash();
    assert_eq!(
        e.execute(Caller::Account(3), cmd.clone()),
        Err(TxError::Unauthorized)
    );
    assert_eq!(e.state_hash(), before);
    let cmd = command(&e, Command::Advance { seconds: 1800 });
    assert_eq!(
        e.execute_recorded(Caller::System, cmd, |_, _| Err("disk")),
        Err("disk")
    );
    assert_eq!(e.state_hash(), before);
    apply(&mut e, Command::Advance { seconds: 1800 });
    let before = e.state_hash();
    let mut c = command(&e, Command::Advance { seconds: 1 });
    c.actions = vec![Action::ReleaseProtection {
        account: e.world().account_ref(3).unwrap(),
        amount: Money::new(8000).unwrap(),
    }];
    assert!(e.execute(Caller::System, c).is_err());
    assert_eq!(e.state_hash(), before);
    let mut c = command(&e, Command::Advance { seconds: 1 });
    c.actions = vec![Action::MoveStock {
        from: e.world().stock_ref(1).unwrap(),
        to: e.world().stock_ref(2).unwrap(),
        quantity: Quantity::new(1000).unwrap(),
    }];
    assert_eq!(e.execute(Caller::System, c), Err(TxError::Unauthorized));
    assert_eq!(e.state_hash(), before);
    let mut c = command(&e, Command::Advance { seconds: 3600 });
    c.actions.push(Action::Economy {
        command: Command::Close,
    });
    assert!(e.execute(Caller::System, c).is_err());
    assert_eq!(e.state_hash(), before);
}
#[test]
fn malformed_profiles_overlapping_batches_and_conflicting_region_clock_are_rejected() {
    let original: Value = serde_json::from_slice(INPUT).unwrap();
    for key in ["electric_wh", "water_l", "travel_seconds"] {
        let mut v = original.clone();
        v["economy"][key] = json!(1);
        assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    let mut v = original.clone();
    v["economy"]["product_stocks"][1] = v["economy"]["product_stocks"][0].clone();
    assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
    let region: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-region-opening.json")).unwrap();
    let mut v = original;
    v["region"] = region["region"].clone();
    assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
}

#[test]
fn independent_capacity_pool_cannot_be_enabled_beside_private_economic_corridor() {
    use mycity_core::transaction::Resource;
    assert!(Engine::new(
        assemble(INPUT).unwrap(),
        vec![Resource::new(1, 10).unwrap()]
    )
    .is_err());
}

#[test]
fn month_end_tax_cash_shortfall_survives_activity_handoff_and_never_mints_payment() {
    let mut e = engine();
    run_to(&mut e, DAY);
    let mut env = command(&e, Command::Advance { seconds: 1 });
    env.actions = vec![Action::Transfer {
        from: e.world().account_ref(3).unwrap(),
        to: e.world().account_ref(4).unwrap(),
        amount: Money::new(608000).unwrap(),
    }];
    e.execute(Caller::System, env).unwrap();
    run_to(&mut e, 30 * DAY);
    let tax = state(&e).claims[2].unwrap();
    assert_eq!(tax.principal.value(), 800);
    assert_eq!(tax.paid, Money::ZERO);
    assert_eq!(cash(&e.world().view(), 3), 0);
    apply(&mut e, Command::BeginExit);
    let close = command(&e, Command::Close);
    assert!(e.execute(Caller::System, close).is_err());
    apply(&mut e, Command::HandoffDebt { claim: 3 });
    apply(&mut e, Command::Close);
    assert_eq!(state(&e).claims[2].unwrap().outstanding().value(), 800);
    invariants(&e.world().view());
}
#[test]
fn external_debt_cannot_reuse_generated_refund_or_tax_source() {
    for source in [2, 3] {
        let mut v: Value = serde_json::from_slice(INPUT).unwrap();
        v["economy"]["debt"] = json!({"source":source,"payer":3,"payee":1,"amount":100,"due":0});
        assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
