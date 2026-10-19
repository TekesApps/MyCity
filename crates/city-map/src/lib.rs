//! Immutable, bounded spatial laboratory. No UI or implicit construction authority.
use serde::{Deserialize, Serialize};
pub mod geometry;
pub mod gpu_layout;
pub mod probe;
pub mod scene;
pub mod terrain;

pub const SCHEMA: &str = "mycity-m0-spatial-v1";
pub const XY_LIMIT_CM: i64 = 1_000_000_000;
pub const Z_LIMIT_CM: i64 = 1_000_000;
pub const LAND_CELL_CM: i64 = 1600;
pub const TERRAIN_STEP_CM: i64 = 400;
pub const GEOMETRY_BUDGET_BYTES: usize = 1024 * 1024;
pub const TERRAIN_RESIDENT_BUDGET_BYTES: usize = 10 * 1024 * 1024;
pub const IMPORT_PEAK_BUDGET_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    Input,
    Unsupported,
    Bound,
    Limit,
    Hash,
    Png,
    NoData,
    Collision,
    NeedsDetailedValidation,
    Direction,
    Layer,
    Permission,
    Seam,
    Stale,
    Overflow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}
impl Point {
    pub fn validate(self) -> Result<(), Error> {
        if self.x < -XY_LIMIT_CM
            || self.x > XY_LIMIT_CM
            || self.y < -XY_LIMIT_CM
            || self.y > XY_LIMIT_CM
        {
            return Err(Error::Bound);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}
impl Rect {
    pub fn validate(self) -> Result<(), Error> {
        self.min.validate()?;
        self.max.validate()?;
        if self.min.x >= self.max.x || self.min.y >= self.max.y {
            return Err(Error::Input);
        }
        Ok(())
    }
    pub fn contains_point(self, p: Point) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn contains(self, other: Self) -> bool {
        self.contains_point(other.min) && self.contains_point(other.max)
    }
    pub fn overlaps(self, other: Self) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }
    pub fn expanded(self, margin: i64) -> Result<Self, Error> {
        self.validate()?;
        if !(0..=100_000).contains(&margin) {
            return Err(Error::Bound);
        }
        let result = Self {
            min: Point {
                x: self.min.x - margin,
                y: self.min.y - margin,
            },
            max: Point {
                x: self.max.x + margin,
                y: self.max.y + margin,
            },
        };
        result.validate()?;
        Ok(result)
    }
    pub fn area_cm2(self) -> Result<i128, Error> {
        self.validate()?;
        Ok(i128::from(self.max.x - self.min.x) * i128::from(self.max.y - self.min.y))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapSize {
    Small,
    Medium,
    Large,
}
impl MapSize {
    pub fn cells(self) -> u32 {
        match self {
            Self::Small => 64,
            Self::Medium => 128,
            Self::Large => 256,
        }
    }
    pub fn side_cm(self) -> i64 {
        i64::from(self.cells()) * LAND_CELL_CM
    }
    pub fn vertices(self) -> usize {
        (self.side_cm() / TERRAIN_STEP_CM + 1) as usize
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub world: u64,
    pub branch: u64,
    pub map: u32,
    pub size: MapSize,
    pub origin: Point,
    pub metric_frame: String,
    pub vertical_datum: String,
}
impl Frame {
    pub fn bounds(&self) -> Result<Rect, Error> {
        self.origin.validate()?;
        if self.world == 0
            || self.branch == 0
            || self.map == 0
            || self.metric_frame.is_empty()
            || self.vertical_datum.is_empty()
            || self.metric_frame.len() > 64
            || self.vertical_datum.len() > 64
        {
            return Err(Error::Input);
        }
        let max = Point {
            x: self.origin.x + self.size.side_cm(),
            y: self.origin.y + self.size.side_cm(),
        };
        let r = Rect {
            min: self.origin,
            max,
        };
        r.validate()?;
        Ok(r)
    }
    pub fn global(&self, local: Point) -> Result<Point, Error> {
        self.bounds()?;
        if local.x < 0
            || local.y < 0
            || local.x > self.size.side_cm()
            || local.y > self.size.side_cm()
        {
            return Err(Error::Bound);
        }
        Ok(Point {
            x: self.origin.x + local.x,
            y: self.origin.y + local.y,
        })
    }
    /// Half-open cells. A point on the far outer boundary has no cell.
    pub fn cell(&self, local: Point) -> Result<(u32, u32), Error> {
        self.global(local)?;
        if local.x == self.size.side_cm() || local.y == self.size.side_cm() {
            return Err(Error::Bound);
        }
        Ok((
            (local.x / LAND_CELL_CM) as u32,
            (local.y / LAND_CELL_CM) as u32,
        ))
    }
}

pub(crate) fn round_ratio(n: i128, d: i128) -> Result<i64, Error> {
    if d <= 0 {
        return Err(Error::Input);
    }
    let magnitude = n.checked_abs().ok_or(Error::Overflow)?;
    let result = magnitude.checked_add(d / 2).ok_or(Error::Overflow)? / d;
    i64::try_from(if n < 0 { -result } else { result }).map_err(|_| Error::Overflow)
}
