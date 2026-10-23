//! Candidate-only, bounded geometry revision through Scene validation and WALK adaptation.
//!
//! This module never publishes a revision, grants land rights, or advances a calendar.
use crate::{
    geometry::{Facing, Quadratic, MAX_HOLES},
    scene::{Input as SceneInput, Scene},
    terrain::Terrain,
    traffic_adapter::{
        self, AdapterError, AdapterIds, AdapterInput, FacilityOperation, RoadDirection,
        TerminalGrant,
    },
    Error as SceneError, Point,
};
use mycity_core::market_access::TransportAllocationProfile;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDimension {
    DoorClearWidth,
    AnchorClearWidth,
    TerminalClearWidth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryEdit {
    /// Control point only. Road start/end and all explicit network IDs remain fixed.
    RoadControlPoint(Point),
    /// Move or reorient a door while explicitly reconnecting the first corridor segment.
    /// Elevation, mode, width, enter/exit flags and authorization remain inherited from parent.
    DoorPose {
        point: Point,
        facing: Facing,
        first_connector_waypoint: Point,
    },
    PortClearWidth {
        port: PortDimension,
        width_cm: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateError {
    Scene(SceneError),
    Adapter(AdapterError),
    InvalidBaseline,
    NoChange,
    Profile,
    Encode,
}

pub struct GeometryRevisionCandidate {
    pub prior_scene_sha256: String,
    pub candidate_scene_sha256: String,
    pub prior_adapter_sha256: [u8; 32],
    pub candidate_adapter_sha256: [u8; 32],
    pub prior_profile_sha256: [u8; 32],
    pub candidate_profile_sha256: [u8; 32],
    pub prior_terrain_sha256: String,
    pub candidate_terrain_sha256: String,
    pub prior_served_people: u64,
    pub candidate_served_people: u64,
    pub prior_edge_travel_seconds: Vec<u32>,
    pub candidate_edge_travel_seconds: Vec<u32>,
    pub profile_bytes: Vec<u8>,
    pub scene: Scene,
    /// Retained owned-capacity estimate for this result: Scene heaps, exact profile bytes,
    /// route-time vectors and digest strings. Allocator metadata is not included.
    pub candidate_owned_capacity_bytes: usize,
}

fn prebound_scene_input(input: &SceneInput) -> Result<(), CandidateError> {
    if input.schema.len() > 64
        || input.map.metric_frame.len() > 64
        || input.map.vertical_datum.len() > 64
        || input.neighbor.metric_frame.len() > 64
        || input.neighbor.vertical_datum.len() > 64
        || input.region.holes.len() > MAX_HOLES
        || input.connector.len() > 32
        || input.obstacles.len() > 64
        || input.neighbor_obstacles.len() > 64
    {
        return Err(CandidateError::InvalidBaseline);
    }
    Ok(())
}

fn prebound_profile(profile: &TransportAllocationProfile) -> Result<(), CandidateError> {
    let opening = &profile.opening;
    let assignment = &profile.assignment;
    if opening.nodes.len() > mycity_core::transport::MAX_NODES
        || opening.edges.len() > mycity_core::transport::MAX_EDGES
        || opening.entrances.len() > mycity_core::transport::MAX_ENTRANCES
        || opening.demands.len() > mycity_core::transport::MAX_DEMANDS
        || assignment.edge_flows.len() > mycity_core::transport::MAX_EDGES
        || assignment.unserved.len() > mycity_core::transport::MAX_DEMANDS
        || assignment.boundary_flows.len() > mycity_core::transport::MAX_EDGES
        || assignment.demand_outcomes.len() > mycity_core::transport::MAX_DEMANDS
        || assignment.path_receipts.len() > mycity_core::transport::MAX_PATH_RECEIPTS
        || assignment
            .path_receipts
            .iter()
            .any(|path| path.edge_ids.len() > mycity_core::transport::MAX_PATH_EDGES)
        || profile.partitions.len() > mycity_core::market_access::MAX_TRIP_PARTITIONS
        || opening.validate().is_err()
    {
        return Err(CandidateError::InvalidBaseline);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AdapterConfig<'a> {
    expected_map_id: u32,
    city_id: u32,
    revision: u64,
    building_owner_id: u32,
    ids: AdapterIds,
    terminal: TerminalGrant,
    connector: FacilityOperation,
    road: FacilityOperation,
    road_direction: RoadDirection,
    demands: &'a [mycity_core::transport::OdDemand],
}

#[derive(Serialize)]
struct AdapterFingerprint<'a> {
    expected_map_id: u32,
    city_id: u32,
    revision: u64,
    building_owner_id: u32,
    ids: [u32; 8],
    terminal_owner_id: u32,
    terminal_entrance_id: u32,
    terminal_port: crate::geometry::Port,
    connector: [u32; 3],
    road: [u32; 3],
    road_direction: RoadDirectionName,
    demands: &'a [mycity_core::transport::OdDemand],
}

#[derive(Serialize)]
enum RoadDirectionName {
    StartToEnd,
    EndToStart,
    Both,
}

fn adapter_sha256(config: AdapterConfig<'_>) -> Result<[u8; 32], CandidateError> {
    let direction = match config.road_direction {
        RoadDirection::StartToEnd => RoadDirectionName::StartToEnd,
        RoadDirection::EndToStart => RoadDirectionName::EndToStart,
        RoadDirection::Both => RoadDirectionName::Both,
    };
    let bytes = serde_json::to_vec(&AdapterFingerprint {
        expected_map_id: config.expected_map_id,
        city_id: config.city_id,
        revision: config.revision,
        building_owner_id: config.building_owner_id,
        ids: [
            config.ids.building_node,
            config.ids.anchor_node,
            config.ids.terminal_node,
            config.ids.building_entrance,
            config.ids.connector_forward_edge,
            config.ids.connector_reverse_edge,
            config.ids.road_forward_edge,
            config.ids.road_reverse_edge,
        ],
        terminal_owner_id: config.terminal.owner_id,
        terminal_entrance_id: config.terminal.entrance_id,
        terminal_port: config.terminal.port,
        connector: [
            config.connector.capacity_people_per_period,
            config.connector.free_flow_speed_cm_per_second,
            u32::from(config.connector.open),
        ],
        road: [
            config.road.capacity_people_per_period,
            config.road.free_flow_speed_cm_per_second,
            u32::from(config.road.open),
        ],
        road_direction: direction,
        demands: config.demands,
    })
    .map_err(|_| CandidateError::Encode)?;
    Ok(Sha256::digest(bytes).into())
}

impl<'a> From<AdapterInput<'a>> for AdapterConfig<'a> {
    fn from(input: AdapterInput<'a>) -> Self {
        Self {
            expected_map_id: input.expected_map_id,
            city_id: input.city_id,
            revision: input.revision,
            building_owner_id: input.building_owner_id,
            ids: input.ids,
            terminal: input.terminal,
            connector: input.connector,
            road: input.road,
            road_direction: input.road_direction,
            demands: input.demands,
        }
    }
}

impl<'a> AdapterConfig<'a> {
    fn input(self) -> AdapterInput<'a> {
        AdapterInput {
            expected_map_id: self.expected_map_id,
            city_id: self.city_id,
            revision: self.revision,
            building_owner_id: self.building_owner_id,
            ids: self.ids,
            terminal: self.terminal,
            connector: self.connector,
            road: self.road,
            road_direction: self.road_direction,
            demands: self.demands,
        }
    }
}

/// Rebuild one candidate from the exact prior Scene input and source Terrain values.
/// Facility capacity/speed, OD demands and graph IDs are copied unchanged. A wider road does
/// not imply more capacity; only the explicit facility config can change capacity.
pub fn build_candidate(
    prior_input: &SceneInput,
    terrain: &Terrain,
    neighbor_terrain: &Terrain,
    adapter: AdapterInput<'_>,
    prior_profile: &TransportAllocationProfile,
    prior_profile_bytes: &[u8],
    edit: GeometryEdit,
) -> Result<GeometryRevisionCandidate, CandidateError> {
    prebound_scene_input(prior_input)?;
    if adapter.demands.len() > mycity_core::transport::MAX_DEMANDS {
        return Err(CandidateError::InvalidBaseline);
    }
    if prior_profile_bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES {
        return Err(CandidateError::InvalidBaseline);
    }
    prebound_profile(prior_profile)?;
    prior_profile
        .validate()
        .map_err(|_| CandidateError::InvalidBaseline)?;
    let config = AdapterConfig::from(adapter);
    let prior_scene = Scene::assemble(prior_input.clone(), terrain, neighbor_terrain)
        .map_err(CandidateError::Scene)?;
    let prior_adapter_sha256 = adapter_sha256(config)?;
    let prior_opening =
        traffic_adapter::adapt(&prior_scene, config.input()).map_err(CandidateError::Adapter)?;
    if prior_profile.opening != prior_opening
        || prior_profile.assignment
            != prior_opening
                .assign()
                .map_err(|_| CandidateError::Profile)?
        || prior_profile.world_id != prior_input.map.world
        || prior_profile.branch_id != prior_input.map.branch
        || serde_json::from_slice::<TransportAllocationProfile>(prior_profile_bytes)
            .map_or(true, |decoded| &decoded != prior_profile)
    {
        return Err(CandidateError::InvalidBaseline);
    }

    let mut candidate_input = prior_input.clone();
    let mut candidate_config = config;
    match edit {
        GeometryEdit::RoadControlPoint(control) => {
            if control == candidate_input.road.control {
                return Err(CandidateError::NoChange);
            }
            candidate_input.road = Quadratic {
                start: prior_input.road.start,
                control,
                end: prior_input.road.end,
            };
        }
        GeometryEdit::DoorPose {
            point,
            facing,
            first_connector_waypoint,
        } => {
            if candidate_input.door.point == point
                && candidate_input.door.facing == facing
                && candidate_input.connector.get(1) == Some(&first_connector_waypoint)
            {
                return Err(CandidateError::NoChange);
            }
            if candidate_input.connector.len() < 2 {
                return Err(CandidateError::InvalidBaseline);
            }
            candidate_input.door.point = point;
            candidate_input.door.facing = facing;
            candidate_input.connector[0] = point;
            candidate_input.connector[1] = first_connector_waypoint;
        }
        GeometryEdit::PortClearWidth { port, width_cm } => {
            match port {
                PortDimension::DoorClearWidth => candidate_input.door.clear_width_cm = width_cm,
                PortDimension::AnchorClearWidth => candidate_input.anchor.clear_width_cm = width_cm,
                PortDimension::TerminalClearWidth => {
                    candidate_config.terminal.port.clear_width_cm = width_cm
                }
            }
            let old = match port {
                PortDimension::DoorClearWidth => prior_input.door.clear_width_cm,
                PortDimension::AnchorClearWidth => prior_input.anchor.clear_width_cm,
                PortDimension::TerminalClearWidth => config.terminal.port.clear_width_cm,
            };
            if width_cm == old {
                return Err(CandidateError::NoChange);
            }
        }
    }
    candidate_input.geometry_revision = candidate_input
        .geometry_revision
        .checked_add(1)
        .ok_or(CandidateError::Scene(SceneError::Overflow))?;
    candidate_config.revision = candidate_config
        .revision
        .checked_add(1)
        .ok_or(CandidateError::Scene(SceneError::Overflow))?;

    let candidate_scene = Scene::assemble(candidate_input, terrain, neighbor_terrain)
        .map_err(CandidateError::Scene)?;
    let candidate_adapter_sha256 = adapter_sha256(candidate_config)?;
    let candidate_opening = traffic_adapter::adapt(&candidate_scene, candidate_config.input())
        .map_err(CandidateError::Adapter)?;
    let candidate_assignment = candidate_opening
        .assign()
        .map_err(|_| CandidateError::Profile)?;
    let profile = TransportAllocationProfile {
        schema: prior_profile.schema,
        world_id: prior_profile.world_id,
        branch_id: prior_profile.branch_id,
        market_profile_id: prior_profile.market_profile_id,
        period_seconds: prior_profile.period_seconds,
        allocation_scope: prior_profile.allocation_scope,
        demand_repeat_policy: prior_profile.demand_repeat_policy,
        road_capacity_effect: prior_profile.road_capacity_effect,
        opening: candidate_opening,
        assignment: candidate_assignment,
        partitions: prior_profile.partitions.clone(),
        fee_policy: prior_profile.fee_policy,
    };
    profile.validate().map_err(|_| CandidateError::Profile)?;
    let profile_bytes = serde_json::to_vec(&profile).map_err(|_| CandidateError::Encode)?;
    let candidate_profile_sha256 = Sha256::digest(&profile_bytes).into();
    let prior_edge_travel_seconds: Vec<u32> = prior_opening
        .edges
        .iter()
        .map(|edge| edge.free_flow_seconds)
        .collect();
    let candidate_edge_travel_seconds: Vec<u32> = profile
        .opening
        .edges
        .iter()
        .map(|edge| edge.free_flow_seconds)
        .collect();
    let candidate_served_people = profile.assignment.served_people;
    let scene_owned = candidate_scene.owned_capacity_bytes();
    let prior_scene_sha256 = prior_scene.state_hash();
    let candidate_scene_sha256 = candidate_scene.state_hash();
    let prior_terrain_sha256 = terrain.state_hash();
    let candidate_terrain_sha256 = terrain.state_hash();
    let candidate_owned_capacity_bytes = size_of::<GeometryRevisionCandidate>()
        + (scene_owned - size_of::<Scene>())
        + profile_bytes.capacity()
        + prior_edge_travel_seconds.capacity() * size_of::<u32>()
        + candidate_edge_travel_seconds.capacity() * size_of::<u32>()
        + prior_scene_sha256.capacity()
        + candidate_scene_sha256.capacity()
        + prior_terrain_sha256.capacity()
        + candidate_terrain_sha256.capacity();
    Ok(GeometryRevisionCandidate {
        prior_scene_sha256,
        candidate_scene_sha256,
        prior_adapter_sha256,
        candidate_adapter_sha256,
        prior_terrain_sha256,
        candidate_terrain_sha256,
        prior_served_people: prior_profile.assignment.served_people,
        candidate_served_people,
        prior_edge_travel_seconds,
        candidate_edge_travel_seconds,
        prior_profile_sha256: Sha256::digest(prior_profile_bytes).into(),
        candidate_profile_sha256,
        profile_bytes,
        scene: candidate_scene,
        candidate_owned_capacity_bytes,
    })
}
