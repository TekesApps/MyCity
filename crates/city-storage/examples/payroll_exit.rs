use mycity_core::{
    timeline::{Command as T, Cycle, PayrollSpec},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::json;
use std::{env, path::PathBuf};

const RULES: &str = "m0-storage-process-fixture-v1";
fn m(n: i64) -> Money {
    Money::new(n).unwrap()
}
fn envelope(store: &Store, command: T) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: store.committed_sequence() + 1,
        expected_revision: store.engine().unwrap().world().view().revision,
        actions: vec![Action::Timeline { command }],
    }
}
fn execute(store: &mut Store, command: T) -> Result<(), String> {
    let request = envelope(store, command);
    let outcome = store
        .execute(Caller::System, request)
        .map_err(|e| format!("store execute: {e:?}"))?;
    outcome
        .model
        .map_err(|e| format!("Timeline command: {e:?}"))?;
    if !outcome.has_persistent_receipt {
        return Err("command did not receive a persistent receipt".into());
    }
    Ok(())
}
fn drive(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..10_000 {
        execute(store, T::Advance { target, limit: 256 })?;
        if store
            .engine()
            .map_err(|e| format!("store engine: {e:?}"))?
            .timeline()
            .ok_or_else(|| "timeline unavailable".to_string())?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("Timeline did not complete target {target}"))
}
fn cash(store: &Store, id: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.cash.value())
        .ok_or_else(|| format!("account {id} is absent"))
}
fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn run() -> Result<(), String> {
    let mut args = env::args_os();
    let _program = args.next();
    let path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: cargo run -p mycity-storage --example payroll_exit -- <new-world-dir>")?;
    if args.next().is_some() {
        return Err("provide exactly one new-world directory".into());
    }
    if path.exists() {
        return Err(format!(
            "refusing to reuse existing world directory: {}",
            path.display()
        ));
    }

    let mut store = Store::create(
        &path,
        include_bytes!("../../../fixtures/m0-e0-opening.json"),
        vec![],
        RULES,
    )
    .map_err(|e| format!("store create: {e:?}"))?;
    execute(&mut store, T::Enable)?;
    execute(
        &mut store,
        T::AddPayroll {
            spec: PayrollSpec {
                id: 1,
                payer: 3,
                payee: 2,
                home_city: 0,
                hourly_cents: m(7),
                cycle: Cycle::Weekly,
                ends_at: 40 * 86400,
            },
        },
    )?;
    drive(&mut store, 7 * 86400)?;
    drive(&mut store, 7 * 86400 + 10 * 3600)?;

    let before = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "timeline unavailable".to_string())?
        .payroll[0];
    require(
        before.accrued == m(399) && before.billed == m(392) && before.paid == m(392),
        "expected one settled weekly wage cycle and 7 cents of new accrued wages",
    )?;
    let payer_cash = cash(&store, 3)?;
    let payee_cash = cash(&store, 2)?;
    let end = envelope(&store, T::EndForExit { id: 1 });
    let end_result = store
        .execute(Caller::System, end.clone())
        .map_err(|e| format!("EndForExit: {e:?}"))?;
    end_result
        .model
        .map_err(|e| format!("EndForExit model: {e:?}"))?;
    require(
        end_result.has_persistent_receipt,
        "EndForExit receipt missing",
    )?;
    require(
        cash(&store, 3)? == payer_cash,
        "EndForExit changed payer cash",
    )?;
    require(
        cash(&store, 2)? == payee_cash,
        "EndForExit changed payee cash",
    )?;

    store
        .checkpoint()
        .map_err(|e| format!("checkpoint: {e:?}"))?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(|e| format!("cold open: {e:?}"))?;
    let replay_before = store
        .state_hash()
        .map_err(|e| format!("state hash: {e:?}"))?;
    let replay = store
        .execute(Caller::System, end)
        .map_err(|e| format!("EndForExit replay: {e:?}"))?;
    replay
        .model
        .map_err(|e| format!("EndForExit replay model: {e:?}"))?;
    require(replay.has_persistent_receipt, "replayed receipt missing")?;
    require(
        store
            .state_hash()
            .map_err(|e| format!("state hash: {e:?}"))?
            == replay_before,
        "replayed EndForExit changed durable state",
    )?;

    execute(&mut store, T::SettlePayroll { id: 1 })?;
    let settled = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "timeline unavailable".to_string())?
        .payroll[0];
    require(
        settled.accrued == m(399) && settled.billed == m(399) && settled.paid == m(399),
        "explicit settlement did not pay the 7-cent arrear exactly once",
    )?;
    require(
        cash(&store, 3)? == payer_cash - 7,
        "payer settlement delta mismatch",
    )?;
    require(
        cash(&store, 2)? == payee_cash + 7,
        "payee settlement delta mismatch",
    )?;
    require(
        cash(&store, 3)? + cash(&store, 2)? == payer_cash + payee_cash,
        "two-party settlement did not conserve cash",
    )?;
    println!(
        "{}",
        json!({
            "scenario": "timeline-payroll-end-for-exit",
            "actual_store_checkpoint_and_reopen": true,
            "original_end_operation_replayed": true,
            "accrued_cents": settled.accrued.value(),
            "billed_cents": settled.billed.value(),
            "paid_cents": settled.paid.value(),
            "settled_arrears_cents": 7,
            "payer_cash_delta_at_end_for_exit": 0,
            "legal_company_closure": false
        })
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
