//! Actual complete role grants and OOM recovery, not formal performance evidence.
#[path = "support/lifecycle_artifacts.rs"]
mod lifecycle_artifacts;
use mycity_core::{
    timeline::Command as T,
    transaction::{Action, Envelope},
};
use mycity_headless::{
    normal_pool::{self, broker::PendingRegistration, Process},
    sim_client::Client,
    sim_host::Command as C,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const MIB: usize = 1024 * 1024;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = lifecycle_artifacts::root().join(format!(
            "city-role-arena-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = lifecycle_artifacts::cleanup(&self.0);
        }
    }
}
fn exe() -> &'static Path {
    lifecycle_artifacts::exe()
}
fn events(p: &Path) -> Vec<Value> {
    fs::read_to_string(p.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
fn call(c: &mut Client, id: u64, command: C) -> Value {
    c.submit(id, command).unwrap();
    loop {
        let e = c.next(Duration::from_secs(10)).unwrap();
        if e.kind == "result" {
            assert!(e.payload.get("Err").is_none(), "{:?}", e);
            return e.payload["Ok"].clone();
        }
    }
}
#[test]
fn sim_full_arena_fills_after_models_and_unknown_durable_oom_recovers_original_ticket() {
    let t = Temp::new();
    let mut pool = Process::spawn(exe(), &t.0.join("pool"), None).unwrap();
    let world = t.0.join("world");
    let mut c = Client::new();
    c.start_in_pool(exe(), &world, true, true, pool.root()).unwrap();
    assert_eq!(c.next(Duration::from_secs(10)).unwrap().kind, "ready");
    let s = normal_pool::status(pool.root()).unwrap();
    assert_eq!(s.granted_capacity_bytes, 59 * MIB);
    assert_eq!(s.available_touched_bytes, 5 * MIB);
    let mut registration = PendingRegistration::new_model(pool.root()).unwrap();
    assert!(registration.resolve().is_err());
    let e = events(pool.root());
    let filled = e
        .iter()
        .find(|e| e["kind"] == "sim_actual_codec_slots_filled")
        .unwrap();
    let a = &filled["arena"]["activation"];
    assert_eq!(a["grant_bytes"], 59 * MIB);
    assert_eq!(a["initialized_bytes"], a["arena_bytes"]);
    assert_eq!(filled["backing_sha256"].as_str().unwrap().len(), 64);
    let envelope = Envelope {
        world: 1,
        branch: 1,
        operation_id: 71,
        expected_revision: 0,
        actions: vec![Action::Timeline { command: T::Enable }],
    };
    c.submit(
        1,
        C::ProbeExhaust {
            envelope: envelope.clone(),
        },
    )
    .unwrap();
    assert!(c.next(Duration::from_secs(10)).is_err());
    c.wait_for_sim_exit(Duration::from_secs(10)).unwrap();
    drop(c);
    let returned = normal_pool::status(pool.root()).unwrap();
    assert_eq!(returned.available_touched_bytes, 64 * MIB);
    assert_eq!(returned.workspace_grants, returned.workspace_returns);
    let mut c = Client::new();
    c.start_in_pool(exe(), &world, false, false, pool.root()).unwrap();
    assert_eq!(c.next(Duration::from_secs(10)).unwrap().kind, "ready");
    let before = call(&mut c, 1, C::Status);
    assert_eq!(before["sequence"], 1);
    let proof = call(&mut c, 2, C::ReceiptProof { operation: 71 });
    assert_eq!(proof["present"], true);
    let retry = call(&mut c, 3, C::Execute { envelope });
    assert_eq!(retry["has_persistent_receipt"], true);
    assert!(retry["model"].get("Ok").is_some());
    let after = call(&mut c, 4, C::Status);
    assert_eq!(before["sequence"], after["sequence"]);
    assert_eq!(before["state_sha256"], after["state_sha256"]);
    c.stop().unwrap();
    assert_eq!(
        normal_pool::status(pool.root())
            .unwrap()
            .available_touched_bytes,
        64 * MIB
    );
    pool.finish().unwrap();
}
#[test]
fn construction_oom_cannot_fill_or_refund_before_the_parent_actually_waits() {
    let t = Temp::new();
    let mut pool = Process::spawn(exe(), &t.0.join("pool"), None).unwrap();
    let mut registration = PendingRegistration::new(pool.root()).unwrap();
    let mut supervisor = registration.resolve().unwrap();
    let mut cmd = Command::new(exe());
    supervisor.configure(&mut cmd).unwrap();
    let child = cmd
        .args(["sim-host", "create"])
        .arg(t.0.join("world"))
        .env("MYCITY_ROLE_CONSTRUCTION_OOM", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    supervisor.bind(pid).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("memory allocation"));
    assert_eq!(
        normal_pool::status(pool.root())
            .unwrap()
            .granted_capacity_bytes,
        59 * MIB
    );
    assert!(!events(pool.root())
        .iter()
        .any(|v| v["kind"] == "sim_actual_codec_slots_filled"));
    supervisor.after_sim_wait(pid, out.status).unwrap();
    assert_eq!(
        normal_pool::status(pool.root())
            .unwrap()
            .available_touched_bytes,
        64 * MIB
    );
    assert!(!t.0.join("world").exists());
    pool.finish().unwrap();
}
#[test]
fn complete_reference_and_sim_sessions_cannot_overlap_and_twenty_mib_is_returned() {
    use mycity_headless::{
        scale_model_task::{self as task, Mode},
        scale_reference::Tier,
    };
    let t = Temp::new();
    let mut pool = Process::spawn(exe(), &t.0.join("pool"), None).unwrap();
    let dir = task::prepare(&t.0, Tier::Small, Mode::Reference).unwrap();
    let mut registration = PendingRegistration::new_model(pool.root()).unwrap();
    let mut supervisor = registration.resolve().unwrap();
    assert_eq!(
        normal_pool::status(pool.root())
            .unwrap()
            .granted_capacity_bytes,
        20 * MIB
    );
    let mut registration = PendingRegistration::new(pool.root()).unwrap();
    assert!(registration.resolve().is_err());
    let mut cmd = Command::new(exe());
    supervisor.configure(&mut cmd).unwrap();
    let child = cmd
        .arg("scale-model-task")
        .arg(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    supervisor.bind(pid).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        normal_pool::status(pool.root())
            .unwrap()
            .granted_capacity_bytes,
        20 * MIB
    );
    supervisor.after_sim_wait(pid, out.status).unwrap();
    task::validate(&dir).unwrap();
    let e = events(pool.root());
    let f = e
        .iter()
        .find(|e| e["kind"] == "helper_actual_compare_slots_filled")
        .unwrap();
    assert_eq!(f["owner_kind"], "model");
    assert_eq!(f["arena"]["activation"]["grant_bytes"], 20 * MIB);
    assert_eq!(
        normal_pool::status(pool.root())
            .unwrap()
            .available_touched_bytes,
        64 * MIB
    );
    pool.finish().unwrap();
}
