//! Prebuilt Engine destination. Failed restore never publishes a partial model.
use super::*;
use crate::restore_into::{self, Restore};
use serde::de::{DeserializeSeed, Visitor};
use std::{
    fmt,
    io::{self, Read},
};
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct SlotIdentity {
    pub world: usize,
    pub world_tables: [usize; 7],
    pub engine_tables: [usize; 3],
    pub timeline_tables: [usize; 2],
    pub background_tables: [usize; 8],
    pub preparation_backing: [usize; 28],
    pub owned_model_bytes: usize,
    pub retained_prepare_bytes: usize,
}
/// A role-owned slot remains allocated through null/reset/failure. Not a pool grant.
pub struct EngineSlot {
    engine: Engine,
    valid: bool,
}
impl EngineSlot {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            engine: Engine::new(World::restore_empty()?, vec![]).map_err(|e| format!("{e:?}"))?,
            valid: false,
        })
    }
    /// Initialize fresh recorded capital into this slot; never call during replay.
    pub fn initialize_into(
        &mut self,
        opening: &[u8],
        resources: &[Resource],
    ) -> Result<(), String> {
        self.reset_preserving_capacity();
        let result = (|| {
            if resources.len() > MAX_RESOURCES {
                return Err("initial_resource_limit".into());
            }
            crate::assembly::assemble_into(opening, &mut self.engine.world)?;
            for r in resources {
                self.engine.resources.push(*r);
            }
            self.engine.resources.sort_by_key(|r| r.id);
            let resources = &self.engine.resources;
            let world = &self.engine.world;
            if world.economy.is_some() && !resources.is_empty()
                || resources
                    .iter()
                    .any(|r| r.id == 0 || r.reserved != 0 || r.occupied != 0)
                || resources.windows(2).any(|w| w[0].id == w[1].id)
                || world.exchange.is_some_and(|e| {
                    e.edges.iter().any(|e| {
                        !resources
                            .iter()
                            .any(|r| r.id == e.resource && r.capacity > 0)
                    })
                })
                || world.boot.is_some_and(|b| {
                    b.resources
                        .iter()
                        .any(|id| !resources.iter().any(|r| r.id == *id && r.capacity > 0))
                })
            {
                return Err("initial_resources".into());
            }
            self.engine.leases.resize(MAX_LEASES, None);
            self.engine.validate_resident()
        })();
        match result {
            Ok(()) => {
                self.valid = true;
                Ok(())
            }
            Err(e) => {
                self.reset_preserving_capacity();
                Err(e)
            }
        }
    }
    /// Move the initialized root to its single writer without allocating another root.
    pub fn into_engine(self) -> Result<Engine, String> {
        if self.valid {
            Ok(self.engine)
        } else {
            Err("unpublished_engine_slot".into())
        }
    }
    pub fn identity(&self) -> Result<SlotIdentity, String> {
        self.engine.backing_identity()
    }
    pub fn reset_preserving_capacity(&mut self) {
        self.valid = false;
        self.engine.world.reset_backing();
        self.engine.resources.clear();
        self.engine.leases.clear();
        self.engine.next_token = 1;
        self.engine.journal.clear();
        self.engine.timeline.deactivate();
        self.engine.background.deactivate();
        if let Some(mut state) = self.engine.spatial_permissions.take() {
            state.clear_preserving_capacity();
            if let Some(workspace) = self.engine.preparation_workspace.as_ref() {
                let mut spare = workspace
                    .spatial_permissions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                *spare = Some(state);
            }
        }
    }
    pub fn engine(&self) -> Result<&Engine, String> {
        if self.valid {
            Ok(&self.engine)
        } else {
            Err("unpublished_engine_slot".into())
        }
    }
    pub fn restore_into(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > ENGINE_BYTE_BUDGET {
            self.reset_preserving_capacity();
            return Err("checkpoint_byte_limit".into());
        }
        self.restore_reader_into(bytes)
    }
    pub fn restore_reader_into(&mut self, reader: impl Read) -> Result<(), String> {
        self.reset_preserving_capacity();
        struct Limited<R> {
            reader: R,
            bytes: usize,
        }
        impl<R: Read> Read for Limited<R> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                if out.is_empty() {
                    return Ok(0);
                }
                if self.bytes > ENGINE_BYTE_BUDGET {
                    return Err(io::Error::other("checkpoint_byte_limit"));
                }
                let count = out.len().min(ENGINE_BYTE_BUDGET + 1 - self.bytes);
                let n = self.reader.read(&mut out[..count])?;
                self.bytes += n;
                if self.bytes > ENGINE_BYTE_BUDGET {
                    return Err(io::Error::other("checkpoint_byte_limit"));
                }
                Ok(n)
            }
        }
        let result = (|| {
            let reader = crate::fixed_restore::JsonReader::with_string_limit(
                Limited { reader, bytes: 0 },
                384,
            );
            let mut d = serde_json::Deserializer::from_reader(reader);
            self.engine.restore(&mut d).map_err(|e| e.to_string())?;
            d.end().map_err(|e| e.to_string())?;
            self.engine.validate_resident()
        })();
        match result {
            Ok(()) => {
                self.valid = true;
                Ok(())
            }
            Err(e) => {
                self.reset_preserving_capacity();
                Err(e)
            }
        }
    }
    /// Checkpoint-state copy: hot journal is intentionally excluded, as in saved Engine.
    pub fn copy_from_into(&mut self, source: &Engine) -> Result<(), String> {
        self.reset_preserving_capacity();
        let result = (|| {
            source.world.copy_into(&mut self.engine.world)?;
            self.engine.resources.copy_active_from(&source.resources);
            self.engine.leases.copy_active_from(&source.leases);
            self.engine.next_token = source.next_token;
            self.engine.timeline.clone_from(&source.timeline);
            self.engine.background.clone_from(&source.background);
            if let Some(source_state) = source.spatial_permissions.as_ref() {
                let workspace = self
                    .engine
                    .preparation_workspace
                    .as_ref()
                    .ok_or("missing_workspace")?;
                let mut state = workspace
                    .spatial_permissions
                    .lock()
                    .map_err(|_| "spatial_permission_slot_poison")?
                    .take()
                    .map(Ok)
                    .unwrap_or_else(|| {
                        crate::spatial_transport_permissions::SpatialPermissionState::empty_backing(
                        )
                        .map_err(|error| format!("spatial_permission_capacity:{error:?}"))
                    })?;
                if let Err(error) = source_state.copy_into(&mut state) {
                    state.clear_preserving_capacity();
                    *workspace
                        .spatial_permissions
                        .lock()
                        .map_err(|_| "spatial_permission_slot_poison")? = Some(state);
                    return Err(format!("spatial_permission_copy:{error:?}"));
                }
                // During copy the source Engine retains its spare. Keep the destination's spare
                // slot empty until publish_swap moves that backing across; allocating it here
                // would make source-active + source-spare + destination-active + destination-spare
                // four full states instead of the admitted three-state peak.
                self.engine.spatial_permissions = Some(state);
            }
            self.engine.validate_resident()
        })();
        match result {
            Ok(()) => {
                self.valid = true;
                Ok(())
            }
            Err(e) => {
                self.reset_preserving_capacity();
                Err(e)
            }
        }
    }
    /// Swap with an existing active root; both roots retain their own backing.
    pub fn publish_swap(&mut self, active: &mut Engine) -> Result<(), String> {
        if !self.valid {
            return Err("unpublished_engine_slot".into());
        }
        std::mem::swap(&mut self.engine, active);
        // The incoming active image was restored directly into this slot's backing. Reuse the
        // former active Engine's spare for it, then reset this slot to retain the former active
        // rows as its own spare. This keeps the combined publication peak at three fixed states.
        if let (Some(source), Some(target)) = (
            self.engine.preparation_workspace.as_ref(),
            active.preparation_workspace.as_ref(),
        ) {
            let mut source_spare = source
                .spatial_permissions
                .lock()
                .map_err(|_| "spatial_permission_slot_poison")?;
            if source_spare.is_some() {
                let mut target_spare = target
                    .spatial_permissions
                    .lock()
                    .map_err(|_| "spatial_permission_slot_poison")?;
                if target_spare.is_none() {
                    *target_spare = source_spare.take();
                }
            }
        }
        // The old active image is no longer a recovery source once the candidate is published.
        // Keep its bounded buffers in this role slot, but release its logical rows so Store
        // retains only the single reusable spare permission backing here.
        self.reset_preserving_capacity();
        Ok(())
    }
}
impl Engine {
    pub fn backing_identity(&self) -> Result<SlotIdentity, String> {
        Ok(SlotIdentity {
            world: self.world.as_ref() as *const World as usize,
            world_tables: self.world.backing_ids(),
            engine_tables: [
                self.resources.allocation(),
                self.leases.allocation(),
                self.journal.allocation(),
            ],
            timeline_tables: [
                self.timeline.backing().claims.allocation(),
                self.timeline.backing().payroll.allocation(),
            ],
            background_tables: self.background.backing().backing_ids(),
            preparation_backing: self
                .preparation_workspace
                .as_ref()
                .ok_or("missing_workspace")?
                .backing_ids()?,
            owned_model_bytes: self.owned_capacity_bytes(),
            retained_prepare_bytes: {
                let (a, b, c) = preparation_workspace_capacity();
                a + b + c
            },
        })
    }
}
impl Restore for Engine {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        struct Read<'a>(&'a mut Engine);
        impl<'de> serde::de::Visitor<'de> for Read<'_> {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("in-place model object")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
                let value = self.0;
                let mut seen = 0u64;
                while let Some(key) = map.next_key::<crate::restore_into::Key>()? {
                    let bit = match key.as_str() {
                        "world" => 1 << 0,
                        "resources" => 1 << 1,
                        "leases" => 1 << 2,
                        "next_token" => 1 << 3,
                        "journal" => 1 << 4,
                        "timeline" => 1 << 5,
                        "background" => 1 << 6,
                        "spatial_permissions" => 1 << 7,
                        _ => return Err(serde::de::Error::custom("restore_unknown_field")),
                    };
                    if seen & bit != 0 {
                        return Err(serde::de::Error::custom("restore_duplicate_field"));
                    }
                    match key.as_str() {
                        "world" => {
                            map.next_value_seed(restore_into::Into(&mut value.world))?;
                        }
                        "resources" => {
                            map.next_value_seed(restore_into::Into(&mut value.resources))?;
                        }
                        "leases" => {
                            map.next_value_seed(restore_into::Into(&mut value.leases))?;
                        }
                        "next_token" => {
                            map.next_value_seed(restore_into::Copy(&mut value.next_token))?;
                        }
                        "journal" => {
                            map.next_value_seed(restore_into::Into(&mut value.journal))?;
                        }
                        "timeline" => {
                            map.next_value_seed(restore_into::Into(&mut value.timeline))?;
                        }
                        "background" => {
                            map.next_value_seed(restore_into::Into(&mut value.background))?;
                        }
                        "spatial_permissions" => {
                            let workspace = value
                                .preparation_workspace
                                .as_ref()
                                .ok_or_else(|| serde::de::Error::custom("missing_workspace"))?;
                            map.next_value_seed(SpatialPermissionField {
                                target: &mut value.spatial_permissions,
                                spare: &workspace.spatial_permissions,
                            })?;
                        }
                        _ => unreachable!(),
                    }
                    seen |= bit;
                }
                if seen & 127 != 127 {
                    return Err(serde::de::Error::custom("restore_missing_field"));
                }
                Ok(())
            }
        }
        d.deserialize_map(Read(self))
    }
}

struct SpatialPermissionField<'a> {
    target: &'a mut Option<crate::spatial_transport_permissions::SpatialPermissionState>,
    spare:
        &'a std::sync::Mutex<Option<crate::spatial_transport_permissions::SpatialPermissionState>>,
}

impl<'de> DeserializeSeed<'de> for SpatialPermissionField<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        struct Optional<'a>(SpatialPermissionField<'a>);
        impl<'de> Visitor<'de> for Optional<'_> {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a spatial permission state")
            }

            fn visit_some<D2: serde::Deserializer<'de>>(
                self,
                deserializer: D2,
            ) -> Result<(), D2::Error> {
                let mut state = if let Some(state) = self.0.target.take() {
                    state
                } else {
                    self.0
                        .spare
                        .lock()
                        .map_err(|_| serde::de::Error::custom("spatial_permission_slot_poison"))?
                        .take()
                        .map(Ok)
                        .unwrap_or_else(|| {
                            crate::spatial_transport_permissions::SpatialPermissionState::empty_backing()
                                .map_err(|error| serde::de::Error::custom(format!("{error:?}")))
                        })?
                };
                let result = state.restore_into_existing(deserializer);
                *self.0.target = Some(state);
                result
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<(), E> {
                Err(E::custom("null_spatial_permission_state"))
            }
        }
        deserializer.deserialize_option(Optional(self))
    }
}
