#[path = "support/lifecycle_artifacts.rs"]
mod lifecycle_artifacts;
use mycity_headless::{
    scale_model_task::{self as task, Mode},
    scale_reference::Tier,
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = lifecycle_artifacts::root().join(format!(
            "mycity-model-task-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = lifecycle_artifacts::cleanup(&self.0);
    }
}
fn exe() -> &'static str {
    lifecycle_artifacts::exe().to_str().unwrap()
}
#[test]
fn invalid_source_partial_and_old_result_never_authorize_a_task() {
    let t = Temp::new();
    let dir = task::prepare(&t.0, Tier::Small, Mode::Reference).unwrap();
    assert!(task::validate(&dir).is_err());
    fs::write(dir.join("result.json.tmp"), b"{}").unwrap();
    assert!(task::validate(&dir).is_err());
    let mut request: Value =
        serde_json::from_slice(&fs::read(dir.join("request.json")).unwrap()).unwrap();
    request["source_sha256"] = "wrong-source".into();
    fs::write(
        dir.join("request.json"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let out = Command::new(exe())
        .arg("scale-model-task")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("model_task_request_identity"));
    assert!(!dir.join("result.json").exists());
    let t = Temp::new();
    let dir = task::prepare(&t.0, Tier::Small, Mode::Reference).unwrap();
    fs::write(dir.join("result.json"), b"{}").unwrap();
    let out = Command::new(exe())
        .arg("scale-model-task")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("model_task_result_exists"));
}
#[test]
fn killed_reference_leaves_no_authoritative_result_or_parent_artifacts() {
    let t = Temp::new();
    let dir = task::prepare(&t.0, Tier::Full, Mode::Reference).unwrap();
    let mut child = Command::new(exe())
        .arg("scale-model-task")
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !dir.join("cpu-targets.jsonl.tmp").exists() {
        assert!(child.try_wait().unwrap().is_none());
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(!dir.join("result.json").exists());
    assert!(task::validate(&dir).is_err());
    assert!(!t.0.join("cpu-final-engine.json").exists());
}
#[test]
fn streamed_reference_keeps_frozen_small_oracle_and_rejects_corrupted_artifact() {
    let t = Temp::new();
    let dir = task::prepare(&t.0, Tier::Small, Mode::Reference).unwrap();
    let out = Command::new(exe())
        .arg("scale-model-task")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result = task::validate(&dir).unwrap();
    assert_eq!(result.artifacts.len(), 3);
    let report: Value =
        serde_json::from_slice(&fs::read(dir.join("cpu-report.json")).unwrap()).unwrap();
    assert_eq!(report["completed_hour_targets"], 720);
    assert_eq!(report["air_windows"], 724);
    assert_eq!(report["counts"]["claims"], 2000);
    assert_eq!(report["reference_arithmetic_and_checkpoint"], "passed");
    // The external report must preserve the old Value representation of the
    // actual restored numerical state, including f32 extrema widened to f64.
    let engine = mycity_core::transaction::Engine::restore_checkpoint(
        &fs::read(dir.join("cpu-final-engine.json")).unwrap(),
    )
    .unwrap();
    let legacy_field =
        serde_json::to_value(engine.background().unwrap().field().unwrap().report()).unwrap();
    // Both sides cross the old JSON wire. serde_json without float_roundtrip
    // can shift f64 by one ULP during parsing; do not compare parsed and in-memory values.
    let expected_field: Value =
        serde_json::from_slice(&serde_json::to_vec(&legacy_field).unwrap()).unwrap();
    assert_eq!(report["field"], expected_field);
    let golden = fs::read_to_string(dir.join("cpu-targets.jsonl")).unwrap();
    assert_eq!(golden.lines().count(), 723);
    let mut bytes = fs::read(dir.join("cpu-final-engine.json")).unwrap();
    bytes[0] ^= 1;
    fs::write(dir.join("cpu-final-engine.json"), bytes).unwrap();
    assert!(task::validate(&dir)
        .unwrap_err()
        .contains("model_task_artifact_integrity"));
}
#[test]
fn fingerprint_enforces_the_bound_and_exact_bytes() {
    let t = Temp::new();
    let file = t.0.join("bytes");
    fs::write(&file, vec![31u8; 65537]).unwrap();
    assert!(task::fingerprint(&file, 65536).is_err());
    assert_eq!(
        task::fingerprint(&file, 65537).unwrap(),
        (65537, mycity_core::sha256(&vec![31; 65537]))
    );
}
#[test]
fn killing_supervisor_closes_parent_pipe_and_stops_reference_child() {
    let t = Temp::new();
    let root = t.0.join("probe");
    let mut parent = Command::new(exe())
        .arg("scale-model-reference-probe")
        .arg(&root)
        .arg("full")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let dir = root.join("reference");
    let began = Instant::now();
    while !dir.join("cpu-targets.jsonl.tmp").exists() {
        assert!(parent.try_wait().unwrap().is_none());
        assert!(began.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    let child: Value = serde_json::from_slice(&fs::read(dir.join("child.json")).unwrap()).unwrap();
    let pid = child["pid"].as_u64().unwrap();
    parent.kill().unwrap();
    assert!(!parent.wait().unwrap().success());
    let began = Instant::now();
    loop {
        let out = Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "stat="])
            .output()
            .unwrap();
        let state = String::from_utf8(out.stdout).unwrap();
        if state.trim().is_empty() || state.trim().starts_with('Z') {
            break;
        }
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "orphan pid={pid} state={state}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(task::validate(&dir).is_err());
    assert!(!root.join("cpu-final-engine.json").exists());
}

#[test]
fn fixed_before_reopen_borrows_real_pool_pages_and_returns_only_after_wait() {
    use mycity_headless::{
        normal_pool::{broker::PendingRegistration, Process},
        sim_client::Client,
    };
    let t = Temp::new();
    let mut sim = Client::new();
    sim.start_scale(
        std::path::Path::new(exe()),
        &t.0.join("world"),
        true,
        false,
        Tier::Small,
    )
    .unwrap();
    assert_eq!(sim.next(Duration::from_secs(10)).unwrap().kind, "ready");
    sim.submit(1, mycity_headless::sim_host::Command::Checkpoint)
        .unwrap();
    let reply = sim.next(Duration::from_secs(10)).unwrap();
    assert!(reply.payload.get("Ok").is_some(), "{reply:?}");
    sim.stop().unwrap();
    let mut pool = Process::spawn(std::path::Path::new(exe()), &t.0.join("pool"), None).unwrap();
    let dir = task::prepare(&t.0, Tier::Small, Mode::BeforeReopen).unwrap();
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    let mut supervisor = registration.resolve().unwrap();
    let mut cmd = Command::new(exe());
    supervisor.configure(&mut cmd).unwrap();
    let child = cmd
        .arg("scale-model-task")
        .arg(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    supervisor.bind(child.id()).unwrap();
    let pid = child.id();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records = || {
        fs::read_to_string(pool.root().join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .collect::<Vec<_>>()
    };
    // Even successful process exit is not credited until the real parent's explicit wait/revoke.
    assert!(!records()
        .iter()
        .any(|v| v["kind"] == "returned_after_helper_wait_or_cancel"));
    supervisor.after_sim_wait(pid, output.status).unwrap();
    task::validate(&dir).unwrap();
    let workspace: Value =
        serde_json::from_slice(&fs::read(dir.join("workspace.json")).unwrap()).unwrap();
    assert_eq!(workspace["owner_capacity_bytes"], 4 * 1024 * 1024);
    assert_eq!(workspace["available_touched_bytes"], 4 * 1024 * 1024);
    assert_eq!(workspace["full_balance_scan_passed"], true);
    let events = records();
    let names: Vec<_> = events
        .iter()
        .filter(|v| v["owner_kind"] == "helper_compare")
        .map(|v| v["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "prepared_before_helper_spawn",
            "helper_actual_compare_slots_filled",
            "retiring_after_helper_wait_or_cancel",
            "returned_after_helper_wait_or_cancel"
        ]
    );
    pool.finish().unwrap();
}
#[test]
fn helper_killed_before_allocation_keeps_grant_until_real_wait_and_can_cancel_unspawned() {
    use mycity_headless::normal_pool::{broker::PendingRegistration, Process};
    let t = Temp::new();
    let mut pool = Process::spawn(std::path::Path::new(exe()), &t.0.join("pool"), None).unwrap();
    // An unspawned supervisor Drop must use Cancel, not fake a successful wait.
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    drop(registration.resolve().unwrap());
    let dir = task::prepare(&t.0, Tier::Small, Mode::BeforeReopen).unwrap();
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    let mut supervisor = registration.resolve().unwrap();
    let mut cmd = Command::new(exe());
    supervisor.configure(&mut cmd).unwrap();
    // Parent pipe gate keeps this real child before Owner allocation/Filled.
    let mut child = cmd
        .arg("scale-model-task")
        .arg(&dir)
        .arg("parent-pipe")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    supervisor.bind(child.id()).unwrap();
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    assert!(registration.resolve().is_err());
    supervisor.after_sim_wait(child.id(), status).unwrap();
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    drop(registration.resolve().unwrap());
    assert!(!dir.join("result.json").exists());
    pool.finish().unwrap();
}

#[test]
fn filled_helper_death_and_parent_sigkill_never_credit_without_actual_wait() {
    use mycity_headless::normal_pool::{broker::PendingRegistration, Process};
    use std::io::Write;
    use std::os::unix::process::ExitStatusExt;
    // The same harness runs this small supervisor in a real separate process.
    if let Some(root) = std::env::var_os("MYCITY_HELPER_PARENT_DRIVER") {
        let root = PathBuf::from(root);
        let mut registration = PendingRegistration::new_helper(&root.join("pool")).unwrap();
        let mut supervisor = registration.resolve().unwrap();
        let dir = root.join("before-reopen");
        let mut cmd = Command::new(exe());
        supervisor.configure(&mut cmd).unwrap();
        let mut child = cmd
            .arg("scale-model-task")
            .arg(&dir)
            .arg("parent-pipe")
            .env("MYCITY_HELPER_COMPARE_PROBE", "filled")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        supervisor.bind(child.id()).unwrap();
        child.stdin.as_mut().unwrap().write_all(b"1").unwrap();
        fs::write(root.join("helper-pid"), child.id().to_string()).unwrap();
        // Parent is deliberately SIGKILLed by the outer test; no Drop or fake wait.
        loop {
            // A normal timeout/failure still reaps; the intended test SIGKILL
            // happens while the helper is gated and therefore never reaches this.
            if let Some(status) = child.try_wait().unwrap() {
                supervisor.after_sim_wait(child.id(), status).unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let t = Temp::new();
    let mut pool = Process::spawn(std::path::Path::new(exe()), &t.0.join("pool"), None).unwrap();
    let dir = task::prepare(&t.0, Tier::Small, Mode::BeforeReopen).unwrap();
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    let mut supervisor = registration.resolve().unwrap();
    let mut cmd = Command::new(exe());
    supervisor.configure(&mut cmd).unwrap();
    let mut child = cmd
        .arg("scale-model-task")
        .arg(&dir)
        .env("MYCITY_HELPER_COMPARE_PROBE", "filled")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    supervisor.bind(child.id()).unwrap();
    let wait_filled = |dir: &std::path::Path| {
        let start = Instant::now();
        while !dir.join("filled-probe.json").exists() {
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    wait_filled(&dir);
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(9));
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    assert!(registration.resolve().is_err());
    supervisor.after_sim_wait(child.id(), status).unwrap();
    supervisor.after_sim_wait(child.id(), status).unwrap(); // idempotent credit
    pool.finish().unwrap();
    let lines = fs::read_to_string(t.0.join("pool/events.jsonl")).unwrap();
    assert_eq!(
        lines
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|v| v["kind"] == "returned_after_helper_wait_or_cancel")
            .count(),
        1
    );

    let orphan = Temp::new();
    let mut pool =
        Process::spawn(std::path::Path::new(exe()), &orphan.0.join("pool"), None).unwrap();
    let dir = task::prepare(&orphan.0, Tier::Small, Mode::BeforeReopen).unwrap();
    let mut parent = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "filled_helper_death_and_parent_sigkill_never_credit_without_actual_wait",
        ])
        .env("MYCITY_HELPER_PARENT_DRIVER", &orphan.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_filled(&dir);
    parent.kill().unwrap();
    assert_eq!(parent.wait().unwrap().signal(), Some(9));
    // EOF asks helper to exit but is never accepted as proof that grants can return.
    std::thread::sleep(Duration::from_millis(100));
    let mut registration = PendingRegistration::new_helper(pool.root()).unwrap();
    assert!(registration.resolve().is_err());
    assert!(!dir.join("result.json").exists());
    assert!(pool.finish().unwrap_err().contains("normal_pool_exit"));
    let lines = fs::read_to_string(orphan.0.join("pool/events.jsonl")).unwrap();
    assert!(!lines
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .any(|v| v["kind"] == "returned_after_helper_wait_or_cancel"));
}
