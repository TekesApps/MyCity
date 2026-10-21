//! Prebuilt Engine destination. Failed restore never publishes a partial model.
use super::*;
use crate::restore_into::Restore;
use std::io::{self, Read};
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct SlotIdentity {
    pub world: usize,
    pub world_tables: [usize; 7],
    pub engine_tables: [usize; 3],
    pub timeline_tables: [usize; 2],
    pub background_tables: [usize; 8],
    pub preparation_backing: [usize; 27],
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
        self.valid = false;
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
        crate::restore_into::object!(d,self,Engine,127u64,{
            world,0=>Into,resources,1=>Into,leases,2=>Into,next_token,3=>Copy,journal,4=>Into,timeline,5=>Into,background,6=>Into,
        })
    }
}
