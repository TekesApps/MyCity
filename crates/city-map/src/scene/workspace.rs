//! Eagerly initialized geometry arrays; the immutable Scene consumes these exact backings.
use super::*;
use crate::Rect;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SceneWorkspaceIdentity {
    pub backing: [usize; 13],
    pub heap_capacity_bytes: usize,
}
pub struct SceneWorkspace {
    pub(super) road: Vec<Point>,
    pub(super) connector: Vec<Point>,
    holes: Vec<Rect>,
    ports: Vec<Point>,
    obstacles: Vec<Prism>,
    neighbor_obstacles: Vec<Prism>,
    strings: [String; 7],
}
fn initialized<T: Clone>(value: T, count: usize) -> Result<Vec<T>, Error> {
    let mut v = Vec::new();
    v.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    if v.capacity() != count {
        return Err(Error::Limit);
    }
    v.resize(count, value);
    std::hint::black_box(v.as_slice());
    v.clear();
    Ok(v)
}
fn text() -> Result<String, Error> {
    let bytes = initialized(0xa5u8, 64)?;
    // Empty retains the already initialized byte backing; no UTF8 payload is exposed.
    String::from_utf8(bytes).map_err(|_| Error::Input)
}
fn copy_vec<T: Copy>(from: &mut Vec<T>, to: &mut Vec<T>) -> Result<(), Error> {
    if from.len() > to.capacity() {
        return Err(Error::Limit);
    }
    to.clear();
    to.extend_from_slice(from);
    std::mem::swap(from, to);
    Ok(())
}
fn copy_text(from: &mut String, to: &mut String) -> Result<(), Error> {
    if from.len() > to.capacity() {
        return Err(Error::Limit);
    }
    to.clear();
    to.push_str(from);
    std::mem::swap(from, to);
    Ok(())
}
impl SceneWorkspace {
    pub fn new() -> Result<Self, Error> {
        let p = Point { x: 0, y: 0 };
        let r = Rect { min: p, max: p };
        let prism = Prism {
            footprint: r,
            z_min_cm: 0,
            z_max_cm: 0,
            layer: Layer::G,
        };
        Ok(Self {
            road: initialized(p, MAX_VERTICES)?,
            connector: initialized(p, MAX_VERTICES)?,
            holes: initialized(r, MAX_HOLES)?,
            ports: initialized(p, 32)?,
            obstacles: initialized(prism, 64)?,
            neighbor_obstacles: initialized(prism, 64)?,
            strings: [
                text()?,
                text()?,
                text()?,
                text()?,
                text()?,
                text()?,
                text()?,
            ],
        })
    }
    pub fn identity(&self) -> SceneWorkspaceIdentity {
        SceneWorkspaceIdentity {
            backing: [
                self.road.as_ptr() as usize,
                self.connector.as_ptr() as usize,
                self.holes.as_ptr() as usize,
                self.ports.as_ptr() as usize,
                self.obstacles.as_ptr() as usize,
                self.neighbor_obstacles.as_ptr() as usize,
                self.strings[0].as_ptr() as usize,
                self.strings[1].as_ptr() as usize,
                self.strings[2].as_ptr() as usize,
                self.strings[3].as_ptr() as usize,
                self.strings[4].as_ptr() as usize,
                self.strings[5].as_ptr() as usize,
                self.strings[6].as_ptr() as usize,
            ],
            heap_capacity_bytes: (self.road.capacity()
                + self.connector.capacity()
                + self.ports.capacity())
                * size_of::<Point>()
                + self.holes.capacity() * size_of::<Rect>()
                + (self.obstacles.capacity() + self.neighbor_obstacles.capacity())
                    * size_of::<Prism>()
                + self.strings.iter().map(String::capacity).sum::<usize>(),
        }
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.identity().heap_capacity_bytes
    }
    pub(super) fn retain_input(&mut self, mut input: Input) -> Result<Input, Error> {
        copy_vec(&mut input.region.holes, &mut self.holes)?;
        copy_vec(&mut input.connector, &mut self.ports)?;
        copy_vec(&mut input.obstacles, &mut self.obstacles)?;
        copy_vec(&mut input.neighbor_obstacles, &mut self.neighbor_obstacles)?;
        for (src, dst) in [
            &mut input.schema,
            &mut input.map.metric_frame,
            &mut input.map.vertical_datum,
            &mut input.neighbor.metric_frame,
            &mut input.neighbor.vertical_datum,
        ]
        .into_iter()
        .zip(self.strings.iter_mut())
        {
            copy_text(src, dst)?;
        }
        Ok(input)
    }
    pub(super) fn retain_hashes(
        &mut self,
        first: &mut String,
        second: &mut String,
    ) -> Result<(), Error> {
        copy_text(first, &mut self.strings[5])?;
        copy_text(second, &mut self.strings[6])
    }
}
impl Scene {
    pub fn workspace_identity(&self) -> SceneWorkspaceIdentity {
        let i = &self.input;
        SceneWorkspaceIdentity {
            backing: [
                self.road.backing_identity().0,
                self.connector.backing_identity().0,
                i.region.holes.as_ptr() as usize,
                i.connector.as_ptr() as usize,
                i.obstacles.as_ptr() as usize,
                i.neighbor_obstacles.as_ptr() as usize,
                i.schema.as_ptr() as usize,
                i.map.metric_frame.as_ptr() as usize,
                i.map.vertical_datum.as_ptr() as usize,
                i.neighbor.metric_frame.as_ptr() as usize,
                i.neighbor.vertical_datum.as_ptr() as usize,
                self.terrain_sha256.as_ptr() as usize,
                self.neighbor_terrain_sha256.as_ptr() as usize,
            ],
            heap_capacity_bytes: self.owned_capacity_bytes() - size_of::<Self>(),
        }
    }
}
