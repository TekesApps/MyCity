//! Store-level recovery cases for clearing an unfinished, never-operational project.
//! These tests intentionally use the real Store, Engine, timeline, leases and WAL.
use mycity_core::{
    construction::{
        AbortClearanceMaterialRule, AbortClearanceSpec, EngineCommand as C, Gate, GateCategory,
        GateDecision, Kind, Material, Phase, Project, MAX_MATERIALS, OPERATION_PREFIX,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const RULES: &str = "construction-engine-v1";
const HOUR: u64 = 3_600;
static NEXT: AtomicU64 = AtomicU64::new(0);
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(2_000_000);

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("mycity-clearance-{}-{n}", std::process::id()));
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
                "unfinished clearance artifacts retained at {}",
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
    let mut rules = [AbortClearanceMaterialRule::default(); MAX_MATERIALS];
    rules[0] = AbortClearanceMaterialRule {
        stock_id: 1,
        quantity_per_waste_capacity_unit: 500,
        worker_seconds_per_thousand_quantity: 3_600,
    };
    project.abort_clearance_spec = Some(AbortClearanceSpec {
        base_worker_seconds: 600,
        material_rules: rules,
    });
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 3;
    project.waste_receiver_resource = 82;
    // Two capacity units cover the full 1,000-unit material contract. A zero-waste
    // clearance below comes from no consumed material, not an invalid empty profile.
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

fn envelope(s: &Store, operation_id: u64, command: C) -> Envelope {
    let view = s.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![Action::Construction { command }],
    }
}
fn operation_id() -> u64 {
    NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed)
}
fn command(s: &mut Store, caller: Caller, command: C) {
    let request = envelope(s, operation_id(), command);
    let response = s.execute(caller, request).unwrap();
    assert!(
        response.model.is_ok(),
        "command failed: {:?}",
        response.model
    );
}
fn cash(s: &Store, account: u32) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|entry| entry.id == account)
        .unwrap()
        .cash
        .value()
}
fn project(s: &Store) -> Project {
    s.engine().unwrap().world().view().construction.unwrap()
}
fn resource(s: &Store, id: u32) -> mycity_core::transaction::Resource {
    *s.engine()
        .unwrap()
        .resources()
        .iter()
        .find(|entry| entry.id == id)
        .unwrap()
}
fn advance(s: &mut Store, target: u64) {
    for _ in 0..100 {
        let request = {
            let view = s.engine().unwrap().world().view();
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: operation_id(),
                expected_revision: view.revision,
                actions: vec![Action::Timeline {
                    command: T::Advance {
                        target,
                        limit: 8_192,
                    },
                }],
            }
        };
        let response = s.execute(Caller::System, request).unwrap();
        assert!(response.model.is_ok());
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
    panic!("timeline did not reach target");
}
fn start_original_building(s: &mut Store) {
    command(s, Caller::Account(4), C::File);
    command(
        s,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 11,
        },
    );
    command(
        s,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 12,
        },
    );
    command(s, Caller::Account(4), C::Prepare);
    let operation = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(s).next_effect);
    let request = envelope(s, operation, C::Start);
    assert!(s
        .execute(Caller::Account(4), request)
        .unwrap()
        .model
        .is_ok());
    command(s, Caller::Account(3), C::Pause);
}

/// Establishes the requested 13h half-built fixture using the real timeline and work action.
fn half_built_store(s: &mut Store) {
    start_original_building(s);
    command(s, Caller::Account(3), C::Resume);
    let enable = {
        let view = s.engine().unwrap().world().view();
        Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: operation_id(),
            expected_revision: view.revision,
            actions: vec![Action::Timeline { command: T::Enable }],
        }
    };
    assert!(s.execute(Caller::System, enable).unwrap().model.is_ok());
    advance(s, 13 * HOUR);
    let op = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(s).next_effect);
    let request = envelope(
        s,
        op,
        C::Work {
            at: GameTime::new((13 * HOUR) as i64).unwrap(),
            elapsed_seconds: (13 * HOUR) as u32,
            worker_seconds: 14_400,
        },
    );
    assert!(s
        .execute(Caller::Account(3), request)
        .unwrap()
        .model
        .is_ok());
    command(s, Caller::Account(3), C::Pause);
    let p = project(s);
    assert_eq!(p.phase, Phase::Paused);
    assert_eq!(p.kind, Kind::Build);
    assert_eq!(p.completed_worker_seconds, 14_400);
    assert_eq!(p.materials[0].consumed.value(), 500);
    assert_eq!(p.started_at, Some(GameTime::ZERO));
}

fn approve_clearance(s: &mut Store) {
    command(
        s,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    );
    command(
        s,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 21,
        },
    );
    command(
        s,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 22,
        },
    );
    command(s, Caller::Account(4), C::Prepare);
}

#[test]
fn unfinished_project_clearance_start_waste_and_close_recover_without_duplicate_effects() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = create(&path);
    half_built_store(&mut s);
    let before = project(&s);
    let cash_owner = cash(&s, 4);
    let cash_worker = cash(&s, 3);
    let stock_before = s
        .engine()
        .unwrap()
        .world()
        .view()
        .stocks
        .into_iter()
        .next()
        .unwrap();
    let site_token = before.site_token.unwrap();
    assert_eq!(resource(&s, 80).occupied, 1);
    assert_eq!(resource(&s, 81).capacity, 2);
    assert!(!before.operating_published);
    approve_clearance(&mut s);

    let start_id = operation_id();
    let start_request = envelope(&s, start_id, C::Start);
    let seq_before_start = s.committed_sequence();
    let lost = s.execute_with_observer(Caller::Account(4), start_request.clone(), |point| {
        if point == WritePoint::Applied {
            Err(io::Error::other("clearance start response lost"))
        } else {
            Ok(())
        }
    });
    assert!(lost.is_err());
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), seq_before_start + 1);
    let after_start = project(&s);
    assert_eq!(after_start.kind, Kind::Demolish);
    assert_eq!(after_start.phase, Phase::Demolition);
    assert!(!after_start.operating_published);
    assert_eq!(after_start.operating_published_at, None);
    assert_eq!(after_start.operating_withdrawn_at, None);
    assert_eq!(after_start.site_token, Some(site_token));
    assert_eq!(after_start.completed_worker_seconds, 0);
    assert_eq!(
        after_start.abort_clearance.unwrap().original_kind,
        Kind::Build
    );
    assert_eq!(
        after_start
            .abort_clearance
            .unwrap()
            .original_planned_worker_seconds,
        28_800
    );
    assert_eq!(
        after_start
            .abort_clearance
            .unwrap()
            .original_completed_worker_seconds,
        14_400
    );
    assert_eq!(
        after_start.abort_clearance.unwrap().original_spent,
        before.spent
    );
    assert_eq!(
        after_start.abort_clearance.unwrap().original_consumed[0].value(),
        500
    );
    assert_eq!(
        after_start
            .abort_clearance
            .unwrap()
            .original_basis_remainders[0],
        before.materials[0].basis_remainder
    );
    assert_eq!(
        after_start.abort_clearance.unwrap().started_at,
        Some(GameTime::ZERO)
    );
    assert_eq!(
        after_start.abort_clearance.unwrap().required_worker_seconds,
        2_400
    );
    assert_eq!(after_start.abort_clearance.unwrap().waste_units, 1);
    assert_eq!(cash(&s, 4), cash_owner);
    assert_eq!(cash(&s, 3), cash_worker);
    let stock_after_start = s
        .engine()
        .unwrap()
        .world()
        .view()
        .stocks
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(stock_after_start.quantity, stock_before.quantity);
    assert_eq!(stock_after_start.basis, stock_before.basis);
    assert_eq!(resource(&s, 80).occupied, 1);
    assert_eq!(resource(&s, 81).capacity, 2);
    assert_eq!(resource(&s, 82).occupied, 0);
    let state_after_start = s.state_hash().unwrap();
    let replay = s.execute(Caller::Account(4), start_request).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(s.state_hash().unwrap(), state_after_start);
    assert_eq!(s.committed_sequence(), seq_before_start + 1);

    advance(&mut s, 13 * HOUR + 2_400);
    let work = C::Work {
        at: GameTime::new((13 * HOUR + 2_400) as i64).unwrap(),
        elapsed_seconds: 2_400,
        worker_seconds: 2_400,
    };
    let work_id = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(&s).next_effect);
    let work_request = envelope(&s, work_id, work);
    assert!(s
        .execute(Caller::Account(3), work_request)
        .unwrap()
        .model
        .is_ok());
    assert_eq!(project(&s).completed_worker_seconds, 2_400);
    assert_eq!(
        project(&s).abort_clearance.unwrap().required_worker_seconds,
        2_400
    );
    assert_eq!(
        s.engine().unwrap().world().view().stocks[0].quantity,
        stock_before.quantity
    );
    assert_eq!(resource(&s, 81).capacity, 2);
    let cash_before_waste = cash(&s, 4);
    let receiver_before_waste = cash(&s, 3);
    let waste_id = operation_id();
    let waste_request = envelope(&s, waste_id, C::AcceptWaste);
    let seq_before_waste = s.committed_sequence();
    let lost = s.execute_with_observer(Caller::Account(3), waste_request.clone(), |point| {
        if point == WritePoint::Applied {
            Err(io::Error::other("clearance waste response lost"))
        } else {
            Ok(())
        }
    });
    assert!(lost.is_err());
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), seq_before_waste + 1);
    assert_eq!(cash(&s, 4), cash_before_waste - 250);
    assert_eq!(cash(&s, 3), receiver_before_waste + 250);
    assert_eq!(resource(&s, 82).occupied, 1);
    let waste_state = s.state_hash().unwrap();
    let replay = s.execute(Caller::Account(3), waste_request).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(s.committed_sequence(), seq_before_waste + 1);
    assert_eq!(s.state_hash().unwrap(), waste_state);
    assert_eq!(cash(&s, 4), cash_before_waste - 250);
    assert_eq!(resource(&s, 82).occupied, 1);

    // Transaction deduplication alone is insufficient: a new operation id must
    // not receive the same project's waste and charge the owner a second time.
    let second_accept = envelope(&s, operation_id(), C::AcceptWaste);
    let rejected = s.execute(Caller::Account(3), second_accept).unwrap();
    assert_eq!(
        rejected.model,
        Err(mycity_core::transaction::TxError::Construction(
            mycity_core::construction::Error::WrongPhase
        ))
    );
    assert_eq!(s.committed_sequence(), seq_before_waste + 2);
    assert_eq!(s.state_hash().unwrap(), waste_state);
    assert_eq!(cash(&s, 4), cash_before_waste - 250);
    assert_eq!(resource(&s, 82).occupied, 1);

    let close_id = operation_id();
    let close_request = envelope(&s, close_id, C::Close);
    let close_seq = s.committed_sequence();
    let lost = s.execute_with_observer(Caller::Account(4), close_request.clone(), |point| {
        if point == WritePoint::Applied {
            Err(io::Error::other("clearance close response lost"))
        } else {
            Ok(())
        }
    });
    assert!(lost.is_err());
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), close_seq + 1);
    assert_eq!(project(&s).phase, Phase::Closed);
    assert_eq!(project(&s).site_token, None);
    assert_eq!(resource(&s, 80).occupied, 0);
    assert_eq!(resource(&s, 82).occupied, 1);
    assert_eq!(resource(&s, 81).capacity, 2);
    let closed_hash = s.state_hash().unwrap();
    let replay = s.execute(Caller::Account(4), close_request).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(s.state_hash().unwrap(), closed_hash);
    assert_eq!(s.committed_sequence(), close_seq + 1);
    s.checkpoint().unwrap();
    drop(s);

    let cold = Store::open(&path, RULES).unwrap();
    assert_eq!(cold.state_hash().unwrap(), closed_hash);
    assert_eq!(project(&cold).phase, Phase::Closed);
    assert_eq!(project(&cold).site_token, None);
    assert_eq!(resource(&cold, 80).occupied, 0);
    assert_eq!(resource(&cold, 82).occupied, 1);
    assert_eq!(resource(&cold, 81).capacity, 2);
    assert_eq!(cash(&cold, 4), cash_before_waste - 250);
}

#[test]
fn precommit_clearance_waste_failure_cold_recovers_unmodified_then_charges_once_on_retry() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = create(&path);
    half_built_store(&mut s);
    approve_clearance(&mut s);
    command(&mut s, Caller::Account(4), C::Start);
    advance(&mut s, 13 * HOUR + 2_400);
    let work_id = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(&s).next_effect);
    let work = envelope(
        &s,
        work_id,
        C::Work {
            at: GameTime::new((13 * HOUR + 2_400) as i64).unwrap(),
            elapsed_seconds: 2_400,
            worker_seconds: 2_400,
        },
    );
    assert!(s.execute(Caller::Account(3), work).unwrap().model.is_ok());

    let waste_id = operation_id();
    let waste = envelope(&s, waste_id, C::AcceptWaste);
    let sequence_before = s.committed_sequence();
    let hash_before = s.state_hash().unwrap();
    let payer_before = cash(&s, 4);
    let receiver_before = cash(&s, 3);
    let occupied_before = resource(&s, 82).occupied;
    let site_occupied_before = resource(&s, 80).occupied;
    let failed = s.execute_with_observer(Caller::Account(3), waste.clone(), |point| {
        if point == WritePoint::Prepared {
            Err(io::Error::other("reject precommit waste write"))
        } else {
            Ok(())
        }
    });
    assert!(failed.is_err());
    drop(s);

    let mut recovered = Store::open(&path, RULES).unwrap();
    assert_eq!(recovered.committed_sequence(), sequence_before);
    assert_eq!(recovered.state_hash().unwrap(), hash_before);
    assert_eq!(cash(&recovered, 4), payer_before);
    assert_eq!(cash(&recovered, 3), receiver_before);
    assert_eq!(resource(&recovered, 82).occupied, occupied_before);
    assert_eq!(resource(&recovered, 80).occupied, site_occupied_before);
    assert!(!project(&recovered).waste_accepted);

    let retry = recovered
        .execute(Caller::Account(3), waste.clone())
        .unwrap();
    assert!(retry.model.is_ok());
    assert_eq!(recovered.committed_sequence(), sequence_before + 1);
    assert_eq!(cash(&recovered, 4), payer_before - 250);
    assert_eq!(cash(&recovered, 3), receiver_before + 250);
    assert_eq!(resource(&recovered, 82).occupied, occupied_before + 1);
    assert_eq!(resource(&recovered, 80).occupied, site_occupied_before);
    assert!(project(&recovered).waste_accepted);
    let committed_hash = recovered.state_hash().unwrap();
    let duplicate = recovered.execute(Caller::Account(3), waste).unwrap();
    assert!(duplicate.has_persistent_receipt);
    assert_eq!(recovered.committed_sequence(), sequence_before + 1);
    assert_eq!(recovered.state_hash().unwrap(), committed_hash);
    assert_eq!(cash(&recovered, 4), payer_before - 250);
    assert_eq!(resource(&recovered, 82).occupied, occupied_before + 1);
}

#[test]
fn no_waste_still_requires_base_clearance_labor_and_does_not_charge_or_fake_receipt_units() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = create(&path);
    start_original_building(&mut s);
    approve_clearance(&mut s);
    command(&mut s, Caller::Account(4), C::Start);
    assert_eq!(
        project(&s).abort_clearance.unwrap().required_worker_seconds,
        600
    );
    assert_eq!(project(&s).abort_clearance.unwrap().waste_units, 0);
    let before = cash(&s, 4);
    let view = s.engine().unwrap().world().view();
    let enable = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: operation_id(),
        expected_revision: view.revision,
        actions: vec![Action::Timeline { command: T::Enable }],
    };
    assert!(s.execute(Caller::System, enable).unwrap().model.is_ok());
    // Clearance started at zero; only the 600 seconds after the 09:00 shift
    // start are effective work, but elapsed time includes the preceding night.
    let work_at = 9 * HOUR + 600;
    advance(&mut s, work_at);
    let op = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(&s).next_effect);
    let request = envelope(
        &s,
        op,
        C::Work {
            at: GameTime::new(work_at as i64).unwrap(),
            elapsed_seconds: work_at as u32,
            worker_seconds: 600,
        },
    );
    assert!(s
        .execute(Caller::Account(3), request)
        .unwrap()
        .model
        .is_ok());
    let accept = envelope(&s, operation_id(), C::AcceptWaste);
    let result = s.execute(Caller::Account(3), accept).unwrap();
    assert!(result.model.is_ok());
    assert!(result
        .model
        .unwrap()
        .created_tokens
        .iter()
        .all(Option::is_none));
    assert_eq!(cash(&s, 4), before - 600);
    assert_eq!(resource(&s, 82).occupied, 0);
    assert!(project(&s).waste_accepted);
}

#[test]
fn full_receiver_capacity_preserves_half_built_site_without_fee_or_clearance() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = create(&path);
    half_built_store(&mut s);
    approve_clearance(&mut s);
    command(&mut s, Caller::Account(4), C::Start);
    advance(&mut s, 13 * HOUR + 2_400);
    let op = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(&s).next_effect);
    let work = envelope(
        &s,
        op,
        C::Work {
            at: GameTime::new((13 * HOUR + 2_400) as i64).unwrap(),
            elapsed_seconds: 2_400,
            worker_seconds: 2_400,
        },
    );
    assert!(s.execute(Caller::Account(3), work).unwrap().model.is_ok());
    let reserve = {
        let e = s.engine().unwrap();
        Envelope {
            world: e.world().view().scope.world(),
            branch: e.world().view().scope.branch(),
            operation_id: operation_id(),
            expected_revision: e.world().view().revision,
            actions: vec![Action::ReserveCapacity {
                resource: 82,
                units: 10,
            }],
        }
    };
    let token = s
        .execute(Caller::Account(2), reserve)
        .unwrap()
        .model
        .unwrap()
        .created_tokens[0]
        .unwrap();
    let occupy = {
        let e = s.engine().unwrap();
        Envelope {
            world: e.world().view().scope.world(),
            branch: e.world().view().scope.branch(),
            operation_id: operation_id(),
            expected_revision: e.world().view().revision,
            actions: vec![Action::OccupyCapacity { token }],
        }
    };
    assert!(s.execute(Caller::Account(2), occupy).unwrap().model.is_ok());
    let cash_before = cash(&s, 4);
    let before_seq = s.committed_sequence();
    let waste = envelope(&s, operation_id(), C::AcceptWaste);
    let blocked = s.execute(Caller::Account(3), waste).unwrap();
    assert_eq!(
        blocked.model,
        Err(mycity_core::transaction::TxError::Capacity)
    );
    assert_eq!(cash(&s, 4), cash_before);
    assert_eq!(s.committed_sequence(), before_seq + 1);
    assert_eq!(project(&s).phase, Phase::Demolition);
    assert!(!project(&s).waste_accepted);
    assert_eq!(resource(&s, 80).occupied, 1);
    assert_eq!(resource(&s, 82).occupied, 10);
}

#[test]
fn insufficient_cash_preserves_site_and_has_no_disposal_fee() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = create(&path);
    half_built_store(&mut s);
    approve_clearance(&mut s);
    command(&mut s, Caller::Account(4), C::Start);
    advance(&mut s, 13 * HOUR + 2_400);
    let op = OPERATION_PREFIX | (7_u64 << 32) | u64::from(project(&s).next_effect);
    let work = envelope(
        &s,
        op,
        C::Work {
            at: GameTime::new((13 * HOUR + 2_400) as i64).unwrap(),
            elapsed_seconds: 2_400,
            worker_seconds: 2_400,
        },
    );
    assert!(s.execute(Caller::Account(3), work).unwrap().model.is_ok());
    let account_ref = s.engine().unwrap().world().account_ref(4).unwrap();
    let recipient_ref = s.engine().unwrap().world().account_ref(2).unwrap();
    let cash_before_drain = cash(&s, 4);
    let drain = Envelope {
        world: s.engine().unwrap().world().view().scope.world(),
        branch: s.engine().unwrap().world().view().scope.branch(),
        operation_id: operation_id(),
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![Action::Transfer {
            from: account_ref,
            to: recipient_ref,
            amount: Money::new(cash_before_drain - 1).unwrap(),
        }],
    };
    assert!(s.execute(Caller::Account(4), drain).unwrap().model.is_ok());
    let cash_before_waste = cash(&s, 4);
    let seq_before_waste = s.committed_sequence();
    let waste = envelope(&s, operation_id(), C::AcceptWaste);
    let blocked = s.execute(Caller::Account(3), waste).unwrap();
    assert_eq!(
        blocked.model,
        Err(mycity_core::transaction::TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert_eq!(cash(&s, 4), cash_before_waste);
    assert_eq!(cash(&s, 4), 1);
    assert_eq!(s.committed_sequence(), seq_before_waste + 1);
    assert_eq!(project(&s).phase, Phase::Demolition);
    assert!(!project(&s).waste_accepted);
    assert_eq!(resource(&s, 80).occupied, 1);
    assert_eq!(resource(&s, 82).occupied, 0);
}
