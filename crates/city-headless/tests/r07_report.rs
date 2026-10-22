use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn metrics_report_error_is_nonzero_and_cannot_publish_success() {
    let root = std::env::temp_dir().join(format!(
        "mycity-r07-report-error-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .arg("layout-r07-session")
        .arg(&root)
        .arg("--report-fault")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("metrics_unknown_class"));
    assert!(!root.join("report.json").exists());
    assert!(!root.join("snapshots").exists());
    assert_eq!(
        fs::metadata(root.join("compute-durations.bin"))
            .unwrap()
            .len(),
        0
    );
    fs::remove_dir_all(root).unwrap();
}
