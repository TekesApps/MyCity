//! Rust parent keeps the original full L1 present until actual driver drain.
use super::*;
use crate::{
    joint_driver::{self, Job},
    normal_pool,
};
use std::process::{Command, Stdio};
fn spawn(
    dir: &Path,
    name: &str,
    args: &[&std::ffi::OsStr],
    pipe: bool,
    pool: bool,
) -> Result<Job, String> {
    let mut c = Command::new(std::env::current_exe().map_err(err)?);
    c.args(args)
        .env("MYCITY_HEAP_LOG", dir.join("heaps.jsonl"))
        .stdin(if pipe { Stdio::piped() } else { Stdio::null() })
        .stdout(File::create_new(dir.join(format!("{name}.stdout"))).map_err(err)?)
        .stderr(File::create_new(dir.join(format!("{name}.stderr"))).map_err(err)?);
    if pool {
        c.env("MYCITY_NORMAL_POOL", dir.join("normal-pool"));
    } else {
        c.env_remove("MYCITY_NORMAL_POOL");
    }
    if name == "business" {
        c.env("MYCITY_STEADY_PARENT_PIPE", "1");
    }
    c.env_remove("MYCITY_BROKER_SESSION")
        .env_remove("MYCITY_BROKER_SUPERVISOR")
        .env_remove("MYCITY_NORMAL_WORKER_GRANT");
    Ok(Job(c.spawn().map_err(err)?))
}
pub fn run(dir: &Path, scenario: Scenario, plan: Plan) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    save(
        &dir.join("manifest.json"),
        &json!({"schema":"mycity-steady-supervisor-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"scenario":scenario,"formal":plan.formal,
  "warmup_seconds":plan.warmup.as_secs(),"measurement_seconds":plan.measure.as_secs(),"external_owned_charge_bytes":crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,
  "normal_pool_bytes":if scenario==Scenario::L0 {0}else{normal_pool::BYTES},"scope":"fixed original inputs; smoke has original full records but shortened clock and cannot accept steady","formal_acceptance":"requires independent audit"}),
    )?;
    let mut events = Events::new(&dir.join("common-events.jsonl"))?;
    let mut pool = if scenario == Scenario::L0 {
        None
    } else {
        Some(normal_pool::Process::spawn(
            &std::env::current_exe().map_err(err)?,
            &dir.join("normal-pool"),
            Some(&dir.join("heaps.jsonl")),
        )?)
    };
    let seconds = plan.total().as_secs().to_string();
    let layout_dir = dir.join("layout");
    let mut layout = if scenario != Scenario::L0 {
        let mut job = spawn(
            &dir,
            "layout",
            &[
                "steady-layout-session".as_ref(),
                layout_dir.as_os_str(),
                seconds.as_ref(),
            ],
            true,
            true,
        )?;
        let began = Instant::now();
        loop {
            if joint_driver::done(&mut job)? {
                return Err("steady_layout_early_exit".into());
            }
            if let Ok(m) = load(&layout_dir.join("manifest.json")) {
                if m["external_owned_charge_bytes"] != normal_pool::BYTES
                    || m["normal_joint_budget_bytes"] != crate::layout_load::NORMAL_BYTES
                {
                    return Err("steady_layout_shape".into());
                }
                break;
            }
            if began.elapsed() > Duration::from_secs(120) {
                return Err("steady_layout_ready_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let sent = events.elapsed().as_nanos();
        job.0
            .stdin
            .as_mut()
            .ok_or("steady_layout_clock_pipe")?
            .write_all(b"C")
            .map_err(err)?;
        loop {
            if let Ok(m) = load(&layout_dir.join("clock-sync.json")) {
                let clock: crate::heap_meter::ClockReading =
                    serde_json::from_value(m["clock"].clone()).map_err(err)?;
                if clock.pid != job.0.id() {
                    return Err("steady_layout_clock_pid".into());
                }
                let observed = events.elapsed().as_nanos();
                save(
                    &dir.join("layout-clock-map.json"),
                    &json!({"pid":clock.pid,"instance_unix_ns":clock.instance_unix_ns,"layout_origin_parent_ns_min":sent as i128-clock.elapsed_ns as i128,"layout_origin_parent_ns_max":observed as i128-clock.elapsed_ns as i128,"uncertainty_ns":observed-sent,"method":"C token causal interval; batch inclusion must account for both this and driver origin uncertainty"}),
                )?;
                break;
            }
            if joint_driver::done(&mut job)? || began.elapsed() > Duration::from_secs(120) {
                return Err("steady_layout_clock_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        events.write("layout_ready_before_warmup", json!({"pid":job.0.id()}))?;
        Some(job)
    } else {
        None
    };
    let business = dir.join("business");
    let mut child = spawn(
        &dir,
        "business",
        &[
            "steady-driver".as_ref(),
            business.as_os_str(),
            scenario.name().as_ref(),
            plan.name().as_ref(),
        ],
        true,
        pool.is_some(),
    )?;
    let mut clock_seen = false;
    let mut sync_sent = None;
    loop {
        if sync_sent.is_none() && business.join("clock-sync-ready.json").exists() {
            let sent = events.elapsed().as_nanos();
            child
                .0
                .stdin
                .as_mut()
                .ok_or("steady_clock_pipe")?
                .write_all(b"C")
                .map_err(err)?;
            sync_sent = Some(sent);
        }
        if !clock_seen && business.join("clock-started.json").exists() {
            let m = match load(&business.join("clock-started.json")) {
                Ok(m) => m,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
            };
            let d = m["clock_sync_elapsed_ns"]
                .as_u64()
                .ok_or("steady_clock_sync_missing")? as i128;
            let observed = events.elapsed().as_nanos() as i128;
            let sent = sync_sent.ok_or("steady_clock_unsent")? as i128;
            save(
                &dir.join("clock-map.json"),
                &json!({"driver_origin_parent_ns_min":sent-d,"driver_origin_parent_ns_max":observed-d,"uncertainty_ns":observed-sent,"method":"C token send to child receive to marker observed; monotonic causal interval"}),
            )?;
            clock_seen = true;
            events.write("driver_clock_start_observed",json!({"pid":child.0.id(),"mapping":"publication/observation interval, not an identical cross-process Instant"}))?;
        }
        if joint_driver::done(&mut child)? {
            break;
        }
        if let Some(j) = layout.as_mut() {
            if joint_driver::done(j)? {
                return Err("steady_layout_exited_before_driver".into());
            }
        }
        if events.elapsed() > plan.total() + Duration::from_secs(1200) {
            return Err("steady_supervisor_timeout".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !clock_seen {
        return Err("steady_no_driver_clock".into());
    }
    let report = load(&business.join("report.json"))?;
    if report["source_sha256"] != env!("MYCITY_SOURCE_SHA256")
        || report["normal_pool_balanced"] != true
    {
        return Err("steady_driver_report".into());
    }
    events.write(
        "driver_exited_before_layout_stop",
        json!({"pid":child.0.id()}),
    )?;
    if let Some(j) = layout.as_mut() {
        j.0.stdin
            .as_mut()
            .ok_or("steady_layout_owner_pipe")?
            .write_all(b"S")
            .map_err(err)?;
        while !joint_driver::done(j)? {
            if events.elapsed() > plan.total() + Duration::from_secs(1500) {
                return Err("steady_layout_stop_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let l = load(&layout_dir.join("report.json"))?;
        if l["all_published_scans_completed"] != true {
            return Err("steady_layout_incomplete".into());
        }
        let snapshots = l["snapshots"]
            .as_array()
            .ok_or("steady_snapshots_missing")?;
        if snapshots
            .iter()
            .any(|x| x["record_restore_passed"] != true || x["pressure_readback_passed"] != true)
        {
            return Err("steady_snapshot_verification".into());
        }
        let generation = snapshots
            .last()
            .and_then(|x| x["generation"].as_u64())
            .ok_or("steady_latest_snapshot")?;
        let latest = layout_dir.join(format!("snapshots/snapshot-{generation:06}"));
        let restored = dir.join("layout-restored");
        events.write(
            "layout_cold_restore_requested",
            json!({"generation":generation}),
        )?;
        let mut restore = spawn(
            &dir,
            "restore",
            &[
                "layout-load-only".as_ref(),
                latest.as_os_str(),
                restored.as_os_str(),
            ],
            false,
            true,
        )?;
        let began = Instant::now();
        while !joint_driver::done(&mut restore)? {
            if began.elapsed() > Duration::from_secs(120) {
                return Err("steady_layout_restore_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if load(&restored.join("report.json"))?["cold_reopen_and_scan"] != "passed" {
            return Err("steady_layout_restore".into());
        }
        events.write(
            "layout_cold_restored",
            json!({"generation":generation,"seconds":began.elapsed().as_secs_f64()}),
        )?;
    }
    if let Some(p) = pool.as_mut() {
        let s = normal_pool::status(p.root())?;
        if s.available_touched_bytes != normal_pool::BYTES || s.granted_capacity_bytes != 0 {
            return Err("steady_pool_unbalanced".into());
        }
        p.finish()?;
    }
    events.write("all_children_waited", json!({"normal_pool_balanced":true}))?;
    save(
        &dir.join("report.json"),
        &json!({"schema":"mycity-steady-supervisor-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"scenario":scenario,"formal":plan.formal,
  "experiment_completed":true,"whole_seconds":events.elapsed().as_secs_f64(),"business_within_measurement_window":report["business_within_measurement_window"],
  "formal_steady":"requires independent window/latency/resource audit","three_rounds":"not_accepted_by_single_round","eight_hour":"not_run","real_16GB":"not_run","full_M0":"not_accepted"}),
    )
}
