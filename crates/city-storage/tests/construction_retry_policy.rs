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
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const RULES: &str = "construction-engine-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(8_000_000);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-construction-retry-{}-{n}",
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
                "construction retry artifacts retained at {}",
                self.0.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn opening(legacy: bool) -> Vec<u8> {
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
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 3;
    project.waste_receiver_resource = 82;
    project.waste_units = 2;
    project.waste_fee = Money::new(250).unwrap();

    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    if legacy {
        // Old serialized worlds have no policy field and retain legacy receipt semantics.
        value["construction"]
            .as_object_mut()
            .unwrap()
            .remove("execution_policy");
    }
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path, legacy: bool) -> Store {
    Store::create(
        path,
        &opening(legacy),
        vec![
            Resource::new(80, 1).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
        RULES,
    )
    .unwrap()
}

fn env(s: &Store, operation_id: u64, command: C) -> Envelope {
    env_action(s, operation_id, Action::Construction { command })
}

fn env_action(s: &Store, operation_id: u64, action: Action) -> Envelope {
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

fn transact(s: &mut Store, caller: Caller, command: C) {
    let request = env(s, next_id(), command);
    let response = s.execute(caller, request).unwrap();
    assert!(
        response.model.is_ok(),
        "unexpected model result: {:?}",
        response.model
    );
}

fn ordinary_action(s: &mut Store, caller: Caller, action: Action) {
    let request = env_action(s, next_id(), action);
    let response = s.execute(caller, request).unwrap();
    assert!(
        response.model.is_ok(),
        "unexpected model result: {:?}",
        response.model
    );
}

fn approvals(s: &mut Store) {
    transact(s, Caller::Account(4), C::File);
    transact(
        s,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 11,
        },
    );
    transact(
        s,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 12,
        },
    );
    transact(s, Caller::Account(4), C::Prepare);
}

fn effect_id(s: &Store) -> u64 {
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

fn advance(s: &mut Store, target: u64) {
    for _ in 0..100 {
        ordinary_action(
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
    panic!("timeline failed to reach requested work time");
}

#[test]
fn recoverable_start_capacity_failure_can_retry_with_new_envelope_after_release() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path, false);
    approvals(&mut store);

    let reserve_request = env_action(
        &store,
        next_id(),
        Action::ReserveCapacity {
            resource: 80,
            units: 1,
        },
    );
    let lease = store
        .execute(Caller::Account(2), reserve_request)
        .unwrap()
        .model
        .unwrap()
        .created_tokens[0]
        .unwrap();
    ordinary_action(
        &mut store,
        Caller::Account(2),
        Action::OccupyCapacity { token: lease },
    );

    let failed_id = effect_id(&store);
    let failed_start = env(&store, failed_id, C::Start);
    let sequence_before = store.committed_sequence();
    let engine_before = store.engine().unwrap().state_hash();
    let failed = store
        .execute(Caller::Account(4), failed_start.clone())
        .unwrap();
    assert_eq!(failed.model, Err(TxError::Capacity));
    assert_eq!(store.committed_sequence(), sequence_before + 1);
    assert_eq!(store.engine().unwrap().state_hash(), engine_before);

    ordinary_action(
        &mut store,
        Caller::Account(2),
        Action::ReleaseCapacity { token: lease },
    );
    let successful_id = next_id();
    assert_ne!(failed_id, successful_id);
    let successful_start = env(&store, successful_id, C::Start);
    assert!(store
        .execute(Caller::Account(4), successful_start.clone())
        .unwrap()
        .model
        .is_ok());
    let started = store.engine().unwrap().world().view().construction.unwrap();
    let site_token = started.site_token.unwrap();
    let cash_after_start = cash(&store, 4);
    let occupied_after_start = store
        .engine()
        .unwrap()
        .resources()
        .iter()
        .find(|resource| resource.id == 80)
        .unwrap()
        .occupied;
    assert_eq!(occupied_after_start, 1);

    // Both successful and failed operation IDs keep their original receipts.
    let seq_before_replays = store.committed_sequence();
    assert_eq!(
        store
            .execute(Caller::Account(4), failed_start.clone())
            .unwrap()
            .model,
        Err(TxError::Capacity)
    );
    let successful_replay = store
        .execute(Caller::Account(4), successful_start.clone())
        .unwrap();
    assert!(successful_replay.has_persistent_receipt);
    assert!(successful_replay.model.is_ok());
    assert_eq!(store.committed_sequence(), seq_before_replays);
    assert_eq!(cash(&store, 4), cash_after_start);
    let project = store.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(project.site_token, Some(site_token));
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|resource| resource.id == 80)
            .unwrap()
            .occupied,
        occupied_after_start
    );

    let engine_hash = store.engine().unwrap().state_hash();
    drop(store);
    let mut reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(reopened.engine().unwrap().state_hash(), engine_hash);
    assert_eq!(
        reopened
            .execute(Caller::Account(4), failed_start.clone())
            .unwrap()
            .model,
        Err(TxError::Capacity)
    );
    assert!(
        reopened
            .execute(Caller::Account(4), successful_start.clone())
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(reopened.engine().unwrap().state_hash(), engine_hash);
    reopened.checkpoint().unwrap();
    drop(reopened);

    let mut checkpointed = Store::open(&path, RULES).unwrap();
    assert_eq!(checkpointed.engine().unwrap().state_hash(), engine_hash);
    assert_eq!(
        checkpointed
            .execute(Caller::Account(4), failed_start)
            .unwrap()
            .model,
        Err(TxError::Capacity)
    );
    assert!(
        checkpointed
            .execute(Caller::Account(4), successful_start)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(checkpointed.engine().unwrap().state_hash(), engine_hash);
}

#[test]
fn recoverable_work_funds_failure_can_retry_after_protection_is_released() {
    const END_OF_SHIFT: u64 = 17 * 3_600;
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path, false);
    approvals(&mut store);
    transact(&mut store, Caller::Account(4), C::Start);
    ordinary_action(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance(&mut store, END_OF_SHIFT);

    let owner_cash = cash(&store, 4);
    let protected = Money::new(owner_cash - 10_000).unwrap();
    let owner_handle = store.engine().unwrap().world().account_ref(4).unwrap();
    ordinary_action(
        &mut store,
        Caller::Account(4),
        Action::Protect {
            account: owner_handle,
            amount: protected,
        },
    );
    let engine_before_failure = store.engine().unwrap().state_hash();
    let project_before = store.engine().unwrap().world().view().construction.unwrap();
    let failed_id = effect_id(&store);
    let failed_work = env(
        &store,
        failed_id,
        C::Work {
            at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
            elapsed_seconds: END_OF_SHIFT as u32,
            worker_seconds: 28_800,
        },
    );
    let sequence_before = store.committed_sequence();
    assert_eq!(
        store
            .execute(Caller::Account(3), failed_work.clone())
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert_eq!(store.committed_sequence(), sequence_before + 1);
    assert_eq!(store.engine().unwrap().state_hash(), engine_before_failure);
    assert_eq!(
        store.engine().unwrap().world().view().construction.unwrap(),
        project_before
    );

    let protected_hash = store.engine().unwrap().state_hash();
    let owner_release = env_action(
        &store,
        next_id(),
        Action::ReleaseProtection {
            account: store.engine().unwrap().world().account_ref(4).unwrap(),
            amount: protected,
        },
    );
    assert_eq!(
        store
            .execute(Caller::Account(4), owner_release)
            .unwrap()
            .model,
        Err(TxError::Unauthorized)
    );
    assert_eq!(store.engine().unwrap().state_hash(), protected_hash);
    // Only the existing System authority may release protected funds.
    let owner_handle = store.engine().unwrap().world().account_ref(4).unwrap();
    ordinary_action(
        &mut store,
        Caller::System,
        Action::ReleaseProtection {
            account: owner_handle,
            amount: protected,
        },
    );
    let successful_id = next_id();
    assert_ne!(failed_id, successful_id);
    let successful_work = env(
        &store,
        successful_id,
        C::Work {
            at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
            elapsed_seconds: END_OF_SHIFT as u32,
            worker_seconds: 28_800,
        },
    );
    let owner_cash_before_success = cash(&store, 4);
    let contractor_cash_before_success = cash(&store, 3);
    assert!(store
        .execute(Caller::Account(3), successful_work.clone())
        .unwrap()
        .model
        .is_ok());
    let owner_cash_after_success = cash(&store, 4);
    let contractor_cash_after_success = cash(&store, 3);
    assert_eq!(owner_cash_after_success, owner_cash_before_success - 28_800);
    assert_eq!(
        contractor_cash_after_success,
        contractor_cash_before_success + 28_800
    );
    let completed = store.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(completed.completed_worker_seconds, 28_800);
    assert_eq!(completed.spent, Money::new(29_200).unwrap());

    let sequence_before_replays = store.committed_sequence();
    assert_eq!(
        store
            .execute(Caller::Account(3), failed_work.clone())
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert!(
        store
            .execute(Caller::Account(3), successful_work.clone())
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(store.committed_sequence(), sequence_before_replays);
    assert_eq!(cash(&store, 4), owner_cash_after_success);
    assert_eq!(cash(&store, 3), contractor_cash_after_success);
    assert_eq!(
        store.engine().unwrap().world().view().construction.unwrap(),
        completed
    );

    let engine_hash = store.engine().unwrap().state_hash();
    drop(store);
    let mut reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(reopened.engine().unwrap().state_hash(), engine_hash);
    assert_eq!(
        reopened
            .execute(Caller::Account(3), failed_work.clone())
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert_eq!(cash(&reopened, 4), owner_cash_after_success);
    assert_eq!(cash(&reopened, 3), contractor_cash_after_success);
    assert_eq!(reopened.engine().unwrap().state_hash(), engine_hash);
    reopened.checkpoint().unwrap();
    drop(reopened);

    let mut checkpointed = Store::open(&path, RULES).unwrap();
    assert_eq!(checkpointed.engine().unwrap().state_hash(), engine_hash);
    assert_eq!(
        checkpointed
            .execute(Caller::Account(3), failed_work)
            .unwrap()
            .model,
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert!(
        checkpointed
            .execute(Caller::Account(3), successful_work)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(checkpointed.engine().unwrap().state_hash(), engine_hash);
}

#[test]
fn legacy_missing_policy_preserves_failed_start_receipt_after_canonical_retry_and_reopen() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path, true);
    approvals(&mut store);
    let project = store.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(
        project.execution_policy,
        mycity_core::construction::ExecutionPolicy::LegacyV1
    );

    let failed_id = next_id();
    let failed_start = env(&store, failed_id, C::Start);
    let engine_before = store.engine().unwrap().state_hash();
    let sequence_before = store.committed_sequence();
    assert_eq!(
        store
            .execute(Caller::Account(4), failed_start.clone())
            .unwrap()
            .model,
        Err(TxError::InvalidOperation)
    );
    assert_eq!(store.committed_sequence(), sequence_before + 1);
    assert_eq!(store.engine().unwrap().state_hash(), engine_before);

    let canonical_id = effect_id(&store);
    assert_ne!(failed_id, canonical_id);
    let canonical_start = env(&store, canonical_id, C::Start);
    assert!(store
        .execute(Caller::Account(4), canonical_start)
        .unwrap()
        .model
        .is_ok());
    let engine_after_start = store.engine().unwrap().state_hash();
    drop(store);

    let mut reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(reopened.engine().unwrap().state_hash(), engine_after_start);
    let sequence_after_reopen = reopened.committed_sequence();
    let old_replay = reopened
        .execute(Caller::Account(4), failed_start.clone())
        .unwrap();
    assert!(old_replay.has_persistent_receipt);
    assert_eq!(old_replay.model, Err(TxError::InvalidOperation));
    assert_eq!(reopened.committed_sequence(), sequence_after_reopen);
    assert_eq!(reopened.engine().unwrap().state_hash(), engine_after_start);
    assert!(
        reopened
            .engine()
            .unwrap()
            .world()
            .view()
            .construction
            .unwrap()
            .site_reserved
    );
    reopened.checkpoint().unwrap();
    drop(reopened);

    let mut checkpointed = Store::open(&path, RULES).unwrap();
    assert_eq!(
        checkpointed.engine().unwrap().state_hash(),
        engine_after_start
    );
    let old_request = failed_start;
    assert_eq!(
        checkpointed
            .execute(Caller::Account(4), old_request)
            .unwrap()
            .model,
        Err(TxError::InvalidOperation)
    );
    assert_eq!(
        checkpointed.engine().unwrap().state_hash(),
        engine_after_start
    );
}
