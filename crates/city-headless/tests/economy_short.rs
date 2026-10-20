use serde_json::Value;
use std::{fs, process::Command};
fn call(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(args)
        .env("MYCITY_DIAGNOSTICS", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn original_short_cases_match_complete_historical_worlds_and_cold_restore() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/m0-t13a-pe1-short/reference-worlds.json"
    ))
    .unwrap();
    let root = std::env::temp_dir().join(format!("mycity-short-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for (kind, key, successful, active) in [
        ("debt", "debt_exit", 4, 4),
        ("prepay", "prepaid_cancel", 27, 26),
        ("refund", "refund_shortfall", 28, 27),
        ("suspend", "suspended_shift", 49, 49),
    ] {
        let dir = root.join(kind);
        let r = call(&[
            "economy-short-probe",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/m0-economy-opening.json"
            ),
            dir.to_str().unwrap(),
            kind,
        ]);
        assert_eq!(r["report"]["world"], reference[key]);
        assert_eq!(r["successful_transactions"], successful);
        assert_eq!(r["active_transactions"], active);
        assert_eq!(
            r["report"]["committed_sequence"],
            successful + u64::from(kind == "debt")
        );
        let restored = call(&["economy-short-restore", dir.to_str().unwrap()]);
        for k in ["world", "state_sha256", "committed_sequence"] {
            assert_eq!(restored[k], r["report"][k]);
        }
        let again = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
            .args([
                "economy-short-probe",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../fixtures/m0-economy-opening.json"
                ),
                dir.to_str().unwrap(),
                kind,
            ])
            .output()
            .unwrap();
        assert!(
            !again.status.success(),
            "existing world must not be overwritten"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
