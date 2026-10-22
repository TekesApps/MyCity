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
pub fn run(root: &Path, vm_trace: bool) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    let start = Instant::now();
    let exe = std::env::current_exe().map_err(err)?;
    fs::write(root.join("manifest.json"),serde_json::to_vec(&serde_json::json!({"schema":"mycity-r07-full-layout-exercise-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"external_owned_charge_bytes":crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,"original_full_L1_records":true,"actual_normal_pool_bytes":crate::normal_pool::BYTES,"snapshot_schedule":"event-driven after prior durable and actual replacement","snapshot_count":2,"formal_acceptance":false})).map_err(err)?).map_err(err)?;
    crate::observer_clock::synchronize(root, || {
        start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    })?;
    let mut pool = crate::normal_pool::Process::spawn_with_vm_trace(
        &exe,
        &root.join("normal-pool"),
        Some(&root.join("heaps.jsonl")),
        vm_trace,
    )?;
    let mut layout = Command::new(&exe)
        .arg("layout-r07-session")
        .arg(root.join("layout"))
        .args(vm_trace.then_some("--vm-trace"))
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

/// Source-only diagnostic candidate: the driver also services the original pool.
/// Children are polled between pool requests/scans, never synchronously waited
/// while they can need the service. Original run() remains a separate control.
pub fn run_combined(root: &Path, vm_trace: bool) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    let start = Instant::now();
    let exe = std::env::current_exe().map_err(err)?;
    let pool_root = root.join("normal-pool");
    fs::write(root.join("manifest.json"), serde_json::to_vec(&serde_json::json!({
        "schema":"mycity-r07-combined-pool-exercise-v1",
        "source_sha256":env!("MYCITY_SOURCE_SHA256"),
        "external_owned_charge_bytes":crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,
        "original_full_L1_records":true,"actual_normal_pool_bytes":crate::normal_pool::BYTES,
        "snapshot_count":2,"formal_acceptance":false,
        "combined_role":"r07-layout-driver and pool service within the existing256KiB root arena",
        "separate_pool_host_process":false,"memory_credit_bytes":0
    })).map_err(err)?).map_err(err)?;
    crate::observer_clock::synchronize(root, || {
        start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    })?;
    let mut layout: Option<crate::joint_driver::Job> = None;
    let mut cold: Option<crate::joint_driver::Job> = None;
    let mut layout_waited = false;
    let snapshot = root.join("layout/snapshots/snapshot-000002");
    crate::normal_pool::serve_controlled(&pool_root, vm_trace, |balance| {
        if !layout_waited {
            if layout.is_none() {
                let mut cmd = Command::new(&exe);
                cmd.arg("layout-r07-session").arg(root.join("layout"));
                if vm_trace {
                    cmd.arg("--vm-trace");
                }
                layout = Some(crate::joint_driver::Job(
                    cmd.env("MYCITY_NORMAL_POOL", &pool_root)
                        .stdin(Stdio::null())
                        .stdout(File::create_new(root.join("layout.stdout")).map_err(err)?)
                        .stderr(File::create_new(root.join("layout.stderr")).map_err(err)?)
                        .spawn()
                        .map_err(err)?,
                ));
            }
            if !crate::joint_driver::done(layout.as_mut().ok_or("r07_combined_layout_owner")?)? {
                return Ok(false);
            }
            // Job::drop also waits, retaining unique ownership on error paths.
            drop(layout.take());
            layout_waited = true;
            cold = Some(crate::joint_driver::Job(
                Command::new(&exe)
                    .arg("layout-load-only")
                    .arg(&snapshot)
                    .arg(root.join("cold-with-provider"))
                    .stdin(Stdio::null())
                    .stdout(File::create_new(root.join("cold.stdout")).map_err(err)?)
                    .stderr(File::create_new(root.join("cold.stderr")).map_err(err)?)
                    .spawn()
                    .map_err(err)?,
            ));
            return Ok(false);
        }
        if !crate::joint_driver::done(cold.as_mut().ok_or("r07_combined_cold_owner")?)? {
            return Ok(false);
        }
        drop(cold.take());
        if balance.available_touched_bytes != crate::normal_pool::BYTES
            || balance.granted_capacity_bytes != 0
        {
            return Err("r07_exercise_pool_unbalanced".into());
        }
        Ok(true)
    })?;
    fs::write(
        root.join("exercise-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "source_sha256":env!("MYCITY_SOURCE_SHA256"),
            "layout_and_live_provider_cold_exit_zero":true,
            "actual_seconds":start.elapsed().as_secs_f64(),"latest_snapshot":snapshot,
            "combined_pool_process":true,"memory_credit_bytes":0,
            "formal_acceptance":false,"real_16gb":"not_run","eight_hour":"not_run"
        }))
        .map_err(err)?,
    )
    .map_err(err)
}
