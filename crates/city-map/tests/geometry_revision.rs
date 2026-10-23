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
    geometry_revision::{build_candidate, CandidateError, GeometryEdit, PortDimension},
    probe,
    scene::{Input as SceneInput, Scene},
    terrain::import,
    traffic_adapter::{
        adapt, AdapterIds, AdapterInput, FacilityOperation, RoadDirection, TerminalGrant,
        WALK_MODE_MASK,
    },
    Point,
};
use sha2::{Digest, Sha256};

const SPATIAL: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const HEIGHTMAP: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");

fn input() -> (probe::Input, mycity_map::terrain::Terrain) {
    let probe: probe::Input = serde_json::from_slice(SPATIAL).unwrap();
    let terrain = import(probe.terrain.clone(), HEIGHTMAP, 1, 0).unwrap();
    (probe, terrain)
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

fn adapter<'a>(scene: &SceneInput, demands: &'a [OdDemand]) -> AdapterInput<'a> {
    adapter_with_revision(scene, demands, 1)
}

fn adapter_with_revision<'a>(
    scene: &SceneInput,
    demands: &'a [OdDemand],
    revision: u64,
) -> AdapterInput<'a> {
    AdapterInput {
        expected_map_id: scene.map.map,
        city_id: 42,
        revision,
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
                z_cm: scene.ground_z_cm,
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

fn profile(
    input: &SceneInput,
    terrain: &mycity_map::terrain::Terrain,
    ds: &[OdDemand],
) -> TransportAllocationProfile {
    profile_with_revision(input, terrain, ds, 1)
}

fn profile_with_revision(
    input: &SceneInput,
    terrain: &mycity_map::terrain::Terrain,
    ds: &[OdDemand],
    revision: u64,
) -> TransportAllocationProfile {
    let scene = Scene::assemble(input.clone(), terrain, terrain).unwrap();
    let opening = adapt(&scene, adapter_with_revision(input, ds, revision)).unwrap();
    let assignment = opening.assign().unwrap();
    TransportAllocationProfile {
        schema: 1,
        world_id: input.map.world,
        branch_id: input.map.branch,
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
            goods_per_trip: Quantity::new(1).unwrap(),
        }],
        fee_policy: FeePolicyInput {
            provider_account: 200,
            cents_per_travel_second: 0,
        },
    }
}

fn assert_opening_fields_unchanged_except_revision_and_travel_time(
    before: &mycity_core::transport::Opening,
    after: &mycity_core::transport::Opening,
) {
    assert_ne!(before.revision, after.revision);
    assert_eq!(before.nodes, after.nodes);
    assert_eq!(before.entrances, after.entrances);
    assert_eq!(before.demands, after.demands);
    assert_eq!(before.edges.len(), after.edges.len());
    for (before, after) in before.edges.iter().zip(&after.edges) {
        let mut before_without_time = *before;
        let mut after_without_time = *after;
        before_without_time.free_flow_seconds = 0;
        after_without_time.free_flow_seconds = 0;
        assert_eq!(before_without_time, after_without_time);
    }
}

#[test]
fn control_point_change_revalidates_scene_and_changes_real_route_time() {
    let (probe, terrain) = input();
    let baseline = profile(&probe.scene, &terrain, &demands());
    let bytes = serde_json::to_vec(&baseline).unwrap();
    let result = build_candidate(
        &probe.scene,
        &terrain,
        &terrain,
        adapter(&probe.scene, &demands()),
        &baseline,
        &bytes,
        GeometryEdit::RoadControlPoint(Point { x: 3000, y: 1800 }),
    )
    .unwrap();
    let old_route_seconds = result.prior_edge_travel_seconds[2];
    let new_route_seconds = result.candidate_edge_travel_seconds[2];
    assert!(new_route_seconds > old_route_seconds);
    assert_ne!(result.prior_scene_sha256, result.candidate_scene_sha256);
    assert_ne!(result.prior_adapter_sha256, result.candidate_adapter_sha256);
    assert_eq!(result.prior_served_people, 4);
    assert_eq!(result.candidate_served_people, 4);
    let expected_candidate_profile_sha256: [u8; 32] = Sha256::digest(&result.profile_bytes).into();
    assert_eq!(
        result.candidate_profile_sha256,
        expected_candidate_profile_sha256
    );
    let candidate_profile: TransportAllocationProfile =
        serde_json::from_slice(&result.profile_bytes).unwrap();
    assert_eq!(candidate_profile.assignment.served_people, 4);
    assert_eq!(
        result.scene.input().geometry_revision,
        probe.scene.geometry_revision + 1
    );
    assert_eq!(
        candidate_profile.opening.revision,
        baseline.opening.revision + 1
    );
    assert_ne!(
        candidate_profile.assignment.input_fingerprint,
        baseline.assignment.input_fingerprint
    );
    assert_opening_fields_unchanged_except_revision_and_travel_time(
        &baseline.opening,
        &candidate_profile.opening,
    );
    assert_eq!(
        result.candidate_owned_capacity_bytes,
        result.scene.owned_capacity_bytes() - std::mem::size_of::<Scene>()
            + std::mem::size_of_val(&result)
            + result.profile_bytes.capacity()
            + result.prior_edge_travel_seconds.capacity() * std::mem::size_of::<u32>()
            + result.candidate_edge_travel_seconds.capacity() * std::mem::size_of::<u32>()
            + result.prior_scene_sha256.capacity()
            + result.candidate_scene_sha256.capacity()
            + result.prior_terrain_sha256.capacity()
            + result.candidate_terrain_sha256.capacity()
    );
    assert_eq!(result.prior_terrain_sha256, result.candidate_terrain_sha256);
}

#[test]
fn port_width_change_is_validated_and_does_not_invent_capacity_or_flow() {
    let (probe, terrain) = input();
    let baseline = profile(&probe.scene, &terrain, &demands());
    let bytes = serde_json::to_vec(&baseline).unwrap();
    let result = build_candidate(
        &probe.scene,
        &terrain,
        &terrain,
        adapter(&probe.scene, &demands()),
        &baseline,
        &bytes,
        GeometryEdit::PortClearWidth {
            port: PortDimension::DoorClearWidth,
            width_cm: 400,
        },
    )
    .unwrap();
    assert_ne!(result.prior_scene_sha256, result.candidate_scene_sha256);
    assert_ne!(result.prior_adapter_sha256, result.candidate_adapter_sha256);
    assert_eq!(result.prior_served_people, result.candidate_served_people);
    let candidate_profile: TransportAllocationProfile =
        serde_json::from_slice(&result.profile_bytes).unwrap();
    assert_eq!(
        result.scene.input().geometry_revision,
        probe.scene.geometry_revision + 1
    );
    assert_eq!(
        candidate_profile.opening.revision,
        baseline.opening.revision + 1
    );
    assert_ne!(
        candidate_profile.assignment.input_fingerprint,
        baseline.assignment.input_fingerprint
    );
    assert_opening_fields_unchanged_except_revision_and_travel_time(
        &baseline.opening,
        &candidate_profile.opening,
    );
    assert_eq!(
        result.prior_edge_travel_seconds,
        result.candidate_edge_travel_seconds
    );
}

#[test]
fn invalid_support_bounds_endpoint_binding_and_modes_are_rejected() {
    let (probe, terrain) = input();
    let baseline = profile(&probe.scene, &terrain, &demands());
    let bytes = serde_json::to_vec(&baseline).unwrap();
    assert!(matches!(
        build_candidate(
            &probe.scene,
            &terrain,
            &terrain,
            adapter(&probe.scene, &demands()),
            &baseline,
            &bytes,
            GeometryEdit::RoadControlPoint(Point {
                x: 3000,
                y: 210_000
            }),
        ),
        Err(CandidateError::Scene(mycity_map::Error::Permission))
    ));
    let mut invalid_input = probe.scene.clone();
    invalid_input.terrain_revision += 1;
    assert!(matches!(
        build_candidate(
            &invalid_input,
            &terrain,
            &terrain,
            adapter(&invalid_input, &demands()),
            &baseline,
            &bytes,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::Scene(mycity_map::Error::Stale))
    ));
    let wrong_endpoint_demands = demands();
    let mut wrong_endpoint = adapter(&probe.scene, &wrong_endpoint_demands);
    wrong_endpoint.terminal.port.point = Point { x: 1001, y: 1000 };
    assert!(matches!(
        build_candidate(
            &probe.scene,
            &terrain,
            &terrain,
            wrong_endpoint,
            &baseline,
            &bytes,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::Adapter(
            mycity_map::traffic_adapter::AdapterError::InvalidGeometryBinding
        ))
    ));
    let mut unsupported = probe.scene.clone();
    unsupported.road_mode = Mode::Vehicle;
    assert!(matches!(
        build_candidate(
            &unsupported,
            &terrain,
            &terrain,
            adapter(&unsupported, &demands()),
            &baseline,
            &bytes,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::Scene(mycity_map::Error::Unsupported))
    ));
}

#[test]
fn too_narrow_port_is_rejected_and_raw_baseline_bytes_are_bound_exactly() {
    let (probe, terrain) = input();
    let baseline = profile(&probe.scene, &terrain, &demands());
    let bytes = serde_json::to_vec(&baseline).unwrap();
    assert!(matches!(
        build_candidate(
            &probe.scene,
            &terrain,
            &terrain,
            adapter(&probe.scene, &demands()),
            &baseline,
            &bytes,
            GeometryEdit::PortClearWidth {
                port: PortDimension::DoorClearWidth,
                width_cm: 100
            },
        ),
        Err(CandidateError::Scene(_))
    ));
    let mut alternate_raw_encoding = bytes.clone();
    alternate_raw_encoding.push(b' ');
    let result = build_candidate(
        &probe.scene,
        &terrain,
        &terrain,
        adapter(&probe.scene, &demands()),
        &baseline,
        &alternate_raw_encoding,
        GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
    )
    .unwrap();
    let expected_prior_profile_sha256: [u8; 32] = Sha256::digest(&alternate_raw_encoding).into();
    assert_eq!(result.prior_profile_sha256, expected_prior_profile_sha256);

    let truncated = &bytes[..bytes.len() - 1];
    assert!(matches!(
        build_candidate(
            &probe.scene,
            &terrain,
            &terrain,
            adapter(&probe.scene, &demands()),
            &baseline,
            truncated,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::InvalidBaseline)
    ));
}

#[test]
fn typed_and_raw_inputs_are_bounded_before_candidate_cloning() {
    let (probe, terrain) = input();
    let baseline = profile(&probe.scene, &terrain, &demands());
    let bytes = serde_json::to_vec(&baseline).unwrap();
    let mut oversized_scene = probe.scene.clone();
    oversized_scene.connector = vec![Point { x: 1, y: 1 }; 33];
    assert!(matches!(
        build_candidate(
            &oversized_scene,
            &terrain,
            &terrain,
            adapter(&oversized_scene, &demands()),
            &baseline,
            &bytes,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::InvalidBaseline)
    ));

    let mut oversized_profile = baseline.clone();
    oversized_profile.opening.nodes.resize(
        mycity_core::transport::MAX_NODES + 1,
        baseline.opening.nodes[0],
    );
    assert!(matches!(
        build_candidate(
            &probe.scene,
            &terrain,
            &terrain,
            adapter(&probe.scene, &demands()),
            &oversized_profile,
            &bytes,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::InvalidBaseline)
    ));

    let mut oversized_bytes = bytes;
    oversized_bytes.resize(
        mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES + 1,
        b' ',
    );
    assert!(matches!(
        build_candidate(
            &probe.scene,
            &terrain,
            &terrain,
            adapter(&probe.scene, &demands()),
            &baseline,
            &oversized_bytes,
            GeometryEdit::RoadControlPoint(Point { x: 2800, y: 700 }),
        ),
        Err(CandidateError::InvalidBaseline)
    ));
}

#[test]
fn geometry_and_transport_revision_heads_advance_independently() {
    let (probe, terrain) = input();
    let mut prior_scene = probe.scene.clone();
    prior_scene.geometry_revision = 5;
    let ds = demands();
    let baseline = profile_with_revision(&prior_scene, &terrain, &ds, 17);
    assert_eq!(prior_scene.geometry_revision, 5);
    assert_eq!(baseline.opening.revision, 17);
    let bytes = serde_json::to_vec(&baseline).unwrap();

    let result = build_candidate(
        &prior_scene,
        &terrain,
        &terrain,
        adapter_with_revision(&prior_scene, &ds, 17),
        &baseline,
        &bytes,
        GeometryEdit::PortClearWidth {
            port: PortDimension::DoorClearWidth,
            width_cm: 400,
        },
    )
    .unwrap();
    let candidate_profile: TransportAllocationProfile =
        serde_json::from_slice(&result.profile_bytes).unwrap();
    assert_eq!(result.scene.input().geometry_revision, 6);
    assert_eq!(candidate_profile.opening.revision, 18);
    assert_ne!(result.prior_adapter_sha256, result.candidate_adapter_sha256);
    assert_opening_fields_unchanged_except_revision_and_travel_time(
        &baseline.opening,
        &candidate_profile.opening,
    );
}

#[test]
fn revision_overflow_rejects_each_parent_without_mutating_it() {
    let (probe, terrain) = input();
    let ds = demands();

    let mut max_geometry_scene = probe.scene.clone();
    max_geometry_scene.geometry_revision = u64::MAX;
    let geometry_baseline = profile_with_revision(&max_geometry_scene, &terrain, &ds, 23);
    let geometry_bytes = serde_json::to_vec(&geometry_baseline).unwrap();
    let geometry_error = build_candidate(
        &max_geometry_scene,
        &terrain,
        &terrain,
        adapter_with_revision(&max_geometry_scene, &ds, 23),
        &geometry_baseline,
        &geometry_bytes,
        GeometryEdit::RoadControlPoint(Point { x: 3000, y: 1800 }),
    );
    assert!(matches!(
        geometry_error,
        Err(CandidateError::Scene(mycity_map::Error::Overflow))
    ));
    assert_eq!(max_geometry_scene.geometry_revision, u64::MAX);
    assert_eq!(geometry_baseline.opening.revision, 23);
    assert_eq!(
        serde_json::to_vec(&geometry_baseline).unwrap(),
        geometry_bytes
    );

    let transport_baseline = profile_with_revision(&probe.scene, &terrain, &ds, u64::MAX);
    let transport_bytes = serde_json::to_vec(&transport_baseline).unwrap();
    let transport_error = build_candidate(
        &probe.scene,
        &terrain,
        &terrain,
        adapter_with_revision(&probe.scene, &ds, u64::MAX),
        &transport_baseline,
        &transport_bytes,
        GeometryEdit::RoadControlPoint(Point { x: 3000, y: 1800 }),
    );
    assert!(matches!(
        transport_error,
        Err(CandidateError::Scene(mycity_map::Error::Overflow))
    ));
    assert_eq!(probe.scene.geometry_revision, 1);
    assert_eq!(transport_baseline.opening.revision, u64::MAX);
    assert_eq!(
        serde_json::to_vec(&transport_baseline).unwrap(),
        transport_bytes
    );
}
