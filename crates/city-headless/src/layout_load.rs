//! Full frozen JPROD storage pressure. Missing business domains remain typed stubs.
//! This module cannot mutate or install a gameplay World.
use mycity_core::layout_pressure::NativePressure;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    hint::black_box,
    io::{BufReader, BufWriter, Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Instant,
};
const MIB: usize = 1024 * 1024;
const COW_PAGES: usize = 128;
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
    name: String,
    bytes: u64,
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
    use sha2::Digest;
    let hashed = HashReader {
        inner: File::open(path).map_err(err)?,
        digest: sha2::Sha256::new(),
        bytes: 0,
    };
    let mut reader = BufReader::with_capacity(MIB, hashed);
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
    write: impl FnOnce(&mut HashWriter<BufWriter<File>>) -> Result<(), String>,
) -> Result<FileEntry, String> {
    use sha2::Digest;
    let file = File::create(dir.join(name)).map_err(err)?;
    let mut writer = HashWriter {
        inner: BufWriter::with_capacity(MIB, file),
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
}
impl Reserve {
    fn loan(&mut self, n: usize) -> Result<(), String> {
        if self.pages.len() < n {
            return Err("layout_cow_backpressure".into());
        }
        for _ in 0..n {
            self.pages.pop();
        }
        self.loan_pages += n;
        Ok(())
    }
    fn repay(&mut self, n: usize) -> Result<(), String> {
        if self.loan_pages < n {
            return Err("layout_cow_loan".into());
        }
        for _ in 0..n {
            self.pages.push(vec![0xa5; MIB]);
        }
        self.loan_pages -= n;
        Ok(())
    }
}
#[derive(Serialize)]
pub struct Scan {
    pub iteration: u64,
    pub native_records: usize,
    pub stub_records: usize,
    pub touched_pressure_bytes: usize,
    pub dirty_bytes: usize,
    pub checksum: u64,
    pub wall_seconds: f64,
    pub cow_pages: usize,
}
pub struct Load {
    native: Arc<NativePressure>,
    tables: Arc<Vec<DomainTable>>,
    pages: Vec<PressurePage>,
    reserve: Arc<Mutex<Reserve>>,
    iterations: u64,
    cow_pages: usize,
    normal_bytes: usize,
    external_charge: usize,
    metadata_charge: usize,
    job: Option<JoinHandle<Result<Value, String>>>,
    root: PathBuf,
    generation: u64,
    _root_lock: File,
}
impl Load {
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
        let native = Arc::new(NativePressure::jprod()?);
        let tables = Arc::new(tables()?);
        let native_bytes = native.manifest().owned_capacity_bytes;
        let table_bytes = tables.iter().map(|t| t.table.owned()).sum::<usize>();
        // Headers, 1MiB IO buffer, manifest, Arc controls and maximum clone-index
        // allocation are precharged. Runtime/observer state supplied by supervisor
        // shares this budget instead of being added to a 3.10GiB padding block.
        let metadata_charge = 4 * MIB;
        let minimum =
            native_bytes + table_bytes + COW_PAGES * MIB + metadata_charge + external_charge;
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
                COW_PAGES * MIB + metadata_charge + external_charge
            } else {
                0
            };
            let mut remaining = quota.checked_sub(charge).ok_or("layout_domain_budget")?;
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
        };
        Ok(Self {
            native,
            tables,
            pages,
            reserve: Arc::new(Mutex::new(reserve)),
            iterations: 0,
            cow_pages: 0,
            normal_bytes,
            external_charge,
            metadata_charge,
            job: None,
            root: root.into(),
            generation: 0,
            _root_lock: root_lock,
        })
    }
    pub fn manifest(&self) -> Value {
        let domains:Vec<_>=self.tables.iter().map(|t|json!({"domain":t.name,"source_kind":t.kind,"source":"layout_stub_not_functional","schema_version":1,"count":t.table.count(),"record_bytes":t.width,"owned_capacity_bytes":t.table.owned(),"owner_bound":t.owners,"index":"record_id_1_based_contiguous; path_refs_zero_based_OD_index"})).collect();
        let mut by_domain = [0usize; 7];
        for p in &self.pages {
            by_domain[p.domain as usize] += p.bytes.capacity();
        }
        json!({"schema":"mycity-l1-layout-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"acceptance_sha256":mycity_core::sha256(ACCEPTANCE),"native":self.native.manifest(),"stubs":domains,
            "population":{"total":57000,"representation":"22800 household stub members; 11400*3 + 11400*2","actors":0},
            "padding_payload_bytes":by_domain,"padding_payload_total_bytes":self.pages.iter().map(|p|p.bytes.capacity()).sum::<usize>(),"pressure_page_bytes":MIB,"mutation_iteration":self.iterations,"pressure_pattern":"splitmix64-init-v1; affine-u64-dirty-v1",
            "cow_reserve_bytes":COW_PAGES*MIB,"metadata_and_io_charge_bytes":self.metadata_charge,"external_owned_charge_bytes":self.external_charge,"normal_joint_budget_bytes":self.normal_bytes,
            "solver_preview":"192MiB preview and 32MiB compute are pressure pages within solver share; no algorithms/GPU/UI",
            "layout_not_business":true,"full_M0":"not_accepted","memory_16gb_gate":"not_run"})
    }
    pub fn scan(&mut self, mut page: impl FnMut(&'static str, usize, u64)) -> Result<Scan, String> {
        let started = Instant::now();
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
        {
            let reserve = self.reserve.lock().map_err(err)?;
            for p in &reserve.pages {
                for chunk in p.chunks(4096) {
                    checksum = checksum.wrapping_add(u64::from(black_box(chunk[0])));
                }
                touched += p.len();
            }
        }
        // Exactly one full MiB, avoiding the possible short final pressure page.
        let mut index = self.iterations as usize % self.pages.len();
        while self.pages[index].bytes.len() != MIB {
            index = (index + 1) % self.pages.len();
        }
        if Arc::strong_count(&self.pages[index].bytes) > 1 {
            self.reserve.lock().map_err(err)?.loan(1)?;
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
            dirty_bytes: dirty.len(),
            checksum: checksum.wrapping_add(u64::from(value)),
            wall_seconds: started.elapsed().as_secs_f64(),
            cow_pages: self.cow_pages,
        })
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
        let root = self.root.clone();
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
                        native,
                        tables,
                        reserve,
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
    pub fn poll_snapshot(&mut self) -> Result<Option<Value>, String> {
        if self.job.as_ref().is_some_and(JoinHandle::is_finished) {
            let job = self.job.take().ok_or("layout_job")?;
            let result = job.join().map_err(|_| "layout_snapshot_panicked")?;
            // Worker drops old Arc pages before it terminates; recover placeholders.
            self.reserve.lock().map_err(err)?.repay(self.cow_pages)?;
            self.cow_pages = 0;
            return result.map(Some);
        }
        Ok(None)
    }
    pub fn finish_snapshot(&mut self) -> Result<Option<Value>, String> {
        if let Some(job) = self.job.take() {
            let result = job.join().map_err(|_| "layout_snapshot_panicked")?;
            self.reserve.lock().map_err(err)?.repay(self.cow_pages)?;
            self.cow_pages = 0;
            return result.map(Some);
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
struct Snapshot {
    root: PathBuf,
    dir: PathBuf,
    generation: u64,
    pages: Vec<PressurePage>,
    native: Arc<NativePressure>,
    tables: Arc<Vec<DomainTable>>,
    reserve: Arc<Mutex<Reserve>>,
    manifest: Value,
}
impl Snapshot {
    fn run(self) -> Result<Value, String> {
        let Self {
            root,
            dir,
            generation,
            pages,
            native,
            tables,
            reserve,
            manifest,
        } = self;
        let start = Instant::now();
        let mut entries = Vec::with_capacity(tables.len() + 3);
        entries.push(write_file(&dir, "native.json", |w| {
            native.write_checkpoint(w)
        })?);
        for table in tables.iter() {
            entries.push(write_file(&dir, &format!("{}.json", table.name), |w| {
                serde_json::to_writer(w, &table.table).map_err(err)
            })?);
        }
        entries.push(write_file(&dir, "pressure.bin", |w| {
            for page in &pages {
                w.write_all(&page.bytes).map_err(err)?;
            }
            Ok(())
        })?);
        // The complete 128MiB reserved work-page image is physically written too.
        // Stream one existing placeholder repeatedly, never allocate another world.
        entries.push(write_file(&dir, "work-reserve.bin", |w| {
            let spare = reserve.lock().map_err(err)?;
            let page = spare.pages.first().ok_or("layout_no_spare")?;
            for _ in 0..COW_PAGES {
                w.write_all(page).map_err(err)?;
            }
            Ok(())
        })?);
        let write_fsync_seconds = start.elapsed().as_secs_f64();
        // Fund the full transient decode from the SAME 128MiB COW reserve. No full
        // padding/world clone is allocated; each domain is restored and released.
        reserve.lock().map_err(err)?.loan(64)?;
        let restore = (|| {
            let entry = |name: &str| {
                entries
                    .iter()
                    .find(|e| e.name == name)
                    .ok_or_else(|| format!("layout_snapshot_entry:{name}"))
            };
            let (candidate, native_restore_bound) =
                read_verified(&dir.join("native.json"), entry("native.json")?, |reader| {
                    NativePressure::restore_checkpoint_profiled(reader)
                })?;
            let original_hash = write_file(&dir, "native-roundtrip.json", |w| {
                candidate.write_checkpoint(w)
            })?;
            if original_hash.sha256 != entries[0].sha256 || original_hash.bytes != entries[0].bytes
            {
                return Err("layout_native_restore_changed".into());
            }
            let mut bound_file =
                File::create(dir.join("native-restore-bound.json")).map_err(err)?;
            serde_json::to_writer_pretty(&mut bound_file, &native_restore_bound).map_err(err)?;
            bound_file.sync_all().map_err(err)?;
            drop(candidate);
            fs::remove_file(dir.join("native-roundtrip.json")).map_err(err)?;
            for table in tables.iter() {
                let name = format!("{}.json", table.name);
                let candidate: Table = read_verified(&dir.join(&name), entry(&name)?, |reader| {
                    serde_json::from_reader(reader).map_err(err)
                })?;
                if candidate != table.table {
                    return Err("layout_stub_restore_changed".into());
                }
            }
            read_verified(
                &dir.join("pressure.bin"),
                entry("pressure.bin")?,
                |reader| {
                    let mut buf = vec![0u8; MIB];
                    for page in &pages {
                        reader
                            .read_exact(&mut buf[..page.bytes.len()])
                            .map_err(err)?;
                        if buf[..page.bytes.len()] != page.bytes[..] {
                            return Err("layout_page_restore_changed".into());
                        }
                    }
                    Ok(())
                },
            )?;
            read_verified(
                &dir.join("work-reserve.bin"),
                entry("work-reserve.bin")?,
                |reader| {
                    let mut buf = vec![0u8; MIB];
                    for _ in 0..COW_PAGES {
                        reader.read_exact(&mut buf).map_err(err)?;
                        if buf.iter().any(|b| *b != 0xa5) {
                            return Err("layout_work_restore_changed".into());
                        }
                    }
                    Ok(())
                },
            )?;
            serde_json::to_value(native_restore_bound).map_err(err)
        })();
        reserve.lock().map_err(err)?.repay(64)?;
        let native_restore_bound = restore?;
        let index = json!({"schema":"mycity-layout-snapshot-v2","generation":generation,"manifest":manifest,"files":entries});
        let mut f = File::create(dir.join("index.json")).map_err(err)?;
        serde_json::to_writer_pretty(&mut f, &index).map_err(err)?;
        f.sync_all().map_err(err)?;
        File::open(&dir).map_err(err)?.sync_all().map_err(err)?;
        let durable = root.join(format!("snapshot-{generation:06}"));
        fs::rename(&dir, &durable).map_err(err)?;
        File::open(&root).map_err(err)?.sync_all().map_err(err)?;
        // Only after full new restore validation and directory durability, retain two.
        if generation > 2 {
            fs::remove_dir_all(root.join(format!("snapshot-{:06}", generation - 2)))
                .map_err(err)?;
            File::open(&root).map_err(err)?.sync_all().map_err(err)?;
        }
        Ok(
            json!({"generation":generation,"write_fsync_seconds":write_fsync_seconds,"durable_complete_seconds":start.elapsed().as_secs_f64(),"files":entries,"native_restore_bound":native_restore_bound,"complete_record_restore":"passed","full_pressure_file_readback":"passed","remaining_free_bytes":free_bytes(&root)?}),
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
            if scans > 0 && elapsed >= wall_seconds as f64 {
                break;
            }
            if elapsed >= next_snapshot as f64 {
                let lateness = elapsed - next_snapshot as f64;
                maximum_snapshot_start_lateness = maximum_snapshot_start_lateness.max(lateness);
                let admission = load.start_snapshot()?;
                serde_json::to_writer(&mut events,&json!({"elapsed_seconds":elapsed,"snapshot_deadline_seconds":next_snapshot,"start_lateness_seconds":lateness,"snapshot_admitted":admission})).map_err(err)?;
                events.write_all(b"\n").map_err(err)?;
                next_snapshot += snapshot_seconds;
            }
            if elapsed >= next_scan as f64 {
                let lateness = elapsed - next_scan as f64;
                maximum_scan_lateness = maximum_scan_lateness.max(lateness);
                let scan = load.scan(|class, _, ns| metrics.observe(class, ns))?;
                metrics.check()?;
                if metrics.owned_bytes() > charge {
                    return Err("layout_diagnostics_exceeds_charge".into());
                }
                serde_json::to_writer(&mut events,&json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"scan_deadline_seconds":next_scan,"scan_ready_age_seconds":lateness,"scan":scan})).map_err(err)?;
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
        + snapshots.capacity() * size_of::<SnapshotSummary>();
    let diagnostic_charged = diagnostic_allocated + 256 * 1024;
    if diagnostic_charged > MIB {
        return Err("layout_complete_diagnostics_cap".into());
    }
    let metrics = metrics.report()?;
    let manifest = load.manifest();
    let report = json!({"schema":"mycity-layout-probe-v2","snapshot_summaries_only":true,"diagnostics":{"allocated_owned_bytes":diagnostic_allocated,"transient_reservation_bytes":256*1024,"charged_bound_bytes":diagnostic_charged,"cap_bytes":MIB,"full_snapshot_file_details":"events.jsonl and retained snapshot indices"},"source_sha256":env!("MYCITY_SOURCE_SHA256"),"requested_wall_seconds":wall_seconds,"work_wall_seconds":started.elapsed().as_secs_f64(),"initialization_seconds":init_seconds,"snapshot_interval_seconds":snapshot_seconds,"scans":scans,"expected_published_scans":wall_seconds.max(1),"all_published_scans_completed":scans==wall_seconds.max(1),"manifest":manifest,"snapshots":snapshots,"maximum_scan_ready_age_seconds":maximum_scan_lateness,"maximum_snapshot_start_lateness_seconds":maximum_snapshot_start_lateness,"compute":metrics,"diagnostic_outcome":outcome,"full_layout_assembled":true,"steady_gate":"not_accepted_requires_three_runs_and_group_system_measurements","eight_hour_gate":"not_run","memory_16gb_gate":"not_run","full_M0":"not_accepted"});
    let mut file = File::create(dir.join("report.json")).map_err(err)?;
    serde_json::to_writer_pretty(&mut file, &report).map_err(err)?;
    file.sync_all().map_err(err)?;
    if let Err(reason) = outcome {
        return Err(format!("layout_probe_failed:{reason}"));
    }
    Ok(report)
}

impl Table {
    fn exact(self) -> Self {
        match self {
            Self::W64(v) => Self::W64(v.into_boxed_slice().into_vec()),
            Self::W128(v) => Self::W128(v.into_boxed_slice().into_vec()),
            Self::W256(v) => Self::W256(v.into_boxed_slice().into_vec()),
            Self::PathRefs(v) => Self::PathRefs(v.into_boxed_slice().into_vec()),
        }
    }
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
        let raw = mycity_core::read_bounded(
            File::open(snapshot_dir.join("index.json")).map_err(err)?,
            256 * 1024,
        )
        .map_err(err)?;
        let index: Value = serde_json::from_slice(&raw).map_err(err)?;
        let manifest = &index["manifest"];
        if !["mycity-layout-snapshot-v1", "mycity-layout-snapshot-v2"]
            .iter()
            .any(|version| index["schema"] == *version)
            || manifest["schema"] != "mycity-l1-layout-v1"
            || manifest["acceptance_sha256"] != mycity_core::sha256(ACCEPTANCE)
        {
            return Err("layout_restore_schema".into());
        }
        let normal_bytes = manifest["normal_joint_budget_bytes"]
            .as_u64()
            .ok_or("layout_restore_normal")? as usize;
        if normal_bytes != NORMAL_BYTES {
            return Err("layout_restore_full_scope".into());
        }
        let external_charge = manifest["external_owned_charge_bytes"]
            .as_u64()
            .ok_or("layout_restore_external")? as usize;
        let metadata_charge = manifest["metadata_and_io_charge_bytes"]
            .as_u64()
            .ok_or("layout_restore_metadata")? as usize;
        if metadata_charge != 4 * MIB
            || external_charge > 64 * MIB
            || manifest["cow_reserve_bytes"] != (COW_PAGES * MIB) as u64
        {
            return Err("layout_restore_work_budget".into());
        }
        let iterations = manifest["mutation_iteration"]
            .as_u64()
            .ok_or("layout_restore_missing_iteration")?;
        if iterations > 31 * 86400 {
            return Err("layout_restore_diagnostic_horizon".into());
        }
        let entries: Vec<FileEntry> =
            serde_json::from_value(index["files"].clone()).map_err(err)?;
        let mut tables = tables()?;
        let mut expected = vec![
            "native.json".to_string(),
            "pressure.bin".into(),
            "work-reserve.bin".into(),
        ];
        expected.extend(tables.iter().map(|t| format!("{}.json", t.name)));
        expected.sort();
        let mut actual: Vec<_> = entries.iter().map(|e| e.name.clone()).collect();
        actual.sort();
        if actual != expected {
            return Err("layout_restore_file_set".into());
        }
        for entry in &entries {
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
        let (native, bound) = read_verified(
            &snapshot_dir.join("native.json"),
            entry("native.json")?,
            |reader| NativePressure::restore_checkpoint_profiled(reader),
        )?;
        if serde_json::to_value(native.manifest()).map_err(err)? != manifest["native"] {
            return Err("layout_restore_native_shape".into());
        }
        for table in &mut tables {
            let name = format!("{}.json", table.name);
            let candidate: Table =
                read_verified(&snapshot_dir.join(&name), entry(&name)?, |reader| {
                    serde_json::from_reader(reader).map_err(err)
                })?;
            if candidate != table.table {
                return Err(format!("layout_restore_stub:{}", table.name));
            }
            table.table = candidate.exact();
        }
        let payload = manifest["padding_payload_bytes"]
            .as_array()
            .ok_or("layout_restore_padding")?;
        if payload.len() != 7 {
            return Err("layout_restore_domain_count".into());
        }
        let total_payload: usize = payload
            .iter()
            .map(|v| {
                v.as_u64()
                    .map(|n| n as usize)
                    .ok_or("layout_restore_payload_number")
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();
        if total_payload
            + native.manifest().owned_capacity_bytes
            + tables.iter().map(|t| t.table.owned()).sum::<usize>()
            + COW_PAGES * MIB
            + metadata_charge
            + external_charge
            != normal_bytes
        {
            return Err("layout_restore_joint_budget".into());
        }
        let pages = read_verified(
            &snapshot_dir.join("pressure.bin"),
            entry("pressure.bin")?,
            |reader| {
                let mut pages = Vec::with_capacity(total_payload.div_ceil(MIB) + 7);
                for (domain, bytes) in payload.iter().enumerate() {
                    let mut remaining =
                        bytes.as_u64().ok_or("layout_restore_domain_payload")? as usize;
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
                Ok(pages)
            },
        )?;
        if entry("work-reserve.bin")?.bytes != (COW_PAGES * MIB) as u64 {
            return Err("layout_restore_work_size".into());
        }
        let reserve_pages = read_verified(
            &snapshot_dir.join("work-reserve.bin"),
            entry("work-reserve.bin")?,
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
                Ok(pages)
            },
        )?;
        let generation = index["generation"]
            .as_u64()
            .ok_or("layout_restore_generation")?;
        if generation == 0 {
            return Err("layout_restore_zero_generation".into());
        }
        let report = json!({"cold_restore_seconds":started.elapsed().as_secs_f64(),"native_restore_bound":bound,"producer_source_sha256":manifest["source_sha256"],"reader_source_sha256":env!("MYCITY_SOURCE_SHA256"),"same_source":manifest["source_sha256"]==env!("MYCITY_SOURCE_SHA256"),"codec":"layout-v2; full native shape/capacity and immutable stub equality; explicit mutation counter required; one-pass hash+load","restored_generation":generation,"restored_mutation_iteration":iterations,"all_payload_files_hashed_and_loaded":true,"byte_length":entries.iter().map(|e|e.bytes).sum::<u64>()});
        Ok((
            Self {
                native: Arc::new(native),
                tables: Arc::new(tables),
                pages,
                reserve: Arc::new(Mutex::new(Reserve {
                    pages: reserve_pages,
                    loan_pages: 0,
                })),
                iterations,
                cow_pages: 0,
                normal_bytes,
                external_charge,
                metadata_charge,
                job: None,
                root: root.into(),
                generation,
                _root_lock: root_lock,
            },
            report,
        ))
    }
}
pub fn reopen(snapshot_dir: &Path, report_dir: &Path) -> Result<Value, String> {
    reopen_inner(snapshot_dir, report_dir, true)
}
pub fn load_only(snapshot_dir: &Path, report_dir: &Path) -> Result<Value, String> {
    reopen_inner(snapshot_dir, report_dir, false)
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

#[cfg(test)]
mod tests {
    use super::*;
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
            (64 * MIB + 1, 4 * MIB, COW_PAGES * MIB),
            (usize::MAX, 4 * MIB, COW_PAGES * MIB),
            (64 * MIB, 0, COW_PAGES * MIB),
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
    fn real_cow_releases_placeholder_before_copy_and_preserves_snapshot() {
        let original = Arc::new(vec![7u8; MIB]);
        let mut current = original.clone();
        let mut reserve = Reserve {
            pages: (0..2).map(|_| vec![0xa5; MIB]).collect(),
            loan_pages: 0,
        };
        assert!(reserve.loan(3).is_err());
        assert_eq!(reserve.pages.len(), 2);
        reserve.loan(1).unwrap();
        Arc::make_mut(&mut current).fill(8);
        assert!(original.iter().all(|v| *v == 7));
        assert!(current.iter().all(|v| *v == 8));
        assert_eq!(
            reserve.pages.len() * MIB + current.len() + original.len(),
            3 * MIB
        );
        drop(original);
        reserve.repay(1).unwrap();
        assert_eq!(reserve.pages.len(), 2);
        assert_eq!(reserve.loan_pages, 0);
        assert!(reserve.repay(1).is_err());
    }
}
