//! Broker owns real worker Children; Sim owns only a capability and a data socket.
//! Supervisor authority is held by the Client that owns and waits for the real Sim Child.
use super::*;
pub use super::startup_owner::StartupOwner;
use std::os::{
    fd::OwnedFd,
    unix::{fs::FileTypeExt, process::ExitStatusExt},
};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
// Fixed stack control storage avoids hidden heap allocations for large Start fields.
#[allow(clippy::large_enum_variant)]
pub(super) enum Op {
    RegistrationEpoch,
    Register {
        expected_generation: u64,
        supervisor: Text<64>,
        #[serde(default)]
        kind: workspace::Kind,
    },
    Bind {
        generation: u64,
        key: Text<64>,
        pid: u32,
    },
    Cancel {
        generation: u64,
        key: Text<64>,
    },
    AcknowledgeRevoke {
        generation: u64,
        key: Text<64>,
    },
    Revoke {
        generation: u64,
        key: Text<64>,
    },
    Start {
        generation: u64,
        key: Text<64>,
        endpoint: Text<256>,
        root: Text<1024>,
        instance: u128,
        gated: bool,
        diagnostics: bool,
    },
    Authorize {
        generation: u64,
        key: Text<64>,
        pid: u32,
        kind: workspace::Kind,
    },
    CodecFilled {
        generation: u64,
        key: Text<64>,
        identity: worker_slots::Identity,
        capacity: usize,
        touched: usize,
        balanced: bool,
        arena: mycity_role_allocator::Snapshot,
        backing_sha256: Text<64>,
    },
    Stop {
        generation: u64,
        key: Text<64>,
        lease: u64,
        kill: bool,
    },
    Kill {
        generation: u64,
        key: Text<64>,
        pid: u32,
        lease: u64,
    },
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Receipt {
    RegistrationEpoch { next: u64 },
    Registered { generation: u64, key: Text<64> },
    Bound { generation: u64 },
    Authorized { generation: u64, capacity: usize },
    CodecFilled { generation: u64 },
    Revoked { generation: u64 },
    Started { lease: u64, pid: u32 },
    Stopped { lease: u64, raw_status: i32 },
}
#[derive(Clone, Copy)]
struct Session {
    kind: workspace::Kind,
    generation: u64,
    supervisor: Text<64>,
    key: Text<64>,
    pid: Option<u32>,
    last_worker: Option<(u64, u32, i32)>,
}
struct OwnedWorker {
    child: Child,
    lease: u64,
}
impl Drop for OwnedWorker {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
#[derive(Default)]
pub(super) struct State {
    next: u64,
    session: Option<Session>,
    owned: Option<OwnedWorker>,
    last_closed: Option<(u64, Text<64>)>,
    awaiting_revoke_ack: bool,
}
impl State {
    pub(super) fn generation(&self) -> Option<u64> {
        self.session.map(|s| s.generation).or_else(|| {
            self.last_closed
                .filter(|_| self.awaiting_revoke_ack)
                .map(|s| s.0)
        })
    }
    pub(super) fn sim_pid(&self) -> Option<u32> {
        self.session.and_then(|s| s.pid)
    }
    pub(super) fn registered(&self) -> bool {
        self.session.is_some() || self.awaiting_revoke_ack
    }
    fn session(
        &self,
        generation: u64,
        key: Text<64>,
        supervisor: bool,
    ) -> Result<Session, ActionError> {
        self.session
            .filter(|s| {
                s.generation == generation
                    && if supervisor {
                        s.supervisor == key
                    } else {
                        s.key == key
                    }
            })
            .ok_or(ActionError::Rejected("normal_broker_session_identity"))
    }
}
impl Pool {
    /// The only broker credit path. Child is owned here, including on lost Start replies.
    fn broker_wait(&mut self, kill: bool) -> Result<Receipt, ActionError> {
        let owned = self
            .broker
            .owned
            .as_mut()
            .ok_or(ActionError::Rejected("normal_broker_no_worker"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            if let Some(status) = owned.child.try_wait().map_err(err)? {
                break status;
            }
            if kill || Instant::now() >= deadline {
                owned.child.kill().map_err(err)?;
                break owned.child.wait().map_err(err)?;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        let pid = owned.child.id();
        let lease = owned.lease;
        let a = self
            .active
            .filter(|a| a.id == lease && a.pid == Some(pid))
            .ok_or_else(|| ActionError::Fatal("normal_broker_owned_grant_identity".into()))?;
        self.event(
            if status.success() {
                "owned_child_wait_success"
            } else {
                "owned_child_wait_failure"
            },
            Some(a),
        )?;
        // Keep owned Child until credit publication succeeds; failure remains Fatal.
        self.give_back(a)?;
        self.broker.owned.take();
        let s = self
            .broker
            .session
            .as_mut()
            .ok_or_else(|| ActionError::Fatal("normal_broker_session_absent".into()))?;
        s.last_worker = Some((lease, pid, status.into_raw()));
        Ok(Receipt::Stopped {
            lease,
            raw_status: status.into_raw(),
        })
    }
    pub(super) fn broker_action(&mut self, op: Op) -> Result<Receipt, ActionError> {
        match op {
            Op::RegistrationEpoch => Ok(Receipt::RegistrationEpoch { next: self.broker.next }),
            Op::Register { supervisor, kind, expected_generation } => {
                // Exact original request can resolve a lost reply. It never allocates twice.
                if let Some(session) = self.broker.session {
                    if session.supervisor == supervisor && session.kind == kind
                        && session.generation == expected_generation
                    {
                        return Ok(Receipt::Registered { generation: session.generation, key: session.key });
                    }
                }
                // A closed request cannot become a fresh registration, even after later sessions.
                if expected_generation != self.broker.next + 1 {
                    return Err(ActionError::Rejected("normal_broker_registration_epoch"));
                }
                if self.broker.registered() || self.active.is_some() {
                    return Err(ActionError::Rejected("normal_broker_session_busy"));
                }
                if self.broker.next >= 1024 {
                    return Err(ActionError::Rejected("normal_broker_session_limit"));
                }
                let generation = self.broker.next + 1;
                let key = secret()?;
                self.prepare_codec(generation, kind)?;
                self.broker.session = Some(Session {
                    kind,
                    generation,
                    supervisor,
                    key,
                    pid: None,
                    last_worker: None,
                });
                self.broker.next = generation;
                self.event("broker_session_registered", None)?;
                Ok(Receipt::Registered { generation, key })
            }
            Op::Bind {
                generation,
                key,
                pid,
            } => {
                let s = self.broker.session(generation, key, true)?;
                if pid <= 1 || s.pid.is_some_and(|p| p != pid) {
                    return Err(ActionError::Rejected("normal_broker_sim_pid"));
                }
                self.bind_codec(generation, pid)?;
                self.broker.session.as_mut().expect("validated").pid = Some(pid);
                self.event(
                    if s.kind == workspace::Kind::SimCodec {
                        "broker_sim_bound"
                    } else {
                        "broker_helper_bound"
                    },
                    None,
                )?;
                Ok(Receipt::Bound { generation })
            }
            Op::Cancel { generation, key } => {
                if self.broker.last_closed == Some((generation, key)) {
                    return Ok(Receipt::Revoked { generation });
                }
                let s = self.broker.session(generation, key, true)?;
                if s.pid.is_some() || self.broker.owned.is_some() {
                    return Err(ActionError::Rejected("normal_broker_cancel_spawned"));
                }
                cleanup_endpoint(
                    self.ready,
                    Binding {
                        generation,
                        key: s.key,
                    },
                )?;
                self.retire_codec(generation)?;
                self.event("broker_unspawned_cancelled", None)?;
                self.broker.awaiting_revoke_ack = true;
                self.broker.session = None;
                self.broker.last_closed = Some((generation, key));
                Ok(Receipt::Revoked { generation })
            }
            Op::AcknowledgeRevoke { generation, key } => {
                if self.broker.last_closed != Some((generation, key)) {
                    return Err(ActionError::Rejected("normal_broker_revoke_ack_identity"));
                }
                if !self.broker.awaiting_revoke_ack {
                    // A duplicate receipt acknowledgement must not log the current
                    // session as if it belonged to the already closed generation.
                    return Ok(Receipt::Revoked { generation });
                }
                self.event("broker_revoke_receipt_acknowledged", None)?;
                self.broker.awaiting_revoke_ack = false;
                Ok(Receipt::Revoked { generation })
            }
            Op::Revoke { generation, key } => {
                if self.broker.last_closed == Some((generation, key)) {
                    return Ok(Receipt::Revoked { generation });
                }
                let s = self.broker.session(generation, key, true)?;
                if self.broker.owned.is_some() {
                    self.broker_wait(true)?;
                }
                cleanup_endpoint(
                    self.ready,
                    Binding {
                        generation,
                        key: s.key,
                    },
                )?;
                self.retire_codec(generation)?;
                self.event("broker_session_revoked_after_sim_wait", None)?;
                self.broker.awaiting_revoke_ack = true;
                self.broker.session = None;
                self.broker.last_closed = Some((generation, key));
                Ok(Receipt::Revoked { generation })
            }
            Op::Authorize {
                generation,
                key,
                pid,
                kind,
            } => {
                let session = self.broker.session(generation, key, false)?;
                if session.pid != Some(pid) {
                    return Err(ActionError::Rejected("normal_workspace_sim_unbound"));
                }
                if session.kind != kind {
                    return Err(ActionError::Rejected("normal_workspace_role_kind"));
                }
                Ok(Receipt::Authorized {
                    generation,
                    capacity: kind.bytes(),
                })
            }
            Op::CodecFilled {
                generation,
                key,
                identity,
                capacity,
                touched,
                balanced,
                arena,
                backing_sha256,
            } => {
                self.broker.session(generation, key, false)?;
                self.fill_codec(
                    generation,
                    identity,
                    capacity,
                    touched,
                    balanced,
                    workspace::Filled {
                        arena,
                        digest: backing_sha256,
                    },
                )?;
                Ok(Receipt::CodecFilled { generation })
            }
            Op::Start {
                generation,
                key,
                endpoint,
                root,
                instance,
                gated,
                diagnostics,
            } => {
                let s = self.broker.session(generation, key, false)?;
                if s.kind != workspace::Kind::SimCodec {
                    return Err(ActionError::Rejected(
                        "normal_broker_helper_cannot_start_worker",
                    ));
                }
                let pid = s
                    .pid
                    .ok_or(ActionError::Rejected("normal_broker_sim_unbound"))?;
                if !self.codec_ready(generation, pid) {
                    return Err(ActionError::Rejected("normal_workspace_not_filled"));
                }
                if self.broker.owned.is_some() || self.active.is_some() {
                    return Err(ActionError::Rejected("normal_broker_worker_busy"));
                }
                let endpoint = Path::new(endpoint.as_str());
                if endpoint != endpoint_path(self.ready, Binding { generation, key })
                    || !fs::symlink_metadata(endpoint)
                        .map(|m| m.file_type().is_socket())
                        .unwrap_or(false)
                    || !Path::new(root.as_str()).is_absolute()
                    || instance == 0
                {
                    return Err(ActionError::Rejected("normal_broker_endpoint_or_root"));
                }
                let (id, _) = self.legacy_action(super::Op::Prepare {
                    parent: pid,
                    parent_instance: instance,
                    key: secret()?,
                })?;
                let id =
                    id.ok_or_else(|| ActionError::Fatal("normal_broker_grant_absent".into()))?;
                let active = self.active.expect("prepared");
                let stream = UnixStream::connect(endpoint).map_err(err)?;
                fs::remove_file(endpoint).map_err(err)?;
                let input: OwnedFd = stream.try_clone().map_err(err)?.into();
                let output: OwnedFd = stream.into();
                let mut cmd = Command::new(std::env::current_exe().map_err(err)?);
                cmd.arg(if gated {
                    "compute-worker-bounded-probe"
                } else {
                    "compute-worker-bounded"
                })
                .arg(root.as_str())
                .stdin(Stdio::from(input))
                .stdout(Stdio::from(output))
                .stderr(Stdio::null())
                .env_remove("MYCITY_BROKER_SESSION")
                .env_remove("MYCITY_BROKER_SUPERVISOR");
                if diagnostics {
                    cmd.env("MYCITY_DIAGNOSTICS", "1");
                } else {
                    cmd.env_remove("MYCITY_DIAGNOSTICS");
                }
                // Pool root is inherited from the host's published root, not the Sim's environment.
                cmd.env("MYCITY_NORMAL_POOL", &self.broker_root).env(
                    "MYCITY_NORMAL_WORKER_GRANT",
                    serde_json::to_string(&WorkerBinding {
                        ready: self.ready,
                        id,
                        key: active.worker_key,
                    })
                    .map_err(err)?,
                );
                let result = cmd.spawn();
                drop(cmd); // critical: no broker-owned duplicate worker data endpoint remains
                let child = match result {
                    Ok(child) => child,
                    Err(e) => {
                        self.give_back(active)?;
                        return Err(ActionError::Rejected(
                            if e.kind() == std::io::ErrorKind::NotFound {
                                "normal_broker_worker_executable"
                            } else {
                                "normal_broker_worker_spawn"
                            },
                        ));
                    }
                };
                let worker_pid = child.id();
                self.broker.owned = Some(OwnedWorker { child, lease: id });
                self.legacy_action(super::Op::Attach {
                    id,
                    key: active.key,
                    pid: worker_pid,
                })?;
                Ok(Receipt::Started {
                    lease: id,
                    pid: worker_pid,
                })
            }
            Op::Stop {
                generation,
                key,
                lease,
                kill,
            } => {
                let s = self.broker.session(generation, key, false)?;
                if let Some((id, _, raw_status)) = s.last_worker {
                    if id == lease {
                        return Ok(Receipt::Stopped { lease, raw_status });
                    }
                }
                if self.broker.owned.as_ref().is_none_or(|w| w.lease != lease) {
                    return Err(ActionError::Rejected("normal_broker_worker_identity"));
                }
                self.broker_wait(kill)
            }
            Op::Kill {
                generation,
                key,
                pid,
                lease: expected,
            } => {
                let s = self.broker.session(generation, key, true)?;
                if let Some((lease, p, raw_status)) = s.last_worker {
                    if p == pid && lease == expected {
                        return Ok(Receipt::Stopped { lease, raw_status });
                    }
                }
                if !self
                    .broker
                    .owned
                    .as_ref()
                    .is_some_and(|w| w.child.id() == pid && w.lease == expected)
                {
                    return Err(ActionError::Rejected("normal_broker_worker_identity"));
                }
                self.broker_wait(true)
            }
        }
    }
}
fn rpc(root: &Path, ready: Ready, op: Op) -> Result<Receipt, String> {
    call(root, ready, super::Op::Broker(op))?
        .broker
        .ok_or("normal_broker_receipt_absent".into())
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    generation: u64,
    key: Text<64>,
}
fn endpoint_path(ready: Ready, binding: Binding) -> PathBuf {
    PathBuf::from(format!(
        "/tmp/mc-w-{:x}-{}-{}.sock",
        ready.boot as u64,
        binding.generation,
        &mycity_core::sha256(binding.key.as_str().as_bytes())[..16]
    ))
}
fn cleanup_endpoint(ready: Ready, binding: Binding) -> Result<(), String> {
    let p = endpoint_path(ready, binding);
    match fs::symlink_metadata(&p) {
        Ok(m) if m.file_type().is_socket() => fs::remove_file(p).map_err(err),
        Ok(_) => Err("normal_broker_endpoint_replaced".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(err(e)),
    }
}
/// Original registration capability, retained before any state-changing RPC.
/// No Clone, serialization or Debug: secret capability must not escape to diagnostics.
#[must_use = "retain registration until resolved or explicitly retired"]
pub struct PendingRegistration {
    root: PathBuf,
    ready: Ready,
    request: Op,
    transferred: bool,
}
impl PendingRegistration {
    pub fn new(root: &Path) -> Result<Self, String> { Self::new_kind(root, workspace::Kind::SimCodec) }
    pub(crate) fn new_kind(root: &Path, kind: workspace::Kind) -> Result<Self, String> {
        let ready = ready(root)?;
        let Receipt::RegistrationEpoch { next } = rpc(root, ready, Op::RegistrationEpoch)? else {
            return Err("normal_broker_registration_epoch_receipt".into());
        };
        let expected_generation = next.checked_add(1).ok_or("normal_broker_registration_epoch")?;
        let request = Op::Register { expected_generation, supervisor: secret()?, kind };
        Ok(Self { root: root.to_owned(), ready, request, transferred: false })
    }
    /// Failure leaves the SAME request/boot/secret here. Caller must retain this owner.
    pub fn resolve(&mut self) -> Result<Supervisor, String> {
        if self.transferred { return Err("normal_broker_registration_transferred".into()); }
        if ready(&self.root)? != self.ready { return Err("normal_broker_registration_ready_changed".into()); }
        let Op::Register { expected_generation, supervisor, .. } = self.request else { unreachable!() };
        let Receipt::Registered { generation, key: sim_key } = rpc(&self.root, self.ready, self.request)? else {
            return Err("normal_broker_register_receipt".into());
        };
        if generation != expected_generation || sim_key.as_str().len() != 64
            || !sim_key.as_str().bytes().all(|b| b.is_ascii_hexdigit())
        { return Err("normal_broker_register_generation_or_key".into()); }
        if ready(&self.root)? != self.ready { return Err("normal_broker_registration_ready_changed".into()); }
        self.transferred = true;
        Ok(Supervisor { root: std::mem::take(&mut self.root), ready: self.ready,
            binding: Binding { generation, key: supervisor }, sim_key, pid: None, closed: false })
    }
}

/// Held only by the process owning the real Sim Child. Never serialized to Sim.
pub struct Supervisor {
    root: PathBuf,
    ready: Ready,
    binding: Binding,
    sim_key: Text<64>,
    pid: Option<u32>,
    closed: bool,
}
impl Supervisor {
    pub fn from_env() -> Result<Option<Self>, String> {
        std::env::var_os("MYCITY_NORMAL_POOL")
            .map(|p| Self::register(Path::new(&p)))
            .transpose()
    }
    pub fn register(root: &Path) -> Result<Self, String> {
        Self::register_kind(root, workspace::Kind::SimCodec)
    }
    pub fn register_model(root: &Path) -> Result<Self, String> {
        Self::register_kind(root, workspace::Kind::Model)
    }
    pub fn register_helper(root: &Path) -> Result<Self, String> {
        Self::register_kind(root, workspace::Kind::HelperCompare)
    }
    pub(crate) fn register_kind(root: &Path, kind: workspace::Kind) -> Result<Self, String> {
        // Legacy callers are migrated in the following integration step. This entry is
        // deliberately kept visibly separate; it does not establish retained-startup safety.
        PendingRegistration::new_kind(root, kind)?.resolve()
    }
    /// Only legal before a Child exists; retain this owner until both replies confirm.
    pub fn cancel_unspawned(&mut self) -> Result<(), String> {
        if self.closed { return Ok(()); }
        if self.pid.is_some() { return Err("normal_broker_cancel_spawned".into()); }
        match rpc(&self.root, self.ready, Op::Cancel { generation: self.binding.generation, key: self.binding.key })? {
            Receipt::Revoked { generation } if generation == self.binding.generation => self.acknowledge_closed(),
            _ => Err("normal_broker_cancel_receipt".into()),
        }
    }
    fn acknowledge_closed(&mut self) -> Result<(), String> {
        match rpc(&self.root, self.ready, Op::AcknowledgeRevoke { generation: self.binding.generation, key: self.binding.key })? {
            Receipt::Revoked { generation } if generation == self.binding.generation => { self.closed = true; Ok(()) },
            _ => Err("normal_broker_revoke_ack_receipt".into()),
        }
    }
    pub fn configure(&self, cmd: &mut Command) -> Result<(), String> {
        cmd.env("MYCITY_NORMAL_POOL", &self.root)
            .env(
                "MYCITY_BROKER_SESSION",
                serde_json::to_string(&Binding {
                    generation: self.binding.generation,
                    key: self.sim_key,
                })
                .map_err(err)?,
            )
            .env_remove("MYCITY_BROKER_SUPERVISOR")
            .env_remove("MYCITY_NORMAL_WORKER_GRANT");
        Ok(())
    }
    pub fn bind(&mut self, pid: u32) -> Result<(), String> {
        self.pid = Some(pid); // preserve responsibility even if Bind reply is lost
        match rpc(
            &self.root,
            self.ready,
            Op::Bind {
                generation: self.binding.generation,
                key: self.binding.key,
                pid,
            },
        )? {
            Receipt::Bound { generation } if generation == self.binding.generation => Ok(()),
            _ => Err("normal_broker_bind_receipt".into()),
        }
    }
    pub fn kill_worker(&self, pid: u32, lease: u64) -> Result<(), String> {
        rpc(
            &self.root,
            self.ready,
            Op::Kill {
                generation: self.binding.generation,
                key: self.binding.key,
                pid,
                lease,
            },
        )?;
        Ok(())
    }
    pub fn after_sim_wait(&mut self, pid: u32, _status: ExitStatus) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        if self.pid != Some(pid) {
            return Err("normal_broker_sim_wait_identity".into());
        }
        match rpc(
            &self.root,
            self.ready,
            Op::Revoke {
                generation: self.binding.generation,
                key: self.binding.key,
            },
        )? {
            Receipt::Revoked { generation } if generation == self.binding.generation => {
                match rpc(
                    &self.root,
                    self.ready,
                    Op::AcknowledgeRevoke {
                        generation: self.binding.generation,
                        key: self.binding.key,
                    },
                )? {
                    Receipt::Revoked { generation } if generation == self.binding.generation => {
                        self.closed = true;
                        Ok(())
                    }
                    _ => Err("normal_broker_revoke_ack_receipt".into()),
                }
            }
            _ => Err("normal_broker_revoke_receipt".into()),
        }
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        if self.pid.is_none() && !self.closed {
            let _ = self.cancel_unspawned();
        }
    }
}

/// Trusted Sim startup only. Capacity was released by Register before Child spawn.
pub(crate) fn codec_epoch_from_env() -> Result<Option<u64>, String> {
    let Some(raw) = std::env::var_os("MYCITY_BROKER_SESSION") else {
        return if std::env::var_os("MYCITY_NORMAL_POOL").is_some() {
            Err("normal_workspace_binding_required".into())
        } else {
            Ok(None)
        };
    };
    let binding: Binding = crate::control_data::decode(
        raw.to_str()
            .ok_or("normal_workspace_binding_utf8")?
            .as_bytes(),
    )?;
    Ok(Some(binding.generation))
}
pub(crate) fn activate_role(kind: workspace::Kind) -> Result<u64, String> {
    let epoch = codec_epoch_from_env()?.unwrap_or(1);
    if let Some(raw) = std::env::var_os("MYCITY_BROKER_SESSION") {
        let b: Binding =
            crate::control_data::decode(raw.to_str().ok_or("normal_binding_utf8")?.as_bytes())?;
        let root = std::env::var_os("MYCITY_NORMAL_POOL").ok_or("normal_workspace_pool_absent")?;
        let root = Path::new(&root);
        let ready = ready(root)?;
        let began = Instant::now();
        loop {
            match rpc(
                root,
                ready,
                Op::Authorize {
                    generation: b.generation,
                    key: b.key,
                    pid: std::process::id(),
                    kind,
                },
            ) {
                Ok(Receipt::Authorized {
                    generation,
                    capacity,
                }) if generation == epoch && capacity == kind.bytes() => break,
                Err(e)
                    if e == "normal_workspace_sim_unbound"
                        && began.elapsed() < Duration::from_secs(5) =>
                {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(e) => return Err(e),
                _ => return Err("normal_workspace_authorize_receipt".into()),
            }
        }
    }
    crate::heap_meter::activate_role(kind.bytes())?;
    if std::env::var_os("MYCITY_ROLE_CONSTRUCTION_OOM").is_some() {
        let bytes = vec![0u8; kind.bytes() * 2];
        std::hint::black_box(bytes);
        return Err("role_construction_oom_probe_unexpected_success".into());
    }
    Ok(epoch)
}
pub(crate) fn acknowledge_role(epoch: u64, backing: &impl Serialize) -> Result<(), String> {
    let arena = crate::heap_meter::arena_snapshot();
    let a = arena.activation.ok_or("normal_role_not_constructed")?;
    let backing_sha256 = Text::new(&mycity_core::sha256(
        &serde_json::to_vec(backing).map_err(err)?,
    ))?;
    let Some(raw) = std::env::var_os("MYCITY_BROKER_SESSION") else {
        return Ok(());
    };
    let b: Binding =
        crate::control_data::decode(raw.to_str().ok_or("normal_binding_utf8")?.as_bytes())?;
    let root = std::env::var_os("MYCITY_NORMAL_POOL").ok_or("normal_workspace_pool_absent")?;
    let root = Path::new(&root);
    let ready = ready(root)?;
    let identity = worker_slots::Identity {
        process_pid: std::process::id(),
        process_instance_unix_ns: heap_meter::process_instance(),
        run_epoch: epoch,
        owner_instance: a.arena_start as u64,
        job: 1,
        generation: 1,
        attempt: 1,
    };
    match rpc(
        root,
        ready,
        Op::CodecFilled {
            generation: b.generation,
            key: b.key,
            identity,
            capacity: a.grant_bytes,
            touched: a.grant_bytes,
            balanced: true,
            arena,
            backing_sha256,
        },
    )? {
        Receipt::CodecFilled { generation } if generation == epoch => Ok(()),
        _ => Err("normal_workspace_fill_receipt".into()),
    }
}

/// Logical worker lease, never a fabricated Child. Slot returns happen only at broker wait.
pub struct Lease {
    root: PathBuf,
    ready: Ready,
    binding: Binding,
    id: u64,
    pid: u32,
    stopped: Option<ExitStatus>,
}
/// Start request owner exists before RPC. An unconfirmed Start is never replayed:
/// broker may already own a Child; only the original supervisor session can retire it.
#[must_use = "retain until Started is stored, or terminate the original session"]
pub struct PendingStart {
    root: PathBuf,
    ready: Ready,
    binding: Binding,
    endpoint: PathBuf,
    listener: UnixListener,
    request: Op,
    attempted: bool,
    confirmed: bool,
}
impl PendingStart {
    pub fn from_env(root: &Path, gated: bool) -> Result<Option<Self>, String> {
        let Some(raw) = std::env::var_os("MYCITY_BROKER_SESSION") else { return Ok(None); };
        let binding: Binding = crate::control_data::decode(
            raw.to_str().ok_or("normal_broker_binding_utf8")?.as_bytes(),
        )?;
        let pool = PathBuf::from(std::env::var_os("MYCITY_NORMAL_POOL")
            .ok_or("normal_broker_pool_absent")?);
        let ready = ready(&pool)?;
        let endpoint = endpoint_path(ready, binding);
        // Construct/validate all request fields before binding any endpoint or sending RPC.
        let request = Op::Start {
            generation: binding.generation, key: binding.key,
            endpoint: Text::new(endpoint.to_str().ok_or("normal_broker_endpoint_utf8")?)?,
            root: Text::new(root.to_str().ok_or("normal_broker_root_utf8")?)?,
            instance: heap_meter::process_instance(), gated,
            diagnostics: crate::cpu_log::enabled(),
        };
        let listener = UnixListener::bind(&endpoint).map_err(err)?;
        let pending = Self { root: pool, ready, binding, endpoint, listener, request,
            attempted: false, confirmed: false };
        fs::set_permissions(&pending.endpoint,
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600)).map_err(err)?;
        pending.listener.set_nonblocking(true).map_err(err)?;
        Ok(Some(pending))
    }
    pub fn unconfirmed(&self) -> bool { self.attempted && !self.confirmed }
    /// Caller stores the returned Lease before ANY accept/clone/reader operation.
    pub fn start(&mut self) -> Result<Lease, String> {
        if self.attempted { return Err("normal_broker_start_not_replayable".into()); }
        self.attempted = true;
        let result = rpc(&self.root, self.ready, self.request)?;
        let Receipt::Started { lease, pid } = result else {
            return Err("normal_broker_start_receipt".into());
        };
        if lease == 0 || pid == 0 { return Err("normal_broker_start_identity".into()); }
        self.confirmed = true;
        Ok(Lease { root: std::mem::take(&mut self.root), ready: self.ready,
            binding: self.binding, id: lease, pid, stopped: None })
    }
    pub fn accept(&mut self) -> Result<UnixStream, String> {
        if !self.confirmed { return Err("normal_broker_start_unconfirmed".into()); }
        let began = Instant::now();
        let stream = loop {
            match self.listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                    && began.elapsed() < Duration::from_secs(2) => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => return Err(err(e)),
            }
        };
        stream.set_nonblocking(false).map_err(err)?;
        Ok(stream)
    }
}
impl Drop for PendingStart {
    fn drop(&mut self) {
        // Unknown Start is NOT cancellation. Preserve its path for original revoke.
        if !self.unconfirmed() { let _ = cleanup_endpoint(self.ready, self.binding); }
    }
}
impl Lease {
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn stop(&mut self, kill: bool) -> Result<ExitStatus, String> {
        if let Some(s) = self.stopped {
            return Ok(s);
        }
        let r = rpc(
            &self.root,
            self.ready,
            Op::Stop {
                generation: self.binding.generation,
                key: self.binding.key,
                lease: self.id,
                kill,
            },
        )?;
        let Receipt::Stopped { lease, raw_status } = r else {
            return Err("normal_broker_stop_receipt".into());
        };
        if lease != self.id {
            return Err("normal_broker_stop_identity".into());
        }
        let status = ExitStatus::from_raw(raw_status);
        self.stopped = Some(status);
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_registration_replay_never_reallocates_or_resurrects_closed_epochs() {
        let root = std::env::temp_dir().join(format!("mc-register-epochs-{}-{}", std::process::id(), heap_meter::process_instance()));
        fs::create_dir(&root).unwrap();
        let mut pool = Pool::new(&root).unwrap();
        let secret1 = secret().unwrap();
        let original = Op::Register { supervisor: secret1, kind: workspace::Kind::SimCodec, expected_generation: 1 };
        let Receipt::Registered { generation, key } = pool.broker_action(original).unwrap() else { panic!("register") };
        let before = pool.summary;
        let events = pool.event_bytes;
        let Receipt::Registered { generation: again, key: again_key } = pool.broker_action(original).unwrap() else { panic!("replay") };
        assert_eq!((again, again_key), (generation, key));
        assert_eq!(pool.summary.workspace_grants, before.workspace_grants);
        assert_eq!(pool.event_bytes, events);
        assert!(pool.broker_action(Op::Register { supervisor: secret1, kind: workspace::Kind::Model, expected_generation: 1 }).is_err());
        assert!(pool.broker_action(Op::Register { supervisor: secret().unwrap(), kind: workspace::Kind::SimCodec, expected_generation: 2 }).is_err());
        pool.broker_action(Op::Cancel { generation, key: secret1 }).unwrap();
        let returns = pool.summary.workspace_returns;
        pool.broker_action(Op::Cancel { generation, key: secret1 }).unwrap();
        assert_eq!(pool.summary.workspace_returns, returns);
        assert!(pool.broker_action(original).is_err());
        let secret2 = secret().unwrap();
        let next = Op::Register { supervisor: secret2, kind: workspace::Kind::SimCodec, expected_generation: 2 };
        assert!(pool.broker_action(next).is_err(), "cancel without ack must block a new session");
        pool.broker_action(Op::AcknowledgeRevoke { generation, key: secret1 }).unwrap();
        let Receipt::Registered { generation: g2, .. } = pool.broker_action(next).unwrap() else { panic!("new") };
        assert!(pool.broker_action(original).is_err());
        pool.broker_action(Op::Cancel { generation: g2, key: secret2 }).unwrap();
        pool.broker_action(Op::AcknowledgeRevoke { generation: g2, key: secret2 }).unwrap();
        assert!(pool.broker_action(original).is_err(), "even an older-than-last tombstone cannot revive");
        assert_eq!(pool.summary.workspace_grants, 2);
        assert_eq!(pool.summary.workspace_returns, 2);
        assert_eq!(pool.summary.available_touched_bytes, BYTES);
        drop(pool);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn helper_capability_cannot_start_worker_or_overlap_a_live_session() {
        let root = std::env::temp_dir().join(format!(
            "mc-helper-auth-{}-{}",
            std::process::id(),
            heap_meter::process_instance()
        ));
        fs::create_dir(&root).unwrap();
        let mut pool = Pool::new(&root).unwrap();
        let supervisor = secret().unwrap();
        let Receipt::Registered { generation, key } = pool
            .broker_action(Op::Register {
                expected_generation: pool.broker.next + 1,
                supervisor,
                kind: workspace::Kind::HelperCompare,
            })
            .unwrap()
        else {
            panic!("register")
        };
        assert!(matches!(
            pool.broker_action(Op::Start {
                generation,
                key,
                endpoint: Text::new("/tmp/unused").unwrap(),
                root: Text::new("/tmp").unwrap(),
                instance: 1,
                gated: false,
                diagnostics: false
            }),
            Err(ActionError::Rejected(
                "normal_broker_helper_cannot_start_worker"
            ))
        ));
        assert!(pool
            .broker_action(Op::Register {
                expected_generation: pool.broker.next + 1,
                supervisor: secret().unwrap(),
                kind: workspace::Kind::SimCodec
            })
            .is_err());
        pool.broker_action(Op::Cancel {
            generation,
            key: supervisor,
        })
        .unwrap();
        assert_eq!(pool.summary.available_touched_bytes, BYTES);
        drop(pool);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn old_generation_capabilities_and_legacy_calls_cannot_touch_new_registration() {
        let root = std::env::temp_dir().join(format!(
            "mc-broker-auth-{}-{}",
            std::process::id(),
            heap_meter::process_instance()
        ));
        fs::create_dir(&root).unwrap();
        let mut pool = Pool::new(&root).unwrap();
        let supervisor = secret().unwrap();
        let Receipt::Registered {
            generation: old,
            key: old_key,
        } = pool
            .broker_action(Op::Register {
                expected_generation: pool.broker.next + 1,
                supervisor,
                kind: workspace::Kind::SimCodec,
            })
            .unwrap()
        else {
            panic!("register");
        };
        pool.broker_action(Op::Cancel {
            generation: old,
            key: supervisor,
        })
        .unwrap();
        pool.broker_action(Op::AcknowledgeRevoke { generation: old, key: supervisor }).unwrap();
        let supervisor2 = secret().unwrap();
        let Receipt::Registered {
            generation: new,
            key: new_key,
        } = pool
            .broker_action(Op::Register {
                expected_generation: pool.broker.next + 1,
                supervisor: supervisor2,
                kind: workspace::Kind::SimCodec,
            })
            .unwrap()
        else {
            panic!("register");
        };
        for op in [
            Op::Stop {
                generation: old,
                key: old_key,
                lease: 1,
                kill: true,
            },
            Op::Kill {
                generation: old,
                key: supervisor,
                pid: 2,
                lease: 1,
            },
            Op::Revoke {
                generation: new,
                key: supervisor,
            },
        ] {
            assert!(pool.broker_action(op).is_err());
        }
        pool.broker_action(Op::Revoke {
            generation: old,
            key: supervisor,
        })
        .unwrap();
        let event_bytes = pool.event_bytes;
        pool.broker_action(Op::AcknowledgeRevoke {
            generation: old,
            key: supervisor,
        })
        .unwrap();
        assert_eq!(
            pool.event_bytes, event_bytes,
            "old receipt cannot append a new-generation acknowledgement"
        ); // old duplicate is a tombstone, not a new revoke
        let current = pool.broker.session(new, new_key, false).unwrap();
        assert_eq!(current.generation, new);
        assert!(pool
            .action(super::super::Op::Prepare {
                parent: 2,
                parent_instance: 1,
                key: secret().unwrap()
            })
            .is_err());
        assert_eq!(pool.summary.grants, 0);
        // The NEW generation still owns four prepared pages. An old tombstone
        // cannot return that capacity or make the pool report 64MiB available.
        assert_eq!(
            pool.summary.available_touched_bytes,
            BYTES - super::super::workspace::BYTES
        );
        assert_eq!(
            pool.summary.workspace_capacity_bytes,
            super::super::workspace::BYTES
        );
        assert_eq!(pool.summary.workspace_grants, 2);
        assert_eq!(pool.summary.workspace_returns, 1);
        pool.broker_action(Op::Cancel {
            generation: new,
            key: supervisor2,
        })
        .unwrap();
        assert_eq!(pool.summary.available_touched_bytes, BYTES);
        assert_eq!(
            pool.summary.workspace_grants,
            pool.summary.workspace_returns
        );
        drop(pool);
        fs::remove_dir_all(root).unwrap();
    }
}
