//! Bounded original opening bytes for the caller's existing unpublished EngineSlot.
//!
//! Loading is transport only, not semantic acceptance. Initialization uses the existing
//! retained-slot initializer and its sole assembly/resource validators. No World, slot,
//! resources, filesystem permission or publication is created by this adapter.
use crate::transaction::{Engine, EngineSlot, Resource};
use crate::{read_bounded, sha256, INPUT_LIMIT};

pub const MAX_SOURCE_LABEL_BYTES: usize = 1024;

/// Descriptive provenance only; not an access permission or real-world-data assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputOrigin {
    SuppliedBytes,
    Reader,
}

#[derive(Debug)]
pub struct OpeningSource {
    bytes: Vec<u8>,
    sha256: String,
    origin: InputOrigin,
    label: Option<String>,
}

impl OpeningSource {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn origin(&self) -> InputOrigin {
        self.origin
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// Bounded, immutable input transport. No schema or entitlement is accepted by loading.
/// Only initialize_into can yield an InitializedOpening view after canonical validation.
#[derive(Debug)]
pub struct OpeningInput {
    source: OpeningSource,
}

/// Read-only success view, tied to both original input and the caller's initialized slot.
/// Its slot borrow prevents reset/reinitialization/consumption while this view is live.
/// Dropping it neither publishes nor reinitializes anything. The caller retains both owners;
/// after the view expires, the existing slot.into_engine() boundary remains caller-owned.
pub struct InitializedOpening<'input, 'slot> {
    source: &'input OpeningSource,
    engine: &'slot Engine,
}

impl InitializedOpening<'_, '_> {
    pub fn source(&self) -> &OpeningSource {
        self.source
    }
    pub fn engine(&self) -> &Engine {
        self.engine
    }
}

impl OpeningInput {
    /// Preserve exact JSON bytes; semantic/JSON errors are deferred to initialize_into.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > INPUT_LIMIT {
            return Err("input_too_large".into());
        }
        Ok(Self::from_owned_bytes(
            bytes.to_vec(),
            InputOrigin::SuppliedBytes,
            None,
        ))
    }

    pub fn read_json(reader: impl std::io::Read) -> Result<Self, String> {
        let bytes = read_bounded(reader, INPUT_LIMIT).map_err(|e| e.to_string())?;
        Ok(Self::from_owned_bytes(bytes, InputOrigin::Reader, None))
    }

    /// The caller must acquire this reader through its real permission provider.
    pub fn read_named_json(reader: impl std::io::Read, label: &str) -> Result<Self, String> {
        if label.len() > MAX_SOURCE_LABEL_BYTES {
            return Err("opening_loader_source_label_too_large".into());
        }
        let bytes = read_bounded(reader, INPUT_LIMIT).map_err(|e| e.to_string())?;
        Ok(Self::from_owned_bytes(
            bytes,
            InputOrigin::Reader,
            Some(label.to_owned()),
        ))
    }

    fn from_owned_bytes(bytes: Vec<u8>, origin: InputOrigin, label: Option<String>) -> Self {
        Self {
            source: OpeningSource {
                sha256: sha256(&bytes),
                bytes,
                origin,
                label,
            },
        }
    }

    /// Source access never implies successful initialization or authority to publish.
    pub fn source(&self) -> &OpeningSource {
        &self.source
    }
    pub fn into_source(self) -> OpeningSource {
        self.source
    }

    /// Initialize the caller's existing unpublished slot exactly once on this path.
    /// Resources must be supplied by the caller; there are no synthesized defaults.
    /// A successful slot is rejected before any mutation, so retries cannot reissue
    /// capital. Failed canonical initialization invalidates/resets the slot through
    /// its existing contract; the immutable input remains available for inspection/retry.
    pub fn initialize_into<'input, 'slot>(
        &'input self,
        slot: &'slot mut EngineSlot,
        resources: &[Resource],
    ) -> Result<InitializedOpening<'input, 'slot>, String> {
        if slot.engine().is_ok() {
            return Err("opening_loader_slot_already_initialized".into());
        }
        slot.initialize_into(self.source.bytes(), resources)?;
        Ok(InitializedOpening {
            source: &self.source,
            engine: slot.engine()?,
        })
    }
}
