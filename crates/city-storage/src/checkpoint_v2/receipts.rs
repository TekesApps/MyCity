//! Validate an archive one typed record at a time; no body image or receipt Vec.
use super::*;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::fmt;
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Schema,
    GenesisSha256,
    First,
    Last,
    Previous,
    Receipts,
}
struct Extra;
impl<'de> DeserializeSeed<'de> for Extra {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        Err(de::Error::custom("v2_receipt_extra"))
    }
}
pub(super) fn operations<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u64>, D::Error> {
    struct Operations;
    impl<'de> Visitor<'de> for Operations {
        type Value = Vec<u64>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("bounded receipt index")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            out.try_reserve_exact(MAX_RECEIPTS)
                .map_err(de::Error::custom)?;
            if out.capacity() != MAX_RECEIPTS {
                return Err(de::Error::custom("v2_index_backing"));
            }
            for _ in 0..MAX_RECEIPTS {
                match a.next_element()? {
                    Some(n) => out.push(n),
                    None => return Ok(out),
                }
            }
            a.next_element_seed(Extra)?;
            Ok(out)
        }
    }
    d.deserialize_seq(Operations)
}
struct Records<'a, F> {
    index: &'a ReceiptIndex,
    visit: &'a mut F,
}
impl<'de, F: FnMut(usize, JournalRecord)> DeserializeSeed<'de> for Records<'_, F> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, F: FnMut(usize, JournalRecord)> Visitor<'de> for Records<'_, F> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("indexed receipts")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        for (n, id) in self.index.operations.iter().enumerate() {
            let record: JournalRecord = a
                .next_element()?
                .ok_or_else(|| de::Error::custom("v2_receipt_short"))?;
            if record.operation_id != *id {
                return Err(de::Error::custom("v2_receipt_operation"));
            }
            (self.visit)(n, record);
        }
        a.next_element_seed(Extra)?;
        Ok(())
    }
}
struct Body<'a, F> {
    index: &'a ReceiptIndex,
    visit: &'a mut F,
}
impl<'de, F: FnMut(usize, JournalRecord)> DeserializeSeed<'de> for Body<'_, F> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de, F: FnMut(usize, JournalRecord)> Visitor<'de> for Body<'_, F> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("complete indexed receipt body")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut fields = 0u8;
        while let Some(field) = a.next_key::<Field>()? {
            let bit = match field {
                Field::Schema => 1,
                Field::GenesisSha256 => 2,
                Field::First => 4,
                Field::Last => 8,
                Field::Previous => 16,
                Field::Receipts => 32,
            };
            if fields & bit != 0 {
                return Err(de::Error::custom("v2_receipt_duplicate"));
            }
            fields |= bit;
            let valid = match field {
                Field::Schema => a.next_value::<String>()? == "mycity-receipts-v2",
                Field::GenesisSha256 => a.next_value::<String>()? == self.index.genesis_sha256,
                Field::First => a.next_value::<u64>()? == self.index.first,
                Field::Last => a.next_value::<u64>()? == self.index.last,
                Field::Previous => a.next_value::<Option<Reference>>()? == self.index.previous,
                Field::Receipts => {
                    a.next_value_seed(Records {
                        index: self.index,
                        visit: self.visit,
                    })?;
                    true
                }
            };
            if !valid {
                return Err(de::Error::custom("v2_receipt_index_content"));
            }
        }
        // serde's old Option<Reference> field permits an omitted null previous.
        if fields | 16 != 63 || (fields & 16 == 0 && self.index.previous.is_some()) {
            return Err(de::Error::custom("v2_receipt_missing_field"));
        }
        Ok(())
    }
}
pub(super) fn scan(
    dir: &Path,
    reference: &Reference,
    index: &ReceiptIndex,
    mut visit: impl FnMut(usize, JournalRecord),
) -> Result<(), Error> {
    let identity = stream::Identity {
        bytes: index.body_bytes,
        sha256: index.body_sha256.clone(),
    };
    stream::verified(
        &dir.join(body_name(reference.generation)),
        &identity,
        CHECKPOINT_LIMIT,
        |reader| {
            let guarded = mycity_core::fixed_restore::JsonReader::with_string_limit(reader, 384);
            let mut decoder = serde_json::Deserializer::from_reader(guarded);
            Body {
                index,
                visit: &mut visit,
            }
            .deserialize(&mut decoder)?;
            decoder.end()?;
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycity_core::transaction::TxError;
    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Dir, Reference, ReceiptIndex, JournalRecord) {
        let dir = Dir(std::env::temp_dir().join(format!(
            "mycity-receipts-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&dir.0).unwrap();
        let record = JournalRecord {
            actor: 0,
            emergency_eligible: false,
            operation_id: 1,
            expected_revision: 0,
            payload_sha256: [0; 32],
            outcome: Err(TxError::StaleRevision),
            actions: [None; 16],
        };
        let index = ReceiptIndex {
            schema: "mycity-receipt-index-v2".into(),
            genesis_sha256: "a".repeat(64),
            first: 1,
            last: 1,
            body_bytes: 0,
            body_sha256: String::new(),
            operations: vec![1],
            previous: None,
        };
        (
            dir,
            Reference {
                generation: 1,
                sha256: String::new(),
            },
            index,
            record,
        )
    }
    fn write(dir: &Path, index: &mut ReceiptIndex, bytes: &[u8]) {
        fs::write(dir.join(body_name(1)), bytes).unwrap();
        index.body_bytes = bytes.len() as u64;
        index.body_sha256 = sha256(bytes);
    }
    #[test]
    fn borrowed_receipt_wire_matches_owned_legacy_and_arbitrary_field_order() {
        let (dir, reference, mut index, record) = fixture();
        #[derive(Serialize)]
        struct Old {
            schema: String,
            genesis_sha256: String,
            first: u64,
            last: u64,
            previous: Option<Reference>,
            receipts: Vec<JournalRecord>,
        }
        let records = [record];
        let view = ReceiptBody {
            schema: "mycity-receipts-v2",
            genesis_sha256: &index.genesis_sha256,
            first: 1,
            last: 1,
            previous: None,
            receipts: &records,
        };
        let bytes = json(&view).unwrap();
        let old = Old {
            schema: view.schema.into(),
            genesis_sha256: view.genesis_sha256.into(),
            first: 1,
            last: 1,
            previous: None,
            receipts: records.to_vec(),
        };
        assert_eq!(bytes, json(&old).unwrap());
        write(&dir.0, &mut index, &bytes);
        let mut seen = Vec::new();
        scan(&dir.0, &reference, &index, |n, r| {
            seen.push((n, r.operation_id))
        })
        .unwrap();
        assert_eq!(seen, [(0, 1)]);
        let mut sorted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        sorted.as_object_mut().unwrap().remove("previous"); // Old serde Option accepts missing null.
        write(&dir.0, &mut index, &json(&sorted).unwrap());
        scan(&dir.0, &reference, &index, |_, _| {}).unwrap();
    }
    #[test]
    fn authenticated_but_malformed_body_tail_metadata_and_count_cannot_pass() {
        let (dir, reference, mut index, record) = fixture();
        let records = [record];
        let bytes = json(&ReceiptBody {
            schema: "mycity-receipts-v2",
            genesis_sha256: &index.genesis_sha256,
            first: 1,
            last: 1,
            previous: None,
            receipts: &records,
        })
        .unwrap();
        for bad in [
            [bytes.as_slice(), b" {}"].concat(),
            bytes[..bytes.len() - 1].to_vec(),
            [bytes[..bytes.len() - 1].as_ref(), b",\"first\":1}"].concat(),
            [bytes[..bytes.len() - 1].as_ref(), b",\"unknown\":0}"].concat(),
        ] {
            write(&dir.0, &mut index, &bad);
            assert!(scan(&dir.0, &reference, &index, |_, _| {}).is_err());
        }
        let base: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for mutate in [
            |v: &mut serde_json::Value| v["first"] = serde_json::json!(2),
            |v: &mut serde_json::Value| v["receipts"] = serde_json::json!([]),
            |v: &mut serde_json::Value| v["receipts"][0]["operation_id"] = serde_json::json!(7),
            |v: &mut serde_json::Value| {
                v["receipts"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"huge":[[[0]]]}))
            },
            |v: &mut serde_json::Value| v["schema"] = serde_json::json!("x".repeat(4096)),
        ] {
            let mut bad = base.clone();
            mutate(&mut bad);
            write(&dir.0, &mut index, &json(&bad).unwrap());
            assert!(scan(&dir.0, &reference, &index, |_, _| {}).is_err());
        }
    }
    #[test]
    fn index_overflow_is_rejected_before_extra_record_materializes() {
        let mut text = format!("[{}", vec!["1"; MAX_RECEIPTS].join(","));
        text.push_str(",[[[[[[0]]]]]]]");
        let mut decoder = serde_json::Deserializer::from_str(&text);
        assert!(operations(&mut decoder)
            .unwrap_err()
            .to_string()
            .contains("v2_receipt_extra"));
    }
}
