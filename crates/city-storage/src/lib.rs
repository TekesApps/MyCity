//! Laboratory persistence: one writer, bounded replay, WAL + explicit committed boundary.
//! No historical migration, power-loss certification or automatic fallback of confirmed data.
mod background;
pub use background::{
    BackgroundPoint, BackgroundReserve, BACKGROUND_ASSET_RESERVE, BACKGROUND_LIVE_ASSETS,
    BACKGROUND_SLOTS,
};
mod capacity;
mod genesis;
mod metadata;
mod prepared;
pub use metadata::{MetadataIdentity, MetadataWorkspace};
pub use prepared::{StoreWorkspace, StoreWorkspaceIdentity};
mod continuation;
pub mod model_capacity;
pub use continuation::{Continuation, CONTINUATION_MAX_PAGES, CONTINUATION_PAGE_BYTES};
mod outbox;
mod query;
pub use outbox::{SourcePoint, SourceStatus};
pub use query::{Page, Query};
mod checkpoint;
mod checkpoint_v2;
mod stream;
mod wal;
pub use capacity::{CapacityPolicy, CapacityStatus};
mod spatial;
pub mod spatial_transport_updates;
mod town_transport;
pub use checkpoint::{CheckpointPoint, CheckpointStatus};
use fs2::FileExt;
use mycity_core::{
    read_bounded, sha256,
    transaction::{
        Caller, CommitContext, Engine, Envelope, JournalRecord, Receipt, Resource, TxError,
        MAX_RECEIPTS,
    },
    Bootstrap, INPUT_LIMIT, SCHEMA,
};
use mycity_map::assets::SpatialAssets;
use serde::{Deserialize, Serialize};
pub use spatial::{CreatePoint, SpatialReference};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const FRAME_LIMIT: usize = 32 * 1024;
pub const WAL_LIMIT: u64 = 64 * 1024 * 1024;
const META_LIMIT: u64 = 4096;
const GENESIS_LIMIT: u64 = 512 * 1024;
static TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(&'static str),
    Model(String),
    Locked,
    RecoveryRequired,
    FactBlocked(&'static str),
    SourceSyncRequired,
    Capacity {
        used_bytes: u64,
        limit_bytes: u64,
        required_bytes: u64,
        emergency: bool,
    },
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::Invalid("invalid_json")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritePoint {
    TransportProofEncoded,
    TransportProofPublished,
    Prepared,
    WalEncoded,
    WalAdmitted,
    WalHeaderHalf,
    WalPayloadHalf,
    WalChecksumHalf,
    WalWritten,
    WalSynced,
    RootEncoded,
    RootPublished,
    CurrentEncoded,
    CurrentRenamed,
    CurrentSynced,
    Applied,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}
#[derive(Serialize)]
struct Genesis<'a> {
    schema: &'a str,
    rules_identity: &'a str,
    #[serde(default, skip_serializing_if = "is_zero")]
    checkpoint_layout: u8,
    opening: &'a [u8],
    resources: &'a [Resource],
    #[serde(skip_serializing_if = "Option::is_none")]
    spatial: Option<&'a SpatialReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spatial_permissions:
        Option<&'a mycity_core::spatial_transport_permissions::SpatialPermissionState>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Root {
    #[serde(deserialize_with = "metadata::text::<_,64>")]
    schema: String,
    sequence: u64,
    generation: u64,
    capacity: CapacityPolicy,
    source_registered: bool,
    continuation: Option<Continuation>,
    background_reserve: Option<BackgroundReserve>,
    #[serde(default)]
    worker_cache_bytes: u64,
    checkpoint: Option<checkpoint::Reference>,
    #[serde(default, skip_serializing_if = "is_zero")]
    checkpoint_layout: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checkpoint_v2: Option<checkpoint_v2::Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spatial_transport_catalog_sha256: Option<[u8; 32]>,
    segment: u64,
    wal_end: u64,
    #[serde(deserialize_with = "metadata::text::<_,64>")]
    wal_chain: String,
    #[serde(deserialize_with = "metadata::text::<_,64>")]
    genesis_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    #[serde(deserialize_with = "metadata::text::<_,64>")]
    schema: String,
    slot: u8,
    #[serde(deserialize_with = "metadata::text::<_,64>")]
    root_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub model: Result<Receipt, TxError>,
    pub has_persistent_receipt: bool,
    pub committed_sequence: u64,
}

struct Persistence {
    owner_pid: u32,
    dir: PathBuf,
    _lock: File,
    wal: File,
    root: Root,
    wal_base_sequence: u64,
    wal_base_chain: String,
    poisoned: bool,
    source_reconciled: bool,
    background_live: Vec<[u8; 32]>,
    canonical: std::cell::RefCell<mycity_core::canonical::Codec>,
    metadata: MetadataWorkspace,
}
impl Drop for Persistence {
    fn drop(&mut self) {
        // Explicit ownership handoff: close alone may leave flock alive briefly in
        // descriptors inherited by an unrelated concurrently spawning child.
        // The lock is private, never cloned, and no writes occur after this point.
        if std::process::id() == self.owner_pid {
            if let Err(e) = FileExt::unlock(&self._lock) {
                let _ = writeln!(io::stderr(), "mycity-storage: unlock_failed: {e}");
            }
        }
    }
}
pub struct Store {
    engine: Engine,
    restore_workspace: mycity_core::transaction::EngineSlot,
    air_candidate: mycity_core::air::BackgroundStateSlot,
    spatial: Option<SpatialAssets>,
    spatial_ref: Option<SpatialReference>,
    io: Persistence,
}

fn validate_spatial_genesis_permissions(
    engine: &Engine,
    permissions: &mycity_core::spatial_transport_permissions::SpatialPermissionState,
    scene_sha256: &str,
    profile: &mycity_core::market_access::TransportAllocationProfile,
    profile_sha256: [u8; 32],
) -> Result<(), Error> {
    use mycity_core::spatial_transport_permissions::{SpatialAccessSurface, SpatialAssetKind};
    permissions
        .validate()
        .map_err(|_| Error::Invalid("spatial_permissions_genesis_invalid"))?;
    if permissions.world_id != engine.world().view().scope.world()
        || permissions.branch_id != engine.world().view().scope.branch()
        || scene_sha256.len() != 64
        || profile.world_id != permissions.world_id
        || profile.branch_id != permissions.branch_id
        || profile.market_profile_id == 0
    {
        return Err(Error::Invalid("spatial_permissions_genesis_scope"));
    }
    let view = engine.world().view();
    for asset in &permissions.assets {
        if asset.revision != 1
            || asset.current_profile_sha256 != profile_sha256
            || hex(&asset.source_scene_sha256) != scene_sha256
            || !view
                .accounts
                .iter()
                .any(|account| account.id == asset.owner_account)
            || !view
                .accounts
                .iter()
                .any(|account| account.id == asset.permit_authority_account)
        {
            return Err(Error::Invalid("spatial_permissions_genesis_binding"));
        }
        for managed in &asset.edges {
            if !managed.initially_open || !managed.currently_open {
                return Err(Error::Invalid(
                    "spatial_permissions_edge_not_open_at_genesis",
                ));
            }
            let edge = profile
                .opening
                .edges
                .iter()
                .find(|edge| edge.id == managed.edge_id && edge.open)
                .ok_or(Error::Invalid("spatial_permissions_edge_missing"))?;
            if edge.mode_mask & 1 == 0 {
                return Err(Error::Invalid("spatial_permissions_edge_not_walkable"));
            }
            match (asset.kind, managed.surface) {
                (SpatialAssetKind::Road, SpatialAccessSurface::Road) => {}
                (SpatialAssetKind::Building, SpatialAccessSurface::Door) => {
                    let touches_owned_entrance = profile.opening.entrances.iter().any(|entrance| {
                        entrance.authorized
                            && entrance.owner == asset.owner_account
                            && (entrance.node == edge.from || entrance.node == edge.to)
                    });
                    if !touches_owned_entrance {
                        return Err(Error::Invalid("spatial_door_edge_owner_mismatch"));
                    }
                }
                _ => return Err(Error::Invalid("spatial_permissions_surface_kind_mismatch")),
            }
        }
    }
    Ok(())
}

/// Copy the host's immutable permission genesis into the Engine's fixed-capacity backing.
/// A derived `Clone` preserves logical rows but may shrink nested `Vec` capacities, which is
/// intentionally rejected by `SpatialPermissionState::validate` and would break hot copies.
fn copy_spatial_permissions_into_engine(
    source: &mycity_core::spatial_transport_permissions::SpatialPermissionState,
) -> Result<mycity_core::spatial_transport_permissions::SpatialPermissionState, Error> {
    let mut target =
        mycity_core::spatial_transport_permissions::SpatialPermissionState::empty_backing()
            .map_err(|_| Error::Invalid("spatial_permissions_backing"))?;
    source
        .copy_into(&mut target)
        .map_err(|_| Error::Invalid("spatial_permissions_copy"))?;
    Ok(target)
}

fn json<T: Serialize>(v: &T) -> Result<Vec<u8>, Error> {
    Ok(serde_json::to_vec(v)?)
}
fn parse_digest(value: &str) -> Result<[u8; 32], Error> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::Invalid("sha256_format"));
    }
    let mut digest = [0u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| Error::Invalid("sha256_format"))?;
        digest[index] =
            u8::from_str_radix(text, 16).map_err(|_| Error::Invalid("sha256_format"))?;
    }
    Ok(digest)
}
fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::Invalid("symlink_dependency"));
    }
    if !metadata.is_file() || metadata.len() > limit {
        return Err(Error::Invalid("dependency_type_or_length"));
    }
    Ok(read_bounded(File::open(path)?, limit)?)
}
fn sync_directory(dir: &Path) -> Result<(), Error> {
    File::open(dir)?.sync_all()?;
    Ok(())
}
fn lock(dir: &Path) -> Result<File, Error> {
    let p = dir.join("LOCK");
    if p.exists() && fs::symlink_metadata(&p)?.file_type().is_symlink() {
        return Err(Error::Invalid("symlink_lock"));
    }
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(p)?;
    FileExt::try_lock_exclusive(&f).map_err(|_| Error::Locked)?;
    Ok(f)
}
fn atomic_file(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    after_rename: impl FnOnce() -> io::Result<()>,
) -> Result<(), Error> {
    let id = TEMP_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map_err(|_| Error::Invalid("temp_generation_overflow"))?;
    let temp = dir.join(format!(".{}-{}-{id}.tmp", name, std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    fs::rename(temp, dir.join(name))?;
    after_rename()?;
    sync_directory(dir)
}

impl Store {
    pub fn create(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
    ) -> Result<Self, Error> {
        Self::create_internal(dir, opening, resources, rules_identity, None, 0, |_| Ok(()))
    }
    pub fn create_spatial(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        input: &[u8],
        png: &[u8],
    ) -> Result<Self, Error> {
        Self::create_spatial_with_observer(
            dir,
            opening,
            resources,
            rules_identity,
            (input, png),
            |_| Ok(()),
        )
    }
    /// A failure after CURRENT publication is uncertain; reopen the existing directory.
    /// Never delete or initialize over it in response to an I/O failure.
    pub fn create_spatial_with_observer(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
        observer: impl FnMut(CreatePoint) -> io::Result<()>,
    ) -> Result<Self, Error> {
        Self::create_internal(
            dir,
            opening,
            resources,
            rules_identity,
            Some(bundle),
            0,
            observer,
        )
    }
    /// Explicit new-world format selection. Existing worlds never upgrade implicitly.
    pub fn create_spatial_v2(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        input: &[u8],
        png: &[u8],
    ) -> Result<Self, Error> {
        Self::create_internal(
            dir,
            opening,
            resources,
            rules_identity,
            Some((input, png)),
            2,
            |_| Ok(()),
        )
    }
    fn create_internal(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: Option<(&[u8], &[u8])>,
        checkpoint_layout: u8,
        observer: impl FnMut(CreatePoint) -> io::Result<()>,
    ) -> Result<Self, Error> {
        Self::create_in_workspace(
            dir,
            opening,
            resources,
            rules_identity,
            bundle,
            (
                checkpoint_layout,
                mycity_core::transaction::EngineWorkspace::new().map_err(Error::Model)?,
                None,
                MetadataWorkspace::new()?,
            ),
            observer,
        )
    }
    /// Explicit role-owned backing. The bool chooses the already supported v1/v2 layout.
    pub fn create_spatial_in_workspace(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
        v2: bool,
        workspace: mycity_core::transaction::EngineWorkspace,
    ) -> Result<Self, Error> {
        Self::create_in_workspace(
            dir,
            opening,
            resources,
            rules_identity,
            Some(bundle),
            (
                if v2 { 2 } else { 0 },
                workspace,
                None,
                MetadataWorkspace::new()?,
            ),
            |_| Ok(()),
        )
    }
    /// Consume prepared core AND spatial backing; over-profile sources never fall back.
    pub fn create_spatial_in_role_workspace(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
        v2: bool,
        workspace: (
            mycity_core::transaction::EngineWorkspace,
            mycity_map::assets::SpatialWorkspace,
        ),
    ) -> Result<Self, Error> {
        Self::create_in_workspace(
            dir,
            opening,
            resources,
            rules_identity,
            Some(bundle),
            (
                if v2 { 2 } else { 0 },
                workspace.0,
                Some(workspace.1),
                MetadataWorkspace::new()?,
            ),
            |_| Ok(()),
        )
    }
    pub fn create_spatial_in_prepared_workspace(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
        v2: bool,
        workspace: StoreWorkspace,
    ) -> Result<Self, Error> {
        Self::create_in_workspace(
            dir,
            opening,
            resources,
            rules_identity,
            Some(bundle),
            (
                if v2 { 2 } else { 0 },
                workspace.model,
                Some(workspace.spatial),
                workspace.metadata,
            ),
            |_| Ok(()),
        )
    }
    fn create_in_workspace(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: Option<(&[u8], &[u8])>,
        setup: (
            u8,
            mycity_core::transaction::EngineWorkspace,
            Option<mycity_map::assets::SpatialWorkspace>,
            MetadataWorkspace,
        ),
        observer: impl FnMut(CreatePoint) -> io::Result<()>,
    ) -> Result<Self, Error> {
        Self::create_in_workspace_with_transport(
            dir,
            opening,
            resources,
            rules_identity,
            bundle,
            None,
            setup,
            None,
            observer,
        )
    }
    /// Create a market world with its immutable Store-owned daily transport allocation.
    pub fn create_with_transport_allocation_profile(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        allocation_profile: &[u8],
    ) -> Result<Self, Error> {
        Self::create_in_workspace_with_transport(
            dir,
            opening,
            resources,
            rules_identity,
            None,
            Some(allocation_profile),
            (
                0,
                mycity_core::transaction::EngineWorkspace::new().map_err(Error::Model)?,
                None,
                MetadataWorkspace::new()?,
            ),
            None,
            |_| Ok(()),
        )
    }
    /// Bind both the physical cross-city network and daily Town transport at genesis.
    /// All assets go through the same existing immutable admission and validation path.
    pub fn create_spatial_with_transport_allocation_profile(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
        allocation_profile: &[u8],
    ) -> Result<Self, Error> {
        Self::create_in_workspace_with_transport(
            dir,
            opening,
            resources,
            rules_identity,
            Some(bundle),
            Some(allocation_profile),
            (
                0,
                mycity_core::transaction::EngineWorkspace::new().map_err(Error::Model)?,
                None,
                MetadataWorkspace::new()?,
            ),
            None,
            |_| Ok(()),
        )
    }
    /// Create a spatial Store with an explicit host-configured road/door permission registry.
    /// The registry is checked against the exact immutable scene and transport profile before
    /// genesis is written; generic legacy constructors cannot silently acquire these rights.
    pub fn create_spatial_with_transport_permissions(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: (&[u8], &[u8]),
        allocation_profile: &[u8],
        permissions: mycity_core::spatial_transport_permissions::SpatialPermissionState,
    ) -> Result<Self, Error> {
        Self::create_in_workspace_with_transport(
            dir,
            opening,
            resources,
            rules_identity,
            Some(bundle),
            Some(allocation_profile),
            (
                0,
                mycity_core::transaction::EngineWorkspace::new().map_err(Error::Model)?,
                None,
                MetadataWorkspace::new()?,
            ),
            Some(permissions),
            |_| Ok(()),
        )
    }
    // Preserve the existing workspace setup tuple and observer API while adding an optional
    // immutable transport allocation at genesis.
    #[allow(clippy::too_many_arguments)]
    fn create_in_workspace_with_transport(
        dir: &Path,
        opening: &[u8],
        resources: Vec<Resource>,
        rules_identity: &str,
        bundle: Option<(&[u8], &[u8])>,
        allocation_bytes: Option<&[u8]>,
        setup: (
            u8,
            mycity_core::transaction::EngineWorkspace,
            Option<mycity_map::assets::SpatialWorkspace>,
            MetadataWorkspace,
        ),
        spatial_permissions: Option<
            mycity_core::spatial_transport_permissions::SpatialPermissionState,
        >,
        mut observer: impl FnMut(CreatePoint) -> io::Result<()>,
    ) -> Result<Self, Error> {
        if opening.len() as u64 > INPUT_LIMIT
            || rules_identity.is_empty()
            || rules_identity.len() > 128
        {
            return Err(Error::Invalid("genesis_input_limit"));
        }
        let (checkpoint_layout, workspace, spatial_workspace, mut metadata) = setup;
        let (mut initial, restore_workspace, air_candidate, codec) = workspace.into_parts();
        initial
            .initialize_into(opening, &resources)
            .map_err(Error::Model)?;
        let mut engine = initial.into_engine().map_err(Error::Model)?;
        let allocation_hash = town_transport::allocation_ref(&engine);
        let allocation_profile = match (allocation_bytes, allocation_hash) {
            (Some(bytes), Some(hash)) => {
                let town = engine
                    .world()
                    .view()
                    .town
                    .ok_or(Error::Invalid("town_missing"))?;
                let profile = town_transport::validate_allocation(
                    bytes,
                    engine.world().view().scope.world(),
                    engine.world().view().scope.branch(),
                    town.profile.id,
                    hash,
                )?;
                town_transport::validate_allocation_project(&engine, &profile)?;
                Some(profile)
            }
            (None, None) => {
                town_transport::validate_no_orphan_road_project(&engine)?;
                None
            }
            _ => return Err(Error::Invalid("town_transport_profile_reference")),
        };
        let identity = identity(&engine);
        let (spatial_ref, spatial) = if let Some((input, png)) = bundle {
            let (reference, assets) =
                SpatialReference::prepare_with_workspace(input, png, &identity, spatial_workspace)?;
            (Some(reference), Some(assets))
        } else {
            (None, None)
        };
        spatial::validate_exchange_binding(&engine, spatial.as_ref())?;
        if let Some(permissions) = spatial_permissions.as_ref() {
            let spatial_ref = spatial_ref
                .as_ref()
                .ok_or(Error::Invalid("spatial_permissions_require_scene"))?;
            let profile = allocation_profile
                .as_ref()
                .ok_or(Error::Invalid("spatial_permissions_require_transport"))?;
            validate_spatial_genesis_permissions(
                &engine,
                permissions,
                spatial_ref.geometry_sha256(),
                profile,
                allocation_hash.ok_or(Error::Invalid("spatial_profile_hash_missing"))?,
            )?;
            engine
                .initialize_spatial_permissions(copy_spatial_permissions_into_engine(permissions)?)
                .map_err(|error| Error::Model(format!("{error:?}")))?;
        }
        let initial_spatial_catalog = if let (Some(permissions), Some(reference), Some(hash)) = (
            spatial_permissions.as_ref(),
            spatial_ref.as_ref(),
            allocation_hash,
        ) {
            let baselines = permissions
                .assets
                .iter()
                .map(
                    |asset| spatial_transport_updates::SpatialTransportAssetBaseline {
                        asset_id: asset.asset_id,
                        kind: asset.kind,
                        revision: asset.revision,
                    },
                )
                .collect::<Vec<_>>();
            let catalog = spatial_transport_updates::SpatialTransportCatalog::new(
                permissions.world_id,
                permissions.branch_id,
                allocation_profile
                    .as_ref()
                    .ok_or(Error::Invalid("spatial_permissions_require_transport"))?
                    .market_profile_id,
                parse_digest(reference.geometry_sha256())?,
                hash,
                &baselines,
            )
            .map_err(|_| Error::Invalid("spatial_catalog_genesis"))?;
            Some(
                spatial_transport_updates::encode_spatial_transport_catalog(&catalog)
                    .map_err(|_| Error::Invalid("spatial_catalog_encode"))?,
            )
        } else {
            None
        };
        drop(allocation_profile);
        let canonical = std::cell::RefCell::new(codec);
        observer(CreatePoint::Validated)?;
        fs::create_dir(dir)?;
        let lock = lock(dir)?;
        if let (Some(bytes), Some(hash)) = (allocation_bytes, allocation_hash) {
            town_transport::admit_immutable_write(
                dir,
                CapacityPolicy::default(),
                town_transport::ALLOCATION_PREFIX,
                bytes,
                &hash,
                false,
            )?;
            town_transport::write_immutable(dir, town_transport::ALLOCATION_PREFIX, bytes, &hash)?;
        }
        if let Some(encoded) = initial_spatial_catalog.as_ref() {
            spatial_transport_updates::stage_spatial_transport_catalog(
                dir,
                CapacityPolicy::default(),
                encoded,
            )
            .map_err(|_| Error::Invalid("spatial_catalog_stage"))?;
        }
        if let Some((input, png)) = bundle {
            spatial::write_dependency(
                dir,
                spatial::INPUT_FILE,
                input,
                CreatePoint::SpatialInputHalf,
                CreatePoint::SpatialInputSynced,
                &mut observer,
            )?;
            spatial::write_dependency(
                dir,
                spatial::PNG_FILE,
                png,
                CreatePoint::SpatialPngHalf,
                CreatePoint::SpatialPngSynced,
                &mut observer,
            )?;
        }
        metadata.file.encode(
            &Genesis {
                schema: if spatial_ref.is_some() {
                    if spatial_permissions.is_some() {
                        "mycity-m0-genesis-v3"
                    } else {
                        "mycity-m0-genesis-v2"
                    }
                } else {
                    "mycity-m0-genesis-v1"
                },
                rules_identity,
                checkpoint_layout,
                opening,
                resources: &resources,
                spatial: spatial_ref.as_ref(),
                spatial_permissions: spatial_permissions.as_ref(),
            },
            GENESIS_LIMIT as usize,
        )?;
        let genesis = metadata.file.as_slice();
        atomic_file(dir, "genesis.json", genesis, || Ok(()))?;
        observer(CreatePoint::GenesisSynced)?;
        let wal = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.join("wal.bin"))?;
        wal.sync_all()?;
        observer(CreatePoint::WalSynced)?;
        let genesis_sha256 = sha256(genesis);
        let wal_base_chain = genesis_sha256.clone();
        let root = Root {
            schema: "mycity-m0-root-v1".into(),
            sequence: 0,
            generation: 0,
            capacity: CapacityPolicy::default(),
            source_registered: false,
            continuation: None,
            background_reserve: None,
            worker_cache_bytes: 0,
            checkpoint: None,
            checkpoint_layout,
            checkpoint_v2: None,
            spatial_transport_catalog_sha256: initial_spatial_catalog.map(|catalog| catalog.sha256),
            segment: 0,
            wal_end: 0,
            wal_chain: genesis_sha256.clone(),
            genesis_sha256,
        };
        metadata.small.encode(&root, META_LIMIT as usize)?;
        let root_bytes = metadata.small.as_slice();
        atomic_file(dir, "root-0.json", root_bytes, || Ok(()))?;
        observer(CreatePoint::RootSynced)?;
        let current = Current {
            schema: "mycity-m0-current-v1".into(),
            slot: 0,
            root_sha256: sha256(root_bytes),
        };
        metadata.small.encode(&current, META_LIMIT as usize)?;
        atomic_file(dir, "CURRENT.json", metadata.small.as_slice(), || {
            observer(CreatePoint::CurrentRenamed)
        })?;
        observer(CreatePoint::CurrentSynced)?;
        sync_directory(dir.parent().unwrap_or(Path::new(".")))?;
        observer(CreatePoint::DirectorySynced)?;
        observer(CreatePoint::Ready)?;
        Ok(Self {
            engine,
            restore_workspace,
            air_candidate,
            spatial,
            spatial_ref,
            io: Persistence {
                owner_pid: std::process::id(),
                dir: dir.to_owned(),
                _lock: lock,
                wal,
                root,
                wal_base_sequence: 0,
                wal_base_chain,
                poisoned: false,
                source_reconciled: !outbox::exists(dir),
                background_live: Vec::with_capacity(BACKGROUND_LIVE_ASSETS),
                canonical,
                metadata,
            },
        })
    }

    pub fn open(dir: &Path, rules_identity: &str) -> Result<Self, Error> {
        Self::open_in_workspace(
            dir,
            rules_identity,
            mycity_core::transaction::EngineWorkspace::new().map_err(Error::Model)?,
        )
    }
    pub fn open_in_workspace(
        dir: &Path,
        rules_identity: &str,
        workspace: mycity_core::transaction::EngineWorkspace,
    ) -> Result<Self, Error> {
        Self::open_with_workspaces(
            dir,
            rules_identity,
            workspace,
            None,
            MetadataWorkspace::new()?,
        )
    }
    pub fn open_in_role_workspace(
        dir: &Path,
        rules_identity: &str,
        workspace: (
            mycity_core::transaction::EngineWorkspace,
            mycity_map::assets::SpatialWorkspace,
        ),
    ) -> Result<Self, Error> {
        Self::open_with_workspaces(
            dir,
            rules_identity,
            workspace.0,
            Some(workspace.1),
            MetadataWorkspace::new()?,
        )
    }
    pub fn open_in_prepared_workspace(
        dir: &Path,
        rules_identity: &str,
        workspace: StoreWorkspace,
    ) -> Result<Self, Error> {
        Self::open_with_workspaces(
            dir,
            rules_identity,
            workspace.model,
            Some(workspace.spatial),
            workspace.metadata,
        )
    }
    fn open_with_workspaces(
        dir: &Path,
        rules_identity: &str,
        workspace: mycity_core::transaction::EngineWorkspace,
        spatial_workspace: Option<mycity_map::assets::SpatialWorkspace>,
        mut metadata: MetadataWorkspace,
    ) -> Result<Self, Error> {
        let (mut initial, mut restore_workspace, mut air_candidate, codec) = workspace.into_parts();
        if fs::symlink_metadata(dir)?.file_type().is_symlink() {
            return Err(Error::Invalid("symlink_world"));
        }
        let lock = lock(dir)?;
        metadata
            .small
            .read(&dir.join("CURRENT.json"), META_LIMIT as usize)?;
        metadata::guard(metadata.small.as_slice(), 768)?;
        let current: Current = serde_json::from_slice(metadata.small.as_slice())?;
        if current.schema != "mycity-m0-current-v1" || current.slot > 1 {
            return Err(Error::Invalid("current_schema_or_slot"));
        }
        metadata.small.read(
            &dir.join(format!("root-{}.json", current.slot)),
            META_LIMIT as usize,
        )?;
        let root_bytes = metadata.small.as_slice();
        if sha256(root_bytes) != current.root_sha256 {
            return Err(Error::Invalid("committed_root_corrupt"));
        }
        metadata::guard(root_bytes, 768)?;
        let root: Root = serde_json::from_slice(root_bytes)?;
        if root.generation % 2 != u64::from(current.slot)
            || root.schema != "mycity-m0-root-v1"
            || root.wal_end > WAL_LIMIT
        {
            return Err(Error::Invalid("root_limits_or_schema"));
        }
        root.capacity.validate()?;
        background::validate_cache_charge(&root)?;
        if root.source_registered && !outbox::exists(dir) {
            return Err(Error::Invalid("registered_source_missing"));
        }
        metadata
            .file
            .read(&dir.join("genesis.json"), GENESIS_LIMIT as usize)?;
        let genesis_bytes = metadata.file.as_slice();
        if sha256(genesis_bytes) != root.genesis_sha256 {
            return Err(Error::Invalid("genesis_corrupt"));
        }
        let genesis = genesis::decode(&mut metadata)?;
        let valid_schema = matches!(
            (
                genesis.schema.as_str(),
                genesis.spatial.is_some(),
                genesis.spatial_permissions.is_some()
            ),
            ("mycity-m0-genesis-v1", false, false)
                | ("mycity-m0-genesis-v2", true, false)
                | ("mycity-m0-genesis-v3", true, true)
        );
        if !valid_schema || genesis.rules_identity != rules_identity {
            return Err(Error::Invalid("unsupported_rules_or_schema"));
        }
        // Fresh reconstruction of the recorded genesis, not a gift to an existing world.
        initial
            .initialize_into(metadata.opening.as_slice(), &metadata.resources)
            .map_err(Error::Model)?;
        let mut genesis_engine = initial.into_engine().map_err(Error::Model)?;
        let transport_profile_hash = town_transport::allocation_ref(&genesis_engine);
        let transport_profile = if let Some(hash) = transport_profile_hash {
            let town = genesis_engine
                .world()
                .view()
                .town
                .ok_or(Error::Invalid("town_transport_market_missing"))?;
            let profile = town_transport::read_allocation(
                dir,
                &mut metadata,
                &hash,
                genesis_engine.world().view().scope.world(),
                genesis_engine.world().view().scope.branch(),
                town.profile.id,
            )?;
            town_transport::validate_allocation_project(&genesis_engine, &profile)?;
            Some(profile)
        } else {
            town_transport::validate_no_orphan_road_project(&genesis_engine)?;
            None
        };
        if let Some(permissions) = genesis.spatial_permissions.as_ref() {
            let spatial_ref = genesis
                .spatial
                .as_ref()
                .ok_or(Error::Invalid("spatial_permissions_require_scene"))?;
            let profile = transport_profile
                .as_ref()
                .ok_or(Error::Invalid("spatial_permissions_require_transport"))?;
            validate_spatial_genesis_permissions(
                &genesis_engine,
                permissions,
                spatial_ref.geometry_sha256(),
                profile,
                transport_profile_hash.ok_or(Error::Invalid("spatial_profile_hash_missing"))?,
            )?;
            genesis_engine
                .initialize_spatial_permissions(copy_spatial_permissions_into_engine(permissions)?)
                .map_err(|error| Error::Model(format!("{error:?}")))?;
        }
        let transport_catalog = load_spatial_transport_catalog_on_open(
            dir,
            root.spatial_transport_catalog_sha256,
            &genesis_engine,
            genesis.spatial.as_ref(),
            transport_profile_hash,
            &mut metadata,
        )?;
        if root.checkpoint_layout != genesis.checkpoint_layout
            || !matches!(root.checkpoint_layout, 0 | 2)
            || root.checkpoint_layout == 0 && root.checkpoint_v2.is_some()
            || root.checkpoint_layout == 2 && root.checkpoint.is_some()
        {
            return Err(Error::Invalid("checkpoint_layout_binding"));
        }
        let (mut sequence, mut chain) = if let Some(reference) = &root.checkpoint_v2 {
            let (sequence, chain) = checkpoint_v2::open_into(
                dir,
                reference,
                &root.genesis_sha256,
                root.sequence,
                transport_profile.as_ref(),
                transport_profile_hash,
                transport_catalog.as_ref(),
                &mut restore_workspace,
                &mut metadata,
            )?;
            restore_workspace
                .publish_swap(&mut genesis_engine)
                .map_err(Error::Model)?;
            (sequence, chain)
        } else if let Some(reference) = &root.checkpoint {
            let cp = checkpoint::load(dir, reference, &mut metadata)?;
            if cp.sequence > root.sequence || cp.genesis_sha256 != root.genesis_sha256 {
                return Err(Error::Invalid("checkpoint_boundary"));
            }
            restore_workspace
                .restore_into(metadata.engine.as_slice())
                .map_err(Error::Model)?;
            checkpoint::verify_archive(dir, reference, &root.genesis_sha256, &mut metadata)?;
            restore_workspace
                .publish_swap(&mut genesis_engine)
                .map_err(Error::Model)?;
            (cp.sequence, cp.chain)
        } else {
            (0, root.genesis_sha256.clone())
        };
        let wal_base_sequence = sequence;
        let wal_base_chain = chain.clone();
        town_transport::verify_latest_market_cursor(
            &genesis_engine,
            dir,
            &mut metadata,
            transport_profile.as_ref(),
            transport_profile_hash,
            transport_catalog.as_ref(),
        )?;
        if let Some(reference) = root.checkpoint.as_ref() {
            checkpoint::verify_transport_proofs(
                dir,
                reference,
                transport_profile.as_ref(),
                transport_profile_hash,
                transport_catalog.as_ref(),
                &genesis_engine,
                &mut metadata,
            )?;
        }
        let mut engine = genesis_engine;
        outbox::verify(dir, engine.world().view().scope)?;
        let spatial_ref = genesis.spatial;
        let spatial = spatial_ref
            .as_ref()
            .map(|r| {
                r.load_with_workspace(
                    dir,
                    &identity(&engine),
                    spatial_workspace,
                    Some(&mut metadata),
                )
            })
            .transpose()?;
        spatial::validate_exchange_binding(&engine, spatial.as_ref())?;
        let wal_path = dir.join(checkpoint::wal_name(root.segment));
        if fs::symlink_metadata(&wal_path)?.file_type().is_symlink() {
            return Err(Error::Invalid("symlink_wal"));
        }
        let mut wal = OpenOptions::new().read(true).write(true).open(wal_path)?;
        let wal_length = wal.metadata()?.len();
        if root.wal_end > wal_length {
            return Err(Error::Invalid("committed_wal_truncated"));
        }
        let mut canonical = std::cell::RefCell::new(codec);
        let mut buffers = wal::Buffers::new();
        let mut pos = 0u64;
        let mut background_live = Vec::with_capacity(BACKGROUND_LIVE_ASSETS);
        while pos < root.wal_end {
            if root.wal_end - pos < 4 {
                return Err(Error::Invalid("committed_header_truncated"));
            }
            let mut len = [0u8; 4];
            wal.read_exact(&mut len)?;
            let length = u32::from_le_bytes(len) as usize;
            if length == 0 || length > FRAME_LIMIT || root.wal_end - pos < 4 + length as u64 + 32 {
                return Err(Error::Invalid("committed_frame_length"));
            }
            let payload = &mut buffers.frame[..length];
            wal.read_exact(payload)?;
            let mut checksum = [0u8; 32];
            wal.read_exact(&mut checksum)?;
            let actual = wal::checksum(&len, payload);
            if actual != checksum {
                return Err(Error::Invalid("committed_frame_checksum"));
            }
            let entry = wal::decode(payload)?;
            sequence += 1;
            if entry.sequence != sequence || entry.previous != chain {
                return Err(Error::Invalid("committed_frame_chain"));
            }
            let mut normalized = mycity_core::canonical::FixedWriter::new(&mut buffers.record);
            canonical
                .get_mut()
                .workspace
                .write(entry.record.get().as_bytes(), &mut normalized)
                .map_err(Error::Model)?;
            // Normalize first: the old Value path used the last duplicate key.
            let record: JournalRecord = serde_json::from_slice(normalized.as_slice())?;
            let actor = record.actor;
            let caller = if actor == 0 {
                Caller::System
            } else {
                Caller::Account(actor)
            };
            let candidate_hash =
                background::successful_hash(&entry.envelope, record.outcome.is_ok());
            let candidate = if let Some(hash) = &candidate_hash {
                background::load_asset_into(dir, hash, &mut air_candidate)?;
                Some(air_candidate.state().map_err(Error::Model)?)
            } else {
                None
            };
            if let Some(hash) = candidate_hash {
                background::remember(&mut background_live, hash)?;
            }
            let transport_proof = match entry.envelope.actions.as_slice() {
                [mycity_core::transaction::Action::TownTransportSettle {
                    profile_id,
                    service_day,
                    generation,
                    proof_sha256,
                }] => {
                    let catalog_hash = if let Some(catalog) = transport_catalog.as_ref() {
                        if catalog.profile_id != *profile_id {
                            return Err(Error::Invalid("town_transport_profile_mismatch"));
                        }
                        Some(
                            catalog
                                .profile_sha256_for_service_day(*service_day)
                                .map_err(|_| {
                                    Error::Invalid("town_transport_catalog_service_day")
                                })?,
                        )
                    } else {
                        None
                    };
                    let proof = town_transport::read_proof(dir, &mut metadata, *proof_sha256)?;
                    Some((
                        proof,
                        *profile_id,
                        *service_day,
                        *generation,
                        *proof_sha256,
                        catalog_hash,
                    ))
                }
                _ => None,
            };
            let transport_input = if let Some((
                proof,
                profile_id,
                service_day,
                generation,
                proof_hash,
                selected_hash,
            )) = transport_proof.as_ref()
            {
                let allocation_hash = (*selected_hash)
                    .or(transport_profile_hash)
                    .ok_or(Error::Invalid("town_transport_profile_missing"))?;
                let market_profile_id = engine
                    .world()
                    .view()
                    .town
                    .ok_or(Error::Invalid("town_transport_market_missing"))?
                    .profile
                    .id;
                let profile = town_transport::read_allocation_for_settlement(
                    dir,
                    &mut metadata,
                    &allocation_hash,
                    engine.world().view().scope.world(),
                    engine.world().view().scope.branch(),
                    market_profile_id,
                )?;
                let input = town_transport::verify_proof_matches_allocation(
                    &engine,
                    proof,
                    &profile,
                    allocation_hash,
                    *proof_hash,
                    *service_day,
                    *generation,
                )?;
                if *profile_id != profile.market_profile_id {
                    return Err(Error::Invalid("town_transport_profile_mismatch"));
                }
                Some(input)
            } else {
                None
            };
            let mut called = false;
            let _ = engine.execute_recorded_transport_diagnosed(
                caller,
                entry.envelope,
                candidate,
                transport_input.as_ref(),
                |_, record| -> Result<(), Error> {
                    called = true;
                    if !wal::matches(
                        &mut canonical.get_mut().workspace,
                        record,
                        &mut buffers.frame,
                        normalized.as_slice(),
                    )? {
                        return Err(Error::Invalid("replay_outcome_mismatch"));
                    }
                    Ok(())
                },
                &mut mycity_core::diagnostics::Trace::disabled(),
            )?;
            if !called {
                return Err(Error::Invalid("record_not_admitted_or_duplicate"));
            }
            chain = hex(&actual);
            pos += 4 + length as u64 + 32;
        }
        if pos != root.wal_end || sequence != root.sequence || chain != root.wal_chain {
            return Err(Error::Invalid("committed_boundary_mismatch"));
        }
        validate_spatial_transport_engine_state(
            &engine,
            transport_catalog.as_ref(),
            genesis.spatial_permissions.as_ref(),
        )?;
        Self::validate_continuation(&engine, &root)?;
        Self::validate_background(&engine, dir, &root, &mut air_candidate)?;
        // Only bytes provably beyond CURRENT's validated committed boundary are removable.
        if wal_length > root.wal_end {
            wal.set_len(root.wal_end)?;
            wal.sync_all()?;
        }
        // A validated CURRENT can reference only fixed root names, never these temps.
        // Remove our orphaned atomic-write files, without following links or touching
        // arbitrary user files. Stream directory entries rather than collecting them.
        let mut cleaned = false;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if [".CURRENT.json-", ".root-0.json-", ".root-1.json-"]
                .iter()
                .any(|p| name.starts_with(p))
                && name.ends_with(".tmp")
                && entry.file_type()?.is_file()
            {
                fs::remove_file(entry.path())?;
                cleaned = true;
            }
        }
        if cleaned {
            sync_directory(dir)?;
        }
        if root.checkpoint_layout == 2 {
            checkpoint_v2::gc(dir, root.checkpoint_v2.as_ref(), root.segment, |_| Ok(()))?;
        }
        background::gc(dir, &root, &background_live)?;
        wal.seek(SeekFrom::Start(root.wal_end))?;
        Ok(Self {
            engine,
            restore_workspace,
            air_candidate,
            spatial,
            spatial_ref,
            io: Persistence {
                owner_pid: std::process::id(),
                dir: dir.to_owned(),
                _lock: lock,
                wal,
                root,
                wal_base_sequence,
                wal_base_chain,
                poisoned: false,
                source_reconciled: !outbox::exists(dir),
                background_live,
                canonical,
                metadata,
            },
        })
    }

    pub fn engine(&self) -> Result<&Engine, Error> {
        if self.io.poisoned || self.io.owner_pid != std::process::id() {
            Err(Error::RecoveryRequired)
        } else {
            Ok(&self.engine)
        }
    }
    /// Stable validation destination, not a broker Filled or physical-memory proof.
    pub fn restore_workspace_identity(
        &self,
    ) -> Result<mycity_core::transaction::SlotIdentity, Error> {
        self.engine()?;
        self.restore_workspace.identity().map_err(Error::Model)
    }
    /// Stable candidate backing only; not a successful candidate or pool grant.
    pub fn candidate_workspace_identity(
        &self,
    ) -> Result<mycity_core::air::StateSlotIdentity, Error> {
        self.engine()?;
        Ok(self.air_candidate.identity())
    }
    pub fn model_workspace_identity(
        &self,
    ) -> Result<mycity_core::transaction::EngineWorkspaceIdentity, Error> {
        self.engine()?;
        let initial = self.engine.backing_identity().map_err(Error::Model)?;
        let restore = self.restore_workspace.identity().map_err(Error::Model)?;
        let candidate = self.air_candidate.identity();
        let codec = self.io.canonical.borrow().identity();
        let owned_bytes = mycity_core::transaction::EngineWorkspace::capacity_from_parts(
            &initial, &restore, &candidate, &codec,
        );
        Ok(mycity_core::transaction::EngineWorkspaceIdentity {
            initial,
            restore,
            candidate,
            codec,
            owned_bytes,
        })
    }
    pub fn spatial(&self) -> Result<Option<&SpatialAssets>, Error> {
        self.engine()?;
        Ok(self.spatial.as_ref())
    }
    pub fn spatial_reference(&self) -> Result<Option<&SpatialReference>, Error> {
        self.engine()?;
        Ok(self.spatial_ref.as_ref())
    }
    /// Joint identity of financial/runtime state and immutable CPU spatial dependencies.
    pub fn business_digest(&self) -> Result<String, Error> {
        let mut codec = self
            .io
            .canonical
            .try_borrow_mut()
            .map_err(|_| Error::Model("store_comparison_busy".into()))?;
        self.engine()?
            .business_digest_with(&mut codec)
            .map_err(Error::Model)
    }
    pub fn state_hash(&self) -> Result<String, Error> {
        #[derive(Serialize)]
        struct View<'a> {
            schema: &'static str,
            engine: String,
            spatial: Option<&'a SpatialReference>,
        }
        let mut codec = self
            .io
            .canonical
            .try_borrow_mut()
            .map_err(|_| Error::Model("store_comparison_busy".into()))?;
        let engine = self
            .engine()?
            .state_hash_with(&mut codec)
            .map_err(Error::Model)?;
        codec
            .hash(&View {
                schema: "mycity-m0-store-state-v1",
                engine,
                spatial: self.spatial_reference()?,
            })
            .map_err(Error::Model)
    }
    /// Owned Rust capacities only; excludes allocator/kernel/OS pages and transient replay.
    pub fn owned_capacity_bytes(&self) -> Result<usize, Error> {
        let initial = self.engine()?.backing_identity().map_err(Error::Model)?;
        let spare = self.restore_workspace.identity().map_err(Error::Model)?;
        Ok(std::mem::size_of::<Self>() - std::mem::size_of::<Engine>()
            + self.engine()?.owned_capacity_bytes()
            + spare.owned_model_bytes
            - std::mem::size_of::<Engine>()
            + initial.retained_prepare_bytes
            + spare.retained_prepare_bytes
            + self
                .io
                .root
                .checkpoint
                .as_ref()
                .map_or(0, |r| r.heap_capacity_bytes())
            + self
                .io
                .root
                .checkpoint_v2
                .as_ref()
                .map_or(0, |r| r.heap_capacity_bytes())
            + self.air_candidate.owned_bytes()
            - std::mem::size_of::<mycity_core::air::BackgroundStateSlot>()
            + self.io.dir.capacity()
            + self.io.background_live.capacity() * std::mem::size_of::<[u8; 32]>()
            + self.io.canonical.borrow().backing_capacity_bytes()
            + self.io.metadata.identity().owned_bytes
            - std::mem::size_of::<MetadataWorkspace>()
            + self.io.root.schema.capacity()
            + self.io.root.wal_chain.capacity()
            + self.io.root.genesis_sha256.capacity()
            + self
                .io
                .root
                .spatial_transport_catalog_sha256
                .as_ref()
                .map_or(0, |_| std::mem::size_of::<[u8; 32]>())
            + self.spatial()?.map_or(0, |a| {
                a.owned_capacity_bytes() - std::mem::size_of::<SpatialAssets>()
            })
            + self.spatial_reference()?.map_or(0, |r| {
                r.owned_capacity_bytes() - std::mem::size_of::<SpatialReference>()
            }))
    }
    pub fn committed_sequence(&self) -> u64 {
        self.io.root.sequence
    }
    fn spatial_transport_catalog(
        &mut self,
    ) -> Result<Option<spatial_transport_updates::SpatialTransportCatalog>, Error> {
        self.engine()?;
        self.io
            .root
            .spatial_transport_catalog_sha256
            .map(|hash| {
                let catalog =
                    spatial_transport_updates::read_spatial_transport_catalog(&self.io.dir, hash)
                        .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
                let permissions = self
                    .engine
                    .spatial_permissions()
                    .ok_or(Error::Invalid("spatial_permissions_missing"))?;
                let first_asset = permissions
                    .assets()
                    .first()
                    .ok_or(Error::Invalid("spatial_permission_asset_missing"))?;
                let context = permissions
                    .authorization_context(first_asset.asset_id)
                    .map_err(|_| Error::Invalid("spatial_catalog_authority_context"))?;
                catalog
                    .validate_with_context(context)
                    .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
                Ok(catalog)
            })
            .transpose()
    }
    fn current_transport_profile_hash(&mut self) -> Result<[u8; 32], Error> {
        let engine = self.engine()?;
        let baseline = town_transport::allocation_ref(engine)
            .ok_or(Error::Invalid("town_transport_profile_missing"))?;
        let Some(catalog_hash) = self.io.root.spatial_transport_catalog_sha256 else {
            if engine.spatial_permissions().is_some() {
                return Err(Error::Invalid("spatial_catalog_pointer_missing"));
            }
            return Ok(baseline);
        };
        let catalog =
            spatial_transport_updates::read_spatial_transport_catalog(&self.io.dir, catalog_hash)
                .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
        let view = engine.world().view();
        let market = view
            .town
            .ok_or(Error::Invalid("town_transport_market_missing"))?;
        let permissions = engine
            .spatial_permissions()
            .ok_or(Error::Invalid("spatial_permissions_missing"))?;
        let first_asset = permissions
            .assets()
            .first()
            .ok_or(Error::Invalid("spatial_permission_asset_missing"))?;
        let context = permissions
            .authorization_context(first_asset.asset_id)
            .map_err(|_| Error::Invalid("spatial_catalog_authority_context"))?;
        catalog
            .validate_with_context(context)
            .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
        if catalog.world_id != view.scope.world()
            || catalog.branch_id != view.scope.branch()
            || catalog.profile_id != market.profile.id
            || catalog.baseline_profile_sha256 != baseline
            || catalog.source_scene_sha256
                != parse_digest(
                    self.spatial_reference()?
                        .ok_or(Error::Invalid("spatial_reference_missing"))?
                        .geometry_sha256(),
                )?
            || catalog.assets().len() != permissions.assets.len()
            || catalog
                .assets()
                .iter()
                .zip(&permissions.assets)
                .any(|(a, b)| {
                    let update_count = catalog
                        .versions()
                        .iter()
                        .filter(|version| version.changed_asset_id == a.asset_id)
                        .count() as u64;
                    a.asset_id != b.asset_id
                        || a.kind != b.kind
                        || a.revision.checked_add(update_count) != Some(b.revision)
                })
        {
            return Err(Error::Invalid("spatial_catalog_binding"));
        }
        let active = catalog
            .versions()
            .last()
            .map_or(catalog.baseline_profile_sha256, |record| {
                record.profile_sha256
            });
        if permissions
            .assets
            .iter()
            .any(|asset| asset.current_profile_sha256 != active)
        {
            return Err(Error::Invalid("spatial_catalog_engine_binding"));
        }
        Ok(active)
    }
    fn validate_spatial_request_candidate(
        &mut self,
        request: mycity_core::spatial_transport_permissions::SpatialUpdateRequest,
    ) -> Result<(), Error> {
        use mycity_core::spatial_transport_permissions::SpatialAccessOperation;
        let current_hash = self.current_transport_profile_hash()?;
        let permissions = self
            .engine
            .spatial_permissions()
            .ok_or(Error::Invalid("spatial_permissions_missing"))?;
        let authorization_context = permissions
            .authorization_context(request.asset_id)
            .map_err(|_| Error::Invalid("spatial_request_authority_context"))?;
        let asset = permissions
            .assets()
            .iter()
            .find(|asset| asset.asset_id == request.asset_id)
            .ok_or(Error::Invalid("spatial_request_asset_missing"))?;
        let request_edge = asset
            .edges
            .iter()
            .find(|edge| edge.edge_id == request.edge_id && edge.surface == request.surface)
            .ok_or(Error::Invalid("spatial_request_edge_unmanaged"))?;
        let allowed_bit = match request.operation {
            SpatialAccessOperation::Close => {
                mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_CLOSE
            }
            SpatialAccessOperation::Restore => {
                mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_RESTORE
            }
        };
        if request.expected_revision != asset.revision
            || request.parent_profile_sha256 != current_hash
            || authorization_context.world_id != permissions.world_id
            || authorization_context.branch_id != permissions.branch_id
            || authorization_context.source_scene_sha256 != asset.source_scene_sha256
            || request.candidate_profile_sha256 == [0; 32]
            || (request_edge.currently_open != (request.operation == SpatialAccessOperation::Close))
            || request_edge.allowed_operations & allowed_bit == 0
        {
            return Err(Error::Invalid("spatial_request_stale_or_unauthorized"));
        }
        let view = self.engine.world().view();
        let market = view
            .town
            .ok_or(Error::Invalid("town_transport_market_missing"))?;
        let candidate = spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &self.io.dir,
            request.candidate_profile_sha256,
            view.scope.world(),
            view.scope.branch(),
            market.profile.id,
        )
        .map_err(|_| Error::Invalid("spatial_request_candidate_missing"))?;
        let current = spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &self.io.dir,
            current_hash,
            view.scope.world(),
            view.scope.branch(),
            market.profile.id,
        )
        .map_err(|_| Error::Invalid("spatial_request_parent_profile_missing"))?;
        let current_ref = current.as_ref();
        let encoded = spatial_transport_updates::preview_profile_asset(
            &current_ref,
            candidate.profile().opening.clone(),
        )
        .map_err(|_| Error::Invalid("spatial_request_candidate_invalid"))?;
        if encoded.sha256 != request.candidate_profile_sha256 {
            return Err(Error::Invalid("spatial_request_candidate_hash"));
        }
        let raw = town_transport::read_verified_allocation_bytes(
            &self.io.dir,
            &request.candidate_profile_sha256,
        )?;
        if raw != encoded.bytes {
            return Err(Error::Invalid("spatial_request_candidate_not_canonical"));
        }
        let mut changed = None;
        for old in &current.profile().opening.edges {
            let new = candidate
                .profile()
                .opening
                .edges
                .iter()
                .find(|edge| edge.id == old.id)
                .ok_or(Error::Invalid("spatial_request_candidate_topology"))?;
            if old.open != new.open && changed.replace((old.id, old.open, new.open)).is_some() {
                return Err(Error::Invalid("spatial_request_multiple_edge_changes"));
            }
        }
        let expected_open = request.operation == SpatialAccessOperation::Restore;
        if changed != Some((request.edge_id, request_edge.currently_open, expected_open)) {
            return Err(Error::Invalid("spatial_request_edge_change"));
        }
        town_transport::validate_allocation_project(&self.engine, candidate.profile())?;
        Ok(())
    }
    /// Validate and stage an immutable candidate profile for a later dual-approved Request.
    /// Staging alone does not change the active root pointer or grant authority; an abandoned
    /// candidate is an unreferenced immutable blob and may consume the bounded disk budget.
    pub fn stage_spatial_transport_candidate(
        &mut self,
        request: mycity_core::spatial_transport_permissions::SpatialUpdateRequest,
        proposed_opening: mycity_core::transport::Opening,
    ) -> Result<[u8; 32], Error> {
        if self.io.poisoned || self.io.owner_pid != std::process::id() {
            return Err(Error::RecoveryRequired);
        }
        let current_hash = self.current_transport_profile_hash()?;
        let view = self.engine.world().view();
        let market = view
            .town
            .ok_or(Error::Invalid("town_transport_market_missing"))?;
        let current = spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &self.io.dir,
            current_hash,
            view.scope.world(),
            view.scope.branch(),
            market.profile.id,
        )
        .map_err(|_| Error::Invalid("town_transport_active_profile_corrupt"))?;
        let permissions = self
            .engine
            .spatial_permissions()
            .ok_or(Error::Invalid("spatial_permissions_missing"))?;
        let authorization_context = permissions
            .authorization_context(request.asset_id)
            .map_err(|_| Error::Invalid("spatial_candidate_authority_context"))?;
        let asset = permissions
            .assets()
            .iter()
            .find(|asset| asset.asset_id == request.asset_id)
            .ok_or(Error::Invalid("spatial_candidate_asset_missing"))?;
        if request.expected_revision != asset.revision
            || request.parent_profile_sha256 != current_hash
            || authorization_context.world_id != permissions.world_id
            || authorization_context.branch_id != permissions.branch_id
            || authorization_context.source_scene_sha256 != asset.source_scene_sha256
            || request.candidate_profile_sha256 == [0; 32]
            || proposed_opening.revision
                != current
                    .profile()
                    .opening
                    .revision
                    .checked_add(1)
                    .ok_or(Error::Invalid("spatial_profile_revision_overflow"))?
        {
            return Err(Error::Invalid("spatial_candidate_request_binding"));
        }
        let managed = asset
            .edges
            .iter()
            .find(|edge| edge.edge_id == request.edge_id && edge.surface == request.surface)
            .ok_or(Error::Invalid("spatial_candidate_edge_unmanaged"))?;
        let required_bit = match request.operation {
            mycity_core::spatial_transport_permissions::SpatialAccessOperation::Close => {
                mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_CLOSE
            }
            mycity_core::spatial_transport_permissions::SpatialAccessOperation::Restore => {
                mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_RESTORE
            }
        };
        if !managed.currently_open
            && request.operation
                == mycity_core::spatial_transport_permissions::SpatialAccessOperation::Close
            || managed.currently_open
                && request.operation
                    == mycity_core::spatial_transport_permissions::SpatialAccessOperation::Restore
            || managed.allowed_operations & required_bit == 0
        {
            return Err(Error::Invalid("spatial_candidate_operation_not_allowed"));
        }
        let changed = current
            .profile()
            .opening
            .edges
            .iter()
            .filter_map(|old| {
                proposed_opening
                    .edges
                    .iter()
                    .find(|new| new.id == old.id)
                    .filter(|new| new.open != old.open)
                    .map(|new| (old.id, old.open, new.open))
            })
            .collect::<Vec<_>>();
        if changed.len() != 1
            || changed[0].0 != request.edge_id
            || changed[0].1 != managed.currently_open
            || changed[0].2
                != (request.operation
                    == mycity_core::spatial_transport_permissions::SpatialAccessOperation::Restore)
        {
            return Err(Error::Invalid("spatial_candidate_edge_change_mismatch"));
        }
        let candidate =
            spatial_transport_updates::preview_profile_asset(&current.as_ref(), proposed_opening)
                .map_err(|_| Error::Invalid("spatial_candidate_invalid"))?;
        if candidate.sha256 != request.candidate_profile_sha256 {
            return Err(Error::Invalid("spatial_candidate_hash_mismatch"));
        }
        town_transport::validate_allocation_project(&self.engine, current.profile())?;
        let candidate_profile: mycity_core::market_access::TransportAllocationProfile =
            serde_json::from_slice(&candidate.bytes)
                .map_err(|_| Error::Invalid("spatial_candidate_profile_decode"))?;
        town_transport::validate_allocation_project(&self.engine, &candidate_profile)?;
        spatial_transport_updates::stage_profile_asset(
            &self.io.dir,
            self.io.root.capacity,
            view.scope.world(),
            view.scope.branch(),
            market.profile.id,
            &candidate,
        )
        .map_err(|_| Error::Invalid("spatial_candidate_stage"))?;
        Ok(candidate.sha256)
    }
    pub fn metadata_workspace_identity(&self) -> Result<MetadataIdentity, Error> {
        self.engine()?;
        Ok(self.io.metadata.identity())
    }
    pub fn execute(&mut self, caller: Caller, envelope: Envelope) -> Result<Outcome, Error> {
        self.execute_with_observer(caller, envelope, |_| Ok(()))
    }
    pub fn execute_with_observer(
        &mut self,
        caller: Caller,
        envelope: Envelope,
        mut observer: impl FnMut(WritePoint) -> io::Result<()>,
    ) -> Result<Outcome, Error> {
        self.execute_observed_diagnosed(
            caller,
            envelope,
            &mut observer,
            &mut mycity_core::diagnostics::Trace::disabled(),
        )
    }
    pub fn execute_diagnosed(
        &mut self,
        caller: Caller,
        envelope: Envelope,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<Outcome, Error> {
        self.execute_observed_diagnosed(caller, envelope, |_| Ok(()), trace)
    }
    pub fn execute_observed_diagnosed(
        &mut self,
        caller: Caller,
        envelope: Envelope,
        mut observer: impl FnMut(WritePoint) -> io::Result<()>,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<Outcome, Error> {
        self.execute_observed_transport_diagnosed(caller, envelope, None, &mut observer, trace)
    }
    pub(crate) fn execute_observed_transport_diagnosed(
        &mut self,
        caller: Caller,
        envelope: Envelope,
        transport: Option<&mycity_core::market_access::SettlementDeliveryInput<'_>>,
        mut observer: impl FnMut(WritePoint) -> io::Result<()>,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<Outcome, Error> {
        if self.io.poisoned || self.io.owner_pid != std::process::id() {
            return Err(Error::RecoveryRequired);
        }
        if envelope.world != self.engine.world().view().scope.world()
            || envelope.branch != self.engine.world().view().scope.branch()
            || envelope.actions.len() > mycity_core::transaction::MAX_ACTIONS
            || envelope.actions.is_empty()
            || envelope.operation_id == 0
            || matches!(caller, Caller::Account(0))
        {
            return Ok(Outcome {
                model: self.engine.execute(caller, envelope),
                has_persistent_receipt: false,
                committed_sequence: self.io.root.sequence,
            });
        }
        let actor = match caller {
            Caller::System => 0,
            Caller::Account(id) => id,
        };
        let hash = digest(&json(&envelope)?);
        let operation = envelope.operation_id;
        let prior_hot = self
            .engine
            .journal()
            .iter()
            .find(|r| r.operation_id == operation)
            .copied();
        let replay = if let Some(record) = prior_hot {
            Some((record.actor, record.payload_sha256, record.outcome))
        } else {
            self.lookup_archived(operation)?
                .map(|record| (record.actor, record.payload_sha256, record.outcome))
        };
        if let Some((record_actor, payload_sha256, outcome)) = replay {
            trace.summary.replayed = true;
            let model = if record_actor != actor {
                Err(TxError::Unauthorized)
            } else if payload_sha256 != hash {
                Err(TxError::OperationConflict)
            } else {
                outcome
            };
            return Ok(Outcome {
                has_persistent_receipt: record_actor == actor && payload_sha256 == hash,
                model,
                committed_sequence: self.io.root.sequence,
            });
        }
        for action in &envelope.actions {
            if let mycity_core::transaction::Action::SpatialPermission {
                command:
                    mycity_core::spatial_transport_permissions::SpatialPermissionCommand::Request {
                        request,
                        ..
                    },
            } = action
            {
                self.validate_spatial_request_candidate(*request)?;
            }
        }
        let transport_action = envelope.actions.iter().any(|action| {
            matches!(
                action,
                mycity_core::transaction::Action::TownTransportSettle { .. }
            )
        });
        if transport_action != transport.is_some() {
            return Err(Error::Invalid("town_transport_requires_verified_proof"));
        }
        outbox::validate_attempt(
            &self.io.dir,
            self.io.root.source_registered,
            &envelope,
            self.engine.world().view().revision,
        )?;
        if outbox::exists(&self.io.dir)
            && !self.io.source_reconciled
            && envelope.actions.iter().any(|a| {
                a.is_new_commitment()
                    || matches!(
                        a,
                        mycity_core::transaction::Action::RegionAdvance { .. }
                            | mycity_core::transaction::Action::Timeline {
                                command: mycity_core::timeline::Command::Advance { .. }
                            }
                    )
            })
        {
            return Err(Error::SourceSyncRequired);
        }
        let candidate_present = match self.background_attachment(caller, &envelope, trace) {
            Ok(c) => c,
            Err(e) => {
                self.io.poisoned = true;
                return Err(e);
            }
        };
        let background_funded = self.background_funded_quote(
            caller,
            &envelope,
            if candidate_present {
                Some(self.air_candidate.state().map_err(Error::Model)?)
            } else {
                None
            },
            trace,
        );
        let continuation_plan =
            self.prepare_continuation(caller, &envelope, background_funded, trace)?;
        let background_plan = self.prepare_background(
            caller,
            &envelope,
            continuation_plan.as_ref(),
            background_funded,
            trace,
        )?;
        let control = envelope
            .actions
            .iter()
            .all(mycity_core::transaction::Action::is_responsibility_control);
        if !control && !background_plan.funded {
            capacity::admit(
                &self.io.dir,
                self.io.root.capacity,
                FRAME_LIMIT as u64 + 2 * META_LIMIT,
                false,
            )?;
        }
        if self.io.root.continuation.is_none()
            && continuation_plan.is_none()
            && self.engine.journal().len() >= MAX_RECEIPTS / 2
            && !self
                .engine
                .journal()
                .iter()
                .any(|r| r.operation_id == operation)
        {
            match self.checkpoint() {
                Ok(()) => {}
                Err(Error::Capacity { .. }) if control => {
                    if self.engine.journal().len() == MAX_RECEIPTS
                        && self
                            .engine
                            .can_advance_existing_responsibility_diagnosed(caller, &envelope, trace)
                    {
                        self.checkpoint_emergency()?;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        let candidate = if candidate_present {
            Some(self.air_candidate.state().map_err(Error::Model)?)
        } else {
            None
        };
        let io = &mut self.io;
        let result = self
            .engine
            .execute_recorded_transport_with_context_diagnosed(
                caller,
                envelope,
                candidate,
                transport,
                |env, record, commit_context| {
                    io.append(
                        env,
                        record,
                        commit_context,
                        continuation_plan.as_ref(),
                        &background_plan,
                        &mut observer,
                    )
                },
                trace,
            );
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                if !matches!(e, Error::Capacity { .. } | Error::FactBlocked(_)) {
                    self.io.poisoned = true;
                }
                return Err(e);
            }
        };
        // Keep fail-closed across an observer unwind after durable/model publication.
        self.io.poisoned = true;
        if let Err(e) = observer(WritePoint::Applied) {
            return Err(e.into());
        }
        self.io.poisoned = false;
        let has_persistent_receipt = self.engine.journal().iter().any(|r| {
            r.operation_id == operation
                && r.actor == actor
                && r.payload_sha256 == hash
                && r.outcome == result
        });
        Ok(Outcome {
            model: result,
            has_persistent_receipt,
            committed_sequence: self.io.root.sequence,
        })
    }
}

impl Persistence {
    fn append(
        &mut self,
        envelope: &Envelope,
        record: &JournalRecord,
        commit_context: CommitContext<'_>,
        continuation_plan: Option<&Continuation>,
        background_plan: &background::Plan,
        observer: &mut impl FnMut(WritePoint) -> io::Result<()>,
    ) -> Result<(), Error> {
        // A callback can unwind after CURRENT is durable but before Engine apply.
        // Arm first; ordinary classified pre-write failures may explicitly disarm.
        self.poisoned = true;
        let result = self.append_inner(
            envelope,
            record,
            commit_context,
            continuation_plan,
            background_plan,
            observer,
        );
        // Success stays armed until Engine apply/journal and Applied observer finish.
        if matches!(&result, Err(Error::Capacity { .. } | Error::FactBlocked(_))) {
            self.poisoned = false;
        }
        result
    }
    fn append_inner(
        &mut self,
        envelope: &Envelope,
        record: &JournalRecord,
        commit_context: CommitContext<'_>,
        continuation_plan: Option<&Continuation>,
        background_plan: &background::Plan,
        observer: &mut impl FnMut(WritePoint) -> io::Result<()>,
    ) -> Result<(), Error> {
        if envelope.actions.iter().any(|a| {
            matches!(
                a,
                mycity_core::transaction::Action::Inbox {
                    command: mycity_core::inbox::Command::External { .. }
                }
            )
        }) && matches!(record.outcome, Err(TxError::InboxFull))
        {
            return Err(Error::FactBlocked("world_fact_inbox_full"));
        }
        observer(WritePoint::Prepared)?;
        let sequence = self
            .root
            .sequence
            .checked_add(1)
            .ok_or(Error::Invalid("sequence_overflow"))?;
        let has_spatial_apply = envelope.actions.iter().any(|action| {
            matches!(
                action,
                mycity_core::transaction::Action::SpatialPermission {
                    command: mycity_core::spatial_transport_permissions::SpatialPermissionCommand::Apply { .. }
                }
            )
        });
        let has_spatial_action = envelope.actions.iter().any(|action| {
            matches!(
                action,
                mycity_core::transaction::Action::SpatialPermission { .. }
            )
        });
        if (record.outcome.is_ok()
            && has_spatial_apply != commit_context.spatial_update_grant.is_some())
            || (record.outcome.is_err() && commit_context.spatial_update_grant.is_some())
            || (record.outcome.is_ok()
                && has_spatial_action != commit_context.spatial_permissions.is_some())
            || (record.outcome.is_err() && commit_context.spatial_permissions.is_some())
        {
            return Err(Error::Invalid("spatial_commit_context_mismatch"));
        }
        if let Some(candidate_state) = commit_context.spatial_permissions {
            candidate_state
                .validate()
                .map_err(|_| Error::Invalid("spatial_candidate_state_invalid"))?;
            if candidate_state.world_id != envelope.world
                || candidate_state.branch_id != envelope.branch
            {
                return Err(Error::Invalid("spatial_candidate_state_scope"));
            }
            let current_state = commit_context
                .prior_spatial_permissions
                .ok_or(Error::Invalid("spatial_permissions_missing"))?;
            validate_spatial_candidate_transition(
                candidate_state,
                current_state,
                commit_context.spatial_update_grant,
            )?;
        }
        if let Some(grant) = commit_context.spatial_update_grant {
            let receipt = record
                .outcome
                .as_ref()
                .map_err(|_| Error::Invalid("spatial_apply_without_successful_receipt"))?;
            let binding = spatial_transport_updates::SpatialTransportCommitBinding {
                engine_operation_id: envelope.operation_id,
                receipt_operation_id: receipt.operation_id,
                receipt_revision: receipt.revision,
                store_sequence: sequence,
            };
            binding
                .validate()
                .map_err(|_| Error::Invalid("spatial_commit_binding"))?;
            if grant.world_id() != envelope.world
                || grant.branch_id() != envelope.branch
                || binding.receipt_operation_id != binding.engine_operation_id
            {
                return Err(Error::Invalid("spatial_commit_identity"));
            }
        }
        let mut next_spatial_catalog = self.root.spatial_transport_catalog_sha256;
        if record.outcome.is_ok() {
            if let Some(grant) = commit_context.spatial_update_grant {
                let receipt = record
                    .outcome
                    .as_ref()
                    .map_err(|_| Error::Invalid("spatial_apply_without_successful_receipt"))?;
                let candidate_state = commit_context
                    .spatial_permissions
                    .ok_or(Error::Invalid("spatial_apply_state_missing"))?;
                next_spatial_catalog = Some(self.prepare_spatial_apply_catalog(
                    envelope,
                    receipt,
                    grant,
                    candidate_state,
                    sequence,
                )?);
            }
        }
        let mut buffers = wal::Buffers::new();
        let payload = wal::encode(
            &mut self.canonical.get_mut().workspace,
            &mut buffers,
            sequence,
            &self.root.wal_chain,
            envelope,
            record,
        )?;
        let length = (payload.len() as u32).to_le_bytes();
        let checksum = wal::checksum(&length, payload);
        observer(WritePoint::WalEncoded)?;
        let wal_end = self
            .root
            .wal_end
            .checked_add(4 + payload.len() as u64 + 32)
            .ok_or(Error::Invalid("wal_overflow"))?;
        if wal_end > WAL_LIMIT {
            return Err(Error::Invalid("wal_limit"));
        }
        let next_continuation = self.next_continuation(continuation_plan, record)?;
        let next_background = self.next_background(background_plan, envelope, record)?;
        capacity::admit_with_reserve(
            &self.dir,
            self.root.capacity,
            4 + payload.len() as u64 + 32 + 2 * META_LIMIT,
            record.emergency_eligible
                || background_plan.funded && record.outcome.is_ok()
                || self.root.continuation.is_some()
                    && continuation_plan.is_some()
                    && record.outcome.is_ok(),
            next_continuation.as_ref().map_or(0, |c| c.remaining_bytes)
                + next_background.as_ref().map_or(0, |b| b.remaining_bytes()),
        )?;
        observer(WritePoint::WalAdmitted)?;
        self.wal.write_all(&length[..2])?;
        observer(WritePoint::WalHeaderHalf)?;
        self.wal.write_all(&length[2..])?;
        let half = payload.len() / 2;
        self.wal.write_all(&payload[..half])?;
        observer(WritePoint::WalPayloadHalf)?;
        self.wal.write_all(&payload[half..])?;
        self.wal.write_all(&checksum[..16])?;
        observer(WritePoint::WalChecksumHalf)?;
        self.wal.write_all(&checksum[16..])?;
        observer(WritePoint::WalWritten)?;
        self.wal.sync_all()?;
        observer(WritePoint::WalSynced)?;
        let root = Root {
            schema: self.root.schema.clone(),
            sequence,
            capacity: self.root.capacity,
            source_registered: self.root.source_registered,
            continuation: next_continuation,
            background_reserve: next_background,
            worker_cache_bytes: self.root.worker_cache_bytes,
            generation: self
                .root
                .generation
                .checked_add(1)
                .ok_or(Error::Invalid("publication_overflow"))?,
            checkpoint: self.root.checkpoint.clone(),
            checkpoint_layout: self.root.checkpoint_layout,
            checkpoint_v2: self.root.checkpoint_v2.clone(),
            spatial_transport_catalog_sha256: next_spatial_catalog,
            segment: self.root.segment,
            wal_end,
            wal_chain: hex(&checksum),
            genesis_sha256: self.root.genesis_sha256.clone(),
        };
        let slot = (root.generation % 2) as u8;
        self.metadata.small.encode(&root, META_LIMIT as usize)?;
        let root_bytes = self.metadata.small.as_slice();
        observer(WritePoint::RootEncoded)?;
        atomic_file(&self.dir, &format!("root-{slot}.json"), root_bytes, || {
            Ok(())
        })?;
        observer(WritePoint::RootPublished)?;
        let current = Current {
            schema: "mycity-m0-current-v1".into(),
            slot,
            root_sha256: sha256(root_bytes),
        };
        self.metadata.small.encode(&current, META_LIMIT as usize)?;
        let current_bytes = self.metadata.small.as_slice();
        observer(WritePoint::CurrentEncoded)?;
        atomic_file(&self.dir, "CURRENT.json", current_bytes, || {
            observer(WritePoint::CurrentRenamed)
        })?;
        observer(WritePoint::CurrentSynced)?;
        self.root = root;
        if let Some(hash) = background::successful_hash(envelope, record.outcome.is_ok()) {
            background::remember(&mut self.background_live, hash)?;
        }
        Ok(())
    }

    fn prepare_spatial_apply_catalog(
        &self,
        envelope: &Envelope,
        receipt: &Receipt,
        grant: mycity_core::spatial_transport_permissions::VerifiedSpatialUpdateGrant,
        candidate_state: &mycity_core::spatial_transport_permissions::SpatialPermissionState,
        sequence: u64,
    ) -> Result<[u8; 32], Error> {
        let parent_catalog_sha256 = self
            .root
            .spatial_transport_catalog_sha256
            .ok_or(Error::Invalid("spatial_catalog_pointer_missing"))?;
        let mut catalog = spatial_transport_updates::read_spatial_transport_catalog(
            &self.dir,
            parent_catalog_sha256,
        )
        .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
        let context = candidate_state
            .authorization_context(grant.asset_id())
            .map_err(|_| Error::Invalid("spatial_apply_authority_context"))?;
        catalog
            .validate_with_context(context)
            .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
        candidate_state
            .validate()
            .map_err(|_| Error::Invalid("spatial_apply_candidate_state_invalid"))?;
        let current_profile_sha256 = catalog
            .versions()
            .last()
            .map_or(catalog.baseline_profile_sha256, |version| {
                version.profile_sha256
            });
        if candidate_state.world_id != grant.world_id()
            || candidate_state.branch_id != grant.branch_id()
            || grant.world_id() != envelope.world
            || grant.branch_id() != envelope.branch
            || grant.request_id() == 0
            || grant.asset_id() == 0
            || grant.parent_profile_sha256() != current_profile_sha256
            || grant.candidate_profile_sha256() == [0; 32]
            || grant.source_scene_sha256() != catalog.source_scene_sha256
            || candidate_state
                .assets()
                .iter()
                .any(|asset| asset.current_profile_sha256 != grant.candidate_profile_sha256())
        {
            return Err(Error::Invalid("spatial_apply_catalog_binding"));
        }
        let mut has_matching_apply = false;
        for action in &envelope.actions {
            if let mycity_core::transaction::Action::SpatialPermission {
                command:
                    mycity_core::spatial_transport_permissions::SpatialPermissionCommand::Apply {
                        request_id,
                        ..
                    },
            } = action
            {
                if *request_id != grant.request_id() || has_matching_apply {
                    return Err(Error::Invalid("spatial_apply_request_mismatch"));
                }
                has_matching_apply = true;
            }
        }
        if !has_matching_apply {
            return Err(Error::Invalid("spatial_apply_action_missing"));
        }
        for baseline in catalog.assets() {
            let updates = catalog
                .versions()
                .iter()
                .filter(|version| version.changed_asset_id == baseline.asset_id)
                .count() as u64;
            let expected_revision = baseline
                .revision
                .checked_add(updates)
                .and_then(|revision| {
                    if baseline.asset_id == grant.asset_id() {
                        revision.checked_add(1)
                    } else {
                        Some(revision)
                    }
                })
                .ok_or(Error::Invalid("spatial_asset_revision_overflow"))?;
            let state = candidate_state
                .assets()
                .iter()
                .find(|asset| asset.asset_id == baseline.asset_id)
                .ok_or(Error::Invalid("spatial_apply_asset_missing"))?;
            if state.revision != expected_revision || state.kind != baseline.kind {
                return Err(Error::Invalid("spatial_apply_asset_revision"));
            }
        }
        let current_profile = spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &self.dir,
            current_profile_sha256,
            catalog.world_id,
            catalog.branch_id,
            catalog.profile_id,
        )
        .map_err(|_| Error::Invalid("spatial_current_profile_corrupt"))?;
        let baseline_profile = spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &self.dir,
            catalog.baseline_profile_sha256,
            catalog.world_id,
            catalog.branch_id,
            catalog.profile_id,
        )
        .map_err(|_| Error::Invalid("spatial_baseline_profile_corrupt"))?;
        let candidate_profile = spatial_transport_updates::VerifiedAllocationAsset::read_immutable(
            &self.dir,
            grant.candidate_profile_sha256(),
            catalog.world_id,
            catalog.branch_id,
            catalog.profile_id,
        )
        .map_err(|_| Error::Invalid("spatial_candidate_profile_corrupt"))?;
        let version_number = catalog
            .versions()
            .len()
            .checked_add(1)
            .and_then(|version| u64::try_from(version).ok())
            .ok_or(Error::Invalid("spatial_version_overflow"))?;
        let version = spatial_transport_updates::prepare_version(
            &baseline_profile.as_ref(),
            &current_profile.as_ref(),
            &grant,
            version_number,
            candidate_profile.profile().opening.clone(),
        )
        .map_err(|_| Error::Invalid("spatial_version_prepare"))?;
        let encoded_profile = spatial_transport_updates::encoded_profile_asset(&version.profile)
            .map_err(|_| Error::Invalid("spatial_candidate_encode"))?;
        if encoded_profile.sha256 != grant.candidate_profile_sha256() {
            return Err(Error::Invalid("spatial_candidate_raw_hash_mismatch"));
        }
        let raw_candidate = town_transport::read_verified_allocation_bytes(
            &self.dir,
            &grant.candidate_profile_sha256(),
        )?;
        if raw_candidate != encoded_profile.bytes {
            return Err(Error::Invalid("spatial_candidate_noncanonical_bytes"));
        }
        let commit = spatial_transport_updates::SpatialTransportCommitBinding {
            engine_operation_id: envelope.operation_id,
            receipt_operation_id: receipt.operation_id,
            receipt_revision: receipt.revision,
            store_sequence: sequence,
        };
        catalog
            .append_authorized(
                &baseline_profile.as_ref(),
                &current_profile.as_ref(),
                &version,
                &grant,
                commit,
                parent_catalog_sha256,
            )
            .map_err(|_| Error::Invalid("spatial_catalog_append"))?;
        catalog
            .validate_with_context(context)
            .map_err(|_| Error::Invalid("spatial_catalog_append_binding"))?;
        spatial_transport_updates::stage_profile_asset(
            &self.dir,
            self.root.capacity,
            catalog.world_id,
            catalog.branch_id,
            catalog.profile_id,
            &encoded_profile,
        )
        .map_err(|_| Error::Invalid("spatial_candidate_stage"))?;
        let encoded_catalog = spatial_transport_updates::encode_spatial_transport_catalog(&catalog)
            .map_err(|_| Error::Invalid("spatial_catalog_encode"))?;
        spatial_transport_updates::stage_spatial_transport_catalog(
            &self.dir,
            self.root.capacity,
            &encoded_catalog,
        )
        .map_err(|_| Error::Invalid("spatial_catalog_stage"))?;
        Ok(encoded_catalog.sha256)
    }
}

fn load_spatial_transport_catalog_on_open(
    dir: &Path,
    pointer: Option<[u8; 32]>,
    genesis_engine: &Engine,
    spatial_ref: Option<&SpatialReference>,
    baseline_hash: Option<[u8; 32]>,
    metadata: &mut MetadataWorkspace,
) -> Result<Option<spatial_transport_updates::SpatialTransportCatalog>, Error> {
    let permissions = genesis_engine.spatial_permissions();
    let (Some(pointer), Some(permissions), Some(spatial_ref), Some(baseline_hash)) =
        (pointer, permissions, spatial_ref, baseline_hash)
    else {
        if pointer.is_some() || permissions.is_some() {
            return Err(Error::Invalid("spatial_catalog_genesis_pair"));
        }
        return Ok(None);
    };

    let world = genesis_engine.world().view();
    let market = world
        .town
        .ok_or(Error::Invalid("spatial_catalog_market_missing"))?;
    let scene_hash = parse_digest(spatial_ref.geometry_sha256())?;
    let catalog = spatial_transport_updates::read_spatial_transport_catalog(dir, pointer)
        .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
    let first_asset = permissions
        .assets()
        .first()
        .ok_or(Error::Invalid("spatial_permission_asset_missing"))?;
    let context = permissions
        .authorization_context(first_asset.asset_id)
        .map_err(|_| Error::Invalid("spatial_catalog_authority_context"))?;
    catalog
        .validate_with_context(context)
        .map_err(|_| Error::Invalid("spatial_catalog_corrupt"))?;
    if catalog.world_id != world.scope.world()
        || catalog.branch_id != world.scope.branch()
        || catalog.profile_id != market.profile.id
        || catalog.source_scene_sha256 != scene_hash
        || catalog.baseline_profile_sha256 != baseline_hash
        || catalog.assets().len() != permissions.assets().len()
        || catalog
            .assets()
            .iter()
            .zip(permissions.assets())
            .any(|(baseline, asset)| {
                baseline.asset_id != asset.asset_id
                    || baseline.kind != asset.kind
                    || baseline.revision != asset.revision
                    || asset.revision != 1
                    || asset.current_profile_sha256 != baseline_hash
            })
    {
        return Err(Error::Invalid("spatial_catalog_genesis_binding"));
    }

    // Verify every immutable profile referenced by the catalog and the exact content-addressed
    // parent chain. The full catalog stays out of Root/WAL; missing ancestry is still corruption.
    let mut previous_profile = town_transport::read_allocation_for_settlement(
        dir,
        metadata,
        &baseline_hash,
        world.scope.world(),
        world.scope.branch(),
        market.profile.id,
    )?;
    let mut previous_profile_hash = baseline_hash;
    for version in catalog.versions() {
        if version.previous_profile_sha256 != previous_profile_hash {
            return Err(Error::Invalid("spatial_catalog_profile_chain"));
        }
        let profile = town_transport::read_allocation_for_settlement(
            dir,
            metadata,
            &version.profile_sha256,
            world.scope.world(),
            world.scope.branch(),
            market.profile.id,
        )?;
        let old_opening = &previous_profile.opening;
        let new_opening = &profile.opening;
        if old_opening.revision.checked_add(1) != Some(new_opening.revision)
            || old_opening.nodes != new_opening.nodes
            || old_opening.edges.len() != new_opening.edges.len()
            || old_opening.entrances != new_opening.entrances
            || old_opening.demands != new_opening.demands
            || previous_profile.schema != profile.schema
            || previous_profile.world_id != profile.world_id
            || previous_profile.branch_id != profile.branch_id
            || previous_profile.market_profile_id != profile.market_profile_id
            || previous_profile.period_seconds != profile.period_seconds
            || previous_profile.allocation_scope != profile.allocation_scope
            || previous_profile.demand_repeat_policy != profile.demand_repeat_policy
            || previous_profile.road_capacity_effect != profile.road_capacity_effect
            || previous_profile.partitions != profile.partitions
            || previous_profile.fee_policy != profile.fee_policy
            || new_opening
                .assign()
                .map_err(|_| Error::Invalid("spatial_catalog_profile_assignment"))?
                != profile.assignment
        {
            return Err(Error::Invalid("spatial_catalog_profile_delta"));
        }
        let mut changed_edge = None;
        for old in &old_opening.edges {
            let new = new_opening
                .edges
                .iter()
                .find(|edge| edge.id == old.id)
                .ok_or(Error::Invalid("spatial_catalog_profile_topology"))?;
            let mut expected = *old;
            expected.open = new.open;
            if expected != *new {
                return Err(Error::Invalid("spatial_catalog_profile_edge_change"));
            }
            if old.open != new.open && changed_edge.replace((old.id, old.open, new.open)).is_some()
            {
                return Err(Error::Invalid("spatial_catalog_multiple_edge_changes"));
            }
        }
        let (edge_id, was_open, is_open) =
            changed_edge.ok_or(Error::Invalid("spatial_catalog_profile_delta"))?;
        if edge_id != version.edge_id {
            return Err(Error::Invalid("spatial_catalog_profile_delta"));
        }
        let asset = permissions
            .assets()
            .iter()
            .find(|asset| asset.asset_id == version.changed_asset_id)
            .ok_or(Error::Invalid("spatial_catalog_asset_missing"))?;
        let managed_edge = asset
            .edges
            .iter()
            .find(|edge| edge.edge_id == version.edge_id && edge.surface == version.surface)
            .ok_or(Error::Invalid("spatial_catalog_edge_unmanaged"))?;
        let prior_edge_updates = catalog
            .versions()
            .iter()
            .take_while(|prior| prior.version < version.version)
            .filter(|prior| {
                prior.changed_asset_id == version.changed_asset_id
                    && prior.edge_id == version.edge_id
            })
            .count();
        let expected_was_open = managed_edge.initially_open ^ (prior_edge_updates % 2 != 0);
        let operation_bit = if was_open && !is_open {
            mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_CLOSE
        } else if !was_open && is_open {
            mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_RESTORE
        } else {
            return Err(Error::Invalid("spatial_catalog_profile_delta"));
        };
        if asset.kind != version.asset_kind
            || !managed_edge.initially_open
            || was_open != expected_was_open
            || managed_edge.allowed_operations & operation_bit == 0
        {
            return Err(Error::Invalid("spatial_catalog_edge_permission"));
        }
        previous_profile = profile;
        previous_profile_hash = version.profile_sha256;
    }

    let mut current = catalog.clone();
    while !current.versions().is_empty() {
        let parent_hash = current.parent_catalog_sha256;
        if parent_hash == [0; 32] {
            return Err(Error::Invalid("spatial_catalog_parent_missing"));
        }
        let parent = spatial_transport_updates::read_spatial_transport_catalog(dir, parent_hash)
            .map_err(|_| Error::Invalid("spatial_catalog_parent_corrupt"))?;
        let prefix_len = parent.versions().len();
        if prefix_len + 1 != current.versions().len()
            || parent.world_id != current.world_id
            || parent.branch_id != current.branch_id
            || parent.profile_id != current.profile_id
            || parent.source_scene_sha256 != current.source_scene_sha256
            || parent.baseline_profile_sha256 != current.baseline_profile_sha256
            || parent.assets() != current.assets()
            || parent.versions() != &current.versions()[..prefix_len]
        {
            return Err(Error::Invalid("spatial_catalog_parent_chain"));
        }
        current = parent;
    }
    if current.parent_catalog_sha256 != [0; 32] {
        return Err(Error::Invalid("spatial_catalog_genesis_parent"));
    }
    catalog
        .validate()
        .map_err(|_| Error::Invalid("spatial_catalog_invalid"))?;
    Ok(Some(catalog))
}

fn validate_spatial_transport_engine_state(
    engine: &Engine,
    catalog: Option<&spatial_transport_updates::SpatialTransportCatalog>,
    genesis: Option<&mycity_core::spatial_transport_permissions::SpatialPermissionState>,
) -> Result<(), Error> {
    let permissions = engine.spatial_permissions();
    let (Some(catalog), Some(permissions), Some(genesis)) = (catalog, permissions, genesis) else {
        if catalog.is_some() || permissions.is_some() || genesis.is_some() {
            return Err(Error::Invalid("spatial_catalog_engine_pair"));
        }
        return Ok(());
    };
    let replayed = replay_spatial_transport_permissions(genesis, catalog)?;
    if permissions.world_id != replayed.world_id
        || permissions.branch_id != replayed.branch_id
        || permissions.assets() != replayed.assets()
    {
        return Err(Error::Invalid("spatial_catalog_engine_binding"));
    }
    Ok(())
}

fn replay_spatial_transport_permissions(
    genesis: &mycity_core::spatial_transport_permissions::SpatialPermissionState,
    catalog: &spatial_transport_updates::SpatialTransportCatalog,
) -> Result<mycity_core::spatial_transport_permissions::SpatialPermissionState, Error> {
    let first_asset = genesis
        .assets()
        .first()
        .ok_or(Error::Invalid("spatial_permission_asset_missing"))?;
    let context = genesis
        .authorization_context(first_asset.asset_id)
        .map_err(|_| Error::Invalid("spatial_catalog_authority_context"))?;
    catalog
        .validate_with_context(context)
        .map_err(|_| Error::Invalid("spatial_catalog_invalid"))?;
    let mut replayed =
        mycity_core::spatial_transport_permissions::SpatialPermissionState::empty_backing()
            .map_err(|_| Error::Invalid("spatial_permission_replay_capacity"))?;
    genesis
        .copy_into(&mut replayed)
        .map_err(|_| Error::Invalid("spatial_permission_genesis_invalid"))?;
    for version in catalog.versions() {
        let witness = version.authorization_witness;
        let request = witness.request;
        if request.asset_id != version.changed_asset_id
            || request.surface != version.surface
            || request.edge_id != version.edge_id
            || request.expected_revision.checked_add(1) != Some(version.asset_revision)
            || version.asset_kind
                != catalog
                    .assets()
                    .iter()
                    .find(|asset| asset.asset_id == version.changed_asset_id)
                    .ok_or(Error::Invalid("spatial_catalog_asset_missing"))?
                    .kind
        {
            return Err(Error::Invalid("spatial_catalog_witness_binding"));
        }
        replayed
            .replay_authorized_update(
                witness.request,
                witness.owner_approval,
                witness.authority_approval,
                version.authorization_evidence_sha256,
            )
            .map_err(|_| Error::Invalid("spatial_catalog_authorization_replay"))?;
        if replayed
            .assets()
            .iter()
            .find(|asset| asset.asset_id == version.changed_asset_id)
            .is_none_or(|asset| asset.revision != version.asset_revision)
        {
            return Err(Error::Invalid("spatial_catalog_asset_revision"));
        }
    }
    Ok(replayed)
}

fn validate_spatial_candidate_transition(
    candidate: &mycity_core::spatial_transport_permissions::SpatialPermissionState,
    current: &mycity_core::spatial_transport_permissions::SpatialPermissionState,
    grant: Option<mycity_core::spatial_transport_permissions::VerifiedSpatialUpdateGrant>,
) -> Result<(), Error> {
    use mycity_core::spatial_transport_permissions::SpatialAccessOperation;
    candidate
        .validate()
        .map_err(|_| Error::Invalid("spatial_candidate_invalid"))?;
    current
        .validate()
        .map_err(|_| Error::Invalid("spatial_current_invalid"))?;
    if candidate.world_id != current.world_id
        || candidate.branch_id != current.branch_id
        || candidate.assets().len() != current.assets().len()
    {
        return Err(Error::Invalid("spatial_candidate_state_scope"));
    }
    if let Some(grant) = grant {
        let witness = grant.witness();
        let request = witness.request;
        let request_context = current
            .authorization_context(request.asset_id)
            .map_err(|_| Error::Invalid("spatial_candidate_request_context"))?;
        let request_digest =
            mycity_core::spatial_transport_permissions::request_sha256(request_context, &request)
                .map_err(|_| Error::Invalid("spatial_candidate_request"))?;
        let target = current
            .assets()
            .iter()
            .find(|asset| asset.asset_id == request.asset_id)
            .ok_or(Error::Invalid("spatial_candidate_asset_missing"))?;
        if grant.world_id() != current.world_id
            || grant.branch_id() != current.branch_id
            || request.request_id != grant.request_id()
            || request.asset_id != grant.asset_id()
            || request.operation != grant.operation()
            || request.surface != grant.surface()
            || request.edge_id != grant.edge_id()
            || request.expected_revision.checked_add(1) != Some(grant.new_revision())
            || request.expected_revision != target.revision
            || request.parent_profile_sha256 != grant.parent_profile_sha256()
            || request.candidate_profile_sha256 != grant.candidate_profile_sha256()
            || request_context.source_scene_sha256 != grant.source_scene_sha256()
            || mycity_core::spatial_transport_permissions::approval_evidence_sha256(
                request_digest,
                &witness.owner_approval,
                &witness.authority_approval,
            )
            .map_err(|_| Error::Invalid("spatial_candidate_evidence"))?
                != grant.evidence_sha256()
            || witness.owner_approval.request_id != request.request_id
            || witness.owner_approval.request_sha256 != request_digest
            || witness.owner_approval.approver_account != target.owner_account
            || witness.owner_approval.role
                != mycity_core::spatial_transport_permissions::SpatialApprovalRole::OwnerConsent
            || !witness.owner_approval.approved
            || witness.authority_approval.request_id != request.request_id
            || witness.authority_approval.request_sha256 != request_digest
            || witness.authority_approval.approver_account != target.permit_authority_account
            || witness.authority_approval.role
                != mycity_core::spatial_transport_permissions::SpatialApprovalRole::AuthorityPermit
            || !witness.authority_approval.approved
            || request.applicant_account == request.executor_account
            || witness.authority_approval.approver_account == request.applicant_account
            || witness.authority_approval.approver_account == request.executor_account
        {
            return Err(Error::Invalid("spatial_candidate_grant_binding"));
        }
        for (before, after) in current.assets().iter().zip(candidate.assets()) {
            if before.asset_id != after.asset_id
                || before.kind != after.kind
                || before.owner_account != after.owner_account
                || before.permit_authority_account != after.permit_authority_account
                || before.source_scene_sha256 != after.source_scene_sha256
                || after.current_profile_sha256 != grant.candidate_profile_sha256()
            {
                return Err(Error::Invalid("spatial_candidate_asset_identity"));
            }
            let is_target = before.asset_id == request.asset_id;
            if is_target && before.kind != grant.asset_kind() {
                return Err(Error::Invalid("spatial_candidate_asset_kind"));
            }
            if after.revision
                != if is_target {
                    before
                        .revision
                        .checked_add(1)
                        .ok_or(Error::Invalid("spatial_candidate_revision_overflow"))?
                } else {
                    before.revision
                }
                || before.edges.len() != after.edges.len()
            {
                return Err(Error::Invalid("spatial_candidate_asset_revision"));
            }
            let mut matched_edge = false;
            for (old_edge, new_edge) in before.edges.iter().zip(&after.edges) {
                if old_edge.edge_id != new_edge.edge_id || old_edge.surface != new_edge.surface {
                    return Err(Error::Invalid("spatial_candidate_edge_identity"));
                }
                let changed = is_target
                    && old_edge.edge_id == request.edge_id
                    && old_edge.surface == request.surface;
                if changed {
                    let operation_bit = match request.operation {
                        SpatialAccessOperation::Close => {
                            mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_CLOSE
                        }
                        SpatialAccessOperation::Restore => {
                            mycity_core::spatial_transport_permissions::SPATIAL_ALLOW_RESTORE
                        }
                    };
                    if matched_edge
                        || (request.operation == SpatialAccessOperation::Close
                            && (!old_edge.currently_open || new_edge.currently_open))
                        || (request.operation == SpatialAccessOperation::Restore
                            && (old_edge.currently_open || !new_edge.currently_open))
                        || !old_edge.initially_open
                        || old_edge.allowed_operations & operation_bit == 0
                    {
                        return Err(Error::Invalid("spatial_candidate_edge_transition"));
                    }
                    matched_edge = true;
                } else if old_edge != new_edge {
                    return Err(Error::Invalid("spatial_candidate_unrelated_edge_changed"));
                }
            }
            if is_target != matched_edge {
                return Err(Error::Invalid("spatial_candidate_edge_missing"));
            }
        }
    } else if current.assets() != candidate.assets() {
        return Err(Error::Invalid(
            "spatial_candidate_asset_changed_without_apply",
        ));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}
fn hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
    out
}

fn identity(engine: &Engine) -> Bootstrap {
    let view = engine.world().view();
    Bootstrap {
        schema: SCHEMA.into(),
        world: view.scope.world(),
        branch: view.scope.branch(),
        seed: view.seed,
        laboratory_only: true,
    }
}
