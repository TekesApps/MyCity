use serde_json::Value;
use std::{
    fs,
    process::{Command, Stdio},
};
#[test]
fn lifetime_heap_peak_survives_release_and_concurrent_records_do_not_interleave() {
    let root = std::env::temp_dir().join(format!("mycity-heap-meter-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let log = root.join("heaps.jsonl");
    let mut children = Vec::new();
    for id in 0..2 {
        children.push(
            Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
                .arg("heap-meter-probe")
                .arg(root.join(id.to_string()))
                .env("MYCITY_HEAP_LOG", &log)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for c in &mut children {
        assert!(c.wait().unwrap().success());
    }
    let records: Vec<Value> = fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0]["pid"], records[1]["pid"]);
    for (id, child) in children.iter().enumerate() {
        let record = records.iter().find(|v| v["pid"] == child.id()).unwrap();
        assert_eq!(record["success"], true);
        assert_eq!(record["role"], "heap-meter-probe");
        let data: Value = serde_json::from_slice(
            &fs::read(root.join(id.to_string()).join("report.json")).unwrap(),
        )
        .unwrap();
        let initial = data[0]["current_requested_bytes"].as_u64().unwrap();
        let live = data[1]["current_requested_bytes"].as_u64().unwrap();
        let released = data[2]["current_requested_bytes"].as_u64().unwrap();
        assert_eq!(live - initial, 2 * 1024 * 1024);
        assert_eq!(live - released, 2 * 1024 * 1024);
        assert!(
            record["heap"]["lifetime_peak_requested_bytes"]
                .as_u64()
                .unwrap()
                >= live
        );
    }
    fs::remove_dir_all(root).unwrap();
}
