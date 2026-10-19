use std::process::Command;

#[test]
fn offline_runs_retain_provenance_and_do_not_claim_simulation() {
    let input = format!(
        "{}/../../fixtures/m0-bootstrap.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let run = || {
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
            .args(["baseline", &input])
            .output()
            .unwrap();
        assert!(out.status.success());
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()
    };
    let a = run();
    let b = run();
    for key in [
        "source_sha256",
        "lock_sha256",
        "input_sha256",
        "rules_sha256",
    ] {
        assert_eq!(a[key], b[key]);
        assert_eq!(a[key].as_str().unwrap().len(), 64);
    }
    assert_ne!(a["run_id"], b["run_id"]);
    assert_eq!(a["economic_prototypes"], "not_run");
    assert_eq!(a["memory_16gb_gate"], "not_run");
}

#[test]
fn bad_invocation_and_missing_input_exit_nonzero() {
    for args in [vec![], vec!["baseline", "/not-a-mycity-file"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
            .args(args)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn opening_cli_assembles_real_bounded_records_without_running_the_economy() {
    let input = format!(
        "{}/../../fixtures/m0-e0-opening.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(["assemble", &input])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["world"]["accounts"].as_array().unwrap().len(), 4);
    assert_eq!(v["world"]["stocks"][0]["basis"], 240000);
    assert_eq!(v["economic_prototypes"], "not_run");
    assert_eq!(v["world"]["revision"], 0);
}

#[test]
fn transaction_probe_exercises_model_without_claiming_durable_success() {
    let input = format!(
        "{}/../../fixtures/m0-e0-opening.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(["transaction-probe", &input])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["transaction_probe"]["durability"], "volatile_only");
    assert_eq!(v["transaction_probe"]["resources"][1]["reserved"], 0);
    assert_eq!(
        v["transaction_probe"]["world"]["accounts"][1]["cash"],
        190000
    );
    assert_eq!(v["memory_16gb_gate"], "not_run");
}

#[test]
fn clock_probe_pages_twenty_thousand_sources_and_does_not_claim_disk_recovery() {
    let input = format!(
        "{}/../../fixtures/m0-bootstrap.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(["clock-probe", &input])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["clock_probe"]["processed"], 20000);
    assert_eq!(v["clock_probe"]["max_ready_page"], 8192);
    assert_eq!(v["clock_probe"]["persistent_recovery"], "not_run");
}

#[test]
fn storage_cli_confirms_persistent_operation_but_not_full_city_recovery() {
    let parent = std::env::temp_dir().join(format!(
        "mycity-cli-store-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&parent).unwrap();
    let world = parent.join("world");
    let input = format!(
        "{}/../../fixtures/m0-e0-opening.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(["storage-probe", &input, world.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_dir_all(parent).unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["storage_probe"]["committed_sequence"], 2);
    assert_eq!(v["storage_probe"]["replay_has_persistent_receipt"], true);
    assert_eq!(v["storage_probe"]["power_loss"], "not_tested");
}

#[test]
fn spatial_cli_imports_actual_png_and_validates_geometry_without_claiming_city_persistence() {
    let root = format!("{}/../../", env!("CARGO_MANIFEST_DIR"));
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
            .args([
                "spatial-probe",
                &format!("{root}fixtures/m0-spatial.json"),
                &format!("{root}fixtures/terrain/m0-flat16.png"),
            ])
            .output()
            .unwrap()
    };
    let a = run();
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    let b = run();
    assert!(b.status.success());
    let a: serde_json::Value = serde_json::from_slice(&a.stdout).unwrap();
    let b: serde_json::Value = serde_json::from_slice(&b.stdout).unwrap();
    assert_eq!(a["spatial_probe"], b["spatial_probe"]);
    assert_eq!(a["spatial_probe"]["terrain_vertices"], 66049);
    assert_eq!(a["spatial_probe"]["store_integration"], "not_implemented");
    assert_eq!(a["economic_prototypes"], "not_run");
    assert_eq!(a["memory_16gb_gate"], "not_run");
}

#[test]
fn spatial_store_cli_restores_cpu_assets_and_money_without_claiming_mutable_construction() {
    let root = format!("{}/../../", env!("CARGO_MANIFEST_DIR"));
    let parent = std::env::temp_dir().join(format!(
        "mycity-cli-spatial-store-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&parent).unwrap();
    let world = parent.join("world");
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args([
            "spatial-store-probe",
            &format!("{root}fixtures/m0-e0-opening.json"),
            &format!("{root}fixtures/m0-spatial.json"),
            &format!("{root}fixtures/terrain/m0-flat16.png"),
            world.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_dir_all(parent).unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let p = &v["spatial_store_probe"];
    assert_eq!(p["committed_sequence"], 1);
    assert_eq!(p["terrain_vertices"], 66049);
    assert_eq!(p["retry"]["has_persistent_receipt"], true);
    assert_eq!(
        p["space"],
        "immutable_genesis_assets_only_not_terrain_edits"
    );
    assert_eq!(v["economic_prototypes"], "not_run");
    assert_eq!(v["memory_16gb_gate"], "not_run");
    assert_eq!(v["rules"]["storage"]["schema"], "mycity-m0-genesis-v2");
}

#[test]
fn economy_cli_executes_real_30_day_cash_goods_and_failure_branches() {
    let input = format!(
        "{}/../../fixtures/m0-economy-opening.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let dir = std::env::temp_dir().join(format!(
        "mycity-economy-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let out = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .arg("economy-probe")
        .arg(&input)
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["economic_prototypes"], "P-E0_and_P-E1_executed");
    let p = &v["economy_probe"];
    assert_eq!(p["pe0"]["world"]["economy"]["closed_days"], 30);
    let accounts = p["pe0"]["world"]["accounts"].as_array().unwrap();
    for (id, cash) in [(1, 808000), (2, 56000), (3, 816000), (4, 1320000)] {
        assert_eq!(
            accounts.iter().find(|a| a["id"] == id).unwrap()["cash"],
            cash
        );
    }
    assert_eq!(
        p["pe1"]["refund_shortfall"]["world"]["economy"]["claims"][1]["paid"],
        1000
    );
    assert_eq!(p["P_BOOT"], "not_run");
    assert_eq!(v["memory_16gb_gate"], "not_run");
    assert!(p["pe0"]["spatial_reference"].is_object());
    assert!(p["pe0"]["checkpoints"].as_array().unwrap().len() >= 4);
    std::fs::remove_dir_all(dir).unwrap();
}
