//! Fixed, immutable genesis dependencies. No filenames or construction grants from input.
use crate::{read_file, Error};
use mycity_core::{sha256, Bootstrap, INPUT_LIMIT};
use mycity_map::{assets::SpatialAssets, terrain::SOURCE_BYTE_LIMIT};
use serde::{Deserialize, Serialize};
use std::path::Path;
pub(crate) const INPUT_FILE: &str = "spatial-input.json";
pub(crate) const PNG_FILE: &str = "spatial-height.png";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialReference {
    schema: String,
    input_bytes: u64,
    input_sha256: String,
    png_bytes: u64,
    png_sha256: String,
    terrain_sha256: String,
    geometry_sha256: String,
    world: u64,
    branch: u64,
    seed: u64,
}
impl SpatialReference {
    pub(crate) fn prepare(
        input: &[u8],
        png: &[u8],
        identity: &Bootstrap,
    ) -> Result<(Self, SpatialAssets), Error> {
        if input.len() as u64 > INPUT_LIMIT || png.len() as u64 > SOURCE_BYTE_LIMIT {
            return Err(Error::Invalid("spatial_input_limit"));
        }
        let assets = SpatialAssets::from_bytes(input, png, identity)
            .map_err(|e| Error::Model(format!("spatial_{e:?}")))?;
        let reference = Self {
            schema: "mycity-m0-spatial-ref-v1".into(),
            input_bytes: input.len() as u64,
            input_sha256: sha256(input),
            png_bytes: png.len() as u64,
            png_sha256: sha256(png),
            terrain_sha256: assets.terrain().state_hash(),
            geometry_sha256: assets.scene().state_hash(),
            world: identity.world,
            branch: identity.branch,
            seed: identity.seed,
        };
        Ok((reference, assets))
    }
    pub(crate) fn load(&self, dir: &Path, identity: &Bootstrap) -> Result<SpatialAssets, Error> {
        if self.schema != "mycity-m0-spatial-ref-v1"
            || self.input_bytes == 0
            || self.input_bytes > INPUT_LIMIT
            || self.png_bytes == 0
            || self.png_bytes > SOURCE_BYTE_LIMIT
            || self.world != identity.world
            || self.branch != identity.branch
            || self.seed != identity.seed
            || [
                &self.input_sha256,
                &self.png_sha256,
                &self.terrain_sha256,
                &self.geometry_sha256,
            ]
            .iter()
            .any(|s| {
                s.len() != 64
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        {
            return Err(Error::Invalid("spatial_reference_invalid"));
        }
        // One bounded read per dependency; these exact buffers are both checked and decoded.
        let input = read_file(&dir.join(INPUT_FILE), self.input_bytes)?;
        if input.len() as u64 != self.input_bytes || sha256(&input) != self.input_sha256 {
            return Err(Error::Invalid("spatial_input_corrupt"));
        }
        let png = read_file(&dir.join(PNG_FILE), self.png_bytes)?;
        if png.len() as u64 != self.png_bytes || sha256(&png) != self.png_sha256 {
            return Err(Error::Invalid("spatial_png_corrupt"));
        }
        let assets = SpatialAssets::from_bytes(&input, &png, identity)
            .map_err(|e| Error::Model(format!("spatial_{e:?}")))?;
        if assets.terrain().state_hash() != self.terrain_sha256
            || assets.scene().state_hash() != self.geometry_sha256
        {
            return Err(Error::Invalid("spatial_derived_state_mismatch"));
        }
        Ok(assets)
    }
    pub fn terrain_sha256(&self) -> &str {
        &self.terrain_sha256
    }
    pub fn geometry_sha256(&self) -> &str {
        &self.geometry_sha256
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.schema.capacity()
            + self.input_sha256.capacity()
            + self.png_sha256.capacity()
            + self.terrain_sha256.capacity()
            + self.geometry_sha256.capacity()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatePoint {
    Validated,
    SpatialInputHalf,
    SpatialInputSynced,
    SpatialPngHalf,
    SpatialPngSynced,
    GenesisSynced,
    WalSynced,
    RootSynced,
    CurrentRenamed,
    CurrentSynced,
    DirectorySynced,
    Ready,
}
pub(crate) fn write_dependency(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    half: CreatePoint,
    synced: CreatePoint,
    observer: &mut impl FnMut(CreatePoint) -> std::io::Result<()>,
) -> Result<(), Error> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(name))?;
    let mid = bytes.len() / 2;
    f.write_all(&bytes[..mid])?;
    observer(half)?;
    f.write_all(&bytes[mid..])?;
    f.sync_all()?;
    drop(f);
    crate::sync_directory(dir)?;
    observer(synced)?;
    Ok(())
}
