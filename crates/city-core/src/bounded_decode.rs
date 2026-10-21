//! General model decoding into trusted capacities, independent of file size hints.
use serde::{
    de::{self, DeserializeSeed, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, io::Read};
pub(crate) struct RejectExtra(pub &'static str);
impl<'de> DeserializeSeed<'de> for RejectExtra {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        Err(de::Error::custom(self.0))
    }
}
pub(crate) struct VectorSeed<'a, T> {
    pub target: &'a mut Vec<T>,
    pub limit: usize,
}
impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for VectorSeed<'_, T> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T: Deserialize<'de>> Visitor<'de> for VectorSeed<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "at most {} trusted records", self.limit)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        if self.target.capacity() != self.limit || !self.target.is_empty() {
            return Err(de::Error::custom("model_decode_backing"));
        }
        for _ in 0..self.limit {
            match seq.next_element()? {
                Some(v) => self.target.push(v),
                None => return Ok(()),
            }
        }
        seq.next_element_seed(RejectExtra("model_rows_extra_record"))?;
        Ok(())
    }
}
pub(crate) fn text64<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    struct Text;
    impl Visitor<'_> for Text {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("at most64 decoded bytes")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
            if s.len() > 64 {
                return Err(E::custom("model_text_limit"));
            }
            Ok(s.to_owned())
        }
    }
    d.deserialize_str(Text)
}
/// Lexical guard runs before serde scratch or any domain records materialize.
/// 64 ASCII bytes may be escaped as 384 JSON bytes; native guard remains128.
pub(crate) fn guard(bytes: &[u8]) -> Result<(), String> {
    let mut r = crate::fixed_restore::JsonReader::with_string_limit(bytes, 384);
    let mut page = [0; 4096];
    while r.read(&mut page).map_err(|e| e.to_string())? != 0 {}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexical_guards_precede_scratch_and_keep_escaped_known_schema_valid() {
        let raw =
            std::str::from_utf8(include_bytes!("../../../fixtures/m0-bootstrap.json")).unwrap();
        use std::fmt::Write;
        let escaped = crate::SCHEMA.bytes().fold(String::new(), |mut out, b| {
            write!(&mut out, "\\u{b:04x}").unwrap();
            out
        });
        assert!(crate::parse_bootstrap(raw.replace(crate::SCHEMA, &escaped).as_bytes()).is_ok());
        let long = raw.replace(crate::SCHEMA, &"x".repeat(4096));
        assert!(crate::parse_bootstrap(long.as_bytes())
            .unwrap_err()
            .contains("fixed_restore_string"));
        let decoded = raw.replace(crate::SCHEMA, &"x".repeat(65));
        assert!(crate::parse_bootstrap(decoded.as_bytes())
            .unwrap_err()
            .contains("model_text_limit"));
        let nested = format!("{}0{}", "[".repeat(129), "]".repeat(129));
        assert!(guard(nested.as_bytes())
            .unwrap_err()
            .contains("fixed_restore_nesting"));
    }
}
