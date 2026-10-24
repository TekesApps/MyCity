//! Source-level tests for the isolated exact-rational V3 candidate; execution is not_run.
#[path = "../src/utility_network/rational_v3.rs"]
mod rational_v3;
#[path = "../../../tests/fixtures/issue-85/v3.rs"]
mod fixtures;

use fixtures::*;
use rational_v3::*;
pub use mycity_core::units;
use sha2::{Digest, Sha256};

fn ea(whole: u64, rem: u64, den: u64) -> ExactAmount { ExactAmount { whole, rem, den } }

#[test]
fn exact_energy_and_water_rates_keep_milli_units_distinct() {
    let energy = contract(Resource::MilliWattHour, 100_000, 3_600);
    let water = contract(Resource::MilliLiter, 10_000, 3_600);
    assert_eq!(energy.consumers[0].required_rate, rate(250, 9));
    assert_eq!(water.consumers[0].required_rate, rate(25, 9));
    assert_eq!(energy.source.resource, Resource::MilliWattHour);
    assert_eq!(water.source.resource, Resource::MilliLiter);
    let clinic = contract(Resource::MilliWattHour, 100_000, 3_600);
    let clinic_water = contract(Resource::MilliLiter, 20_000, 3_600);
    let fire_electricity = contract(Resource::MilliWattHour, 1_000_000, 1_800);
    let fire_water = contract(Resource::MilliLiter, 1_000_000, 1_800);
    assert_eq!(clinic.consumers[0].required_rate, rate(250, 9));
    assert_eq!(clinic_water.consumers[0].required_rate, rate(50, 9));
    assert_eq!(fire_electricity.consumers[0].required_rate, rate(5_000, 9));
    assert_eq!(fire_water.consumers[0].required_rate, rate(5_000, 9));
    assert_eq!(rate(100_000, 3_600), rate(250, 9)); // 100 Wh across one hour = 100 W
    assert_eq!(rate(3_600_000, 3_600), rate(1_000, 1)); // 3,600 Wh across one hour = 3,600 W
}

#[test]
fn public_precision_scale_includes_the_source_generation_rate_denominator() {
    let mut c = contract(Resource::MilliWattHour, 4, 4);
    c.source.physical_supply_rate = rate(20_000, 3);
    assert_eq!(c.consumers[0].required_rate, rate(1, 1));
    assert_eq!(c.precision_denominator(), Ok(3));
}

#[test]
fn whole_window_and_multiple_cuts_restore_to_the_same_checkpoint() {
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let opening = c.opening(&root, 77).unwrap();
    let whole = c.propose(&root, &opening, 3_600, MAX_WORK).unwrap().candidate.unwrap();
    let mut state = opening;
    for cut in [600, 1_200, 1_800, 2_700, 3_600] {
        let candidate = c.propose(&root, &state, cut, MAX_WORK).unwrap().candidate.unwrap();
        state = c.revalidate(&root, &state, &candidate).unwrap();
        assert_eq!(c.revalidate(&root, &state, &candidate), Err(Error::StaleBase));
        let saved = state.checkpoint();
        state = c.restore(&root, saved.clone(), state.digest().unwrap()).unwrap();
        assert_eq!(state.checkpoint(), saved);
    }
    assert_eq!(state.checkpoint(), whole.next);
    assert_eq!(state.checkpoint().sequence, 3_600);
}

#[test]
fn all_3600_one_second_advances_match_the_whole_hour_checkpoint() {
    let root = SyntheticRoot;
    for (resource, amount) in [(Resource::MilliWattHour, 100_000), (Resource::MilliLiter, 10_000)] {
        let c = contract(resource, amount, 3_600);
        let opening = c.opening(&root, 92).unwrap();
        let whole = c.propose(&root, &opening, 3_600, MAX_WORK).unwrap().candidate.unwrap();
        let mut state = opening;
        for second in 1..=3_600 {
            let step = c.propose(&root, &state, second, MAX_WORK).unwrap().candidate.unwrap();
            state = c.revalidate(&root, &state, &step).unwrap();
        }
        assert_eq!(state.checkpoint(), whole.next);
        assert_eq!(state.checkpoint().physical.consumers[0].1, ea(amount, 0, 9));
    }
}

#[test]
fn police_medical_and_fire_contracts_reach_their_literal_hour_totals() {
    let root = SyntheticRoot;
    for (resource, amount, seconds) in [
        (Resource::MilliWattHour, 100_000, 3_600),
        (Resource::MilliLiter, 10_000, 3_600),
        (Resource::MilliWattHour, 100_000, 3_600),
        (Resource::MilliLiter, 20_000, 3_600),
        (Resource::MilliWattHour, 1_000_000, 1_800),
        (Resource::MilliLiter, 1_000_000, 1_800),
    ] {
        let c = contract(resource, amount, seconds);
        let base = c.opening(&root, 93).unwrap();
        let candidate = c.propose(&root, &base, seconds, MAX_WORK).unwrap().candidate.unwrap();
        assert_eq!(candidate.consumer_deltas[0].physical, ea(amount, 0, 9));
    }
}

#[test]
fn one_second_water_meter_keeps_its_exact_fraction_and_full_hour_is_ten_liters() {
    let c = contract(Resource::MilliLiter, 10_000, 3_600);
    let root = SyntheticRoot;
    let base = c.opening(&root, 94).unwrap();
    let one = c.propose(&root, &base, 1, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!(one.consumer_deltas[0].physical, ea(2, 7, 9));
    assert_eq!(one.consumer_deltas[0].credited_milli_delta, 2);
    assert_eq!(one.consumer_deltas[0].fractional_delivered_after, ea(0, 7, 9));
    let full = c.propose(&root, &base, 3_600, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!(full.consumer_deltas[0].physical, ea(10_000, 0, 9));
}

#[test]
fn two_hop_exact_loss_aux_and_ceil_hold_match_literal_arithmetic() {
    let mut c = contract(Resource::MilliWattHour, 1, 3_600);
    let mut terminal_edge = c.edges[0].clone();
    terminal_edge.id = 2;
    terminal_edge.from = 2;
    terminal_edge.to = 3;
    terminal_edge.physical_ref = 16;
    terminal_edge.loss_sink_ref = 17;
    terminal_edge.retention = ratio(2, 3);
    c.edges[0].retention = ratio(2, 3);
    c.edges.push(terminal_edge);
    c.consumers[0].path = vec![1, 2];
    c.consumers[0].node = 3;
    c.source.physical_supply_rate = rate(1, 1_600); // 9/4 over the 3,600-second segment
    c.source.input_capacity_rate = rate(1, 1_000);
    c.source.hard_peak_rate = rate(1, 1_000);
    c.source.available_input_milli = q(3);
    c.source.auxiliary_ratio = ratio(1, 5);
    c.source.auxiliary_sink_ref = 18;
    c.edges[0].capacity_rate = rate(1, 1_600);
    c.edges[0].hard_peak_rate = rate(1, 1_600);
    c.edges[1].capacity_rate = rate(1, 2_400);
    c.edges[1].hard_peak_rate = rate(1, 2_400);
    c.consumers[0].hard_peak_rate = rate(1, 3_600);
    let root = SyntheticRoot;
    let base = c.opening(&root, 78).unwrap();
    let candidate = c.propose(&root, &base, 3_600, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!(candidate.next.common_denominator, 72_000);
    assert_eq!(candidate.consumer_deltas[0].physical, ea(1, 0, 72_000));
    assert_eq!(candidate.edge_deltas[0].physical_input, ea(2, 18_000, 72_000)); // 9/4
    assert_eq!(candidate.edge_deltas[0].physical_loss, ea(0, 54_000, 72_000)); // 3/4
    assert_eq!(candidate.edge_deltas[1].physical_input, ea(1, 36_000, 72_000)); // 3/2
    assert_eq!(candidate.edge_deltas[1].physical_loss, ea(0, 36_000, 72_000)); // 1/2
    assert_eq!(candidate.source_delta.physical_output, ea(2, 18_000, 72_000)); // 9/4
    assert_eq!(candidate.source_delta.physical_auxiliary, ea(0, 32_400, 72_000)); // 9/20
    assert_eq!(candidate.source_delta.physical_gross, ea(2, 50_400, 72_000)); // 27/10
    assert_eq!(candidate.source_delta.source_meter_ceil_delta_milli, 3);
    assert_eq!(candidate.source_delta.prepaid_unconverted_after, ea(0, 21_600, 72_000)); // 3/10
    assert_eq!(candidate.next.physical.source_gross, ea(2, 50_400, 72_000));
}

#[test]
fn shared_edge_capacity_and_peak_are_joint_hard_constraints() {
    let root = SyntheticRoot;
    let mut c = contract(Resource::MilliLiter, 2, 1);
    c.consumers[0].hard_peak_rate = rate(2, 1);
    let mut second = c.consumers[0].clone();
    second.id = 2;
    second.meter = 17;
    second.payer = 22;
    second.contract_ref = 21;
    c.consumers.push(second);
    c.source.physical_supply_rate = rate(10, 1);
    c.source.available_input_milli = q(10);
    c.source.hard_peak_rate = rate(4, 1);
    c.edges[0].capacity_rate = rate(3, 2);
    let base = c.opening(&root, 79).unwrap();
    let p = c.propose(&root, &base, 1, MAX_WORK).unwrap();
    assert_eq!(p.candidate, None);
    assert_eq!(p.evidence, Some(Evidence::EdgeCapacity(1)));
    assert_eq!(p.pending_reason, None);

    c.edges[0].capacity_rate = rate(10, 1);
    c.edges[0].hard_peak_rate = rate(3, 2);
    let base = c.opening(&root, 80).unwrap();
    let p = c.propose(&root, &base, 1, MAX_WORK).unwrap();
    assert_eq!(p.evidence, Some(Evidence::EdgePeak(1)));

    c.edges[0].hard_peak_rate = rate(10, 1);
    c.source.input_capacity_rate = rate(3, 1);
    c.source.hard_peak_rate = rate(10, 1);
    let base = c.opening(&root, 83).unwrap();
    let p = c.propose(&root, &base, 1, MAX_WORK).unwrap();
    assert_eq!(p.evidence, Some(Evidence::SourceInputCapacity(1)));

    c.source.input_capacity_rate = rate(10, 1);
    c.source.hard_peak_rate = rate(3, 1);
    c.consumers[0].hard_peak_rate = rate(2, 1);
    c.consumers[1].hard_peak_rate = rate(2, 1);
    let base = c.opening(&root, 84).unwrap();
    let p = c.propose(&root, &base, 1, MAX_WORK).unwrap();
    assert_eq!(p.evidence, Some(Evidence::SourcePeak(1)));
}

#[test]
fn fingerprints_and_restores_ignore_input_vector_order_but_bind_access_rights() {
    let mut a = contract(Resource::MilliLiter, 1, 3_600);
    a.consumers[0].hard_peak_rate = rate(1, 3_600);
    let mut second = a.consumers[0].clone();
    second.id = 2;
    second.meter = 17;
    second.payer = 22;
    second.contract_ref = 21;
    second.node = 3;
    second.path = vec![2];
    let mut second_edge = a.edges[0].clone();
    second_edge.id = 2;
    second_edge.to = 3;
    second_edge.physical_ref = 19;
    second_edge.loss_sink_ref = 20;
    a.edges.push(second_edge);
    a.consumers.push(second);
    a.source.physical_supply_rate = rate(2, 3_600);
    a.source.hard_peak_rate = rate(2, 3_600);
    a.source.available_input_milli = q(2);
    let mut b = a.clone();
    b.consumers.reverse();
    b.edges.reverse();
    let root = SyntheticRoot;
    assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
    let opening = a.opening(&root, 85).unwrap();
    let first = a.propose(&root, &opening, 60, MAX_WORK).unwrap().candidate.unwrap();
    let state = a.revalidate(&root, &opening, &first).unwrap();
    let restored = b.restore(&root, state.checkpoint(), state.digest().unwrap()).unwrap();
    assert_eq!(restored.checkpoint(), state.checkpoint());
    let second = b.propose(&root, &restored, 120, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!(b.revalidate(&root, &restored, &second).unwrap().cursor(), 120);

    b.consumers[0].connection_right_ref = 999;
    assert_eq!(b.authorize(&root), Err(Error::InvalidAuthorization));
}

#[test]
fn restore_rejects_logically_corrupt_checkpoint_even_when_its_hash_matches() {
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let state = c.opening(&root, 95).unwrap();
    let mut damaged = state.checkpoint();
    damaged.physical.source_gross = ea(1, 0, damaged.common_denominator);
    let trusted_digest: [u8; 32] = Sha256::digest(serde_json::to_vec(&damaged).unwrap()).into();
    assert_eq!(c.restore(&root, damaged, trusted_digest), Err(Error::CorruptCheckpoint));

    // The checkpoint is arithmetically self-consistent and has a matching trusted hash,
    // but its 60-second history exceeds this segment's single 1,000-milli source budget.
    let mut limited = contract(Resource::MilliWattHour, 1_000, 3_600);
    limited.source.physical_supply_rate = rate(250, 9);
    limited.consumers[0].required_rate = rate(250, 9);
    let mut impossible = limited.opening(&root, 96).unwrap().checkpoint();
    let historical_gross = ea(1_666, 6, 9); // 60 * (250/9) = 1,666 + 6/9
    impossible.cursor_second = 60;
    impossible.sequence = 60;
    impossible.physical = PhysicalMeters {
        consumers: vec![(1, historical_gross)],
        source_output: historical_gross,
        source_auxiliary: ea(0, 0, impossible.common_denominator),
        source_gross: historical_gross,
        edges: vec![(1, historical_gross, historical_gross, ea(0, 0, impossible.common_denominator))],
    };
    let trusted_digest: [u8; 32] = Sha256::digest(serde_json::to_vec(&impossible).unwrap()).into();
    assert_eq!(limited.restore(&root, impossible, trusted_digest), Err(Error::CorruptCheckpoint));
}

#[test]
fn opening_revocation_blocks_restore_and_existing_state_operations() {
    struct RevokedOpening;
    impl RootVerifier for RevokedOpening {
        fn verifies(&self, c: &Contract) -> bool { SyntheticRoot.verifies(c) }
        fn verifies_opening(&self, _c: &Contract, _genesis_ref: u64) -> bool { false }
        fn verify_supply(&self, c: &Contract, from: u64, until: u64, gross: ExactAmount)
            -> Option<SupplyWitness> {
            SyntheticRoot.verify_supply(c, from, until, gross)
        }
    }

    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let opening = c.opening(&root, 97).unwrap();
    let candidate = c.propose(&root, &opening, 60, MAX_WORK).unwrap().candidate.unwrap();
    let state = c.revalidate(&root, &opening, &candidate).unwrap();
    let revoked = RevokedOpening;
    assert_eq!(c.restore(&revoked, state.checkpoint(), state.digest().unwrap()), Err(Error::InvalidAuthorization));
    assert_eq!(c.propose(&revoked, &state, 120, MAX_WORK), Err(Error::InvalidAuthorization));
    assert_eq!(c.revalidate(&revoked, &opening, &candidate), Err(Error::InvalidAuthorization));
    assert_eq!(c.seal_at_change(&revoked, &state, 60, 98), Err(Error::InvalidAuthorization));
}

#[test]
fn unused_closed_edges_and_source_stock_aliases_are_rejected() {
    let mut c = contract(Resource::MilliLiter, 1, 1);
    let mut unused = c.edges[0].clone();
    unused.id = 2;
    unused.from = 4;
    unused.to = 5;
    unused.physical_ref = 19;
    unused.stock_alias = None;
    unused.open = false;
    c.edges.push(unused);
    assert_eq!(c.validate(), Err(Error::InvalidPath));

    let mut c = contract(Resource::MilliLiter, 1, 1);
    c.edges[0].stock_alias = Some(c.source.stock_ref);
    assert_eq!(c.validate(), Err(Error::UnknownContract));
}

#[test]
fn insufficient_work_is_unresolved_and_does_not_advance_or_publish_prefix() {
    let c = contract(Resource::MilliWattHour, 1, 3_600);
    let root = SyntheticRoot;
    let base = c.opening(&root, 81).unwrap();
    for budget in [0, 1] {
        let p = c.propose(&root, &base, 3_600, budget).unwrap();
        assert_eq!(p.pending_reason, Some(UnresolvedReason::WorkBudget));
        assert_eq!(p.candidate, None);
        assert_eq!(base.cursor(), 0);
    }
}

#[test]
fn one_second_exact_fraction_is_not_dumped_into_heat_or_extra_supply() {
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let base = c.opening(&root, 86).unwrap();
    let p = c.propose(&root, &base, 1, MAX_WORK).unwrap();
    let candidate = p.candidate.unwrap();
    assert_eq!(candidate.consumer_deltas[0].physical, ea(27, 7, 9));
    assert_eq!(candidate.consumer_deltas[0].credited_milli_delta, 27);
    assert_eq!(candidate.consumer_deltas[0].fractional_delivered_after, ea(0, 7, 9));
    assert_eq!(candidate.source_delta.physical_output, ea(27, 7, 9));
    assert_eq!(candidate.source_delta.physical_auxiliary, ea(0, 0, 9));
    assert_eq!(candidate.source_delta.physical_gross, ea(27, 7, 9));
    assert_eq!(candidate.source_delta.source_meter_ceil_delta_milli, 28);
    assert_eq!(candidate.source_delta.prepaid_unconverted_after, ea(0, 2, 9));
}

#[test]
fn actual_supply_witnesses_cover_each_half_open_window_and_old_proof_cannot_replay() {
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let mut state = c.opening(&root, 87).unwrap();
    let first = c.propose(&root, &state, 60, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!((first.source_witness.from_second, first.source_witness.until_second), (0, 60));
    assert_eq!(first.source_witness.physical_gross, first.source_delta.physical_gross);
    assert_eq!(first.source_delta.prepaid_unconverted_after, ea(0, 3, 9));
    state = c.revalidate(&root, &state, &first).unwrap();
    assert_eq!(c.seal_at_change(&root, &state, 60, 96).unwrap().at_second, 60);
    let second = c.propose(&root, &state, 120, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!((second.source_witness.from_second, second.source_witness.until_second), (60, 120));
    assert_eq!(second.next.fingerprint, first.next.fingerprint);
    assert_eq!(second.next.genesis_ref, 87);
    assert_eq!(second.source_delta.prepaid_unconverted_after, ea(0, 6, 9));
}

#[test]
fn unavailable_supply_witness_is_unresolved_and_never_claims_a_physical_shortage() {
    struct NoReceipt;
    impl RootVerifier for NoReceipt {
        fn verifies(&self, c: &Contract) -> bool { SyntheticRoot.verifies(c) }
        fn verifies_opening(&self, c: &Contract, g: u64) -> bool { SyntheticRoot.verifies_opening(c, g) }
        fn verify_supply(&self, _: &Contract, _: u64, _: u64, _: ExactAmount) -> Option<SupplyWitness> { None }
    }
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let base = c.opening(&NoReceipt, 88).unwrap();
    let p = c.propose(&NoReceipt, &base, 60, MAX_WORK).unwrap();
    assert_eq!(p.pending_reason, Some(UnresolvedReason::MissingSupplyWitness));
    assert_eq!(p.candidate, None);
    assert_eq!(p.evidence, None);
    assert_eq!(base.cursor(), 0);
}

#[test]
fn total_source_budget_is_shared_across_the_whole_stable_segment() {
    let mut c = contract(Resource::MilliWattHour, 100_000, 3_600);
    c.source.available_input_milli = q(1_000);
    let root = SyntheticRoot;
    let base = c.opening(&root, 90).unwrap();
    let p = c.propose(&root, &base, 60, MAX_WORK).unwrap();
    assert_eq!(p.candidate, None);
    assert_eq!(p.evidence, Some(Evidence::SourceBudget(1)));
    assert_eq!(base.cursor(), 0);
}

#[test]
fn clock_scale_source_topology_demand_and_peak_changes_reject_the_old_state() {
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let base = c.opening(&root, 91).unwrap();
    let mut changed = Vec::new();
    let mut x = c.clone(); x.binding.clock_revision += 1; changed.push(x);
    let mut x = c.clone(); x.binding.scope.development_city += 1; changed.push(x);
    let mut x = c.clone(); x.binding.scale_revision += 1; changed.push(x);
    let mut x = c.clone(); x.binding.source_revision += 1; changed.push(x);
    let mut x = c.clone(); x.edges[0].retention = ratio(9, 10); changed.push(x);
    let mut x = c.clone(); x.consumers[0].required_rate = rate(251, 9); changed.push(x);
    let mut x = c.clone(); x.consumers[0].hard_peak_rate = rate(251, 9); changed.push(x);
    for contract in changed {
        assert_eq!(contract.propose(&root, &base, 1, MAX_WORK), Err(Error::StaleBase));
    }
    let mut v1 = c.clone();
    v1.policy_version = 2;
    assert_eq!(v1.authorize(&root), Err(Error::WrongVersion));
}

#[test]
fn an_old_supply_receipt_cannot_cover_a_later_half_open_interval() {
    struct OldReceipt;
    impl RootVerifier for OldReceipt {
        fn verifies(&self, c: &Contract) -> bool { SyntheticRoot.verifies(c) }
        fn verifies_opening(&self, c: &Contract, g: u64) -> bool { SyntheticRoot.verifies_opening(c, g) }
        fn verify_supply(&self, c: &Contract, _: u64, _: u64, amount: ExactAmount) -> Option<SupplyWitness> {
            Some(SupplyWitness { source_window_ref: 100, source_ref: c.source.source_ref,
                from_second: 0, until_second: 60, physical_gross: amount, receipt_digest: [2; 32] })
        }
    }
    let c = contract(Resource::MilliWattHour, 100_000, 3_600);
    let root = SyntheticRoot;
    let opening = c.opening(&root, 89).unwrap();
    let first = c.propose(&root, &opening, 60, MAX_WORK).unwrap().candidate.unwrap();
    let state = c.revalidate(&root, &opening, &first).unwrap();
    assert_eq!(c.propose(&OldReceipt, &state, 120, MAX_WORK), Err(Error::InvalidAuthorization));
}

#[test]
fn prior_ceil_hold_never_supplies_closed_or_zero_capacity_next_window() {
    let root = SyntheticRoot;
    let mut first = contract(Resource::MilliWattHour, 100_000, 3_600);
    first.edges[0].retention = ratio(1, 2);
    first.source.physical_supply_rate = rate(50_000, 900);
    first.source.input_capacity_rate = rate(200_000, 2_700);
    first.source.hard_peak_rate = rate(200_000, 2_700);
    first.source.available_input_milli = q(266_667);
    first.source.auxiliary_ratio = ratio(1, 3);
    first.source.auxiliary_sink_ref = 18;
    first.edges[0].capacity_rate = rate(200_000, 3_600);
    first.edges[0].hard_peak_rate = rate(200_000, 3_600);
    first.consumers[0].hard_peak_rate = rate(250, 9);
    let prior = first.opening(&root, 82).unwrap();
    let c = first.propose(&root, &prior, 3_600, MAX_WORK).unwrap().candidate.unwrap();
    assert_eq!(c.source_delta.prepaid_unconverted_after, ea(0, 9, 27));
    let committed = first.revalidate(&root, &prior, &c).unwrap();
    assert_eq!(first.seal_at_change(&root, &committed, 3_000, 98), Err(Error::Boundary));
    let mut other_segment = first.clone();
    other_segment.binding.source_revision += 1;
    let foreign = other_segment.opening(&root, 97).unwrap();
    assert_eq!(first.seal_at_change(&root, &foreign, 0, 98), Err(Error::StaleBase));
    let sealed = first.seal_at_change(&root, &committed, 3_600, 99).unwrap();
    assert_eq!(sealed.checkpoint.physical.source_gross, ea(266_666, 18, 27));

    let mut next = contract(Resource::MilliWattHour, 1, 3_600);
    next.binding.effective_start = 3_600;
    next.binding.effective_end = 7_200;
    next.window = Window { start_second: 3_600, end_second: 7_200 };
    next.source.physical_supply_rate = rate(0, 1);
    next.source.available_input_milli = q(1);
    next.edges[0].from = next.source.node;
    let base = next.opening(&root, 83).unwrap();
    assert_eq!(base.checkpoint().physical.source_gross,
        ea(0, 0, next.precision_denominator().unwrap()));
    let p = next.propose(&root, &base, 3_601, MAX_WORK).unwrap();
    assert_eq!(p.candidate, None);
    assert_eq!(p.evidence, Some(Evidence::SourceRate(1)));
}
