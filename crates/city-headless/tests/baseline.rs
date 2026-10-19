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
