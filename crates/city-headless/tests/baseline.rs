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
