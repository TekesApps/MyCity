//! The driver owns the only warmup/measurement Instant; sub-processes never shift it.
use super::*;
use crate::{
    joint_driver::Job,
    normal_pool,
    scale_model_task::{self, Mode},
    scale_reference::{self, Step, Tier},
    scale_sim, scale_speed,
    sim_host::Command,
};
use std::process::{Command as ProcessCommand, Stdio};

fn balanced() -> Result<(), String> {
    if let Some(root) = std::env::var_os("MYCITY_NORMAL_POOL") {
        let s = normal_pool::status(Path::new(&root))?;
        if s.available_touched_bytes != normal_pool::BYTES
            || s.granted_capacity_bytes != 0
            || s.workspace_grants != s.workspace_returns
            || s.grants != s.returns
        {
            return Err("steady_unreleased_role_at_handoff".into());
        }
    }
    Ok(())
}
fn prefix(
    dir: &Path,
    reference: &Path,
    events: &mut Events,
    cutoff: Duration,
) -> Result<(), String> {
    crate::sim_client::with_client_scope(|client| prefix_inner(dir, reference, events, cutoff, client))
}
fn prefix_inner(
    dir: &Path,
    reference: &Path,
    events: &mut Events,
    cutoff: Duration,
    client: &mut crate::sim_client::Client,
) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let mut e = scale_sim::independent_experiment(client, dir, true, true)?;
    events.write("warmup_experiment_clock", json!({"path":dir,"experiment_origin_driver_ns":e.clock_origin().duration_since(events.began).as_nanos()}))?;
    let mut lines =
        std::io::BufReader::new(File::open(reference.join("cpu-targets.jsonl")).map_err(err)?);
    let mut hours = 0;
    let mut targets = 0;
    let mut last = 0;
    let outcome = scale_reference::walk_tier(Tier::Full, |step| {
        match step {
            Step::Action(a) => {
                e.raw(vec![a])?;
            }
            Step::Drive(t) => {
                let line = scale_sim::target_line(&mut lines)?.ok_or("steady_prefix_oracle")?;
                let expected = serde_json::from_slice(&line).map_err(err)?;
                e.drive(t, &expected)?;
                targets += 1;
                last = t;
            }
            Step::HourComplete => {
                hours += 1;
                if events.elapsed() >= cutoff {
                    return Err("steady_warmup_prefix_boundary".into());
                }
            }
            _ => (),
        }
        Ok(())
    });
    if let Err(e) = outcome {
        if e != "steady_warmup_prefix_boundary" {
            return Err(e);
        }
    }
    let state = e.call(Command::Status)?;
    e.save_stop()?;
    balanced()?;
    save(
        &dir.join("warmup-report.json"),
        &json!({"hours":hours,"targets":targets,"last_target":last,"cash_and_receipts":state,
  "phase":"isolated_warmup_not_measured_business","original_full_capacity":true,"ended_at_safe_boundary":true,
  "stop_reason":if hours==720 {"complete_30d_branch"}else{"290s_admission_cutoff_at_hour_boundary"}}),
    )?;
    events.write(
        "warmup_branch_released",
        json!({"path":dir,"hours":hours,"targets":targets,"last_target":last,"pool_balanced":true}),
    )
}
fn case(
    dir: &Path,
    kind: &str,
    deadline: Duration,
    allow_prefix: bool,
    events: &mut Events,
) -> Result<bool, String> {
    fs::create_dir(dir).map_err(err)?;
    let mut command = ProcessCommand::new(std::env::current_exe().map_err(err)?);
    let mut job = Job(command
        .args(["steady-l0-case", kind])
        .arg(dir)
        .env_remove("MYCITY_NORMAL_POOL")
        .env_remove("MYCITY_BROKER_SESSION")
        .env_remove("MYCITY_BROKER_SUPERVISOR")
        .env("MYCITY_DIAGNOSTICS", "1")
        .stdin(Stdio::piped())
        .stdout(File::create_new(dir.join("stdout")).map_err(err)?)
        .stderr(File::create_new(dir.join("stderr")).map_err(err)?)
        .spawn()
        .map_err(err)?);
    let token_sent_ns = events.elapsed().as_nanos();
    job.0
        .stdin
        .as_mut()
        .ok_or("steady_case_pipe")?
        .write_all(b"1")
        .map_err(err)?;
    events.write(
        "case_spawned",
        json!({"case":kind,"path":dir,"pid":job.0.id()}),
    )?;
    let mut stopped = false;
    let mut clock_seen = false;
    loop {
        if !clock_seen {
            if let Ok(marker) = load(&dir.join("case-child-ready.json")) {
                let clock: crate::heap_meter::ClockReading =
                    serde_json::from_value(marker["clock"].clone()).map_err(err)?;
                let observed = events.elapsed().as_nanos();
                events.write("case_clock_map", json!({"case":kind,"path":dir,"pid":clock.pid,"instance_unix_ns":clock.instance_unix_ns,"child_origin_driver_ns_min":token_sent_ns as i128-clock.elapsed_ns as i128,"child_origin_driver_ns_max":observed as i128-clock.elapsed_ns as i128,"uncertainty_ns":observed-token_sent_ns}))?;
                clock_seen = true;
            }
        }
        if let Some(s) = job.0.try_wait().map_err(err)? {
            if !s.success() {
                return Err(format!("steady_case_failed:{}:{s}", job.0.id()));
            }
            break;
        }
        if allow_prefix && !stopped && events.elapsed() >= deadline {
            job.0
                .stdin
                .as_mut()
                .ok_or("steady_case_pipe")?
                .write_all(b"S")
                .map_err(err)?;
            stopped = true;
            events.write(
                "case_stop_admission",
                json!({"pid":job.0.id(),"deadline_ns":deadline.as_nanos()}),
            )?;
        }
        if events.elapsed() > deadline + Duration::from_secs(if allow_prefix { 10 } else { 180 }) {
            return Err("steady_case_failed_to_quiesce".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !clock_seen {
        return Err("steady_case_clock_missing".into());
    }
    let r = load(&dir.join("case-summary.json"))?;
    if r["completed_case"] != true && (!allow_prefix || r["safe_prefix"] != true) {
        return Err("steady_case_result".into());
    }
    events.write("case_released",json!({"case":kind,"path":dir,"pid":job.0.id(),"completed_case":r["completed_case"],"safe_prefix":r["safe_prefix"],"allow_prefix":allow_prefix,"within_admission_window":events.elapsed()<=deadline,"owned_heap":crate::heap_meter::snapshot()}))?;
    Ok(r["completed_case"] == true)
}
#[derive(Default, Serialize)]
struct Counts {
    complete_inside_window: [u64; 3],
    complete_in_drain: [u64; 3],
    safe_warmup_prefixes: u64,
}
fn pack(
    root: &Path,
    phase: &str,
    deadline: Duration,
    allow_prefix: bool,
    events: &mut Events,
) -> Result<Counts, String> {
    let mut counts = Counts::default();
    let mut i = 0usize;
    while events.elapsed() < deadline {
        let slot = i % 3;
        let kind = ["economy", "boot", "exchange"][slot];
        if case(
            &root.join(format!("{phase}-{i:05}-{kind}")),
            kind,
            deadline,
            allow_prefix,
            events,
        )? {
            if events.elapsed() <= deadline {
                counts.complete_inside_window[slot] += 1;
            } else {
                counts.complete_in_drain[slot] += 1;
            }
        } else {
            counts.safe_warmup_prefixes += 1;
        }
        i += 1;
    }
    Ok(counts)
}
pub fn run(dir: &Path, scenario: Scenario, plan: Plan) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let measurement = dir.join("measurement");
    if scenario == Scenario::L2 {
        fs::create_dir(&measurement).map_err(err)?;
        // Pure independent reference only. No measured city/startup action pre-created.
        scale_model_task::run(&measurement, Tier::Full, Mode::Reference)?;
    }
    let mut events = Events::new(&dir.join("window-events.jsonl"))?;
    let mut clock_sync_ns = None;
    if std::env::var_os("MYCITY_STEADY_PARENT_PIPE").is_some() {
        save(&dir.join("clock-sync-ready.json"), &json!({"ready":true}))?;
        let mut token = [0];
        std::io::stdin().read_exact(&mut token).map_err(err)?;
        if token != [b'C'] {
            return Err("steady_clock_sync_token".into());
        }
        clock_sync_ns = Some(events.elapsed().as_nanos());
    }
    save(
        &dir.join("clock-started.json"),
        &json!({"source_sha256":env!("MYCITY_SOURCE_SHA256"),"scenario":scenario,"warmup_ns":plan.warmup.as_nanos(),"measurement_ns":plan.measure.as_nanos(),"clock_sync_elapsed_ns":clock_sync_ns,"parent_mapping":"causal send/receive bracket; not identical cross-process Instant"}),
    )?;
    events.write(
        "warmup_started",
        json!({"fixed_end_ns":plan.warmup.as_nanos()}),
    )?;
    let cutoff = plan.warmup - plan.quiesce;
    let mut warmup_branches = 0;
    match scenario {
        Scenario::L0 => {
            pack(dir, "warmup", cutoff, true, &mut events)?;
        }
        Scenario::L1 => events.wait(cutoff),
        Scenario::L2 => {
            while plan.admission(events.elapsed()) {
                prefix(
                    &dir.join(format!("warmup-{warmup_branches:04}")),
                    &measurement,
                    &mut events,
                    cutoff,
                )?;
                warmup_branches += 1;
            }
        }
    }
    balanced()?;
    if events.elapsed() > plan.warmup {
        events.write(
            "warmup_overrun_failed",
            json!({"fixed_end_ns":plan.warmup.as_nanos()}),
        )?;
        return Err("steady_warmup_overrun".into());
    }
    events.write("warmup_quiescent", json!({"pool_balanced":true}))?;
    events.wait(plan.warmup);
    let anchor = events.began + plan.warmup;
    events.write("measurement_started",json!({"fixed_start_ns":plan.warmup.as_nanos(),"fixed_end_ns":plan.total().as_nanos(),"prototype_initialization":"inside_measurement"}))?;
    let counts = match scenario {
        Scenario::L0 => pack(dir, "measurement", plan.total(), false, &mut events)?,
        Scenario::L1 => {
            events.wait(plan.total());
            Counts::default()
        }
        Scenario::L2 => {
            scale_speed::steady_driver(&measurement, anchor, plan.measure)?;
            Counts::default()
        }
    };
    let drain_end = events.elapsed();
    balanced()?;
    let business_met = match scenario {
        Scenario::L2 => Some(
            load(&measurement.join("report.json"))?["steady_window"]
                ["business_and_checkpoint_within_window"]
                == true,
        ),
        Scenario::L0 => Some(counts.complete_in_drain.iter().sum::<u64>() == 0),
        Scenario::L1 => None,
    };
    events.write("measurement_window_and_drain_completed",json!({"fixed_window_end_ns":plan.total().as_nanos(),"actual_drain_end_ns":drain_end.as_nanos(),"business_within_window":business_met,"whole_heap":crate::heap_meter::snapshot()}))?;
    events.file.sync_all().map_err(err)?;
    save(
        &dir.join("report.json"),
        &json!({"schema":"mycity-steady-driver-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"scenario":scenario,"formal":plan.formal,
  "warmup_seconds":plan.warmup.as_secs_f64(),"measurement_seconds":plan.measure.as_secs_f64(),"fixed_window_end_seconds":plan.total().as_secs_f64(),"actual_drain_end_seconds":drain_end.as_secs_f64(),
  "business_within_measurement_window":business_met,"completed_l0_cases":counts,"warmup_branches":warmup_branches,"normal_pool_balanced":true,
  "formal_steady":"requires_independent_window_latency_resource_audit","three_rounds":"not_accepted_by_single_round","eight_hour":"not_run","real_16GB":"not_run","full_M0":"not_accepted"}),
    )
}
