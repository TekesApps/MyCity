//! Store-backed shares, realized tax, dividends and first-version liquidation.
//! The fixed delivery route and single-company tax rule are prototype fixture inputs.
use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{env, path::PathBuf};

const RULES: &str = "town-equity-cli-v1";

fn m(value: i64) -> Money {
    Money::new(value).expect("fixed fixture amount is valid")
}

fn opening() -> Result<Vec<u8>, String> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
            .map_err(|error| error.to_string())?;
    value["sources"].as_array_mut().ok_or("sources")?.extend([
        json!({"id":6,"kind":"inventory_capital","amount":100}),
        json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    value["stocks"].as_array_mut().ok_or("stocks")?.extend([
        json!({"id":2,"owner":3,"commodity":"raw","quantity":10,"basis":100,"source":6}),
        json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    value["town"] = json!({
        "schema_version":1,"id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters":[{
            "company":3,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":{"kind":"person","id":7001},
            "investors":[
                {"party":{"kind":"person","id":7001},"account":2,"opening_shares":6001},
                {"party":{"kind":"treasury","id":1},"account":1,"opening_shares":0}
            ],
            "opening_sources":[
                {"source_id":3,"party":{"kind":"person","id":7001}},
                {"source_id":6,"party":{"kind":"person","id":7001}},
                {"source_id":7,"party":{"kind":"person","id":7001}}
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
    serde_json::to_vec(&value).map_err(|error| error.to_string())
}

fn envelope(store: &Store, action: Action) -> Result<Envelope, String> {
    let view = store.engine().map_err(err)?.world().view();
    Ok(Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    })
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> Result<(), String> {
    let envelope = envelope(store, action)?;
    let outcome = store.execute(caller, envelope).map_err(err)?;
    outcome.model.map_err(|error| format!("{error:?}"))?;
    if !outcome.has_persistent_receipt {
        return Err("missing_persistent_receipt".into());
    }
    Ok(())
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..512 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline")?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("timeline_not_complete:{target}"))
}

fn cash(store: &Store, account: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .map(|row| row.cash.value())
        .ok_or("account_missing".into())
}

fn settle(store: &mut Store) -> Result<mycity_core::town_runtime::MarketSummary, String> {
    execute(store, Caller::System, Action::TownSettle { profile_id: 1 })?;
    Ok(store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("town")?
        .state
        .last_summary)
}

fn err(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

fn run() -> Result<(), String> {
    let path = env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        env::temp_dir().join(format!("mycity-town-equity-{}", std::process::id()))
    });
    if path.exists() {
        return Err(format!("world_path_already_exists:{}", path.display()));
    }
    let bytes = opening()?;
    let mut store = Store::create(&path, &bytes, vec![], RULES).map_err(err)?;
    let seed_cash = cash(&store, 3)?;

    for (investor, amount) in [(1, 10_000), (2, 10_000)] {
        execute(
            &mut store,
            Caller::Account(investor),
            Action::EquityContribute {
                company: 3,
                amount: m(amount),
            },
        )?;
    }
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    execute(
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
    )?;
    let lender_cash_before_loan = cash(&store, 4)?;
    execute(
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
    )?;

    advance_to(&mut store, DAY)?;
    let day1 = settle(&mut store)?;
    let tax_claim = {
        let timeline = store.engine().map_err(err)?.timeline().ok_or("timeline")?;
        timeline
            .claims
            .iter()
            .find(|claim| timeline.tax_source_for_claim(claim).is_some())
            .copied()
            .ok_or("tax_claim_missing")?
    };
    let tax_treasury_before = cash(&store, 1)?;
    advance_to(&mut store, 2 * DAY)?;
    let tax_receipt = cash(&store, 1)? - tax_treasury_before;
    if tax_receipt != tax_claim.principal.value() {
        return Err("tax_receipt_mismatch".into());
    }
    let day2 = settle(&mut store)?;
    advance_to(&mut store, 3 * DAY)?;
    let day3 = settle(&mut store)?;
    advance_to(&mut store, 4 * DAY)?;
    let day4 = settle(&mut store)?;
    let timeline = store.engine().map_err(err)?.timeline().ok_or("timeline")?;
    let loan_claim = timeline
        .claims
        .iter()
        .find(|claim| claim.id == 100)
        .copied()
        .ok_or("loan_claim_missing")?;
    if loan_claim.paid != loan_claim.principal || !loan_claim.processed {
        return Err("loan_repayment_mismatch".into());
    }
    let loan_source = timeline
        .loan_source_for_claim(&loan_claim)
        .ok_or("loan_source_missing")?;

    let treasury_before_dividend = cash(&store, 1)?;
    execute(
        &mut store,
        Caller::Account(2),
        Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: m(10_000),
        },
    )?;
    let town = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("town")?;
    let dividend = town.state.equity_registers.as_slice()[0]
        .cumulative_dividends_distributed
        .value();
    let government_dividend = cash(&store, 1)? - treasury_before_dividend;
    if government_dividend <= 0 || government_dividend > dividend {
        return Err("government_dividend_mismatch".into());
    }
    let treasury = store
        .engine()
        .map_err(err)?
        .world()
        .account_ref(1)
        .map_err(err)?;
    let service = store
        .engine()
        .map_err(err)?
        .world()
        .account_ref(4)
        .map_err(err)?;
    let service_cash_before_fiscal_spend = cash(&store, 4)?;
    execute(
        &mut store,
        Caller::Account(1),
        Action::Transfer {
            from: treasury,
            to: service,
            amount: m(government_dividend),
        },
    )?;
    let service_fiscal_receipt = cash(&store, 4)? - service_cash_before_fiscal_spend;
    if cash(&store, 1)? != treasury_before_dividend || service_fiscal_receipt != government_dividend
    {
        return Err("fiscal_transfer_mismatch".into());
    }
    let fiscal_spend = government_dividend;
    execute(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    )?;
    let company_cash_before_exit = cash(&store, 3)?;
    execute(
        &mut store,
        Caller::Account(1),
        Action::SettleExit { company: 3 },
    )?;
    if cash(&store, 3)? != 0 {
        return Err("exit_settlement_left_company_cash".into());
    }
    let preclose = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("town")?;
    let register = preclose.state.equity_registers.as_slice()[0];
    let returned = register
        .cumulative_capital_returned
        .checked_add(register.cumulative_liquidation_surplus_distributed)
        .map_err(err)?;
    if returned.value() != company_cash_before_exit {
        return Err("exit_settlement_distribution_mismatch".into());
    }
    let government_shares = register
        .holders()
        .iter()
        .find(|holder| holder.account == 1)
        .map_or(0, |holder| holder.shares);
    execute(
        &mut store,
        Caller::Account(1),
        Action::CloseExit { company: 3 },
    )?;
    let final_world = store.engine().map_err(err)?.world().view();
    let exit_phase = final_world
        .town
        .ok_or("town")?
        .state
        .exit
        .ok_or("exit")?
        .phase;
    if exit_phase != mycity_core::town_runtime::ExitPhase::Closed
        || final_world
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .is_none_or(|company| company.cash.value() != 0)
    {
        return Err("exit_close_business_state_mismatch".into());
    }
    let final_engine = store.engine().map_err(err)?;
    let final_timeline = final_engine.timeline().ok_or("timeline")?;
    let typed_capacity_inventory = mycity_core::model_capacity::inventory();
    let final_cash = |account| {
        final_world
            .accounts
            .iter()
            .find(|row| row.id == account)
            .map(|row| row.cash.value())
            .ok_or_else(|| format!("account_missing:{account}"))
    };
    println!(
        "{}",
        serde_json::json!({
            "world": path.display().to_string(),
            "seed_company_cash_cents": seed_cash,
            "day1_sales": day1.actual_sales.value(),
            "tax_assessed_cents": tax_claim.principal.value(),
            "treasury_tax_receipt_cents": tax_receipt,
            "day2_sales": day2.actual_sales.value(),
            "day3_sales": day3.actual_sales.value(),
            "day4_sales": day4.actual_sales.value(),
            "loan_lender_account": loan_source.lender,
            "loan_borrower_account": loan_source.borrower,
            "loan_principal_issued_cents": loan_source.principal.value(),
            "loan_principal_repaid_cents": loan_claim.paid.value(),
            "loan_lender_cash_before_issue_cents": lender_cash_before_loan,
            "ordinary_dividend_cents": dividend,
            "government_ordinary_shares": government_shares,
            "government_ordinary_dividend_cents": government_dividend,
            "fiscal_spend_from_dividend_cents": fiscal_spend,
            "fiscal_service_receipt_cents": service_fiscal_receipt,
            "capital_returned_cents": register.cumulative_capital_returned.value(),
            "liquidation_surplus_cents": register.cumulative_liquidation_surplus_distributed.value(),
            "account1_final_cash_cents": final_cash(1)?,
            "account2_final_cash_cents": final_cash(2)?,
            "account3_final_cash_cents": final_cash(3)?,
            "account4_final_cash_cents": final_cash(4)?,
            "exit_phase": format!("{exit_phase:?}"),
            "memory_observation": {
                "engine_owned_capacity_bytes": final_engine.owned_capacity_bytes(),
                "timeline_owned_bytes": final_timeline.owned_bytes(),
                "timeline_bytes_are_included_in_engine_owned_capacity": true,
                "world_inline_size_bytes": std::mem::size_of::<mycity_core::assembly::World>(),
                "claim_size_bytes": std::mem::size_of::<mycity_core::timeline::Claim>(),
                "claim_source_record_size_bytes": std::mem::size_of::<mycity_core::timeline::ClaimSourceRecord>(),
                "max_claim_sources": mycity_core::timeline::MAX_CLAIM_SOURCES,
                "typed_capacity_inventory": typed_capacity_inventory,
                "process_peak": "not_measured",
                "physical_memory_acceptance": "not_measured"
            },
            "prototype_fixed_route": true
        })
    );
    store.checkpoint().map_err(err)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("town_equity_failed={error}");
        std::process::exit(1);
    }
}
