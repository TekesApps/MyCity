//! Bounded adapter from one validated flat WALK scene to the abstract transport graph.
//!
//! This deliberately does not infer graph connections from proximity. Capacities and
//! operating speed are supplied by facility configuration, while route length is derived
//! from the validated spatial corridors. It supports one building entrance, one connector,
//! one quadratic road, and one explicitly granted terminal only.
use crate::{
    geometry::{Mode, Port},
    scene::Scene,
};
use mycity_core::transport::{self, Entrance, Node, OdDemand, Opening, RoadEdge};

pub const WALK_MODE_MASK: u8 = 1;
pub const MAX_ADAPTER_DEMANDS: usize = transport::MAX_DEMANDS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterError {
    InvalidConfig,
    InvalidGeometryBinding,
    Permission,
    UnsupportedMode,
    Bounds,
    Overflow,
    Transport(transport::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadDirection {
    StartToEnd,
    EndToStart,
    Both,
}

/// Positive capacity and speed are per assignment period and centimetres per second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FacilityOperation {
    pub capacity_people_per_period: u32,
    pub free_flow_speed_cm_per_second: u32,
    pub open: bool,
}

/// Explicit permission to attach the scene's road start to an existing graph terminal.
/// This is a declared topology grant, never a nearest-port lookup.
#[derive(Clone, Copy, Debug)]
pub struct TerminalGrant {
    pub owner_id: u32,
    pub entrance_id: u32,
    pub port: Port,
}

/// All IDs are stable within the resulting Opening and must be nonzero and distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdapterIds {
    pub building_node: u32,
    pub anchor_node: u32,
    pub terminal_node: u32,
    pub building_entrance: u32,
    pub connector_forward_edge: u32,
    pub connector_reverse_edge: u32,
    pub road_forward_edge: u32,
    pub road_reverse_edge: u32,
}

pub struct AdapterInput<'a> {
    /// Must equal the validated scene map identifier.
    pub expected_map_id: u32,
    /// Transport's city identifier; map identifiers are not silently treated as cities.
    pub city_id: u32,
    pub revision: u64,
    pub building_owner_id: u32,
    pub ids: AdapterIds,
    pub terminal: TerminalGrant,
    pub connector: FacilityOperation,
    pub road: FacilityOperation,
    pub road_direction: RoadDirection,
    pub demands: &'a [OdDemand],
}

/// Convert a validated `Scene` into a tiny explicit WALK graph. The caller remains
/// responsible for combining this graph with other city facilities and transport inputs.
pub fn adapt(scene: &Scene, input: AdapterInput<'_>) -> Result<Opening, AdapterError> {
    if input.expected_map_id == 0
        || input.city_id == 0
        || input.revision == 0
        || input.building_owner_id == 0
        || input.terminal.owner_id == 0
        || input.terminal.entrance_id == 0
    {
        return Err(AdapterError::InvalidConfig);
    }
    if input.demands.len() > MAX_ADAPTER_DEMANDS {
        return Err(AdapterError::Bounds);
    }
    if input.expected_map_id != scene.input().map.map {
        return Err(AdapterError::InvalidGeometryBinding);
    }
    if scene.input().road_mode != Mode::Walk
        || scene.input().door.mode != Mode::Walk
        || scene.input().anchor.mode != Mode::Walk
        || input.terminal.port.mode != Mode::Walk
    {
        return Err(AdapterError::UnsupportedMode);
    }
    let road_points = scene.road().points();
    if road_points.len() < 2
        || scene.connector().points().first() != Some(&scene.input().door.point)
        || scene.connector().points().last() != Some(&scene.input().anchor.point)
        || road_points.last() != Some(&scene.input().anchor.point)
        || input.terminal.port.point != road_points[0]
        || input.terminal.port.z_cm != scene.input().ground_z_cm
        || i64::from(input.terminal.port.clear_width_cm) < scene.input().road_width_cm
        || scene.input().door.z_cm != scene.input().ground_z_cm
        || scene.input().anchor.z_cm != scene.input().ground_z_cm
    {
        return Err(AdapterError::InvalidGeometryBinding);
    }
    if !scene.input().door.authorized
        || !scene.input().anchor.authorized
        || !input.terminal.port.authorized
    {
        return Err(AdapterError::Permission);
    }
    for facility in [input.connector, input.road] {
        if facility.capacity_people_per_period == 0 || facility.free_flow_speed_cm_per_second == 0 {
            return Err(AdapterError::InvalidConfig);
        }
    }
    let ids = [
        input.ids.building_node,
        input.ids.anchor_node,
        input.ids.terminal_node,
        input.ids.building_entrance,
        input.terminal.entrance_id,
        input.ids.connector_forward_edge,
        input.ids.connector_reverse_edge,
        input.ids.road_forward_edge,
        input.ids.road_reverse_edge,
    ];
    if ids.contains(&0) {
        return Err(AdapterError::InvalidConfig);
    }
    for (index, id) in ids.iter().enumerate() {
        if ids[..index].contains(id) {
            return Err(AdapterError::InvalidConfig);
        }
    }
    if input.demands.iter().any(|d| {
        d.mode != WALK_MODE_MASK
            || ![input.ids.building_entrance, input.terminal.entrance_id]
                .contains(&d.origin_entrance)
            || ![input.ids.building_entrance, input.terminal.entrance_id]
                .contains(&d.destination_entrance)
    }) {
        return Err(AdapterError::UnsupportedMode);
    }

    let connector_seconds = travel_seconds(
        scene.connector().length_upper_cm(),
        input.connector.free_flow_speed_cm_per_second,
    )?;
    let road_seconds = travel_seconds(
        scene.road().length_upper_cm(),
        input.road.free_flow_speed_cm_per_second,
    )?;
    let mut edges = Vec::with_capacity(4);
    let door = scene.input().door;
    let anchor = scene.input().anchor;
    if door.can_exit && anchor.can_enter {
        edges.push(edge(
            input.ids.connector_forward_edge,
            input.ids.building_node,
            input.ids.anchor_node,
            input.city_id,
            connector_seconds,
            input.connector,
        ));
    }
    if anchor.can_exit && door.can_enter {
        edges.push(edge(
            input.ids.connector_reverse_edge,
            input.ids.anchor_node,
            input.ids.building_node,
            input.city_id,
            connector_seconds,
            input.connector,
        ));
    }
    // Port permissions describe movement across the terminal boundary: entering this
    // scene follows road start -> end, while leaving follows end -> start. This matches
    // `geometry::seam`, which requires source.can_exit and destination.can_enter.
    if input.terminal.port.can_enter
        && matches!(
            input.road_direction,
            RoadDirection::StartToEnd | RoadDirection::Both
        )
    {
        edges.push(edge(
            input.ids.road_forward_edge,
            input.ids.terminal_node,
            input.ids.anchor_node,
            input.city_id,
            road_seconds,
            input.road,
        ));
    }
    if input.terminal.port.can_exit
        && matches!(
            input.road_direction,
            RoadDirection::EndToStart | RoadDirection::Both
        )
    {
        edges.push(edge(
            input.ids.road_reverse_edge,
            input.ids.anchor_node,
            input.ids.terminal_node,
            input.city_id,
            road_seconds,
            input.road,
        ));
    }

    let opening = Opening {
        revision: input.revision,
        nodes: vec![
            Node {
                id: input.ids.building_node,
                city_id: input.city_id,
            },
            Node {
                id: input.ids.anchor_node,
                city_id: input.city_id,
            },
            Node {
                id: input.ids.terminal_node,
                city_id: input.city_id,
            },
        ],
        edges,
        entrances: vec![
            Entrance {
                id: input.ids.building_entrance,
                owner: input.building_owner_id,
                node: input.ids.building_node,
                mode_mask: WALK_MODE_MASK,
                authorized: door.authorized,
            },
            Entrance {
                id: input.terminal.entrance_id,
                owner: input.terminal.owner_id,
                node: input.ids.terminal_node,
                mode_mask: WALK_MODE_MASK,
                authorized: input.terminal.port.authorized,
            },
        ],
        demands: input.demands.to_vec(),
    };
    opening.validate().map_err(AdapterError::Transport)?;
    Ok(opening)
}

fn edge(
    id: u32,
    from: u32,
    to: u32,
    city_id: u32,
    free_flow_seconds: u32,
    facility: FacilityOperation,
) -> RoadEdge {
    RoadEdge {
        id,
        from,
        to,
        city_from: city_id,
        city_to: city_id,
        free_flow_seconds,
        capacity_people: facility.capacity_people_per_period,
        mode_mask: WALK_MODE_MASK,
        open: facility.open,
        boundary_source: None,
    }
}

fn travel_seconds(length_upper_cm: u64, speed_cm_per_second: u32) -> Result<u32, AdapterError> {
    let speed = u64::from(speed_cm_per_second);
    let seconds = length_upper_cm
        .checked_add(speed - 1)
        .ok_or(AdapterError::Overflow)?
        / speed;
    u32::try_from(seconds.max(1)).map_err(|_| AdapterError::Overflow)
}
