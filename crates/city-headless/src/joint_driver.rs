//! Fixed concurrent diagnostic. It cannot sign full pressure/owned/steady gates.
use mycity_core::{read_bounded, sha256};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn read<T: DeserializeOwned>(p: &Path) -> Result<T, String> {
    let b = read_bounded(File::open(p).map_err(err)?, 64 * 1024).map_err(err)?;
    serde_json::from_slice(&b).map_err(err)
}
fn save(p: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut f = File::create_new(p).map_err(err)?;
    serde_json::to_writer_pretty(&mut f, value).map_err(err)?;
    f.write_all(b"\n").map_err(err)?;
    f.sync_all().map_err(err)
}
pub(crate) struct Job(pub(crate) Child);
impl Drop for Job {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
pub(crate) fn start(root: &Path, name: &str, args: &[&std::ffi::OsStr]) -> Result<Job, String> {
    start_owned(root, name, args, false)
}
fn start_owned(
    root: &Path,
    name: &str,
    args: &[&std::ffi::OsStr],
    owner_pipe: bool,
) -> Result<Job, String> {
    Ok(Job(Command::new(std::env::current_exe().map_err(err)?)
        .args(args)
        .stdin(if owner_pipe {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .env("MYCITY_HEAP_LOG", root.join("heaps.jsonl"))
        .env("MYCITY_NORMAL_POOL", root.join("normal-pool"))
        .stdout(Stdio::from(
            File::create_new(root.join(format!("{name}.stdout"))).map_err(err)?,
        ))
        .stderr(Stdio::from(
            File::create_new(root.join(format!("{name}.stderr"))).map_err(err)?,
        ))
        .spawn()
        .map_err(err)?))
}
pub(crate) fn done(job: &mut Job) -> Result<bool, String> {
    match job.0.try_wait().map_err(err)? {
        None => Ok(false),
        Some(s) if s.success() => Ok(true),
        Some(s) => Err(format!("joint_child_failed:{}:{s}", job.0.id())),
    }
}
#[derive(Serialize)]
struct Event<'a> {
    kind: &'a str,
    observed_ns: u64,
    pid: Option<u32>,
    sequence: Option<u64>,
}
struct Clock {
    begin: Instant,
    events: BufWriter<File>,
}
impl Clock {
    fn now(&self) -> u64 {
        self.begin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
    fn event(&mut self, kind: &str, pid: Option<u32>, sequence: Option<u64>) -> Result<(), String> {
        let observed_ns = self.now();
        serde_json::to_writer(
            &mut self.events,
            &Event {
                kind,
                observed_ns,
                pid,
                sequence,
            },
        )
        .map_err(err)?;
        self.events.write_all(b"\n").map_err(err)?;
        self.events.flush().map_err(err)
    }
}
#[derive(Deserialize)]
struct Ready {
    external_owned_charge_bytes: u64,
    normal_joint_budget_bytes: u64,
}
#[derive(Deserialize)]
struct Business {
    completed_hour_targets: u64,
    verified_drive_targets: u64,
    actual_worker_windows: u64,
    complete_final_business_and_air: String,
    cold_complete_image_unchanged: bool,
}
#[derive(Deserialize)]
struct Layout {
    scans: u64,
    all_published_scans_completed: bool,
    snapshots: Vec<Summary>,
    diagnostics: LayoutDiagnostics,
}
#[derive(Deserialize)]
struct Summary {
    generation: u64,
    record_restore_passed: bool,
    pressure_readback_passed: bool,
}
#[derive(Deserialize)]
struct LayoutDiagnostics {
    charged_bound_bytes: usize,
}
#[derive(Deserialize)]
struct Restore {
    cold_reopen_and_scan: String,
}
#[derive(Deserialize)]
struct Current {
    slot: u8,
    root_sha256: String,
}
#[derive(Deserialize)]
struct Root {
    sequence: u64,
}
#[derive(Serialize)]
struct Mark {
    observed_ns: u64,
    sequence: u64,
    current_bytes: String,
    root_bytes: String,
}
fn mark(root: &Path, pending: &Path, clock: &Clock) -> Result<Option<Mark>, String> {
    if !pending.exists() {
        return Ok(None);
    }
    let current =
        read_bounded(File::open(root.join("CURRENT.json")).map_err(err)?, 4096).map_err(err)?;
    let c: Current = serde_json::from_slice(&current).map_err(err)?;
    if c.slot > 1 {
        return Err("joint_root_slot".into());
    }
    let bytes = read_bounded(
        File::open(root.join(format!("root-{}.json", c.slot))).map_err(err)?,
        32 * 1024,
    )
    .map_err(err)?;
    if sha256(&bytes) != c.root_sha256 || !pending.exists() {
        return Ok(None);
    }
    let sequence = serde_json::from_slice::<Root>(&bytes)
        .map_err(err)?
        .sequence;
    Ok(Some(Mark {
        observed_ns: clock.now(),
        sequence,
        current_bytes: String::from_utf8(current).map_err(err)?,
        root_bytes: String::from_utf8(bytes).map_err(err)?,
    }))
}
#[derive(Serialize)]
struct Witness {
    first: Mark,
    last: Mark,
    semantics: &'static str,
}
#[derive(Serialize)]
struct Manifest {
    external_owned_charge_bytes: usize,
    component_caps_bytes: [usize; 7],
    scope: &'static str,
}
#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: &'static str,
    diagnostic_component_caps_bytes: [usize; 8],
    l1_charged_bound_bytes: usize,
    supervisor_requested_heap_peak_bytes: usize,
    whole_ns: u64,
    overlap_witness_in_observed_tree: bool,
    scope: &'static str,
    joint_owned_budget: &'static str,
    steady_gate: &'static str,
    eight_hour_gate: &'static str,
    memory_16gb_gate: &'static str,
}
pub fn run(dir: &Path) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    let mib = 1024 * 1024;
    save(&dir.join("manifest.json"),&Manifest{external_owned_charge_bytes:crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,component_caps_bytes:[crate::diagnostic_contract::SIM,mib,crate::diagnostic_contract::LAYOUT,mib/4,mib,mib/4,crate::diagnostic_contract::POOL],scope:"external worker1 + L1 diagnostics0.75 + supervisor0.25 + driver1 + pool0.25 MiB; observer separate2; Sim and helper full Rust heaps including diagnostics inside64MiB; no duplicate external credit"})?;
    let mut clock = Clock {
        begin: Instant::now(),
        events: BufWriter::with_capacity(
            65536,
            File::create_new(dir.join("common-events.jsonl")).map_err(err)?,
        ),
    };
    clock.event("supervisor_started", Some(std::process::id()), None)?;
    let mut normal_pool = crate::normal_pool::Process::spawn(
        &std::env::current_exe().map_err(err)?,
        &dir.join("normal-pool"),
        Some(&dir.join("heaps.jsonl")),
    )?;
    clock.event("normal_pool_ready_observed", None, None)?;
    let layout_dir = dir.join("layout");
    let mut layout = start(
        &dir,
        "layout",
        &[
            "layout-charged-probe".as_ref(),
            layout_dir.as_os_str(),
            "600".as_ref(),
            "300".as_ref(),
            "67108864".as_ref(),
        ],
    )?;
    let ready_start = Instant::now();
    loop {
        if done(&mut layout)? {
            return Err("joint_layout_stopped_before_ready".into());
        }
        if layout_dir.join("manifest.json").exists() {
            // Producer publishes a small file with fs::write; retry partial EOF.
            if let Ok(r) = read::<Ready>(&layout_dir.join("manifest.json")) {
                if r.external_owned_charge_bytes != 64 * mib as u64
                    || r.normal_joint_budget_bytes != crate::layout_load::NORMAL_BYTES as u64
                {
                    return Err("joint_ready_shape".into());
                }
                break;
            }
        }
        if ready_start.elapsed() > Duration::from_secs(60) {
            return Err("joint_layout_ready_deadline".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    clock.event("layout_ready_observed", Some(layout.0.id()), None)?;
    let launch_at = Instant::now() + Duration::from_secs(120);
    while Instant::now() < launch_at {
        if done(&mut layout)? {
            return Err("joint_layout_stopped_before_launch".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let business_dir = dir.join("business");
    clock.event("business_launch_requested", None, None)?;
    let mut business = start(
        &dir,
        "business",
        &[
            "scale-sim-probe".as_ref(),
            business_dir.as_os_str(),
            "full".as_ref(),
            "measure".as_ref(),
        ],
    )?;
    clock.event("business_spawned", Some(business.0.id()), None)?;
    let pending = layout_dir.join("snapshots/snapshot-000002.pending");
    let mut first = None;
    let mut witnessed = false;
    let mut business_done = false;
    let mut layout_done = false;
    let mut snapshot_seen = false;
    let mut snapshot_done = false;
    while !business_done || !layout_done {
        if !business_done && done(&mut business)? {
            business_done = true;
            clock.event("business_exited", Some(business.0.id()), None)?;
        }
        if !layout_done && done(&mut layout)? {
            layout_done = true;
            clock.event("layout_exited", Some(layout.0.id()), None)?;
        }
        if pending.exists() {
            if !snapshot_seen {
                snapshot_seen = true;
                clock.event("snapshot2_pending_observed", None, None)?;
            }
            if !witnessed && business_dir.join("world/CURRENT.json").exists() {
                if let Some(m) = mark(&business_dir.join("world"), &pending, &clock)? {
                    match first.as_ref() {
                        None => {
                            clock.event(
                                "snapshot2_first_durable_sequence",
                                None,
                                Some(m.sequence),
                            )?;
                            first = Some(m);
                        }
                        Some(f) if m.sequence > f.sequence => {
                            clock.event(
                                "snapshot2_durable_sequence_advanced",
                                None,
                                Some(m.sequence),
                            )?;
                            save(&dir.join("overlap-witness.json"),&Witness{first:first.take().ok_or("joint_witness_missing")?,last:m,semantics:"same supervisor monotonic clock; pending path checked before and after CURRENT/root read; in observed process tree"})?;
                            witnessed = true;
                        }
                        _ => (),
                    }
                }
            }
        } else if snapshot_seen && !snapshot_done {
            snapshot_done = true;
            clock.event("snapshot2_pending_disappeared_observed", None, None)?;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let b: Business = read(&business_dir.join("report.json"))?;
    if b.completed_hour_targets != 720
        || b.verified_drive_targets != 723
        || b.actual_worker_windows != 724
        || b.complete_final_business_and_air != "passed"
        || !b.cold_complete_image_unchanged
    {
        return Err("joint_business_result".into());
    }
    drop(b);
    let l: Layout = read(&layout_dir.join("report.json"))?;
    if l.scans != 600
        || !l.all_published_scans_completed
        || l.snapshots.len() != 2
        || !l.snapshots.iter().enumerate().all(|(i, s)| {
            s.generation == i as u64 + 1 && s.record_restore_passed && s.pressure_readback_passed
        })
        || l.diagnostics.charged_bound_bytes > crate::diagnostic_contract::LAYOUT
    {
        return Err("joint_layout_result".into());
    }
    if !witnessed {
        return Err("joint_no_actual_snapshot_overlap".into());
    }
    let latest = layout_dir.join("snapshots/snapshot-000002");
    let restored = dir.join("layout-restored");
    clock.event("restore_launch_requested", None, None)?;
    let mut restore = start(
        &dir,
        "restore",
        &[
            "layout-load-only".as_ref(),
            latest.as_os_str(),
            restored.as_os_str(),
        ],
    )?;
    while !done(&mut restore)? {
        std::thread::sleep(Duration::from_millis(100));
    }
    clock.event("restore_exited", Some(restore.0.id()), None)?;
    if read::<Restore>(&restored.join("report.json"))?.cold_reopen_and_scan != "passed" {
        return Err("joint_restore_result".into());
    }
    let pool_summary = crate::normal_pool::status(normal_pool.root())?;
    if pool_summary.available_touched_bytes != crate::normal_pool::BYTES
        || pool_summary.granted_capacity_bytes != 0
        || pool_summary.grants != pool_summary.returns
        || pool_summary.grants != 724
        || pool_summary.fill_acknowledgements != 724
    {
        return Err("joint_normal_pool_balance".into());
    }
    normal_pool.finish()?;
    clock.event("normal_pool_exited", None, None)?;
    let supervisor_peak = crate::heap_meter::snapshot().lifetime_peak_requested_bytes;
    if supervisor_peak + crate::heap_meter::TAIL_RESERVED_BYTES
        > crate::diagnostic_contract::SUPERVISOR
    {
        return Err("joint_supervisor_diagnostic_heap_cap".into());
    }
    save(&dir.join("report.json"),&Report{schema:"mycity-joint-budget-diagnostic-v1",source_sha256:env!("MYCITY_SOURCE_SHA256"),diagnostic_component_caps_bytes:[crate::diagnostic_contract::SIM,mib,crate::diagnostic_contract::LAYOUT,2*mib,mib/4,mib,mib/4,crate::diagnostic_contract::POOL],l1_charged_bound_bytes:l.diagnostics.charged_bound_bytes,supervisor_requested_heap_peak_bytes:supervisor_peak,whole_ns:clock.now(),overlap_witness_in_observed_tree:true,scope:"fixed 600s concurrent diagnostic; raw business unchanged; 64MiB fixed whole-role Rust arenas and worker backing; joint physical/native/stack and common diagnostic acceptance unproven",joint_owned_budget:"not_accepted",steady_gate:"not_run",eight_hour_gate:"not_run",memory_16gb_gate:"not_run"})?;
    clock.events.flush().map_err(err)?;
    clock.events.get_ref().sync_all().map_err(err)
}

#[derive(Deserialize)]
struct EnduranceClock {
    formal: bool,
    hours: u64,
    targets: u64,
    queries_completed: u64,
    saves_completed: u64,
    actual_wall_seconds: f64,
    required_wall_seconds: f64,
}
#[derive(Deserialize)]
struct EnduranceReport {
    source_sha256: String,
    clock: EnduranceClock,
    entire_engine_including_metadata: String,
}
/// Full original business and L1 held under one real Rust child owner.
/// A short cadence is functional evidence only, never a formal endurance pass.
pub fn endurance(dir: &Path, formal: bool) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    let mut clock = Clock {
        begin: Instant::now(),
        events: BufWriter::with_capacity(
            65536,
            File::create_new(dir.join("common-events.jsonl")).map_err(err)?,
        ),
    };
    let cadence = if formal { "formal" } else { "smoke" };
    let minimum = if formal { "28800" } else { "600" };
    save(
        &dir.join("manifest.json"),
        &serde_json::json!({
            "schema":"mycity-joint-endurance-v1", "source_sha256":env!("MYCITY_SOURCE_SHA256"),
        "external_owned_charge_bytes":crate::diagnostic_contract::EXTERNAL_WITHOUT_OBSERVER,
            "cadence":cadence, "layout_minimum_seconds":if formal {28800} else {600},
            "layout_hard_failure_seconds":35000,"supervisor_hard_failure_seconds":36000,
            "scan_seconds":1,"snapshot_seconds":300,"normal_pool_bytes":crate::normal_pool::BYTES,
            "scope":"full actual business plus full L1 layout pressure; fixed whole-role Rust arenas inside64MiB, native/stack separate",
            "common_8MiB":"not_accepted", "exact_normal_owned":"not_accepted", "real_16GB":"not_run"
        }),
    )?;
    clock.event("supervisor_started", Some(std::process::id()), None)?;
    let mut pool = crate::normal_pool::Process::spawn(
        &std::env::current_exe().map_err(err)?,
        &dir.join("normal-pool"),
        Some(&dir.join("heaps.jsonl")),
    )?;
    clock.event("normal_pool_ready_observed", None, None)?;
    let layout_dir = dir.join("layout");
    let mut layout = start_owned(
        &dir,
        "layout",
        &[
            "layout-session".as_ref(),
            layout_dir.as_os_str(),
            minimum.as_ref(),
        ],
        true,
    )?;
    let ready = Instant::now();
    loop {
        if done(&mut layout)? {
            return Err("joint_layout_stopped_before_ready".into());
        }
        if let Ok(r) = read::<Ready>(&layout_dir.join("manifest.json")) {
            if r.external_owned_charge_bytes != crate::normal_pool::BYTES as u64
                || r.normal_joint_budget_bytes != crate::layout_load::NORMAL_BYTES as u64
            {
                return Err("joint_ready_shape".into());
            }
            break;
        }
        if ready.elapsed() > Duration::from_secs(120) {
            return Err("joint_layout_ready_deadline".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    clock.event("layout_ready_before_business", Some(layout.0.id()), None)?;
    let business_dir = dir.join("business");
    clock.event("business_launch_requested", None, None)?;
    let mut business = start(
        &dir,
        "business",
        &[
            "scale-endurance".as_ref(),
            business_dir.as_os_str(),
            cadence.as_ref(),
        ],
    )?;
    clock.event("business_spawned", Some(business.0.id()), None)?;
    let mut business_done = false;
    let mut stop_sent = false;
    let mut clock_seen = false;
    loop {
        if clock.begin.elapsed() >= Duration::from_secs(36000) {
            return Err("joint_endurance_deadline".into());
        }
        if !clock_seen && business_dir.join("clock-started.json").exists() {
            clock_seen = true;
            clock.event(
                "business_clock_started_observed",
                Some(business.0.id()),
                None,
            )?;
        }
        if !business_done && done(&mut business)? {
            business_done = true;
            clock.event(
                "business_exited_before_layout_stop",
                Some(business.0.id()),
                None,
            )?;
            let b: EnduranceReport = read(&business_dir.join("report.json"))?;
            if b.source_sha256 != env!("MYCITY_SOURCE_SHA256")
                || b.entire_engine_including_metadata != "equal_independent_fault_reference"
                || b.clock.formal != formal
                || b.clock.hours != 720
                || b.clock.targets != 723
                || b.clock.queries_completed != 2880
                || b.clock.saves_completed != 96
                || !b.clock.actual_wall_seconds.is_finite()
                || b.clock.actual_wall_seconds < b.clock.required_wall_seconds
                || b.clock.required_wall_seconds != if formal { 28800.0 } else { 72.0 }
                || !clock_seen
            {
                return Err("joint_endurance_business_result".into());
            }
            let mut pipe = layout.0.stdin.take().ok_or("joint_layout_owner_pipe")?;
            pipe.write_all(b"S").map_err(err)?;
            drop(pipe);
            stop_sent = true;
            clock.event("layout_stop_sent", Some(layout.0.id()), None)?;
        }
        if done(&mut layout)? {
            if !business_done || !stop_sent {
                return Err("joint_layout_ended_before_business".into());
            }
            clock.event("layout_exited_after_business", Some(layout.0.id()), None)?;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let l: Layout = read(&layout_dir.join("report.json"))?;
    if l.scans < if formal { 28800 } else { 600 }
        || !l.all_published_scans_completed
        || l.snapshots.is_empty()
        || l.snapshots.len() > 128
        || !l.snapshots.iter().enumerate().all(|(i, s)| {
            s.generation == i as u64 + 1 && s.record_restore_passed && s.pressure_readback_passed
        })
        || l.diagnostics.charged_bound_bytes > crate::diagnostic_contract::LAYOUT
    {
        return Err("joint_endurance_layout_result".into());
    }
    let generation = l
        .snapshots
        .last()
        .ok_or("joint_snapshot_missing")?
        .generation;
    let latest = layout_dir.join(format!("snapshots/snapshot-{generation:06}"));
    let restored = dir.join("layout-restored");
    clock.event("restore_launch_requested", None, Some(generation))?;
    let mut restore = start(
        &dir,
        "restore",
        &[
            "layout-load-only".as_ref(),
            latest.as_os_str(),
            restored.as_os_str(),
        ],
    )?;
    let began = Instant::now();
    while !done(&mut restore)? {
        if began.elapsed() >= Duration::from_secs(120) {
            return Err("joint_restore_deadline".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if read::<Restore>(&restored.join("report.json"))?.cold_reopen_and_scan != "passed" {
        return Err("joint_restore_result".into());
    }
    clock.event(
        "latest_layout_restored",
        Some(restore.0.id()),
        Some(generation),
    )?;
    let balance = crate::normal_pool::status(pool.root())?;
    if balance.available_touched_bytes != crate::normal_pool::BYTES
        || balance.granted_capacity_bytes != 0
        || balance.grants != 725
        || balance.returns != 725
        || balance.fill_acknowledgements != 725
    {
        return Err("joint_endurance_pool_balance".into());
    }
    pool.finish()?;
    clock.event("normal_pool_exited", None, None)?;
    let peak = crate::heap_meter::snapshot().lifetime_peak_requested_bytes;
    if peak + crate::heap_meter::TAIL_RESERVED_BYTES > crate::diagnostic_contract::SUPERVISOR {
        return Err("joint_supervisor_diagnostic_heap_cap".into());
    }
    save(
        &dir.join("report.json"),
        &serde_json::json!({
            "schema":"mycity-joint-endurance-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),
            "cadence":cadence,"whole_ns":clock.now(),"latest_restored_generation":generation,
            "layout_scans":l.scans,"business_and_layout_completed":true,
            "L1_coverage":"ready before business spawn; stop after complete business exit",
            "supervisor_requested_heap_peak_bytes":peak,
            "common_8MiB":"not_accepted","exact_normal_owned":"not_accepted",
            "formal_steady":"not_run","four_speeds":"not_run",
            "eight_hour":if formal {"requires_independent_audit"} else {"not_run"},
            "real_16GB":"not_run","full_M0":"not_accepted"
        }),
    )?;
    clock.events.flush().map_err(err)?;
    clock.events.get_ref().sync_all().map_err(err)
}
