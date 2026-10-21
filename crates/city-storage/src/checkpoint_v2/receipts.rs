//! Validate an archive one typed record at a time; no body image or receipt Vec.
use super::*;
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use std::fmt;
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
pub(super) fn scan(
    dir: &Path,
    reference: &Reference,
    index: &ReceiptIndex,
    meta: &mut MetadataWorkspace,
    mut visit: impl FnMut(usize, JournalRecord),
) -> Result<(), Error> {
    use crate::metadata::lexical::{Array, Cursor, Fields, Raw};
    meta.file.read(
        &dir.join(body_name(reference.generation)),
        CHECKPOINT_LIMIT as usize,
    )?;
    let bytes = meta.file.as_slice();
    if bytes.len() as u64 != index.body_bytes || sha256(bytes) != index.body_sha256 {
        return Err(Error::Invalid("v2_receipt_body_identity"));
    }
    let raw = Raw::root(bytes);
    let mut cursor = Cursor::new(raw)?;
    if cursor.peek() != Some(b'{') {
        return Err(Error::Invalid("v2_receipt_body_object"));
    }
    let fields = Fields::parse(
        raw,
        &[
            "schema",
            "genesis_sha256",
            "first",
            "last",
            "previous",
            "receipts",
        ],
        false,
        &mut meta.parser,
    )?;
    let schema: String = fields.get(0, &mut meta.parser)?;
    let genesis: String = fields.get(1, &mut meta.parser)?;
    let first: u64 = fields.get(2, &mut meta.parser)?;
    let last: u64 = fields.get(3, &mut meta.parser)?;
    let previous: Option<Reference> = fields.optional(4, &mut meta.parser)?;
    if schema != "mycity-receipts-v2"
        || genesis != index.genesis_sha256
        || first != index.first
        || last != index.last
        || previous != index.previous
    {
        return Err(Error::Invalid("v2_receipt_index_content"));
    }
    let mut array = Array::new(fields.raw(5)?)?;
    for (n, id) in index.operations.iter().enumerate() {
        let raw = array
            .next(&mut meta.parser)?
            .ok_or(Error::Invalid("v2_receipt_short"))?;
        let record = crate::checkpoint::receipt_decode::decode_raw(raw, &mut meta.parser)?;
        if record.operation_id != *id {
            return Err(Error::Invalid("v2_receipt_operation"));
        }
        visit(n, record);
    }
    if array.next(&mut meta.parser)?.is_some() {
        return Err(Error::Invalid("v2_receipt_extra"));
    }
    Ok(())
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
        scan(
            &dir.0,
            &reference,
            &index,
            &mut MetadataWorkspace::new().unwrap(),
            |n, r| seen.push((n, r.operation_id)),
        )
        .unwrap();
        assert_eq!(seen, [(0, 1)]);
        let mut sorted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        sorted.as_object_mut().unwrap().remove("previous"); // Old serde Option accepts missing null.
        write(&dir.0, &mut index, &json(&sorted).unwrap());
        scan(
            &dir.0,
            &reference,
            &index,
            &mut MetadataWorkspace::new().unwrap(),
            |_, _| {},
        )
        .unwrap();
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
            assert!(scan(
                &dir.0,
                &reference,
                &index,
                &mut MetadataWorkspace::new().unwrap(),
                |_, _| {}
            )
            .is_err());
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
            assert!(scan(
                &dir.0,
                &reference,
                &index,
                &mut MetadataWorkspace::new().unwrap(),
                |_, _| {}
            )
            .is_err());
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
