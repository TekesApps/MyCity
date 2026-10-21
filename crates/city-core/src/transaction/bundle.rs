//! Common, eagerly allocated role backing. Construction is not a broker grant.
use super::{Engine, EngineSlot, SlotIdentity};
use crate::{
    air::{BackgroundStateSlot, StateSlotIdentity},
    canonical::{Codec, CodecIdentity},
};
use serde::Serialize;
use std::mem::size_of;

pub struct EngineWorkspace {
    initial: EngineSlot,
    restore: EngineSlot,
    candidate: BackgroundStateSlot,
    codec: Codec,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct EngineWorkspaceIdentity {
    pub initial: SlotIdentity,
    pub restore: SlotIdentity,
    pub candidate: StateSlotIdentity,
    pub codec: CodecIdentity,
    pub owned_bytes: usize,
}
impl EngineWorkspace {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            initial: EngineSlot::new()?,
            restore: EngineSlot::new()?,
            candidate: BackgroundStateSlot::new(),
            codec: Codec::new()?,
        })
    }
    pub fn identity(&self) -> Result<EngineWorkspaceIdentity, String> {
        let initial = self.initial.identity()?;
        let restore = self.restore.identity()?;
        let candidate = self.candidate.identity();
        let codec = self.codec.identity();
        let owned_bytes = Self::capacity_from_parts(&initial, &restore, &candidate, &codec);
        Ok(EngineWorkspaceIdentity {
            initial,
            restore,
            candidate,
            codec,
            owned_bytes,
        })
    }
    pub fn capacity_from_parts(
        initial: &SlotIdentity,
        restore: &SlotIdentity,
        candidate: &StateSlotIdentity,
        codec: &CodecIdentity,
    ) -> usize {
        size_of::<Self>() - 2 * size_of::<Engine>() - size_of::<BackgroundStateSlot>()
            + initial.owned_model_bytes
            + initial.retained_prepare_bytes
            + restore.owned_model_bytes
            + restore.retained_prepare_bytes
            + candidate.owned_bytes
            + codec.backing_bytes
    }
    /// Move each existing backing to its role; no new root or codec is constructed.
    pub fn into_parts(self) -> (EngineSlot, EngineSlot, BackgroundStateSlot, Codec) {
        (self.initial, self.restore, self.candidate, self.codec)
    }
}
