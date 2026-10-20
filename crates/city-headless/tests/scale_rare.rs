use mycity_headless::scale_rare;
use serde_json::Value;
use std::{fs, path::Path, process::Command};
fn call(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn read(p: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}
#[test]
fn actual_full_anchors_restore_real_sim_trials_and_preserve_cache_charge() {
    let root = std::env::temp_dir().join(format!("mycity-rare-trials-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let anchors = root.join("anchors");
    let report = call(&["scale-rare-anchors", anchors.to_str().unwrap()]);
    assert_eq!(report["claims_anchor"]["at"], 23 * 3600);
    assert_eq!(
        report["source_anchor"]["root"]["worker_cache_bytes"],
        6 * 1024 * 1024
    );
    for kind in ["claims", "source"] {
        let branch = root.join(kind);
        let r = call(&[
            "scale-rare-trial",
            anchors.to_str().unwrap(),
            branch.to_str().unwrap(),
            kind,
        ]);
        assert_eq!(r["actual_engine_sha256"], r["expected_engine_sha256"]);
        assert_eq!(r["cold_reopen_equal"], true);
        assert_eq!(r["root"]["worker_cache_bytes"], 6 * 1024 * 1024);
        assert_eq!(
            report[&format!("{kind}_anchor")]["root"]["worker_cache_bytes"],
            r["root"]["worker_cache_bytes"]
        );
        let reopened = call(&[
            "scale-rare-restore",
            branch.join("world").to_str().unwrap(),
            anchors
                .join(kind)
                .join("expected-engine.json")
                .to_str()
                .unwrap(),
        ]);
        assert_eq!(reopened["cold_archive_restore"], "passed");
        assert_eq!(reopened["actual_engine_sha256"], r["actual_engine_sha256"]);
        let mut claims = 0;
        let mut sim_sources = 0;
        let mut worker_sources = 0;
        for entry in fs::read_dir(branch.join("diagnostics")).unwrap() {
            let p = entry.unwrap().path().join("report.json");
            if !p.exists() {
                continue;
            }
            let d = read(p);
            let c = &d["role_compute"][0]["classes"];
            claims += c["L2_claims_batch"]["samples"].as_u64().unwrap();
            let sources = c["air_source_page"]["samples"].as_u64().unwrap();
            if d["producer"] == "sim" {
                sim_sources += sources;
            } else if d["producer"] == "worker" {
                worker_sources += sources;
            }
        }
        assert_eq!(claims, u64::from(kind == "claims"));
        assert_eq!(sim_sources, if kind == "source" { 2 } else { 0 });
        assert_eq!(worker_sources, if kind == "source" { 2 } else { 0 });
    }
    let original = anchors.join("source/world");
    let rules = format!("sim-t13a-full-v2:{}", env!("MYCITY_SOURCE_SHA256"));
    let store = mycity_storage::Store::open(&original, &rules).unwrap();
    assert!(
        scale_rare::copy_world(&original, &root.join("forbidden-copy"))
            .unwrap_err()
            .contains("Locked")
    );
    drop(store);
    assert!(!root.join("forbidden-copy").exists());
    fs::remove_dir_all(root).unwrap();
}
