//! Pre-implementation typed capacity inventory. No runtime grant or allocation proof.
use crate::{
    air::{Source, State, STRATEGY},
    arena::{AccountDomain, Arena, Handle, InventoryDomain},
    assembly::{Account, OpeningSource, Stock, World, MAX_ACCOUNTS, MAX_SOURCES, MAX_STOCKS},
    background::SOURCE_LIMIT,
    timeline::{Claim, ClaimSourceRecord, Payroll, CLAIM_LIMIT, MAX_CLAIM_SOURCES, PAYROLL_LIMIT},
    transaction::{
        preparation_workspace_capacity, prepared_header_bytes, Engine, HotJournalRecord, Lease,
        Resource, MAX_LEASES, MAX_RECEIPTS, MAX_RESOURCES,
    },
};
use serde::Serialize;
use std::mem::size_of;

#[derive(Debug, Serialize)]
pub struct Part {
    pub name: &'static str,
    pub count: usize,
    pub record_bytes: usize,
    pub bytes: usize,
}
impl Part {
    fn new(name: &'static str, count: usize, record_bytes: usize) -> Self {
        Self {
            name,
            count,
            record_bytes,
            bytes: count * record_bytes,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub engine: Vec<Part>,
    pub prepared: Vec<Part>,
    pub extra_air_state: Vec<Part>,
    pub retained_preparation_workspace: Vec<Part>,
    pub retained_preparation_workspace_bytes: usize,
    pub engine_bytes: usize,
    pub prepared_bytes: usize,
    pub extra_air_state_bytes: usize,
    pub source_limit: usize,
    pub cell_limit: usize,
    pub runtime_ownership_proven: bool,
}

fn timeline_rows() -> [Part; 3] {
    [
        Part::new("claims", CLAIM_LIMIT, size_of::<Claim>()),
        Part::new("payroll", PAYROLL_LIMIT, size_of::<Payroll>()),
        Part::new(
            "sparse_claim_sources",
            MAX_CLAIM_SOURCES,
            size_of::<ClaimSourceRecord>(),
        ),
    ]
}
fn air_payload(sources: usize) -> Vec<Part> {
    vec![
        Part::new("air_sources", sources, size_of::<Source>()),
        Part::new(
            "air_f32_bits_field_and_scratch",
            128 * 128 * 2,
            size_of::<u32>(),
        ),
        Part::new("air_f64_bits_integral", 128 * 128, size_of::<u64>()),
        Part::new("air_strategy_and_input_hash", 1, STRATEGY.len() + 64),
    ]
}
fn background_rows() -> Vec<Part> {
    // State/Input headers are already inline in Option<Background>/Job in each root.
    let mut rows = air_payload(SOURCE_LIMIT);
    rows.push(Part::new(
        "pending_job_input_sources",
        SOURCE_LIMIT,
        size_of::<Source>(),
    ));
    rows.push(Part::new("pending_job_input_strategy", 1, STRATEGY.len()));
    rows
}
fn sum(parts: &[Part]) -> usize {
    parts.iter().map(|p| p.bytes).sum()
}

/// Full schema capacities, not current live lengths or the old post-allocation limits.
/// Inline Region/Economy/Boot/Exchange (including Inbox) are counted in root sizes.
/// Proposed exact-capacity rows/strings must be enforced before this becomes a grant.
pub fn inventory() -> Inventory {
    let mut engine = vec![
        Part::new(
            "engine_inline_root_including_timeline_background_headers",
            1,
            size_of::<Engine>(),
        ),
        Part::new(
            "boxed_world_inline_root_including_fixed_domains",
            1,
            size_of::<World>(),
        ),
        Part::new(
            "account_arena_slots_and_free_indices",
            1,
            Arena::<AccountDomain, Account>::fixed_payload_bytes(MAX_ACCOUNTS),
        ),
        Part::new(
            "stock_arena_slots_and_free_indices",
            1,
            Arena::<InventoryDomain, Stock>::fixed_payload_bytes(MAX_STOCKS),
        ),
        Part::new(
            "account_id_index",
            MAX_ACCOUNTS,
            size_of::<(u32, Handle<AccountDomain>)>(),
        ),
        Part::new(
            "stock_id_index",
            MAX_STOCKS,
            size_of::<(u32, Handle<InventoryDomain>)>(),
        ),
        Part::new("opening_sources", MAX_SOURCES, size_of::<OpeningSource>()),
        Part::new("resources", MAX_RESOURCES, size_of::<Resource>()),
        Part::new("leases", MAX_LEASES, size_of::<Option<Lease>>()),
        Part::new("hot_journal", MAX_RECEIPTS, size_of::<HotJournalRecord>()),
    ];
    engine.push(Part::new(
        "spatial_permission_state_fixed_rows",
        1,
        crate::spatial_transport_permissions::SpatialPermissionState::max_owned_capacity_bytes(),
    ));
    engine.push(Part::new(
        "spatial_geometry_request_and_approval_rows",
        1,
        crate::spatial_geometry_permissions::SpatialGeometryState::max_owned_rows_bytes(),
    ));
    engine.push(Part::new("managed_venue_optional_max_owned_bytes", 1, crate::venue_runtime::Runtime::max_owned_bytes()));
    engine.push(Part::new("education_optional_max_owned_bytes",1,crate::education_runtime::Runtime::owned_bytes()));
    engine.push(Part::new("shared_member_activity_optional_max_owned_bytes",1,crate::member_activity::Ledger::owned_bytes()));
    engine.extend(timeline_rows());
    engine.extend(background_rows());
    let mut prepared = vec![
        Part::new("managed_venue_usage_candidate_rows", 1, crate::venue_runtime::UseCandidate::payload_bytes()),
        Part::new("education_and_member_prepared_overlap_max_bytes",1,crate::education_authority::candidate_bytes()),
        Part::new("education_restore_extra_typed_workspace_bytes",1,crate::education_authority::restore_workspace_bytes()),
        Part::new(
            "prepared_inline_root_including_fixed_domain_copies",
            1,
            prepared_header_bytes(),
        ),
        Part::new(
            "dirty_accounts",
            MAX_ACCOUNTS,
            size_of::<(Handle<AccountDomain>, Account)>(),
        ),
        // All stock records, not merely the initial MAX_ACTIONS*2 Vec reservation.
        Part::new(
            "dirty_stocks",
            MAX_STOCKS,
            size_of::<(Handle<InventoryDomain>, Stock)>(),
        ),
        Part::new("resources", MAX_RESOURCES, size_of::<Resource>()),
        Part::new("leases", MAX_LEASES, size_of::<Option<Lease>>()),
    ];
    prepared.extend(timeline_rows());
    prepared.extend(background_rows());
    prepared.push(Part::new(
        "spatial_permission_candidate_fixed_rows",
        1,
        crate::spatial_transport_permissions::SpatialPermissionState::max_owned_capacity_bytes(),
    ));
    prepared.push(Part::new(
        "spatial_geometry_candidate_request_and_approval_rows",
        1,
        crate::spatial_geometry_permissions::SpatialGeometryState::max_owned_rows_bytes(),
    ));
    let mut extra_air_state = vec![Part::new("state_inline_header", 1, size_of::<State>())];
    // Shared maximum supports the full CPU/reference kernel too, not only four shipments.
    extra_air_state.extend(air_payload(20_000));
    let (tables, work, controls) = preparation_workspace_capacity();
    let retained_preparation_workspace = vec![
        Part::new(
            "per_engine_retained_prepare_tables_and_background",
            1,
            tables,
        ),
        Part::new("per_engine_background_compute_payload", 1, work),
        Part::new(
            "eight_slot_headers_and_conservative_arc_allowance",
            1,
            controls,
        ),
    ];
    Inventory {
        retained_preparation_workspace_bytes: sum(&retained_preparation_workspace),
        retained_preparation_workspace,
        engine_bytes: sum(&engine),
        prepared_bytes: sum(&prepared),
        extra_air_state_bytes: sum(&extra_air_state),
        engine,
        prepared,
        extra_air_state,
        source_limit: 20_000,
        cell_limit: 128 * 128,
        runtime_ownership_proven: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_domain_copy_and_full_dirty_stock_table_is_included() {
        let i = inventory();
        assert_eq!(
            i.prepared
                .iter()
                .find(|p| p.name == "dirty_stocks")
                .unwrap()
                .count,
            MAX_STOCKS
        );
        assert_eq!(
            i.engine
                .iter()
                .find(|p| p.name == "spatial_geometry_request_and_approval_rows")
                .unwrap()
                .bytes,
            crate::spatial_geometry_permissions::SpatialGeometryState::max_owned_rows_bytes()
        );
        assert_eq!(
            i.prepared
                .iter()
                .find(|p| p.name == "spatial_geometry_candidate_request_and_approval_rows")
                .unwrap()
                .bytes,
            crate::spatial_geometry_permissions::SpatialGeometryState::max_owned_rows_bytes()
        );
        assert!(
            i.prepared
                .iter()
                .find(|p| p.name == "dirty_stocks")
                .unwrap()
                .count
                > crate::transaction::MAX_ACTIONS * 2
        );
        assert!(
            i.engine_bytes
                > CLAIM_LIMIT * size_of::<Claim>() + MAX_RECEIPTS * size_of::<HotJournalRecord>()
        );
        assert!(!i.runtime_ownership_proven);
    }
}
