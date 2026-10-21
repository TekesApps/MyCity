//! One immutable, validated local scene; deliberately not a city construction transaction.
mod workspace;
use crate::{geometry::*, terrain::Terrain, Error, Frame, Point, GEOMETRY_BUDGET_BYTES};
use mycity_core::sha256;
use serde::{Deserialize, Serialize};
use std::mem::size_of;
pub use workspace::{SceneWorkspace, SceneWorkspaceIdentity};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[serde(deserialize_with = "crate::bounded::text::<_, 64>")]
    pub schema: String,
    pub geometry_revision: u64,
    pub terrain_revision: u64,
    pub map: Frame,
    pub neighbor: Frame,
    pub region: Region,
    pub building: Prism,
    pub ground_z_cm: i64,
    pub door: Port,
    pub anchor: Port,
    #[serde(deserialize_with = "crate::bounded::vector::<_, _, 32>")]
    pub connector: Vec<Point>,
    pub connector_width_cm: i64,
    pub road: Quadratic,
    pub road_width_cm: i64,
    pub road_mode: Mode,
    #[serde(deserialize_with = "crate::bounded::vector::<_, _, 64>")]
    pub obstacles: Vec<Prism>,
    #[serde(deserialize_with = "crate::bounded::vector::<_, _, 64>")]
    pub neighbor_obstacles: Vec<Prism>,
    pub seam_from: Port,
    pub seam_to: Port,
}
pub struct Scene {
    input: Input,
    road: Corridor,
    connector: Corridor,
    terrain_sha256: String,
    neighbor_terrain_sha256: String,
}
impl Scene {
    pub fn assemble(
        input: Input,
        terrain: &Terrain,
        neighbor_terrain: &Terrain,
    ) -> Result<Self, Error> {
        Self::assemble_internal(input, terrain, neighbor_terrain, None)
    }
    pub fn assemble_in_workspace(
        input: Input,
        terrain: &Terrain,
        neighbor_terrain: &Terrain,
        workspace: SceneWorkspace,
    ) -> Result<Self, Error> {
        Self::assemble_internal(input, terrain, neighbor_terrain, Some(workspace))
    }
    fn assemble_internal(
        input: Input,
        terrain: &Terrain,
        neighbor_terrain: &Terrain,
        mut workspace: Option<SceneWorkspace>,
    ) -> Result<Self, Error> {
        let input = if let Some(w) = workspace.as_mut() {
            w.retain_input(input)?
        } else {
            input
        };
        if input.schema != "mycity-m0-geometry-v1" || input.geometry_revision == 0 {
            return Err(Error::Input);
        }
        if input.terrain_revision != terrain.revision()
            || input.terrain_revision != neighbor_terrain.revision()
        {
            return Err(Error::Stale);
        }
        if input.connector.len() > 32
            || input.obstacles.len() > 64
            || input.neighbor_obstacles.len() > 64
        {
            return Err(Error::Limit);
        }
        for (map, t) in [(&input.map, terrain), (&input.neighbor, neighbor_terrain)] {
            map.bounds()?;
            if map.size != t.manifest().size || map.vertical_datum != t.manifest().vertical_datum {
                return Err(Error::Input);
            }
        }
        let local = crate::Rect {
            min: Point { x: 0, y: 0 },
            max: Point {
                x: input.map.size.side_cm(),
                y: input.map.size.side_cm(),
            },
        };
        if !local.contains(input.region.outer) {
            return Err(Error::Bound);
        }
        input.region.contains_rect(input.building.footprint)?;
        input.building.validate()?;
        if !matches!(input.building.layer, Layer::G)
            || input.ground_z_cm <= input.building.z_min_cm
            || input.ground_z_cm >= input.building.z_max_cm
            || input.door.z_cm != input.ground_z_cm
        {
            return Err(Error::NeedsDetailedValidation);
        }
        terrain.flat_platform(input.building.footprint, input.ground_z_cm)?;
        let road = if let Some(w) = workspace.as_mut() {
            Corridor::quadratic_in(input.road, input.road_width_cm, std::mem::take(&mut w.road))?
        } else {
            Corridor::quadratic(input.road, input.road_width_cm)?
        };
        road.validate_region(&input.region)?;
        // Flat support proof is conservative. No claim of grade, earthworks or bridge solver.
        terrain.flat_platform(road.bounds()?, input.ground_z_cm)?;
        if road.intersects_rect(input.building.footprint)? {
            return Err(Error::NeedsDetailedValidation);
        }
        if input.road_mode != Mode::Walk
            || input.anchor.mode != input.road_mode
            || input.anchor.point != input.road.end
            || input.anchor.z_cm != input.ground_z_cm
        {
            return Err(Error::Unsupported);
        }
        for obstacle in &input.obstacles {
            obstacle.validate()?;
            if input.building.collision(*obstacle, 0)?
                || (obstacle.z_min_cm < input.ground_z_cm + 200
                    && obstacle.z_max_cm > input.ground_z_cm
                    && road.intersects_rect(obstacle.footprint)?)
            {
                return Err(Error::NeedsDetailedValidation);
            }
        }
        let connector = if let Some(w) = workspace.as_mut() {
            Corridor::polyline_in(
                &input.connector,
                input.connector_width_cm,
                std::mem::take(&mut w.connector),
            )?
        } else {
            Corridor::from_polyline(&input.connector, input.connector_width_cm)?
        };
        terrain.flat_platform(connector.bounds()?, input.ground_z_cm)?;
        entrance(
            input.building,
            input.door,
            input.anchor,
            &connector,
            &input.region,
            &input.obstacles,
        )?;
        if input.seam_from.mode != Mode::Walk || input.seam_to.mode != Mode::Walk {
            return Err(Error::Unsupported);
        }
        seam(&input.map, input.seam_from, &input.neighbor, input.seam_to)?;
        for (frame, port, t, obstacles) in [
            (
                &input.map,
                input.seam_from,
                terrain,
                input.obstacles.as_slice(),
            ),
            (
                &input.neighbor,
                input.seam_to,
                neighbor_terrain,
                input.neighbor_obstacles.as_slice(),
            ),
        ] {
            let side = frame.size.side_cm();
            let half = (i64::from(port.clear_width_cm) + 1) / 2;
            let (min, max) = match port.facing {
                Facing::East => (
                    Point {
                        x: side - 200,
                        y: port.point.y - half,
                    },
                    Point {
                        x: side,
                        y: port.point.y + half,
                    },
                ),
                Facing::West => (
                    Point {
                        x: 0,
                        y: port.point.y - half,
                    },
                    Point {
                        x: 200,
                        y: port.point.y + half,
                    },
                ),
                Facing::North => (
                    Point {
                        x: port.point.x - half,
                        y: side - 200,
                    },
                    Point {
                        x: port.point.x + half,
                        y: side,
                    },
                ),
                Facing::South => (
                    Point {
                        x: port.point.x - half,
                        y: 0,
                    },
                    Point {
                        x: port.point.x + half,
                        y: 200,
                    },
                ),
            };
            let support = crate::Rect { min, max };
            t.flat_platform(support, port.z_cm)?;
            if frame.map == input.map.map && support.overlaps(input.building.footprint) {
                return Err(Error::Collision);
            }
            for obstacle in obstacles {
                obstacle.validate()?;
                if support.overlaps(obstacle.footprint)
                    && obstacle.z_min_cm < port.z_cm + 200
                    && obstacle.z_max_cm > port.z_cm
                {
                    return Err(Error::Collision);
                }
            }
        }
        let mut terrain_sha256 = terrain.state_hash();
        let mut neighbor_terrain_sha256 = if std::ptr::eq(terrain, neighbor_terrain) {
            terrain_sha256.clone()
        } else {
            neighbor_terrain.state_hash()
        };
        if let Some(w) = workspace.as_mut() {
            w.retain_hashes(&mut terrain_sha256, &mut neighbor_terrain_sha256)?;
        }
        let result = Self {
            terrain_sha256,
            neighbor_terrain_sha256,
            input,
            road,
            connector,
        };
        if result.owned_capacity_bytes() > GEOMETRY_BUDGET_BYTES {
            return Err(Error::Limit);
        }
        Ok(result)
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn road(&self) -> &Corridor {
        &self.road
    }
    pub fn connector(&self) -> &Corridor {
        &self.connector
    }
    pub fn state_hash(&self) -> String {
        sha256(&serde_json::to_vec(&serde_json::json!({"schema":"mycity-m0-spatial-state-v1","rules":"integer-capsule-flat-support-v1","terrain_sha256":self.terrain_sha256,"neighbor_terrain_sha256":self.neighbor_terrain_sha256,"input":self.input,"road":self.road,"connector":self.connector})).expect("bounded geometry"))
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        let frame_bytes = |f: &Frame| f.metric_frame.capacity() + f.vertical_datum.capacity();
        size_of::<Self>()
            + self.terrain_sha256.capacity()
            + self.neighbor_terrain_sha256.capacity()
            + self.input.schema.capacity()
            + frame_bytes(&self.input.map)
            + frame_bytes(&self.input.neighbor)
            + self.input.region.holes.capacity() * size_of::<crate::Rect>()
            + self.input.connector.capacity() * size_of::<Point>()
            + (self.input.obstacles.capacity() + self.input.neighbor_obstacles.capacity())
                * size_of::<Prism>()
            + self.road.owned_capacity_bytes()
            - size_of::<Corridor>()
            + self.connector.owned_capacity_bytes()
            - size_of::<Corridor>()
    }
}
