//! Disposable cache for the one-worker Sim host. Never contains authoritative assets.
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};
pub const CACHE_BYTES: u64 = 6 * 1024 * 1024;
pub const ASSET_BYTES: usize = mycity_core::background::ASSET_LIMIT;
fn fail(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn owned(name: &str) -> bool {
    name.strip_suffix(".air").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) || name
        .strip_prefix(".pending-")
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}
fn entries(root: &Path) -> Result<Vec<(PathBuf, u64)>, String> {
    if !fs::symlink_metadata(root).map_err(fail)?.is_dir() {
        return Err("cache_root".into());
    }
    let mut result = Vec::new();
    for (n, dir) in fs::read_dir(root).map_err(fail)?.enumerate() {
        if n >= 2 {
            return Err("cache_directory_count".into());
        }
        let dir = dir.map_err(fail)?;
        if !matches!(dir.file_name().to_str(), Some("inputs" | "candidates"))
            || !dir.file_type().map_err(fail)?.is_dir()
        {
            return Err("cache_directory".into());
        }
        for (n, entry) in fs::read_dir(dir.path()).map_err(fail)?.enumerate() {
            if n >= 8 {
                return Err("cache_file_count".into());
            }
            let entry = entry.map_err(fail)?;
            if !entry.file_type().map_err(fail)?.is_file()
                || !entry.file_name().to_str().is_some_and(owned)
            {
                return Err("cache_asset".into());
            }
            result.push((entry.path(), entry.metadata().map_err(fail)?.len()));
        }
    }
    Ok(result)
}
pub fn actual_bytes(root: &Path) -> Result<u64, String> {
    let n = entries(root)?.iter().try_fold(0u64, |sum, (_, len)| {
        sum.checked_add(*len).ok_or("cache_overflow")
    })?;
    if n > CACHE_BYTES {
        return Err("cache_bytes".into());
    }
    Ok(n)
}
/// Caller must first stop/reap the old worker (or confirm it is idle after Complete).
/// Run before EVERY attempt, including retries of the same job after a crash.
/// Preflight all owned names before deleting anything; unknown files are never removed.
pub fn reset_after_worker_stopped(root: &Path) -> Result<(), String> {
    if !root.exists() {
        fs::create_dir(root).map_err(fail)?;
    }
    let files = entries(root)?;
    for (path, _) in files {
        fs::remove_file(path).map_err(fail)?;
    }
    for name in ["inputs", "candidates"] {
        let dir = root.join(name);
        if !dir.exists() {
            fs::create_dir(&dir).map_err(fail)?;
        }
        File::open(dir).and_then(|f| f.sync_all()).map_err(fail)?;
    }
    File::open(root).and_then(|f| f.sync_all()).map_err(fail)?;
    Ok(())
}
pub fn publish(root: &Path, directory: &str, bytes: &[u8]) -> Result<String, String> {
    if !matches!(directory, "inputs" | "candidates") || bytes.len() > ASSET_BYTES {
        return Err("cache_publish_asset".into());
    }
    let hash = mycity_core::sha256(bytes);
    let dir = root.join(directory);
    let existing = dir.join(format!("{hash}.air")).exists();
    let extra = if existing { 0 } else { 2 * bytes.len() as u64 };
    if actual_bytes(root)?.saturating_add(extra) > CACHE_BYTES {
        return Err("cache_publish_peak".into());
    }
    crate::compute_worker::publish(&dir, bytes)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    pid: u32,
    sim: u32,
    executable: String,
    cache: String,
}
fn owner_path(root: &Path) -> Result<PathBuf, String> {
    Ok(root
        .parent()
        .ok_or("cache_parent")?
        .join("air-worker-owner.json"))
}
/// OS-held fence shared by Sim cache reset and every bounded worker request.
/// The lock is outside the disposable directory and is never removed/replaced.
pub fn exclusive(root: &Path) -> Result<File, String> {
    use fs2::FileExt;
    let path = root.parent().ok_or("cache_parent")?.join("air-worker.lock");
    if path.exists() && !fs::symlink_metadata(&path).map_err(fail)?.is_file() {
        return Err("cache_lock_file".into());
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(fail)?;
    file.try_lock_exclusive().map_err(|_| "cache_worker_busy")?;
    Ok(file)
}
/// Publish PID provenance BEFORE sending a request. An unregistered child receives no work.
pub fn register(root: &Path, pid: u32, executable: &Path) -> Result<(), String> {
    use std::io::Write;
    let path = owner_path(root)?;
    let tmp = path.with_extension(format!("pending-{}", std::process::id()));
    let bytes = serde_json::to_vec(&Owner {
        pid,
        sim: std::process::id(),
        executable: executable.to_str().ok_or("cache_executable_utf8")?.into(),
        cache: root.to_str().ok_or("cache_root_utf8")?.into(),
    })
    .map_err(fail)?;
    if bytes.len() > 4096 {
        return Err("cache_owner_bytes".into());
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)
        .map_err(fail)?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(fail)?;
    fs::rename(tmp, path).map_err(fail)?;
    File::open(root.parent().ok_or("cache_parent")?)
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    Ok(())
}
/// Uses durable provenance, so a NEW supervisor can recover without old in-memory PIDs.
pub fn stop_registered(root: &Path, executable: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let path = owner_path(root)?;
    if !path.exists() {
        return Ok(());
    }
    if !fs::symlink_metadata(&path).map_err(fail)?.is_file() {
        return Err("cache_owner_file".into());
    }
    let bytes = mycity_core::read_bounded(File::open(path).map_err(fail)?, 4096).map_err(fail)?;
    let owner: Owner = serde_json::from_slice(&bytes).map_err(fail)?;
    if owner.pid <= 1
        || owner.pid == std::process::id()
        || owner.executable != executable.to_str().ok_or("cache_executable_utf8")?
        || owner.cache != root.to_str().ok_or("cache_root_utf8")?
    {
        return Err("cache_owner_identity".into());
    }
    let mut ps = Command::new("ps")
        .args(["-p", &owner.pid.to_string(), "-o", "ppid=,stat=,command="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(fail)?;
    let output = mycity_core::read_bounded(ps.stdout.take().ok_or("cache_ps_stdout")?, 64 * 1024)
        .map_err(fail)?;
    if !ps.wait().map_err(fail)?.success() {
        return Ok(());
    }
    let info = String::from_utf8(output).map_err(fail)?;
    let mut parts = info.split_whitespace();
    let parent = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or("cache_worker_parent")?;
    let state = parts.next().ok_or("cache_worker_state")?;
    if state.starts_with('Z') {
        return Ok(());
    }
    // A reused PID with different provenance is never signalled. The OS fence below
    // independently blocks reuse if an actual old writer still holds it.
    if (parent != owner.sim && parent != 1)
        || !info.contains(&owner.executable)
        || !info.contains("compute-worker-bounded")
        || !info.trim_end().ends_with(&owner.cache)
    {
        return Ok(());
    }
    if !Command::new("kill")
        .args(["-KILL", &owner.pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map_err(fail)?
        .success()
    {
        return Err("cache_stop_worker".into());
    }
    Ok(())
}
pub fn recover_exclusive(root: &Path, executable: &Path) -> Result<File, String> {
    stop_registered(root, executable)?;
    // SIGKILL delivery is asynchronous. Never reset until the OS lock is released.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match exclusive(root) {
            Ok(file) => return Ok(file),
            Err(error) if error == "cache_worker_busy" && std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Err(error) => return Err(error),
        }
    }
}
