//! Full frozen JPROD storage pressure. Missing business domains remain typed stubs.
//! This module cannot mutate or install a gameplay World.
mod bounded_table;
mod decode_workspace;
pub(crate) mod r07;
use decode_workspace::{DecodeWorkspace, DECODE_BYTES};
mod restore_index;
mod reusable_io;
use mycity_core::layout_pressure::NativePressure;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    hint::black_box,
    io::{BufWriter, Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, TryLockError},
    thread::{self, JoinHandle},
    time::Instant,
};
const MIB: usize = 1024 * 1024;
const WORK_PAGES: usize = 128;
const COW_PAGES: usize = 64;
const PAGE: usize = 8192;
pub const NORMAL_BYTES: usize = 310 * 1024 * 1024 * 1024usize / 100;
const FREE_FLOOR: u64 = 2 * 1024 * 1024 * 1024;
fn pattern(size: usize, seed: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; size];
    let mut state = seed;
    for chunk in bytes.chunks_mut(8) {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = state;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        let raw = (x ^ (x >> 31)).to_le_bytes();
        chunk.copy_from_slice(&raw[..chunk.len()]);
    }
    bytes
}
const ACCEPTANCE: &[u8] = include_bytes!("../../../fixtures/m0-t13a-acceptance.json");
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(
    bound(
        serialize = "[u64; N]: Serialize",
        deserialize = "[u64; N]: Deserialize<'de>"
    ),
    deny_unknown_fields
)]
struct Stub<const N: usize> {
    record_id: u64,
    owner_or_group: u64,
    generation: u32,
    schema_version: u16,
    source_kind: u16,
    payload: [u64; N],
}
#[derive(Serialize, Deserialize, PartialEq)]
#[serde(tag = "record_type", content = "records", deny_unknown_fields)]
enum Table {
    W64(Vec<Stub<5>>),
    W128(Vec<Stub<13>>),
    W256(Vec<Stub<29>>),
    PathRefs(Vec<u32>),
}
fn stub<const N: usize>(id: u64, kind: u16, owner: u64) -> Stub<N> {
    let mut payload = [0; N];
    for (i, w) in payload.iter_mut().enumerate() {
        *w = id.wrapping_mul(0x9e3779b97f4a7c15).rotate_left(i as u32);
    }
    Stub {
        record_id: id,
        owner_or_group: owner,
        generation: 1,
        schema_version: 1,
        source_kind: kind,
        payload,
    }
}
impl Table {
    fn new(count: usize, width: usize, kind: u16, owners: usize) -> Result<Self, String> {
        macro_rules! make {
            ($n:literal,$variant:ident) => {{
                let mut v = Vec::with_capacity(count);
                for id in 1..=count {
                    v.push(stub::<$n>(id as u64, kind, ((id - 1) % owners + 1) as u64));
                }
                Self::$variant(v)
            }};
        }
        Ok(match width {
            64 => make!(5, W64),
            128 => make!(13, W128),
            256 => make!(29, W256),
            4 => Self::PathRefs((0..count).map(|i| (i % 14800) as u32).collect()),
            _ => return Err("stub_width".into()),
        })
    }
    fn owned(&self) -> usize {
        match self {
            Self::W64(v) => v.capacity() * size_of::<Stub<5>>(),
            Self::W128(v) => v.capacity() * size_of::<Stub<13>>(),
            Self::W256(v) => v.capacity() * size_of::<Stub<29>>(),
            Self::PathRefs(v) => v.capacity() * 4,
        }
    }
    fn count(&self) -> usize {
        match self {
            Self::W64(v) => v.len(),
            Self::W128(v) => v.len(),
            Self::W256(v) => v.len(),
            Self::PathRefs(v) => v.len(),
        }
    }
    fn scan(&self, mut page: impl FnMut(usize, u64)) -> u64 {
        let mut total = 0u64;
        macro_rules! walk {
            ($v:expr) => {
                for chunk in $v.chunks(PAGE) {
                    let mut sum = 0u64;
                    for record in chunk {
                        let r = black_box(record);
                        sum = sum.wrapping_add(
                            r.record_id
                                ^ r.owner_or_group
                                ^ u64::from(r.generation)
                                ^ u64::from(r.schema_version)
                                ^ u64::from(r.source_kind),
                        );
                        for x in r.payload {
                            sum = sum.wrapping_add(black_box(x));
                        }
                    }
                    page(chunk.len(), black_box(sum));
                    total = total.wrapping_add(sum);
                }
            };
        }
        match self {
            Self::W64(v) => walk!(v),
            Self::W128(v) => walk!(v),
            Self::W256(v) => walk!(v),
            Self::PathRefs(v) => {
                for chunk in v.chunks(PAGE) {
                    let sum = chunk.iter().map(|x| u64::from(black_box(*x))).sum();
                    page(chunk.len(), sum);
                    total = total.wrapping_add(sum);
                }
            }
        }
        black_box(total)
    }
}
struct DomainTable {
    name: &'static str,
    width: usize,
    owners: usize,
    kind: u16,
    table: Table,
}
fn tables() -> Result<Vec<DomainTable>, String> {
    // Versioned candidate widths: base definition plus R03 M-* rows. Native
    // types are charged separately, and each stub explicitly disclaims behavior.
    let specs = [
        ("maps", 8, 128, 8),
        ("households", 22800, 256, 8),
        ("housing", 24000, 256, 7000),
        ("visitors", 1000, 256, 8),
        ("operators", 2500, 128, 1980),
        ("entities", 1980, 256, 8),
        ("equity_links", 12000, 64, 1980),
        ("meter", 160000, 128, 7000),
        ("general_contracts", 3000, 128, 1980),
        ("od", 14800, 128, 8),
        ("path_refs", 900000, 4, 14800),
        ("cargo_batches", 8500, 128, 1980),
        ("passenger_batches", 40000, 128, 22800),
        ("bus_train_vehicles", 2000, 128, 2500),
        ("ship_aircraft_vehicles", 500, 128, 2500),
        ("car_vehicles", 30000, 64, 22800),
        ("matters", 120, 256, 8),
        ("team_members", 32, 128, 8),
        ("analysis_tasks", 32, 256, 32),
        ("models", 1, 128, 1),
        ("preview", 1, 128, 1),
        ("hosts", 7000, 64, 8),
        ("teaching_batches", 600, 128, 7000),
        ("current_services", 1500, 128, 7000),
        ("cases", 36, 256, 8),
        ("bills", 16, 256, 8),
    ];
    let acceptance: Value = serde_json::from_slice(ACCEPTANCE).map_err(err)?;
    let mut result = Vec::with_capacity(specs.len());
    for (i, (name, count, width, owners)) in specs.into_iter().enumerate() {
        let expected = acceptance["l1"]["counts"]
            .get(name)
            .or_else(|| acceptance["l1"]["additional_counts"].get(name))
            .and_then(Value::as_u64)
            .ok_or("unknown_stub_count")?;
        if expected != count as u64 {
            return Err(format!("frozen_count:{name}"));
        }
        result.push(DomainTable {
            name,
            width,
            owners,
            kind: i as u16 + 1,
            table: Table::new(count, width, i as u16 + 1, owners)?,
        });
    }
    // Population is a sum on 22800 household records, not 57000 additional actors.
    if let Table::W256(v) = &mut result[1].table {
        for (i, r) in v.iter_mut().enumerate() {
            r.payload[0] = 2 + u64::from(i < 11400);
        }
    }
    Ok(result)
}
#[derive(Clone)]
struct PressurePage {
    domain: u8,
    bytes: Arc<Vec<u8>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEntry {
    #[serde(deserialize_with = "restore_index::bounded_text")]
    name: String,
    bytes: u64,
    #[serde(deserialize_with = "restore_index::bounded_text")]
    sha256: String,
}
struct HashWriter<W> {
    inner: W,
    digest: sha2::Sha256,
    bytes: u64,
}
impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        let n = self.inner.write(b)?;
        self.digest.update(&b[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
struct HashReader<R> {
    inner: R,
    digest: sha2::Sha256,
    bytes: u64,
}
impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        let n = self.inner.read(buf)?;
        self.digest.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}
fn read_verified<T>(
    path: &Path,
    entry: &FileEntry,
    decode: impl FnOnce(&mut dyn Read) -> Result<T, String>,
) -> Result<T, String> {
    let mut backing = vec![0u8; MIB];
    read_verified_buffered(path, entry, &mut backing, decode)
}
fn read_verified_buffered<T>(
    path: &Path,
    entry: &FileEntry,
    backing: &mut [u8],
    decode: impl FnOnce(&mut dyn Read) -> Result<T, String>,
) -> Result<T, String> {
    use sha2::Digest;
    let hashed = HashReader {
        inner: File::open(path).map_err(err)?,
        digest: sha2::Sha256::new(),
        bytes: 0,
    };
    let mut reader = reusable_io::Reader::new(hashed, backing);
    let value = decode(&mut reader)?;
    let mut trailing = [0u8; 1];
    if reader.read(&mut trailing).map_err(err)? != 0 {
        return Err("layout_restore_trailing_bytes".into());
    }
    let hashed = reader.into_inner();
    if hashed.bytes != entry.bytes || format!("{:x}", hashed.digest.finalize()) != entry.sha256 {
        return Err(format!("layout_restore_checksum:{}", entry.name));
    }
    Ok(value)
}
fn write_file(
    dir: &Path,
    name: &str,
    backing: &mut [u8],
    write: impl FnOnce(&mut HashWriter<reusable_io::Writer<'_, File>>) -> Result<(), String>,
) -> Result<FileEntry, String> {
    use sha2::Digest;
    let file = File::create(dir.join(name)).map_err(err)?;
    let mut writer = HashWriter {
        inner: reusable_io::Writer::new(file, backing),
        digest: sha2::Sha256::new(),
        bytes: 0,
    };
    write(&mut writer)?;
    writer.flush().map_err(err)?;
    writer.inner.get_ref().sync_all().map_err(err)?;
    Ok(FileEntry {
        name: name.into(),
        bytes: writer.bytes,
        sha256: format!("{:x}", writer.digest.finalize()),
    })
}
fn free_bytes(dir: &Path) -> Result<u64, String> {
    // Native statvfs through the existing safe fs2 API: avoid unobservable,
    // short-lived df subprocesses on every pressure/snapshot safety check.
    fs2::available_space(dir).map_err(err)
}
struct Reserve {
    pages: Vec<Vec<u8>>,
    loan_pages: usize,
    decode_loan_pages: usize,
}
impl Reserve {
    /// Transfer an already initialized MiB allocation into a dirty COW page.
    /// Avoid releasing a usable backing only to request another equal-sized
    /// allocation; snapshot decoding and repayment remain separately checked.
    fn copy_page(&mut self, original: &[u8]) -> Result<Vec<u8>, String> {
        if original.len() != MIB {
            return Err("layout_cow_page_size".into());
        }
        let mut page = self.pages.pop().ok_or("layout_cow_backpressure")?;
        page.copy_from_slice(original);
        self.loan_pages += 1;
        Ok(page)
    }
    fn begin_decode(&mut self) -> Result<(), String> {
        if self.decode_loan_pages != 0 {
            return Err("layout_decode_busy".into());
        }
        self.decode_loan_pages = DECODE_BYTES / MIB;
        self.loan_pages += self.decode_loan_pages;
        Ok(())
    }
    fn end_decode(&mut self) -> Result<(), String> {
        if self.decode_loan_pages != DECODE_BYTES / MIB || self.loan_pages < self.decode_loan_pages
        {
            return Err("layout_decode_borrow".into());
        }
        self.loan_pages -= self.decode_loan_pages;
        self.decode_loan_pages = 0;
        Ok(())
    }
    fn reclaim_cow(&mut self, pages: Vec<PressurePage>, expected: usize) -> Result<(), String> {
        if self.decode_loan_pages != 0 || self.loan_pages != expected {
            return Err("layout_cow_outstanding_decode_or_count".into());
        }
        let unique = pages
            .iter()
            .filter(|p| Arc::strong_count(&p.bytes) == 1)
            .count();
        if unique != expected || self.pages.len() + expected != COW_PAGES {
            return Err("layout_cow_return_count".into());
        }
        for page in &pages {
            if Arc::strong_count(&page.bytes) == 1
                && (page.bytes.len() != MIB || page.bytes.capacity() != MIB)
            {
                return Err("layout_cow_return_shape".into());
            }
        }
        // Only called by the owning main thread AFTER join, with no scan in
        // flight. Shared pages remain world-owned; unique pages are actual old
        // backing, not freshly allocated replacement placeholders.
        for page in pages {
            if let Ok(mut bytes) = Arc::try_unwrap(page.bytes) {
                bytes.fill(0xa5);
                self.pages.push(bytes);
                self.loan_pages -= 1;
            }
        }
        if self.loan_pages != 0 {
            return Err("layout_cow_return_incomplete".into());
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Scan {
    pub iteration: u64,
    pub native_records: usize,
    pub stub_records: usize,
    pub touched_pressure_bytes: usize,
    pub decode_workspace_scanned_bytes: usize,
    pub decode_workspace_borrowed_bytes: usize,
    pub dirty_bytes: usize,
    pub checksum: u64,
    pub wall_seconds: f64,
    pub cow_pages: usize,
}
pub struct Load {
    native: Arc<NativePressure>,
    tables: Arc<Vec<DomainTable>>,
    pages: Vec<PressurePage>,
    r07: Option<r07::Backing>,
    reserve: Arc<Mutex<Reserve>>,
    iterations: u64,
    cow_pages: usize,
    normal_bytes: usize,
    external_charge: usize,
    metadata_charge: usize,
    job: Option<JoinHandle<SnapshotCompletion>>,
    snapshot_buffers: Arc<Mutex<SnapshotBuffers>>,
    decode_workspace: Arc<Mutex<DecodeWorkspace>>,
    root: PathBuf,
    generation: u64,
    _root_lock: File,
}
fn r07_logical(normal: usize, external: usize, metadata: usize) -> Result<usize, String> {
    let assigned = [60usize, 30, 85, 25, 40, 25]
        .into_iter()
        .try_fold(0usize, |sum, pct| {
            sum.checked_add(normal.checked_mul(pct).ok_or("r07_budget_overflow")? / 310)
                .ok_or("r07_budget_overflow")
        })?;
    normal
        .checked_sub(assigned)
        .and_then(|n| n.checked_sub(WORK_PAGES * MIB))
        .and_then(|n| n.checked_sub(metadata))
        .and_then(|n| n.checked_sub(external))
        .ok_or_else(|| "r07_budget_underflow".into())
}
fn activate_layout_region(
    normal_bytes: usize,
    external_charge: usize,
    independent: usize,
) -> Result<(), String> {
    let grant = normal_bytes
        .checked_sub(external_charge)
        .and_then(|n| n.checked_sub(independent))
        .ok_or("layout_allocator_budget")?;
    crate::heap_meter::activate_role(grant).map(|_| ())
}
impl Load {
    /// Owns one process-lifetime L1 region. A second Load or another active role
    /// in this process is rejected; callers use a separate CLI process.
    pub fn full(root: &Path, external_owned_charge_bytes: usize) -> Result<Self, String> {
        Self::construct(root, NORMAL_BYTES, external_owned_charge_bytes)
    }
    fn construct(root: &Path, normal_bytes: usize, external_charge: usize) -> Result<Self, String> {
        // Shared reservation includes model/IPC components, not only diagnostics.
        // The independent diagnostic hot-data cap remains 8MiB.
        if external_charge > 64 * MIB {
            return Err("layout_external_reservation_limit".into());
        }
        fs::create_dir(root).map_err(err)?;
        let root_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.join("layout.lock"))
            .map_err(err)?;
        fs2::FileExt::try_lock_exclusive(&root_lock).map_err(err)?;
        // This process owns the entire actual L1 working set. The original
        // metadata and 128MiB reserve remain inside this SAME fixed region;
        // temporary decode/dirty pages reuse freed holes instead of leaving a
        // second System allocation resident. A second Load/other active role
        // is rejected by the allocator; there is no fallback or larger grant.
        let independent_logical = r07_logical(normal_bytes, external_charge, 4 * MIB)?;
        activate_layout_region(
            normal_bytes,
            external_charge,
            r07::mapped_capacity(independent_logical),
        )?;
        let snapshot_buffers = Arc::new(Mutex::new(SnapshotBuffers::new()));
        let native = Arc::new(NativePressure::jprod()?);
        let tables = Arc::new(tables()?);
        let decode_workspace = Arc::new(Mutex::new(DecodeWorkspace::new(&tables)?));
        let native_bytes = native.manifest().owned_capacity_bytes;
        let table_bytes = tables.iter().map(|t| t.table.owned()).sum::<usize>();
        // Headers, 1MiB IO buffer, manifest, Arc controls and maximum clone-index
        // allocation are precharged. Runtime/observer state supplied by supervisor
        // shares this budget instead of being added to a 3.10GiB padding block.
        let metadata_charge = 4 * MIB;
        let minimum =
            native_bytes + table_bytes + WORK_PAGES * MIB + metadata_charge + external_charge;
        let padding = normal_bytes
            .checked_sub(minimum)
            .ok_or("layout_budget_too_small")?;
        let mut pages = Vec::with_capacity(padding.div_ceil(MIB) + 7);
        let mut assigned = 0usize;
        for (domain, percent) in [60usize, 30, 85, 25, 40, 25, 45].into_iter().enumerate() {
            let quota = if domain == 6 {
                normal_bytes - assigned
            } else {
                normal_bytes * percent / 310
            };
            assigned += quota;
            let charge = if domain == 0 {
                native_bytes + table_bytes
            } else if domain == 6 {
                WORK_PAGES * MIB + metadata_charge + external_charge
            } else {
                0
            };
            let mut remaining = quota.checked_sub(charge).ok_or("layout_domain_budget")?;
            if domain == 6 {
                if remaining != independent_logical {
                    return Err("r07_domain_budget".into());
                }
                remaining %= r07::QUANTUM;
            }
            while remaining > 0 {
                let n = remaining.min(MIB);
                pages.push(PressurePage {
                    domain: domain as u8,
                    bytes: Arc::new(pattern(n, pages.len() as u64 + 1)),
                });
                remaining -= n;
            }
        }
        let reserve = Reserve {
            pages: (0..COW_PAGES).map(|_| vec![0xa5; MIB]).collect(),
            loan_pages: 0,
            decode_loan_pages: 0,
        };
        Ok(Self {
            native,
            tables,
            pages,
            r07: Some(r07::Backing::full(independent_logical)?),
            reserve: Arc::new(Mutex::new(reserve)),
            iterations: 0,
            cow_pages: 0,
            normal_bytes,
            external_charge,
            metadata_charge,
            snapshot_buffers,
            decode_workspace,
            job: None,
            root: root.into(),
            generation: 0,
            _root_lock: root_lock,
        })
    }
    pub fn manifest(&self) -> Value {
        let work = self
            .decode_workspace
            .try_lock()
            .map(|w| w.manifest())
            .unwrap_or_else(|_| json!({"state":"busy_or_poisoned","owned_bytes":DECODE_BYTES}));
        let domains:Vec<_>=self.tables.iter().map(|t|json!({"domain":t.name,"source_kind":t.kind,"source":"layout_stub_not_functional","schema_version":1,"count":t.table.count(),"record_bytes":t.width,"owned_capacity_bytes":t.table.owned(),"owner_bound":t.owners,"index":"record_id_1_based_contiguous; path_refs_zero_based_OD_index"})).collect();
        let mut by_domain = [0usize; 7];
        for p in &self.pages {
            by_domain[p.domain as usize] += p.bytes.capacity();
        }
        if let Some(r07) = &self.r07 {
            by_domain[6] = r07.state().logical_bytes;
        }
        json!({"schema":if self.r07.is_some(){"mycity-l1-layout-v2"}else{"mycity-l1-layout-v1"},"source_sha256":env!("MYCITY_SOURCE_SHA256"),"acceptance_sha256":mycity_core::sha256(ACCEPTANCE),"native":self.native.manifest(),"stubs":domains,
            "population":{"total":57000,"representation":"22800 household stub members; 11400*3 + 11400*2","actors":0},
            "padding_payload_bytes":by_domain,"padding_payload_total_bytes":by_domain.iter().sum::<usize>(),"pressure_page_bytes":MIB,"mutation_iteration":self.iterations,"pressure_pattern":if self.r07.is_some(){"splitmix64-init-v1; affine-u64-nonr07-dirty-v1"}else{"splitmix64-init-v1; affine-u64-dirty-v1"},
            "r07":self.r07.as_ref().map(r07::Backing::state),"r07_generator":self.r07.as_ref().map(|_|r07::Backing::pattern()),
            "r07_mapping_control_bytes":self.r07.as_ref().map_or(0,r07::Backing::control_bytes),
            "cow_reserve_bytes":WORK_PAGES*MIB,"cow_concurrent_pages":COW_PAGES,"decode_workspace":work,"metadata_and_io_charge_bytes":self.metadata_charge,"external_owned_charge_bytes":self.external_charge,"normal_joint_budget_bytes":self.normal_bytes,
            "solver_preview":"192MiB preview and 32MiB compute are pressure pages within solver share; no algorithms/GPU/UI",
            "layout_not_business":true,"full_M0":"not_accepted","memory_16gb_gate":"not_run"})
    }
    pub fn scan(&mut self, mut page: impl FnMut(&'static str, usize, u64)) -> Result<Scan, String> {
        let started = Instant::now();
        self.refresh_r07()?;
        let mut native_records = 0;
        let mut last = Instant::now();
        let mut checksum = self.native.scan(|n, _| {
            native_records += n;
            let elapsed = last.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            page("L1_native_record_index_page", n, elapsed);
            last = Instant::now();
        })?;
        let mut stub_records = 0;
        for table in self.tables.iter() {
            let mut last = Instant::now();
            checksum = checksum.wrapping_add(table.table.scan(|n, _| {
                stub_records += n;
                page(
                    "L1_stub_record_index_page",
                    n,
                    last.elapsed().as_nanos().min(u64::MAX as u128) as u64,
                );
                last = Instant::now();
            }));
        }
        let mut touched = 0usize;
        for p in &self.pages {
            for chunk in p.bytes.chunks(4096) {
                checksum = checksum.wrapping_add(u64::from(black_box(chunk[0])));
            }
            touched += p.bytes.len();
        }
        if let Some(r07) = &self.r07 {
            let (bytes, value) = r07.touch();
            touched += bytes;
            checksum = checksum.wrapping_add(value);
        }
        {
            let reserve = self.reserve.lock().map_err(err)?;
            for p in &reserve.pages {
                for chunk in p.chunks(4096) {
                    checksum = checksum.wrapping_add(u64::from(black_box(chunk[0])));
                }
                touched += p.len();
            }
        }
        let (decode_scanned, decode_borrowed) = match self.decode_workspace.try_lock() {
            Ok(mut workspace) => {
                checksum = checksum.wrapping_add(workspace.touch()?);
                touched += workspace.owned_bytes();
                (workspace.owned_bytes(), 0)
            }
            Err(TryLockError::WouldBlock) => (0, DECODE_BYTES),
            Err(TryLockError::Poisoned(_)) => return Err("layout_decode_workspace_poisoned".into()),
        };
        // Exactly one full MiB, avoiding the possible short final pressure page.
        let mut index = self.iterations as usize % self.pages.len();
        while self.pages[index].bytes.len() != MIB {
            index = (index + 1) % self.pages.len();
        }
        if Arc::strong_count(&self.pages[index].bytes) > 1 {
            let page = self
                .reserve
                .lock()
                .map_err(err)?
                .copy_page(&self.pages[index].bytes)?;
            self.pages[index].bytes = Arc::new(page);
            self.cow_pages += 1;
        }
        let dirty = Arc::make_mut(&mut self.pages[index].bytes);
        let value = (self.iterations % 251 + 1) as u8;
        // Distinct deterministic dirty bytes, rather than compressible uniform assets.
        let mut state = self.iterations + 0x4d5943495459;
        for chunk in dirty.chunks_mut(8) {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let x = state.wrapping_mul(0xd1342543de82ef95);
            chunk.copy_from_slice(&x.to_le_bytes()[..chunk.len()]);
        }
        self.iterations += 1;
        Ok(Scan {
            iteration: self.iterations,
            native_records,
            stub_records,
            touched_pressure_bytes: touched,
            decode_workspace_scanned_bytes: decode_scanned,
            decode_workspace_borrowed_bytes: decode_borrowed,
            dirty_bytes: dirty.len(),
            checksum: checksum.wrapping_add(u64::from(value)),
            wall_seconds: started.elapsed().as_secs_f64(),
            cow_pages: self.cow_pages,
        })
    }
    fn refresh_r07(&mut self) -> Result<(), String> {
        let Some(backing) = &mut self.r07 else {
            return Ok(());
        };
        let decision = crate::r07_runtime::current(backing.state().reading_sequence)?;
        let state = backing.apply(decision.permitted_bytes, decision.sequence)?;
        let owner = crate::native_observer::capture(std::process::id(), None, Instant::now())
            .ok()
            .map(|s| s.identity);
        let applied = r07::Applied {
            source: crate::control_data::Text::new(env!("MYCITY_SOURCE_SHA256"))?,
            owner,
            state,
            permitted_bytes: decision.permitted_bytes,
            reason: crate::control_data::Text::new(decision.reason)?,
        };
        let parent = self.root.parent().ok_or("r07_state_parent")?;
        let tmp = parent.join("r07-state.tmp");
        let mut file = File::create(&tmp).map_err(err)?;
        serde_json::to_writer(&mut file, &applied).map_err(err)?;
        file.write_all(b"\n").map_err(err)?;
        file.sync_all().map_err(err)?;
        fs::rename(tmp, parent.join("r07-state.json")).map_err(err)
    }
    pub fn start_snapshot(&mut self) -> Result<Value, String> {
        if self.job.is_some() {
            return Err("layout_snapshot_busy".into());
        }
        // Includes full non-sparse candidate, native JSON encoding expansion and
        // per-domain stubs; retain two complete restore points until publication.
        let worst_candidate = self.normal_bytes as u64 + 512 * 1024 * 1024;
        let free = free_bytes(&self.root)?;
        if free < worst_candidate + FREE_FLOOR {
            return Err(format!("layout_disk_safety:available={free}:candidate={worst_candidate}:floor={FREE_FLOOR}"));
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or("layout_generation_overflow")?;
        if self.root.join(format!("snapshot-{generation:06}")).exists() {
            return Err("layout_stale_restore_point".into());
        }
        self.generation = generation;
        let dir = self.root.join(format!("snapshot-{generation:06}.pending"));
        fs::create_dir(&dir).map_err(err)?;
        let pages = self.pages.clone();
        let native = self.native.clone();
        let tables = self.tables.clone();
        let reserve = self.reserve.clone();
        let buffers = self.snapshot_buffers.clone();
        let decode_workspace = self.decode_workspace.clone();
        let root = self.root.clone();
        let r07 = self.r07.as_ref().map(r07::Backing::snapshot);
        let manifest = self.manifest();
        self.job = Some(
            thread::Builder::new()
                .name("layout-snapshot".into())
                .stack_size(2 * MIB)
                .spawn(move || {
                    Snapshot {
                        root,
                        dir,
                        generation,
                        pages,
                        r07,
                        native,
                        tables,
                        reserve,
                        buffers,
                        decode_workspace,
                        manifest,
                    }
                    .run()
                })
                .map_err(err)?,
        );
        Ok(
            json!({"generation":generation,"available_bytes":free,"required_candidate_upper_bytes":worst_candidate,"minimum_free_bytes":FREE_FLOOR}),
        )
    }
    fn complete_snapshot(&mut self, completed: SnapshotCompletion) -> Result<Value, String> {
        let reclaimed = self.cow_pages;
        let returned = self
            .reserve
            .lock()
            .map_err(err)?
            .reclaim_cow(completed.pages, reclaimed);
        if let Err(error) = returned {
            return Err(format!("{error}:snapshot_outcome={:?}", completed.result));
        }
        self.cow_pages = 0;
        completed.result.map(|mut result| {
            result["cow_backings_returned_after_join"] = json!(reclaimed);
            result
        })
    }
    pub fn poll_snapshot(&mut self) -> Result<Option<Value>, String> {
        if self.job.as_ref().is_some_and(JoinHandle::is_finished) {
            let job = self.job.take().ok_or("layout_job")?;
            let result = job.join().map_err(|_| "layout_snapshot_panicked")?;
            return self.complete_snapshot(result).map(Some);
        }
        Ok(None)
    }
    pub fn finish_snapshot(&mut self) -> Result<Option<Value>, String> {
        if let Some(job) = self.job.take() {
            let result = job.join().map_err(|_| "layout_snapshot_panicked")?;
            return self.complete_snapshot(result).map(Some);
        }
        Ok(None)
    }
}
impl Drop for Load {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.join();
        }
    }
}
struct SnapshotBuffers {
    io: Vec<u8>,
    compare: Vec<u8>,
}
impl SnapshotBuffers {
    fn new() -> Self {
        Self {
            io: vec![0; MIB],
            compare: vec![0; MIB],
        }
    }
}
struct SnapshotCompletion {
    result: Result<Value, String>,
    pages: Vec<PressurePage>,
}
// Fixed 128-byte little-endian records, written before the operation. Fields:
// magic/version, sequence, generation, phase, file-or-page-index, request bytes,
// allocator requested/used/free/failures, reserve available/total loans/decode
// loans, clock elapsed, reserved. No heap formatting or allocation on this path.
struct AllocationTrace {
    file: File,
    sequence: u64,
    generation: u64,
}
impl AllocationTrace {
    fn mark(
        &mut self,
        phase: u64,
        index: u64,
        request: usize,
        reserve: &Reserve,
    ) -> Result<(), String> {
        let a = crate::heap_meter::arena_snapshot();
        let fields = [
            0x4d59435452414301,
            self.sequence,
            self.generation,
            phase,
            index,
            request as u64,
            a.current_requested_bytes as u64,
            a.arena_used_bytes as u64,
            a.arena_free_bytes as u64,
            a.allocation_failures,
            reserve.pages.len() as u64,
            reserve.loan_pages as u64,
            reserve.decode_loan_pages as u64,
            crate::heap_meter::clock_reading().elapsed_ns,
            0,
            0,
        ];
        let mut bytes = [0u8; 128];
        for (chunk, value) in bytes.chunks_exact_mut(8).zip(fields) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        self.file.write_all(&bytes).map_err(err)?;
        self.file.sync_data().map_err(err)?;
        self.sequence += 1;
        Ok(())
    }
}
struct Snapshot {
    root: PathBuf,
    dir: PathBuf,
    generation: u64,
    pages: Vec<PressurePage>,
    r07: Option<r07::Snapshot>,
    native: Arc<NativePressure>,
    tables: Arc<Vec<DomainTable>>,
    reserve: Arc<Mutex<Reserve>>,
    manifest: Value,
    buffers: Arc<Mutex<SnapshotBuffers>>,
    decode_workspace: Arc<Mutex<DecodeWorkspace>>,
}
#[derive(Serialize)]
struct SnapshotIndex<'a> {
    schema: &'static str,
    generation: u64,
    manifest: &'a Value,
    files: &'a [FileEntry],
}
impl Snapshot {
    fn run(self) -> SnapshotCompletion {
        let result = self.run_inner();
        SnapshotCompletion {
            result,
            pages: self.pages,
        }
    }
    fn run_inner(&self) -> Result<Value, String> {
        let Self {
            root,
            dir,
            generation,
            pages,
            r07,
            native,
            tables,
            reserve,
            manifest,
            buffers,
            decode_workspace,
        } = self;
        if let Some(r07) = r07 {
            if manifest["r07"] != serde_json::to_value(r07.state).map_err(err)?
                || manifest["r07_generator"] != r07::Backing::pattern()
            {
                return Err("r07_snapshot_generation_mismatch".into());
            }
        }
        let mut buffers = buffers.lock().map_err(err)?;
        let SnapshotBuffers { io, compare } = &mut *buffers;
        let mut trace = AllocationTrace {
            file: File::create_new(dir.join("allocation-phases.bin")).map_err(err)?,
            sequence: 0,
            generation: *generation,
        };
        let start = Instant::now();
        let mut entries = Vec::with_capacity(tables.len() + 3);
        trace.mark(1, 0, 0, &*reserve.lock().map_err(err)?)?;
        entries.push(write_file(dir, "native.json", io, |w| {
            native.write_checkpoint(w)
        })?);
        for (i, table) in tables.iter().enumerate() {
            trace.mark(1, i as u64 + 1, 0, &*reserve.lock().map_err(err)?)?;
            entries.push(write_file(dir, &format!("{}.json", table.name), io, |w| {
                serde_json::to_writer(w, &table.table).map_err(err)
            })?);
        }
        trace.mark(1, 27, 0, &*reserve.lock().map_err(err)?)?;
        entries.push(write_file(dir, "pressure.bin", io, |w| {
            for page in pages {
                w.write_all(&page.bytes).map_err(err)?;
            }
            if let Some(r07) = r07 {
                r07.write(w)?;
            }
            Ok(())
        })?);
        // The complete 128MiB reserved work-page image is physically written too.
        // Stream one existing placeholder repeatedly, never allocate another world.
        trace.mark(1, 28, 0, &*reserve.lock().map_err(err)?)?;
        entries.push(write_file(dir, "work-reserve.bin", io, |w| {
            let spare = reserve.lock().map_err(err)?;
            let page = spare.pages.first().ok_or("layout_no_spare")?;
            for _ in 0..WORK_PAGES {
                w.write_all(page).map_err(err)?;
            }
            Ok(())
        })?);
        let write_fsync_seconds = start.elapsed().as_secs_f64();
        // Borrow the same initialized 64MiB typed workspace on every generation.
        // No page is dropped or reallocated; the other 64MiB remains for COW.
        let mut decoded = decode_workspace.lock().map_err(err)?;
        decoded.valid = false;
        trace.mark(2, 0, DECODE_BYTES, &*reserve.lock().map_err(err)?)?;
        reserve.lock().map_err(err)?.begin_decode()?;
        let restore = (|| {
            let entry = |name: &str| {
                entries
                    .iter()
                    .find(|e| e.name == name)
                    .ok_or_else(|| format!("layout_snapshot_entry:{name}"))
            };
            trace.mark(3, 0, 0, &*reserve.lock().map_err(err)?)?;
            let native_restore_bound = read_verified_buffered(
                &dir.join("native.json"),
                entry("native.json")?,
                io,
                |reader| decoded.native.restore_checkpoint_into(reader),
            )?;
            trace.mark(4, 0, 0, &*reserve.lock().map_err(err)?)?;
            let original_hash = write_file(dir, "native-roundtrip.json", io, |w| {
                decoded.native.write_checkpoint(w)
            })?;
            if original_hash.sha256 != entries[0].sha256 || original_hash.bytes != entries[0].bytes
            {
                return Err("layout_native_restore_changed".into());
            }
            let mut bound_file =
                File::create(dir.join("native-restore-bound.json")).map_err(err)?;
            serde_json::to_writer_pretty(&mut bound_file, &native_restore_bound).map_err(err)?;
            bound_file.sync_all().map_err(err)?;
            fs::remove_file(dir.join("native-roundtrip.json")).map_err(err)?;
            for (i, table) in tables.iter().enumerate() {
                trace.mark(5, i as u64 + 1, 0, &*reserve.lock().map_err(err)?)?;
                let name = format!("{}.json", table.name);
                read_verified_buffered(&dir.join(&name), entry(&name)?, io, |reader| {
                    decoded.decode_table(reader, &table.table)
                })?;
            }
            trace.mark(6, 27, 0, &*reserve.lock().map_err(err)?)?;
            read_verified_buffered(
                &dir.join("pressure.bin"),
                entry("pressure.bin")?,
                io,
                |reader| {
                    let buf = &mut *compare;
                    for page in pages {
                        reader
                            .read_exact(&mut buf[..page.bytes.len()])
                            .map_err(err)?;
                        if buf[..page.bytes.len()] != page.bytes[..] {
                            return Err("layout_page_restore_changed".into());
                        }
                    }
                    if let Some(r07) = r07 {
                        r07.verify(reader, buf)?;
                    }
                    Ok(())
                },
            )?;
            trace.mark(7, 28, 0, &*reserve.lock().map_err(err)?)?;
            read_verified_buffered(
                &dir.join("work-reserve.bin"),
                entry("work-reserve.bin")?,
                io,
                |reader| {
                    let buf = &mut *compare;
                    for _ in 0..WORK_PAGES {
                        reader.read_exact(buf).map_err(err)?;
                        if buf.iter().any(|b| *b != 0xa5) {
                            return Err("layout_work_restore_changed".into());
                        }
                    }
                    Ok(())
                },
            )?;
            // Keep this bounded result on the stack; the persistent workspace
            // remains allocated across generations, including failures.
            Ok::<_, String>(native_restore_bound)
        })();
        decoded.valid = restore.is_ok();
        drop(decoded);
        reserve.lock().map_err(err)?.end_decode()?;
        trace.mark(9, 0, 0, &*reserve.lock().map_err(err)?)?;
        let native_restore_bound = restore?;
        // Serialize borrowed metadata rather than cloning the complete manifest
        // and all entries into a second Value tree during publication.
        let index = SnapshotIndex {
            schema: if r07.is_some() {
                "mycity-layout-snapshot-v3"
            } else {
                "mycity-layout-snapshot-v2"
            },
            generation: *generation,
            manifest,
            files: &entries,
        };
        let mut f = File::create(dir.join("index.json")).map_err(err)?;
        serde_json::to_writer_pretty(&mut f, &index).map_err(err)?;
        f.sync_all().map_err(err)?;
        File::open(dir).map_err(err)?.sync_all().map_err(err)?;
        let durable = root.join(format!("snapshot-{generation:06}"));
        fs::rename(dir, &durable).map_err(err)?;
        File::open(root).map_err(err)?.sync_all().map_err(err)?;
        // Only after full new restore validation and directory durability, retain two.
        if *generation > 2 {
            fs::remove_dir_all(root.join(format!("snapshot-{:06}", generation - 2)))
                .map_err(err)?;
            File::open(root).map_err(err)?.sync_all().map_err(err)?;
        }
        Ok(
            json!({"generation":generation,"write_fsync_seconds":write_fsync_seconds,"durable_complete_seconds":start.elapsed().as_secs_f64(),"files":entries,"native_restore_bound":native_restore_bound,"complete_record_restore":"passed","full_pressure_file_readback":"passed","remaining_free_bytes":free_bytes(root)?}),
        )
    }
}

// Retain fixed-width summaries, not every 29-file JSON tree for the 8h run.
// Full per-file evidence is streamed to events and retained snapshot indices.
#[derive(Serialize)]
struct SnapshotSummary {
    generation: u64,
    durable_complete_seconds: f64,
    write_fsync_seconds: f64,
    file_count: usize,
    file_bytes: u64,
    record_restore_passed: bool,
    pressure_readback_passed: bool,
}
impl SnapshotSummary {
    fn from_result(value: &Value) -> Result<Self, String> {
        let files = value["files"].as_array().ok_or("snapshot_summary_files")?;
        Ok(Self {
            generation: value["generation"]
                .as_u64()
                .ok_or("snapshot_summary_generation")?,
            durable_complete_seconds: value["durable_complete_seconds"]
                .as_f64()
                .ok_or("snapshot_summary_duration")?,
            write_fsync_seconds: value["write_fsync_seconds"]
                .as_f64()
                .ok_or("snapshot_summary_fsync")?,
            file_count: files.len(),
            file_bytes: files.iter().try_fold(0u64, |n, f| {
                n.checked_add(f["bytes"].as_u64().ok_or("snapshot_summary_bytes")?)
                    .ok_or("snapshot_summary_overflow")
            })?,
            record_restore_passed: value["complete_record_restore"] == "passed",
            pressure_readback_passed: value["full_pressure_file_readback"] == "passed",
        })
    }
}
/// Diagnostic or frozen standalone L1 run. Duration<2100s cannot pass steady gates.
pub fn probe(dir: &Path, wall_seconds: u64, snapshot_seconds: u64) -> Result<Value, String> {
    probe_charged(dir, wall_seconds, snapshot_seconds, None)
}
/// Joint diagnostic reservation replaces padding; it never removes native/stub records.
/// This is a budget reservation, not proof of complete external allocation accounting.
pub fn probe_charged(
    dir: &Path,
    wall_seconds: u64,
    snapshot_seconds: u64,
    external_charge: Option<usize>,
) -> Result<Value, String> {
    probe_schedule(dir, wall_seconds, snapshot_seconds, external_charge, false)
}

fn read_owner_stop(mut reader: impl Read) -> Result<(), String> {
    let mut byte = [0];
    reader.read_exact(&mut byte).map_err(err)?;
    if byte == [b'S'] {
        Ok(())
    } else {
        Err("layout_owner_command".into())
    }
}
fn read_owner_commands(mut reader: impl Read, marker: &Path) -> Result<(), String> {
    loop {
        let mut byte = [0];
        reader.read_exact(&mut byte).map_err(err)?;
        match byte[0] {
            b'S' => return read_owner_stop(&byte[..]),
            b'C' => {
                let clock = crate::heap_meter::clock_reading();
                let mut file = File::create_new(marker).map_err(err)?;
                serde_json::to_writer(&mut file, &json!({"clock":clock})).map_err(err)?;
                file.sync_all().map_err(err)?;
            }
            _ => return Err("layout_owner_command".into()),
        }
    }
}
fn published_before(deadline: u64, end: Option<std::time::Duration>) -> bool {
    end.is_none_or(|end| {
        std::time::Duration::from_secs(deadline) < end.max(std::time::Duration::from_nanos(1))
    })
}

/// Parent must explicitly stop; a broken owner pipe is a failure, not success.
pub fn session(dir: &Path, minimum_seconds: u64) -> Result<Value, String> {
    if ![600, 28800].contains(&minimum_seconds) {
        return Err("layout_session_minimum".into());
    }
    probe_schedule(dir, minimum_seconds, 300, Some(64 * 1024 * 1024), true)
}

/// Same full records and pressure, owner-held through steady measurement drain.
pub fn steady_session(dir: &Path, seconds: u64) -> Result<Value, String> {
    if ![15, 2100].contains(&seconds) {
        return Err("steady_layout_duration".into());
    }
    probe_schedule(dir, seconds, 300, Some(64 * 1024 * 1024), true)
}

fn probe_schedule(
    dir: &Path,
    wall_seconds: u64,
    snapshot_seconds: u64,
    external_charge: Option<usize>,
    owner_session: bool,
) -> Result<Value, String> {
    use crate::scale_metrics::Metrics;
    use std::time::Duration;
    if wall_seconds > 28800
        || snapshot_seconds == 0
        || (wall_seconds >= 2100 && snapshot_seconds != 300)
        || (snapshot_seconds > 0 && wall_seconds.max(1).div_ceil(snapshot_seconds) > 128)
    {
        return Err("layout_probe_schedule".into());
    }
    fs::create_dir(dir).map_err(err)?;
    let mut metrics = Metrics::create(&dir.join("compute-durations.bin"))?;
    let initial = Instant::now();
    // Both active L1 histograms may allocate lazily after construction.
    let metric_reserve = metrics.owned_bytes() + 2 * 40001 * std::mem::size_of::<u32>();
    let charge = external_charge.unwrap_or(metric_reserve);
    if charge < metric_reserve {
        return Err("layout_external_diagnostics_reservation".into());
    }
    let mut load = Load::full(&dir.join("snapshots"), charge)?;
    let init_seconds = initial.elapsed().as_secs_f64();
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&load.manifest()).map_err(err)?,
    )
    .map_err(err)?;
    let mut events =
        BufWriter::with_capacity(65536, File::create(dir.join("events.jsonl")).map_err(err)?);
    let started = Instant::now();
    let owner = if owner_session {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let clock_marker = dir.join("clock-sync.json");
        std::thread::Builder::new()
            .name("layout-owner".into())
            .stack_size(64 * 1024)
            .spawn(move || {
                let result = read_owner_commands(std::io::stdin(), &clock_marker);
                let _ = tx.send((Instant::now(), result));
            })
            .map_err(err)?;
        Some(rx)
    } else {
        None
    };
    let mut end = (!owner_session).then_some(Duration::from_secs(wall_seconds));
    let mut next_scan = 0u64;
    let mut next_snapshot = 0u64;
    let mut snapshots = Vec::with_capacity(128);
    let mut scans = 0u64;
    let mut maximum_scan_lateness = 0f64;
    let mut maximum_snapshot_start_lateness = 0f64;
    let outcome = (|| {
        loop {
            if let Some(result) = load.poll_snapshot()? {
                serde_json::to_writer(&mut events,&json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"snapshot_durable":result})).map_err(err)?;
                events.write_all(b"\n").map_err(err)?;
                events.flush().map_err(err)?;
                snapshots.push(SnapshotSummary::from_result(&result)?);
            }
            let elapsed = started.elapsed().as_secs_f64();
            if let Some(owner) = &owner {
                if end.is_none() {
                    match owner.try_recv() {
                        Ok((received, result)) => {
                            result?;
                            let received = received.duration_since(started);
                            let cutoff = received.max(Duration::from_secs(wall_seconds));
                            serde_json::to_writer(&mut events, &json!({"owner_stop_received_seconds":received.as_secs_f64(),"publication_end_seconds":cutoff.as_secs_f64()})).map_err(err)?;
                            events.write_all(b"\n").map_err(err)?;
                            end = Some(cutoff);
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => (),
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            return Err("layout_owner_lost".into())
                        }
                    }
                }
                if elapsed >= 35000.0 {
                    return Err("layout_session_horizon".into());
                }
            }
            let published = |deadline| published_before(deadline, end);
            if end.is_some_and(|end| started.elapsed() >= end)
                && !published(next_scan)
                && !published(next_snapshot)
            {
                break;
            }
            if published(next_snapshot) && elapsed >= next_snapshot as f64 {
                let lateness = elapsed - next_snapshot as f64;
                maximum_snapshot_start_lateness = maximum_snapshot_start_lateness.max(lateness);
                let admission = load.start_snapshot()?;
                serde_json::to_writer(&mut events,&json!({"elapsed_seconds":elapsed,"snapshot_deadline_seconds":next_snapshot,"start_lateness_seconds":lateness,"snapshot_admitted":admission})).map_err(err)?;
                events.write_all(b"\n").map_err(err)?;
                next_snapshot += snapshot_seconds;
            }
            if published(next_scan) && elapsed >= next_scan as f64 {
                let lateness = elapsed - next_scan as f64;
                maximum_scan_lateness = maximum_scan_lateness.max(lateness);
                let raw_begin = metrics.written_bytes();
                let clock_begin = crate::heap_meter::clock_reading();
                let scan = load.scan(|class, _, ns| metrics.observe(class, ns))?;
                let clock_end = crate::heap_meter::clock_reading();
                let allocator_after_scan = crate::heap_meter::arena_snapshot();
                let allocator_after_scan_clock = crate::heap_meter::clock_reading();
                let snapshot_inflight = load.job.is_some();
                let raw_end = metrics.written_bytes();
                metrics.check()?;
                if metrics.owned_bytes() > charge {
                    return Err("layout_diagnostics_exceeds_charge".into());
                }
                serde_json::to_writer(&mut events,&json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"scan_deadline_seconds":next_scan,"scan_ready_age_seconds":lateness,"scan":scan,"raw_begin":raw_begin,"raw_end":raw_end,"clock_begin":clock_begin,"clock_end":clock_end,"allocator_after_scan":allocator_after_scan,"allocator_after_scan_clock":allocator_after_scan_clock,"snapshot_inflight":snapshot_inflight})).map_err(err)?;
                events.write_all(b"\n").map_err(err)?;
                events.flush().map_err(err)?;
                scans += 1;
                next_scan += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if let Some(result) = load.finish_snapshot()? {
            serde_json::to_writer(&mut events,&json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"snapshot_durable":result})).map_err(err)?;
            events.write_all(b"\n").map_err(err)?;
            snapshots.push(SnapshotSummary::from_result(&result)?);
        }
        Ok::<(), String>(())
    })();
    events.flush().map_err(err)?;
    events.get_ref().sync_all().map_err(err)?;
    let diagnostic_allocated = metrics.owned_bytes()
        + events.capacity()
        + snapshots.capacity() * size_of::<SnapshotSummary>()
        + if owner_session { 4096 } else { 0 };
    let diagnostic_charged = diagnostic_allocated + 256 * 1024;
    if diagnostic_charged > crate::diagnostic_contract::LAYOUT {
        return Err("layout_complete_diagnostics_cap".into());
    }
    let metrics = metrics.report()?;
    let manifest = load.manifest();
    let expected_scans = end
        .unwrap_or(Duration::from_secs(wall_seconds))
        .as_secs_f64()
        .ceil()
        .max(1.0) as u64;
    let report = json!({"schema":"mycity-layout-probe-v2","owner_session":owner_session,"publication_end_seconds":end.map(|x|x.as_secs_f64()),"snapshot_summaries_only":true,"diagnostics":{"allocated_owned_bytes":diagnostic_allocated,"transient_reservation_bytes":256*1024,"charged_bound_bytes":diagnostic_charged,"cap_bytes":crate::diagnostic_contract::LAYOUT,"full_snapshot_file_details":"events.jsonl and retained snapshot indices"},"source_sha256":env!("MYCITY_SOURCE_SHA256"),"requested_wall_seconds":wall_seconds,"work_wall_seconds":started.elapsed().as_secs_f64(),"initialization_seconds":init_seconds,"snapshot_interval_seconds":snapshot_seconds,"scans":scans,"expected_published_scans":expected_scans,"all_published_scans_completed":scans==expected_scans,"manifest":manifest,"snapshots":snapshots,"maximum_scan_ready_age_seconds":maximum_scan_lateness,"maximum_snapshot_start_lateness_seconds":maximum_snapshot_start_lateness,"compute":metrics,"diagnostic_outcome":outcome,"full_layout_assembled":true,"steady_gate":"not_accepted_requires_three_runs_and_group_system_measurements","eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    let mut file = File::create(dir.join("report.json")).map_err(err)?;
    serde_json::to_writer_pretty(&mut file, &report).map_err(err)?;
    file.sync_all().map_err(err)?;
    if let Err(reason) = outcome {
        return Err(format!("layout_probe_failed:{reason}"));
    }
    Ok(report)
}

impl Load {
    /// Cold, file-backed restoration of the entire layout, not only hash readback.
    /// This diagnostic codec has no authority over a gameplay World. Compatible
    /// source changes are disclosed, and native shape/capacity must match exactly.
    pub fn restore(snapshot_dir: &Path) -> Result<(Self, Value), String> {
        let started = Instant::now();
        let root = snapshot_dir.parent().ok_or("layout_snapshot_parent")?;
        let root_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("layout.lock"))
            .map_err(err)?;
        fs2::FileExt::try_lock_exclusive(&root_lock).map_err(err)?;
        let raw =
            restore_index::read_raw(File::open(snapshot_dir.join("index.json")).map_err(err)?)?;
        let index = restore_index::decode(&raw)?;
        let manifest = &index.manifest;
        let v3 = index.schema == "mycity-layout-snapshot-v3";
        if (v3
            && (manifest.schema != "mycity-l1-layout-v2"
                || manifest.r07.is_none()
                || manifest.r07_generator.as_deref() != Some(r07::Backing::pattern())))
            || (!v3
                && (!["mycity-layout-snapshot-v1", "mycity-layout-snapshot-v2"]
                    .contains(&index.schema.as_str())
                    || manifest.schema != "mycity-l1-layout-v1"
                    || manifest.r07.is_some()
                    || manifest.r07_generator.is_some()))
            || manifest.acceptance_sha256 != mycity_core::sha256(ACCEPTANCE)
        {
            return Err("layout_restore_schema".into());
        }
        let normal_bytes = manifest.normal_joint_budget_bytes;
        if normal_bytes != NORMAL_BYTES {
            return Err("layout_restore_full_scope".into());
        }
        let external_charge = manifest.external_owned_charge_bytes;
        let metadata_charge = manifest.metadata_and_io_charge_bytes;
        if metadata_charge != 4 * MIB
            || external_charge > 64 * MIB
            || manifest.cow_reserve_bytes != WORK_PAGES * MIB
        {
            return Err("layout_restore_work_budget".into());
        }
        let iterations = manifest
            .mutation_iteration
            .ok_or("layout_restore_missing_iteration")?;
        if iterations > 31 * 86400 {
            return Err("layout_restore_diagnostic_horizon".into());
        }
        let entries = &index.files;
        // Only the full cold-layout path owns a full region. Records-only
        // compatibility decoding below deliberately does not activate it.
        // Schema/work-budget rejection precedes this process-lifetime switch.
        let independent_logical = if v3 {
            r07_logical(normal_bytes, external_charge, metadata_charge)?
        } else {
            0
        };
        if let Some(state) = manifest.r07 {
            state.validate(independent_logical)?;
        }
        activate_layout_region(
            normal_bytes,
            external_charge,
            r07::mapped_capacity(independent_logical),
        )?;
        let snapshot_buffers = Arc::new(Mutex::new(SnapshotBuffers::new()));
        let mut buffers = snapshot_buffers.lock().map_err(err)?;
        let SnapshotBuffers { io, compare } = &mut *buffers;
        let mut tables = tables()?;
        let decode_workspace = Arc::new(Mutex::new(DecodeWorkspace::new(&tables)?));
        let exactly_once = |name: &str| entries.iter().filter(|e| e.name == name).count() == 1;
        if entries.len() != tables.len() + 3
            || !["native.json", "pressure.bin", "work-reserve.bin"]
                .iter()
                .all(|name| exactly_once(name))
            || !tables
                .iter()
                .all(|table| exactly_once(&format!("{}.json", table.name)))
        {
            return Err("layout_restore_file_set".into());
        }
        for entry in entries {
            let path = snapshot_dir.join(&entry.name);
            let metadata = fs::symlink_metadata(&path).map_err(err)?;
            if !metadata.file_type().is_file()
                || metadata.len() != entry.bytes
                || entry.bytes > normal_bytes as u64 + 512 * MIB as u64
            {
                return Err("layout_restore_file_shape".into());
            }
        }
        let entry = |name: &str| {
            entries
                .iter()
                .find(|e| e.name == name)
                .ok_or_else(|| format!("layout_restore_entry:{name}"))
        };
        let mut native = NativePressure::jprod()?;
        let bound = read_verified_buffered(
            &snapshot_dir.join("native.json"),
            entry("native.json")?,
            io,
            |reader| native.restore_checkpoint_into(reader),
        )?;
        if serde_json::to_value(native.manifest()).map_err(err)?
            != serde_json::to_value(
                manifest
                    .native
                    .as_ref()
                    .ok_or("layout_restore_native_shape")?,
            )
            .map_err(err)?
        {
            return Err("layout_restore_native_shape".into());
        }
        let mut decoded = decode_workspace.lock().map_err(err)?;
        for table in &mut tables {
            let name = format!("{}.json", table.name);
            read_verified_buffered(&snapshot_dir.join(&name), entry(&name)?, io, |reader| {
                decoded.decode_table(reader, &table.table)
            })?;
            decoded.copy_decoded_table(&mut table.table)?;
        }
        decoded.valid = true;
        drop(decoded);
        let payload = manifest
            .padding_payload_bytes
            .as_ref()
            .ok_or("layout_restore_padding")?;
        let total_payload = payload.iter().try_fold(0usize, |total, value| {
            total
                .checked_add(*value)
                .ok_or("layout_restore_payload_overflow")
        })?;
        let fixed_bytes = native.manifest().owned_capacity_bytes
            + tables.iter().map(|t| t.table.owned()).sum::<usize>()
            + WORK_PAGES * MIB
            + metadata_charge
            + external_charge;
        if Some(total_payload) != normal_bytes.checked_sub(fixed_bytes) {
            return Err("layout_restore_joint_budget".into());
        }
        let r07 = if v3 {
            Some(r07::Backing::full(independent_logical)?)
        } else {
            None
        };
        let historic_replaced = manifest.r07.map_or(0, |s| s.replaced_bytes);
        if entry("pressure.bin")?.bytes
            != total_payload
                .checked_sub(historic_replaced)
                .ok_or("r07_snapshot_length")? as u64
        {
            return Err("r07_snapshot_length".into());
        }
        let pages = read_verified_buffered(
            &snapshot_dir.join("pressure.bin"),
            entry("pressure.bin")?,
            io,
            |reader| {
                let mut pages = Vec::with_capacity(total_payload.div_ceil(MIB) + 7);
                for (domain, bytes) in payload.iter().enumerate() {
                    let mut remaining = if v3 && domain == 6 {
                        *bytes % r07::QUANTUM
                    } else {
                        *bytes
                    };
                    while remaining > 0 {
                        let n = remaining.min(MIB);
                        let mut data = vec![0u8; n];
                        reader.read_exact(&mut data).map_err(err)?;
                        pages.push(PressurePage {
                            domain: domain as u8,
                            bytes: Arc::new(data),
                        });
                        remaining -= n;
                    }
                }
                if let (Some(r07), Some(old)) = (&r07, manifest.r07) {
                    r07.read_snapshot_prefix(reader, old, compare)?;
                }
                Ok(pages)
            },
        )?;
        if entry("work-reserve.bin")?.bytes != (WORK_PAGES * MIB) as u64 {
            return Err("layout_restore_work_size".into());
        }
        let reserve_pages = read_verified_buffered(
            &snapshot_dir.join("work-reserve.bin"),
            entry("work-reserve.bin")?,
            io,
            |reader| {
                let mut pages = Vec::with_capacity(COW_PAGES);
                for _ in 0..COW_PAGES {
                    let mut data = vec![0u8; MIB];
                    reader.read_exact(&mut data).map_err(err)?;
                    if data.iter().any(|b| *b != 0xa5) {
                        return Err("layout_restore_work_pattern".into());
                    }
                    pages.push(data);
                }
                // The remaining half is a canonical reserve image, verified in
                // full. Live initialized typed storage was prebuilt above.
                for _ in COW_PAGES..WORK_PAGES {
                    reader.read_exact(compare).map_err(err)?;
                    if compare.iter().any(|b| *b != 0xa5) {
                        return Err("layout_restore_work_pattern".into());
                    }
                }
                Ok(pages)
            },
        )?;
        let generation = index.generation;
        if generation == 0 {
            return Err("layout_restore_zero_generation".into());
        }
        let report = json!({"cold_restore_seconds":started.elapsed().as_secs_f64(),"native_restore_bound":bound,"producer_source_sha256":manifest.source_sha256,"reader_source_sha256":env!("MYCITY_SOURCE_SHA256"),"same_source":manifest.source_sha256.as_deref()==Some(env!("MYCITY_SOURCE_SHA256")),"codec":"canonical layout-v1/v2/v3 generator profile; v3 R07 reconstructed full, old runtime credit not inherited; bounded typed index; full native shape/capacity and immutable stub equality; explicit mutation counter required; one-pass hash+load","restored_generation":generation,"restored_mutation_iteration":iterations,"all_payload_files_hashed_and_loaded":true,"byte_length":entries.iter().map(|e|e.bytes).sum::<u64>()});
        drop(buffers);
        Ok((
            Self {
                native: Arc::new(native),
                tables: Arc::new(tables),
                pages,
                r07,
                reserve: Arc::new(Mutex::new(Reserve {
                    pages: reserve_pages,
                    loan_pages: 0,
                    decode_loan_pages: 0,
                })),
                iterations,
                cow_pages: 0,
                normal_bytes,
                external_charge,
                metadata_charge,
                snapshot_buffers,
                decode_workspace,
                job: None,
                root: root.into(),
                generation,
                _root_lock: root_lock,
            },
            report,
        ))
    }
}
/// Standalone physical backing exerciser; synthetic requests are NOT runtime
/// credit and this does not instantiate or accept a full L1 city workload.
pub fn r07_backing_probe(dir: &Path) -> Result<Value, String> {
    fs::create_dir(dir).map_err(err)?;
    crate::heap_meter::activate_role(4 * MIB)?;
    let logical = r07_logical(NORMAL_BYTES, 64 * MIB, 4 * MIB)?;
    let mut stages = Vec::with_capacity(8);
    let sample = |label: &str, state: r07::State| -> Result<Value, String> {
        let resource = crate::native_observer::capture(std::process::id(), None, Instant::now())
            .map_err(err)?;
        Ok(
            json!({"label":label,"state":state,"resource":resource,"arena":crate::heap_meter::arena_snapshot()}),
        )
    };
    let mut b = r07::Backing::full(logical)?;
    let full = sample("full", b.state())?;
    stages.push(full.clone());
    let pin = b.snapshot();
    let pinned = b.apply(32 * MIB, 1)?;
    if pinned.replaced_bytes != 0 {
        return Err("r07_probe_pinned_release".into());
    }
    stages.push(sample("pinned_request_deferred", pinned)?);
    drop(pin);
    let released = b.apply(32 * MIB + r07::QUANTUM - 1, 2)?;
    if released.replaced_bytes != 32 * MIB {
        return Err("r07_probe_release_rounding".into());
    }
    let after = sample("unique_drop", released)?;
    let before_foot = full["resource"]["usage"]["footprint_bytes"]
        .as_u64()
        .ok_or("r07_probe_footprint")?;
    let after_foot = after["resource"]["usage"]["footprint_bytes"]
        .as_u64()
        .ok_or("r07_probe_footprint")?;
    if before_foot.saturating_sub(after_foot) < 24 * MIB as u64 {
        return Err("r07_probe_physical_release_not_observed".into());
    }
    stages.push(after);
    let prefix = b.snapshot();
    let old = prefix.state;
    let mut io = vec![0; MIB];
    let mut compare = vec![0; MIB];
    let file = write_file(dir, "r07-prefix.bin", &mut io, |w| prefix.write(w))?;
    if file.bytes != old.mapped_bytes as u64 {
        return Err("r07_probe_prefix_length".into());
    }
    let repaid = b.apply(0, 3)?;
    if repaid.replaced_bytes != 0 {
        return Err("r07_probe_repayment".into());
    }
    read_verified_buffered(&dir.join("r07-prefix.bin"), &file, &mut io, |reader| {
        prefix.verify(reader, &mut compare)
    })?;
    stages.push(sample("repaid_with_old_prefix_pinned", repaid)?);
    drop(prefix);
    drop(b);
    let reconstructed = r07::Backing::full(logical)?;
    read_verified_buffered(&dir.join("r07-prefix.bin"), &file, &mut io, |reader| {
        reconstructed.read_snapshot_prefix(reader, old, &mut compare)
    })?;
    stages.push(sample(
        "full_reconstructed_without_historical_credit",
        reconstructed.state(),
    )?);
    let report = json!({"schema":"mycity-r07-backing-probe-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"logical_r07_bytes":logical,"generator":r07::Backing::pattern(),"mapping_control_bytes_bound":reconstructed.control_bytes(),"stages":stages,"prefix_file":file,"physical_drop_bytes":before_foot-after_foot,"same_process_reconstruction_only":true,"request_is_artificial_mechanism_test_not_runtime_credit":true,"full_L1_native_stubs_not_instantiated":true,"normal_gate":"not_tested","eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(err)?,
    )
    .map_err(err)?;
    Ok(report)
}
pub fn reopen(snapshot_dir: &Path, report_dir: &Path) -> Result<Value, String> {
    reopen_inner(snapshot_dir, report_dir, true)
}
pub fn load_only(snapshot_dir: &Path, report_dir: &Path) -> Result<Value, String> {
    reopen_inner(snapshot_dir, report_dir, false)
}
/// Compatibility test of ALL original typed records, without allocating the
/// 3.10GiB pressure image. Never signs the full layout or joint-memory gate.
pub fn restore_records_only(snapshot_dir: &Path, report_dir: &Path) -> Result<Value, String> {
    use sha2::Digest;
    fs::create_dir(report_dir).map_err(err)?;
    let started = Instant::now();
    let raw = mycity_core::read_bounded(
        File::open(snapshot_dir.join("index.json")).map_err(err)?,
        256 * 1024,
    )
    .map_err(err)?;
    let index: Value = serde_json::from_slice(&raw).map_err(err)?;
    if !["mycity-layout-snapshot-v2", "mycity-layout-snapshot-v3"]
        .iter()
        .any(|schema| index["schema"] == *schema)
        || index["manifest"]["acceptance_sha256"] != mycity_core::sha256(ACCEPTANCE)
    {
        return Err("layout_record_compat_schema".into());
    }
    let entries: Vec<FileEntry> = serde_json::from_value(index["files"].clone()).map_err(err)?;
    let mut checked = Vec::new();
    let entry = |name: &str| -> Result<&FileEntry, String> {
        let mut found = entries.iter().filter(|e| e.name == name);
        let entry = found
            .next()
            .ok_or_else(|| format!("layout_record_missing:{name}"))?;
        if found.next().is_some() {
            return Err("layout_record_duplicate_file".into());
        }
        Ok(entry)
    };
    let compare = |record: &FileEntry,
                   writer: HashWriter<std::io::Sink>|
     -> Result<Value, String> {
        let digest = format!("{:x}", writer.digest.finalize());
        if writer.bytes != record.bytes || digest != record.sha256 {
            return Err(format!("layout_record_roundtrip:{}", record.name));
        }
        Ok(
            json!({"name":record.name,"bytes":record.bytes,"sha256":digest,"canonical_roundtrip_equal":true}),
        )
    };
    let native_entry = entry("native.json")?;
    let (native, native_bound) =
        read_verified(&snapshot_dir.join("native.json"), native_entry, |r| {
            NativePressure::restore_checkpoint_profiled(r)
        })?;
    if serde_json::to_value(native.manifest()).map_err(err)? != index["manifest"]["native"] {
        return Err("layout_record_native_manifest".into());
    }
    let mut native_count = 0;
    let native_scan = native.scan(|n, _| native_count += n)?;
    let mut writer = HashWriter {
        inner: std::io::sink(),
        digest: sha2::Sha256::new(),
        bytes: 0,
    };
    native.write_checkpoint(&mut writer)?;
    checked.push(compare(native_entry, writer)?);
    drop(native);
    let expected = tables()?;
    let mut table_count = 0;
    for table in &expected {
        let name = format!("{}.json", table.name);
        let record = entry(&name)?;
        let candidate = read_verified(&snapshot_dir.join(&name), record, |r| {
            bounded_table::decode(r, &table.table)
        })?;
        if candidate != table.table || candidate.owned() != table.table.owned() {
            return Err(format!("layout_record_changed:{}", table.name));
        }
        table_count += candidate.count();
        let mut writer = HashWriter {
            inner: std::io::sink(),
            digest: sha2::Sha256::new(),
            bytes: 0,
        };
        serde_json::to_writer(&mut writer, &candidate).map_err(err)?;
        checked.push(compare(record, writer)?);
    }
    let report = json!({"schema":"mycity-layout-record-compat-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),
        "producer_source_sha256":index["manifest"]["source_sha256"],"original_index_sha256":mycity_core::sha256(&raw),
        "native_records":native_count,"native_scan":native_scan,"stub_records":table_count,
        "native_restore_bound":native_bound,"all_original_component_files":checked,
        "actual_seconds":started.elapsed().as_secs_f64(),"scope":"record components only; original constant counts and canonical bytes; pressure/reserve payload NOT loaded",
        "full_layout":"not_run","joint_normal64":"not_accepted","common8":"not_accepted","steady":"not_run","eight_hour":"not_run","real16gb":"not_run","full_M0":"not_accepted"});
    let mut f = File::create(report_dir.join("report.json")).map_err(err)?;
    serde_json::to_writer_pretty(&mut f, &report).map_err(err)?;
    f.sync_all().map_err(err)?;
    Ok(report)
}
fn reopen_inner(snapshot_dir: &Path, report_dir: &Path, save: bool) -> Result<Value, String> {
    fs::create_dir(report_dir).map_err(err)?;
    let (mut load, restore) = Load::restore(snapshot_dir)?;
    let manifest = load.manifest();
    let scan = load.scan(|_, _, _| {})?;
    let next = if save {
        let admitted = load.start_snapshot()?;
        let saved = load.finish_snapshot()?.ok_or("layout_reopen_save")?;
        Some(json!({"admission":admitted,"snapshot":saved}))
    } else {
        None
    };
    let report = json!({"schema":"mycity-layout-cold-reopen-v2","restore":restore,"manifest":manifest,"resumed_scan":scan,"next_generation":next,"cold_reopen_and_scan":"passed","next_generation_requested":save,"steady_gate":"not_run","eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    fs::write(
        report_dir.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(err)?,
    )
    .map_err(err)?;
    Ok(report)
}

/// Diagnostic TWO-generation protocol exercise. There is deliberately no fixed
/// save deadline here: formal/steady schedules above remain entirely unchanged.
/// Every admitted scan still uses the original full records and1MiB dirty work.
pub fn r07_session(dir: &Path, report_fault: bool) -> Result<Value, String> {
    use std::time::Duration;
    fs::create_dir(dir).map_err(err)?;
    let mut metrics = crate::scale_metrics::Metrics::create(&dir.join("compute-durations.bin"))?;
    // Explicit diagnostic-only fault, before allocating the full layout. Both
    // branches use the same report publisher; no error-shaped success JSON.
    if report_fault {
        metrics.observe("r07_deliberately_unknown_report_class", 0);
        return publish_r07_report(dir, &mut metrics, json!({"diagnostic_report_fault":true}));
    }
    let mut load = Load::full(&dir.join("snapshots"), 64 * MIB)?;
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&load.manifest()).map_err(err)?,
    )
    .map_err(err)?;
    let mut events = BufWriter::with_capacity(
        65536,
        File::create_new(dir.join("events.jsonl")).map_err(err)?,
    );
    let began = Instant::now();
    let mut next_scan = 0u64;
    let mut admitted = 0u64;
    let mut completed = 0u64;
    let mut first_credit_at = None;
    let mut second_started = None;
    let mut snapshots = Vec::with_capacity(2);
    let mut scans = 0u64;
    loop {
        if began.elapsed() > Duration::from_secs(120) {
            return Err("r07_exercise_deadline".into());
        }
        if let Some(result) = load.poll_snapshot()? {
            completed += 1;
            serde_json::to_writer(&mut events,&json!({"elapsed_seconds":began.elapsed().as_secs_f64(),"snapshot_durable":&result})).map_err(err)?;
            events.write_all(b"\n").map_err(err)?;
            events.flush().map_err(err)?;
            snapshots.push(result);
        }
        if began.elapsed().as_secs() >= next_scan {
            let scan = load.scan(|class, _, ns| metrics.observe(class, ns))?;
            if scan.dirty_bytes != MIB {
                return Err("r07_exercise_dirty_contract".into());
            }
            serde_json::to_writer(&mut events,&json!({"elapsed_seconds":began.elapsed().as_secs_f64(),"scan":scan,"r07":load.r07.as_ref().map(r07::Backing::state),"arena":crate::heap_meter::arena_snapshot()})).map_err(err)?;
            events.write_all(b"\n").map_err(err)?;
            events.flush().map_err(err)?;
            next_scan += 1;
            scans += 1;
        }
        let replaced = load.r07.as_ref().map_or(0, |b| b.state().replaced_bytes);
        if completed == 1 && replaced > 0 && first_credit_at.is_none() {
            first_credit_at = Some(began.elapsed());
        }
        let ready_second = admitted == 1
            && completed == 1
            && first_credit_at.is_some_and(|at| began.elapsed() >= at + Duration::from_secs(1));
        if admitted == 0 || ready_second {
            let admission = load.start_snapshot()?;
            admitted += 1;
            if admitted == 2 {
                second_started = Some(began.elapsed());
            }
            serde_json::to_writer(&mut events,&json!({"elapsed_seconds":began.elapsed().as_secs_f64(),"snapshot_admitted":admission})).map_err(err)?;
            events.write_all(b"\n").map_err(err)?;
            events.flush().map_err(err)?;
        }
        if completed == 2
            && second_started.is_some_and(|at| began.elapsed() >= at + Duration::from_secs(4))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let result = json!({"schema":"mycity-r07-full-session-exercise-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"elapsed_seconds":began.elapsed().as_secs_f64(),"scans":scans,"snapshots":snapshots,"manifest":load.manifest(),"all_admitted_dirty_bytes_exactly_one_mib":true,"snapshot_scheduling":"diagnostic event-driven, exactly two actual generations; no formal deadline claim","formal_steady":"not_accepted","eight_hour":"not_run","memory_16gb_gate":"not_run"});
    publish_r07_report(dir, &mut metrics, result)
}
fn publish_r07_report(
    dir: &Path,
    metrics: &mut crate::scale_metrics::Metrics,
    mut result: Value,
) -> Result<Value, String> {
    // Complete/check raw diagnostics before creating a successful report. A
    // failed report must propagate through the CLI and leave no success file.
    result["compute"] = metrics.report()?;
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&result).map_err(err)?,
    )
    .map_err(err)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn owner_stop_requires_explicit_byte_and_publication_cutoff_is_fixed() {
        use std::time::Duration;
        assert!(super::read_owner_stop(&b"S"[..]).is_ok());
        assert!(super::read_owner_stop(&b""[..]).is_err());
        assert!(super::read_owner_stop(&b"X"[..]).is_err());
        // The fixed 600-second session publishes deadlines 0..599, even
        // when the last scan is processed late. A stop at 600.2 adds 600.
        let exact = Some(Duration::from_secs(600));
        assert!(super::published_before(599, exact));
        assert!(!super::published_before(600, exact));
        assert!(super::published_before(
            600,
            Some(Duration::from_millis(600200))
        ));
        assert!(!super::published_before(
            601,
            Some(Duration::from_millis(600200))
        ));
        assert!(super::published_before(34999, None));
    }
    use super::*;
    #[test]
    fn borrowed_snapshot_index_preserves_existing_disk_schema_and_file_order() {
        let manifest = json!({"native":{"fields":[["accounts",40000]]},"mutation_iteration":7});
        let entries = vec![
            FileEntry {
                name: "native.json".into(),
                bytes: 123,
                sha256: "a".repeat(64),
            },
            FileEntry {
                name: "pressure.bin".into(),
                bytes: 456,
                sha256: "b".repeat(64),
            },
        ];
        let old = json!({"schema":"mycity-layout-snapshot-v2","generation":2,"manifest":manifest,"files":entries});
        let borrowed = SnapshotIndex {
            schema: "mycity-layout-snapshot-v2",
            generation: 2,
            manifest: &manifest,
            files: &entries,
        };
        let encoded = serde_json::to_vec(&borrowed).unwrap();
        let decoded: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, old);
        assert_eq!(entries[0].name, "native.json");
        assert_eq!(entries[1].name, "pressure.bin");
    }
    #[test]
    fn external_component_reservation_rejects_oversize_before_allocation() {
        let root =
            std::env::temp_dir().join(format!("mycity-layout-external-{}", std::process::id()));
        assert!(!root.exists());
        for charge in [64 * MIB + 1, usize::MAX] {
            assert!(
                matches!(Load::full(&root, charge), Err(e) if e == "layout_external_reservation_limit")
            );
            assert!(!root.exists());
        }
    }
    #[test]
    fn restore_rejects_component_reservation_before_loading_any_large_file() {
        let root = std::env::temp_dir().join(format!(
            "mycity-layout-restore-budget-{}",
            std::process::id()
        ));
        let snapshot = root.join("snapshot-000001");
        fs::create_dir_all(&snapshot).unwrap();
        for (charge, metadata, cow) in [
            (64 * MIB + 1, 4 * MIB, WORK_PAGES * MIB),
            (usize::MAX, 4 * MIB, WORK_PAGES * MIB),
            (64 * MIB, 0, WORK_PAGES * MIB),
            (64 * MIB, 4 * MIB, 0),
        ] {
            // Deliberately omit every large file and even file entries. Budget
            // rejection must happen before file-set checks, tables() or decoding.
            let index = json!({"schema":"mycity-layout-snapshot-v2","manifest":{
                "schema":"mycity-l1-layout-v1","acceptance_sha256":mycity_core::sha256(ACCEPTANCE),
                "normal_joint_budget_bytes":NORMAL_BYTES,"external_owned_charge_bytes":charge,
                "metadata_and_io_charge_bytes":metadata,"cow_reserve_bytes":cow}});
            fs::write(
                snapshot.join("index.json"),
                serde_json::to_vec(&index).unwrap(),
            )
            .unwrap();
            assert!(
                matches!(Load::restore(&snapshot), Err(e) if e == "layout_restore_work_budget")
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn eight_hour_snapshot_summaries_do_not_retain_file_trees() {
        let full = json!({"generation":1,"durable_complete_seconds":1.25,"write_fsync_seconds":0.75,
            "files":(0..29).map(|_|json!({"bytes":123,"name":"x".repeat(4096)})).collect::<Vec<_>>(),
            "complete_record_restore":"passed","full_pressure_file_readback":"passed"});
        let mut summaries = Vec::with_capacity(128);
        for _ in 0..96 {
            summaries.push(SnapshotSummary::from_result(&full).unwrap());
        }
        assert!(summaries.capacity() * size_of::<SnapshotSummary>() <= 8192);
        assert!(serde_json::to_vec(&summaries).unwrap().len() < 32768);
        assert_eq!(summaries[95].file_bytes, 29 * 123);
        assert!(summaries[95].record_restore_passed && summaries[95].pressure_readback_passed);
    }
    #[test]
    fn all_stub_counts_widths_population_and_references_are_real_records() {
        let tables = tables().unwrap();
        assert_eq!(tables.len(), 26);
        assert_eq!(size_of::<Stub<5>>(), 64);
        assert_eq!(size_of::<Stub<13>>(), 128);
        assert_eq!(size_of::<Stub<29>>(), 256);
        let mut count = 0;
        for table in &tables {
            let mut scanned = 0;
            table.table.scan(|n, _| scanned += n);
            assert_eq!(scanned, table.table.count());
            count += scanned;
        }
        assert!(count > 1_200_000);
        if let Table::W256(v) = &tables[1].table {
            assert_eq!(v.iter().map(|r| r.payload[0]).sum::<u64>(), 57000);
        } else {
            panic!("household stub format");
        }
        if let Table::PathRefs(v) = &tables[10].table {
            assert_eq!(v.len(), 900000);
            assert!(v.iter().all(|i| *i < 14800));
        } else {
            panic!("path reference format");
        }
    }
    #[test]
    fn real_cow_transfers_existing_reserve_allocation_and_preserves_snapshot() {
        let original = Arc::new(vec![7u8; MIB]);
        let mut current = original.clone();
        let mut reserve = Reserve {
            pages: (0..2).map(|_| vec![0xa5; MIB]).collect(),
            loan_pages: 0,
            decode_loan_pages: 0,
        };
        assert!(reserve.copy_page(&[0; 7]).is_err());
        assert_eq!(reserve.pages.len(), 2);
        let existing_allocation = reserve.pages.last().unwrap().as_ptr();
        assert!(reserve.copy_page(&[0u8; 7]).is_err());
        assert_eq!(reserve.pages.len(), 2);
        current = Arc::new(reserve.copy_page(&current).unwrap());
        assert_eq!(current.as_ptr(), existing_allocation);
        Arc::make_mut(&mut current).fill(8);
        assert!(original.iter().all(|v| *v == 7));
        assert!(current.iter().all(|v| *v == 8));
        assert_eq!(
            reserve.pages.len() * MIB + current.len() + original.len(),
            3 * MIB
        );
        let old_address = original.as_ptr();
        // This unit uses a two-page reserve; pad to the production count without
        // changing the COW transfer being tested.
        reserve
            .pages
            .extend((0..COW_PAGES - 2).map(|_| vec![0xa5; MIB]));
        reserve
            .reclaim_cow(
                vec![PressurePage {
                    domain: 0,
                    bytes: original,
                }],
                1,
            )
            .unwrap();
        assert_eq!(reserve.pages.last().unwrap().as_ptr(), old_address);
        assert_eq!(reserve.pages.len(), COW_PAGES);
        assert_eq!(reserve.loan_pages, 0);
        assert!(reserve.pages.last().unwrap().iter().all(|b| *b == 0xa5));
    }
    #[test]
    fn snapshot_error_returns_pages_after_join_and_keeps_shared_pages_world_owned() {
        let original = Arc::new(vec![7; MIB]);
        let mut current = original.clone();
        let unchanged = Arc::new(vec![9; MIB]);
        let mut reserve = Reserve {
            pages: (0..COW_PAGES).map(|_| vec![0xa5; MIB]).collect(),
            loan_pages: 0,
            decode_loan_pages: 0,
        };
        let snapshot_pages = vec![
            PressurePage {
                domain: 0,
                bytes: original,
            },
            PressurePage {
                domain: 1,
                bytes: unchanged.clone(),
            },
        ];
        let old_address = snapshot_pages[0].bytes.as_ptr();
        // Force the exact previously racy ordering: worker finishes its work
        // while main has borrowed the new backing but not replaced the old Arc.
        let copied = reserve.copy_page(&current).unwrap();
        let worker = thread::spawn(move || SnapshotCompletion {
            result: Err("injected_readback_failure".into()),
            pages: snapshot_pages,
        });
        let completed = worker.join().unwrap();
        assert_eq!(Arc::strong_count(&current), 2);
        current = Arc::new(copied);
        assert_eq!(current[0], 7);
        assert!(completed.result.is_err());
        reserve.reclaim_cow(completed.pages, 1).unwrap();
        assert_eq!(reserve.pages.last().unwrap().as_ptr(), old_address);
        assert_eq!(Arc::strong_count(&unchanged), 1);
        assert_eq!(unchanged[0], 9);
        assert_eq!(reserve.pages.len(), COW_PAGES);
        assert_eq!(reserve.loan_pages, 0);
    }
    #[test]
    fn page_return_cannot_cancel_decode_debt_or_fabricate_missing_pages() {
        let mut reserve = Reserve {
            pages: (0..COW_PAGES).map(|_| vec![0xa5; MIB]).collect(),
            loan_pages: 0,
            decode_loan_pages: 0,
        };
        reserve.begin_decode().unwrap();
        let available = reserve.pages.len();
        assert!(reserve.reclaim_cow(Vec::new(), 1).is_err());
        assert_eq!(reserve.pages.len(), available);
        assert_eq!(reserve.decode_loan_pages, DECODE_BYTES / MIB);
        reserve.end_decode().unwrap();
        // A missing actual COW backing may not be replaced with a fresh Vec.
        reserve.pages.pop();
        reserve.loan_pages = 1;
        assert!(reserve.reclaim_cow(Vec::new(), 1).is_err());
        assert_eq!(reserve.pages.len(), available - 1);
        assert_eq!(reserve.loan_pages, 1);
    }
    #[test]
    fn persistent_decode_borrow_preserves_all_sixty_four_concurrent_cow_pages() {
        assert_eq!(WORK_PAGES * MIB, 128 * MIB);
        assert_eq!(COW_PAGES * MIB, 64 * MIB);
        assert_eq!(DECODE_BYTES + COW_PAGES * MIB, WORK_PAGES * MIB);
        let mut reserve = Reserve {
            pages: (0..COW_PAGES).map(|_| vec![0xa5; MIB]).collect(),
            loan_pages: 0,
            decode_loan_pages: 0,
        };
        let mut world: Vec<_> = (0..COW_PAGES)
            .map(|i| PressurePage {
                domain: 0,
                bytes: Arc::new(vec![i as u8; MIB]),
            })
            .collect();
        let snapshot = world.clone();
        let old_ids: Vec<_> = snapshot.iter().map(|p| p.bytes.as_ptr()).collect();
        reserve.begin_decode().unwrap();
        assert!(reserve.begin_decode().is_err());
        for page in &mut world {
            page.bytes = Arc::new(reserve.copy_page(&page.bytes).unwrap());
        }
        assert!(reserve.pages.is_empty());
        assert!(reserve.copy_page(&snapshot[0].bytes).is_err());
        assert_eq!(reserve.loan_pages, WORK_PAGES);
        reserve.end_decode().unwrap();
        assert!(reserve.end_decode().is_err());
        reserve.reclaim_cow(snapshot, COW_PAGES).unwrap();
        assert_eq!(reserve.loan_pages, 0);
        assert_eq!(reserve.pages.len(), COW_PAGES);
        let returned: Vec<_> = reserve.pages.iter().map(|p| p.as_ptr()).collect();
        assert_eq!(returned, old_ids);
        for (i, page) in world.iter().enumerate() {
            assert_eq!(page.bytes[0], i as u8);
        }
    }
    #[test]
    fn borrowed_file_buffers_reset_hashes_verify_eof_and_keep_addresses() {
        let root = std::env::temp_dir().join(format!("mycity-layout-io-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let mut buffers = SnapshotBuffers::new();
        let addresses = (buffers.io.as_ptr(), buffers.compare.as_ptr());
        for n in [1, MIB + 19, 11] {
            let bytes = pattern(n, n as u64);
            let entry = write_file(&root, "payload", &mut buffers.io, |w| {
                w.write_all(&bytes).map_err(err)
            })
            .unwrap();
            let read =
                read_verified_buffered(&root.join("payload"), &entry, &mut buffers.io, |r| {
                    let mut result = Vec::new();
                    r.read_to_end(&mut result).map_err(err)?;
                    Ok(result)
                })
                .unwrap();
            assert_eq!(read, bytes);
            assert_eq!(addresses, (buffers.io.as_ptr(), buffers.compare.as_ptr()));
            assert!(read_verified_buffered(
                &root.join("payload"),
                &entry,
                &mut buffers.io,
                |_| Ok(())
            )
            .is_err());
            let mut bad = entry;
            bad.sha256 = "0".repeat(64);
            assert!(
                read_verified_buffered(&root.join("payload"), &bad, &mut buffers.io, |r| {
                    std::io::copy(r, &mut std::io::sink()).map_err(err)
                })
                .is_err()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
