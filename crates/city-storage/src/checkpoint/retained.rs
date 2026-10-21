//! Decode the original v1 wire into retained bytes/receipt tables, not a file-sized DOM.
use super::*;
use crate::metadata::{
    lexical::{Array, Fields, Raw},
    reader, ByteSeed, RowsSeed,
};

#[derive(Deserialize)]
struct Label(#[serde(deserialize_with = "crate::metadata::text::<_,64>")] String);
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
    let fields = Fields::parse(
        Raw::root(bytes),
        &["data_sha256", "operations", "previous"],
        false,
        &mut meta.parser,
    )?;
    let data: Label = fields.get(0, &mut meta.parser)?;
    reader::seed(
        fields.raw(1)?,
        RowsSeed(&mut meta.operations),
        &mut meta.parser,
    )?;
    let index = IndexHeader {
        data_sha256: data.0,
        previous: fields.optional(2, &mut meta.parser)?,
    };
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
    let fields = Fields::parse(
        Raw::root(bytes),
        &[
            "schema",
            "sequence",
            "chain",
            "genesis_sha256",
            "engine",
            "receipts",
            "previous",
        ],
        false,
        &mut meta.parser,
    )?;
    let schema: Label = fields.get(0, &mut meta.parser)?;
    if schema.0 != "mycity-m0-checkpoint-v1" {
        return Err(Error::Invalid("checkpoint_schema_or_receipts"));
    }
    let sequence = fields.get(1, &mut meta.parser)?;
    let chain: Label = fields.get(2, &mut meta.parser)?;
    let genesis: Label = fields.get(3, &mut meta.parser)?;
    reader::seed(
        fields.raw(4)?,
        ByteSeed(
            &mut meta.engine,
            mycity_core::transaction::ENGINE_BYTE_BUDGET,
        ),
        &mut meta.parser,
    )?;
    let mut rows = Array::new(fields.raw(5)?)?;
    while let Some(raw) = rows.next(&mut meta.parser)? {
        if meta.receipts.len() == meta.receipts.capacity() {
            return Err(Error::Invalid("metadata_extra_element"));
        }
        meta.receipts
            .push(super::receipt_decode::decode_raw(raw, &mut meta.parser)?);
    }
    let cp = Header {
        sequence,
        chain: chain.0,
        genesis_sha256: genesis.0,
        previous: fields.optional(6, &mut meta.parser)?,
    };
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
