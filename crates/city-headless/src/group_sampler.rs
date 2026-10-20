//! Bounded ps-snapshot accounting of the host's entire observed process tree.
use mycity_core::read_bounded;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const LOG_BYTES: u64 = 64 * 1024 * 1024;
const ROWS: usize = 8192;
#[derive(Clone)]
struct Known {
    role: String,
    birth: Option<String>,
    parent: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    pub rss_bytes: u64,
    pub state: String,
    pub birth: String,
    pub role: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Stats {
    pub snapshots: u64,
    pub peak_sum_rss_bytes: u64,
    pub peak_at_seconds: f64,
    pub peak_processes: Vec<Process>,
    pub peak_disk_bytes: u64,
    pub peak_experiment_bytes: u64,
    pub maximum_group_processes: usize,
    pub log_bytes: u64,
    pub last_group: Vec<Process>,
    pub observed_pids: Vec<u32>,
    pub unobserved_declared_pids: Vec<u32>,
    pub elapsed_seconds: f64,
    pub maximum_gap_seconds: f64,
}
pub type Registry = Arc<Mutex<BTreeMap<u32, (String, Option<u32>)>>>;
pub type Observed = Arc<Mutex<BTreeSet<(u32, String)>>>;
pub fn wait_observed(observed: &Observed, pid: u32, role: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if observed
            .lock()
            .map_err(|_| "sampler_observed_lock")?
            .contains(&(pid, role.into()))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("sampler_role_not_observed".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}
pub fn declare(
    registry: &Registry,
    pid: u32,
    role: &str,
    parent: Option<u32>,
) -> Result<(), String> {
    let mut r = registry.lock().map_err(|_| "sampler_registry_lock")?;
    if r.len() >= 2048 && !r.contains_key(&pid) {
        return Err("sampler_registry_limit".into());
    }
    r.insert(pid, (role.into(), parent));
    Ok(())
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn disk(dir: &Path) -> Result<(u64, usize), String> {
    if !dir.exists() {
        return Ok((0, 0));
    }
    let mut todo = vec![dir.to_path_buf()];
    let mut bytes = 0u64;
    let mut files = 0;
    while let Some(path) = todo.pop() {
        for e in fs::read_dir(path).map_err(err)? {
            let e = e.map_err(err)?;
            let m = match e.metadata() {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(err(e)),
            };
            if e.file_type().map_err(err)?.is_symlink() {
                return Err("sampler_disk_symlink".into());
            }
            files += 1;
            if files > 2048 {
                return Err("sampler_disk_files".into());
            }
            if m.is_dir() {
                todo.push(e.path());
            } else if m.is_file() {
                bytes = bytes.checked_add(m.len()).ok_or("sampler_disk_overflow")?;
            }
        }
    }
    Ok((bytes, files))
}
fn ps() -> Result<(u32, Vec<Process>), String> {
    let mut child = Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss=,stat=,lstart="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(err)?;
    let id = child.id();
    let raw = match read_bounded(
        child.stdout.take().ok_or("sampler_ps_stdout")?,
        4 * 1024 * 1024,
    ) {
        Ok(b) => b,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err(e));
        }
    };
    if !child.wait().map_err(err)?.success() {
        return Err("sampler_ps_exit".into());
    }
    let text = String::from_utf8(raw).map_err(err)?;
    let mut rows = Vec::new();
    for line in text.lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        if words.len() != 9 || rows.len() >= ROWS {
            return Err("sampler_ps_shape".into());
        }
        rows.push(Process {
            pid: words[0].parse().map_err(err)?,
            ppid: words[1].parse().map_err(err)?,
            rss_bytes: words[2]
                .parse::<u64>()
                .map_err(err)?
                .checked_mul(1024)
                .ok_or("sampler_rss_overflow")?,
            state: words[3].into(),
            birth: words[4..].join(" "),
            role: String::new(),
        });
    }
    Ok((id, rows))
}
struct Sampling {
    registry: Registry,
    observed: Observed,
    known: BTreeMap<u32, Known>,
    root: u32,
    world: PathBuf,
    started: Instant,
    file: File,
    stats: Stats,
    last: f64,
}
impl Sampling {
    fn sample(&mut self) -> Result<(), String> {
        let (ps_pid, rows) = ps()?;
        let at = self.started.elapsed().as_secs_f64();
        let declarations = self
            .registry
            .lock()
            .map_err(|_| "sampler_registry_lock")?
            .clone();
        for (pid, (role, parent)) in declarations {
            let entry = self.known.entry(pid).or_insert(Known {
                role: role.clone(),
                birth: None,
                parent,
            });
            entry.role = role;
            entry.parent = parent;
        }
        let mut members = BTreeSet::new();
        members.insert(self.root);
        for row in &rows {
            if self.known.get(&row.pid).is_some_and(|k| {
                k.birth.as_ref().map_or(
                    k.parent.is_none() || k.parent == Some(row.ppid) || row.ppid == 1,
                    |b| b == &row.birth,
                )
            }) {
                members.insert(row.pid);
            }
        }
        for _ in 0..32 {
            let before = members.len();
            for row in &rows {
                if members.contains(&row.ppid) {
                    members.insert(row.pid);
                }
            }
            if before == members.len() {
                break;
            }
        }
        let mut group = Vec::new();
        for mut row in rows.into_iter().filter(|r| members.contains(&r.pid)) {
            let k = self.known.entry(row.pid).or_insert(Known {
                role: if row.pid == ps_pid {
                    "sampler_ps".into()
                } else {
                    "observed_descendant".into()
                },
                birth: None,
                parent: Some(row.ppid),
            });
            if k.birth.as_ref().is_some_and(|b| b != &row.birth) {
                if !members.contains(&row.ppid) {
                    continue;
                }
                k.role = if row.pid == ps_pid {
                    "sampler_ps".into()
                } else {
                    "observed_descendant".into()
                };
                k.parent = Some(row.ppid);
            }
            k.birth = Some(row.birth.clone());
            row.role = k.role.clone();
            group.push(row);
        }
        if self.known.len() > 12000 {
            return Err("sampler_identity_limit".into());
        }
        let sum = group.iter().try_fold(0u64, |n, p| {
            n.checked_add(p.rss_bytes).ok_or("sampler_sum_overflow")
        })?;
        let (disk_bytes, disk_files) = disk(&self.world)?;
        let (experiment_bytes, _) = disk(self.world.parent().ok_or("sampler_experiment_root")?)?;
        let record = json!({"schema":"mycity-group-sample-v1","elapsed_seconds":at,"unix_ms":SystemTime::now().duration_since(UNIX_EPOCH).map_err(err)?.as_millis(),"sum_rss_bytes":sum,"processes":group,"world_actual_file_bytes":disk_bytes,"world_entries":disk_files,"experiment_actual_file_bytes":experiment_bytes});
        let bytes = serde_json::to_vec(&record).map_err(err)?;
        if bytes.len() > 128 * 1024
            || self.stats.snapshots >= 12000
            || self.stats.log_bytes + bytes.len() as u64 + 1 > LOG_BYTES
        {
            return Err("sampler_log_limit".into());
        }
        self.file
            .write_all(&bytes)
            .and_then(|_| self.file.write_all(b"\n"))
            .map_err(err)?;
        self.stats.log_bytes += bytes.len() as u64 + 1;
        self.stats.snapshots += 1;
        self.stats.maximum_gap_seconds = self.stats.maximum_gap_seconds.max(at - self.last);
        self.last = at;
        if sum > self.stats.peak_sum_rss_bytes {
            self.stats.peak_sum_rss_bytes = sum;
            self.stats.peak_at_seconds = at;
            self.stats.peak_processes = group.clone();
        }
        self.stats.peak_disk_bytes = self.stats.peak_disk_bytes.max(disk_bytes);
        self.stats.peak_experiment_bytes = self.stats.peak_experiment_bytes.max(experiment_bytes);
        self.stats.maximum_group_processes = self.stats.maximum_group_processes.max(group.len());
        for p in &group {
            self.observed
                .lock()
                .map_err(|_| "sampler_observed_lock")?
                .insert((p.pid, p.role.clone()));
        }
        self.stats.last_group = group;
        self.stats.elapsed_seconds = at;
        Ok(())
    }
}
pub struct Sampler {
    pub registry: Registry,
    pub observed: Observed,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Stats, String>>>,
}
impl Sampler {
    pub fn start(path: &Path, world: &Path) -> Result<Self, String> {
        let registry = Arc::new(Mutex::new(BTreeMap::new()));
        declare(
            &registry,
            std::process::id(),
            "host_cpu_reference_and_sampler",
            None,
        )?;
        let stop = Arc::new(AtomicBool::new(false));
        let observed = Arc::new(Mutex::new(BTreeSet::new()));
        let mut s = Sampling {
            registry: registry.clone(),
            observed: observed.clone(),
            known: BTreeMap::new(),
            root: std::process::id(),
            world: world.to_path_buf(),
            started: Instant::now(),
            file: File::create(path).map_err(err)?,
            stats: Stats::default(),
            last: 0.0,
        };
        // First sample precedes any Sim/worker spawn.
        s.sample()?;
        let flag = stop.clone();
        let handle = thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(250));
                s.sample()?;
                if flag.load(Ordering::SeqCst) {
                    break;
                }
            }
            s.file.sync_all().map_err(err)?;
            s.stats.observed_pids = s
                .known
                .iter()
                .filter_map(|(pid, k)| k.birth.is_some().then_some(*pid))
                .collect();
            s.stats.unobserved_declared_pids = s
                .registry
                .lock()
                .map_err(|_| "sampler_registry_lock")?
                .keys()
                .filter(|pid| s.known.get(pid).is_none_or(|k| k.birth.is_none()))
                .copied()
                .collect();
            Ok(s.stats)
        });
        Ok(Self {
            registry,
            observed,
            stop,
            handle: Some(handle),
        })
    }
    pub fn finish(&mut self) -> Result<Stats, String> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .take()
            .ok_or("sampler_finished")?
            .join()
            .map_err(|_| "sampler_thread_panic")?
    }
}
impl Drop for Sampler {
    fn drop(&mut self) {
        if self.handle.is_some() {
            let _ = self.finish();
        }
    }
}
pub fn system_snapshot() -> Result<Value, String> {
    let mut values = serde_json::Map::new();
    for (key, program, args) in [
        ("physical_memory_bytes", "sysctl", vec!["-n", "hw.memsize"]),
        ("swap", "sysctl", vec!["vm.swapusage"]),
        ("vm_stat", "vm_stat", vec![]),
    ] {
        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(err)?;
        let raw =
            read_bounded(child.stdout.take().ok_or("system_stdout")?, 16 * 1024).map_err(err)?;
        if !child.wait().map_err(err)?.success() {
            return Err("system_snapshot_exit".into());
        }
        values.insert(
            key.into(),
            Value::String(String::from_utf8(raw).map_err(err)?),
        );
    }
    Ok(Value::Object(values))
}
