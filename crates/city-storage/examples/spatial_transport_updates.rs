//! Stage one exact, bounded road/door access candidate for an existing spatial Store.
//!
//! This CLI does not create approvals, execute a change, settle a market, or claim runtime
//! acceptance. A staged profile is only an immutable candidate for the normal owner/authority
//! request workflow. Example:
//! `cargo run -p mycity-storage --example spatial_transport_updates -- WORLD 70 24 close 2 3 2`
use mycity_core::{
    spatial_transport_permissions::{
        SpatialAccessOperation, SpatialAuthorizationContext, SpatialUpdateRequest,
    },
    units::GameTime,
};
use mycity_storage::{spatial_transport_updates::preview_profile_asset, Store};
use serde::Serialize;
use std::{env, io, path::PathBuf};

#[derive(Serialize)]
struct StagedCandidate {
    status: &'static str,
    world_path: String,
    /// Trusted context derived from the Store's immutable spatial permission registry.
    authorization_context: SpatialAuthorizationContext,
    request: SpatialUpdateRequest,
    candidate_profile_sha256: [u8; 32],
    candidate_profile_bytes: usize,
}

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: spatial_transport_updates WORLD ASSET_ID EDGE_ID close|restore APPLICANT_ACCOUNT EXECUTOR_ACCOUNT EFFECTIVE_SERVICE_DAY",
    )
}

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let path = PathBuf::from(args.next().ok_or_else(|| usage().to_string())?);
    let asset_id: u32 = args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .parse()
        .map_err(|error| format!("{error:?}"))?;
    let edge_id: u32 = args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .parse()
        .map_err(|error| format!("{error:?}"))?;
    let operation = match args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .as_ref()
    {
        "close" => SpatialAccessOperation::Close,
        "restore" => SpatialAccessOperation::Restore,
        _ => return Err(usage().to_string()),
    };
    let applicant: u32 = args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .parse()
        .map_err(|error| format!("{error:?}"))?;
    let executor: u32 = args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .parse()
        .map_err(|error| format!("{error:?}"))?;
    let service_day: u64 = args
        .next()
        .ok_or_else(|| usage().to_string())?
        .to_string_lossy()
        .parse()
        .map_err(|error| format!("{error:?}"))?;
    if args.next().is_some() || asset_id == 0 || edge_id == 0 || applicant == 0 || executor == 0 {
        return Err(usage().to_string());
    }
    let effective_seconds = service_day
        .checked_mul(mycity_core::timeline::DAY)
        .ok_or_else(|| usage().to_string())?;
    let effective_time =
        GameTime::new(i64::try_from(effective_seconds).map_err(|error| format!("{error:?}"))?)
            .map_err(|error| format!("{error:?}"))?;

    let mut store = Store::open(&path, "m0-spatial-transport-updates-v1")
        .map_err(|error| format!("{error:?}"))?;
    let engine = store.engine().map_err(|error| format!("{error:?}"))?;
    let permissions = engine
        .spatial_permissions()
        .ok_or_else(|| "spatial permission registry missing".to_string())?;
    let asset = permissions
        .assets()
        .iter()
        .find(|asset| asset.asset_id == asset_id)
        .ok_or_else(|| "managed spatial asset missing".to_string())?;
    let managed_edge = asset
        .edges
        .iter()
        .find(|edge| edge.edge_id == edge_id)
        .ok_or_else(|| "managed edge missing".to_string())?;
    let surface = managed_edge.surface;
    let expected_open = operation == SpatialAccessOperation::Close;
    if managed_edge.currently_open != expected_open {
        return Err("requested operation does not match current edge state".into());
    }
    let (world_id, branch_id, profile_id) = {
        let view = engine.world().view();
        let market = view
            .town
            .as_ref()
            .ok_or_else(|| "town transport market missing".to_string())?;
        (view.scope.world(), view.scope.branch(), market.profile.id)
    };
    let parent_profile_sha256 = asset.current_profile_sha256;
    let authorization_context = permissions
        .authorization_context(asset_id)
        .map_err(|error| format!("{error:?}"))?;
    let expected_asset_revision = asset.revision;
    let request_id = permissions
        .request_id_highwater
        .checked_add(1)
        .ok_or_else(|| usage().to_string())?;
    let current =
        mycity_storage::spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &path,
            parent_profile_sha256,
            world_id,
            branch_id,
            profile_id,
        )
        .map_err(|error| format!("{error:?}"))?;
    let mut proposed = current.profile().opening.clone();
    proposed.revision = proposed
        .revision
        .checked_add(1)
        .ok_or_else(|| usage().to_string())?;
    let edge = proposed
        .edges
        .iter_mut()
        .find(|edge| edge.id == edge_id)
        .ok_or_else(|| "candidate edge missing from profile".to_string())?;
    edge.open = operation == SpatialAccessOperation::Restore;
    let candidate = preview_profile_asset(&current.as_ref(), proposed.clone())
        .map_err(|error| format!("{error:?}"))?;
    let request = SpatialUpdateRequest {
        request_id,
        applicant_account: applicant,
        executor_account: executor,
        asset_id,
        operation,
        surface,
        edge_id,
        expected_revision: expected_asset_revision,
        candidate_profile_sha256: candidate.sha256,
        effective_time,
        parent_profile_sha256,
    };
    let stored = store
        .stage_spatial_transport_candidate(request, proposed)
        .map_err(|error| format!("{error:?}"))?;
    if stored != candidate.sha256 {
        return Err("staged candidate digest mismatch".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&StagedCandidate {
            status: "STAGED_CANDIDATE_ONLY_REQUIRES_OWNER_AND_AUTHORITY_APPROVAL",
            world_path: path.display().to_string(),
            authorization_context,
            request,
            candidate_profile_sha256: candidate.sha256,
            candidate_profile_bytes: candidate.bytes.len(),
        })
        .map_err(|error| format!("{error:?}"))?
    );
    eprintln!(
        "No permission request or approval was committed; candidate staging does not change the active transport profile."
    );
    Ok(())
}
