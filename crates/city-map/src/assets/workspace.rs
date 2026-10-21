//! The normal role's source profile; it preserves every frozen output grid size.
use super::*;
use crate::{
    scene::{SceneWorkspace, SceneWorkspaceIdentity},
    terrain::{TerrainWorkspace, TerrainWorkspaceIdentity},
};
use serde::Serialize;
pub const NORMAL_INPUT_BYTES: usize = 64 * 1024;
pub const NORMAL_PNG_BYTES: usize = 64 * 1024;
pub struct SpatialWorkspace {
    terrain: TerrainWorkspace,
    scene: SceneWorkspace,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SpatialWorkspaceIdentity {
    pub terrain: TerrainWorkspaceIdentity,
    pub scene: SceneWorkspaceIdentity,
    /// Headers plus physical grid, decoder, and scene backing. Parsed metadata is separate.
    pub prepared_owned_bytes: usize,
}
impl SpatialWorkspace {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            terrain: TerrainWorkspace::new()?,
            scene: SceneWorkspace::new()?,
        })
    }
    pub fn identity(&self) -> SpatialWorkspaceIdentity {
        let terrain = self.terrain.identity();
        let scene = self.scene.identity();
        let prepared_owned_bytes = std::mem::size_of::<Self>()
            + terrain.grid_capacity_bytes
            + terrain.decoder_capacity_bytes
            + scene.heap_capacity_bytes;
        SpatialWorkspaceIdentity {
            terrain,
            scene,
            prepared_owned_bytes,
        }
    }
    pub fn assemble(
        mut self,
        input: Input,
        png: &[u8],
        expected: &Bootstrap,
    ) -> Result<SpatialAssets, Error> {
        if png.len() > NORMAL_PNG_BYTES {
            return Err(Error::Limit);
        }
        validate_identity(&input, expected)?;
        self.terrain
            .load(input.terrain, png, input.scene.terrain_revision)?;
        let (terrain, decoder) = self.terrain.into_terrain()?;
        let scene = Scene::assemble_in_workspace(input.scene, &terrain, &terrain, self.scene)?;
        Ok(SpatialAssets {
            identity: input.bootstrap,
            terrain,
            scene,
            decoder: Some(decoder),
        })
    }
    pub fn from_bytes(
        self,
        input: &[u8],
        png: &[u8],
        expected: &Bootstrap,
    ) -> Result<SpatialAssets, Error> {
        if input.len() > NORMAL_INPUT_BYTES || png.len() > NORMAL_PNG_BYTES {
            return Err(Error::Limit);
        }
        crate::bounded::guard(input)?;
        let parsed = serde_json::from_slice(input).map_err(|_| Error::Input)?;
        self.assemble(parsed, png, expected)
    }
}
impl SpatialAssets {
    pub fn normal_backing_identity(
        &self,
    ) -> Result<(SceneWorkspaceIdentity, ([usize; 4], usize)), Error> {
        let decoder = self.decoder.as_ref().ok_or(Error::Input)?;
        Ok((self.scene.workspace_identity(), decoder.identity()))
    }
}
