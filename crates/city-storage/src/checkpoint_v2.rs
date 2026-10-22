//! Raw bounded world images plus a permanent, separately verified original-receipt chain.
//! The previous Engine image is historical; no silent rollback or WAL-bridge recovery.
use super::*;
mod receipts;
use checkpoint::{CheckpointPoint as Point, CHECKPOINT_LIMIT};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
    generation: u64,
    sha256: String,
}
impl Reference {
    pub(super) fn heap_capacity_bytes(&self) -> usize {
        self.sha256.capacity()
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldPoint {
    generation: u64,
    sequence: u64,
    chain: String,
    bytes: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: String,
    genesis_sha256: String,
    current: WorldPoint,
    previous: Option<WorldPoint>,
    receipts: Option<Reference>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptIndex {
    schema: String,
    genesis_sha256: String,
    first: u64,
    last: u64,
    body_bytes: u64,
    body_sha256: String,
    #[serde(deserialize_with = "receipts::operations")]
    operations: Vec<u64>,
    previous: Option<Reference>,
}
#[derive(Serialize)]
struct ReceiptBody<'a> {
    schema: &'a str,
    genesis_sha256: &'a str,
    first: u64,
    last: u64,
    previous: Option<&'a Reference>,
    receipts: &'a [JournalRecord],
}

pub(super) const fn capacity_metadata_headers() -> usize {
    std::mem::size_of::<Snapshot>()
        + std::mem::size_of::<ReceiptIndex>()
        + std::mem::size_of::<ReceiptBody<'_>>()
}
fn world_name(g: u64) -> String {
    format!("world-{g}.json")
}
fn manifest_name(g: u64) -> String {
    format!("world-index-{g}.json")
}
fn body_name(g: u64) -> String {
    format!("receipts-{g}.json")
}
fn index_name(g: u64) -> String {
    format!("receipt-index-{g}.json")
}
fn snapshot(dir: &Path, r: &Reference) -> Result<Snapshot, Error> {
    let bytes = read_file(&dir.join(manifest_name(r.generation)), 32 * 1024)?;
    if sha256(&bytes) != r.sha256 {
        return Err(Error::Invalid("v2_world_index_checksum"));
    }
    let s: Snapshot = serde_json::from_slice(&bytes)?;
    if s.schema != "mycity-world-snapshot-v2"
        || s.current.generation != r.generation
        || s.current.bytes > 4 * 1024 * 1024
        || s.current.bytes == 0
        || s.previous.as_ref().is_some_and(|p| {
            p.generation >= r.generation
                || p.sequence > s.current.sequence
                || p.bytes > 4 * 1024 * 1024
                || p.bytes == 0
        })
        || s.receipts
            .as_ref()
            .is_some_and(|h| h.generation > r.generation)
    {
        return Err(Error::Invalid("v2_world_index_shape"));
    }
    Ok(s)
}
fn world_identity(p: &WorldPoint) -> stream::Identity {
    stream::Identity {
        bytes: p.bytes,
        sha256: p.sha256.clone(),
    }
}
fn restore_world_into(
    path: &Path,
    identity: &stream::Identity,
    slot: &mut mycity_core::transaction::EngineSlot,
) -> Result<(), Error> {
    let result = stream::verified(path, identity, 4 * 1024 * 1024, |reader| {
        slot.restore_reader_into(reader).map_err(Error::Model)
    });
    if result.is_err() {
        slot.reset_preserving_capacity();
    }
    result
}

fn index(dir: &Path, r: &Reference) -> Result<ReceiptIndex, Error> {
    let bytes = read_file(&dir.join(index_name(r.generation)), 32 * 1024)?;
    if sha256(&bytes) != r.sha256 {
        return Err(Error::Invalid("v2_receipt_index_checksum"));
    }
    let i: ReceiptIndex = serde_json::from_slice(&bytes)?;
    if i.schema != "mycity-receipt-index-v2"
        || i.operations.is_empty()
        || i.operations.len() > MAX_RECEIPTS
        || i.first == 0
        || i.first > i.last
        || i.last - i.first + 1 != i.operations.len() as u64
        || i.body_bytes > CHECKPOINT_LIMIT
        || i.body_bytes == 0
        || i.operations.contains(&0)
        || i.operations
            .iter()
            .enumerate()
            .any(|(n, x)| i.operations[..n].contains(x))
        || i.previous
            .as_ref()
            .is_some_and(|p| p.generation >= r.generation)
    {
        return Err(Error::Invalid("v2_receipt_index_shape"));
    }
    Ok(i)
}

// Kept as the existing restore boundary; the two transport inputs are optional trust bindings.
#[allow(clippy::too_many_arguments)]
pub(super) fn open_into(
    dir: &Path,
    r: &Reference,
    genesis: &str,
    ceiling: u64,
    transport_profile: Option<&mycity_core::market_access::TransportAllocationProfile>,
    transport_allocation_hash: Option<[u8; 32]>,
    slot: &mut mycity_core::transaction::EngineSlot,
    meta: &mut MetadataWorkspace,
) -> Result<(u64, String), Error> {
    let s = snapshot(dir, r)?;
    if s.genesis_sha256 != genesis || s.current.sequence > ceiling {
        return Err(Error::Invalid("v2_world_boundary"));
    }
    let mut cursor = s.receipts.clone();
    let mut next = s.current.sequence;
    // Restore the authoritative current Engine before archive verification. A road-effect
    // proof's historical epoch is derived from its service day and this persisted publication
    // timestamp, never from a caller-provided capacity or the current edge alone.
    restore_world_into(
        &dir.join(world_name(s.current.generation)),
        &world_identity(&s.current),
        slot,
    )?;
    let mut transport_refs = town_transport::ArchivedProofRefs::new()?;
    while let Some(r) = cursor {
        let i = index(dir, &r)?;
        if i.genesis_sha256 != genesis || i.last != next {
            return Err(Error::Invalid("v2_receipt_sequence_chain"));
        }
        transport_refs.clear();
        let mut capture_error = None;
        receipts::scan(dir, &r, &i, meta, |_, record| {
            if capture_error.is_none() {
                capture_error = transport_refs.push_record(&record).err();
            }
        })?;
        if let Some(error) = capture_error {
            return Err(error);
        }
        transport_refs.verify(
            dir,
            meta,
            slot.engine().map_err(Error::Model)?,
            transport_profile,
            transport_allocation_hash,
        )?;
        next = i.first - 1;
        cursor = i.previous;
    }
    if next != 0 {
        return Err(Error::Invalid("v2_receipt_chain_gap"));
    }
    // Historical bytes must remain intact. They are not an automatic fallback.
    if let Some(p) = &s.previous {
        stream::verify_bytes(
            &dir.join(world_name(p.generation)),
            &world_identity(p),
            4 * 1024 * 1024,
        )?;
    }
    Ok((s.current.sequence, s.current.chain))
}
pub(super) fn lookup(
    dir: &Path,
    r: Option<&Reference>,
    operation: u64,
    meta: &mut MetadataWorkspace,
) -> Result<Option<JournalRecord>, Error> {
    let Some(r) = r else {
        return Ok(None);
    };
    let s = snapshot(dir, r)?;
    let mut cursor = s.receipts;
    while let Some(r) = cursor {
        let i = index(dir, &r)?;
        if i.operations.contains(&operation) {
            let mut found = None;
            receipts::scan(dir, &r, &i, meta, |_, record| {
                if record.operation_id == operation {
                    found = Some(record);
                }
            })?;
            return found.map(Some).ok_or(Error::Invalid("v2_receipt_missing"));
        }
        cursor = i.previous;
    }
    Ok(None)
}
type ReceiptPage = [Option<(usize, JournalRecord)>; 64];
pub(super) const fn receipt_page_staging_bytes() -> usize {
    std::mem::size_of::<ReceiptPage>()
}
pub(super) fn rows(
    dir: &Path,
    r: Option<&Reference>,
    skip: &mut usize,
    limit: usize,
    rows: &mut Vec<serde_json::Value>,
    meta: &mut MetadataWorkspace,
) -> Result<(), Error> {
    if limit > 64 || rows.len() > limit {
        return Err(Error::Invalid("query_page_limit"));
    }
    let Some(r) = r else {
        return Ok(());
    };
    let mut cursor = snapshot(dir, r)?.receipts;
    while rows.len() < limit {
        let Some(r) = cursor else {
            break;
        };
        let i = index(dir, &r)?;
        if *skip >= i.operations.len() {
            *skip -= i.operations.len();
            cursor = i.previous;
            continue;
        }
        let end = i.operations.len() - *skip;
        let begin = end.saturating_sub(limit - rows.len());
        let mut page: ReceiptPage = [None; 64];
        receipts::scan(dir, &r, &i, meta, |n, record| {
            if (begin..end).contains(&n) {
                page[n - begin] = Some((n, record));
            }
        })?;
        // Publish a page only after the complete archive (including its tail)
        // passes metadata, sequence, EOF, length and checksum validation.
        *skip = 0;
        for (n, record) in page.into_iter().take(end - begin).rev().flatten() {
            rows.push(serde_json::json!({"committed_sequence":i.first+n as u64,"record":record}));
        }
        cursor = i.previous;
    }
    Ok(())
}
fn reachable(dir: &Path, head: Option<&Reference>, generation: u64) -> Result<bool, Error> {
    let mut cursor = head.cloned();
    while let Some(r) = cursor {
        if r.generation == generation {
            return Ok(true);
        }
        cursor = index(dir, &r)?.previous;
    }
    Ok(false)
}
fn generation(name: &str, prefix: &str, suffix: &str) -> Option<u64> {
    let n = name
        .strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse::<u64>()
        .ok()?;
    (name == format!("{prefix}{n}{suffix}")).then_some(n)
}
/// Only after CURRENT, world, all committed receipt bodies and WAL have validated.
/// Non-reachable receipt candidates were never committed. No committed segment is removed.
pub(super) fn gc(
    dir: &Path,
    r: Option<&Reference>,
    segment: u64,
    mut observe: impl FnMut(Point) -> io::Result<()>,
) -> Result<(), Error> {
    let s = r.map(|r| snapshot(dir, r)).transpose()?;
    let mut changed = false;
    for e in fs::read_dir(dir)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        let obsolete = if let Some(g) = generation(&name, "world-", ".json") {
            !s.as_ref().is_some_and(|s| {
                s.current.generation == g || s.previous.as_ref().is_some_and(|p| p.generation == g)
            })
        } else if let Some(g) = generation(&name, "world-index-", ".json") {
            r.is_none_or(|r| r.generation != g)
        } else if let Some(g) = generation(&name, "receipts-", ".json")
            .or_else(|| generation(&name, "receipt-index-", ".json"))
        {
            !reachable(dir, s.as_ref().and_then(|s| s.receipts.as_ref()), g)?
        } else if name == "wal.bin" {
            segment != 0
        } else if let Some(g) = generation(&name, "wal-", ".bin") {
            g != segment
        } else {
            [
                ".world-",
                ".world-index-",
                ".receipts-",
                ".receipt-index-",
                ".wal-",
            ]
            .iter()
            .any(|p| name.starts_with(p))
                && name.ends_with(".tmp")
        };
        if obsolete {
            fs::remove_file(e.path())?;
            changed = true;
            observe(Point::GcFileRemoved)?;
        }
    }
    if changed {
        sync_directory(dir)?;
    }
    Ok(())
}
impl Store {
    pub(super) fn checkpoint_v2(
        &mut self,
        emergency: bool,
        mut observer: impl FnMut(Point) -> io::Result<()>,
        publication_started: &mut bool,
    ) -> Result<(), Error> {
        (|| {
            let generation = self
                .io
                .root
                .generation
                .checked_add(1)
                .ok_or(Error::Invalid("publication_overflow"))?;
            let previous = self
                .io
                .root
                .checkpoint_v2
                .as_ref()
                .map(|r| snapshot(&self.io.dir, r))
                .transpose()?;
            let image = stream::measure(4 * 1024 * 1024, |writer| {
                self.engine.write_checkpoint(writer).map_err(Error::Model)
            })?;
            let point = WorldPoint {
                generation,
                sequence: self.io.root.sequence,
                chain: self.io.root.wal_chain.clone(),
                bytes: image.bytes,
                sha256: image.sha256.clone(),
            };
            let prior_receipts = previous.as_ref().and_then(|s| s.receipts.clone());
            let journal = self.engine.journal();
            let receipt = if journal.is_empty() {
                None
            } else {
                let n = journal.len() as u64;
                let first = self
                    .io
                    .root
                    .sequence
                    .checked_sub(n)
                    .and_then(|x| x.checked_add(1))
                    .ok_or(Error::Invalid("v2_receipt_count"))?;
                let b = ReceiptBody {
                    schema: "mycity-receipts-v2",
                    genesis_sha256: &self.io.root.genesis_sha256,
                    first,
                    last: self.io.root.sequence,
                    previous: prior_receipts.as_ref(),
                    receipts: journal,
                };
                let body = stream::measure(CHECKPOINT_LIMIT, |writer| {
                    serde_json::to_writer(writer, &b).map_err(Error::from)
                })?;
                let i = json(&ReceiptIndex {
                    schema: "mycity-receipt-index-v2".into(),
                    genesis_sha256: b.genesis_sha256.to_owned(),
                    first,
                    last: b.last,
                    body_bytes: body.bytes,
                    body_sha256: body.sha256.clone(),
                    operations: journal.iter().map(|r| r.operation_id).collect(),
                    previous: prior_receipts.clone(),
                })?;
                if i.len() > 32 * 1024 {
                    return Err(Error::Invalid("v2_receipt_index_bytes"));
                }
                Some((b, body, i))
            };
            let head = receipt
                .as_ref()
                .map(|(_, _, i)| Reference {
                    generation,
                    sha256: sha256(i),
                })
                .or_else(|| prior_receipts.clone());
            let manifest = json(&Snapshot {
                schema: "mycity-world-snapshot-v2".into(),
                genesis_sha256: self.io.root.genesis_sha256.clone(),
                current: point,
                previous: previous.map(|s| s.current),
                receipts: head,
            })?;
            if manifest.len() > 32 * 1024 {
                return Err(Error::Invalid("v2_world_index_bytes"));
            }
            let required = image.bytes
                + manifest.len() as u64
                + receipt
                    .as_ref()
                    .map_or(0, |(_, b, i)| b.bytes + i.len() as u64)
                + 2 * META_LIMIT;
            capacity::admit(&self.io.dir, self.io.root.capacity, required, emergency)?;
            let available = fs2::available_space(&self.io.dir)?;
            if available < required + 2 * 1024 * 1024 * 1024 {
                return Err(Error::Invalid("v2_disk_free_floor"));
            }
            *publication_started = true;
            stream::atomic(
                &self.io.dir,
                &world_name(generation),
                &image,
                4 * 1024 * 1024,
                |writer| self.engine.write_checkpoint(writer).map_err(Error::Model),
                |path| restore_world_into(path, &image, &mut self.restore_workspace),
            )?;
            observer(Point::WorldSynced)?;
            if let Some((body, identity, i)) = receipt {
                stream::atomic(
                    &self.io.dir,
                    &body_name(generation),
                    &identity,
                    CHECKPOINT_LIMIT,
                    |writer| serde_json::to_writer(writer, &body).map_err(Error::from),
                    |path| stream::verify_bytes(path, &identity, CHECKPOINT_LIMIT),
                )?;
                observer(Point::ReceiptsSynced)?;
                atomic_file(&self.io.dir, &index_name(generation), &i, || Ok(()))?;
                observer(Point::ReceiptIndexSynced)?;
            }
            atomic_file(&self.io.dir, &manifest_name(generation), &manifest, || {
                Ok(())
            })?;
            observer(Point::SnapshotSynced)?;
            let wal_name = checkpoint::wal_name(generation);
            atomic_file(&self.io.dir, &wal_name, &[], || Ok(()))?;
            let wal = OpenOptions::new()
                .read(true)
                .write(true)
                .open(self.io.dir.join(&wal_name))?;
            observer(Point::SegmentSynced)?;
            let old_segment = self.io.root.segment;
            let mut root = self.io.root.clone();
            root.generation = generation;
            root.segment = generation;
            root.wal_end = 0;
            root.checkpoint_v2 = Some(Reference {
                generation,
                sha256: sha256(&manifest),
            });
            let slot = (generation % 2) as u8;
            self.io.metadata.small.encode(&root, META_LIMIT as usize)?;
            let root_bytes = self.io.metadata.small.as_slice();
            atomic_file(
                &self.io.dir,
                &format!("root-{slot}.json"),
                root_bytes,
                || Ok(()),
            )?;
            observer(Point::RootPublished)?;
            let current = Current {
                schema: "mycity-m0-current-v1".into(),
                slot,
                root_sha256: sha256(root_bytes),
            };
            self.io
                .metadata
                .small
                .encode(&current, META_LIMIT as usize)?;
            atomic_file(
                &self.io.dir,
                "CURRENT.json",
                self.io.metadata.small.as_slice(),
                || observer(Point::CurrentRenamed),
            )?;
            observer(Point::CurrentSynced)?;
            self.io.root = root;
            self.io.wal = wal;
            self.engine.retire_checkpointed_journal();
            self.io.background_live.clear();
            background::gc(&self.io.dir, &self.io.root, &self.io.background_live)?;
            if old_segment == 0 {
                fs::remove_file(self.io.dir.join("wal.bin"))?;
            }
            gc(
                &self.io.dir,
                self.io.root.checkpoint_v2.as_ref(),
                generation,
                &mut observer,
            )?;
            sync_directory(&self.io.dir)?;
            observer(Point::Retired)?;
            Ok(())
        })()
    }
}
