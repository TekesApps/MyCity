//! Serial full-byte comparison. One caller codec, bounded readback, no paired Vec.
use mycity_core::{canonical::Codec, transaction::Engine};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Equal<R> {
    reader: R,
    bytes: u64,
}
impl<R: Read> Write for Equal<R> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let mut check = [0u8; 4096];
        for chunk in b.chunks(check.len()) {
            self.reader.read_exact(&mut check[..chunk.len()])?;
            if chunk != &check[..chunk.len()] {
                return Err(io::Error::other("business_bytes_differ"));
            }
        }
        self.bytes += b.len() as u64;
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Temp file never contains the only restore input; errors retain it for diagnosis.
/// On success close before removing the exact path. No t13 directory prefix cleanup.
pub(crate) fn business(a: &Engine, b: &Engine, codec: &mut Codec) -> Result<(), String> {
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "mycity-business-{}-{number}.json",
        std::process::id()
    ));
    let result = (|| {
        let mut output = BufWriter::with_capacity(
            8192,
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| e.to_string())?,
        );
        a.write_business_comparison(codec, &mut output)?;
        output.flush().map_err(|e| e.to_string())?;
        drop(output);
        let mut equal = Equal {
            reader: BufReader::with_capacity(8192, File::open(&path).map_err(|e| e.to_string())?),
            bytes: 0,
        };
        b.write_business_comparison(codec, &mut equal)?;
        let mut end = [0; 1];
        if equal.reader.read(&mut end).map_err(|e| e.to_string())? != 0 {
            return Err("business_trailing_bytes".into());
        }
        Ok(())
    })();
    result
        .map_err(|e: String| format!("{e}; comparison evidence retained at {}", path.display()))?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_reader_rejects_different_or_short_input_and_checks_tail() {
        let mut equal = Equal {
            reader: io::Cursor::new(b"abc"),
            bytes: 0,
        };
        assert!(equal.write_all(b"abd").is_err());
        let mut equal = Equal {
            reader: io::Cursor::new(b"ab"),
            bytes: 0,
        };
        assert!(equal.write_all(b"abc").is_err());
        let mut equal = Equal {
            reader: io::Cursor::new(b"abcd"),
            bytes: 0,
        };
        equal.write_all(b"abc").unwrap();
        assert_eq!(equal.reader.read(&mut [0]).unwrap(), 1);
        let bytes = vec![7; 20000];
        let mut equal = Equal {
            reader: io::Cursor::new(&bytes),
            bytes: 0,
        };
        equal.write_all(&bytes).unwrap();
        assert_eq!(equal.bytes, 20000);
        assert_eq!(equal.reader.read(&mut [0]).unwrap(), 0);
    }
}
