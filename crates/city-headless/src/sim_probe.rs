//! Reproducible W3 process laboratory. No wall-clock endurance/performance claim.
use crate::{
    sim_client::Client,
    sim_host::{Command, Event, ProbePoint},
};
use mycity_core::{
    background::Command as B,
    exchange::Command as X,
    schedule::Phase,
    sha256,
    timeline::Command as T,
    transaction::{Action, Envelope},
};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
struct Run<'a> {
    client: &'a mut Client,
    id: u64,
    log: File,
    lines: usize,
}
impl Run<'_> {
    fn record(&mut self, value: Value) -> Result<(), String> {
        self.lines += 1;
        if self.lines > 512 {
            return Err("sim_probe_transcript_limit".into());
        }
        let bytes = serde_json::to_vec(&value).map_err(err)?;
        if bytes.len() > 64 * 1024 {
            return Err("sim_probe_frame_limit".into());
        }
        self.log
            .write_all(&bytes)
            .and_then(|_| self.log.write_all(b"\n"))
            .map_err(err)
    }
    fn next(&mut self) -> Result<Event, String> {
        let event = self.client.next(Duration::from_secs(10))?;
        self.record(json!({"event":event}))?;
        Ok(event)
    }
    fn start(&mut self, command: Command) -> Result<(), String> {
        self.id += 1;
        self.record(json!({"id":self.id,"command":command}))?;
        self.client.submit(self.id, command)
    }
    fn finish(&mut self) -> Result<Value, String> {
        for _ in 0..24 {
            let e = self.next()?;
            if e.kind == "result" {
                return e
                    .payload
                    .get("Ok")
                    .cloned()
                    .ok_or_else(|| format!("sim_probe_result:{:?}", e.payload));
            }
        }
        Err("sim_probe_result_limit".into())
    }
    fn call(&mut self, c: Command) -> Result<Value, String> {
        self.start(c)?;
        self.finish()
    }
    fn action(&mut self, a: Action) -> Result<(), String> {
        let state = self.call(Command::Status)?;
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: state["sequence"].as_u64().ok_or("sequence")? + 1,
            expected_revision: state["revision"].as_u64().ok_or("revision")?,
            actions: vec![a],
        };
        let out = self.call(Command::Execute { envelope })?;
        if out["model"].get("Ok").is_none() {
            return Err(format!("sim_probe_model:{out}"));
        }
        Ok(())
    }
    fn prepare(&mut self) -> Result<(), String> {
        for quote in [1, 2] {
            self.action(Action::Exchange {
                command: X::Dispatch { quote },
            })?;
        }
        self.action(Action::Timeline { command: T::Enable })?;
        self.action(Action::Background { command: B::Enable })?;
        for _ in 0..30 {
            self.action(Action::Timeline {
                command: T::Advance {
                    target: 30,
                    limit: 256,
                },
            })?;
            let state = self.call(Command::Status)?;
            if serde_json::from_value::<Option<Phase>>(state["timeline"]["phase"].clone())
                .map_err(err)?
                == Some(Phase::P4)
            {
                return Ok(());
            }
        }
        Err("sim_probe_no_barrier".into())
    }
    fn barrier(&mut self) -> Result<Value, String> {
        for _ in 0..8 {
            let e = self.next()?;
            if e.kind == "probe_barrier" {
                return Ok(e.payload);
            }
            if e.kind == "result" {
                return Err("sim_probe_barrier_missing".into());
            }
        }
        Err("sim_probe_progress_limit".into())
    }
}
pub fn run(dir: &Path) -> Result<Value, String> {
    crate::sim_client::with_client_scope(|client| run_inner(dir, client))
}
fn run_inner(dir: &Path, client: &mut Client) -> Result<Value, String> {
    fs::create_dir(dir).map_err(err)?;
    let dir = dir.canonicalize().map_err(err)?;
    let exe = std::env::current_exe().map_err(err)?;
    let start = Instant::now();
    let cases = [
        ("baseline", None, false),
        ("worker_sources", Some(ProbePoint::WorkerSources), false),
        ("worker_cells", Some(ProbePoint::WorkerCells), false),
        ("sim_worker_cells", Some(ProbePoint::WorkerCells), true),
        ("sim_leased", Some(ProbePoint::Leased), true),
        ("sim_asset", Some(ProbePoint::CandidatePublished), true),
        ("sim_committed_reply", Some(ProbePoint::Committed), true),
        ("cancel_retry", Some(ProbePoint::WorkerCells), false),
    ];
    let mut results = Vec::new();
    let mut reference = None;
    for (name, point, kill_sim) in cases {
        let case = dir.join(name);
        fs::create_dir(&case).map_err(err)?;
        let world = case.join("world");
        client.start_probe(&exe, &world, true)?;
        let mut run = Run {
            client: &mut *client,
            id: 0,
            log: File::create(case.join("transcript.jsonl")).map_err(err)?,
            lines: 0,
        };
        let ready = run.next()?;
        if ready.kind != "ready" {
            return Err("sim_probe_not_ready".into());
        }
        let sim_pid = run.client.pid();
        run.prepare()?;
        let mut fault = None;
        let receipt = if let Some(point) = point {
            run.start(Command::ProbeRequired { job: 1, point })?;
            let at = run.barrier()?;
            let worker_pid = run.client.worker_pid();
            fault =
                Some(json!({"point":point,"sim_pid":sim_pid,"worker_pid":worker_pid,"barrier":at}));
            if kill_sim {
                // No helper cleanup of the worker: replacement reads durable provenance itself.
                if !std::process::Command::new("kill")
                    .args(["-KILL", &sim_pid.to_string()])
                    .status()
                    .map_err(err)?
                    .success()
                {
                    return Err("sim_probe_kill".into());
                }
                run.client.wait_for_sim_exit(Duration::from_secs(2))?;
                run.client.stop()?;
                run.client.start(&exe, &world, false)?;
                if run.next()?.kind != "ready" {
                    return Err("sim_probe_reopen_not_ready".into());
                }
                run.call(Command::RunRequired { job: 1 })?
            } else if name == "cancel_retry" {
                run.client.acknowledge_probe(point, true)?;
                loop {
                    let event = run.next()?;
                    if event.kind == "result" {
                        if event.payload.get("Err").is_none() {
                            return Err("sim_probe_cancel_succeeded".into());
                        }
                        break;
                    }
                }
                let state = run.call(Command::Status)?;
                if state["background"]["pending"]["failure"] != "cancelled"
                    || state["background"]["time"] != 0
                {
                    return Err("sim_probe_cancel_erased_obligation".into());
                }
                run.call(Command::RunRequired { job: 1 })?
            } else {
                run.client.kill_owned_worker()?;
                run.client.acknowledge_probe(point, false)?;
                run.finish()?
            }
        } else {
            run.call(Command::RunRequired { job: 1 })?
        };
        let state = run.call(Command::Status)?;
        if state["background"]["time"] != 30 || state["background"]["version"] != 1 {
            return Err("sim_probe_field_not_accepted".into());
        }
        if receipt["replayed"] != (point == Some(ProbePoint::Committed)) {
            return Err("sim_probe_receipt_replay".into());
        }
        run.call(Command::Checkpoint)?;
        let again = run.call(Command::RunRequired { job: 1 })?;
        if again["record"] != receipt["record"] || again["replayed"] != true {
            return Err("sim_probe_cold_receipt_changed".into());
        }
        run.call(Command::Stop)?;
        run.client.stop()?;
        run.log.sync_all().map_err(err)?;
        // Read only after sole writer has exited. Normalize revision, not business state.
        let store = mycity_storage::Store::open(
            &world,
            &format!("sim-w3:{}", env!("MYCITY_SOURCE_SHA256")),
        )
        .map_err(err)?;
        let mut business =
            serde_json::to_value(store.engine().map_err(err)?.world().view()).map_err(err)?;
        business["revision"] = json!(0);
        let business_sha = sha256(&serde_json::to_vec(&business).map_err(err)?);
        let field_sha = sha256(
            &store
                .engine()
                .map_err(err)?
                .background()
                .and_then(|b| b.field())
                .ok_or("sim_probe_field")?
                .checkpoint()?,
        );
        let comparison = (business_sha.clone(), field_sha.clone());
        if let Some(expected) = &reference {
            if expected != &comparison {
                return Err("sim_probe_fault_changed_business_or_field".into());
            }
        } else {
            reference = Some(comparison);
        }
        results.push(json!({"case":name,"fault":fault,"status":state,"receipt":receipt,"business_without_revision_sha256":business_sha,"complete_air_state_sha256":field_sha,"transcript_frames":run.lines,"persistent_reopen_verified":true}));
    }
    let report = json!({"schema":"mycity-sim-probe-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"cases":results,"wall_seconds":start.elapsed().as_secs_f64(),"numerical_comparison":"same_cpu_full_state_exact_not_independent_algorithm","whole_process_group_measurement":"not_run","endurance_1800_seconds":"not_run","memory_16gb_gate":"not_run"});
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(err)?,
    )
    .map_err(err)?;
    Ok(report)
}
