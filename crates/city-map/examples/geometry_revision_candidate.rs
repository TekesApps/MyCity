//! Materialize a finite, synthetic geometry-revision candidate for inspection.
//!
//! Usage: cargo run -p mycity-map --example geometry_revision_candidate -- <fresh-output-dir> [curve|door-width|door-pose]
//! This CLI never publishes the result or modifies a Store, permission state, or calendar.

use mycity_core::{
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TripPartition,
    },
    transport::OdDemand,
    units::Quantity,
};
use mycity_map::{
    geometry::{Facing, Mode, Port},
    geometry_revision::{build_candidate, GeometryEdit, PortDimension},
    probe,
    scene::Scene,
    terrain::{import, Terrain},
    traffic_adapter::{
        adapt, AdapterIds, AdapterInput, FacilityOperation, RoadDirection, TerminalGrant,
        WALK_MODE_MASK,
    },
    Point,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

const SPATIAL_FIXTURE: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const HEIGHTMAP_FIXTURE: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");

fn hex(bytes: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(*byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
    }
    encoded
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn demands() -> [OdDemand; 1] {
    [OdDemand {
        source_id: 1,
        origin_entrance: 11,
        destination_entrance: 12,
        mode: WALK_MODE_MASK,
        people: 4,
    }]
}

fn adapter(map_id: u32, ground_z_cm: i64, ds: &[OdDemand]) -> AdapterInput<'_> {
    AdapterInput {
        expected_map_id: map_id,
        city_id: 42,
        revision: 1,
        building_owner_id: 100,
        ids: AdapterIds {
            building_node: 1,
            anchor_node: 2,
            terminal_node: 3,
            building_entrance: 11,
            connector_forward_edge: 21,
            connector_reverse_edge: 22,
            road_forward_edge: 23,
            road_reverse_edge: 24,
        },
        terminal: TerminalGrant {
            owner_id: 200,
            entrance_id: 12,
            port: Port {
                point: Point { x: 1000, y: 1000 },
                z_cm: ground_z_cm,
                mode: Mode::Walk,
                facing: Facing::West,
                clear_width_cm: 700,
                can_enter: true,
                can_exit: true,
                authorized: true,
            },
        },
        connector: FacilityOperation {
            capacity_people_per_period: 4,
            free_flow_speed_cm_per_second: 100,
            open: true,
        },
        road: FacilityOperation {
            capacity_people_per_period: 4,
            free_flow_speed_cm_per_second: 100,
            open: true,
        },
        road_direction: RoadDirection::Both,
        demands: ds,
    }
}

fn baseline_profile(
    source: &probe::Input,
    terrain: &Terrain,
    ds: &[OdDemand],
) -> Result<TransportAllocationProfile, String> {
    let scene = Scene::assemble(source.scene.clone(), terrain, terrain)
        .map_err(|error| format!("baseline Scene rejected: {error:?}"))?;
    let opening = adapt(
        &scene,
        adapter(source.scene.map.map, scene.input().ground_z_cm, ds),
    )
    .map_err(|error| format!("baseline adapter rejected: {error:?}"))?;
    let assignment = opening
        .assign()
        .map_err(|error| format!("baseline assignment rejected: {error:?}"))?;
    let profile = TransportAllocationProfile {
        schema: 1,
        world_id: source.scene.map.world,
        branch_id: source.scene.map.branch,
        market_profile_id: 1,
        period_seconds: mycity_core::timeline::DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: None,
        opening,
        assignment,
        partitions: vec![TripPartition {
            quote_id: 1,
            outbound_source: 1,
            return_source: 1,
            trip_count: 4,
            goods_per_trip: Quantity::new(1).map_err(|error| format!("quantity: {error:?}"))?,
        }],
        fee_policy: FeePolicyInput {
            provider_account: 200,
            cents_per_travel_second: 0,
        },
    };
    profile
        .validate()
        .map_err(|error| format!("baseline profile rejected: {error:?}"))?;
    Ok(profile)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {}: {error}", path.display()))
}

fn parse_args() -> Result<(PathBuf, GeometryEdit), String> {
    let mut args = env::args_os().skip(1);
    let output = args.next().map(PathBuf::from).ok_or_else(|| {
        "usage: geometry_revision_candidate <fresh-output-dir> [curve|door-width|door-pose]"
            .to_owned()
    })?;
    let edit = match args.next().as_deref().and_then(|value| value.to_str()) {
        None | Some("curve") => GeometryEdit::RoadControlPoint(Point { x: 3000, y: 1800 }),
        Some("door-width") => GeometryEdit::PortClearWidth {
            port: PortDimension::DoorClearWidth,
            width_cm: 400,
        },
        Some("door-pose") => GeometryEdit::DoorPose {
            point: Point { x: 4000, y: 3000 },
            facing: Facing::East,
            first_connector_waypoint: Point { x: 5000, y: 3000 },
        },
        Some(other) => {
            return Err(format!(
                "unsupported edit {other:?}; choose curve, door-width, or door-pose"
            ))
        }
    };
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    Ok((output, edit))
}

fn run() -> Result<(), String> {
    let (output_dir, edit) = parse_args()?;
    if output_dir.exists() {
        return Err(format!(
            "refusing to overwrite existing output path: {}",
            output_dir.display()
        ));
    }

    let started = Instant::now();
    let source: probe::Input = serde_json::from_slice(SPATIAL_FIXTURE)
        .map_err(|error| format!("decode pinned spatial fixture: {error}"))?;
    let terrain = import(source.terrain.clone(), HEIGHTMAP_FIXTURE, 1, 0)
        .map_err(|error| format!("import pinned flat terrain: {error:?}"))?;
    let ds = demands();
    let prior_profile = baseline_profile(&source, &terrain, &ds)?;
    let prior_profile_bytes = serde_json::to_vec(&prior_profile)
        .map_err(|error| format!("encode baseline profile: {error}"))?;
    let baseline_scene = Scene::assemble(source.scene.clone(), &terrain, &terrain)
        .map_err(|error| format!("baseline Scene rejected: {error:?}"))?;
    let baseline_scene_owned_capacity_bytes = baseline_scene.owned_capacity_bytes();
    drop(baseline_scene);

    let result = build_candidate(
        &source.scene,
        &terrain,
        &terrain,
        adapter(source.scene.map.map, source.scene.ground_z_cm, &ds),
        &prior_profile,
        &prior_profile_bytes,
        edit,
    )
    .map_err(|error| format!("candidate rejected: {error:?}"))?;
    let model_elapsed_ms = started.elapsed().as_millis();
    let revision_report = json!({
        "parent_geometry_revision": source.scene.geometry_revision,
        "candidate_geometry_revision": result.scene.input().geometry_revision,
        "parent_transport_revision": prior_profile.opening.revision,
        "candidate_transport_revision": serde_json::from_slice::<TransportAllocationProfile>(
            &result.profile_bytes,
        )
        .map_err(|error| format!("decode candidate profile revision: {error}"))?
        .opening
        .revision,
    });
    let terrain_manifest_source_sha256 = source.terrain.source_sha256.clone();
    drop(prior_profile_bytes);
    drop(prior_profile);
    drop(source);
    let post_baseline_release_owned_capacity_bytes =
        result.candidate_owned_capacity_bytes + terrain.owned_capacity_bytes();
    let scene_input_bytes = serde_json::to_vec_pretty(result.scene.input())
        .map_err(|error| format!("encode candidate Scene input: {error}"))?;
    let payload_logical_bytes = scene_input_bytes.len() + result.profile_bytes.len();
    let edit_label = match edit {
        GeometryEdit::RoadControlPoint(_) => "road_control_point",
        GeometryEdit::PortClearWidth {
            port: PortDimension::DoorClearWidth,
            ..
        } => "door_clear_width",
        GeometryEdit::PortClearWidth {
            port: PortDimension::AnchorClearWidth,
            ..
        } => "anchor_clear_width",
        GeometryEdit::PortClearWidth {
            port: PortDimension::TerminalClearWidth,
            ..
        } => "terminal_clear_width",
        GeometryEdit::DoorPose { .. } => "door_pose",
    };
    let report = json!({
        "schema": "mycity-m0-geometry-revision-candidate-v1",
        "result": "candidate_only_not_activated",
        "authority": "synthetic fixture input only; no legal grant or permission decision",
        "persistence": "not_written_to_store",
        "calendar": "not_advanced",
        "edit": edit_label,
        "door_pose": match edit {
            GeometryEdit::DoorPose { point, facing, first_connector_waypoint } => json!({
                "point_cm": {"x": point.x, "y": point.y},
                "facing": format!("{facing:?}").to_ascii_lowercase(),
                "first_connector_waypoint_cm": {
                    "x": first_connector_waypoint.x,
                    "y": first_connector_waypoint.y
                },
                "inherited_without_mutation": [
                    "door_z_cm", "mode", "clear_width_cm", "can_enter", "can_exit", "authorized"
                ],
                "anchor_and_road_endpoint_preserved": true
            }),
            _ => serde_json::Value::Null,
        },
        "revisions": revision_report,
        "source_sha256": {
            "spatial_fixture_bytes": hex(sha256(SPATIAL_FIXTURE)),
            "heightmap_fixture_bytes": hex(sha256(HEIGHTMAP_FIXTURE)),
            "terrain_manifest_source": terrain_manifest_source_sha256,
            "terrain_state": result.prior_terrain_sha256,
            "prior_scene": result.prior_scene_sha256,
            "candidate_scene": result.candidate_scene_sha256,
            "adapter_before": hex(result.prior_adapter_sha256),
            "adapter_after": hex(result.candidate_adapter_sha256),
            "prior_profile_raw": hex(result.prior_profile_sha256),
            "candidate_profile": hex(result.candidate_profile_sha256),
        },
        "movement": {
            "prior_served_people_per_period": result.prior_served_people,
            "candidate_served_people_per_period": result.candidate_served_people,
            "prior_edge_free_flow_seconds": result.prior_edge_travel_seconds,
            "candidate_edge_free_flow_seconds": result.candidate_edge_travel_seconds,
        },
        "resource": {
            "model_wall_ms": model_elapsed_ms,
            "baseline_scene_owned_capacity_bytes": baseline_scene_owned_capacity_bytes,
            "terrain_owned_capacity_bytes": terrain.owned_capacity_bytes(),
            "candidate_result_owned_capacity_bytes": result.candidate_owned_capacity_bytes,
            "candidate_scene_included_in_candidate_result": true,
            "post_baseline_release_owned_capacity_bytes": post_baseline_release_owned_capacity_bytes,
            "allocator_metadata_included": false,
            "assembly_and_serialization_peak_included": false,
            "diagnostic_and_output_buffers_included": false,
            "diagnostic_and_output_buffers_included": false,
            "candidate_scene_input_logical_bytes": scene_input_bytes.len(),
            "candidate_profile_logical_bytes": result.profile_bytes.len(),
            "payload_logical_bytes_excluding_this_report": payload_logical_bytes,
        },
        "limits": [
            "synthetic pinned fixture only",
            "flat terrain and WALK adapter only",
            "candidate is not activated and has no Store, permission, or legal workflow integration",
            "not a city-scale performance or 16GB acceptance result",
        ],
    });
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("encode candidate report: {error}"))?;

    fs::create_dir(&output_dir)
        .map_err(|error| format!("create fresh output dir {}: {error}", output_dir.display()))?;
    write_new(
        &output_dir.join("candidate-scene-input.json"),
        &scene_input_bytes,
    )?;
    write_new(
        &output_dir.join("candidate-profile.json"),
        &result.profile_bytes,
    )?;
    write_new(&output_dir.join("candidate-report.json"), &report_bytes)?;

    let scene_len = fs::metadata(output_dir.join("candidate-scene-input.json"))
        .map_err(|error| format!("stat candidate Scene input: {error}"))?
        .len();
    let profile_len = fs::metadata(output_dir.join("candidate-profile.json"))
        .map_err(|error| format!("stat candidate profile: {error}"))?
        .len();
    let report_len = fs::metadata(output_dir.join("candidate-report.json"))
        .map_err(|error| format!("stat candidate report: {error}"))?
        .len();
    let output_dir_json = serde_json::to_string(&output_dir.display().to_string())
        .map_err(|error| format!("encode output path: {error}"))?;
    println!(
        "{{\"result\":\"candidate_only_not_activated\",\"output_dir\":{},\"actual_output_logical_bytes\":{},\"report_logical_bytes\":{},\"elapsed_total_ms\":{}}}",
        output_dir_json,
        scene_len + profile_len + report_len,
        report_len,
        started.elapsed().as_millis(),
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("geometry_revision_candidate: {error}");
        std::process::exit(2);
    }
}
