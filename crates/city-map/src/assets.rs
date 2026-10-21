//! Immutable CPU assets bound to one laboratory world's identity.
mod workspace;
use crate::{
    probe::Input,
    scene::Scene,
    terrain::{self, Terrain},
    Error, SCHEMA,
};
use mycity_core::{Bootstrap, INPUT_LIMIT};
pub use workspace::{
    SpatialWorkspace, SpatialWorkspaceIdentity, NORMAL_INPUT_BYTES, NORMAL_PNG_BYTES,
};

pub struct SpatialAssets {
    identity: Bootstrap,
    terrain: Terrain,
    scene: Scene,
    decoder: Option<terrain::DecoderReserve>,
}
fn validate_identity(input: &Input, expected: &Bootstrap) -> Result<(), Error> {
    if input.schema != SCHEMA
        || input.bootstrap.validate().is_err()
        || &input.bootstrap != expected
        || input.scene.map.world != expected.world
        || input.scene.map.branch != expected.branch
    {
        return Err(Error::Input);
    }
    Ok(())
}
impl SpatialAssets {
    pub fn from_bytes(input: &[u8], png: &[u8], expected: &Bootstrap) -> Result<Self, Error> {
        if input.len() as u64 > INPUT_LIMIT {
            return Err(Error::Limit);
        }
        let input: Input = serde_json::from_slice(input).map_err(|_| Error::Input)?;
        Self::assemble(input, png, expected)
    }
    pub fn assemble(input: Input, png: &[u8], expected: &Bootstrap) -> Result<Self, Error> {
        validate_identity(&input, expected)?;
        let terrain = terrain::import(input.terrain, png, input.scene.terrain_revision, 0)?;
        // Native v1 bundle explicitly shares a single immutable terrain asset between the
        // two map frames. Arbitrary independent neighbor DEMs require a future schema.
        let scene = Scene::assemble(input.scene, &terrain, &terrain)?;
        Ok(Self {
            identity: input.bootstrap,
            terrain,
            scene,
            decoder: None,
        })
    }
    pub fn identity(&self) -> &Bootstrap {
        &self.identity
    }
    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        std::mem::size_of::<Self>() - std::mem::size_of::<Terrain>() - std::mem::size_of::<Scene>()
            + self.identity.schema.capacity()
            + self.terrain.owned_capacity_bytes()
            + self.scene.owned_capacity_bytes()
            + self.decoder.as_ref().map_or(0, |d| {
                d.owned_capacity_bytes() - std::mem::size_of::<terrain::DecoderReserve>()
            })
    }
}
