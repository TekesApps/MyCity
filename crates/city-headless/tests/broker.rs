//! Real broker/Sim/worker processes. Finite fault coverage, not an8h/16GB pass.
use mycity_core::{
    background::Command as B,
    exchange::Command as X,
    schedule::Phase,
    timeline::Command as T,
    transaction::{Action, Envelope},
};
use mycity_headless::{
    normal_pool::{self, broker::Supervisor, Process, BYTES},
    sim_client::Client,
    sim_host::{Command, ProbePoint},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let base = std::env::var_os("MYCITY_BROKER_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let p = base.join(format!(
            "mc-broker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("broker_fixture_retained_after_panic:{}", self.0.display());
        } else if std::env::var_os("MYCITY_BROKER_TEST_ARTIFACT_ROOT").is_none() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn exe() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_mycity-headless"))
}
fn call(c: &mut Client, id: &mut u64, command: Command) -> Value {
    *id += 1;
    c.submit(*id, command).unwrap();
    for _ in 0..24 {
        let e = c.next(Duration::from_secs(10)).unwrap();
        if e.kind == "result" {
            assert!(e.payload.get("Err").is_none(), "{e:?}");
            return e.payload["Ok"].clone();
        }
    }
    panic!("result absent")
}
fn action(c: &mut Client, id: &mut u64, action: Action) {
    let s = call(c, id, Command::Status);
    let v = call(
        c,
        id,
        Command::Execute {
            envelope: Envelope {
                world: 1,
                branch: 1,
                operation_id: s["sequence"].as_u64().unwrap() + 1,
                expected_revision: s["revision"].as_u64().unwrap(),
                actions: vec![action],
            },
        },
    );
    assert!(v["model"].get("Ok").is_some());
}
fn prepare(c: &mut Client, id: &mut u64) {
    for quote in [2, 1] {
        action(
            c,
            id,
            Action::Exchange {
                command: X::Dispatch { quote },
            },
        );
    }
    action(c, id, Action::Timeline { command: T::Enable });
    action(c, id, Action::Background { command: B::Enable });
    for _ in 0..30 {
        action(
            c,
            id,
            Action::Timeline {
                command: T::Advance {
                    target: 30,
                    limit: 256,
                },
            },
        );
        let s = call(c, id, Command::Status);
        if serde_json::from_value::<Option<Phase>>(s["timeline"]["phase"].clone()).unwrap()
            == Some(Phase::P4)
        {
            return;
        }
    }
    panic!("P4 absent")
}
fn barrier(c: &mut Client) {
    for _ in 0..8 {
        let e = c.next(Duration::from_secs(10)).unwrap();
        if e.kind == "probe_barrier" {
            return;
        }
        assert_ne!(e.kind, "result", "{e:?}");
    }
    panic!("barrier absent")
}
#[test]
fn broker_wait_and_real_sim_death_revoke_restore_capacity_and_cold_result() {
    let mut reference = None;
    for point in [
        ProbePoint::Leased,
        ProbePoint::WorkerSources,
        ProbePoint::WorkerCells,
        ProbePoint::CandidatePublished,
        ProbePoint::Committed,
    ] {
        let t = Temp::new();
        let root = t.0.join("pool");
        let world = t.0.join("world");
        let mut pool = Process::spawn(exe(), &root, None).unwrap();
        let mut c = Client::spawn_in_pool(exe(), &world, true, true, &root).unwrap();
        c.next(Duration::from_secs(10)).unwrap();
        let mut id = 0;
        let initial = normal_pool::status(&root).unwrap();
        assert_eq!(initial.workspace_capacity_bytes, 59 * 1024 * 1024);
        assert_eq!(initial.workspace_fills, 1);
        assert_eq!(initial.available_touched_bytes, BYTES - 59 * 1024 * 1024);
        let sim_status = call(&mut c, &mut id, Command::Status);
        assert_eq!(
            sim_status["runtime"]["sim_codec"]["owner_capacity_bytes"],
            4 * 1024 * 1024
        );
        assert_eq!(
            sim_status["runtime"]["sim_codec"]["full_balance_scan_passed"],
            true
        );
        prepare(&mut c, &mut id);
        assert!(Supervisor::register(&root).is_err());
        id += 1;
        c.submit(id, Command::ProbeRequired { job: 1, point })
            .unwrap();
        barrier(&mut c);
        let before = normal_pool::status(&root).unwrap();
        if point == ProbePoint::Leased {
            assert_eq!(before.grants, 0);
        } else {
            assert_eq!(before.grants, 1);
            assert_eq!(before.fill_acknowledgements, 1);
        }
        if let Some(pid) = c.worker_pid() {
            let parent = std::process::Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "ppid="])
                .output()
                .unwrap();
            let actual_parent = String::from_utf8(parent.stdout)
                .unwrap()
                .trim()
                .parse::<u32>()
                .unwrap();
            let ready: Value =
                serde_json::from_slice(&fs::read(root.join("ready.json")).unwrap()).unwrap();
            assert_eq!(u64::from(actual_parent), ready["pid"].as_u64().unwrap());
            assert_ne!(actual_parent, c.pid());
            if point == ProbePoint::WorkerCells {
                assert!(std::process::Command::new("kill")
                    .args(["-STOP", &pid.to_string()])
                    .status()
                    .unwrap()
                    .success());
            }
        }
        // Actual external SIGKILL, then Client's actual Child wait, then broker Revoke.
        assert!(std::process::Command::new("kill")
            .args(["-KILL", &c.pid().to_string()])
            .status()
            .unwrap()
            .success());
        c.wait_for_sim_exit(Duration::from_secs(3)).unwrap();
        let after = normal_pool::status(&root).unwrap();
        assert_eq!(after.available_touched_bytes, BYTES);
        assert_eq!(after.workspace_grants, after.workspace_returns);
        assert_eq!(after.workspace_capacity_bytes, 0);
        assert_eq!(after.grants, after.returns);
        let mut recovered = Client::spawn_in_pool(exe(), &world, false, false, &root).unwrap();
        recovered.next(Duration::from_secs(10)).unwrap();
        let result = call(&mut recovered, &mut id, Command::RunRequired { job: 1 });
        assert_eq!(result["replayed"], point == ProbePoint::Committed);
        let s = call(&mut recovered, &mut id, Command::Status);
        assert_eq!(s["background"]["time"], 30);
        if let Some(expected) = &reference {
            assert_eq!(&s["background"]["field"], expected);
        } else {
            reference = Some(s["background"]["field"].clone());
        }
        call(&mut recovered, &mut id, Command::Stop);
        recovered.stop().unwrap();
        c.stop().unwrap();
        let final_state = normal_pool::status(&root).unwrap();
        assert_eq!(final_state.grants, final_state.returns);
        assert_eq!(final_state.available_touched_bytes, BYTES);
        pool.finish().unwrap();
        let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
        assert!(events.contains("broker_session_revoked_after_sim_wait"));
    }
}
#[test]
fn broker_owned_worker_fault_retries_inside_same_sim_without_double_credit() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let mut c = Client::spawn_in_pool(exe(), &t.0.join("world"), true, true, &root).unwrap();
    c.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    prepare(&mut c, &mut id);
    id += 1;
    c.submit(
        id,
        Command::ProbeRequired {
            job: 1,
            point: ProbePoint::WorkerCells,
        },
    )
    .unwrap();
    barrier(&mut c);
    c.kill_owned_worker().unwrap();
    c.acknowledge_probe(ProbePoint::WorkerCells, false).unwrap();
    let mut failed = 0;
    loop {
        let e = c.next(Duration::from_secs(10)).unwrap();
        if e.kind == "attempt_failed" {
            failed += 1;
        }
        if e.kind == "result" {
            assert!(e.payload.get("Ok").is_some(), "{e:?}");
            break;
        }
    }
    assert_eq!(failed, 1);
    call(&mut c, &mut id, Command::Stop);
    c.stop().unwrap();
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.grants, 2);
    assert_eq!(s.returns, 2);
    pool.finish().unwrap();
}

struct FaultPool {
    child: std::process::Child,
    root: PathBuf,
}
impl FaultPool {
    fn spawn(root: &Path, fault: &str) -> Self {
        let child = std::process::Command::new(exe())
            .arg("normal-pool-host")
            .arg(root)
            .env("MYCITY_NORMAL_POOL_FAULT", fault)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut p = Self {
            child,
            root: root.to_owned(),
        };
        let start = std::time::Instant::now();
        while let Err(error) = normal_pool::status(root) {
            assert!(p.child.try_wait().unwrap().is_none());
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "broker ready failure:{error}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        p
    }
    fn finish(&mut self) {
        self.child.stdin.take();
        let start = std::time::Instant::now();
        loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                assert!(s.success());
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(self.root.join("report.json").exists());
    }
}
impl Drop for FaultPool {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
#[test]
fn lost_bind_reply_reaps_actual_sim_and_cancels_generation_before_replacement() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut p = FaultPool::spawn(&root, "after-broker-bind-reply");
    assert!(Client::spawn_in_pool(exe(), &t.0.join("world"), true, false, &root).is_err());
    let mut c = Client::spawn_in_pool(exe(), &t.0.join("replacement"), true, false, &root).unwrap();
    c.next(Duration::from_secs(10)).unwrap();
    c.stop().unwrap();
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.grants, 0);
    assert_eq!(s.available_touched_bytes, BYTES);
    p.finish();
}
#[test]
fn lost_revoke_request_or_reply_retries_after_real_sim_wait_without_duplicate_credit() {
    for fault in ["before-broker-revoke", "after-broker-revoke-reply"] {
        let t = Temp::new();
        let root = t.0.join("pool");
        let mut p = FaultPool::spawn(&root, fault);
        let mut c = Client::spawn_in_pool(exe(), &t.0.join("world"), true, true, &root).unwrap();
        c.next(Duration::from_secs(10)).unwrap();
        let mut id = 0;
        prepare(&mut c, &mut id);
        id += 1;
        c.submit(
            id,
            Command::ProbeRequired {
                job: 1,
                point: ProbePoint::WorkerCells,
            },
        )
        .unwrap();
        barrier(&mut c);
        assert!(std::process::Command::new("kill")
            .args(["-KILL", &c.pid().to_string()])
            .status()
            .unwrap()
            .success());
        assert!(c.wait_for_sim_exit(Duration::from_secs(3)).is_err());
        let first = normal_pool::status(&root).unwrap();
        assert_eq!(
            first.returns,
            if fault == "before-broker-revoke" {
                0
            } else {
                1
            }
        );
        assert!(Supervisor::register(&root).is_err()); // actual reap alone cannot bypass receipt acknowledgement
        c.wait_for_sim_exit(Duration::from_secs(3)).unwrap();
        c.wait_for_sim_exit(Duration::from_secs(3)).unwrap();
        let after = normal_pool::status(&root).unwrap();
        assert_eq!(after.grants, 1);
        assert_eq!(after.returns, 1);
        assert_eq!(after.available_touched_bytes, BYTES);
        let mut replacement =
            Client::spawn_in_pool(exe(), &t.0.join("replacement"), true, false, &root).unwrap();
        replacement.next(Duration::from_secs(10)).unwrap();
        replacement.stop().unwrap();
        c.stop().unwrap();
        p.finish();
    }
}
#[test]
fn sim_killed_after_worker_spawn_before_start_reply_recovers_without_knowing_worker_id() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut p = FaultPool::spawn(&root, "after-broker-spawn-pause");
    let mut c = Client::spawn_in_pool(exe(), &t.0.join("world"), true, false, &root).unwrap();
    c.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    prepare(&mut c, &mut id);
    id += 1;
    c.submit(id, Command::RunRequired { job: 1 }).unwrap();
    let start = std::time::Instant::now();
    loop {
        if fs::read_to_string(root.join("events.jsonl"))
            .unwrap()
            .contains("child_attached")
        {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(c.worker_pid().is_none()); // Start response has deliberately not reached Sim
    c.kill_sim().unwrap();
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.grants, 1);
    assert_eq!(s.fill_acknowledgements, 0);
    assert_eq!(s.returns, 1);
    assert_eq!(s.available_touched_bytes, BYTES);
    p.finish();
}

#[test]
fn actual_sim_spawn_permission_failure_cancels_unbound_registration() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let nonexe = t.0.join("not-executable");
    fs::write(&nonexe, b"fixture, not an executable").unwrap();
    assert!(Client::spawn_in_pool(&nonexe, &t.0.join("world"), true, false, &root).is_err());
    let mut c = Client::spawn_in_pool(exe(), &t.0.join("replacement"), true, false, &root).unwrap();
    c.next(Duration::from_secs(10)).unwrap();
    c.stop().unwrap();
    assert_eq!(normal_pool::status(&root).unwrap().grants, 0);
    pool.finish().unwrap();
    assert!(fs::read_to_string(root.join("events.jsonl"))
        .unwrap()
        .contains("broker_unspawned_cancelled"));
}
#[test]
fn broker_death_is_fail_closed_and_cannot_adopt_old_ready_or_claim_return() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = FaultPool::spawn(&root, "after-broker-spawn-pause");
    let mut c = Client::spawn_in_pool(exe(), &t.0.join("world"), true, true, &root).unwrap();
    c.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    prepare(&mut c, &mut id);
    // Consume the pause fault in Start, then pause the real worker at a Sim gate.
    id += 1;
    c.submit(
        id,
        Command::ProbeRequired {
            job: 1,
            point: ProbePoint::WorkerCells,
        },
    )
    .unwrap();
    barrier(&mut c);
    let before = normal_pool::status(&root).unwrap();
    assert_eq!(before.grants, 1);
    assert_eq!(before.returns, 0);
    pool.child.kill().unwrap();
    pool.child.wait().unwrap();
    assert!(c.kill_sim().is_err());
    assert!(normal_pool::status(&root).is_err());
    assert!(Process::spawn(exe(), &root, None).is_err());
    assert!(!root.join("report.json").exists());
    let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
    assert!(!events.contains("returned_after_wait_or_abort"));
}

#[test]
fn partial_sim_workspace_refill_after_real_child_exit_stops_pool_without_false_credit() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = FaultPool::spawn(&root, "workspace-return-page-2");
    let mut c = Client::spawn_in_pool(exe(), &t.0.join("world"), true, false, &root).unwrap();
    c.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    call(&mut c, &mut id, Command::Stop);
    assert!(c.stop().is_err()); // real Sim is waited, but partial physical return cannot be acknowledged.
    let status = pool.child.wait().unwrap();
    assert!(!status.success());
    let rows: Vec<Value> = fs::read_to_string(root.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let failure = rows
        .iter()
        .find(|r| r["kind"] == "injected_partial_workspace_return_failure")
        .unwrap();
    assert_eq!(failure["phase"], "retiring");
    assert_eq!(failure["summary"]["workspace_returns"], 0);
    assert_eq!(
        failure["summary"]["workspace_capacity_bytes"],
        59 * 1024 * 1024
    );
    assert!(!rows
        .iter()
        .any(|r| r["kind"] == "returned_after_sim_wait_or_cancel"));
    assert!(!root.join("report.json").exists());
}
