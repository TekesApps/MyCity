//! Explicit bounded reads from local regular files into the existing immutable opening input.
//!
//! An OS-readable file is only an input source. It does not prove ownership, permission to use
//! referenced assets, legal rights, or acceptance by the opening assembler or Store.
use mycity_core::{
    opening_loader::{OpeningInput, MAX_SOURCE_LABEL_BYTES},
    transaction::{Resource, MAX_RESOURCES},
    sha256, INPUT_LIMIT,
};
use serde::Deserialize;
use std::{
    fs::{self, File, Metadata},
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const LOCAL_FILE_SOURCE_LABEL: &str = "local_file_os_read";
pub const MAX_LOCAL_PATH_BYTES: usize = 4096;
pub const MAX_LOCAL_BYTES_LIMIT: usize = 16 * 1024 * 1024;
pub const MAX_RESOURCE_MANIFEST_BYTES: usize = 4096;
pub const RESOURCE_MANIFEST_SCHEMA: &str = "mycity-resources-v1";

/// Provenance for the exact opened handle and the bytes consumed through that handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedFileProvenance {
    pub source_label: &'static str,
    pub canonical_path: PathBuf,
    pub file_identity: FileIdentity,
    pub bytes_read: usize,
    pub sha256: String,
}

/// Platform file identity. Unix records the opened inode; other platforms retain the canonical
/// path and metadata stamp, which is weaker and is not represented as an inode guarantee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileIdentity {
    pub canonical_path: PathBuf,
    pub length: u64,
    pub modified: SystemTime,
    #[cfg(unix)]
    pub device: u64,
    #[cfg(unix)]
    pub inode: u64,
}

#[derive(Debug)]
pub struct OpenedOpeningInput {
    input: OpeningInput,
    provenance: OpenedFileProvenance,
}

#[derive(Debug)]
pub struct LoadedResourceManifest {
    pub resources: Vec<Resource>,
    pub provenance: OpenedFileProvenance,
}

#[derive(Debug)]
pub struct OpenedLocalBytes {
    pub bytes: Vec<u8>,
    pub provenance: OpenedFileProvenance,
}

struct CheckedLocalFile {
    file: File,
    canonical_path: PathBuf,
    before: FileIdentity,
    limit: usize,
}

impl OpenedOpeningInput {
    pub fn input(&self) -> &OpeningInput {
        &self.input
    }

    pub fn provenance(&self) -> &OpenedFileProvenance {
        &self.provenance
    }
}

impl OpenedFileProvenance {
    /// Stable JSON-shaped diagnostic data for CLI reports; this is descriptive provenance only.
    pub fn to_json(&self) -> serde_json::Value {
        let modified = self
            .file_identity
            .modified
            .duration_since(UNIX_EPOCH)
            .map(|duration| {
                serde_json::json!({
                    "unix_seconds": duration.as_secs(),
                    "subsec_nanos": duration.subsec_nanos(),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        #[cfg(unix)]
        let native_identity = serde_json::json!({
            "device": self.file_identity.device,
            "inode": self.file_identity.inode,
        });
        #[cfg(not(unix))]
        let native_identity = serde_json::json!({"kind": "path_and_metadata_only"});
        serde_json::json!({
            "source_label": self.source_label,
            "canonical_path": self.canonical_path.to_string_lossy(),
            "file_identity": {
                "length": self.file_identity.length,
                "modified": modified,
                "native": native_identity,
            },
            "bytes_read": self.bytes_read,
            "sha256": self.sha256,
        })
    }
}

impl OpenedLocalBytes {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn provenance(&self) -> &OpenedFileProvenance {
        &self.provenance
    }
}

/// Open and read one explicitly selected local regular file using OS read permissions.
///
/// Bytes are collected once by `OpeningInput::read_named_json`, under the core input bound. The
/// returned provenance hashes those actual bytes. Metadata for the handle and canonical path is
/// compared before/after; this detects observed replacement or ordinary mutation, but cannot
/// guarantee detection of a concurrent rewrite that restores all compared metadata.
pub fn read_local_file(path: impl AsRef<Path>) -> Result<OpenedOpeningInput, String> {
    let (input, provenance) = read_file_with_limit(path.as_ref(), INPUT_LIMIT as usize)?;
    Ok(OpenedOpeningInput { input, provenance })
}

/// Read one local byte asset with a caller-selected explicit bound up to 16 MiB.
/// The single returned Vec is the only input buffer; callers may use 64 KiB for JSON or the
/// existing 16 MiB terrain-source limit for PNG, without changing either downstream contract.
pub fn read_local_bytes(
    path: impl AsRef<Path>,
    limit: usize,
) -> Result<OpenedLocalBytes, String> {
    if limit == 0 || limit > MAX_LOCAL_BYTES_LIMIT {
        return Err("opening_source_invalid_byte_limit".into());
    }
    let mut checked = open_local_file(path.as_ref(), limit)?;
    let mut bytes = Vec::new();
    (&mut checked.file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "opening_source_read_failed".to_owned())?;
    if bytes.len() > limit {
        return Err("input_too_large".into());
    }
    if bytes.is_empty() {
        return Err("opening_source_empty_file".into());
    }
    let identity = finish_local_file(&checked)?;
    let provenance = OpenedFileProvenance {
        source_label: LOCAL_FILE_SOURCE_LABEL,
        canonical_path: checked.canonical_path,
        file_identity: identity,
        bytes_read: bytes.len(),
        sha256: sha256(&bytes),
    };
    Ok(OpenedLocalBytes { bytes, provenance })
}

/// Read an explicit bounded resources manifest. It can only declare id/capacity; occupancy and
/// reservation are initialized by the existing Resource constructor and remain Store-owned.
pub fn load_resources_file(path: impl AsRef<Path>) -> Result<LoadedResourceManifest, String> {
    let (input, provenance) = read_file_with_limit(path.as_ref(), MAX_RESOURCE_MANIFEST_BYTES)
        .map_err(|error| {
            if error == "input_too_large" {
                "resource_manifest_too_large".to_owned()
            } else {
                error
            }
        })?;
    let parsed: ResourceManifestInput = serde_json::from_slice(input.source().bytes())
        .map_err(|_| "resource_manifest_invalid_json".to_owned())?;
    if parsed.schema != RESOURCE_MANIFEST_SCHEMA {
        return Err("resource_manifest_unsupported_schema".into());
    }
    if parsed.resources.len() > MAX_RESOURCES {
        return Err("resource_manifest_count_limit".into());
    }
    let mut resources = Vec::with_capacity(parsed.resources.len());
    for entry in parsed.resources {
        if entry.id == 0 || resources.iter().any(|resource: &Resource| resource.id == entry.id) {
            return Err("resource_manifest_duplicate_or_zero_id".into());
        }
        resources.push(Resource::new(entry.id, entry.capacity).map_err(|_| "resource_manifest_invalid_resource")?);
    }
    Ok(LoadedResourceManifest { resources, provenance })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceManifestInput {
    schema: String,
    resources: Vec<ResourceManifestEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceManifestEntry {
    id: u32,
    capacity: u32,
}

fn read_file_with_limit(
    path: &Path,
    limit: usize,
) -> Result<(OpeningInput, OpenedFileProvenance), String> {
    let mut checked = open_local_file(path, limit)?;
    let bounded_reader = (&mut checked.file).take(limit as u64 + 1);
    let input = OpeningInput::read_named_json(bounded_reader, LOCAL_FILE_SOURCE_LABEL)?;
    if input.source().bytes().len() > limit {
        return Err("input_too_large".into());
    }
    if input.source().bytes().is_empty() {
        return Err("opening_source_empty_file".into());
    }
    let identity = finish_local_file(&checked)?;
    let source = input.source();
    let bytes_read = source.bytes().len();
    let sha256 = source.sha256().to_owned();
    Ok((
        input,
        OpenedFileProvenance {
            source_label: LOCAL_FILE_SOURCE_LABEL,
            canonical_path: checked.canonical_path,
            file_identity: identity,
            bytes_read,
            sha256,
        },
    ))
}

fn open_local_file(path: &Path, limit: usize) -> Result<CheckedLocalFile, String> {
    if path.as_os_str().is_empty() {
        return Err("opening_source_empty_path".into());
    }
    if path.as_os_str().len() > MAX_LOCAL_PATH_BYTES {
        return Err("opening_source_path_too_large".into());
    }
    if LOCAL_FILE_SOURCE_LABEL.len() > MAX_SOURCE_LABEL_BYTES {
        return Err("opening_source_label_too_large".into());
    }

    let file = open_nonblocking(path)?;
    let before_metadata = file
        .metadata()
        .map_err(|_| "opening_source_metadata_failed".to_owned())?;
    if !before_metadata.is_file() {
        return Err("opening_source_not_regular_file".into());
    }
    if before_metadata.len() > limit as u64 {
        return Err("input_too_large".into());
    }
    let canonical_path = fs::canonicalize(path)
        .map_err(|_| "opening_source_canonicalize_failed".to_owned())?;
    if canonical_path.as_os_str().len() > MAX_LOCAL_PATH_BYTES {
        return Err("opening_source_path_too_large".into());
    }
    let before = file_identity(&before_metadata, canonical_path.clone())?;
    let named_before = fs::metadata(&canonical_path)
        .map_err(|_| "opening_source_metadata_failed".to_owned())?;
    ensure_same_file(&before, &file_identity(&named_before, canonical_path.clone())?)?;

    Ok(CheckedLocalFile {
        file,
        canonical_path,
        before,
        limit,
    })
}

// Open first with NONBLOCK so a FIFO cannot hang before the descriptor metadata check. On
// platforms without this safe rustix path, fail closed instead of claiming equivalent behavior.
#[cfg(target_os = "macos")]
fn open_nonblocking(path: &Path) -> Result<File, String> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| "opening_source_open_failed".to_owned())?;
    Ok(File::from(fd))
}

#[cfg(not(target_os = "macos"))]
fn open_nonblocking(_path: &Path) -> Result<File, String> {
    Err("opening_source_platform_unsupported".into())
}

fn finish_local_file(checked: &CheckedLocalFile) -> Result<FileIdentity, String> {
    let after_metadata = checked.file
        .metadata()
        .map_err(|_| "opening_source_metadata_failed".to_owned())?;
    let after = file_identity(&after_metadata, checked.canonical_path.clone())?;
    ensure_same_file(&checked.before, &after)?;
    let named_after = fs::metadata(&checked.canonical_path)
        .map_err(|_| "opening_source_metadata_failed".to_owned())?;
    ensure_same_file(
        &checked.before,
        &file_identity(&named_after, checked.canonical_path.clone())?,
    )?;
    if after.length > checked.limit as u64 {
        return Err("input_too_large".into());
    }
    Ok(after)
}

fn file_identity(metadata: &Metadata, canonical_path: PathBuf) -> Result<FileIdentity, String> {
    if !metadata.is_file() {
        return Err("opening_source_not_regular_file".into());
    }
    let modified = metadata
        .modified()
        .map_err(|_| "opening_source_metadata_failed".to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            canonical_path,
            length: metadata.len(),
            modified,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileIdentity {
            canonical_path,
            length: metadata.len(),
            modified,
        })
    }
}

fn ensure_same_file(before: &FileIdentity, after: &FileIdentity) -> Result<(), String> {
    if before != after {
        return Err("opening_source_changed_during_read".into());
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "opening_source_provider_tests.rs"]
mod source_provider_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_change_is_rejected() {
        let first = FileIdentity {
            canonical_path: PathBuf::from("/candidate/opening.json"),
            length: 10,
            modified: UNIX_EPOCH,
            #[cfg(unix)]
            device: 1,
            #[cfg(unix)]
            inode: 2,
        };
        let mut changed = first.clone();
        changed.length += 1;
        assert_eq!(
            ensure_same_file(&first, &changed),
            Err("opening_source_changed_during_read".into())
        );
    }
}
