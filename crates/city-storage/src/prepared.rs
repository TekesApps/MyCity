//! Actual caller-owned Store construction backing; not a broker grant.
use super::*;
pub struct StoreWorkspace {
    pub(crate) model: mycity_core::transaction::EngineWorkspace,
    pub(crate) spatial: mycity_map::assets::SpatialWorkspace,
    pub(crate) metadata: MetadataWorkspace,
}
#[derive(Debug, Serialize)]
pub struct StoreWorkspaceIdentity {
    pub model: mycity_core::transaction::EngineWorkspaceIdentity,
    pub spatial: mycity_map::assets::SpatialWorkspaceIdentity,
    pub metadata: MetadataIdentity,
}
impl StoreWorkspace {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            model: mycity_core::transaction::EngineWorkspace::new().map_err(Error::Model)?,
            spatial: mycity_map::assets::SpatialWorkspace::new()
                .map_err(|e| Error::Model(format!("spatial_{e:?}")))?,
            metadata: MetadataWorkspace::new()?,
        })
    }
    pub fn identity(&self) -> Result<StoreWorkspaceIdentity, Error> {
        Ok(StoreWorkspaceIdentity {
            model: self.model.identity().map_err(Error::Model)?,
            spatial: self.spatial.identity(),
            metadata: self.metadata.identity(),
        })
    }
}
