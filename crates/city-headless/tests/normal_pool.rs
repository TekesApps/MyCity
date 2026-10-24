//! Actual 64MiB owner and real bounded workers. Not a full L1/30d performance gate.
use mycity_core::{
    air::{Input, Source, State, STRATEGY},
    sha256,
};
use mycity_headless::{
    compute_worker::{self as protocol, Request, Stage, Ticket, Worker, PROTOCOL},
    normal_pool::{self, Process, BYTES, GRANT_BYTES},
};
use std::os::unix::net::UnixStream;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let base = std::env::var_os("MYCITY_POOL_TEST_ARTIFACT_ROOT").map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let p = base.join(format!(
            "mc-pool-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn tasks(&self, n: usize) -> PathBuf {
        let p = self.0.join(format!("tasks-{n}"));
        fs::create_dir(&p).unwrap();
        fs::create_dir(p.join("inputs")).unwrap();
        fs::create_dir(p.join("candidates")).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os("MYCITY_POOL_TEST_ARTIFACT_ROOT").is_some() {
            eprintln!("worker_regression_fixture_retained:{}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn exe() -> &'static Path {
    static EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXE.get_or_init(|| {
        let path = std::env::var_os("MYCITY_PROTOCOL_WORKER_EXE").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")));
        assert!(path.is_absolute() && path.is_file(), "actual fixed CLI absent");
        path
    }).as_path()
}
fn state() -> State {
    State::new(Input {
        schema: STRATEGY.into(),
        side: 32,
        target: 120,
        initial_micro: 2_000_000,
        boundary_micro: [Some(2_000_000); 4],
        boundary_until: 120,
        sources: (1..=3)
            .map(|id| Source {
                id,
                cell: 528 + id,
                start: 0,
                end: 120,
                rate_micro: 10000,
            })
            .collect(),
    })
    .unwrap()
}
fn request(dir: &Path, state: &State, job: u64) -> Request {
    Request {
        protocol: PROTOCOL.into(),
        strategy: STRATEGY.into(),
        ticket: Ticket {
            world: 1,
            branch: 1,
            job,
            incarnation: 1,
            generation: 1,
            attempt: 1,
            base_revision: 0,
            field_version: 0,
            start: 0,
            end: 120,
            rules_sha256: sha256(b"pool-test-rules"),
            source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
            input_sha256: state.input().identity(),
            checkpoint_sha256: protocol::publish(&dir.join("inputs"), &state.checkpoint().unwrap())
                .unwrap(),
        },
    }
}
#[test]
fn real_worker_retains_and_reuses_slots_until_owned_wait_then_returns_actual_pages() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, Some(&t.0.join("heaps.jsonl"))).unwrap();
    assert_eq!(
        normal_pool::status(&root).unwrap().available_touched_bytes,
        BYTES
    );
    let tasks = t.tasks(1);
    let mut worker = Worker::new();
    worker.start_bounded(exe(), &tasks, Some(&root), false).unwrap();
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.available_touched_bytes, BYTES - GRANT_BYTES);
    assert_eq!(s.fill_acknowledgements, 0);
    let mut expected = state();
    while !expected.complete() {
        expected.page(256, 256).unwrap();
    }
    let expected = expected.checkpoint().unwrap();
    for job in [1, 2] {
        worker.submit(&request(&tasks, &state(), job)).unwrap();
        loop {
            let e = worker.next(Duration::from_secs(5)).unwrap();
            if e.stage == Stage::Complete {
                let bytes = fs::read(
                    tasks
                        .join("candidates")
                        .join(format!("{}.air", e.candidate_sha256.unwrap())),
                )
                .unwrap();
                assert_eq!(bytes, expected);
                break;
            }
        }
    }
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.grants, 1);
    assert_eq!(s.fill_acknowledgements, 1);
    assert_eq!(s.returns, 0);
    assert_eq!(s.available_touched_bytes, BYTES - GRANT_BYTES);
    assert!(worker
        .stop_when_idle(Duration::from_secs(2))
        .unwrap()
        .success());
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.available_touched_bytes, BYTES);
    assert_eq!(s.grants, s.returns);
    assert_eq!(s.granted_capacity_bytes, 0);
    pool.finish().unwrap();
    let r: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("report.json")).unwrap()).unwrap();
    assert_eq!(r["balanced_after_owned_wait"], true);
    let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
    let fills: Vec<serde_json::Value> = events
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|e: &serde_json::Value| e["kind"] == "worker_actual_slots_filled")
        .collect();
    assert_eq!(fills.len(), 1);
    assert!(fills[0]["typed_instance"].as_u64().unwrap() > 0);
    assert_eq!(
        fills[0]["typed_capacity"],
        mycity_core::air::WORKSPACE_BYTES
    );
    assert_eq!(fills[0]["summary"]["granted_capacity_bytes"], GRANT_BYTES);
    assert_eq!(GRANT_BYTES, 5 * 1024 * 1024);
    assert!(r["normal_owned_acceptance"]
        .as_str()
        .unwrap()
        .starts_with("not_accepted"));
}
#[test]
fn killed_workers_only_return_capacity_after_parent_wait_and_spawn_failure_aborts() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let mut failed = Worker::new();
    assert!(failed.start_bounded(Path::new("/nonexistent-mycity-worker"),
        &t.tasks(0), Some(&root), false).is_err());
    // Failed spawn keeps the original unspawned grant until explicit Abort ACK.
    assert!(!failed.is_empty());
    assert_eq!(normal_pool::status(&root).unwrap().returns, 0);
    failed.retire(Duration::ZERO).unwrap();
    assert!(failed.is_empty());
    assert_eq!(normal_pool::status(&root).unwrap().returns, 1);
    for (i, stage) in [Stage::Sources, Stage::Cells, Stage::Published]
        .into_iter()
        .enumerate()
    {
        let tasks = t.tasks(i + 1);
        let mut worker = Worker::new();
        worker.start_bounded(exe(), &tasks, Some(&root), true).unwrap();
        worker
            .submit(&request(&tasks, &state(), i as u64 + 1))
            .unwrap();
        loop {
            let e = worker.next(Duration::from_secs(5)).unwrap();
            if e.stage == stage {
                break;
            }
            worker.acknowledge_probe(&e).unwrap();
        }
        let s = normal_pool::status(&root).unwrap();
        assert_eq!(s.available_touched_bytes, BYTES - GRANT_BYTES);
        assert!(!worker.stop().unwrap().success());
        let s = normal_pool::status(&root).unwrap();
        assert_eq!(s.available_touched_bytes, BYTES);
        assert_eq!(s.grants, s.returns);
    }
    pool.finish().unwrap();
    let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
    assert_eq!(
        events
            .lines()
            .filter(|s| s.contains("owned_child_wait_failure"))
            .count(),
        3
    );
}
#[test]
fn old_boot_bad_control_and_parent_pipe_closure_cannot_create_credit() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut child = Command::new(exe())
        .arg("normal-pool-host")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let began = Instant::now();
    while normal_pool::status(&root).is_err() {
        assert!(child.try_wait().unwrap().is_none());
        assert!(began.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("ready.json")).unwrap()).unwrap();
    let mut socket = UnixStream::connect(root.join("pool.sock")).unwrap();
    let bad = serde_json::json!({"source":ready["source"],"boot":0,"action":"status"});
    serde_json::to_writer(&mut socket, &bad).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut out = String::new();
    socket.read_to_string(&mut out).unwrap();
    assert!(out.contains("normal_pool_request_boot"));
    let mut socket = UnixStream::connect(root.join("pool.sock")).unwrap();
    // The server may reject at8KiB while the final excess byte is still being
    // sent. BrokenPipe/reset is a legitimate fail-closed response, not a grant.
    if let Err(e) = socket.write_all(&vec![b' '; 8193]) {
        assert!(
            matches!(
                e.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            ),
            "{e}"
        );
    }
    let _ = socket.shutdown(std::net::Shutdown::Write);
    let mut out = String::new();
    let _ = socket.read_to_string(&mut out);
    if let Ok(reply) = serde_json::from_str::<serde_json::Value>(&out) {
        assert_eq!(reply["error"], "normal_pool_control_bound");
    } // A rejected connection may also lose or truncate its reply; event below is required.
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.rejected_requests, 2);
    assert_eq!(s.available_touched_bytes, BYTES);
    assert_eq!(s.grants, 0);
    child.stdin.take();
    let began = Instant::now();
    loop {
        if let Some(s) = child.try_wait().unwrap() {
            assert!(s.success());
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(root.join("report.json").exists());
    let events: Vec<serde_json::Value> = fs::read_to_string(root.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        events
            .iter()
            .filter(
                |e| e["kind"] == "rejected" && e["rejection_reason"] == "normal_pool_control_bound"
            )
            .count(),
        1
    );
}

#[test]
fn existing_run_directory_or_live_ready_is_never_adopted_by_a_new_process() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    assert!(Process::spawn(exe(), &root, None).is_err());
    assert_eq!(
        normal_pool::status(&root).unwrap().available_touched_bytes,
        BYTES
    );
    pool.finish().unwrap();
    assert!(Process::spawn(exe(), &root, None).is_err());
}
#[test]
fn internal_mutation_failures_terminate_service_without_publishing_balanced_credit() {
    for (i, fault) in ["after-attach-event", "return-page-2"]
        .into_iter()
        .enumerate()
    {
        let t = Temp::new();
        let root = t.0.join("pool");
        let mut child = Command::new(exe())
            .arg("normal-pool-host")
            .arg(&root)
            .env("MYCITY_NORMAL_POOL_FAULT", fault)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let began = Instant::now();
        while normal_pool::status(&root).is_err() {
            assert!(child.try_wait().unwrap().is_none());
            assert!(began.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
        let tasks = t.tasks(i);
        let mut worker = Worker::new();
        let result = worker.start_bounded(exe(), &tasks, Some(&root), false);
        if fault == "after-attach-event" {
            assert!(result.is_err());
        } else {
            result.unwrap();
            worker.submit(&request(&tasks, &state(), 1)).unwrap();
            loop {
                let e = worker.next(Duration::from_secs(5)).unwrap();
                if e.stage == Stage::Complete {
                    break;
                }
            }
            assert!(worker.stop_when_idle(Duration::from_secs(2)).is_err());
        }
        let began = Instant::now();
        loop {
            if let Some(s) = child.try_wait().unwrap() {
                assert!(!s.success());
                break;
            }
            assert!(began.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!root.join("report.json").exists());
        let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
        assert!(!events.contains("returned_after_wait_or_abort"));
        if fault == "return-page-2" {
            let failure: serde_json::Value = events
                .lines()
                .map(|s| serde_json::from_str(s).unwrap())
                .find(|v: &serde_json::Value| v["kind"] == "injected_partial_return_failure")
                .unwrap();
            assert_eq!(
                failure["owner_actual_capacity_bytes"],
                BYTES - GRANT_BYTES + 1024 * 1024
            );
            assert_eq!(failure["summary"]["returns"], 0);
        }
    }
}

#[test]
fn fragmented_control_waits_for_suffix_but_timeout_eof_and_identity_remain_distinct() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let ready: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("ready.json")).unwrap()).unwrap();
    let fake_key = "0".repeat(64);
    let request = serde_json::json!({"source":ready["source"],"boot":ready["boot"],
        "action":{"attach":{"id":0,"key":fake_key,"pid":1}}});
    let mut packet = serde_json::to_vec(&request).unwrap();
    packet.push(b'\n');
    let connect = || {
        let s = UnixStream::connect(root.join("pool.sock")).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s
    };
    let reply = |s: &mut UnixStream| -> serde_json::Value {
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        serde_json::from_str(&out).unwrap()
    };
    let mut socket = connect();
    socket.write_all(&packet[..17]).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let early = socket.read(&mut [0]);
    assert!(
        matches!(early, Err(ref e) if matches!(e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)),
        "{early:?}"
    );
    socket.write_all(&packet[17..]).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    assert_eq!(
        reply(&mut socket)["error"],
        "normal_pool_stale_or_wrong_grant"
    );

    let mut socket = connect();
    socket.write_all(&packet[..17]).unwrap();
    let began = Instant::now();
    assert_eq!(reply(&mut socket)["error"], "normal_pool_control_timeout");
    assert!(began.elapsed() >= Duration::from_millis(1500));
    assert!(began.elapsed() < Duration::from_secs(5));

    let mut socket = connect();
    socket.write_all(&packet[..17]).unwrap();
    socket.shutdown(std::net::Shutdown::Write).unwrap();
    assert_eq!(reply(&mut socket)["error"], "normal_pool_control_eof");
    assert_eq!(normal_pool::status(&root).unwrap().grants, 0);
    for (field, value, expected) in [
        (
            "source",
            serde_json::json!("0".repeat(64)),
            "normal_pool_request_source",
        ),
        ("boot", serde_json::json!(0), "normal_pool_request_boot"),
    ] {
        let mut bad = request.clone();
        bad[field] = value;
        let mut socket = connect();
        serde_json::to_writer(&mut socket, &bad).unwrap();
        socket.write_all(b"\n").unwrap();
        assert_eq!(reply(&mut socket)["error"], expected);
    }
    let s = normal_pool::status(&root).unwrap();
    assert_eq!(s.grants, 0);
    assert_eq!(s.returns, 0);
    assert_eq!(s.available_touched_bytes, BYTES);
    assert_eq!(s.rejected_requests, 5);
    pool.finish().unwrap();
    let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
    let reasons: Vec<_> = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|e| e["kind"] == "rejected")
        .map(|e| e["rejection_reason"].clone())
        .collect();
    assert_eq!(
        reasons,
        serde_json::json!([
            "normal_pool_stale_or_wrong_grant",
            "normal_pool_control_timeout",
            "normal_pool_control_eof",
            "normal_pool_request_source",
            "normal_pool_request_boot"
        ])
        .as_array()
        .unwrap()
        .clone()
    );
    assert!(!events.contains(&fake_key));
}

#[cfg(target_os = "macos")]
#[test]
fn mapped_pool_unmaps_before_grant_and_rebuilds_only_after_owned_wait() {
    use mycity_headless::native_observer::{capture, Identity};
    fn top(root: &Path) -> Vec<serde_json::Value> {
        let mut paths: Vec<_> = fs::read_dir(root)
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("vm-regions-")
                    && p.extension().is_some_and(|s| s == "jsonl")
            })
            .collect();
        paths.sort();
        fs::read_to_string(paths.last().unwrap())
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
    fn vm(pid: u32) -> Vec<u8> {
        let out = Command::new("/usr/bin/vmmap")
            .args(["-wide", "-pages", "-interleaved", &pid.to_string()])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(out.stdout.len() < 4 * 1024 * 1024);
        out.stdout
    }
    fn intervals(raw: &[u8]) -> Vec<(usize, usize)> {
        String::from_utf8_lossy(raw)
            .lines()
            .filter_map(|l| {
                l.split_whitespace().find_map(|w| {
                    let (a, b) = w.split_once('-')?;
                    if a.len() < 5 || b.len() < 5 {
                        return None;
                    }
                    Some((
                        usize::from_str_radix(a, 16).ok()?,
                        usize::from_str_radix(b, 16).ok()?,
                    ))
                })
            })
            .collect()
    }
    let t = Temp::new();
    let proof = std::env::var_os("MYCITY_POOL_PROOF_DIR").map(PathBuf::from);
    if let Some(path) = &proof {
        fs::create_dir(path).unwrap();
    }
    let base = proof
        .as_ref()
        .map_or_else(|| t.0.clone(), |p| p.join("runtime"));
    if proof.is_some() {
        fs::create_dir(&base).unwrap();
    }
    let root = base.join("mapped-pool");
    let mut pool =
        Process::spawn_vm_diagnostic(exe(), &root, Some(&t.0.join("heaps.jsonl"))).unwrap();
    let before_list = top(&root);
    let identity: Identity = serde_json::from_value(before_list[0]["owner"].clone()).unwrap();
    let before = capture(identity.pid, Some(identity), Instant::now()).unwrap();
    let before_vm = vm(identity.pid);
    let before_pages: Vec<_> = before_list
        .iter()
        .filter(|v| v["kind"] == "normal_pool_mapped_page")
        .collect();
    assert_eq!(before_pages.len(), 64);
    if let Some(p) = &proof {
        fs::write(p.join("before.vmmap"), &before_vm).unwrap();
        fs::write(
            p.join("before-addresses.json"),
            serde_json::to_vec_pretty(&before_list).unwrap(),
        )
        .unwrap();
    }
    let tasks = base.join("tasks-99");
    fs::create_dir(&tasks).unwrap();
    fs::create_dir(tasks.join("inputs")).unwrap();
    fs::create_dir(tasks.join("candidates")).unwrap();
    let mut worker = Worker::new();
    worker.start_bounded(exe(), &tasks, Some(&root), false).unwrap();
    let during = capture(identity.pid, Some(identity), Instant::now()).unwrap();
    let during_vm = vm(identity.pid);
    let during_list = top(&root);
    if let Some(p) = &proof {
        fs::write(p.join("loan.vmmap"), &during_vm).unwrap();
        fs::write(
            p.join("loan-addresses.json"),
            serde_json::to_vec_pretty(&during_list).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(during_list.len() - 2, 64 - GRANT_BYTES / (1024 * 1024));
    let released: Vec<_> = before_pages
        .iter()
        .filter(|v| v["ordinal"].as_u64().unwrap() as usize >= 64 - GRANT_BYTES / (1024 * 1024))
        .collect();
    let spans = intervals(&during_vm);
    for r in &released {
        let start = r["start"].as_u64().unwrap() as usize;
        let end = start + r["capacity"].as_u64().unwrap() as usize;
        assert!(
            !spans.iter().any(|(a, b)| *a < end && *b > start),
            "released donor mapping still present"
        );
    }
    assert!(
        before.usage.footprint_bytes
            >= during.usage.footprint_bytes + (GRANT_BYTES - 1024 * 1024) as u64
    );
    assert!(worker
        .stop_when_idle(Duration::from_secs(2))
        .unwrap()
        .success());
    let after = capture(identity.pid, Some(identity), Instant::now()).unwrap();
    let after_vm = vm(identity.pid);
    let after_list = top(&root);
    if let Some(p) = &proof {
        fs::write(p.join("returned.vmmap"), &after_vm).unwrap();
        fs::write(
            p.join("returned-addresses.json"),
            serde_json::to_vec_pretty(&after_list).unwrap(),
        )
        .unwrap();
    }
    let after_pages: Vec<_> = after_list
        .iter()
        .filter(|v| v["kind"] == "normal_pool_mapped_page")
        .collect();
    assert_eq!(after_pages.len(), 64);
    let spans = intervals(&after_vm);
    for r in &after_pages {
        let start = r["start"].as_u64().unwrap() as usize;
        let end = start + r["capacity"].as_u64().unwrap() as usize;
        assert!(spans.iter().any(|(a, b)| *a <= start && *b >= end));
        if r["ordinal"].as_u64().unwrap() as usize >= 64 - GRANT_BYTES / (1024 * 1024) {
            assert!(r["allocation_generation"].as_u64().unwrap() > 64);
        }
    }
    assert!(
        after.usage.footprint_bytes
            >= during.usage.footprint_bytes + (GRANT_BYTES - 1024 * 1024) as u64
    );
    assert_eq!(
        normal_pool::status(&root).unwrap().available_touched_bytes,
        BYTES
    );
    pool.finish().unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["mapped_backing"]["initialized_mapped_bytes"], BYTES);
    assert_eq!(
        report["mapped_backing"]["peak_initialized_mapped_bytes"],
        BYTES
    );
    assert_eq!(report["rust_metadata_cap_bytes"], 256 * 1024);
    eprintln!(
        "{}",
        serde_json::json!({"source_sha256":env!("MYCITY_SOURCE_SHA256"),"pool_identity":identity,"before":before,"during":during,"after":after,"released_original_ranges":released,"all_released_addresses_unmapped":true,"returned_map_generations_new":true,"vm_raw_sha256":[sha256(&before_vm),sha256(&during_vm),sha256(&after_vm)],"mapped_backing":report["mapped_backing"],"scope":"actual donor mapping/footprint/owned-wait mechanism only; not joint normal or real16GB acceptance"})
    );
}
