//! Store-backed 28-day business baseline and pressure scenarios, followed by an
//! observable liquidation attempt. Route/utility inputs remain fixed profile
//! substitutes; turnover-tax assessment is explicitly outside this scenario.
use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    town_runtime::{ExitPhase, MarketSummary},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const RULES: &str = "town-business-28d-store-v1";
const DAYS: u64 = 28;
const DEBT_ID: u64 = 1;

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Base,
    CashShortage,
    EnergyPriceRise,
}

impl Scenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "base" => Ok(Self::Base),
            "cash-shortage" => Ok(Self::CashShortage),
            "energy-price-rise" => Ok(Self::EnergyPriceRise),
            _ => Err(format!("unknown_scenario:{value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::CashShortage => "cash-shortage",
            Self::EnergyPriceRise => "energy-price-rise",
        }
    }

    fn inputs(self) -> (i64, i64) {
        match self {
            Self::Base => (200_000, 1),
            Self::CashShortage => (0, 1),
            Self::EnergyPriceRise => (200_000, 1_000),
        }
    }
}

fn err(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

fn opening(scenario: Scenario) -> Result<Vec<u8>, String> {
    let (household_cash, utility_cost) = scenario.inputs();
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
            .map_err(err)?;
    for source in value["sources"].as_array_mut().ok_or("sources")? {
        if source["id"].as_u64() == Some(2) {
            source["amount"] = household_cash.into();
        }
    }
    value["sources"].as_array_mut().ok_or("sources")?.extend([
        json!({"id":6,"kind":"inventory_capital","amount":100000}),
        json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    for account in value["accounts"].as_array_mut().ok_or("accounts")? {
        if account["id"].as_u64() == Some(2) {
            account["cash"] = household_cash.into();
        }
    }
    value["stocks"].as_array_mut().ok_or("stocks")?.extend([
        json!({"id":2,"owner":3,"commodity":"raw","quantity":100000,"basis":100000,"source":6}),
        json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    value["town"] = json!({
        "schema_version": 1,
        "id": 1,
        "households": [{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":1000}],
        "employers": [{"account":3,"worker_limit":1}],
        "shops": [{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes": [{
            "recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,
            "input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,
            "waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,
            "utility_cost_per_output_q":utility_cost,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,
            "utility_account":1,"freight_account":1,"disposal_account":1
        }],
        "delivery": [{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,
            "route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,
            "freight_per_q":1,"utility_surcharge_per_q":1}],
        "exit_policy": {
            "policy_id":1,
            "company_account":3,
            "requester":2,
            "executor":1,
            "legal_clearance":"cleared",
            "priority_policy":{"order":["tax","wage","secured_debt","unsecured_debt","other"],"within_tier":"pro_rata_by_outstanding_stable_id"},
            "claims_complete":true,
            "tax_assessment_complete":true,
            "asset_register_complete":true,
            "employment_exit_terms_complete":true,
            "claims":[{"id":1,"payee":1,"class":"unsecured_debt"}],
            "payroll":[{"id":1,"payee":2}]
        }
    });
    serde_json::to_vec(&value).map_err(err)
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
    let request = envelope(store, action)?;
    let outcome = store.execute(caller, request).map_err(err)?;
    outcome.model.map_err(err)?;
    if !outcome.has_persistent_receipt {
        return Err("missing_persistent_receipt".into());
    }
    Ok(())
}

fn checkpoint_reopen(mut store: Store, path: &Path, boundary: &str) -> Result<Store, String> {
    let before = store.state_hash().map_err(err)?;
    store.checkpoint().map_err(err)?;
    drop(store);
    let recovered = Store::open(path, RULES).map_err(err)?;
    let after = recovered.state_hash().map_err(err)?;
    if before != after {
        return Err(format!("cold_open_state_hash_mismatch:{boundary}"));
    }
    Ok(recovered)
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..128 {
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
            .ok_or("missing_timeline")?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("timeline_not_complete:{target}"))
}

fn settle_market(store: &mut Store) -> Result<MarketSummary, String> {
    execute(store, Caller::System, Action::TownSettle { profile_id: 1 })?;
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .map(|town| town.state.last_summary)
        .ok_or_else(|| "missing_town".into())
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path =
        PathBuf::from(args.next().ok_or(
            "usage: town_business_28d NEW_STORE_DIR [base|cash-shortage|energy-price-rise]",
        )?);
    let scenario = Scenario::parse(&args.next().unwrap_or_else(|| "base".into()))?;
    if args.next().is_some() || path.exists() {
        return Err("unexpected_argument_or_store_path_exists".into());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(err)?;
    }

    let mut store = Store::create(&path, &opening(scenario)?, vec![], RULES).map_err(err)?;
    let (initial_household_cash, initial_company_cash) = {
        let view = store.engine().map_err(err)?.world().view();
        let balance = |id| {
            view.accounts
                .iter()
                .find(|account| account.id == id)
                .map(|account| account.cash.value())
                .ok_or_else(|| format!("missing_initial_account:{id}"))
        };
        (balance(2)?, balance(3)?)
    };
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
                    hourly_cents: Money::new(100).map_err(err)?,
                    cycle: Cycle::Daily,
                    ends_at: DAYS * DAY,
                },
            },
        },
    )?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Lend {
                id: DEBT_ID,
                lender: 1,
                borrower: 3,
                principal: Money::new(100_000).map_err(err)?,
                due: 14 * DAY,
            },
        },
    )?;

    let mut production = 0i128;
    let mut waste = 0i128;
    let mut sales = 0i128;
    let mut goods_payments = 0i128;
    let mut wages_accrued = 0i128;
    let mut freight = 0i128;
    let mut utilities = 0i128;
    let initial_cash: i128 = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum();
    for day in 1..=DAYS {
        advance_to(&mut store, day * DAY)?;
        if day >= 14 {
            let outstanding = store
                .engine()
                .map_err(err)?
                .timeline()
                .and_then(|timeline| timeline.claims.iter().find(|claim| claim.id == DEBT_ID))
                .map(|claim| claim.principal.value() - claim.paid.value())
                .unwrap_or(0);
            if outstanding > 0 {
                execute(
                    &mut store,
                    Caller::System,
                    Action::Timeline {
                        command: TimelineCommand::SettleClaim { id: DEBT_ID },
                    },
                )?;
            }
        }
        let summary = settle_market(&mut store)?;
        production += i128::from(summary.produced.value());
        waste += i128::from(summary.waste.value());
        sales += i128::from(summary.actual_sales.value());
        goods_payments += i128::from(summary.goods_payments.value());
        wages_accrued += i128::from(summary.wages_accrued.value());
        freight += i128::from(summary.freight.value());
        utilities += i128::from(summary.utilities.value());

        if day == 14 || day == DAYS {
            store = checkpoint_reopen(store, &path, &format!("day_{day}"))?;
        }
    }
    let (
        debt_paid,
        debt_outstanding,
        wages_paid,
        wage_arrears,
        household_cash_day28,
        company_cash,
        raw_remaining,
        product_remaining,
        final_cash,
    ) = {
        let engine = store.engine().map_err(err)?;
        let timeline = engine.timeline().ok_or("missing_timeline")?;
        let debt_paid = timeline
            .claims
            .iter()
            .find(|claim| claim.id == DEBT_ID)
            .map(|claim| claim.paid.value())
            .unwrap_or(0);
        let debt_outstanding = timeline
            .claims
            .iter()
            .find(|claim| claim.id == DEBT_ID)
            .map(|claim| claim.principal.value() - claim.paid.value())
            .unwrap_or(0);
        let payroll = timeline
            .payroll
            .iter()
            .find(|row| row.spec.id == 1)
            .ok_or("missing_payroll")?;
        let wages_paid = payroll.paid.value();
        let wage_arrears = payroll.billed.value() - payroll.paid.value();
        let view = engine.world().view();
        let company_cash = view
            .accounts
            .iter()
            .find(|account| account.id == 3)
            .ok_or("missing_company")?
            .cash
            .value();
        let household_cash_day28 = view
            .accounts
            .iter()
            .find(|account| account.id == 2)
            .ok_or("missing_household")?
            .cash
            .value();
        let raw_remaining = view
            .stocks
            .iter()
            .find(|stock| stock.id == 2)
            .ok_or("missing_raw")?
            .quantity
            .value();
        let product_remaining = view
            .stocks
            .iter()
            .find(|stock| stock.id == 3)
            .ok_or("missing_product")?
            .quantity
            .value();
        let final_cash = view
            .accounts
            .iter()
            .map(|account| i128::from(account.cash.value()))
            .sum::<i128>();
        (
            debt_paid,
            debt_outstanding,
            wages_paid,
            wage_arrears,
            household_cash_day28,
            company_cash,
            raw_remaining,
            product_remaining,
            final_cash,
        )
    };
    if final_cash != initial_cash {
        return Err(format!("cash_not_conserved:{initial_cash}:{final_cash}"));
    }
    if i128::from(raw_remaining) + production + waste != 100_000
        || i128::from(product_remaining) + sales != production
    {
        return Err(format!(
            "stock_not_conserved:raw_initial=100000:raw_remaining={raw_remaining}:production={production}:waste={waste}:sales={sales}:product_remaining={product_remaining}"
        ));
    }
    execute(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    )?;
    let mut store = checkpoint_reopen(store, &path, "after_begin_exit")?;
    let phase = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("missing_town_after_cold_open")?
        .state
        .exit
        .ok_or("missing_exit_after_cold_open")?
        .phase;
    if phase != ExitPhase::Liquidating {
        return Err(format!("expected_liquidating_after_restart:{phase:?}"));
    }
    let settle_request = envelope(&store, Action::SettleExit { company: 3 })?;
    let settle = store
        .execute(Caller::Account(1), settle_request)
        .map_err(err)?;
    if !settle.has_persistent_receipt {
        return Err("settle_exit_missing_receipt".into());
    }
    let settle_status = match settle.model {
        Ok(_) => "settlement_recorded".to_owned(),
        Err(error) => format!("waiting_or_blocked:{error:?}"),
    };
    let close_request = envelope(&store, Action::CloseExit { company: 3 })?;
    let close = store
        .execute(Caller::Account(1), close_request)
        .map_err(err)?;
    if !close.has_persistent_receipt {
        return Err("close_exit_missing_receipt".into());
    }
    let close_status = match close.model {
        Ok(_) => "close_command_applied".to_owned(),
        Err(error) => format!("waiting_or_blocked:{error:?}"),
    };
    let store = checkpoint_reopen(store, &path, "after_exit_settlement_and_close_attempt")?;
    let (
        phase_after,
        final_company_cash,
        final_household_cash,
        final_cash_after_exit,
        raw_remaining_after_exit,
        product_remaining_after_exit,
        debt_paid_after_exit,
        debt_outstanding_after_exit,
        wages_paid_after_exit,
        wage_arrears_after_exit,
    ) = {
        let engine = store.engine().map_err(err)?;
        let view = engine.world().view();
        let timeline = engine.timeline().ok_or("missing_timeline_final")?;
        let exit = view
            .town
            .ok_or("missing_town_final")?
            .state
            .exit
            .ok_or("missing_exit_final")?;
        let company_cash = view
            .accounts
            .iter()
            .find(|account| account.id == 3)
            .ok_or("missing_company_final")?
            .cash
            .value();
        let household_cash = view
            .accounts
            .iter()
            .find(|account| account.id == 2)
            .ok_or("missing_household_final")?
            .cash
            .value();
        let total_cash = view
            .accounts
            .iter()
            .map(|account| i128::from(account.cash.value()))
            .sum::<i128>();
        let raw = view
            .stocks
            .iter()
            .find(|stock| stock.id == 2)
            .ok_or("missing_raw_final")?
            .quantity
            .value();
        let product = view
            .stocks
            .iter()
            .find(|stock| stock.id == 3)
            .ok_or("missing_product_final")?
            .quantity
            .value();
        let claim = timeline.claims.iter().find(|claim| claim.id == DEBT_ID);
        let debt_paid_after_exit = claim.map(|claim| claim.paid.value()).unwrap_or(0);
        let debt_outstanding_after_exit = claim
            .map(|claim| claim.principal.value() - claim.paid.value())
            .unwrap_or(0);
        let payroll = timeline
            .payroll
            .iter()
            .find(|row| row.spec.id == 1)
            .ok_or("missing_payroll_final")?;
        (
            exit.phase,
            company_cash,
            household_cash,
            total_cash,
            raw,
            product,
            debt_paid_after_exit,
            debt_outstanding_after_exit,
            payroll.paid.value(),
            payroll.billed.value() - payroll.paid.value(),
        )
    };
    if final_cash_after_exit != initial_cash {
        return Err(format!(
            "cash_not_conserved_after_exit:{initial_cash}:{final_cash_after_exit}"
        ));
    }
    if i128::from(raw_remaining_after_exit) + production + waste != 100_000
        || i128::from(product_remaining_after_exit) + sales != production
    {
        return Err("stock_not_conserved_after_exit".into());
    }
    println!(
        "scenario={} days={} initial_household_cash={} initial_company_cash={} company_cash_after_loan={} loan_principal=100000 loan_due_day=14 recovered_at_days=14,28 production={} waste={} sales={} goods_payments={} freight={} utilities={} wages_accrued={} wages_paid_day28={} wage_arrears_day28={} wages_paid_after_exit={} wage_arrears_after_exit={} debt_paid_day28={} debt_outstanding_day28={} debt_paid_after_exit={} debt_outstanding_after_exit={} household_cash_day28={} household_cash_after_exit={} company_cash_day28={} company_cash_after_exit={} raw_remaining_day28={} product_remaining_day28={} raw_remaining_after_exit={} product_remaining_after_exit={} exit_after_day28=Liquidating settle={} close={} final_phase={:?} dynamic_turnover_tax=not_configured_not_integrated live_transport=false live_utility=false",
        scenario.name(), DAYS, initial_household_cash, initial_company_cash,
        initial_company_cash + 100_000,
        production, waste, sales, goods_payments, freight, utilities,
        wages_accrued, wages_paid, wage_arrears, wages_paid_after_exit, wage_arrears_after_exit,
        debt_paid, debt_outstanding, debt_paid_after_exit, debt_outstanding_after_exit,
        household_cash_day28, final_household_cash, company_cash, final_company_cash,
        raw_remaining, product_remaining, raw_remaining_after_exit, product_remaining_after_exit,
        settle_status, close_status, phase_after
    );
    Ok(())
}
