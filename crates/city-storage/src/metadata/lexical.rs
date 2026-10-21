//! Fixed storage JSON scanning. Ignore mode deliberately follows serde_json's
//! IgnoredAny grammar, not Content's Unicode/number/depth validation.
use serde::de;
use serde_json::Error;
pub(crate) const INPUT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const STACK_BYTES: usize = INPUT_BYTES / 16;
pub(crate) const TEXT_BYTES: usize = 128;
pub(crate) const IDENTIFIER_BYTES: usize = 64;
pub(crate) fn error(message: &'static str) -> Error {
    de::Error::custom(message)
}

pub(crate) struct Scratch {
    bits: Vec<u8>,
    used: usize,
}
impl Scratch {
    pub fn new() -> Result<Self, Error> {
        let mut bits = Vec::new();
        bits.try_reserve_exact(STACK_BYTES)
            .map_err(|_| error("lexical_allocation"))?;
        if bits.capacity() != STACK_BYTES {
            return Err(error("lexical_capacity"));
        }
        bits.resize(STACK_BYTES, 0xa5);
        std::hint::black_box(bits.iter().fold(0u64, |n, b| n + u64::from(*b)));
        Ok(Self { bits, used: 0 })
    }
    pub fn identity(&self) -> (usize, usize) {
        (self.bits.as_ptr() as usize, self.bits.capacity())
    }
    fn push(&mut self, object: bool) -> Result<(), Error> {
        if self.used >= self.bits.len() * 8 {
            return Err(error("lexical_stack_capacity"));
        }
        let (byte, bit) = (self.used / 8, self.used % 8);
        self.bits[byte] = (self.bits[byte] & !(1 << bit)) | ((object as u8) << bit);
        self.used += 1;
        Ok(())
    }
    fn top(&self) -> Option<bool> {
        if self.used == 0 {
            None
        } else {
            let i = self.used - 1;
            Some(self.bits[i / 8] & (1 << (i % 8)) != 0)
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Raw<'de> {
    pub bytes: &'de [u8],
    pub depth: usize,
}
impl<'de> Raw<'de> {
    pub fn root(bytes: &'de [u8]) -> Self {
        Self { bytes, depth: 0 }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Ignore,
    Content,
}
pub(crate) struct Text {
    bytes: [u8; TEXT_BYTES],
    len: usize,
    overflow: bool,
}
impl Text {
    fn new() -> Self {
        Self {
            bytes: [0; TEXT_BYTES],
            len: 0,
            overflow: false,
        }
    }
    fn append(&mut self, bytes: &[u8]) {
        if !self.overflow && self.len + bytes.len() <= TEXT_BYTES {
            self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
        } else {
            self.overflow = true;
        }
    }
    pub fn as_str(&self) -> Result<&str, Error> {
        if self.overflow {
            return Err(error("lexical_known_text_limit"));
        }
        std::str::from_utf8(&self.bytes[..self.len]).map_err(|_| error("lexical_utf8"))
    }
    pub fn identifier(&self) -> Identifier<'_> {
        if self.overflow || self.len > IDENTIFIER_BYTES {
            Identifier::UnknownLong
        } else {
            Identifier::Text(std::str::from_utf8(&self.bytes[..self.len]).expect("validated text"))
        }
    }
}
pub(crate) enum Identifier<'a> {
    Text(&'a str),
    UnknownLong,
}
impl Identifier<'_> {
    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::Text(s) => *s == name,
            Self::UnknownLong => false,
        }
    }
    // All storage schema identifiers are <=64B, asserted at struct entry and
    // field-table construction. Independent overflow state can never alias one.
    pub fn for_visitor(&self) -> &str {
        match self {
            Self::Text(s) => s,
            Self::UnknownLong => {
                "_________________________________________________________________"
            }
        }
    }
}
pub(crate) struct Cursor<'de> {
    pub bytes: &'de [u8],
    pub at: usize,
    pub depth: usize,
}
impl<'de> Cursor<'de> {
    pub fn new(raw: Raw<'de>) -> Result<Self, Error> {
        if raw.bytes.len() > INPUT_BYTES {
            return Err(error("lexical_input_limit"));
        }
        Ok(Self {
            bytes: raw.bytes,
            at: 0,
            depth: raw.depth,
        })
    }
    pub fn space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.at += 1;
        }
    }
    pub fn peek(&mut self) -> Option<u8> {
        self.space();
        self.bytes.get(self.at).copied()
    }
    pub fn eat(&mut self, b: u8) -> Result<(), Error> {
        if self.peek() != Some(b) {
            return Err(error("lexical_delimiter"));
        }
        self.at += 1;
        Ok(())
    }
    pub fn end(&mut self) -> Result<(), Error> {
        self.space();
        if self.at != self.bytes.len() {
            Err(error("lexical_trailing"))
        } else {
            Ok(())
        }
    }
    pub fn enter(&mut self) -> Result<(), Error> {
        if self.depth + 1 >= 128 {
            return Err(error("recursion limit exceeded"));
        }
        self.depth += 1;
        Ok(())
    }
    fn hex(&mut self) -> Result<u16, Error> {
        let mut n = 0u16;
        for _ in 0..4 {
            let b = *self
                .bytes
                .get(self.at)
                .ok_or_else(|| error("lexical_escape_eof"))?;
            self.at += 1;
            n = (n << 4)
                | match b {
                    b'0'..=b'9' => u16::from(b - b'0'),
                    b'a'..=b'f' => u16::from(b - b'a' + 10),
                    b'A'..=b'F' => u16::from(b - b'A' + 10),
                    _ => return Err(error("lexical_hex")),
                };
        }
        Ok(n)
    }
    pub fn text(&mut self, strict: bool) -> Result<Text, Error> {
        self.eat(b'"')?;
        let mut out = Text::new();
        loop {
            let start = self.at;
            while let Some(&b) = self.bytes.get(self.at) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.at += 1;
            }
            if strict {
                let run = &self.bytes[start..self.at];
                std::str::from_utf8(run).map_err(|_| error("lexical_utf8"))?;
                out.append(run);
            }
            let b = *self
                .bytes
                .get(self.at)
                .ok_or_else(|| error("lexical_string_eof"))?;
            self.at += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = *self
                        .bytes
                        .get(self.at)
                        .ok_or_else(|| error("lexical_escape_eof"))?;
                    self.at += 1;
                    let simple = match e {
                        b'"' | b'\\' | b'/' => Some(e),
                        b'b' => Some(8),
                        b'f' => Some(12),
                        b'n' => Some(b'\n'),
                        b'r' => Some(b'\r'),
                        b't' => Some(b'\t'),
                        b'u' => None,
                        _ => return Err(error("lexical_escape")),
                    };
                    if let Some(b) = simple {
                        if strict {
                            out.append(&[b]);
                        }
                    } else {
                        let first = self.hex()?;
                        if !strict {
                            continue;
                        }
                        let code = match first {
                            0xd800..=0xdbff => {
                                if self.bytes.get(self.at..self.at + 2) != Some(b"\\u") {
                                    return Err(error("lexical_surrogate"));
                                }
                                self.at += 2;
                                let second = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(error("lexical_surrogate"));
                                }
                                0x10000
                                    + ((u32::from(first) - 0xd800) << 10)
                                    + (u32::from(second) - 0xdc00)
                            }
                            0xdc00..=0xdfff => return Err(error("lexical_surrogate")),
                            _ => u32::from(first),
                        };
                        let c = char::from_u32(code).ok_or_else(|| error("lexical_codepoint"))?;
                        let mut encoded = [0; 4];
                        out.append(c.encode_utf8(&mut encoded).as_bytes());
                    }
                }
                _ => return Err(error("lexical_control_character")),
            }
        }
    }
    fn literal(&mut self, text: &[u8]) -> Result<(), Error> {
        if self.bytes.get(self.at..self.at + text.len()) != Some(text) {
            return Err(error("lexical_literal"));
        }
        self.at += text.len();
        Ok(())
    }
    pub fn number(&mut self) -> Result<&'de [u8], Error> {
        self.space();
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        match self.bytes.get(self.at) {
            Some(b'0') => {
                self.at += 1;
                if self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                    return Err(error("lexical_number_zero"));
                }
            }
            Some(b'1'..=b'9') => {
                self.at += 1;
                while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
            }
            _ => return Err(error("lexical_number")),
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            let digits = self.at;
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            if self.at == digits {
                return Err(error("lexical_fraction"));
            }
        }
        if self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b'e' | b'E'))
        {
            self.at += 1;
            if self
                .bytes
                .get(self.at)
                .is_some_and(|b| matches!(b, b'+' | b'-'))
            {
                self.at += 1;
            }
            let digits = self.at;
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            if self.at == digits {
                return Err(error("lexical_exponent"));
            }
        }
        Ok(&self.bytes[start..self.at])
    }
    pub fn raw(&mut self, scratch: &mut Scratch) -> Result<Raw<'de>, Error> {
        self.space();
        let start = self.at;
        self.skip(scratch, Mode::Ignore)?;
        Ok(Raw {
            bytes: &self.bytes[start..self.at],
            depth: self.depth,
        })
    }
    pub fn skip(&mut self, scratch: &mut Scratch, mode: Mode) -> Result<(), Error> {
        scratch.used = 0;
        let strict = mode == Mode::Content;
        let mut need_value = true;
        loop {
            if need_value {
                match self.peek().ok_or_else(|| error("lexical_value_eof"))? {
                    b'[' | b'{' => {
                        let object = self.bytes[self.at] == b'{';
                        self.at += 1;
                        if strict && self.depth + scratch.used + 1 >= 128 {
                            return Err(error("recursion limit exceeded"));
                        }
                        scratch.push(object)?;
                        if self.peek() == Some(if object { b'}' } else { b']' }) {
                            self.at += 1;
                            scratch.used -= 1;
                        } else {
                            if object {
                                self.text(strict)?;
                                self.eat(b':')?;
                            }
                            need_value = true;
                            continue;
                        }
                    }
                    b'"' => {
                        self.text(strict)?;
                    }
                    b'n' => self.literal(b"null")?,
                    b't' => self.literal(b"true")?,
                    b'f' => self.literal(b"false")?,
                    b'-' | b'0'..=b'9' => {
                        let number = self.number()?;
                        if strict {
                            let _: serde_json::Number = serde_json::from_slice(number)?;
                        }
                    }
                    _ => return Err(error("lexical_value")),
                }
            }
            let Some(object) = scratch.top() else {
                return Ok(());
            };
            match self.peek() {
                Some(b',') => {
                    self.at += 1;
                    if object {
                        self.text(strict)?;
                        self.eat(b':')?;
                    }
                    need_value = true;
                }
                Some(b'}') if object => {
                    self.at += 1;
                    scratch.used -= 1;
                    need_value = false;
                }
                Some(b']') if !object => {
                    self.at += 1;
                    scratch.used -= 1;
                    need_value = false;
                }
                _ => return Err(error("lexical_container_end")),
            }
        }
    }
}
pub(crate) fn validate_content(raw: Raw<'_>, scratch: &mut Scratch) -> Result<(), Error> {
    let mut c = Cursor::new(raw)?;
    c.skip(scratch, Mode::Content)?;
    c.end()
}

pub(crate) struct Array<'de> {
    cursor: Cursor<'de>,
    first: bool,
    done: bool,
}
impl<'de> Array<'de> {
    pub fn new(raw: Raw<'de>) -> Result<Self, Error> {
        let mut cursor = Cursor::new(raw)?;
        cursor.enter()?;
        cursor.eat(b'[')?;
        Ok(Self {
            cursor,
            first: true,
            done: false,
        })
    }
    pub fn next(&mut self, scratch: &mut Scratch) -> Result<Option<Raw<'de>>, Error> {
        if self.done {
            return Ok(None);
        }
        if self.cursor.peek() == Some(b']') {
            self.cursor.at += 1;
            self.cursor.end()?;
            self.done = true;
            return Ok(None);
        }
        if !self.first {
            self.cursor.eat(b',')?;
        }
        self.first = false;
        Ok(Some(self.cursor.raw(scratch)?))
    }
}
pub(crate) struct Fields<'de, const N: usize> {
    pub names: &'static [&'static str; N],
    pub values: [Option<Raw<'de>>; N],
}
impl<'de, const N: usize> Fields<'de, N> {
    pub fn parse(
        raw: Raw<'de>,
        names: &'static [&'static str; N],
        ignore_unknown: bool,
        scratch: &mut Scratch,
    ) -> Result<Self, Error> {
        if names.iter().any(|n| n.len() > IDENTIFIER_BYTES) {
            return Err(error("lexical_schema_identifier"));
        }
        let mut result = Self {
            names,
            values: [None; N],
        };
        let mut c = Cursor::new(raw)?;
        if c.peek() == Some(b'[') {
            let mut a = Array::new(raw)?;
            for slot in &mut result.values {
                *slot = a.next(scratch)?;
                if slot.is_none() {
                    return Ok(result);
                }
            }
            if a.next(scratch)?.is_some() {
                return Err(error("lexical_extra_field"));
            }
            return Ok(result);
        }
        c.enter()?;
        c.eat(b'{')?;
        let mut first = true;
        loop {
            if c.peek() == Some(b'}') {
                c.at += 1;
                c.end()?;
                return Ok(result);
            }
            if !first {
                c.eat(b',')?;
            }
            first = false;
            let key = c.text(true)?;
            c.eat(b':')?;
            let index = names.iter().position(|name| key.identifier().matches(name));
            match index {
                Some(i) => {
                    if result.values[i].is_some() {
                        return Err(error("lexical_duplicate_field"));
                    }
                    result.values[i] = Some(c.raw(scratch)?);
                }
                None if ignore_unknown => {
                    c.skip(scratch, Mode::Ignore)?;
                }
                None => return Err(error("lexical_unknown_field")),
            }
        }
    }
    pub fn raw(&self, index: usize) -> Result<Raw<'de>, Error> {
        self.values[index].ok_or_else(|| de::Error::missing_field(self.names[index]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compare(bytes: &[u8], mode: Mode, scratch: &mut Scratch) {
        let old = match mode {
            Mode::Ignore => serde_json::from_slice::<serde::de::IgnoredAny>(bytes).is_ok(),
            Mode::Content => serde_json::from_slice::<serde_json::Value>(bytes).is_ok(),
        };
        let mut c = Cursor::new(Raw::root(bytes)).unwrap();
        let result = c.skip(scratch, mode).and_then(|()| c.end());
        assert_eq!(
            old,
            result.is_ok(),
            "mode={} input={} new={result:?}",
            if mode == Mode::Ignore {
                "ignore"
            } else {
                "content"
            },
            String::from_utf8_lossy(&bytes[..bytes.len().min(160)])
        );
    }
    #[test]
    fn ignore_and_content_keep_distinct_old_scalar_grammars() {
        let mut scratch = Scratch::new().unwrap();
        let identity = scratch.identity();
        for bytes in [
            b"null".as_slice(),
            b"true",
            b"false",
            b"0",
            b"-0",
            b"1e400",
            b"1e-400",
            b"18446744073709551616",
            b"01",
            b"1.",
            b"1e+",
            b"1e--2",
            br#""\ud800""#,
            br#""\udc00""#,
            br#""\ud800\udc00""#,
            br#""\ud800\u0061""#,
            br#""\q""#,
            b"\"\xff\"",
            b"[0,]",
            b"{\"a\":1,}",
            b"[{}]",
            b"{\"a\":[1,2,3]}",
            b"[}",
            b"{]",
            b"false true",
        ] {
            compare(bytes, Mode::Ignore, &mut scratch);
            compare(bytes, Mode::Content, &mut scratch);
        }
        assert_eq!(scratch.identity(), identity);
    }
    #[test]
    fn full_16mib_ignored_nesting_fits_one_initialized_bit_stack_and_overflow_recovers() {
        let mut scratch = Scratch::new().unwrap();
        let identity = scratch.identity();
        assert_eq!(identity.1, 1024 * 1024);
        // Exact original file ceiling: nested empty arrays need two bytes per level.
        let depth = INPUT_BYTES / 2;
        let mut input = vec![b'['; INPUT_BYTES];
        input[depth..].fill(b']');
        compare(&input, Mode::Ignore, &mut scratch);
        input.fill(b'[');
        compare(&input, Mode::Ignore, &mut scratch);
        compare(b"{\"a\":[1,2]}", Mode::Ignore, &mut scratch);
        assert_eq!(scratch.identity(), identity);
    }
    #[test]
    fn long_escaped_keys_do_not_alias_known_identifiers_or_skip_late_errors() {
        let mut scratch = Scratch::new().unwrap();
        let identity = scratch.identity();
        let repeat = (INPUT_BYTES - 64) / 6;
        let valid = format!("{{\"{}\":0,\"kind\":1}}", "\\u0061".repeat(repeat));
        let fields =
            Fields::parse(Raw::root(valid.as_bytes()), &["kind"], true, &mut scratch).unwrap();
        assert_eq!(
            crate::metadata::reader::decode::<u32>(fields.raw(0).unwrap(), &mut scratch).unwrap(),
            1
        );
        assert!(
            Fields::parse(Raw::root(valid.as_bytes()), &["kind"], false, &mut scratch).is_err()
        );
        let invalid = valid.replacen("\":0", "\\uD800\":0", 1);
        assert!(
            Fields::parse(Raw::root(invalid.as_bytes()), &["kind"], true, &mut scratch).is_err()
        );
        let duplicate = br#"{"kind":0,"k\u0069nd":1}"#;
        assert!(Fields::parse(Raw::root(duplicate), &["kind"], true, &mut scratch).is_err());
        assert_eq!(scratch.identity(), identity);
    }
    #[test]
    fn finite_known_values_reject_long_wrong_types_without_old_string_fallback() {
        let mut scratch = Scratch::new().unwrap();
        let identity = scratch.identity();
        let value = format!("\"{}\"", "\\n".repeat((INPUT_BYTES - 2) / 2));
        compare(value.as_bytes(), Mode::Ignore, &mut scratch);
        compare(value.as_bytes(), Mode::Content, &mut scratch);
        assert!(
            crate::metadata::reader::decode::<bool>(Raw::root(value.as_bytes()), &mut scratch)
                .is_err()
        );
        assert!(
            crate::metadata::reader::decode::<()>(Raw::root(value.as_bytes()), &mut scratch)
                .is_err()
        );
        assert_eq!(scratch.identity(), identity);
    }
}
