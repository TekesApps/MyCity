use mycity_core::{
    construction::{
        EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material, Project,
        OPERATION_PREFIX,
    },
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
    let e = s.engine().unwrap();
    let view = e.world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![Action::Construction { command }],
    }
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
