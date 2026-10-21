//! Actual external balance. Only worker byte slots are converted into grants.
//! Ordinary Engine allocations remain EXTRA pressure, never subtracted from this pool.
pub mod broker;
mod workspace;
use crate::{control_data::Text, heap_meter, worker_slots};
use serde::{Deserialize, Serialize};
use std::os::unix::net::{UnixListener, UnixStream};
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
const SOURCE: &str = env!("MYCITY_SOURCE_SHA256");
const MIB: usize = 1024 * 1024;
pub const BYTES: usize = 64 * MIB;
pub const GRANT_BYTES: usize = worker_slots::OWNER_BYTES;
pub const DIAGNOSTIC_BYTES: usize = 256 * 1024;
const FILL: u8 = 0xa7;
const CONTROL: usize = 8192;
const MAX_GRANTS: u64 = 2048;
pub const EVENT_BYTES: usize = 1536;
const PERIODIC_EVENT_SECONDS: u64 = 30;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn secret() -> Result<Text<64>, String> {
    let mut b = [0; 32];
    File::open("/dev/urandom")
        .map_err(err)?
        .read_exact(&mut b)
        .map_err(err)?;
    Text::new(&mycity_core::sha256(&b))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ready {
    schema: Text<48>,
    source: Text<64>,
    boot: u128,
    pid: u32,
    pub capacity_bytes: usize,
    pub diagnostic_reserved_bytes: usize,
    fault_injection: Option<Text<32>>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub capacity_bytes: usize,
    pub available_touched_bytes: usize,
    pub granted_capacity_bytes: usize,
    pub workspace_capacity_bytes: usize,
    pub workspace_grants: u64,
    pub workspace_returns: u64,
    pub workspace_fills: u64,
    pub grants: u64,
    pub returns: u64,
    pub fill_acknowledgements: u64,
    pub scans: u64,
    pub rejected_requests: u64,
    pub maximum_prepare_to_fill_or_reap_ns: u64,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
// Fixed stack control storage avoids hidden heap allocations for large Start fields.
#[allow(clippy::large_enum_variant)]
enum Op {
    Prepare {
        parent: u32,
        parent_instance: u128,
        key: Text<64>,
    },
    Attach {
        id: u64,
        key: Text<64>,
        pid: u32,
    },
    Filled {
        id: u64,
        key: Text<64>,
        identity: worker_slots::Identity,
        capacity_bytes: usize,
        available_touched_bytes: usize,
        full_balance_scan_passed: bool,
    },
    Reap {
        id: u64,
        key: Text<64>,
        pid: u32,
        exit_success: bool,
    },
    Abort {
        id: u64,
        key: Text<64>,
    },
    Status,
    Broker(broker::Op),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    source: Text<64>,
    boot: u128,
    action: Op,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    error: Option<Text<64>>,
    id: Option<u64>,
    worker_key: Option<Text<64>>,
    summary: Summary,
    #[serde(default)]
    broker: Option<broker::Receipt>,
}
#[derive(Clone, Copy)]
struct Active {
    id: u64,
    parent: u32,
    parent_instance: u128,
    key: Text<64>,
    worker_key: Text<64>,
    pid: Option<u32>,
    filled: Option<worker_slots::Identity>,
    released_ns: u64,
}
#[derive(Clone, Copy)]
struct Returned {
    id: u64,
    key: Text<64>,
    pid: Option<u32>,
}
#[derive(Serialize)]
struct Event {
    kind: &'static str,
    rejection_reason: Option<&'static str>,
    broker_generation: Option<u64>,
    broker_sim_pid: Option<u32>,
    observed_ns: u64,
    id: Option<u64>,
    parent: Option<u32>,
    parent_instance: Option<u128>,
    pid: Option<u32>,
    worker_identity: Option<worker_slots::Identity>,
    summary: Summary,
    owner_actual_capacity_bytes: usize,
}
#[derive(Debug)]
enum ActionError {
    Rejected(&'static str),
    Fatal(String),
}
impl From<String> for ActionError {
    fn from(value: String) -> Self {
        Self::Fatal(value)
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    ReturnSecondPage,
    WorkspaceReturnSecondPage,
    AfterAttachEvent,
    AfterBrokerSpawnPause,
    AfterBrokerBindReply,
    BeforeBrokerRevoke,
    AfterBrokerRevokeReply,
}
struct Pool {
    broker_root: PathBuf,
    broker: broker::State,
    ready: Ready,
    fault: Option<Fault>,
    pages: [Option<Vec<u8>>; 64],
    workspaces: [Option<workspace::Entry>; 4],
    active: Option<Active>,
    last: Option<Returned>,
    summary: Summary,
    began: Instant,
    events: BufWriter<File>,
    event_bytes: usize,
}
fn page() -> Result<Vec<u8>, String> {
    let mut p = Vec::new();
    p.try_reserve_exact(MIB).map_err(err)?;
    if p.capacity() != MIB {
        return Err("normal_pool_page_capacity".into());
    }
    p.resize(MIB, FILL);
    Ok(p)
}
impl Pool {
    fn new(root: &Path) -> Result<Self, String> {
        let fault_raw = std::env::var("MYCITY_NORMAL_POOL_FAULT").ok();
        let fault = match fault_raw.as_deref() {
            None => None,
            Some("return-page-2") => Some(Fault::ReturnSecondPage),
            Some("workspace-return-page-2") => Some(Fault::WorkspaceReturnSecondPage),
            Some("after-attach-event") => Some(Fault::AfterAttachEvent),
            Some("after-broker-spawn-pause") => Some(Fault::AfterBrokerSpawnPause),
            Some("after-broker-bind-reply") => Some(Fault::AfterBrokerBindReply),
            Some("before-broker-revoke") => Some(Fault::BeforeBrokerRevoke),
            Some("after-broker-revoke-reply") => Some(Fault::AfterBrokerRevokeReply),
            Some(_) => return Err("normal_pool_fault_mode".into()),
        };
        let mut pool = Self {
            broker_root: root.to_owned(),
            broker: broker::State::default(),
            fault,
            ready: Ready {
                schema: Text::new("mycity-normal-pool-v1")?,
                source: Text::new(SOURCE)?,
                boot: heap_meter::process_instance(),
                pid: std::process::id(),
                capacity_bytes: BYTES,
                diagnostic_reserved_bytes: DIAGNOSTIC_BYTES,
                fault_injection: fault_raw.as_deref().map(Text::new).transpose()?,
            },
            pages: std::array::from_fn(|_| None),
            workspaces: [None; 4],
            active: None,
            last: None,
            summary: Summary {
                capacity_bytes: BYTES,
                available_touched_bytes: 0,
                granted_capacity_bytes: 0,
                workspace_capacity_bytes: 0,
                workspace_grants: 0,
                workspace_returns: 0,
                workspace_fills: 0,
                grants: 0,
                returns: 0,
                fill_acknowledgements: 0,
                scans: 0,
                rejected_requests: 0,
                maximum_prepare_to_fill_or_reap_ns: 0,
            },
            began: Instant::now(),
            events: BufWriter::with_capacity(
                8192,
                File::create_new(root.join("events.jsonl")).map_err(err)?,
            ),
            event_bytes: 0,
        };
        for p in &mut pool.pages {
            *p = Some(page()?);
        }
        pool.summary.available_touched_bytes = BYTES;
        pool.scan()?;
        pool.event("created", None)?;
        Ok(pool)
    }
    fn now(&self) -> u64 {
        self.began.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
    fn scan(&mut self) -> Result<(), String> {
        let mut total = 0;
        for p in self.pages.iter().flatten() {
            if p.len() != MIB || p.capacity() != MIB || !p.iter().all(|b| *b == FILL) {
                return Err("normal_pool_balance_corrupt".into());
            }
            total += p.capacity();
        }
        let grant = self.workspace_bytes()
            + if self.active.is_some() {
                GRANT_BYTES
            } else {
                0
            };
        if self.workspace_bytes() != self.summary.workspace_capacity_bytes
            || total != self.summary.available_touched_bytes
            || total + grant != BYTES
            || grant != self.summary.granted_capacity_bytes
        {
            return Err("normal_pool_balance".into());
        }
        self.summary.scans += 1;
        Ok(())
    }
    fn event(&mut self, kind: &'static str, a: Option<Active>) -> Result<(), String> {
        self.event_reason(kind, a, None)
    }
    fn event_reason(
        &mut self,
        kind: &'static str,
        a: Option<Active>,
        rejection_reason: Option<&'static str>,
    ) -> Result<(), String> {
        if kind == "child_attached" && self.fault == Some(Fault::AfterAttachEvent) {
            self.fault = None;
            return Err("normal_pool_injected_event_failure".into());
        }
        let e = Event {
            kind,
            rejection_reason,
            broker_generation: self.broker.generation(),
            broker_sim_pid: self.broker.sim_pid(),
            observed_ns: self.now(),
            id: a.map(|v| v.id),
            parent: a.map(|v| v.parent),
            parent_instance: a.map(|v| v.parent_instance),
            pid: a.and_then(|v| v.pid),
            worker_identity: a.and_then(|v| v.filled),
            summary: self.summary,
            owner_actual_capacity_bytes: self.pages.iter().flatten().map(Vec::capacity).sum(),
        };
        self.write_event(&e)
    }
    fn write_event(&mut self, e: &impl Serialize) -> Result<(), String> {
        struct Count<'a> {
            sink: &'a mut BufWriter<File>,
            used: &'a mut usize,
            row_bytes: usize,
        }
        impl Write for Count<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if self.row_bytes.saturating_add(b.len()) > EVENT_BYTES {
                    return Err(std::io::Error::other("normal_pool_event_row_bound"));
                }
                if self.used.saturating_add(b.len()) > 8 * MIB {
                    return Err(std::io::Error::other("normal_pool_event_bound"));
                }
                self.sink.write_all(b)?;
                *self.used += b.len();
                self.row_bytes += b.len();
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.sink.flush()
            }
        }
        let mut sink = Count {
            sink: &mut self.events,
            used: &mut self.event_bytes,
            row_bytes: 0,
        };
        serde_json::to_writer(&mut sink, e).map_err(err)?;
        sink.write_all(b"\n").map_err(err)?;
        sink.flush().map_err(err)
    }
    fn active(&self, id: u64, key: Text<64>, worker: bool) -> Result<Active, ActionError> {
        self.active
            .filter(|a| {
                a.id == id
                    && if worker {
                        a.worker_key == key
                    } else {
                        a.key == key
                    }
            })
            .ok_or(ActionError::Rejected("normal_pool_stale_or_wrong_grant"))
    }
    fn give_back(&mut self, a: Active) -> Result<(), String> {
        // Keep the active grant until ALL replacement pages have been built.
        // Allocation failure stops the pool rather than publishing false credit.
        for i in 60..64 {
            if self.pages[i].is_some() {
                return Err("normal_pool_duplicate_page".into());
            }
            if i == 61 && self.fault == Some(Fault::ReturnSecondPage) {
                self.fault = None;
                self.event("injected_partial_return_failure", Some(a))?;
                return Err("normal_pool_injected_return_page_failure".into());
            }
            self.pages[i] = Some(page()?);
        }
        if a.filled.is_none() {
            self.summary.maximum_prepare_to_fill_or_reap_ns = self
                .summary
                .maximum_prepare_to_fill_or_reap_ns
                .max(self.now() - a.released_ns);
        }
        self.active = None;
        self.last = Some(Returned {
            id: a.id,
            key: a.key,
            pid: a.pid,
        });
        self.summary.available_touched_bytes += GRANT_BYTES;
        self.summary.granted_capacity_bytes -= GRANT_BYTES;
        self.summary.returns += 1;
        self.scan()?;
        self.event("returned_after_wait_or_abort", Some(a))
    }
    fn action(&mut self, op: Op) -> Result<(Option<u64>, Option<Text<64>>), ActionError> {
        if self.broker.registered() && !matches!(op, Op::Filled { .. } | Op::Status) {
            return Err(ActionError::Rejected("normal_pool_legacy_broker_conflict"));
        }
        self.legacy_action(op)
    }
    fn legacy_action(&mut self, op: Op) -> Result<(Option<u64>, Option<Text<64>>), ActionError> {
        match op {
            Op::Broker(_) => Err(ActionError::Rejected(
                "normal_pool_broker_dispatch_required",
            )),
            Op::Prepare {
                parent,
                parent_instance,
                key,
            } => {
                if let Some(a) = self.active {
                    if a.parent == parent && a.parent_instance == parent_instance && a.key == key {
                        return Ok((Some(a.id), Some(a.worker_key)));
                    }
                    return Err(ActionError::Rejected("normal_pool_grant_busy"));
                }
                if parent == 0 || parent_instance == 0 || self.summary.grants >= MAX_GRANTS {
                    return Err(ActionError::Rejected("normal_pool_grant_limit_or_identity"));
                }
                self.scan()?;
                let a = Active {
                    id: self.summary.grants + 1,
                    parent,
                    parent_instance,
                    key,
                    worker_key: secret()?,
                    pid: None,
                    filled: None,
                    released_ns: self.now(),
                };
                for i in 60..64 {
                    self.pages[i].take();
                }
                self.active = Some(a);
                self.summary.grants += 1;
                self.summary.available_touched_bytes -= GRANT_BYTES;
                self.summary.granted_capacity_bytes += GRANT_BYTES;
                self.scan()?;
                self.event("grant_released_before_spawn", Some(a))?;
                Ok((Some(a.id), Some(a.worker_key)))
            }
            Op::Attach { id, key, pid } => {
                let mut a = self.active(id, key, false)?;
                if pid == 0 || a.pid.is_some_and(|p| p != pid) {
                    return Err(ActionError::Rejected("normal_pool_attach_identity"));
                }
                if a.pid.is_none() {
                    a.pid = Some(pid);
                    self.active = Some(a);
                    self.event("child_attached", Some(a))?;
                }
                Ok((Some(id), None))
            }
            Op::Filled {
                id,
                key,
                identity,
                capacity_bytes,
                available_touched_bytes,
                full_balance_scan_passed,
            } => {
                let mut a = self.active(id, key, true)?;
                if a.pid != Some(identity.process_pid)
                    || identity.process_instance_unix_ns == 0
                    || identity.run_epoch == 0
                    || identity.job == 0
                    || identity.generation == 0
                    || !(1..=3).contains(&identity.attempt)
                    || capacity_bytes != GRANT_BYTES
                    || available_touched_bytes != GRANT_BYTES
                    || !full_balance_scan_passed
                {
                    return Err(ActionError::Rejected("normal_pool_worker_fill_identity"));
                }
                if let Some(old) = a.filled {
                    if old != identity {
                        return Err(ActionError::Rejected("normal_pool_fill_already_bound"));
                    }
                    return Ok((Some(id), None));
                }
                a.filled = Some(identity);
                self.active = Some(a);
                self.summary.fill_acknowledgements += 1;
                self.summary.maximum_prepare_to_fill_or_reap_ns = self
                    .summary
                    .maximum_prepare_to_fill_or_reap_ns
                    .max(self.now() - a.released_ns);
                self.event("worker_actual_slots_filled", Some(a))?;
                Ok((Some(id), None))
            }
            Op::Reap {
                id,
                key,
                pid,
                exit_success,
            } => {
                if self
                    .last
                    .is_some_and(|v| v.id == id && v.key == key && v.pid == Some(pid))
                {
                    return Ok((Some(id), None));
                }
                let a = self.active(id, key, false)?;
                if a.pid != Some(pid) {
                    return Err(ActionError::Rejected("normal_pool_reap_identity"));
                }
                self.event(
                    if exit_success {
                        "owned_child_wait_success"
                    } else {
                        "owned_child_wait_failure"
                    },
                    Some(a),
                )?;
                self.give_back(a)?;
                Ok((Some(id), None))
            }
            Op::Abort { id, key } => {
                if self
                    .last
                    .is_some_and(|v| v.id == id && v.key == key && v.pid.is_none())
                {
                    return Ok((Some(id), None));
                }
                let a = self.active(id, key, false)?;
                if a.pid.is_some() {
                    return Err(ActionError::Rejected("normal_pool_abort_live_child"));
                }
                self.give_back(a)?;
                Ok((Some(id), None))
            }
            Op::Status => {
                self.scan()?;
                Ok((None, None))
            }
        }
    }
}
fn read_wire<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> Result<T, &'static str> {
    let mut data = [0; CONTROL];
    let mut used = 0;
    loop {
        if used == CONTROL {
            return Err("normal_pool_control_bound");
        }
        let count = match r.read(&mut data[used..]) {
            Ok(0) => return Err("normal_pool_control_eof"),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err("normal_pool_control_timeout");
            }
            Err(_) => return Err("normal_pool_control_io"),
        };
        used += count;
        if let Some(end) = data[..used].iter().position(|b| *b == b'\n') {
            if end + 1 != used {
                return Err("normal_pool_control_trailing");
            }
            return crate::control_data::decode(&data[..end])
                .map_err(|_| "normal_pool_control_decode");
        }
    }
}
fn write_wire(w: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *w, value).map_err(err)?;
    w.write_all(b"\n").map_err(err)?;
    w.flush().map_err(err)
}
fn ready(root: &Path) -> Result<Ready, String> {
    let r: Ready = crate::control_data::parse(&root.join("ready.json"))?;
    if r.schema.as_str() != "mycity-normal-pool-v1"
        || r.source.as_str() != SOURCE
        || r.boot == 0
        || r.pid == 0
        || r.capacity_bytes != BYTES
        || r.diagnostic_reserved_bytes != DIAGNOSTIC_BYTES
    {
        return Err("normal_pool_ready_identity".into());
    }
    Ok(r)
}
fn call(root: &Path, r: Ready, action: Op) -> Result<Reply, String> {
    if ready(root)?.boot != r.boot {
        return Err("normal_pool_old_boot".into());
    }
    let mut socket = UnixStream::connect(root.join("pool.sock")).map_err(err)?;
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(err)?;
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(err)?;
    write_wire(
        &mut socket,
        &Request {
            source: Text::new(SOURCE)?,
            boot: r.boot,
            action,
        },
    )?;
    let reply: Reply = read_wire(&mut socket)?;
    if let Some(e) = reply.error {
        return Err(e.as_str().into());
    }
    Ok(reply)
}
pub fn status(root: &Path) -> Result<Summary, String> {
    Ok(call(root, ready(root)?, Op::Status)?.summary)
}
/// Parent-only capability. Reclaim is only called after the owned Child wait.
pub struct Grant {
    root: PathBuf,
    ready: Ready,
    id: u64,
    key: Text<64>,
    worker_key: Text<64>,
    pid: Option<u32>,
    returned: bool,
}
impl Grant {
    pub fn from_env() -> Result<Option<Self>, String> {
        std::env::var_os("MYCITY_NORMAL_POOL")
            .map(|root| Self::prepare(Path::new(&root)))
            .transpose()
    }
    pub fn prepare(root: &Path) -> Result<Self, String> {
        let ready = ready(root)?;
        let key = secret()?;
        let r = call(
            root,
            ready,
            Op::Prepare {
                parent: std::process::id(),
                parent_instance: heap_meter::process_instance(),
                key,
            },
        )?;
        Ok(Self {
            root: root.to_owned(),
            ready,
            id: r.id.ok_or("normal_pool_reply_id")?,
            key,
            worker_key: r.worker_key.ok_or("normal_pool_reply_key")?,
            pid: None,
            returned: false,
        })
    }
    pub fn configure(&self, cmd: &mut Command) -> Result<(), String> {
        let binding = WorkerBinding {
            ready: self.ready,
            id: self.id,
            key: self.worker_key,
        };
        cmd.env("MYCITY_NORMAL_POOL", &self.root).env(
            "MYCITY_NORMAL_WORKER_GRANT",
            serde_json::to_string(&binding).map_err(err)?,
        );
        Ok(())
    }
    pub fn attach(&mut self, pid: u32) -> Result<(), String> {
        // Save PID BEFORE transport: lost attach reply must still require wait.
        self.pid = Some(pid);
        call(
            &self.root,
            self.ready,
            Op::Attach {
                id: self.id,
                key: self.key,
                pid,
            },
        )?;
        Ok(())
    }
    pub fn after_wait(&mut self, pid: u32, status: ExitStatus) -> Result<(), String> {
        if self.returned {
            return Ok(());
        }
        if self.pid != Some(pid) {
            return Err("normal_pool_owned_wait_identity".into());
        }
        let reap = Op::Reap {
            id: self.id,
            key: self.key,
            pid,
            exit_success: status.success(),
        };
        match call(&self.root, self.ready, reap) {
            Ok(_) => (),
            Err(e) if e == "normal_pool_reap_identity" => {
                // The attach request may not have reached the server. Child is
                // already waited, so binding it now cannot authorize a live refund.
                call(
                    &self.root,
                    self.ready,
                    Op::Attach {
                        id: self.id,
                        key: self.key,
                        pid,
                    },
                )?;
                call(&self.root, self.ready, reap)?;
            }
            Err(e) => return Err(e),
        }
        self.returned = true;
        Ok(())
    }
}
impl Drop for Grant {
    fn drop(&mut self) {
        if !self.returned && self.pid.is_none() {
            let _ = call(
                &self.root,
                self.ready,
                Op::Abort {
                    id: self.id,
                    key: self.key,
                },
            );
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerBinding {
    ready: Ready,
    id: u64,
    key: Text<64>,
}
pub struct WorkerGrant {
    root: PathBuf,
    binding: WorkerBinding,
}
impl WorkerGrant {
    pub fn from_env() -> Result<Option<Self>, String> {
        let Some(raw) = std::env::var_os("MYCITY_NORMAL_WORKER_GRANT") else {
            return if std::env::var_os("MYCITY_NORMAL_POOL").is_some() {
                Err("normal_pool_worker_grant_required".into())
            } else {
                Ok(None)
            };
        };
        let s = raw.to_str().ok_or("normal_pool_worker_binding_utf8")?;
        let binding = crate::control_data::decode(s.as_bytes())?;
        let root = std::env::var_os("MYCITY_NORMAL_POOL").ok_or("normal_pool_worker_root")?;
        Ok(Some(Self {
            root: root.into(),
            binding,
        }))
    }
    pub fn filled(&self, owner: &worker_slots::Owner) -> Result<(), String> {
        let r = owner.report()?;
        let begin = Instant::now();
        loop {
            match call(
                &self.root,
                self.binding.ready,
                Op::Filled {
                    id: self.binding.id,
                    key: self.binding.key,
                    identity: owner.identity(),
                    capacity_bytes: r.owner_capacity_bytes,
                    available_touched_bytes: r.available_touched_bytes,
                    full_balance_scan_passed: r.full_balance_scan_passed,
                },
            ) {
                Err(e)
                    if e == "normal_pool_worker_fill_identity"
                        && begin.elapsed() < Duration::from_secs(5) =>
                {
                    std::thread::sleep(Duration::from_millis(2))
                }
                v => return v.map(|_| ()),
            }
        }
    }
}
/// One bounded owner server; private control connection commands are short-lived.
pub fn serve(root: &Path) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    let listener = UnixListener::bind(root.join("pool.sock")).map_err(err)?;
    listener.set_nonblocking(true).map_err(err)?;
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&stopped);
    std::thread::Builder::new()
        .name("normal-pool-parent".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut data = [0; 32];
            while let Ok(n) = std::io::stdin().read(&mut data) {
                if n == 0 {
                    break;
                }
            }
            stop.store(true, Ordering::Release);
        })
        .map_err(err)?;
    let mut pool = Pool::new(root)?;
    let mut f = File::create_new(root.join("ready.json")).map_err(err)?;
    serde_json::to_writer(&mut f, &pool.ready).map_err(err)?;
    f.sync_all().map_err(err)?;
    let mut next_scan = Instant::now() + Duration::from_secs(1);
    let mut next_periodic_event = Instant::now() + Duration::from_secs(PERIODIC_EVENT_SECONDS);
    while !stopped.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut socket, _)) => {
                // BSD/macOS accept can inherit O_NONBLOCK from the listener.
                // Read timeout alone does not clear it: partial JSON is a valid frame prefix.
                socket.set_nonblocking(false).map_err(err)?;
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(err)?;
                socket
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(err)?;
                let request: Result<Request, _> = read_wire(&mut socket);
                if pool.fault == Some(Fault::BeforeBrokerRevoke)
                    && matches!(
                        &request,
                        Ok(Request {
                            action: Op::Broker(broker::Op::Revoke { .. }),
                            ..
                        })
                    )
                {
                    pool.fault = None;
                    continue; // fault lab: request lost before mutation, no credit granted
                }
                let answer = match request {
                    Ok(r) if r.source.as_str() != SOURCE => {
                        Err(ActionError::Rejected("normal_pool_request_source"))
                    }
                    Ok(r) if r.boot != pool.ready.boot => {
                        Err(ActionError::Rejected("normal_pool_request_boot"))
                    }
                    Ok(r) => match r.action {
                        Op::Broker(op) => pool
                            .broker_action(op)
                            .map(|receipt| (None, None, Some(receipt))),
                        op => pool.action(op).map(|(id, key)| (id, key, None)),
                    },
                    Err(e) => Err(ActionError::Rejected(e)),
                };
                let reply = match answer {
                    Ok((id, key, broker)) => Reply {
                        error: None,
                        id,
                        worker_key: key,
                        broker,
                        summary: pool.summary,
                    },
                    Err(ActionError::Fatal(e)) => return Err(format!("normal_pool_internal:{e}")),
                    Err(ActionError::Rejected(e)) => {
                        pool.summary.rejected_requests += 1;
                        pool.event_reason("rejected", pool.active, Some(e))?;
                        Reply {
                            error: Some(Text::new(e)?),
                            id: None,
                            worker_key: None,
                            broker: None,
                            summary: pool.summary,
                        }
                    }
                };
                // A lost reply does not undo the grant. All successful actions are
                // idempotent under the same capability; no automatic credit on EOF.
                match (pool.fault, reply.broker) {
                    (Some(Fault::AfterBrokerSpawnPause), Some(broker::Receipt::Started { .. })) => {
                        pool.fault = None;
                        std::thread::sleep(Duration::from_secs(2)); // fault lab: Sim can die before Start receipt
                    }
                    (Some(Fault::AfterBrokerBindReply), Some(broker::Receipt::Bound { .. }))
                    | (
                        Some(Fault::AfterBrokerRevokeReply),
                        Some(broker::Receipt::Revoked { .. }),
                    ) => {
                        pool.fault = None;
                        continue; // fault lab: successful mutation, lost reply
                    }
                    _ => (),
                }
                let _ = write_wire(&mut socket, &reply);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(e) => return Err(err(e)),
        }
        if Instant::now() >= next_scan {
            pool.scan()?;
            if Instant::now() >= next_periodic_event {
                pool.event("periodic_balance_scan", pool.active)?;
                next_periodic_event = Instant::now() + Duration::from_secs(PERIODIC_EVENT_SECONDS);
            }
            next_scan = Instant::now() + Duration::from_secs(1);
        }
    }
    pool.scan()?;
    pool.event("parent_pipe_closed", pool.active)?;
    let balanced = pool.active.is_none() && !pool.broker.registered();
    #[derive(Serialize)]
    struct Final {
        source_sha256: &'static str,
        ready: Ready,
        summary: Summary,
        balanced_after_owned_wait: bool,
        normal_owned_acceptance: &'static str,
        scope: &'static str,
    }
    let mut f = File::create_new(root.join("report.json")).map_err(err)?;
    serde_json::to_writer_pretty(&mut f,&Final{source_sha256:SOURCE,ready:pool.ready,summary:pool.summary,balanced_after_owned_wait:balanced,normal_owned_acceptance:"not_accepted_unconverted_Engine_work_is_extra_pressure",scope:"64MiB physical balance plus worker grants; observed transitions are not a proof of continuously full pressure; native/stack separate"}).map_err(err)?;
    f.sync_all().map_err(err)?;
    pool.events.flush().map_err(err)?;
    pool.events.get_ref().sync_all().map_err(err)?;
    if !balanced {
        return Err("normal_pool_unreaped_grant_on_exit".into());
    }
    Ok(())
}
pub struct Process {
    child: Child,
    input: Option<ChildStdin>,
    root: PathBuf,
}
impl Process {
    pub fn spawn(exe: &Path, root: &Path, heap_log: Option<&Path>) -> Result<Self, String> {
        match fs::symlink_metadata(root) {
            Ok(_) => return Err("normal_pool_run_directory_exists".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(err(e)),
        }
        let mut cmd = Command::new(exe);
        cmd.env_remove("MYCITY_NORMAL_POOL_FAULT")
            .arg("normal-pool-host")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                File::create_new(root.with_extension("stderr")).map_err(err)?,
            ));
        if let Some(log) = heap_log {
            cmd.env("MYCITY_HEAP_LOG", log);
        }
        let mut child = cmd.spawn().map_err(err)?;
        let input = child.stdin.take();
        let mut process = Self {
            child,
            input,
            root: root.to_owned(),
        };
        let began = Instant::now();
        loop {
            if let Some(s) = process.child.try_wait().map_err(err)? {
                return Err(format!("normal_pool_start_exit:{s}"));
            }
            if let Ok(r) = ready(root) {
                if r.pid != process.child.id() {
                    return Err("normal_pool_start_pid_mismatch".into());
                }
                return Ok(process);
            }
            if began.elapsed() > Duration::from_secs(10) {
                return Err("normal_pool_ready_deadline".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.input.take();
        let began = Instant::now();
        loop {
            if let Some(s) = self.child.try_wait().map_err(err)? {
                return if s.success() {
                    Ok(())
                } else {
                    Err(format!("normal_pool_exit:{s}"))
                };
            }
            if began.elapsed() > Duration::from_secs(10) {
                return Err("normal_pool_stop_deadline".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.input.take();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "mycity-pool-unit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn actual_64mib_pages_grant_identity_and_duplicate_receipts_never_double_credit() {
        let t = Temp::new();
        let mut p = Pool::new(&t.0).unwrap();
        let key = secret().unwrap();
        assert_eq!(p.summary.available_touched_bytes, BYTES);
        let op = Op::Prepare {
            parent: 10,
            parent_instance: 20,
            key,
        };
        let (id, w) = p.action(op).unwrap();
        let (id, w) = (id.unwrap(), w.unwrap());
        assert_eq!(p.summary.available_touched_bytes, BYTES - GRANT_BYTES);
        assert_eq!(
            p.pages.iter().flatten().map(Vec::capacity).sum::<usize>(),
            BYTES - GRANT_BYTES
        );
        assert_eq!(p.action(op).unwrap().0, Some(id));
        assert_eq!(p.summary.grants, 1);
        assert!(p
            .action(Op::Prepare {
                parent: 11,
                parent_instance: 20,
                key: secret().unwrap()
            })
            .is_err());
        p.action(Op::Attach { id, key, pid: 30 }).unwrap();
        p.action(Op::Attach { id, key, pid: 30 }).unwrap();
        assert!(p.action(Op::Abort { id, key }).is_err());
        assert!(p.action(Op::Attach { id, key, pid: 31 }).is_err());
        let identity = worker_slots::Identity {
            process_pid: 30,
            process_instance_unix_ns: 50,
            run_epoch: 1,
            owner_instance: 0,
            job: 1,
            generation: 1,
            attempt: 1,
        };
        let fill = Op::Filled {
            id,
            key: w,
            identity,
            capacity_bytes: GRANT_BYTES,
            available_touched_bytes: GRANT_BYTES,
            full_balance_scan_passed: true,
        };
        assert!(p
            .action(Op::Filled {
                id,
                key,
                identity,
                capacity_bytes: GRANT_BYTES,
                available_touched_bytes: GRANT_BYTES,
                full_balance_scan_passed: true
            })
            .is_err());
        p.action(fill).unwrap();
        p.action(fill).unwrap();
        assert_eq!(p.summary.fill_acknowledgements, 1);
        assert!(p
            .action(Op::Reap {
                id,
                key,
                pid: 31,
                exit_success: true
            })
            .is_err());
        let reap = Op::Reap {
            id,
            key,
            pid: 30,
            exit_success: false,
        };
        p.action(reap).unwrap();
        p.action(reap).unwrap();
        assert_eq!(p.summary.returns, 1);
        p.scan().unwrap();
        assert_eq!(p.summary.available_touched_bytes, BYTES);
        let key2 = secret().unwrap();
        let next = p
            .action(Op::Prepare {
                parent: 10,
                parent_instance: 20,
                key: key2,
            })
            .unwrap()
            .0
            .unwrap();
        assert!(next > id);
        p.action(reap).unwrap();
        assert_eq!(p.summary.available_touched_bytes, BYTES - GRANT_BYTES);
        assert!(p.action(fill).is_err());
        p.action(Op::Abort {
            id: next,
            key: key2,
        })
        .unwrap();
        p.action(Op::Abort {
            id: next,
            key: key2,
        })
        .unwrap();
        assert_eq!(p.summary.returns, 2);
        p.scan().unwrap();
    }
    #[test]
    fn every_control_variant_round_trips_u128_without_a_dynamic_tagged_content_tree() {
        let key = secret().unwrap();
        let identity = worker_slots::Identity {
            process_pid: 1,
            process_instance_unix_ns: u128::MAX,
            run_epoch: 1,
            owner_instance: 0,
            job: 1,
            generation: 1,
            attempt: 1,
        };
        for action in [
            Op::Prepare {
                parent: 1,
                parent_instance: u128::MAX,
                key,
            },
            Op::Attach { id: 1, key, pid: 2 },
            Op::Filled {
                id: 1,
                key,
                identity,
                capacity_bytes: GRANT_BYTES,
                available_touched_bytes: GRANT_BYTES,
                full_balance_scan_passed: true,
            },
            Op::Reap {
                id: 1,
                key,
                pid: 2,
                exit_success: false,
            },
            Op::Abort { id: 1, key },
            Op::Status,
            Op::Broker(broker::Op::Start {
                generation: 1,
                key,
                endpoint: Text::new("/tmp/probe.sock").unwrap(),
                root: Text::new("/tmp/model").unwrap(),
                instance: u128::MAX,
                gated: true,
                diagnostics: true,
            }),
            Op::Broker(broker::Op::AcknowledgeRevoke { generation: 1, key }),
        ] {
            let request = Request {
                source: Text::new(SOURCE).unwrap(),
                boot: u128::MAX,
                action,
            };
            let data = serde_json::to_vec(&request).unwrap();
            let restored: Request = serde_json::from_slice(&data).unwrap();
            assert_eq!(restored.boot, u128::MAX);
            assert_eq!(serde_json::to_vec(&restored).unwrap(), data);
        }
    }
    #[test]
    fn control_wire_bound_truncation_trailing_and_escape_reject_before_growth() {
        assert!(read_wire::<Request>(&mut &vec![b' '; CONTROL + 1][..]).is_err());
        assert!(read_wire::<Request>(&mut &b"{}"[..]).is_err());
        assert!(read_wire::<Request>(&mut &b"{}\n{}\n"[..]).is_err());
        assert!(read_wire::<Request>(&mut &b"{\"source\":\"\\u0061\"}\n"[..]).is_err());
        assert!(
            std::mem::size_of::<Pool>() + 2 * CONTROL + 40 * 1024 + 8192 + 64 * 1024
                < DIAGNOSTIC_BYTES
        );
    }
    #[test]
    fn fixed_event_envelope_covers_u128_and_original_eight_hour_diagnostics() {
        let text = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let event = Event {
            kind: text,
            rejection_reason: Some(text),
            broker_generation: Some(u64::MAX),
            broker_sim_pid: Some(u32::MAX),
            observed_ns: u64::MAX,
            id: Some(u64::MAX),
            parent: Some(u32::MAX),
            parent_instance: Some(u128::MAX),
            pid: Some(u32::MAX),
            worker_identity: Some(worker_slots::Identity {
                process_pid: u32::MAX,
                process_instance_unix_ns: u128::MAX,
                run_epoch: u64::MAX,
                owner_instance: u64::MAX,
                job: u64::MAX,
                generation: u32::MAX,
                attempt: u8::MAX,
            }),
            summary: Summary {
                capacity_bytes: usize::MAX,
                available_touched_bytes: usize::MAX,
                granted_capacity_bytes: usize::MAX,
                workspace_capacity_bytes: usize::MAX,
                workspace_grants: u64::MAX,
                workspace_returns: u64::MAX,
                workspace_fills: u64::MAX,
                grants: u64::MAX,
                returns: u64::MAX,
                fill_acknowledgements: u64::MAX,
                scans: u64::MAX,
                rejected_requests: u64::MAX,
                maximum_prepare_to_fill_or_reap_ns: u64::MAX,
            },
            owner_actual_capacity_bytes: usize::MAX,
        };
        assert!(serde_json::to_vec(&event).unwrap().len() < EVENT_BYTES);
        // Original724 windows plus retries (up to768),960 periodic checkpoints,
        // sixteen session handoffs and64 exceptional transitions. Diagnostic bound only.
        let rows = 5 * 768 + 28_800 / PERIODIC_EVENT_SECONDS + 8 * 16 + 64;
        assert!(rows as usize * EVENT_BYTES < 8 * MIB);
    }
}
