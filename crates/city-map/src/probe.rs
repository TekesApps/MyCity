//! Reproducible headless assembly of one immutable spatial fixture.
use crate::{scene, terrain, Error, Point, SCHEMA};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub schema: String,
    pub bootstrap: mycity_core::Bootstrap,
    pub terrain: terrain::Manifest,
    pub scene: scene::Input,
}
pub fn run(input: Input, png: &[u8]) -> Result<Value, Error> {
    if input.schema != SCHEMA
        || input.bootstrap.validate().is_err()
        || input.scene.map.world != input.bootstrap.world
        || input.scene.map.branch != input.bootstrap.branch
    {
        return Err(Error::Input);
    }
    let budget = terrain::budget(&input.terrain, png.len(), 0)?;
    let t = terrain::import(input.terrain, png, input.scene.terrain_revision, 0)?;
    // The fixture explicitly reuses one immutable height asset for two map instances.
    // This is not evidence that arbitrary neighbor DEMs line up.
    let s = scene::Scene::assemble(input.scene, &t, &t)?;
    let tile = crate::gpu_layout::TileUpload::pack(
        &t,
        &s.input().map,
        s.input().geometry_revision,
        1,
        0,
        0,
        0,
    )?;
    let geometry_upload = crate::gpu_layout::vertices(
        s.road().points(),
        s.input().ground_z_cm,
        Point { x: 0, y: 0 },
        0,
    )?;
    let side = t.manifest().size.side_cm();
    let queries = [
        Point { x: 0, y: 0 },
        Point { x: side, y: side },
        Point {
            x: side / 2,
            y: side / 2,
        },
    ];
    let heights = queries
        .iter()
        .map(|p| t.height_cm(*p))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"schema":"mycity-m0-spatial-evidence-v1","result":"immutable_flat_scene_validated",
        "gpu_byte_layout":{"status":"cpu_packing_only_no_backend_execution","wgsl_sha256":mycity_core::sha256(crate::gpu_layout::LAYOUT.as_bytes()),
            "header_bytes":tile.header().len(),"height_bytes":tile.heights().len(),"flag_bytes":tile.flags().len(),"tile_owned_capacity_bytes":tile.owned_capacity_bytes(),
            "vertex_bytes":geometry_upload.len(),"vertex_stride_bytes":16,"max_height_conversion_error_cm":tile.max_conversion_error_cm,"stamp":tile.stamp()},
        "terrain_sha256":t.state_hash(),"geometry_sha256":s.state_hash(),"source_sha256":t.manifest().source_sha256,
        "terrain_revision":t.revision(),"geometry_revision":s.input().geometry_revision,
        "terrain_vertices":t.heights().len(),"terrain_owned_capacity_bytes":t.owned_capacity_bytes(),
        "geometry_owned_capacity_bytes":s.owned_capacity_bytes(),"owned_capacity_bytes":t.owned_capacity_bytes()+s.owned_capacity_bytes(),
        "allocator_metadata_included":false,"parse_and_report_pages_included":false,"planned_import_peak":budget,
        "queries_cm":queries,"height_results_cm":heights,"road_vertices":s.road().points(),
        "road_radius_including_error_cm":s.road().radius_cm(),"road_length_upper_cm":s.road().length_upper_cm(),
        "connector_length_upper_cm":s.connector().length_upper_cm(),"frame":s.input().map,"neighbor":s.input().neighbor,
        "seam":"declared_directional_walk_geometry_and_2m_approach_checked_not_complete_route","neighbor_asset":"same_immutable_synthetic_fixture",
        "permissions":"laboratory_input_facts_not_a_legal_authority_check","store_integration":"not_implemented","construction_authority":"none_readonly_laboratory",
        "vehicle_turning_and_grades":"not_implemented_flat_support_only","search_provider":"not_implemented",
        "memory_16gb_gate":"not_run"}),
    )
}
