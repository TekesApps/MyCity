use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "town-business-exit-store-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-town-business-exit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        eprintln!("TOWN_BUSINESS_EXIT_ARTIFACT={}", path.display());
        Self(fs::canonicalize(path).unwrap())
    }

    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() {
            eprintln!("TOWN_BUSINESS_EXIT_PRESERVED={}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn policy(claims_complete: bool, claims: Value, payroll: Value) -> Value {
    json!({
        "policy_id": 1,
        "company_account": 3,
        "requester": 2,
        "executor": 1,
        "legal_clearance": "cleared",
        "priority_policy": {
            "order": ["tax", "wage", "secured_debt", "unsecured_debt", "other"],
            "within_tier": "pro_rata_by_outstanding_stable_id"
        },
        "claims_complete": claims_complete,
        "tax_assessment_complete": true,
        "asset_register_complete": true,
        "employment_exit_terms_complete": true,
        "claims": claims,
        "payroll": payroll
    })
}

fn opening(
    company_cash: i64,
    make_inventory: bool,
    claims_complete: bool,
    claims: Value,
    payroll: Value,
) -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        json!({"id": 6, "kind": "inventory_capital", "amount": if make_inventory { 20 } else { 0 }}),
        json!({"id": 7, "kind": "inventory_capital", "amount": 0}),
    ]);
    for account in value["accounts"].as_array_mut().unwrap() {
        if account["id"] == 3 {
            account["cash"] = company_cash.into();
            account["source"] = 3.into();
        }
    }
    value["sources"][2]["amount"] = company_cash.into();
    value["stocks"].as_array_mut().unwrap().extend([
        json!({"id": 2, "owner": 3, "commodity": "raw", "quantity": if make_inventory { 20 } else { 0 }, "basis": if make_inventory { 20 } else { 0 }, "source": 6}),
        json!({"id": 3, "owner": 3, "commodity": "product", "quantity": 0, "basis": 0, "source": 7}),
    ]);
    let market = if make_inventory {
        json!({
            "schema_version": 1,
            "id": 1,
            "households": [{"id": 1, "account": 2, "households": 1, "need_good": 1, "need_per_household": 10, "reference_price_per_q": 102}],
            "employers": [{"account": 3, "worker_limit": 1}],
            "shops": [{"shop_id": 1, "account": 3, "stock_id": 3, "good": 1, "shelf_price_per_q": 100}],
            "recipes": [{"recipe_id": 1, "employer": 3, "assigned_workers": 1, "input_stock_id": 2, "input_per_output": 1, "output_good": 1, "output_stock_id": 3, "output_per_worker_day": 20, "waste_per_output_q": 0, "available_utility_per_day": 1000, "utility_per_output_q": 1, "utility_cost_per_output_q": 1, "freight_cost_per_input_q": 0, "disposal_cost_per_input_q": 0, "utility_account": 1, "freight_account": 1, "disposal_account": 1}],
            "delivery": [{"quote_id": 1, "shop": 1, "household_group": 1, "good": 1, "reachable": true, "route_source": 77, "freight_account": 1, "utility_account": 1, "available_capacity": 100, "freight_per_q": 1, "utility_surcharge_per_q": 1}],
            "exit_policy": policy(claims_complete, claims, payroll)
        })
    } else {
        json!({
            "schema_version": 1,
            "id": 1,
            "households": [{"id": 1, "account": 2, "households": 1, "need_good": 1, "need_per_household": 10, "reference_price_per_q": 102}],
            "employers": [{"account": 3, "worker_limit": 1}],
            "shops": [{"shop_id": 1, "account": 3, "stock_id": 3, "good": 1, "shelf_price_per_q": 100}],
            "recipes": [],
            "delivery": [{"quote_id": 1, "shop": 1, "household_group": 1, "good": 1, "reachable": true, "route_source": 77, "freight_account": 1, "utility_account": 1, "available_capacity": 100, "freight_per_q": 1, "utility_surcharge_per_q": 1}],
            "exit_policy": policy(claims_complete, claims, payroll)
        })
    };
    value["town"] = market;
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path, opening: Vec<u8>) -> Store {
    Store::create(path, &opening, vec![], RULES).unwrap()
}

fn envelope(store: &Store, operation_id: u64, action: Action) -> Envelope {
    let view = store.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn next(store: &Store, action: Action) -> Envelope {
    envelope(store, store.committed_sequence() + 1, action)
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> u64 {
    let request = next(store, action);
    let id = request.operation_id;
    let outcome = store.execute(caller, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
    id
}

fn advance_to(store: &mut Store, target: u64) {
    for _ in 0..256 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
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
    panic!("timeline did not reach {target}");
}

fn add_payroll(store: &mut Store) {
    execute(
        store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: 3,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: Money::new(100).unwrap(),
                    cycle: Cycle::Daily,
                    ends_at: 4 * DAY,
                },
            },
        },
    );
}

fn add_claim(store: &mut Store, first_id: u64, count: u32, amount: i64) {
    execute(
        store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Claims {
                first_id,
                count,
                payer: 3,
                payee: 4,
                amount: Money::new(amount).unwrap(),
                due: 2 * DAY,
            },
        },
    );
}

fn market_settle(store: &mut Store) -> mycity_core::town_runtime::MarketSummary {
    execute(store, Caller::System, Action::TownSettle { profile_id: 1 });
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .last_summary
}

fn cash(store: &Store, account: u32) -> i64 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .unwrap()
        .cash
        .value()
}

fn quantity(store: &Store, stock: u32) -> i64 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .stocks
        .iter()
        .find(|row| row.id == stock)
        .unwrap()
        .quantity
        .value()
}

fn total_cash(store: &Store) -> i128 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum()
}

fn begin_exit_request(store: &Store) -> Envelope {
    next(
        store,
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    )
}

fn replay_exact(store: &mut Store, record: mycity_core::transaction::JournalRecord) {
    let view = store.engine().unwrap().world().view();
    let caller = if record.actor == 0 {
        Caller::System
    } else {
        Caller::Account(record.actor)
    };
    let outcome = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: record.operation_id,
                expected_revision: record.expected_revision,
                actions: record.actions.into_iter().flatten().collect(),
            },
        )
        .unwrap();
    assert_eq!(outcome.model, record.outcome);
    assert!(outcome.has_persistent_receipt);
}

fn execute_lost_applied(store: &mut Store, caller: Caller, request: Envelope) {
    let outcome = store.execute_with_observer(caller, request, |point| {
        if point == WritePoint::Applied {
            Err(std::io::Error::other("injected lost exit response"))
        } else {
            Ok(())
        }
    });
    assert!(outcome.is_err());
}

#[test]
fn liquidation_sells_existing_market_stock_then_pays_wages_and_debt_once_across_recovery() {
    let temp = Temp::new();
    let path = temp.world();
    let opening = opening(
        0,
        true,
        true,
        json!([{"id": 1, "payee": 4, "class": "unsecured_debt"}]),
        json!([{"id": 1, "payee": 2}]),
    );
    let mut store = create(&path, opening);
    let initial_household_cash = cash(&store, 2);
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    add_payroll(&mut store);
    add_claim(&mut store, 1, 1, 1_200);
    let initial_cash = total_cash(&store);

    advance_to(&mut store, DAY);
    let treasury = store.engine().unwrap().world().account_ref(1).unwrap();
    let company = store.engine().unwrap().world().account_ref(3).unwrap();
    execute(
        &mut store,
        Caller::Account(1),
        Action::Transfer {
            from: treasury,
            to: company,
            amount: Money::new(20).unwrap(),
        },
    );
    let day1 = market_settle(&mut store);
    assert_eq!(day1.produced, Quantity::new(20).unwrap());
    assert_eq!(day1.actual_sales, Quantity::new(10).unwrap());
    assert_eq!(quantity(&store, 2), 0);
    assert_eq!(quantity(&store, 3), 10);
    assert_eq!(cash(&store, 3), 1_000);
    assert_eq!(day1.freight, Money::new(10).unwrap());
    assert_eq!(day1.utilities, Money::new(30).unwrap());
    assert_eq!(
        cash(&store, 1),
        1_000_020,
        "startup grant returns as production utility fees; delivery freight and utilities add 20"
    );
    assert_eq!(
        total_cash(&store),
        initial_cash,
        "grant, utility payment and sales conserve cash"
    );
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().payroll[0].paid,
        Money::ZERO
    );

    let begin = begin_exit_request(&store);
    let begin_id = begin.operation_id;
    execute_lost_applied(&mut store, Caller::Account(2), begin.clone());
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .unwrap()
            .state
            .exit
            .unwrap()
            .phase,
        mycity_core::town_runtime::ExitPhase::Liquidating
    );
    let begin_record = store.receipt(begin_id).unwrap().unwrap();
    let begin_hash = store.state_hash().unwrap();
    replay_exact(&mut store, begin_record);
    assert_eq!(store.state_hash().unwrap(), begin_hash);

    advance_to(&mut store, 2 * DAY);
    let day2 = market_settle(&mut store);
    assert_eq!(
        day2.produced,
        Quantity::ZERO,
        "exit must stop new production"
    );
    assert_eq!(
        day2.actual_sales,
        Quantity::new(10).unwrap(),
        "existing inventory remains saleable"
    );
    assert_eq!(quantity(&store, 3), 0);
    assert_eq!(cash(&store, 3), 2_000);
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().claims[0].paid,
        Money::ZERO,
        "automatic Timeline payment must not bypass exit ranking"
    );
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().payroll[0].accrued,
        Money::new(800).unwrap()
    );
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().payroll[0].paid,
        Money::ZERO
    );
    let manual_claim_request = next(
        &store,
        Action::Timeline {
            command: TimelineCommand::SettleClaim { id: 1 },
        },
    );
    let manual_claim = store.execute(Caller::System, manual_claim_request).unwrap();
    assert!(manual_claim.has_persistent_receipt);
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().claims[0].paid,
        Money::ZERO
    );

    let settle = next(&store, Action::SettleExit { company: 3 });
    let settle_id = settle.operation_id;
    execute_lost_applied(&mut store, Caller::Account(1), settle.clone());
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let after_settlement_hash = store.state_hash().unwrap();
    let settled_record = store.receipt(settle_id).unwrap().unwrap();
    replay_exact(&mut store, settled_record);
    assert_eq!(store.state_hash().unwrap(), after_settlement_hash);
    assert_eq!(cash(&store, 3), 0);
    assert_eq!(
        cash(&store, 2),
        initial_household_cash + 800
            - (day1.actual_sales.value() + day2.actual_sales.value()) * 102,
        "household cash is its opening balance plus wages less actual purchases at 102 per unit"
    );
    assert_eq!(cash(&store, 4), 1_201_200);
    assert_eq!(
        total_cash(&store),
        initial_cash,
        "wage/debt settlement conserves cash"
    );
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().payroll[0].paid,
        Money::new(800).unwrap()
    );
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().claims[0].paid,
        Money::new(1_200).unwrap()
    );

    let close_request = next(&store, Action::CloseExit { company: 3 });
    let close_id = close_request.operation_id;
    let close = store.execute(Caller::Account(1), close_request).unwrap();
    assert!(close.model.is_ok());
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .unwrap()
            .state
            .exit
            .unwrap()
            .phase,
        mycity_core::town_runtime::ExitPhase::Closed
    );
    let closed_hash = store.state_hash().unwrap();
    let close_record = store.receipt(close_id).unwrap().unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(store.state_hash().unwrap(), closed_hash);
    replay_exact(&mut store, close_record);
    assert_eq!(store.state_hash().unwrap(), closed_hash);
    assert_eq!(cash(&store, 3), 0);

    advance_to(&mut store, 3 * DAY);
    let closed_day_settle = next(&store, Action::TownSettle { profile_id: 1 });
    let after_close = store.execute(Caller::System, closed_day_settle).unwrap();
    if let Ok(receipt) = after_close.model {
        assert_eq!(receipt.town.unwrap().produced, Quantity::ZERO);
        assert_eq!(
            store
                .engine()
                .unwrap()
                .world()
                .view()
                .town
                .unwrap()
                .state
                .last_summary
                .actual_sales,
            Quantity::ZERO
        );
    }
    assert_eq!(cash(&store, 3), 0);
    assert_eq!(quantity(&store, 2), 0);
    assert_eq!(quantity(&store, 3), 0);
}

#[test]
fn secured_claims_share_partial_exit_cash_by_stable_id_and_remain_liquidating() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(
        &path,
        opening(
            101,
            false,
            true,
            json!([
                {"id": 1, "payee": 4, "class": "secured_debt"},
                {"id": 2, "payee": 4, "class": "secured_debt"}
            ]),
            json!([]),
        ),
    );
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    add_claim(&mut store, 1, 1, 100);
    add_claim(&mut store, 2, 1, 100);
    advance_to(&mut store, DAY);
    market_settle(&mut store);
    execute(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    );
    advance_to(&mut store, 2 * DAY);
    market_settle(&mut store);

    assert_eq!(cash(&store, 3), 101);
    execute(
        &mut store,
        Caller::Account(1),
        Action::SettleExit { company: 3 },
    );
    let timeline = store.engine().unwrap().timeline().unwrap();
    assert_eq!(timeline.claims[0].paid, Money::new(51).unwrap());
    assert_eq!(timeline.claims[1].paid, Money::new(50).unwrap());
    assert_eq!(cash(&store, 3), 0);
    let runtime = store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .exit
        .unwrap();
    assert_eq!(
        runtime.phase,
        mycity_core::town_runtime::ExitPhase::Liquidating
    );

    let before_close = store.state_hash().unwrap();
    let close_request = next(&store, Action::CloseExit { company: 3 });
    let close = store.execute(Caller::Account(1), close_request).unwrap();
    assert!(close.model.is_err(), "unpaid claims must prevent close");
    assert_eq!(store.state_hash().unwrap(), before_close);
    assert_eq!(cash(&store, 3), 0);
}

#[test]
fn incomplete_policy_and_unregistered_cash_residual_wait_without_bypass_or_distribution() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path, opening(101, false, false, json!([]), json!([])));
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    advance_to(&mut store, DAY);
    market_settle(&mut store);

    let unauthorized = store
        .execute(Caller::Account(4), begin_exit_request(&store))
        .unwrap();
    assert!(unauthorized.model.is_err());
    assert!(store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .exit
        .is_none());
    execute(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    );
    advance_to(&mut store, 2 * DAY);
    market_settle(&mut store);
    let cash_before = cash(&store, 3);

    let settle_request = next(&store, Action::SettleExit { company: 3 });
    let settle = store.execute(Caller::Account(1), settle_request).unwrap();
    assert!(settle.has_persistent_receipt);
    assert_eq!(cash(&store, 3), cash_before);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .unwrap()
            .state
            .exit
            .unwrap()
            .phase,
        mycity_core::town_runtime::ExitPhase::Liquidating
    );
    let close_request = next(&store, Action::CloseExit { company: 3 });
    let close = store.execute(Caller::Account(1), close_request).unwrap();
    assert!(
        close.model.is_err(),
        "incomplete genesis declaration cannot close"
    );
    assert_eq!(cash(&store, 3), cash_before);

    let from = store.engine().unwrap().world().account_ref(3).unwrap();
    let to = store.engine().unwrap().world().account_ref(4).unwrap();
    let direct_transfer = next(
        &store,
        Action::Transfer {
            from,
            to,
            amount: Money::new(1).unwrap(),
        },
    );
    let direct = store.execute(Caller::Account(3), direct_transfer).unwrap();
    assert!(
        direct.model.is_err(),
        "ordinary transfer cannot bypass the exit settlement writer"
    );
    assert_eq!(cash(&store, 3), cash_before);

    let residual_path = temp.0.join("residual-world");
    let residual_opening = opening(101, false, true, json!([]), json!([]));
    let mut residual = create(&residual_path, residual_opening);
    execute(
        &mut residual,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    advance_to(&mut residual, DAY);
    market_settle(&mut residual);
    execute(
        &mut residual,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    );
    advance_to(&mut residual, 2 * DAY);
    market_settle(&mut residual);
    let before = cash(&residual, 3);
    let close_request = next(&residual, Action::CloseExit { company: 3 });
    let close = residual.execute(Caller::Account(1), close_request).unwrap();
    assert!(
        close.model.is_err(),
        "cash without a shareholder register remains unassigned"
    );
    assert_eq!(cash(&residual, 3), before);
    assert_eq!(before, 101);
}
