//! Run the shared Store-backed geometry revision microfixture and retain its world directory.
//!
//! The fixture uses explicit synthetic account IDs and is not player authentication evidence.
//! It executes the same scenario as the `same_store_authorized_geometry_revision_changes_trade_and_cold_replays`
//! integration test: a road-control-point revision, separate owner and authority approvals,
//! day-2 market settlement, proof history, checkpoint reopen, and operation replay checks.
//!
//! Usage: `cargo run -p mycity-storage --example spatial_geometry_scenario -- WORLD_DIRECTORY`
//! The path must not already exist. The resulting Store files are retained for independent
//! inspection with the normal Store reader.
//!
//! To inspect an externally prepared working copy, use
//! `cargo run -p mycity-storage --example spatial_geometry_scenario -- --verify-open WORLD_COPY`.
//! `Store::open` may finish recovery and write to that copy, so prepare and hash the copy outside
//! this process; this mode never clones, replaces, or deletes Store files.

#[path = "support/spatial_geometry_apply.rs"]
mod scenario;

use std::{env, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let first = args.next().ok_or_else(|| {
        "usage: spatial_geometry_scenario WORLD_DIRECTORY | --stale-after-close road|door WORLD_DIRECTORY | --verify-open WORKING_COPY".to_string()
    })?;
    if first == "--stale-after-close" {
        let changed_asset = match args.next().as_deref() {
            Some(value) if value == "road" => 70,
            Some(value) if value == "door" => 71,
            _ => return Err(stale_usage()),
        };
        let world = PathBuf::from(args.next().ok_or_else(stale_usage)?);
        if args.next().is_some() || world.exists() {
            return Err(stale_usage());
        }
        scenario::run_stale_geometry_after_access_close(&world, changed_asset);
        println!(
            "{{\"status\":\"stale_geometry_rejected\",\"changed_asset_id\":{changed_asset},\"world\":{}}}",
            serde_json::to_string(&world.display().to_string()).map_err(|error| format!("{error:?}"))?
        );
        return Ok(());
    }
    if first == "--verify-open" {
        let world = PathBuf::from(args.next().ok_or_else(verify_usage)?);
        let metadata = std::fs::symlink_metadata(&world).ok();
        if args.next().is_some()
            || !metadata
                .as_ref()
                .is_some_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        {
            return Err(verify_usage());
        }
        eprintln!(
            "Store::open may write during recovery; use a separately cloned working copy, never the authoritative source."
        );
        let report = verify_open(&world)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| format!("{error:?}"))?
        );
        return Ok(());
    }
    let world = PathBuf::from(first);
    if args.next().is_some() {
        return Err(scenario_usage());
    }
    if world.exists() {
        return Err(format!(
            "refusing to overwrite existing Store directory: {}",
            world.display()
        ));
    }
    scenario::run(&world);
    println!(
        "{{\"status\":\"scenario_complete\",\"fixture\":\"m0_spatial_geometry_day1_day2\",\"world\":{}}}",
        serde_json::to_string(&world.display().to_string()).map_err(|error| format!("{error:?}"))?
    );
    Ok(())
}

fn scenario_usage() -> String {
    "usage: spatial_geometry_scenario WORLD_DIRECTORY (must not exist)".into()
}

fn stale_usage() -> String {
    "usage: spatial_geometry_scenario --stale-after-close road|door WORLD_DIRECTORY (must not exist)".into()
}

fn verify_usage() -> String {
    "usage: spatial_geometry_scenario --verify-open EXISTING_WORKING_COPY".into()
}

fn verify_open(world: &std::path::Path) -> Result<serde_json::Value, String> {
    use mycity_storage::Store;
    use serde_json::json;

    let mut store = Store::open(world, "m0-spatial-transport-updates-v1")
        .map_err(|error| format!("Store::open failed: {error:?}"))?;
    let state_hash = store
        .state_hash()
        .map_err(|error| format!("state hash failed: {error:?}"))?;
    let owned_capacity_bytes = store
        .owned_capacity_bytes()
        .map_err(|error| format!("owned capacity read failed: {error:?}"))?;
    let engine = store
        .engine()
        .map_err(|error| format!("Engine read failed: {error:?}"))?;
    let view = engine.world().view();
    let timeline = engine.timeline().map(|timeline| {
        json!({
            "last_complete_seconds": timeline.progress.last_complete,
            "current_seconds": timeline.progress.at,
            "service_day": timeline.progress.last_complete
                / mycity_core::timeline::DAY,
            "phase_active": timeline.progress.phase.is_some()
        })
    });
    let market = view
        .town
        .as_ref()
        .map(|town| (town.profile.id, town.profile.transport_allocation_sha256));
    let geometry_adapter_basis_sha256 = store
        .geometry_adapter_basis()
        .map_err(|error| format!("adapter basis read failed: {error:?}"))?
        .map(|basis| basis.sha256().map(|hash| hex(&hash)))
        .transpose()
        .map_err(|error| format!("adapter basis hash failed: {error:?}"))?;
    let active_scene = store
        .spatial()
        .map_err(|error| format!("active Scene read failed: {error:?}"))?
        .map(|assets| {
            let scene = assets.scene();
            let input = scene.input();
            json!({
                "state_sha256": scene.state_hash(),
                "world_id": input.map.world,
                "branch_id": input.map.branch,
                "map_id": input.map.map,
                "geometry_revision": input.geometry_revision
            })
        });
    let logical_file_bytes =
        logical_file_bytes(world).map_err(|error| format!("file size scan failed: {error}"))?;
    Ok(json!({
        "status":"existing_store_opened_for_inspection",
        "world_copy":world.display().to_string(),
        "copy_identity_verified_by_this_command":false,
        "store_open_may_have_performed_recovery_writes":true,
        "committed_sequence":store.committed_sequence(),
        "engine_revision":view.revision,
        "world_id":view.scope.world(),
        "branch_id":view.scope.branch(),
        "state_sha256":state_hash,
        "market_profile_id":market.map(|entry| entry.0),
        "active_profile_sha256":market.map(|entry| format!("{}", hex(&entry.1))),
        "geometry_adapter_basis_sha256":geometry_adapter_basis_sha256,
        "timeline":timeline,
        "active_scene":active_scene,
        "owned_capacity_bytes":owned_capacity_bytes,
        "logical_file_bytes":logical_file_bytes,
        "identity_note":"Fixture account IDs are simulation inputs, not player authentication evidence."
    }))
}

fn logical_file_bytes(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0_u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            total = total.saturating_add(logical_file_bytes(&entry.path())?);
        } else if kind.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

fn hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}
