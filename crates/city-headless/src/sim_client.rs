//! One in-flight host command, bounded IPC; no Store writer exists in this process.
use crate::{
    compute_worker as wire,
    sim_host::{Command as SimCommand, Event, Request},
};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};
pub struct Client {
    child: Child,
    input: Option<ChildStdin>,
    receiver: Option<Receiver<Result<Event, String>>>,
    reader: Option<JoinHandle<()>>,
    outstanding: Option<u64>,
    worker_pid: Option<u32>,
    cache_root: PathBuf,
    executable: PathBuf,
}
impl Client {
    pub fn spawn(executable: &Path, world: &Path, create: bool) -> Result<Self, String> {
        Self::spawn_inner(executable, world, create, false, false)
    }
    pub fn spawn_probe(executable: &Path, world: &Path, create: bool) -> Result<Self, String> {
        Self::spawn_inner(executable, world, create, true, false)
    }
    pub fn spawn_joint(
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
    ) -> Result<Self, String> {
        Self::spawn_inner(executable, world, create, probe, true)
    }
    fn spawn_inner(
        executable: &Path,
        world: &Path,
        create: bool,
        probe: bool,
        joint: bool,
    ) -> Result<Self, String> {
        if !world.is_absolute() {
            return Err("sim_world_absolute_path_required".into());
        }
        let executable = executable.canonicalize().map_err(|e| e.to_string())?;
        let mut child = Command::new(&executable)
            .arg(match (joint, probe) {
                (false, false) => "sim-host",
                (false, true) => "sim-host-probe",
                (true, false) => "sim-host-joint",
                (true, true) => "sim-host-joint-probe",
            })
            .arg(if create { "create" } else { "open" })
            .arg(world)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().ok_or("sim_stdout")?;
        let (sender, receiver) = mpsc::sync_channel(4);
        let reader = thread::spawn(move || {
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
        });
        Ok(Self {
            child,
            input,
            receiver: Some(receiver),
            reader: Some(reader),
            outstanding: Some(0),
            worker_pid: None,
            cache_root: world.join("air-worker"),
            executable,
        })
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
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
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err("sim_exit_timeout".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn kill_sim(&mut self) -> Result<(), String> {
        self.child.kill().map_err(|e| e.to_string())?;
        self.child.wait().map_err(|e| e.to_string())?;
        self.kill_owned_worker()?;
        self.stop()
    }
    pub fn stop(&mut self) -> Result<(), String> {
        self.input.take();
        // Caller normally sends Stop and drains its reply first. This is a bounded reap.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                self.child.kill().map_err(|e| e.to_string())?;
                self.child.wait().map_err(|e| e.to_string())?;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        self.kill_owned_worker()?;
        self.receiver.take();
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "sim_reader_panic")?;
        }
        Ok(())
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
