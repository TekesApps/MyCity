use mycity_core::{
    air::{Input, Source, State, STRATEGY},
    sha256,
};
use mycity_headless::compute_worker::{
    self as protocol, Event, Request, Stage, Ticket, Worker, PROTOCOL,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
fn request(root: &Path, state: &State, job: u64, generation: u32) -> Result<Request, String> {
    let hash = protocol::publish(&root.join("inputs"), &state.checkpoint()?)?;
    Ok(Request {
        protocol: PROTOCOL.into(),
        strategy: STRATEGY.into(),
        controlled: None,
        ticket: Ticket {
            world: 1,
            branch: 1,
            job,
            incarnation: 1,
            generation,
            attempt: generation as u8,
            base_revision: job - 1,
            field_version: job - 1,
            start: state.report().time,
            end: state.input().target,
            rules_sha256: sha256(b"finite-worker-probe-rules-v1"),
            source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
            input_sha256: state.input().identity(),
            checkpoint_sha256: hash,
        },
    })
}
fn task(target: u32, dense: bool) -> Input {
    Input {
        schema: STRATEGY.into(),
        side: 128,
        target,
        initial_micro: 2_000_000,
        boundary_micro: [Some(2_000_000); 4],
        boundary_until: target,
        sources: if dense {
            (0..20000)
                .map(|i| Source {
                    id: i + 1,
                    cell: i % 16384,
                    start: 0,
                    end: target,
                    rate_micro: 100,
                })
                .collect()
        } else {
            vec![Source {
                id: 1,
                cell: 8256,
                start: 0,
                end: target,
                rate_micro: 10000,
            }]
            .into()
        },
    }
}
fn until(worker: &mut Worker, wanted: Stage, events: &mut Vec<Event>) -> Result<Event, String> {
    for _ in 0..4 {
        let e = worker.next(Duration::from_secs(30))?;
        let done = e.stage == wanted;
        events.push(e.clone());
        if done {
            return Ok(e);
        }
        worker.acknowledge_probe(&e)?;
    }
    Err("worker_probe_missing_stage".into())
}
fn root(dir: &Path) -> Result<(), String> {
    fs::create_dir(dir)
        .and_then(|_| fs::create_dir(dir.join("inputs")))
        .and_then(|_| fs::create_dir(dir.join("candidates")))
        .map_err(|e| e.to_string())
}
fn candidate(dir: &Path, request: &Request, base: &State, event: &Event) -> Result<State, String> {
    let bytes = protocol::read_asset(
        &dir.join("candidates"),
        event
            .candidate_sha256
            .as_deref()
            .ok_or("worker_probe_no_candidate")?,
    )?;
    protocol::verify_candidate(request, base, &bytes)
}
pub fn run(dir: &Path) -> Result<Value, String> {
    protocol::with_worker(|worker| run_owned(dir, worker))
}
fn run_owned(dir: &Path, worker: &mut Worker) -> Result<Value, String> {
    fs::create_dir(dir).map_err(|e| e.to_string())?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let started = Instant::now();
    let persistent = dir.join("persistent");
    root(&persistent)?;
    let base = State::new(task(60, false))?;
    worker.start_unbounded(&executable, &persistent, false)?;
    let pid = worker.pid();
    let first = request(&persistent, &base, 1, 1)?;
    worker.submit(&first)?;
    if worker.submit(&first).is_ok() {
        return Err("worker_probe_unbounded_inflight".into());
    }
    let mut events = Vec::new();
    let e = until(worker, Stage::Complete, &mut events)?;
    let result = candidate(&persistent, &first, &base, &e)?;
    let mut next_input = task(180, false);
    next_input.clear_sources()?;
    let next = result.continue_from_boundary(next_input)?;
    let second = request(&persistent, &next, 2, 1)?;
    worker.submit(&second)?;
    let e = until(worker, Stage::Complete, &mut events)?;
    let final_state = candidate(&persistent, &second, &next, &e)?;
    if worker.pid() != pid
        || final_state.report().time != 180
        || final_state.report().emitted_micro != 600000
    {
        return Err("worker_probe_continuity".into());
    }
    worker.retire(Duration::ZERO)?;
    fs::write(
        persistent.join("events.json"),
        serde_json::to_vec_pretty(&events).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut faults = Vec::new();
    for (name, stage, target, dense) in [
        ("source_kill", Stage::Sources, 86400, true),
        ("tile_kill", Stage::Cells, 86400, true),
        ("published_kill", Stage::Published, 60, false),
    ] {
        let case = dir.join(name);
        root(&case)?;
        let base = State::new(task(target, dense))?;
        let first = request(&case, &base, 1, 1)?;
        worker.start_unbounded(&executable, &case, true)?;
        let killed_pid = worker.pid();
        worker.submit(&first)?;
        let mut events = Vec::new();
        let last = until(worker, stage, &mut events)?;
        if stage != Stage::Published
            && fs::read_dir(case.join("candidates"))
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
        {
            return Err("worker_probe_kill_too_late".into());
        }
        if matches!(stage, Stage::Sources | Stage::Cells)
            && !last.cursor.is_some_and(|c| {
                c > 0
                    && c < if stage == Stage::Sources {
                        20000
                    } else {
                        16384
                    }
            })
        {
            return Err("worker_probe_not_partial_page".into());
        }
        // Actual SIGKILL via Child::kill, not a synthetic error return.
        let killed_status = worker.stop()?;
        if killed_status.code().is_some() {
            return Err("worker_probe_not_killed".into());
        }
        worker.retire(Duration::ZERO)?;
        let retry = request(&case, &base, 1, 2)?;
        worker.start_unbounded(&executable, &case, false)?;
        let recovered_pid = worker.pid();
        worker.submit(&retry)?;
        let e = until(worker, Stage::Complete, &mut events)?;
        let restored = candidate(&case, &retry, &base, &e)?;
        if stage == Stage::Published && last.candidate_sha256 != e.candidate_sha256 {
            return Err("worker_probe_publish_not_idempotent".into());
        }
        worker.retire(Duration::ZERO)?;
        fs::write(
            case.join("events.json"),
            serde_json::to_vec_pretty(&events).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        faults.push(json!({"case":name,"killed_pid":killed_pid,"termination":"signal","partial_cursor":last.cursor,"recovered_pid":recovered_pid,"recomputed_exact_cpu_state":true,"report":restored.report(),"candidate_sha256":e.candidate_sha256}));
    }
    Ok(
        json!({"schema":"mycity-worker-probe-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"protocol":PROTOCOL,
        "persistent_worker_pid":pid,"two_jobs_same_process":true,"fault_stage_barrier":true,"continuous_report":final_state.report(),"faults":faults,"wall_seconds":started.elapsed().as_secs_f64(),
        "max_inflight":1,"reader_queue_frames":4,"control_bytes":protocol::CONTROL_BYTES,"asset_bytes":mycity_core::air::CHECKPOINT_BYTES,
        "world_store_integration":"not_exercised_in_this_probe","sim_process":"not_implemented","group_resource_measurement":"not_run","memory_16gb_gate":"not_run"}),
    )
}
