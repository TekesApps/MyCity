//! Real service/RPC fixtures, included privately from normal_pool.rs by root.
//! Source only until root runs them. Shell children are not air fulfilment.
use super::*;
use crate::compute_worker::Worker;
use std::fs::OpenOptions;
use std::sync::{atomic::AtomicU64, Mutex};
use std::thread::{self, JoinHandle};

static SERIAL: Mutex<()> = Mutex::new(());
static NEXT: AtomicU64 = AtomicU64::new(1);
const EXPECTED_CAPACITY: usize = 64 * 1024 * 1024;
const EXPECTED_GRANT: usize = 5 * 1024 * 1024;
// One original failed fixture, not a ledger or reconstructed recovery token.
// Never start another fixture while its original capabilities remain here.
static RETAINED: Mutex<Option<State>> = Mutex::new(None);

struct State {
    root: PathBuf,
    stop: Arc<AtomicBool>,
    service: Option<JoinHandle<Result<(), String>>>,
    pending: Option<PendingGrant>,
    grant: Option<Grant>,
    child: Option<Child>,
    reaped: Option<ExitStatus>,
    worker: Option<Worker>,
    worker_paths: Option<(PathBuf, PathBuf)>,
    worker_exit: Option<ExitStatus>,
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.into()) }
}

impl State {
    fn start(fault: Fault) -> Result<Self, String> {
        let base = std::env::var_os("MYCITY_PROTOCOL_POOL_ROOT")
            .ok_or("MYCITY_PROTOCOL_POOL_ROOT is required; no fallback")?;
        let base = fs::canonicalize(base).map_err(err)?;
        let work = fs::canonicalize("/Volumes/HP P900/dev/MyCity/work").map_err(err)?;
        require(base.starts_with(work) && base.is_dir(), "pool fixture must use existing HP work root")?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map_err(err)?.subsec_nanos();
        let root = base.join(format!("p102-{:x}-{:x}-{:x}",
            std::process::id(), nonce, NEXT.fetch_add(1, Ordering::Relaxed)));
        // macOS sockaddr_un includes a terminating byte in its 104-byte path.
        require(root.join("pool.sock").as_os_str().len() < 104, "set a shorter external pool root")?;
        require(!root.exists(), "fixture path exists; originals must not be replaced")?;
        eprintln!("issue102_pool_fixture_root={}", root.display());
        let stop = Arc::new(AtomicBool::new(false));
        let service_stop = Arc::clone(&stop);
        let service_root = root.clone();
        let service = thread::Builder::new().name("p102-real-pool".into()).spawn(move || {
            serve_inner(&service_root, false, Some(false), Some(fault), move |_| {
                Ok(service_stop.load(Ordering::Acquire))
            })
        }).map_err(err)?;
        Ok(Self { root, stop, service: Some(service), pending: None, grant: None,
            child: None, reaped: None, worker: None, worker_paths: None, worker_exit: None })
    }

    fn record(&self, phase: &str, result: &str, summary: Option<Summary>) -> Result<(), String> {
        let row = serde_json::json!({
            "source_sha256": SOURCE, "phase": phase, "result": result,
            "summary": summary, "fixture_root": self.root,
            "owner_pid": std::process::id(), "owner_instance": heap_meter::process_instance(),
            "child_pid": self.child.as_ref().map(Child::id),
            "actual_wait_status": self.reaped.map(|status| format!("{status}")),
            "grant_identity": self.grant.as_ref().map(Grant::protocol_identity),
            "grant_attached_pid": self.grant.as_ref().and_then(|grant| grant.pid),
            "grant_acknowledged_return": self.grant.as_ref().map(|grant| grant.returned),
            "pending_resolved": self.pending.as_ref().map(|pending| pending.resolved),
            "worker_retirement_state": self.worker.as_ref().map(Worker::retirement_state),
            "worker_acknowledged_exit": self.worker_exit.map(|status| format!("{status}")),
            "worker_paths": self.worker_paths,
            "scope": "real_pool_rpc_and_owned_shell_child_not_air_or_budget_acceptance"
        });
        let mut file = OpenOptions::new().create(true).append(true)
            .open(self.root.join("fixture-commands.jsonl")).map_err(err)?;
        serde_json::to_writer(&mut file, &row).map_err(err)?;
        file.write_all(b"\n").map_err(err)?;
        file.sync_all().map_err(err)
    }

    fn wait_ready(&self) -> Result<(), String> {
        let began = Instant::now();
        loop {
            match status(&self.root) {
                Ok(summary) => return self.record("ready", "ok", Some(summary)),
                Err(error) => {
                    if self.service.as_ref().is_some_and(|service| service.is_finished()) {
                        return Err(format!("pool exited before ready: {error}"));
                    }
                    if began.elapsed() >= Duration::from_secs(15) {
                        return Err(format!("pool ready deadline: {error}"));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }

    fn snapshot(&self, phase: &str, grants: u64, returns: u64) -> Result<Summary, String> {
        let summary = status(&self.root)?;
        self.record(phase, "observed", Some(summary))?;
        require(BYTES == EXPECTED_CAPACITY && GRANT_BYTES == EXPECTED_GRANT,
            "original 64MiB/5MiB fixture contract changed")?;
        require(summary.capacity_bytes == EXPECTED_CAPACITY, "original 64MiB capacity changed")?;
        require(summary.grants == grants && summary.returns == returns, "unexpected grant/return count")?;
        let outstanding = grants.checked_sub(returns).ok_or("negative outstanding grants")?;
        require(outstanding <= 1, "fixture must own at most one original grant")?;
        let expected = if outstanding == 0 { 0 } else { EXPECTED_GRANT };
        require(summary.granted_capacity_bytes == expected, "wrong granted byte balance")?;
        require(summary.available_touched_bytes == EXPECTED_CAPACITY - expected, "wrong available byte balance")?;
        Ok(summary)
    }

    fn event_id(&self, kind: &str) -> Result<u64, String> {
        let bytes = mycity_core::read_bounded(
            File::open(self.root.join("events.jsonl")).map_err(err)?, MIB as u64,
        ).map_err(err)?;
        let rows = std::str::from_utf8(&bytes).map_err(err)?;
        let mut found = None;
        for line in rows.lines() {
            let row: serde_json::Value = serde_json::from_str(line).map_err(err)?;
            if row["kind"].as_str() == Some(kind) {
                require(found.is_none(), "duplicate event for the sole original grant")?;
                found = row["id"].as_u64();
            }
        }
        found.ok_or_else(|| format!("original server event missing: {kind}"))
    }

    fn begin_pending(&mut self) -> Result<(), String> {
        require(self.pending.is_none() && self.grant.is_none(), "duplicate pending owner")?;
        self.pending = Some(PendingGrant::new(&self.root)?);
        Ok(())
    }

    fn resolve(&mut self) -> Result<(), String> {
        require(self.grant.is_none(), "already have a live grant")?;
        let grant = self.pending.as_mut().ok_or("pending missing")?.resolve()?;
        // Store the original capability before any further fallible observation.
        self.grant = Some(grant);
        Ok(())
    }

    fn spawn_shell(&mut self) -> Result<u32, String> {
        require(self.child.is_none(), "already own a child")?;
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "read protocol_line"])
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
        self.grant.as_ref().ok_or("grant missing")?.configure(&mut command)?;
        let child = command.spawn().map_err(err)?;
        let pid = child.id();
        self.child = Some(child);
        self.record("shell_spawn", "ok; blocked on owned stdin, not an air worker", None)?;
        Ok(pid)
    }

    fn child_is_live(&mut self) -> Result<bool, String> {
        if self.reaped.is_none() {
            self.reaped = self.child.as_mut().ok_or("child missing")?.try_wait().map_err(err)?;
        }
        Ok(self.reaped.is_none())
    }

    fn wait_child(&mut self, kill: bool) -> Result<ExitStatus, String> {
        if let Some(status) = self.reaped { return Ok(status); }
        let child = self.child.as_mut().ok_or("child missing")?;
        drop(child.stdin.take());
        if kill { let _ = child.kill(); }
        let status = child.wait().map_err(err)?;
        self.reaped = Some(status);
        self.record("actual_child_wait", "ok; status retained before Reap RPC", None)?;
        Ok(status)
    }

    fn return_waited(&mut self) -> Result<(), String> {
        let status = self.reaped.ok_or("no actual wait; refuse to mint a status")?;
        let pid = self.child.as_ref().ok_or("child missing")?.id();
        self.grant.as_mut().ok_or("grant missing")?.after_wait(pid, status)
    }

    fn abort_unspawned(&mut self) -> Result<(), String> {
        require(self.child.is_none(), "spawned child cannot use Abort")?;
        let grant = self.grant.as_mut().ok_or("grant missing")?;
        require(grant.pid.is_none(), "attached grant cannot use Abort")?;
        // Same original Abort RPC as Grant::drop, explicitly retain cap on error.
        call(&grant.root, grant.ready, Op::Abort { id: grant.id, key: grant.key })?;
        grant.returned = true; // Set only after the actual server acknowledgement.
        Ok(())
    }

    fn init_worker(&mut self) -> Result<(), String> {
        require(self.worker.is_none() && self.pending.is_none()
            && self.grant.is_none() && self.child.is_none(), "mixed lifecycle owners")?;
        // Install the persistent owner BEFORE any fallible setup or RPC.
        self.worker = Some(Worker::new());
        let executable = PathBuf::from(std::env::var_os("MYCITY_PROTOCOL_WORKER_EXE")
            .ok_or("MYCITY_PROTOCOL_WORKER_EXE is required; no fallback")?);
        require(executable.is_absolute() && executable.is_file(),
            "worker executable must be an existing absolute file")?;
        let task = self.root.join("w");
        self.worker_paths = Some((executable, task.clone()));
        fs::create_dir(&task).map_err(err)?;
        fs::create_dir(task.join("inputs")).map_err(err)?;
        fs::create_dir(task.join("candidates")).map_err(err)?;
        self.record("worker_owner_installed", "Empty; CLI path required; no business request", None)
    }

    fn start_worker(&mut self) -> Result<(), String> {
        let (executable, task) = self.worker_paths.as_ref().ok_or("worker paths missing")?;
        self.worker.as_mut().ok_or("persistent worker missing")?
            .start_bounded(executable, task, Some(&self.root), false)
    }

    fn worker_state(&self) -> Result<serde_json::Value, String> {
        Ok(self.worker.as_ref().ok_or("persistent worker missing")?.retirement_state())
    }

    fn retire_worker(&mut self) -> Result<Option<ExitStatus>, String> {
        let result = self.worker.as_mut().ok_or("persistent worker missing")?
            .retire(Duration::from_secs(1))?;
        if let Some(status) = result { self.worker_exit = Some(status); }
        Ok(result)
    }

    fn require_repeated_start_rejected(&mut self) -> Result<(), String> {
        let before = self.worker_state()?;
        let repeated = self.start_worker();
        self.record("same_worker_repeated_start", &format!("{repeated:?}"), None)?;
        require(repeated.as_ref().err().is_some_and(|error| error == "worker_owner_not_retired"),
            "start must reject the original nonempty owner")?;
        require(self.worker_state()? == before, "rejected start changed original owner")
    }

    fn require_empty_retire(&mut self) -> Result<(), String> {
        require(self.worker.as_ref().ok_or("persistent worker missing")?.is_empty(),
            "owner must be Empty before the local empty-retire check")?;
        let cached = self.worker_exit;
        let empty_result = self.retire_worker();
        self.record("empty_worker_retire", &format!("{empty_result:?}; local Empty result, not another Reap ACK"), None)?;
        require(empty_result?.is_none(), "Empty retirement must not manufacture an exit")?;
        require(self.worker_exit == cached, "Empty retirement replaced observed exit evidence")?;
        self.snapshot("empty_retire_no_additional_return", 1, 1)?;
        Ok(())
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if self.worker.is_some() {
            // The original persistent Worker stays in State on every failure.
            let first = self.retire_worker();
            self.record("cleanup_same_worker_retire", &format!("{first:?}"), None)?;
            if let Err(error) = first {
                self.record("cleanup_same_worker_retry_reason", &error, None)?;
                let second = self.retire_worker();
                self.record("cleanup_same_worker_second_retire", &format!("{second:?}"), None)?;
                second?;
            }
            require(self.worker.as_ref().ok_or("persistent worker missing")?.is_empty(),
                "unconfirmed Worker must remain in original retained State")?;
        }
        if self.child.is_some() {
            // Even a setup failure before attach must record PID BEFORE RPC.
            let pid = self.child.as_ref().ok_or("child missing")?.id();
            if self.grant.as_ref().is_some_and(|grant| grant.pid.is_none()) {
                let attached = self.grant.as_mut().ok_or("grant missing")?.attach(pid);
                let _ = self.record("cleanup_attach", &format!("{attached:?}"), None);
            }
            self.wait_child(true)?;
            if !self.grant.as_ref().ok_or("grant missing")?.returned {
                // One explicit retry handles the selected one-shot transport cutpoint.
                let first = self.return_waited();
                if let Err(error) = first {
                    self.record("cleanup_reap_first_error", &error, None)?;
                    self.return_waited()?;
                }
            }
        } else {
            if self.grant.is_none() && self.pending.as_ref().is_some_and(|pending| !pending.resolved) {
                let first = self.resolve();
                if let Err(error) = first {
                    self.record("cleanup_pending_first_error", &error, None)?;
                    self.resolve()?;
                }
            }
            if self.grant.as_ref().is_some_and(|grant| !grant.returned) {
                self.abort_unspawned()?;
            }
        }
        // Never stop the server just because a local object disappeared.
        if self.root.join("ready.json").exists() {
            let summary = status(&self.root)?;
            self.record("cleanup_balance", "observed", Some(summary))?;
            require(summary.grants == summary.returns
                && summary.available_touched_bytes == BYTES
                && summary.granted_capacity_bytes == 0, "unbalanced cleanup; retain original owner")?;
        } else {
            require(self.pending.is_none() && self.grant.is_none() && self.child.is_none(),
                "missing ready with live capability; retain owner")?;
        }
        self.stop.store(true, Ordering::Release);
        if let Some(service) = self.service.take() {
            service.join().map_err(|_| "real pool service panicked")??;
        }
        require(self.root.join("report.json").is_file(), "pool report absent")?;
        self.record("service_join", "ok; original ready/events/report retained", None)
    }
}

struct Guard { state: Option<State> }
impl Guard {
    fn finish(&mut self) -> Result<(), String> {
        let Some(mut state) = self.state.take() else { return Ok(()); };
        match state.cleanup() {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = state.record("failed_owner_retained", &error, None);
                let mut retained = RETAINED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                // SERIAL prevents another fixture taking this single owner slot.
                if retained.is_none() { *retained = Some(state); }
                else {
                    // Defensive preservation if SERIAL contract is ever broken.
                    // Neither drop a live Grant nor pretend recovery succeeded.
                    std::mem::forget(state);
                }
                Err(format!("fixture failed; original owner retained, no balanced success: {error}"))
            }
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) { let _ = self.finish(); }
}

fn run(fault: Fault, body: impl FnOnce(&mut State) -> Result<(), String>) -> Result<(), String> {
    let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    require(RETAINED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_none(),
        "a failed fixture still owns capabilities; refuse another real pool")?;
    let mut guard = Guard { state: Some(State::start(fault)?) };
    let state = guard.state.as_mut().ok_or("fixture missing")?;
    let result = state.wait_ready().and_then(|()| body(state));
    let log_result = state.record("test_body", &format!("{result:?}"), None);
    let cleanup = guard.finish();
    let errors: Vec<String> = [result, log_result, cleanup].into_iter()
        .filter_map(Result::err).collect();
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[test]
fn real_prepare_lost_ack_replays_same_pending_then_unspawned_abort_once() -> Result<(), String> {
    run(Fault::AfterPrepareReply, |state| {
        state.snapshot("before_prepare", 0, 0)?;
        state.begin_pending()?;
        let first = state.resolve();
        state.record("prepare_first_rpc", &format!("{first:?}"), None)?;
        require(first.is_err(), "Prepare fault did not lose acknowledgement")?;
        state.snapshot("prepare_applied_ack_lost", 1, 0)?;
        let issued_id = state.event_id("grant_released_before_spawn")?;
        state.resolve()?;
        let grant = state.grant.as_ref().ok_or("grant missing")?;
        let original = grant.protocol_identity();
        require(original.0 == ready(&state.root)?.boot && original.1 != 0, "original boot/id mismatch")?;
        require(original.1 == issued_id, "pending retry did not recover the original issued id")?;
        state.snapshot("same_pending_resolved_no_second_grant", 1, 0)?;
        match state.pending.as_mut().ok_or("pending missing")?.resolve() {
            Err(error) => require(error == "normal_pool_prepare_already_resolved", "wrong duplicate error")?,
            Ok(unexpected) => {
                // No Child exists in this case; its original Abort is safe.
                drop(unexpected);
                return Err("duplicate resolve unexpectedly succeeded".into());
            }
        }
        state.abort_unspawned()?;
        state.snapshot("original_unspawned_abort", 1, 1)?;
        state.abort_unspawned()?; // Real duplicate original Abort request, not a local fake.
        state.snapshot("duplicate_abort_no_second_return", 1, 1)?;
        Ok(())
    })
}

#[test]
fn real_attach_lost_ack_keeps_live_child_charged_until_actual_wait() -> Result<(), String> {
    run(Fault::AfterAttachReply, |state| {
        state.begin_pending()?;
        state.resolve()?;
        let pid = state.spawn_shell()?;
        let attached = state.grant.as_mut().ok_or("grant missing")?.attach(pid);
        state.record("attach_rpc_ack_lost", &format!("{attached:?}"), None)?;
        require(attached.is_err(), "Attach fault did not lose acknowledgement")?;
        require(state.grant.as_ref().and_then(|grant| grant.pid) == Some(pid), "parent lost actual PID")?;
        require(state.child_is_live()?, "owned shell exited before charged-live observation")?;
        state.snapshot("actual_child_live_no_return_on_lost_ack", 1, 0)?;
        state.wait_child(false)?;
        state.return_waited()?;
        state.snapshot("actual_wait_then_original_return", 1, 1)?;
        state.return_waited()?;
        state.snapshot("duplicate_after_wait_no_second_return", 1, 1)?;
        Ok(())
    })
}

fn reap_case(fault: Fault, returned_after_error: u64) -> Result<(), String> {
    run(fault, |state| {
        state.begin_pending()?;
        state.resolve()?;
        let pid = state.spawn_shell()?;
        state.grant.as_mut().ok_or("grant missing")?.attach(pid)?;
        state.snapshot("attached_before_wait", 1, 0)?;
        let actual = state.wait_child(false)?;
        let identity = state.grant.as_ref().ok_or("grant missing")?.protocol_identity();
        let first = state.return_waited();
        state.record("first_reap_rpc", &format!("{first:?}"), None)?;
        require(first.is_err(), "selected Reap fault did not fail")?;
        if returned_after_error == 0 {
            require(first.as_ref().err().is_some_and(|error| error == "normal_pool_injected_before_reap"),
                "before-action fault must be the original explicit rejection")?;
        }
        require(!state.grant.as_ref().ok_or("grant missing")?.returned, "unacknowledged local return")?;
        require(state.reaped == Some(actual), "actual cached exit was lost")?;
        state.snapshot("after_failed_reap_distinguish_mutation", 1, returned_after_error)?;
        state.return_waited()?;
        require(state.grant.as_ref().ok_or("grant missing")?.protocol_identity() == identity,
            "retry replaced original pool/grant")?;
        require(state.reaped == Some(actual), "retry fabricated a different exit")?;
        state.snapshot("same_owner_exit_retry_acknowledged", 1, 1)?;
        state.return_waited()?;
        state.snapshot("duplicate_retry_no_double_credit", 1, 1)?;
        Ok(())
    })
}

#[test]
fn real_reap_lost_reply_keeps_same_exit_and_capability_without_double_credit() -> Result<(), String> {
    reap_case(Fault::AfterReapReply, 1)
}

#[test]
fn real_reap_before_action_rejection_returns_nothing_until_same_evidence_retry() -> Result<(), String> {
    reap_case(Fault::BeforeReapAction, 0)
}

#[test]
fn persistent_worker_prepare_lost_ack_retires_original_pending_before_empty() -> Result<(), String> {
    run(Fault::AfterPrepareReply, |state| {
        state.snapshot("worker_before_prepare", 0, 0)?;
        state.init_worker()?;
        let started = state.start_worker();
        state.record("worker_prepare_ack_lost_start", &format!("{started:?}"), None)?;
        require(started.is_err(), "Worker start did not expose lost Prepare ACK")?;
        let pending = state.worker_state()?;
        require(pending["empty"] == false && pending["pending_prepare"] == true,
            "lost Prepare must retain original nonempty pending Worker")?;
        require(pending["worker_pid"].is_null() && pending["direct_grant"].is_null()
            && pending["actual_exit_cached"] == false, "unspawned Prepare invented a PID/grant/exit")?;
        state.snapshot("worker_unconfirmed_prepare_charged", 1, 0)?;
        let original_id = state.event_id("grant_released_before_spawn")?;
        state.require_repeated_start_rejected()?;
        let retired = state.retire_worker();
        state.record("worker_pending_resolve_abort_retire", &format!("{retired:?}"), None)?;
        require(retired?.is_none(), "unspawned retirement invented an OS exit")?;
        require(state.worker_state()?["empty"] == true, "acknowledged Abort did not empty owner")?;
        require(state.event_id("returned_after_wait_or_abort")? == original_id,
            "retire did not return the original issued grant")?;
        state.snapshot("worker_original_pending_abort_once", 1, 1)?;
        state.require_empty_retire()
    })
}

#[test]
fn persistent_worker_attach_lost_ack_keeps_actual_child_until_wait_and_return() -> Result<(), String> {
    run(Fault::AfterAttachReply, |state| {
        state.init_worker()?;
        let started = state.start_worker();
        state.record("worker_attach_ack_lost_start", &format!("{started:?}"), None)?;
        require(started.is_err(), "Worker start did not expose lost Attach ACK")?;
        let bound = state.worker_state()?;
        let actual_pid = state.worker.as_ref().ok_or("persistent worker missing")?.pid_opt()
            .ok_or("original Worker lost its spawned Child")?;
        require(bound["worker_pid"].as_u64() == Some(u64::from(actual_pid))
            && bound["empty"] == false && bound["direct_grant"].is_array(),
            "attach error lost original Child/grant")?;
        require(bound["actual_exit_cached"] == false, "attach response is not a wait")?;
        state.snapshot("worker_spawned_attach_lost_no_return", 1, 0)?;
        state.require_repeated_start_rejected()?;
        let retired = state.retire_worker();
        state.record("worker_attach_error_actual_wait_retire", &format!("{retired:?}"), None)?;
        require(retired?.is_some(), "spawned retirement omitted actual exit")?;
        require(state.worker_state()?["empty"] == true, "real wait and return did not empty owner")?;
        state.snapshot("worker_attach_error_return_once", 1, 1)?;
        state.require_empty_retire()
    })
}

fn persistent_worker_reap_case(fault: Fault, returns_after_first_error: u64) -> Result<(), String> {
    run(fault, |state| {
        state.init_worker()?;
        state.start_worker()?;
        let bound = state.worker_state()?;
        require(bound["empty"] == false && bound["ready_to_submit"] == true
            && bound["direct_grant"].is_array() && bound["worker_pid"].as_u64().is_some(),
            "real bounded Worker was not started")?;
        state.snapshot("worker_reap_before_retire", 1, 0)?;
        let first = state.retire_worker();
        state.record("worker_first_retire_fault", &format!("{first:?}"), None)?;
        require(first.is_err(), "Worker first retire did not expose selected Reap fault")?;
        if returns_after_first_error == 0 {
            require(first.as_ref().err().is_some_and(|error| error == "normal_pool_injected_before_reap"),
                "BeforeReapAction must be explicit pre-action rejection")?;
        }
        let failed = state.worker_state()?;
        require(failed["actual_exit_cached"] == true && failed["empty"] == false,
            "failed Reap lost the real cached exit or prematurely emptied Worker")?;
        require(failed["direct_grant"] == bound["direct_grant"]
            && failed["worker_pid"] == bound["worker_pid"], "failed retirement replaced original owner")?;
        state.snapshot("worker_failed_retire_actual_server_outcome", 1, returns_after_first_error)?;
        state.require_repeated_start_rejected()?;
        let second = state.retire_worker();
        state.record("worker_same_cached_exit_retry", &format!("{second:?}"), None)?;
        require(second?.is_some(), "same spawned Worker retry must return its actual exit")?;
        require(state.worker_state()?["empty"] == true, "acknowledged retry did not empty owner")?;
        state.snapshot("worker_retry_return_once", 1, 1)?;
        state.require_empty_retire()
    })
}

#[test]
fn persistent_worker_reap_before_action_retains_exit_and_grant_until_same_owner_retry() -> Result<(), String> {
    persistent_worker_reap_case(Fault::BeforeReapAction, 0)
}

#[test]
fn persistent_worker_reap_after_reply_loss_keeps_cached_exit_without_double_return() -> Result<(), String> {
    persistent_worker_reap_case(Fault::AfterReapReply, 1)
}

#[test]
fn persistent_worker_external_owner_survives_unwind_and_reap_retry_before_same_panic_resumes() -> Result<(), String> {
    run(Fault::BeforeReapAction, |state| {
        const PANIC_MARKER: &str = "p102-original-worker-closure-panic";
        let initial = state.snapshot("worker_unwind_before_start", 0, 0)?;
        require(initial.rejected_requests == 0, "panic fixture started with rejected RPCs")?;
        state.init_worker()?;
        let (executable, task) = state.worker_paths.as_ref().ok_or("worker paths missing")?.clone();
        let pool = state.root.clone();
        let mut before_panic_owner = None;
        let mut before_panic_summary = None;
        // The external fixture owns Worker throughout catch/unwind/retirement.
        let worker = state.worker.as_mut().ok_or("persistent worker missing")?;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::compute_worker::with_worker_on(worker, |owner| -> Result<(), String> {
                owner.start_bounded(&executable, &task, Some(&pool), false)?;
                before_panic_owner = Some(owner.retirement_state());
                let summary = status(&pool)?;
                require(summary.grants == 1 && summary.returns == 0
                    && summary.granted_capacity_bytes == EXPECTED_GRANT,
                    "panic closure did not hold the actual charged worker")?;
                before_panic_summary = Some(summary);
                std::panic::panic_any(PANIC_MARKER)
            })
        }));
        state.record("worker_panic_before_unwind_captured",
            &format!("original_before_panic_owner={before_panic_owner:?}"), before_panic_summary)?;
        let same_panic = match caught {
            Err(payload) => payload.downcast_ref::<&str>().copied() == Some(PANIC_MARKER),
            Ok(result) => {
                state.record("worker_unwind_returned_without_original_panic", &format!("{result:?}"), None)?;
                false
            }
        };
        state.record("worker_original_panic_resumed", &format!("same_panic={same_panic}"), None)?;
        require(same_panic, "with_worker_on swallowed/replaced the original closure panic")?;
        let captured = before_panic_owner.ok_or("panic happened before actual Worker started")?;
        require(captured["empty"] == false && captured["worker_pid"].as_u64().is_some(),
            "panic fixture had no original spawned owner")?;
        require(state.worker_state()?["empty"] == true,
            "unwind left owner unconfirmed; retain same external Worker for guard recovery")?;
        let final_summary = state.snapshot("worker_unwind_reap_retry_balanced", 1, 1)?;
        require(final_summary.rejected_requests == 1,
            "one original BeforeReapAction rejection must precede successful unwind retirement")?;
        // This case observes the wrapper's actual retirement, not a synthetic
        // ExitStatus. Empty retire must still return None and add no credit.
        state.require_empty_retire()
    })
}
