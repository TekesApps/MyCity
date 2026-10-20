use mycity_headless::group_sampler::Sampler;
use std::{fs, path::PathBuf, process::Command, time::Duration};
#[test]
fn snapshot_sums_all_observed_descendants_and_labels_measurement_overhead() {
    let root: PathBuf =
        std::env::temp_dir().join(format!("mycity-group-sampler-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let world = root.join("world");
    fs::create_dir(&world).unwrap();
    fs::write(world.join("fixture"), [7; 32]).unwrap();
    let mut sampler = Sampler::start(&root.join("samples.jsonl"), &world).unwrap();
    let mut child = Command::new("sleep").arg("0.8").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let stats = sampler.finish().unwrap();
    assert!(stats.snapshots >= 3);
    assert!(stats.observed_pids.contains(&pid));
    assert!(stats.peak_disk_bytes >= 32);
    assert!(stats.peak_experiment_bytes >= stats.peak_disk_bytes);
    let mut peak = 0;
    let mut saw_child = false;
    let mut saw_ps = false;
    for line in fs::read_to_string(root.join("samples.jsonl"))
        .unwrap()
        .lines()
    {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let rows = v["processes"].as_array().unwrap();
        let sum: u64 = rows.iter().map(|p| p["rss_bytes"].as_u64().unwrap()).sum();
        assert_eq!(sum, v["sum_rss_bytes"].as_u64().unwrap());
        peak = peak.max(sum);
        saw_child |= rows.iter().any(|p| p["pid"] == pid);
        saw_ps |= rows.iter().any(|p| p["role"] == "sampler_ps");
    }
    assert!(saw_child && saw_ps);
    assert_eq!(peak, stats.peak_sum_rss_bytes);
    fs::remove_dir_all(root).unwrap();
}
