//! OpeningInput -> original Store -> continuous production -> authenticated cold restore.
//! The profile is synthetic and proves persistence wiring, not real-world production.
use mycity_core::{
    continuous_supply::{Command as SupplyCommand, View},
    opening_loader::{InputOrigin, OpeningInput},
    timeline,
    transaction::{Action, Caller, EngineWorkspace, Envelope},
};
use mycity_storage::Store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[path = "../../../tests/fixtures/continuous-supply/opening.rs"]
mod fixture;

const RULES: &str = "opening-continuous-supply-v1";
const APPROVED_BASE: &str = "/Volumes/HP P900/dev/MyCity/work";
const ROOT_ENV: &str = "MYCITY_OPENING_CONTINUOUS_SUPPLY_TEST_ROOT";
const PROBE_ENV: &str = "MYCITY_OPENING_CONTINUOUS_SUPPLY_PROBE";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Probe {
    world: PathBuf,
    expected: Value,
    checkpoint_after_open: bool,
}

fn evidence_dir() -> PathBuf {
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
    let root = fs::canonicalize(
        std::env::var_os(ROOT_ENV).expect("set an existing unique HP P900 evidence directory"),
    )
    .expect("evidence root must exist");
    assert!(root.is_dir() && root != approved && root.starts_with(&approved));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = root.join(format!("opening-continuous-supply-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).expect("reserve a new retained evidence directory");
    println!("retained OpeningInput continuous-supply evidence: {}", path.display());
    path
}

fn write_json(path: &Path, value: &Value) {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(value).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}

fn apply(store: &mut Store, caller: Caller, action: Action) -> Envelope {
    let engine = store.engine().unwrap();
    let (scope, revision) = engine.world().scope_revision();
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

fn advance(store: &mut Store, target: u64) {
    for _ in 0..128 {
        apply(
            store,
            Caller::System,
            Action::Timeline {
                command: timeline::Command::Advance { target, limit: 1 },
            },
        );
        if store.engine().unwrap().timeline().unwrap().progress.target.is_none() {
            assert_eq!(
                store.engine().unwrap().timeline().unwrap().progress.last_complete,
                target
            );
            return;
        }
    }
    panic!("bounded original Timeline did not finish");
}

fn view(store: &Store) -> View {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .continuous_supply
        .expect("configured original continuous supply remains present")
}

fn expected_state(store: &Store) -> Value {
    json!({
        "state_hash": store.state_hash().unwrap(),
        "continuous_supply": view(store),
        "due": store.engine().unwrap().continuous_supply_due_state().unwrap(),
        "timeline": store.engine().unwrap().timeline().unwrap().progress,
        "registration": store.engine().unwrap().continuous_supply_registration(),
    })
}

fn spawn_probe(dir: &Path, probe: Probe, name: &str) {
    let stdout = dir.join(format!("{name}-stdout.log"));
    let stderr = dir.join(format!("{name}-stderr.log"));
    write_json(
        &dir.join(format!("{name}-launch.json")),
        &json!({ "probe": probe, "stdout": stdout, "stderr": stderr, "timeout_seconds": 60 }),
    );
    let out = OpenOptions::new().create_new(true).write(true).open(&stdout).unwrap();
    let err = OpenOptions::new().create_new(true).write(true).open(&stderr).unwrap();
    let child_result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "opening_continuous_supply_cold_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PROBE_ENV, serde_json::to_string(&probe).unwrap())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn();
    let mut child = match child_result {
        Ok(child) => child,
        Err(error) => {
            write_json(
                &dir.join(format!("{name}-result.json")),
                &json!({ "spawn_error": error.to_string(), "stdout": stdout, "stderr": stderr }),
            );
            panic!("owned cold child failed to spawn; retained evidence: {}", dir.display());
        }
    };
    let pid = child.id();
    write_json(&dir.join(format!("{name}-started.json")), &json!({ "pid": pid }));
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
                let kill_error = child.kill().err().map(|e| e.to_string());
                wait_error = Some(match kill_error {
                    Some(kill_error) => format!("try_wait: {error}; kill: {kill_error}"),
                    None => format!("try_wait: {error}; child kill requested"),
                });
                break child.wait().ok();
            }
        }
    };
    write_json(
        &dir.join(format!("{name}-result.json")),
        &json!({
            "pid": pid,
            "exit_code": status.as_ref().and_then(|status| status.code()),
            "success": status.as_ref().is_some_and(|status| status.success()),
            "timed_out": timed_out,
            "wait_error": wait_error,
            "stdout": stdout,
            "stderr": stderr,
        }),
    );
    assert!(
        !timed_out && wait_error.is_none() && status.is_some_and(|status| status.success()),
        "owned cold child failed; retained evidence: {}",
        dir.display()
    );
}

#[test]
fn opening_input_continuous_supply_wal_checkpoint_and_nonzero_stop() {
    let dir = evidence_dir();
    let world = dir.join("world");
    let opening = serde_json::to_vec(&fixture::input()).unwrap();
    let input = OpeningInput::from_json(&opening).unwrap();
    assert_eq!(input.source().origin(), InputOrigin::SuppliedBytes);
    assert_eq!(input.source().bytes(), opening);
    let workspace = EngineWorkspace::new().unwrap();
    let before = workspace.identity().unwrap();
    let mut store = Store::create_opening_input_in_workspace_with_observer(
        &world,
        &input,
        fixture::resources(),
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
    let due = store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap();
    assert_eq!(due.city_id, 42);
    assert_eq!(due.source_id, 86);
    assert!(store.engine().unwrap().continuous_supply_is_authenticated());

    apply(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: timeline::Command::Enable,
        },
    );
    let start = apply(
        &mut store,
        Caller::Account(1),
        Action::ContinuousSupply {
            command: SupplyCommand::Start {
                run_id: 1,
                pumps: 3,
            },
        },
    );
    let started_receipt = store
        .receipt(start.operation_id)
        .unwrap()
        .expect("start must have an original Store receipt");
    assert_eq!(started_receipt.actor, 1);
    assert!(started_receipt.outcome.is_ok());
    assert!(matches!(
        started_receipt.actions[0],
        Some(Action::ContinuousSupply {
            command: SupplyCommand::Start { run_id: 1, pumps: 3 }
        })
    ));
    assert!(store
        .engine()
        .unwrap()
        .continuous_supply_receipt_operations()
        .contains(&Some(start.operation_id)));
    advance(&mut store, 1);
    let expected = expected_state(&store);
    write_json(&dir.join("expected-wal-prefix.json"), &expected);
    drop(store);

    // Parent did not checkpoint: the first independent process must consume the WAL tail.
    spawn_probe(
        &dir,
        Probe {
            world: world.clone(),
            expected: expected.clone(),
            checkpoint_after_open: true,
        },
        "wal-recovery",
    );
    // The first process checkpoints that state; the second cold process reads that checkpoint.
    spawn_probe(
        &dir,
        Probe {
            world: world.clone(),
            expected,
            checkpoint_after_open: false,
        },
        "checkpoint-recovery",
    );

    let mut reopened = Store::open(&world, RULES).unwrap();
    assert!(reopened.engine().unwrap().continuous_supply_is_authenticated());
    advance(&mut reopened, 3);
    apply(
        &mut reopened,
        Caller::Account(1),
        Action::ContinuousSupply {
            command: SupplyCommand::Stop,
        },
    );
    let stopped = view(&reopened);
    let run = stopped.runs.iter().flatten().last().unwrap();
    assert_eq!(run.started_at, 0);
    assert_eq!(run.ended_at, Some(3));
    assert_eq!(run.production_seconds, 3);
    assert_eq!(stopped.genset.active_seconds, 3);
    let final_state = expected_state(&reopened);
    write_json(&dir.join("stopped-state.json"), &final_state);
    reopened.checkpoint().unwrap();
    drop(reopened);
    let final_open = Store::open(&world, RULES).unwrap();
    assert_eq!(expected_state(&final_open), final_state);
    assert!(final_open.engine().unwrap().continuous_supply_is_authenticated());
}

#[test]
#[ignore = "fresh process Store::open probe launched by the parent test"]
fn opening_continuous_supply_cold_probe() {
    let Ok(encoded) = std::env::var(PROBE_ENV) else {
        return;
    };
    let probe: Probe = serde_json::from_str(&encoded).unwrap();
    let world = fs::canonicalize(&probe.world).unwrap();
    let dir = world.parent().unwrap();
    let root = fs::canonicalize(std::env::var_os(ROOT_ENV).expect("parent evidence root")).unwrap();
    assert!(world.starts_with(&root) && world != root);
    let workspace = EngineWorkspace::new().unwrap();
    let mut store = Store::open_in_workspace(&world, RULES, workspace).unwrap();
    let engine = store.engine().unwrap();
    assert!(engine.continuous_supply_is_authenticated());
    assert!(engine.continuous_supply_due_state().unwrap().is_some());
    assert_eq!(expected_state(&store), probe.expected);
    if probe.checkpoint_after_open {
        store.checkpoint().unwrap();
        assert_eq!(expected_state(&store), probe.expected);
        write_json(&dir.join("wal-reader-checkpointed.json"), &json!({ "complete": true }));
    }
}
