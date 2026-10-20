//! Storage growth lower bound on the actual full initial world; not the L2 run.
use crate::scale_reference;
use mycity_core::{
    sha256,
    transaction::{Caller, Envelope},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    time::Instant,
};
const RULES: &str = "t13a-storage-growth-diagnostic-v1";
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn checkpoint_hash(s: &Store) -> Result<String, String> {
    Ok(sha256(&s.engine().map_err(err)?.export_checkpoint()?))
}
/// Repeated paused-world saves are a LOWER bound: no daily growth, no Sim/IPC,
/// no business playback. Failure here cannot be reported as 96-save acceptance.
pub fn run(dir: &Path) -> Result<Value, String> {
    run_inner(dir, false)
}
pub fn run_v2(dir: &Path) -> Result<Value, String> {
    run_inner(dir, true)
}
fn run_inner(dir: &Path, v2: bool) -> Result<Value, String> {
    fs::create_dir(dir).map_err(err)?;
    let world = dir.join("world");
    let mut events = File::create(dir.join("events.jsonl")).map_err(err)?;
    let started = Instant::now();
    let create = if v2 {
        Store::create_spatial_v2
    } else {
        Store::create_spatial
    };
    let mut store = create(
        &world,
        scale_reference::OPENING,
        scale_reference::resources()?,
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .map_err(err)?;
    let policy: Value = serde_json::from_slice(scale_reference::POLICY).map_err(err)?;
    let mut first = None;
    for (i, action) in scale_reference::startup_actions(&policy)?
        .into_iter()
        .enumerate()
    {
        let v = store.engine().map_err(err)?.world().view();
        let env = Envelope {
            world: v.scope.world(),
            branch: v.scope.branch(),
            operation_id: i as u64 + 1,
            expected_revision: v.revision,
            actions: vec![action],
        };
        let result = store.execute(Caller::System, env.clone()).map_err(err)?;
        result.model.map_err(err)?;
        if !result.has_persistent_receipt {
            return Err("scale_store_missing_receipt".into());
        }
        if i == 0 {
            first = Some(env);
        }
    }
    let first = first.ok_or("scale_store_no_startup")?;
    let first_receipt =
        serde_json::to_vec(&store.receipt(first.operation_id).map_err(err)?).map_err(err)?;
    let timeline = store
        .engine()
        .map_err(err)?
        .timeline()
        .ok_or("scale_store_no_timeline")?;
    if timeline.claims.len() != 20000 || timeline.payroll.len() != 64 {
        return Err("scale_store_counts".into());
    }
    let view = store.engine().map_err(err)?.world().view();
    if view.accounts.len() != 64
        || view.stocks.len() != 128
        || view
            .region
            .as_ref()
            .ok_or("scale_store_region")?
            .cities
            .iter()
            .flatten()
            .count()
            != 8
        || view
            .exchange
            .as_ref()
            .ok_or("scale_store_exchange")?
            .maps
            .len()
            != 8
    {
        return Err("scale_store_world_counts".into());
    }
    drop(view);
    let original = checkpoint_hash(&store)?;
    let sequence = store.committed_sequence();
    let engine_bytes = store.engine().map_err(err)?.export_checkpoint()?.len();
    let initialized_seconds = started.elapsed().as_secs_f64();
    let mut saves = Vec::with_capacity(96);
    let mut cold_samples = Vec::with_capacity(8);
    let mut failed = None;
    for n in 1..=96 {
        let before = store.checkpoint_status().map_err(err)?;
        let t = Instant::now();
        if let Err(e) = store.checkpoint() {
            failed = Some(
                json!({"requested_save":n,"error":format!("{e:?}"),"seconds":t.elapsed().as_secs_f64(),"last_good_status":before}),
            );
            // A rejected checkpoint may poison the handle; only cold reopen is trusted.
            drop(store);
            let t = Instant::now();
            store = Store::open(&world, RULES).map_err(err)?;
            cold_samples.push(json!({"after_rejected_save":n,"seconds":t.elapsed().as_secs_f64()}));
            if checkpoint_hash(&store)? != original {
                return Err("scale_store_failed_save_changed_world".into());
            }
            break;
        }
        let seconds = t.elapsed().as_secs_f64();
        let status = store.checkpoint_status().map_err(err)?;
        let row = json!({"save":n,"seconds":seconds,"status":status});
        serde_json::to_writer(&mut events, &row).map_err(err)?;
        events.write_all(b"\n").map_err(err)?;
        events.flush().map_err(err)?;
        saves.push(row);
        if matches!(n, 1 | 2 | 4 | 8 | 12 | 16 | 32 | 64 | 96) {
            drop(store);
            let t = Instant::now();
            store = Store::open(&world, RULES).map_err(err)?;
            let cold_seconds = t.elapsed().as_secs_f64();
            if checkpoint_hash(&store)? != original || store.committed_sequence() != sequence {
                return Err("scale_store_cold_state_changed".into());
            }
            let t = Instant::now();
            let receipt = store.receipt(first.operation_id).map_err(err)?;
            let receipt_seconds = t.elapsed().as_secs_f64();
            if serde_json::to_vec(&receipt).map_err(err)? != first_receipt {
                return Err("scale_store_oldest_receipt_changed".into());
            }
            let t = Instant::now();
            let result = store.execute(Caller::System, first.clone()).map_err(err)?;
            let retry_seconds = t.elapsed().as_secs_f64();
            if !result.has_persistent_receipt
                || result.model.is_err()
                || store.committed_sequence() != sequence
                || checkpoint_hash(&store)? != original
            {
                return Err("scale_store_retry_changed_state".into());
            }
            let t = Instant::now();
            if store.receipt(sequence + 1).map_err(err)?.is_some() {
                return Err("scale_store_phantom_receipt".into());
            }
            let absent_lookup_seconds = t.elapsed().as_secs_f64();
            cold_samples.push(json!({"after_save":n,"cold_seconds":cold_seconds,"oldest_receipt_seconds":receipt_seconds,"original_command_retry_seconds":retry_seconds,"absent_receipt_lookup_seconds":absent_lookup_seconds,"full_checkpoint_state_equal":true,"original_receipt_equal":true}));
        }
    }
    events.sync_all().map_err(err)?;
    let report = json!({"schema":"mycity-full-storage-growth-lower-bound-v1","checkpoint_layout":if v2 {2} else {1},"source_sha256":env!("MYCITY_SOURCE_SHA256"),"opening_sha256":sha256(scale_reference::OPENING),"policy_sha256":sha256(scale_reference::POLICY),"full_initial_accounts":64,"full_initial_stocks":128,"payroll":64,"claims":20000,"engine_checkpoint_bytes":engine_bytes,"initialization_seconds":initialized_seconds,"wall_seconds":started.elapsed().as_secs_f64(),"original_engine_checkpoint_sha256":original,"committed_startup_commands":sequence,"requested_saves":96,"completed_saves":saves.len(),"saves":saves,"cold_samples":cold_samples,"rejected_save":failed,"final_state_equal":checkpoint_hash(&store)?==original,"final_status":store.checkpoint_status().map_err(err)?,"business_time_advanced":false,"scope":"paused full initial world; lower bound, not full L2 business/IPC, no fabricated throughput acceptance","ninety_six_save_gate":if saves.len()==96 { "storage_lower_bound_only" } else { "failed_lower_bound" },"eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(err)?,
    )
    .map_err(err)?;
    if failed.is_some() {
        return Err("scale_store_96_save_lower_bound_failed; see report.json".into());
    }
    Ok(report)
}
