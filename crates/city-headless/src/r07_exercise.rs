//! Original full-L1 protocol exercise with TWO diagnostic snapshots. Not a
//! frozen formal profile: event-driven save schedule exists only to exercise v3.
use std::{
    fs::{self, File},
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn run(root: &Path) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    let start = Instant::now();
    let exe = std::env::current_exe().map_err(err)?;
    fs::write(root.join("manifest.json"),serde_json::to_vec(&serde_json::json!({"schema":"mycity-r07-full-layout-exercise-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"external_owned_charge_bytes":crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,"original_full_L1_records":true,"actual_normal_pool_bytes":crate::normal_pool::BYTES,"snapshot_schedule":"event-driven after prior durable and actual replacement","snapshot_count":2,"formal_acceptance":false})).map_err(err)?).map_err(err)?;
    crate::observer_clock::synchronize(root, || {
        start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    })?;
    let mut pool = crate::normal_pool::Process::spawn(
        &exe,
        &root.join("normal-pool"),
        Some(&root.join("heaps.jsonl")),
    )?;
    let mut layout = Command::new(&exe)
        .arg("layout-r07-session")
        .arg(root.join("layout"))
        .env("MYCITY_NORMAL_POOL", root.join("normal-pool"))
        .stdin(Stdio::null())
        .stdout(File::create_new(root.join("layout.stdout")).map_err(err)?)
        .stderr(File::create_new(root.join("layout.stderr")).map_err(err)?)
        .spawn()
        .map_err(err)?;
    if !layout.wait().map_err(err)?.success() {
        return Err("r07_exercise_layout_failed".into());
    }
    let snapshot = root.join("layout/snapshots/snapshot-000002");
    let cold = Command::new(&exe)
        .arg("layout-load-only")
        .arg(&snapshot)
        .arg(root.join("cold-with-provider"))
        .stdin(Stdio::null())
        .stdout(File::create_new(root.join("cold.stdout")).map_err(err)?)
        .stderr(File::create_new(root.join("cold.stderr")).map_err(err)?)
        .status()
        .map_err(err)?;
    if !cold.success() {
        return Err("r07_exercise_cold_failed".into());
    }
    let balance = crate::normal_pool::status(pool.root())?;
    if balance.available_touched_bytes != crate::normal_pool::BYTES
        || balance.granted_capacity_bytes != 0
    {
        return Err("r07_exercise_pool_unbalanced".into());
    }
    pool.finish()?;
    fs::write(root.join("exercise-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"source_sha256":env!("MYCITY_SOURCE_SHA256"),"layout_and_live_provider_cold_exit_zero":true,"actual_seconds":start.elapsed().as_secs_f64(),"latest_snapshot":snapshot,"formal_acceptance":false,"real_16gb":"not_run","eight_hour":"not_run"})).map_err(err)?).map_err(err)
}
