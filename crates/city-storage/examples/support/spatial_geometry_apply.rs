//! Shared executable microfixture for the geometry-apply integration test and CLI.
//! The account IDs below are fixture identities, not player authentication or security proof.
use mycity_core::{
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TripPartition,
    },
    spatial_geometry_permissions::SpatialGeometryCommand,
    spatial_transport_permissions::{
        request_sha256, SpatialAccessEdge, SpatialAccessOperation, SpatialAccessSurface,
        SpatialApprovalRole, SpatialAssetKind, SpatialAssetState, SpatialPermissionCommand,
        SpatialPermissionState, SpatialUpdateApproval, SpatialUpdateRequest, SPATIAL_ALLOW_CLOSE,
        SPATIAL_ALLOW_RESTORE,
    },
    timeline::{Command as TimelineCommand, DAY},
    transaction::{Action, Caller, Envelope},
    transport::OdDemand,
    units::{GameTime, Quantity},
};
use mycity_map::{
    geometry::{Facing, Mode, Port},
    probe,
    scene::Scene,
    terrain::import,
    traffic_adapter::{
        adapt, AdapterIds, FacilityOperation, RoadDirection, TerminalGrant, WALK_MODE_MASK,
    },
    Point,
};
use mycity_storage::{
    spatial_geometry_updates::SpatialGeometryAdapterBasis, spatial_transport_updates::*, Store,
    WritePoint,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, io};

const SPATIAL_FIXTURE: &[u8] = include_bytes!("../../../../fixtures/m0-spatial.json");
const HEIGHTMAP_FIXTURE: &[u8] = include_bytes!("../../../../fixtures/terrain/m0-flat16.png");
const OPENING_FIXTURE: &[u8] = include_bytes!("../../../../fixtures/m0-e0-opening.json");
const SPATIAL_RULES: &str = "m0-spatial-transport-updates-v1";

fn geometry_adapter_basis(scene: &Scene) -> SpatialGeometryAdapterBasis {
    let demands = vec![
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
    SpatialGeometryAdapterBasis {
        schema: mycity_storage::spatial_geometry_updates::ADAPTER_BASIS_SCHEMA,
        world_id: scene.input().map.world,
        branch_id: scene.input().map.branch,
        profile_id: 1,
        expected_map_id: scene.input().map.map,
        city_id: 1,
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
        demands,
    }
}

fn scene_profile_and_hash() -> (TransportAllocationProfile, [u8; 32]) {
    let input: probe::Input = serde_json::from_slice(SPATIAL_FIXTURE).unwrap();
    let terrain = import(input.terrain, HEIGHTMAP_FIXTURE, 1, 0).unwrap();
    let scene = Scene::assemble(input.scene, &terrain, &terrain).unwrap();
    let basis = geometry_adapter_basis(&scene);
    let opening = adapt(&scene, basis.adapter_input(1)).unwrap();
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
    // The explicit geometry adapter basis names city 1; genesis must register that city.
    value["region"] = serde_json::json!({
        "schema": "mycity-m0-region-v1",
        "active_city": 1,
        "cities": [{
            "id": 1,
            "development_elapsed": 0,
            "population": 100,
            "willing": 0,
            "housing_capacity": 4000
        }],
        "routes": [],
        "obligations": []
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

pub fn run(world: &std::path::Path) {
    assert!(
        !world.exists(),
        "refusing to overwrite an existing Store directory: {}",
        world.display()
    );
    if let Some(parent) = world.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let world = world.to_path_buf();

    let source: probe::Input = serde_json::from_slice(SPATIAL_FIXTURE).unwrap();
    let terrain = import(source.terrain.clone(), HEIGHTMAP_FIXTURE, 1, 0).unwrap();
    let scene = Scene::assemble(source.scene.clone(), &terrain, &terrain).unwrap();
    let basis = geometry_adapter_basis(&scene);
    let mut baseline = scene_profile_and_hash().0;
    baseline.fee_policy.cents_per_travel_second = 10;
    let baseline_bytes = serde_json::to_vec(&baseline).unwrap();
    let baseline_profile_sha256: [u8; 32] = Sha256::digest(&baseline_bytes).into();
    let genesis_scene_sha256 = hex_decode_32(scene.state_hash().as_str()).unwrap();
    let mut store = Store::create_spatial_with_geometry_basis(
        &world,
        &opening_with_market(baseline_profile_sha256),
        vec![],
        SPATIAL_RULES,
        (SPATIAL_FIXTURE, HEIGHTMAP_FIXTURE),
        &baseline_bytes,
        permission_state(genesis_scene_sha256, baseline_profile_sha256),
        basis,
    )
    .unwrap();

    let genesis_scene_state = store.spatial().unwrap().unwrap().scene().state_hash();
    let (sales_day1, proof_day1_sha256, out_day1, back_day1) = settle(&mut store, &world, 1);
    assert_eq!((sales_day1, out_day1, back_day1), (40, 4, 4));
    let proof_day1_bytes =
        fs::read(world.join(format!("town-proof-{}.json", hex(&proof_day1_sha256)))).unwrap();
    let proof_day1: mycity_core::market_access::TransportProofAsset =
        serde_json::from_slice(&proof_day1_bytes).unwrap();
    assert_eq!(proof_day1.service_day, 1);
    assert_eq!(
        proof_day1.allocation_profile_sha256,
        baseline_profile_sha256
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .as_ref()
            .unwrap()
            .state
            .last_summary
            .freight
            .value(),
        6_480
    );

    let descriptor_sha256 = store
        .stage_road_geometry_candidate(70, Point { x: 3000, y: 1800 }, 2)
        .unwrap();
    let descriptor =
        mycity_storage::spatial_geometry_updates::read_descriptor(&world, descriptor_sha256)
            .unwrap();
    assert_eq!(descriptor.parent_profile_sha256, baseline_profile_sha256);
    assert_eq!(descriptor.parent_geometry_revision, 1);
    assert_eq!(descriptor.candidate_geometry_revision, 2);
    assert_eq!(descriptor.effective_service_day, 2);
    assert_eq!(
        store.spatial().unwrap().unwrap().scene().state_hash(),
        genesis_scene_state
    );
    let candidate_profile_sha256 = descriptor.candidate_profile_sha256;

    let geometry_revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .map_or(0, |state| state.revision());
    assert_eq!(geometry_revision, 0);
    let request_id = 1;
    let request_action = Action::SpatialGeometry {
        command: SpatialGeometryCommand::Request {
            expected_state_revision: geometry_revision,
            request_id,
            executor_account: 1,
            asset_id: 70,
            expected_geometry_revision: descriptor.parent_geometry_revision,
            expected_asset_revision: 1,
            descriptor_sha256,
        },
    };
    assert!(run_action(&mut store, Caller::Account(2), request_action)
        .model
        .is_ok());

    // Applying before the two independent approvals is a real rejected command and must leave
    // both the active Scene and the service-day profile unchanged.
    let missing_approval_scene = store.spatial().unwrap().unwrap().scene().state_hash();
    let missing_approval = run_action(
        &mut store,
        Caller::Account(1),
        Action::SpatialGeometry {
            command: SpatialGeometryCommand::Apply {
                expected_state_revision: 1,
                request_id,
                descriptor_sha256,
                candidate_profile_sha256,
            },
        },
    );
    assert!(missing_approval.has_persistent_receipt);
    assert!(missing_approval.model.is_err());
    assert_eq!(
        store.spatial().unwrap().unwrap().scene().state_hash(),
        missing_approval_scene
    );

    for (expected_state_revision, account) in [(1, 3), (2, 4)] {
        let approval = run_action(
            &mut store,
            Caller::Account(account),
            Action::SpatialGeometry {
                command: SpatialGeometryCommand::Approve {
                    expected_state_revision,
                    request_id,
                    descriptor_sha256,
                    approved: true,
                },
            },
        );
        assert!(approval.model.is_ok());
    }
    assert_eq!(
        store
            .engine()
            .unwrap()
            .spatial_geometry()
            .unwrap()
            .revision(),
        3
    );

    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .last_complete,
        DAY
    );
    let view = store.engine().unwrap().world().view();
    let apply_envelope = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![Action::SpatialGeometry {
            command: SpatialGeometryCommand::Apply {
                expected_state_revision: 3,
                request_id,
                descriptor_sha256,
                candidate_profile_sha256,
            },
        }],
    };
    let mut reached_applied = false;
    let sequence_before_lost_reply = store.committed_sequence();
    let lost_reply =
        store.execute_with_observer(Caller::Account(1), apply_envelope.clone(), |point| {
            if point == WritePoint::Applied {
                reached_applied = true;
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "injected lost geometry-apply reply after Applied",
                ))
            } else {
                Ok(())
            }
        });
    let sequence_after_lost_reply = store.committed_sequence();
    let lost_reply_summary = match &lost_reply {
        Ok(outcome) => format!(
            "Ok(model={:?}, persistent_receipt={}, outcome_sequence={})",
            outcome.model, outcome.has_persistent_receipt, outcome.committed_sequence
        ),
        Err(error) => format!("Err({error:?})"),
    };
    let apply_receipt_summary = match store.engine() {
        Ok(engine) => engine
            .journal()
            .iter()
            .find(|record| record.operation_id == apply_envelope.operation_id)
            .map(|record| {
                format!(
                    "operation={} outcome={:?}",
                    record.operation_id, record.outcome
                )
            })
            .unwrap_or_else(|| "no matching Engine journal receipt".to_owned()),
        Err(error) => format!("Engine journal unavailable: {error:?}"),
    };
    assert!(
        lost_reply.is_err(),
        "expected injected lost reply; reached_applied={reached_applied}, result={lost_reply_summary}, apply_receipt={apply_receipt_summary}, committed_sequence={sequence_before_lost_reply}->{sequence_after_lost_reply}"
    );
    assert!(
        reached_applied,
        "the geometry command must commit before reply loss; result={lost_reply_summary}, apply_receipt={apply_receipt_summary}, committed_sequence={sequence_before_lost_reply}->{sequence_after_lost_reply}"
    );
    assert_eq!(sequence_after_lost_reply, sequence_before_lost_reply + 1);
    let apply_operation_id = apply_envelope.operation_id;
    // An observer error after durable publication poisons the current handle. Recover through
    // the normal Store open path before advancing or settling the candidate's effective day.
    drop(store);
    let mut store = Store::open(&world, SPATIAL_RULES).unwrap();
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .last_complete,
        DAY
    );
    let recovered_apply = store
        .engine()
        .unwrap()
        .journal()
        .iter()
        .find(|record| record.operation_id == apply_operation_id)
        .unwrap();
    assert!(recovered_apply.outcome.is_ok());

    // Apply while the completed day is 1; the candidate is authorized for day 2.
    // Advancing first would make its effective day stale (it must be completed_day + 1).
    advance_to_day(&mut store, 2);
    let active_scene = store.spatial().unwrap().unwrap().scene().state_hash();
    assert_ne!(active_scene, genesis_scene_state);
    assert_eq!(
        store
            .spatial_scene_for_service_day(1)
            .unwrap()
            .unwrap()
            .state_hash(),
        genesis_scene_state
    );
    assert_eq!(
        store
            .spatial_scene_for_service_day(2)
            .unwrap()
            .unwrap()
            .state_hash(),
        hex(&descriptor.candidate_scene_state_sha256)
    );

    let (sales_day2, proof_day2_sha256, out_day2, back_day2) = settle(&mut store, &world, 2);
    assert_eq!((sales_day2, out_day2, back_day2), (40, 4, 4));
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .as_ref()
            .unwrap()
            .state
            .last_summary
            .freight
            .value(),
        6_560
    );
    let proof_day2_bytes =
        fs::read(world.join(format!("town-proof-{}.json", hex(&proof_day2_sha256)))).unwrap();
    let proof_day2: mycity_core::market_access::TransportProofAsset =
        serde_json::from_slice(&proof_day2_bytes).unwrap();
    assert_eq!(proof_day2.service_day, 2);
    assert_eq!(
        proof_day2.allocation_profile_sha256,
        candidate_profile_sha256
    );
    assert_ne!(
        proof_day1.assignment.demand_outcomes[0].mean_travel_seconds,
        proof_day2.assignment.demand_outcomes[0].mean_travel_seconds
    );

    let committed_hash = store.state_hash().unwrap();
    let committed_sequence = store.committed_sequence();
    store.checkpoint().unwrap();
    drop(store);

    let mut cold = Store::open(&world, SPATIAL_RULES).unwrap();
    assert_eq!(cold.state_hash().unwrap(), committed_hash);
    assert_eq!(cold.committed_sequence(), committed_sequence);
    assert_eq!(
        cold.spatial().unwrap().unwrap().scene().state_hash(),
        active_scene
    );
    let state_before_retry = cold.state_hash().unwrap();
    let sequence_before_retry = cold.committed_sequence();
    // Checkpointing can evict the operation from the bounded hot Engine journal. Query the
    // Store's persistent receipt archive for the original full record instead.
    let apply_record = cold.receipt(apply_operation_id).unwrap().unwrap();
    let original_apply_record_bytes = serde_json::to_vec(&apply_record).unwrap();
    assert_eq!(apply_record.operation_id, apply_operation_id);
    assert_eq!(apply_record.actor, 1);
    assert_eq!(
        apply_record.expected_revision,
        apply_envelope.expected_revision
    );
    let expected_apply_payload_sha256: [u8; 32] =
        Sha256::digest(serde_json::to_vec(&apply_envelope).unwrap()).into();
    assert_eq!(apply_record.payload_sha256, expected_apply_payload_sha256);
    assert!(apply_record.outcome.is_ok());
    let exact_retry = cold
        .execute(Caller::Account(1), apply_envelope.clone())
        .unwrap();
    assert!(exact_retry.has_persistent_receipt);
    assert!(exact_retry.model.is_ok());
    assert_eq!(exact_retry.model, apply_record.outcome);
    assert_eq!(exact_retry.committed_sequence, sequence_before_retry);
    assert_eq!(cold.committed_sequence(), sequence_before_retry);
    assert_eq!(cold.state_hash().unwrap(), state_before_retry);
    assert_eq!(state_before_retry, committed_hash);

    // Reusing the committed operation ID with altered payload must conflict rather than
    // reapplying the geometry, changing the active profile, or overwriting the old proofs.
    let mut changed_retry = apply_envelope;
    if let Action::SpatialGeometry {
        command:
            SpatialGeometryCommand::Apply {
                candidate_profile_sha256,
                ..
            },
    } = &mut changed_retry.actions[0]
    {
        candidate_profile_sha256[0] ^= 1;
    } else {
        unreachable!("fixture envelope contains one geometry Apply");
    }
    let changed_retry_outcome = cold.execute(Caller::Account(1), changed_retry).unwrap();
    assert_eq!(
        changed_retry_outcome.model,
        Err(mycity_core::transaction::TxError::OperationConflict)
    );
    assert!(!changed_retry_outcome.has_persistent_receipt);
    assert_eq!(
        changed_retry_outcome.committed_sequence,
        sequence_before_retry
    );
    assert_eq!(cold.committed_sequence(), sequence_before_retry);
    assert_eq!(cold.state_hash().unwrap(), state_before_retry);
    assert_eq!(
        serde_json::to_vec(&cold.receipt(apply_operation_id).unwrap().unwrap()).unwrap(),
        original_apply_record_bytes
    );
    assert_eq!(cold.state_hash().unwrap(), committed_hash);

    let current: Value =
        serde_json::from_slice(&fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
    let root_slot = current["slot"].as_u64().unwrap();
    let root: Value =
        serde_json::from_slice(&fs::read(world.join(format!("root-{root_slot}.json"))).unwrap())
            .unwrap();
    let catalog_hash: [u8; 32] =
        serde_json::from_value(root["spatial_transport_catalog_sha256"].clone()).unwrap();
    let catalog = read_spatial_transport_catalog(&world, catalog_hash).unwrap();
    assert_eq!(
        catalog.profile_sha256_for_service_day(1).unwrap(),
        baseline_profile_sha256
    );
    assert_eq!(
        catalog.profile_sha256_for_service_day(2).unwrap(),
        candidate_profile_sha256
    );
    assert_eq!(
        profile_sha256_for_proof(&catalog, &proof_day1).unwrap(),
        baseline_profile_sha256
    );
    assert_eq!(
        profile_sha256_for_proof(&catalog, &proof_day2).unwrap(),
        candidate_profile_sha256
    );
    assert!(world
        .join(format!("town-proof-{}.json", hex(&proof_day1_sha256)))
        .is_file());
    assert_eq!(
        VerifiedAllocationAsset::read_immutable(&world, baseline_profile_sha256, 1, 1, 1)
            .unwrap()
            .profile(),
        &baseline
    );
    eprintln!(
        "SPATIAL_GEOMETRY_APPLY_METRICS={}",
        serde_json::json!({
            "scope":"same_store_road_curve_geometry_apply_and_market_microfixture_not_scale_performance",
            "world":world.display().to_string(),
            "geometry_effective_service_day":descriptor.effective_service_day,
            "sales":[sales_day1,sales_day2],
            "freight_cents":[6480,6560],
            "served_people":[[out_day1,back_day1],[out_day2,back_day2]],
            "proof_sha256":[hex(&proof_day1_sha256),hex(&proof_day2_sha256)],
            "baseline_profile_sha256":hex(&baseline_profile_sha256),
            "candidate_profile_sha256":hex(&candidate_profile_sha256),
            "genesis_scene_state_sha256":genesis_scene_state,
            "candidate_scene_state_sha256":hex(&descriptor.candidate_scene_state_sha256),
            "catalog_sha256":hex(&catalog_hash),
            "apply_operation_id":apply_operation_id,
            "store_sequence":cold.committed_sequence(),
            "store_owned_capacity_bytes_after_cold":cold.owned_capacity_bytes().unwrap(),
            "world_logical_file_bytes":logical_directory_bytes(&world),
            "final_state_hash":committed_hash
        })
    );
}

fn current_catalog_asset_revision(world: &std::path::Path, asset_id: u32) -> u64 {
    let current: Value =
        serde_json::from_slice(&fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
    let slot = current["slot"].as_u64().unwrap();
    let root: Value =
        serde_json::from_slice(&fs::read(world.join(format!("root-{slot}.json"))).unwrap())
            .unwrap();
    let catalog_sha: [u8; 32] =
        serde_json::from_value(root["spatial_transport_catalog_sha256"].clone()).unwrap();
    let catalog = read_spatial_transport_catalog(world, catalog_sha).unwrap();
    catalog
        .versions()
        .iter()
        .rev()
        .find(|record| record.changed_asset_id == asset_id)
        .map(|record| record.asset_revision)
        .or_else(|| {
            catalog
                .assets()
                .iter()
                .find(|asset| asset.asset_id == asset_id)
                .map(|asset| asset.revision)
        })
        .unwrap()
}

fn apply_transport_access_change(
    store: &mut Store,
    world: &std::path::Path,
    asset_id: u32,
    edge_id: u32,
    surface: SpatialAccessSurface,
    owner: u32,
    applicant: u32,
    service_day: u64,
    operation: SpatialAccessOperation,
) {
    let (permission_revision, request, digest, proposed, candidate_sha256) = {
        let permission = store.engine().unwrap().spatial_permissions().unwrap();
        let asset = permission
            .assets()
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .unwrap();
        let current = VerifiedAllocationAsset::read_immutable(
            world,
            asset.current_profile_sha256,
            permission.world_id,
            permission.branch_id,
            1,
        )
        .unwrap();
        let mut proposed = current.profile().opening.clone();
        proposed.revision = proposed
            .revision
            .checked_add(1)
            .expect("fixture transport revision does not overflow");
        proposed
            .edges
            .iter_mut()
            .find(|edge| edge.id == edge_id)
            .unwrap()
            .open = operation == SpatialAccessOperation::Restore;
        let candidate = preview_profile_asset(&current.as_ref(), proposed.clone()).unwrap();
        let request = SpatialUpdateRequest {
            request_id: permission.request_id_highwater + 1,
            applicant_account: applicant,
            executor_account: 1,
            asset_id,
            operation,
            surface,
            edge_id,
            expected_revision: asset.revision,
            candidate_profile_sha256: candidate.sha256,
            effective_time: GameTime::new((service_day * DAY) as i64).unwrap(),
            parent_profile_sha256: asset.current_profile_sha256,
        };
        let context = permission.authorization_context(asset_id).unwrap();
        (
            permission.revision,
            request,
            request_sha256(context, &request).unwrap(),
            proposed,
            candidate.sha256,
        )
    };
    assert_eq!(
        store
            .stage_spatial_transport_candidate(request, proposed)
            .unwrap(),
        candidate_sha256
    );
    assert!(run_action(
        store,
        Caller::Account(applicant),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Request {
                expected_state_revision: permission_revision,
                request,
            },
        }
    )
    .model
    .is_ok());
    let approval_revision = store
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
                expected_state_revision: approval_revision,
                approval: owner_approval,
            },
        }
    )
    .model
    .is_ok());
    let authority_revision = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .revision;
    let authority = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .assets()
        .iter()
        .find(|asset| asset.asset_id == asset_id)
        .unwrap()
        .permit_authority_account;
    assert!(run_action(
        store,
        Caller::Account(authority),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Approve {
                expected_state_revision: authority_revision,
                approval: SpatialUpdateApproval {
                    request_id: request.request_id,
                    request_sha256: digest,
                    approver_account: authority,
                    role: SpatialApprovalRole::AuthorityPermit,
                    approved: true,
                },
            },
        }
    )
    .model
    .is_ok());
    advance_to_day(store, service_day);
    let permission_revision = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .revision;
    assert!(run_action(
        store,
        Caller::Account(1),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Apply {
                expected_state_revision: permission_revision,
                request_id: request.request_id,
                request_sha256: digest,
            },
        }
    )
    .model
    .is_ok());
}

pub fn run_stale_geometry_after_access_close(world: &std::path::Path, changed_asset_id: u32) {
    run_stale_geometry_inner(world, changed_asset_id, false);
}

pub fn run_stale_geometry_withdrawal(world: &std::path::Path, changed_asset_id: u32) {
    run_stale_geometry_inner(world, changed_asset_id, true);
}

fn run_stale_geometry_inner(world: &std::path::Path, changed_asset_id: u32, withdraw: bool) {
    run(world);
    let historical_proofs: Vec<_> = fs::read_dir(world)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("town-proof-")
        })
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    let descriptor_sha256 = store
        .stage_road_geometry_candidate(70, Point { x: 3200, y: 1800 }, 3)
        .unwrap();
    let descriptor =
        mycity_storage::spatial_geometry_updates::read_descriptor(world, descriptor_sha256)
            .unwrap();
    let expected_asset_revision = current_catalog_asset_revision(world, 70);
    let (state_revision, request_id) = {
        let state = store.engine().unwrap().spatial_geometry().unwrap();
        (state.revision(), state.request_id_highwater + 1)
    };
    let candidate_profile_sha256 = descriptor.candidate_profile_sha256;
    let request = Action::SpatialGeometry {
        command: SpatialGeometryCommand::Request {
            expected_state_revision: state_revision,
            request_id,
            executor_account: 1,
            asset_id: 70,
            expected_geometry_revision: descriptor.parent_geometry_revision,
            expected_asset_revision,
            descriptor_sha256,
        },
    };
    assert!(run_action(&mut store, Caller::Account(2), request)
        .model
        .is_ok());
    for approver in [3, 4] {
        let expected_state_revision = store
            .engine()
            .unwrap()
            .spatial_geometry()
            .unwrap()
            .revision();
        assert!(run_action(
            &mut store,
            Caller::Account(approver),
            Action::SpatialGeometry {
                command: SpatialGeometryCommand::Approve {
                    expected_state_revision,
                    request_id,
                    descriptor_sha256,
                    approved: true,
                },
            }
        )
        .model
        .is_ok());
    }

    let (edge_id, surface, owner, applicant) = match changed_asset_id {
        70 => (24, SpatialAccessSurface::Road, 3, 2),
        71 => (22, SpatialAccessSurface::Door, 2, 3),
        _ => panic!("fixture only registers road asset 70 and door asset 71"),
    };
    let scene_before_close = store.spatial().unwrap().unwrap().scene().state_hash();
    apply_transport_access_change(
        &mut store,
        world,
        changed_asset_id,
        edge_id,
        surface,
        owner,
        applicant,
        3,
        SpatialAccessOperation::Close,
    );
    let after_close_scene = store.spatial().unwrap().unwrap().scene().state_hash();
    assert_eq!(after_close_scene, scene_before_close);
    let scene_revision = store
        .spatial_scene_for_service_day(3)
        .unwrap()
        .unwrap()
        .input()
        .geometry_revision;
    let current: Value =
        serde_json::from_slice(&fs::read(world.join("CURRENT.json")).unwrap()).unwrap();
    let slot = current["slot"].as_u64().unwrap();
    let root: Value =
        serde_json::from_slice(&fs::read(world.join(format!("root-{slot}.json"))).unwrap())
            .unwrap();
    let catalog_sha: [u8; 32] =
        serde_json::from_value(root["spatial_transport_catalog_sha256"].clone()).unwrap();
    let catalog = read_spatial_transport_catalog(world, catalog_sha).unwrap();
    let active_profile_sha = catalog.profile_sha256_for_service_day(3).unwrap();
    let active_profile =
        VerifiedAllocationAsset::read_immutable(world, active_profile_sha, 1, 1, 1).unwrap();
    let target_asset_revision = current_catalog_asset_revision(world, 70);
    assert_eq!(scene_revision, 2);
    assert_eq!(active_profile.profile().opening.revision, 3);
    assert_eq!(
        target_asset_revision,
        expected_asset_revision + u64::from(changed_asset_id == 70)
    );
    let geometry_state_revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .revision();
    let before_rejected_apply_scene = store.spatial().unwrap().unwrap().scene().state_hash();
    let before_rejected_apply_state = store.state_hash().unwrap();
    let before_rejected_apply_sequence = store.committed_sequence();
    let before_rejected_apply_current = fs::read(world.join("CURRENT.json")).unwrap();
    let rejected_operation_id = before_rejected_apply_sequence + 1;
    let view = store.engine().unwrap().world().view();
    // Closed access makes a geometry rebuild unsafe before descriptor/model execution.
    // This is a Store preflight rejection, not a durable failed model receipt.
    let stale = store.execute(
        Caller::Account(1),
        Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: rejected_operation_id,
            expected_revision: view.revision,
            actions: vec![Action::SpatialGeometry {
                command: SpatialGeometryCommand::Apply {
                    expected_state_revision: geometry_state_revision,
                    request_id,
                    descriptor_sha256,
                    candidate_profile_sha256,
                },
            }],
        },
    );
    assert!(matches!(
        stale,
        Err(mycity_storage::Error::Invalid(
            "geometry_non_genesis_transport_state"
        ))
    ));
    assert_eq!(store.committed_sequence(), before_rejected_apply_sequence);
    assert_eq!(store.state_hash().unwrap(), before_rejected_apply_state);
    assert_eq!(
        fs::read(world.join("CURRENT.json")).unwrap(),
        before_rejected_apply_current
    );
    assert!(store.receipt(rejected_operation_id).unwrap().is_none());
    assert_eq!(
        store.spatial().unwrap().unwrap().scene().state_hash(),
        before_rejected_apply_scene
    );
    eprintln!(
        "SPATIAL_GEOMETRY_STALE_APPLY={}",
        serde_json::json!({
            "world":world.display().to_string(),
            "changed_asset_id":changed_asset_id,
            "descriptor_sha256":hex(&descriptor_sha256),
            "candidate_profile_sha256":hex(&candidate_profile_sha256),
            "request_expected_geometry_revision":descriptor.parent_geometry_revision,
            "request_expected_asset_revision":expected_asset_revision,
            "parent_transport_revision":descriptor.parent_transport_revision,
            "active_geometry_revision":scene_revision,
            "active_transport_revision":active_profile.profile().opening.revision,
            "active_target_asset_revision":target_asset_revision,
            "scene_state_unchanged":before_rejected_apply_scene == after_close_scene,
            "apply_preflight_rejected":true,
            "rejection_layer":"store_preflight",
            "rejection_reason":"geometry_non_genesis_transport_state",
            "store_sequence_unchanged":store.committed_sequence() == before_rejected_apply_sequence,
            "store_state_unchanged":store.state_hash().unwrap() == before_rejected_apply_state,
            "has_persistent_receipt":false
        })
    );

    if withdraw {
        drop(store);
        exercise_geometry_withdrawal(world, request_id, descriptor_sha256);
        for (path, bytes) in historical_proofs {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        return;
    }

    // Keep the first Close world recoverable, then restore through the same dual-approved
    // path. The genesis permissions are unchanged even though geometry changed travel costs.
    drop(store);
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    assert_eq!(store.state_hash().unwrap(), before_rejected_apply_state);
    assert_eq!(store.committed_sequence(), before_rejected_apply_sequence);
    apply_transport_access_change(
        &mut store,
        world,
        changed_asset_id,
        edge_id,
        surface,
        owner,
        applicant,
        4,
        SpatialAccessOperation::Restore,
    );
    apply_later_geometry(&mut store, world, 5);
    // A further Close/Restore must anchor to this second geometry, not the first or genesis.
    for (day, operation) in [
        (6, SpatialAccessOperation::Close),
        (7, SpatialAccessOperation::Restore),
    ] {
        apply_transport_access_change(
            &mut store,
            world,
            changed_asset_id,
            edge_id,
            surface,
            owner,
            applicant,
            day,
            operation,
        );
        let hash = store.state_hash().unwrap();
        let sequence = store.committed_sequence();
        let scene_hash = store.spatial().unwrap().unwrap().scene().state_hash();
        drop(store);
        store = Store::open(world, SPATIAL_RULES).unwrap();
        assert_eq!(store.state_hash().unwrap(), hash);
        assert_eq!(store.committed_sequence(), sequence);
        assert_eq!(
            store.spatial().unwrap().unwrap().scene().state_hash(),
            scene_hash
        );
    }
    for (path, bytes) in historical_proofs {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

fn apply_later_geometry(store: &mut Store, world: &std::path::Path, day: u64) {
    let descriptor_sha256 = store
        .stage_road_geometry_candidate(70, Point { x: 3200, y: 1800 }, day)
        .unwrap();
    let descriptor =
        mycity_storage::spatial_geometry_updates::read_descriptor(world, descriptor_sha256)
            .unwrap();
    let state = store.engine().unwrap().spatial_geometry().unwrap();
    let request_id = state.request_id_highwater + 1;
    let expected_state_revision = state.revision();
    let expected_asset_revision = current_catalog_asset_revision(world, 70);
    assert!(run_action(
        store,
        Caller::Account(2),
        Action::SpatialGeometry {
            command: SpatialGeometryCommand::Request {
                expected_state_revision,
                request_id,
                executor_account: 1,
                asset_id: 70,
                expected_geometry_revision: descriptor.parent_geometry_revision,
                expected_asset_revision,
                descriptor_sha256,
            },
        }
    )
    .model
    .is_ok());
    for account in [3, 4] {
        let expected_state_revision = store
            .engine()
            .unwrap()
            .spatial_geometry()
            .unwrap()
            .revision();
        assert!(run_action(
            store,
            Caller::Account(account),
            Action::SpatialGeometry {
                command: SpatialGeometryCommand::Approve {
                    expected_state_revision,
                    request_id,
                    descriptor_sha256,
                    approved: true,
                },
            }
        )
        .model
        .is_ok());
    }
    // Commit geometry at the preceding completed boundary; it activates next service day.
    advance_to_day(store, day - 1);
    let expected_state_revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .revision();
    assert!(run_action(
        store,
        Caller::Account(1),
        Action::SpatialGeometry {
            command: SpatialGeometryCommand::Apply {
                expected_state_revision,
                request_id,
                descriptor_sha256,
                candidate_profile_sha256: descriptor.candidate_profile_sha256,
            },
        }
    )
    .model
    .is_ok());
    let scene_hash = store.spatial().unwrap().unwrap().scene().state_hash();
    assert_eq!(
        store
            .spatial()
            .unwrap()
            .unwrap()
            .scene()
            .input()
            .geometry_revision,
        3
    );
    let hash = store.state_hash().unwrap();
    let sequence = store.committed_sequence();
    store.checkpoint().unwrap();
    // The caller subsequently drops and cold-opens this full multi-geometry catalog.
    assert_eq!(store.state_hash().unwrap(), hash);
    assert_eq!(store.committed_sequence(), sequence);
    assert_eq!(
        store.spatial().unwrap().unwrap().scene().state_hash(),
        scene_hash
    );
}

fn exercise_geometry_withdrawal(
    world: &std::path::Path,
    request_id: u64,
    descriptor_sha256: [u8; 32],
) {
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    let scene = store.spatial().unwrap().unwrap().scene().state_hash();
    let view_before = serde_json::to_value(store.engine().unwrap().world().view()).unwrap();
    let geometry_before = serde_json::to_value(store.engine().unwrap().spatial_geometry()).unwrap();
    let permissions_before =
        serde_json::to_value(store.engine().unwrap().spatial_permissions()).unwrap();
    let withdraw = |revision, hash| Action::SpatialGeometry {
        command: SpatialGeometryCommand::Withdraw {
            expected_state_revision: revision,
            request_id,
            descriptor_sha256: hash,
        },
    };
    let state_revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .revision();
    let sequence_before = store.committed_sequence();
    let mut wrong_hash = descriptor_sha256;
    wrong_hash[0] ^= 1;
    let scope = store.engine().unwrap().world().scope_revision();
    let bad = store.execute(
        Caller::Account(2),
        Envelope {
            world: scope.0.world(),
            branch: scope.0.branch(),
            operation_id: sequence_before + 1,
            expected_revision: scope.1,
            actions: vec![withdraw(state_revision, wrong_hash)],
        },
    );
    assert!(bad.is_err());
    assert_eq!(store.committed_sequence(), sequence_before);
    assert_eq!(
        serde_json::to_value(store.engine().unwrap().spatial_geometry()).unwrap(),
        geometry_before
    );
    // The designated executor is not the applicant, owner, or permit authority.
    let denied = run_action(
        &mut store,
        Caller::Account(1),
        withdraw(state_revision, descriptor_sha256),
    );
    assert_eq!(
        denied.model,
        Err(mycity_core::transaction::TxError::InvalidOperation)
    );
    assert!(denied.has_persistent_receipt);
    assert_eq!(
        serde_json::to_value(store.engine().unwrap().world().view()).unwrap(),
        view_before
    );
    assert_eq!(
        serde_json::to_value(store.engine().unwrap().spatial_geometry()).unwrap(),
        geometry_before
    );
    let state_revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .revision();
    let scope = store.engine().unwrap().world().scope_revision();
    let original = Envelope {
        world: scope.0.world(),
        branch: scope.0.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: scope.1,
        actions: vec![withdraw(state_revision, descriptor_sha256)],
    };
    let mut applied = false;
    let lost = store.execute_with_observer(Caller::Account(2), original.clone(), |point| {
        if point == WritePoint::Applied {
            applied = true;
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "withdraw-applied-response-lost",
            ));
        }
        Ok(())
    });
    assert!(
        applied,
        "legal withdrawal must reach original commit path: {lost:?}"
    );
    assert!(matches!(lost, Err(mycity_storage::Error::Io(_))));
    // Release the original lock before cold-opening the uncertain committed result.
    drop(store);
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    let state = store.engine().unwrap().spatial_geometry().unwrap();
    assert!(!state
        .requests()
        .iter()
        .any(|r| r.request.request_id == request_id));
    assert!(serde_json::to_value(state).unwrap()["approvals"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["request_id"] != request_id));
    let after = store.state_hash().unwrap();
    let sequence = store.committed_sequence();
    let receipt = serde_json::to_vec(&store.receipt(original.operation_id).unwrap()).unwrap();
    assert!(store
        .execute(Caller::Account(2), original.clone())
        .unwrap()
        .model
        .is_ok());
    assert_eq!(store.state_hash().unwrap(), after);
    assert_eq!(store.committed_sequence(), sequence);
    assert_eq!(
        store.spatial().unwrap().unwrap().scene().state_hash(),
        scene
    );
    let after_view = serde_json::to_value(store.engine().unwrap().world().view()).unwrap();
    let mut expected_view = view_before.clone();
    expected_view["revision"] = serde_json::json!(view_before["revision"].as_u64().unwrap() + 1);
    // Exactly the one successful withdrawal advances World revision; all other
    // World fields, accounts, stocks and business progress remain identical.
    assert_eq!(after_view, expected_view);
    assert_eq!(
        serde_json::to_value(store.engine().unwrap().spatial_permissions()).unwrap(),
        permissions_before
    );
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    assert_eq!(store.state_hash().unwrap(), after);
    assert_eq!(store.committed_sequence(), sequence);
    assert_eq!(
        serde_json::to_vec(&store.receipt(original.operation_id).unwrap()).unwrap(),
        receipt
    );
    assert!(store
        .execute(Caller::Account(2), original)
        .unwrap()
        .model
        .is_ok());
    assert_eq!(store.state_hash().unwrap(), after);
    assert_eq!(store.committed_sequence(), sequence);
    eprintln!("WITHDRAW_RESTORED request={request_id} sequence={sequence} state={after}");
}

pub fn run_expired_geometry_withdrawal(world: &std::path::Path) {
    run(world);
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    let day = store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .progress
        .last_complete
        / DAY
        + 1;
    let hash = store
        .stage_road_geometry_candidate(70, Point { x: 3200, y: 1800 }, day)
        .unwrap();
    let descriptor =
        mycity_storage::spatial_geometry_updates::read_descriptor(world, hash).unwrap();
    let request_id = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .request_id_highwater
        + 1;
    let revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .revision();
    assert!(run_action(
        &mut store,
        Caller::Account(2),
        Action::SpatialGeometry {
            command: SpatialGeometryCommand::Request {
                expected_state_revision: revision,
                request_id,
                executor_account: 1,
                asset_id: 70,
                expected_geometry_revision: descriptor.parent_geometry_revision,
                expected_asset_revision: current_catalog_asset_revision(world, 70),
                descriptor_sha256: hash
            }
        }
    )
    .model
    .is_ok());
    for account in [3, 4] {
        let revision = store
            .engine()
            .unwrap()
            .spatial_geometry()
            .unwrap()
            .revision();
        assert!(run_action(
            &mut store,
            Caller::Account(account),
            Action::SpatialGeometry {
                command: SpatialGeometryCommand::Approve {
                    expected_state_revision: revision,
                    request_id,
                    descriptor_sha256: hash,
                    approved: true
                }
            }
        )
        .model
        .is_ok());
    }
    advance_to_day(&mut store, day + 1);
    let target = (day + 1) * DAY + 1;
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
            break;
        }
        assert!(run_action(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8192
                }
            }
        )
        .model
        .is_ok());
    }
    let progress = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(progress.last_complete, target);
    assert!(progress.phase.is_none() && progress.target.is_none());
    assert_ne!(progress.last_complete % DAY, 0);
    drop(store);
    exercise_geometry_withdrawal(world, request_id, hash);
    // Freed authority slots admit a new request; highwater identity never resets.
    let mut store = Store::open(world, SPATIAL_RULES).unwrap();
    assert_eq!(
        store
            .engine()
            .unwrap()
            .spatial_geometry()
            .unwrap()
            .active_request_count(),
        0
    );
    advance_to_day(&mut store, day + 2);
    let next_hash = store
        .stage_road_geometry_candidate(70, Point { x: 3200, y: 1800 }, day + 3)
        .unwrap();
    let next_descriptor =
        mycity_storage::spatial_geometry_updates::read_descriptor(world, next_hash).unwrap();
    let state = store.engine().unwrap().spatial_geometry().unwrap();
    assert_eq!(state.request_id_highwater, request_id);
    let revision = state.revision();
    assert!(run_action(
        &mut store,
        Caller::Account(2),
        Action::SpatialGeometry {
            command: SpatialGeometryCommand::Request {
                expected_state_revision: revision,
                request_id: request_id + 1,
                executor_account: 1,
                asset_id: 70,
                expected_geometry_revision: next_descriptor.parent_geometry_revision,
                expected_asset_revision: current_catalog_asset_revision(world, 70),
                descriptor_sha256: next_hash,
            }
        }
    )
    .model
    .is_ok());
    let revision = store
        .engine()
        .unwrap()
        .spatial_geometry()
        .unwrap()
        .revision();
    assert!(run_action(
        &mut store,
        Caller::Account(2),
        Action::SpatialGeometry {
            command: SpatialGeometryCommand::Withdraw {
                expected_state_revision: revision,
                request_id: request_id + 1,
                descriptor_sha256: next_hash,
            }
        }
    )
    .model
    .is_ok());
    assert_eq!(
        store
            .engine()
            .unwrap()
            .spatial_geometry()
            .unwrap()
            .active_request_count(),
        0
    );
}
