//! Execute a real 28-game-day Engine market loop under named, reproducible profiles.
//!
//! Route reachability/capacity and utility availability/cost are explicit profile inputs,
//! not live transport or utility-network assignments. Timeline remains authoritative for
//! wages, the optional loan, and optional turnover-tax fixture claims; TownSettle remains
//! authoritative for market cash and stock changes. Run without arguments to retain the
//! original base profile.
use mycity_core::{
    assembly::assemble,
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Engine, Envelope, ENGINE_BYTE_BUDGET},
    units::Money,
};
use serde_json::Value;
use std::env;

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const DAYS: u64 = 28;
const DEBT_ID: u64 = 1;
const TURNOVER_TAX_CLAIM_ID_BASE: u64 = 10_000;
const TURNOVER_TAX_AUTHORITY: u32 = 1;
const TURNOVER_TAX_PAYER: u32 = 3;
const TURNOVER_TAX_BPS_DENOMINATOR: i128 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    Base,
    CashShortage,
    RawShortage,
    NoRoute,
    CapacityCut,
    EnergyPriceRise,
    DemandDrop,
}

impl Scenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "base" => Ok(Self::Base),
            "cash-shortage" => Ok(Self::CashShortage),
            "raw-shortage" => Ok(Self::RawShortage),
            "no-route" => Ok(Self::NoRoute),
            "capacity-cut" => Ok(Self::CapacityCut),
            "energy-price-rise" => Ok(Self::EnergyPriceRise),
            "demand-drop" => Ok(Self::DemandDrop),
            _ => Err(format!("unknown_scenario:{value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::CashShortage => "cash-shortage",
            Self::RawShortage => "raw-shortage",
            Self::NoRoute => "no-route",
            Self::CapacityCut => "capacity-cut",
            Self::EnergyPriceRise => "energy-price-rise",
            Self::DemandDrop => "demand-drop",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Inputs {
    scenario: Scenario,
    household_cash: i64,
    company_cash: i64,
    raw_quantity: i64,
    daily_need: i64,
    route_reachable: bool,
    route_capacity: i64,
    production_utility_cost_per_q: i64,
    debt_principal: i64,
    debt_due_day: u64,
    turnover_tax_bps: u32,
}

impl Inputs {
    fn for_scenario(scenario: Scenario) -> Self {
        let mut inputs = Self {
            scenario,
            household_cash: 200_000,
            company_cash: 600_000,
            raw_quantity: 100_000,
            daily_need: 10,
            route_reachable: true,
            route_capacity: 100,
            production_utility_cost_per_q: 1,
            debt_principal: 100_000,
            debt_due_day: 14,
            turnover_tax_bps: 0,
        };
        match scenario {
            Scenario::Base => {}
            Scenario::CashShortage => inputs.household_cash = 0,
            Scenario::RawShortage => inputs.raw_quantity = 15,
            Scenario::NoRoute => {
                inputs.route_reachable = false;
                inputs.route_capacity = 0;
            }
            Scenario::CapacityCut => inputs.route_capacity = 3,
            Scenario::EnergyPriceRise => inputs.production_utility_cost_per_q = 1_000,
            Scenario::DemandDrop => inputs.daily_need = 2,
        }
        inputs
    }

    fn apply_override(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "household-cash" => self.household_cash = parse_i64(key, value)?,
            "company-cash" => self.company_cash = parse_i64(key, value)?,
            "raw-quantity" => self.raw_quantity = parse_i64(key, value)?,
            "daily-need" => self.daily_need = parse_i64(key, value)?,
            "route-capacity" => self.route_capacity = parse_i64(key, value)?,
            "production-utility-cost" => {
                self.production_utility_cost_per_q = parse_i64(key, value)?
            }
            "route-reachable" => {
                self.route_reachable = match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("invalid_{key}:{value}")),
                }
            }
            "debt-principal" => self.debt_principal = parse_i64(key, value)?,
            "debt-due-day" => {
                self.debt_due_day = value
                    .parse()
                    .map_err(|_| format!("invalid_{key}:{value}"))?
            }
            "turnover-tax-bps" => {
                self.turnover_tax_bps = value
                    .parse()
                    .map_err(|_| format!("invalid_{key}:{value}"))?
            }
            _ => return Err(format!("unknown_option:{key}")),
        }
        Ok(())
    }

    fn validate(self) -> Result<(), String> {
        if self.household_cash < 0
            || self.company_cash < 0
            || self.raw_quantity < 0
            || self.daily_need <= 0
            || self.route_capacity < 0
            || self.production_utility_cost_per_q < 0
            || self.debt_principal < 0
            || self.debt_due_day == 0
            || self.debt_due_day > DAYS
            || self.turnover_tax_bps > 10_000
        {
            return Err("invalid_scenario_values_or_turnover_tax_bps_over_10000".into());
        }
        Ok(())
    }
}

fn parse_i64(name: &str, value: &str) -> Result<i64, String> {
    value.parse().map_err(|_| format!("invalid_{name}:{value}"))
}

fn parse_args() -> Result<Inputs, String> {
    let mut args = env::args().skip(1);
    let mut scenario = Scenario::Base;
    let mut overrides = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Err("usage: town_market [--scenario base|cash-shortage|raw-shortage|no-route|capacity-cut|energy-price-rise|demand-drop] [--household-cash N] [--company-cash N] [--raw-quantity N] [--daily-need N] [--route-reachable true|false] [--route-capacity N] [--production-utility-cost N] [--debt-principal N] [--debt-due-day 1..28] [--turnover-tax-bps 0..10000]".into());
        }
        let (key, value) = arg
            .strip_prefix("--")
            .ok_or_else(|| format!("invalid_argument:{arg}"))?
            .split_once('=')
            .map(|(key, value)| (key.to_owned(), Some(value.to_owned())))
            .unwrap_or_else(|| (arg.trim_start_matches("--").to_owned(), None));
        if key == "scenario" {
            let value = match value {
                Some(value) => value,
                None => args.next().ok_or("missing_value:scenario")?,
            };
            scenario = Scenario::parse(&value)?;
        } else {
            let value = match value {
                Some(value) => value,
                None => args.next().ok_or_else(|| format!("missing_value:{key}"))?,
            };
            overrides.push((key, value));
        }
    }
    let mut inputs = Inputs::for_scenario(scenario);
    for (key, value) in overrides {
        inputs.apply_override(&key, &value)?;
    }
    inputs.validate()?;
    Ok(inputs)
}

fn command(engine: &mut Engine, action: Action) -> Result<(), String> {
    let view = engine.world().view();
    let operation_id = view.revision + 100;
    engine
        .execute(
            Caller::System,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id,
                expected_revision: view.revision,
                actions: vec![action],
            },
        )
        .map(|_| ())
        .map_err(|error| format!("engine:{error:?}"))
}

fn money(value: i64) -> Result<Money, String> {
    Money::new(value).map_err(|error| format!("money:{error:?}"))
}

fn build_engine(inputs: Inputs) -> Result<Engine, String> {
    let mut opening: Value = serde_json::from_slice(OPENING).map_err(|e| e.to_string())?;
    for source in opening["sources"].as_array_mut().ok_or("opening_sources")? {
        match source["id"].as_u64() {
            Some(2) => source["amount"] = inputs.household_cash.into(),
            Some(3) => source["amount"] = inputs.company_cash.into(),
            _ => {}
        }
    }
    for account in opening["accounts"]
        .as_array_mut()
        .ok_or("opening_accounts")?
    {
        match account["id"].as_u64() {
            Some(2) => account["cash"] = inputs.household_cash.into(),
            Some(3) => account["cash"] = inputs.company_cash.into(),
            _ => {}
        }
    }
    opening["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":inputs.raw_quantity}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    opening["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":inputs.raw_quantity,"basis":inputs.raw_quantity,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    opening["town"] = serde_json::json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":inputs.daily_need,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{
            "recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,
            "input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,
            "waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,
            "utility_cost_per_output_q":inputs.production_utility_cost_per_q,
            "freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,
            "utility_account":1,"freight_account":1,"disposal_account":1
        }],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,
            "reachable":inputs.route_reachable,"route_source":77,"freight_account":1,"utility_account":1,
            "available_capacity":inputs.route_capacity,"freight_per_q":1,"utility_surcharge_per_q":1}]
    });
    let world = assemble(&serde_json::to_vec(&opening).map_err(|e| e.to_string())?)?;
    let engine =
        Engine::new(world, Vec::new()).map_err(|error| format!("engine_init:{error:?}"))?;
    if engine.owned_capacity_bytes() > ENGINE_BYTE_BUDGET {
        return Err("engine_memory_budget_exceeded".into());
    }
    Ok(engine)
}

fn main() -> Result<(), String> {
    let inputs = parse_args()?;
    let mut engine = build_engine(inputs)?;
    let initial = engine.world().view();
    let initial_cash: i128 = initial
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum();
    let initial_cash_by_account: Vec<_> = initial
        .accounts
        .iter()
        .map(|account| (account.id, account.cash.value()))
        .collect();
    let initial_treasury_cash = initial_cash_by_account
        .iter()
        .find(|(account, _)| *account == TURNOVER_TAX_AUTHORITY)
        .map(|(_, cash)| *cash)
        .ok_or("missing_initial_treasury")?;
    let initial_raw = initial
        .stocks
        .iter()
        .find(|stock| stock.id == 2)
        .ok_or("missing_raw")?;
    let initial_product = initial
        .stocks
        .iter()
        .find(|stock| stock.id == 3)
        .ok_or("missing_product")?;
    let initial_raw_quantity = initial_raw.quantity.value();
    let initial_raw_basis = initial_raw.basis.value();
    let initial_product_quantity = initial_product.quantity.value();
    let initial_product_basis = initial_product.basis.value();
    drop(initial);

    let mut produced = 0i128;
    let mut waste = 0i128;
    let mut sales = 0i128;
    let mut reference_need = 0i128;
    let mut market_demand = 0i128;
    let mut unmet_market_demand = 0i128;
    let mut goods_payments = 0i128;
    let mut freight = 0i128;
    let mut utilities = 0i128;
    let mut disposal = 0i128;
    let mut wages_accrued = 0i128;
    let mut labor_basis = 0i128;
    let mut idle_labor_expense = 0i128;
    let mut turnover_tax_basis = 0i128;
    let mut turnover_tax_rounding_remainder = 0i128;

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
                    id: 1,
                    payer: 3,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: money(100)?,
                    cycle: Cycle::Daily,
                    ends_at: DAYS * DAY,
                },
            },
        },
    )?;
    if inputs.debt_principal > 0 {
        command(
            &mut engine,
            Action::Timeline {
                command: TimelineCommand::Lend {
                    id: DEBT_ID,
                    lender: 1,
                    borrower: 3,
                    principal: money(inputs.debt_principal)?,
                    due: inputs.debt_due_day * DAY,
                },
            },
        )?;
    }

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
        if inputs.debt_principal > 0 && day >= inputs.debt_due_day {
            let outstanding = engine
                .timeline()
                .and_then(|timeline| timeline.claims.iter().find(|claim| claim.id == DEBT_ID))
                .map(|claim| claim.principal.value() - claim.paid.value())
                .unwrap_or(0);
            if outstanding > 0 {
                command(
                    &mut engine,
                    Action::Timeline {
                        command: TimelineCommand::SettleClaim { id: DEBT_ID },
                    },
                )?;
            }
        }
        let receipt = {
            let view = engine.world().view();
            engine
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
                .map_err(|error| format!("town_day_{day}:{error:?}"))?
        };
        let summary = receipt
            .town
            .filter(|summary| summary.day == day)
            .ok_or_else(|| format!("missing_day_summary:{day}"))?;
        produced += i128::from(summary.produced.value());
        waste += i128::from(summary.waste.value());
        sales += i128::from(summary.actual_sales.value());
        reference_need += i128::from(summary.reference_need.value());
        market_demand += i128::from(summary.market_demand.value());
        unmet_market_demand += i128::from(summary.unmet_market_demand.value());
        goods_payments += i128::from(summary.goods_payments.value());
        turnover_tax_basis = turnover_tax_basis
            .checked_add(i128::from(summary.goods_payments.value()))
            .ok_or("turnover_tax_basis_overflow")?;
        if inputs.turnover_tax_bps > 0 {
            let numerator = i128::from(summary.goods_payments.value())
                .checked_mul(i128::from(inputs.turnover_tax_bps))
                .and_then(|amount| amount.checked_add(turnover_tax_rounding_remainder))
                .ok_or("turnover_tax_calculation_overflow")?;
            let assessed_cents = numerator / TURNOVER_TAX_BPS_DENOMINATOR;
            turnover_tax_rounding_remainder = numerator % TURNOVER_TAX_BPS_DENOMINATOR;
            if assessed_cents > 0 {
                let amount = i64::try_from(assessed_cents)
                    .map_err(|_| "turnover_tax_amount_out_of_range")?;
                let id = TURNOVER_TAX_CLAIM_ID_BASE
                    .checked_add(day)
                    .ok_or("turnover_tax_claim_id_overflow")?;
                let due = day
                    .checked_add(1)
                    .and_then(|next_day| next_day.checked_mul(DAY))
                    .ok_or("turnover_tax_due_overflow")?;
                command(
                    &mut engine,
                    Action::Timeline {
                        command: TimelineCommand::Claims {
                            first_id: id,
                            count: 1,
                            payer: TURNOVER_TAX_PAYER,
                            payee: TURNOVER_TAX_AUTHORITY,
                            amount: money(amount)?,
                            due,
                        },
                    },
                )?;
            }
        }
        freight += i128::from(summary.freight.value());
        utilities += i128::from(summary.utilities.value());
        disposal += i128::from(summary.disposal.value());
        wages_accrued += i128::from(summary.wages_accrued.value());
        labor_basis += i128::from(summary.labor_basis.value());
        idle_labor_expense += i128::from(summary.idle_labor_expense.value());
    }

    let final_view = engine.world().view();
    let household = final_view
        .accounts
        .iter()
        .find(|account| account.id == 2)
        .ok_or("missing_household")?;
    let producer = final_view
        .accounts
        .iter()
        .find(|account| account.id == 3)
        .ok_or("missing_producer")?;
    let treasury = final_view
        .accounts
        .iter()
        .find(|account| account.id == 1)
        .ok_or("missing_treasury")?;
    let input = final_view
        .stocks
        .iter()
        .find(|stock| stock.id == 2)
        .ok_or("missing_raw")?;
    let output = final_view
        .stocks
        .iter()
        .find(|stock| stock.id == 3)
        .ok_or("missing_product")?;
    let raw_remaining = input.quantity.value();
    let raw_basis_remaining = input.basis.value();
    let product_remaining = output.quantity.value();
    let product_basis_remaining = output.basis.value();
    let final_cash: i128 = final_view
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum();
    let final_cash_by_account: Vec<_> = final_view
        .accounts
        .iter()
        .map(|account| (account.id, account.cash.value()))
        .collect();
    let cash_deltas_by_account: Vec<_> = initial_cash_by_account
        .iter()
        .map(|(id, initial)| {
            let final_balance = final_cash_by_account
                .iter()
                .find(|(final_id, _)| final_id == id)
                .map(|(_, value)| *value)
                .unwrap_or_default();
            (*id, final_balance - initial)
        })
        .collect();
    let raw_consumed = initial_raw_quantity - raw_remaining;
    let output_accounted =
        sales + i128::from(product_remaining) - i128::from(initial_product_quantity);
    let sold_basis_by_conservation = i128::from(initial_raw_basis) + labor_basis
        - i128::from(raw_basis_remaining)
        - i128::from(product_basis_remaining);
    let (loan_principal_paid, debt_outstanding) = engine
        .timeline()
        .and_then(|timeline| timeline.claims.iter().find(|claim| claim.id == DEBT_ID))
        .map(|claim| {
            (
                claim.paid.value(),
                claim.principal.value() - claim.paid.value(),
            )
        })
        .unwrap_or((0, 0));
    let timeline = engine.timeline().ok_or("missing_timeline")?;
    let current_time = timeline.progress.last_complete;
    let tax_claims: Vec<_> = timeline
        .claims
        .iter()
        .filter(|claim| {
            (TURNOVER_TAX_CLAIM_ID_BASE + 1..=TURNOVER_TAX_CLAIM_ID_BASE + DAYS).contains(&claim.id)
        })
        .collect();
    let tax_assessed: i128 = tax_claims
        .iter()
        .map(|claim| i128::from(claim.principal.value()))
        .sum();
    let tax_paid: i128 = tax_claims
        .iter()
        .map(|claim| i128::from(claim.paid.value()))
        .sum();
    let tax_outstanding: i128 = tax_claims
        .iter()
        .map(|claim| i128::from(claim.principal.value() - claim.paid.value()))
        .sum();
    let tax_future_due_claims: Vec<_> = tax_claims
        .iter()
        .filter(|claim| claim.due > current_time && claim.principal != claim.paid)
        .collect();
    let tax_future_due: i128 = tax_future_due_claims
        .iter()
        .map(|claim| i128::from(claim.principal.value() - claim.paid.value()))
        .sum();
    let tax_next_due_day = tax_future_due_claims
        .iter()
        .map(|claim| claim.due.div_ceil(DAY))
        .min()
        .unwrap_or(0);
    let wage_paid = engine
        .timeline()
        .and_then(|timeline| timeline.payroll.iter().find(|payroll| payroll.spec.id == 1))
        .map(|payroll| payroll.paid.value())
        .unwrap_or(0);
    let wage_arrears = engine
        .timeline()
        .and_then(|timeline| timeline.payroll.iter().find(|payroll| payroll.spec.id == 1))
        .map(|payroll| payroll.billed.value() - payroll.paid.value())
        .unwrap_or(0);
    if final_cash != initial_cash
        || i128::from(raw_consumed) != produced + waste
        || produced != output_accounted
        || wages_accrued != labor_basis + idle_labor_expense
        || i128::from(wage_paid + wage_arrears) > wages_accrued
        || debt_outstanding < 0
        || i128::from(loan_principal_paid) > i128::from(inputs.debt_principal)
        || tax_assessed != tax_paid + tax_outstanding
        || tax_future_due > tax_outstanding
        || tax_claims
            .iter()
            .any(|claim| claim.payer != TURNOVER_TAX_PAYER || claim.payee != TURNOVER_TAX_AUTHORITY)
        || (inputs.turnover_tax_bps == 0 && !tax_claims.is_empty())
    {
        return Err("28_day_or_turnover_tax_conservation_failed".into());
    }
    let engine_owned_capacity_bytes = engine.owned_capacity_bytes();
    let world_layout = engine.world().layout();
    println!(
        "scenario={} days={DAYS} engine_owned_capacity_bytes={engine_owned_capacity_bytes} engine_cap_bytes={ENGINE_BYTE_BUDGET} world_layout={world_layout} profile_substitutes=transport_route_and_capacity,utility_availability_and_cost live_transport=false live_utility=false tax_policy=not_configured turnover_tax_fixture={} turnover_tax_bps={} tax_payer_account_id={} tax_authority_account_id={} tax_basis_goods_payments={turnover_tax_basis} tax_assessed={tax_assessed} tax_paid={tax_paid} tax_outstanding={tax_outstanding} tax_future_due={tax_future_due} tax_future_due_claims={} tax_next_due_day={} tax_rounding_remainder_bps={turnover_tax_rounding_remainder} initial_cash_total={initial_cash} final_cash_total={final_cash} cash_initial_by_account={initial_cash_by_account:?} cash_final_by_account={final_cash_by_account:?} cash_delta_by_account={cash_deltas_by_account:?} treasury_cash_initial={initial_treasury_cash} household_cash_final={} producer_cash_final={} treasury_cash_final={} raw_initial={} raw_consumed={raw_consumed} raw_remaining={raw_remaining} raw_basis_initial={initial_raw_basis} raw_basis_final={raw_basis_remaining} product_produced={produced} waste={waste} product_sold={sales} product_remaining={product_remaining} product_basis_initial={initial_product_basis} product_basis_final={product_basis_remaining} sold_basis_by_conservation={sold_basis_by_conservation} reference_need={reference_need} market_demand={market_demand} unmet_market_demand={unmet_market_demand} goods_payments={goods_payments} freight={freight} utilities={utilities} disposal={disposal} wages_accrued={wages_accrued} wages_paid={wage_paid} wages_arrears={wage_arrears} capitalized_labor_basis={labor_basis} idle_labor_expense={idle_labor_expense} debt_principal={} debt_due_day={} debt_paid={} debt_outstanding={debt_outstanding} configured_route_reachable={} configured_route_capacity={} configured_production_utility_cost_per_q={} configured_daily_need={} starting_household_cash={} starting_company_cash={}",
        inputs.scenario.name(),
        if inputs.turnover_tax_bps > 0 { "fixture_turnover" } else { "none" },
        inputs.turnover_tax_bps, TURNOVER_TAX_PAYER, TURNOVER_TAX_AUTHORITY,
        tax_future_due_claims.len(), tax_next_due_day,
        household.cash.value(), producer.cash.value(), treasury.cash.value(), initial_raw_quantity,
        inputs.debt_principal, inputs.debt_due_day, loan_principal_paid, inputs.route_reachable, inputs.route_capacity,
        inputs.production_utility_cost_per_q, inputs.daily_need, inputs.household_cash, inputs.company_cash,
    );
    Ok(())
}
