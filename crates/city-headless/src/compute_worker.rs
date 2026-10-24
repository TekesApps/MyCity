//! A pure, persistent air worker. No Store handle or business command is available here.
use mycity_core::{
    air::{AirWorkspace, Phase, Progress, State, CHECKPOINT_BYTES, STRATEGY},
    read_bounded, sha256,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};

pub const PROTOCOL: &str = "mycity-air-worker-v2";
pub const CONTROL_BYTES: usize = 64 * 1024;
/// IPC readers only decode fixed, non-recursive protocol records (at most one
/// `CONTROL_BYTES` frame); do not reserve the platform's default thread stack
/// for each long-lived reader. This is a requested stack reservation, not a
/// physical-memory credit or a measured RSS reduction.
pub(crate) const IPC_READER_STACK_BYTES: usize = 512 * 1024;
pub const DIRECTORY_BYTES: u64 = 32 * 1024 * 1024;
pub const DIRECTORY_FILES: usize = 32;

#[path = "worker_cpu_metrics.rs"]
pub mod cpu_metrics;

#[path = "worker_protocol_observation.rs"]
mod protocol_observation;
// Inline state only. Bounded label/digest Strings and transient JSON allocations are
// separate and remain charged to the original role arena; this is not native memory.
const _: () = assert!(std::mem::size_of::<protocol_observation::Attempt>() <= 4096);

pub(crate) fn spawn_ipc_reader(
    name: &'static str,
    task: impl FnOnce() + Send + 'static,
) -> Result<thread::JoinHandle<()>, String> {
    thread::Builder::new()
        .name(name.into())
        .stack_size(IPC_READER_STACK_BYTES)
        .spawn(task)
        .map_err(|error| format!("ipc_reader_spawn:{name}:{error}"))
}

#[cfg(test)]
pub(crate) fn kill_wait_then<T>(
    child: &mut Child,
    after_wait: impl FnOnce(&mut Child, u32, ExitStatus) -> Result<T, String>,
) -> Result<T, String> {
    let pid = child.id();
    let _ = child.kill();
    let status = child
        .wait()
        .map_err(|error| format!("ipc_reader_child_wait:{error}"))?;
    after_wait(child, pid, status)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    pub world: u64,
    pub branch: u64,
    pub job: u64,
    pub incarnation: u64,
    pub generation: u32,
    pub attempt: u8,
    pub base_revision: u64,
    pub field_version: u64,
    pub start: u32,
    pub end: u32,
    pub rules_sha256: String,
    pub source_sha256: String,
    pub input_sha256: String,
    pub checkpoint_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub strategy: String,
    pub ticket: Ticket,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Sources,
    Cells,
    Published,
    Complete,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub protocol: String,
    pub ticket: Ticket,
    pub pid: u32,
    pub stage: Stage,
    pub candidate_sha256: Option<String>,
    pub candidate_bytes: usize,
    pub cursor: Option<u32>,
    #[serde(default)]
    pub cpu: Option<cpu_metrics::CpuTelemetry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeAck {
    protocol: String,
    ticket: Ticket,
    stage: Stage,
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn frame<R: BufRead>(r: &mut R) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    // fill_buf/consume never allocates from an unbounded read_until line.
    loop {
        let available = r.fill_buf().map_err(|e| e.to_string())?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err("worker_truncated_frame".into())
            };
        }
        let n = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |n| n + 1);
        if bytes.len() + n > CONTROL_BYTES {
            return Err("worker_control_bytes".into());
        }
        let done = available[n - 1] == b'\n';
        bytes.extend_from_slice(&available[..n]);
        r.consume(n);
        if done {
            return Ok(Some(bytes));
        }
    }
}
pub(crate) fn write_frame(w: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() + 1 > CONTROL_BYTES {
        return Err("worker_control_bytes".into());
    }
    w.write_all(&bytes)
        .and_then(|_| w.write_all(b"\n"))
        .and_then(|_| w.flush())
        .map_err(|e| e.to_string())
}
fn directory_usage(dir: &Path) -> Result<u64, String> {
    let mut size = 0u64;
    for (n, entry) in fs::read_dir(dir).map_err(|e| e.to_string())?.enumerate() {
        if n >= DIRECTORY_FILES {
            return Err("worker_directory_files".into());
        }
        let e = entry.map_err(|e| e.to_string())?;
        if !e.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err("worker_nonregular_asset".into());
        }
        size = size
            .checked_add(e.metadata().map_err(|e| e.to_string())?.len())
            .ok_or("worker_directory_bytes")?;
        if size > DIRECTORY_BYTES {
            return Err("worker_directory_bytes".into());
        }
    }
    Ok(size)
}
pub fn read_asset(dir: &Path, hash: &str) -> Result<Vec<u8>, String> {
    if !digest(hash) {
        return Err("worker_asset_digest".into());
    }
    let path = dir.join(format!("{hash}.air"));
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("worker_nonregular_asset".into());
    }
    let bytes = read_bounded(
        File::open(path).map_err(|e| e.to_string())?,
        CHECKPOINT_BYTES as u64,
    )
    .map_err(|e| e.to_string())?;
    if sha256(&bytes) != hash {
        return Err("worker_asset_checksum".into());
    }
    Ok(bytes)
}
/// Read directly into the caller's fixed slot; no full temporary Vec.
pub(crate) fn read_asset_into(
    dir: &Path,
    hash: &str,
    bytes: &mut crate::worker_slots::Bytes,
) -> Result<(), String> {
    if !digest(hash) {
        return Err("worker_asset_digest".into());
    }
    let path = dir.join(format!("{hash}.air"));
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("worker_nonregular_asset".into());
    }
    bytes.read_from(File::open(path).map_err(|e| e.to_string())?)?;
    if sha256(bytes.as_slice()) != hash {
        return Err("worker_asset_checksum".into());
    }
    Ok(())
}
/// Idempotent publication verifies every byte without a second owned checkpoint.
fn verify_asset_stream(dir: &Path, hash: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let path = dir.join(format!("{hash}.air"));
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("worker_nonregular_asset".into());
    }
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash_state = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut size = 0usize;
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        size = size.checked_add(n).ok_or("worker_asset_bytes")?;
        if size > CHECKPOINT_BYTES {
            return Err("worker_asset_bytes".into());
        }
        hash_state.update(&buffer[..n]);
    }
    if format!("{:x}", hash_state.finalize()) != hash {
        return Err("worker_asset_checksum".into());
    }
    Ok(())
}
/// Immutable, durable publish. Existing equal content is idempotent, never replaced.
pub fn publish(dir: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > CHECKPOINT_BYTES {
        return Err("worker_asset_bytes".into());
    }
    let hash = sha256(bytes);
    let target = dir.join(format!("{hash}.air"));
    if target.exists() {
        verify_asset_stream(dir, &hash)?;
        File::open(&target)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        return Ok(hash);
    }
    if directory_usage(dir)?.saturating_add(bytes.len() as u64) > DIRECTORY_BYTES {
        return Err("worker_directory_bytes".into());
    }
    // Count target AND temporary publication slot before taking a responsibility.
    if fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .take(DIRECTORY_FILES)
        .count()
        > DIRECTORY_FILES - 2
    {
        return Err("worker_directory_files".into());
    }
    let temporary = dir.join(format!(".pending-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        match fs::hard_link(&temporary, &target) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                verify_asset_stream(dir, &hash)?;
            }
            Err(e) => return Err(e.to_string()),
        }
        fs::remove_file(&temporary).map_err(|e| e.to_string())?;
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(hash)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
// The bounded branch borrows its private fixed kernel; no owned State escapes it.
// Keep the legacy owned adapter inline rather than adding a Box allocation.
#[allow(clippy::large_enum_variant)]
enum TaskState<'a> {
    Owned(State),
    Fixed(&'a mut AirWorkspace),
}
impl TaskState<'_> {
    fn complete(&self) -> bool {
        match self {
            Self::Owned(s) => s.complete(),
            Self::Fixed(s) => s.complete(),
        }
    }
    fn progress(&self) -> Option<Progress> {
        match self {
            Self::Owned(s) => s.progress().cloned(),
            Self::Fixed(s) => s.progress(),
        }
    }
    fn page(&mut self, sources: u32, cells: u32) -> Result<(), String> {
        match self {
            Self::Owned(s) => s.page(sources, cells),
            Self::Fixed(s) => s.page(sources, cells),
        }
    }
    fn page_diagnosed(
        &mut self,
        sources: u32,
        cells: u32,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<(), String> {
        match self {
            Self::Owned(s) => s.page_diagnosed(sources, cells, trace),
            Self::Fixed(s) => s.page_diagnosed(sources, cells, trace),
        }
    }
    fn checkpoint_to(&mut self, writer: &mut impl Write) -> Result<usize, String> {
        match self {
            Self::Owned(s) => s.checkpoint_to(writer),
            Self::Fixed(s) => s.checkpoint_to(writer),
        }
    }
    fn legacy_checkpoint(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Owned(s) => s.checkpoint(),
            Self::Fixed(_) => Err("worker_fixed_owned_checkpoint_forbidden".into()),
        }
    }
    fn finish(&mut self, log: Option<&mut crate::cpu_log::CpuLog>) -> Result<(), String> {
        if let Self::Fixed(s) = self {
            let loaded = s.report()?;
            s.reset()?;
            let reset = s.report()?;
            if let Some(log) = log {
                log.worker_air_workspace(&loaded, &reset)?;
            }
        }
        Ok(())
    }
}
fn validate_request(request: &Request) -> Result<(), String> {
    let t = &request.ticket;
    if request.protocol != PROTOCOL
        || request.strategy != STRATEGY
        || t.source_sha256 != env!("MYCITY_SOURCE_SHA256")
        || ![&t.rules_sha256, &t.input_sha256, &t.checkpoint_sha256]
            .iter()
            .all(|s| digest(s))
        || t.world == 0
        || t.branch == 0
        || t.job == 0
        || t.incarnation == 0
        || t.generation == 0
        || !(1..=3).contains(&t.attempt)
        || t.start >= t.end
    {
        return Err("worker_ticket".into());
    }
    Ok(())
}
fn input<'a>(
    root: &Path,
    request: &Request,
    slots: Option<&mut crate::worker_slots::Owner>,
    kernel: Option<&'a mut AirWorkspace>,
) -> Result<TaskState<'a>, String> {
    let t = &request.ticket;
    directory_usage(&root.join("inputs"))?;
    match (slots, kernel) {
        (Some(owner), Some(kernel)) => {
            owner.with_slot(crate::worker_slots::Kind::Input, |bytes| {
                read_asset_into(&root.join("inputs"), &t.checkpoint_sha256, bytes)?;
                kernel.restore(bytes.as_slice())
            })?;
            if !kernel.matches_input(t.start, t.end, &t.input_sha256) {
                kernel.reset()?;
                return Err("worker_frozen_input".into());
            }
            Ok(TaskState::Fixed(kernel))
        }
        (None, None) => {
            let state = State::restore(&read_asset(&root.join("inputs"), &t.checkpoint_sha256)?)?;
            if state.report().time != t.start
                || state.input().target != t.end
                || state.progress().is_some()
                || state.input().identity() != t.input_sha256
                || state.complete()
            {
                return Err("worker_frozen_input".into());
            }
            Ok(TaskState::Owned(state))
        }
        _ => Err("worker_fixed_workspace_pair".into()),
    }
}
fn emit(
    w: &mut impl Write,
    ticket: &Ticket,
    stage: Stage,
    candidate: Option<&str>,
    size: usize,
    cursor: Option<u32>,
    cpu: Option<&mut cpu_metrics::CpuAttemptSampler>,
) -> Result<(), String> {
    write_frame(
        w,
        &Event {
            protocol: PROTOCOL.into(),
            ticket: ticket.clone(),
            pid: std::process::id(),
            stage,
            candidate_sha256: candidate.map(str::to_owned),
            candidate_bytes: size,
            cursor,
            cpu: cpu.map(|sampler| sampler.sample_before_event()),
        },
    )
}
/// Blocking child entry point. One control request at a time; stdin EOF stops it.
pub fn run(root: &Path) -> Result<(), String> {
    run_inner(root, false, false)
}
/// Fault laboratory only: explicit ACK holds each observed phase before execution continues.
pub fn run_probe_gated(root: &Path) -> Result<(), String> {
    run_inner(root, true, false)
}
fn gate(
    reader: &mut impl BufRead,
    request: &Request,
    stage: Stage,
    gated: bool,
) -> Result<(), String> {
    if !gated {
        return Ok(());
    }
    let bytes = frame(reader)?.ok_or("worker_probe_ack_eof")?;
    let ack: ProbeAck = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if ack.protocol != PROTOCOL || ack.ticket != request.ticket || ack.stage != stage {
        return Err("worker_probe_ack_identity".into());
    }
    Ok(())
}
pub fn run_bounded_probe(root: &Path) -> Result<(), String> {
    run_inner(root, true, true)
}
pub fn run_bounded(root: &Path) -> Result<(), String> {
    run_inner(root, false, true)
}
fn run_inner(root: &Path, gated: bool, bounded: bool) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::with_capacity(8192, stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    directory_usage(&root.join("inputs"))?;
    directory_usage(&root.join("candidates"))?;
    let mut diag = if crate::cpu_log::enabled() {
        let base = root
            .parent()
            .and_then(Path::parent)
            .ok_or("worker_diagnostic_parent")?
            .join("diagnostics");
        Some(crate::cpu_log::CpuLog::create(&base, "worker")?)
    } else {
        None
    };
    let mut slots: Option<crate::worker_slots::Owner> = None;
    let mut kernel: Option<AirWorkspace> = None;
    let pool_grant = if bounded {
        crate::normal_pool::WorkerGrant::from_env()?
    } else {
        None
    };
    // Explicit fault-lab omission only; ordinary execution always samples.
    let omit_cpu = gated && std::env::var_os("MYCITY_AIR_CPU_TELEMETRY_OMIT").is_some();
    while let Some(bytes) = frame(&mut reader)? {
        let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        validate_request(&request)?;
        let mut cpu = cpu_metrics::CpuAttemptSampler::begin(crate::heap_meter::process_instance());
        let _cache_fence = if bounded {
            Some(crate::worker_cache::exclusive(root)?)
        } else {
            None
        };
        if bounded {
            crate::worker_cache::actual_bytes(root)?;
            let path = root
                .join("inputs")
                .join(format!("{}.air", request.ticket.checkpoint_sha256));
            if fs::metadata(path).map_err(|e| e.to_string())?.len()
                > crate::worker_cache::ASSET_BYTES as u64
            {
                return Err("cache_input_bytes".into());
            }
        }
        if bounded {
            if let Some(owner) = slots.as_mut() {
                owner.rebind(
                    request.ticket.incarnation,
                    request.ticket.job,
                    request.ticket.generation,
                    request.ticket.attempt,
                )?;
            } else {
                let owner = crate::worker_slots::Owner::new(
                    request.ticket.incarnation,
                    request.ticket.job,
                    request.ticket.generation,
                    request.ticket.attempt,
                )?;
                let air = AirWorkspace::new()?;
                if let Some(grant) = pool_grant.as_ref() {
                    grant.filled(&owner, &air)?;
                }
                slots = Some(owner);
                kernel = Some(air);
            }
        }
        if let (Some(log), Some(owner)) = (diag.as_mut(), slots.as_ref()) {
            log.set_external_diagnostics_owned_bytes(owner.metadata_charge_bytes())?;
        }
        let mut state = input(root, &request, slots.as_mut(), kernel.as_mut())?;
        if let Some(log) = diag.as_mut() {
            log.begin(serde_json::json!({"ticket":request.ticket,"kind":"worker_computation"}))?;
        }
        let mut source_seen = false;
        let mut cells_seen = false;
        while !state.complete() {
            if let Some(log) = diag.as_mut() {
                let mut sink = |p| log.page(p);
                let mut trace = mycity_core::diagnostics::Trace::with_pages(&mut sink);
                state.page_diagnosed(if bounded && gated { 1 } else { 256 }, 256, &mut trace)?;
                log.check()?;
            } else {
                state.page(if bounded && gated { 1 } else { 256 }, 256)?;
            }
            if let Some(p) = state.progress() {
                let (seen, stage) = match p.phase {
                    Phase::Sources => (&mut source_seen, Stage::Sources),
                    Phase::Cells => (&mut cells_seen, Stage::Cells),
                };
                if !*seen && p.cursor > 0 {
                    emit(&mut writer, &request.ticket, stage, None, 0, Some(p.cursor), (!omit_cpu).then_some(&mut cpu))?;
                    gate(&mut reader, &request, stage, gated)?;
                    *seen = true;
                }
            }
        }
        let (hash, size) = if let Some(owner) = slots.as_mut() {
            owner.with_slot(crate::worker_slots::Kind::Candidate, |bytes| {
                let size = state.checkpoint_to(bytes)?;
                let hash = crate::worker_cache::publish(root, "candidates", bytes.as_slice())?;
                Ok((hash, size))
            })?
        } else {
            let bytes = state.legacy_checkpoint()?;
            let hash = publish(&root.join("candidates"), &bytes)?;
            (hash, bytes.len())
        };
        emit(
            &mut writer,
            &request.ticket,
            Stage::Published,
            Some(&hash),
            size,
            None,
            (!omit_cpu).then_some(&mut cpu),
        )?;
        gate(&mut reader, &request, Stage::Published, gated)?;
        state.finish(diag.as_mut())?;
        if let Some(owner) = slots.as_ref() {
            owner.check_balance()?;
            if let Some(log) = diag.as_mut() {
                log.worker_controlled_slots(&owner.report()?)?;
            }
        }
        if let Some(log) = diag.as_mut() {
            log.worker_complete(
                serde_json::json!({"ticket":request.ticket,"candidate_sha256":hash}),
            )?;
        }
        emit(
            &mut writer,
            &request.ticket,
            Stage::Complete,
            Some(&hash),
            size,
            None,
            (!omit_cpu).then_some(&mut cpu),
        )?;
    }
    if let Some(log) = diag.as_mut() {
        log.finish()?;
    }
    Ok(())
}

/// One child and a four-frame bounded reader. stderr is discarded, never an unbounded pipe.
/// The owning Sim supplies durable retries; this transport does not retry or mutate a world.
enum Input {
    Pipe(ChildStdin),
    Socket(std::os::unix::net::UnixStream),
}
impl Write for Input {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Pipe(s) => s.write(bytes),
            Self::Socket(s) => s.write(bytes),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Pipe(s) => s.flush(),
            Self::Socket(s) => s.flush(),
        }
    }
}
#[must_use = "hold the owner through start errors and acknowledged retirement"]
pub struct Worker {
    // Captured from this actual supervising process, never derived from a task/PID.
    // Observation metadata only; the existing owned Child/lease remains authoritative.
    owner_process_instance: u128,
    observed_process_instance: Option<u128>,
    observation: Option<protocol_observation::Attempt>,
    observation_error: Option<&'static str>,
    child: Option<Child>,
    broker: Option<crate::normal_pool::broker::Lease>,
    stdin: Option<Input>,
    receiver: Option<Receiver<Result<Event, String>>>,
    reader: Option<JoinHandle<()>>,
    outstanding: Option<Ticket>,
    root: PathBuf,
    gated: bool,
    pool_grant: Option<crate::normal_pool::Grant>,
    pending_grant: Option<crate::normal_pool::PendingGrant>,
    broker_start: Option<crate::normal_pool::broker::PendingStart>,
    reaped: Option<ExitStatus>,
    ready_to_submit: bool,
}
impl Worker {
    /// Empty persistent owner; no RPC, process spawn or capacity mutation.
    pub fn new() -> Self {
        Self { owner_process_instance: crate::heap_meter::process_instance(),
            observation: None, observation_error: None, observed_process_instance: None,
            child: None, broker: None, stdin: None, receiver: None, reader: None,
            outstanding: None, root: PathBuf::new(), gated: false, pool_grant: None,
            pending_grant: None, broker_start: None, reaped: None, ready_to_submit: false }
    }
    pub fn is_empty(&self) -> bool {
        self.child.is_none() && self.broker.is_none() && self.pending_grant.is_none()
            && self.pool_grant.is_none() && self.broker_start.is_none() && self.reader.is_none()
    }
    pub fn start_unbounded(&mut self, executable: &Path, root: &Path, gated: bool)
        -> Result<(), String> {
        self.start(executable, root, gated, false, None)
    }
    pub fn start_bounded(&mut self, executable: &Path, root: &Path,
        pool: Option<&Path>, gated: bool) -> Result<(), String> {
        self.start(executable, root, gated, true, pool)
    }
    fn install_reader(&mut self, stdout: impl Read + Send + 'static) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(4);
        self.receiver = Some(receiver);
        self.reader = Some(spawn_ipc_reader("worker-ipc-reader", move || {
            let mut r = BufReader::with_capacity(8192, stdout);
            loop {
                let result = match frame(&mut r) {
                    Ok(Some(b)) => serde_json::from_slice::<Event>(&b).map_err(|e| e.to_string()),
                    Ok(None) => Err("worker_eof".into()),
                    Err(e) => Err(e),
                };
                let failed = result.is_err();
                if sender.send(result).is_err() || failed { break; }
            }
        })?);
        self.ready_to_submit = true;
        Ok(())
    }
    fn start(&mut self, executable: &Path, root: &Path, gated: bool,
        bounded: bool, pool: Option<&Path>) -> Result<(), String> {
        if !self.is_empty() { return Err("worker_owner_not_retired".into()); }
        self.observation = None;
        self.observation_error = None;
        self.observed_process_instance = None;
        self.root = root.to_owned();
        self.gated = gated;
        self.ready_to_submit = false;
        if bounded && pool.is_none() {
            self.broker_start = crate::normal_pool::broker::PendingStart::from_env(root, gated)?;
            if self.broker_start.is_some() {
                // Store Started lease BEFORE accept, clone, or reader setup can fail.
                self.broker = Some(self.broker_start.as_mut().expect("start owner").start()?);
                let stream = self.broker_start.as_mut().expect("start owner").accept()?;
                self.stdin = Some(Input::Socket(stream));
                let Some(Input::Socket(stream)) = self.stdin.as_ref() else { unreachable!() };
                let stdout = stream.try_clone().map_err(|e| e.to_string())?;
                return self.install_reader(stdout);
            }
        }
        if bounded {
            let pool_root = pool.map(Path::to_owned)
                .or_else(|| std::env::var_os("MYCITY_NORMAL_POOL").map(PathBuf::from));
            if let Some(root) = pool_root {
                self.pending_grant = Some(crate::normal_pool::PendingGrant::new(&root)?);
                self.pool_grant = Some(self.pending_grant.as_mut().expect("prepare owner").resolve()?);
                self.pending_grant = None; // resolved request has transferred, no capability lost
            }
        }
        let mut command = Command::new(executable);
        command.arg(if gated && bounded { "compute-worker-bounded-probe" }
            else if gated { "compute-worker-probe-gated" }
            else if bounded { "compute-worker-bounded" } else { "compute-worker" })
            .arg(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Some(grant) = self.pool_grant.as_ref() { grant.configure(&mut command)?; }
        // From this assignment onward the original Child never lives in a fallible local.
        self.child = Some(command.spawn().map_err(|e| e.to_string())?);
        if let Some(grant) = self.pool_grant.as_mut() {
            grant.attach(self.child.as_ref().expect("child owner").id())?;
        }
        let child = self.child.as_mut().expect("child owner");
        self.stdin = Some(Input::Pipe(child.stdin.take().ok_or("worker_stdin")?));
        let stdout = child.stdout.take().ok_or("worker_stdout")?;
        self.install_reader(stdout)
    }
    /// Absence means no known worker PID, not a synthetic zero or proof of retirement.
    pub fn pid_opt(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id).or_else(|| self.broker.as_ref().map(|lease| lease.pid()))
    }
    pub fn requires_session_exit(&self) -> bool {
        self.broker.is_none()
            && self.broker_start.as_ref().is_some_and(|start| start.unconfirmed())
    }
    pub fn retirement_state(&self) -> serde_json::Value {
        serde_json::json!({"worker_pid": self.pid_opt(),
            "pending_prepare": self.pending_grant.is_some(),
            "direct_grant": self.pool_grant.as_ref().map(|grant| grant.protocol_identity()),
            "broker_lease": self.broker_lease(),
            "broker_start_unconfirmed": self.broker_start.as_ref().is_some_and(|start| start.unconfirmed()),
            "actual_exit_cached": self.reaped.is_some(), "ready_to_submit": self.ready_to_submit,
            "empty": self.is_empty()})
    }
    /// The SAME owner retries. Never clear an uncertain Prepare/Start or a failed Reap.
    pub fn retire(&mut self, timeout: Duration) -> Result<Option<ExitStatus>, String> {
        if self.is_empty() { return Ok(None); }
        if self.requires_session_exit() {
            return Err("worker_broker_start_unconfirmed_session_exit_required".into());
        }
        let status = if self.child.is_some() || self.broker.is_some() {
            Some(self.stop_when_idle(timeout)?)
        } else {
            if self.pending_grant.is_some() {
                self.pool_grant = Some(self.pending_grant.as_mut().expect("prepare owner").resolve()?);
                self.pending_grant = None;
            }
            if let Some(grant) = self.pool_grant.as_mut() { grant.abort_unspawned()?; }
            None
        };
        // Only acknowledged retirement reaches this point. Child status is not a substitute
        // for original pool acknowledgement. Reader has joined on the spawned path.
        self.child = None; self.broker = None; self.pool_grant = None;
        self.pending_grant = None; self.broker_start = None; self.reaped = None;
        self.stdin = None; self.receiver = None; self.outstanding = None;
        self.ready_to_submit = false;
        Ok(status)
    }
    /// Only the probe transport uses ACKs; ordinary workers never wait for one.
    pub fn acknowledge_probe(&mut self, event: &Event) -> Result<(), String> {
        if !self.gated {
            return Ok(());
        }
        if Some(&event.ticket) != self.outstanding.as_ref() || event.stage == Stage::Complete {
            return Err("worker_probe_ack_identity".into());
        }
        write_frame(
            self.stdin.as_mut().ok_or("worker_stopped")?,
            &ProbeAck {
                protocol: PROTOCOL.into(),
                ticket: event.ticket.clone(),
                stage: event.stage,
            },
        )
    }
    pub fn pid(&self) -> u32 {
        self.pid_opt().expect("known spawned worker PID")
    }
    pub fn broker_lease(&self) -> Option<u64> {
        self.broker.as_ref().map(|lease| lease.id())
    }
    fn close_input(&mut self) {
        if let Some(Input::Socket(s)) = self.stdin.as_ref() {
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
        self.stdin.take();
    }
    pub fn submit(&mut self, request: &Request) -> Result<(), String> {
        if !self.ready_to_submit { return Err("worker_not_ready".into()); }
        if self.outstanding.is_some() {
            return Err("worker_busy".into());
        }
        let sent_at = std::time::Instant::now();
        write_frame(self.stdin.as_mut().ok_or("worker_stopped")?, request)?;
        self.outstanding = Some(request.ticket.clone());
        match protocol_observation::Attempt::begin(request, self, sent_at) {
            Ok(attempt) => { self.observation = Some(attempt); self.observation_error = None; }
            Err(reason) => { self.observation = None; self.observation_error = Some(reason); }
        }
        Ok(())
    }
    /// Latest attempt observation only. It is not a live-owner, Store, or retirement receipt.
    /// Missing/failed projection never changes required work or the canonical wire checks.
    pub fn task_observation(&self) -> serde_json::Value {
        let data = self.observation.as_ref().and_then(|attempt| attempt.latest());
        serde_json::json!({"schema":"mycity-cpu-task-observation-v2",
            "state":if self.observation_error.is_some() {"unavailable"} else if data.is_some() {"observed"} else {"not_yet_observed"},
            "reason":self.observation_error, "data":data,
            "owner_retired_now":self.is_empty(), "retained_last_attempt":true,
            "authority":"observation_only_not_grant_store_acceptance_or_resource_return",
            "queue_age":"unknown_not_instrumented"})
    }
    pub fn next(&mut self, timeout: Duration) -> Result<Event, String> {
        let event = self
            .receiver
            .as_ref()
            .ok_or("worker_stopped")?
            .recv_timeout(timeout)
            .map_err(|e| e.to_string())??;
        let received_at = std::time::Instant::now();
        if event.protocol != PROTOCOL
            || event.pid != self.pid()
            || Some(&event.ticket) != self.outstanding.as_ref()
            || event.candidate_bytes > CHECKPOINT_BYTES
        {
            return Err("worker_response_identity".into());
        }
        match event.stage {
            Stage::Sources | Stage::Cells
                if event.candidate_sha256.is_none()
                    && event.candidate_bytes == 0
                    && event.cursor.is_some_and(|c| c > 0) => {}
            Stage::Published | Stage::Complete if event.cursor.is_none() => {
                let hash = event
                    .candidate_sha256
                    .as_deref()
                    .ok_or("worker_missing_candidate")?;
                let bytes = read_asset(&self.root.join("candidates"), hash)?;
                if bytes.len() != event.candidate_bytes {
                    return Err("worker_candidate_size".into());
                }
                if event.stage == Stage::Complete {
                    self.outstanding = None;
                }
            }
            _ => return Err("worker_response_shape".into()),
        }
        // Only after the existing full-ticket and actual asset/hash/length checks.
        // An unavailable observation must never turn valid business into a rejection.
        if let Some(cpu) = event.cpu.as_ref().filter(|cpu| cpu.validate().is_ok()) {
            if let Some(instance) = cpu.process_instance.as_deref().and_then(|s| s.parse::<u128>().ok()) {
                // Never replace a known application instance on a still-owned Child/Lease.
                if self.observed_process_instance.is_none() { self.observed_process_instance = Some(instance); }
            }
        }
        if let Some(mut attempt) = self.observation.take() {
            match attempt.observe_after_next(self, &event, received_at) {
                Ok(_) => self.observation = Some(attempt),
                Err(reason) => self.observation_error = Some(reason),
            }
        }
        Ok(event)
    }
    /// Completed workers may close stdin and flush their final evidence. Active
    /// jobs retain the existing kill path. Deadline expiry is still a real kill.
    pub fn stop_when_idle(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        if self.outstanding.is_some() { return self.stop(); }
        self.ready_to_submit = false;
        self.close_input();
        self.receiver.take();
        if let Some(lease) = self.broker.as_mut() {
            let status = lease.stop(false)?;
            self.reaped = Some(status); // broker's original waited receipt, not local Child wait
            self.join_reader()?;
            return Ok(status);
        }
        if self.child.is_none() { return Err("worker_no_spawned_owner_use_retire".into()); }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.reaped.is_none() {
                self.reaped = self.child.as_mut().expect("direct owner")
                    .try_wait().map_err(|e| e.to_string())?;
            }
            if self.reaped.is_some() { return self.publish_direct_wait(); }
            if std::time::Instant::now() >= deadline { return self.stop(); }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn join_reader(&mut self) -> Result<(), String> {
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "worker_reader_panic")?;
        }
        Ok(())
    }
    fn publish_direct_wait(&mut self) -> Result<ExitStatus, String> {
        let status = self.reaped.ok_or("worker_actual_wait_missing")?;
        if let Some(grant) = self.pool_grant.as_mut() {
            grant.after_wait(self.child.as_ref().ok_or("worker_child_owner_missing")?.id(), status)?;
        }
        self.join_reader()?;
        Ok(status)
    }
    pub fn stop(&mut self) -> Result<ExitStatus, String> {
        self.ready_to_submit = false;
        self.close_input();
        self.receiver.take(); // unblock bounded sender before join
        if let Some(lease) = self.broker.as_mut() {
            let status = lease.stop(true)?;
            self.reaped = Some(status);
            self.join_reader()?;
            return Ok(status);
        }
        let child = self.child.as_mut().ok_or("worker_no_spawned_owner_use_retire")?;
        if self.reaped.is_none() {
            self.reaped = child.try_wait().map_err(|e| e.to_string())?;
            if self.reaped.is_none() {
                // A kill error is not a wait result. Still attempt real wait; if wait
                // fails, original Child/Grant stay in this owner for explicit retry.
                if let Err(error) = child.kill() {
                    self.reaped = child.try_wait().map_err(|e| e.to_string())?;
                    if self.reaped.is_none() { return Err(format!("worker_kill_unconfirmed:{error}")); }
                }
                if self.reaped.is_none() {
                    self.reaped = Some(child.wait().map_err(|e| e.to_string())?);
                }
            }
        }
        self.publish_direct_wait()
    }
}
impl Default for Worker { fn default() -> Self { Self::new() } }
impl Drop for Worker {
    fn drop(&mut self) {
        if let Err(error) = self.retire(Duration::ZERO) {
            // Best effort only. Reliable normal-error/unwind retirement belongs to
            // with_worker; forced process death needs the existing external owner.
            eprintln!("worker_drop_retirement_unconfirmed:{error}");
        }
    }
}

/// Keep one original owner outside the work closure, including during unwind.
/// Two finite attempts recover one-shot transport faults; persistent errors are
/// explicit failures, never 'cleaned up'. Managed Sim must then exit its session
/// so original supervisor wait/revoke can run. Standalone probes report unrecovered
/// responsibility and terminate; they have no fictional external recovery owner.
pub fn with_worker<T>(work: impl FnOnce(&mut Worker) -> Result<T, String>) -> Result<T, String> {
    let mut owner = Worker::new();
    with_worker_on(&mut owner, work)
}
/// An outer session/fixture may retain this exact owner even if retirement fails
/// and the original panic is resumed. This does not manufacture a recovery token.
pub fn with_worker_on<T>(owner: &mut Worker,
    work: impl FnOnce(&mut Worker) -> Result<T, String>) -> Result<T, String> {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(owner)));
    let mut retirement = owner.retire(Duration::from_secs(1));
    if let Err(error) = retirement.as_ref() {
        eprintln!("worker_retirement_retry:{error}:{}", owner.retirement_state());
        retirement = owner.retire(Duration::from_secs(1));
    }
    match outcome {
        Err(panic) => {
            if let Err(error) = retirement {
                eprintln!("worker_unwind_retirement_unconfirmed:{error}:{}", owner.retirement_state());
            }
            std::panic::resume_unwind(panic)
        }
        Ok(result) => match retirement {
            Ok(_) => result,
            Err(error) => Err(format!("worker_retirement_unconfirmed:{error}:{};work_error:{}",
                owner.retirement_state(), result.err().as_deref().unwrap_or("none"))),
        },
    }
}

/// First implementation deliberately recomputes the CPU result before accepting it.
/// This validates pure numerical output only; Sim must additionally verify its durable ticket.
pub fn verify_candidate(request: &Request, base: &State, bytes: &[u8]) -> Result<State, String> {
    let t = &request.ticket;
    if sha256(&base.checkpoint()?) != t.checkpoint_sha256
        || base.input().identity() != t.input_sha256
        || base.report().time != t.start
        || base.input().target != t.end
    {
        return Err("worker_base_identity".into());
    }
    let candidate = State::restore(bytes)?;
    if !candidate.complete() || candidate.input() != base.input() {
        return Err("worker_incomplete_candidate".into());
    }
    let mut reference = base.clone();
    while !reference.complete() {
        reference.page(8192, 1024)?;
    }
    if candidate != reference {
        return Err("worker_cpu_mismatch".into());
    }
    Ok(candidate)
}

#[cfg(test)]
mod fixed_asset_tests {
    use super::*;
    #[test]
    fn candidate_file_reads_directly_into_slot_and_failure_returns_real_capacity() {
        let root = std::env::temp_dir().join(format!(
            "sim-codec-{}-{}",
            std::process::id(),
            crate::heap_meter::process_instance()
        ));
        fs::create_dir(&root).unwrap();
        let data = b"bounded original payload";
        let hash = sha256(data);
        fs::write(root.join(format!("{hash}.air")), data).unwrap();
        let mut owner = crate::worker_slots::Owner::new(1, 1, 1, 1).unwrap();
        owner
            .with_slot(crate::worker_slots::Kind::Candidate, |bytes| {
                read_asset_into(&root, &hash, bytes)?;
                assert_eq!(bytes.as_slice(), data);
                Ok(())
            })
            .unwrap();
        let wrong = "0".repeat(64);
        fs::write(root.join(format!("{wrong}.air")), data).unwrap();
        assert!(owner
            .with_slot(crate::worker_slots::Kind::Candidate, |bytes| {
                read_asset_into(&root, &wrong, bytes)
            })
            .is_err());
        owner.check_balance().unwrap();
        assert_eq!(
            owner.report().unwrap().available_touched_bytes,
            crate::worker_slots::OWNER_BYTES
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod ipc_reader_tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    #[test]
    fn ipc_reader_uses_bounded_stack_reads_a_maximum_frame_and_exits_on_eof() {
        assert_eq!(IPC_READER_STACK_BYTES, 512 * 1024);
        let (mut writer, reader_stream) = UnixStream::pair().unwrap();
        let reader = spawn_ipc_reader("ipc-reader-contract-test", move || {
            let mut buffered = BufReader::with_capacity(8192, reader_stream);
            let payload = frame(&mut buffered).unwrap().unwrap();
            assert_eq!(payload.len(), CONTROL_BYTES);
            let event: Event = serde_json::from_slice(&payload).unwrap();
            assert_eq!(event.protocol, PROTOCOL);
            assert!(frame(&mut buffered).unwrap().is_none());
        })
        .unwrap();

        let event = Event {
            protocol: PROTOCOL.into(),
            ticket: Ticket {
                world: 1,
                branch: 1,
                job: 1,
                incarnation: 1,
                generation: 1,
                attempt: 1,
                base_revision: 1,
                field_version: 1,
                start: 0,
                end: 1,
                rules_sha256: "0".repeat(64),
                source_sha256: "0".repeat(64),
                input_sha256: "0".repeat(64),
                checkpoint_sha256: "0".repeat(64),
            },
            pid: 1,
            stage: Stage::Complete,
            candidate_sha256: Some("0".repeat(64)),
            candidate_bytes: CONTROL_BYTES,
            cursor: None,
            cpu: None,
        };
        let mut bytes = serde_json::to_vec(&event).unwrap();
        assert!(bytes.len() < CONTROL_BYTES - 1);
        // A bounded, ignored JSON string reaches the maximum accepted frame size.
        bytes.pop();
        // The total framed bytes include both the closing brace and newline.
        bytes.extend(std::iter::repeat(b' ').take(CONTROL_BYTES - 2 - bytes.len()));
        bytes.push(b'}');
        bytes.push(b'\n');
        writer.write_all(&bytes).unwrap();
        drop(writer);
        reader.join().unwrap();
    }

    #[test]
    fn reader_spawn_failure_cleanup_kills_and_waits_owned_child() {
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let child_id = child.id();
        let reaped = kill_wait_then(&mut child, |child, pid, status| {
            assert_eq!(pid, child_id);
            assert!(!status.success());
            assert!(child.try_wait().unwrap().is_some());
            Ok(true)
        })
        .unwrap();
        assert!(reaped);
    }

    #[test]
    fn ipc_reader_rejects_oversized_and_truncated_frames() {
        let oversized = vec![b'x'; CONTROL_BYTES + 1];
        assert_eq!(
            frame(&mut BufReader::new(std::io::Cursor::new(oversized))).unwrap_err(),
            "worker_control_bytes"
        );
        assert_eq!(
            frame(&mut BufReader::new(std::io::Cursor::new(b"partial"))).unwrap_err(),
            "worker_truncated_frame"
        );
    }
}
