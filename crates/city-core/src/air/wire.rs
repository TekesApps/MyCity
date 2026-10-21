//! Canonical checkpoint payload only. State::Serialize stays the independent wire oracle.
use super::State;
use serde::Serialize;
use std::io::{self, Write};
const PAIRS: &[u8;200] = b"00010203040506070809101112131415161718192021222324252627282930313233343536373839404142434445464748495051525354555657585960616263646566676869707172737475767778798081828384858687888990919293949596979899";
fn decimal(mut value: u64, digits: &mut [u8; 20]) -> &[u8] {
    let mut at = digits.len();
    while value >= 100 {
        let pair = (value % 100) as usize * 2;
        value /= 100;
        at -= 2;
        digits[at..at + 2].copy_from_slice(&PAIRS[pair..pair + 2]);
    }
    if value < 10 {
        at -= 1;
        digits[at] = b'0' + value as u8;
    } else {
        at -= 2;
        let pair = value as usize * 2;
        digits[at..at + 2].copy_from_slice(&PAIRS[pair..pair + 2]);
    }
    &digits[at..]
}
fn unsigned_array<T: Copy + Into<u64>>(writer: &mut impl Write, values: &[T]) -> io::Result<()> {
    let mut chunk = [0u8; 8192];
    let mut used = 1;
    chunk[0] = b'[';
    let mut digits = [0u8; 20];
    for (i, value) in values.iter().enumerate() {
        // A comma, 20 digits and a closing bracket must always fit.
        if used + 22 > chunk.len() {
            writer.write_all(&chunk[..used])?;
            used = 0;
        }
        if i > 0 {
            chunk[used] = b',';
            used += 1;
        }
        let number = decimal((*value).into(), &mut digits);
        chunk[used..used + number.len()].copy_from_slice(number);
        used += number.len();
    }
    chunk[used] = b']';
    used += 1;
    writer.write_all(&chunk[..used])
}
fn bytes(writer: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
    writer.write_all(bytes).map_err(|e| e.to_string())
}
fn value(writer: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(writer, value).map_err(|e| e.to_string())
}
pub(super) fn payload(writer: &mut impl Write, s: &State) -> Result<(), String> {
    bytes(writer, b"{\"input\":")?;
    value(writer, &s.input)?;
    bytes(writer, b",\"input_sha256\":")?;
    value(writer, &s.input_sha256)?;
    bytes(writer, b",\"time\":")?;
    value(writer, &s.time)?;
    bytes(writer, b",\"field\":")?;
    unsigned_array(writer, &s.field).map_err(|e| e.to_string())?;
    bytes(writer, b",\"scratch\":")?;
    unsigned_array(writer, &s.scratch).map_err(|e| e.to_string())?;
    bytes(writer, b",\"integral\":")?;
    unsigned_array(writer, &s.integral).map_err(|e| e.to_string())?;
    bytes(writer, b",\"progress\":")?;
    value(writer, &s.progress)?;
    bytes(writer, b",\"totals\":")?;
    value(writer, &s.totals)?;
    bytes(writer, b",\"pending\":")?;
    value(writer, &s.pending)?;
    bytes(writer, b",\"steps\":")?;
    value(writer, &s.steps)?;
    bytes(writer, b"}")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_decimal_boundary_and_chunk_tail_matches_independent_serde() {
        let mut rows = vec![0, u32::MAX as u64, u64::MAX];
        let mut power = 1u64;
        for _ in 0..20 {
            rows.extend([power - 1, power, power.saturating_add(1)]);
            let Some(next) = power.checked_mul(10) else {
                break;
            };
            power = next;
        }
        let mut x = 7u64;
        for _ in 0..16384 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            rows.push(x);
        }
        for n in [0, 1, 3, 40, 409, 819, 820, 821, 16384, rows.len()] {
            let mut out = Vec::new();
            unsigned_array(&mut out, &rows[..n]).unwrap();
            assert_eq!(out, serde_json::to_vec(&rows[..n]).unwrap());
        }
        let small: Vec<u32> = rows.iter().map(|v| *v as u32).collect();
        let mut out = Vec::new();
        unsigned_array(&mut out, &small).unwrap();
        assert_eq!(out, serde_json::to_vec(&small).unwrap());
    }
    #[test]
    fn short_writes_errors_and_original_byte_bound_are_not_bypassed() {
        struct Short {
            bytes: Vec<u8>,
            fail: Option<usize>,
        }
        impl Write for Short {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                if self.fail.is_some_and(|cap| self.bytes.len() >= cap) {
                    return Err(io::Error::other("cut"));
                }
                let n = b.len().min(3);
                self.bytes.extend_from_slice(&b[..n]);
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let values = vec![u64::MAX; 2000];
        let mut short = Short {
            bytes: Vec::new(),
            fail: None,
        };
        unsigned_array(&mut short, &values).unwrap();
        assert_eq!(short.bytes, serde_json::to_vec(&values).unwrap());
        for cap in [0, 3, 8192, 20000] {
            let mut cut = Short {
                bytes: Vec::new(),
                fail: Some(cap),
            };
            assert!(unsigned_array(&mut cut, &values)
                .unwrap_err()
                .to_string()
                .contains("cut"));
        }
        let mut sink = io::sink();
        let mut bounded = super::super::BoundedEncoding {
            inner: &mut sink,
            bytes: super::super::CHECKPOINT_BYTES - 1,
        };
        assert!(unsigned_array(&mut bounded, &values)
            .unwrap_err()
            .to_string()
            .contains("air_checkpoint_bytes"));
        assert_eq!(bounded.bytes, super::super::CHECKPOINT_BYTES - 1);
    }
}
