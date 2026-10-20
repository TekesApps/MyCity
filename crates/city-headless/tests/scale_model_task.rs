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
        let path = std::env::temp_dir().join(format!(
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
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn exe() -> &'static str {
    env!("CARGO_BIN_EXE_mycity-headless")
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
