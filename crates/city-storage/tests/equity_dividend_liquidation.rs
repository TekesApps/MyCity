//! Independent account-level oracle for ordinary dividends followed by liquidation.
//!
//! Uses the existing v147 Store, Timeline, market, tax, equity and exit writers. This test
//! focuses on the distinction between ordinary dividends and paid-in capital: a dividend
//! reduces retained earnings, but must not reduce the capital basis subsequently returned.
//! It is source-only until the owner runs the authorized Cargo window.

use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope, JournalRecord},
    units::{Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "equity-dividend-liquidation-oracle-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-equity-dividend-liquidation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("EQUITY_DIVIDEND_LIQUIDATION_ARTIFACT={}", path.display());
        Self(path)
    }

    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() {
            eprintln!("EQUITY_DIVIDEND_LIQUIDATION_PRESERVED={}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn m(value: i64) -> Money {
    Money::new(value).unwrap()
}

fn opening() -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        json!({"id":6,"kind":"inventory_capital","amount":100}),
        json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    value["stocks"].as_array_mut().unwrap().extend([
        json!({"id":2,"owner":3,"commodity":"raw","quantity":10,"basis":100,"source":6}),
        json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    let person = json!({"kind":"person","id":7001});
    let treasury_party = json!({"kind":"treasury","id":1});
    value["town"] = json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters":[{
            "company":3,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":person,
            "investors":[
                {"party":person,"account":2,"opening_shares":6001},
                {"party":treasury_party,"account":1,"opening_shares":0}
            ],
            "opening_sources":[
                {"source_id":3,"party":person},
                {"source_id":6,"party":person},
                {"source_id":7,"party":person}
            ]
        }],
        "tax_rules":[{"company":3,"policy_id":77,"kind":"daily_realized_market_margin","authority_account":1,"rate_basis_points":5000}],
        "exit_policy":{
            "policy_id":1,"company_account":3,"requester":2,"executor":1,"legal_clearance":"cleared",
            "priority_policy":{"order":["tax","wage","secured_debt","unsecured_debt","other"],"within_tier":"pro_rata_by_outstanding_stable_id"},
            "claims_complete":true,"tax_assessment_complete":true,"asset_register_complete":true,"employment_exit_terms_complete":true,
            "claims":[{"id":100,"payee":4,"class":"unsecured_debt"}],
            "payroll":[{"id":1,"payee":2}]
        }
    });
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path) -> Store {
    Store::create(path, &opening(), vec![], RULES).unwrap()
}

fn request(store: &Store, action: Action) -> Envelope {
    let view = store.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn run(store: &mut Store, caller: Caller, action: Action) -> u64 {
    let envelope = request(store, action);
    let operation_id = envelope.operation_id;
    let outcome = store.execute(caller, envelope).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
    operation_id
}

fn advance_to(store: &mut Store, target: u64) {
    for _ in 0..512 {
        run(
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

fn settle(store: &mut Store) -> mycity_core::town_runtime::MarketSummary {
    run(store, Caller::System, Action::TownSettle { profile_id: 1 });
    store
        .engine()
        .unwrap()
        .world().town_market()
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

fn replay_exact(store: &mut Store, record: JournalRecord) {
    assert!(record.outcome.is_ok());
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

fn lose_applied_reply(store: &mut Store, caller: Caller, envelope: Envelope) {
    let mut reached_applied = false;
    let result = store.execute_with_observer(caller, envelope, |point| {
        if point == WritePoint::Applied {
            reached_applied = true;
            Err(std::io::Error::other("injected liquidation response loss"))
        } else {
            Ok(())
        }
    });
    assert!(reached_applied);
    match result {
        Err(mycity_storage::Error::Io(error))
            if error.to_string() == "injected liquidation response loss" => {}
        other => panic!("expected Applied response loss, got {other:?}"),
    }
}

#[test]
fn paid_dividend_does_not_reduce_liquidation_capital_and_allocations_cold_replay_exactly() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path);
    let initial_cash = total_cash(&store);
    assert_eq!(initial_cash, 3_000_000);

    let opening_register = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    assert_eq!(opening_register.opening_paid_in, m(600_100));
    assert_eq!(
        opening_register
            .holders()
            .iter()
            .find(|row| row.account == 2)
            .unwrap()
            .shares,
        6_001
    );

    // Two actual contributions issue 100 shares each. They make paid-in capital exactly
    // 620,100c while preserving the source-bound opening assets (600,000 cash + 100 stock).
    run(
        &mut store,
        Caller::Account(1),
        Action::EquityContribute {
            company: 3,
            amount: m(10_000),
        },
    );
    run(
        &mut store,
        Caller::Account(2),
        Action::EquityContribute {
            company: 3,
            amount: m(10_000),
        },
    );
    {
        let register = store
            .engine()
            .unwrap()
            .world().town_market()
            .unwrap()
            .state
            .equity_registers
            .as_slice()[0];
        assert_eq!(register.issued_shares, 6_201);
        assert_eq!(
            register
                .holders()
                .iter()
                .find(|row| row.account == 1)
                .unwrap()
                .paid_in,
            m(10_000)
        );
        assert_eq!(
            register
                .holders()
                .iter()
                .find(|row| row.account == 2)
                .unwrap()
                .paid_in,
            m(610_100)
        );
    }

    run(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    run(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: 3,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: m(100),
                    cycle: Cycle::Daily,
                    ends_at: DAY,
                },
            },
        },
    );
    run(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Lend {
                id: 100,
                lender: 4,
                borrower: 3,
                principal: m(500),
                due: 4 * DAY,
            },
        },
    );

    advance_to(&mut store, DAY);
    assert_eq!(settle(&mut store).actual_sales, Quantity::new(10).unwrap());
    let first_tax = {
        let timeline = store.engine().unwrap().timeline().unwrap();
        let claim = timeline
            .claims
            .iter()
            .find(|claim| timeline.tax_source_for_claim(claim).is_some())
            .unwrap();
        let source = timeline.tax_source_for_claim(claim).unwrap();
        assert_eq!(source.taxable_basis, m(90));
        assert_eq!(source.assessed, m(45));
        claim.id
    };
    advance_to(&mut store, 2 * DAY);
    let settled_tax = settle(&mut store);
    assert_eq!(settled_tax.actual_sales, Quantity::ZERO);
    {
        let timeline = store.engine().unwrap().timeline().unwrap();
        let claim = timeline
            .claims
            .iter()
            .find(|claim| claim.id == first_tax)
            .unwrap();
        assert_eq!(claim.paid, m(45));
        assert!(claim.processed);
    }
    advance_to(&mut store, 3 * DAY);
    assert_eq!(settle(&mut store).actual_sales, Quantity::ZERO);
    advance_to(&mut store, 4 * DAY);
    assert_eq!(settle(&mut store).actual_sales, Quantity::ZERO);
    {
        let timeline = store.engine().unwrap().timeline().unwrap();
        let loan = timeline
            .claims
            .iter()
            .find(|claim| claim.id == 100)
            .unwrap();
        assert_eq!(loan.paid, m(500));
        assert!(loan.processed);
    }

    // Independent cash oracle before ordinary distribution: 600,000 opening company cash,
    // +20,000 contributions, +1,000 sales, -800 payroll, -10 utility, -45 tax; the 500c
    // loan is both received and repaid. Opening inventory basis is not a second cash payment.
    assert_eq!(cash(&store, 3), 620_145);
    let total_before_dividend = total_cash(&store);
    let treasury_before_dividend = cash(&store, 1);
    let person_before_dividend = cash(&store, 2);
    let dividend = request(
        &store,
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(10_000),
        },
    );
    let dividend_id = dividend.operation_id;
    lose_applied_reply(&mut store, Caller::Account(2), dividend);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let dividend_receipt = store.receipt(dividend_id).unwrap().unwrap();
    assert!(dividend_receipt.outcome.is_ok());
    assert_eq!(
        dividend_receipt.outcome.as_ref().unwrap().settled[0],
        Some(m(45))
    );
    replay_exact(&mut store, dividend_receipt);
    assert_eq!(cash(&store, 3), 620_100);
    assert_eq!(cash(&store, 1) - treasury_before_dividend, 1);
    assert_eq!(cash(&store, 2) - person_before_dividend, 44);
    assert_eq!(total_cash(&store), total_before_dividend);
    let dividend_register = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    assert_eq!(dividend_register.cumulative_dividends_distributed, m(45));
    assert_eq!(dividend_register.opening_paid_in, m(600_100));
    assert_eq!(dividend_register.cumulative_capital_returned, Money::ZERO);

    // Fiscal reuse is an actual transfer after the treasury received its 1c dividend.
    let treasury_ref = store.engine().unwrap().world().account_ref(1).unwrap();
    let supplier_ref = store.engine().unwrap().world().account_ref(4).unwrap();
    run(
        &mut store,
        Caller::Account(1),
        Action::Transfer {
            from: treasury_ref,
            to: supplier_ref,
            amount: m(1),
        },
    );
    assert_eq!(cash(&store, 1), treasury_before_dividend);

    let pre_exit_cash = [
        cash(&store, 1),
        cash(&store, 2),
        cash(&store, 3),
        cash(&store, 4),
    ];
    let begin_exit_id = run(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    );
    let settle_exit = request(&store, Action::SettleExit { company: 3 });
    let settle_exit_id = settle_exit.operation_id;
    lose_applied_reply(&mut store, Caller::Account(1), settle_exit);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let settled_exit_hash = store.state_hash().unwrap();
    let exit_receipt = store.receipt(settle_exit_id).unwrap().unwrap();
    assert!(exit_receipt.outcome.is_ok());
    assert_eq!(
        exit_receipt.outcome.as_ref().unwrap().settled[0],
        Some(m(620_100))
    );
    replay_exact(&mut store, exit_receipt);
    assert_eq!(store.state_hash().unwrap(), settled_exit_hash);

    // This is the central counterexample: the prior 45c dividend does not reduce the
    // 620,100c paid-in capital claim. The entire post-dividend company balance returns to
    // holders in proportion to 100 treasury shares and 6,101 personal shares; no surplus.
    assert_eq!(cash(&store, 3), 0);
    assert_eq!(cash(&store, 1) - pre_exit_cash[0], 10_000);
    assert_eq!(cash(&store, 2) - pre_exit_cash[1], 610_100);
    assert_eq!(cash(&store, 4), pre_exit_cash[3]);
    let register = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    assert_eq!(register.cumulative_dividends_distributed, m(45));
    assert_eq!(register.cumulative_capital_returned, m(620_100));
    assert_eq!(
        register.cumulative_liquidation_surplus_distributed,
        Money::ZERO
    );
    assert_eq!(total_cash(&store), initial_cash);

    let close = request(&store, Action::CloseExit { company: 3 });
    let close_id = close.operation_id;
    run(
        &mut store,
        Caller::Account(1),
        Action::CloseExit { company: 3 },
    );
    let closed_hash = store.state_hash().unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(store.state_hash().unwrap(), closed_hash);
    let close_receipt = store.receipt(close_id).unwrap().unwrap();
    let begin_exit_receipt = store.receipt(begin_exit_id).unwrap().unwrap();
    let settle_exit_receipt = store.receipt(settle_exit_id).unwrap().unwrap();
    replay_exact(&mut store, close_receipt);
    replay_exact(&mut store, begin_exit_receipt);
    replay_exact(&mut store, settle_exit_receipt);
    assert_eq!(store.state_hash().unwrap(), closed_hash);
    assert_eq!(cash(&store, 3), 0);
    assert_eq!(total_cash(&store), initial_cash);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world().town_market()
            .unwrap()
            .state
            .exit
            .unwrap()
            .phase,
        mycity_core::town_runtime::ExitPhase::Closed
    );
}
