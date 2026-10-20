//! Complete state checkpoints, immutable receipt chain and bounded hot WAL.
use super::*;
pub const CHECKPOINT_LIMIT: u64 = 16 * 1024 * 1024;
pub const ARCHIVE_LIMIT: u64 = 128 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
    generation: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    schema: String,
    pub sequence: u64,
    pub chain: String,
    pub genesis_sha256: String,
    pub engine: Vec<u8>,
    receipts: Vec<JournalRecord>,
    previous: Option<Reference>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    data_sha256: String,
    operations: Vec<u64>,
    previous: Option<Reference>,
}
fn index_name(generation: u64) -> String {
    format!("checkpoint-index-{generation}.json")
}
fn load_index(dir: &Path, r: &Reference) -> Result<Index, Error> {
    let bytes = read_file(&dir.join(index_name(r.generation)), 32 * 1024)?;
    if sha256(&bytes) != r.sha256 {
        return Err(Error::Invalid("checkpoint_index_checksum"));
    }
    let index: Index = serde_json::from_slice(&bytes)?;
    if index.operations.len() > MAX_RECEIPTS
        || index
            .previous
            .as_ref()
            .is_some_and(|p| p.generation >= r.generation)
    {
        return Err(Error::Invalid("checkpoint_index_limits"));
    }
    Ok(index)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointPoint {
    SnapshotSynced,
    SegmentSynced,
    RootPublished,
    CurrentRenamed,
    CurrentSynced,
    Retired,
}
#[derive(Debug, Serialize)]
pub struct CheckpointStatus {
    pub sequence: u64,
    pub generation: u64,
    pub hot_receipts: usize,
    pub wal_bytes: u64,
    pub archive_bytes: u64,
    pub archive_limit: u64,
}
pub(super) fn wal_name(segment: u64) -> String {
    if segment == 0 {
        "wal.bin".into()
    } else {
        format!("wal-{segment}.bin")
    }
}
fn cp_name(generation: u64) -> String {
    format!("checkpoint-{generation}.json")
}
pub(super) fn load(dir: &Path, r: &Reference) -> Result<Checkpoint, Error> {
    let index = load_index(dir, r)?;
    let bytes = read_file(&dir.join(cp_name(r.generation)), CHECKPOINT_LIMIT)?;
    if sha256(&bytes) != index.data_sha256 {
        return Err(Error::Invalid("checkpoint_checksum"));
    }
    let cp: Checkpoint = serde_json::from_slice(&bytes)?;
    if cp.schema != "mycity-m0-checkpoint-v1" || cp.receipts.len() > MAX_RECEIPTS {
        return Err(Error::Invalid("checkpoint_schema_or_receipts"));
    }
    if cp.previous != index.previous
        || cp
            .receipts
            .iter()
            .map(|r| r.operation_id)
            .collect::<Vec<_>>()
            != index.operations
    {
        return Err(Error::Invalid("checkpoint_index_content"));
    }
    if let Some(previous) = &cp.previous {
        if previous.generation >= r.generation {
            return Err(Error::Invalid("checkpoint_archive_cycle"));
        }
    }
    Ok(cp)
}
pub(super) fn verify_archive(dir: &Path, r: &Reference, genesis: &str) -> Result<(), Error> {
    let mut cursor = Some(r.clone());
    let mut ceiling = u64::MAX;
    while let Some(r) = cursor {
        let cp = load(dir, &r)?;
        if cp.genesis_sha256 != genesis || cp.sequence > ceiling {
            return Err(Error::Invalid("receipt_archive_chain"));
        }
        ceiling = cp.sequence;
        cursor = cp.previous;
    }
    Ok(())
}
fn archive_bytes(dir: &Path) -> Result<u64, Error> {
    let mut total = 0u64;
    for e in fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_file() {
            total = total
                .checked_add(e.metadata()?.len())
                .ok_or(Error::Invalid("archive_size_overflow"))?;
        }
    }
    Ok(total)
}
impl Store {
    /// Read an original hot or checkpointed receipt without re-executing it.
    /// Host-local diagnostic API; this does not grant mutation authority.
    pub fn receipt(&mut self, operation: u64) -> Result<Option<JournalRecord>, Error> {
        self.engine()?;
        if let Some(record) = self
            .engine
            .journal()
            .iter()
            .find(|r| r.operation_id == operation)
        {
            return Ok(Some(*record));
        }
        self.lookup_archived(operation)
    }
    pub(super) fn lookup_archived(
        &mut self,
        operation: u64,
    ) -> Result<Option<JournalRecord>, Error> {
        let result = (|| {
            let mut cursor = self.io.root.checkpoint.clone();
            while let Some(r) = cursor {
                let index = load_index(&self.io.dir, &r)?;
                if index.operations.contains(&operation) {
                    let cp = load(&self.io.dir, &r)?;
                    return cp
                        .receipts
                        .iter()
                        .find(|r| r.operation_id == operation)
                        .copied()
                        .map(Some)
                        .ok_or(Error::Invalid("checkpoint_missing_receipt"));
                }
                cursor = index.previous;
            }
            Ok(None)
        })();
        if result.is_err() {
            self.io.poisoned = true;
        }
        result
    }
    pub(super) fn receipt_rows(
        &mut self,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<serde_json::Value>, usize), Error> {
        let result = (|| {
            let total = usize::try_from(self.io.root.sequence)
                .map_err(|_| Error::Invalid("query_sequence_range"))?;
            if offset > total {
                return Err(Error::Invalid("query_offset"));
            }
            let mut rows = Vec::with_capacity(limit);
            let mut skipped = offset;
            for (i, record) in self.engine.journal().iter().enumerate().rev() {
                if skipped > 0 {
                    skipped -= 1;
                    continue;
                }
                if rows.len() == limit {
                    break;
                }
                rows.push(serde_json::json!({"committed_sequence":self.io.root.sequence-self.engine.journal().len()as u64+i as u64+1,"record":record}));
            }
            let mut cursor = self.io.root.checkpoint.clone();
            while rows.len() < limit {
                let Some(r) = cursor else {
                    break;
                };
                let index = load_index(&self.io.dir, &r)?;
                if skipped >= index.operations.len() {
                    skipped -= index.operations.len();
                    cursor = index.previous;
                    continue;
                }
                let cp = load(&self.io.dir, &r)?;
                for (i, record) in cp.receipts.iter().enumerate().rev() {
                    if skipped > 0 {
                        skipped -= 1;
                        continue;
                    }
                    if rows.len() == limit {
                        break;
                    }
                    rows.push(serde_json::json!({"committed_sequence":cp.sequence-cp.receipts.len()as u64+i as u64+1,"record":record}));
                }
                cursor = cp.previous;
            }
            Ok((rows, total))
        })();
        if result
            .as_ref()
            .is_err_and(|e| !matches!(e, Error::Invalid("query_offset")))
        {
            self.io.poisoned = true;
        }
        result
    }
    pub fn checkpoint_status(&self) -> Result<CheckpointStatus, Error> {
        self.engine()?;
        Ok(CheckpointStatus {
            sequence: self.io.root.sequence,
            generation: self.io.root.generation,
            hot_receipts: self.engine.journal().len(),
            wal_bytes: self.io.root.wal_end,
            archive_bytes: archive_bytes(&self.io.dir)?,
            archive_limit: ARCHIVE_LIMIT,
        })
    }
    pub fn checkpoint(&mut self) -> Result<(), Error> {
        self.checkpoint_internal(false, |_| Ok(()))
    }
    pub fn checkpoint_with_observer(
        &mut self,
        observer: impl FnMut(CheckpointPoint) -> io::Result<()>,
    ) -> Result<(), Error> {
        self.checkpoint_internal(false, observer)
    }
    pub(super) fn checkpoint_emergency(&mut self) -> Result<(), Error> {
        self.checkpoint_internal(true, |_| Ok(()))
    }
    fn checkpoint_internal(
        &mut self,
        emergency: bool,
        mut observer: impl FnMut(CheckpointPoint) -> io::Result<()>,
    ) -> Result<(), Error> {
        self.engine()?;
        let result = (|| {
            let generation = self
                .io
                .root
                .generation
                .checked_add(1)
                .ok_or(Error::Invalid("publication_overflow"))?;
            let cp = Checkpoint {
                schema: "mycity-m0-checkpoint-v1".into(),
                sequence: self.io.root.sequence,
                chain: self.io.root.wal_chain.clone(),
                genesis_sha256: self.io.root.genesis_sha256.clone(),
                engine: self.engine.export_checkpoint().map_err(Error::Model)?,
                receipts: self.engine.journal().to_vec(),
                previous: self.io.root.checkpoint.clone(),
            };
            let bytes = json(&cp)?;
            capacity::admit(
                &self.io.dir,
                self.io.root.capacity,
                bytes.len() as u64 + 32 * 1024 + 2 * META_LIMIT,
                emergency,
            )?;
            if bytes.len() as u64 > CHECKPOINT_LIMIT
                || archive_bytes(&self.io.dir)?
                    .checked_add(bytes.len() as u64)
                    .ok_or(Error::Invalid("archive_size_overflow"))?
                    > ARCHIVE_LIMIT
            {
                return Err(Error::Invalid("archive_capacity"));
            }
            // An orphan with this generation may exist after an interrupted attempt. It is
            // never referenced by CURRENT; replace atomically, never truncate a live dependency.
            atomic_file(&self.io.dir, &cp_name(generation), &bytes, || Ok(()))?;
            let index = json(&Index {
                data_sha256: sha256(&bytes),
                operations: cp.receipts.iter().map(|r| r.operation_id).collect(),
                previous: cp.previous.clone(),
            })?;
            atomic_file(&self.io.dir, &index_name(generation), &index, || Ok(()))?;
            observer(CheckpointPoint::SnapshotSynced)?;
            let new_name = wal_name(generation);
            atomic_file(&self.io.dir, &new_name, &[], || Ok(()))?;
            let new_wal = OpenOptions::new()
                .read(true)
                .write(true)
                .open(self.io.dir.join(&new_name))?;
            observer(CheckpointPoint::SegmentSynced)?;
            let old_segment = self.io.root.segment;
            let mut root = self.io.root.clone();
            root.generation = generation;
            root.segment = generation;
            root.wal_end = 0;
            root.checkpoint = Some(Reference {
                generation,
                sha256: sha256(&index),
            });
            let root_bytes = json(&root)?;
            let slot = (generation % 2) as u8;
            atomic_file(
                &self.io.dir,
                &format!("root-{slot}.json"),
                &root_bytes,
                || Ok(()),
            )?;
            observer(CheckpointPoint::RootPublished)?;
            let current = Current {
                schema: "mycity-m0-current-v1".into(),
                slot,
                root_sha256: sha256(&root_bytes),
            };
            atomic_file(&self.io.dir, "CURRENT.json", &json(&current)?, || {
                observer(CheckpointPoint::CurrentRenamed)
            })?;
            observer(CheckpointPoint::CurrentSynced)?;
            self.io.root = root;
            self.io.wal = new_wal;
            self.engine.retire_checkpointed_journal();
            self.io.background_live.clear();
            background::gc(&self.io.dir, &self.io.root, &self.io.background_live)?;
            // Exact outcomes are retained in the immutable chain before removing old frames.
            fs::remove_file(self.io.dir.join(wal_name(old_segment)))?;
            sync_directory(&self.io.dir)?;
            observer(CheckpointPoint::Retired)?;
            Ok(())
        })();
        if result
            .as_ref()
            .is_err_and(|e| !matches!(e, Error::Capacity { .. }))
        {
            self.io.poisoned = true;
        }
        result
    }
}
