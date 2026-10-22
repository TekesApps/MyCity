//! Paired 28-day employment interruption/change using the real Engine, Timeline and TownSettle.
//!
//! The household's destination job is an explicit assignment to the source-funded Company
//! account (account 5). That job has payroll but no modeled recipe. It is not job search,
//! skills matching, a labor market, or a complete service-sector model. All cash starts from
//! the fixed opening fixture; no sales, wages, or cash transfers are prewritten.
use mycity_core::{
    assembly::{assemble, Role},
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Engine, Envelope, ENGINE_BYTE_BUDGET},
    units::Money,
};
use serde::Serialize;
use serde_json::Value;
use std::env;

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const DAYS: u64 = 28;
const DEFAULT_SWITCH_DAY: u64 = 10;
const PAID_WORKDAY_SECONDS: u64 = 8 * 60 * 60;
const HOUSEHOLD: u32 = 2;
const PRODUCER: u32 = 3;
const DESTINATION_EMPLOYER: u32 = 5;
const OLD_PAYROLL: u32 = 1;
const NEW_PAYROLL: u32 = 2;

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct Totals {
    produced: i128,
    sold: i128,
    unmet: i128,
    waste: i128,
    goods_payments: i128,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ResultRow {
    scenario: &'static str,
    changed: bool,
    totals: Totals,
    initial_cash: i128,
    final_cash: i128,
    household_open: i64,
    household_close: i64,
    producer_open: i64,
    producer_close: i64,
    destination_employer_open: i64,
    destination_employer_close: i64,
    old_wage_cents_per_hour: i64,
    new_wage_cents_per_hour: i64,
    initial_raw: i64,
    raw_close: i64,
    initial_product: i64,
    initial_retail_stock: i64,
    product_close: i64,
    retail_units_sold: i64,
    retail_goods_revenue: i64,
    old_accrued: i64,
    old_paid: i64,
    old_work_seconds_at_switch: u64,
    old_work_seconds_close: u64,
    old_arrears_at_switch: i64,
    old_arrears_close: i64,
    new_accrued: i64,
    new_paid: i64,
    new_work_seconds: u64,
    new_arrears: i64,
}

#[derive(Clone, Copy, Debug)]
struct Scenario {
    name: &'static str,
    household_cash: i64,
    producer_cash: i64,
    destination_cash: i64,
    old_wage_cents: i64,
    new_wage_cents: i64,
    initial_retail_stock: i64,
}

const ARREARS: Scenario = Scenario {
    name: "arrears_stress",
    household_cash: 5_000,
    producer_cash: 1_000,
    destination_cash: 60_000,
    old_wage_cents: 100,
    new_wage_cents: 50,
    initial_retail_stock: 0,
};
const SOLVENT: Scenario = Scenario {
    name: "solvent_production_pair",
    household_cash: 5_000,
    producer_cash: 60_000,
    destination_cash: 60_000,
    old_wage_cents: 100,
    new_wage_cents: 50,
    initial_retail_stock: 0,
};
const PURCHASING_POWER_HIGH: Scenario = Scenario {
    name: "purchasing_power_high_wage",
    household_cash: 0,
    producer_cash: 60_000,
    destination_cash: 60_000,
    old_wage_cents: 20,
    new_wage_cents: 100,
    initial_retail_stock: 1_000,
};
const PURCHASING_POWER_LOW: Scenario = Scenario {
    name: "purchasing_power_low_wage",
    new_wage_cents: 20,
    ..PURCHASING_POWER_HIGH
};

fn money(cents: i64) -> Result<Money, String> {
    Money::new(cents).map_err(|error| format!("money:{error:?}"))
}

fn command(engine: &mut Engine, action: Action) -> Result<(), String> {
    let view = engine.world().view();
    engine
        .execute(
            Caller::System,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: view.revision + 100,
                expected_revision: view.revision,
                actions: vec![action],
            },
        )
        .map(|_| ())
        .map_err(|error| format!("engine:{error:?}"))
}

fn build_engine(scenario: Scenario) -> Result<Engine, String> {
    let mut opening: Value = serde_json::from_slice(OPENING).map_err(|error| error.to_string())?;
    // Deliberately finite, source-backed household and producer balances make the wage
    // change visible; both paired runs start from the same funds.
    for source in opening["sources"].as_array_mut().ok_or("opening_sources")? {
        if source["id"].as_u64() == Some(2) {
            source["amount"] = scenario.household_cash.into();
        }
        if source["id"].as_u64() == Some(u64::from(PRODUCER)) {
            source["amount"] = scenario.producer_cash.into();
        }
    }
    for account in opening["accounts"]
        .as_array_mut()
        .ok_or("opening_accounts")?
    {
        if account["id"].as_u64() == Some(u64::from(HOUSEHOLD)) {
            account["cash"] = scenario.household_cash.into();
        }
        if account["id"].as_u64() == Some(u64::from(PRODUCER)) {
            account["cash"] = scenario.producer_cash.into();
        }
    }
    opening["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":10000}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
        serde_json::json!({"id":8,"kind":"cash_capital","amount":scenario.destination_cash}),
        serde_json::json!({"id":9,"kind":"inventory_capital","amount":scenario.initial_retail_stock}),
    ]);
    opening["accounts"].as_array_mut().unwrap().push(
        serde_json::json!({"id":5,"role":"company","cash":scenario.destination_cash,"source":8}),
    );
    opening["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":10000,"basis":10000,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
        serde_json::json!({"id":4,"owner":5,"commodity":"product","quantity":scenario.initial_retail_stock,"basis":scenario.initial_retail_stock,"source":9}),
    ]);
    opening["town"] = serde_json::json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":4,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1},{"account":5,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100},
            {"shop_id":2,"account":5,"stock_id":4,"good":1,"shelf_price_per_q":100}],
        "recipes":[{
            "recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,
            "input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,
            "waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,
            "utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,
            "utility_account":1,"freight_account":1,"disposal_account":1
        }],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,
            "reachable":true,"route_source":77,"freight_account":1,"utility_account":1,
            "available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1},
            {"quote_id":2,"shop":2,"household_group":1,"good":1,
            "reachable":true,"route_source":78,"freight_account":1,"utility_account":1,
            "available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}]
    });
    let world = assemble(&serde_json::to_vec(&opening).map_err(|error| error.to_string())?)?;
    let view = world.view();
    for employer in [PRODUCER, DESTINATION_EMPLOYER] {
        if !view
            .accounts
            .iter()
            .any(|account| account.id == employer && account.role == Role::Company)
            || !view.town.as_ref().is_some_and(|town| {
                town.profile
                    .employers
                    .as_slice()
                    .iter()
                    .any(|spec| spec.account == employer)
            })
        {
            return Err(format!(
                "payroll_employer_role_or_profile_mismatch:{employer}"
            ));
        }
    }
    let engine =
        Engine::new(world, Vec::new()).map_err(|error| format!("engine_init:{error:?}"))?;
    if engine.owned_capacity_bytes() > ENGINE_BYTE_BUDGET {
        return Err("engine_memory_budget_exceeded".into());
    }
    Ok(engine)
}

fn cash(engine: &Engine, account_id: u32) -> Result<i64, String> {
    engine
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .map(|account| account.cash.value())
        .ok_or_else(|| format!("missing_account:{account_id}"))
}

fn total_cash(engine: &Engine) -> i128 {
    engine
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum()
}

fn parse_switch_day() -> Result<u64, String> {
    let mut args = env::args().skip(1);
    let day = match args.next() {
        None => DEFAULT_SWITCH_DAY,
        Some(arg) if arg == "--switch-day" => args
            .next()
            .ok_or("missing_value:switch-day")?
            .parse::<u64>()
            .map_err(|_| "invalid_value:switch-day")?,
        Some(arg) => arg
            .strip_prefix("--switch-day=")
            .ok_or_else(|| format!("unknown_argument:{arg}"))?
            .parse::<u64>()
            .map_err(|_| "invalid_value:switch-day")?,
    };
    if args.next().is_some() {
        return Err("unexpected_extra_argument".into());
    }
    if day == 0 || day >= DAYS {
        return Err("switch_day_must_be_between_1_and_27".into());
    }
    Ok(day)
}

fn run(scenario: Scenario, changed: bool, switch_day: u64) -> Result<ResultRow, String> {
    let mut engine = build_engine(scenario)?;
    let initial_cash = total_cash(&engine);
    let household_open = cash(&engine, HOUSEHOLD)?;
    let producer_open = cash(&engine, PRODUCER)?;
    let destination_employer_open = cash(&engine, DESTINATION_EMPLOYER)?;
    let initial_raw = engine
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == 2)
        .ok_or("missing_raw_stock")?
        .quantity
        .value();
    let initial_product = engine
        .world()
        .view()
        .stocks
        .iter()
        .filter(|stock| stock.id == 3 || stock.id == 4)
        .map(|stock| stock.quantity.value())
        .sum::<i64>();

    command(
        &mut engine,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    command(
        &mut engine,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: OLD_PAYROLL,
                    payer: PRODUCER,
                    payee: HOUSEHOLD,
                    home_city: 0,
                    hourly_cents: money(scenario.old_wage_cents)?,
                    cycle: Cycle::Daily,
                    ends_at: DAYS * DAY,
                },
            },
        },
    )?;

    let mut totals = Totals::default();
    let mut old_arrears_at_switch = 0;
    let mut old_billed_at_switch = 0;
    let mut old_paid_at_switch = 0;
    let mut old_work_seconds_at_switch = 0;
    for day in 1..=DAYS {
        for _ in 0..128 {
            command(
                &mut engine,
                Action::Timeline {
                    command: TimelineCommand::Advance {
                        target: day * DAY,
                        limit: 8192,
                    },
                },
            )?;
            if engine
                .timeline()
                .is_some_and(|timeline| timeline.progress.target.is_none())
            {
                break;
            }
        }
        if engine
            .timeline()
            .is_none_or(|timeline| timeline.progress.target.is_some())
        {
            return Err(format!("timeline_did_not_finish_day:{day}"));
        }

        command(
            &mut engine,
            Action::Timeline {
                command: TimelineCommand::SettlePayroll { id: OLD_PAYROLL },
            },
        )?;
        if changed && day > switch_day {
            command(
                &mut engine,
                Action::Timeline {
                    command: TimelineCommand::SettlePayroll { id: NEW_PAYROLL },
                },
            )?;
        }
        let view = engine.world().view();
        let receipt = engine
            .execute(
                Caller::System,
                Envelope {
                    world: view.scope.world(),
                    branch: view.scope.branch(),
                    operation_id: view.revision + 100,
                    expected_revision: view.revision,
                    actions: vec![Action::TownSettle { profile_id: 1 }],
                },
            )
            .map_err(|error| format!("town_day_{day}:{error:?}"))?;
        let summary = receipt
            .town
            .filter(|summary| summary.day == day)
            .ok_or_else(|| format!("missing_day_summary:{day}"))?;
        totals.produced += i128::from(summary.produced.value());
        totals.sold += i128::from(summary.actual_sales.value());
        totals.unmet += i128::from(summary.unmet_market_demand.value());
        totals.waste += i128::from(summary.waste.value());
        totals.goods_payments += i128::from(summary.goods_payments.value());

        if changed && day == switch_day {
            // At the completed day boundary, end the old payroll first. Terminate freezes its
            // accrual and tries to pay billed wages; any unpayable billed-minus-paid balance
            // remains on that authoritative Payroll row. The next contract starts at exactly
            // this timestamp, so it earns no overlapping seconds.
            command(
                &mut engine,
                Action::Timeline {
                    command: TimelineCommand::Terminate { id: OLD_PAYROLL },
                },
            )?;
            let old = engine
                .timeline()
                .and_then(|timeline| {
                    timeline
                        .payroll
                        .iter()
                        .find(|row| row.spec.id == OLD_PAYROLL)
                })
                .copied()
                .ok_or("missing_terminated_payroll")?;
            if old.working || old.next_due.is_some() || old.spec.ends_at != day * DAY {
                return Err("old_payroll_not_terminated_at_boundary".into());
            }
            old_billed_at_switch = old.billed.value();
            old_paid_at_switch = old.paid.value();
            old_arrears_at_switch = old.billed.value() - old.paid.value();
            old_work_seconds_at_switch = old.work_seconds;
            command(
                &mut engine,
                Action::Timeline {
                    command: TimelineCommand::AddPayroll {
                        spec: PayrollSpec {
                            id: NEW_PAYROLL,
                            payer: DESTINATION_EMPLOYER,
                            payee: HOUSEHOLD,
                            home_city: 0,
                            hourly_cents: money(scenario.new_wage_cents)?,
                            cycle: Cycle::Daily,
                            ends_at: DAYS * DAY,
                        },
                    },
                },
            )?;
            let new = engine
                .timeline()
                .and_then(|timeline| {
                    timeline
                        .payroll
                        .iter()
                        .find(|row| row.spec.id == NEW_PAYROLL)
                })
                .ok_or("missing_new_payroll")?;
            if new.started != day * DAY
                || new.spec.payer != DESTINATION_EMPLOYER
                || old.spec.ends_at != new.started
            {
                return Err("new_payroll_effective_time_mismatch".into());
            }
        }
    }

    let timeline = engine.timeline().ok_or("missing_timeline")?;
    let old = timeline
        .payroll
        .iter()
        .find(|row| row.spec.id == OLD_PAYROLL)
        .ok_or("missing_old_payroll")?;
    let old_arrears_close = old.billed.value() - old.paid.value();
    let old_work_seconds_close = old.work_seconds;
    let (new_accrued, new_paid, new_arrears, new_work_seconds) = timeline
        .payroll
        .iter()
        .find(|row| row.spec.id == NEW_PAYROLL)
        .map(|row| {
            (
                row.accrued.value(),
                row.paid.value(),
                row.billed.value() - row.paid.value(),
                row.work_seconds,
            )
        })
        .unwrap_or((0, 0, 0, 0));
    // Ending work stops accrual, not collection of wages already owed. Existing
    // Timeline settlement may repay arrears from the old employer's later sales.
    if changed
        && (old.billed.value() != old_billed_at_switch
            || old.paid.value() < old_paid_at_switch
            || old_arrears_close < 0
            || old_arrears_at_switch - old_arrears_close != old.paid.value() - old_paid_at_switch)
    {
        return Err("terminated_payroll_arrears_reconciliation_failed".into());
    }
    let retail_remaining = engine
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == 4)
        .ok_or("missing_retail_stock_close")?
        .quantity
        .value();
    let retail_units_sold = scenario
        .initial_retail_stock
        .checked_sub(retail_remaining)
        .ok_or("retail_stock_increased_without_supply")?;
    if retail_units_sold < 0 {
        return Err("retail_stock_increased_without_supply".into());
    }
    let retail_goods_revenue = retail_units_sold
        .checked_mul(100)
        .ok_or("retail_revenue_overflow")?;
    if destination_employer_open + retail_goods_revenue - cash(&engine, DESTINATION_EMPLOYER)?
        != new_paid
    {
        return Err("new_employer_wages_and_retail_revenue_do_not_reconcile".into());
    }
    let old_days = if changed { switch_day } else { DAYS };
    let new_days = if changed { DAYS - switch_day } else { 0 };
    if old_work_seconds_close != old_days * PAID_WORKDAY_SECONDS
        || new_work_seconds != new_days * PAID_WORKDAY_SECONDS
        || old.accrued.value() != (old_days as i64) * 8 * scenario.old_wage_cents
        || new_accrued != (new_days as i64) * 8 * scenario.new_wage_cents
    {
        return Err("work_and_wage_totals_do_not_match_nonoverlapping_contract_intervals".into());
    }
    if changed && old_work_seconds_close != old_work_seconds_at_switch {
        return Err("terminated_payroll_accrued_new_work_after_switch".into());
    }
    let raw_close = engine
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == 2)
        .ok_or("missing_raw_stock_close")?
        .quantity
        .value();
    let product_close = engine
        .world()
        .view()
        .stocks
        .iter()
        .filter(|stock| stock.id == 3 || stock.id == 4)
        .map(|stock| stock.quantity.value())
        .sum::<i64>();
    if i128::from(initial_raw - raw_close) != totals.produced + totals.waste {
        return Err("raw_material_conservation_failed".into());
    }
    if i128::from(initial_product) + totals.produced - totals.sold != i128::from(product_close) {
        return Err("product_inventory_conservation_failed".into());
    }
    let final_cash = total_cash(&engine);
    if final_cash != initial_cash {
        return Err("cash_conservation_failed".into());
    }
    Ok(ResultRow {
        scenario: scenario.name,
        changed,
        totals,
        initial_cash,
        final_cash,
        household_open,
        household_close: cash(&engine, HOUSEHOLD)?,
        producer_open,
        producer_close: cash(&engine, PRODUCER)?,
        destination_employer_open,
        destination_employer_close: cash(&engine, DESTINATION_EMPLOYER)?,
        old_wage_cents_per_hour: scenario.old_wage_cents,
        new_wage_cents_per_hour: if changed { scenario.new_wage_cents } else { 0 },
        initial_raw,
        raw_close,
        initial_product,
        initial_retail_stock: scenario.initial_retail_stock,
        product_close,
        retail_units_sold,
        retail_goods_revenue,
        old_accrued: old.accrued.value(),
        old_paid: old.paid.value(),
        old_work_seconds_at_switch,
        old_work_seconds_close,
        old_arrears_at_switch,
        old_arrears_close,
        new_accrued,
        new_paid,
        new_work_seconds,
        new_arrears,
    })
}

fn assert_same_opening(stay: ResultRow, changed: ResultRow) -> Result<(), String> {
    if stay.initial_cash != changed.initial_cash
        || stay.household_open != changed.household_open
        || stay.producer_open != changed.producer_open
        || stay.destination_employer_open != changed.destination_employer_open
        || stay.initial_raw != changed.initial_raw
        || stay.initial_product != changed.initial_product
    {
        return Err(format!("paired_opening_mismatch:{stay:?};{changed:?}"));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let switch_day = parse_switch_day()?;
    let arrears_stay = run(ARREARS, false, switch_day)?;
    let arrears_changed = run(ARREARS, true, switch_day)?;
    assert_same_opening(arrears_stay, arrears_changed)?;
    if switch_day == DEFAULT_SWITCH_DAY && arrears_changed.old_arrears_at_switch <= 0 {
        return Err("arrears_stress_did_not_create_unpaid_wages".into());
    }

    let solvent_stay = run(SOLVENT, false, switch_day)?;
    let solvent_changed = run(SOLVENT, true, switch_day)?;
    assert_same_opening(solvent_stay, solvent_changed)?;
    if switch_day == DEFAULT_SWITCH_DAY
        && solvent_changed.totals.produced >= solvent_stay.totals.produced
    {
        return Err(format!(
            "solvent_production_change_not_observed:{solvent_stay:?};{solvent_changed:?}"
        ));
    }

    let purchasing_high = run(PURCHASING_POWER_HIGH, true, switch_day)?;
    let purchasing_low = run(PURCHASING_POWER_LOW, true, switch_day)?;
    assert_same_opening(purchasing_high, purchasing_low)?;
    if purchasing_high.new_paid <= purchasing_low.new_paid {
        return Err("purchasing_power_wage_payment_not_different".into());
    }
    if switch_day == DEFAULT_SWITCH_DAY && purchasing_high.totals.sold <= purchasing_low.totals.sold
    {
        return Err(format!(
            "purchasing_power_sales_effect_not_observed:{purchasing_high:?};{purchasing_low:?}"
        ));
    }

    let rows = [
        arrears_stay,
        arrears_changed,
        solvent_stay,
        solvent_changed,
        purchasing_high,
        purchasing_low,
    ];
    let output = serde_json::json!({
        "model":"explicit_employment_change_fixed_opening",
        "days":DAYS,
        "switch_day":switch_day,
        "money_unit":"cents",
        "opening_capital_is_source_backed":true,
        "continuous_self_funding_demonstrated":false,
        "employment_assignment":"explicit fixed Company payroll; not endogenous job search or skills matching",
        "transport_and_utility":"profile substitutes only",
        "legal_employment_flow":"not modeled",
        "product_inventory_conservation":"opening_stock + actual production - actual sales = closing_stock",
        "scenarios":rows,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
    );
    for row in rows {
        if row.initial_cash != row.final_cash {
            return Err("cash_conservation_failed_after_reporting".into());
        }
    }
    Ok(())
}
