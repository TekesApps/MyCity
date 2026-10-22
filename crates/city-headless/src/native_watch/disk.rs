use super::err;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::{fs, path::Path};
pub(super) fn hash(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(err)?;
    let mut bytes = [0; 65536];
    let mut h = Sha256::new();
    loop {
        let n = f.read(&mut bytes).map_err(err)?;
        if n == 0 {
            break;
        }
        h.update(&bytes[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
#[derive(Clone, Copy)]
pub(super) struct Disk {
    pub total: u64,
    pub latency: u64,
}
pub(super) fn scan(root: &Path) -> Result<Disk, String> {
    let mut stack = Vec::with_capacity(64);
    stack.push(fs::read_dir(root).map_err(err)?);
    let mut total = 0u64;
    let mut latency = 0u64;
    let mut files = 0;
    while let Some(iter) = stack.last_mut() {
        let entry = match iter.next() {
            Some(Ok(e)) => e,
            Some(Err(e)) => return Err(err(e)),
            None => {
                stack.pop();
                continue;
            }
        };
        let kind = entry.file_type().map_err(err)?;
        if kind.is_symlink() {
            return Err("native_observer_artifact_symlink".into());
        }
        if kind.is_dir() {
            if stack.len() == 64 {
                return Err("native_observer_disk_depth".into());
            }
            match fs::read_dir(entry.path()) {
                Ok(next) => stack.push(next),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(err(e)),
            }
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(err(e)),
        };
        files += 1;
        if files > 1_000_000 {
            return Err("native_observer_disk_file_capacity".into());
        }
        total = total
            .checked_add(meta.len())
            .ok_or("native_observer_disk_overflow")?;
        if entry.file_name() == "compute-durations.bin" {
            latency += meta.len();
        }
    }
    Ok(Disk { total, latency })
}
/// Stack-bound per-line serialization; logs cannot grow an in-memory DOM.
pub(super) struct Line {
    bytes: Vec<u8>,
    used: usize,
}
impl Line {
    pub fn new() -> Self {
        Self {
            bytes: vec![0; 256 * 1024],
            used: 0,
        }
    }
    pub fn encode(&mut self, value: &impl serde::Serialize) -> Result<&[u8], String> {
        self.used = 0;
        serde_json::to_writer(&mut *self, value).map_err(err)?;
        self.write_all(b"\n").map_err(err)?;
        Ok(&self.bytes[..self.used])
    }
}
impl Write for Line {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.bytes.len() - self.used {
            return Err(std::io::Error::other("native_observer_frame_capacity"));
        }
        self.bytes[self.used..self.used + bytes.len()].copy_from_slice(bytes);
        self.used += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
