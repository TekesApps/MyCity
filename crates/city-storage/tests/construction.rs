use mycity_core::{
    construction::{
        EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material, Project,
        OPERATION_PREFIX,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource},
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
const RULES: &str = "construction-engine-v1";
const V106_RULES: &str = "6b70d8bc08d00625b369ae92d58ae5ce9dceff9d31a733fe17732de3dd38c87d";
const V106_GENESIS: &[u8] = include_bytes!(
    "../../../docs/evidence/core-construction-compat/attempt-01/old-v106-world/genesis.json"
);
const V106_CURRENT: &[u8] = include_bytes!(
    "../../../docs/evidence/core-construction-compat/attempt-01/old-v106-world/CURRENT.json"
);
const V106_ROOT_0: &[u8] = include_bytes!(
    "../../../docs/evidence/core-construction-compat/attempt-01/old-v106-world/root-0.json"
);
const V106_ROOT_1: &[u8] = include_bytes!(
    "../../../docs/evidence/core-construction-compat/attempt-01/old-v106-world/root-1.json"
);
const V106_WAL: &[u8] = include_bytes!(
    "../../../docs/evidence/core-construction-compat/attempt-01/old-v106-world/wal.bin"
);
const V106_PROBE: &[u8] = include_bytes!(
    "../../../docs/evidence/core-construction-compat/attempt-01/old-v106-storage-probe.json"
);
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("mycity-construction-{}-{n}", std::process::id()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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
    let mut v: Value = serde_json::from_slice(OPENING).unwrap();
    v["construction"] = serde_json::to_value(project).unwrap();
    serde_json::to_vec(&v).unwrap()
}
fn create(path: &Path) -> Store {
    Store::create(path, &opening(), vec![Resource::new(80, 1).unwrap()], RULES).unwrap()
}
fn env(s: &Store, operation_id: u64, command: C) -> Envelope {
    env_action(s, operation_id, Action::Construction { command })
}
fn env_action(s: &Store, operation_id: u64, action: Action) -> Envelope {
    let e = s.engine().unwrap();
    let view = e.world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![action],
    }
}
fn ordinary_action(s: &mut Store, caller: Caller, action: Action) {
    let id = ordinary(s);
    let request = env_action(s, id, action);
    assert!(s.execute(caller, request).unwrap().model.is_ok());
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
    panic!("timeline failed to reach work end");
}
fn ordinary(s: &Store) -> u64 {
    s.engine().unwrap().world().view().revision + 100
}
fn call(s: &mut Store, caller: Caller, command: C, op: u64) {
    let request = env(s, op, command);
    assert!(s.execute(caller, request).unwrap().model.is_ok());
}
fn approvals(s: &mut Store) {
    let id = ordinary(s);
    call(s, Caller::Account(4), C::File, id);
    let id = ordinary(s);
    call(
        s,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 11,
        },
        id,
    );
    let id = ordinary(s);
    call(
        s,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 12,
        },
        id,
    );
    let id = ordinary(s);
    call(s, Caller::Account(4), C::Prepare, id);
}
fn start_id(s: &Store) -> u64 {
    let p = s.engine().unwrap().world().view().construction.unwrap();
    OPERATION_PREFIX | (p.id << 32) | u64::from(p.next_effect)
}
fn cash(s: &Store, id: u32) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == id)
        .unwrap()
        .cash
        .value()
}

#[test]
fn construction_wal_cold_reopen_retry_and_checkpointed_receipt_archive_decode() {
    let t = Temp::new();
    let path = t.world();
    let mut s = create(&path);
    approvals(&mut s);
    let op = start_id(&s);
    let request = env(&s, op, C::Start);
    let before_cash = cash(&s, 4);

    // The durable Engine/WAL operation is applied, then the response path fails.
    let outcome = s.execute_with_observer(Caller::Account(4), request.clone(), |point| {
        if point == WritePoint::Applied {
            return Err(io::Error::other("simulated lost response"));
        }
        Ok(())
    });
    assert!(outcome.is_err());
    drop(s);

    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), 5);
    assert_eq!(cash(&s, 4), before_cash);
    let project = s.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(project.phase, mycity_core::construction::Phase::Building);
    let token = project.site_token.unwrap();
    let lease = s
        .engine()
        .unwrap()
        .leases()
        .iter()
        .flatten()
        .find(|l| l.token == token)
        .unwrap();
    assert_eq!(
        (lease.actor, lease.resource, lease.units, lease.occupied),
        (4, 80, 1, true)
    );

    let replay = s.execute(Caller::Account(4), request.clone()).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(cash(&s, 4), before_cash);
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .construction
            .unwrap()
            .next_effect,
        project.next_effect
    );
    s.checkpoint().unwrap();
    let archived = serde_json::to_value(s.receipt(op).unwrap().unwrap()).unwrap();
    assert_eq!(archived["actions"][0]["kind"], json!("construction"));
    drop(s);

    let mut s = Store::open(&path, RULES).unwrap();
    let archived_after_cold_open = s.receipt(op).unwrap().unwrap();
    assert!(matches!(
        archived_after_cold_open.actions[0],
        Some(Action::Construction { command: C::Start })
    ));
    let replay = s.execute(Caller::Account(4), request).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(s.committed_sequence(), 5);
    assert_eq!(cash(&s, 4), before_cash);
    let project = s.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(project.site_token, Some(token));
    assert_eq!(
        s.engine()
            .unwrap()
            .resources()
            .iter()
            .find(|r| r.id == 80)
            .unwrap()
            .occupied,
        1
    );
}

#[test]
fn construction_work_money_and_materials_commit_once_across_wal_lost_response_and_cold_retry() {
    const END_OF_SHIFT: u64 = 17 * 3_600;
    let t = Temp::new();
    let path = t.world();
    let mut s = create(&path);
    approvals(&mut s);
    let start = start_id(&s);
    call(&mut s, Caller::Account(4), C::Start, start);
    ordinary_action(
        &mut s,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance(&mut s, END_OF_SHIFT);

    let work_id = start_id(&s);
    let sequence_before_work = s.committed_sequence();
    let request = env(
        &s,
        work_id,
        C::Work {
            at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
            elapsed_seconds: END_OF_SHIFT as u32,
            worker_seconds: 28_800,
        },
    );
    let owner_cash_before = cash(&s, 4);
    let contractor_cash_before = cash(&s, 3);
    let stock_before = s
        .engine()
        .unwrap()
        .world()
        .view()
        .stocks
        .into_iter()
        .find(|stock| stock.id == 1)
        .unwrap();
    let lost_response = s.execute_with_observer(Caller::Account(3), request.clone(), |point| {
        if point == WritePoint::Applied {
            return Err(io::Error::other("simulated lost work response"));
        }
        Ok(())
    });
    assert!(lost_response.is_err());
    drop(s);

    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(cash(&s, 4), owner_cash_before - 28_800);
    assert_eq!(cash(&s, 3), contractor_cash_before + 28_800);
    let project = s.engine().unwrap().world().view().construction.unwrap();
    assert_eq!(project.completed_worker_seconds, 28_800);
    assert_eq!(project.spent, Money::new(29_200).unwrap());
    assert_eq!(project.phase, mycity_core::construction::Phase::Building);
    let stock_after = s
        .engine()
        .unwrap()
        .world()
        .view()
        .stocks
        .into_iter()
        .find(|stock| stock.id == 1)
        .unwrap();
    assert_eq!(
        stock_after.quantity.value(),
        stock_before.quantity.value() - 1_000
    );
    assert_eq!(stock_after.basis.value(), stock_before.basis.value() - 400);

    let retry = s.execute(Caller::Account(3), request.clone()).unwrap();
    assert!(retry.has_persistent_receipt);
    assert_eq!(cash(&s, 4), owner_cash_before - 28_800);
    assert_eq!(cash(&s, 3), contractor_cash_before + 28_800);
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .construction
            .unwrap()
            .completed_worker_seconds,
        28_800
    );
    assert_eq!(s.committed_sequence(), sequence_before_work + 1);
    drop(s);

    let mut s = Store::open(&path, RULES).unwrap();
    assert!(matches!(
        s.receipt(work_id).unwrap().unwrap().actions[0],
        Some(Action::Construction {
            command: C::Work { .. }
        })
    ));
    assert_eq!(cash(&s, 4), owner_cash_before - 28_800);
    assert_eq!(cash(&s, 3), contractor_cash_before + 28_800);
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .construction
            .unwrap()
            .completed_worker_seconds,
        28_800
    );
    let stock = s
        .engine()
        .unwrap()
        .world()
        .view()
        .stocks
        .into_iter()
        .find(|stock| stock.id == 1)
        .unwrap();
    assert_eq!(stock.quantity.value(), 599_000);
    assert_eq!(stock.basis.value(), 239_600);
    assert_eq!(s.committed_sequence(), sequence_before_work + 1);
}

#[test]
fn v106_store_wal_checkpoint_and_receipt_reopen_with_identical_identity() {
    const LEGACY_ENGINE_STATE: &str =
        "2188baf12c0adf263c33bb3cf233e8005d73394aedb7a12bfb23bc4542cda0e6";
    const LEGACY_JOURNAL: &str = "cb1a68bcda51ec4b1bb2932380d5e2b4ad07f02c91fb5b823c2ee8d516ee1b90";
    let temp = Temp::new();
    let path = temp.world();
    fs::create_dir(&path).unwrap();
    for (name, bytes) in [
        ("genesis.json", V106_GENESIS),
        ("CURRENT.json", V106_CURRENT),
        ("root-0.json", V106_ROOT_0),
        ("root-1.json", V106_ROOT_1),
        ("wal.bin", V106_WAL),
    ] {
        fs::write(path.join(name), bytes).unwrap();
    }
    let mut store = Store::open(&path, V106_RULES).unwrap();
    let (view, journal_hash, engine_hash, world_hash) = {
        let engine = store.engine().unwrap();
        let view = engine.world().view();
        let journal_hash = mycity_core::sha256(&serde_json::to_vec(engine.journal()).unwrap());
        (
            view,
            journal_hash,
            engine.state_hash(),
            engine.world().state_hash(),
        )
    };
    assert_eq!(engine_hash, LEGACY_ENGINE_STATE);
    let legacy_probe: serde_json::Value = serde_json::from_slice(V106_PROBE).unwrap();
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        legacy_probe["storage_probe"]["world"]
    );
    assert!(!world_hash.is_empty());
    let store_hash_before_replay = store.state_hash().unwrap();
    assert_eq!(store.committed_sequence(), 2);
    assert_eq!(journal_hash, LEGACY_JOURNAL);
    assert_eq!(
        view.accounts
            .iter()
            .map(|a| a.cash.value())
            .collect::<Vec<_>>(),
        [1_000_000, 190_000, 600_000, 1_210_000]
    );
    assert_eq!(view.stocks[0].quantity.value(), 600_000);
    assert_eq!(view.stocks[0].basis.value(), 240_000);
    assert_eq!(store.engine().unwrap().resources()[0].occupied, 8);

    let (family, company, supplier) = {
        let engine = store.engine().unwrap();
        (
            engine.world().account_ref(2).unwrap(),
            engine.world().account_ref(3).unwrap(),
            engine.world().account_ref(4).unwrap(),
        )
    };
    let retry = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: 1,
        expected_revision: 0,
        actions: vec![
            Action::Protect {
                account: family,
                amount: Money::new(10_000).unwrap(),
            },
            Action::Transfer {
                from: company,
                to: supplier,
                amount: Money::new(10_000).unwrap(),
            },
            Action::ReserveCapacity {
                resource: 1,
                units: 8,
            },
        ],
    };
    let replay = store.execute(Caller::System, retry).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok());
    assert_eq!(store.engine().unwrap().state_hash(), LEGACY_ENGINE_STATE);
    assert_eq!(store.state_hash().unwrap(), store_hash_before_replay);
}
