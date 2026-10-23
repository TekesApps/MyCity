use mycity_core::transport::{OdDemand, UnservedReason};
use mycity_map::{
    geometry::{Facing, Mode, Port},
    probe,
    scene::Scene,
    terrain::import,
    traffic_adapter::{
        adapt, AdapterError, AdapterIds, AdapterInput, FacilityOperation, RoadDirection,
        TerminalGrant, WALK_MODE_MASK,
    },
    Point,
};

const SPATIAL: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const HEIGHTMAP: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");

fn make_scene(door_can_enter: bool) -> Scene {
    let mut fixture: probe::Input = serde_json::from_slice(SPATIAL).unwrap();
    fixture.scene.door.can_enter = door_can_enter;
    let terrain = import(fixture.terrain, HEIGHTMAP, 1, 0).unwrap();
    Scene::assemble(fixture.scene, &terrain, &terrain).unwrap()
}

fn port(point: Point) -> Port {
    Port {
        point,
        z_cm: 0,
        mode: Mode::Walk,
        facing: Facing::West,
        // m0-spatial's road corridor is 700 cm wide; the terminal grant must provide
        // enough clear width to connect physically to that corridor.
        clear_width_cm: 700,
        can_enter: true,
        can_exit: true,
        authorized: true,
    }
}

fn demand(source_id: u64, origin_entrance: u32, destination_entrance: u32) -> OdDemand {
    OdDemand {
        source_id,
        origin_entrance,
        destination_entrance,
        mode: WALK_MODE_MASK,
        people: 3,
    }
}

fn config(demands: &[OdDemand]) -> AdapterInput<'_> {
    AdapterInput {
        expected_map_id: 1,
        city_id: 42,
        revision: 7,
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
            port: port(Point { x: 1000, y: 1000 }),
        },
        connector: FacilityOperation {
            capacity_people_per_period: 5,
            free_flow_speed_cm_per_second: 100,
            open: true,
        },
        road: FacilityOperation {
            capacity_people_per_period: 9,
            free_flow_speed_cm_per_second: 100,
            open: true,
        },
        road_direction: RoadDirection::Both,
        demands,
    }
}

#[test]
fn explicit_geometry_and_facility_config_produce_transportable_walk_flow() {
    let scene = make_scene(true);
    let demands = [demand(1, 11, 12)];
    let opening = adapt(&scene, config(&demands)).unwrap();
    assert_eq!(opening.revision, 7);
    assert_eq!(
        opening.nodes.iter().map(|n| n.city_id).collect::<Vec<_>>(),
        vec![42; 3]
    );
    assert_eq!(opening.edges.len(), 4);
    assert!(opening.edges.iter().any(|e| e.capacity_people == 5));
    assert!(opening.edges.iter().any(|e| e.capacity_people == 9));
    let result = opening.assign().unwrap();
    assert_eq!(result.requested_people, 3);
    assert_eq!(result.served_people, 3);
    assert_eq!(result.path_receipts[0].edge_ids, vec![21, 24]);
}

#[test]
fn capacity_and_closed_facilities_change_assignment_results() {
    let scene = make_scene(true);
    let demands = [demand(1, 11, 12)];
    let mut input = config(&demands);
    input.connector.capacity_people_per_period = 2;
    let limited = adapt(&scene, input).unwrap().assign().unwrap();
    assert_eq!(limited.served_people, 2);
    assert_eq!(limited.unserved_people, 1);
    assert_eq!(
        limited.unserved[0].reason,
        UnservedReason::CapacityExhausted
    );

    let mut input = config(&demands);
    input.road.open = false;
    let closed_road = adapt(&scene, input).unwrap().assign().unwrap();
    assert_eq!(closed_road.served_people, 0);
    assert_eq!(closed_road.unserved[0].reason, UnservedReason::NoOpenRoute);

    let mut input = config(&demands);
    input.connector.open = false;
    let closed_connector = adapt(&scene, input).unwrap().assign().unwrap();
    assert_eq!(closed_connector.served_people, 0);
    assert_eq!(
        closed_connector.unserved[0].reason,
        UnservedReason::NoOpenRoute
    );
}

#[test]
fn directionality_and_entrance_adjustments_remove_real_graph_paths() {
    let scene = make_scene(true);
    let outward = [demand(1, 11, 12)];
    let mut input = config(&outward);
    input.road_direction = RoadDirection::StartToEnd;
    let no_reverse_road = adapt(&scene, input).unwrap().assign().unwrap();
    assert_eq!(no_reverse_road.served_people, 0);
    assert_eq!(
        no_reverse_road.unserved[0].reason,
        UnservedReason::NoOpenRoute
    );

    let scene = make_scene(false);
    let inbound = [demand(2, 12, 11)];
    let opening = adapt(&scene, config(&inbound)).unwrap();
    assert!(!opening.edges.iter().any(|e| e.id == 22));
    let result = opening.assign().unwrap();
    assert_eq!(result.served_people, 0);
    assert_eq!(result.unserved[0].reason, UnservedReason::NoOpenRoute);

    let scene = make_scene(true);
    let inbound = [demand(3, 12, 11)];
    let mut input = config(&inbound);
    input.terminal.port.can_enter = false;
    let opening = adapt(&scene, input).unwrap();
    assert!(!opening.edges.iter().any(|e| e.id == 23));
    assert_eq!(opening.assign().unwrap().served_people, 0);

    let outbound = [demand(4, 11, 12)];
    let mut input = config(&outbound);
    input.terminal.port.can_exit = false;
    let opening = adapt(&scene, input).unwrap();
    assert!(!opening.edges.iter().any(|e| e.id == 24));
    assert_eq!(opening.assign().unwrap().served_people, 0);
}

#[test]
fn rejects_wrong_map_nearby_terminal_wrong_height_and_missing_authority() {
    let scene = make_scene(true);
    let mut input = config(&[]);
    input.expected_map_id = 2;
    assert_eq!(
        adapt(&scene, input).unwrap_err(),
        AdapterError::InvalidGeometryBinding
    );

    let mut input = config(&[]);
    input.terminal.port.point.x += 1; // proximity is not a topology grant
    assert_eq!(
        adapt(&scene, input).unwrap_err(),
        AdapterError::InvalidGeometryBinding
    );

    let mut input = config(&[]);
    input.terminal.port.z_cm = 1;
    assert_eq!(
        adapt(&scene, input).unwrap_err(),
        AdapterError::InvalidGeometryBinding
    );

    let mut input = config(&[]);
    input.terminal.port.authorized = false;
    assert_eq!(adapt(&scene, input).unwrap_err(), AdapterError::Permission);
}

#[test]
fn rejects_unsupported_modes_and_bad_facility_capacities() {
    let scene = make_scene(true);
    let vehicle = [OdDemand {
        mode: 2,
        ..demand(1, 11, 12)
    }];
    assert_eq!(
        adapt(&scene, config(&vehicle)).unwrap_err(),
        AdapterError::UnsupportedMode
    );

    let mut input = config(&[]);
    input.road.capacity_people_per_period = 0;
    assert_eq!(
        adapt(&scene, input).unwrap_err(),
        AdapterError::InvalidConfig
    );
}
