//! Borrow fixed backing across files; no hidden per-file MiB allocation.
use std::io::{self, Read, Write};

pub(super) struct Reader<'a, R> {
    inner: R,
    backing: &'a mut [u8],
    position: usize,
    filled: usize,
}
impl<'a, R> Reader<'a, R> {
    pub fn new(inner: R, backing: &'a mut [u8]) -> Self {
        assert!(!backing.is_empty());
        Self {
            inner,
            backing,
            position: 0,
            filled: 0,
        }
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
}
impl<R: Read> Read for Reader<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.position == self.filled {
            if output.len() >= self.backing.len() {
                return self.inner.read(output);
            }
            self.filled = self.inner.read(self.backing)?;
            self.position = 0;
        }
        let n = output.len().min(self.filled - self.position);
        output[..n].copy_from_slice(&self.backing[self.position..self.position + n]);
        self.position += n;
        Ok(n)
    }
}

pub(super) struct Writer<'a, W> {
    inner: W,
    backing: &'a mut [u8],
    filled: usize,
}
impl<'a, W: Write> Writer<'a, W> {
    pub fn new(inner: W, backing: &'a mut [u8]) -> Self {
        assert!(!backing.is_empty());
        Self {
            inner,
            backing,
            filled: 0,
        }
    }
    fn drain(&mut self) -> io::Result<()> {
        // On failure the caller abandons the pending file; no Drop retries a
        // partially written buffer and no successful flush is fabricated.
        self.inner.write_all(&self.backing[..self.filled])?;
        self.filled = 0;
        Ok(())
    }
    pub fn get_ref(&self) -> &W {
        &self.inner
    }
}
impl<W: Write> Write for Writer<'_, W> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.len() > self.backing.len() - self.filled {
            self.drain()?;
        }
        if input.len() >= self.backing.len() {
            return self.inner.write(input);
        }
        self.backing[self.filled..self.filled + input.len()].copy_from_slice(input);
        self.filled += input.len();
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.drain()?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_io_empty_reads_and_file_switch_preserve_backing_and_bytes() {
        struct Short<'a>(&'a mut Vec<u8>);
        impl Write for Short<'_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let n = bytes.len().min(3);
                self.0.extend_from_slice(&bytes[..n]);
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut backing = [0u8; 17];
        let address = backing.as_ptr();
        for size in [0, 1, 16, 17, 18, 91] {
            let expected: Vec<_> = (0..size).map(|n| n as u8).collect();
            let mut output = Vec::new();
            {
                let mut writer = Writer::new(Short(&mut output), &mut backing);
                for chunk in expected.chunks(11) {
                    writer.write_all(chunk).unwrap();
                }
                writer.flush().unwrap();
            }
            assert_eq!(output, expected);
            let mut reader = Reader::new(&output[..], &mut backing);
            let mut result = Vec::new();
            assert_eq!(reader.read(&mut []).unwrap(), 0);
            let mut byte = [0];
            while reader.read(&mut byte).unwrap() != 0 {
                result.push(byte[0]);
            }
            assert_eq!(result, expected);
            assert_eq!(address, backing.as_ptr());
        }
    }
    #[test]
    fn flush_errors_propagate_without_drop_retry() {
        struct Fail;
        impl Write for Fail {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::Other.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut backing = [0; 8];
        let mut writer = Writer::new(Fail, &mut backing);
        writer.write_all(b"a").unwrap();
        assert!(writer.flush().is_err());
    }
}
