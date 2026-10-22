//! Finite storage-backed stop-operations demonstration; this is not legal company closure.
use mycity_core::{
    timeline::{Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::Value;
use std::{env, error::Error, fs, path::Path};

const RULES: &str = "town-suspend-operations-source-fixture-v1";
const LOSS_UTILITY_COST_PER_OUTPUT_Q: i64 = 10_000;

#[derive(Clone, Copy, Debug)]
struct EquityProjection {
    total: i128,
    cash: i128,
    inventory_basis: i128,
    payroll_receivable: i128,
    payroll_payable: i128,
    claims_receivable: i128,
    claims_payable: i128,
}

fn equity_projection(store: &Store, account: u32) -> Result<EquityProjection, Box<dyn Error>> {
    let view = store.engine()?.world().view();
    let timeline = timeline(store)?;
    let cash = i128::from(
        view.accounts
            .iter()
            .find(|row| row.id == account)
            .ok_or_else(|| std::io::Error::other("company account"))?
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
    Ok(EquityProjection {
        total: cash + inventory_basis + payroll_receivable + claims_receivable
            - payroll_payable
            - claims_payable,
        cash,
        inventory_basis,
        payroll_receivable,
        payroll_payable,
        claims_receivable,
        claims_payable,
    })
}

fn total_cash(store: &Store) -> Result<i128, Box<dyn Error>> {
    Ok(store
        .engine()?
        .world()
        .view()
        .accounts
        .iter()
        .map(|row| i128::from(row.cash.value()))
        .sum())
}

fn timeline(store: &Store) -> Result<&mycity_core::timeline::Timeline, Box<dyn Error>> {
    store
        .engine()?
        .timeline()
        .ok_or_else(|| std::io::Error::other("timeline state unavailable").into())
}

fn opening(loss_mode: bool) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))?;
    value["sources"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("sources"))?
        .extend([
            serde_json::json!({"id":6,"kind":"inventory_capital","amount":100000}),
            serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
            serde_json::json!({"id":8,"kind":"cash_capital","amount":200000}),
        ]);
    value["accounts"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("accounts"))?
        .push(serde_json::json!({"id":5,"role":"household","cash":200000,"source":8}));
    value["stocks"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("stocks"))?
        .extend([
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
    value["town"]["recipes"][0]["utility_cost_per_output_q"] = serde_json::json!(if loss_mode {
        LOSS_UTILITY_COST_PER_OUTPUT_Q
    } else {
        1
    });
    Ok(serde_json::to_vec(&value)?)
}

fn execute(store: &mut Store, action: Action) -> Result<(), Box<dyn Error>> {
    let request = {
        let view = store.engine()?.world().view();
        Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: store.committed_sequence() + 1,
            expected_revision: view.revision,
            actions: vec![action],
        }
    };
    let outcome = store.execute(Caller::System, request)?;
    if let Err(error) = outcome.model {
        return Err(format!("model action failed: {error:?}").into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let path = args.next().ok_or_else(|| {
        std::io::Error::other(
            "usage: cargo run -p mycity-storage --example town_suspend -- <new-world-dir> [--loss]",
        )
    })?;
    let mode = args.next();
    let loss_mode = match mode.as_deref() {
        None => false,
        Some(value) if value == "--loss" => true,
        Some(_) => return Err(std::io::Error::other("only optional mode is --loss").into()),
    };
    if args.next().is_some() {
        return Err(std::io::Error::other("too many arguments").into());
    }
    if Path::new(&path).exists() {
        return Err(std::io::Error::other(
            "target path already exists; choose a new world directory",
        )
        .into());
    }
    fs::create_dir_all(
        Path::new(&path)
            .parent()
            .ok_or_else(|| std::io::Error::other("world path parent"))?,
    )?;
    let mut store = Store::create(&path, &opening(loss_mode)?, vec![], RULES)?;
    execute(&mut store, Action::Timeline { command: T::Enable })?;
    for (id, payer) in [(1, 3), (2, 4)] {
        execute(
            &mut store,
            Action::Timeline {
                command: T::AddPayroll {
                    spec: PayrollSpec {
                        id,
                        payer,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: Money::new(100)?,
                        cycle: Cycle::Daily,
                        ends_at: 7 * DAY,
                    },
                },
            },
        )?;
    }
    execute(
        &mut store,
        Action::Timeline {
            command: T::Claims {
                first_id: 1,
                count: 1,
                payer: 3,
                payee: 4,
                amount: Money::new(1_000)?,
                due: 3 * DAY,
            },
        },
    )?;
    let initial_equity = equity_projection(&store, 3)?;
    let fixture_claim = timeline(&store)?.claims[0];
    if fixture_claim.payer != 3 || fixture_claim.payee != 4 || fixture_claim.paid != Money::ZERO {
        return Err(std::io::Error::other("fixture payable projection is invalid").into());
    }
    for _ in 0..100 {
        execute(
            &mut store,
            Action::Timeline {
                command: T::Advance {
                    target: DAY,
                    limit: 8192,
                },
            },
        )?;
        if timeline(&store)?.progress.target.is_none() {
            break;
        }
    }
    execute(&mut store, Action::TownSettle { profile_id: 1 })?;
    let before_stop_equity = equity_projection(&store, 3)?;
    let before_stop_world = store.engine()?.world().view();
    let before_stop_summary = before_stop_world
        .town
        .as_ref()
        .ok_or_else(|| std::io::Error::other("town state"))?
        .state
        .last_summary;
    if loss_mode {
        if before_stop_equity.total >= initial_equity.total {
            return Err(std::io::Error::other(
                "loss fixture did not produce a measured equity loss",
            )
            .into());
        }
        if timeline(&store)?.claims[0] != fixture_claim {
            return Err(
                std::io::Error::other("loss fixture claim changed before suspension").into(),
            );
        }
    }
    execute(
        &mut store,
        Action::TownSuspendOperations {
            profile_id: 1,
            company_account: 3,
        },
    )?;
    store.checkpoint()?;
    let equity_at_stop = equity_projection(&store, 3)?;
    let claim_at_stop = timeline(&store)?.claims[0];
    if claim_at_stop != fixture_claim {
        return Err(std::io::Error::other("suspension changed the existing claim").into());
    }
    let day2_cash_before = total_cash(&store)?;
    let stopped_work_seconds = timeline(&store)?
        .payroll
        .iter()
        .find(|row| row.spec.payer == 3)
        .ok_or_else(|| std::io::Error::other("suspended payroll"))?
        .work_seconds;
    let day1 = store.engine()?.world().view();
    let state = day1
        .town
        .as_ref()
        .ok_or_else(|| std::io::Error::other("town state"))?
        .state;
    let suspended_stock = day1
        .stocks
        .iter()
        .find(|row| row.id == 3)
        .ok_or_else(|| std::io::Error::other("suspended shop stock"))?
        .quantity;
    let suspended_raw_stock = day1
        .stocks
        .iter()
        .find(|row| row.id == 2)
        .ok_or_else(|| std::io::Error::other("suspended raw stock"))?
        .quantity;
    let other_shop_stock = day1
        .stocks
        .iter()
        .find(|row| row.id == 1)
        .ok_or_else(|| std::io::Error::other("other shop stock"))?
        .quantity;
    let suspended_cash = day1
        .accounts
        .iter()
        .find(|row| row.id == 3)
        .ok_or_else(|| std::io::Error::other("suspended company account"))?
        .cash;
    for _ in 0..100 {
        execute(
            &mut store,
            Action::Timeline {
                command: T::Advance {
                    target: 2 * DAY,
                    limit: 8192,
                },
            },
        )?;
        if timeline(&store)?.progress.target.is_none() {
            break;
        }
    }
    execute(&mut store, Action::TownSettle { profile_id: 1 })?;
    let view = store.engine()?.world().view();
    let settled = view
        .town
        .as_ref()
        .ok_or_else(|| std::io::Error::other("town state"))?;
    let suspended_stock_after = view
        .stocks
        .iter()
        .find(|row| row.id == 3)
        .ok_or_else(|| std::io::Error::other("suspended shop stock"))?
        .quantity;
    let other_shop_stock_after = view
        .stocks
        .iter()
        .find(|row| row.id == 1)
        .ok_or_else(|| std::io::Error::other("other shop stock"))?
        .quantity;
    let suspended_cash_after = view
        .accounts
        .iter()
        .find(|row| row.id == 3)
        .ok_or_else(|| std::io::Error::other("suspended company account"))?
        .cash;
    let suspended_raw_stock_after = view
        .stocks
        .iter()
        .find(|row| row.id == 2)
        .ok_or_else(|| std::io::Error::other("suspended raw stock"))?
        .quantity;
    if settled.state.last_summary.actual_sales == mycity_core::units::Quantity::ZERO
        || settled.state.last_summary.produced != mycity_core::units::Quantity::ZERO
        || suspended_stock_after != suspended_stock
        || suspended_raw_stock_after != suspended_raw_stock
        || other_shop_stock_after >= other_shop_stock
        || suspended_cash_after != suspended_cash
    {
        return Err(
            std::io::Error::other("day-2 suspension/continuing-shop evidence failed").into(),
        );
    }
    let next_day_equity = equity_projection(&store, 3)?;
    let day2_cash_after = total_cash(&store)?;
    let stopped_work_seconds_after = timeline(&store)?
        .payroll
        .iter()
        .find(|row| row.spec.payer == 3)
        .ok_or_else(|| std::io::Error::other("suspended payroll"))?
        .work_seconds;
    if next_day_equity.total != equity_at_stop.total
        || stopped_work_seconds_after != stopped_work_seconds
        || day2_cash_after != day2_cash_before
        || timeline(&store)?.claims[0] != claim_at_stop
    {
        return Err(std::io::Error::other(
            "post-suspension equity, labor, or cash conservation failed",
        )
        .into());
    }
    if loss_mode
        && (before_stop_summary.produced != mycity_core::units::Quantity::ZERO
            || before_stop_summary.wages_accrued == Money::ZERO
            || before_stop_summary.idle_labor_expense == Money::ZERO
            || suspended_stock != mycity_core::units::Quantity::ZERO)
    {
        return Err(std::io::Error::other("loss mode did not suppress production").into());
    }
    println!(
        "mode={} utility_cost_per_output_q={} equity_initial={} equity_before_stop={} equity_delta_before_stop={} equity_next_day={} equity_next_day_delta={} cash_before_stop={} inventory_basis_before_stop={} payroll_receivable_before_stop={} payroll_payable_before_stop={} claims_receivable_before_stop={} claims_payable_before_stop={} existing_claim_payer={} existing_claim_payee={} existing_claim_principal={} existing_claim_paid={} day1_produced={} day1_sales={} day1_utilities={} day1_wages_accrued={} day1_idle_labor_expense={} stopped_account=3 suspended_at={} settled_day={} suspended_shop_stock_unchanged=true suspended_account_cash_no_gain=true other_shop_stock_decreased=true other_shop_actual_sales={} city_cash_conserved=true legal_company_close=false",
        if loss_mode { "loss" } else { "ordinary" },
        if loss_mode { LOSS_UTILITY_COST_PER_OUTPUT_Q } else { 1 },
        initial_equity.total,
        before_stop_equity.total,
        before_stop_equity.total - initial_equity.total,
        next_day_equity.total,
        next_day_equity.total - equity_at_stop.total,
        before_stop_equity.cash,
        before_stop_equity.inventory_basis,
        before_stop_equity.payroll_receivable,
        before_stop_equity.payroll_payable,
        before_stop_equity.claims_receivable,
        before_stop_equity.claims_payable,
        claim_at_stop.payer,
        claim_at_stop.payee,
        claim_at_stop.principal,
        claim_at_stop.paid,
        before_stop_summary.produced,
        before_stop_summary.actual_sales,
        before_stop_summary.utilities,
        before_stop_summary.wages_accrued,
        before_stop_summary.idle_labor_expense,
        state.suspended_operators.as_slice()[0].suspended_at,
        settled.state.settled_day,
        settled.state.last_summary.actual_sales,
    );
    println!(
        "world={} persistent_sequence={}",
        path.to_string_lossy(),
        store.committed_sequence()
    );
    Ok(())
}
