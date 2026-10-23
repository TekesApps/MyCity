use mycity_core::{
    construction::{
        ConstructionCompensation, EngineCommand as C, Gate, GateCategory, GateDecision, Kind,
        Material, Project,
    },
    transaction::{Action, Caller, Envelope, Resource, TxError},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const RULES: &str = "construction-compensation-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-construction-compensation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
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
        if std::thread::panicking() || std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() {
            eprintln!(
                "mycity compensation test artifacts retained at {}",
                self.0.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn opening() -> Vec<u8> {
    let mut project = Project::new(
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
            Gate {
                id: 3,
                category: GateCategory::Compensation,
                evidence_id: 0,
                party: 2,
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
    project.compensation =
        Some(ConstructionCompensation::new(3, 2, Money::new(50_000).unwrap()).unwrap());
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 3;
    project.waste_receiver_resource = 82;
    project.waste_units = 2;
    project.waste_fee = Money::new(250).unwrap();
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
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

fn opening_with_owner_cash(cash: i64) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&opening()).unwrap();
    let owner = value["accounts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == 4)
        .unwrap();
    owner["cash"] = json!(cash);
    let source = value["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == 4)
        .unwrap();
    source["amount"] = json!(cash);
    serde_json::to_vec(&value).unwrap()
}

fn create_from(path: &Path, opening: &[u8]) -> Store {
    Store::create(
        path,
        opening,
        vec![
            Resource::new(80, 1).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
        RULES,
    )
    .unwrap()
}

fn request(s: &Store, operation_id: u64, command: C) -> Envelope {
    request_action(s, operation_id, Action::Construction { command })
}

fn request_action(s: &Store, operation_id: u64, action: Action) -> Envelope {
    let view = s.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

#[test]
fn failed_payment_receipt_survives_cold_open_and_new_operation_can_retry_after_funding() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create_from(&path, &opening_with_owner_cash(40_000));
    execute(&mut store, Caller::Account(4), 710_001, C::File);
    let compensation = C::DecideGate {
        gate_id: 3,
        decision: GateDecision::Accepted,
        evidence_id: 31,
    };
    let failed_id = 710_002;
    let failed_request = request(&store, failed_id, compensation);
    let before = store.engine().unwrap().world().view();
    let failed = store
        .execute(Caller::Account(2), failed_request.clone())
        .unwrap();
    assert_eq!(failed.model, Err(TxError::InsufficientFunds));
    let after = store.engine().unwrap().world().view();
    assert_eq!(after.accounts, before.accounts);
    assert_eq!(after.construction, before.construction);
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let replay = store
        .execute(Caller::Account(2), failed_request.clone())
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.model, Err(TxError::InsufficientFunds));
    assert_eq!(cash(&store, 4), 40_000);
    assert_eq!(cash(&store, 2), 200_000);

    let from = store.engine().unwrap().world().account_ref(3).unwrap();
    let to = store.engine().unwrap().world().account_ref(4).unwrap();
    let transfer = request_action(
        &store,
        710_003,
        Action::Transfer {
            from,
            to,
            amount: Money::new(20_000).unwrap(),
        },
    );
    assert!(store
        .execute(Caller::Account(3), transfer)
        .unwrap()
        .model
        .is_ok());
    assert_eq!(cash(&store, 4), 60_000);

    // Retrying the old operation never re-evaluates it after funds arrive.
    let old_retry = store.execute(Caller::Account(2), failed_request).unwrap();
    assert_eq!(old_retry.model, Err(TxError::InsufficientFunds));
    assert_eq!(cash(&store, 4), 60_000);
    let success_request = request(&store, failed_id + 2, compensation);
    let success = store.execute(Caller::Account(2), success_request).unwrap();
    assert!(success.model.is_ok());
    assert_eq!(cash(&store, 4), 10_000);
    assert_eq!(cash(&store, 2), 250_000);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .construction
            .unwrap()
            .compensation
            .unwrap()
            .settled_operation_id(),
        Some(failed_id + 2)
    );
}

fn execute(s: &mut Store, caller: Caller, op: u64, command: C) {
    let request = request(s, op, command);
    assert!(s.execute(caller, request).unwrap().model.is_ok());
}

fn cash(s: &Store, account: u32) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .unwrap()
        .cash
        .value()
}

#[test]
fn compensation_transfer_is_atomic_and_exactly_once_across_cold_wal_and_checkpoint_replay() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path);

    let system_request = request(&store, 700_001, C::File);
    let system_file = store.execute(Caller::System, system_request).unwrap();
    assert_eq!(system_file.model, Err(TxError::Unauthorized));
    let outsider_file_request = request(&store, 700_002, C::File);
    let outsider_file = store
        .execute(Caller::Account(3), outsider_file_request)
        .unwrap();
    assert_eq!(outsider_file.model, Err(TxError::Unauthorized));
    execute(&mut store, Caller::Account(4), 700_003, C::File);

    let compensation = C::DecideGate {
        gate_id: 3,
        decision: GateDecision::Accepted,
        evidence_id: 31,
    };
    let outsider_accept_request = request(&store, 700_004, compensation);
    let outsider_accept = store
        .execute(Caller::Account(3), outsider_accept_request)
        .unwrap();
    assert_eq!(outsider_accept.model, Err(TxError::Unauthorized));
    let system_accept_request = request(&store, 700_005, compensation);
    let system_accept = store
        .execute(Caller::System, system_accept_request)
        .unwrap();
    assert_eq!(system_accept.model, Err(TxError::Unauthorized));

    let payer_before = cash(&store, 4);
    let payee_before = cash(&store, 2);
    let op = 700_006;
    let paid_request = request(&store, op, compensation);
    let lost_reply =
        store.execute_with_observer(Caller::Account(2), paid_request.clone(), |point| {
            if point == WritePoint::Applied {
                return Err(io::Error::other("simulated lost compensation reply"));
            }
            Ok(())
        });
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(cash(&store, 4), payer_before - 50_000);
    assert_eq!(cash(&store, 2), payee_before + 50_000);
    let project = store.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(project.spent, Money::new(50_000).unwrap());
    assert_eq!(
        project.compensation.unwrap().settled_operation_id(),
        Some(op)
    );
    let replay = store
        .execute(Caller::Account(2), paid_request.clone())
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(cash(&store, 4), payer_before - 50_000);
    assert_eq!(cash(&store, 2), payee_before + 50_000);

    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let archived = store.receipt(op).unwrap().unwrap();
    assert!(matches!(
        archived.actions[0],
        Some(Action::Construction {
            command: C::DecideGate {
                gate_id: 3,
                decision: GateDecision::Accepted,
                evidence_id: 31,
            }
        })
    ));
    assert!(
        store
            .execute(Caller::Account(2), paid_request)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(cash(&store, 4), payer_before - 50_000);
    assert_eq!(cash(&store, 2), payee_before + 50_000);

    let second_request = request(&store, op + 1, compensation);
    let second_operation = store.execute(Caller::Account(2), second_request).unwrap();
    assert_eq!(
        second_operation.model,
        Err(TxError::Construction(
            mycity_core::construction::Error::InvalidInput
        ))
    );
    assert_eq!(cash(&store, 4), payer_before - 50_000);
    assert_eq!(cash(&store, 2), payee_before + 50_000);

    // A later rejection does not implicitly refund a completed compensation contract.
    execute(
        &mut store,
        Caller::Account(1),
        op + 2,
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Rejected,
            evidence_id: 0,
        },
    );
    assert_eq!(cash(&store, 4), payer_before - 50_000);
    assert_eq!(cash(&store, 2), payee_before + 50_000);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .construction
            .unwrap()
            .spent,
        Money::new(50_000).unwrap()
    );
}
