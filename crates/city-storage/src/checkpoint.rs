//! Complete state checkpoints, immutable receipt chain and bounded hot WAL.
use super::*;
pub const CHECKPOINT_LIMIT: u64 = 16 * 1024 * 1024;
pub const ARCHIVE_LIMIT: u64 = 128 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
    generation: u64,
    #[serde(deserialize_with = "crate::metadata::text::<_,64>")]
    sha256: String,
}
impl Reference {
    pub(super) fn heap_capacity_bytes(&self) -> usize {
        self.sha256.capacity()
    }
}
pub(super) mod receipt_decode;
mod retained;
use retained::load_index;
pub(super) use retained::{load, verify_archive};

/// Hash the exact serde_json envelope bytes without materializing a second full JSON buffer
/// while WAL frames, decoded receipt actions and the fixed receipt archive are live.
fn envelope_digest(envelope: &mycity_core::transaction::Envelope) -> Result<[u8; 32], Error> {
    use sha2::{Digest, Sha256};

    struct HashWriter(Sha256);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, envelope)?;
    Ok(writer.0.finalize().into())
}

pub(super) fn verify_transport_proofs(
    dir: &Path,
    reference: &Reference,
    profile: Option<&mycity_core::market_access::TransportAllocationProfile>,
    allocation_hash: Option<[u8; 32]>,
    catalog: Option<&super::spatial_transport_updates::SpatialTransportCatalog>,
    engine: &mycity_core::transaction::Engine,
    meta: &mut MetadataWorkspace,
) -> Result<(), Error> {
    let mut refs = town_transport::ArchivedProofRefs::new()?;
    let mut cursor = Some(reference.clone());
    while let Some(reference) = cursor {
        let checkpoint = load(dir, &reference, meta)?;
        refs.clear();
        for record in &meta.receipts {
            refs.push_record(record)?;
        }
        refs.verify(dir, meta, engine, profile, allocation_hash, catalog)?;
        cursor = checkpoint.previous;
    }
    Ok(())
}

fn index_name(generation: u64) -> String {
    format!("checkpoint-index-{generation}.json")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointPoint {
    WorldSynced,
    ReceiptsSynced,
    ReceiptIndexSynced,
    GcFileRemoved,
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
/// Shared by live Store and pre-publication cold/WAL recovery. Authenticates only
/// the receipt chain bound by this exact root, never a caller-provided record file.
pub(super) fn lookup_archived_at(dir:&Path,root:&Root,operation:u64,metadata:&mut MetadataWorkspace)
    ->Result<Option<JournalRecord>,Error> {
    if root.checkpoint_layout==2 {
        return checkpoint_v2::lookup(dir,root.checkpoint_v2.as_ref(),operation,metadata);
    }
    let mut cursor=root.checkpoint.clone();
    while let Some(reference)=cursor {
        let index=load_index(dir,&reference,metadata)?;
        if metadata.operations.contains(&operation) {
            load(dir,&reference,metadata)?;
            return metadata.receipts.iter().find(|r|r.operation_id==operation).copied()
                .map(Some).ok_or(Error::Invalid("checkpoint_missing_receipt"));
        }
        cursor=index.previous;
    }
    Ok(None)
}

impl Store {
    /// Rehydrate full, durable hot receipts from the currently committed WAL segment. The
    /// Engine retry window stores compact summaries only; checkpoint/archive records retain
    /// the original `JournalRecord` bytes and action payloads.
    fn load_hot_receipts(&mut self) -> Result<(), Error> {
        let expected = self.engine.journal().len();
        self.io.metadata.receipts.clear();
        if expected == 0 {
            if self.io.root.sequence != self.io.wal_base_sequence || self.io.root.wal_end != 0 {
                return Err(Error::Invalid("hot_receipt_segment_boundary"));
            }
            return Ok(());
        }
        if expected > MAX_RECEIPTS
            || self.io.root.sequence.checked_sub(self.io.wal_base_sequence) != Some(expected as u64)
        {
            return Err(Error::Invalid("hot_receipt_count"));
        }
        let path = self.io.dir.join(wal_name(self.io.root.segment));
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(Error::Invalid("symlink_wal"));
        }
        let mut wal = OpenOptions::new().read(true).open(path)?;
        if self.io.root.wal_end > wal.metadata()?.len() {
            return Err(Error::Invalid("committed_wal_truncated"));
        }
        let mut buffers = wal::Buffers::new();
        let mut position = 0u64;
        let mut sequence = self.io.wal_base_sequence;
        let mut chain = self.io.wal_base_chain.clone();
        while position < self.io.root.wal_end {
            if self.io.root.wal_end - position < 4 {
                return Err(Error::Invalid("committed_header_truncated"));
            }
            let mut length_bytes = [0; 4];
            wal.read_exact(&mut length_bytes)?;
            let length = u32::from_le_bytes(length_bytes) as usize;
            if length == 0
                || length > FRAME_LIMIT
                || self.io.root.wal_end - position < 4 + length as u64 + 32
            {
                return Err(Error::Invalid("committed_frame_length"));
            }
            let payload = &mut buffers.frame[..length];
            wal.read_exact(payload)?;
            let mut checksum = [0; 32];
            wal.read_exact(&mut checksum)?;
            let actual = wal::checksum(&length_bytes, payload);
            if actual != checksum {
                return Err(Error::Invalid("committed_frame_checksum"));
            }
            let entry = wal::decode(payload)?;
            sequence = sequence
                .checked_add(1)
                .ok_or(Error::Invalid("sequence_overflow"))?;
            if entry.sequence != sequence || entry.previous != chain {
                return Err(Error::Invalid("committed_frame_chain"));
            }
            let mut normalized = mycity_core::canonical::FixedWriter::new(&mut buffers.record);
            self.io
                .canonical
                .get_mut()
                .workspace
                .write(entry.record.get().as_bytes(), &mut normalized)
                .map_err(Error::Model)?;
            let record: JournalRecord = serde_json::from_slice(normalized.as_slice())?;
            if record.operation_id != entry.envelope.operation_id
                || record.expected_revision != entry.envelope.expected_revision
                || record.payload_sha256 != envelope_digest(&entry.envelope)?
            {
                return Err(Error::Invalid("hot_receipt_wal_identity"));
            }
            if self.io.metadata.receipts.len() == self.io.metadata.receipts.capacity() {
                return Err(Error::Invalid("hot_receipt_capacity"));
            }
            self.io.metadata.receipts.push(record);
            chain = super::hex(&actual);
            position += 4 + length as u64 + 32;
        }
        if position != self.io.root.wal_end
            || sequence != self.io.root.sequence
            || chain != self.io.root.wal_chain
            || self.io.metadata.receipts.len() != expected
            || self
                .engine
                .journal()
                .iter()
                .zip(&self.io.metadata.receipts)
                .any(|(hot, durable)| !hot.matches_durable(durable))
        {
            return Err(Error::Invalid("hot_receipt_wal_mismatch"));
        }
        Ok(())
    }

    fn receipt_rows_from_hot(&mut self) -> Result<(), Error> {
        let result = self.load_hot_receipts();
        if result.is_err() {
            self.io.poisoned = true;
        }
        result
    }

    /// Read an original hot or checkpointed receipt without re-executing it.
    /// Host-local diagnostic API; this does not grant mutation authority.
    pub fn receipt(&mut self, operation: u64) -> Result<Option<JournalRecord>, Error> {
        self.engine()?;
        if self
            .engine
            .journal()
            .iter()
            .any(|r| r.operation_id == operation)
        {
            let result = self.load_hot_receipts().and_then(|()| {
                self.io
                    .metadata
                    .receipts
                    .iter()
                    .find(|r| r.operation_id == operation)
                    .copied()
                    .map(Some)
                    .ok_or(Error::Invalid("hot_receipt_missing"))
            });
            if result.is_err() {
                self.io.poisoned = true;
            }
            return result;
        }
        self.lookup_archived(operation)
    }
    pub(super) fn lookup_archived(
        &mut self,
        operation: u64,
    ) -> Result<Option<JournalRecord>, Error> {
        let result=lookup_archived_at(&self.io.dir,&self.io.root,operation,&mut self.io.metadata);
        if result.is_err() {self.io.poisoned=true;}
        result
    }

    pub(super) fn receipt_rows(
        &mut self,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<serde_json::Value>, usize), Error> {
        let result = (|| {
            self.receipt_rows_from_hot()?;
            let total = usize::try_from(self.io.root.sequence)
                .map_err(|_| Error::Invalid("query_sequence_range"))?;
            if offset > total {
                return Err(Error::Invalid("query_offset"));
            }
            let mut rows = Vec::with_capacity(limit);
            let mut skipped = offset;
            for (i, record) in self.io.metadata.receipts.iter().enumerate().rev() {
                if skipped > 0 {
                    skipped -= 1;
                    continue;
                }
                if rows.len() == limit {
                    break;
                }
                rows.push(serde_json::json!({"committed_sequence":self.io.root.sequence-self.io.metadata.receipts.len()as u64+i as u64+1,"record":record}));
            }
            if self.io.root.checkpoint_layout == 2 {
                checkpoint_v2::rows(
                    &self.io.dir,
                    self.io.root.checkpoint_v2.as_ref(),
                    &mut skipped,
                    limit,
                    &mut rows,
                    &mut self.io.metadata,
                )?;
                return Ok((rows, total));
            }
            let mut cursor = self.io.root.checkpoint.clone();
            while rows.len() < limit {
                let Some(r) = cursor else {
                    break;
                };
                let index = load_index(&self.io.dir, &r, &mut self.io.metadata)?;
                if skipped >= self.io.metadata.operations.len() {
                    skipped -= self.io.metadata.operations.len();
                    cursor = index.previous;
                    continue;
                }
                let cp = load(&self.io.dir, &r, &mut self.io.metadata)?;
                for (i, record) in self.io.metadata.receipts.iter().enumerate().rev() {
                    if skipped > 0 {
                        skipped -= 1;
                        continue;
                    }
                    if rows.len() == limit {
                        break;
                    }
                    rows.push(serde_json::json!({"committed_sequence":cp.sequence-self.io.metadata.receipts.len()as u64+i as u64+1,"record":record}));
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
        observer: impl FnMut(CheckpointPoint) -> io::Result<()>,
    ) -> Result<(), Error> {
        self.engine()?;
        // Arm before any observer or persistent publication. Unwinding cannot
        // leave an apparently usable Store with a newer durable CURRENT.
        self.io.poisoned = true;
        self.load_hot_receipts()?;
        let mut publication_started = false;
        let result = if self.io.root.checkpoint_layout == 2 {
            self.checkpoint_v2(emergency, observer, &mut publication_started)
        } else {
            self.checkpoint_legacy_inner(emergency, observer, &mut publication_started)
        };
        if result.is_ok()
            || (!publication_started && matches!(&result, Err(Error::Capacity { .. })))
        {
            self.io.poisoned = false;
        }
        result
    }
    fn checkpoint_legacy_inner(
        &mut self,
        emergency: bool,
        mut observer: impl FnMut(CheckpointPoint) -> io::Result<()>,
        publication_started: &mut bool,
    ) -> Result<(), Error> {
        (|| {
            let generation = self
                .io
                .root
                .generation
                .checked_add(1)
                .ok_or(Error::Invalid("publication_overflow"))?;
            #[derive(Serialize)]
            struct View<'a> {
                schema: &'a str,
                sequence: u64,
                chain: &'a str,
                genesis_sha256: &'a str,
                engine: &'a [u8],
                receipts: &'a [JournalRecord],
                previous: Option<&'a Reference>,
            }
            #[derive(Serialize)]
            struct IndexView<'a> {
                data_sha256: String,
                operations: &'a [u64],
                previous: Option<&'a Reference>,
            }
            let meta = &mut self.io.metadata;
            meta.engine
                .write_with(mycity_core::transaction::ENGINE_BYTE_BUDGET, |out| {
                    self.engine.write_checkpoint(out).map_err(Error::Model)
                })?;
            let cp = View {
                schema: "mycity-m0-checkpoint-v1",
                sequence: self.io.root.sequence,
                chain: &self.io.root.wal_chain,
                genesis_sha256: &self.io.root.genesis_sha256,
                engine: meta.engine.as_slice(),
                receipts: &meta.receipts,
                previous: self.io.root.checkpoint.as_ref(),
            };
            meta.file.encode(&cp, CHECKPOINT_LIMIT as usize)?;
            let bytes = meta.file.as_slice();
            capacity::admit(
                &self.io.dir,
                self.io.root.capacity,
                bytes.len() as u64 + 32 * 1024 + 2 * META_LIMIT,
                emergency,
            )?;
            if archive_bytes(&self.io.dir)?
                .checked_add(bytes.len() as u64)
                .ok_or(Error::Invalid("archive_size_overflow"))?
                > ARCHIVE_LIMIT
            {
                return Err(Error::Invalid("archive_capacity"));
            }
            *publication_started = true;
            atomic_file(&self.io.dir, &cp_name(generation), bytes, || Ok(()))?;
            if self.engine.journal().len() > meta.operations.capacity() {
                return Err(Error::Invalid("checkpoint_operations_capacity"));
            }
            meta.operations.clear();
            meta.operations
                .extend(self.engine.journal().iter().map(|r| r.operation_id));
            meta.small.encode(
                &IndexView {
                    data_sha256: sha256(bytes),
                    operations: &meta.operations,
                    previous: self.io.root.checkpoint.as_ref(),
                },
                32 * 1024,
            )?;
            atomic_file(
                &self.io.dir,
                &index_name(generation),
                meta.small.as_slice(),
                || Ok(()),
            )?;
            let index_hash = sha256(meta.small.as_slice());
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
                sha256: index_hash,
            });
            meta.small.encode(&root, META_LIMIT as usize)?;
            let root_bytes = meta.small.as_slice();
            let slot = (generation % 2) as u8;
            atomic_file(
                &self.io.dir,
                &format!("root-{slot}.json"),
                root_bytes,
                || Ok(()),
            )?;
            observer(CheckpointPoint::RootPublished)?;
            let current = Current {
                schema: "mycity-m0-current-v1".into(),
                slot,
                root_sha256: sha256(root_bytes),
            };
            meta.small.encode(&current, META_LIMIT as usize)?;
            atomic_file(&self.io.dir, "CURRENT.json", meta.small.as_slice(), || {
                observer(CheckpointPoint::CurrentRenamed)
            })?;
            observer(CheckpointPoint::CurrentSynced)?;
            self.io.wal_base_sequence = root.sequence;
            self.io.wal_base_chain = root.wal_chain.clone();
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
        })()
    }
}

#[cfg(test)]
mod envelope_digest_tests {
    use super::*;

    #[test]
    fn streamed_max_action_envelope_hash_matches_legacy_vec_bytes() {
        use mycity_core::transaction::{Action, Envelope, MAX_ACTIONS};

        let action = Action::SpatialPermission {
            command:
                mycity_core::spatial_transport_permissions::SpatialPermissionCommand::Request {
                    expected_state_revision: 1,
                    request: mycity_core::spatial_transport_permissions::SpatialUpdateRequest {
                        request_id: 7,
                        applicant_account: 2,
                        executor_account: 3,
                        asset_id: 5,
                        operation:
                            mycity_core::spatial_transport_permissions::SpatialAccessOperation::Close,
                        surface:
                            mycity_core::spatial_transport_permissions::SpatialAccessSurface::Road,
                        edge_id: 9,
                        expected_revision: 1,
                        candidate_profile_sha256: [0xA5; 32],
                        effective_time: mycity_core::units::GameTime::new(86_400).unwrap(),
                        parent_profile_sha256: [0x5A; 32],
                    },
                },
        };
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: 99,
            expected_revision: 4,
            actions: vec![action; MAX_ACTIONS],
        };
        let legacy_bytes = super::super::json(&envelope).unwrap();
        assert_eq!(
            envelope_digest(&envelope).unwrap(),
            super::super::digest(&legacy_bytes),
        );
    }
}
