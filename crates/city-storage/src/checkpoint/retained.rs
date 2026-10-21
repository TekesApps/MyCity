//! Decode the original v1 wire into retained bytes/receipt tables, not a file-sized DOM.
use super::*;
use crate::metadata::{self, ByteSeed, RowsSeed};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::fmt;

#[derive(Deserialize)]
struct Label(#[serde(deserialize_with = "metadata::text::<_,64>")] String);
pub(crate) struct Header {
    pub sequence: u64,
    pub chain: String,
    pub genesis_sha256: String,
    pub previous: Option<Reference>,
}
pub(crate) struct IndexHeader {
    pub data_sha256: String,
    pub previous: Option<Reference>,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum CpField {
    Schema,
    Sequence,
    Chain,
    GenesisSha256,
    Engine,
    Receipts,
    Previous,
}
struct CpSeed<'a> {
    engine: &'a mut metadata::Bytes,
    receipts: &'a mut Vec<JournalRecord>,
}
struct ReceiptSeed<'a>(&'a mut Vec<JournalRecord>);
impl<'de> DeserializeSeed<'de> for ReceiptSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for ReceiptSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded original WAL receipts")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.0.clear();
        while self.0.len() < self.0.capacity() {
            // Borrow from the retained file, then decode known fields directly.
            // Legacy JSON may contain arbitrary whitespace and ignored fields;
            // its original 16MiB file bound is not the 32KiB WAL frame bound.
            let Some(raw) = seq.next_element::<&serde_json::value::RawValue>()? else {
                return Ok(());
            };
            self.0
                .push(super::receipt_decode::decode(raw.get()).map_err(de::Error::custom)?);
        }
        seq.next_element_seed(metadata::RejectExtra)?;
        Ok(())
    }
}
impl<'de> DeserializeSeed<'de> for CpSeed<'_> {
    type Value = Header;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Header, D::Error> {
        d.deserialize_struct(
            "Checkpoint",
            &[
                "schema",
                "sequence",
                "chain",
                "genesis_sha256",
                "engine",
                "receipts",
                "previous",
            ],
            self,
        )
    }
}
impl<'de> Visitor<'de> for CpSeed<'_> {
    type Value = Header;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("original v1 checkpoint")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Header, A::Error> {
        let schema: Label = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("schema"))?;
        let sequence = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("sequence"))?;
        let chain: Label = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("chain"))?;
        let genesis: Label = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("genesis_sha256"))?;
        seq.next_element_seed(ByteSeed(
            self.engine,
            mycity_core::transaction::ENGINE_BYTE_BUDGET,
        ))?
        .ok_or_else(|| de::Error::missing_field("engine"))?;
        seq.next_element_seed(ReceiptSeed(self.receipts))?
            .ok_or_else(|| de::Error::missing_field("receipts"))?;
        let previous = seq.next_element()?.unwrap_or(None);
        seq.next_element_seed(metadata::RejectExtra)?;
        if schema.0 != "mycity-m0-checkpoint-v1" {
            return Err(de::Error::custom("checkpoint_schema_or_receipts"));
        }
        Ok(Header {
            sequence,
            chain: chain.0,
            genesis_sha256: genesis.0,
            previous,
        })
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Header, A::Error> {
        let mut seen = [false; 7];
        let mut schema = None;
        let mut sequence = None;
        let mut chain = None;
        let mut genesis = None;
        let mut previous = None;
        while let Some(field) = map.next_key::<CpField>()? {
            let (slot, name) = match field {
                CpField::Schema => (0, "schema"),
                CpField::Sequence => (1, "sequence"),
                CpField::Chain => (2, "chain"),
                CpField::GenesisSha256 => (3, "genesis_sha256"),
                CpField::Engine => (4, "engine"),
                CpField::Receipts => (5, "receipts"),
                CpField::Previous => (6, "previous"),
            };
            if seen[slot] {
                return Err(de::Error::duplicate_field(name));
            }
            seen[slot] = true;
            match field {
                CpField::Schema => schema = Some(map.next_value::<Label>()?.0),
                CpField::Sequence => sequence = Some(map.next_value()?),
                CpField::Chain => chain = Some(map.next_value::<Label>()?.0),
                CpField::GenesisSha256 => genesis = Some(map.next_value::<Label>()?.0),
                CpField::Engine => map.next_value_seed(ByteSeed(
                    self.engine,
                    mycity_core::transaction::ENGINE_BYTE_BUDGET,
                ))?,
                CpField::Receipts => map.next_value_seed(ReceiptSeed(self.receipts))?,
                CpField::Previous => previous = map.next_value()?,
            }
        }
        if schema.as_deref() != Some("mycity-m0-checkpoint-v1") || !seen[4] || !seen[5] {
            return Err(de::Error::custom("checkpoint_schema_or_receipts"));
        }
        Ok(Header {
            sequence: sequence.ok_or_else(|| de::Error::missing_field("sequence"))?,
            chain: chain.ok_or_else(|| de::Error::missing_field("chain"))?,
            genesis_sha256: genesis.ok_or_else(|| de::Error::missing_field("genesis_sha256"))?,
            previous,
        })
    }
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum IndexField {
    DataSha256,
    Operations,
    Previous,
}
struct IndexSeed<'a>(&'a mut Vec<u64>);
impl<'de> DeserializeSeed<'de> for IndexSeed<'_> {
    type Value = IndexHeader;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<IndexHeader, D::Error> {
        d.deserialize_struct("Index", &["data_sha256", "operations", "previous"], self)
    }
}
impl<'de> Visitor<'de> for IndexSeed<'_> {
    type Value = IndexHeader;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("original checkpoint index")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<IndexHeader, A::Error> {
        let data: Label = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("data_sha256"))?;
        seq.next_element_seed(RowsSeed(self.0))?
            .ok_or_else(|| de::Error::missing_field("operations"))?;
        let previous = seq.next_element()?.unwrap_or(None);
        seq.next_element_seed(metadata::RejectExtra)?;
        Ok(IndexHeader {
            data_sha256: data.0,
            previous,
        })
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<IndexHeader, A::Error> {
        let mut seen = [false; 3];
        let mut data = None;
        let mut previous = None;
        while let Some(field) = map.next_key::<IndexField>()? {
            let (slot, name) = match field {
                IndexField::DataSha256 => (0, "data_sha256"),
                IndexField::Operations => (1, "operations"),
                IndexField::Previous => (2, "previous"),
            };
            if seen[slot] {
                return Err(de::Error::duplicate_field(name));
            }
            seen[slot] = true;
            match field {
                IndexField::DataSha256 => data = Some(map.next_value::<Label>()?.0),
                IndexField::Operations => map.next_value_seed(RowsSeed(self.0))?,
                IndexField::Previous => previous = map.next_value()?,
            }
        }
        if !seen[1] {
            return Err(de::Error::missing_field("operations"));
        }
        Ok(IndexHeader {
            data_sha256: data.ok_or_else(|| de::Error::missing_field("data_sha256"))?,
            previous,
        })
    }
}
pub(crate) fn load_index(
    dir: &Path,
    r: &Reference,
    meta: &mut MetadataWorkspace,
) -> Result<IndexHeader, Error> {
    meta.operations.clear();
    meta.small
        .read(&dir.join(index_name(r.generation)), 32 * 1024)?;
    let bytes = meta.small.as_slice();
    if sha256(bytes) != r.sha256 {
        return Err(Error::Invalid("checkpoint_index_checksum"));
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let index = IndexSeed(&mut meta.operations).deserialize(&mut d)?;
    d.end()?;
    if index
        .previous
        .as_ref()
        .is_some_and(|p| p.generation >= r.generation)
    {
        return Err(Error::Invalid("checkpoint_index_limits"));
    }
    Ok(index)
}
pub(crate) fn load(
    dir: &Path,
    r: &Reference,
    meta: &mut MetadataWorkspace,
) -> Result<Header, Error> {
    meta.receipts.clear();
    let index = load_index(dir, r, meta)?;
    meta.file
        .read(&dir.join(cp_name(r.generation)), CHECKPOINT_LIMIT as usize)?;
    let bytes = meta.file.as_slice();
    if sha256(bytes) != index.data_sha256 {
        return Err(Error::Invalid("checkpoint_checksum"));
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let cp = CpSeed {
        engine: &mut meta.engine,
        receipts: &mut meta.receipts,
    }
    .deserialize(&mut d)?;
    d.end()?;
    if cp.previous != index.previous
        || !meta
            .receipts
            .iter()
            .map(|r| r.operation_id)
            .eq(meta.operations.iter().copied())
    {
        return Err(Error::Invalid("checkpoint_index_content"));
    }
    Ok(cp)
}
pub(crate) fn verify_archive(
    dir: &Path,
    r: &Reference,
    genesis: &str,
    meta: &mut MetadataWorkspace,
) -> Result<(), Error> {
    let mut cursor = Some(r.clone());
    let mut ceiling = u64::MAX;
    while let Some(r) = cursor {
        let cp = load(dir, &r, meta)?;
        if cp.genesis_sha256 != genesis || cp.sequence > ceiling {
            return Err(Error::Invalid("receipt_archive_chain"));
        }
        ceiling = cp.sequence;
        cursor = cp.previous;
    }
    Ok(())
}
