//! #21 existing map adapter/proof consumer, on the v219 common asset city.
//! The police graph keeps its original source; this does not assert both road graphs coincide.
#[path="asset_opening.rs"]mod asset;
use mycity_core::{market_access::{AllocationScope,DemandRepeatPolicy,FeePolicyInput,TransportAllocationProfile,TripPartition},timeline::DAY,transport::{Opening,OdDemand},units::Quantity};
use mycity_map::{geometry::{Facing,Mode,Port},probe,scene::Scene,terrain::import,traffic_adapter::{adapt,AdapterIds,AdapterInput,FacilityOperation,RoadDirection,TerminalGrant,WALK_MODE_MASK},Point};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
const SPATIAL:&[u8]=include_bytes!("../../../fixtures/m0-spatial.json");
pub const HEIGHTMAP:&[u8]=include_bytes!("../../../fixtures/terrain/m0-flat16.png");
#[derive(Clone,Copy)]pub enum Scenario{Open,RoadClosed,DoorCannotEnter}
fn od(source_id: u64, origin_entrance: u32, destination_entrance: u32) -> OdDemand {
    OdDemand {
        source_id,
        origin_entrance,
        destination_entrance,
        mode: WALK_MODE_MASK,
        people: 4,
    }
}

fn make_profile_opening(scenario: Scenario) -> Opening {
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
                owner_id: 7, // existing common-city merchant account
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
    opening
}


pub fn opening(scenario:Scenario)->(Value,Vec<u8>,Vec<u8>){
 let opening=make_profile_opening(scenario);let assignment=opening.assign().unwrap();
 let profile=TransportAllocationProfile{schema:1,world_id:1,branch_id:1,market_profile_id:1,period_seconds:DAY as u32,allocation_scope:AllocationScope::SingleTownExclusive,demand_repeat_policy:DemandRepeatPolicy::RepeatFixedDailyDemand,road_capacity_effect:None,opening,assignment,partitions:vec![TripPartition{quote_id:1,outbound_source:101,return_source:102,trip_count:4,goods_per_trip:Quantity::new(25).unwrap()}],fee_policy:FeePolicyInput{provider_account:1,cents_per_travel_second:1}};
 profile.validate().unwrap();let bytes=serde_json::to_vec(&profile).unwrap();let hash:[u8;32]=Sha256::digest(&bytes).into();
 let mut input=asset::input();input["town"]["transport_allocation_sha256"]=json!(hash);
 // Explicit willingness-to-pay input; no cash or inventory endowment changes.
 input["town"]["households"][0]["reference_price_per_q"]=json!(1000);
 let mut spatial:Value=serde_json::from_slice(SPATIAL).unwrap();spatial["bootstrap"]["seed"]=json!(107);if matches!(scenario,Scenario::DoorCannotEnter){spatial["scene"]["door"]["can_enter"]=json!(false);}
 (input,bytes,serde_json::to_vec(&spatial).unwrap())
}
pub fn resources()->Vec<mycity_core::transaction::Resource>{asset::resources()}
