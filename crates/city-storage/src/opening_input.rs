//! Two closed source adapters into the original Store-owned initial slot.
use super::*;
use mycity_core::{
    opening_loader::OpeningInput,
    transaction::{EngineSlot, EngineWorkspace},
};

// This module is private: no external source implementation can pair unrelated bytes
// and initialization. Both supported sources preserve original raw genesis bytes.
pub(crate) trait GenesisOpening {
    fn original_bytes(&self) -> &[u8];
    fn initialize_slot(&self, slot: &mut EngineSlot, resources: &[Resource]) -> Result<(), String>;
}
impl GenesisOpening for [u8] {
    fn original_bytes(&self) -> &[u8] {
        self
    }
    fn initialize_slot(&self, slot: &mut EngineSlot, resources: &[Resource]) -> Result<(), String> {
        slot.initialize_into(self, resources)
    }
}
impl GenesisOpening for OpeningInput {
    fn original_bytes(&self) -> &[u8] {
        self.source().bytes()
    }
    fn initialize_slot(&self, slot: &mut EngineSlot, resources: &[Resource]) -> Result<(), String> {
        // Drop only the borrowed success view. The same slot is consumed by the
        // original Store factory; no second root, reparse or publication is added.
        self.initialize_into(slot, resources).map(|_view| ())
    }
}
impl Store {
    /// Initialize and publish one ordinary Store using original opening bytes.
    /// Loading the input grants no filesystem permission or non-laboratory rights.
    pub fn create_opening_input(
        dir: &Path,
        input: &OpeningInput,
        resources: Vec<Resource>,
        rules_identity: &str,
    ) -> Result<Self, Error> {
        Self::create_opening_input_in_workspace_with_observer(
            dir,
            input,
            resources,
            rules_identity,
            EngineWorkspace::new().map_err(Error::Model)?,
            |_| Ok(()),
        )
    }
    /// Ordinary spatial genesis from the same immutable opening transport.
    /// The existing Scene/exchange binding validation and publication order are retained.
    pub fn create_opening_input_spatial(
        dir: &Path,
        input: &OpeningInput,
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
    ) -> Result<Self, Error> {
        Self::create_in_workspace_with_transport(
            dir,
            input,
            resources,
            rules_identity,
            Some(bundle),
            None,
            (
                0,
                EngineWorkspace::new().map_err(Error::Model)?,
                None,
                MetadataWorkspace::new()?,
            ),
            None,
            None,
            |_| Ok(()),
        )
    }
    /// Publish the initial backing of the caller's original workspace exactly once.
    /// The original create observer contract is retained: an error after CURRENT
    /// publication is uncertain, so preserve the directory and reopen it; do not
    /// initialize over it or reissue original capital in response to a lost reply.
    pub fn create_opening_input_in_workspace_with_observer(
        dir: &Path,
        input: &OpeningInput,
        resources: Vec<Resource>,
        rules_identity: &str,
        workspace: EngineWorkspace,
        observer: impl FnMut(CreatePoint) -> io::Result<()>,
    ) -> Result<Self, Error> {
        Self::create_in_workspace_with_transport(
            dir,
            input,
            resources,
            rules_identity,
            None,
            None,
            (0, workspace, None, MetadataWorkspace::new()?),
            None,
            None,
            observer,
        )
    }
}
