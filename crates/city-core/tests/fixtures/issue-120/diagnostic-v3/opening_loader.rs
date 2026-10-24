//! Bounded transport for the existing opening JSON and its exact source bytes.
//!
//! Assembly, including nested profiles and entitlements, has one authority:
//! `assembly::assemble`. This adapter never parses then serializes an Opening, does not
//! invent a second assembler, and does not authorize file access or publish genesis.
use crate::{assembly, read_bounded, sha256, INPUT_LIMIT};

pub const MAX_SOURCE_LABEL_BYTES: usize = 1024;

/// Descriptive provenance, never a permission or a claim that input is real-world data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputOrigin {
    SuppliedBytes,
    Reader,
}

/// Immutable original input, suitable for handing to the existing genesis writer.
/// Its digest identifies these bytes, not a normalized JSON object or a Store envelope.
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

    /// Optional caller-supplied description; it is untrusted and grants no access.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// An unpublished assembled root paired with the bytes used to assemble it.
/// The root is boxed so transporting this wrapper does not repeatedly move an inline World.
/// Box storage remains a real allocation; assembly return temporaries are not eliminated.
/// No mutable accessor is exposed: callers consume the pair at their existing writer boundary.
/// This is an initialization result, never a save/restore entry point.
pub struct LoadedOpening {
    world: Box<assembly::World>,
    source: OpeningSource,
}

impl LoadedOpening {
    /// Accept the current canonical schema only, with the assembler's exact errors.
    /// JSON is decoded once by the existing assembler; the original bytes are retained.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > INPUT_LIMIT {
            return Err("input_too_large".into());
        }
        Self::from_owned_bytes(bytes.to_vec(), InputOrigin::SuppliedBytes, None)
    }

    /// Read at most INPUT_LIMIT + 1 bytes, independent of metadata or declared length.
    pub fn read_json(reader: impl std::io::Read) -> Result<Self, String> {
        let bytes = read_bounded(reader, INPUT_LIMIT).map_err(|e| e.to_string())?;
        Self::from_owned_bytes(bytes, InputOrigin::Reader, None)
    }

    /// Keep a bounded source description alongside the exact bytes. The caller must
    /// obtain the reader from its real permission provider before invoking this method.
    pub fn read_named_json(reader: impl std::io::Read, label: &str) -> Result<Self, String> {
        if label.len() > MAX_SOURCE_LABEL_BYTES {
            return Err("opening_loader_source_label_too_large".into());
        }
        let bytes = read_bounded(reader, INPUT_LIMIT).map_err(|e| e.to_string())?;
        Self::from_owned_bytes(bytes, InputOrigin::Reader, Some(label.to_owned()))
    }

    fn from_owned_bytes(
        bytes: Vec<u8>,
        origin: InputOrigin,
        label: Option<String>,
    ) -> Result<Self, String> {
        // Test-only stage markers: diagnostics use --nocapture in a fresh process.
        // They are excluded from non-test production builds and do not adjust stack size.
        #[cfg(test)]
        eprintln!("issue120:owned:entered-before-assemble bytes={}", bytes.len());
        let world = assembly::assemble(&bytes)?;
        #[cfg(test)]
        eprintln!("issue120:owned:assemble-returned");
        let source = OpeningSource {
            sha256: sha256(&bytes),
            bytes,
            origin,
            label,
        };
        #[cfg(test)]
        eprintln!("issue120:owned:before-box");
        let world = Box::new(world);
        #[cfg(test)]
        eprintln!("issue120:owned:box-returned");
        Ok(Self { world, source })
    }

    pub fn world(&self) -> &assembly::World {
        &self.world
    }

    pub fn source(&self) -> &OpeningSource {
        &self.source
    }

    /// Preserve the pair for the existing publication boundary. Store integration must
    /// keep source.bytes() verbatim and use its existing genesis/hash protocol.
    pub fn into_parts(self) -> (Box<assembly::World>, OpeningSource) {
        (self.world, self.source)
    }
}
