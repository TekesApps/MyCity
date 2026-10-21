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
    #[serde(deserialize_with = "crate::metadata::text::<_,64>")]
    schema: String,
    input_bytes: u64,
    #[serde(deserialize_with = "crate::metadata::text::<_,64>")]
    input_sha256: String,
    png_bytes: u64,
    #[serde(deserialize_with = "crate::metadata::text::<_,64>")]
    png_sha256: String,
    #[serde(deserialize_with = "crate::metadata::text::<_,64>")]
    terrain_sha256: String,
    #[serde(deserialize_with = "crate::metadata::text::<_,64>")]
    geometry_sha256: String,
    world: u64,
    branch: u64,
    seed: u64,
}
impl SpatialReference {
    pub(crate) fn prepare_with_workspace(
        input: &[u8],
        png: &[u8],
        identity: &Bootstrap,
        workspace: Option<mycity_map::assets::SpatialWorkspace>,
    ) -> Result<(Self, SpatialAssets), Error> {
        if input.len() as u64 > INPUT_LIMIT || png.len() as u64 > SOURCE_BYTE_LIMIT {
            return Err(Error::Invalid("spatial_input_limit"));
        }
        let assets = if let Some(w) = workspace {
            w.from_bytes(input, png, identity)
        } else {
            SpatialAssets::from_bytes(input, png, identity)
        }
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
    pub(crate) fn load_with_workspace(
        &self,
        dir: &Path,
        identity: &Bootstrap,
        workspace: Option<mycity_map::assets::SpatialWorkspace>,
        metadata: Option<&mut crate::MetadataWorkspace>,
    ) -> Result<SpatialAssets, Error> {
        if workspace.is_some()
            && (self.input_bytes > mycity_map::assets::NORMAL_INPUT_BYTES as u64
                || self.png_bytes > mycity_map::assets::NORMAL_PNG_BYTES as u64)
        {
            return Err(Error::Invalid("spatial_normal_source_limit"));
        }
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
        if let Some(w) = workspace {
            let meta = metadata.ok_or(Error::Invalid("spatial_normal_metadata_missing"))?;
            meta.spatial_input
                .read(&dir.join(INPUT_FILE), self.input_bytes as usize)?;
            meta.png
                .read(&dir.join(PNG_FILE), self.png_bytes as usize)?;
            let input = meta.spatial_input.as_slice();
            let png = meta.png.as_slice();
            if input.len() as u64 != self.input_bytes || sha256(input) != self.input_sha256 {
                return Err(Error::Invalid("spatial_input_corrupt"));
            }
            if png.len() as u64 != self.png_bytes || sha256(png) != self.png_sha256 {
                return Err(Error::Invalid("spatial_png_corrupt"));
            }
            let assets = w
                .from_bytes(input, png, identity)
                .map_err(|e| Error::Model(format!("spatial_{e:?}")))?;
            self.verify_derived(&assets)?;
            return Ok(assets);
        }
        // General importer remains independent of the normal role's source profile.
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
        self.verify_derived(&assets)?;
        Ok(assets)
    }
    fn verify_derived(&self, assets: &SpatialAssets) -> Result<(), Error> {
        if assets.terrain().state_hash() != self.terrain_sha256
            || assets.scene().state_hash() != self.geometry_sha256
        {
            return Err(Error::Invalid("spatial_derived_state_mismatch"));
        }
        Ok(())
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

/// The regional prototype explicitly reuses one immutable flat terrain template for the profile's bounded three to eight metric frames.
/// These are dedicated metric terminals, not proof that a walking road supports trucks or pipes.
pub(crate) fn validate_exchange_binding(
    engine: &mycity_core::transaction::Engine,
    assets: Option<&SpatialAssets>,
) -> Result<(), Error> {
    let Some(exchange) = engine.world().view().exchange else {
        return Ok(());
    };
    let assets = assets.ok_or(Error::Invalid("exchange_spatial_bundle_required"))?;
    let terrain = assets.terrain();
    let manifest = terrain.manifest();
    let scene = assets.scene().input();
    if !manifest.synthetic
        || !matches!(manifest.validity, mycity_map::terrain::Validity::AllValid)
        || !matches!(manifest.water, mycity_map::terrain::Water::AllDry)
        || terrain.heights().iter().any(|h| *h != 0)
        || scene.map.size.side_cm() != 102400
        || scene.neighbor.size.side_cm() != 102400
    {
        return Err(Error::Invalid("exchange_flat_terrain_template"));
    }
    for frame in [&scene.map, &scene.neighbor] {
        if frame.origin.y != 0
            || frame.metric_frame != "synthetic-local-v1"
            || frame.vertical_datum != "synthetic-z0"
            || !exchange
                .maps
                .iter()
                .any(|p| p.city == frame.map && p.origin_x_cm == frame.origin.x)
        {
            return Err(Error::Invalid("exchange_spatial_frame_mismatch"));
        }
    }
    if scene.map.map != exchange.home_city {
        return Err(Error::Invalid("exchange_spatial_home"));
    }
    Ok(())
}
