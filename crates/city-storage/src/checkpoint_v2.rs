//! Raw bounded world images plus a permanent, separately verified original-receipt chain.
//! The previous Engine image is historical; no silent rollback or WAL-bridge recovery.
use super::*;
use checkpoint::{CheckpointPoint as Point, CHECKPOINT_LIMIT};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
    generation: u64,
    sha256: String,
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
    operations: Vec<u64>,
    previous: Option<Reference>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptBody {
    schema: String,
    genesis_sha256: String,
    first: u64,
    last: u64,
    previous: Option<Reference>,
    receipts: Vec<JournalRecord>,
}

pub(super) const fn capacity_metadata_headers() -> usize {
    std::mem::size_of::<Snapshot>()
        + std::mem::size_of::<ReceiptIndex>()
        + std::mem::size_of::<ReceiptBody>()
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
fn world_bytes(dir: &Path, p: &WorldPoint) -> Result<Vec<u8>, Error> {
    let bytes = read_file(&dir.join(world_name(p.generation)), 4 * 1024 * 1024)?;
    if bytes.len() as u64 != p.bytes || sha256(&bytes) != p.sha256 {
        return Err(Error::Invalid("v2_world_checksum"));
    }
    Ok(bytes)
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
fn body(dir: &Path, r: &Reference, i: &ReceiptIndex) -> Result<ReceiptBody, Error> {
    let bytes = read_file(&dir.join(body_name(r.generation)), CHECKPOINT_LIMIT)?;
    if bytes.len() as u64 != i.body_bytes || sha256(&bytes) != i.body_sha256 {
        return Err(Error::Invalid("v2_receipt_checksum"));
    }
    let b: ReceiptBody = serde_json::from_slice(&bytes)?;
    if b.schema != "mycity-receipts-v2"
        || b.genesis_sha256 != i.genesis_sha256
        || b.first != i.first
        || b.last != i.last
        || b.previous != i.previous
        || b.receipts.len() != i.operations.len()
        || b.receipts
            .iter()
            .zip(&i.operations)
            .any(|(r, id)| r.operation_id != *id)
    {
        return Err(Error::Invalid("v2_receipt_index_content"));
    }
    Ok(b)
}
pub(super) fn open(
    dir: &Path,
    r: &Reference,
    genesis: &str,
    ceiling: u64,
) -> Result<(Engine, u64, String), Error> {
    let s = snapshot(dir, r)?;
    if s.genesis_sha256 != genesis || s.current.sequence > ceiling {
        return Err(Error::Invalid("v2_world_boundary"));
    }
    let mut cursor = s.receipts.clone();
    let mut next = s.current.sequence;
    while let Some(r) = cursor {
        let i = index(dir, &r)?;
        if i.genesis_sha256 != genesis || i.last != next {
            return Err(Error::Invalid("v2_receipt_sequence_chain"));
        }
        let _validated = body(dir, &r, &i)?;
        next = i.first - 1;
        cursor = i.previous;
    }
    if next != 0 {
        return Err(Error::Invalid("v2_receipt_chain_gap"));
    }
    // Historical bytes must remain intact. They are not an automatic fallback.
    if let Some(p) = &s.previous {
        drop(world_bytes(dir, p)?);
    }
    let engine =
        Engine::restore_checkpoint(&world_bytes(dir, &s.current)?).map_err(Error::Model)?;
    Ok((engine, s.current.sequence, s.current.chain))
}
pub(super) fn lookup(
    dir: &Path,
    r: Option<&Reference>,
    operation: u64,
) -> Result<Option<JournalRecord>, Error> {
    let Some(r) = r else {
        return Ok(None);
    };
    let s = snapshot(dir, r)?;
    let mut cursor = s.receipts;
    while let Some(r) = cursor {
        let i = index(dir, &r)?;
        if i.operations.contains(&operation) {
            return body(dir, &r, &i)?
                .receipts
                .into_iter()
                .find(|r| r.operation_id == operation)
                .map(Some)
                .ok_or(Error::Invalid("v2_receipt_missing"));
        }
        cursor = i.previous;
    }
    Ok(None)
}
pub(super) fn rows(
    dir: &Path,
    r: Option<&Reference>,
    skip: &mut usize,
    limit: usize,
    rows: &mut Vec<serde_json::Value>,
) -> Result<(), Error> {
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
        let b = body(dir, &r, &i)?;
        for (n, r) in b.receipts.iter().enumerate().rev() {
            if *skip > 0 {
                *skip -= 1;
                continue;
            }
            if rows.len() == limit {
                break;
            }
            rows.push(serde_json::json!({"committed_sequence":b.first+n as u64,"record":r}));
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
    ) -> Result<(), Error> {
        let result = (|| {
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
            let bytes = self.engine.export_checkpoint().map_err(Error::Model)?;
            // Validate the exact complete serialized image, without a JSON Value tree.
            drop(Engine::restore_checkpoint(&bytes).map_err(Error::Model)?);
            let point = WorldPoint {
                generation,
                sequence: self.io.root.sequence,
                chain: self.io.root.wal_chain.clone(),
                bytes: bytes.len() as u64,
                sha256: sha256(&bytes),
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
                    schema: "mycity-receipts-v2".into(),
                    genesis_sha256: self.io.root.genesis_sha256.clone(),
                    first,
                    last: self.io.root.sequence,
                    previous: prior_receipts.clone(),
                    receipts: journal.to_vec(),
                };
                let body = json(&b)?;
                if body.len() as u64 > CHECKPOINT_LIMIT {
                    return Err(Error::Invalid("v2_receipt_bytes"));
                }
                let i = json(&ReceiptIndex {
                    schema: "mycity-receipt-index-v2".into(),
                    genesis_sha256: b.genesis_sha256,
                    first,
                    last: b.last,
                    body_bytes: body.len() as u64,
                    body_sha256: sha256(&body),
                    operations: journal.iter().map(|r| r.operation_id).collect(),
                    previous: prior_receipts.clone(),
                })?;
                if i.len() > 32 * 1024 {
                    return Err(Error::Invalid("v2_receipt_index_bytes"));
                }
                Some((body, i))
            };
            let head = receipt
                .as_ref()
                .map(|(_, i)| Reference {
                    generation,
                    sha256: sha256(i),
                })
                .or(prior_receipts);
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
            let required = bytes.len() as u64
                + manifest.len() as u64
                + receipt
                    .as_ref()
                    .map_or(0, |(b, i)| (b.len() + i.len()) as u64)
                + 2 * META_LIMIT;
            capacity::admit(&self.io.dir, self.io.root.capacity, required, emergency)?;
            let available = fs2::available_space(&self.io.dir)?;
            if available < required + 2 * 1024 * 1024 * 1024 {
                return Err(Error::Invalid("v2_disk_free_floor"));
            }
            atomic_file(&self.io.dir, &world_name(generation), &bytes, || Ok(()))?;
            observer(Point::WorldSynced)?;
            if let Some((b, i)) = receipt {
                atomic_file(&self.io.dir, &body_name(generation), &b, || Ok(()))?;
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
            let root_bytes = json(&root)?;
            atomic_file(
                &self.io.dir,
                &format!("root-{slot}.json"),
                &root_bytes,
                || Ok(()),
            )?;
            observer(Point::RootPublished)?;
            atomic_file(
                &self.io.dir,
                "CURRENT.json",
                &json(&Current {
                    schema: "mycity-m0-current-v1".into(),
                    slot,
                    root_sha256: sha256(&root_bytes),
                })?,
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
