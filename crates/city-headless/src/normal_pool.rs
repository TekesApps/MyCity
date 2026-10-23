//! Actual external balance. Worker I/O and typed air backings share a pre-spawn grant.
//! Sim/model/stream roles own one fixed allocator region, authenticated before
//! construction; worker backing retains its separate5MiB grant. Native/stack separate.
pub mod broker;
mod mapped_pages;
pub(crate) mod workspace;
use crate::{control_data::Text, heap_meter, worker_slots};
use mapped_pages::{Budget as MappedBudget, Page};
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
pub const GRANT_BYTES: usize = worker_slots::OWNER_BYTES + mycity_core::air::WORKSPACE_BYTES;
const WORKER_START_PAGE: usize = 64 - GRANT_BYTES / MIB;
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
        typed_instance: u64,
        typed_capacity: usize,
        typed_scan_passed: bool,
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
    typed_instance: Option<u64>,
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
    typed_instance: Option<u64>,
    typed_capacity: usize,
    summary: Summary,
    owner_actual_capacity_bytes: usize,
    /// Rust heap/role-arena counters sampled in this process at event time.
    /// This is not native footprint and does not include stacks or mmap pages.
    #[serde(rename = "mem_v1")]
    process_memory: ProcessMemory,
}
#[derive(Serialize)]
struct ProcessMemory {
    /// Versioned wire contract: each `*_bytes` array has an index map in the
    /// accompanying evidence note; absent role arena remains JSON null.
    heap_bytes: [usize; 2],
    arena_bytes: Option<[usize; 5]>,
    arena_failures: Option<u64>,
}
impl ProcessMemory {
    fn capture() -> Self {
        let heap = heap_meter::snapshot();
        let arena = heap_meter::arena_snapshot();
        Self {
            heap_bytes: [
                heap.current_requested_bytes,
                heap.lifetime_peak_requested_bytes,
            ],
            arena_bytes: arena.activation.map(|activation| {
                [
                    activation.grant_bytes,
                    activation.initialized_bytes,
                    arena.current_requested_bytes,
                    arena.lifetime_peak_requested_bytes,
                    arena.bootstrap_quarantined_bytes,
                ]
            }),
            arena_failures: arena.activation.map(|_| arena.allocation_failures),
        }
    }
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
    pages: [Option<Page>; 64],
    backing: Arc<MappedBudget>,
    vm_trace_enabled: bool,
    vm_trace_epoch: Option<u64>,
    vm_trace_lists: u64,
    vm_trace_bytes: usize,
    workspaces: [Option<workspace::Entry>; 4],
    active: Option<Active>,
    last: Option<Returned>,
    summary: Summary,
    began: Instant,
    events: BufWriter<File>,
    event_bytes: usize,
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
            backing: MappedBudget::new(BYTES),
            vm_trace_enabled: false,
            vm_trace_epoch: None,
            vm_trace_lists: 0,
            vm_trace_bytes: 0,
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
        for i in 0..64 {
            pool.pages[i] = Some(pool.make_page()?);
        }
        pool.summary.available_touched_bytes = BYTES;
        pool.scan()?;
        pool.event("created", None)?;
        Ok(pool)
    }
    fn make_page(&mut self) -> Result<Page, String> {
        match Page::new(&self.backing) {
            Ok(page) => Ok(page),
            Err(e) => {
                self.event("mapped_page_allocation_failed", self.active)?;
                Err(e)
            }
        }
    }
    fn write_vm_trace(&mut self, root: &Path) -> Result<(), String> {
        let report = self.backing.report();
        if self.vm_trace_bytes + crate::vm_trace::MAX_BYTES > 16 * MIB {
            return Err("normal_pool_vm_trace_total_cap".into());
        }
        let arena = heap_meter::arena_snapshot().activation;
        let mut writer = crate::vm_trace::Writer::new(
            &root.join(format!("vm-regions-{:06}.jsonl", self.vm_trace_lists)),
            self.vm_trace_lists,
            report.epoch,
            self.pages.iter().flatten().count() + usize::from(arena.is_some()),
        )?;
        if let Some(a) = arena {
            writer.arena(a)?;
        }
        for (i, p) in self.pages.iter().enumerate() {
            if let Some(p) = p {
                let start = p.as_ptr() as usize;
                let nested = arena.is_some_and(|a| {
                    start >= a.arena_start && start + p.capacity() <= a.arena_start + a.arena_bytes
                });
                writer.range(crate::vm_trace::Range {
                    kind: "normal_pool_mapped_page",
                    ordinal: i,
                    allocation_generation: p.generation,
                    start,
                    len: p.len(),
                    capacity: p.capacity(),
                    logical_start: None,
                    logical_len: None,
                    nested_in_arena: nested,
                    grant_bytes: None,
                })?;
            }
        }
        let list = writer.finish()?;
        self.vm_trace_bytes += list.bytes;
        self.vm_trace_lists += 1;
        self.vm_trace_epoch = Some(report.epoch);
        Ok(())
    }
    fn now(&self) -> u64 {
        self.began.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
    fn scan(&mut self) -> Result<(), String> {
        let mut total = 0;
        let backing = self.backing.report();
        let owned = self
            .pages
            .iter()
            .flatten()
            .map(Page::capacity)
            .sum::<usize>();
        if backing.reserved_bytes != owned
            || backing.initialized_mapped_bytes != owned
            || backing.peak_initialized_mapped_bytes > BYTES
        {
            return Err("normal_pool_mapped_balance".into());
        }
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
            typed_instance: a.and_then(|v| v.typed_instance),
            typed_capacity: if a.is_some_and(|v| v.typed_instance.is_some()) {
                mycity_core::air::WORKSPACE_BYTES
            } else {
                0
            },
            summary: self.summary,
            owner_actual_capacity_bytes: self.pages.iter().flatten().map(Page::capacity).sum(),
            process_memory: ProcessMemory::capture(),
        };
        self.write_event(&e)
    }
    fn write_event(&mut self, e: &impl Serialize) -> Result<(), String> {
        self.write_event_bounded(e, EVENT_BYTES)
    }
    fn write_event_bounded(&mut self, e: &impl Serialize, row_limit: usize) -> Result<(), String> {
        struct Count<'a> {
            sink: &'a mut BufWriter<File>,
            used: &'a mut usize,
            row_bytes: usize,
            row_limit: usize,
        }
        impl Write for Count<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if self.row_bytes.saturating_add(b.len()) > self.row_limit {
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
            row_limit,
        };
        serde_json::to_writer(&mut sink, e).map_err(err)?;
        sink.write_all(b"\n").map_err(err)?;
        sink.flush().map_err(err)?;
        if self.vm_trace_enabled && self.vm_trace_epoch != Some(self.backing.report().epoch) {
            let root = self.broker_root.clone();
            self.write_vm_trace(&root)?;
        }
        Ok(())
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
        for i in WORKER_START_PAGE..64 {
            if self.pages[i].is_some() {
                return Err("normal_pool_duplicate_page".into());
            }
            if i == WORKER_START_PAGE + 1 && self.fault == Some(Fault::ReturnSecondPage) {
                self.fault = None;
                self.event("injected_partial_return_failure", Some(a))?;
                return Err("normal_pool_injected_return_page_failure".into());
            }
            self.pages[i] = Some(self.make_page()?);
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
                    typed_instance: None,
                    released_ns: self.now(),
                };
                for i in WORKER_START_PAGE..64 {
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
                typed_instance,
                typed_capacity,
                typed_scan_passed,
            } => {
                let mut a = self.active(id, key, true)?;
                if a.pid != Some(identity.process_pid)
                    || identity.process_instance_unix_ns == 0
                    || identity.run_epoch == 0
                    || identity.job == 0
                    || identity.generation == 0
                    || !(1..=3).contains(&identity.attempt)
                    || capacity_bytes != worker_slots::OWNER_BYTES
                    || available_touched_bytes != worker_slots::OWNER_BYTES
                    || typed_instance == 0
                    || typed_capacity != mycity_core::air::WORKSPACE_BYTES
                    || !typed_scan_passed
                    || !full_balance_scan_passed
                {
                    return Err(ActionError::Rejected("normal_pool_worker_fill_identity"));
                }
                if let Some(old) = a.filled {
                    if old != identity || a.typed_instance != Some(typed_instance) {
                        return Err(ActionError::Rejected("normal_pool_fill_already_bound"));
                    }
                    return Ok((Some(id), None));
                }
                a.filled = Some(identity);
                a.typed_instance = Some(typed_instance);
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
fn read_wire_into<T: serde::de::DeserializeOwned>(
    r: &mut impl Read,
    data: &mut [u8],
) -> Result<T, &'static str> {
    if data.len() != CONTROL {
        return Err("normal_pool_control_buffer_size");
    }
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
fn read_wire<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> Result<T, &'static str> {
    let mut data = [0; CONTROL];
    read_wire_into(r, &mut data)
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
    pub fn filled(
        &self,
        owner: &worker_slots::Owner,
        air: &mycity_core::air::AirWorkspace,
    ) -> Result<(), String> {
        let r = owner.report()?;
        let typed = air.report()?;
        if typed.loaded || !typed.allocation_set_stable {
            return Err("normal_pool_typed_initial_state".into());
        }
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
                    typed_instance: typed.workspace_instance,
                    typed_capacity: typed.payload_capacity_bytes,
                    typed_scan_passed: typed.full_fill_scan_passed,
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
    serve_inner(root, true, None, |_| Ok(false))
}
/// The same pool service/backing with a bounded nonblocking owner state machine.
/// The caller must actually wait every owned child before requesting shutdown.
/// This is a diagnostic candidate, not additional model or native memory credit.
pub(crate) fn serve_controlled(
    root: &Path,
    vm_trace: bool,
    control: impl FnMut(&Summary) -> Result<bool, String>,
) -> Result<(), String> {
    serve_inner(root, false, Some(vm_trace), control)
}
fn serve_inner(
    root: &Path,
    watch_parent: bool,
    explicit_vm_trace: Option<bool>,
    mut control: impl FnMut(&Summary) -> Result<bool, String>,
) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    let listener = UnixListener::bind(root.join("pool.sock")).map_err(err)?;
    listener.set_nonblocking(true).map_err(err)?;
    let stopped = Arc::new(AtomicBool::new(false));
    if watch_parent {
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
    }
    let mut pool = Pool::new(root)?;
    // In the combined root this allocation uses the already active 256 KiB
    // diagnostic arena. Keep one fixed frame buffer for the entire service loop.
    let mut control_buffer = Vec::new();
    control_buffer.try_reserve_exact(CONTROL).map_err(err)?;
    control_buffer.resize(CONTROL, 0);
    if let Some(arena) = heap_meter::arena_snapshot().activation {
        let start = control_buffer.as_ptr() as usize;
        let end = start
            .checked_add(control_buffer.capacity())
            .ok_or("normal_pool_control_buffer_address")?;
        let arena_end = arena
            .arena_start
            .checked_add(arena.arena_bytes)
            .ok_or("normal_pool_control_arena_address")?;
        if start < arena.arena_start || end > arena_end {
            return Err("normal_pool_control_buffer_outside_role_arena".into());
        }
    }
    let vm_trace = match explicit_vm_trace {
        Some(value) => value,
        None => match std::env::var("MYCITY_POOL_VM_TRACE").ok().as_deref() {
            None => false,
            Some("1") => true,
            Some(_) => return Err("normal_pool_vm_trace_mode".into()),
        },
    };
    if vm_trace {
        pool.vm_trace_enabled = true;
        pool.write_vm_trace(root)?;
    }
    let mut f = File::create_new(root.join("ready.json")).map_err(err)?;
    serde_json::to_writer(&mut f, &pool.ready).map_err(err)?;
    f.sync_all().map_err(err)?;
    let mut next_scan = Instant::now() + Duration::from_secs(1);
    let mut next_periodic_event = Instant::now() + Duration::from_secs(PERIODIC_EVENT_SECONDS);
    while !stopped.load(Ordering::Acquire) {
        if control(&pool.summary)? {
            break;
        }
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
                let request: Result<Request, _> = read_wire_into(&mut socket, &mut control_buffer);
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
    pool.event(
        if watch_parent {
            "parent_pipe_closed"
        } else {
            "controlled_children_waited"
        },
        pool.active,
    )?;
    let balanced = pool.active.is_none() && !pool.broker.registered();
    #[derive(Serialize)]
    struct Final {
        source_sha256: &'static str,
        ready: Ready,
        summary: Summary,
        balanced_after_owned_wait: bool,
        mapped_backing: mapped_pages::Report,
        rust_metadata_cap_bytes: usize,
        normal_owned_acceptance: &'static str,
        scope: &'static str,
    }
    let mut f = File::create_new(root.join("report.json")).map_err(err)?;
    serde_json::to_writer_pretty(&mut f,&Final{source_sha256:SOURCE,ready:pool.ready,summary:pool.summary,balanced_after_owned_wait:balanced,mapped_backing:pool.backing.report(),rust_metadata_cap_bytes:DIAGNOSTIC_BYTES,normal_owned_acceptance:"not_accepted_until_complete_joint_physical_validation",scope:"64MiB initialized region/page balance including whole Rust model roles and worker backing; continuously full joint physical pressure, native/stack and diagnostics require observed validation"}).map_err(err)?;
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
        Self::spawn_with_vm_trace(exe, root, heap_log, false)
    }
    /// Explicit address-trace constructor for isolated CLI diagnostics only.
    pub fn spawn_vm_diagnostic(
        exe: &Path,
        root: &Path,
        heap_log: Option<&Path>,
    ) -> Result<Self, String> {
        Self::spawn_with_vm_trace(exe, root, heap_log, true)
    }
    pub(crate) fn spawn_with_vm_trace(
        exe: &Path,
        root: &Path,
        heap_log: Option<&Path>,
        vm_trace: bool,
    ) -> Result<Self, String> {
        match fs::symlink_metadata(root) {
            Ok(_) => return Err("normal_pool_run_directory_exists".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(err(e)),
        }
        let mut cmd = Command::new(exe);
        cmd.env_remove("MYCITY_POOL_VM_TRACE");
        if vm_trace {
            cmd.env("MYCITY_POOL_VM_TRACE", "1");
        }
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
    fn controlled_test_root(name: &str) -> (Temp, PathBuf) {
        // Unix sockets admit at most SUN_LEN bytes. Unit-only metadata uses the
        // existing scoped temp fixture; full CLI restore artifacts stay on SSD.
        let temp = Temp::new();
        let root = temp.0.join(name);
        (temp, root)
    }
    #[test]
    fn controlled_owner_keeps_original_backing_and_requires_balanced_shutdown() {
        let (_temp, root) = controlled_test_root("balanced");
        let mut calls = 0;
        serve_controlled(&root, false, |summary| {
            assert_eq!(summary.capacity_bytes, BYTES);
            assert_eq!(summary.available_touched_bytes, BYTES);
            assert_eq!(summary.granted_capacity_bytes, 0);
            calls += 1;
            Ok(calls == 2)
        })
        .unwrap();
        let ready: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("ready.json")).unwrap()).unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("report.json")).unwrap()).unwrap();
        assert_eq!(calls, 2);
        assert_eq!(ready["pid"], std::process::id());
        assert_eq!(report["balanced_after_owned_wait"], true);
        assert_eq!(report["summary"]["capacity_bytes"], BYTES);
        assert_eq!(report["summary"]["available_touched_bytes"], BYTES);
        assert_eq!(report["summary"]["granted_capacity_bytes"], 0);
        assert_eq!(report["rust_metadata_cap_bytes"], DIAGNOSTIC_BYTES);
        let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
        assert!(events.contains("controlled_children_waited"));
        let first: serde_json::Value =
            serde_json::from_str(events.lines().next().unwrap()).unwrap();
        let process_memory = &first["mem_v1"];
        assert_eq!(process_memory["heap_bytes"].as_array().unwrap().len(), 2);
        assert!(
            process_memory["arena_bytes"].is_null()
                || process_memory["arena_bytes"].as_array().unwrap().len() == 5
        );
        assert_eq!(
            process_memory["arena_failures"].is_null(),
            process_memory["arena_bytes"].is_null()
        );
    }
    #[test]
    fn controlled_owner_failure_cannot_publish_a_successful_pool_report() {
        let (_temp, root) = controlled_test_root("failure");
        assert_eq!(
            serve_controlled(&root, false, |_| Err("controlled_test_failure".into())),
            Err("controlled_test_failure".into())
        );
        assert!(root.join("ready.json").exists());
        assert!(!root.join("report.json").exists());
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
            p.pages.iter().flatten().map(Page::capacity).sum::<usize>(),
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
            capacity_bytes: worker_slots::OWNER_BYTES,
            available_touched_bytes: worker_slots::OWNER_BYTES,
            full_balance_scan_passed: true,
            typed_instance: 1,
            typed_capacity: mycity_core::air::WORKSPACE_BYTES,
            typed_scan_passed: true,
        };
        assert!(p
            .action(Op::Filled {
                id,
                key,
                identity,
                capacity_bytes: worker_slots::OWNER_BYTES,
                available_touched_bytes: worker_slots::OWNER_BYTES,
                full_balance_scan_passed: true,
                typed_instance: 1,
                typed_capacity: mycity_core::air::WORKSPACE_BYTES,
                typed_scan_passed: true,
            })
            .is_err());
        for (typed_instance, typed_capacity, typed_scan_passed) in [
            (0, mycity_core::air::WORKSPACE_BYTES, true),
            (1, mycity_core::air::WORKSPACE_BYTES - 1, true),
            (1, mycity_core::air::WORKSPACE_BYTES, false),
        ] {
            assert!(p
                .action(Op::Filled {
                    id,
                    key: w,
                    identity,
                    capacity_bytes: worker_slots::OWNER_BYTES,
                    available_touched_bytes: worker_slots::OWNER_BYTES,
                    full_balance_scan_passed: true,
                    typed_instance,
                    typed_capacity,
                    typed_scan_passed,
                })
                .is_err());
            assert_eq!(p.summary.fill_acknowledgements, 0);
        }
        p.action(fill).unwrap();
        assert!(p
            .action(Op::Filled {
                id,
                key: w,
                identity,
                capacity_bytes: worker_slots::OWNER_BYTES,
                available_touched_bytes: worker_slots::OWNER_BYTES,
                full_balance_scan_passed: true,
                typed_instance: 2,
                typed_capacity: mycity_core::air::WORKSPACE_BYTES,
                typed_scan_passed: true,
            })
            .is_err());
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
                capacity_bytes: worker_slots::OWNER_BYTES,
                available_touched_bytes: worker_slots::OWNER_BYTES,
                full_balance_scan_passed: true,
                typed_instance: 1,
                typed_capacity: mycity_core::air::WORKSPACE_BYTES,
                typed_scan_passed: true,
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
            std::mem::size_of::<Pool>() + 2 * CONTROL + 40 * 1024 + 8192 + 64 * 1024 + CONTROL
                < DIAGNOSTIC_BYTES
        );
    }
    fn request_frame(boot: u128) -> Vec<u8> {
        serde_json::to_vec(&Request {
            source: Text::new(SOURCE).unwrap(),
            boot,
            action: Op::Status,
        })
        .unwrap()
    }
    #[test]
    fn control_wire_into_accepts_maximum_legal_frame_and_rejects_overflow_and_trailing_bytes() {
        let mut maximum = request_frame(11);
        maximum.resize(CONTROL - 1, b' ');
        maximum.push(b'\n');
        assert_eq!(maximum.len(), CONTROL);
        let mut buffer = vec![0; CONTROL];
        let parsed: Request = read_wire_into(&mut &maximum[..], &mut buffer).unwrap();
        assert_eq!(parsed.boot, 11);

        let overlong = vec![b' '; CONTROL + 1];
        assert!(matches!(
            read_wire_into::<Request>(&mut &overlong[..], &mut buffer),
            Err("normal_pool_control_bound")
        ));

        let mut trailing = request_frame(12);
        trailing.extend_from_slice(b"\n{}\n");
        assert!(matches!(
            read_wire_into::<Request>(&mut &trailing[..], &mut buffer),
            Err("normal_pool_control_trailing")
        ));
    }
    #[test]
    fn control_wire_into_reuses_buffer_without_leaking_previous_frame_bytes() {
        let mut long = request_frame(21);
        long.resize(2048, b' ');
        long.push(b'\n');
        let short = [request_frame(22), b"\n".to_vec()].concat();
        let mut buffer = vec![0xa5; CONTROL];

        let first: Request = read_wire_into(&mut &long[..], &mut buffer).unwrap();
        let second: Request = read_wire_into(&mut &short[..], &mut buffer).unwrap();
        assert_eq!(first.boot, 21);
        assert_eq!(second.boot, 22);
    }
    #[test]
    fn control_wire_into_reports_truncation_and_io_failure() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "synthetic read failure",
                ))
            }
        }

        let mut buffer = vec![0; CONTROL];
        assert!(matches!(
            read_wire_into::<Request>(&mut &b"{}"[..], &mut buffer),
            Err("normal_pool_control_eof")
        ));
        assert!(matches!(
            read_wire_into::<Request>(&mut FailingReader, &mut buffer),
            Err("normal_pool_control_io")
        ));
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
            typed_instance: Some(u64::MAX),
            typed_capacity: usize::MAX,
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
            process_memory: ProcessMemory {
                heap_bytes: [usize::MAX; 2],
                arena_bytes: Some([usize::MAX; 5]),
                arena_failures: Some(u64::MAX),
            },
        };
        let encoded = serde_json::to_vec(&event).unwrap();
        assert!(
            encoded.len() < EVENT_BYTES,
            "encoded_max_row_bytes={} limit={EVENT_BYTES}",
            encoded.len()
        );
        let decoded: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded["mem_v1"]["arena_bytes"][0], usize::MAX);
        let missing = serde_json::to_value(ProcessMemory {
            heap_bytes: [0; 2],
            arena_bytes: None,
            arena_failures: None,
        })
        .unwrap();
        assert!(missing["arena_bytes"].is_null());
        assert!(missing["arena_failures"].is_null());
        // Original724 windows plus retries (up to768),960 periodic checkpoints,
        // sixteen session handoffs and64 exceptional transitions. Diagnostic bound only.
        let rows = 5 * 768 + 28_800 / PERIODIC_EVENT_SECONDS + 8 * 16 + 64;
        assert!(rows as usize * EVENT_BYTES < 8 * MIB);
    }
}
