//! Original genesis decoded through the retained fixed lexical workspace.
use super::*;
use crate::metadata::{
    lexical::{Fields, Raw},
    reader,
};
#[derive(Deserialize)]
struct Label(#[serde(deserialize_with = "metadata::text::<_,64>")] String);
#[derive(Deserialize)]
struct Rules(#[serde(deserialize_with = "metadata::text::<_,128>")] String);
pub(super) struct Header {
    pub schema: String,
    pub rules_identity: String,
    pub checkpoint_layout: u8,
    pub spatial: Option<SpatialReference>,
    pub spatial_permissions:
        Option<mycity_core::spatial_transport_permissions::SpatialPermissionState>,
    pub geometry_adapter_basis:
        Option<crate::spatial_geometry_updates::SpatialGeometryAdapterBasis>,
}
pub(super) fn decode(meta: &mut MetadataWorkspace) -> Result<Header, Error> {
    let fields = Fields::parse(
        Raw::root(meta.file.as_slice()),
        &[
            "schema",
            "rules_identity",
            "checkpoint_layout",
            "opening",
            "resources",
            "spatial",
            "spatial_permissions",
            "geometry_adapter_basis",
        ],
        false,
        &mut meta.parser,
    )?;
    let schema: Label = fields.get(0, &mut meta.parser)?;
    let rules: Rules = fields.get(1, &mut meta.parser)?;
    let checkpoint_layout = match fields.values[2] {
        Some(raw) => reader::decode(raw, &mut meta.parser)?,
        None => 0,
    };
    reader::seed(
        fields.raw(3)?,
        metadata::ByteSeed(&mut meta.opening, INPUT_LIMIT as usize),
        &mut meta.parser,
    )?;
    reader::seed(
        fields.raw(4)?,
        metadata::RowsSeed(&mut meta.resources),
        &mut meta.parser,
    )?;
    Ok(Header {
        schema: schema.0,
        rules_identity: rules.0,
        checkpoint_layout,
        spatial: fields.optional(5, &mut meta.parser)?,
        spatial_permissions: fields.optional(6, &mut meta.parser)?,
        geometry_adapter_basis: fields.optional(7, &mut meta.parser)?,
    })
}
