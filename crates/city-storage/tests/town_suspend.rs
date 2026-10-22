use mycity_core::{
    timeline::{Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "town-suspend-operations-source-fixture-v1";
const LOSS_UTILITY_COST_PER_OUTPUT_Q: i64 = 10_000;
static ID: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-town-suspend-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("TOWN_SUSPEND_ARTIFACT_CREATED={}", path.display());
        Self(path)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("TOWN_SUSPEND_ARTIFACT_PRESERVED={}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn opening() -> Vec<u8> {
    opening_with_utility_cost(1)
}

fn opening_with_utility_cost(utility_cost: i64) -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":100000}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
        serde_json::json!({"id":8,"kind":"cash_capital","amount":200000}),
    ]);
    value["accounts"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":5,"role":"household","cash":200000,"source":8}));
    value["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":100000,"basis":100000,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    value["town"] = serde_json::json!({
        "schema_version":1,"id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":1000},{"id":2,"account":5,"households":1,"need_good":0,"need_per_household":10,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1},{"account":4,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100},{"shop_id":2,"account":4,"stock_id":1,"good":0,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1},{"quote_id":2,"shop":2,"household_group":2,"good":0,"reachable":true,"route_source":78,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}]
    });
    value["town"]["recipes"][0]["utility_cost_per_output_q"] = serde_json::json!(utility_cost);
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path) -> Store {
    Store::create(path, &opening(), vec![], RULES).unwrap()
}
fn create_loss_fixture(path: &Path) -> Store {
    Store::create(
        path,
        &opening_with_utility_cost(LOSS_UTILITY_COST_PER_OUTPUT_Q),
        vec![],
        RULES,
    )
    .unwrap()
}
fn envelope(store: &Store, action: Action) -> Envelope {
    let engine = store.engine().unwrap();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: store.committed_sequence() + 1,
        expected_revision: engine.world().view().revision,
        actions: vec![action],
    }
}
fn execute(store: &mut Store, action: Action) {
    let request = envelope(store, action);
    let outcome = store.execute(Caller::System, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
}
fn add_payroll(store: &mut Store, id: u32, payer: u32) {
    execute(
        store,
        Action::Timeline {
            command: T::AddPayroll {
                spec: PayrollSpec {
                    id,
                    payer,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: Money::new(100).unwrap(),
                    cycle: Cycle::Daily,
                    ends_at: 7 * DAY,
                },
            },
        },
    );
}
fn add_fixture_claim(store: &mut Store) {
    execute(
        store,
        Action::Timeline {
            command: T::Claims {
                first_id: 1,
                count: 1,
                payer: 3,
                payee: 4,
                amount: Money::new(1_000).unwrap(),
                due: 3 * DAY,
            },
        },
    );
}
fn drive(store: &mut Store, target: u64) {
    for _ in 0..100 {
        execute(
            store,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("Timeline did not complete {target}");
}

fn equity_projection(store: &Store, account: u32) -> i128 {
    let view = store.engine().unwrap().world().view();
    let timeline = store.engine().unwrap().timeline().unwrap();
    let cash = i128::from(
        view.accounts
            .iter()
            .find(|row| row.id == account)
            .unwrap()
            .cash
            .value(),
    );
    let inventory_basis = view
        .stocks
        .iter()
        .filter(|row| row.owner == account)
        .map(|row| i128::from(row.basis.value()))
        .sum::<i128>();
    let payroll_receivable = timeline
        .payroll
        .iter()
        .filter(|row| row.spec.payee == account)
        .map(|row| i128::from(row.accrued.value() - row.paid.value()))
        .sum::<i128>();
    let payroll_payable = timeline
        .payroll
        .iter()
        .filter(|row| row.spec.payer == account)
        .map(|row| i128::from(row.accrued.value() - row.paid.value()))
        .sum::<i128>();
    let claims_receivable = timeline
        .claims
        .iter()
        .filter(|row| row.payee == account)
        .map(|row| i128::from(row.principal.value() - row.paid.value()))
        .sum::<i128>();
    let claims_payable = timeline
        .claims
        .iter()
        .filter(|row| row.payer == account)
        .map(|row| i128::from(row.principal.value() - row.paid.value()))
        .sum::<i128>();
    cash + inventory_basis + payroll_receivable + claims_receivable
        - payroll_payable
        - claims_payable
}

fn total_cash(store: &Store) -> i128 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .map(|row| i128::from(row.cash.value()))
        .sum()
}

#[test]
fn stop_operations_is_atomic_durable_and_does_not_stop_unrelated_business() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path);
    execute(&mut store, Action::Timeline { command: T::Enable });
    add_payroll(&mut store, 1, 3);
    add_payroll(&mut store, 2, 4);
    drive(&mut store, DAY);
    execute(&mut store, Action::TownSettle { profile_id: 1 });
    let day1 = store.engine().unwrap().world().view();
    assert!(day1.town.unwrap().state.last_summary.actual_sales > Quantity::ZERO);
    assert!(day1.town.unwrap().state.last_summary.produced > Quantity::ZERO);
    let paid_before = store.engine().unwrap().timeline().unwrap().payroll[0].paid;
    let cash_before = day1.accounts.iter().find(|row| row.id == 3).unwrap().cash;
    let stock_before = day1.stocks.iter().find(|row| row.id == 2).unwrap().quantity;
    let stopped_shop_stock_before = day1.stocks.iter().find(|row| row.id == 3).unwrap().quantity;
    let continuing_shop_stock_before = day1.stocks.iter().find(|row| row.id == 1).unwrap().quantity;

    let request = envelope(
        &store,
        Action::TownSuspendOperations {
            profile_id: 1,
            company_account: 3,
        },
    );
    let lost_reply = store.execute_with_observer(Caller::System, request.clone(), |point| {
        if point == WritePoint::Applied {
            Err(std::io::Error::other("simulate lost suspension reply"))
        } else {
            Ok(())
        }
    });
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let after = store.engine().unwrap().world().view();
    assert_eq!(
        after.town.unwrap().state.suspended_operators.as_slice()[0].account,
        3
    );
    assert!(!store.engine().unwrap().timeline().unwrap().payroll[0].working);
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().payroll[0].paid,
        paid_before
    );
    assert_eq!(
        after.accounts.iter().find(|row| row.id == 3).unwrap().cash,
        cash_before
    );
    assert_eq!(
        after
            .stocks
            .iter()
            .find(|row| row.id == 2)
            .unwrap()
            .quantity,
        stock_before
    );
    let hash_after_suspend = store.state_hash().unwrap();
    assert!(store
        .execute(Caller::System, request.clone())
        .unwrap()
        .model
        .is_ok());
    assert_eq!(store.state_hash().unwrap(), hash_after_suspend);
    let cash_at_suspend = store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == 3)
        .unwrap()
        .cash;
    store.checkpoint().unwrap();
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let rejected = store.execute(
        Caller::System,
        envelope(
            &store,
            Action::Timeline {
                command: T::AddPayroll {
                    spec: PayrollSpec {
                        id: 3,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: Money::new(100).unwrap(),
                        cycle: Cycle::Daily,
                        ends_at: 7 * DAY,
                    },
                },
            },
        ),
    );
    assert!(rejected.unwrap().model.is_err());
    drive(&mut store, 2 * DAY);
    execute(&mut store, Action::TownSettle { profile_id: 1 });
    let view = store.engine().unwrap().world().view();
    assert!(view.town.unwrap().state.last_summary.actual_sales > Quantity::ZERO);
    assert_eq!(
        view.stocks.iter().find(|row| row.id == 3).unwrap().quantity,
        stopped_shop_stock_before
    );
    assert!(
        view.stocks.iter().find(|row| row.id == 1).unwrap().quantity < continuing_shop_stock_before
    );
    assert!(view.accounts.iter().find(|row| row.id == 3).unwrap().cash <= cash_at_suspend);
    assert_eq!(
        view.stocks.iter().find(|row| row.id == 2).unwrap().quantity,
        stock_before
    );
    assert!(view.town.unwrap().state.last_summary.wages_accrued > Money::ZERO);
}

#[test]
fn loss_fixture_measures_operating_loss_then_suspends_only_that_business() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create_loss_fixture(&path);
    execute(&mut store, Action::Timeline { command: T::Enable });
    add_payroll(&mut store, 1, 3);
    add_payroll(&mut store, 2, 4);
    add_fixture_claim(&mut store);
    let claim_at_open = store.engine().unwrap().timeline().unwrap().claims[0];
    assert_eq!(claim_at_open.payer, 3);
    assert_eq!(claim_at_open.payee, 4);
    assert_eq!(claim_at_open.paid, Money::ZERO);
    let initial_equity = equity_projection(&store, 3);

    drive(&mut store, DAY);
    execute(&mut store, Action::TownSettle { profile_id: 1 });
    let before_stop = store.engine().unwrap().world().view();
    let summary = before_stop.town.as_ref().unwrap().state.last_summary;
    let pre_stop_equity = equity_projection(&store, 3);
    assert_eq!(summary.produced, Quantity::ZERO);
    assert!(summary.wages_accrued > Money::ZERO);
    assert!(summary.idle_labor_expense > Money::ZERO);
    assert!(
        pre_stop_equity < initial_equity,
        "idle payroll must reduce equity"
    );
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().claims[0],
        claim_at_open
    );
    let pre_stop_cash = before_stop
        .accounts
        .iter()
        .find(|row| row.id == 3)
        .unwrap()
        .cash;
    let pre_stop_raw = before_stop
        .stocks
        .iter()
        .find(|row| row.id == 2)
        .unwrap()
        .quantity;
    let pre_stop_finished = before_stop
        .stocks
        .iter()
        .find(|row| row.id == 3)
        .unwrap()
        .quantity;
    let other_shop_stock_before = before_stop
        .stocks
        .iter()
        .find(|row| row.id == 1)
        .unwrap()
        .quantity;

    execute(
        &mut store,
        Action::TownSuspendOperations {
            profile_id: 1,
            company_account: 3,
        },
    );
    let equity_at_stop = equity_projection(&store, 3);
    assert_eq!(equity_at_stop, pre_stop_equity);
    let payroll_at_stop = *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .payroll
        .iter()
        .find(|row| row.spec.payer == 3)
        .unwrap();
    let claim_at_stop = store.engine().unwrap().timeline().unwrap().claims[0];
    assert_eq!(claim_at_stop, claim_at_open);
    let cash_before_next_day = total_cash(&store);

    drive(&mut store, 2 * DAY);
    execute(&mut store, Action::TownSettle { profile_id: 1 });
    let after_next_day = store.engine().unwrap().world().view();
    let next_summary = after_next_day.town.as_ref().unwrap().state.last_summary;
    assert!(
        next_summary.actual_sales > Quantity::ZERO,
        "the other shop must sell"
    );
    assert_eq!(next_summary.produced, Quantity::ZERO);
    assert!(
        after_next_day
            .stocks
            .iter()
            .find(|row| row.id == 1)
            .unwrap()
            .quantity
            < other_shop_stock_before
    );
    assert_eq!(
        after_next_day
            .stocks
            .iter()
            .find(|row| row.id == 2)
            .unwrap()
            .quantity,
        pre_stop_raw
    );
    assert_eq!(
        after_next_day
            .stocks
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .quantity,
        pre_stop_finished
    );
    assert_eq!(
        after_next_day
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .cash,
        pre_stop_cash
    );
    assert_eq!(total_cash(&store), cash_before_next_day);
    let payroll_after = *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .payroll
        .iter()
        .find(|row| row.spec.payer == 3)
        .unwrap();
    assert!(!payroll_after.working);
    assert_eq!(payroll_after.next_due, None);
    assert_eq!(payroll_after.work_seconds, payroll_at_stop.work_seconds);
    assert_eq!(payroll_after.accrued, payroll_at_stop.accrued);
    assert_eq!(payroll_after.billed, payroll_at_stop.billed);
    assert_eq!(payroll_after.paid, payroll_at_stop.paid);
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().claims[0],
        claim_at_stop
    );
    assert_eq!(equity_projection(&store, 3), equity_at_stop);
}
