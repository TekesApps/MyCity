//! Canonicalization for bounded model schemas, matching serde_json Value ordering.
//! Input stays borrowed. Fixed initialized traversal storage replaces a JSON DOM;
//! arrays stream without a row-count cap. No recursive object tables on the stack.
use serde::{
    de::{self, DeserializeSeed, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::value::RawValue;
use std::{
    fmt,
    io::{self, Write},
    mem::size_of,
};
const KEYS: usize = 64;
const DEPTH: usize = 128;
#[derive(Clone, Copy)]
struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> Text<N> {
    const EMPTY: Self = Self {
        bytes: [0; N],
        len: 0,
    };
    fn str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("decoded UTF8")
    }
}
impl<const N: usize> Serialize for Text<N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.str())
    }
}
impl<'de, const N: usize> Deserialize<'de> for Text<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<const N: usize>;
        impl<const N: usize> Visitor<'_> for V<N> {
            type Value = Text<N>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "at most {N} decoded bytes")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.len() > N {
                    return Err(E::custom("canonical_text_bound"));
                }
                let mut text = Text::EMPTY;
                text.bytes[..s.len()].copy_from_slice(s.as_bytes());
                text.len = s.len();
                Ok(text)
            }
        }
        d.deserialize_str(V::<N>)
    }
}
#[derive(Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
}
impl Span {
    const EMPTY: Self = Self { start: 0, end: 0 };
    fn borrowed(whole: &str, raw: &RawValue) -> Result<Self, String> {
        let start = (raw.get().as_ptr() as usize)
            .checked_sub(whole.as_ptr() as usize)
            .ok_or("canonical_borrow")?;
        let end = start
            .checked_add(raw.get().len())
            .filter(|n| *n <= whole.len())
            .ok_or("canonical_borrow")?;
        Ok(Self { start, end })
    }
}
#[derive(Clone, Copy)]
struct Entry {
    key: Text<64>,
    value: Span,
}
impl Entry {
    const EMPTY: Self = Self {
        key: Text::EMPTY,
        value: Span::EMPTY,
    };
}
struct Frame {
    kind: u8,
    count: usize,
    next: usize,
    array: Span,
    entries: [Entry; KEYS],
}
impl Frame {
    fn empty() -> Self {
        Self {
            kind: 0,
            count: 0,
            next: 0,
            array: Span::EMPTY,
            entries: [Entry::EMPTY; KEYS],
        }
    }
}
struct Object<'a> {
    frame: &'a mut Frame,
    whole: &'a str,
}
impl<'de> DeserializeSeed<'de> for Object<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Object<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded model object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        self.frame.kind = 1;
        self.frame.count = 0;
        self.frame.next = 0;
        while let Some(key) = a.next_key::<Text<64>>()? {
            let slot = self.frame.entries[..self.frame.count]
                .iter()
                .position(|e| e.key.str() == key.str());
            let slot = match slot {
                Some(i) => i,
                None if self.frame.count < KEYS => {
                    let i = self.frame.count;
                    self.frame.count += 1;
                    i
                }
                None => return Err(de::Error::custom("canonical_unique_key_bound")),
            };
            let raw: &RawValue = a.next_value()?;
            let value = Span::borrowed(self.whole, raw).map_err(de::Error::custom)?;
            // Value's last decoded duplicate key wins, independent of spelling.
            self.frame.entries[slot] = Entry { key, value };
        }
        self.frame.entries[..self.frame.count]
            .sort_unstable_by(|a, b| a.key.str().cmp(b.key.str()));
        Ok(())
    }
}
// RawValue's syntax-only skip accepts out-of-range numbers in discarded duplicate
// subtrees. Validate every value with the old Value parser's scalar semantics
// before applying last-key wins. No object/array/value tree is retained.
struct Valid;
impl<'de> Deserialize<'de> for Valid {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Valid;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("valid bounded model JSON")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Valid, E> {
                Ok(Valid)
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Valid, E> {
                Ok(Valid)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Valid, E> {
                Ok(Valid)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Valid, E> {
                Ok(Valid)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Valid, E> {
                Ok(Valid)
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<Valid, E> {
                Ok(Valid)
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut a: A) -> Result<Valid, A::Error> {
                while a.next_element::<Valid>()?.is_some() {}
                Ok(Valid)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Valid, A::Error> {
                while a.next_key::<Text<64>>()?.is_some() {
                    let _: Valid = a.next_value()?;
                }
                Ok(Valid)
            }
        }
        d.deserialize_any(V)
    }
}
/// Trusted, reusable backing. All128 traversal frames are initialized and charged
/// once; input can never drive capacity, recursive allocation or frame growth.
pub struct Workspace {
    frames: Vec<Frame>,
}
impl Workspace {
    pub fn new() -> Result<Self, String> {
        let mut frames = Vec::new();
        frames.try_reserve_exact(DEPTH).map_err(|e| e.to_string())?;
        if frames.capacity() != DEPTH {
            return Err("canonical_capacity".into());
        }
        for _ in 0..DEPTH {
            frames.push(Frame::empty());
        }
        Ok(Self { frames })
    }
    pub const fn backing_bytes() -> usize {
        DEPTH * size_of::<Frame>()
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.frames.capacity() * size_of::<Frame>()
    }
    pub fn write(&mut self, input: &[u8], mut writer: impl Write) -> Result<(), String> {
        crate::bounded_decode::guard(input)?;
        let text = std::str::from_utf8(input).map_err(|e| e.to_string())?;
        let _: Valid = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let raw: &RawValue = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut pending = Some(Span::borrowed(text, raw)?);
        let mut depth = 0usize;
        loop {
            if let Some(span) = pending.take() {
                let node = &text[span.start..span.end];
                match node.as_bytes()[0] {
                    b'{' => {
                        if depth == DEPTH {
                            return Err("canonical_depth".into());
                        }
                        let mut d = serde_json::Deserializer::from_str(node);
                        Object {
                            frame: &mut self.frames[depth],
                            whole: text,
                        }
                        .deserialize(&mut d)
                        .map_err(|e| e.to_string())?;
                        d.end().map_err(|e| e.to_string())?;
                        writer.write_all(b"{").map_err(|e| e.to_string())?;
                        depth += 1;
                    }
                    b'[' => {
                        if depth == DEPTH {
                            return Err("canonical_depth".into());
                        }
                        let f = &mut self.frames[depth];
                        f.kind = 2;
                        f.next = 0;
                        f.array = Span {
                            start: span.start + 1,
                            end: span.end - 1,
                        };
                        writer.write_all(b"[").map_err(|e| e.to_string())?;
                        depth += 1;
                    }
                    b'"' => {
                        let value: Text<384> =
                            serde_json::from_str(node).map_err(|e| e.to_string())?;
                        serde_json::to_writer(&mut writer, &value).map_err(|e| e.to_string())?;
                    }
                    b'n' | b't' | b'f' => writer
                        .write_all(node.as_bytes())
                        .map_err(|e| e.to_string())?,
                    _ => {
                        let value: serde_json::Number =
                            serde_json::from_str(node).map_err(|e| e.to_string())?;
                        serde_json::to_writer(&mut writer, &value).map_err(|e| e.to_string())?;
                    }
                }
                continue;
            }
            if depth == 0 {
                return Ok(());
            }
            let f = &mut self.frames[depth - 1];
            if f.kind == 1 {
                if f.next == f.count {
                    writer.write_all(b"}").map_err(|e| e.to_string())?;
                    depth -= 1;
                    continue;
                }
                if f.next != 0 {
                    writer.write_all(b",").map_err(|e| e.to_string())?;
                }
                let entry = f.entries[f.next];
                f.next += 1;
                serde_json::to_writer(&mut writer, &entry.key).map_err(|e| e.to_string())?;
                writer.write_all(b":").map_err(|e| e.to_string())?;
                pending = Some(entry.value);
            } else {
                while f.array.start < f.array.end
                    && text.as_bytes()[f.array.start].is_ascii_whitespace()
                {
                    f.array.start += 1;
                }
                if f.array.start == f.array.end {
                    writer.write_all(b"]").map_err(|e| e.to_string())?;
                    depth -= 1;
                    continue;
                }
                if f.next != 0 {
                    if text.as_bytes()[f.array.start] != b',' {
                        return Err("canonical_array_comma".into());
                    }
                    f.array.start += 1;
                    writer.write_all(b",").map_err(|e| e.to_string())?;
                }
                let fragment = &text[f.array.start..f.array.end];
                let mut values =
                    serde_json::Deserializer::from_str(fragment).into_iter::<&RawValue>();
                let value = values
                    .next()
                    .ok_or("canonical_array_value")?
                    .map_err(|e| e.to_string())?;
                let span = Span::borrowed(text, value)?;
                f.array.start = span.end;
                f.next += 1;
                pending = Some(span);
            }
        }
    }
}
/// Explicit caller-owned model comparison slot. Reuse serially across engines;
/// never store one per Engine. Not a pool grant or arbitrary-JSON serializer.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CodecIdentity {
    pub byte_backing: usize,
    pub traversal_backing: usize,
    pub backing_bytes: usize,
}
pub struct Codec {
    bytes: Vec<u8>,
    pub workspace: Workspace,
}
impl Codec {
    pub const LIMIT: usize = 4 * 1024 * 1024;
    pub fn new() -> Result<Self, String> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(Self::LIMIT)
            .map_err(|e| e.to_string())?;
        if bytes.capacity() != Self::LIMIT {
            return Err("comparison_capacity".into());
        }
        bytes.resize(Self::LIMIT, 0);
        Ok(Self {
            bytes,
            workspace: Workspace::new()?,
        })
    }
    pub fn identity(&self) -> CodecIdentity {
        CodecIdentity {
            byte_backing: self.bytes.as_ptr() as usize,
            traversal_backing: self.workspace.frames.as_ptr() as usize,
            backing_bytes: self.backing_capacity_bytes(),
        }
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.backing_capacity_bytes()
    }
    pub fn backing_capacity_bytes(&self) -> usize {
        self.bytes.capacity() + Workspace::backing_bytes()
    }
    pub fn write(&mut self, value: &impl Serialize, writer: impl Write) -> Result<(), String> {
        let mut input = FixedWriter::new(&mut self.bytes);
        serde_json::to_writer(&mut input, value).map_err(|e| e.to_string())?;
        struct Limited<W> {
            inner: W,
            bytes: usize,
        }
        impl<W: Write> Write for Limited<W> {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                if b.len() > Codec::LIMIT - self.bytes {
                    return Err(io::Error::other("comparison_output_bound"));
                }
                let n = self.inner.write(b)?;
                self.bytes += n;
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.inner.flush()
            }
        }
        self.workspace.write(
            input.as_slice(),
            Limited {
                inner: writer,
                bytes: 0,
            },
        )
    }
    /// Reuse the same initialized byte slot for a typed checkpoint and its seed.
    /// Result cannot borrow the input. No second encoded-body buffer is created.
    pub fn with_encoded<T>(
        &mut self,
        encode: impl FnOnce(&mut FixedWriter<'_>) -> Result<(), String>,
        consume: impl FnOnce(&[u8]) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut input = FixedWriter::new(&mut self.bytes);
        encode(&mut input)?;
        consume(input.as_slice())
    }
    pub fn hash(&mut self, value: &impl Serialize) -> Result<String, String> {
        use sha2::{Digest, Sha256};
        struct Hash(Sha256);
        impl Write for Hash {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0.update(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut hash = Hash(Sha256::new());
        self.write(value, &mut hash)?;
        Ok(format!("{:x}", hash.0.finalize()))
    }
}
/// Borrow an already allocated codec slot. Never grow or allocate on overflow.
pub struct FixedWriter<'a> {
    bytes: &'a mut [u8],
    len: usize,
}
impl<'a> FixedWriter<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, len: 0 }
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl Write for FixedWriter<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let end = self
            .len
            .checked_add(data.len())
            .filter(|n| *n <= self.bytes.len())
            .ok_or_else(|| io::Error::other("canonical_codec_bound"))?;
        self.bytes[self.len..end].copy_from_slice(data);
        self.len = end;
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compare(work: &mut Workspace, text: &str) {
        let old: serde_json::Value = serde_json::from_str(text).unwrap();
        let expected = serde_json::to_vec(&old).unwrap();
        let mut actual = Vec::new();
        work.write(text.as_bytes(), &mut actual).unwrap();
        assert_eq!(actual, expected, "{text}");
    }
    #[test]
    fn old_value_is_an_independent_oracle_for_duplicates_escapes_and_numbers() {
        let mut work = Workspace::new().unwrap();
        let backing = work.frames.as_ptr();
        let cap = work.owned_capacity_bytes();
        for text in [
            r#"{"z":3,"a":1,"nested":{"b":false,"a":null},"array":[3,1,{"z":0,"a":2}]}"#,
            r#"{"a":1,"\u0061":2,"z":"\u4e2d\u6587","quote":"a\"b\\c\n\t\u0001"}"#,
            r#"[0,-0,0.0,-0.0,18446744073709551615,-9223372036854775808,1e100,1e-100,0.1,1.2300,1e+0]"#,
            r#"{"😀":"🙂","中":1,"ä":2,"z":3}"#,
            r#"[[],{},true,false,null,"",[1,[2,[3]]]]"#,
        ] {
            compare(&mut work, text);
        }
        let duplicate = format!(
            "{{{}}}",
            (0..80)
                .map(|n| format!("\"a\":{n}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        compare(&mut work, &duplicate);
        // All64 encoded ASCII characters can be escaped without counting closing quote as token data.
        let escaped = format!("\"{}\"", "\\u0061".repeat(64));
        compare(&mut work, &escaped);
        assert_eq!(work.frames.as_ptr(), backing);
        assert_eq!(work.owned_capacity_bytes(), cap);
    }
    #[test]
    fn large_arrays_and_nested_objects_use_same_fixed_backing_and_errors_do_not_poison_reuse() {
        let mut work = Workspace::new().unwrap();
        let text = format!(
            "[{}]",
            (0..20000)
                .map(|n| format!("{{\"z\":{n},\"a\":0}}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        compare(&mut work, &text);
        let nested = format!("{}0{}", "{\"a\":".repeat(100), "}".repeat(100));
        compare(&mut work, &nested);
        let large_map = format!(
            "{{{}}}",
            (0..65)
                .map(|n| format!("\"a{n}\":0"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(work
            .write(large_map.as_bytes(), io::sink())
            .unwrap_err()
            .contains("canonical_unique_key_bound"));
        assert!(work
            .write(format!("\"{}\"", "x".repeat(385)).as_bytes(), io::sink())
            .is_err());
        assert!(work
            .write(
                format!("{}0{}", "[".repeat(129), "]".repeat(129)).as_bytes(),
                io::sink()
            )
            .is_err());
        assert!(work.write(b"{} trailing", io::sink()).is_err());
        compare(&mut work, r#"{"z":0,"a":1}"#);
    }
    #[test]
    fn overwritten_values_still_obey_old_value_validity() {
        let mut work = Workspace::new().unwrap();
        for text in [
            r#"{"a":1e400,"a":1}"#,
            r#"{"a":{"nested":[1e400]},"a":1}"#,
            r#"{"a":"\uD800","a":1}"#,
        ] {
            assert!(serde_json::from_str::<serde_json::Value>(text).is_err());
            assert!(work.write(text.as_bytes(), io::sink()).is_err());
        }
        compare(&mut work, r#"{"a":1e100,"a":1}"#);
    }
    #[test]
    fn fixed_codec_and_writer_failure_never_fall_back_to_allocation() {
        let mut work = Workspace::new().unwrap();
        let mut slot = [0u8; 4];
        assert!(work
            .write(b"[1,2]", FixedWriter::new(&mut slot))
            .unwrap_err()
            .contains("canonical_codec_bound"));
        compare(&mut work, "[1,2]");
    }
}
