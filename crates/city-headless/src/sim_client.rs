//! One in-flight host command, bounded IPC; no Store writer exists in this process.
use crate::{
    compute_worker as wire,
    sim_host::{Command as SimCommand, Event, Profile, Request},
};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process::{ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};
/// Explicit laboratory cuts at the existing partial-startup error boundaries.
/// These do not claim to reproduce OS pipe/thread allocation failure.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StartupFault { MissingStdout, ReaderRejected }

#[must_use = "retain the Client slot until explicit stop confirms retirement"]
pub struct Client {
    owner: crate::normal_pool::broker::StartupOwner,
    last_pid: Option<u32>,
    waited_and_retired: bool,
    worker_lease: Option<u64>,
    input: Option<ChildStdin>,
    receiver: Option<Receiver<Result<Event, String>>>,
    reader: Option<JoinHandle<()>>,
    outstanding: Option<u64>,
    worker_pid: Option<u32>,
    cache_root: PathBuf,
    executable: PathBuf,
}
impl Client {
    /// Empty local slot; no process, broker request or capacity side effect.
    pub fn new() -> Self {
        Self { owner: crate::normal_pool::broker::StartupOwner::new(), last_pid: None, waited_and_retired: false,
            worker_lease: None, input: None, receiver: None, reader: None,
            outstanding: None, worker_pid: None, cache_root: PathBuf::new(), executable: PathBuf::new() }
    }
    pub fn is_empty(&self) -> bool {
        self.owner.is_empty() && self.input.is_none() && self.receiver.is_none() && self.reader.is_none()
    }

    pub fn start(&mut self, executable: &Path, world: &Path, create: bool) -> Result<(), String> {
        self.start_inner(executable, world, create, false, Profile::Base, false, None)
    }
    pub fn start_probe(&mut self, executable: &Path, world: &Path, create: bool) -> Result<(), String> {
        self.start_inner(executable, world, create, true, Profile::Base, false, None)
    }
    pub fn start_joint(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
    ) -> Result<(), String> {
        self.start_inner(
            executable,
            world,
            create,
            probe,
            Profile::Joint,
            false,
            None,
        )
    }
    pub fn start_scale_full(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
    ) -> Result<(), String> {
        self.start_scale(
            executable,
            world,
            create,
            probe,
            crate::scale_reference::Tier::Full,
        )
    }
    pub fn start_scale(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
        tier: crate::scale_reference::Tier,
    ) -> Result<(), String> {
        self.start_inner(
            executable,
            world,
            create,
            probe,
            Profile::Scale(tier),
            false,
            None,
        )
    }
    pub fn start_scale_measured(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
        tier: crate::scale_reference::Tier,
    ) -> Result<(), String> {
        self.start_inner(
            executable,
            world,
            create,
            probe,
            Profile::Scale(tier),
            true,
            None,
        )
    }
    /// Explicit pool avoids process-global environment mutation in real integration tests.
    pub fn start_in_pool(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
        pool: &Path,
    ) -> Result<(), String> {
        self.start_inner(
            executable,
            world,
            create,
            probe,
            Profile::Base,
            false,
            Some(pool),
        )
    }
    fn start_inner(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
        profile: Profile,
        measured: bool,
        pool: Option<&Path>,
    ) -> Result<(), String> {
        self.start_inner_with_fault(executable, world, create, probe, profile, measured, pool, None)
    }
    #[doc(hidden)]
    pub fn start_in_pool_with_fault(&mut self, executable: &Path, world: &Path, create: bool,
        pool: &Path, fault: StartupFault) -> Result<(), String>
    {
        self.start_inner_with_fault(executable, world, create, false, Profile::Base, false, Some(pool), Some(fault))
    }
    fn start_inner_with_fault(
        &mut self,
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
        profile: Profile,
        measured: bool,
        pool: Option<&Path>,
        fault: Option<StartupFault>,
    ) -> Result<(), String> {
        if !self.is_empty() { return Err("sim_slot_not_empty".into()); }
        self.waited_and_retired = false;
        self.last_pid = None;
        if !world.is_absolute() {
            return Err("sim_world_absolute_path_required".into());
        }
        let executable = executable.canonicalize().map_err(|e| e.to_string())?;
        self.executable = executable.clone();
        self.cache_root = world.join("air-worker");
        let configured = pool.map(Path::to_owned).or_else(|| std::env::var_os("MYCITY_NORMAL_POOL").map(PathBuf::from));
        if let Some(root) = configured.as_ref() {
            self.owner.prepare_sim(root)?;
            self.owner.resolve()?;
        }
        let mut command = Command::new(&executable);
        if configured.is_none() {
            command.env_remove("MYCITY_BROKER_SESSION").env_remove("MYCITY_BROKER_SUPERVISOR");
        }
        if measured {
            command.env("MYCITY_DIAGNOSTICS", "1");
        } else {
            command.env_remove("MYCITY_DIAGNOSTICS");
        }
        command.arg(profile.command(probe))
            .arg(if create { "create" } else { "open" }).arg(world)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let spawned = self.owner.spawn(&mut command);
        self.last_pid = self.owner.pid_opt()?;
        spawned?;
        self.input = self.owner.child_mut()?.stdin.take();
        if fault == Some(StartupFault::MissingStdout) { self.owner.child_mut()?.stdout.take(); }
        let stdout = self.owner.child_mut()?.stdout.take().ok_or("sim_stdout")?;
        let (sender, receiver) = mpsc::sync_channel(4);
        self.receiver = Some(receiver);
        if fault == Some(StartupFault::ReaderRejected) { return Err("sim_injected_reader_spawn_rejected".into()); }
        let reader = wire::spawn_ipc_reader("sim-ipc-reader", move || {
            let mut r = BufReader::with_capacity(8192, stdout);
            loop {
                let result = match wire::frame(&mut r) {
                    Ok(Some(b)) => serde_json::from_slice::<Event>(&b).map_err(|e| e.to_string()),
                    Ok(None) => Err("sim_eof".into()),
                    Err(e) => Err(e),
                };
                let failed = result.is_err();
                if sender.send(result).is_err() || failed {
                    break;
                }
            }
        })?;
        self.reader = Some(reader);
        self.outstanding = Some(0);
        Ok(())
    }
    pub fn pid(&self) -> u32 {
        self.last_pid.expect("Sim PID exists only after an actual spawn")
    }
    pub fn submit(&mut self, id: u64, command: SimCommand) -> Result<(), String> {
        if id == 0 || self.outstanding.is_some() {
            return Err("sim_busy_or_id".into());
        }
        wire::write_frame(
            self.input.as_mut().ok_or("sim_stopped")?,
            &Request {
                protocol: "mycity-sim-v1".into(),
                id,
                command,
            },
        )?;
        self.outstanding = Some(id);
        Ok(())
    }
    pub fn next(&mut self, timeout: Duration) -> Result<Event, String> {
        let event = self
            .receiver
            .as_ref()
            .ok_or("sim_stopped")?
            .recv_timeout(timeout)
            .map_err(|e| e.to_string())??;
        if event.protocol != "mycity-sim-v1"
            || event.sim_pid != self.pid()
            || Some(event.id) != self.outstanding
        {
            return Err("sim_event_identity".into());
        }
        match event.kind.as_str() {
            "ready" if event.id == 0 => self.outstanding = None,
            "result" if event.id != 0 => self.outstanding = None,
            "worker_started" if event.id != 0 => {
                let pid = u32::try_from(
                    event.payload["worker_pid"]
                        .as_u64()
                        .ok_or("sim_worker_pid")?,
                )
                .map_err(|_| "sim_worker_pid")?;
                if pid <= 1 || pid == self.pid() || pid == std::process::id() {
                    return Err("sim_worker_pid".into());
                }
                self.worker_pid = Some(pid);
                self.worker_lease = event.payload["broker_lease"].as_u64();
                if self.owner.supervisor().is_some() && self.worker_lease.is_none() {
                    return Err("sim_broker_worker_lease_absent".into());
                }
            }
            "worker_progress" | "attempt_failed" | "probe_barrier" if event.id != 0 => (),
            _ => return Err("sim_event_kind".into()),
        }
        Ok(event)
    }
    pub fn acknowledge_probe(
        &mut self,
        point: crate::sim_host::ProbePoint,
        cancel: bool,
    ) -> Result<(), String> {
        let id = self
            .outstanding
            .filter(|id| *id != 0)
            .ok_or("sim_not_waiting")?;
        wire::write_frame(
            self.input.as_mut().ok_or("sim_stopped")?,
            &Request {
                protocol: "mycity-sim-v1".into(),
                id,
                command: SimCommand::ProbeAck { point, cancel },
            },
        )
    }
    pub fn worker_pid(&self) -> Option<u32> {
        self.worker_pid
    }
    /// Verify the known role/path/parent before signalling. No guessed process IDs.
    pub fn kill_owned_worker(&mut self) -> Result<(), String> {
        let Some(pid) = self.worker_pid else {
            return Ok(());
        };
        if let Some(s) = self.owner.supervisor() {
            s.kill_worker(
                pid,
                self.worker_lease.ok_or("sim_broker_worker_lease_absent")?,
            )?;
            self.worker_pid = None;
            self.worker_lease = None;
            return Ok(());
        }
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "ppid=,stat=,command="])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            self.worker_pid = None;
            return Ok(());
        }
        if output.stdout.len() > wire::CONTROL_BYTES {
            return Err("sim_ps_bytes".into());
        }
        let info = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
        let mut parts = info.split_whitespace();
        let parent = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or("sim_worker_parent")?;
        let state = parts.next().ok_or("sim_worker_state")?;
        if state.starts_with('Z') {
            self.worker_pid = None;
            return Ok(());
        }
        if (parent != self.pid() && parent != 1)
            || !info.contains(self.executable.to_str().ok_or("sim_executable_utf8")?)
            || !info.contains("compute-worker-bounded")
            || !info
                .trim_end()
                .ends_with(self.cache_root.to_str().ok_or("sim_cache_utf8")?)
        {
            return Err("sim_worker_identity_changed".into());
        }
        if !Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .map_err(|e| e.to_string())?
            .success()
        {
            return Err("sim_worker_kill".into());
        }
        self.worker_pid = None;
        Ok(())
    }
    /// Reap only Sim after an external signal; deliberately leaves worker recovery
    /// to a new supervisor/Sim and its persistent registry + OS fence.
    pub fn wait_for_sim_exit(&mut self, timeout: Duration) -> Result<(), String> {
        if self.waited_and_retired { return Ok(()); }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.owner.try_wait()?.is_some() { self.after_sim_wait()?; return Ok(()); }
            if std::time::Instant::now() >= deadline { return Err("sim_exit_timeout".into()); }
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn after_sim_wait(&mut self) -> Result<(), String> {
        if self.owner.supervisor().is_some() {
            self.owner.retire()?;
            self.worker_pid = None;
            self.worker_lease = None;
        } else {
            self.kill_owned_worker()?;
            self.owner.retire()?;
        }
        self.waited_and_retired = true;
        Ok(())
    }
    pub fn kill_sim(&mut self) -> Result<(), String> {
        if self.waited_and_retired { return self.stop(); }
        self.owner.kill_wait()?;
        self.after_sim_wait()?;
        self.stop()
    }
    pub fn stop(&mut self) -> Result<(), String> {
        self.input.take();
        if self.owner.pid_opt()?.is_some() {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if self.owner.try_wait()?.is_some() { break; }
                if std::time::Instant::now() >= deadline { self.owner.kill_wait()?; break; }
                thread::sleep(Duration::from_millis(10));
            }
            self.after_sim_wait()?;
        } else { self.owner.retire()?; }
        self.receiver.take();
        self.outstanding = None;
        if let Some(reader) = self.reader.take() { reader.join().map_err(|_| "sim_reader_panic")?; }
        Ok(())
    }

}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// The slot outlives every fallible/panic operation in the closure. This is a CLI
/// terminal boundary: persistent retirement failure is reported and exits(2).
pub fn with_client_scope<T>(body: impl FnOnce(&mut Client) -> Result<T, String>) -> Result<T, String> {
    crate::normal_pool::supervised_scope(Client::new(), body, Client::stop, "sim-client", |client| client.owner.observation())
}

impl Default for Client { fn default() -> Self { Self::new() } }
