use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{Money, Quantity},
};
use mycity_storage::{Query, Store, WritePoint};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const RULES: &str = "town-market-engine-v0.1";
static ID: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mycity-town-store-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn opening() -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":100000}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    value["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":100000,"basis":100000,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    value["town"] = serde_json::json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}]
    });
    serde_json::to_vec(&value).unwrap()
}

fn opening_with_daily_sellout() -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&opening()).unwrap();
    for source in value["sources"].as_array_mut().unwrap() {
        if source["id"] == 2 {
            source["amount"] = 1_000_000.into();
        }
    }
    for account in value["accounts"].as_array_mut().unwrap() {
        if account["id"] == 2 {
            account["cash"] = 1_000_000.into();
        }
    }
    value["town"]["households"][0]["need_per_household"] = 100.into();
    serde_json::to_vec(&value).unwrap()
}

fn envelope(store: &Store, action: Action) -> Envelope {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    }
}
fn execute(store: &mut Store, action: Action) {
    let request = envelope(store, action);
    assert!(store
        .execute(Caller::System, request)
        .unwrap()
        .model
        .is_ok());
}
fn cash(store: &Store, id: u32) -> Money {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == id)
        .unwrap()
        .cash
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    sequence: u64,
    hash: String,
    cash: Vec<(u32, i64)>,
    stocks: Vec<(u32, i64, i64, u32)>,
    payroll: mycity_core::timeline::Payroll,
    settled_day: u64,
    summary: mycity_core::town_runtime::MarketSummary,
    payroll_cursor: mycity_core::town_runtime::PayrollCursor,
    output_remainder: mycity_core::town_runtime::RecipeOutputRemainder,
}

fn snapshot(store: &mut Store) -> Snapshot {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    let timeline = engine.timeline().unwrap();
    let payroll = timeline
        .payroll
        .iter()
        .find(|row| row.spec.id == 1)
        .copied()
        .unwrap();
    let town = view.town.as_ref().unwrap();
    Snapshot {
        sequence: store.committed_sequence(),
        hash: store.state_hash().unwrap(),
        cash: view
            .accounts
            .iter()
            .map(|row| (row.id, row.cash.value()))
            .collect(),
        stocks: view
            .stocks
            .iter()
            .map(|row| (row.id, row.quantity.value(), row.basis.value(), row.lot_id))
            .collect(),
        payroll,
        settled_day: town.state.settled_day,
        summary: town.state.last_summary,
        payroll_cursor: town
            .state
            .payroll
            .as_slice()
            .iter()
            .find(|row| row.id == 1)
            .copied()
            .unwrap(),
        output_remainder: town
            .state
            .recipe_output_remainders
            .as_slice()
            .iter()
            .find(|row| row.recipe_id == 1)
            .copied()
            .unwrap(),
    }
}

fn advance_to_day(store: &mut Store, target: u64) {
    for _ in 0..128 {
        execute(
            store,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("timeline failed to reach target {target}");
}

#[test]
fn town_work_and_market_day_recover_once_after_applied_reply_loss() {
    let temp = Temp::new();
    let path = temp.0.join("world");
    let mut store = Store::create(&path, &opening(), vec![], RULES).unwrap();
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: 3,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: Money::new(100).unwrap(),
                    cycle: Cycle::Daily,
                    ends_at: 7 * DAY,
                },
            },
        },
    );
    for _ in 0..100 {
        execute(
            &mut store,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target: DAY,
                    limit: 8192,
                },
            },
        );
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            break;
        }
    }
    let operation_id = store.committed_sequence() + 1;
    let request = envelope(&store, Action::TownSettle { profile_id: 1 });
    assert_eq!(request.operation_id, operation_id);
    let before_cash = cash(&store, 3);
    let result = store.execute_with_observer(Caller::System, request.clone(), |point| {
        if point == WritePoint::Applied {
            Err(std::io::Error::other(
                "simulated lost town settlement reply",
            ))
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert!(store.engine().is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(store.committed_sequence(), operation_id);
    let state_hash = store.state_hash().unwrap();
    let view = store.engine().unwrap().world().view();
    assert_eq!(view.town.unwrap().state.settled_day, 1);
    assert_eq!(
        view.town.unwrap().state.last_summary.produced,
        Quantity::new(100).unwrap()
    );
    assert_eq!(
        cash(&store, 3),
        before_cash.checked_add(Money::new(900).unwrap()).unwrap()
    );

    let replay = store.execute(Caller::System, request.clone()).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(store.state_hash().unwrap(), state_hash);
    assert_eq!(
        cash(&store, 3),
        before_cash.checked_add(Money::new(900).unwrap()).unwrap()
    );
    store.checkpoint().unwrap();
    drop(store);

    let mut store = match Store::open(&path, RULES) {
        Ok(store) => store,
        Err(error) => {
            let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/evidence/core-production-validation/attempt-08/raw/checkpoint-failure-world");
            fs::create_dir_all(&evidence).unwrap();
            for entry in fs::read_dir(&path).unwrap() {
                let entry = entry.unwrap();
                let destination = evidence.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    fs::create_dir_all(&destination).unwrap();
                    for nested in fs::read_dir(entry.path()).unwrap() {
                        let nested = nested.unwrap();
                        fs::copy(nested.path(), destination.join(nested.file_name())).unwrap();
                    }
                } else {
                    fs::copy(entry.path(), destination).unwrap();
                }
            }
            panic!("checkpoint cold-open failed: {error:?}");
        }
    };
    let receipt = store.receipt(operation_id).unwrap().unwrap();
    assert!(matches!(
        receipt.actions[0],
        Some(Action::TownSettle { profile_id: 1 })
    ));
    assert!(
        store
            .execute(Caller::System, request)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(store.committed_sequence(), operation_id);
    assert_eq!(store.state_hash().unwrap(), state_hash);
}

#[test]
fn twenty_eight_day_store_checkpoints_reopen_with_contiguous_receipts_and_work_remainders() {
    const DAYS: u64 = 28;
    const FINAL_SHIFT_SECONDS: u64 = 60;
    let temp = Temp::new();
    let path = temp.0.join("world");
    let mut store = Store::create(&path, &opening_with_daily_sellout(), vec![], RULES).unwrap();
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: 3,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: Money::new(100).unwrap(),
                    cycle: Cycle::Daily,
                    ends_at: (DAYS - 1) * DAY + 9 * 3600 + FINAL_SHIFT_SECONDS,
                },
            },
        },
    );

    let mut town_receipts = Vec::with_capacity(DAYS as usize);
    for day in 1..=DAYS {
        advance_to_day(&mut store, day * DAY);
        execute(&mut store, Action::TownSettle { profile_id: 1 });
        let operation_id = store.committed_sequence();
        town_receipts.push((day, operation_id));
        let current = store.engine().unwrap().world().view();
        assert_eq!(current.town.as_ref().unwrap().state.settled_day, day);
        drop(current);

        if day % 7 == 0 {
            let before = snapshot(&mut store);
            store.checkpoint().unwrap();
            drop(store);
            store = Store::open(&path, RULES).unwrap();
            assert_eq!(snapshot(&mut store), before, "cold reopen after day {day}");
            for &(receipt_day, receipt_id) in &town_receipts {
                let record = store.receipt(receipt_id).unwrap().unwrap();
                assert_eq!(record.operation_id, receipt_id);
                assert!(matches!(
                    record.actions[0],
                    Some(Action::TownSettle { profile_id: 1 })
                ));
                assert_eq!(record.outcome.unwrap().town.unwrap().day, receipt_day);
            }
        }
    }

    let final_state = snapshot(&mut store);
    let expected_work_seconds = 27 * 8 * 3600 + FINAL_SHIFT_SECONDS;
    assert_eq!(final_state.payroll.work_seconds, expected_work_seconds);
    assert_eq!(final_state.payroll.accrued, Money::new(21_601).unwrap());
    assert_eq!(final_state.payroll.paid, Money::new(21_601).unwrap());
    assert_eq!(final_state.payroll.remainder, 2_400);
    assert_eq!(
        final_state.payroll_cursor.work_seconds,
        expected_work_seconds
    );
    assert_eq!(
        final_state.payroll_cursor.accrued,
        Money::new(21_601).unwrap()
    );
    assert_eq!(final_state.payroll_cursor.paid, Money::new(21_601).unwrap());
    assert_eq!(final_state.output_remainder.numerator, 6_000);
    assert_eq!(final_state.settled_day, DAYS);

    let mut cursor: Option<String> = None;
    let mut rows = Vec::new();
    loop {
        let page = store.query(Query::Receipts, cursor.as_deref(), 64).unwrap();
        rows.extend(page.rows);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    rows.sort_by_key(|row| row["committed_sequence"].as_u64().unwrap());
    assert_eq!(rows.len() as u64, store.committed_sequence());
    for (index, row) in rows.iter().enumerate() {
        let sequence = index as u64 + 1;
        assert_eq!(row["committed_sequence"].as_u64(), Some(sequence));
        assert_eq!(row["record"]["operation_id"].as_u64(), Some(sequence));
    }
    for &(day, receipt_id) in &town_receipts {
        let record = store.receipt(receipt_id).unwrap().unwrap();
        assert_eq!(record.outcome.unwrap().town.unwrap().day, day);
    }
    eprintln!(
        "28_day_store_fixture=daily_sellout_need100 receipt_count={} town_receipts={town_receipts:?} final_snapshot={final_state:?}",
        rows.len()
    );
}

#[test]
fn partial_paid_shift_with_unsold_inventory_is_idle_not_recipe_remainder() {
    let temp = Temp::new();
    let path = temp.0.join("world");
    let mut store = Store::create(&path, &opening(), vec![], RULES).unwrap();
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: 3,
                    payee: 2,
                    home_city: 0,
                    hourly_cents: Money::new(100).unwrap(),
                    cycle: Cycle::Daily,
                    ends_at: DAY + 9 * 3600 + 60,
                },
            },
        },
    );

    advance_to_day(&mut store, DAY);
    execute(&mut store, Action::TownSettle { profile_id: 1 });
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .stocks
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .quantity,
        Quantity::new(90).unwrap()
    );

    advance_to_day(&mut store, 2 * DAY);
    let request = envelope(&store, Action::TownSettle { profile_id: 1 });
    let response = store.execute(Caller::System, request).unwrap();
    let summary = response.model.unwrap().town.unwrap();
    assert_eq!(summary.day, 2);
    assert_eq!(summary.produced, Quantity::ZERO);
    assert_eq!(summary.labor_basis, Money::ZERO);
    assert_eq!(summary.wages_accrued, Money::new(1).unwrap());
    assert_eq!(summary.idle_labor_expense, Money::new(1).unwrap());
    let town = store.engine().unwrap().world().view().town.unwrap();
    assert_eq!(
        town.state.recipe_output_remainders.as_slice()[0].numerator,
        0
    );
}
