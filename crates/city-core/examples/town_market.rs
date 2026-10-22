//! Execute a real 28-day Engine market loop. The route and daily utility limits are
//! fixed test-profile inputs; this example does not claim the traffic adapter is live.
use mycity_core::{
    assembly::assemble,
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Engine, Envelope, ENGINE_BYTE_BUDGET},
    units::{Money, Quantity},
};
use serde_json::Value;

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const DAYS: u64 = 28;

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

fn main() -> Result<(), String> {
    let mut opening: Value = serde_json::from_slice(OPENING).map_err(|e| e.to_string())?;
    opening["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":100000}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    opening["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":100000,"basis":100000,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    opening["town"] = serde_json::json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{
            "recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,
            "input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,
            "waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,
            "utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,
            "utility_account":1,"freight_account":1,"disposal_account":1
        }],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,
            "route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,
            "freight_per_q":1,"utility_surcharge_per_q":1}]
    });
    let world = assemble(&serde_json::to_vec(&opening).map_err(|e| e.to_string())?)?;
    let mut engine = Engine::new(world, Vec::new()).map_err(|e| format!("engine_init:{e:?}"))?;
    if engine.owned_capacity_bytes() > ENGINE_BYTE_BUDGET {
        return Err("engine_memory_budget_exceeded".into());
    }
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
    let initial_raw = initial.stocks.iter().find(|stock| stock.id == 2).unwrap();
    let initial_product = initial.stocks.iter().find(|stock| stock.id == 3).unwrap();
    let initial_raw_quantity = initial_raw.quantity.value();
    let initial_raw_basis = initial_raw.basis.value();
    let initial_product_quantity = initial_product.quantity.value();
    let initial_product_basis = initial_product.basis.value();
    drop(initial);
    let mut produced = 0i128;
    let mut waste = 0i128;
    let mut sales = 0i128;
    let mut goods_payments = 0i128;
    let mut freight = 0i128;
    let mut utilities = 0i128;
    let mut disposal = 0i128;
    let mut wages_accrued = 0i128;
    let mut labor_basis = 0i128;
    let mut idle_labor_expense = 0i128;
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
                    hourly_cents: Money::new(100).map_err(|e| format!("money:{e:?}"))?,
                    cycle: Cycle::Daily,
                    ends_at: DAYS * DAY,
                },
            },
        },
    )?;

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
                .is_some_and(|t| t.progress.target.is_none())
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
                .map_err(|e| format!("town_day_{day}:{e:?}"))?
        };
        let summary = receipt
            .town
            .filter(|summary| summary.day == day)
            .ok_or_else(|| format!("missing_day_summary:{day}"))?;
        produced += i128::from(summary.produced.value());
        waste += i128::from(summary.waste.value());
        sales += i128::from(summary.actual_sales.value());
        goods_payments += i128::from(summary.goods_payments.value());
        freight += i128::from(summary.freight.value());
        utilities += i128::from(summary.utilities.value());
        disposal += i128::from(summary.disposal.value());
        wages_accrued += i128::from(summary.wages_accrued.value());
        labor_basis += i128::from(summary.labor_basis.value());
        idle_labor_expense += i128::from(summary.idle_labor_expense.value());
    }

    let final_view = engine.world().view();
    let household = final_view.accounts.iter().find(|a| a.id == 2).unwrap();
    let producer = final_view.accounts.iter().find(|a| a.id == 3).unwrap();
    let input = final_view.stocks.iter().find(|s| s.id == 2).unwrap();
    let output = final_view.stocks.iter().find(|s| s.id == 3).unwrap();
    if household.cash <= Money::ZERO || producer.cash <= Money::ZERO {
        return Err("cash_not_conserved".into());
    }
    if input.quantity >= Quantity::new(100000).unwrap() || output.quantity == Quantity::ZERO {
        return Err("production_or_sales_not_applied".into());
    }
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
    let raw_consumed = initial_raw_quantity - input.quantity.value();
    let output_accounted =
        sales + i128::from(output.quantity.value()) - i128::from(initial_product_quantity);
    if final_cash != initial_cash
        || i128::from(raw_consumed) != produced + waste
        || produced != output_accounted
        || wages_accrued != labor_basis + idle_labor_expense
    {
        return Err("28_day_conservation_failed".into());
    }
    let sold_basis_by_conservation = i128::from(initial_raw_basis) + labor_basis
        - i128::from(input.basis.value())
        - i128::from(output.basis.value());
    let engine_owned_capacity_bytes = engine.owned_capacity_bytes();
    let world_layout = engine.world().layout();
    println!(
        "days={DAYS} engine_owned_capacity_bytes={engine_owned_capacity_bytes} engine_cap_bytes={ENGINE_BYTE_BUDGET} world_layout={world_layout} cash_total_initial={initial_cash} cash_total_final={final_cash} cash_initial_by_account={initial_cash_by_account:?} cash_final_by_account={final_cash_by_account:?} cash_delta_by_account={cash_deltas_by_account:?} household_cash={} producer_cash={} raw_consumed={raw_consumed} raw_remaining={} raw_basis_initial={initial_raw_basis} raw_basis_final={} product_produced={produced} waste={waste} product_sold={sales} product_remaining={} product_basis_initial={initial_product_basis} product_basis_final={} sold_basis_by_conservation={sold_basis_by_conservation} goods_payments={goods_payments} freight={freight} utilities={utilities} disposal={disposal} wages_accrued={wages_accrued} capitalized_labor_basis={labor_basis} idle_labor_expense={idle_labor_expense}",
        household.cash.value(),
        producer.cash.value(),
        input.quantity.value(),
        input.basis.value(),
        output.quantity.value(),
        output.basis.value(),
    );
    println!(
        "final_day_summary={:?}",
        final_view.town.map(|town| town.state.last_summary)
    );
    Ok(())
}
