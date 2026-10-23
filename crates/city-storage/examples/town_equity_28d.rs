//! 28-day Store-backed equity/tax business case with a household cash-pressure variant.
#[path = "support/town_equity_28d.rs"]
mod scenario;

use mycity_storage::Store;
use serde_json::json;
use std::{env, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("town_equity_28d_failed={error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let first = args
        .next()
        .ok_or("usage: town_equity_28d NEW_STORE_DIR [base|household-cash-shortage] | --verify-open EXISTING_WORKING_COPY")?;
    if first == "--verify-open" {
        let path = PathBuf::from(
            args.next()
                .ok_or("usage: --verify-open EXISTING_WORKING_COPY")?,
        );
        if args.next().is_some() {
            return Err("unexpected_argument".into());
        }
        return verify_open(&path);
    }

    let world = PathBuf::from(first);
    let scenario = scenario::Scenario::parse(
        &args
            .next()
            .unwrap_or_else(|| "base".into())
            .to_string_lossy(),
    )?;
    if args.next().is_some() {
        return Err("unexpected_argument".into());
    }
    let report = scenario::run(&world, scenario)?;
    println!("TOWN_EQUITY_28D_RESULT={report}");
    Ok(())
}

fn verify_open(path: &std::path::Path) -> Result<(), String> {
    let store = Store::open(path, scenario::RULES).map_err(|error| format!("{error:?}"))?;
    let engine = store.engine().map_err(|error| format!("{error:?}"))?;
    let view = engine.world().view();
    let accounts = view
        .accounts
        .iter()
        .map(|account| {
            json!({
                "id": account.id,
                "cash_cents": account.cash.value(),
                "protected_cents": account.protected.value()
            })
        })
        .collect::<Vec<_>>();
    let (settled_day, tax_cursors, equity_registers, exit) = match view.town {
        Some(town) => (
            Some(town.state.settled_day),
            town.state
                .tax_cursors
                .as_slice()
                .iter()
                .map(|cursor| {
                    json!({
                        "company": cursor.company,
                        "policy_id": cursor.policy_id,
                        "through_day": cursor.through_day,
                        "last_operation_id": cursor.last_operation_id,
                        "last_claim_id": cursor.last_claim_id,
                        "cumulative_assessed_cents": cursor.cumulative_assessed.value(),
                        "retained_earnings_cents": cursor.retained_earnings_cents
                    })
                })
                .collect::<Vec<_>>(),
            town.state
                .equity_registers
                .as_slice()
                .iter()
                .map(|register| {
                    json!({
                        "company": register.company,
                        "issued_shares": register.issued_shares,
                        "opening_issued_shares": register.opening_issued_shares,
                        "opening_paid_in_cents": register.opening_paid_in.value(),
                        "dividends_distributed_cents": register.cumulative_dividends_distributed.value(),
                        "capital_returned_cents": register.cumulative_capital_returned.value(),
                        "liquidation_surplus_distributed_cents": register.cumulative_liquidation_surplus_distributed.value()
                    })
                })
                .collect::<Vec<_>>(),
            town.state.exit.map(|state| {
                json!({
                    "policy_id": state.policy_id,
                    "company_account": state.company_account,
                    "started_at": state.started_at,
                    "phase": format!("{:?}", state.phase)
                })
            }),
        ),
        None => (None, Vec::new(), Vec::new(), None),
    };
    let report = json!({
        "mode": "verify_open_only",
        "store_open_may_recover_or_write": true,
        "business_commands_executed": 0,
        "rules": scenario::RULES,
        "committed_sequence": store.committed_sequence(),
        "state_hash": store.state_hash().map_err(|error| format!("{error:?}"))?,
        "store_owned_capacity_bytes": store.owned_capacity_bytes().map_err(|error| format!("{error:?}"))?,
        "engine_owned_capacity_bytes": engine.owned_capacity_bytes(),
        "world_revision": view.revision,
        "accounts": accounts,
        "town_settled_day": settled_day,
        "tax_cursors": tax_cursors,
        "equity_registers": equity_registers,
        "exit": exit
    });
    println!("TOWN_EQUITY_28D_VERIFY={report}");
    Ok(())
}
