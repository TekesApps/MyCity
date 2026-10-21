//! Fixed L1 diagnostic table decoder. No gameplay World authority.
use super::{Stub, Table};
use mycity_core::fixed_restore::{JsonReader, Records};
use serde::de::{self, DeserializeSeed, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::{fmt, io::Read};
#[derive(Clone, Copy, Deserialize, PartialEq)]
enum Kind {
    W64,
    W128,
    W256,
    PathRefs,
}
#[derive(Deserialize)]
#[serde(field_identifier)]
enum Field {
    #[serde(rename = "record_type")]
    Kind,
    #[serde(rename = "records")]
    Records,
}
struct TableSeed<'a> {
    expected: &'a Table,
}
impl<'de> DeserializeSeed<'de> for TableSeed<'_> {
    type Value = Table;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Table, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for TableSeed<'_> {
    type Value = Table;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("frozen typed layout table")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Table, A::Error> {
        let expected_kind = match self.expected {
            Table::W64(_) => Kind::W64,
            Table::W128(_) => Kind::W128,
            Table::W256(_) => Kind::W256,
            Table::PathRefs(_) => Kind::PathRefs,
        };
        let mut kind = None;
        let mut records = None;
        while let Some(field) = map.next_key::<Field>()? {
            match field {
                Field::Kind => {
                    if kind.is_some() {
                        return Err(de::Error::duplicate_field("record_type"));
                    }
                    let received = map.next_value::<Kind>()?;
                    if received != expected_kind {
                        return Err(de::Error::custom("layout frozen record type"));
                    }
                    kind = Some(received);
                }
                Field::Records => {
                    if records.is_some() {
                        return Err(de::Error::duplicate_field("records"));
                    }
                    // The frozen expected kind/count supplies the allocation bounds even
                    // when records precedes record_type. No serde Content buffering.
                    macro_rules! records {
                        ($v:ident,$t:ty,$count:expr) => {
                            Table::$v(map.next_value_seed(Records::<$t>::new($count))?)
                        };
                    }
                    records = Some(match self.expected {
                        Table::W64(v) => records!(W64, Stub<5>, v.len()),
                        Table::W128(v) => records!(W128, Stub<13>, v.len()),
                        Table::W256(v) => records!(W256, Stub<29>, v.len()),
                        Table::PathRefs(v) => records!(PathRefs, u32, v.len()),
                    });
                }
            }
        }
        if kind.is_none() {
            return Err(de::Error::missing_field("record_type"));
        }
        records.ok_or_else(|| de::Error::missing_field("records"))
    }
}
pub(super) fn decode(reader: impl Read, expected: &Table) -> Result<Table, String> {
    let mut d = serde_json::Deserializer::from_reader(JsonReader::new(reader));
    let result = TableSeed { expected }
        .deserialize(&mut d)
        .map_err(|e| e.to_string())?;
    d.end().map_err(|e| e.to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_variants_restore_exactly_and_accept_records_before_tag() {
        for (width, tag) in [(64, "W64"), (128, "W128"), (256, "W256"), (4, "PathRefs")] {
            let expected = Table::new(3, width, 2, 2).unwrap();
            let bytes = serde_json::to_vec(&expected).unwrap();
            let restored = decode(&bytes[..], &expected).unwrap();
            assert!(restored == expected);
            assert_eq!(restored.owned(), expected.owned());
            assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
            let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let swapped = format!(
                "{{\"records\":{},\"record_type\":\"{}\"}}",
                fields["records"], tag
            );
            assert!(decode(swapped.as_bytes(), &expected).unwrap() == expected);
        }
    }
    #[test]
    fn duplicate_missing_wrong_fields_extra_records_and_tokens_reject() {
        let expected = Table::PathRefs(vec![1, 2]);
        for text in [
            r#"{"record_type":"PathRefs","records":[1]}"#,
            r#"{"record_type":"PathRefs","records":[1,2,{}]}"#,
            r#"{"records":[1,2]}"#,
            r#"{"record_type":"W64","records":[1,2]}"#,
            r#"{"records":[1,2],"record_type":"PathRefs","record_type":"PathRefs"}"#,
            r#"{"record_type":"PathRefs","records":[1,2],"records":[1,2]}"#,
            r#"{"record_type":"PathRefs","records":[1,2],"extra":0}"#,
            r#"{"record_type":"PathRefs","records":[1,2]} false"#,
        ] {
            assert!(decode(text.as_bytes(), &expected).is_err(), "{text}");
        }
        let long = format!("{{\"{}\":0}}", "x".repeat(2000));
        assert!(decode(long.as_bytes(), &expected).is_err());
    }
}
