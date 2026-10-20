//! Laboratory persistence: one writer, bounded replay, WAL + explicit committed boundary.
//! No historical migration, power-loss certification or automatic fallback of confirmed data.
mod background;
pub use background::{
    BackgroundPoint, BackgroundReserve, BACKGROUND_ASSET_RESERVE, BACKGROUND_LIVE_ASSETS,
    BACKGROUND_SLOTS,
};
mod capacity;
mod continuation;
pub use continuation::{Continuation, CONTINUATION_MAX_PAGES, CONTINUATION_PAGE_BYTES};
mod outbox;
mod query;
pub use outbox::{SourcePoint, SourceStatus};
pub use query::{Page, Query};
mod checkpoint;
mod checkpoint_v2;
pub use capacity::{CapacityPolicy, CapacityStatus};
mod spatial;
pub use checkpoint::{CheckpointPoint, CheckpointStatus};
use fs2::FileExt;
use mycity_core::{
    assembly::assemble,
    read_bounded, sha256,
    transaction::{
        Caller, Engine, Envelope, JournalRecord, Receipt, Resource, TxError, MAX_RECEIPTS,
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
    Prepared,
    WalHeaderHalf,
    WalPayloadHalf,
    WalChecksumHalf,
    WalWritten,
    WalSynced,
    RootPublished,
    CurrentRenamed,
    CurrentSynced,
    Applied,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Genesis {
    schema: String,
    rules_identity: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    checkpoint_layout: u8,
    opening: Vec<u8>,
    resources: Vec<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spatial: Option<SpatialReference>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Root {
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
    segment: u64,
    wal_end: u64,
    wal_chain: String,
    genesis_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    schema: String,
    slot: u8,
    root_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WalEntry {
    sequence: u64,
    previous: String,
    envelope: Envelope,
    record: serde_json::Value,
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
    poisoned: bool,
    source_reconciled: bool,
    background_live: Vec<[u8; 32]>,
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
    spatial: Option<SpatialAssets>,
    spatial_ref: Option<SpatialReference>,
    io: Persistence,
}

fn json<T: Serialize>(v: &T) -> Result<Vec<u8>, Error> {
    Ok(serde_json::to_vec(v)?)
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
        mut observer: impl FnMut(CreatePoint) -> io::Result<()>,
    ) -> Result<Self, Error> {
        if opening.len() as u64 > INPUT_LIMIT
            || rules_identity.is_empty()
            || rules_identity.len() > 128
        {
            return Err(Error::Invalid("genesis_input_limit"));
        }
        let engine = Engine::new(assemble(opening).map_err(Error::Model)?, resources.clone())
            .map_err(|e| Error::Model(format!("{e:?}")))?;
        let identity = identity(&engine);
        let (spatial_ref, spatial) = if let Some((input, png)) = bundle {
            let (reference, assets) = SpatialReference::prepare(input, png, &identity)?;
            (Some(reference), Some(assets))
        } else {
            (None, None)
        };
        spatial::validate_exchange_binding(&engine, spatial.as_ref())?;
        observer(CreatePoint::Validated)?;
        fs::create_dir(dir)?;
        let lock = lock(dir)?;
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
        let genesis = json(&Genesis {
            schema: if spatial_ref.is_some() {
                "mycity-m0-genesis-v2"
            } else {
                "mycity-m0-genesis-v1"
            }
            .into(),
            rules_identity: rules_identity.into(),
            checkpoint_layout,
            opening: opening.to_vec(),
            resources,
            spatial: spatial_ref.clone(),
        })?;
        if genesis.len() as u64 > GENESIS_LIMIT {
            return Err(Error::Invalid("genesis_encoded_limit"));
        }
        atomic_file(dir, "genesis.json", &genesis, || Ok(()))?;
        observer(CreatePoint::GenesisSynced)?;
        let wal = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.join("wal.bin"))?;
        wal.sync_all()?;
        observer(CreatePoint::WalSynced)?;
        let genesis_sha256 = sha256(&genesis);
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
            segment: 0,
            wal_end: 0,
            wal_chain: genesis_sha256.clone(),
            genesis_sha256,
        };
        let root_bytes = json(&root)?;
        atomic_file(dir, "root-0.json", &root_bytes, || Ok(()))?;
        observer(CreatePoint::RootSynced)?;
        atomic_file(
            dir,
            "CURRENT.json",
            &json(&Current {
                schema: "mycity-m0-current-v1".into(),
                slot: 0,
                root_sha256: sha256(&root_bytes),
            })?,
            || observer(CreatePoint::CurrentRenamed),
        )?;
        observer(CreatePoint::CurrentSynced)?;
        sync_directory(dir.parent().unwrap_or(Path::new(".")))?;
        observer(CreatePoint::DirectorySynced)?;
        observer(CreatePoint::Ready)?;
        Ok(Self {
            engine,
            spatial,
            spatial_ref,
            io: Persistence {
                owner_pid: std::process::id(),
                dir: dir.to_owned(),
                _lock: lock,
                wal,
                root,
                poisoned: false,
                source_reconciled: !outbox::exists(dir),
                background_live: Vec::with_capacity(BACKGROUND_LIVE_ASSETS),
            },
        })
    }

    pub fn open(dir: &Path, rules_identity: &str) -> Result<Self, Error> {
        if fs::symlink_metadata(dir)?.file_type().is_symlink() {
            return Err(Error::Invalid("symlink_world"));
        }
        let lock = lock(dir)?;
        let current: Current =
            serde_json::from_slice(&read_file(&dir.join("CURRENT.json"), META_LIMIT)?)?;
        if current.schema != "mycity-m0-current-v1" || current.slot > 1 {
            return Err(Error::Invalid("current_schema_or_slot"));
        }
        let root_bytes = read_file(&dir.join(format!("root-{}.json", current.slot)), META_LIMIT)?;
        if sha256(&root_bytes) != current.root_sha256 {
            return Err(Error::Invalid("committed_root_corrupt"));
        }
        let root: Root = serde_json::from_slice(&root_bytes)?;
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
        let genesis_bytes = read_file(&dir.join("genesis.json"), GENESIS_LIMIT)?;
        if sha256(&genesis_bytes) != root.genesis_sha256 {
            return Err(Error::Invalid("genesis_corrupt"));
        }
        let genesis: Genesis = serde_json::from_slice(&genesis_bytes)?;
        drop(genesis_bytes);
        let valid_schema = matches!(
            (genesis.schema.as_str(), genesis.spatial.is_some()),
            ("mycity-m0-genesis-v1", false) | ("mycity-m0-genesis-v2", true)
        );
        if !valid_schema || genesis.rules_identity != rules_identity {
            return Err(Error::Invalid("unsupported_rules_or_schema"));
        }
        // Fresh reconstruction of the recorded genesis, not a gift to an existing world.
        let genesis_engine = Engine::new(
            assemble(&genesis.opening).map_err(Error::Model)?,
            genesis.resources,
        )
        .map_err(|e| Error::Model(format!("{e:?}")))?;
        if root.checkpoint_layout != genesis.checkpoint_layout
            || !matches!(root.checkpoint_layout, 0 | 2)
            || root.checkpoint_layout == 0 && root.checkpoint_v2.is_some()
            || root.checkpoint_layout == 2 && root.checkpoint.is_some()
        {
            return Err(Error::Invalid("checkpoint_layout_binding"));
        }
        let (mut engine, mut sequence, mut chain) = if let Some(reference) = &root.checkpoint_v2 {
            checkpoint_v2::open(dir, reference, &root.genesis_sha256, root.sequence)?
        } else if let Some(reference) = &root.checkpoint {
            let cp = checkpoint::load(dir, reference)?;
            if cp.sequence > root.sequence || cp.genesis_sha256 != root.genesis_sha256 {
                return Err(Error::Invalid("checkpoint_boundary"));
            }
            checkpoint::verify_archive(dir, reference, &root.genesis_sha256)?;
            (
                Engine::restore_checkpoint(&cp.engine).map_err(Error::Model)?,
                cp.sequence,
                cp.chain,
            )
        } else {
            (genesis_engine, 0, root.genesis_sha256.clone())
        };
        outbox::verify(dir, engine.world().view().scope)?;
        let spatial_ref = genesis.spatial;
        let spatial = spatial_ref
            .as_ref()
            .map(|r| r.load(dir, &identity(&engine)))
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
            let mut payload = vec![0; length];
            wal.read_exact(&mut payload)?;
            let mut checksum = [0u8; 32];
            wal.read_exact(&mut checksum)?;
            let mut checked = Vec::with_capacity(length + 4);
            checked.extend_from_slice(&len);
            checked.extend_from_slice(&payload);
            let actual = digest(&checked);
            if actual != checksum {
                return Err(Error::Invalid("committed_frame_checksum"));
            }
            let entry: WalEntry = serde_json::from_slice(&payload)?;
            sequence += 1;
            if entry.sequence != sequence || entry.previous != chain {
                return Err(Error::Invalid("committed_frame_chain"));
            }
            let actor = entry.record["actor"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or(Error::Invalid("record_actor"))?;
            let caller = if actor == 0 {
                Caller::System
            } else {
                Caller::Account(actor)
            };
            let record: JournalRecord = serde_json::from_value(entry.record.clone())?;
            let candidate_hash =
                background::successful_hash(&entry.envelope, record.outcome.is_ok());
            let candidate = candidate_hash
                .as_ref()
                .map(|h| background::load_asset(dir, h))
                .transpose()?;
            if let Some(hash) = candidate_hash {
                background::remember(&mut background_live, hash)?;
            }
            let mut called = false;
            let _ = engine.execute_recorded_with_background(
                caller,
                entry.envelope,
                candidate.as_ref(),
                |_, record| -> Result<(), Error> {
                    called = true;
                    if serde_json::to_value(record)? != entry.record {
                        return Err(Error::Invalid("replay_outcome_mismatch"));
                    }
                    Ok(())
                },
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
        Self::validate_continuation(&engine, &root)?;
        Self::validate_background(&engine, dir, &root)?;
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
            spatial,
            spatial_ref,
            io: Persistence {
                owner_pid: std::process::id(),
                dir: dir.to_owned(),
                _lock: lock,
                wal,
                root,
                poisoned: false,
                source_reconciled: !outbox::exists(dir),
                background_live,
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
    pub fn spatial(&self) -> Result<Option<&SpatialAssets>, Error> {
        self.engine()?;
        Ok(self.spatial.as_ref())
    }
    pub fn spatial_reference(&self) -> Result<Option<&SpatialReference>, Error> {
        self.engine()?;
        Ok(self.spatial_ref.as_ref())
    }
    /// Joint identity of financial/runtime state and immutable CPU spatial dependencies.
    pub fn state_hash(&self) -> Result<String, Error> {
        Ok(sha256(&json(
            &serde_json::json!({"schema":"mycity-m0-store-state-v1","engine":self.engine()?.state_hash(),"spatial":self.spatial_reference()?}),
        )?))
    }
    /// Owned Rust capacities only; excludes allocator/kernel/OS pages and transient replay.
    pub fn owned_capacity_bytes(&self) -> Result<usize, Error> {
        Ok(std::mem::size_of::<Self>() - std::mem::size_of::<Engine>()
            + self.engine()?.owned_capacity_bytes()
            + self.io.dir.capacity()
            + self.io.background_live.capacity() * std::mem::size_of::<[u8; 32]>()
            + self.io.root.schema.capacity()
            + self.io.root.wal_chain.capacity()
            + self.io.root.genesis_sha256.capacity()
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
    pub fn execute(&mut self, caller: Caller, envelope: Envelope) -> Result<Outcome, Error> {
        self.execute_with_observer(caller, envelope, |_| Ok(()))
    }
    pub fn execute_with_observer(
        &mut self,
        caller: Caller,
        envelope: Envelope,
        mut observer: impl FnMut(WritePoint) -> io::Result<()>,
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
        let prior = self
            .engine
            .journal()
            .iter()
            .find(|r| r.operation_id == operation)
            .copied();
        if let Some(record) = match prior {
            Some(r) => Some(r),
            None => self.lookup_archived(operation)?,
        } {
            let model = if record.actor != actor {
                Err(TxError::Unauthorized)
            } else if record.payload_sha256 != hash {
                Err(TxError::OperationConflict)
            } else {
                record.outcome
            };
            return Ok(Outcome {
                has_persistent_receipt: record.actor == actor && record.payload_sha256 == hash,
                model,
                committed_sequence: self.io.root.sequence,
            });
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
        let candidate = match self.background_attachment(caller, &envelope) {
            Ok(c) => c,
            Err(e) => {
                self.io.poisoned = true;
                return Err(e);
            }
        };
        let background_funded = self.background_funded_quote(caller, &envelope, candidate.as_ref());
        let continuation_plan = self.prepare_continuation(caller, &envelope, background_funded)?;
        let background_plan = self.prepare_background(
            caller,
            &envelope,
            continuation_plan.as_ref(),
            background_funded,
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
                            .can_advance_existing_responsibility(caller, &envelope)
                    {
                        self.checkpoint_emergency()?;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        let io = &mut self.io;
        let result = self.engine.execute_recorded_with_background(
            caller,
            envelope,
            candidate.as_ref(),
            |env, record| {
                io.append(
                    env,
                    record,
                    continuation_plan.as_ref(),
                    &background_plan,
                    &mut observer,
                )
            },
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
        if let Err(e) = observer(WritePoint::Applied) {
            self.io.poisoned = true;
            return Err(e.into());
        }
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
        let payload = json(&WalEntry {
            sequence,
            previous: self.root.wal_chain.clone(),
            envelope: envelope.clone(),
            record: serde_json::to_value(record)?,
        })?;
        if payload.len() > FRAME_LIMIT {
            return Err(Error::Invalid("frame_limit"));
        }
        let length = (payload.len() as u32).to_le_bytes();
        let mut check = Vec::with_capacity(payload.len() + 4);
        check.extend_from_slice(&length);
        check.extend_from_slice(&payload);
        let checksum = digest(&check);
        let wal_end = self
            .root
            .wal_end
            .checked_add(check.len() as u64 + 32)
            .ok_or(Error::Invalid("wal_overflow"))?;
        if wal_end > WAL_LIMIT {
            return Err(Error::Invalid("wal_limit"));
        }
        let next_continuation = self.next_continuation(continuation_plan, record)?;
        let next_background = self.next_background(background_plan, envelope, record)?;
        capacity::admit_with_reserve(
            &self.dir,
            self.root.capacity,
            check.len() as u64 + 32 + 2 * META_LIMIT,
            record.emergency_eligible
                || background_plan.funded && record.outcome.is_ok()
                || self.root.continuation.is_some()
                    && continuation_plan.is_some()
                    && record.outcome.is_ok(),
            next_continuation.as_ref().map_or(0, |c| c.remaining_bytes)
                + next_background.as_ref().map_or(0, |b| b.remaining_bytes()),
        )?;
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
            segment: self.root.segment,
            wal_end,
            wal_chain: hex(&checksum),
            genesis_sha256: self.root.genesis_sha256.clone(),
        };
        let slot = (root.generation % 2) as u8;
        let root_bytes = json(&root)?;
        atomic_file(&self.dir, &format!("root-{slot}.json"), &root_bytes, || {
            Ok(())
        })?;
        observer(WritePoint::RootPublished)?;
        let current = Current {
            schema: "mycity-m0-current-v1".into(),
            slot,
            root_sha256: sha256(&root_bytes),
        };
        atomic_file(&self.dir, "CURRENT.json", &json(&current)?, || {
            observer(WritePoint::CurrentRenamed)
        })?;
        observer(WritePoint::CurrentSynced)?;
        self.root = root;
        if let Some(hash) = background::successful_hash(envelope, record.outcome.is_ok()) {
            background::remember(&mut self.background_live, hash)?;
        }
        Ok(())
    }
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
