//! Fixed diagnostic record decoding and shared lexical limits.
//! General model record visitors live in bounded_decode/arena, not in this schema.
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::{
    fmt,
    io::{self, Read},
    marker::PhantomData,
    mem::size_of,
};

pub const RESTORE_BYTES: usize = 64 * 1024 * 1024;
pub const PARSER_RESERVE: usize = 6 * 1024 * 1024;
const INPUT_BYTES: usize = 128 * 1024 * 1024;

/// Only use with sealed, fixed-size record types. Count comes from the frozen
/// local layout, never the file or a deserializer's untrusted size hint.
pub struct Records<T: Copy> {
    count: usize,
    marker: PhantomData<T>,
}

impl<T: Copy> Records<T> {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            marker: PhantomData,
        }
    }
}
impl<'de, T: Copy + Deserialize<'de>> DeserializeSeed<'de> for Records<T> {
    type Value = Vec<T>;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}
struct RejectExtra;
impl<'de> DeserializeSeed<'de> for RejectExtra {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, _d: D) -> Result<(), D::Error> {
        Err(de::Error::custom("fixed_restore_extra_record"))
    }
}
impl<'de, T: Copy + Deserialize<'de>> Visitor<'de> for Records<T> {
    type Value = Vec<T>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "exactly {} fixed records", self.count)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
        let bytes = self
            .count
            .checked_mul(size_of::<T>())
            .filter(|_| size_of::<T>() != 0)
            .ok_or_else(|| de::Error::custom("fixed_restore_capacity_overflow"))?;
        if bytes > RESTORE_BYTES - PARSER_RESERVE {
            return Err(de::Error::custom("fixed_restore_budget_before_allocation"));
        }
        let mut out = Vec::new();
        out.try_reserve_exact(self.count)
            .map_err(de::Error::custom)?;
        if out.capacity() != self.count {
            return Err(de::Error::custom("fixed_restore_capacity"));
        }
        for i in 0..self.count {
            let value = seq
                .next_element()?
                .ok_or_else(|| de::Error::invalid_length(i, &self))?;
            out.push(value);
        }
        // Reject before deserializing any extra record, including nested data.
        seq.next_element_seed(RejectExtra)?;
        Ok(out)
    }
}
pub(crate) fn vector<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Copy + Deserialize<'de>,
{
    Records::new(N).deserialize(d)
}

/// Limit parser scratch before serde accumulates a long token. Native/layout
/// records contain only bounded identifiers, booleans and fixed-width integers.
pub struct JsonReader<R> {
    reader: R,
    quoted: bool,
    escaped: bool,
    string_bytes: usize,
    string_limit: usize,
    digits: usize,
    bytes: usize,
    depth: usize,
}
impl<R: Read> JsonReader<R> {
    pub fn new(reader: R) -> Self {
        Self::with_string_limit(reader, 128)
    }
    /// Trusted caller selects a lexical token bound before deserialization.
    pub fn with_string_limit(reader: R, string_limit: usize) -> Self {
        Self {
            reader,
            quoted: false,
            escaped: false,
            string_bytes: 0,
            string_limit,
            digits: 0,
            bytes: 0,
            depth: 0,
        }
    }
}
impl<R: Read> Read for JsonReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let limit = out.len().min(4096);
        let n = self.reader.read(&mut out[..limit])?;
        self.bytes = self
            .bytes
            .checked_add(n)
            .ok_or_else(|| io::Error::other("fixed_restore_input"))?;
        if self.bytes > INPUT_BYTES {
            return Err(io::Error::other("fixed_restore_input"));
        }
        for &b in &out[..n] {
            if self.quoted {
                if b == b'"' && !self.escaped {
                    self.quoted = false;
                    continue;
                }
                self.string_bytes += 1;
                if self.string_bytes > self.string_limit {
                    return Err(io::Error::other("fixed_restore_string"));
                }
                if self.escaped {
                    self.escaped = false;
                } else if b == b'\\' {
                    self.escaped = true;
                }
            } else if b == b'"' {
                self.quoted = true;
                self.escaped = false;
                self.string_bytes = 0;
                self.digits = 0;
            } else if b.is_ascii_digit() {
                self.digits += 1;
                if self.digits > 20 {
                    return Err(io::Error::other("fixed_restore_integer"));
                }
            } else {
                self.digits = 0;
                match b {
                    b'{' | b'[' => {
                        self.depth += 1;
                        if self.depth > 128 {
                            return Err(io::Error::other("fixed_restore_nesting"));
                        }
                    }
                    b'}' | b']' => self.depth = self.depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DECODED: AtomicUsize = AtomicUsize::new(0);
    #[derive(Clone, Copy, Debug)]
    struct Counted(u64);
    impl<'de> Deserialize<'de> for Counted {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            DECODED.fetch_add(1, Ordering::SeqCst);
            u64::deserialize(d).map(Self)
        }
    }
    fn decode<T: Copy + for<'de> Deserialize<'de>>(
        text: &str,
        count: usize,
    ) -> Result<Vec<T>, String> {
        let mut d = serde_json::Deserializer::from_reader(JsonReader::new(text.as_bytes()));
        let result = Records::<T>::new(count)
            .deserialize(&mut d)
            .map_err(|e| e.to_string())?;
        d.end().map_err(|e| e.to_string())?;
        Ok(result)
    }
    #[test]
    fn capacity_overflow_and_extra_record_reject_before_record_decode() {
        DECODED.store(0, Ordering::SeqCst);
        assert!(decode::<Counted>("[]", usize::MAX)
            .unwrap_err()
            .contains("capacity_overflow"));
        assert_eq!(DECODED.load(Ordering::SeqCst), 0);
        assert!(decode::<Counted>("[1,{\"unbounded\":[[[[2]]]]}]", 1)
            .unwrap_err()
            .contains("extra_record"));
        assert_eq!(DECODED.load(Ordering::SeqCst), 1);
        let values = decode::<Counted>("[7]", 1).unwrap();
        assert_eq!(values[0].0, 7);
        assert_eq!(values.capacity(), 1);
    }
    #[test]
    fn fixed_counts_eof_and_token_limits_reject_malformed_streams() {
        assert!(decode::<u64>("[1]", 2).is_err());
        assert!(decode::<u64>("[1] false", 1).is_err());
        assert!(decode::<u64>(&format!("[{}]", "9".repeat(2000)), 1)
            .unwrap_err()
            .contains("integer"));
        let long_key = format!("{{\"{}\":0}}", "x".repeat(2000));
        let mut reader = JsonReader::new(long_key.as_bytes());
        assert!(io::copy(&mut reader, &mut io::sink()).is_err());
        let mut reader = JsonReader::new(&b"  "[..]);
        reader.bytes = INPUT_BYTES - 1;
        assert!(reader.read(&mut [0; 4]).is_err());
    }
    #[test]
    fn nested_ignored_field_is_bounded_before_parser_scratch_can_grow() {
        #[derive(Clone, Copy, Debug, Deserialize)]
        struct Fixed {
            value: u64,
        }
        let input = format!(
            "[{{\"value\":7,\"extra\":{}0{}}}]",
            "[".repeat(256),
            "]".repeat(256)
        );
        let result = decode::<Fixed>(&input, 1);
        if let Ok(values) = &result {
            assert_eq!(values[0].value, 7);
        }
        assert!(result.unwrap_err().contains("nesting"));
    }
    #[test]
    fn record_budget_is_checked_before_reservation() {
        let n = RESTORE_BYTES / size_of::<u64>();
        assert!(decode::<u64>("[]", n)
            .unwrap_err()
            .contains("budget_before_allocation"));
        assert!(decode::<u64>("[]", 0).unwrap().is_empty());
    }
}
