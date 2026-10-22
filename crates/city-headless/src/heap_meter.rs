//! Whole-process requested Rust heap high-water mark, separate from RSS/footprint.
//! Alloc+copy realloc overlap is counted. Active roles use a fixed first-fit arena.
//! Reserved arena/prefix are reported separately; never add them to requested
//! bytes to invent RSS or deduct common diagnostics twice.
//! No reset is used. Native allocator metadata, stacks and foreign allocations
//! remain outside this counter and are never reported as measured by it.
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::OnceLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
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
/// Process-local monotonic reading. Cross-process epochs are never assumed equal.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ClockReading {
    pub pid: u32,
    pub instance_unix_ns: u128,
    pub elapsed_ns: u64,
}
pub fn clock_reading() -> ClockReading {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    ClockReading {
        pid: std::process::id(),
        instance_unix_ns: process_instance(),
        elapsed_ns: EPOCH
            .get_or_init(Instant::now)
            .elapsed()
            .as_nanos()
            .try_into()
            .expect("process clock horizon"),
    }
}
static FIXED_HELPER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Called before fixed helper workspace creation. No peak reset or subtraction.
pub(crate) fn fixed_helper() {
    FIXED_HELPER.store(true, std::sync::atomic::Ordering::Relaxed);
}
pub const TAIL_RESERVED_BYTES: usize = 64 * 1024;
#[global_allocator]
static ALLOCATOR: mycity_role_allocator::Allocator = mycity_role_allocator::Allocator::new();
#[derive(Serialize)]
pub struct Snapshot {
    pub current_requested_bytes: usize,
    pub lifetime_peak_requested_bytes: usize,
}
/// Isolated real L0 suite child. No pool is concurrently retained in this scenario.
pub fn activate_standalone_prototype() -> Result<(), String> {
    if std::env::var_os("MYCITY_NORMAL_POOL").is_some() {
        return Err("steady_prototype_unexpected_pool".into());
    }
    activate_role(59 * 1024 * 1024).map(|_| ())
}
pub(crate) fn activate_role(bytes: usize) -> Result<mycity_role_allocator::Activation, String> {
    ALLOCATOR.activate(bytes).map_err(str::to_owned)
}
pub(crate) fn arena_snapshot() -> mycity_role_allocator::Snapshot {
    ALLOCATOR.snapshot()
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
    role_arena: mycity_role_allocator::Snapshot,
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
            "native-metrics-probe" => Some(crate::diagnostic_contract::OBSERVER),
            "scale-model-task" if FIXED_HELPER.load(std::sync::atomic::Ordering::Relaxed) => {
                Some(crate::worker_slots::OWNER_BYTES + crate::diagnostic_contract::HELPER)
            }
            "scale-sim-probe" | "scale-endurance" | "speed-driver" | "steady-driver" => {
                Some(crate::diagnostic_contract::DRIVER)
            }
            "joint-budget-probe" | "joint-endurance" | "speed-run" | "pe0-speed-probe"
            | "steady-run" => Some(crate::diagnostic_contract::SUPERVISOR),
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
        if file.metadata().map_err(|e| e.to_string())?.len() + 2048 > 8 * 1024 * 1024 {
            return Err("heap_log_capacity".into());
        }
        let record=Record{schema:"mycity-rust-heap-v1",pid:std::process::id(),instance_unix_ns:self.instance,role:&self.role,source_sha256:env!("MYCITY_SOURCE_SHA256"),success,heap:snapshot(),role_arena:arena_snapshot(),tail_reserved_bytes:TAIL_RESERVED_BYTES,scope:"Rust heap through final record construction; 64KiB tail reserved, not exact exit peak; realloc copies counted; no reset; native/stack/mapped/foreign excluded"};
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
