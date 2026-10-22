//! Bounded adapter from an exact transport assignment to a town market plan.
//!
//! This module reserves portions of already-served OD cohorts. It never interprets
//! unused road capacity as new market capacity and never settles cash or inventory.

use crate::{
    town_economy::{plan_day, DeliveryQuote, MarketInput, SettlementBatch},
    transport::{Assignment, OdDemand, Opening, UnservedReason},
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_TRIP_PARTITIONS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TripPartition {
    pub quote_id: u64,
    pub outbound_source: u64,
    pub return_source: u64,
    /// Requested maximum trips in each OD direction, not road capacity. Actual service is
    /// reserved deterministically in quote-id order and returned as `allocated_trips`.
    pub trip_count: u32,
    /// Commodity quantity carried per allocated round trip. Never mixed with people.
    pub goods_per_trip: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeePolicyInput {
    pub provider_account: u32,
    /// Externally supplied cents per travel-second, applied to both legs per trip.
    pub cents_per_travel_second: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeePolicyIntegration {
    ExternalInputUnintegrated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessStatus {
    Reachable,
    PartiallyServed,
    NoOutboundService,
    NoReturnService,
    NoRoundTripAllocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryAccess {
    pub quote_id: u64,
    pub outbound_source: u64,
    pub return_source: u64,
    pub requested_trip_count: u32,
    pub allocated_trips: u32,
    pub goods_per_trip: Quantity,
    pub accessible_quantity: Quantity,
    pub outbound_mean_seconds: Option<u64>,
    pub return_mean_seconds: Option<u64>,
    pub outbound_boundary_sources: Vec<u64>,
    pub return_boundary_sources: Vec<u64>,
    pub status: AccessStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessPlan {
    /// Digest over the exact opening, assignment, market quote declarations, partitions and tariff.
    pub source_digest: [u8; 32],
    pub transport_revision: u64,
    pub transport_fingerprint: [u8; 32],
    /// The caller supplied this tariff. This adapter does not estimate a market tariff.
    pub fee_policy_input: FeePolicyInput,
    pub fee_policy_integration: FeePolicyIntegration,
    pub delivery_access: Vec<DeliveryAccess>,
    /// Pure proposal only; caller still needs Engine validation and commit.
    pub batch: SettlementBatch,
}

/// Verified transport-derived delivery inputs and a compact per-quote audit receipt.
///
/// The caller supplies `Opening` and `Assignment` by reference. A writer should resolve those
/// references from its immutable, hash-verified transport snapshot; do not put either full
/// object in a small `TownSettle` command. `source_digest` binds the exact market input,
/// assignment, quote partitions, and fee policy used here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedDeliveryQuotes {
    /// Present when this was resolved from a hash-verified immutable input snapshot.
    pub source_snapshot: Option<TransportSnapshotRef>,
    pub profile_id: Option<u32>,
    pub source_digest: [u8; 32],
    pub transport_revision: u64,
    pub transport_fingerprint: [u8; 32],
    pub fee_policy_input: FeePolicyInput,
    pub delivery_quotes: Vec<DeliveryQuote>,
    pub delivery_access: Vec<DeliveryAccess>,
}

/// Compact immutable-snapshot locator suitable for a bounded settlement command. Storage
/// resolves the generation to the repository's hash-bearing immutable reference and verifies
/// its bytes before handing borrowed source records to this module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportSnapshotRef {
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidTransport,
    StaleAssignment,
    InvalidAssignment,
    InvalidMarket,
    InvalidPartition,
    DuplicatePartition,
    MissingQuote,
    DuplicateSourceBinding,
    InvalidSnapshotRef,
    MissingOutcome,
    Overflow,
    Planning,
}

fn outcome(assignment: &Assignment, source: u64) -> Option<&crate::transport::DemandOutcome> {
    assignment
        .demand_outcomes
        .iter()
        .find(|x| x.source_id == source)
}

fn demand(opening: &Opening, source: u64) -> Option<&OdDemand> {
    opening.demands.iter().find(|x| x.source_id == source)
}

fn boundary_sources(
    edges_by_id: &BTreeMap<u32, &crate::transport::RoadEdge>,
    assignment: &Assignment,
    source: u64,
) -> Result<Vec<u64>, Error> {
    let mut sources = Vec::new();
    let mut found = false;
    for receipt in assignment
        .path_receipts
        .iter()
        .filter(|path| path.source_id == source)
    {
        found = true;
        for edge_id in &receipt.edge_ids {
            let edge = edges_by_id.get(edge_id).ok_or(Error::InvalidTransport)?;
            if edge.city_from != edge.city_to {
                sources.push(edge.boundary_source.ok_or(Error::InvalidTransport)?);
            }
        }
    }
    if !found
        && assignment
            .demand_outcomes
            .iter()
            .find(|outcome| outcome.source_id == source)
            .is_none_or(|outcome| outcome.served_people != 0)
    {
        return Err(Error::MissingOutcome);
    }
    sources.sort_unstable();
    sources.dedup();
    Ok(sources)
}

fn entrance(opening: &Opening, entrance_id: u32) -> Option<&crate::transport::Entrance> {
    opening.entrances.iter().find(|x| x.id == entrance_id)
}

fn account_for_household(input: &MarketInput, group_id: u32) -> Option<u32> {
    input
        .households
        .iter()
        .find(|x| x.id == group_id)
        .map(|x| x.account)
}

fn shop_account(input: &MarketInput, shop_id: u32, good: u8) -> Option<u32> {
    input
        .shops
        .iter()
        .find(|x| x.shop_id == shop_id && x.good == good)
        .map(|x| x.account)
}

fn market_declarations_valid(input: &MarketInput, fee: FeePolicyInput) -> bool {
    fee.provider_account != 0
        && input
            .accounts
            .iter()
            .any(|x| x.account == fee.provider_account)
        && input.delivery.iter().all(|q| {
            q.quote_id != 0
                && q.shop != 0
                && q.good < crate::town_economy::MAX_GOODS as u8
                && account_for_household(input, q.household_group).is_some()
                && shop_account(input, q.shop, q.good).is_some()
                && input
                    .accounts
                    .iter()
                    .any(|a| a.account == q.utility_account)
        })
}

fn ceil_div(n: u128, d: u128) -> Result<u128, Error> {
    if d == 0 {
        return Err(Error::InvalidPartition);
    }
    n.checked_add(d - 1).ok_or(Error::Overflow).map(|x| x / d)
}

fn source_digest(
    opening: &Opening,
    assignment: &Assignment,
    input: &MarketInput,
    partitions: &[TripPartition],
    fee: FeePolicyInput,
) -> Result<[u8; 32], Error> {
    let opening_fingerprint = opening
        .input_fingerprint()
        .map_err(|_| Error::InvalidTransport)?;
    let mut ps = partitions.to_vec();
    ps.sort_by_key(|x| x.quote_id);
    let bytes = serde_json::to_vec(&(opening_fingerprint, assignment, input, ps, fee))
        .map_err(|_| Error::InvalidMarket)?;
    Ok(Sha256::digest(bytes).into())
}

/// Resolve verified transport assignments into current-day market delivery quotes without
/// invoking the market planner. `opening` and `assignment` should be borrowed from the same
/// immutable, hash-verified transport snapshot identified by the caller's snapshot reference.
/// This is the integration boundary for a writer that must continue with its own production
/// executions and call `plan_day_with_execution` exactly once.
pub fn resolve_delivery_quotes(
    input: &MarketInput,
    opening: &Opening,
    assignment: &Assignment,
    partitions: &[TripPartition],
    fee: FeePolicyInput,
) -> Result<ResolvedDeliveryQuotes, Error> {
    // A permission or topology edit after assignment makes the receipt stale, even if the
    // edited opening is no longer independently valid (for example, an entrance was revoked).
    if !assignment.matches(opening) {
        return Err(Error::StaleAssignment);
    }
    opening.validate().map_err(|_| Error::InvalidTransport)?;
    assignment
        .verify(opening)
        .map_err(|_| Error::InvalidAssignment)?;
    let edges_by_id: BTreeMap<_, _> = opening.edges.iter().map(|edge| (edge.id, edge)).collect();
    if !market_declarations_valid(input, fee) {
        return Err(Error::InvalidMarket);
    }
    if input.delivery.len() > MAX_TRIP_PARTITIONS || partitions.len() > MAX_TRIP_PARTITIONS {
        return Err(Error::InvalidPartition);
    }
    if partitions.len() != input.delivery.len() {
        return Err(Error::MissingQuote);
    }

    let quotes: BTreeMap<_, _> = input.delivery.iter().map(|q| (q.quote_id, q)).collect();
    if quotes.len() != input.delivery.len() {
        return Err(Error::DuplicatePartition);
    }
    let mut by_quote = BTreeMap::new();
    let mut source_bindings = BTreeSet::new();
    for p in partitions {
        if p.quote_id == 0 || p.goods_per_trip == Quantity::ZERO {
            return Err(Error::InvalidPartition);
        }
        if p.outbound_source == p.return_source {
            return Err(Error::InvalidPartition);
        }
        let q = *quotes.get(&p.quote_id).ok_or(Error::MissingQuote)?;
        let household_account =
            account_for_household(input, q.household_group).ok_or(Error::InvalidMarket)?;
        let merchant_account = shop_account(input, q.shop, q.good).ok_or(Error::InvalidMarket)?;
        let out = demand(opening, p.outbound_source).ok_or(Error::MissingOutcome)?;
        let back = demand(opening, p.return_source).ok_or(Error::MissingOutcome)?;
        let out_origin = entrance(opening, out.origin_entrance).ok_or(Error::InvalidTransport)?;
        let out_destination =
            entrance(opening, out.destination_entrance).ok_or(Error::InvalidTransport)?;
        let back_origin = entrance(opening, back.origin_entrance).ok_or(Error::InvalidTransport)?;
        let back_destination =
            entrance(opening, back.destination_entrance).ok_or(Error::InvalidTransport)?;
        if out_origin.owner != household_account
            || out_destination.owner != merchant_account
            || back_origin.owner != merchant_account
            || back_destination.owner != household_account
        {
            return Err(Error::InvalidPartition);
        }
        if out.mode != back.mode || out.mode.count_ones() != 1 {
            return Err(Error::InvalidPartition);
        }
        // Read the exact per-OD paths rather than using aggregate boundary totals, so a
        // separate city's movement cannot block this quote and legitimate cross-city paths
        // remain visible in the receipt.
        boundary_sources(&edges_by_id, assignment, p.outbound_source)?;
        boundary_sources(&edges_by_id, assignment, p.return_source)?;
        if !source_bindings.insert((p.quote_id, p.outbound_source, p.return_source)) {
            return Err(Error::DuplicateSourceBinding);
        }
        if by_quote.insert(p.quote_id, *p).is_some() {
            return Err(Error::DuplicatePartition);
        }
    }
    if by_quote.len() != quotes.len() {
        return Err(Error::MissingQuote);
    }
    // Partition shared OD service deterministically by quote id. `trip_count` is the
    // requested maximum; the receipt records the actually served round-trip allocation.
    let mut source_remaining: BTreeMap<u64, u32> = assignment
        .demand_outcomes
        .iter()
        .map(|x| (x.source_id, x.served_people))
        .collect();
    let mut actual_by_quote = BTreeMap::new();
    let mut partitions_ordered = partitions.to_vec();
    partitions_ordered.sort_by_key(|x| x.quote_id);
    for p in &partitions_ordered {
        let out_remaining = *source_remaining
            .get(&p.outbound_source)
            .ok_or(Error::MissingOutcome)?;
        let return_remaining = *source_remaining
            .get(&p.return_source)
            .ok_or(Error::MissingOutcome)?;
        let allocated = p.trip_count.min(out_remaining).min(return_remaining);
        *source_remaining
            .get_mut(&p.outbound_source)
            .ok_or(Error::MissingOutcome)? -= allocated;
        *source_remaining
            .get_mut(&p.return_source)
            .ok_or(Error::MissingOutcome)? -= allocated;
        actual_by_quote.insert(p.quote_id, allocated);
    }

    let mut delivery_quotes = input.delivery.clone();
    let mut access = Vec::with_capacity(partitions.len());
    for q in &mut delivery_quotes {
        let p = *by_quote.get(&q.quote_id).ok_or(Error::MissingQuote)?;
        let allocated_trips = *actual_by_quote
            .get(&q.quote_id)
            .ok_or(Error::MissingQuote)?;
        let out = outcome(assignment, p.outbound_source).ok_or(Error::MissingOutcome)?;
        let back = outcome(assignment, p.return_source).ok_or(Error::MissingOutcome)?;
        let out_seconds = out.mean_travel_seconds;
        let back_seconds = back.mean_travel_seconds;
        let outbound_boundary_sources =
            boundary_sources(&edges_by_id, assignment, p.outbound_source)?;
        let return_boundary_sources = boundary_sources(&edges_by_id, assignment, p.return_source)?;
        let status = if out.served_people == 0 {
            AccessStatus::NoOutboundService
        } else if back.served_people == 0 {
            AccessStatus::NoReturnService
        } else if allocated_trips == 0 {
            AccessStatus::NoRoundTripAllocation
        } else if allocated_trips < p.trip_count {
            AccessStatus::PartiallyServed
        } else {
            AccessStatus::Reachable
        };
        let accessible_quantity = if allocated_trips > 0 {
            p.goods_per_trip
                .checked_mul(u64::from(allocated_trips))
                .map_err(|_| Error::Overflow)?
        } else {
            Quantity::ZERO
        };
        let total_seconds = match (out_seconds, back_seconds) {
            (Some(a), Some(b)) if allocated_trips > 0 => a.checked_add(b).ok_or(Error::Overflow)?,
            _ => 0,
        };
        let trip_fee_total = u128::from(total_seconds)
            .checked_mul(u128::from(fee.cents_per_travel_second))
            .and_then(|x| x.checked_mul(u128::from(allocated_trips)))
            .ok_or(Error::Overflow)?;
        let per_q = if accessible_quantity == Quantity::ZERO {
            0
        } else {
            // `Quantity` is in milli-native units; this conservative integer rate covers the
            // declared trips at full quote quantity without turning time itself into revenue.
            ceil_div(trip_fee_total, accessible_quantity.value() as u128)?
        };
        q.reachable = allocated_trips > 0;
        q.route_source = q.quote_id;
        q.freight_account = fee.provider_account;
        q.available_capacity = accessible_quantity;
        q.freight_per_q = Money::new(i64::try_from(per_q).map_err(|_| Error::Overflow)?)
            .map_err(|_| Error::Overflow)?;
        access.push(DeliveryAccess {
            quote_id: q.quote_id,
            outbound_source: p.outbound_source,
            return_source: p.return_source,
            requested_trip_count: p.trip_count,
            allocated_trips,
            goods_per_trip: p.goods_per_trip,
            accessible_quantity,
            outbound_mean_seconds: out_seconds,
            return_mean_seconds: back_seconds,
            outbound_boundary_sources,
            return_boundary_sources,
            status,
        });
    }
    access.sort_by_key(|x| x.quote_id);
    Ok(ResolvedDeliveryQuotes {
        source_snapshot: None,
        profile_id: None,
        source_digest: source_digest(opening, assignment, input, partitions, fee)?,
        transport_revision: assignment.network_revision,
        transport_fingerprint: assignment.input_fingerprint,
        fee_policy_input: fee,
        delivery_quotes,
        delivery_access: access,
    })
}

/// Resolve quotes using source records loaded from a hash-verified immutable snapshot. The
/// command carries only `profile_id` and this compact reference; it never embeds the bounded
/// `Opening` or `Assignment`. `input` carries the authoritative world/branch/revision/day and
/// operation scope already present in the enclosing Engine transaction.
pub fn resolve_delivery_quotes_from_snapshot(
    profile_id: u32,
    source_snapshot: TransportSnapshotRef,
    input: &MarketInput,
    opening: &Opening,
    assignment: &Assignment,
    partitions: &[TripPartition],
    fee: FeePolicyInput,
) -> Result<ResolvedDeliveryQuotes, Error> {
    if profile_id == 0 || source_snapshot.generation == 0 {
        return Err(Error::InvalidSnapshotRef);
    }
    let mut resolved = resolve_delivery_quotes(input, opening, assignment, partitions, fee)?;
    let bytes = serde_json::to_vec(&(profile_id, &source_snapshot, resolved.source_digest))
        .map_err(|_| Error::InvalidSnapshotRef)?;
    resolved.source_digest = Sha256::digest(bytes).into();
    resolved.profile_id = Some(profile_id);
    resolved.source_snapshot = Some(source_snapshot);
    Ok(resolved)
}

/// Resolve transport access then make the existing pure town-economy plan. This convenience
/// entry point is not an Engine settlement and has no recipe-execution input.
pub fn plan_accessible_day(
    input: &MarketInput,
    opening: &Opening,
    assignment: &Assignment,
    partitions: &[TripPartition],
    fee: FeePolicyInput,
) -> Result<AccessPlan, Error> {
    let resolved = resolve_delivery_quotes(input, opening, assignment, partitions, fee)?;
    let mut market = input.clone();
    market.delivery = resolved.delivery_quotes;
    let batch = plan_day(&market).map_err(|_| Error::Planning)?;
    Ok(AccessPlan {
        source_digest: resolved.source_digest,
        transport_revision: resolved.transport_revision,
        transport_fingerprint: resolved.transport_fingerprint,
        fee_policy_input: resolved.fee_policy_input,
        fee_policy_integration: FeePolicyIntegration::ExternalInputUnintegrated,
        delivery_access: resolved.delivery_access,
        batch,
    })
}

/// Helper for callers presenting why a source had no service; the adapter still leaves the
/// market quote inaccessible and does not convert a physical failure into a charge.
pub fn source_failure(assignment: &Assignment, source: u64) -> Option<UnservedReason> {
    assignment
        .unserved
        .iter()
        .find(|x| x.source_id == source)
        .map(|x| x.reason)
}
