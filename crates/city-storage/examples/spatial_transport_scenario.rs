use mycity_core::{
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TripPartition,
    },
    spatial_transport_permissions::{
        request_sha256, SpatialAccessEdge, SpatialAccessOperation, SpatialAccessSurface,
        SpatialApprovalRole, SpatialAssetKind, SpatialAssetState, SpatialPermissionCommand,
        SpatialPermissionState, SpatialUpdateApproval, SpatialUpdateRequest, SPATIAL_ALLOW_CLOSE,
        SPATIAL_ALLOW_RESTORE,
    },
    timeline::{Command as TimelineCommand, DAY},
    transaction::{Action, Caller, Envelope},
    transport::{Entrance, Node, OdDemand, Opening, RoadEdge},
    units::{GameTime, Quantity},
};
use mycity_map::{
    geometry::{Facing, Mode, Port},
    probe,
    scene::Scene,
    terrain::import,
    traffic_adapter::{
        adapt, AdapterIds, AdapterInput, FacilityOperation, RoadDirection, TerminalGrant,
        WALK_MODE_MASK,
    },
    Point,
};
use mycity_storage::{spatial_transport_updates::*, Store, WritePoint};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const SPATIAL_FIXTURE: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const HEIGHTMAP_FIXTURE: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
const OPENING_FIXTURE: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SPATIAL_RULES: &str = "m0-spatial-transport-updates-v1";
static TEMP_ID: AtomicU64 = AtomicU64::new(1);

fn allocation() -> TransportAllocationProfile {
    let opening = Opening {
        revision: 1,
        nodes: vec![Node { id: 1, city_id: 1 }, Node { id: 2, city_id: 1 }],
        edges: vec![
            RoadEdge {
                id: 10,
                from: 1,
                to: 2,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: 8,
                mode_mask: 1,
                open: true,
                boundary_source: None,
            },
            RoadEdge {
                id: 11,
                from: 2,
                to: 1,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: 8,
                mode_mask: 1,
                open: true,
                boundary_source: None,
            },
        ],
        entrances: vec![
            Entrance {
                id: 1,
                owner: 1,
                node: 1,
                mode_mask: 1,
                authorized: true,
            },
            Entrance {
                id: 2,
                owner: 2,
                node: 2,
                mode_mask: 1,
                authorized: true,
            },
        ],
        demands: vec![
            OdDemand {
                source_id: 101,
                origin_entrance: 1,
                destination_entrance: 2,
                mode: 1,
                people: 4,
            },
            OdDemand {
                source_id: 102,
                origin_entrance: 2,
                destination_entrance: 1,
                mode: 1,
                people: 4,
            },
        ],
    };
    let assignment = opening.assign().unwrap();
    TransportAllocationProfile {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        market_profile_id: 1,
        period_seconds: DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: None,
        opening,
        assignment,
        partitions: vec![TripPartition {
            quote_id: 1,
            outbound_source: 101,
            return_source: 102,
            trip_count: 1,
            goods_per_trip: Quantity::new(1).unwrap(),
        }],
        fee_policy: FeePolicyInput {
            provider_account: 1,
            cents_per_travel_second: 0,
        },
    }
}

fn raw_verified(profile: &TransportAllocationProfile) -> (Vec<u8>, VerifiedAllocationAsset) {
    let mut raw = b" \n".to_vec();
    raw.extend(serde_json::to_vec(profile).unwrap());
    raw.extend(b"\n \t");
    let hash = Sha256::digest(&raw).into();
    let verified = VerifiedAllocationAsset::from_raw_bytes(&raw, hash, 1, 1, 1).unwrap();
    (raw, verified)
}

fn raw_asset_parent_child_payload_and_historical_proof_binding_are_exact() {
    let baseline = allocation();
    let (_raw_bytes, baseline_asset) = raw_verified(&baseline);
    let baseline_ref = baseline_asset.as_ref();
    let mut changed = baseline.opening.clone();
    changed.revision += 1;
    changed.edges[0].open = false;

    let child_asset = preview_profile_asset(&baseline_ref, changed.clone()).unwrap();
    let mut child = baseline.clone();
    child.opening = changed;
    child.assignment = child.opening.assign().unwrap();
    assert_eq!(
        serde_json::from_slice::<TransportAllocationProfile>(&child_asset.bytes).unwrap(),
        child
    );
    let version = SpatialTransportVersion {
        schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
        request_id: 77,
        asset_id: 5,
        asset_kind: mycity_core::spatial_transport_permissions::SpatialAssetKind::Road,
        asset_revision: 2,
        world_id: 1,
        branch_id: 1,
        surface: mycity_core::spatial_transport_permissions::SpatialAccessSurface::Road,
        edge_id: 10,
        version: 1,
        effective_service_day: 2,
        source_scene_sha256: [7; 32],
        authorization_evidence_sha256: [8; 32],
        previous_profile_sha256: baseline_asset.asset_sha256(),
        profile_sha256: child_asset.sha256,
        profile: child,
    };

    let old_proof = mycity_core::market_access::TransportProofAsset {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        profile_id: 1,
        allocation_profile_sha256: baseline_asset.asset_sha256(),
        service_day: 1,
        period_seconds: DAY as u32,
        generation: 1,
        opening: baseline.opening.clone(),
        assignment: baseline.assignment.clone(),
        partitions: baseline.partitions.clone(),
        fee_policy: baseline.fee_policy,
    };
    assert_eq!(
        profile_for_proof(&baseline_ref, &[version.clone()], &old_proof).unwrap(),
        &baseline
    );
    assert_eq!(
        profile_for_service_day(&baseline_ref, &[version.clone()], 2).unwrap(),
        &version.profile
    );

    let mut mismatch = old_proof;
    mismatch.allocation_profile_sha256 = version.profile_sha256;
    assert!(profile_for_proof(&baseline_ref, std::slice::from_ref(&version), &mismatch).is_err());
    assert_eq!(
        encoded_profile_asset(&version.profile).unwrap().sha256,
        child_asset.sha256
    );
}

fn source_asset_hash_mismatch_is_rejected_before_candidate_building() {
    let profile = allocation();
    let raw = serde_json::to_vec(&profile).unwrap();
    assert_eq!(
        VerifiedAllocationAsset::from_raw_bytes(&raw, [0; 32], 1, 1, 1).err(),
        Some(UpdateError::InvalidSourceAsset)
    );
}

fn scene_profile_and_hash() -> (TransportAllocationProfile, [u8; 32]) {
    let input: probe::Input = serde_json::from_slice(SPATIAL_FIXTURE).unwrap();
    let terrain = import(input.terrain, HEIGHTMAP_FIXTURE, 1, 0).unwrap();
    let scene = Scene::assemble(input.scene, &terrain, &terrain).unwrap();
    let demands = [
        OdDemand {
            source_id: 101,
            origin_entrance: 11,
            destination_entrance: 12,
            mode: WALK_MODE_MASK,
            people: 4,
        },
        OdDemand {
            source_id: 102,
            origin_entrance: 12,
            destination_entrance: 11,
            mode: WALK_MODE_MASK,
            people: 4,
        },
    ];
    let opening = adapt(
        &scene,
        AdapterInput {
            expected_map_id: 1,
            city_id: 1,
            revision: 1,
            building_owner_id: 2,
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
                owner_id: 3,
                entrance_id: 12,
                port: Port {
                    point: Point { x: 1000, y: 1000 },
                    z_cm: 0,
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
            demands: &demands,
        },
    )
    .unwrap();
    let assignment = opening.assign().unwrap();
    let profile = TransportAllocationProfile {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        market_profile_id: 1,
        period_seconds: DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: None,
        opening,
        assignment,
        partitions: vec![TripPartition {
            quote_id: 1,
            outbound_source: 101,
            return_source: 102,
            trip_count: 4,
            goods_per_trip: Quantity::new(10).unwrap(),
        }],
        fee_policy: FeePolicyInput {
            provider_account: 3,
            cents_per_travel_second: 1,
        },
    };
    profile.validate().unwrap();
    let scene_hash: [u8; 32] = hex_decode_32(scene.state_hash().as_str()).unwrap();
    (profile, scene_hash)
}

fn opening_with_market(allocation_hash: [u8; 32]) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(OPENING_FIXTURE).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":100000}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":100}),
    ]);
    value["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":100000,"basis":100000,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":100,"basis":100,"source":7}),
    ]);
    value["town"] = serde_json::json!({
        "schema_version":1,
        "id":1,
        "transport_allocation_sha256":allocation_hash,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":40,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":0}]
    });
    serde_json::to_vec(&value).unwrap()
}

fn permission_state(scene_hash: [u8; 32], profile_hash: [u8; 32]) -> SpatialPermissionState {
    SpatialPermissionState::from_genesis(
        1,
        1,
        vec![
            SpatialAssetState {
                asset_id: 70,
                kind: SpatialAssetKind::Road,
                owner_account: 3,
                permit_authority_account: 4,
                source_scene_sha256: scene_hash,
                revision: 1,
                current_profile_sha256: profile_hash,
                edges: vec![SpatialAccessEdge {
                    edge_id: 24,
                    surface: SpatialAccessSurface::Road,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
                }],
            },
            SpatialAssetState {
                asset_id: 71,
                kind: SpatialAssetKind::Building,
                owner_account: 2,
                permit_authority_account: 4,
                source_scene_sha256: scene_hash,
                revision: 1,
                current_profile_sha256: profile_hash,
                edges: vec![SpatialAccessEdge {
                    edge_id: 22,
                    surface: SpatialAccessSurface::Door,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
                }],
            },
        ],
    )
    .unwrap()
}

fn create_store(path: &std::path::Path) -> (Store, TransportAllocationProfile, [u8; 32]) {
    let (profile, scene_hash) = scene_profile_and_hash();
    let raw = serde_json::to_vec(&profile).unwrap();
    let profile_hash = Sha256::digest(&raw).into();
    let store = Store::create_spatial_with_transport_permissions(
        path,
        &opening_with_market(profile_hash),
        vec![],
        SPATIAL_RULES,
        (SPATIAL_FIXTURE, HEIGHTMAP_FIXTURE),
        &raw,
        permission_state(scene_hash, profile_hash),
    )
    .unwrap();
    (store, profile, scene_hash)
}

#[derive(Clone, Copy)]
enum SpatialCheckpointMutation {
    Owner,
    PermitAuthority,
    CurrentlyOpen,
}

fn rehash_engine_checkpoint(
    world: &std::path::Path,
    mutation: SpatialCheckpointMutation,
) -> Vec<(PathBuf, Vec<u8>)> {
    let current_path = world.join("CURRENT.json");
    let current_bytes = fs::read(&current_path).unwrap();
    let mut current: Value = serde_json::from_slice(&current_bytes).unwrap();
    let slot = current["slot"].as_u64().unwrap();
    let root_path = world.join(format!("root-{slot}.json"));
    let root_bytes = fs::read(&root_path).unwrap();
    let mut root: Value = serde_json::from_slice(&root_bytes).unwrap();
    // Follow the active Root pointer; these worlds may use either checkpoint layout.
    let v2 = root["checkpoint_v2"].is_object();
    let generation = if v2 {
        root["checkpoint_v2"]["generation"].as_u64().unwrap()
    } else {
        root["checkpoint"]["generation"].as_u64().unwrap()
    };
    let manifest_path = if v2 {
        world.join(format!("world-index-{generation}.json"))
    } else {
        world.join(format!("checkpoint-index-{generation}.json"))
    };
    let manifest_bytes = fs::read(&manifest_path).unwrap();
    let mut manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let image_path = if v2 {
        world.join(format!("world-{generation}.json"))
    } else {
        world.join(format!("checkpoint-{generation}.json"))
    };
    let image_bytes = fs::read(&image_path).unwrap();
    let mut image: Value = serde_json::from_slice(&image_bytes).unwrap();
    // V2 stores the Engine image directly. The retained v1 checkpoint wraps the Engine's
    // serialized JSON bytes in its `engine` byte array; mutate that authoritative payload.
    let mut engine: Value = if v2 {
        image.clone()
    } else {
        let encoded = image["engine"].as_array().unwrap();
        let bytes: Vec<u8> = encoded
            .iter()
            .map(|byte| u8::try_from(byte.as_u64().unwrap()).unwrap())
            .collect();
        serde_json::from_slice(&bytes).unwrap()
    };
    let permission = engine.get_mut("spatial_permissions").unwrap();
    let before_revision = permission["revision"].clone();
    let before_profile_hash = permission["assets"][0]["current_profile_sha256"].clone();
    match mutation {
        SpatialCheckpointMutation::Owner => {
            let owner = permission["assets"][0]["owner_account"].as_u64().unwrap();
            permission["assets"][0]["owner_account"] =
                serde_json::json!(if owner == 2 { 3 } else { 2 });
        }
        SpatialCheckpointMutation::PermitAuthority => {
            let authority = permission["assets"][0]["permit_authority_account"]
                .as_u64()
                .unwrap();
            permission["assets"][0]["permit_authority_account"] =
                serde_json::json!(if authority == 1 { 4 } else { 1 });
        }
        SpatialCheckpointMutation::CurrentlyOpen => {
            let edge = &mut permission["assets"][0]["edges"][0]["currently_open"];
            *edge = serde_json::json!(!edge.as_bool().unwrap());
        }
    }
    assert_eq!(permission["revision"], before_revision);
    assert_eq!(
        permission["assets"][0]["current_profile_sha256"],
        before_profile_hash
    );
    let engine_bytes = serde_json::to_vec(&engine).unwrap();
    if v2 {
        image = engine;
    } else {
        image["engine"] = serde_json::json!(engine_bytes);
    }
    let catalog_pointer = root["spatial_transport_catalog_sha256"].clone();
    let root_sequence = root["sequence"].clone();

    let image_bytes = serde_json::to_vec(&image).unwrap();
    if v2 {
        manifest["current"]["bytes"] = serde_json::json!(image_bytes.len() as u64);
        manifest["current"]["sha256"] = serde_json::json!(mycity_core::sha256(&image_bytes));
    } else {
        manifest["data_sha256"] = serde_json::json!(mycity_core::sha256(&image_bytes));
    }
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    if v2 {
        root["checkpoint_v2"]["sha256"] = serde_json::json!(mycity_core::sha256(&manifest_bytes));
    } else {
        root["checkpoint"]["sha256"] = serde_json::json!(mycity_core::sha256(&manifest_bytes));
    }
    let root_bytes = serde_json::to_vec(&root).unwrap();
    current["root_sha256"] = serde_json::json!(mycity_core::sha256(&root_bytes));

    let originals = vec![
        (current_path.clone(), current_bytes),
        (root_path.clone(), fs::read(&root_path).unwrap()),
        (manifest_path.clone(), fs::read(&manifest_path).unwrap()),
        (image_path.clone(), fs::read(&image_path).unwrap()),
    ];
    fs::write(&image_path, image_bytes).unwrap();
    fs::write(&manifest_path, manifest_bytes).unwrap();
    fs::write(&root_path, root_bytes).unwrap();
    fs::write(&current_path, serde_json::to_vec(&current).unwrap()).unwrap();

    let persisted_root: Value = serde_json::from_slice(&fs::read(&root_path).unwrap()).unwrap();
    assert_eq!(
        persisted_root["spatial_transport_catalog_sha256"],
        catalog_pointer
    );
    assert_eq!(persisted_root["sequence"], root_sequence);
    originals
}

fn assert_spatial_checkpoint_tamper_rejected(
    world: &std::path::Path,
    mutation: SpatialCheckpointMutation,
) {
    let original_files = rehash_engine_checkpoint(world, mutation);
    let result = Store::open(world, SPATIAL_RULES);
    let error = format!(
        "{:#?}",
        result
            .err()
            .expect("tampered Engine checkpoint must be rejected")
    );
    assert!(
        error.contains("spatial_catalog_engine_binding"),
        "unexpected cold-open error: {error}"
    );
    for (path, bytes) in original_files {
        fs::write(path, bytes).unwrap();
    }
}

fn run_action(store: &mut Store, caller: Caller, action: Action) -> mycity_storage::Outcome {
    let view = store.engine().unwrap().world().view();
    store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: store.committed_sequence() + 1,
                expected_revision: view.revision,
                actions: vec![action],
            },
        )
        .unwrap()
}

fn advance_to_day(store: &mut Store, target_day: u64) {
    if store.engine().unwrap().timeline().is_none() {
        let outcome = run_action(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Enable,
            },
        );
        assert!(outcome.model.is_ok());
    }
    let target = target_day * DAY;
    for _ in 0..128 {
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .last_complete
            == target
        {
            return;
        }
        let outcome = run_action(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        assert!(outcome.model.is_ok());
    }
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .last_complete,
        target
    );
}

// Keep each independent actor, asset, edge, day, and approval choice explicit in the CLI
// scenario matrix so the same request path exercises road/door and approval variants.
#[allow(clippy::too_many_arguments)]
fn submit_request(
    store: &mut Store,
    world: &std::path::Path,
    asset_id: u32,
    operation: SpatialAccessOperation,
    edge_id: u32,
    surface: SpatialAccessSurface,
    applicant: u32,
    executor: u32,
    owner: u32,
    authority: u32,
    service_day: u64,
    approve_authority: bool,
) -> (SpatialUpdateRequest, [u8; 32]) {
    let (permission_revision, request_id, parent, asset_revision, authorization_context) = {
        let permission = store.engine().unwrap().spatial_permissions().unwrap();
        let asset = permission
            .assets()
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .unwrap();
        (
            permission.revision,
            permission.request_id_highwater + 1,
            asset.current_profile_sha256,
            asset.revision,
            permission.authorization_context(asset_id).unwrap(),
        )
    };
    let view = store.engine().unwrap().world().view();
    let market = view.town.as_ref().unwrap();
    let current = VerifiedAllocationAsset::read_immutable(
        world,
        parent,
        view.scope.world(),
        view.scope.branch(),
        market.profile.id,
    )
    .unwrap();
    let mut proposed = current.profile().opening.clone();
    proposed.revision += 1;
    let target = proposed
        .edges
        .iter_mut()
        .find(|edge| edge.id == edge_id)
        .unwrap();
    target.open = operation == SpatialAccessOperation::Restore;
    let candidate = preview_profile_asset(&current.as_ref(), proposed.clone()).unwrap();
    let request = SpatialUpdateRequest {
        request_id,
        applicant_account: applicant,
        executor_account: executor,
        asset_id,
        operation,
        surface,
        edge_id,
        expected_revision: asset_revision,
        candidate_profile_sha256: candidate.sha256,
        effective_time: GameTime::new((service_day * DAY) as i64).unwrap(),
        parent_profile_sha256: parent,
    };
    let before_hash = store.state_hash().unwrap();
    let before_sequence = store.committed_sequence();
    let mut forged = request;
    forged.candidate_profile_sha256[0] ^= 1;
    let view = store.engine().unwrap().world().view();
    let forged_envelope = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: before_sequence + 1,
        expected_revision: view.revision,
        actions: vec![Action::SpatialPermission {
            command: SpatialPermissionCommand::Request {
                expected_state_revision: permission_revision,
                request: forged,
            },
        }],
    };
    assert!(store
        .execute(Caller::Account(applicant), forged_envelope)
        .is_err());
    assert_eq!(store.committed_sequence(), before_sequence);
    assert_eq!(store.state_hash().unwrap(), before_hash);
    let staged = store
        .stage_spatial_transport_candidate(request, proposed)
        .unwrap();
    assert_eq!(staged, candidate.sha256);
    let digest = request_sha256(authorization_context, &request).unwrap();
    let outcome = run_action(
        store,
        Caller::Account(applicant),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Request {
                expected_state_revision: permission_revision,
                request,
            },
        },
    );
    assert!(outcome.model.is_ok());
    let owner_revision = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .revision;
    let owner_approval = SpatialUpdateApproval {
        request_id: request.request_id,
        request_sha256: digest,
        approver_account: owner,
        role: SpatialApprovalRole::OwnerConsent,
        approved: true,
    };
    assert!(run_action(
        store,
        Caller::Account(owner),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Approve {
                expected_state_revision: owner_revision,
                approval: owner_approval,
            },
        }
    )
    .model
    .is_ok());
    if approve_authority {
        let authority_revision = store
            .engine()
            .unwrap()
            .spatial_permissions()
            .unwrap()
            .revision;
        let authority_approval = SpatialUpdateApproval {
            request_id: request.request_id,
            request_sha256: digest,
            approver_account: authority,
            role: SpatialApprovalRole::AuthorityPermit,
            approved: true,
        };
        assert!(run_action(
            store,
            Caller::Account(authority),
            Action::SpatialPermission {
                command: SpatialPermissionCommand::Approve {
                    expected_state_revision: authority_revision,
                    approval: authority_approval,
                },
            }
        )
        .model
        .is_ok());
    }
    (request, digest)
}

fn make_apply_envelope(store: &Store, request_id: u64, digest: [u8; 32]) -> Envelope {
    let permission = store.engine().unwrap().spatial_permissions().unwrap();
    let state_revision = permission.revision;
    let view = store.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![Action::SpatialPermission {
            command: SpatialPermissionCommand::Apply {
                expected_state_revision: state_revision,
                request_id,
                request_sha256: digest,
            },
        }],
    }
}

fn apply_request(store: &mut Store, request_id: u64, digest: [u8; 32], executor: u32) -> Envelope {
    let envelope = make_apply_envelope(store, request_id, digest);
    let outcome = store
        .execute(Caller::Account(executor), envelope.clone())
        .unwrap();
    assert!(outcome.model.is_ok());
    envelope
}

fn settle(store: &mut Store, world: &std::path::Path, day: u64) -> (i64, [u8; 32], u32, u32) {
    advance_to_day(store, day);
    let operation_id = store.committed_sequence() + 1;
    let outcome = store.settle_town_market(operation_id).unwrap();
    assert!(outcome.has_persistent_receipt);
    assert!(outcome.model.is_ok());
    let view = store.engine().unwrap().world().view();
    let market = view.town.unwrap();
    let settled = market.state.last_transport.unwrap();
    let proof_path = world.join(format!(
        "town-proof-{}.json",
        hex(&settled.issued_proof_sha256)
    ));
    let proof_bytes = fs::read(proof_path).unwrap();
    let proof: mycity_core::market_access::TransportProofAsset =
        serde_json::from_slice(&proof_bytes).unwrap();
    assert_eq!(proof.service_day, day);
    let served = |source_id| {
        proof
            .assignment
            .demand_outcomes
            .iter()
            .find(|entry| entry.source_id == source_id)
            .unwrap()
            .served_people
    };
    (
        market.state.last_summary.actual_sales.value(),
        settled.issued_proof_sha256,
        served(101),
        served(102),
    )
}

fn same_store_dual_approved_road_and_door_updates_change_trade_and_recover() {
    let temp = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&temp).unwrap();
    let world = temp.join(format!(
        "mycity-spatial-update-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let (mut store, _baseline, _) = create_store(&world);

    let (sales1, proof1, out1, ret1) = settle(&mut store, &world, 1);
    assert_eq!((sales1, out1, ret1), (40, 4, 4));
    let (close_road, road_digest) = submit_request(
        &mut store,
        &world,
        70,
        SpatialAccessOperation::Close,
        24,
        SpatialAccessSurface::Road,
        2,
        1,
        3,
        4,
        2,
        true,
    );
    advance_to_day(&mut store, 2);
    let close_envelope = apply_request(&mut store, close_road.request_id, road_digest, 1);
    store.checkpoint().unwrap();
    let (sales2, proof2, out2, ret2) = settle(&mut store, &world, 2);
    assert_eq!((sales2, out2, ret2), (0, 0, 4));
    assert_ne!(proof1, proof2);

    let after_close_hash = store.state_hash().unwrap();
    let after_close_sequence = store.committed_sequence();
    let stale_candidate = VerifiedAllocationAsset::read_immutable(
        &world,
        close_road.candidate_profile_sha256,
        1,
        1,
        1,
    )
    .unwrap();
    assert!(store
        .stage_spatial_transport_candidate(close_road, stale_candidate.profile().opening.clone())
        .is_err());
    assert_eq!(store.committed_sequence(), after_close_sequence);
    assert_eq!(store.state_hash().unwrap(), after_close_hash);

    let (restore_road, restore_digest) = submit_request(
        &mut store,
        &world,
        70,
        SpatialAccessOperation::Restore,
        24,
        SpatialAccessSurface::Road,
        2,
        1,
        3,
        4,
        3,
        true,
    );
    advance_to_day(&mut store, 3);
    let _restore_envelope = apply_request(&mut store, restore_road.request_id, restore_digest, 1);
    let (sales3, proof3, out3, ret3) = settle(&mut store, &world, 3);
    assert_eq!((sales3, out3, ret3), (40, 4, 4));
    assert_ne!(proof2, proof3);

    let (close_door, door_digest) = submit_request(
        &mut store,
        &world,
        71,
        SpatialAccessOperation::Close,
        22,
        SpatialAccessSurface::Door,
        3,
        1,
        2,
        4,
        4,
        false,
    );
    advance_to_day(&mut store, 4);
    let before_denied = store.state_hash().unwrap();
    let denied_envelope = make_apply_envelope(&store, close_door.request_id, door_digest);
    let denied = store.execute(Caller::Account(1), denied_envelope).unwrap();
    assert!(denied.has_persistent_receipt);
    assert!(denied.model.is_err());
    assert_eq!(store.state_hash().unwrap(), before_denied);
    let permission = store.engine().unwrap().spatial_permissions().unwrap();
    assert!(permission
        .requests
        .iter()
        .any(|record| record.request.request_id == close_door.request_id));
    let authority_approval = SpatialUpdateApproval {
        request_id: close_door.request_id,
        request_sha256: door_digest,
        approver_account: 4,
        role: SpatialApprovalRole::AuthorityPermit,
        approved: true,
    };
    let authority_revision = permission.revision;
    assert!(run_action(
        &mut store,
        Caller::Account(4),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Approve {
                expected_state_revision: authority_revision,
                approval: authority_approval,
            },
        }
    )
    .model
    .is_ok());
    let apply_door = make_apply_envelope(&store, close_door.request_id, door_digest);
    let mut reached_applied = false;
    let lost_reply = store.execute_with_observer(Caller::Account(1), apply_door.clone(), |point| {
        if point == WritePoint::Applied {
            reached_applied = true;
            Err(io::Error::new(
                io::ErrorKind::Other,
                "injected lost reply after Applied",
            ))
        } else {
            Ok(())
        }
    });
    assert!(lost_reply.is_err());
    assert!(reached_applied);
    drop(store);
    let mut store = Store::open(&world, SPATIAL_RULES).unwrap();
    let committed_revision = {
        let committed = store
            .engine()
            .unwrap()
            .journal()
            .iter()
            .find(|entry| entry.operation_id == apply_door.operation_id)
            .unwrap();
        assert!(
            committed.outcome.is_ok(),
            "lost reply must follow successful business apply"
        );
        committed.outcome.as_ref().unwrap().revision
    };
    let replay = store
        .execute(Caller::Account(1), apply_door.clone())
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok());
    let (sales4, proof4, out4, ret4) = settle(&mut store, &world, 4);
    assert_eq!((sales4, out4, ret4), (0, 4, 0));
    assert_ne!(proof3, proof4);

    let (restore_door, door_restore_digest) = submit_request(
        &mut store,
        &world,
        71,
        SpatialAccessOperation::Restore,
        22,
        SpatialAccessSurface::Door,
        3,
        1,
        2,
        4,
        5,
        true,
    );
    advance_to_day(&mut store, 5);
    let _apply_restore = apply_request(&mut store, restore_door.request_id, door_restore_digest, 1);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .stocks
            .iter()
            .find(|stock| stock.id == 3)
            .unwrap()
            .quantity
            .value(),
        20,
        "only 20 funded units remain before day-5 trade"
    );
    let (sales5, proof5, out5, ret5) = settle(&mut store, &world, 5);
    assert_eq!((sales5, out5, ret5), (20, 4, 4));
    assert_ne!(proof4, proof5);
    assert_eq!(sales1 + sales2 + sales3 + sales4 + sales5, 100);

    let state_hash = store.state_hash().unwrap();
    let owned_before_cold = store.owned_capacity_bytes().unwrap();
    store.checkpoint().unwrap();
    let current_pointer: Value =
        serde_json::from_slice(&fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
    let root_slot = current_pointer["slot"].as_u64().unwrap();
    let root_bytes = fs::read(world.join(format!("root-{root_slot}.json"))).unwrap();
    let root: Value = serde_json::from_slice(&root_bytes).unwrap();
    let root_sha256 = current_pointer["root_sha256"].as_str().unwrap().to_owned();
    let catalog_hash: [u8; 32] =
        serde_json::from_value(root["spatial_transport_catalog_sha256"].clone()).unwrap();
    let catalog = read_spatial_transport_catalog(&world, catalog_hash).unwrap();
    let active_profile_sha256 = catalog.profile_sha256_for_service_day(5).unwrap();
    assert_eq!(catalog.versions().len(), 4);
    let final_view_ref = store.engine().unwrap().world().view();
    assert_eq!(
        final_view_ref
            .stocks
            .iter()
            .find(|stock| stock.id == 3)
            .unwrap()
            .quantity
            .value(),
        0
    );
    let final_view = serde_json::to_value(final_view_ref).unwrap();
    let final_accounts = final_view["accounts"].clone();
    let final_stocks = final_view["stocks"].clone();
    for proof_hash in [proof1, proof2, proof3, proof4, proof5] {
        let old_proof = world.join(format!("town-proof-{}.json", hex(&proof_hash)));
        assert!(
            old_proof.is_file(),
            "historical proof bytes must remain available"
        );
    }
    drop(store);
    let mut cold = Store::open(&world, SPATIAL_RULES).unwrap();
    assert_eq!(cold.state_hash().unwrap(), state_hash);
    assert!(cold.owned_capacity_bytes().unwrap() > 0);
    let replay = cold.execute(Caller::Account(1), close_envelope).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok());
    assert_eq!(cold.state_hash().unwrap(), state_hash);
    let apply_door_operation_id = apply_door.operation_id;
    let replay_door = cold.execute(Caller::Account(1), apply_door).unwrap();
    assert!(replay_door.has_persistent_receipt);
    assert!(replay_door.model.is_ok());
    assert_eq!(cold.state_hash().unwrap(), state_hash);
    drop(cold);
    for mutation in [
        SpatialCheckpointMutation::Owner,
        SpatialCheckpointMutation::PermitAuthority,
        SpatialCheckpointMutation::CurrentlyOpen,
    ] {
        assert_spatial_checkpoint_tamper_rejected(&world, mutation);
    }
    let cold = Store::open(&world, SPATIAL_RULES).unwrap();
    assert_eq!(cold.state_hash().unwrap(), state_hash);
    eprintln!(
        "SPATIAL_RUNTIME_UPDATE_METRICS={}",
        serde_json::json!({
            "scope":"same_store_three_node_single_map_walk_microfixture_not_scale_performance",
            "world":world.display().to_string(),
            "service_days":[1,2,3,4,5],
            "sales":[sales1,sales2,sales3,sales4,sales5],
            "proof_hashes":[hex(&proof1),hex(&proof2),hex(&proof3),hex(&proof4),hex(&proof5)],
            "served":[[out1,ret1],[out2,ret2],[out3,ret3],[out4,ret4],[out5,ret5]],
            "store_owned_bytes_before_cold":owned_before_cold,
            "store_owned_bytes_after_cold":cold.owned_capacity_bytes().unwrap(),
            "store_sequence":cold.committed_sequence(),
            "apply_operation_id":apply_door_operation_id,
            "receipt_revision":committed_revision,
            "root_sha256":root_sha256,
            "catalog_sha256":hex(&catalog_hash),
            "active_profile_sha256_day5":hex(&active_profile_sha256),
            "catalog_sidecar_bytes":fs::metadata(world.join(format!("{}{}.json", SPATIAL_TRANSPORT_CATALOG_PREFIX, hex(&catalog_hash)))).unwrap().len(),
            "accounts_after_day5":final_accounts,
            "stocks_after_day5":final_stocks,
            "world_logical_file_bytes_after_replay":logical_directory_bytes(&world),
            "final_state_hash":state_hash
        })
    );
    if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_none() && !std::thread::panicking() {
        let _ = fs::remove_dir_all(&world);
    } else {
        eprintln!(
            "SPATIAL_RUNTIME_UPDATE_ARTIFACT_PRESERVED={}",
            world.display()
        );
    }
}

fn spatial_permission_backing_survives_present_absent_present_restore() {
    let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&root).unwrap();
    let world = root.join(format!(
        "mycity-spatial-permission-slot-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let (store, _, _) = create_store(&world);
    let with_permissions = store.engine().unwrap().export_checkpoint().unwrap();
    drop(store);

    let mut legacy: Value = serde_json::from_slice(&with_permissions).unwrap();
    assert!(legacy
        .as_object_mut()
        .unwrap()
        .remove("spatial_permissions")
        .is_some());
    let without_permissions = serde_json::to_vec(&legacy).unwrap();
    let mut slot = mycity_core::transaction::EngineSlot::new().unwrap();
    slot.restore_into(&with_permissions).unwrap();
    let present_identity = slot.identity().unwrap();
    assert!(slot.engine().unwrap().spatial_permissions().is_some());
    slot.restore_into(&without_permissions).unwrap();
    assert!(slot.engine().unwrap().spatial_permissions().is_none());
    let truncated = &with_permissions[..with_permissions.len() - 1];
    assert!(slot.restore_into(truncated).is_err());
    assert!(slot.engine().is_err());
    slot.restore_into(&with_permissions).unwrap();
    assert!(slot.engine().unwrap().spatial_permissions().is_some());
    assert_eq!(slot.identity().unwrap(), present_identity);
    if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_none() {
        let _ = fs::remove_dir_all(world);
    }
}

fn hex(digest: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}

fn hex_decode_32(input: &str) -> Option<[u8; 32]> {
    if input.len() != 64 {
        return None;
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&input[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn logical_directory_bytes(root: &std::path::Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            total += logical_directory_bytes(&entry.path());
        } else if metadata.is_file() {
            total += metadata.len();
        }
    }
    total
}

fn main() {
    if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_none() {
        eprintln!(
            "Set MYCITY_TEST_ARTIFACT_ROOT to a preserved evidence directory before running."
        );
        std::process::exit(2);
    }
    eprintln!(
        "Scenario scope: 3-node single-map WALK fixture; microfunctional evidence only, not city-scale performance acceptance."
    );
    raw_asset_parent_child_payload_and_historical_proof_binding_are_exact();
    source_asset_hash_mismatch_is_rejected_before_candidate_building();
    same_store_dual_approved_road_and_door_updates_change_trade_and_recover();
    spatial_permission_backing_survives_present_absent_present_restore();
}
