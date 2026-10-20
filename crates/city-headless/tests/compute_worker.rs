use mycity_core::{
    air::{Input, Source, State, CHECKPOINT_BYTES, STRATEGY},
    sha256,
};
use mycity_headless::compute_worker::{self as protocol, Request, Stage, Ticket, Worker, PROTOCOL};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mycity-compute-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn task_root(&self) -> PathBuf {
        let p = self.0.join("tasks");
        fs::create_dir(&p).unwrap();
        fs::create_dir(p.join("inputs")).unwrap();
        fs::create_dir(p.join("candidates")).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_mycity-headless"))
}
fn state() -> State {
    State::new(Input {
        schema: STRATEGY.into(),
        side: 32,
        target: 120,
        initial_micro: 2_000_000,
        boundary_micro: [Some(2_000_000); 4],
        boundary_until: 120,
        sources: vec![Source {
            id: 1,
            cell: 528,
            start: 0,
            end: 120,
            rate_micro: 10000,
        }],
    })
    .unwrap()
}
fn request(dir: &Path, state: &State) -> Request {
    Request {
        protocol: PROTOCOL.into(),
        strategy: STRATEGY.into(),
        ticket: Ticket {
            world: 1,
            branch: 1,
            job: 1,
            incarnation: 1,
            generation: 1,
            attempt: 1,
            base_revision: 0,
            field_version: 0,
            start: 0,
            end: 120,
            rules_sha256: sha256(b"test-rules"),
            source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
            input_sha256: state.input().identity(),
            checkpoint_sha256: protocol::publish(&dir.join("inputs"), &state.checkpoint().unwrap())
                .unwrap(),
        },
    }
}
#[test]
fn actual_persistent_worker_and_gated_sigkill_recovery_match_full_cpu_state() {
    let t = Temp::new();
    let out = Command::new(executable())
        .arg("worker-probe")
        .arg(t.0.join("probe"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["two_jobs_same_process"], true);
    assert_eq!(v["source_sha256"], env!("MYCITY_SOURCE_SHA256"));
    assert_eq!(v["continuous_report"]["time"], 180);
    assert_eq!(v["continuous_report"]["emitted_micro"], 600000);
    for fault in v["faults"].as_array().unwrap() {
        assert_eq!(fault["termination"], "signal");
        assert_ne!(fault["killed_pid"], fault["recovered_pid"]);
        assert_eq!(fault["recomputed_exact_cpu_state"], true);
    }
    assert_eq!(v["faults"].as_array().unwrap().len(), 3);
    assert_eq!(v["world_store_integration"], "not_implemented");
    assert_eq!(v["memory_16gb_gate"], "not_run");
}
#[test]
fn stale_source_unknown_schema_wrong_input_and_attempt_overflow_fail_without_candidate() {
    for name in [
        "source",
        "schema",
        "input",
        "attempt",
        "start",
        "checkpoint",
        "unknown",
    ] {
        let t = Temp::new();
        let root = t.task_root();
        let state = state();
        let r = request(&root, &state);
        let mut value = serde_json::to_value(r).unwrap();
        match name {
            "source" => value["ticket"]["source_sha256"] = json!("0".repeat(64)),
            "schema" => value["protocol"] = json!("future-worker"),
            "input" => value["ticket"]["input_sha256"] = json!("0".repeat(64)),
            "attempt" => value["ticket"]["attempt"] = json!(4),
            "start" => value["ticket"]["start"] = json!(1),
            "checkpoint" => value["ticket"]["checkpoint_sha256"] = json!("../not-an-asset"),
            "unknown" => value["extra"] = json!(1),
            _ => unreachable!(),
        }
        let mut child = Command::new(executable())
            .arg("compute-worker")
            .arg(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(!out.status.success(), "{name}");
        assert!(out.stdout.is_empty());
        assert_eq!(fs::read_dir(root.join("candidates")).unwrap().count(), 0);
    }
}
#[test]
fn oversized_or_truncated_control_and_directory_quota_fail_closed() {
    for bytes in [
        vec![b' '; protocol::CONTROL_BYTES + 1],
        b"{\"protocol\":".to_vec(),
    ] {
        let t = Temp::new();
        let root = t.task_root();
        let mut child = Command::new(executable())
            .arg("compute-worker")
            .arg(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let _ = child.stdin.take().unwrap().write_all(&bytes);
        assert!(!child.wait_with_output().unwrap().status.success());
        assert_eq!(fs::read_dir(root.join("candidates")).unwrap().count(), 0);
    }
    let t = Temp::new();
    let root = t.task_root();
    for n in 0..33 {
        fs::write(root.join("inputs").join(format!("{n}.bin")), b"x").unwrap();
    }
    let out = Command::new(executable())
        .arg("compute-worker")
        .arg(root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
}
#[test]
fn complete_but_forged_finite_candidate_is_rejected_by_full_cpu_check() {
    let t = Temp::new();
    let root = t.task_root();
    let base = state();
    let request = request(&root, &base);
    let mut reference = base.clone();
    while !reference.complete() {
        reference.page(8192, 1024).unwrap();
    }
    let good = reference.checkpoint().unwrap();
    assert_eq!(
        protocol::verify_candidate(&request, &base, &good).unwrap(),
        reference
    );
    assert!(protocol::verify_candidate(&request, &base, &base.checkpoint().unwrap()).is_err());
    let mut value = serde_json::to_value(&reference).unwrap();
    value["field"][0] = json!(3f32.to_bits());
    let forged: State = serde_json::from_value(value).unwrap();
    assert_eq!(
        protocol::verify_candidate(&request, &base, &forged.checkpoint().unwrap()).unwrap_err(),
        "worker_cpu_mismatch"
    );
    let mut wrong = request.clone();
    wrong.ticket.checkpoint_sha256 = "0".repeat(64);
    assert!(protocol::verify_candidate(&wrong, &base, &good).is_err());
    let hash = protocol::publish(&root.join("candidates"), &good).unwrap();
    assert_eq!(
        hash,
        protocol::publish(&root.join("candidates"), &good).unwrap()
    );
    fs::write(
        root.join("candidates").join(format!("{hash}.air")),
        b"corrupt",
    )
    .unwrap();
    assert!(protocol::publish(&root.join("candidates"), &good).is_err());
    assert_eq!(
        fs::read(root.join("candidates").join(format!("{hash}.air"))).unwrap(),
        b"corrupt"
    );
    let huge = root.join("inputs").join(format!("{}.air", "0".repeat(64)));
    fs::File::create(huge)
        .unwrap()
        .set_len(CHECKPOINT_BYTES as u64 + 1)
        .unwrap();
    assert!(protocol::read_asset(&root.join("inputs"), &"0".repeat(64)).is_err());
}
#[test]
fn stopping_worker_without_draining_its_response_reaps_child() {
    let t = Temp::new();
    let root = t.task_root();
    let base = state();
    let request = request(&root, &base);
    let mut worker = Worker::spawn(executable(), &root).unwrap();
    worker.submit(&request).unwrap();
    assert!(worker.submit(&request).is_err());
    let mut completed = false;
    for _ in 0..4 {
        let e = worker.next(Duration::from_secs(5)).unwrap();
        if e.stage == Stage::Complete {
            completed = true;
            break;
        }
    }
    assert!(completed);
    worker.submit(&request).unwrap();
    // Stop without draining this second response. A bounded reader must not hang its join.
    let status = worker.stop().unwrap();
    assert!(status.code().is_none());
}
