//! Whole-process requested Rust heap high-water mark, separate from RSS/footprint.
//! PeakAlloc's alloc+copy realloc path is part of this measured binary's cost.
//! No reset is used. Native allocator metadata, stacks and foreign allocations
//! remain outside this counter and are never reported as measured by it.
use serde::Serialize;
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
pub const TAIL_RESERVED_BYTES: usize = 64 * 1024;
#[global_allocator]
static ALLOCATOR: peak_alloc::PeakAlloc = peak_alloc::PeakAlloc;
#[derive(Serialize)]
pub struct Snapshot {
    pub current_requested_bytes: usize,
    pub lifetime_peak_requested_bytes: usize,
}
pub fn snapshot() -> Snapshot {
    let current_requested_bytes = ALLOCATOR.current_usage();
    Snapshot {
        current_requested_bytes,
        lifetime_peak_requested_bytes: ALLOCATOR.peak_usage(),
    }
}
pub struct Recorder {
    path: Option<PathBuf>,
    role: String,
    instance: u128,
}
#[derive(Serialize)]
struct Record<'a> {
    schema: &'static str,
    pid: u32,
    instance_unix_ns: u128,
    role: &'a str,
    source_sha256: &'static str,
    success: bool,
    heap: Snapshot,
    tail_reserved_bytes: usize,
    scope: &'static str,
}
impl Recorder {
    pub fn from_env() -> Self {
        Self {
            path: std::env::var_os("MYCITY_HEAP_LOG").map(PathBuf::from),
            role: std::env::args()
                .nth(1)
                .unwrap_or_default()
                .chars()
                .take(48)
                .collect(),
            instance: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        }
    }
    pub fn finish(&self, success: bool) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !path.is_absolute() {
            return Err("heap_log_absolute_path_required".into());
        }
        let parent = path.parent().ok_or("heap_log_parent")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        fs2::FileExt::lock_exclusive(&file).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() + 1024 > 8 * 1024 * 1024 {
            return Err("heap_log_capacity".into());
        }
        let record=Record{schema:"mycity-rust-heap-v1",pid:std::process::id(),instance_unix_ns:self.instance,role:&self.role,source_sha256:env!("MYCITY_SOURCE_SHA256"),success,heap:snapshot(),tail_reserved_bytes:TAIL_RESERVED_BYTES,scope:"Rust heap through final record construction; 64KiB tail reserved, not exact exit peak; realloc copies counted; no reset; native/stack/mapped/foreign excluded"};
        serde_json::to_writer(&mut file, &record).map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    }
}

pub fn probe(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir(dir).map_err(|e| e.to_string())?;
    let before = snapshot();
    let data = vec![7u8; 2 * 1024 * 1024];
    std::hint::black_box(data.as_slice());
    let allocated = snapshot();
    drop(data);
    let released = snapshot();
    if allocated.current_requested_bytes < before.current_requested_bytes + 2 * 1024 * 1024
        || released.current_requested_bytes + 2 * 1024 * 1024 > allocated.current_requested_bytes
        || released.lifetime_peak_requested_bytes < allocated.current_requested_bytes
    {
        return Err("heap_meter_round_trip".into());
    }
    let mut file = std::fs::File::create_new(dir.join("report.json")).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, &[before, allocated, released])
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
