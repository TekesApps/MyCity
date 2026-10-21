//! Executable design inventory, not evidence that legacy parsing/clone paths are bounded.
use super::{Persistence, Root, Store};
use mycity_core::{
    model_capacity::Inventory,
    transaction::{
        Action, Engine, Envelope, JournalRecord, Receipt, Resource, MAX_ACTIONS, MAX_RECEIPTS,
        MAX_RESOURCES,
    },
};
use serde::Serialize;
use std::mem::size_of;

const MIB: usize = 1024 * 1024;
pub const ORIGINAL_NORMAL_BYTES: usize = 64 * MIB;

#[derive(Debug, Serialize)]
pub struct MetadataPart {
    pub name: &'static str,
    pub count: usize,
    pub record_bytes: usize,
    pub bytes: usize,
}
fn metadata() -> Vec<MetadataPart> {
    let specs = [
        (
            "store_root_and_inline_headers_excluding_engine",
            1,
            size_of::<Store>() - size_of::<Engine>(),
        ),
        ("candidate_root_and_verification_root", 2, size_of::<Root>()),
        (
            "v2_snapshot_receipt_index_body_headers",
            1,
            super::checkpoint_v2::capacity_metadata_headers(),
        ),
        (
            "source_queue_header",
            1,
            super::outbox::capacity_metadata_headers(),
        ),
        (
            "source_queue_and_staged_copy",
            super::outbox::PENDING_LIMIT * 2,
            size_of::<mycity_core::inbox::ExternalFact>(),
        ),
        ("receipt_operations_index", MAX_RECEIPTS, size_of::<u64>()),
        (
            "receipt_page_fixed_staging",
            1,
            super::checkpoint_v2::receipt_page_staging_bytes(),
        ),
        (
            "current_and_replay_journal_records",
            2,
            size_of::<JournalRecord>(),
        ),
        ("current_and_replay_receipts", 2, size_of::<Receipt>()),
        (
            "current_replay_and_wal_envelope_headers",
            3,
            size_of::<Envelope>(),
        ),
        (
            "current_replay_and_wal_actions",
            3 * MAX_ACTIONS,
            size_of::<Action>(),
        ),
        // Encoded input reuses the codec. Decoded Opening has its own typed rows;
        // file length alone does not bound a materialized serde object graph.
        (
            "decoded_opening_inline_headers_including_domain_openings",
            1,
            size_of::<mycity_core::assembly::Opening>(),
        ),
        (
            "opening_account_rows",
            mycity_core::assembly::MAX_ACCOUNTS,
            size_of::<mycity_core::assembly::OpeningAccount>(),
        ),
        (
            "opening_stock_rows",
            mycity_core::assembly::MAX_STOCKS,
            size_of::<mycity_core::assembly::OpeningStock>(),
        ),
        (
            "opening_source_rows",
            mycity_core::assembly::MAX_SOURCES,
            size_of::<mycity_core::assembly::OpeningSource>(),
        ),
        (
            "opening_city_rows",
            mycity_core::region::MAX_CITIES,
            size_of::<mycity_core::region::City>(),
        ),
        (
            "opening_route_rows",
            mycity_core::region::MAX_ROUTES,
            size_of::<mycity_core::region::Route>(),
        ),
        (
            "opening_obligation_rows",
            mycity_core::region::MAX_OBLIGATIONS,
            size_of::<mycity_core::region::Obligation>(),
        ),
        // Opening + Bootstrap + domain schema/profile + economy transport label.
        ("opening_schema_profile_and_transport_strings", 11, 64),
        ("genesis_resources", MAX_RESOURCES, size_of::<Resource>()),
        ("background_live_hashes", super::BACKGROUND_LIVE_ASSETS, 32),
        ("bounded_path_slots", 8, 4096),
        ("bounded_metadata_hash_or_schema_strings", 64, 64),
        ("stream_parser_scratch", 1, 64 * 1024),
        (
            "canonical_iterative_traversal_backing",
            1,
            mycity_core::canonical::Workspace::backing_bytes(),
        ),
        (
            "wal_frame_and_record_buffers",
            1,
            std::mem::size_of::<super::wal::Buffers>(),
        ),
        (
            "verified_input_output_controls_and_buffers",
            1,
            super::stream::capacity_metadata_bytes(),
        ),
        ("bounded_token_scratch", 1, 4096),
    ];
    specs
        .into_iter()
        .map(|(name, count, record_bytes)| MetadataPart {
            name,
            count,
            record_bytes,
            bytes: count * record_bytes,
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct Scenario {
    pub name: &'static str,
    pub engine_slots: usize,
    pub prepared_slots: usize,
    pub retained_preparation_workspace_slots: usize,
    pub retained_preparation_workspace_slot_bytes: usize,
    pub extra_air_slots: usize,
    pub extra_air_slot_bytes: usize,
    pub spatial_resident_bytes: usize,
    pub io_codec_bytes: usize,
    pub metadata_target_bytes: usize,
    pub concurrent_worker_bytes: usize,
    pub total_bytes: usize,
}
impl Scenario {
    fn new(
        name: &'static str,
        engines: usize,
        spatial: usize,
        worker: usize,
        i: &Inventory,
        metadata_bytes: usize,
    ) -> Self {
        let mut s = Self {
            name,
            engine_slots: engines,
            prepared_slots: 1,
            retained_preparation_workspace_slots: engines,
            retained_preparation_workspace_slot_bytes: i.retained_preparation_workspace_bytes,
            extra_air_slots: 5,
            extra_air_slot_bytes: mycity_core::air::WORKSPACE_BYTES
                + size_of::<mycity_core::air::AirWorkspace>(),
            spatial_resident_bytes: spatial,
            io_codec_bytes: 4 * MIB,
            metadata_target_bytes: metadata_bytes,
            concurrent_worker_bytes: worker,
            total_bytes: 0,
        };
        s.total_bytes = engines * i.engine_bytes
            + i.prepared_bytes
            + engines * i.retained_preparation_workspace_bytes
            + s.extra_air_slots * s.extra_air_slot_bytes
            + spatial
            + s.io_codec_bytes
            + s.metadata_target_bytes
            + worker;
        s
    }
}

#[derive(Debug, Serialize)]
pub struct Plan {
    pub schema: &'static str,
    pub pointer_width_bits: usize,
    pub core: Inventory,
    pub metadata: Vec<MetadataPart>,
    pub metadata_bytes: usize,
    pub store_inline_non_engine_bytes: usize,
    pub persistence_inline_bytes_already_in_store: usize,
    pub scenarios: Vec<Scenario>,
    pub required_bytes: usize,
    pub budget_bytes: usize,
    pub target_slots_fit: bool,
    pub runtime_grant_accepted: bool,
    pub prerequisites: [&'static str; 8],
    pub exclusions: [&'static str; 5],
}

/// Conservatively charge current per-Engine retained preparation workspaces in
/// addition to the active Prepared target and all five general Air slots. Even a fit is NOT permission to grant these bytes to
/// the current Vec/DOM implementation. No model, image or source data is allocated.
pub fn plan(budget: usize) -> Plan {
    let i = mycity_core::model_capacity::inventory();
    let metadata = metadata();
    let metadata_bytes = metadata.iter().map(|p| p.bytes).sum();
    // Store already contains SpatialAssets/Terrain/Scene inline headers. Reserve
    // only their heap payload here; identity's schema uses the metadata string row.
    let spatial = mycity_map::TERRAIN_RESIDENT_BUDGET_BYTES
        - size_of::<mycity_map::terrain::Terrain>()
        + mycity_map::GEOMETRY_BUDGET_BYTES
        - size_of::<mycity_map::scene::Scene>();
    let scenarios = vec![
        Scenario::new("reference_or_fault_reference", 1, 0, 0, &i, metadata_bytes),
        // Keep genesis in this conservative topology: no speculative early drop credit.
        Scenario::new(
            "sim_open_genesis_current_and_validation",
            3,
            spatial,
            5 * MIB,
            &i,
            metadata_bytes,
        ),
        Scenario::new(
            "sim_prepare_capture_accept_or_checkpoint",
            2,
            spatial,
            5 * MIB,
            &i,
            metadata_bytes,
        ),
        Scenario::new("ordinary_compare_two_models", 2, 0, 0, &i, metadata_bytes),
    ];
    let required = scenarios.iter().map(|s| s.total_bytes).max().unwrap();
    Plan { schema: "mycity-model-workspace-capacity-plan-v1", pointer_width_bits: usize::BITS as usize,
        core: i, metadata, metadata_bytes, store_inline_non_engine_bytes: size_of::<Store>() - size_of::<Engine>(),
        persistence_inline_bytes_already_in_store: size_of::<Persistence>(), scenarios,
        required_bytes: required, budget_bytes: budget, target_slots_fit: required <= budget,
        runtime_grant_accepted: false,
        prerequisites: [
            "Exact fully initialized fixed arrays/strings, active lengths, no hidden growth or deep Clone escape",
            "Five explicit shared Air scratch slots; all capture/accept/validate/staged-reload entries borrow them",
            "Charge all per-Engine retained prepare tables/background scratch and controls; extra active Prepared target is conservatively retained; durable-before-publish/full rollback",
            "Streaming seed restore with token/depth/sequence bounds before materialization; no old/new Vec capacity overlap",
            "Stream WAL, receipt archive, hashes and canonical borrowed views; no Value or full encoded-body Vec",
            "One 4MiB codec reused across serial paths, including legacy checkpoint decoding; no live nested codec borrow",
            "Typed metadata rows must use exact backing; enforce path4096/token4096/schema-or-hash64 limits before allocation",
            "Actual broker grants before all allocations, touch all backing, real child wait before credit return",
        ],
        exclusions: [
            "Current legacy dynamic parser/DOM/clone allocations remain unbounded by this target plan",
            "Terrain import transient up to128MiB is not a resident Store operation and is NOT accepted under this64MiB plan",
            "L1 layout pressure remains separately charged, not another64MiB model slot",
            "Stack, allocator metadata and foreign libraries need separate joint physical accounting",
            "Shared8MiB diagnostics, formal performance, four rates, steady,8h and real16GB remain unaccepted",
        ] }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feasibility_boundary_never_authorizes_current_runtime() {
        let p = plan(ORIGINAL_NORMAL_BYTES);
        assert!(
            p.target_slots_fit,
            "full proposed topology exceeds original64MiB: {}",
            p.required_bytes
        );
        assert!(plan(p.required_bytes).target_slots_fit);
        assert!(!plan(p.required_bytes - 1).target_slots_fit);
        assert!(!p.runtime_grant_accepted);
        assert_eq!(p.scenarios[1].engine_slots, 3);
        assert_eq!(p.scenarios[1].retained_preparation_workspace_slots, 3);
        assert!(p.core.retained_preparation_workspace_bytes > 1_000_000);
    }
}
