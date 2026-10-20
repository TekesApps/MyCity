//! Independent sole-writer Sim. IPC carries bounded commands/results, never world files.
use crate::{
    compute_worker::{self as wire, Request as Work, Stage, Ticket, Worker},
    worker_cache,
};
use mycity_core::{
    background::{self, Command as B, Failure},
    sha256,
    transaction::{Action, Caller, Envelope},
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
    Progress,
    Execute { envelope: Envelope },
    RunRequired { job: u64 },
    ProbeRequired { job: u64, point: ProbePoint },
    ProbeAck { point: ProbePoint, cancel: bool },
    Checkpoint,
    ReceiptProof { operation: u64 },
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
#[derive(Clone, Copy)]
pub(crate) enum Profile {
    Base,
    Joint,
    Scale(crate::scale_reference::Tier),
}
impl Profile {
    pub(crate) fn command(self, probe: bool) -> &'static str {
        match (self, probe) {
            (Self::Base, false) => "sim-host",
            (Self::Base, true) => "sim-host-probe",
            (Self::Joint, false) => "sim-host-joint",
            (Self::Joint, true) => "sim-host-joint-probe",
            (Self::Scale(crate::scale_reference::Tier::Full), false) => "sim-host-scale-full",
            (Self::Scale(crate::scale_reference::Tier::Full), true) => "sim-host-scale-full-probe",
            (Self::Scale(crate::scale_reference::Tier::Small), false) => "sim-host-scale-small",
            (Self::Scale(crate::scale_reference::Tier::Small), true) => {
                "sim-host-scale-small-probe"
            }
            (Self::Scale(crate::scale_reference::Tier::Medium), false) => "sim-host-scale-medium",
            (Self::Scale(crate::scale_reference::Tier::Medium), true) => {
                "sim-host-scale-medium-probe"
            }
        }
    }
    fn rules(self) -> String {
        let name = match self {
            Self::Base => "sim-w3",
            Self::Joint => "sim-w4",
            Self::Scale(crate::scale_reference::Tier::Full) => "sim-t13a-full-v2",
            Self::Scale(crate::scale_reference::Tier::Small) => "sim-t13a-small-v2",
            Self::Scale(crate::scale_reference::Tier::Medium) => "sim-t13a-medium-v2",
        };
        format!("{name}:{}", env!("MYCITY_SOURCE_SHA256"))
    }
}
fn progress(store: &Store) -> Result<Value, String> {
    let e = store.engine().map_err(err)?;
    Ok(
        json!({"revision":e.world().view().revision,"sequence":store.committed_sequence(),
        "timeline":e.timeline().map(|t|t.progress),
        "background":e.background().map(|b|json!({"version":b.version(),"time":b.time(),
            "pending":b.pending().map(|p|json!({"id":p.id,"generation":p.generation,"incarnation":p.incarnation,"attempt":p.attempt,"failure":p.failure}))}))}),
    )
}
fn status(
    store: &Store,
    worker: &Option<Worker>,
    diag: &Option<crate::cpu_log::CpuLog>,
) -> Result<Value, String> {
    let e = store.engine().map_err(err)?;
    let b = e.background();
    let view = e.world().view();
    let air_sha = b
        .and_then(|b| b.field())
        .map(|f| f.checkpoint().map(|b| sha256(&b)))
        .transpose()?;
    let runtime = json!({"accounts":view.accounts,"boot":view.boot,"active_city":view.region.as_ref().map(|r|r.active_city),"obligations":view.region.as_ref().map(|r|r.obligations),"payroll":e.timeline().map(|t|t.payroll.iter().take(8).collect::<Vec<_>>()),"claims":e.timeline().map(|t|t.claims.iter().take(8).collect::<Vec<_>>()),"claim_count":e.timeline().map(|t|t.claims.len()),"payroll_count":e.timeline().map(|t|t.payroll.len()),"resources":e.resources(),"store_owned_bytes":store.owned_capacity_bytes().map_err(err)?,"diagnostics_owned_bytes":diag.as_ref().map_or(0,crate::cpu_log::CpuLog::owned_bytes)});

    Ok(
        json!({"revision":view.revision,"sequence":store.committed_sequence(),"business_sha256":crate::joint_reference::business_digest(e)?,"runtime":runtime,"timeline":e.timeline().map(|t|t.progress),
 "background":b.map(|b|json!({"version":b.version(),"time":b.time(),"incarnation":b.incarnation(),"pending":b.pending().map(|j|json!({"id":j.id,"generation":j.generation,"attempt":j.attempt,"running":j.running,"failure":j.failure,"start":j.start,"end":j.end})),"field":b.field().map(|f|f.report()),"state_sha256":air_sha})),
 "state_sha256":store.state_hash().map_err(err)?,"capacity":store.capacity_status().map_err(err)?,"worker_pid":worker.as_ref().map(|w|w.pid()),"source_sha256":env!("MYCITY_SOURCE_SHA256"),"memory_16gb_gate":"not_run"}),
    )
}
fn control(
    store: &mut Store,
    command: B,
    diag: &mut Option<crate::cpu_log::CpuLog>,
) -> Result<(), String> {
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
    crate::cpu_log::execute(store, Caller::System, envelope, diag)?
        .model
        .map_err(err)?;
    Ok(())
}
struct Required<'a> {
    id: u64,
    job: u64,
    rules: &'a str,
    diag: &'a mut Option<crate::cpu_log::CpuLog>,
}
fn required(
    store: &mut Store,
    worker: &mut Option<Worker>,
    context: Required<'_>,
    w: &mut impl Write,
    reader: &mut impl BufRead,
    mut probe: Option<ProbePoint>,
) -> Result<Value, String> {
    let Required {
        id,
        job,
        rules,
        diag,
    } = context;
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
        control(store, B::RebindLease { job }, diag)?;
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
                rules_sha256: sha256(rules.as_bytes()),
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
                diag,
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
                    diag,
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
        let hash = match crate::cpu_log::import(
            store,
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
            diag,
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
                        diag,
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
        let outcome = crate::cpu_log::execute(store, Caller::System, envelope, diag)?;
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
    run_inner(world, create, false, Profile::Base)
}
pub fn run_probe(world: &Path, create: bool) -> Result<(), String> {
    run_inner(world, create, true, Profile::Base)
}
pub fn run_joint(world: &Path, create: bool, probe: bool) -> Result<(), String> {
    run_inner(world, create, probe, Profile::Joint)
}
pub fn run_scale_full(world: &Path, create: bool, probe: bool) -> Result<(), String> {
    run_scale(world, create, probe, crate::scale_reference::Tier::Full)
}
pub fn run_scale(
    world: &Path,
    create: bool,
    probe: bool,
    tier: crate::scale_reference::Tier,
) -> Result<(), String> {
    run_inner(world, create, probe, Profile::Scale(tier))
}
fn run_inner(
    world: &Path,
    create: bool,
    probe_enabled: bool,
    profile: Profile,
) -> Result<(), String> {
    let rules = profile.rules();
    let mut store = if create {
        let (opening, resources) = match profile {
            Profile::Scale(tier) => (tier.opening(), crate::scale_reference::resources()?),
            Profile::Joint => (
                crate::joint_reference::OPENING,
                crate::joint_reference::resources()?,
            ),
            Profile::Base => (
                include_bytes!("../../../fixtures/m0-exchange-opening.json").as_slice(),
                crate::joint_reference::resources()?,
            ),
        };
        let constructor = if matches!(profile, Profile::Scale(_)) {
            Store::create_spatial_v2
        } else {
            Store::create_spatial
        };
        constructor(
            world,
            opening,
            resources,
            &rules,
            include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
            include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
        )
        .map_err(err)?
    } else {
        Store::open(world, &rules).map_err(err)?
    };
    let mut diag = if crate::cpu_log::enabled() {
        Some(crate::cpu_log::CpuLog::create(
            &world
                .parent()
                .ok_or("diagnostic_world_parent")?
                .join("diagnostics"),
            "sim",
        )?)
    } else {
        None
    };
    let mut worker = None;
    let stdin = std::io::stdin();
    let mut reader = BufReader::with_capacity(8192, stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    event(&mut writer, 0, "ready", status(&store, &worker, &diag)?)?;
    while let Some(bytes) = wire::frame(&mut reader)? {
        let request: Request = serde_json::from_slice(&bytes).map_err(err)?;
        if request.protocol != PROTOCOL || request.id == 0 {
            return Err("sim_request_identity".into());
        }
        let stop = matches!(request.command, Command::Stop);
        let outcome = match request.command {
            Command::Status => status(&store, &worker, &diag),
            Command::Progress => progress(&store),
            Command::ReceiptProof { operation } => store.receipt(operation).map_err(err).and_then(|record| {
                let bytes = serde_json::to_vec(&record).map_err(err)?;
                Ok(json!({"operation":operation,"present":record.is_some(),"sha256":sha256(&bytes),"bytes":bytes.len()}))
            }),
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
                    crate::cpu_log::execute(&mut store,Caller::System,envelope,&mut diag)
                        .and_then(|v| serde_json::to_value(v).map_err(err))
                }
            }
            Command::RunRequired { job } => required(
                &mut store,
                &mut worker,
                Required {
                    id: request.id,
                    job,
                    rules: &rules,
                    diag:&mut diag,
                },
                &mut writer,
                &mut reader,
                None,
            ),
            Command::ProbeRequired { job, point } if probe_enabled => required(
                &mut store,
                &mut worker,
                Required {
                    id: request.id,
                    job,
                    rules: &rules,
                    diag:&mut diag,
                },
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
    if let Some(log) = diag.as_mut() {
        log.finish()?;
    }
    Ok(())
}
