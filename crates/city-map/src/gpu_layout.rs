//! Portable byte packing, tested on CPU only. No wgpu device or GPU authority.
use crate::{
    geometry::MAX_VERTICES, terrain::Terrain, Error, Frame, Point, TERRAIN_STEP_CM, Z_LIMIT_CM,
};
use serde::Serialize;
use std::mem::size_of;
pub const BLOCK_CELLS: usize = 64;
pub const HEADER_BYTES: usize = 64;
pub const TILE_UPLOAD_BUDGET_BYTES: usize = 64 * 1024;
pub const LAYOUT: &str = include_str!("gpu_layout.wgsl");
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Stamp {
    pub world: u64,
    pub branch: u64,
    pub map: u32,
    pub terrain_revision: u64,
    pub geometry_revision: u64,
    pub generation: u64,
    pub terrain_sha256: String,
}
pub struct TileUpload {
    header: [u8; HEADER_BYTES],
    heights: Vec<u8>,
    flags: Vec<u8>,
    stamp: Stamp,
    pub world_origin_cm: Point,
    pub z_origin_cm: i64,
    pub max_conversion_error_cm: f64,
}
impl TileUpload {
    pub fn pack(
        t: &Terrain,
        frame: &Frame,
        geometry_revision: u64,
        generation: u64,
        x0: usize,
        y0: usize,
        z_origin_cm: i64,
    ) -> Result<Self, Error> {
        frame.bounds()?;
        let n = t.manifest().size.vertices();
        if frame.size != t.manifest().size || frame.vertical_datum != t.manifest().vertical_datum {
            return Err(Error::Input);
        }
        if geometry_revision == 0
            || generation == 0
            || x0 >= n - 1
            || y0 >= n - 1
            || x0 % BLOCK_CELLS != 0
            || y0 % BLOCK_CELLS != 0
            || !(-Z_LIMIT_CM..=Z_LIMIT_CM).contains(&z_origin_cm)
        {
            return Err(Error::Bound);
        }
        let width = (n - 1 - x0).min(BLOCK_CELLS) + 1;
        let height = (n - 1 - y0).min(BLOCK_CELLS) + 1;
        let words = [
            width as u32,
            height as u32,
            x0 as u32,
            y0 as u32,
            TERRAIN_STEP_CM as u32,
            1,
            frame.map,
            0,
            t.revision() as u32,
            (t.revision() >> 32) as u32,
            geometry_revision as u32,
            (geometry_revision >> 32) as u32,
            generation as u32,
            (generation >> 32) as u32,
            (z_origin_cm as i32) as u32,
            0,
        ];
        let mut header = [0; HEADER_BYTES];
        for (i, w) in words.iter().enumerate() {
            header[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut heights = Vec::with_capacity(width * height * 4);
        let mut flags = Vec::with_capacity(width * height * 4);
        let mut error = 0.0f64;
        for y in y0..y0 + height {
            for x in x0..x0 + width {
                let cm = t.heights()[y * n + x];
                let meters = (cm - z_origin_cm) as f32 / 100.0;
                error =
                    error.max((f64::from(meters) * 100.0 + z_origin_cm as f64 - cm as f64).abs());
                heights.extend_from_slice(&meters.to_le_bytes());
                flags.extend_from_slice(&u32::from(t.water_at_vertex(x, y)?).to_le_bytes());
            }
        }
        if error > 0.5 {
            return Err(Error::NeedsDetailedValidation);
        }
        let stamp = Stamp {
            world: frame.world,
            branch: frame.branch,
            map: frame.map,
            terrain_revision: t.revision(),
            geometry_revision,
            generation,
            terrain_sha256: t.state_hash(),
        };
        let result = Self {
            header,
            heights,
            flags,
            stamp,
            world_origin_cm: frame.global(Point {
                x: x0 as i64 * TERRAIN_STEP_CM,
                y: y0 as i64 * TERRAIN_STEP_CM,
            })?,
            z_origin_cm,
            max_conversion_error_cm: error,
        };
        if result.owned_capacity_bytes() > TILE_UPLOAD_BUDGET_BYTES {
            return Err(Error::Limit);
        }
        Ok(result)
    }
    pub fn header(&self) -> &[u8; HEADER_BYTES] {
        &self.header
    }
    pub fn heights(&self) -> &[u8] {
        &self.heights
    }
    pub fn flags(&self) -> &[u8] {
        &self.flags
    }
    pub fn stamp(&self) -> &Stamp {
        &self.stamp
    }
    /// Trusted host compares its current immutable snapshot and job generation.
    pub fn matches_current(&self, current: &Stamp) -> bool {
        &self.stamp == current
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>()
            + self.heights.capacity()
            + self.flags.capacity()
            + self.stamp.terrain_sha256.capacity()
    }
}
/// vec4<f32> stride 16, block-local meters. This is a rendering/candidate projection.
/// A span beyond one 256m block needs explicit clipping/chunking, never global f32.
pub fn vertices(
    points: &[Point],
    z_cm: i64,
    origin: Point,
    z_origin_cm: i64,
) -> Result<Vec<u8>, Error> {
    origin.validate()?;
    if points.len() > MAX_VERTICES
        || !(-Z_LIMIT_CM..=Z_LIMIT_CM).contains(&z_cm)
        || !(-Z_LIMIT_CM..=Z_LIMIT_CM).contains(&z_origin_cm)
    {
        return Err(Error::Limit);
    }
    let mut result = Vec::with_capacity(points.len() * 16);
    for p in points {
        p.validate()?;
        let x = p.x - origin.x;
        let y = p.y - origin.y;
        if !(0..=25600).contains(&x) || !(0..=25600).contains(&y) {
            return Err(Error::Bound);
        }
        let values = [
            x as f32 / 100.0,
            y as f32 / 100.0,
            (z_cm - z_origin_cm) as f32 / 100.0,
            1.0,
        ];
        for f in values {
            result.extend_from_slice(&f.to_le_bytes());
        }
    }
    Ok(result)
}
