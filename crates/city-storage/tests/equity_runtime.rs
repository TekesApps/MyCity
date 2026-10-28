use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope, JournalRecord},
    units::Money,
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "town-equity-runtime-store-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-equity-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        eprintln!("EQUITY_RUNTIME_ARTIFACT={}", path.display());
        Self(fs::canonicalize(path).unwrap())
    }

    fn world(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() {
            eprintln!("EQUITY_RUNTIME_PRESERVED={}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn m(value: i64) -> Money {
    Money::new(value).unwrap()
}

fn opening(raw_quantity: i64, raw_basis: i64) -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        json!({"id": 6, "kind": "inventory_capital", "amount": raw_basis}),
        json!({"id": 7, "kind": "inventory_capital", "amount": 0}),
    ]);
    value["stocks"].as_array_mut().unwrap().extend([
        json!({"id": 2, "owner": 3, "commodity": "raw", "quantity": raw_quantity, "basis": raw_basis, "source": 6}),
        json!({"id": 3, "owner": 3, "commodity": "product", "quantity": 0, "basis": 0, "source": 7}),
    ]);

    let seeded_paid_in = 600_000 + raw_basis;
    assert_eq!(seeded_paid_in % 100, 0);
    let opening_shares = seeded_paid_in / 100;
    let person = json!({"kind":"person", "id":7001});
    let treasury = json!({"kind":"treasury", "id":1});
    let market = json!({
        "schema_version": 1,
        "id": 1,
        "households": [{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers": [{"account":3,"worker_limit":1}],
        "shops": [{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes": [{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
        "delivery": [{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters": [{
            "company":3,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":person,
            "investors":[
                {"party":person,"account":2,"opening_shares":opening_shares},
                {"party":treasury,"account":1,"opening_shares":0}
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
    value["town"] = market;
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path, raw_quantity: i64, raw_basis: i64) -> Store {
    Store::create(path, &opening(raw_quantity, raw_basis), vec![], RULES).unwrap()
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

fn rejected(store: &mut Store, caller: Caller, action: Action) {
    let envelope = request(store, action);
    let outcome = store.execute(caller, envelope).unwrap();
    assert!(outcome.model.is_err(), "unexpectedly accepted: {outcome:?}");
    assert!(outcome.has_persistent_receipt);
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

fn add_payroll(store: &mut Store) {
    run(
        store,
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
    assert!(
        record.outcome.is_ok(),
        "expected successful operation receipt"
    );
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

fn execute_lost_applied(store: &mut Store, caller: Caller, envelope: Envelope) {
    let mut reached_applied = false;
    let result = store.execute_with_observer(caller, envelope, |point| {
        if point == WritePoint::Applied {
            reached_applied = true;
            Err(std::io::Error::other("injected equity response loss"))
        } else {
            Ok(())
        }
    });
    assert!(
        reached_applied,
        "injected failure must occur after durable apply"
    );
    match result {
        Err(mycity_storage::Error::Io(error))
            if error.to_string() == "injected equity response loss" => {}
        other => panic!("expected injected Applied response loss, got {other:?}"),
    }
}

#[test]
fn actual_contributions_tax_dividend_treasury_spend_and_liquidation_survive_recovery() {
    let temp = Temp::new();
    let path = temp.world("success-world");
    let mut store = create(&path, 10, 100);
    let initial_cash = total_cash(&store);
    let opening_paid = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0]
        .opening_paid_in;
    assert_eq!(opening_paid, m(600_100));

    let initial_register = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    let person = initial_register
        .holders()
        .iter()
        .find(|row| row.account == 2)
        .unwrap();
    assert_eq!(person.shares, 6_001);
    let contribution_actions = [
        (
            Caller::Account(1),
            Action::EquityContribute {
                company: 3,
                amount: m(10_000),
            },
        ),
        (
            Caller::Account(2),
            Action::EquityContribute {
                company: 3,
                amount: m(10_000),
            },
        ),
    ];
    for (index, (caller, action)) in contribution_actions.into_iter().enumerate() {
        let contribution_id = run(&mut store, caller, action);
        if index == 0 {
            let contribution_hash = store.state_hash().unwrap();
            let contribution_receipt = store.receipt(contribution_id).unwrap().unwrap();
            replay_exact(&mut store, contribution_receipt);
            assert_eq!(store.state_hash().unwrap(), contribution_hash);
        }
    }
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
    let before_issue_cap = store.state_hash().unwrap();
    rejected(
        &mut store,
        Caller::Account(1),
        Action::EquityContribute {
            company: 3,
            amount: m(400_000),
        },
    );
    assert_eq!(store.state_hash().unwrap(), before_issue_cap);

    run(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    add_payroll(&mut store);
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
    let day1 = request(&store, Action::TownSettle { profile_id: 1 });
    let day1_id = day1.operation_id;
    execute_lost_applied(&mut store, Caller::System, day1);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let after_day1_hash = store.state_hash().unwrap();
    let receipt = store.receipt(day1_id).unwrap().unwrap();
    assert!(
        receipt.outcome.is_ok(),
        "TownSettle must have applied successfully"
    );
    assert_eq!(
        receipt.outcome.as_ref().unwrap().town.unwrap().actual_sales,
        mycity_core::units::Quantity::new(10).unwrap()
    );
    replay_exact(&mut store, receipt);
    assert_eq!(store.state_hash().unwrap(), after_day1_hash);
    let timeline = store.engine().unwrap().timeline().unwrap();
    let tax_claim = timeline
        .claims
        .iter()
        .find(|claim| timeline.tax_source_for_claim(claim).is_some())
        .copied()
        .unwrap();
    assert!(tax_claim.principal > Money::ZERO);
    let first_cursor = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .tax_cursors
        .as_slice()[0];
    assert!(first_cursor.retained_earnings_cents > 0);
    assert_eq!(first_cursor.cumulative_assessed, tax_claim.principal);

    let treasury_before_tax = cash(&store, 1);
    advance_to(&mut store, 2 * DAY);
    assert_eq!(
        cash(&store, 1) - treasury_before_tax,
        tax_claim.principal.value()
    );
    settle(&mut store);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world().town_market()
            .unwrap()
            .state
            .tax_cursors
            .as_slice()[0]
            .through_day,
        2
    );

    let debt_blocked_hash = store.state_hash().unwrap();
    rejected(
        &mut store,
        Caller::Account(2),
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(10_000),
        },
    );
    assert_eq!(store.state_hash().unwrap(), debt_blocked_hash);
    advance_to(&mut store, 3 * DAY);
    settle(&mut store);
    advance_to(&mut store, 4 * DAY);
    settle(&mut store);
    let timeline = store.engine().unwrap().timeline().unwrap();
    let loan_claim = timeline
        .claims
        .iter()
        .find(|claim| claim.id == 100)
        .copied()
        .unwrap();
    assert_eq!(loan_claim.paid, m(500));
    assert!(loan_claim.processed);
    let loan_source = timeline.loan_source_for_claim(&loan_claim).unwrap();
    assert_eq!(loan_source.lender, 4);
    assert_eq!(loan_source.borrower, 3);
    assert_eq!(loan_source.principal, m(500));

    let company_handle = store.engine().unwrap().world().account_ref(3).unwrap();
    let protected_cash = m(cash(&store, 3));
    run(
        &mut store,
        Caller::Account(3),
        Action::Protect {
            account: company_handle,
            amount: protected_cash,
        },
    );
    let protected_hash = store.state_hash().unwrap();
    rejected(
        &mut store,
        Caller::Account(2),
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(10_000),
        },
    );
    assert_eq!(store.state_hash().unwrap(), protected_hash);
    run(
        &mut store,
        Caller::System,
        Action::ReleaseProtection {
            account: company_handle,
            amount: protected_cash,
        },
    );

    let treasury_before_distribution = cash(&store, 1);
    let distribution = request(
        &store,
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(10_000),
        },
    );
    let distribution_id = distribution.operation_id;
    execute_lost_applied(&mut store, Caller::Account(2), distribution);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let distribution_hash = store.state_hash().unwrap();
    let distribution_receipt = store.receipt(distribution_id).unwrap().unwrap();
    assert!(distribution_receipt.outcome.is_ok());
    let distribution_result = distribution_receipt.outcome.as_ref().unwrap();
    assert!(distribution_result.settled[0].is_some_and(|amount| amount > Money::ZERO));
    replay_exact(&mut store, distribution_receipt);
    assert_eq!(store.state_hash().unwrap(), distribution_hash);
    let government_dividend = cash(&store, 1) - treasury_before_distribution;
    assert!(government_dividend > 0);
    let vendor_before_fiscal_spend = cash(&store, 4);
    let treasury_ref = store.engine().unwrap().world().account_ref(1).unwrap();
    let vendor_ref = store.engine().unwrap().world().account_ref(4).unwrap();
    run(
        &mut store,
        Caller::Account(1),
        Action::Transfer {
            from: treasury_ref,
            to: vendor_ref,
            amount: m(government_dividend),
        },
    );
    assert_eq!(cash(&store, 1), treasury_before_distribution);
    assert_eq!(
        cash(&store, 4),
        vendor_before_fiscal_spend + government_dividend
    );
    let register = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    assert!(register.cumulative_dividends_distributed > Money::ZERO);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world().town_market()
            .unwrap()
            .state
            .tax_cursors
            .as_slice()[0]
            .retained_earnings_cents,
        0
    );

    let before_exit_company_cash = cash(&store, 3);
    run(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    );
    let settle_exit = request(&store, Action::SettleExit { company: 3 });
    let settle_exit_id = settle_exit.operation_id;
    execute_lost_applied(&mut store, Caller::Account(1), settle_exit);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let after_exit_settle = store.state_hash().unwrap();
    let exit_settle_receipt = store.receipt(settle_exit_id).unwrap().unwrap();
    assert!(exit_settle_receipt.outcome.is_ok());
    let exit_settlement = exit_settle_receipt.outcome.as_ref().unwrap();
    assert!(exit_settlement.settled[0].is_some_and(|amount| amount > Money::ZERO));
    replay_exact(&mut store, exit_settle_receipt);
    assert_eq!(store.state_hash().unwrap(), after_exit_settle);
    assert_eq!(cash(&store, 3), 0);
    assert!(before_exit_company_cash > 0);
    let register = store
        .engine()
        .unwrap()
        .world().town_market()
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    assert!(register.cumulative_capital_returned > Money::ZERO);
    let returned = register
        .cumulative_capital_returned
        .checked_add(register.cumulative_liquidation_surplus_distributed)
        .unwrap();
    assert_eq!(returned.value(), before_exit_company_cash);

    let close = request(&store, Action::CloseExit { company: 3 });
    let close_id = close.operation_id;
    run(
        &mut store,
        Caller::Account(1),
        Action::CloseExit { company: 3 },
    );
    let closed_hash = store.state_hash().unwrap();
    let close_receipt = store.receipt(close_id).unwrap().unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(store.state_hash().unwrap(), closed_hash);
    replay_exact(&mut store, close_receipt);
    assert_eq!(store.state_hash().unwrap(), closed_hash);
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
    assert_eq!(total_cash(&store), initial_cash);
}

#[test]
fn loss_and_paid_unmodeled_claims_cannot_be_distributed() {
    let temp = Temp::new();
    let loss_path = temp.world("loss-world");
    let mut loss = create(&loss_path, 10, 20_000);
    run(
        &mut loss,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    add_payroll(&mut loss);
    advance_to(&mut loss, DAY);
    settle(&mut loss);
    assert!(
        loss.engine()
            .unwrap()
            .world().town_market()
            .unwrap()
            .state
            .tax_cursors
            .as_slice()[0]
            .retained_earnings_cents
            <= 0
    );
    let before_loss_distribution = loss.state_hash().unwrap();
    rejected(
        &mut loss,
        Caller::Account(2),
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(100),
        },
    );
    assert_eq!(loss.state_hash().unwrap(), before_loss_distribution);

    let income_path = temp.world("unmodeled-income-world");
    let mut income = create(&income_path, 10, 100);
    run(
        &mut income,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    add_payroll(&mut income);
    run(
        &mut income,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Claims {
                first_id: 98,
                count: 1,
                payer: 3,
                payee: 4,
                amount: m(150),
                due: 2 * DAY,
            },
        },
    );
    run(
        &mut income,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Claims {
                first_id: 99,
                count: 1,
                payer: 4,
                payee: 3,
                amount: m(1),
                due: 2 * DAY,
            },
        },
    );
    advance_to(&mut income, DAY);
    settle(&mut income);
    advance_to(&mut income, 2 * DAY);
    settle(&mut income);
    let timeline = income.engine().unwrap().timeline().unwrap();
    let compensation = timeline.claims.iter().find(|claim| claim.id == 98).unwrap();
    assert_eq!(compensation.principal, m(150));
    assert_eq!(compensation.paid, compensation.principal);
    assert!(compensation.processed);
    assert!(timeline.loan_source_for_claim(compensation).is_none());
    let unmodeled_receivable = timeline.claims.iter().find(|claim| claim.id == 99).unwrap();
    assert_eq!(unmodeled_receivable.paid, unmodeled_receivable.principal);
    assert!(unmodeled_receivable.processed);
    assert!(timeline
        .loan_source_for_claim(unmodeled_receivable)
        .is_none());
    let before_unknown_income_distribution = income.state_hash().unwrap();
    rejected(
        &mut income,
        Caller::Account(2),
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(100),
        },
    );
    assert_eq!(
        income.state_hash().unwrap(),
        before_unknown_income_distribution
    );
}
