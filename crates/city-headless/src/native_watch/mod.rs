//! Native bounded process-group observer. It never grants pressure replacement credit.
mod clock;
mod disk;
pub mod fixture;
pub mod owner;
pub mod registration;
mod summary;
mod system;
mod tracker;
use crate::{heap_meter, native_observer::capture};
use serde::{ser::SerializeSeq, Serialize};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn now(origin: Instant) -> u64 {
    origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}
fn save(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = File::create_new(path).map_err(err)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(err)?;
    file.write_all(b"\n").map_err(err)?;
    file.sync_all().map_err(err)
}
struct Members<'a>(&'a [tracker::Member]);
impl Serialize for Members<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for x in self.0 {
            let id = x.sample.identity;
            seq.serialize_element(&(
                id.pid,
                id.parent_pid,
                id.start_abstime,
                x.sample.usage.resident_bytes,
                x.role,
            ))?;
        }
        seq.end()
    }
}
struct Footprints<'a>(&'a [tracker::Member]);
impl Serialize for Footprints<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for x in self.0 {
            seq.serialize_element(&(
                x.sample.identity.pid,
                x.sample.identity.start_abstime,
                x.sample.usage.footprint_bytes,
            ))?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct SystemFrame {
    vm_sysctl: system::Vm,
    vm_page_size_bytes: u64,
    physical_memory_bytes: u64,
    disk_bytes: u64,
    available_disk_bytes: u64,
    swap_total_bytes: u64,
}
#[derive(Serialize)]
struct Frame<'a> {
    clock_begin_ns: u64,
    clock_end_ns: u64,
    t: f64,
    window_s: f64,
    rss_bytes: u64,
    phys_footprint_bytes: u64,
    missing_footprint: usize,
    missing_rss: usize,
    group_capture_complete: bool,
    members: Members<'a>,
    member_footprints_pid_birth_bytes: Footprints<'a>,
    identity_uncertainties: &'a [tracker::Issue],
    pending_registration_bytes: u64,
    registration_lock_busy: bool,
    r07_applied: Option<crate::layout_load::r07::Applied>,
    r07_reading: Option<crate::r07_runtime::Reading>,
    r07_coverage_complete: bool,
    r07_provider_paused: bool,
    pressure_dispatch_flag: u64,
    swap_used_bytes: u64,
    observer_hot_bound_bytes: usize,
    cpu_io_footprint: Option<&'a [tracker::Member]>,
    system: Option<SystemFrame>,
}
struct Stats {
    samples: u64,
    log_bytes: u64,
    max_gap: u64,
    max_capture: u64,
    last_end: Option<u64>,
    rss: u64,
    foot: u64,
    rss_frame: u64,
    foot_frame: u64,
    disk: u64,
    missing: u64,
    r07_last: Option<crate::layout_load::r07::Applied>,
    r07_max: usize,
}
impl Stats {
    fn new() -> Self {
        Self {
            samples: 0,
            log_bytes: 0,
            max_gap: 0,
            max_capture: 0,
            last_end: None,
            rss: 0,
            foot: 0,
            rss_frame: 0,
            foot_frame: 0,
            disk: 0,
            missing: 0,
            r07_last: None,
            r07_max: 0,
        }
    }
}
fn diagnostic_charge(root: &Path) -> Result<u64, String> {
    let path = root.join("model/manifest.json");
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(err(e)),
    };
    #[derive(serde::Deserialize)]
    struct Projection {
        #[serde(default)]
        external_owned_charge_bytes: u64,
    }
    let raw = mycity_core::read_bounded(file, 65536).map_err(err)?;
    let value: Projection = serde_json::from_slice(&raw).map_err(err)?;
    Ok(value.external_owned_charge_bytes)
}
// Only orchestration processes already included in the joint observer are
// eligible. Layout/pool/model workers must never become residual credits.
fn r07_root_enabled(command: &str) -> bool {
    ["steady-run", "r07-layout-driver", "joint-endurance"].contains(&command)
}
fn r07_component_slot(role: &str, is_observer: bool) -> Option<usize> {
    if is_observer {
        Some(0)
    } else if [
        "steady-run",
        "direct-root",
        "r07-layout-driver",
        "joint-endurance",
    ]
    .contains(&role)
    {
        Some(1)
    } else if ["steady-driver", "scale-endurance"].contains(&role) {
        Some(2)
    } else {
        None
    }
}
fn r07_combined_pool_root(command: &[String]) -> bool {
    command.get(1).is_some_and(|s| s == "r07-layout-driver")
        && command
            .get(3..)
            .is_some_and(|flags| flags.iter().any(|s| s == "--combined-pool"))
}
fn r07_eligible_component_slot(
    role: &str,
    is_observer: bool,
    pid: u32,
    excluded_root_pid: Option<u32>,
) -> Option<usize> {
    // A mixed pool/orchestration process cannot exchange its normal backing for
    // diagnostic padding. Reject by actual child PID even before role registration
    // and on any later identity anomaly; raw group accounting is unchanged.
    if excluded_root_pid == Some(pid) {
        None
    } else {
        r07_component_slot(role, is_observer)
    }
}
fn r07_layout_role(role: &str) -> bool {
    [
        "steady-layout-session",
        "layout-session",
        "layout-load-only",
        "layout-charged-probe",
        "layout-r07-session",
    ]
    .contains(&role)
}

fn publish_r07(
    root: &Path,
    tracker: &tracker::Tracker,
    nonce: &str,
    sequence: u64,
    begin: u64,
    excluded_root_pid: Option<u32>,
) -> Result<crate::r07_runtime::Reading, String> {
    use crate::r07_runtime::{Component, Reading, Role};
    let mut components = [None; 3];
    for m in &tracker.members {
        let slot = r07_eligible_component_slot(
            m.role.as_str(),
            m.sample.identity.pid == std::process::id(),
            m.sample.identity.pid,
            excluded_root_pid,
        );
        if let Some(i) = slot {
            if components[i].is_some() {
                return Err("r07_ambiguous_role".into());
            }
            components[i] = Some(Component {
                role: [Role::Observer, Role::Supervisor, Role::Driver][i],
                identity: m.sample.identity,
                footprint_bytes: m.sample.usage.footprint_bytes,
            });
        }
    }
    let reading = Reading {
        schema: 1,
        source: crate::control_data::Text::new(env!("MYCITY_SOURCE_SHA256"))?,
        nonce: crate::control_data::Text::new(nonce)?,
        sequence,
        monotonic_begin_ns: begin,
        monotonic_end_ns: crate::r07_runtime::monotonic_ns()?,
        components,
    };
    let tmp = root.join("r07-runtime.tmp");
    save(&tmp, &reading)?;
    fs::rename(tmp, root.join("r07-runtime.json")).map_err(err)?;
    Ok(reading)
}
fn read_r07(
    root: &Path,
    tracker: &tracker::Tracker,
) -> Result<Option<crate::layout_load::r07::Applied>, String> {
    let file = match File::open(root.join("model/layout/r07-state.json")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    let raw = mycity_core::read_bounded(file, 4096).map_err(err)?;
    let ack: crate::layout_load::r07::Applied = serde_json::from_slice(&raw).map_err(err)?;
    if ack.source.as_str() != env!("MYCITY_SOURCE_SHA256") {
        return Err("r07_ack_source".into());
    }
    ack.state.validate(ack.state.logical_bytes)?;
    Ok(ack
        .owner
        .filter(|id| tracker.members.iter().any(|m| m.sample.identity == *id))
        .map(|_| ack))
}
struct Capture<'a> {
    root: &'a Path,
    origin: Instant,
    tracker: &'a mut tracker::Tracker,
    journal: &'a mut registration::Journal,
    nonce: &'a str,
    guard: &'a mut system::Guard,
    stats: &'a mut Stats,
    writer: &'a mut BufWriter<File>,
    buffer: &'a mut disk::Line,
    host: &'a system::Host,
    r07_enabled: bool,
    r07_pause_allowed: bool,
    r07_excluded_root_pid: Option<u32>,
}
fn tick(context: Capture<'_>) -> Result<(), String> {
    let Capture {
        root,
        origin,
        tracker,
        journal,
        nonce,
        guard,
        stats,
        writer,
        buffer,
        host,
        r07_enabled,
        r07_pause_allowed,
        r07_excluded_root_pid,
    } = context;
    let begin = now(origin);
    let monotonic_begin = crate::r07_runtime::monotonic_ns()?;
    journal.read(nonce, |r| tracker.register(r))?;
    tracker.poll(origin)?;
    let critical = system::critical()?;
    guard.observe(now(origin), critical)?;
    let usage = disk::scan(root)?;
    let free = fs2::available_space(root).map_err(err)?;
    if usage.total > 20 * 1024 * 1024 * 1024 {
        return Err("native_world_disk_limit".into());
    }
    if free < 2 * 1024 * 1024 * 1024 {
        return Err("native_free_disk_floor".into());
    }
    let other = diagnostic_charge(root)?;
    let own = heap_meter::arena_snapshot();
    let charged = own
        .activation
        .ok_or("native_observer_arena_missing")?
        .grant_bytes;
    if own.allocation_failures != 0
        || own.lifetime_peak_requested_bytes + heap_meter::TAIL_RESERVED_BYTES
            > crate::diagnostic_contract::OBSERVER
        || charged as u64 + other > 8 * 1024 * 1024
    {
        return Err("native_observer_diagnostic_budget".into());
    }
    let rss = tracker
        .members
        .iter()
        .map(|m| m.sample.usage.resident_bytes)
        .sum();
    let foot = tracker
        .members
        .iter()
        .map(|m| m.sample.usage.footprint_bytes)
        .sum();
    let missing = tracker.issues.len();
    let periodic =
        stats.samples == 0 || begin / 60_000_000_000 > stats.last_end.unwrap_or(0) / 60_000_000_000;
    let sys = periodic.then(|| SystemFrame {
        vm_sysctl: system::vm(),
        vm_page_size_bytes: host.page_size_bytes,
        physical_memory_bytes: host.physical_memory_bytes,
        disk_bytes: usage.total,
        available_disk_bytes: free,
        swap_total_bytes: critical.total,
    });
    // Fault control exists only for the explicit diagnostic root. Sampling,
    // pressure/swap guards and owner protection continue throughout the pause.
    let r07_provider_paused = r07_pause_allowed && root.join("r07-provider-pause").exists();
    let r07_reading = if r07_enabled && !r07_provider_paused {
        Some(publish_r07(
            root,
            tracker,
            nonce,
            stats.samples + 1,
            monotonic_begin,
            r07_excluded_root_pid,
        )?)
    } else {
        None
    };
    let r07_applied = read_r07(root, tracker)?;
    let expected_r07 = r07_enabled
        && tracker
            .members
            .iter()
            .any(|m| r07_layout_role(m.role.as_str()));
    let coverage_bytes = r07_reading.map_or(0, |r| {
        r.components
            .iter()
            .flatten()
            .map(|c| c.footprint_bytes)
            .sum::<u64>()
    });
    let r07_coverage_complete = match r07_applied {
        Some(ack) => {
            (ack.state.replaced_bytes as u64)
                <= coverage_bytes / crate::layout_load::r07::QUANTUM as u64
                    * crate::layout_load::r07::QUANTUM as u64
        }
        None => !expected_r07,
    };
    stats.r07_last = r07_applied;
    if let Some(ack) = r07_applied {
        stats.r07_max = stats.r07_max.max(ack.state.replaced_bytes);
    }
    let pending = journal.pending()?;
    let end = now(origin);
    let frame = Frame {
        clock_begin_ns: begin,
        clock_end_ns: end,
        t: end as f64 / 1e9,
        window_s: (end - begin) as f64 / 1e9,
        rss_bytes: rss,
        phys_footprint_bytes: foot,
        missing_footprint: missing,
        missing_rss: missing,
        group_capture_complete: missing == 0
            && pending == 0
            && !journal.busy
            && r07_coverage_complete,
        members: Members(&tracker.members),
        member_footprints_pid_birth_bytes: Footprints(&tracker.members),
        identity_uncertainties: &tracker.issues,
        pending_registration_bytes: pending,
        registration_lock_busy: journal.busy,
        r07_applied,
        r07_reading,
        r07_coverage_complete,
        r07_provider_paused,
        pressure_dispatch_flag: critical.pressure,
        swap_used_bytes: critical.used,
        observer_hot_bound_bytes: charged,
        cpu_io_footprint: periodic.then_some(&tracker.members),
        system: sys,
    };
    let bytes = buffer.encode(&frame)?;
    if stats.log_bytes + bytes.len() as u64 + usage.latency > 256 * 1024 * 1024 {
        return Err("native_measurement_log_limit".into());
    }
    writer.write_all(bytes).map_err(err)?;
    writer.flush().map_err(err)?;
    stats.log_bytes += bytes.len() as u64;
    stats.max_capture = stats.max_capture.max(end - begin);
    if let Some(last) = stats.last_end {
        stats.max_gap = stats.max_gap.max(end - last);
    }
    stats.last_end = Some(end);
    if rss > stats.rss {
        stats.rss = rss;
        stats.rss_frame = stats.samples;
    }
    if foot > stats.foot {
        stats.foot = foot;
        stats.foot_frame = stats.samples;
    }
    stats.disk = stats.disk.max(usage.total);
    stats.missing += missing as u64;
    stats.samples += 1;
    Ok(())
}
fn cleanup(
    child: &mut Child,
    origin: Instant,
    tracker: &mut tracker::Tracker,
    journal: &mut registration::Journal,
    nonce: &str,
) -> Result<(), String> {
    let _ = journal.read(nonce, |r| tracker.register(r));
    let _ = tracker.poll(origin);
    // The direct Child has not been reaped if try_wait returns None; its PID
    // cannot be recycled. This also covers failure before the first birth read.
    if child.try_wait().map_err(err)?.is_none() {
        child.kill().map_err(err)?;
    }
    // Only proven descendants whose current birth still matches. There is no
    // permission to signal an arbitrary PID from a previous observer frame.
    for signal in [rustix::process::Signal::TERM, rustix::process::Signal::KILL] {
        for identity in tracker.owned_identities() {
            if identity.pid == std::process::id() {
                continue;
            }
            if let Ok(sample) = capture(identity.pid, None, origin) {
                if sample.identity.start_abstime == identity.start_abstime {
                    if let Some(pid) = i32::try_from(identity.pid)
                        .ok()
                        .and_then(rustix::process::Pid::from_raw)
                    {
                        let _ = rustix::process::kill_process(pid, signal);
                    }
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let done = child.try_wait().map_err(err)?.is_some();
            journal.read(nonce, |r| tracker.register(r))?;
            tracker.poll(origin)?;
            if done && tracker.live_other_than(std::process::id()) == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Err("native_cleanup_live_or_unknown_descendants".into())
}
/// CLI wrapper; target must be the same fixed executable, never a foreign command.
pub fn run(root: &Path, command: &[String]) -> Result<(), String> {
    if command.is_empty() || command.len() > 32 || command.iter().any(|s| s.len() > 4096) {
        return Err("native_observer_command_capacity".into());
    }
    if Path::new(&command[0]).canonicalize().map_err(err)?
        != std::env::current_exe()
            .map_err(err)?
            .canonicalize()
            .map_err(err)?
    {
        return Err("native_observer_same_binary_required".into());
    }
    if !command.get(1).is_some_and(|s| {
        [
            "steady-run",
            "r07-layout-driver",
            "joint-endurance",
            "speed-run",
            "pe0-speed-probe",
            "native-watch-fixture",
        ]
        .contains(&s.as_str())
    }) {
        return Err("native_observer_unsupported_root_command".into());
    }
    fs::create_dir(root).map_err(err)?;
    let root = root.canonicalize().map_err(err)?;
    heap_meter::activate_role(crate::diagnostic_contract::OBSERVER)?;
    let origin = Instant::now();
    let host = system::host()?;
    let initial = system::critical()?;
    let mut guard = system::Guard::new(initial.used);
    let own = capture(std::process::id(), None, origin)
        .map_err(|e| format!("native_observer_self:{e:?}"))?;
    let mut tracker = tracker::Tracker::new(own)?;
    let nonce = clock::nonce()?;
    File::create_new(root.join("registrations.jsonl")).map_err(err)?;
    let mut journal = registration::Journal::new(&root.join("registrations.jsonl"))?;
    let mut samples = BufWriter::with_capacity(
        65536,
        File::create_new(root.join("host-samples.jsonl")).map_err(err)?,
    );
    let mut buffer = disk::Line::new();
    let mut stats = Stats::new();
    let binary_before = disk::hash(Path::new(&command[0]))?;
    let mut process = Command::new(&command[0]);
    let r07_enabled = command.get(1).is_some_and(|s| r07_root_enabled(s));
    process.env_remove("MYCITY_R07_RUNTIME");
    if r07_enabled {
        process.env("MYCITY_R07_RUNTIME", root.join("r07-runtime.json"));
    }
    process
        .arg("native-observed-root")
        .args(&command[1..])
        .process_group(0)
        .env("MYCITY_HEAP_LOG", root.join("model/heaps.jsonl"))
        .env("MYCITY_PROCESS_LOG", root.join("registrations.jsonl"))
        .env("MYCITY_PROCESS_NONCE", &nonce)
        .env("MYCITY_OBSERVER_CLOCK", "1")
        .env("MYCITY_OBSERVER_OWNER_PID", std::process::id().to_string())
        .stdin(Stdio::piped())
        .stdout(File::create_new(root.join("model.stdout")).map_err(err)?)
        .stderr(File::create_new(root.join("model.stderr")).map_err(err)?);
    let spawn = now(origin);
    let mut child = process.spawn().map_err(err)?;
    let excluded_root_pid = r07_combined_pool_root(command).then_some(child.id());
    let mut exit: Option<ExitStatus> = None;
    let result = (|| {
        if let Ok(sample) = capture(child.id(), None, origin) {
            if sample.identity.parent_pid != std::process::id() {
                return Err("native_spawn_parent_mismatch".into());
            }
            tracker.add(
                sample.identity,
                crate::control_data::Text::new("direct-root")?,
                false,
            )?;
            save(&root.join("observer-child.json"), &sample)?;
            if excluded_root_pid.is_some() {
                save(
                    &root.join("r07-excluded-root.json"),
                    &serde_json::json!({
                        "schema":"mycity-r07-excluded-mixed-root-v1",
                        "source_sha256":env!("MYCITY_SOURCE_SHA256"),
                        "identity":sample.identity,"exclusion":"entire_actual_child_pid_for_lifetime",
                        "reason":"combined root owns the normal64MiB pool; not diagnostic-only",
                        "raw_group_unchanged":true,"estimated_footprint_deduction_bytes":0
                    }),
                )?;
            }
        } else {
            return Err("native_spawn_identity_unconfirmed".into());
        }
        child
            .stdin
            .as_mut()
            .ok_or("native_observer_owner_pipe")?
            .write_all(b"O")
            .map_err(err)?;
        clock::exchange(&mut child, &root, origin, spawn)?;
        let mut post = 0;
        let mut next = now(origin);
        loop {
            tick(Capture {
                root: &root,
                origin,
                tracker: &mut tracker,
                journal: &mut journal,
                nonce: &nonce,
                guard: &mut guard,
                stats: &mut stats,
                writer: &mut samples,
                buffer: &mut buffer,
                host: &host,
                r07_enabled,
                r07_pause_allowed: command.get(1).is_some_and(|s| s == "r07-layout-driver"),
                r07_excluded_root_pid: excluded_root_pid,
            })?;
            if exit.is_none() {
                exit = child.try_wait().map_err(err)?;
            }
            if exit.is_some() {
                post += 1;
                if post >= 3 {
                    if tracker.live_other_than(std::process::id()) != 0 {
                        return Err("native_model_exited_with_live_or_unknown_descendants".into());
                    }
                    break;
                }
            }
            if origin.elapsed() > Duration::from_secs(36000) {
                return Err("native_observer_wall_limit".into());
            }
            next = next
                .checked_add(250_000_000)
                .ok_or("native_observer_clock_overflow")?;
            std::thread::sleep(Duration::from_nanos(next.saturating_sub(now(origin))));
        }
        Ok::<(), String>(())
    })();
    let cleanup_error = if result.is_err() {
        cleanup(&mut child, origin, &mut tracker, &mut journal, &nonce).err()
    } else {
        None
    };
    if exit.is_none() {
        exit = child.try_wait().map_err(err)?;
    }
    samples.flush().map_err(err)?;
    samples.get_ref().sync_all().map_err(err)?;
    let unchanged = binary_before == disk::hash(Path::new(&command[0]))?;
    let success =
        result.is_ok() && cleanup_error.is_none() && exit.is_some_and(|s| s.success()) && unchanged;
    let other = diagnostic_charge(&root).ok();
    let report = summary::Report {
        host: &host,
        success,
        reason: result.as_ref().err().map(String::as_str),
        cleanup: cleanup_error.as_deref(),
        command,
        binary_hash: &binary_before,
        unchanged,
        exit: exit.and_then(|s| s.code()),
        duration: origin.elapsed().as_secs_f64(),
        stats: &stats,
        tracker: &tracker,
        registrations: journal.records,
        other,
        initial_swap: initial.used,
        guard: &guard,
    };
    save(&root.join("observer-report.json"), &report)?;
    if success {
        Ok(())
    } else {
        Err("native_observer_failed; see observer-report.json".into())
    }
}

#[cfg(test)]
mod r07_role_tests {
    use super::*;

    #[test]
    fn mixed_pool_root_is_excluded_before_and_after_role_registration() {
        for role in ["direct-root", "r07-layout-driver", "unknown"] {
            assert_eq!(r07_eligible_component_slot(role, false, 17, Some(17)), None);
        }
        assert_eq!(
            r07_eligible_component_slot("r07-layout-driver", false, 17, None),
            Some(1)
        );
        assert_eq!(
            r07_eligible_component_slot("native-observer", true, 16, Some(17)),
            Some(0)
        );
        let args = |values: &[&str]| values.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(r07_combined_pool_root(&args(&[
            "exe",
            "r07-layout-driver",
            "dir",
            "--combined-pool"
        ])));
        assert!(!r07_combined_pool_root(&args(&[
            "exe",
            "r07-layout-driver",
            "--combined-pool"
        ])));
        assert!(!r07_combined_pool_root(&args(&[
            "exe",
            "r07-layout-driver",
            "dir",
            "--vm-trace"
        ])));
        assert!(!r07_combined_pool_root(&args(&[
            "exe",
            "steady-run",
            "dir",
            "--combined-pool"
        ])));
    }
    #[test]
    fn joint_endurance_uses_only_same_use_orchestrators() {
        assert!(r07_root_enabled("joint-endurance"));
        assert_eq!(r07_component_slot("joint-endurance", false), Some(1));
        assert_eq!(r07_component_slot("scale-endurance", false), Some(2));
        assert!(r07_layout_role("layout-session"));
        assert_eq!(r07_component_slot("native-observer", true), Some(0));
        for role in [
            "layout-session",
            "normal-pool",
            "sim-host",
            "model-worker",
            "unknown",
        ] {
            assert_eq!(r07_component_slot(role, false), None);
            assert!(!r07_root_enabled(role));
        }
    }

    #[test]
    fn steady_and_diagnostic_roles_keep_existing_slots() {
        for role in ["steady-run", "direct-root", "r07-layout-driver"] {
            assert_eq!(r07_component_slot(role, false), Some(1));
        }
        assert_eq!(r07_component_slot("steady-driver", false), Some(2));
        assert!(r07_root_enabled("steady-run"));
        assert!(r07_root_enabled("r07-layout-driver"));
        assert!(!r07_root_enabled("speed-run"));
        assert!(!r07_root_enabled("scale-endurance"));
        assert!(r07_layout_role("steady-layout-session"));
    }
}
