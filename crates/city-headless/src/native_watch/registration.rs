//! Optional observational lifecycle journal; never city authority or memory credit.
use super::err;
use crate::{
    control_data::Text,
    native_observer::{capture, Sample},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    Started,
    Finished,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub schema: Text<40>,
    pub nonce: Text<32>,
    pub source: Text<64>,
    pub kind: Kind,
    pub role: Text<48>,
    pub sample: Sample,
}
struct Buffer {
    data: [u8; 1024],
    used: usize,
}
impl Write for Buffer {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if b.len() > self.data.len() - self.used {
            return Err(std::io::Error::other("native_registration_capacity"));
        }
        self.data[self.used..self.used + b.len()].copy_from_slice(b);
        self.used += b.len();
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn publish(finished: bool) -> Result<(), String> {
    let Some(path) = std::env::var_os("MYCITY_PROCESS_LOG") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() {
        return Err("native_registration_absolute_path".into());
    }
    let nonce = std::env::var("MYCITY_PROCESS_NONCE").map_err(err)?;
    if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("native_registration_nonce".into());
    }
    let mut args = std::env::args().skip(1);
    let first = args.next().unwrap_or_default();
    let role = if first == "native-observed-root" {
        args.next().unwrap_or_default()
    } else {
        first
    };
    let role: String = role.chars().take(48).collect();
    let record = Record {
        schema: Text::new("mycity-observed-process-v1")?,
        nonce: Text::new(&nonce)?,
        source: Text::new(env!("MYCITY_SOURCE_SHA256"))?,
        kind: if finished {
            Kind::Finished
        } else {
            Kind::Started
        },
        role: Text::new(&role)?,
        sample: capture(std::process::id(), None, Instant::now())
            .map_err(|e| format!("native_registration_capture:{e:?}"))?,
    };
    let mut buffer = Buffer {
        data: [0; 1024],
        used: 0,
    };
    serde_json::to_writer(&mut buffer, &record).map_err(err)?;
    buffer.write_all(b"\n").map_err(err)?;
    let mut file = OpenOptions::new()
        .append(true)
        .read(true)
        .open(path)
        .map_err(err)?;
    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("native_registration_lock_timeout".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(err(e)),
        }
    }
    if file.metadata().map_err(err)?.len() + buffer.used as u64 > 16 * 1024 * 1024 {
        return Err("native_registration_log_capacity".into());
    }
    file.write_all(&buffer.data[..buffer.used]).map_err(err)?;
    file.sync_data().map_err(err)
}
pub(super) struct Journal {
    file: File,
    offset: u64,
    bytes: Vec<u8>,
    pub records: u64,
    pub busy: bool,
}
impl Journal {
    pub fn new(path: &Path) -> Result<Self, String> {
        Ok(Self {
            file: OpenOptions::new().read(true).open(path).map_err(err)?,
            offset: 0,
            bytes: vec![0; 64 * 1024],
            records: 0,
            busy: false,
        })
    }
    pub fn pending(&self) -> Result<u64, String> {
        Ok(self
            .file
            .metadata()
            .map_err(err)?
            .len()
            .saturating_sub(self.offset))
    }
    pub fn read(
        &mut self,
        nonce: &str,
        mut accept: impl FnMut(Record) -> Result<(), String>,
    ) -> Result<(), String> {
        self.busy = false;
        match fs2::FileExt::try_lock_shared(&self.file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                self.busy = true;
                return Ok(());
            }
            Err(e) => return Err(err(e)),
        }
        let outcome = (|| {
            let len = self.file.metadata().map_err(err)?.len();
            if len < self.offset || len > 16 * 1024 * 1024 {
                return Err("native_registration_journal_length".into());
            }
            self.file.seek(SeekFrom::Start(self.offset)).map_err(err)?;
            loop {
                let n = self.file.read(&mut self.bytes).map_err(err)?;
                if n == 0 {
                    break;
                }
                let mut used = 0;
                for line in self.bytes[..n].split_inclusive(|b| *b == b'\n') {
                    if line.last() != Some(&b'\n') {
                        break;
                    }
                    if line.len() > 1024 {
                        return Err("native_registration_record_capacity".into());
                    }
                    let r: Record = serde_json::from_slice(line).map_err(err)?;
                    if r.schema.as_str() != "mycity-observed-process-v1"
                        || r.source.as_str() != env!("MYCITY_SOURCE_SHA256")
                        || r.nonce.as_str() != nonce
                    {
                        return Err("native_registration_identity".into());
                    }
                    accept(r)?;
                    self.records += 1;
                    used += line.len();
                }
                self.offset += used as u64;
                if used < n {
                    if n - used >= 1024 {
                        return Err("native_registration_partial_capacity".into());
                    }
                    break;
                }
            }
            Ok(())
        })();
        fs2::FileExt::unlock(&self.file).map_err(err)?;
        outcome
    }
}
