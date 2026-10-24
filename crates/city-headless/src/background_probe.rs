//! W2 finite real worker -> Sim-owned Store demonstration, not a separate Sim supervisor.
use mycity_core::{
    background::Command as B,
    exchange::{Command as X, Status},
    schedule::Phase,
    sha256,
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource},
};
use mycity_headless::compute_worker::{self as protocol, Request, Stage, Ticket, Worker, PROTOCOL};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn envelope(s: &Store, a: Action) -> Result<Envelope, String> {
    let view = s.engine().map_err(err)?.world().view();
    Ok(Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: match a {
            Action::Background {
                command: B::Accept { job, .. },
            } => mycity_core::background::accept_operation_id(job).unwrap(),
            _ => s.committed_sequence() + 1,
        },
        expected_revision: view.revision,
        actions: vec![a],
    })
}
fn apply(s: &mut Store, a: Action) -> Result<Envelope, String> {
    let cmd = envelope(s, a)?;
    let out = s.execute(Caller::System, cmd.clone()).map_err(err)?;
    out.model.map_err(err)?;
    if !out.has_persistent_receipt {
        return Err("background_probe_receipt_missing".into());
    }
    Ok(cmd)
}
pub fn run(dir: &Path) -> Result<Value, String> {
    protocol::with_worker(|worker| run_owned(dir, worker))
}
fn run_owned(dir: &Path, worker: &mut Worker) -> Result<Value, String> {
    fs::create_dir(dir).map_err(err)?;
    let rules = format!("background-w2:{}", env!("MYCITY_SOURCE_SHA256"));
    let world = dir.join("world");
    let mut store = Store::create_spatial(
        &world,
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
        .map(|(id, c)| Resource::new(id, c))
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?,
        &rules,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .map_err(err)?;
    let initial = store.engine().map_err(err)?.world().view();
    let cash = initial.accounts.iter().map(|a| a.cash.value()).sum::<i64>();
    apply(
        &mut store,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    )?;
    apply(&mut store, Action::Timeline { command: T::Enable })?;
    apply(&mut store, Action::Background { command: B::Enable })?;
    apply(
        &mut store,
        Action::Background {
            command: B::Restart,
        },
    )?;
    let task_root = dir.join("tasks");
    fs::create_dir(&task_root).map_err(err)?;
    fs::create_dir(task_root.join("inputs")).map_err(err)?;
    fs::create_dir(task_root.join("candidates")).map_err(err)?;
    worker.start_unbounded(&std::env::current_exe().map_err(err)?, &task_root, false)?;
    let worker_pid = worker.pid();
    let started = Instant::now();
    let mut jobs = Vec::new();
    let mut last_receipt = None;
    for _ in 0..64 {
        apply(
            &mut store,
            Action::Timeline {
                command: T::Advance {
                    target: 120,
                    limit: 256,
                },
            },
        )?;
        let progress = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("background_probe_timeline")?
            .progress;
        if progress.target.is_none() {
            break;
        }
        if progress.phase != Some(Phase::P4) {
            continue;
        }
        let job = store
            .engine()
            .map_err(err)?
            .background()
            .and_then(|b| b.pending())
            .cloned()
            .ok_or("background_probe_job")?;
        apply(
            &mut store,
            Action::Background {
                command: B::Lease { job: job.id },
            },
        )?;
        let b = store
            .engine()
            .map_err(err)?
            .background()
            .ok_or("background_probe_state")?;
        let job = b.pending().cloned().ok_or("background_probe_job")?;
        let input = b.frozen_input()?;
        let input_bytes = input.checkpoint()?;
        let checkpoint_sha256 = protocol::publish(&task_root.join("inputs"), &input_bytes)?;
        let view = store.engine().map_err(err)?.world().view();
        let ticket = Ticket {
            world: view.scope.world(),
            branch: view.scope.branch(),
            job: job.id,
            incarnation: job.incarnation,
            generation: job.generation,
            attempt: job.attempt,
            base_revision: job.base_revision,
            field_version: job.field_version,
            start: job.start,
            end: job.end,
            rules_sha256: sha256(rules.as_bytes()),
            source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
            input_sha256: job.input.identity(),
            checkpoint_sha256,
        };
        let request = Request {
            protocol: PROTOCOL.into(),
            strategy: mycity_core::air::STRATEGY.into(),
            ticket,
        };
        worker.submit(&request)?;
        let mut completed = None;
        for _ in 0..4 {
            let event = worker.next(Duration::from_secs(30))?;
            if event.stage == Stage::Complete {
                completed = Some(event);
                break;
            }
        }
        let event = completed.ok_or("background_probe_incomplete")?;
        let output_hash = event
            .candidate_sha256
            .as_deref()
            .ok_or("background_probe_hash")?;
        let bytes = protocol::read_asset(&task_root.join("candidates"), output_hash)?;
        let before = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("background_probe_timeline")?
            .progress;
        let candidate_hash = store
            .import_background_candidate(job.id, job.generation, &bytes)
            .map_err(err)?;
        let cmd = apply(
            &mut store,
            Action::Background {
                command: B::Accept {
                    job: job.id,
                    generation: job.generation,
                    candidate_sha256: candidate_hash,
                },
            },
        )?;
        let result = store
            .engine()
            .map_err(err)?
            .background()
            .and_then(|b| b.field())
            .ok_or("background_probe_field")?
            .report();
        if before
            != store
                .engine()
                .map_err(err)?
                .timeline()
                .ok_or("background_probe_timeline")?
                .progress
        {
            return Err("background_probe_barrier_advanced_early".into());
        }
        jobs.push(json!({"ticket":request.ticket,"candidate_sha256":output_hash,"result":result,"accept":cmd}));
        last_receipt = Some(cmd);
    }
    worker.stop()?;
    let e = store.engine().map_err(err)?;
    let progress = e.timeline().ok_or("background_probe_timeline")?.progress;
    let final_world = e.world().view();
    let quote = final_world
        .exchange
        .ok_or("background_probe_exchange")?
        .quotes[0];
    if progress.last_complete != 120
        || progress.target.is_some()
        || quote.status != Status::Delivered
        || quote.invoice.value() <= 0
        || quote.freight.value() <= 0
        || final_world
            .accounts
            .iter()
            .map(|a| a.cash.value())
            .sum::<i64>()
            != cash
        || jobs.is_empty()
    {
        return Err("background_probe_business_not_completed".into());
    }
    let field = e
        .background()
        .and_then(|b| b.field())
        .ok_or("background_probe_field")?
        .report();
    let engine_owned = e.owned_capacity_bytes();
    let before_cp = store.state_hash().map_err(err)?;
    let capacity_before = store.capacity_status().map_err(err)?;
    store.checkpoint().map_err(err)?;
    drop(store);
    let mut store = Store::open(&world, &rules).map_err(err)?;
    if store.state_hash().map_err(err)? != before_cp {
        return Err("background_probe_restore".into());
    }
    let last = last_receipt.ok_or("background_probe_last_receipt")?;
    let sequence = store.committed_sequence();
    let retry = store.execute(Caller::System, last).map_err(err)?;
    if retry.model.is_err()
        || !retry.has_persistent_receipt
        || store.committed_sequence() != sequence
    {
        return Err("background_probe_original_receipt".into());
    }
    let report = json!({"schema":"mycity-background-store-probe-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"rules_identity":rules,
        "controller_pid":std::process::id(),"worker_pid":worker_pid,"jobs":jobs,"field":field,"world":final_world,"initial_total_cash_cents":cash,
        "quote1":{"status":quote.status,"delivered":quote.delivered,"invoice":quote.invoice,"freight":quote.freight},
        "timeline":progress,"engine_owned_bytes":engine_owned,"state_sha256":before_cp,"checkpoint_reopen_original_receipt":true,
        "capacity_before_checkpoint":capacity_before,"capacity_after_checkpoint":store.capacity_status().map_err(err)?,"checkpoint":store.checkpoint_status().map_err(err)?,
        "wall_seconds":started.elapsed().as_secs_f64(),"independent_sim":"not_implemented","joint_1800_seconds":"not_run","whole_process_group_resources":"not_run","memory_16gb_gate":"not_run"});
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(err)?,
    )
    .map_err(err)?;
    Ok(report)
}
