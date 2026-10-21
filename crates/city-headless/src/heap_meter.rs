//! Whole-process requested Rust heap high-water mark, separate from RSS/footprint.
//! PeakAlloc's alloc+copy realloc path is part of this measured binary's cost.
//! No reset is used. Native allocator metadata, stacks and foreign allocations
//! remain outside this counter and are never reported as measured by it.
use serde::Serialize;
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};
/// Shared by heap records and all diagnostic producers in this process. Directory
/// uniqueness still comes from create_dir, not clock resolution or PID alone.
pub fn process_instance() -> u128 {
    static INSTANCE: OnceLock<u128> = OnceLock::new();
    *INSTANCE.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    })
}
static FIXED_HELPER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Called before fixed helper workspace creation. No peak reset or subtraction.
pub(crate) fn fixed_helper() {
    FIXED_HELPER.store(true, std::sync::atomic::Ordering::Relaxed);
}
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
            instance: process_instance(),
        }
    }
    pub fn finish(&self, success: bool) -> Result<(), String> {
        let cap = match self.role.as_str() {
            "scale-model-task" if FIXED_HELPER.load(std::sync::atomic::Ordering::Relaxed) => {
                Some(crate::worker_slots::OWNER_BYTES + crate::diagnostic_contract::HELPER)
            }
            "scale-sim-probe" | "scale-endurance" | "speed-driver" => {
                Some(crate::diagnostic_contract::DRIVER)
            }
            "joint-budget-probe" | "joint-endurance" | "speed-run" | "pe0-speed-probe" => {
                Some(crate::diagnostic_contract::SUPERVISOR)
            }
            "normal-pool-host" => {
                Some(crate::normal_pool::BYTES + crate::normal_pool::DIAGNOSTIC_BYTES)
            }
            _ => None,
        };
        let within = cap.is_none_or(|cap| {
            snapshot()
                .lifetime_peak_requested_bytes
                .saturating_add(TAIL_RESERVED_BYTES)
                <= cap
        });
        let success = success && within;
        let Some(path) = &self.path else {
            return if within {
                Ok(())
            } else {
                Err("diagnostic_whole_process_heap_cap".into())
            };
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
        file.sync_all().map_err(|e| e.to_string())?;
        if !within {
            return Err("diagnostic_whole_process_heap_cap".into());
        }
        Ok(())
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
