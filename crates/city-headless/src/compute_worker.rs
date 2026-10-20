//! A pure, persistent air worker. No Store handle or business command is available here.
use mycity_core::{
    air::{Phase, State, CHECKPOINT_BYTES, STRATEGY},
    read_bounded, sha256,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};

pub const PROTOCOL: &str = "mycity-air-worker-v1";
pub const CONTROL_BYTES: usize = 64 * 1024;
pub const DIRECTORY_BYTES: u64 = 32 * 1024 * 1024;
pub const DIRECTORY_FILES: usize = 32;

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
/// Immutable, durable publish. Existing equal content is idempotent, never replaced.
pub fn publish(dir: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > CHECKPOINT_BYTES {
        return Err("worker_asset_bytes".into());
    }
    let hash = sha256(bytes);
    let target = dir.join(format!("{hash}.air"));
    if target.exists() {
        read_asset(dir, &hash)?;
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
                read_asset(dir, &hash)?;
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
fn input(root: &Path, request: &Request) -> Result<State, String> {
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
    directory_usage(&root.join("inputs"))?;
    let state = State::restore(&read_asset(&root.join("inputs"), &t.checkpoint_sha256)?)?;
    let report = state.report();
    if report.time != t.start
        || state.input().target != t.end
        || state.progress().is_some()
        || state.input().identity() != t.input_sha256
        || state.complete()
    {
        return Err("worker_frozen_input".into());
    }
    Ok(state)
}
fn emit(
    w: &mut impl Write,
    ticket: &Ticket,
    stage: Stage,
    candidate: Option<&str>,
    size: usize,
    cursor: Option<u32>,
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
    while let Some(bytes) = frame(&mut reader)? {
        let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
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
        let mut state = input(root, &request)?;
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
                    emit(&mut writer, &request.ticket, stage, None, 0, Some(p.cursor))?;
                    gate(&mut reader, &request, stage, gated)?;
                    *seen = true;
                }
            }
        }
        let bytes = state.checkpoint()?;
        let hash = if bounded {
            crate::worker_cache::publish(root, "candidates", &bytes)?
        } else {
            publish(&root.join("candidates"), &bytes)?
        };
        emit(
            &mut writer,
            &request.ticket,
            Stage::Published,
            Some(&hash),
            bytes.len(),
            None,
        )?;
        gate(&mut reader, &request, Stage::Published, gated)?;
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
            bytes.len(),
            None,
        )?;
    }
    if let Some(log) = diag.as_mut() {
        log.finish()?;
    }
    Ok(())
}

/// One child and a four-frame bounded reader. stderr is discarded, never an unbounded pipe.
/// The owning Sim supplies durable retries; this transport does not retry or mutate a world.
pub struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    receiver: Option<Receiver<Result<Event, String>>>,
    reader: Option<JoinHandle<()>>,
    outstanding: Option<Ticket>,
    root: PathBuf,
    gated: bool,
}
impl Worker {
    pub fn spawn(executable: &Path, root: &Path) -> Result<Self, String> {
        Self::spawn_inner(executable, root, false, false)
    }
    pub fn spawn_probe_gated(executable: &Path, root: &Path) -> Result<Self, String> {
        Self::spawn_inner(executable, root, true, false)
    }
    pub fn spawn_bounded_probe(executable: &Path, root: &Path) -> Result<Self, String> {
        Self::spawn_inner(executable, root, true, true)
    }
    pub fn spawn_bounded(executable: &Path, root: &Path) -> Result<Self, String> {
        Self::spawn_inner(executable, root, false, true)
    }
    fn spawn_inner(
        executable: &Path,
        root: &Path,
        gated: bool,
        bounded: bool,
    ) -> Result<Self, String> {
        let mut child = Command::new(executable)
            .arg(if gated && bounded {
                "compute-worker-bounded-probe"
            } else if gated {
                "compute-worker-probe-gated"
            } else if bounded {
                "compute-worker-bounded"
            } else {
                "compute-worker"
            })
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("worker_stdout")?;
        let (sender, receiver) = mpsc::sync_channel(4);
        let reader = thread::spawn(move || {
            let mut r = BufReader::with_capacity(8192, stdout);
            loop {
                let result = match frame(&mut r) {
                    Ok(Some(b)) => serde_json::from_slice::<Event>(&b).map_err(|e| e.to_string()),
                    Ok(None) => Err("worker_eof".into()),
                    Err(e) => Err(e),
                };
                let failed = result.is_err();
                if sender.send(result).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            receiver: Some(receiver),
            reader: Some(reader),
            outstanding: None,
            root: root.to_owned(),
            gated,
        })
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
        self.child.id()
    }
    pub fn submit(&mut self, request: &Request) -> Result<(), String> {
        if self.outstanding.is_some() {
            return Err("worker_busy".into());
        }
        write_frame(self.stdin.as_mut().ok_or("worker_stopped")?, request)?;
        self.outstanding = Some(request.ticket.clone());
        Ok(())
    }
    pub fn next(&mut self, timeout: Duration) -> Result<Event, String> {
        let event = self
            .receiver
            .as_ref()
            .ok_or("worker_stopped")?
            .recv_timeout(timeout)
            .map_err(|e| e.to_string())??;
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
        Ok(event)
    }
    pub fn stop(&mut self) -> Result<ExitStatus, String> {
        // Kill also interrupts an executing job; no cooperative cancellation assumption.
        if self.child.try_wait().map_err(|e| e.to_string())?.is_none() {
            self.child.kill().map_err(|e| e.to_string())?;
        }
        let status = self.child.wait().map_err(|e| e.to_string())?;
        self.stdin.take();
        self.receiver.take(); // unblock a reader blocked on its bounded sender before join
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "worker_reader_panic")?;
        }
        Ok(status)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop();
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
