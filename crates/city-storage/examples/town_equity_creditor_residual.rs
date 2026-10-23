use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope, JournalRecord},
    units::{Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

const RULES: &str = "town-equity-runtime-store-v1";

fn m(value: i64) -> Money {
    Money::new(value).unwrap()
}

fn opening() -> Result<Vec<u8>, String> {
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
            .map_err(|error| error.to_string())?;
    value["sources"]
        .as_array_mut()
        .ok_or("opening sources")?
        .extend([
            json!({"id":6,"kind":"inventory_capital","amount":100}),
            json!({"id":7,"kind":"inventory_capital","amount":0}),
        ]);
    value["stocks"]
        .as_array_mut()
        .ok_or("opening stocks")?
        .extend([
            json!({"id":2,"owner":3,"commodity":"raw","quantity":10,"basis":100,"source":6}),
            json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
        ]);
    value["town"] = json!({
        "schema_version":1,"id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"freight_account":1,"utility_account":1,"disposal_account":1}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters":[{
            "company":3,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":{"kind":"person","id":7001},
            "investors":[{"party":{"kind":"person","id":7001},"account":2,"opening_shares":6001},{"party":{"kind":"treasury","id":1},"account":1,"opening_shares":0}],
            "opening_sources":[{"source_id":3,"party":{"kind":"person","id":7001}},{"source_id":6,"party":{"kind":"person","id":7001}},{"source_id":7,"party":{"kind":"person","id":7001}}]
        }],
        "tax_rules":[{"company":3,"policy_id":77,"kind":"daily_realized_market_margin","authority_account":1,"rate_basis_points":5000}],
        "exit_policy":{
            "policy_id":1,"company_account":3,"requester":2,"executor":1,"legal_clearance":"cleared",
            "priority_policy":{"order":["tax","wage","secured_debt","unsecured_debt","other"],"within_tier":"pro_rata_by_outstanding_stable_id"},
            "claims_complete":true,"tax_assessment_complete":true,"asset_register_complete":true,"employment_exit_terms_complete":true,
            "claims":[{"id":100,"payee":4,"class":"unsecured_debt"}],"payroll":[{"id":1,"payee":2}]
        }
    });
    serde_json::to_vec(&value).map_err(|error| error.to_string())
}

fn request(store: &Store, action: Action) -> Result<Envelope, String> {
    let view = store.engine().map_err(err)?.world().view();
    Ok(Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    })
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> Result<u64, String> {
    let envelope = request(store, action)?;
    let id = envelope.operation_id;
    let outcome = store.execute(caller, envelope).map_err(err)?;
    if outcome.model.is_err() || !outcome.has_persistent_receipt {
        return Err(format!("operation {id} rejected: {:?}", outcome.model));
    }
    Ok(id)
}

fn lose_applied(store: &mut Store, caller: Caller, envelope: Envelope) -> Result<(), String> {
    let mut applied = false;
    let result = store.execute_with_observer(caller, envelope, |point| {
        if point == WritePoint::Applied {
            applied = true;
            Err(std::io::Error::other("injected CLI reply loss"))
        } else {
            Ok(())
        }
    });
    if !applied || !matches!(result, Err(mycity_storage::Error::Io(_))) {
        return Err(format!("expected applied response loss, got {result:?}"));
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
            .ok_or("timeline missing")?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("timeline did not reach {target}"))
}

fn settle(store: &mut Store) -> Result<mycity_core::town_runtime::MarketSummary, String> {
    execute(store, Caller::System, Action::TownSettle { profile_id: 1 })?;
    Ok(store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("town missing")?
        .state
        .last_summary)
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
        .ok_or_else(|| format!("missing account {account}"))
}

fn replay_exact(store: &mut Store, record: JournalRecord) -> Result<(), String> {
    if record.outcome.is_err() {
        return Err("cannot replay a failed receipt as a success".to_owned());
    }
    let view = store.engine().map_err(err)?.world().view();
    let caller = if record.actor == 0 {
        Caller::System
    } else {
        Caller::Account(record.actor)
    };
    let outcome = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: record.operation_id,
                expected_revision: record.expected_revision,
                actions: record.actions.into_iter().flatten().collect(),
            },
        )
        .map_err(err)?;
    if outcome.model != record.outcome || !outcome.has_persistent_receipt {
        return Err("exact replay changed the durable operation receipt".to_owned());
    }
    Ok(())
}

fn err(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: cargo run --locked --release -p mycity-storage --example town_equity_creditor_residual -- NEW_WORLD_DIR")?;
    if args.next().is_some() || path.exists() {
        return Err("expected one new world directory path".to_owned());
    }
    let parent = path.parent().ok_or("world path needs a parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut store = Store::create(&path, &opening()?, vec![], RULES).map_err(err)?;
    let initial_store_owned_bytes = store.owned_capacity_bytes().map_err(err)?;
    let initial_engine_owned_bytes = store.engine().map_err(err)?.owned_capacity_bytes();
    let initial_cash = [
        cash(&store, 1)?,
        cash(&store, 2)?,
        cash(&store, 3)?,
        cash(&store, 4)?,
    ];

    execute(
        &mut store,
        Caller::Account(1),
        Action::EquityContribute {
            company: 3,
            amount: m(10_000),
        },
    )?;
    execute(
        &mut store,
        Caller::Account(2),
        Action::EquityContribute {
            company: 3,
            amount: m(10_000),
        },
    )?;
    let enable_id = execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    let payroll_id = execute(
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

    advance_to(&mut store, DAY)?;
    let day1 = settle(&mut store)?;
    if day1.actual_sales != Quantity::new(10).map_err(err)? {
        return Err(format!("unexpected day-1 sales: {:?}", day1.actual_sales));
    }
    for day in 2..=4 {
        advance_to(&mut store, day * DAY)?;
        settle(&mut store)?;
    }
    if cash(&store, 3)? != 620_145 {
        return Err(format!("unexpected operating cash: {}", cash(&store, 3)?));
    }

    let lend_id = execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Lend {
                id: 100,
                lender: 4,
                borrower: 3,
                principal: m(1_000),
                due: 5 * DAY,
            },
        },
    )?;
    let begin_exit_id = execute(
        &mut store,
        Caller::Account(2),
        Action::BeginExit {
            company: 3,
            policy_id: 1,
        },
    )?;
    let early = request(&store, Action::SettleExit { company: 3 })?;
    let early_id = early.operation_id;
    lose_applied(&mut store, Caller::Account(1), early)?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(err)?;
    let early_record = store
        .receipt(early_id)
        .map_err(err)?
        .ok_or("early exit receipt missing")?;
    if early_record.outcome.is_err() || cash(&store, 3)? != 621_145 || cash(&store, 2)? != 189_780 {
        return Err("future creditor did not block shareholder payout".to_owned());
    }
    replay_exact(&mut store, early_record)?;

    advance_to(&mut store, 5 * DAY)?;
    let day5_settle = request(&store, Action::TownSettle { profile_id: 1 })?;
    let day5_id = day5_settle.operation_id;
    lose_applied(&mut store, Caller::System, day5_settle)?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(err)?;
    let day5_record = store
        .receipt(day5_id)
        .map_err(err)?
        .ok_or("day-5 settlement receipt missing")?;
    if day5_record.outcome.is_err()
        || day5_record
            .outcome
            .as_ref()
            .unwrap()
            .town
            .unwrap()
            .actual_sales
            != Quantity::ZERO
    {
        return Err("day-5 settlement failed or produced unexpected sales".to_owned());
    }
    replay_exact(&mut store, day5_record)?;
    let loan = store
        .engine()
        .map_err(err)?
        .timeline()
        .ok_or("timeline missing")?
        .claims
        .iter()
        .find(|claim| claim.id == 100)
        .copied()
        .ok_or("loan claim missing")?;
    if loan.paid != Money::ZERO
        || !loan.processed
        || cash(&store, 4)? != initial_cash[3] - 1_000
        || cash(&store, 3)? != 621_145
    {
        return Err("day-5 settlement bypassed ranked exit creditor payment".to_owned());
    }

    let exit = request(&store, Action::SettleExit { company: 3 })?;
    let exit_id = exit.operation_id;
    lose_applied(&mut store, Caller::Account(1), exit)?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(err)?;
    let exit_record = store
        .receipt(exit_id)
        .map_err(err)?
        .ok_or("exit receipt missing")?;
    if exit_record.outcome.is_err()
        || exit_record.outcome.as_ref().unwrap().settled[0] != Some(m(621_145))
    {
        return Err(
            "ranked exit did not pay the creditor and distribute the expected residual".to_owned(),
        );
    }
    replay_exact(&mut store, exit_record)?;
    let paid_loan = store
        .engine()
        .map_err(err)?
        .timeline()
        .ok_or("timeline missing")?
        .claims
        .iter()
        .find(|claim| claim.id == 100)
        .copied()
        .ok_or("loan claim missing after exit")?;
    if paid_loan.paid != m(1_000) || !paid_loan.processed {
        return Err("SettleExit did not persist the ranked creditor payment".to_owned());
    }
    let register = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("town missing")?
        .state
        .equity_registers
        .as_slice()[0];
    if register.cumulative_capital_returned != m(620_100)
        || register.cumulative_liquidation_surplus_distributed != m(45)
    {
        return Err("capital/surplus classification differs from independent oracle".to_owned());
    }
    let close = request(&store, Action::CloseExit { company: 3 })?;
    let close_id = close.operation_id;
    let closed = store.execute(Caller::Account(1), close).map_err(err)?;
    if closed.model.is_err() {
        return Err(format!("close rejected: {:?}", closed.model));
    }
    let hash = store.state_hash().map_err(err)?;
    store.checkpoint().map_err(err)?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(err)?;
    if store.state_hash().map_err(err)? != hash {
        return Err("checkpoint reopen hash mismatch".to_owned());
    }
    let close_record = store
        .receipt(close_id)
        .map_err(err)?
        .ok_or("close receipt missing")?;
    replay_exact(&mut store, close_record)?;
    let exit_record = store
        .receipt(exit_id)
        .map_err(err)?
        .ok_or("exit receipt missing")?;
    replay_exact(&mut store, exit_record)?;
    let day5_record = store
        .receipt(day5_id)
        .map_err(err)?
        .ok_or("day-5 receipt missing")?;
    replay_exact(&mut store, day5_record)?;
    let early_record = store
        .receipt(early_id)
        .map_err(err)?
        .ok_or("early receipt missing")?;
    replay_exact(&mut store, early_record)?;
    let begin_exit_record = store
        .receipt(begin_exit_id)
        .map_err(err)?
        .ok_or("begin-exit receipt missing")?;
    replay_exact(&mut store, begin_exit_record)?;
    let lend_record = store
        .receipt(lend_id)
        .map_err(err)?
        .ok_or("loan receipt missing")?;
    replay_exact(&mut store, lend_record)?;
    let payroll_record = store
        .receipt(payroll_id)
        .map_err(err)?
        .ok_or("payroll receipt missing")?;
    replay_exact(&mut store, payroll_record)?;
    let enable_record = store
        .receipt(enable_id)
        .map_err(err)?
        .ok_or("timeline enable receipt missing")?;
    replay_exact(&mut store, enable_record)?;
    if store.state_hash().map_err(err)? != hash {
        return Err("receipt replay changed the cold checkpoint state".to_owned());
    }

    let final_store_owned_bytes = store.owned_capacity_bytes().map_err(err)?;
    let final_engine_owned_bytes = store.engine().map_err(err)?.owned_capacity_bytes();
    println!("{{\"scenario\":\"future_creditor_then_equity_residual\",\"day1_sales\":{},\"early_settle_no_shareholder_payment\":true,\"creditor_paid_cents\":1000,\"company_cash_before_exit_distribution\":620145,\"paid_in_returned_cents\":{},\"surplus_distributed_cents\":{},\"treasury_final_cents\":{},\"household_final_cents\":{},\"company_final_cents\":{},\"creditor_final_cents\":{},\"closed_state_hash\":\"{}\",\"checkpoint_cold_open\":true,\"measured\":false}}",
        day1.actual_sales.value(), register.cumulative_capital_returned.value(), register.cumulative_liquidation_surplus_distributed.value(), cash(&store, 1)?, cash(&store, 2)?, cash(&store, 3)?, cash(&store, 4)?, hash);
    println!("{{\"owned_capacity_bytes_initial\":{},\"owned_capacity_bytes_final\":{},\"engine_owned_capacity_bytes_initial\":{},\"engine_owned_capacity_bytes_final\":{},\"process_peak_rss\":\"not_measured\"}}", initial_store_owned_bytes, final_store_owned_bytes, initial_engine_owned_bytes, final_engine_owned_bytes);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("town_equity_creditor_residual: {error}");
        std::process::exit(1);
    }
}
