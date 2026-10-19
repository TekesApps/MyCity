//! Bounded exact predicates and conservative corridors. Never infer network adjacency.
use crate::{
    round_ratio, Error, Frame, Point, Rect, GEOMETRY_BUDGET_BYTES, XY_LIMIT_CM, Z_LIMIT_CM,
};
use serde::{Deserialize, Serialize};
use std::mem::size_of;
pub const MAX_HOLES: usize = 8;
pub const MAX_VERTICES: usize = 1025;
pub const CURVE_ERROR_CM: i64 = 5;
const SCALE: i128 = 1i128 << 32;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub outer: Rect,
    pub holes: Vec<Rect>,
}
impl Region {
    pub fn validate(&self) -> Result<(), Error> {
        self.outer.validate()?;
        if self.holes.len() > MAX_HOLES {
            return Err(Error::Limit);
        }
        for (i, h) in self.holes.iter().enumerate() {
            h.validate()?;
            if h.min.x <= self.outer.min.x
                || h.min.y <= self.outer.min.y
                || h.max.x >= self.outer.max.x
                || h.max.y >= self.outer.max.y
            {
                return Err(Error::Input);
            }
            if self.holes[..i].iter().any(|old| old.overlaps(*h)) {
                return Err(Error::Input);
            }
        }
        Ok(())
    }
    pub fn contains_rect(&self, r: Rect) -> Result<(), Error> {
        self.validate()?;
        r.validate()?;
        if !self.outer.contains(r) || self.holes.iter().any(|h| h.overlaps(r)) {
            return Err(Error::Permission);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    U1,
    U2,
    U3,
    G,
    A1,
    A2,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prism {
    pub footprint: Rect,
    pub z_min_cm: i64,
    pub z_max_cm: i64,
    pub layer: Layer,
}
impl Prism {
    pub fn validate(self) -> Result<(), Error> {
        self.footprint.validate()?;
        if self.z_min_cm < -Z_LIMIT_CM
            || self.z_max_cm > Z_LIMIT_CM
            || self.z_min_cm >= self.z_max_cm
        {
            return Err(Error::Bound);
        }
        Ok(())
    }
    /// Layer labels do not exempt physical overlap. Boundary contact is allowed at zero clearance.
    pub fn collision(self, other: Self, clearance_cm: i64) -> Result<bool, Error> {
        self.validate()?;
        other.validate()?;
        let inflated = self.footprint.expanded(clearance_cm)?;
        Ok(inflated.overlaps(other.footprint)
            && self.z_min_cm - clearance_cm < other.z_max_cm
            && other.z_min_cm < self.z_max_cm + clearance_cm)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quadratic {
    pub start: Point,
    pub control: Point,
    pub end: Point,
}
#[derive(Debug, Serialize)]
pub struct Corridor {
    points: Vec<Point>,
    radius_cm: i64,
    error_cm: i64,
}
impl Corridor {
    pub fn points(&self) -> &[Point] {
        &self.points
    }
    pub fn radius_cm(&self) -> i64 {
        self.radius_cm
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.points.capacity() * size_of::<Point>()
    }
    pub fn from_polyline(points: &[Point], width_cm: i64) -> Result<Self, Error> {
        if points.len() > MAX_VERTICES {
            return Err(Error::Limit);
        }
        Self::make(points.to_vec(), width_cm, 0)
    }
    fn make(points: Vec<Point>, width_cm: i64, error_cm: i64) -> Result<Self, Error> {
        if points.len() < 2 || points.len() > MAX_VERTICES || !(1..=20000).contains(&width_cm) {
            return Err(Error::Limit);
        }
        for p in &points {
            p.validate()?;
        }
        if points.windows(2).any(|p| p[0] == p[1]) {
            return Err(Error::Input);
        }
        let result = Self {
            points,
            radius_cm: (width_cm + 1) / 2 + error_cm,
            error_cm,
        };
        if result.owned_capacity_bytes() > GEOMETRY_BUDGET_BYTES {
            return Err(Error::Limit);
        }
        Ok(result)
    }
    pub fn quadratic(q: Quadratic, width_cm: i64) -> Result<Self, Error> {
        for p in [q.start, q.control, q.end] {
            p.validate()?;
        }
        if q.start == q.end {
            return Err(Error::Input);
        }
        type P = (i128, i128);
        fn half(a: P, b: P) -> P {
            ((a.0 + b.0) / 2, (a.1 + b.1) / 2)
        }
        fn subdivide(a: P, b: P, c: P, depth: u32, out: &mut Vec<Point>) -> Result<(), Error> {
            // Max vector distance from the chord at the SAME parameter is <= |2b-a-c|/4.
            // L1 bounds Euclidean distance: 4cm analytical + <1cm endpoint quantization.
            let error = (2 * b.0 - a.0 - c.0).abs() + (2 * b.1 - a.1 - c.1).abs();
            if error <= 4 * 4 * SCALE {
                if out.len() == MAX_VERTICES {
                    return Err(Error::Limit);
                }
                let p = Point {
                    x: round_ratio(c.0, SCALE)?,
                    y: round_ratio(c.1, SCALE)?,
                };
                if out.last() != Some(&p) {
                    out.push(p);
                }
                return Ok(());
            }
            if depth == 16 {
                return Err(Error::Limit);
            }
            let ab = half(a, b);
            let bc = half(b, c);
            let mid = half(ab, bc);
            subdivide(a, ab, mid, depth + 1, out)?;
            subdivide(mid, bc, c, depth + 1, out)
        }
        let mut points = Vec::with_capacity(MAX_VERTICES);
        points.push(q.start);
        let p = |p: Point| (i128::from(p.x) * SCALE, i128::from(p.y) * SCALE);
        subdivide(p(q.start), p(q.control), p(q.end), 0, &mut points)?;
        Self::make(points, width_cm, CURVE_ERROR_CM)
    }
    pub fn bounds(&self) -> Result<Rect, Error> {
        let min = Point {
            x: self.points.iter().map(|p| p.x).min().ok_or(Error::Input)?,
            y: self.points.iter().map(|p| p.y).min().ok_or(Error::Input)?,
        };
        let max = Point {
            x: self.points.iter().map(|p| p.x).max().ok_or(Error::Input)?,
            y: self.points.iter().map(|p| p.y).max().ok_or(Error::Input)?,
        };
        // Horizontal/vertical centerlines still have nonzero physical width.
        let result = Rect {
            min: Point {
                x: min.x - self.radius_cm,
                y: min.y - self.radius_cm,
            },
            max: Point {
                x: max.x + self.radius_cm,
                y: max.y + self.radius_cm,
            },
        };
        result.validate()?;
        Ok(result)
    }
    pub fn validate_region(&self, region: &Region) -> Result<(), Error> {
        region.validate()?;
        if !region.outer.contains(self.bounds()?) {
            return Err(Error::Permission);
        }
        for h in &region.holes {
            if self.intersects_rect(*h)? {
                return Err(Error::Permission);
            }
        }
        Ok(())
    }
    /// Exact capsule-vs-rectangle test for the frozen polyline and expanded radius.
    /// A conflict with the original curved road may be conservative within error_cm.
    pub fn intersects_rect(&self, r: Rect) -> Result<bool, Error> {
        r.validate()?;
        for s in self.points.windows(2) {
            if r.contains_point(s[0]) || r.contains_point(s[1]) {
                return Ok(true);
            }
            let corners = [
                r.min,
                Point {
                    x: r.max.x,
                    y: r.min.y,
                },
                r.max,
                Point {
                    x: r.min.x,
                    y: r.max.y,
                },
            ];
            for i in 0..4 {
                if segments_touch(s[0], s[1], corners[i], corners[(i + 1) % 4])
                    || point_near(s[0], corners[i], corners[(i + 1) % 4], self.radius_cm)
                    || point_near(s[1], corners[i], corners[(i + 1) % 4], self.radius_cm)
                    || point_near(corners[i], s[0], s[1], self.radius_cm)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    /// ceil of each segment length in centimeters, an explicit conservative length estimate.
    pub fn length_upper_cm(&self) -> u64 {
        self.points
            .windows(2)
            .map(|s| {
                let dx = i128::from(s[1].x - s[0].x);
                let dy = i128::from(s[1].y - s[0].y);
                let square = (dx * dx + dy * dy) as u128;
                let mut low = 0u64;
                let mut high = 4 * XY_LIMIT_CM as u64;
                while low < high {
                    let mid = low + (high - low) / 2;
                    if u128::from(mid) * u128::from(mid) < square {
                        low = mid + 1
                    } else {
                        high = mid
                    }
                }
                low
            })
            .sum()
    }
}
fn cross(a: Point, b: Point, c: Point) -> i128 {
    i128::from(b.x - a.x) * i128::from(c.y - a.y) - i128::from(b.y - a.y) * i128::from(c.x - a.x)
}
fn on(a: Point, b: Point, p: Point) -> bool {
    cross(a, b, p) == 0
        && p.x >= a.x.min(b.x)
        && p.x <= a.x.max(b.x)
        && p.y >= a.y.min(b.y)
        && p.y <= a.y.max(b.y)
}
fn segments_touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ac = cross(a, b, c);
    let ad = cross(a, b, d);
    let ca = cross(c, d, a);
    let cb = cross(c, d, b);
    (ac.signum() != ad.signum()
        && ac != 0
        && ad != 0
        && ca.signum() != cb.signum()
        && ca != 0
        && cb != 0)
        || on(a, b, c)
        || on(a, b, d)
        || on(c, d, a)
        || on(c, d, b)
}
fn point_near(p: Point, a: Point, b: Point, r: i64) -> bool {
    let dx = i128::from(b.x - a.x);
    let dy = i128::from(b.y - a.y);
    let px = i128::from(p.x - a.x);
    let py = i128::from(p.y - a.y);
    let length = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let r2 = i128::from(r) * i128::from(r);
    if dot <= 0 {
        return px * px + py * py <= r2;
    }
    if dot >= length {
        let x = i128::from(p.x - b.x);
        let y = i128::from(p.y - b.y);
        return x * x + y * y <= r2;
    }
    let area = dx * py - dy * px;
    area * area <= r2 * length
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Walk,
    Vehicle,
    Water,
    Electric,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facing {
    East,
    North,
    West,
    South,
}
impl Facing {
    fn vector(self) -> Point {
        match self {
            Self::East => Point { x: 1, y: 0 },
            Self::West => Point { x: -1, y: 0 },
            Self::North => Point { x: 0, y: 1 },
            Self::South => Point { x: 0, y: -1 },
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    pub point: Point,
    pub z_cm: i64,
    pub mode: Mode,
    pub facing: Facing,
    pub clear_width_cm: u32,
    pub can_enter: bool,
    pub can_exit: bool,
    pub authorized: bool,
}
impl Port {
    fn validate(self) -> Result<(), Error> {
        self.point.validate()?;
        if self.z_cm < -Z_LIMIT_CM
            || self.z_cm > Z_LIMIT_CM
            || self.clear_width_cm == 0
            || self.clear_width_cm > 20000
        {
            return Err(Error::Bound);
        }
        Ok(())
    }
}
pub fn seam(a: &Frame, ap: Port, b: &Frame, bp: Port) -> Result<(), Error> {
    ap.validate()?;
    bp.validate()?;
    let ar = a.bounds()?;
    let br = b.bounds()?;
    if a.world != b.world
        || a.branch != b.branch
        || a.map == b.map
        || a.metric_frame != b.metric_frame
        || a.vertical_datum != b.vertical_datum
        || ar.overlaps(br)
        || a.global(ap.point)? != b.global(bp.point)?
    {
        return Err(Error::Seam);
    }
    if ap.clear_width_cm != bp.clear_width_cm {
        return Err(Error::Unsupported);
    }
    let margin = (i64::from(ap.clear_width_cm) + 1) / 2;
    let p = a.global(ap.point)?;
    let joint = if ar.max.x == br.min.x {
        p.x == ar.max.x
            && p.y - margin >= ar.min.y.max(br.min.y)
            && p.y + margin <= ar.max.y.min(br.max.y)
            && ap.facing == Facing::East
            && bp.facing == Facing::West
    } else if ar.min.x == br.max.x {
        p.x == ar.min.x
            && p.y - margin >= ar.min.y.max(br.min.y)
            && p.y + margin <= ar.max.y.min(br.max.y)
            && ap.facing == Facing::West
            && bp.facing == Facing::East
    } else if ar.max.y == br.min.y {
        p.y == ar.max.y
            && p.x - margin >= ar.min.x.max(br.min.x)
            && p.x + margin <= ar.max.x.min(br.max.x)
            && ap.facing == Facing::North
            && bp.facing == Facing::South
    } else if ar.min.y == br.max.y {
        p.y == ar.min.y
            && p.x - margin >= ar.min.x.max(br.min.x)
            && p.x + margin <= ar.max.x.min(br.max.x)
            && ap.facing == Facing::South
            && bp.facing == Facing::North
    } else {
        false
    };
    if !joint || ap.z_cm != bp.z_cm || ap.mode != bp.mode {
        return Err(Error::Seam);
    }
    if !ap.authorized || !bp.authorized {
        return Err(Error::Permission);
    }
    if !ap.can_exit || !bp.can_enter {
        return Err(Error::Direction);
    }
    Ok(())
}
/// The first outward segment must follow the door normal. Later segments may turn.
/// Candidate radius never changes the submitted endpoint; a mismatch requires a new plan.
pub fn entrance(
    building: Prism,
    door: Port,
    anchor: Port,
    path: &Corridor,
    region: &Region,
    obstacles: &[Prism],
) -> Result<(), Error> {
    building.validate()?;
    door.validate()?;
    anchor.validate()?;
    if door.mode != Mode::Walk || anchor.mode != Mode::Walk {
        return Err(Error::Unsupported);
    }
    if i64::from(door.clear_width_cm) < path.radius_cm * 2
        || i64::from(anchor.clear_width_cm) < path.radius_cm * 2
    {
        return Err(Error::Bound);
    }
    if obstacles.len() > 64 {
        return Err(Error::Limit);
    }
    if !matches!(building.layer, Layer::G)
        || door.z_cm != anchor.z_cm
        || door.z_cm < building.z_min_cm
        || door.z_cm >= building.z_max_cm
    {
        return Err(Error::Layer);
    }
    if !door.authorized || !anchor.authorized {
        return Err(Error::Permission);
    }
    if door.mode != anchor.mode || !door.can_exit || !anchor.can_enter {
        return Err(Error::Direction);
    }
    if path.points.first() != Some(&door.point) || path.points.last() != Some(&anchor.point) {
        return Err(Error::Input);
    }
    let f = building.footprint;
    let on_face = match door.facing {
        Facing::East => door.point.x == f.max.x && door.point.y > f.min.y && door.point.y < f.max.y,
        Facing::West => door.point.x == f.min.x && door.point.y > f.min.y && door.point.y < f.max.y,
        Facing::North => {
            door.point.y == f.max.y && door.point.x > f.min.x && door.point.x < f.max.x
        }
        Facing::South => {
            door.point.y == f.min.y && door.point.x > f.min.x && door.point.x < f.max.x
        }
    };
    let available = match door.facing {
        Facing::East | Facing::West => (door.point.y - f.min.y).min(f.max.y - door.point.y),
        Facing::North | Facing::South => (door.point.x - f.min.x).min(f.max.x - door.point.x),
    };
    if available < path.radius_cm {
        return Err(Error::Bound);
    }
    let d = door.facing.vector();
    let next = path.points[1];
    if !on_face || (next.x - door.point.x) * d.x + (next.y - door.point.y) * d.y <= 0 {
        return Err(Error::Direction);
    }
    path.validate_region(region)?;
    // The capsule starts on the door plane; clip only the initial cap into the host wall.
    // All subsequent segments must remain outside the host shell plus clearance.
    let trim = path.radius_cm;
    let shifted = Point {
        x: door.point.x + d.x * (trim + 1),
        y: door.point.y + d.y * (trim + 1),
    };
    if !on(door.point, next, shifted) {
        return Err(Error::NeedsDetailedValidation);
    }
    let mut clipped = path.points.clone();
    clipped[0] = shifted;
    if clipped[0] == clipped[1] {
        return Err(Error::NeedsDetailedValidation);
    }
    let clipped = Corridor::make(clipped, path.radius_cm * 2, 0)?;
    if clipped.intersects_rect(f)? {
        return Err(Error::Collision);
    }
    for obstacle in obstacles {
        obstacle.validate()?;
        // Walking body needs 2m clear vertical interval, independent of the layer label.
        if obstacle.z_min_cm < door.z_cm + 200
            && obstacle.z_max_cm > door.z_cm
            && path.intersects_rect(obstacle.footprint)?
        {
            return Err(Error::Collision);
        }
    }
    Ok(())
}
