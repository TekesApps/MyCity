use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-air-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/air/run36-cpu.json")
}
fn probe(t: &Temp) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .arg("air-probe")
        .arg(fixture())
        .arg(t.0.join("suite"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn cpu_suite_reopens_five_file_checkpoints_in_new_processes_and_reports_real_scope() {
    let t = Temp::new();
    let v = probe(&t);
    assert_eq!(v["cases"].as_array().unwrap().len(), 5);
    for c in v["cases"].as_array().unwrap() {
        assert_eq!(c["report"]["complete"], true);
        assert_eq!(c["same_state_after_source_and_tile_file_restore"], true);
        assert_eq!(
            c["report"]["field_sha256"],
            c["new_process_resume"]["report"]["field_sha256"]
        );
        assert_eq!(
            c["report"]["integral_sha256"],
            c["new_process_resume"]["report"]["integral_sha256"]
        );
        assert_eq!(c["comparison"]["passed"], true);
        assert!(c["report"]["owned_bytes"].as_u64().unwrap() <= 2 * 1024 * 1024);
    }
    assert_eq!(v["cases"][3]["report"]["time"], 30 * 86400);
    assert_eq!(v["cases"][4]["source_count"], 20000);
    assert_eq!(v["source_sha256"], env!("MYCITY_SOURCE_SHA256"));
    assert_eq!(v["world_store_integration"], "not_implemented");
    assert_eq!(v["memory_16gb_gate"], "not_run");
    assert_eq!(v["gpu"], "not_run");
}
#[test]
fn stale_source_input_corruption_and_existing_output_cannot_publish_a_new_candidate() {
    let t = Temp::new();
    probe(&t);
    let dir = t.0.join("suite/configured");
    let checkpoint = dir.join("tile.air");
    let mut b = fs::read(&checkpoint).unwrap();
    b[20] ^= 1;
    let corrupt = t.0.join("corrupt.air");
    fs::write(&corrupt, &b).unwrap();
    let mut i: Value = serde_json::from_slice(&fs::read(fixture()).unwrap()).unwrap();
    i["initial_micro"] = json!(2_000_001);
    let wrong = t.0.join("wrong.json");
    fs::write(&wrong, serde_json::to_vec(&i).unwrap()).unwrap();
    for (input, cp) in [(fixture(), corrupt.clone()), (wrong, checkpoint.clone())] {
        let target = t.0.join("must-not-exist.air");
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
            .arg("air-resume")
            .arg(input)
            .arg(cp)
            .arg(&target)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(!target.exists());
    }
    let existing = dir.join("complete.air");
    let before = fs::read(&existing).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .arg("air-resume")
        .arg(fixture())
        .arg(checkpoint)
        .arg(&existing)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(fs::read(existing).unwrap(), before);
    // Correct artifact header, damaged kernel payload: checksum/parse rejection, no output.
    let mut b = fs::read(dir.join("source.air")).unwrap();
    let len = b.len();
    b[len - 2] ^= 1;
    fs::write(&corrupt, b).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .arg("air-resume")
        .arg(fixture())
        .arg(corrupt)
        .arg(t.0.join("must-not-exist.air"))
        .output()
        .unwrap();
    assert!(!out.status.success());
}
