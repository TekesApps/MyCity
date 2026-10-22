use mycity_core::{
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
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
