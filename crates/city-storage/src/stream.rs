//! Bounded two-pass immutable output and complete-file verification.
//! Buffers are explicit; this does not authorize the whole model allocation pool.
use super::*;
use sha2::{Digest, Sha256};
const PAGE: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Identity {
    pub bytes: u64,
    pub sha256: String,
}
struct Output<W> {
    inner: W,
    page: [u8; PAGE],
    used: usize,
    bytes: u64,
    limit: u64,
    hash: Sha256,
}
impl<W: Write> Output<W> {
    fn new(inner: W, limit: u64) -> Self {
        Self {
            inner,
            page: [0; PAGE],
            used: 0,
            bytes: 0,
            limit,
            hash: Sha256::new(),
        }
    }
    fn drain(&mut self) -> io::Result<()> {
        self.inner.write_all(&self.page[..self.used])?;
        self.hash.update(&self.page[..self.used]);
        self.used = 0;
        Ok(())
    }
    fn finish(mut self) -> Result<Identity, Error> {
        self.flush()?;
        Ok(Identity {
            bytes: self.bytes,
            sha256: hex(&self.hash.finalize().into()),
        })
    }
}
impl<W: Write> Write for Output<W> {
    fn write(&mut self, mut data: &[u8]) -> io::Result<usize> {
        let size = data.len();
        let end = self
            .bytes
            .checked_add(size as u64)
            .filter(|end| *end <= self.limit)
            .ok_or_else(|| io::Error::other("stream_output_limit"))?;
        while !data.is_empty() {
            let n = data.len().min(PAGE - self.used);
            self.page[self.used..self.used + n].copy_from_slice(&data[..n]);
            self.used += n;
            data = &data[n..];
            if self.used == PAGE {
                self.drain()?;
            }
        }
        self.bytes = end;
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.drain()?;
        self.inner.flush()
    }
}
pub(super) fn measure(
    limit: u64,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), Error>,
) -> Result<Identity, Error> {
    let mut out = Output::new(io::sink(), limit);
    encode(&mut out)?;
    out.finish()
}
struct Input {
    file: File,
    page: [u8; PAGE],
    pos: usize,
    end: usize,
    bytes: u64,
    limit: u64,
    hash: Sha256,
}
impl Read for Input {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        if self.pos == self.end {
            let limit = self
                .limit
                .saturating_sub(self.bytes)
                .saturating_add(1)
                .min(PAGE as u64) as usize;
            let n = self.file.read(&mut self.page[..limit])?;
            self.bytes += n as u64;
            if self.bytes > self.limit {
                return Err(io::Error::other("stream_input_limit"));
            }
            self.hash.update(&self.page[..n]);
            self.pos = 0;
            self.end = n;
        }
        let n = out.len().min(self.end - self.pos);
        out[..n].copy_from_slice(&self.page[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}
/// Return the temporary decoded value only after strict EOF, length and hash.
pub(super) fn verified<T>(
    path: &Path,
    expected: &Identity,
    limit: u64,
    decode: impl FnOnce(&mut dyn Read) -> Result<T, Error>,
) -> Result<T, Error> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() != expected.bytes || meta.len() > limit {
        return Err(Error::Invalid("stream_input_shape"));
    }
    let mut input = Input {
        file: File::open(path)?,
        page: [0; PAGE],
        pos: 0,
        end: 0,
        bytes: 0,
        limit,
        hash: Sha256::new(),
    };
    let value = decode(&mut input)?;
    // A decoder which stops at a valid prefix cannot publish a model.
    if input.pos != input.end
        || input.read(&mut [0; 1])? != 0
        || input.bytes != expected.bytes
        || hex(&input.hash.finalize().into()) != expected.sha256
    {
        return Err(Error::Invalid("stream_input_checksum_or_tail"));
    }
    Ok(value)
}
/// Full historical-byte validation without materializing an Engine or encoded Vec.
pub(super) fn verify_bytes(path: &Path, expected: &Identity, limit: u64) -> Result<(), Error> {
    verified(path, expected, limit, |reader| {
        io::copy(reader, &mut io::sink())?;
        Ok(())
    })
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
/// Both passes must use the same immutable source. Compare actual second-pass
/// bytes/hash, fsync, validate the real tmp, then rename and sync the directory.
pub(super) fn atomic(
    dir: &Path,
    name: &str,
    expected: &Identity,
    limit: u64,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), Error>,
    validate: impl FnOnce(&Path) -> Result<(), Error>,
) -> Result<(), Error> {
    let path = dir.join(format!(
        ".{name}-{}-{}.tmp",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    let _cleanup = Temporary(path.clone());
    let mut writer = Output::new(&mut file, limit);
    encode(&mut writer)?;
    if writer.finish()? != *expected {
        return Err(Error::Invalid("stream_second_pass_mismatch"));
    }
    file.sync_all()?;
    drop(file);
    validate(&path)?;
    fs::rename(&path, dir.join(name))?;
    sync_directory(dir)?;
    Ok(())
}

/// Conservative simultaneous I/O controls and buffers, separate from parser scratch.
pub(super) const fn capacity_metadata_bytes() -> usize {
    std::mem::size_of::<Output<File>>()
        + std::mem::size_of::<Input>()
        + 2 * std::mem::size_of::<Identity>()
        + 2 * 64
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "mycity-stream-{}-{}",
                std::process::id(),
                TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn complete_stream_hash_length_and_unconsumed_tail_are_required() {
        let d = Dir::new();
        let p = d.0.join("input");
        let bytes = vec![b'x'; 3 * PAGE + 17];
        fs::write(&p, &bytes).unwrap();
        let identity = Identity {
            bytes: bytes.len() as u64,
            sha256: sha256(&bytes),
        };
        verify_bytes(&p, &identity, identity.bytes).unwrap();
        assert!(verified(&p, &identity, identity.bytes, |r| {
            r.read_exact(&mut [0; 2])?;
            Ok(())
        })
        .is_err());
        let wrong = Identity {
            sha256: "0".repeat(64),
            ..identity.clone()
        };
        assert!(verify_bytes(&p, &wrong, identity.bytes).is_err());
        assert!(verify_bytes(&p, &identity, identity.bytes - 1).is_err());
        // File grows after metadata admission; the EOF probe must observe it.
        assert!(verified(&p, &identity, identity.bytes, |r| {
            io::copy(r, &mut io::sink())?;
            OpenOptions::new()
                .append(true)
                .open(&p)?
                .write_all(b"tail")?;
            Ok(())
        })
        .is_err());
        fs::write(&p, &bytes[..bytes.len() - 1]).unwrap();
        assert!(verify_bytes(&p, &identity, identity.bytes).is_err());
    }
    #[test]
    fn second_pass_or_validation_failure_cannot_publish_and_tmp_is_reclaimed() {
        let d = Dir::new();
        let target = d.0.join("world.json");
        fs::write(&target, b"old").unwrap();
        let identity = measure(100, |w| {
            w.write_all(b"new")?;
            Ok(())
        })
        .unwrap();
        assert!(atomic(
            &d.0,
            "world.json",
            &identity,
            100,
            |w| {
                w.write_all(b"bad")?;
                Ok(())
            },
            |_| panic!("must not validate mismatch")
        )
        .is_err());
        assert!(atomic(
            &d.0,
            "world.json",
            &identity,
            100,
            |w| {
                w.write_all(b"new")?;
                Ok(())
            },
            |_| Err(Error::Invalid("injected_validation"))
        )
        .is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert_eq!(fs::read_dir(&d.0).unwrap().count(), 1);
        atomic(
            &d.0,
            "world.json",
            &identity,
            100,
            |w| {
                w.write_all(b"new")?;
                Ok(())
            },
            |p| verify_bytes(p, &identity, 100),
        )
        .unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(fs::read_dir(&d.0).unwrap().count(), 1);
        assert!(measure(2, |w| {
            w.write_all(b"new")?;
            Ok(())
        })
        .is_err());
    }
}
