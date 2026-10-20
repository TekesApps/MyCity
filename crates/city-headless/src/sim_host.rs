//! Independent sole-writer Sim. IPC carries bounded commands/results, never world files.
use crate::{
    compute_worker::{self as wire, Request as Work, Stage, Ticket, Worker},
    worker_cache,
};
use mycity_core::{
    background::{self, Command as B, Failure},
    sha256,
    transaction::{Action, Caller, Envelope, Resource},
};
use mycity_storage::Store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    time::Duration,
};
const PROTOCOL: &str = "mycity-sim-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub id: u64,
    pub command: Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Status,
    Execute { envelope: Envelope },
    RunRequired { job: u64 },
    ProbeRequired { job: u64, point: ProbePoint },
    ProbeAck { point: ProbePoint, cancel: bool },
    Checkpoint,
    Stop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbePoint {
    Leased,
    WorkerSources,
    WorkerCells,
    CandidatePublished,
    Committed,
}
fn gate(
    point: ProbePoint,
    pending: &mut Option<ProbePoint>,
    r: &mut impl BufRead,
    w: &mut impl Write,
    id: u64,
    payload: Value,
) -> Result<bool, String> {
    if *pending != Some(point) {
        return Ok(false);
    }
    *pending = None;
    event(
        w,
        id,
        "probe_barrier",
        json!({"point":point,"details":payload}),
    )?;
    let bytes = wire::frame(r)?.ok_or("sim_probe_ack_eof")?;
    let ack: Request = serde_json::from_slice(&bytes).map_err(err)?;
    if ack.protocol != PROTOCOL || ack.id != id {
        return Err("sim_probe_ack_identity".into());
    }
    match ack.command {
        Command::ProbeAck { point: p, cancel } if p == point => Ok(cancel),
        _ => Err("sim_probe_ack_point".into()),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub protocol: String,
    pub id: u64,
    pub sim_pid: u32,
    pub kind: String,
    pub payload: Value,
}
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn event(w: &mut impl Write, id: u64, kind: &str, payload: Value) -> Result<(), String> {
    wire::write_frame(
        w,
        &Event {
            protocol: PROTOCOL.into(),
            id,
            sim_pid: std::process::id(),
            kind: kind.into(),
            payload,
        },
    )
}
fn rules() -> String {
    format!("sim-w3:{}", env!("MYCITY_SOURCE_SHA256"))
}
fn status(store: &Store, worker: &Option<Worker>) -> Result<Value, String> {
    let e = store.engine().map_err(err)?;
    let b = e.background();
    Ok(
        json!({"revision":e.world().view().revision,"sequence":store.committed_sequence(),"timeline":e.timeline().map(|t|t.progress),
 "background":b.map(|b|json!({"version":b.version(),"time":b.time(),"incarnation":b.incarnation(),"pending":b.pending().map(|j|json!({"id":j.id,"generation":j.generation,"attempt":j.attempt,"running":j.running,"failure":j.failure,"start":j.start,"end":j.end})),"field":b.field().map(|f|f.report())})),
 "state_sha256":store.state_hash().map_err(err)?,"capacity":store.capacity_status().map_err(err)?,"worker_pid":worker.as_ref().map(|w|w.pid()),"source_sha256":env!("MYCITY_SOURCE_SHA256"),"memory_16gb_gate":"not_run"}),
    )
}
fn control(store: &mut Store, command: B) -> Result<(), String> {
    let e = store.engine().map_err(err)?;
    let view = e.world().view();
    let envelope = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: 0xd000_0000_0000_0000
            | store
                .committed_sequence()
                .checked_add(1)
                .filter(|n| *n <= background::JOB_ID_LIMIT)
                .ok_or("sim_control_id")?,
        expected_revision: view.revision,
        actions: vec![Action::Background { command }],
    };
    store
        .execute(Caller::System, envelope)
        .map_err(err)?
        .model
        .map_err(err)?;
    Ok(())
}
fn required(
    store: &mut Store,
    worker: &mut Option<Worker>,
    id: u64,
    job: u64,
    w: &mut impl Write,
    reader: &mut impl BufRead,
    mut probe: Option<ProbePoint>,
) -> Result<Value, String> {
    let operation = background::accept_operation_id(job)?;
    // Return the ORIGINAL persisted record; never rebuild an Envelope after lost ACK.
    if let Some(record) = store.receipt(operation).map_err(err)? {
        return Ok(json!({"replayed":true,"record":record}));
    }
    loop {
        let pending = store
            .engine()
            .map_err(err)?
            .background()
            .and_then(|b| b.pending())
            .ok_or("sim_no_pending")?;
        if pending.id != job
            || pending.attempt >= 3
            || pending.failure == Some(Failure::InvalidInput)
        {
            return Err("sim_retry_exhausted_or_wrong_job".into());
        }
        // Old process is reaped before clearing any file or binding a new execution epoch.
        if let Some(mut old) = worker.take() {
            old.stop()?;
        }
        let cache = store.prepare_worker_cache().map_err(err)?;
        let exe = std::env::current_exe().map_err(err)?;
        let cache_fence = worker_cache::recover_exclusive(&cache, &exe)?;
        control(store, B::RebindLease { job })?;
        worker_cache::reset_after_worker_stopped(&cache)?;
        let b = store
            .engine()
            .map_err(err)?
            .background()
            .ok_or("sim_no_background")?;
        let pending = b.pending().cloned().ok_or("sim_no_pending")?;
        let input = b.frozen_input()?;
        let checkpoint = worker_cache::publish(&cache, "inputs", &input.checkpoint()?)?;
        let view = store.engine().map_err(err)?.world().view();
        let request = Work {
            protocol: wire::PROTOCOL.into(),
            strategy: mycity_core::air::STRATEGY.into(),
            ticket: Ticket {
                world: view.scope.world(),
                branch: view.scope.branch(),
                job,
                incarnation: pending.incarnation,
                generation: pending.generation,
                attempt: pending.attempt,
                base_revision: pending.base_revision,
                field_version: pending.field_version,
                start: pending.start,
                end: pending.end,
                rules_sha256: sha256(rules().as_bytes()),
                source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
                input_sha256: pending.input.identity(),
                checkpoint_sha256: checkpoint,
            },
        };
        if gate(
            ProbePoint::Leased,
            &mut probe,
            reader,
            w,
            id,
            json!({"job":job,"attempt":pending.attempt}),
        )? {
            control(
                store,
                B::Fail {
                    job,
                    generation: pending.generation,
                    failure: Failure::Cancelled,
                },
            )?;
            return Err("sim_attempt_cancelled".into());
        }
        let gated = probe.is_some();
        let mut cancelled = false;
        let attempt = (|| -> Result<Vec<u8>, String> {
            let exe = std::env::current_exe().map_err(err)?;
            *worker = Some(if gated {
                Worker::spawn_bounded_probe(&exe, &cache)?
            } else {
                Worker::spawn_bounded(&exe, &cache)?
            });
            let child = worker.as_mut().ok_or("sim_worker_missing")?;
            worker_cache::register(&cache, child.pid(), &exe)?;
            drop(cache_fence);
            event(
                w,
                id,
                "worker_started",
                json!({"worker_pid":child.pid(),"ticket":request.ticket}),
            )?;
            child.submit(&request)?;
            for _ in 0..4 {
                let progress = child.next(Duration::from_secs(30))?;
                event(
                    w,
                    id,
                    "worker_progress",
                    serde_json::to_value(&progress).map_err(err)?,
                )?;
                let point = match progress.stage {
                    Stage::Sources => Some(ProbePoint::WorkerSources),
                    Stage::Cells => Some(ProbePoint::WorkerCells),
                    _ => None,
                };
                if let Some(point) = point {
                    if gate(
                        point,
                        &mut probe,
                        reader,
                        w,
                        id,
                        json!({"worker_pid":child.pid(),"cursor":progress.cursor}),
                    )? {
                        cancelled = true;
                        return Err("sim_attempt_cancelled".into());
                    }
                }
                if progress.stage != Stage::Complete {
                    child.acknowledge_probe(&progress)?;
                }
                if progress.stage == Stage::Complete {
                    return wire::read_asset(
                        &cache.join("candidates"),
                        progress
                            .candidate_sha256
                            .as_deref()
                            .ok_or("sim_candidate_hash")?,
                    );
                }
            }
            Err("sim_worker_incomplete".into())
        })();
        let bytes = match attempt {
            Ok(b) => b,
            Err(error) => {
                if let Some(mut old) = worker.take() {
                    old.stop()?;
                }
                control(
                    store,
                    B::Fail {
                        job,
                        generation: pending.generation,
                        failure: if cancelled {
                            Failure::Cancelled
                        } else {
                            Failure::Transient
                        },
                    },
                )?;
                event(
                    w,
                    id,
                    "attempt_failed",
                    json!({"attempt":pending.attempt,"reason":error}),
                )?;
                if cancelled {
                    return Err("sim_attempt_cancelled".into());
                }
                continue;
            }
        };
        // The pure worker's files are copied/validated into Sim-owned immutable assets.
        let hash = match store.import_background_candidate_with_observer(
            job,
            pending.generation,
            &bytes,
            |point| {
                if point == mycity_storage::BackgroundPoint::AssetPublished {
                    let cancelled = gate(
                        ProbePoint::CandidatePublished,
                        &mut probe,
                        reader,
                        w,
                        id,
                        json!({"job":job}),
                    )
                    .map_err(std::io::Error::other)?;
                    if cancelled {
                        return Err(std::io::Error::other(
                            "sim_probe_cancel_at_import_unsupported",
                        ));
                    }
                }
                Ok(())
            },
        ) {
            Ok(h) => h,
            Err(error) => {
                // Validation failure remains a required obligation. IO uncertainty must reopen.
                if matches!(
                    error,
                    mycity_storage::Error::Invalid(_) | mycity_storage::Error::Model(_)
                ) {
                    control(
                        store,
                        B::Fail {
                            job,
                            generation: pending.generation,
                            failure: Failure::InvalidInput,
                        },
                    )?;
                }
                return Err(err(error));
            }
        };
        let view = store.engine().map_err(err)?.world().view();
        let envelope = Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: operation,
            expected_revision: view.revision,
            actions: vec![Action::Background {
                command: B::Accept {
                    job,
                    generation: pending.generation,
                    candidate_sha256: hash,
                },
            }],
        };
        let outcome = store.execute(Caller::System, envelope).map_err(err)?;
        outcome.model.map_err(err)?;
        if !outcome.has_persistent_receipt {
            return Err("sim_admission_not_durable".into());
        }
        if gate(
            ProbePoint::Committed,
            &mut probe,
            reader,
            w,
            id,
            json!({"job":job,"operation_id":operation}),
        )? {
            return Err("sim_probe_cancel_after_commit_unsupported".into());
        }
        return Ok(
            json!({"replayed":false,"record":store.receipt(operation).map_err(err)?.ok_or("sim_receipt_missing")?}),
        );
    }
}
pub fn run(world: &Path, create: bool) -> Result<(), String> {
    run_inner(world, create, false)
}
pub fn run_probe(world: &Path, create: bool) -> Result<(), String> {
    run_inner(world, create, true)
}
fn run_inner(world: &Path, create: bool, probe_enabled: bool) -> Result<(), String> {
    let rules = rules();
    let mut store = if create {
        Store::create_spatial(
            world,
            include_bytes!("../../../fixtures/m0-exchange-opening.json"),
            [
                (101, 1),
                (102, 1),
                (103, 1),
                (104, 1),
                (201, 2000),
                (202, 2000),
            ]
            .into_iter()
            .map(|(id, c)| Resource::new(id, c).map_err(err))
            .collect::<Result<Vec<_>, _>>()?,
            &rules,
            include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
            include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
        )
        .map_err(err)?
    } else {
        Store::open(world, &rules).map_err(err)?
    };
    let mut worker = None;
    let stdin = std::io::stdin();
    let mut reader = BufReader::with_capacity(8192, stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    event(&mut writer, 0, "ready", status(&store, &worker)?)?;
    while let Some(bytes) = wire::frame(&mut reader)? {
        let request: Request = serde_json::from_slice(&bytes).map_err(err)?;
        if request.protocol != PROTOCOL || request.id == 0 {
            return Err("sim_request_identity".into());
        }
        let stop = matches!(request.command, Command::Stop);
        let outcome = match request.command {
            Command::Status => status(&store, &worker),
            Command::Execute { envelope } => {
                let prefix = envelope.operation_id & 0xf000_0000_0000_0000;
                if matches!(prefix, 0xd000_0000_0000_0000 | 0xe000_0000_0000_0000)
                    || envelope
                        .actions
                        .iter()
                        .any(|a| matches!(a,Action::Background{command} if *command!=B::Enable))
                {
                    Err("sim_internal_control_reserved".into())
                } else {
                    store
                        .execute(Caller::System, envelope)
                        .map_err(err)
                        .and_then(|v| serde_json::to_value(v).map_err(err))
                }
            }
            Command::RunRequired { job } => required(
                &mut store,
                &mut worker,
                request.id,
                job,
                &mut writer,
                &mut reader,
                None,
            ),
            Command::ProbeRequired { job, point } if probe_enabled => required(
                &mut store,
                &mut worker,
                request.id,
                job,
                &mut writer,
                &mut reader,
                Some(point),
            ),
            Command::ProbeRequired { .. } | Command::ProbeAck { .. } => {
                Err("sim_probe_command_disabled_or_unexpected".into())
            }
            Command::Checkpoint => store
                .checkpoint()
                .map_err(err)
                .map(|_| json!({"checkpointed":true})),
            Command::Stop => Ok(json!({"stopped":true})),
        };
        event(
            &mut writer,
            request.id,
            "result",
            serde_json::to_value(outcome).map_err(err)?,
        )?;
        if stop {
            break;
        }
    }
    if let Some(mut old) = worker.take() {
        old.stop()?;
    }
    Ok(())
}
