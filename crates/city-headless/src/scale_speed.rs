//! Original full trajectory at fixed wall-clock releases. No reduced days/load.
//! A completed experiment may MISS its speed target; completion is not acceptance.
use crate::{
    joint_driver, normal_pool,
    scale_model_task::{self, Mode},
    scale_reference::{self, Step, Tier},
    scale_sim::{self, Experiment},
    sim_client::Client,
    sim_host::Command,
};
use mycity_core::read_bounded;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn save(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = File::create_new(path).map_err(err)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(err)?;
    file.write_all(b"\n").map_err(err)?;
    file.sync_all().map_err(err)
}
#[derive(Clone, Copy)]
pub struct Rate {
    day: Option<u32>,
}
impl Rate {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(Self {
            day: match s {
                "unpaced" => None,
                "600" => Some(600),
                "60" => Some(60),
                "10" => Some(10),
                "1" => Some(1),
                _ => return Err("speed_rate".into()),
            },
        })
    }
    fn name(self) -> &'static str {
        match self.day {
            None => "unpaced",
            Some(600) => "600",
            Some(60) => "60",
            Some(10) => "10",
            Some(1) => "1",
            _ => unreachable!(),
        }
    }
    // Divide after multiplying: all day boundaries are exact, no accumulated
    // truncation of 1/24 or 10/24 seconds across the 720-hour trajectory.
    fn boundary(self, hours: u64) -> Duration {
        Duration::from_nanos(u64::from(self.day.unwrap_or(0)) * 1_000_000_000 * hours / 24)
    }
    fn release(self, target: u64) -> Result<Duration, String> {
        if !(1..=2_592_000).contains(&target) {
            return Err("speed_target".into());
        }
        Ok(self.boundary(target.div_ceil(3600) - 1))
    }
    fn timeout(self) -> Duration {
        Duration::from_secs((u64::from(self.day.unwrap_or(0)) * 30).max(1800) + 1200)
    }
}
struct Clock {
    began: Instant,
    publication_cutoff: Option<Duration>,
    queries: u64,
    saves: u64,
    maximum_query_lateness: Duration,
    maximum_save_lateness: Duration,
}
impl Clock {
    fn elapsed(&self) -> Duration {
        self.began.elapsed()
    }
    fn next_query(&self) -> Duration {
        Duration::from_secs((self.queries + 1) * 10)
    }
    fn next_save(&self) -> Duration {
        Duration::from_secs((self.saves + 1) * 300)
    }
    /// Fixed cutoff per call. Final call freezes publication at the predefined
    /// final-checkpoint boundary; new future requests are never marked completed.
    fn service(&mut self, e: &mut Experiment, cutoff: Duration) -> Result<(), String> {
        let cutoff = cutoff.min(self.publication_cutoff.unwrap_or(cutoff));
        loop {
            let q = self.next_query();
            let s = self.next_save();
            let (is_save, due) = if q <= s { (false, q) } else { (true, s) };
            if due > cutoff {
                break;
            }
            let submitted = self.elapsed();
            e.call(if is_save {
                Command::Checkpoint
            } else {
                Command::Progress
            })?;
            let completed = self.elapsed();
            let late = completed.saturating_sub(due);
            let ordinal = if is_save {
                self.saves += 1;
                self.maximum_save_lateness = self.maximum_save_lateness.max(late);
                self.saves
            } else {
                self.queries += 1;
                self.maximum_query_lateness = self.maximum_query_lateness.max(late);
                self.queries
            };
            e.record(json!({"speed_periodic":if is_save {"save"}else{"query"},"ordinal":ordinal,
                "deadline_seconds":due.as_secs_f64(),"submitted_seconds":submitted.as_secs_f64(),
                "completed_seconds":completed.as_secs_f64(),"completion_lateness_seconds":late.as_secs_f64()}))?;
        }
        Ok(())
    }
    fn wait(&mut self, e: &mut Experiment, release: Duration) -> Result<Duration, String> {
        loop {
            self.service(e, self.elapsed())?;
            let now = self.elapsed();
            if now >= release {
                return Ok(now);
            }
            std::thread::sleep((release - now).min(Duration::from_millis(100)));
        }
    }
}
#[derive(Serialize)]
struct Day {
    day: u32,
    business_durable_seconds: f64,
    fixed_period_start_seconds: Option<f64>,
    fixed_period_to_business_seconds: Option<f64>,
    adjacent_business_completion_seconds: f64,
    budget_met: bool,
}
/// Supervisor owns the real normal pool and actual driver Child. No Python
/// functional launcher. Run under the unchanged external system observer.
pub fn run(dir: &Path, rate: Rate) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    let whole = Instant::now();
    save(
        &dir.join("manifest.json"),
        &json!({"schema":"mycity-speed-supervisor-v1",
        "source_sha256":env!("MYCITY_SOURCE_SHA256"),"rate":rate.name(),
        "external_owned_charge_bytes":crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,"normal_pool_bytes":normal_pool::BYTES,
        "scope":"full actual business, L1 absent; full model Rust arenas inside64MiB; native/stack separate",
        "process_exit_zero":"completed experiment, NOT speed acceptance"}),
    )?;
    let mut pool = normal_pool::Process::spawn(
        &std::env::current_exe().map_err(err)?,
        &dir.join("normal-pool"),
        Some(&dir.join("heaps.jsonl")),
    )?;
    let business = dir.join("business");
    let mut child = joint_driver::start(
        &dir,
        "business",
        &[
            "speed-driver".as_ref(),
            business.as_os_str(),
            rate.name().as_ref(),
        ],
    )?;
    while !joint_driver::done(&mut child)? {
        if whole.elapsed() > rate.timeout() {
            return Err("speed_supervisor_deadline".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    #[derive(Deserialize)]
    struct Completed {
        source_sha256: String,
        completed_hours: u64,
        verified_targets: u64,
        worker_windows: u64,
        entire_engine_including_metadata: String,
    }
    let b: Completed = serde_json::from_slice(
        &read_bounded(
            File::open(business.join("report.json")).map_err(err)?,
            65536,
        )
        .map_err(err)?,
    )
    .map_err(err)?;
    if b.source_sha256 != env!("MYCITY_SOURCE_SHA256")
        || b.completed_hours != 720
        || b.verified_targets != 723
        || b.worker_windows != 724
        || b.entire_engine_including_metadata != "equal_independent_normal_reference"
    {
        return Err("speed_child_incomplete".into());
    }
    let balance = normal_pool::status(pool.root())?;
    if balance.grants != 724
        || balance.returns != 724
        || balance.fill_acknowledgements != 724
        || balance.available_touched_bytes != normal_pool::BYTES
        || balance.granted_capacity_bytes != 0
    {
        return Err("speed_pool_balance".into());
    }
    pool.finish()?;
    let peak = crate::heap_meter::snapshot().lifetime_peak_requested_bytes;
    if peak + crate::heap_meter::TAIL_RESERVED_BYTES > crate::diagnostic_contract::SUPERVISOR {
        return Err("speed_supervisor_heap_cap".into());
    }
    save(
        &dir.join("report.json"),
        &json!({"schema":"mycity-speed-supervisor-v1",
        "source_sha256":env!("MYCITY_SOURCE_SHA256"),"rate":rate.name(),"experiment_completed":true,
        "whole_wall_seconds":whole.elapsed().as_secs_f64(),"supervisor_heap_peak_requested_bytes":peak,
        "speed_acceptance":"see business/report.json; completion is not target acceptance",
        "shared_8MiB":"not_accepted","normal_3_10GiB":"not_accepted","L1":"absent",
        "eight_hour":"not_run","real_16GB":"not_run","full_M0":"not_accepted"}),
    )
}
pub fn driver(dir: &Path, rate: Rate) -> Result<(), String> {
    driver_inner(dir, rate, None)
}
/// Reference is prepared before warmup; no measured city exists before anchor.
pub(crate) fn steady_driver(dir: &Path, anchor: Instant, span: Duration) -> Result<(), String> {
    scale_model_task::validate_reference(&dir.join("reference"), Tier::Full)?;
    driver_inner(
        dir,
        Rate::parse(if span == Duration::from_secs(1800) {
            "60"
        } else {
            "unpaced"
        })?,
        Some((anchor, span)),
    )
}
fn driver_inner(dir: &Path, rate: Rate, steady: Option<(Instant, Duration)>) -> Result<(), String> {
    if steady.is_none() {
        fs::create_dir(dir).map_err(err)?;
    } else if dir.join("world").exists() {
        return Err("steady_measured_city_preexists".into());
    }
    let dir = dir.canonicalize().map_err(err)?;
    save(
        &dir.join("speed-manifest.json"),
        &json!({"schema":"mycity-speed-plan-v1",
        "source_sha256":env!("MYCITY_SOURCE_SHA256"),"rate":rate.name(),"game_days":30,"hours":720,"targets":723,
        "hour_release":"(ceil(target/3600)-1)*day_wall/24; actions after hour h wait for boundary h",
        "hour_deadline":"h*day_wall/24; no extra wait at final end",
        "operating_start":"immediately before actual Sim create/open, after independent CPU reference",
        "operating_end":"after final business checkpoint and draining observations published at or before that checkpoint completion",
        "observations":"query every10s/save every300s from operating t0; publication stops at final business checkpoint completion; later deadlines cancelled/unpublished",
        "proof_work":"reference and cold restoration/comparison independently reported; never deducted from observed resources",
        "speed_miss":"continue full trajectory and report miss; no tolerance, shorter sample or dropped fact"}),
    )?;
    let whole = Instant::now();
    if steady.is_none() {
        scale_model_task::run(&dir, Tier::Full, Mode::Reference)?;
    }
    let reference_seconds = whole.elapsed().as_secs_f64();
    // Must precede the actual Store creation request, not just model initialization.
    let mut clock = Clock {
        began: steady.map_or_else(Instant::now, |(anchor, _)| anchor),
        publication_cutoff: steady.map(|(_, span)| span),
        queries: 0,
        saves: 0,
        maximum_query_lateness: Duration::ZERO,
        maximum_save_lateness: Duration::ZERO,
    };
    let mut e = scale_sim::independent_experiment(&dir, true, true)?;
    e.record(json!({"experiment_origin_measurement_ns":e.clock_origin().duration_since(clock.began).as_nanos(),"mapping":"same process Instant offset; origin before actual Sim creation"}))?;
    let sim_ready_seconds = clock.elapsed().as_secs_f64();
    e.record(json!({"speed_operating_started_before_sim_create":true,"sim_ready_seconds":sim_ready_seconds}))?;
    let mut lines = BufReader::new(File::open(dir.join("cpu-targets.jsonl")).map_err(err)?);
    let mut hours = 0u64;
    let mut targets = 0u64;
    let mut days = Vec::with_capacity(30);
    let mut previous_day = Duration::ZERO;
    let mut last_day_start = Duration::ZERO;
    scale_reference::walk_tier(Tier::Full, |step| {
        if clock.elapsed() > rate.timeout() {
            return Err("speed_operating_deadline".into());
        }
        match step {
            Step::Action(action) => {
                clock.wait(&mut e, rate.boundary(hours))?;
                e.raw(vec![action])?;
            }
            Step::Initialized => {
                e.record(json!({"speed_initialized_seconds":clock.elapsed().as_secs_f64()}))?;
            }
            Step::Drive(target) => {
                let release = rate.release(target)?;
                let submitted = clock.wait(&mut e, release)?;
                let line = scale_sim::target_line(&mut lines)?.ok_or("speed_missing_target")?;
                let expected: Value = serde_json::from_slice(&line).map_err(err)?;
                e.record(json!({"speed_target_submitted":target,"release_seconds":release.as_secs_f64(),"submitted_seconds":submitted.as_secs_f64(),
                    "nominal_release_age_seconds":submitted.saturating_sub(release).as_secs_f64()}))?;
                e.drive(target, &expected)?;
                targets += 1;
                e.record(json!({"speed_target_completed":target,"completed_seconds":clock.elapsed().as_secs_f64()}))?;
            }
            Step::HourComplete => {
                hours += 1;
                if hours % 24 == 0 {
                    let complete = clock.elapsed();
                    let day = hours / 24;
                    let fixed = rate.day.map(|_| rate.boundary((day - 1) * 24));
                    if fixed.is_some_and(|s| complete < s) {
                        return Err("speed_early_day".into());
                    }
                    let period = fixed.map(|s| complete - s);
                    let adjacent = complete - previous_day;
                    last_day_start = fixed.unwrap_or(previous_day);
                    let budget = Duration::from_secs(u64::from(rate.day.unwrap_or(60)));
                    days.push(Day {
                        day: day as u32,
                        business_durable_seconds: complete.as_secs_f64(),
                        fixed_period_start_seconds: fixed.map(|s| s.as_secs_f64()),
                        fixed_period_to_business_seconds: period.map(|s| s.as_secs_f64()),
                        adjacent_business_completion_seconds: adjacent.as_secs_f64(),
                        budget_met: period.unwrap_or(adjacent) <= budget,
                    });
                    if steady.is_some() {
                        let boundary = e.call(Command::Status)?;
                        e.record(json!({"steady_quiescent_day_boundary":day,"same_phase":"hour complete; no active driver request",
                            "game_target":day*86400,"status":boundary,"driver_heap":crate::heap_meter::snapshot(),
                            "observed_seconds":clock.elapsed().as_secs_f64()}))?;
                        // The large status Value has been serialized and dropped.
                        // This small marker opens a physical idle-sampling gap;
                        // the next actual RPC request closes it without any wait.
                        e.record(json!({"steady_quiescent_day_released":day,
                            "game_target":day*86400,"phase":"status_report_dropped_before_next_request"}))?;
                    }
                    previous_day = complete;
                }
            }
            Step::PeakShipments => (),
        }
        Ok(())
    })?;
    let (windows, verified, failed) = e.counts();
    if hours != 720
        || targets != 723
        || windows != 724
        || verified != 723
        || failed != 0
        || days.len() != 30
        || scale_sim::target_line(&mut lines)?.is_some()
    {
        return Err("speed_incomplete".into());
    }
    clock.service(&mut e, clock.elapsed())?;
    let business_complete = clock.elapsed();
    let final_checkpoint_request = e.id.checked_add(1).ok_or("speed_request_id")?;
    let checkpoint_submitted = clock.elapsed();
    e.call(Command::Checkpoint)?;
    let publication_end = clock.elapsed();
    e.record(json!({"speed_final_business_checkpoint":final_checkpoint_request,
        "submitted_seconds":checkpoint_submitted.as_secs_f64(),"completed_seconds":publication_end.as_secs_f64(),
        "publication_frozen":true}))?;
    clock.service(&mut e, publication_end)?;
    let operating_end = clock.elapsed();
    let post_operating = Instant::now();
    let budget = Duration::from_secs(u64::from(rate.day.unwrap_or(60)));
    let final_day_with_checkpoint = operating_end - last_day_start;
    let all_days = days.iter().all(|d| d.budget_met) && final_day_with_checkpoint <= budget;
    let total_met = operating_end <= budget * 30;
    let met = all_days && total_met;
    e.record(json!({"speed_operating_end_seconds":operating_end.as_secs_f64(),"final_checkpoint_completed_seconds":publication_end.as_secs_f64(),
        "publication_end_seconds":publication_end.as_secs_f64(),"all_published_observations_completed":true}))?;
    let measured_business_complete = publication_end;
    let mut steady_closed = None;
    let steady_boundary_before_cold = if let Some((_, span)) = steady {
        while clock.elapsed() < span {
            clock.service(&mut e, clock.elapsed().min(span))?;
            std::thread::sleep(
                span.saturating_sub(clock.elapsed())
                    .min(Duration::from_millis(20)),
            );
        }
        // Exactly the fixed publication cutoff, even if drain is late.
        clock.service(&mut e, span)?;
        let completed = clock.elapsed();
        let boundary = e.call(Command::Status)?;
        steady_closed = Some(completed);
        Some(boundary)
    } else {
        None
    };
    // Shutdown and cold proof remain observed, outside the fixed measurement window.
    e.call(Command::Stop)?;
    e.client.stop()?;
    scale_model_task::run(&dir, Tier::Full, Mode::BeforeReopen)?;
    let cold = Instant::now();
    e.client = Client::spawn_scale_measured(
        &std::env::current_exe().map_err(err)?,
        &dir.join("world"),
        false,
        false,
        Tier::Full,
    )?;
    if e.client.next(Duration::from_secs(35))?.kind != "ready" {
        return Err("speed_cold_ready".into());
    }
    let cold_ready_seconds = cold.elapsed().as_secs_f64();
    e.call(Command::Checkpoint)?;
    e.call(Command::Stop)?;
    e.client.stop()?;
    let compared = scale_model_task::run(&dir, Tier::Full, Mode::CompareSpeed)?;
    let mut report = json!({"schema":"mycity-speed-driver-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"rate":rate.name(),
        "completed_hours":hours,"verified_targets":targets,"worker_windows":windows,
        "entire_engine_including_metadata":"equal_independent_normal_reference",
        "reference_wall_seconds":reference_seconds,"sim_ready_seconds":sim_ready_seconds,"operating_wall_seconds":operating_end.as_secs_f64(),
        "business_targets_complete_seconds":business_complete.as_secs_f64(),"final_checkpoint_request_id":final_checkpoint_request,
        "final_checkpoint_submitted_seconds":checkpoint_submitted.as_secs_f64(),"final_checkpoint_completed_seconds":publication_end.as_secs_f64(),
        "publication_end_seconds":publication_end.as_secs_f64(),"periodic_queries_completed":clock.queries,"periodic_saves_completed":clock.saves,
        "next_unpublished_query_deadline_seconds":clock.next_query().as_secs_f64(),"next_unpublished_save_deadline_seconds":clock.next_save().as_secs_f64(),
        "maximum_query_completion_lateness_seconds":clock.maximum_query_lateness.as_secs_f64(),"maximum_save_completion_lateness_seconds":clock.maximum_save_lateness.as_secs_f64(),
        "days":days,"final_day_with_checkpoint_seconds":final_day_with_checkpoint.as_secs_f64(),
        "target_day_wall_seconds":rate.day,"evaluation_day_budget_seconds":budget.as_secs_f64(),"all_daily_budgets_met":all_days,
        "whole_30day_budget_met":total_met,"speed_target_met":met,
        "full_profile_60_actual_operating_target":if rate.day==Some(60) {if met {"passed_this_run"}else{"failed_this_run"}}else{"not_evaluated_this_rate"},
        "full_profile_unpaced_target":if rate.day.is_none() {if met {"passed_this_run"}else{"failed_this_run"}}else{"not_evaluated_this_rate"},
        "cold_ready_seconds":cold_ready_seconds,"cold_proof_and_shutdown_seconds":post_operating.elapsed().as_secs_f64(),
        "whole_driver_wall_seconds":whole.elapsed().as_secs_f64(),"image_sha256":compared.image_sha256,
        "L1":"absent","common_8MiB":"not_accepted","exact_normal_owned":"not_accepted","eight_hour":"not_run","real_16GB":"not_run","full_M0":"not_accepted"});
    if let Some((_, span)) = steady {
        report["steady_window"] = json!({"measurement_seconds":span.as_secs_f64(),
            "last_business_seconds":business_complete.as_secs_f64(),"last_business_checkpoint_seconds":measured_business_complete.as_secs_f64(),
            "business_and_checkpoint_within_window":measured_business_complete<=span,
            "fixed_window_end_seconds":span.as_secs_f64(),"scheduled_observations_drained_seconds":steady_closed.map(|d|d.as_secs_f64()),
            "quiescent_boundary_before_cold":steady_boundary_before_cold,"cold_proof_drain_end_seconds":clock.elapsed().as_secs_f64(),
            "periodic_queries_completed":clock.queries,"periodic_saves_completed":clock.saves,
            "maximum_save_lateness_seconds":clock.maximum_save_lateness.as_secs_f64(),
            "formal_steady_acceptance":"requires actual whole-window and independent resource/latency audit"});
    }
    report["profile"] = json!("L2_full_actual");
    report["mandatory_pe0_gate"] = json!("not_evaluated_other_profile");
    report["legacy_session_speed"] = json!("not_retested");
    save(&dir.join("report.json"), &report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_period_starts_early_but_future_periods_are_not_released_early() {
        for name in ["600", "60", "10", "1"] {
            let rate = Rate::parse(name).unwrap();
            let day = u64::from(rate.day.unwrap());
            assert_eq!(rate.release(3600).unwrap(), Duration::ZERO);
            assert_eq!(rate.release(86400 + 1).unwrap(), Duration::from_secs(day));
            assert_eq!(
                rate.release(1296000 + 120).unwrap(),
                Duration::from_secs(day * 15)
            );
            assert_eq!(rate.boundary(720), Duration::from_secs(day * 30));
            assert!(rate.release(2592000).unwrap() < rate.boundary(720));
        }
        assert!(Rate::parse("0").is_err());
        assert!(Rate::parse("2").is_err());
        assert!(Rate::parse("60").unwrap().release(0).is_err());
        assert!(Rate::parse("60").unwrap().release(2592001).is_err());
    }
}
