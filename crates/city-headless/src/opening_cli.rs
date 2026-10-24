//! Explicit local input selection into the existing single Store factory.
use crate::opening_source_provider;
use mycity_core::INPUT_LIMIT;
use mycity_storage::Store;
use serde_json::{json, Value};
use std::path::Path;

pub fn initialize(args: &[String]) -> Result<Value, String> {
    if args.len() != 5 && args.len() != 7 {
        return Err("usage: mycity-headless init-opening NEW_WORLD OPENING_JSON RESOURCES_JSON [SPATIAL_JSON HEIGHT_PNG]".into());
    }
    let dir = Path::new(&args[2]);
    // All sources are explicitly selected and read before the first Store mutation.
    // Loading proves no license, title, business certification, or semantic acceptance.
    let opening = opening_source_provider::read_local_file(Path::new(&args[3]))?;
    let resources = opening_source_provider::load_resources_file(Path::new(&args[4]))?;
    let spatial = if args.len() == 7 {
        Some((
            opening_source_provider::read_local_bytes(Path::new(&args[5]), INPUT_LIMIT as usize)?,
            opening_source_provider::read_local_bytes(
                Path::new(&args[6]),
                mycity_map::terrain::SOURCE_BYTE_LIMIT as usize,
            )?,
        ))
    } else {
        None
    };
    let resource_source = resources.provenance.to_json();
    // No second initialization or retry over an uncertain directory. On any error,
    // leave the original files and resolve publication by opening that same Store.
    let store = match spatial.as_ref() {
        Some((input, png)) => Store::create_opening_input_spatial(
            dir, opening.input(), resources.resources,
            env!("MYCITY_SOURCE_SHA256"), (input.bytes(), png.bytes()),
        ),
        None => Store::create_opening_input(
            dir, opening.input(), resources.resources, env!("MYCITY_SOURCE_SHA256"),
        ),
    }.map_err(|e| format!("init_opening_store:{e:?}; preserve directory; resolve uncertain publication by opening it, never recreate"))?;
    let (scope, revision) = store
        .engine()
        .map_err(|e| format!("{e:?}"))?
        .world()
        .scope_revision();
    Ok(json!({
        "schema":"mycity-init-opening-v1", "source_sha256":env!("MYCITY_SOURCE_SHA256"),
        "world":scope.world(), "branch":scope.branch(), "revision":revision,
        "sequence":store.committed_sequence(), "state_sha256":store.state_hash().map_err(|e|format!("{e:?}"))?,
        "opening_source":opening.provenance().to_json(), "resource_source":resource_source,
        "spatial_sources":spatial.as_ref().map(|(input,png)|json!({"json":input.provenance().to_json(),"height_png":png.provenance().to_json()})),
        "input_authority":"explicit_local_file_os_read_only; not_license_title_or_business_certification",
        "acceptance":"canonical_Store_initialization_and_publication_only",
        "memory_16gb_gate":"not_run"
    }))
}
