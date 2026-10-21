//! Real child ownership and fixed windows; tests do not accept formal steady.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static N: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mc-steady-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("steady_fixture_retained:{}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn exe() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_mycity-headless"))
}
fn load(p: &Path) -> Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}
#[test]
fn actual_full_l0_cases_fit_original_region_and_verify_all_existing_assertions() {
    let t = Temp::new();
    for kind in ["economy", "boot", "exchange"] {
        let dir = t.0.join(kind);
        fs::create_dir(&dir).unwrap();
        let mut c = Command::new(exe())
            .args(["steady-l0-case", kind])
            .arg(&dir)
            .env("MYCITY_DIAGNOSTICS", "1")
            .env_remove("MYCITY_NORMAL_POOL")
            .stdin(Stdio::piped())
            .stdout(fs::File::create(dir.join("stdout")).unwrap())
            .stderr(fs::File::create(dir.join("stderr")).unwrap())
            .spawn()
            .unwrap();
        c.stdin.as_mut().unwrap().write_all(b"1").unwrap();
        let start = Instant::now();
        loop {
            if let Some(s) = c.try_wait().unwrap() {
                assert!(
                    s.success(),
                    "{kind}: {s}; {}",
                    fs::read_to_string(dir.join("stderr")).unwrap()
                );
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(180));
            std::thread::sleep(Duration::from_millis(20));
        }
        let r = load(&dir.join("case-summary.json"));
        assert_eq!(r["completed_case"], true);
        assert_eq!(r["safe_prefix"], false);
    }
}
#[test]
fn actual_short_l0_window_stops_only_at_safe_boundary_and_never_accepts_formal() {
    let t = Temp::new();
    let dir = t.0.join("run");
    let out = Command::new(exe())
        .arg("steady-run")
        .arg(&dir)
        .args(["l0", "smoke"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = load(&dir.join("report.json"));
    assert_eq!(r["formal"], false);
    assert_eq!(r["experiment_completed"], true);
    assert_eq!(r["three_rounds"], "not_accepted_by_single_round");
    let rows: Vec<Value> = fs::read_to_string(dir.join("business/window-events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let m = rows
        .iter()
        .find(|r| r["kind"] == "measurement_started")
        .unwrap();
    assert!(m["elapsed_ns"].as_u64().unwrap() >= 3_000_000_000);
    assert_eq!(m["data"]["fixed_start_ns"], 3_000_000_000u64);
    assert!(rows
        .iter()
        .any(|r| r["kind"] == "case_released" && r["data"]["safe_prefix"] == true));
    let driver = load(&dir.join("business/report.json"));
    assert_eq!(driver["business_within_measurement_window"], false);
    assert_eq!(driver["completed_l0_cases"]["complete_in_drain"][0], 1);
    assert!(!rows.iter().any(|r| r["kind"] == "case_released"
        && r["data"]["allow_prefix"] == false
        && r["data"]["safe_prefix"] == true));
    let map = load(&dir.join("clock-map.json"));
    assert!(map["uncertainty_ns"].as_i64().unwrap() >= 0);
}
#[test]
fn actual_owner_eof_is_failure_and_cannot_be_safe_prefix_success() {
    let t = Temp::new();
    let mut c = Command::new(exe())
        .args(["steady-l0-case", "economy"])
        .arg(&t.0)
        .env_remove("MYCITY_NORMAL_POOL")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut p = c.stdin.take().unwrap();
    p.write_all(b"1").unwrap();
    drop(p);
    assert!(!c.wait().unwrap().success());
    assert!(!t.0.join("case-summary.json").exists());
}
