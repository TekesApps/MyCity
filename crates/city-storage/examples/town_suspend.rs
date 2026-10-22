//! Finite storage-backed stop-operations demonstration; this is not legal company closure.
use mycity_core::{
    timeline::{Command as T, Cycle, DAY, PayrollSpec},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::Value;
use std::{env, error::Error, fs, path::Path};

const RULES: &str = "town-suspend-operations-source-fixture-v1";

fn opening() -> Result<Vec<u8>, Box<dyn Error>> {
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
    let path = env::args_os().nth(1).ok_or_else(|| {
        std::io::Error::other(
            "usage: cargo run -p mycity-storage --example town_suspend -- <new-world-dir>",
        )
    })?;
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
    let mut store = Store::create(&path, &opening()?, vec![], RULES)?;
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
        if store.engine()?.timeline()?.progress.target.is_none() {
            break;
        }
    }
    execute(&mut store, Action::TownSettle { profile_id: 1 })?;
    execute(
        &mut store,
        Action::TownSuspendOperations {
            profile_id: 1,
            company_account: 3,
        },
    )?;
    store.checkpoint()?;
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
        if store.engine()?.timeline()?.progress.target.is_none() {
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
    if settled.state.last_summary.actual_sales == mycity_core::units::Quantity::ZERO
        || suspended_stock_after != suspended_stock
        || other_shop_stock_after >= other_shop_stock
        || suspended_cash_after > suspended_cash
    {
        return Err(
            std::io::Error::other("day-2 suspension/continuing-shop evidence failed").into(),
        );
    }
    println!(
        "stopped_account=3 suspended_at={} settled_day={} suspended_shop_stock_unchanged=true suspended_account_cash_no_gain=true other_shop_stock_decreased=true other_shop_actual_sales={} legal_company_close=false",
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
