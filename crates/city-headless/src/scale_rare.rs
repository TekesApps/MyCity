//! Same-full-load independent restored branches. Never injects cash or rewrites dates.
use crate::{
    joint_reference::{self, Reference},
    scale_reference::{self, Tier},
    scale_sim,
};
use mycity_core::{
    assembly::assemble,
    background::{self, Command as B},
    boot::{Command as Boot, Facility},
    exchange::Command as X,
    inbox::Command as I,
    schedule::Phase,
    sha256,
    timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    time::Instant,
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = File::create_new(path).map_err(err)?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(err)
}
fn json_file(path: &Path, v: &Value) -> Result<(), String> {
    write(path, &serde_json::to_vec_pretty(v).map_err(err)?)
}
fn expected(e: &Engine, target: u64) -> Result<Value, String> {
    let b = e.background().ok_or("rare_background")?;
    Ok(
        json!({"target":target,"business_sha256":joint_reference::business_digest(e)?,"air_sha256":sha256(&b.field().ok_or("rare_air")?.checkpoint()?),"windows":b.version()}),
    )
}
fn checked_dir(dir: &Path) -> Result<std::path::PathBuf, String> {
    fs::create_dir(dir).map_err(err)?;
    dir.canonicalize().map_err(err)
}
/// Runtime-local locks/owner/cache are rebuilt; all authoritative bytes are copied.
/// Store and worker fences prevent copying an actively written source.
pub fn copy_world(from: &Path, to: &Path) -> Result<(), String> {
    let canonical = from.canonicalize().map_err(err)?;
    let from = canonical.as_path();
    let rules = format!("sim-t13a-full-v2:{}", env!("MYCITY_SOURCE_SHA256"));
    let _store = Store::open(from, &rules).map_err(err)?;
    let _fence = crate::worker_cache::recover_exclusive(
        &from.join("air-worker"),
        &std::env::current_exe().map_err(err)?,
    )?;
    fs::create_dir(to).map_err(err)?;
    fn copy(from: &Path, to: &Path, top: bool, count: &mut usize) -> Result<(), String> {
        for entry in fs::read_dir(from).map_err(err)? {
            let entry = entry.map_err(err)?;
            let name = entry.file_name();
            if top
                && matches!(
                    name.to_str(),
                    Some("LOCK" | "air-worker.lock" | "air-worker-owner.json" | "air-worker")
                )
            {
                continue;
            }
            *count += 1;
            if *count > 512 {
                return Err("rare_copy_file_bound".into());
            }
            let ty = entry.file_type().map_err(err)?;
            if ty.is_symlink() {
                return Err("rare_copy_symlink".into());
            }
            let target = to.join(&name);
            if ty.is_dir() {
                fs::create_dir(&target).map_err(err)?;
                copy(&entry.path(), &target, false, count)?;
            } else if ty.is_file() {
                fs::copy(entry.path(), &target).map_err(err)?;
                File::open(&target)
                    .and_then(|f| f.sync_all())
                    .map_err(err)?;
            } else {
                return Err("rare_copy_nonregular".into());
            }
        }
        File::open(to).and_then(|f| f.sync_all()).map_err(err)
    }
    copy(from, to, true, &mut 0)?;
    let _opened = Store::open(to, &rules).map_err(err)?;
    if scale_sim::image(from)? != scale_sim::image(to)? {
        return Err("rare_copy_authority_changed".into());
    }
    Ok(())
}
fn apply(
    e: &mut Engine,
    action: Action,
    candidate: Option<&mycity_core::air::State>,
    operation: Option<u64>,
) -> Result<(), String> {
    let v = e.world().view();
    let envelope = Envelope {
        world: v.scope.world(),
        branch: v.scope.branch(),
        operation_id: operation.unwrap_or(v.revision + 1),
        expected_revision: v.revision,
        actions: vec![action],
    };
    e.execute_recorded_with_background(Caller::System, envelope, candidate, |_, _| {
        Ok::<(), String>(())
    })?
    .map_err(err)?;
    Ok(())
}
/// Reference only. These executions never contribute actual Sim timing samples.
fn cpu_drive(e: &mut Engine, target: u64) -> Result<(), String> {
    for _ in 0..1000 {
        let p = e.timeline().ok_or("rare_timeline")?.progress;
        if p.target.is_none() && p.last_complete == target {
            return Ok(());
        }
        if p.phase == Some(Phase::P4) && e.background().and_then(|b| b.pending()).is_some() {
            let job = e
                .background()
                .and_then(|b| b.pending())
                .ok_or("rare_job")?
                .id;
            let id = 0xd000_0000_0000_0000 | (e.world().view().revision + 1);
            apply(
                e,
                Action::Background {
                    command: B::RebindLease { job },
                },
                None,
                Some(id),
            )?;
            let b = e.background().ok_or("rare_background")?;
            let generation = b.pending().ok_or("rare_job")?.generation;
            let mut state = b.frozen_input()?;
            while !state.complete() {
                state.page(8192, 1024)?;
            }
            use sha2::Digest;
            let hash: [u8; 32] = sha2::Sha256::digest(state.checkpoint()?).into();
            apply(
                e,
                Action::Background {
                    command: B::Accept {
                        job,
                        generation,
                        candidate_sha256: hash,
                    },
                },
                Some(&state),
                Some(background::accept_operation_id(job)?),
            )?;
        } else {
            apply(
                e,
                Action::Timeline {
                    command: T::Advance {
                        target,
                        limit: 8192,
                    },
                },
                None,
                None,
            )?;
        }
    }
    Err("rare_cpu_bound".into())
}
pub fn anchors(dir: &Path) -> Result<Value, String> {
    let dir = checked_dir(dir)?;
    let source = dir.join("source");
    fs::create_dir(&source).map_err(err)?;
    let mut sim = scale_sim::independent_experiment(&source, true, false)?;
    let mut cpu = Reference::from_unstarted_engine(
        Engine::new(
            assemble(scale_reference::OPENING)?,
            scale_reference::resources()?,
        )
        .map_err(err)?,
    )?;
    let policy: Value = serde_json::from_slice(scale_reference::POLICY).map_err(err)?;
    for a in scale_reference::startup_actions(&policy)? {
        cpu.execute(vec![a])?;
        sim.raw(vec![a])?;
    }
    cpu.drive(60)?;
    sim.drive(60, &expected(&cpu.engine, 60)?)?;
    for a in std::iter::once(Action::Boot {
        command: Boot::Start {
            facility: Facility::Generator,
            reverse_request: false,
        },
    })
    .chain((1..=4).map(|quote| Action::Exchange {
        command: X::Dispatch { quote },
    })) {
        cpu.execute(vec![a])?;
        sim.raw(vec![a])?;
    }
    sim.save_stop()?;
    let (source_bytes, source_root) = scale_sim::image(&source.join("world"))?;
    let mut source_expected = Engine::restore_checkpoint(&source_bytes)?;
    cpu_drive(&mut source_expected, 180)?;
    write(
        &source.join("expected-engine.json"),
        &source_expected.export_checkpoint()?,
    )?;
    let claims = dir.join("claims");
    fs::create_dir(&claims).map_err(err)?;
    copy_world(&source.join("world"), &claims.join("world"))?;
    let mut sim = scale_sim::independent_experiment(&claims, false, false)?;
    cpu.drive(180)?;
    sim.drive(180, &expected(&cpu.engine, 180)?)?;
    for sequence in 1..=4 {
        for command in [I::Settle { sequence }, I::Archive { sequence }] {
            let a = Action::Inbox { command };
            cpu.execute(vec![a])?;
            sim.raw(vec![a])?;
        }
    }
    for hour in 1..=23 {
        let target = hour * 3600;
        cpu.drive(target)?;
        sim.drive(target, &expected(&cpu.engine, target)?)?;
    }
    sim.save_stop()?;
    let (claims_bytes, claims_root) = scale_sim::image(&claims.join("world"))?;
    let mut claims_expected = Engine::restore_checkpoint(&claims_bytes)?;
    cpu_drive(&mut claims_expected, 86400)?;
    write(
        &claims.join("expected-engine.json"),
        &claims_expected.export_checkpoint()?,
    )?;
    let t = claims_expected.timeline().ok_or("rare_timeline")?;
    if t.claims.len() != 20000
        || t.payroll.len() != 64
        || t.claims.iter().filter(|c| c.processed).count() != 1000
        || t.claims.iter().map(|c| c.paid.value()).sum::<i64>() != 1000
    {
        return Err("rare_claim_oracle".into());
    }
    let result = json!({"schema":"mycity-rare-anchors-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"opening_sha256":sha256(Tier::Full.opening()),"policy_sha256":sha256(Tier::Full.policy()),"source_anchor":{"at":60,"target":180,"world_sha256":sha256(&source_bytes),"root":source_root,"expected_engine_sha256":sha256(&source_expected.export_checkpoint()?)},"claims_anchor":{"at":23*3600,"target":86400,"world_sha256":sha256(&claims_bytes),"root":claims_root,"expected_engine_sha256":sha256(&claims_expected.export_checkpoint()?)},"formal_T13A":"not_accepted"});
    json_file(&dir.join("report.json"), &result)?;
    Ok(result)
}
pub fn trial(anchors: &Path, dir: &Path, kind: &str) -> Result<Value, String> {
    let target = match kind {
        "source" => 180,
        "claims" => 86400,
        _ => return Err("rare_trial_kind".into()),
    };
    let dir = checked_dir(dir)?;
    let anchor = anchors.join(kind);
    let began = Instant::now();
    copy_world(&anchor.join("world"), &dir.join("world"))?;
    let expected_bytes = mycity_core::read_bounded(
        File::open(anchor.join("expected-engine.json")).map_err(err)?,
        4 * 1024 * 1024,
    )
    .map_err(err)?;
    let golden = Engine::restore_checkpoint(&expected_bytes)?;
    let point = expected(&golden, target)?;
    drop(golden);
    let mut sim = scale_sim::independent_experiment(&dir, false, true)?;
    sim.drive(target, &point)?;
    sim.save_stop()?;
    let (actual, root) = scale_sim::image(&dir.join("world"))?;
    if actual != expected_bytes {
        return Err("rare_complete_engine_mismatch".into());
    }
    // Cold reopen is observational, verifies durable roots and original receipts again.
    let rules = format!("sim-t13a-full-v2:{}", env!("MYCITY_SOURCE_SHA256"));
    let store = Store::open(&dir.join("world"), &rules).map_err(err)?;
    if store.engine().map_err(err)?.export_checkpoint()? != actual {
        return Err("rare_cold_mismatch".into());
    }
    let result = json!({"schema":"mycity-rare-trial-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"experiment":dir.to_str(),"stratum":kind,"target":target,"trial_wall_seconds":began.elapsed().as_secs_f64(),"complete_engine_equal":true,"expected_engine_sha256":sha256(&expected_bytes),"actual_engine_sha256":sha256(&actual),"cold_reopen_equal":true,"root":root,"formal_T13A":"not_accepted"});
    json_file(&dir.join("report.json"), &result)?;
    Ok(result)
}

/// Actual cold verification for a complete independently extracted archive. No clock advance.
pub fn restore(world: &Path, expected: &Path) -> Result<Value, String> {
    let rules = format!("sim-t13a-full-v2:{}", env!("MYCITY_SOURCE_SHA256"));
    let expected = mycity_core::read_bounded(File::open(expected).map_err(err)?, 4 * 1024 * 1024)
        .map_err(err)?;
    let store = Store::open(world, &rules).map_err(err)?;
    let bytes = store.engine().map_err(err)?.export_checkpoint()?;
    if bytes != expected {
        return Err("rare_archive_restore_mismatch".into());
    }
    Ok(
        json!({"schema":"mycity-rare-archive-restore-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"actual_engine_sha256":sha256(&bytes),"cold_archive_restore":"passed","sequence":store.committed_sequence(),"formal_T13A":"not_accepted"}),
    )
}
