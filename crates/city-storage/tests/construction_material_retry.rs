use mycity_core::{
    construction::{
        EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material, Project,
        OPERATION_PREFIX,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource, TxError},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const RULES: &str = "construction-engine-v1";
const END_OF_SHIFT: u64 = 17 * 3_600;
static NEXT: AtomicU64 = AtomicU64::new(0);
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(9_000_000);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-construction-material-retry-{}-{n}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }

    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "construction material retry artifacts retained at {}",
                self.0.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn opening() -> Vec<u8> {
    let project = Project::new(
        7,
        Kind::Build,
        4,
        3,
        1,
        80,
        1,
        1,
        Money::new(1_000_000).unwrap(),
        Money::new(3_600).unwrap(),
        28_800,
        &[
            Gate {
                id: 1,
                category: GateCategory::OwnerConsent,
                evidence_id: 0,
                party: 2,
                decided: GateDecision::Pending,
            },
            Gate {
                id: 2,
                category: GateCategory::Permit,
                evidence_id: 0,
                party: 1,
                decided: GateDecision::Pending,
            },
        ],
        &[Material {
            stock_id: 1,
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(400).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    value["sources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": 6, "kind": "inventory_capital", "amount": 0}));
    value["stocks"].as_array_mut().unwrap().push(json!({
        "id": 2,
        "owner": 4,
        "commodity": "raw",
        "quantity": 0,
        "basis": 0,
        "source": 6
    }));
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path) -> Store {
    Store::create(
        path,
        &opening(),
        vec![
            Resource::new(80, 1).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
        RULES,
    )
    .unwrap()
}

fn envelope(s: &Store, operation_id: u64, action: Action) -> Envelope {
    let view = s.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn next_id() -> u64 {
    NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed)
}

fn apply(s: &mut Store, caller: Caller, action: Action) {
    let request = envelope(s, next_id(), action);
    let outcome = s.execute(caller, request).unwrap();
    assert!(
        outcome.model.is_ok(),
        "unexpected result: {:?}",
        outcome.model
    );
}

fn construction(s: &mut Store, caller: Caller, command: C) {
    apply(s, caller, Action::Construction { command });
}

fn approvals(s: &mut Store) {
    construction(s, Caller::Account(4), C::File);
    construction(
        s,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 21,
        },
    );
    construction(
        s,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 22,
        },
    );
    construction(s, Caller::Account(4), C::Prepare);
}

fn canonical_effect_id(s: &Store) -> u64 {
    let project = s.engine().unwrap().world().view().construction.unwrap();
    OPERATION_PREFIX | (project.id << 32) | u64::from(project.next_effect)
}

fn cash(s: &Store, id: u32) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == id)
        .unwrap()
        .cash
        .value()
}

fn stocks(s: &Store) -> Vec<mycity_core::assembly::StockView> {
    s.engine().unwrap().world().view().stocks
}

fn move_stock(s: &mut Store, from: u32, to: u32, quantity: i64) {
    let e = s.engine().unwrap();
    let from = e.world().stock_ref(from).unwrap();
    let to = e.world().stock_ref(to).unwrap();
    let request = envelope(
        s,
        next_id(),
        Action::MoveStock {
            from,
            to,
            quantity: Quantity::new(quantity).unwrap(),
        },
    );
    let outcome = s.execute(Caller::Account(4), request).unwrap();
    assert!(
        outcome.model.is_ok(),
        "stock movement failed: {:?}",
        outcome.model
    );
}

fn advance(s: &mut Store, target: u64) {
    for _ in 0..100 {
        apply(
            s,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        if s.engine()
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
    panic!("timeline failed to reach the 17-hour work time");
}

#[test]
fn construction_material_failure_retries_after_same_lot_is_restored_and_survives_cold_recovery() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path);
    approvals(&mut store);

    let start = envelope(
        &store,
        canonical_effect_id(&store),
        Action::Construction { command: C::Start },
    );
    assert!(store
        .execute(Caller::Account(4), start)
        .unwrap()
        .model
        .is_ok());
    let site_token = store
        .engine()
        .unwrap()
        .world()
        .view()
        .construction
        .unwrap()
        .site_token
        .unwrap();
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|resource| resource.id == 80)
            .unwrap()
            .occupied,
        1
    );

    apply(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance(&mut store, END_OF_SHIFT);
    let original_stocks = stocks(&store);
    assert_eq!(original_stocks.len(), 2);
    assert_eq!(original_stocks[0].id, 1);
    assert_eq!(original_stocks[0].quantity, Quantity::new(600_000).unwrap());
    assert_eq!(original_stocks[0].basis, Money::new(240_000).unwrap());
    assert_eq!(original_stocks[1].id, 2);
    assert_eq!(original_stocks[1].quantity, Quantity::ZERO);
    assert_eq!(original_stocks[1].basis, Money::ZERO);
    let owner_cash_before = cash(&store, 4);
    let contractor_cash_before = cash(&store, 3);
    let project_before = store.engine().unwrap().world().view().construction.unwrap();

    // Move the complete existing lot out of the project's source inventory.
    move_stock(&mut store, 1, 2, 600_000);
    let moved = stocks(&store);
    assert_eq!(moved[0].quantity, Quantity::ZERO);
    assert_eq!(moved[0].basis, Money::ZERO);
    assert_eq!(moved[1].quantity, Quantity::new(600_000).unwrap());
    assert_eq!(moved[1].basis, Money::new(240_000).unwrap());
    assert_eq!(moved[0].offset, Quantity::new(600_000).unwrap());
    assert_eq!(moved[1].offset, original_stocks[0].offset);
    assert_eq!(moved[1].lot_id, original_stocks[0].lot_id);
    assert_eq!(moved[1].origin_quantity, original_stocks[0].origin_quantity);
    assert_eq!(moved[1].origin_basis, original_stocks[0].origin_basis);
    let engine_before = store.engine().unwrap().state_hash();

    // The canonical internal effect ID records a durable Materials failure without
    // consuming the effect, changing money, or mutating the project.
    let failed_id = canonical_effect_id(&store);
    let failed_work = envelope(
        &store,
        failed_id,
        Action::Construction {
            command: C::Work {
                at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
                elapsed_seconds: END_OF_SHIFT as u32,
                worker_seconds: 28_800,
            },
        },
    );
    let sequence_before_failure = store.committed_sequence();
    let failed = store
        .execute(Caller::Account(3), failed_work.clone())
        .unwrap();
    assert_eq!(
        failed.model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Materials
        ))
    );
    assert_eq!(store.committed_sequence(), sequence_before_failure + 1);
    assert_eq!(store.engine().unwrap().state_hash(), engine_before);
    assert_eq!(
        store.engine().unwrap().world().view().construction.unwrap(),
        project_before
    );
    assert_eq!(cash(&store, 4), owner_cash_before);
    assert_eq!(cash(&store, 3), contractor_cash_before);
    assert_eq!(stocks(&store), moved);

    // Return the exact same lot, then retry under a distinct ordinary Envelope ID.
    move_stock(&mut store, 2, 1, 600_000);
    let returned = stocks(&store);
    assert_eq!(returned[0].quantity, original_stocks[0].quantity);
    assert_eq!(returned[0].basis, original_stocks[0].basis);
    assert_eq!(returned[0].lot_id, original_stocks[0].lot_id);
    assert_eq!(
        returned[0].origin_quantity,
        original_stocks[0].origin_quantity
    );
    assert_eq!(returned[0].origin_basis, original_stocks[0].origin_basis);
    assert_eq!(returned[0].offset, original_stocks[0].offset);
    assert_eq!(returned[1].offset, Quantity::new(600_000).unwrap());
    assert_eq!(returned[1].quantity, Quantity::ZERO);
    assert_eq!(returned[1].basis, Money::ZERO);
    assert_eq!(returned[1].lot_id, original_stocks[0].lot_id);
    assert_eq!(
        returned[1].origin_quantity,
        original_stocks[0].origin_quantity
    );
    assert_eq!(returned[1].origin_basis, original_stocks[0].origin_basis);
    let successful_id = next_id();
    assert_ne!(successful_id, failed_id);
    let successful_work = envelope(
        &store,
        successful_id,
        Action::Construction {
            command: C::Work {
                at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
                elapsed_seconds: END_OF_SHIFT as u32,
                worker_seconds: 28_800,
            },
        },
    );
    assert!(store
        .execute(Caller::Account(3), successful_work.clone())
        .unwrap()
        .model
        .is_ok());
    let after_work = stocks(&store);
    assert_eq!(after_work[0].quantity, Quantity::new(599_000).unwrap());
    assert_eq!(after_work[0].basis, Money::new(239_600).unwrap());
    assert_eq!(after_work[0].offset, Quantity::new(1_000).unwrap());
    assert_eq!(after_work[1].offset, Quantity::new(600_000).unwrap());
    assert_eq!(after_work[1].quantity, Quantity::ZERO);
    assert_eq!(after_work[1].basis, Money::ZERO);
    assert_eq!(cash(&store, 4), owner_cash_before - 28_800);
    assert_eq!(cash(&store, 3), contractor_cash_before + 28_800);
    let completed = store.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(completed.completed_worker_seconds, 28_800);
    assert_eq!(completed.spent, Money::new(29_200).unwrap());
    assert_eq!(
        completed.materials[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(completed.site_token, Some(site_token));

    let sequence_before_replay = store.committed_sequence();
    assert_eq!(
        store
            .execute(Caller::Account(3), failed_work.clone())
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Materials
        ))
    );
    assert!(
        store
            .execute(Caller::Account(3), successful_work.clone())
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(store.committed_sequence(), sequence_before_replay);
    assert_eq!(stocks(&store), after_work);
    assert_eq!(cash(&store, 4), owner_cash_before - 28_800);
    assert_eq!(cash(&store, 3), contractor_cash_before + 28_800);

    let engine_hash = store.engine().unwrap().state_hash();
    drop(store);
    let mut wal_reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(wal_reopened.engine().unwrap().state_hash(), engine_hash);
    assert_eq!(stocks(&wal_reopened), after_work);
    assert_eq!(cash(&wal_reopened, 4), owner_cash_before - 28_800);
    assert_eq!(cash(&wal_reopened, 3), contractor_cash_before + 28_800);
    assert_eq!(
        wal_reopened
            .execute(Caller::Account(3), failed_work.clone())
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Materials
        ))
    );
    assert!(
        wal_reopened
            .execute(Caller::Account(3), successful_work.clone())
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(wal_reopened.engine().unwrap().state_hash(), engine_hash);

    wal_reopened.checkpoint().unwrap();
    drop(wal_reopened);
    let mut checkpoint_reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(
        checkpoint_reopened.engine().unwrap().state_hash(),
        engine_hash
    );
    assert_eq!(stocks(&checkpoint_reopened), after_work);
    assert_eq!(cash(&checkpoint_reopened, 4), owner_cash_before - 28_800);
    assert_eq!(
        cash(&checkpoint_reopened, 3),
        contractor_cash_before + 28_800
    );
    assert_eq!(
        checkpoint_reopened
            .execute(Caller::Account(3), failed_work)
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Materials
        ))
    );
    assert!(
        checkpoint_reopened
            .execute(Caller::Account(3), successful_work)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(
        checkpoint_reopened.engine().unwrap().state_hash(),
        engine_hash
    );
}
