//! Synthetic V3 fixtures only; these values are not provider or root evidence.
use mycity_core::units::Quantity;
use crate::rational_v3::*;

pub struct SyntheticRoot;
impl RootVerifier for SyntheticRoot {
    fn verifies(&self, c: &Contract) -> bool {
        let b = &c.binding;
        b.root_ref == 7 && b.provider_ref == 8 && b.source_ref == c.source.source_ref
            && b.stock_ref == c.source.stock_ref && b.contract_ref == c.source.contract_ref
            && c.consumers.iter().all(|d| d.connection_right_ref == 12 && d.payer != 0
                && d.contract_ref != 0 && d.meter != 0 && d.quality == 1)
            && c.edges.iter().all(|e| e.physical_ref != 0 && e.loss_sink_ref != 0 && e.quality == 1)
    }
    fn verifies_opening(&self, _contract: &Contract, genesis_ref: u64) -> bool { genesis_ref != 0 }
    fn verify_supply(&self, c: &Contract, from: u64, until: u64, physical_gross: ExactAmount)
        -> Option<SupplyWitness> {
        let duration = until.checked_sub(from)?;
        let source_window_ref = from.checked_mul(MAX_WINDOW_SECONDS + 1)?
            .checked_add(duration)?.max(1);
        Some(SupplyWitness { source_window_ref, source_ref: c.source.source_ref,
            from_second: from, until_second: until, physical_gross, receipt_digest: [1; 32] })
    }
}

pub fn q(v: i64) -> Quantity { Quantity::new(v).unwrap() }
pub fn rate(n: u64, d: u32) -> Rate { Rate::new(n, d).unwrap() }
pub fn ratio(n: u32, d: u32) -> Ratio { Ratio::new(n, d, n == 0).unwrap() }

pub fn contract(resource: Resource, amount: u64, seconds: u64) -> Contract {
    let r = Rate::new(amount, seconds as u32).unwrap();
    let binding = AuthorityBinding { scope: Scope { development_city: 1 }, world_ref: 2, branch_ref: 3,
        root_ref: 7, provider_ref: 8, provider_revision: 1, source_ref: 9, source_revision: 1,
        stock_ref: 10, contract_ref: 11, clock_revision: 1, scale_revision: 1,
        effective_start: 0, effective_end: seconds };
    let source = Source { id: 1, kind: match resource { Resource::MilliWattHour => SourceKind::ContinuousElectricity,
        Resource::MilliLiter => SourceKind::StoredWater }, node: 1, resource, quality: 1, source_ref: 9,
        stock_ref: 10, contract_ref: 11, physical_supply_rate: r, input_capacity_rate: rate(1_000_000_000, 1),
        hard_peak_rate: rate(1_000_000_000, 1), available_input_milli: q(amount as i64),
        auxiliary_ratio: ratio(0, 1), auxiliary_sink_ref: 0, open: true };
    let consumer = Consumer { id: 1, node: 2, resource, quality: 1, source: 1, path: vec![1],
        connection_right_ref: 12, payer: 13, meter: 14, contract_ref: 11, priority: 0,
        required_rate: r, hard_peak_rate: rate(1_000_000_000, 1) };
    let edge = Edge { id: 1, from: 1, to: 2, resource, quality: 1, physical_ref: 15,
        stock_alias: None, capacity_rate: rate(1_000_000_000, 1), hard_peak_rate: rate(1_000_000_000, 1),
        retention: ratio(1, 1), loss_sink_ref: 16, open: true };
    Contract { policy_version: POLICY_VERSION, binding, window: Window { start_second: 0, end_second: seconds },
        resource, quality: 1, source, edges: vec![edge], consumers: vec![consumer] }
}
