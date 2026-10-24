//! OpeningInput -> original Store -> original Housing/Timeline -> receipt-authenticated restore.
//! Synthetic lease/need registration validates simulation wiring, not real service delivery.
use mycity_core::{
    housing::{genesis_lease_offer_sha256, lease_terms_sha256, GenesisLeaseOffer, LeaseTerms},
    opening_loader::{InputOrigin, OpeningInput},
    schedule::Phase,
    service_needs as needs,
    timeline::Command as TimelineCommand,
    transaction::{Action, Caller, Engine, EngineSlot, EngineWorkspace, Envelope, Resource},
    units::{GameTime, Money},
    timeline::Cycle,
};
use mycity_storage::Store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const APPROVED_BASE: &str = "/Volumes/HP P900/dev/MyCity/work";
const ROOT_ENV: &str = "MYCITY_OPENING_SERVICE_TEST_ROOT";
const PROBE_ENV: &str = "MYCITY_OPENING_SERVICE_PROBE";
const RULES: &str = "issue120-opening-service-v1";
const VENUE_OPENING: &[u8] =
    include_bytes!("fixtures/opening-venue-store/managed-people-v1-opening.json");
const NEEDS_PROFILE: &[u8] = include_bytes!(
    "fixtures/opening-venue-store/managed-people-service-needs-v1-profile.json"
);
const EIGHT_AM: u64 = needs::EIGHT_AM;
const RESOURCES: &[(u32, u32)] = &[(501, 4), (502, 4), (503, 5)];
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Probe {
    world: PathBuf,
    expected: Value,
    entry_operation: u64,
    offer_sha256: [u8; 32],
    checkpoint_after_open: bool,
}

struct RetainedRun {
    path: PathBuf,
}

impl RetainedRun {
    fn new() -> Self {
        let approved = fs::canonicalize(APPROVED_BASE).expect("HP P900 work mount required");
        assert_eq!(approved, Path::new(APPROVED_BASE));
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_ne!(
                fs::metadata("/Volumes/HP P900").unwrap().dev(),
                fs::metadata("/Volumes").unwrap().dev(),
                "HP P900 must be mounted, not a local fallback",
            );
        }
        let configured = PathBuf::from(
            std::env::var_os(ROOT_ENV).expect("set an existing unique HP P900 work directory"),
        );
        assert!(configured.is_absolute());
        let root = fs::canonicalize(configured).expect("configured evidence root must exist");
        assert!(root.is_dir() && root != approved && root.starts_with(&approved));
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "opening-service-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("reserve a new retained evidence directory");
        println!("retained opening/service evidence: {}", path.display());
        Self { path }
    }

    fn world(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write_json(&self, name: &str, value: &Value) {
        let bytes = serde_json::to_vec_pretty(value).unwrap();
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(self.path.join(name))
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
    }
}

fn offer() -> GenesisLeaseOffer {
    let terms = LeaseTerms {
        lease_id: 901,
        clause_version: 1,
        contract_source_id: 9101,
        landlord_authorization_id: 9201,
        tenant_consent_id: 9301,
        rent_per_cycle: Money::new(1_000).unwrap(),
        cycle: Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: 3,
        starts_at: GameTime::ZERO,
    };
    GenesisLeaseOffer {
        offer_id: 901,
        world_id: 1,
        branch_id: 1,
        unit_id: 71,
        building_id: 70,
        owner_account: 1,
        tenant_account: 2,
        household_id: 7,
        valid_from: GameTime::ZERO,
        valid_until: GameTime::new((400 * needs::DAY) as i64).unwrap(),
        asset_source_id: 7002,
        authority_source_id: 8001,
        contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
        lease_id: terms.lease_id,
        clause_version: terms.clause_version,
        rent_per_cycle: terms.rent_per_cycle,
        cycle: terms.cycle,
        deposit: terms.deposit,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: lease_terms_sha256(terms).unwrap(),
    }
}

fn opening_bytes() -> Vec<u8> {
    let mut opening: Value = serde_json::from_slice(VENUE_OPENING).unwrap();
    let profile: Value = serde_json::from_slice(NEEDS_PROFILE).unwrap();
    opening["bootstrap"]["seed"] = json!(107);
    opening["housing"]["city_id"] = json!(1);
    opening["housing"]["lease_offers"] = json!([offer()]);
    opening["service_needs"] = profile;
    serde_json::to_vec(&opening).unwrap()
}

fn legacy_opening_bytes(service_opening: &[u8]) -> Vec<u8> {
    let mut opening: Value = serde_json::from_slice(service_opening).unwrap();
    opening.as_object_mut().unwrap().remove("service_needs");
    opening["housing"].as_object_mut().unwrap().remove("city_id");
    serde_json::to_vec(&opening).unwrap()
}

fn resources() -> Vec<Resource> {
    RESOURCES
        .iter()
        .map(|(id, capacity)| Resource::new(*id, *capacity).unwrap())
        .collect()
}

fn apply(store: &mut Store, caller: Caller, action: Action) -> Envelope {
    let (scope, revision) = store.engine().unwrap().world().scope_revision();
    let envelope = Envelope {
        world: scope.world(),
        branch: scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: revision,
        actions: vec![action],
    };
    let outcome = store.execute(caller, envelope.clone()).unwrap();
    assert!(
        outcome.has_persistent_receipt && outcome.model.is_ok(),
        "original Store action failed: {outcome:?}"
    );
    envelope
}

fn state(store: &Store) -> Value {
    let engine = store.engine().unwrap();
    json!({
        "state_hash": store.state_hash().unwrap(),
        "registration": engine.service_needs_registration(),
        "entry_receipt_operations": engine.service_needs_receipt_operations(),
        "due_state": engine.service_needs_due_state().unwrap(),
        "timeline": engine.timeline().unwrap().progress,
    })
}

fn assert_entry_receipt(store: &mut Store, operation: u64, offer_hash: [u8; 32]) {
    let record = store
        .receipt(operation)
        .unwrap()
        .expect("original accepted HousingLease receipt must remain available");
    assert_eq!(record.actor, 2);
    assert!(record.outcome.is_ok());
    assert!(matches!(
        record.actions[0],
        Some(Action::HousingLease { offer_id: 901, offer_sha256 }) if offer_sha256 == offer_hash
    ));
}

fn spawn_probe(run: &RetainedRun, probe: Probe, name: &str) {
    let stdout = run.path.join(format!("{name}-stdout.log"));
    let stderr = run.path.join(format!("{name}-stderr.log"));
    run.write_json(
        &format!("{name}-launch.json"),
        &json!({ "probe": probe, "stdout": stdout, "stderr": stderr, "timeout_seconds": 60 }),
    );
    let stdout_file = OpenOptions::new().create_new(true).write(true).open(&stdout).unwrap();
    let stderr_file = OpenOptions::new().create_new(true).write(true).open(&stderr).unwrap();
    let encoded = serde_json::to_string(&probe).unwrap();
    let spawned = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "opening_service_needs_cold_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PROBE_ENV, encoded)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            run.write_json(&format!("{name}-result.json"), &json!({
                "spawn_error": error.to_string(), "stdout": stdout, "stderr": stderr,
            }));
            panic!("cold Store probe failed to spawn; evidence retained at {}", run.path.display());
        }
    };
    let pid = child.id();
    run.write_json(&format!("{name}-started.json"), &json!({ "pid": pid }));
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut timed_out = false;
    let mut wait_error = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                timed_out = true;
                if let Err(error) = child.kill() {
                    wait_error = Some(format!("timeout kill: {error}"));
                }
                break child.wait().ok();
            }
            Err(error) => {
                let kill_error = child.kill().err().map(|error| error.to_string());
                wait_error = Some(match kill_error {
                    Some(kill_error) => format!("try_wait: {error}; kill: {kill_error}"),
                    None => format!("try_wait: {error}; child kill requested"),
                });
                break child.wait().ok();
            }
        }
    };
    run.write_json(&format!("{name}-result.json"), &json!({
        "pid": pid,
        "exit_code": status.as_ref().and_then(|status| status.code()),
        "success": status.as_ref().is_some_and(|status| status.success()),
        "timed_out": timed_out,
        "wait_error": wait_error,
        "stdout": stdout,
        "stderr": stderr,
    }));
    assert!(
        !timed_out && wait_error.is_none() && status.is_some_and(|status| status.success()),
        "cold Store probe failed; evidence retained at {}",
        run.path.display()
    );
}

#[test]
fn opening_input_service_entry_partial_p5_cold_receipt_auth_and_slot_reuse() {
    let run = RetainedRun::new();
    let bytes = opening_bytes();
    let input = OpeningInput::from_json(&bytes).unwrap();
    assert_eq!(input.source().origin(), InputOrigin::SuppliedBytes);
    assert_eq!(input.source().bytes(), bytes);
    let service_world = run.world("service-world");
    let workspace = EngineWorkspace::new().unwrap();
    let before = workspace.identity().unwrap();
    let mut store = Store::create_opening_input_in_workspace_with_observer(
        &service_world,
        &input,
        resources(),
        RULES,
        workspace,
        |_| Ok(()),
    )
    .unwrap();
    let after = store.model_workspace_identity().unwrap();
    assert_eq!(after.initial.world, before.initial.world);
    assert_eq!(after.initial.world_tables, before.initial.world_tables);
    assert_eq!(after.initial.engine_tables, before.initial.engine_tables);
    assert_eq!(after.initial.timeline_tables, before.initial.timeline_tables);
    assert_eq!(after.initial.background_tables, before.initial.background_tables);
    assert_eq!(after.initial.preparation_backing, before.initial.preparation_backing);
    assert_eq!(after.restore, before.restore);
    assert_eq!(after.candidate, before.candidate);
    assert_eq!(after.codec, before.codec);
    let registration = store.engine().unwrap().service_needs_registration().unwrap();
    assert!(store.engine().unwrap().service_needs_is_authenticated());
    assert!(store.engine().unwrap().service_needs_due_state().unwrap().is_some());

    apply(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    let lease_hash = genesis_lease_offer_sha256(offer());
    let entry_envelope = apply(
        &mut store,
        Caller::Account(2),
        Action::HousingLease {
            offer_id: 901,
            offer_sha256: lease_hash,
        },
    );
    let entry_operation = store.engine().unwrap().service_needs_receipt_operations()[0]
        .expect("accepted HousingLease entry must retain its originating operation");
    assert_eq!(entry_operation, entry_envelope.operation_id);
    assert_entry_receipt(&mut store, entry_operation, lease_hash);

    let mut partial_operation = None;
    for _ in 0..8192 {
        let envelope = apply(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target: EIGHT_AM,
                    limit: 1,
                },
            },
        );
        if store.engine().unwrap().service_needs_due_state().unwrap().unwrap().pending.is_some() {
            partial_operation = Some(envelope.operation_id);
            break;
        }
    }
    let partial_operation = partial_operation.expect("Timeline must expose the bounded partial P5 page");
    let timeline = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(timeline.phase, Some(Phase::P5));
    assert_eq!(timeline.target, Some(EIGHT_AM));
    assert!(timeline.last_complete < EIGHT_AM);
    let partial_state = state(&store);
    let partial = store.engine().unwrap().service_needs_due_state().unwrap().unwrap();
    assert_eq!(partial.pending.unwrap().at, EIGHT_AM);
    let service_checkpoint = store.engine().unwrap().export_checkpoint().unwrap();
    drop(store);

    let expected = partial_state.clone();
    spawn_probe(
        &run,
        Probe {
            world: service_world.clone(),
            expected: expected.clone(),
            entry_operation,
            offer_sha256: lease_hash,
            checkpoint_after_open: true,
        },
        "wal-recovery",
    );
    spawn_probe(
        &run,
        Probe {
            world: service_world.clone(),
            expected: expected.clone(),
            entry_operation,
            offer_sha256: lease_hash,
            checkpoint_after_open: false,
        },
        "checkpoint-recovery",
    );
    run.write_json(
        "service-state.json",
        &json!({
            "entry_operation": entry_operation,
            "partial_timeline_operation": partial_operation,
            "registration": registration,
            "service_checkpoint_bytes": service_checkpoint.len(),
            "expected_state": partial_state,
            "claim": "synthetic service scheduling only; no service delivery or entitlement",
        }),
    );

    let legacy_bytes = legacy_opening_bytes(&bytes);
    let legacy_input = OpeningInput::from_json(&legacy_bytes).unwrap();
    let legacy_world = run.world("legacy-world");
    let legacy = Store::create_opening_input(&legacy_world, &legacy_input, resources(), RULES).unwrap();
    assert!(legacy.engine().unwrap().service_needs_registration().is_none());
    assert_eq!(legacy.engine().unwrap().service_needs_due_state().unwrap(), None);
    let legacy_checkpoint = legacy.engine().unwrap().export_checkpoint().unwrap();
    drop(legacy);

    let mut active = Engine::restore_checkpoint(&legacy_checkpoint).unwrap();
    let mut restore_slot = EngineSlot::new().unwrap();
    restore_slot.restore_into(&service_checkpoint).unwrap();
    let restored_service = restore_slot.engine().unwrap();
    assert_eq!(restored_service.service_needs_registration(), Some(registration));
    assert!(!restored_service.service_needs_is_authenticated(),
        "direct EngineSlot restore is structural and cannot authenticate the entry receipt");
    assert!(restored_service.service_needs_due_state().is_err(),
        "unverified service state must not be presented as a trusted due view");
    assert_eq!(restored_service.service_needs_receipt_operations()[0], Some(entry_operation));
    restore_slot.publish_swap(&mut active).unwrap();
    assert_eq!(active.service_needs_registration(), Some(registration));
    assert!(!active.service_needs_is_authenticated());

    restore_slot.restore_into(&legacy_checkpoint).unwrap();
    restore_slot.publish_swap(&mut active).unwrap();
    assert!(active.service_needs_registration().is_none());
    assert_eq!(active.service_needs_due_state().unwrap(), None,
        "legacy image must clear the prior optional service model");
    let legacy_active = active.export_checkpoint().unwrap();

    let damaged_service_checkpoint = &service_checkpoint[..service_checkpoint.len() - 1];
    assert!(restore_slot.restore_into(damaged_service_checkpoint).is_err());
    assert!(restore_slot.engine().is_err(), "failed restore must leave the candidate slot unpublished");
    assert!(restore_slot.publish_swap(&mut active).is_err(), "an invalid candidate cannot replace active state");
    assert_eq!(active.export_checkpoint().unwrap(), legacy_active,
        "failed service restore must leave the published legacy active image unchanged");

    restore_slot.restore_into(&service_checkpoint).unwrap();
    assert_eq!(restore_slot.engine().unwrap().service_needs_registration(), Some(registration));
    assert!(!restore_slot.engine().unwrap().service_needs_is_authenticated());
    assert!(restore_slot.engine().unwrap().service_needs_due_state().is_err());
    restore_slot.publish_swap(&mut active).unwrap();
    assert_eq!(active.service_needs_registration(), Some(registration));
    assert!(!active.service_needs_is_authenticated(),
        "the original receipt is authenticated by Store::open, never by raw slot restoration");
    assert_eq!(active.service_needs_receipt_operations()[0], Some(entry_operation));
    assert!(active.service_needs_due_state().is_err(),
        "publishing a structurally restored EngineSlot must not claim Store authentication");
}

#[test]
#[ignore = "fresh process Store::open probe launched by the parent test"]
fn opening_service_needs_cold_probe() {
    let Ok(encoded) = std::env::var(PROBE_ENV) else {
        return;
    };
    let probe: Probe = serde_json::from_str(&encoded).unwrap();
    let world = fs::canonicalize(&probe.world).unwrap();
    let root = fs::canonicalize(
        std::env::var_os(ROOT_ENV)
            .expect("parent provides retained evidence root"),
    )
    .unwrap();
    assert!(world.starts_with(&root) && world != root);
    let mut store = Store::open(&world, RULES).unwrap();
    let engine = store.engine().unwrap();
    assert!(engine.service_needs_is_authenticated(), "Store must authenticate original entry receipt");
    assert!(engine.service_needs_due_state().unwrap().is_some(),
        "Store cold open authenticates the original service entry");
    assert_eq!(state(&store), probe.expected);
    assert_entry_receipt(&mut store, probe.entry_operation, probe.offer_sha256);
    if probe.checkpoint_after_open {
        store.checkpoint().unwrap();
        assert_eq!(state(&store), probe.expected);
    }
}
