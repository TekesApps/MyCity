//! Store/Engine recovery gate for the managed_people_v1 opening added in v168.
//! The checked-in opening is a synthetic legal game input from a retained HP P900 run,
//! not evidence of real venue authorization or real-world service delivery.
use mycity_core::{
    assembly::SourceKind,
    opening_loader::{InputOrigin, OpeningInput},
    timeline::Command as TimelineCommand,
    transaction::{Action, Caller, EngineWorkspace, Envelope, Resource},
    units::Money,
    venue_capacity::UseStatus,
    venue_runtime::Command as VenueCommand,
};
use mycity_storage::{CreatePoint, Error, Store};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const APPROVED_BASE: &str = "/Volumes/HP P900/dev/MyCity/work";
const ROOT_ENV: &str = "MYCITY_VENUE_STORE_TEST_ROOT";
const PROBE_ENV: &str = "MYCITY_VENUE_STORE_CHILD_PROBE";
const RULES: &str = "issue118-managed-venue-v1";
const OPENING: &[u8] = include_bytes!("fixtures/opening-venue-store/managed-people-v1-opening.json");
const OPENING_SHA256: &str = "eb8529e57b6b3af3550656a17275dc34b02a0674c31b9e13cbf8756d6dc812b8";
const RUN_BYTES_LIMIT: u64 = 128 * 1024 * 1024;
const CASH: &[(u32, i64, u32)] = &[
    (1, 1_000_000, 1),
    (2, 200_000, 2),
    (3, 600_000, 3),
    (4, 1_200_000, 4),
];
const SOURCES: &[(u32, SourceKind, i64)] = &[
    (1, SourceKind::CashCapital, 1_000_000),
    (2, SourceKind::CashCapital, 200_000),
    (3, SourceKind::CashCapital, 600_000),
    (4, SourceKind::CashCapital, 1_200_000),
    (5, SourceKind::InventoryCapital, 240_000),
];
const RESOURCES: &[(u32, u32)] = &[(501, 4), (502, 4), (503, 5)];
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Probe {
    world: PathBuf,
    phase: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn configured_root() -> PathBuf {
    let approved = fs::canonicalize(APPROVED_BASE).expect("HP P900 work mount required");
    assert_eq!(approved, Path::new(APPROVED_BASE), "approved work root must not redirect");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(
            fs::metadata("/Volumes/HP P900").unwrap().dev(),
            fs::metadata("/Volumes").unwrap().dev(),
            "HP P900 must be mounted, not a local fallback",
        );
    }
    let configured = PathBuf::from(std::env::var_os(ROOT_ENV).expect(
        "set MYCITY_VENUE_STORE_TEST_ROOT to an existing unique directory under HP P900 work",
    ));
    assert!(configured.is_absolute());
    let root = fs::canonicalize(configured).expect("configured test root must already exist");
    assert!(root.is_dir() && root != approved && root.starts_with(&approved));
    root
}

fn file_manifest(path: &Path) -> BTreeMap<String, (u64, String)> {
    fn visit(base: &Path, here: &Path, out: &mut BTreeMap<String, (u64, String)>) {
        for entry in fs::read_dir(here).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "world artifacts may not redirect outside retained root");
            if kind.is_dir() {
                visit(base, &entry.path(), out);
            } else {
                assert!(kind.is_file());
                let mut file = File::open(entry.path()).unwrap();
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 8192];
                let mut size = 0u64;
                loop {
                    let read = file.read(&mut buffer).unwrap();
                    if read == 0 { break; }
                    size += read as u64;
                    hasher.update(&buffer[..read]);
                }
                out.insert(
                    entry.path().strip_prefix(base).unwrap().to_string_lossy().into_owned(),
                    (size, format!("{:x}", hasher.finalize())),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(path, path, &mut out);
    out
}

fn assert_run_budget(root: &Path) {
    let bytes: u64 = file_manifest(root).values().map(|(size, _)| *size).sum();
    assert!(bytes <= RUN_BYTES_LIMIT, "retained test artifacts exceed 128 MiB: {bytes}");
}

struct RetainedRun { root: PathBuf, path: PathBuf }
impl RetainedRun {
    fn new() -> Self {
        let root = configured_root();
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = root.join(format!("issue120-venue-{}-{nonce}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).expect("reserve unique retained run; never reuse or delete it");
        assert!(fs::canonicalize(&path).unwrap().starts_with(&root));
        println!("retained venue Store evidence: {}", path.display());
        let run = Self { root, path };
        run.write_json("case.json", &json!({
            "pid": std::process::id(), "fixture_sha256": digest(OPENING),
            "fixture_declared_sha256": OPENING_SHA256, "fixture_bytes": OPENING.len(),
            "fixture_source": "/Volumes/HP P900/dev/MyCity/work/v118run14-76501/venue/lifecycle-76536-0/opening.json",
            "source_interpretation": "synthetic legal input retained from a successful Store run; not real-world authorization or delivery evidence",
            "validation": "runtime_in_progress; final result in parent/child logs",
        }));
        run
    }
    fn world(&self, name: &str) -> PathBuf { self.path.join(name) }
    fn write_json(&self, name: &str, value: &Value) {
        let bytes = serde_json::to_vec_pretty(value).unwrap();
        let mut file = OpenOptions::new().create_new(true).write(true).open(self.path.join(name)).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        assert_run_budget(&self.root);
    }
}
// Deliberately no Drop cleanup. Failed and incomplete worlds remain available for recovery.

fn make_input(bytes: &[u8]) -> OpeningInput {
    OpeningInput::from_json(bytes).expect("fixed original opening bytes must decode as input wrapper")
}

fn resources() -> Vec<Resource> {
    RESOURCES.iter().map(|(id, capacity)| Resource::new(*id, *capacity).unwrap()).collect()
}

fn envelope(store: &Store, operation_id: u64, action: Action) -> Envelope {
    let (scope, expected_revision) = store.engine().unwrap().world().scope_revision();
    Envelope { world: scope.world(), branch: scope.branch(), operation_id, expected_revision, actions: vec![action] }
}

fn assert_opening_facts(store: &Store) {
    let raw: Value = serde_json::from_slice(OPENING).unwrap();
    let input_accounts: Vec<_> = raw["accounts"].as_array().unwrap().iter()
        .map(|account| (
            account["id"].as_u64().unwrap() as u32,
            account["cash"].as_i64().unwrap(),
            account["source"].as_u64().unwrap() as u32,
        ))
        .collect();
    assert_eq!(input_accounts, CASH.to_vec(), "literal input cash source binding and amounts");
    let engine = store.engine().unwrap();
    let world = engine.world().view();
    for &(id, cash, _input_source) in CASH {
        let account = world.accounts.iter().find(|row| row.id == id).expect("literal opening account");
        assert_eq!(account.cash, Money::new(cash).unwrap());
        assert_eq!(account.opening_source_id, 0,
            "runtime opening_source_id stores charter provenance only; this input has no equity charter");
    }
    let actual_sources: Vec<_> = world.opening_sources.iter().map(|row| (row.id, row.kind, row.amount.value())).collect();
    assert_eq!(actual_sources, SOURCES.to_vec());
    let stock = world.stocks.iter().find(|row| row.id == 1).expect("literal initial inventory lot");
    assert_eq!((stock.owner, stock.quantity.value(), stock.basis.value(), stock.lot_id), (4, 600_000, 240_000, 5));
    assert_eq!((stock.origin_quantity.value(), stock.origin_basis.value(), stock.offset.value()), (600_000, 240_000, 0));
    assert_eq!(raw["housing"]["households"].as_array().unwrap().len(), 1);
    assert_eq!(raw["housing"]["households"][0]["people"].as_u64(), Some(10));
    let housing = world.housing.as_ref().expect("opening housing retained");
    assert_eq!(housing.household_registration_count, 1);
    let venue = world.venue.as_ref().expect("managed venue opening retained");
    assert_eq!((venue.venue_count, venue.offer_count), (2, 4));
    assert!(engine.owned_capacity_bytes() <= mycity_core::transaction::ENGINE_BYTE_BUDGET,
        "valid venue control must satisfy the existing total Engine budget");
}

fn assert_initial_resource_state(store: &Store) {
    let actual_resources: Vec<_> = store.engine().unwrap().resources().iter()
        .map(|r| (r.id, r.capacity, r.reserved, r.occupied)).collect();
    assert_eq!(actual_resources,
        RESOURCES.iter().map(|&(id, capacity)| (id, capacity, 0, 0)).collect::<Vec<_>>());
}

fn assert_initial_state(store: &Store) {
    assert_opening_facts(store);
    assert_initial_resource_state(store);
}

fn assert_original_genesis(world: &Path) {
    let genesis_bytes = fs::read(world.join("genesis.json")).unwrap();
    let genesis: Value = serde_json::from_slice(&genesis_bytes).unwrap();
    let persisted: Vec<u8> = serde_json::from_value(genesis["opening"].clone()).unwrap();
    assert_eq!(persisted, OPENING, "genesis preserves exact supplied opening bytes");
    assert_eq!(genesis["rules_identity"], RULES);
    assert_eq!(digest(&persisted), OPENING_SHA256);
}

fn assert_venue_after_enter(store: &Store) {
    let engine = store.engine().unwrap();
    let venue = engine.world().view().venue.unwrap();
    assert_eq!(venue.uses.len(), 1);
    assert_eq!(venue.uses[0].status, UseStatus::Actual);
    assert_eq!((venue.uses[0].units, venue.uses[0].token.number), (3, 1));
    assert_eq!(venue.activities.len(), 1);
    let entered = venue.activities[0].entered.unwrap();
    assert_eq!((entered.event_id, entered.operation_id, entered.occurred_at), (100, 3, 0));
    let resources: Vec<_> = engine.resources().iter().map(|r| (r.id, r.capacity, r.reserved, r.occupied)).collect();
    assert_eq!(resources, vec![(501, 4, 0, 3), (502, 4, 0, 0), (503, 5, 0, 0)]);
    assert_opening_facts(store);
}

fn spawn_probe(run: &RetainedRun, world: &Path, phase: &str) {
    let probe = Probe { world: world.to_owned(), phase: phase.to_owned() };
    let stdout_path = run.path.join(format!("{phase}-stdout.log"));
    let stderr_path = run.path.join(format!("{phase}-stderr.log"));
    run.write_json(&format!("{phase}-launch.json"), &json!({
        "probe": probe.clone(), "stdout_path": stdout_path.clone(), "stderr_path": stderr_path.clone(),
        "timeout_seconds": 60,
    }));
    let stdout_file = OpenOptions::new().create_new(true).write(true).open(&stdout_path).unwrap();
    let stderr_file = OpenOptions::new().create_new(true).write(true).open(&stderr_path).unwrap();
    let child_result = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "fresh_store_restore_probe_child", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, serde_json::to_string(&probe).unwrap())
        .stdout(Stdio::from(stdout_file)).stderr(Stdio::from(stderr_file)).spawn();
    let mut child = match child_result {
        Ok(child) => child,
        Err(error) => {
            run.write_json(&format!("{phase}-result.json"), &json!({
                "spawn_error": error.to_string(), "timed_out": false,
                "stdout_path": stdout_path, "stderr_path": stderr_path,
            }));
            panic!("fresh Store helper failed to spawn; evidence retained at {}", run.path.display());
        }
    };
    let child_pid = child.id();
    run.write_json(&format!("{phase}-started.json"), &json!({
        "child_pid": child_pid, "probe": probe.clone(),
        "stdout_path": stdout_path.clone(), "stderr_path": stderr_path.clone(),
    }));
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut timed_out = false;
    let mut wait_error = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                timed_out = true;
                if let Err(error) = child.kill() { wait_error = Some(format!("kill: {error}")); }
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
    let stdout_bytes = fs::metadata(&stdout_path).map(|m| m.len()).ok();
    let stderr_bytes = fs::metadata(&stderr_path).map(|m| m.len()).ok();
    run.write_json(&format!("{phase}-result.json"), &json!({
        "child_pid": child_pid, "exit_code": status.as_ref().and_then(|s| s.code()),
        "success": status.as_ref().is_some_and(|status| status.success()),
        "timed_out": timed_out, "wait_error": wait_error,
        "stdout_path": stdout_path, "stdout_bytes": stdout_bytes,
        "stderr_path": stderr_path, "stderr_bytes": stderr_bytes, "probe": probe.clone(),
    }));
    assert!(!timed_out && wait_error.is_none() && status.is_some_and(|status| status.success()),
        "fresh Store helper failed or timed out; preserve artifacts at {}", run.path.display());
}

#[test]
fn managed_people_v1_opening_same_slot_wal_checkpoint_and_no_overwrite() {
    assert_eq!(digest(OPENING), OPENING_SHA256);
    let parsed = make_input(OPENING);
    assert_eq!(parsed.source().origin(), InputOrigin::SuppliedBytes);
    assert_eq!(parsed.source().bytes(), OPENING);
    assert_eq!(parsed.source().sha256(), OPENING_SHA256);
    let run = RetainedRun::new();

    let venue_world = run.world("managed-venue-world");
    let workspace = EngineWorkspace::new().unwrap();
    let before = workspace.identity().unwrap();
    let input = make_input(OPENING);
    let mut store = Store::create_opening_input_in_workspace_with_observer(
        &venue_world, &input, resources(), RULES, workspace, |_| Ok(()),
    ).unwrap();
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
    assert_original_genesis(&venue_world);
    assert_initial_state(&store);

    let enable = envelope(&store, 1, Action::Timeline { command: TimelineCommand::Enable });
    assert!(store.execute(Caller::System, enable).unwrap().model.is_ok());
    let accept = envelope(&store, 2, Action::Venue { use_id: 1, command: VenueCommand::Accept });
    let accepted = store.execute(Caller::Account(3), accept).unwrap().model.unwrap();
    let token = accepted.created_tokens[0].expect("accept receipt binds the venue lease token");
    assert_eq!(token.number, 1);
    assert_eq!(accepted.operation_id, 2);
    let enter = envelope(&store, 3, Action::Venue {
        use_id: 1,
        command: VenueCommand::Enter { event_id: 100, token_number: token.number, occurred_at: 0 },
    });
    let entered = store.execute(Caller::System, enter.clone()).unwrap().model.unwrap();
    assert_eq!(entered.operation_id, 3);
    assert_venue_after_enter(&store);
    assert_original_genesis(&venue_world);
    drop(store); // release the original process's Store lock before actual cold-open subprocesses

    // The first child recovers from WAL, replays the identical event, and checkpoints.
    spawn_probe(&run, &venue_world, "wal-recovery");
    // The next independently started process restores from the newly written checkpoint.
    spawn_probe(&run, &venue_world, "checkpoint-recovery");
    let before_duplicate_create = file_manifest(&venue_world);
    assert!(Store::create_opening_input(&venue_world, &input, resources(), RULES).is_err());
    assert_eq!(file_manifest(&venue_world), before_duplicate_create, "published world bytes may not be replaced");

    // An invalid account binding is tested only after the exact legal control above succeeds.
    let mut bad_value: Value = serde_json::from_slice(OPENING).unwrap();
    bad_value["venue"]["venues"][0]["owner"] = json!(999);
    let bad_bytes = serde_json::to_vec(&bad_value).unwrap();
    let bad_input = make_input(&bad_bytes);
    let bad_world = run.world("invalid-owner");
    let bad = Store::create_opening_input(&bad_world, &bad_input, resources(), RULES);
    let bad_error = bad.as_ref().err().expect("invalid reference must be rejected");
    assert!(matches!(&bad, Err(Error::Model(error)) if error == "missing_account"), "invalid ref must reach the model reference gate: {bad_error:?}");
    assert!(!bad_world.join("CURRENT.json").exists());
    assert_eq!(bad_input.source().bytes(), bad_bytes);
    assert_eq!(bad_input.source().sha256(), digest(&bad_bytes));
    let corrected_world = run.world("corrected-source-new-world");
    let corrected = Store::create_opening_input(&corrected_world, &input, resources(), RULES).unwrap();
    assert_initial_state(&corrected);
    assert_original_genesis(&corrected_world);
    drop(corrected);
    assert!(file_manifest(&venue_world).contains_key("genesis.json"));
    run.write_json("invalid-control.json", &json!({
        "legal_control_sha256": OPENING_SHA256, "invalid_owner": 999,
        "invalid_error": "missing_account", "invalid_world_published": false,
        "corrected_source_installed_at": corrected_world,
        "original_world_unchanged": file_manifest(&venue_world) == before_duplicate_create,
    }));

    // A response lost after CURRENT publication is uncertain: preserve and reopen the same path.
    let lost_world = run.world("after-current-response-lost");
    let mut reached_current = false;
    let result = Store::create_opening_input_in_workspace_with_observer(
        &lost_world, &input, resources(), RULES, EngineWorkspace::new().unwrap(), |point| {
            if point == CreatePoint::CurrentRenamed {
                reached_current = true;
                Err(std::io::Error::other("injected-after-CURRENT-response-loss"))
            } else { Ok(()) }
        },
    );
    assert!(reached_current, "the exact post-CURRENT cutpoint must be reached");
    let after_current_error = format!("{:?}", result.as_ref().err().expect("observer cutpoint must report its original error"));
    assert!(matches!(result, Err(Error::Io(_))));
    assert!(lost_world.join("CURRENT.json").is_file());
    assert_original_genesis(&lost_world);
    spawn_probe(&run, &lost_world, "after-current-recovery");
    let mut reopened = Store::open(&lost_world, RULES).unwrap();
    assert_initial_state(&reopened);
    let transfer = envelope(&reopened, 1, Action::Transfer {
        from: reopened.engine().unwrap().world().account_ref(1).unwrap(),
        to: reopened.engine().unwrap().world().account_ref(4).unwrap(),
        amount: Money::new(137).unwrap(),
    });
    assert!(reopened.execute(Caller::Account(1), transfer).unwrap().model.is_ok());
    assert_transferred_capital(&reopened);
    reopened.checkpoint().unwrap();
    drop(reopened);
    spawn_probe(&run, &lost_world, "after-current-checkpoint");
    let before_lost_retry = file_manifest(&lost_world);
    assert!(Store::create_opening_input(&lost_world, &input, resources(), RULES).is_err());
    assert_eq!(file_manifest(&lost_world), before_lost_retry, "re-create must not reissue original capital or overwrite receipts");
    let reopened = Store::open(&lost_world, RULES).unwrap();
    assert_transferred_capital(&reopened);
    drop(reopened);
    run.write_json("after-current-recovery.json", &json!({
        "reached_current_rename": reached_current, "original_create_error": after_current_error,
        "reopened_same_world_path": lost_world, "transfer_operation": 1,
        "treasury_cash_after_retry": 999_863, "supplier_cash_after_retry": 1_200_137,
        "recreate_preserved_files": true,
    }));
}

#[test]
#[ignore = "fresh process probe invoked by the parent test after the prior Store lock is dropped"]
fn fresh_store_restore_probe_child() {
    let Ok(encoded) = std::env::var(PROBE_ENV) else { return; };
    let probe: Probe = serde_json::from_str(&encoded).unwrap();
    let root = configured_root();
    let world = fs::canonicalize(&probe.world).expect("same retained world path");
    assert!(world.starts_with(&root) && world != root);
    let mut store = Store::open(&world, RULES).unwrap();
    assert_original_genesis(&world);
    if probe.phase == "after-current-recovery" {
        assert_initial_state(&store);
        println!("fresh process pid={} phase={} reopened same published opening", std::process::id(), probe.phase);
        return;
    }
    if probe.phase == "after-current-checkpoint" {
        assert_transferred_capital(&store);
        let transfer = Envelope {
            world: 1, branch: 1, operation_id: 1, expected_revision: 0,
            actions: vec![Action::Transfer {
                from: store.engine().unwrap().world().account_ref(1).unwrap(),
                to: store.engine().unwrap().world().account_ref(4).unwrap(),
                amount: Money::new(137).unwrap(),
            }],
        };
        let receipt = store.receipt(1).unwrap().unwrap().outcome.unwrap();
        assert_eq!(store.execute(Caller::Account(1), transfer).unwrap().model.unwrap(), receipt);
        assert_transferred_capital(&store);
        println!("fresh process pid={} phase={} replayed transfer receipt once", std::process::id(), probe.phase);
        return;
    }
    assert_venue_after_enter(&store);
    let original = store.receipt(3).unwrap().expect("durable original enter receipt");
    let prior_receipt = original.outcome.expect("original enter transaction succeeded");
    assert_eq!(prior_receipt.operation_id, 3);
    let accept = store.receipt(2).unwrap().expect("durable original accept receipt");
    let accept_receipt = accept.outcome.expect("original accept transaction succeeded");
    assert_eq!(accept_receipt.operation_id, 2);
    assert_eq!(accept_receipt.created_tokens[0].expect("Accept creates the lease token").number, 1);
    assert_eq!(prior_receipt.created_tokens[0], None, "Enter reuses the accepted token and creates none");
    let replay = Envelope {
        world: 1, branch: 1, operation_id: 3, expected_revision: 2,
        actions: vec![Action::Venue { use_id: 1, command: VenueCommand::Enter {
            event_id: 100, token_number: 1, occurred_at: 0,
        }}],
    };
    let replayed = store.execute(Caller::System, replay).unwrap();
    assert_eq!(replayed.model.unwrap(), prior_receipt, "same operation returns the original receipt");
    assert!(replayed.has_persistent_receipt);
    assert_venue_after_enter(&store);
    let view = store.engine().unwrap().world().view().venue.unwrap();
    assert_eq!(view.uses.iter().filter(|row| row.status == UseStatus::Actual).count(), 1);
    assert_eq!(view.activities.iter().filter(|row| row.entered.is_some()).count(), 1);
    if probe.phase == "wal-recovery" { store.checkpoint().unwrap(); }
    println!("fresh process pid={} phase={} receipt={:?}", std::process::id(), probe.phase, prior_receipt);
}

fn assert_transferred_capital(store: &Store) {
    let view = store.engine().unwrap().world().view();
    assert_eq!(view.accounts.iter().find(|a| a.id == 1).unwrap().cash, Money::new(999_863).unwrap());
    assert_eq!(view.accounts.iter().find(|a| a.id == 4).unwrap().cash, Money::new(1_200_137).unwrap());
    assert_eq!(store.committed_sequence(), 1);
}
