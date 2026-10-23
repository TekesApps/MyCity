use mycity_core::{
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TripPartition,
    },
    timeline::{Command as TimelineCommand, DAY},
    transaction::{Action, Caller, Envelope},
    transport::{OdDemand, Opening},
    units::Quantity,
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
use mycity_storage::Store;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SPATIAL: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const HEIGHTMAP: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
const RULES: &str = "m0-spatial-transport-market-v1";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let artifact_root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").map(PathBuf::from);
        let root = artifact_root
            .as_ref()
            .cloned()
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-spatial-market-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("SPATIAL_MARKET_ARTIFACT_CREATED={}", path.display());
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() || std::thread::panicking() {
            eprintln!("SPATIAL_MARKET_ARTIFACT_PRESERVED={}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum Scenario {
    Open,
    RoadClosed,
    DoorCannotEnter,
}

impl Scenario {
    fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::RoadClosed => "road_closed",
            Self::DoorCannotEnter => "door_cannot_enter",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ResultSnapshot {
    state_hash: String,
    sales: i64,
    reachable: bool,
    proof_hash: [u8; 32],
    outbound_served: u32,
    return_served: u32,
}

fn od(source_id: u64, origin_entrance: u32, destination_entrance: u32) -> OdDemand {
    OdDemand {
        source_id,
        origin_entrance,
        destination_entrance,
        mode: WALK_MODE_MASK,
        people: 4,
    }
}

fn make_profile_opening(scenario: Scenario) -> (Opening, u128) {
    let mut fixture: probe::Input = serde_json::from_slice(SPATIAL).unwrap();
    if matches!(scenario, Scenario::DoorCannotEnter) {
        fixture.scene.door.can_enter = false;
    }
    let terrain = import(fixture.terrain, HEIGHTMAP, 1, 0).unwrap();
    let scene = Scene::assemble(fixture.scene, &terrain, &terrain).unwrap();
    let demands = [od(101, 11, 12), od(102, 12, 11)];
    let road = FacilityOperation {
        capacity_people_per_period: 4,
        free_flow_speed_cm_per_second: 100,
        open: !matches!(scenario, Scenario::RoadClosed),
    };
    let started = Instant::now();
    let opening = adapt(
        &scene,
        AdapterInput {
            expected_map_id: 1,
            city_id: 1,
            revision: 1,
            building_owner_id: 2, // household account
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
                owner_id: 3, // merchant account
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
            road,
            road_direction: RoadDirection::Both,
            demands: &demands,
        },
    )
    .unwrap();
    (opening, started.elapsed().as_nanos())
}

fn opening_owned_capacity_bytes(opening: &Opening) -> usize {
    std::mem::size_of::<Opening>()
        + opening.nodes.capacity() * std::mem::size_of::<mycity_core::transport::Node>()
        + opening.edges.capacity() * std::mem::size_of::<mycity_core::transport::RoadEdge>()
        + opening.entrances.capacity() * std::mem::size_of::<mycity_core::transport::Entrance>()
        + opening.demands.capacity() * std::mem::size_of::<OdDemand>()
}

fn assignment_owned_capacity_bytes(assignment: &mycity_core::transport::Assignment) -> usize {
    use mycity_core::transport::{BoundaryFlow, DemandOutcome, EdgeFlow, PathReceipt, Unserved};
    std::mem::size_of::<mycity_core::transport::Assignment>()
        + assignment.edge_flows.capacity() * std::mem::size_of::<EdgeFlow>()
        + assignment.unserved.capacity() * std::mem::size_of::<Unserved>()
        + assignment.boundary_flows.capacity() * std::mem::size_of::<BoundaryFlow>()
        + assignment.demand_outcomes.capacity() * std::mem::size_of::<DemandOutcome>()
        + assignment.path_receipts.capacity() * std::mem::size_of::<PathReceipt>()
        + assignment
            .path_receipts
            .iter()
            .map(|receipt| receipt.edge_ids.capacity() * std::mem::size_of::<u32>())
            .sum::<usize>()
}

fn allocation_bytes(opening: Opening) -> (Vec<u8>, [u8; 32], usize, usize, u128) {
    let opening_bytes = opening_owned_capacity_bytes(&opening);
    let assignment_started = Instant::now();
    let assignment = opening.assign().unwrap();
    let assignment_elapsed = assignment_started.elapsed().as_nanos();
    let assignment_bytes = assignment_owned_capacity_bytes(&assignment);
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
            provider_account: 1,
            cents_per_travel_second: 1,
        },
    };
    profile.validate().unwrap();
    let bytes = serde_json::to_vec(&profile).unwrap();
    let hash = Sha256::digest(&bytes).into();
    (
        bytes,
        hash,
        opening_bytes,
        assignment_bytes,
        assignment_elapsed,
    )
}

fn opening_with_market(allocation_hash: [u8; 32]) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
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

fn execute(store: &mut Store, action: Action) {
    let view = store.engine().unwrap().world().view();
    let request = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    };
    assert!(store
        .execute(Caller::System, request)
        .unwrap()
        .model
        .is_ok());
}

fn advance_day(store: &mut Store) {
    execute(
        store,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    for _ in 0..128 {
        execute(
            store,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target: DAY,
                    limit: 8192,
                },
            },
        );
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("timeline did not reach service day boundary");
}

fn run_scenario(scenario: Scenario) -> ResultSnapshot {
    let temp = Temp::new();
    let world = temp.0.join("world");
    let (opening, adapter_ns) = make_profile_opening(scenario);
    let (allocation, allocation_hash, opening_owned_bytes, assignment_owned_bytes, assign_ns) =
        allocation_bytes(opening);
    let create_started = Instant::now();
    let mut store = Store::create_with_transport_allocation_profile(
        &world,
        &opening_with_market(allocation_hash),
        vec![],
        RULES,
        &allocation,
    )
    .unwrap();
    let create_ns = create_started.elapsed().as_nanos();
    let store_owned_create = store.owned_capacity_bytes().unwrap();
    let advance_started = Instant::now();
    advance_day(&mut store);
    let advance_ns = advance_started.elapsed().as_nanos();
    let operation_id = store.committed_sequence() + 1;
    let settle_started = Instant::now();
    let settled = store.settle_town_market(operation_id).unwrap();
    let original_receipt = settled.model.unwrap();
    let settle_ns = settle_started.elapsed().as_nanos();
    let store_owned_postsettle = store.owned_capacity_bytes().unwrap();
    let view = store.engine().unwrap().world().view();
    let market = view.town.as_ref().unwrap();
    let receipt = market.state.last_transport.unwrap();
    let proof_path = world.join(format!(
        "town-proof-{}.json",
        hex(&receipt.issued_proof_sha256)
    ));
    let proof_bytes = fs::read(&proof_path).unwrap();
    let actual_proof_hash: [u8; 32] = Sha256::digest(&proof_bytes).into();
    assert_eq!(actual_proof_hash, receipt.issued_proof_sha256);
    let proof: mycity_core::market_access::TransportProofAsset =
        serde_json::from_slice(&proof_bytes).unwrap();
    assert_eq!(proof.service_day, 1);
    assert_eq!(proof.generation, 1);
    assert!(proof.assignment.matches(&proof.opening));
    let outbound_served = proof
        .assignment
        .demand_outcomes
        .iter()
        .find(|x| x.source_id == 101)
        .unwrap()
        .served_people;
    let return_served = proof
        .assignment
        .demand_outcomes
        .iter()
        .find(|x| x.source_id == 102)
        .unwrap()
        .served_people;
    let sales = market.state.last_summary.actual_sales.value();
    let reachable = market.state.last_summary.actual_sales > Quantity::ZERO;
    let before_reopen = ResultSnapshot {
        state_hash: store.state_hash().unwrap(),
        sales,
        reachable,
        proof_hash: receipt.issued_proof_sha256,
        outbound_served,
        return_served,
    };
    let checkpoint_started = Instant::now();
    store.checkpoint().unwrap();
    let checkpoint_ns = checkpoint_started.elapsed().as_nanos();
    drop(store);
    let cold_open_started = Instant::now();
    let mut reopened = Store::open(&world, RULES).unwrap();
    let cold_open_ns = cold_open_started.elapsed().as_nanos();
    let store_owned_postcold = reopened.owned_capacity_bytes().unwrap();
    let recovered_market = reopened.engine().unwrap().world().view().town.unwrap();
    assert_eq!(
        recovered_market
            .state
            .last_transport
            .unwrap()
            .issued_proof_sha256,
        before_reopen.proof_hash
    );
    let recovered_proof_bytes = fs::read(&proof_path).unwrap();
    let recovered_proof_hash: [u8; 32] = Sha256::digest(&recovered_proof_bytes).into();
    assert_eq!(recovered_proof_hash, before_reopen.proof_hash);
    assert_eq!(reopened.state_hash().unwrap(), before_reopen.state_hash);
    let replay_started = Instant::now();
    let replay = reopened.settle_town_market(operation_id).unwrap();
    let replay_ns = replay_started.elapsed().as_nanos();
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.model.unwrap(), original_receipt);
    assert_eq!(reopened.state_hash().unwrap(), before_reopen.state_hash);
    let replay_market = reopened.engine().unwrap().world().view().town.unwrap();
    assert_eq!(
        replay_market.state.last_summary.actual_sales.value(),
        before_reopen.sales
    );
    assert_eq!(
        replay_market
            .state
            .last_transport
            .unwrap()
            .issued_proof_sha256,
        before_reopen.proof_hash
    );
    assert_eq!(fs::read(&proof_path).unwrap(), recovered_proof_bytes);
    eprintln!(
        "SPATIAL_MARKET_METRICS={}",
        serde_json::json!({
            "scenario": scenario.label(),
            "scope": "3_node_single_map_walk_microfixture_not_scale_performance",
            "opening_owned_vec_capacity_bytes": opening_owned_bytes,
            "assignment_owned_vec_capacity_bytes_including_path_edge_ids": assignment_owned_bytes,
            "store_owned_create_bytes": store_owned_create,
            "store_owned_postsettlement_bytes": store_owned_postsettle,
            "store_owned_postcoldopen_bytes": store_owned_postcold,
            "allocation_profile_file_logical_bytes": allocation.len(),
            "proof_file_logical_bytes": proof_bytes.len(),
            "adapter_wall_ns": adapter_ns,
            "assignment_wall_ns": assign_ns,
            "store_create_wall_ns": create_ns,
            "timeline_advance_wall_ns": advance_ns,
            "store_settlement_wall_ns_including_proof_io": settle_ns,
            "checkpoint_wall_ns": checkpoint_ns,
            "cold_open_wall_ns": cold_open_ns,
            "exact_replay_wall_ns": replay_ns,
            "store_path": world.display().to_string(),
        })
    );
    before_reopen
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

#[test]
fn spatial_walk_access_changes_store_market_arrival_trade_and_survives_cold_replay() {
    let open = run_scenario(Scenario::Open);
    assert_eq!(open.outbound_served, 4);
    assert_eq!(open.return_served, 4);
    assert!(open.reachable);
    assert_eq!(open.sales, 40);

    let closed = run_scenario(Scenario::RoadClosed);
    assert_eq!(closed.outbound_served, 0);
    assert_eq!(closed.return_served, 0);
    assert!(!closed.reachable);
    assert_eq!(closed.sales, 0);

    let one_way = run_scenario(Scenario::DoorCannotEnter);
    assert_eq!(one_way.outbound_served, 4);
    assert_eq!(one_way.return_served, 0);
    assert!(!one_way.reachable);
    assert_eq!(one_way.sales, 0);

    assert_ne!(open.proof_hash, closed.proof_hash);
    assert_ne!(open.proof_hash, one_way.proof_hash);
}
