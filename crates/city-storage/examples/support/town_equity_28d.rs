//! Shared bounded scenario runner for the new Store example and integration test.
use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::path::Path;

pub const RULES: &str = "town-equity-28d-store-v1";
pub const DAYS: u64 = 28;
const LOAN_ID: u64 = 1;
const COMPANY: u32 = 3;
const HOUSEHOLD: u32 = 2;
const TREASURY: u32 = 1;
const SUPPLIER: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scenario {
    Base,
    HouseholdCashShortage,
}

impl Scenario {
    #[cfg_attr(test, allow(dead_code))]
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "base" => Ok(Self::Base),
            "household-cash-shortage" => Ok(Self::HouseholdCashShortage),
            _ => Err(format!("unknown_scenario:{value}")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::HouseholdCashShortage => "household-cash-shortage",
        }
    }

    fn initial_household_cash(self) -> i64 {
        match self {
            Self::Base => 200_000,
            Self::HouseholdCashShortage => 0,
        }
    }
}

fn err(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

pub fn opening(scenario: Scenario) -> Result<Vec<u8>, String> {
    let household_cash = scenario.initial_household_cash();
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../../fixtures/m0-e0-opening.json"))
            .map_err(err)?;
    for source in value["sources"].as_array_mut().ok_or("sources")? {
        if source["id"].as_u64() == Some(2) {
            source["amount"] = household_cash.into();
        }
    }
    value["sources"].as_array_mut().ok_or("sources")?.extend([
        json!({"id":6,"kind":"inventory_capital","amount":2800}),
        json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    for account in value["accounts"].as_array_mut().ok_or("accounts")? {
        if account["id"].as_u64() == Some(HOUSEHOLD.into()) {
            account["cash"] = household_cash.into();
        }
    }
    value["stocks"].as_array_mut().ok_or("stocks")?.extend([
        json!({"id":2,"owner":3,"commodity":"raw","quantity":280,"basis":2800,"source":6}),
        json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    let person = json!({"kind":"person","id":7001});
    let treasury = json!({"kind":"treasury","id":1});
    value["town"] = json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{
            "recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,
            "input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,
            "waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,
            "utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,
            "utility_account":1,"freight_account":1,"disposal_account":1
        }],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,
            "route_source":77,"freight_account":1,"utility_account":1,"available_capacity":10,
            "freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters":[{
            "company":3,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,
                "max_direct_shareholders":8,"ordinary_shares_only":true,
                "cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":person,
            "investors":[
                {"party":person,"account":2,"opening_shares":6028},
                {"party":treasury,"account":1,"opening_shares":0}
            ],
            "opening_sources":[
                {"source_id":3,"party":person},
                {"source_id":6,"party":person},
                {"source_id":7,"party":person}
            ]
        }],
        "tax_rules":[{"company":3,"policy_id":77,"kind":"daily_realized_market_margin",
            "authority_account":1,"rate_basis_points":5000}],
        "exit_policy":{
            "policy_id":1,"company_account":3,"requester":2,"executor":1,"legal_clearance":"cleared",
            "priority_policy":{"order":["tax","wage","secured_debt","unsecured_debt","other"],
                "within_tier":"pro_rata_by_outstanding_stable_id"},
            "claims_complete":true,"tax_assessment_complete":true,"asset_register_complete":true,
            "employment_exit_terms_complete":true,
            "claims":[{"id":LOAN_ID,"payee":4,"class":"unsecured_debt"}],
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

fn account_cash(store: &Store, id: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.cash.value())
        .ok_or_else(|| format!("missing_account:{id}"))
}

fn total_cash(store: &Store) -> Result<i128, String> {
    Ok(store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .map(|row| i128::from(row.cash.value()))
        .sum())
}

fn stock_quantity(store: &Store, id: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .stocks
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.quantity.value())
        .ok_or_else(|| format!("missing_stock:{id}"))
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

fn add_payroll(store: &mut Store) -> Result<(), String> {
    execute(
        store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: COMPANY,
                    payee: HOUSEHOLD,
                    home_city: 0,
                    hourly_cents: Money::new(100).map_err(err)?,
                    cycle: Cycle::Daily,
                    ends_at: DAYS * DAY,
                },
            },
        },
    )
}

fn settle(store: &mut Store) -> Result<mycity_core::town_runtime::MarketSummary, String> {
    execute(store, Caller::System, Action::TownSettle { profile_id: 1 })?;
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .map(|town| town.state.last_summary)
        .ok_or_else(|| "missing_town_summary".into())
}

fn loan_outstanding(store: &Store) -> Result<i64, String> {
    Ok(store
        .engine()
        .map_err(err)?
        .timeline()
        .ok_or("missing_timeline")?
        .claims
        .iter()
        .find(|claim| claim.id == LOAN_ID)
        .map(|claim| claim.principal.value() - claim.paid.value())
        .unwrap_or(0))
}

fn tax_totals(store: &Store) -> Result<(usize, i64, i64), String> {
    let engine = store.engine().map_err(err)?;
    let timeline = engine.timeline().ok_or("missing_timeline")?;
    let mut count = 0;
    let mut assessed = 0_i64;
    let mut paid = 0_i64;
    for claim in timeline.claims.iter() {
        if timeline.tax_source_for_claim(claim).is_some() {
            count += 1;
            assessed += claim.principal.value();
            paid += claim.paid.value();
        }
    }
    Ok((count, assessed, paid))
}

pub fn run(path: &Path, scenario: Scenario) -> Result<Value, String> {
    if path.exists() {
        return Err(format!("store_path_already_exists:{}", path.display()));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    let mut store = Store::create(path, &opening(scenario)?, vec![], RULES).map_err(err)?;
    let opening_resources = resource_snapshot(&store)?;
    let initial_cash = total_cash(&store)?;
    let opening_company_cash = account_cash(&store, COMPANY)?;
    let opening_raw_basis = 2_800_i64;

    execute(
        &mut store,
        Caller::Account(TREASURY),
        Action::EquityContribute {
            company: COMPANY,
            amount: Money::new(10_000).map_err(err)?,
        },
    )?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    add_payroll(&mut store)?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Lend {
                id: LOAN_ID,
                lender: SUPPLIER,
                borrower: COMPANY,
                principal: Money::new(1_000).map_err(err)?,
                due: 14 * DAY,
            },
        },
    )?;

    let mut sales = 0_i64;
    let mut production = 0_i64;
    let mut waste = 0_i64;
    let mut goods_payments = 0_i64;
    let mut utilities = 0_i64;
    let mut sales_by_day = Vec::new();
    let mut tax_assessed_by_day = Vec::new();
    for day in 1..=DAYS {
        advance_to(&mut store, day * DAY)?;
        if day >= 14 && loan_outstanding(&store)? > 0 {
            execute(
                &mut store,
                Caller::System,
                Action::Timeline {
                    command: TimelineCommand::SettleClaim { id: LOAN_ID },
                },
            )?;
        }
        let summary = settle(&mut store)?;
        sales += summary.actual_sales.value();
        production += summary.produced.value();
        waste += summary.waste.value();
        goods_payments += summary.goods_payments.value();
        utilities += summary.utilities.value();
        sales_by_day.push(summary.actual_sales.value());
        let (_, assessed, _) = tax_totals(&store)?;
        tax_assessed_by_day.push(assessed);
        if day == 14 || day == DAYS {
            store = checkpoint_reopen(store, path, &format!("day_{day}"))?;
        }
    }
    advance_to(&mut store, (DAYS + 1) * DAY)?;
    let final_day = settle(&mut store)?;
    sales += final_day.actual_sales.value();
    production += final_day.produced.value();
    waste += final_day.waste.value();
    goods_payments += final_day.goods_payments.value();
    utilities += final_day.utilities.value();
    sales_by_day.push(final_day.actual_sales.value());
    let (tax_claim_count, tax_assessed, tax_paid) = tax_totals(&store)?;
    store = checkpoint_reopen(store, path, "day_29_tax_maturity")?;

    let cash_before_distribution = account_cash(&store, COMPANY)?;
    let treasury_before_distribution = account_cash(&store, TREASURY)?;
    let household_before_distribution = account_cash(&store, HOUSEHOLD)?;
    let retained = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("missing_town")?
        .state
        .tax_cursors
        .as_slice()[0]
        .retained_earnings_cents;
    let dividend_attempt = envelope(
        &store,
        Action::EquityDistributeRetainedEarnings {
            company: COMPANY,
            requested: Money::new(10_000).map_err(err)?,
        },
    )?;
    let dividend_operation_id = dividend_attempt.operation_id;
    let dividend_outcome = store
        .execute(Caller::Account(HOUSEHOLD), dividend_attempt)
        .map_err(err)?;
    if !dividend_outcome.has_persistent_receipt {
        return Err("dividend_missing_durable_receipt".into());
    }
    let dividend_receipt = store
        .receipt(dividend_operation_id)
        .map_err(err)?
        .ok_or("dividend_receipt_missing")?;
    let dividend_status = match dividend_receipt.outcome {
        Ok(_) => "distributed".to_owned(),
        Err(error) => format!("rejected:{error:?}"),
    };
    store = checkpoint_reopen(store, path, "after_dividend_attempt")?;

    let dividend_amount = if dividend_status == "distributed" {
        store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .town
            .ok_or("missing_town")?
            .state
            .equity_registers
            .as_slice()[0]
            .cumulative_dividends_distributed
            .value()
    } else {
        0
    };
    let government_dividend = account_cash(&store, TREASURY)? - treasury_before_distribution;
    let household_dividend = account_cash(&store, HOUSEHOLD)? - household_before_distribution;
    if household_dividend + government_dividend != dividend_amount {
        return Err(format!("dividend_cash_split_mismatch:{household_dividend}:{government_dividend}:{dividend_amount}"));
    }
    let company_after_distribution = account_cash(&store, COMPANY)?;
    let supplier_before_fiscal = account_cash(&store, SUPPLIER)?;
    if government_dividend > 0 {
        let treasury_ref = store
            .engine()
            .map_err(err)?
            .world()
            .account_ref(TREASURY)
            .map_err(err)?;
        let supplier_ref = store
            .engine()
            .map_err(err)?
            .world()
            .account_ref(SUPPLIER)
            .map_err(err)?;
        execute(
            &mut store,
            Caller::Account(TREASURY),
            Action::Transfer {
                from: treasury_ref,
                to: supplier_ref,
                amount: Money::new(government_dividend).map_err(err)?,
            },
        )?;
    }
    let fiscal_receipt = account_cash(&store, SUPPLIER)? - supplier_before_fiscal;

    let before_exit = account_cash(&store, COMPANY)?;
    execute(
        &mut store,
        Caller::Account(HOUSEHOLD),
        Action::BeginExit {
            company: COMPANY,
            policy_id: 1,
        },
    )?;
    store = checkpoint_reopen(store, path, "after_begin_exit")?;
    let exit_settlement_request = envelope(&store, Action::SettleExit { company: COMPANY })?;
    let exit_settlement_id = exit_settlement_request.operation_id;
    let exit_settlement = store
        .execute(Caller::Account(TREASURY), exit_settlement_request)
        .map_err(err)?;
    if !exit_settlement.has_persistent_receipt {
        return Err("exit_settlement_no_receipt".into());
    }
    let exit_settlement_status = match exit_settlement.model {
        Ok(_) => "settlement_recorded".to_owned(),
        Err(error) => format!("waiting_or_blocked:{error:?}"),
    };
    store = checkpoint_reopen(store, path, "after_exit_settlement")?;
    let close_request = envelope(&store, Action::CloseExit { company: COMPANY })?;
    let close_id = close_request.operation_id;
    let close = store
        .execute(Caller::Account(TREASURY), close_request)
        .map_err(err)?;
    if !close.has_persistent_receipt {
        return Err("close_no_receipt".into());
    }
    let close_status = match close.model {
        Ok(_) => "closed".to_owned(),
        Err(error) => format!("waiting_or_blocked:{error:?}"),
    };
    store = checkpoint_reopen(store, path, "after_close_attempt")?;

    let final_state = store.engine().map_err(err)?.world().view();
    let town = final_state.town.ok_or("missing_town_final")?;
    let register = town.state.equity_registers.as_slice()[0];
    let exit_phase = town.state.exit.ok_or("missing_exit_final")?.phase;
    let payroll = store
        .engine()
        .map_err(err)?
        .timeline()
        .ok_or("missing_timeline_final")?
        .payroll
        .iter()
        .find(|row| row.spec.id == 1)
        .ok_or("missing_payroll_final")?;
    let final_cash = total_cash(&store)?;
    if final_cash != initial_cash {
        return Err(format!("cash_not_conserved:{initial_cash}:{final_cash}"));
    }
    let raw_remaining = stock_quantity(&store, 2)?;
    let product_remaining = stock_quantity(&store, 3)?;
    if raw_remaining + production + waste != 280 || product_remaining + sales != production {
        return Err(format!("inventory_not_conserved:raw={raw_remaining}:production={production}:waste={waste}:sales={sales}:product={product_remaining}"));
    }
    if exit_settlement_id == close_id {
        return Err("operation_id_reused".into());
    }

    let account_cash_rows = final_state
        .accounts
        .iter()
        .map(|row| json!({"id":row.id,"cash":row.cash.value()}))
        .collect::<Vec<_>>();
    let base_arithmetic = json!({
        "max_production_and_sales_q":280,
        "gross_sales_at_full_demand":28000,
        "finite_opening_raw_basis":2800,
        "full_sales_goods_basis":25480,
        "recipe_utility_cost_at_full_output":280,
        "28_day_accrued_wages":22400,
        "28_day_labor_capitalized_into_production":22400,
        "idle_accrued_labor_expense_at_full_output":0,
        "full_sales_goods_basis_breakdown":"raw basis 2800 + utility 280 + capitalized accrued labor 22400",
        "realized_margin_formula":"gross_sales - sold_goods_basis - (accrued_wages - labor_capitalized_into_production)",
        "realized_margin_before_tax":2520,
        "margin_tax_at_50_percent":1260,
        "daily_tax_rounding_rule":"each service day floor(max(margin, 0) * rate_basis_points / 10000) in cents; sum daily claims",
        "personal_opening_shares":6028,
        "personal_paid_in_cash_and_raw_basis":602800,
        "government_contribution":10000,
        "government_shares_at_100_cents_each":100,
        "combined_paid_in_capital":612800,
        "expected_dividend_person_cents_largest_remainder":1239,
        "expected_dividend_government_cents_largest_remainder":21,
        "loan_borrowed_and_repaid_once":1000
    });
    let pressure_arithmetic = json!({
        "cash_shortage_daily_sales_pattern":"starting cash 0; daily 800 accrued/paid wages and 102 landed price, with production blocked until the prior output lot is sold",
        "cash_shortage_expected_sales_q":219,
        "cash_shortage_expected_production_q":220,
        "cash_shortage_unit_sold_goods_basis":91,
        "cash_shortage_expected_unsold_raw_q":60,
        "cash_shortage_expected_unsold_product_q":1,
        "cash_shortage_expected_goods_basis_sold_cents":19929,
        "cash_shortage_expected_labor_capitalized_cents":17600,
        "cash_shortage_expected_idle_accrued_labor_cents":4800,
        "cash_shortage_expected_taxable_margin_cents":-2829,
        "cash_shortage_expected_daily_rounded_tax_cents":938,
        "cash_shortage_expected_positive_daily_tax_claims":22,
        "cash_shortage_expected_retained_after_tax_cents":-3767,
        "cash_shortage_expected_dividend_cents":0,
        "cash_shortage_initial_total_cash_cents":2800000
    });
    let reference_arithmetic = json!({
        "base":base_arithmetic,
        "household_cash_shortage":pressure_arithmetic
    });
    let input_contract = json!({
        "households":1,"companies":1,"recipes":1,"shops":1,"payroll_rows":1,
        "opening_company_cash_cents":opening_company_cash,
        "opening_raw_quantity":280,"opening_raw_basis_cents":opening_raw_basis,
        "household_opening_cash_cents":scenario.initial_household_cash(),
        "government_equity_contribution_cents":10000,"loan_principal_cents":1000,
        "hourly_wage_cents_per_hour":100,"paid_hours_per_day":8,
        "output_q_per_day":10,
        "utility_cost_per_output_q_cents":1,"shelf_price_cents_per_q":100,
        "delivery_surcharge_cents_per_q":2,"tax_rate_basis_points":5000,
        "reference_arithmetic_cents":reference_arithmetic,
        "route_provider":"fixed quote profile source 77 (not live geometry)",
        "utility_provider":"fixed recipe/profile cost (not live Boot supply)",
        "opening_cash_and_raw_are_finite_external_capital":true,
        "self_financing_claim":"not_asserted"
    });

    // Keep the report schema flat for callers while splitting construction into bounded
    // objects so one large `json!` invocation does not hit serde_json's macro recursion limit.
    let mut report = serde_json::Map::new();
    report.insert("scenario".into(), json!(scenario.name()));
    report.insert("rules".into(), json!(RULES));
    report.insert("game_days".into(), json!(DAYS));
    report.insert("self_financing_claim".into(), json!("not_asserted"));
    report.insert("input_contract".into(), input_contract);
    let observed = json!({
        "sales_by_day":sales_by_day,"sales_total":sales,"production_total":production,
        "waste_total":waste,"goods_payments_total_cents":goods_payments,
        "market_utility_payments_total_cents":utilities,
        "production_utility_cost_cents":production,
        "buyer_utility_surcharge_cents":sales,
        "initial_total_cash_cents":initial_cash,
        "household_opening_cash_cents":scenario.initial_household_cash(),
        "household_final_cash_cents":account_cash(&store,HOUSEHOLD)?,
        "seller_payroll_billed_cents":payroll.billed.value(),
        "seller_payroll_paid_cents":payroll.paid.value(),
        "seller_payroll_accrued_cents":payroll.accrued.value(),
        "raw_remaining":raw_remaining,"product_remaining":product_remaining,
        "loan_outstanding_after_day29_cents":loan_outstanding(&store)?,
        "tax_assessed_cents":tax_assessed,"tax_paid_cents":tax_paid,"tax_claim_count":tax_claim_count,
        "retained_earnings_after_day29_cents":retained
    });
    let settlement = json!({
        "dividend_status":dividend_status,"dividend_operation_id":dividend_operation_id,
        "dividend_total_cents":dividend_amount,"government_dividend_receipt_cents":government_dividend,
        "household_dividend_receipt_cents":household_dividend,
        "fiscal_supplier_receipt_cents":fiscal_receipt,
        "company_cash_before_distribution_cents":cash_before_distribution,
        "company_cash_after_distribution_cents":company_after_distribution,
        "company_cash_before_exit_cents":before_exit,
        "exit_settlement_status":exit_settlement_status,"close_status":close_status,
        "exit_phase":format!("{exit_phase:?}"),"equity_issued_shares":register.issued_shares,
        "capital_returned_cents":register.cumulative_capital_returned.value(),
        "liquidation_surplus_distributed_cents":register.cumulative_liquidation_surplus_distributed.value(),
        "final_accounts":account_cash_rows,"total_cash_final_cents":final_cash
    });
    for section in [observed, settlement] {
        let fields = section.as_object().ok_or("report_section_not_object")?;
        report.extend(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }
    report.insert(
        "route_provider".into(),
        json!("fixed quote profile source 77; no live spatial graph"),
    );
    report.insert(
        "utility_provider".into(),
        json!("fixed per-output cost; no live utility supply"),
    );
    report.insert("resources".into(), json!({"opening":opening_resources,"final":resource_snapshot(&store)?,"scope":"owned capacities, not native process peak"}));
    report.insert("final_sequence".into(), json!(store.committed_sequence()));
    report.insert(
        "final_state_hash".into(),
        json!(store.state_hash().map_err(err)?),
    );
    report.insert("full_16gb".into(), json!("not_run"));
    report.insert("native_process_peak".into(), json!("not_measured"));
    Ok(Value::Object(report))
}

fn resource_snapshot(store: &Store) -> Result<Value, String> {
    let engine = store.engine().map_err(err)?;
    let world_owned = engine.world().layout()["owned_capacity_bytes"]
        .as_u64()
        .ok_or("missing_world_owned_capacity")?;
    Ok(
        json!({"store_owned_bytes":store.owned_capacity_bytes().map_err(err)?,
        "engine_owned_bytes":engine.owned_capacity_bytes(),"world_owned_bytes":world_owned}),
    )
}
