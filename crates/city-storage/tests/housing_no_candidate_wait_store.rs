//! Issue #26: real Store no-eligible-candidate Wait, recovery and CAS retry.
//! Minimal fixture helpers copied from housing_move_store_runtime.rs at v173
//! (3f48b3243eafb495faffd7a39ec9e73d4a65e935); none of its tests are included.
//! Source only: author did not run compilation, formatting or tests.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, LeaseTerms, MoveCostQuote, MoveScore, MoveWait, OpeningDwelling,
        OpeningHousehold, OpeningHousing, OpeningMoveAssessment, OpeningMoveCandidate,
        MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
    },
    model_rows::Rows,
    timeline::{Claim, Command as T, Cycle, PayrollSpec, RentClaimSource, DAY},
    transaction::{Action, Caller, Envelope, TxError},
    units::{GameTime, Money},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const RULES: &str = "m0-storage-process-fixture-v1";
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
const TENANT: u32 = 2;
const EMPLOYER: u32 = 3;
const LANDLORD: u32 = 4;
const PROVIDER: u32 = 3;
const HOUSEHOLD: u32 = 501;
const OLD_UNIT: u32 = 101;
const NEW_UNIT: u32 = 102;
const OLD_OFFER: u64 = 9_001;
const NEW_OFFER: u64 = 9_002;
const OLD_LEASE: u32 = 9_001;
const NEW_LEASE: u32 = 9_002;
const ASSESSMENT: u64 = 501;
const CITY: u32 = 1;
const ACCEPT_AT: u64 = 14 * DAY;
const MOVE_AT: u64 = 38 * DAY;
const OLD_RENT: i64 = 480_000;
const NEW_RENT: i64 = 40_000;
const MOVING_FEE: i64 = 10_000;
const OPENING_TENANT_CASH: i64 = 85_000;

// An explicitly configured, already mounted external evidence root is mandatory.
// Exclusive directory creation fails on collision. No Drop cleanup, including on failure.
fn artifact_world() -> PathBuf {
    let root = PathBuf::from(std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
        .expect("root must supply MYCITY_TEST_ARTIFACT_ROOT on the external evidence volume"));
    assert!(root.is_absolute() && root.is_dir());
    let root = fs::canonicalize(root).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/"));
    let path = root.join(format!(
        "issue26-no-candidate-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    eprintln!("ISSUE26_NO_CANDIDATE_ARTIFACT_PRESERVED={}", path.display());
    path.join("world")
}

fn money(value: i64) -> Money {
    Money::new(value).unwrap()
}

fn game_time(value: u64) -> GameTime {
    GameTime::new(i64::try_from(value).unwrap()).unwrap()
}

fn score(
    lower: u8,
    upper: u8,
    environment_source: u64,
    public_service_source: u64,
    assessed_at: u64,
) -> MoveScore {
    MoveScore {
        lower,
        upper,
        fully_covered: true,
        environment_source,
        public_service_source,
        source_version: 1,
        assessed_at: game_time(assessed_at),
        valid_through: game_time(120 * DAY),
        hard_failure_mask: 0,
    }
}

fn dwelling(unit_id: u32, building_id: u32) -> OpeningDwelling {
    OpeningDwelling {
        fact: DwellingFact {
            unit_id,
            building_id,
            owner_account: LANDLORD,
            building_unit_capacity: 1,
            person_capacity: 4,
            delivered: true,
            legal_to_occupy: true,
            has_legal_access: true,
            source_version: 1,
        },
        origin: DwellingOriginKind::ImmutableGenesisEndowment,
        asset_source_id: 7_000 + u64::from(unit_id),
    }
}

fn offer(
    offer_id: u64,
    lease_id: u32,
    unit_id: u32,
    building_id: u32,
    rent: i64,
    starts_at: u64,
) -> GenesisLeaseOffer {
    let terms = LeaseTerms {
        lease_id,
        clause_version: 1,
        contract_source_id: 9_100 + u64::from(lease_id),
        landlord_authorization_id: 9_200 + u64::from(lease_id),
        tenant_consent_id: 9_300 + u64::from(lease_id),
        rent_per_cycle: money(rent),
        cycle: Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: 1,
        starts_at: game_time(starts_at),
    };
    GenesisLeaseOffer {
        offer_id,
        world_id: 1,
        branch_id: 1,
        unit_id,
        building_id,
        owner_account: LANDLORD,
        tenant_account: TENANT,
        household_id: HOUSEHOLD,
        valid_from: GameTime::ZERO,
        valid_until: Date::new(2001, 4, 1).unwrap().midnight(),
        asset_source_id: 7_000 + u64::from(unit_id),
        authority_source_id: 8_000 + offer_id,
        contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
        lease_id,
        clause_version: terms.clause_version,
        rent_per_cycle: terms.rent_per_cycle,
        cycle: terms.cycle,
        deposit: Money::ZERO,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: lease_terms_sha256(terms).unwrap(),
    }
}

fn opening_housing(city_id: Option<u32>) -> OpeningHousing {
    let unit_limit = 2;
    let old_offer = offer(OLD_OFFER, OLD_LEASE, OLD_UNIT, 10, OLD_RENT, ACCEPT_AT);
    let new_offer = offer(NEW_OFFER, NEW_LEASE, NEW_UNIT, 11, NEW_RENT, MOVE_AT);
    let old_score = score(10, 20, 5_001, 5_002, ACCEPT_AT);
    let candidate_score = score(60, 70, 5_003, 5_004, ACCEPT_AT);
    let assessment = OpeningMoveAssessment {
        assessment_id: ASSESSMENT,
        household_id: HOUSEHOLD,
        source_id: 5_005,
        version: 1,
        old_score,
        low_since: Some(GameTime::ZERO),
        covered_through: game_time(120 * DAY),
        continuous_low_and_covered: true,
        cooldown_until: None,
        essential_costs_per_month: Money::ZERO,
    };
    let candidate = OpeningMoveCandidate {
        assessment_id: ASSESSMENT,
        offer_id: NEW_OFFER,
        score: candidate_score,
        move_cost: MoveCostQuote {
            source_id: 5_006,
            valid_through: game_time(120 * DAY),
            reachable: true,
            travel_seconds: 0,
            moving_charge: money(MOVING_FEE),
            moving_provider_account: PROVIDER,
            old_lease_close_due: Money::ZERO,
            old_close_payee_account: LANDLORD,
            old_clause_version: 1,
            old_contract_exit_source_id: 9_100 + u64::from(OLD_LEASE),
            old_escrow_account: 1,
            deposit_resolution_source_id: 0,
            old_deposit_refund_authorized: Money::ZERO,
            old_deposit_deduction_authorized: Money::ZERO,
            old_deposit_deduction_account: 0,
        },
    };

    let mut dwellings = Rows::<OpeningDwelling, MAX_M0_HOUSING_UNITS>::new().unwrap();
    dwellings.try_push(dwelling(OLD_UNIT, 10)).unwrap();
    dwellings.try_push(dwelling(NEW_UNIT, 11)).unwrap();
    let mut households = Rows::<OpeningHousehold, MAX_M0_HOUSING_UNITS>::new().unwrap();
    households
        .try_push(OpeningHousehold {
            household_id: HOUSEHOLD,
            account: TENANT,
            people: 3,
            source_id: 8_001,
        })
        .unwrap();
    let mut lease_offers = Rows::<GenesisLeaseOffer, MAX_GENESIS_LEASE_OFFERS>::new().unwrap();
    lease_offers.try_push(old_offer).unwrap();
    lease_offers.try_push(new_offer).unwrap();
    let mut move_assessments = Rows::new().unwrap();
    let mut move_candidates = Rows::new().unwrap();
    if city_id.is_some() {
        move_assessments.try_push(assessment).unwrap();
        move_candidates.try_push(candidate).unwrap();
    }

    OpeningHousing {
        unit_limit,
        city_id,
        dwellings,
        households,
        lease_offers,
        move_assessments,
        move_candidates,
    }
}

fn create_with_housing(path: &Path, housing: OpeningHousing) -> Store {
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-region-opening.json")).unwrap();
    // Keep the region identity needed by housing, while making this fixture's timeline
    // independent of inter-city transit and recurring municipal obligations.
    opening["region"]["routes"] = json!([]);
    opening["region"]["obligations"] = json!([]);
    opening["accounts"][1]["cash"] = json!(OPENING_TENANT_CASH);
    opening["sources"][1]["amount"] = json!(OPENING_TENANT_CASH);
    opening["housing"] = serde_json::to_value(housing).unwrap();
    Store::create(path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES).unwrap()
}
fn envelope(store: &Store, action: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: store.committed_sequence() + 1,
        expected_revision: store.engine().unwrap().world().view().revision,
        actions: vec![action],
    }
}

fn execute(store: &mut Store, caller: Caller, action: Action) {
    let request = envelope(store, action);
    let request_diagnostic = format!("{request:?}");
    let outcome = store.execute(caller, request).unwrap();
    assert!(
        outcome.model.is_ok(),
        "request={request_diagnostic}; outcome={outcome:?}"
    );
    assert!(outcome.has_persistent_receipt);
}
fn timeline(command: T) -> Action {
    Action::Timeline { command }
}

fn drive_to(store: &mut Store, target: u64) {
    for _ in 0..10_000 {
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        if progress.target.is_none() && progress.last_complete == target {
            return;
        }
        execute(
            store,
            Caller::System,
            timeline(T::Advance { target, limit: 256 }),
        );
    }
    panic!("Timeline did not complete target {target}");
}

fn add_payroll(store: &mut Store) {
    execute(store, Caller::System, timeline(T::Enable));
    execute(
        store,
        Caller::System,
        timeline(T::AddPayroll {
            spec: PayrollSpec {
                id: 700,
                payer: EMPLOYER,
                payee: TENANT,
                home_city: 2,
                hourly_cents: money(875),
                cycle: Cycle::Weekly,
                ends_at: 150 * DAY,
            },
        }),
    );
}

fn accept_old_lease(store: &mut Store) {
    let offer = opening_housing(Some(CITY)).lease_offers[0];
    execute(
        store,
        Caller::Account(TENANT),
        Action::HousingLease {
            offer_id: OLD_OFFER,
            offer_sha256: genesis_lease_offer_sha256(offer),
        },
    );
}

fn rent_claim(store: &Store, lease_id: u32) -> Claim {
    *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claims
        .iter()
        .find(|claim| claim.id == u64::from(lease_id))
        .unwrap()
}

fn account_cash(store: &Store, account_id: u32) -> i64 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .unwrap()
        .cash
        .value()
}

fn total_cash(store: &Store) -> i64 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| account.cash.value())
        .sum()
}
fn move_action(expected_wait_operation_id: u64, city_id: u32) -> Action {
    Action::HousingMove {
        assessment_id: ASSESSMENT,
        city_id,
        expected_wait_operation_id,
    }
}

fn assert_cash(store: &Store, moved: bool) {
    // Independent literal oracle: five weekly wages of 49,000, January invoice
    // ceil(480,000 * 17/31) = 263,226; only a Move pays the 10,000 fee.
    assert_eq!(total_cash(store), 2_885_000);
    assert_eq!(account_cash(store, 1), 1_000_000);
    assert_eq!(account_cash(store, TENANT), if moved { 56_774 } else { 66_774 });
    assert_eq!(account_cash(store, EMPLOYER), if moved { 365_000 } else { 355_000 });
    assert_eq!(account_cash(store, LANDLORD), 1_463_226);
}

fn assert_lease_and_claim(store: &Store, moved: bool) {
    let housing = store.engine().unwrap().world().view().housing.unwrap();
    assert_eq!(housing.units.len(), 2);
    assert_eq!(housing.consumed_offer_ids.as_slice(), if moved { &[9_001, 9_002][..] } else { &[9_001][..] });
    assert_eq!(housing.units.iter().filter(|u| u.household_id == HOUSEHOLD).count(), 1);
    let old = housing.units.iter().find(|u| u.unit_id == OLD_UNIT).unwrap();
    let new = housing.units.iter().find(|u| u.unit_id == NEW_UNIT).unwrap();
    if moved {
        assert_eq!((old.household_id, old.lease_id, old.arrears_ref), (0, 0, 0));
        assert_eq!((new.household_id, new.lease_id), (HOUSEHOLD, NEW_LEASE));
    } else {
        assert_eq!((old.household_id, old.lease_id), (HOUSEHOLD, OLD_LEASE));
        assert_eq!(old.rent_per_cycle.value(), 480_000);
        assert_eq!((new.household_id, new.lease_id), (0, 0));
    }
    let claim = rent_claim(store, OLD_LEASE);
    assert_eq!((claim.id, claim.payer, claim.payee, claim.due), (9_001, 2, 4, 31 * DAY));
    assert!(claim.processed);
    assert_eq!(claim.paid.value(), 263_226);
    assert_eq!(claim.principal.value(), if moved { 383_226 } else { 263_226 });
    assert_eq!(claim.principal.value() - claim.paid.value(), if moved { 120_000 } else { 0 });
    assert_eq!(store.engine().unwrap().timeline().unwrap().rent_source_for_claim(&claim),
        Some(RentClaimSource { landlord: 4, tenant: 2, lease_id: 9_001, unit_id: 101 }));
    let runtime = store.engine().unwrap().housing_move_state().unwrap();
    assert_eq!(runtime.history().len(), if moved { 1 } else { 0 });
    if moved {
        assert!(runtime.waits().is_empty());
        assert_eq!(runtime.history()[0].claim_id, 9_001);
        assert_eq!(runtime.history()[0].closed_at, game_time(38 * DAY));
        assert_eq!(runtime.history()[0].source,
            RentClaimSource { landlord: 4, tenant: 2, lease_id: 9_001, unit_id: 101 });
        let new_claim = rent_claim(store, NEW_LEASE);
        assert_eq!((new_claim.principal.value(), new_claim.paid.value(), new_claim.due),
            (30_000, 0, 59 * DAY));
    }
    assert_cash(store, moved);
}

fn exercise_no_candidate_wait(reachable: bool) {
    let path = artifact_world();
    let mut housing = opening_housing(Some(CITY));
    // This is fixture assembly BEFORE Store creation, never a runtime authority edit.
    // All scores, sources, offers, versions and identity stay unchanged across recovery.
    housing.move_candidates[0].move_cost.reachable = reachable;
    let mut store = create_with_housing(&path, housing);
    let genesis_bytes = fs::read(path.join("genesis.json")).unwrap();
    add_payroll(&mut store);
    drive_to(&mut store, ACCEPT_AT);
    accept_old_lease(&mut store);
    drive_to(&mut store, 37 * DAY);
    let summary = store.engine().unwrap().housing_paid_wage_window(HOUSEHOLD).unwrap();
    assert_eq!(summary.status, mycity_core::housing::PaidWageWindowStatus::Complete);
    assert_eq!(summary.actual_paid_wages.value(), 245_000);
    assert_eq!((summary.start_day, summary.end_day), (Some(6), Some(37)));
    let before_housing = serde_json::to_value(store.engine().unwrap().world().view().housing).unwrap();
    let before_timeline = serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap();
    let before_hash = store.engine().unwrap().state_hash();
    let before_sequence = store.committed_sequence();
    let request = envelope(&store, move_action(0, CITY));
    let outcome = store.execute(Caller::Account(TENANT), request.clone()).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
    assert_eq!(outcome.committed_sequence, before_sequence + 1);
    assert_eq!(outcome.model.unwrap().operation_id, request.operation_id);
    let wait = store.engine().unwrap().housing_move_state().unwrap().wait_for(HOUSEHOLD).unwrap();
    assert_eq!(wait.reason, if reachable { MoveWait::CandidateNotBetter } else { MoveWait::NoReachableCandidate });
    assert_eq!(wait.operation_id, request.operation_id);
    assert_eq!((wait.household_id, wait.city_id, wait.assessment_id, wait.assessment_version),
        (501, 1, 501, 1));
    assert_eq!(wait.observed_at, game_time(37 * DAY));
    assert_eq!(store.engine().unwrap().housing_move_state().unwrap().waits(), &[wait]);
    // A new Wait changes model revision/runtime and persistence sequence; it does not
    // move money, alter occupancy, accrue rent or advance the Timeline cursor.
    assert_ne!(store.engine().unwrap().state_hash(), before_hash);
    assert_eq!(serde_json::to_value(store.engine().unwrap().world().view().housing).unwrap(), before_housing);
    assert_eq!(serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap(), before_timeline);
    assert_lease_and_claim(&store, false);
    let wait_hash = store.engine().unwrap().state_hash();
    let wait_world_hash = store.engine().unwrap().world().state_hash();
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(fs::read(path.join("genesis.json")).unwrap(), genesis_bytes);
    assert_eq!(store.committed_sequence(), outcome.committed_sequence);
    assert_eq!(store.engine().unwrap().state_hash(), wait_hash);
    assert_eq!(store.engine().unwrap().world().state_hash(), wait_world_hash);
    assert_eq!(store.engine().unwrap().housing_move_state().unwrap().wait_for(HOUSEHOLD), Some(wait));
    assert_eq!(serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap(), before_timeline);
    let replay = store.execute(Caller::Account(TENANT), request.clone()).unwrap();
    assert_eq!(replay.model, outcome.model);
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.committed_sequence, outcome.committed_sequence);
    assert_eq!(store.committed_sequence(), outcome.committed_sequence);
    assert_eq!(store.engine().unwrap().state_hash(), wait_hash);
    assert_lease_and_claim(&store, false);

    // A fresh operation with missing CAS is a durable failed receipt, not another Wait.
    let stale_request = envelope(&store, move_action(0, CITY));
    let stale = store.execute(Caller::Account(TENANT), stale_request.clone()).unwrap();
    assert_eq!(stale.model, Err(TxError::InvalidOperation));
    assert!(stale.has_persistent_receipt);
    assert_eq!(stale.committed_sequence, outcome.committed_sequence + 1);
    assert_eq!(store.engine().unwrap().state_hash(), wait_hash);
    assert_eq!(store.engine().unwrap().housing_move_state().unwrap().wait_for(HOUSEHOLD), Some(wait));
    assert_lease_and_claim(&store, false);

    // Only legal Timeline advancement changes eligibility: the ORIGINAL offer starts
    // at day 38. No new Opening, profile, assessment, score or household is supplied.
    drive_to(&mut store, MOVE_AT);
    assert_eq!(store.engine().unwrap().housing_move_state().unwrap().wait_for(HOUSEHOLD), Some(wait));
    let retry_before_sequence = store.committed_sequence();
    let retry_before_timeline = serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap();
    let retry_request = envelope(&store, move_action(wait.operation_id, CITY));
    let retry = store.execute(Caller::Account(TENANT), retry_request.clone()).unwrap();
    assert!(retry.model.is_ok(), "{retry:?}");
    assert!(retry.has_persistent_receipt);
    assert_eq!(retry.committed_sequence, retry_before_sequence + 1);
    assert_eq!(retry.model.unwrap().operation_id, retry_request.operation_id);
    assert_lease_and_claim(&store, reachable);
    if !reachable {
        assert_eq!(serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap(), retry_before_timeline);
        let renewed = store.engine().unwrap().housing_move_state().unwrap().wait_for(HOUSEHOLD).unwrap();
        assert_eq!(renewed.reason, MoveWait::NoReachableCandidate);
        assert_eq!(renewed.operation_id, retry_request.operation_id);
        assert_eq!(renewed.observed_at, game_time(38 * DAY));
        assert_eq!((renewed.assessment_id, renewed.assessment_version), (wait.assessment_id, wait.assessment_version));
        assert_eq!(store.engine().unwrap().housing_move_state().unwrap().waits(), &[renewed]);
    }
    let final_hash = store.engine().unwrap().state_hash();
    let final_world_hash = store.engine().unwrap().world().state_hash();
    let final_timeline = serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap();
    let final_waits = store.engine().unwrap().housing_move_state().unwrap().waits().to_vec();
    let final_sequence = store.committed_sequence();
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(fs::read(path.join("genesis.json")).unwrap(), genesis_bytes);
    assert_eq!(store.engine().unwrap().state_hash(), final_hash);
    assert_eq!(store.engine().unwrap().world().state_hash(), final_world_hash);
    assert_eq!(store.committed_sequence(), retry.committed_sequence);
    assert_eq!(store.engine().unwrap().housing_move_state().unwrap().waits(), final_waits.as_slice());
    for (original_request, original_outcome) in [(request, outcome), (stale_request, stale), (retry_request, retry)] {
        let replay = store.execute(Caller::Account(TENANT), original_request).unwrap();
        assert_eq!(replay.model, original_outcome.model);
        assert!(replay.has_persistent_receipt);
        // Outcome reports the current durable Store cursor even for an older receipt.
        assert_eq!(replay.committed_sequence, final_sequence);
        assert_eq!(store.committed_sequence(), final_sequence);
        assert_eq!(store.engine().unwrap().state_hash(), final_hash);
        assert_eq!(store.engine().unwrap().world().state_hash(), final_world_hash);
        assert_eq!(serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap(), final_timeline);
        assert_lease_and_claim(&store, reachable);
    }
}

#[test]
fn unreachable_candidate_wait_survives_cold_open_and_original_token_retry() {
    exercise_no_candidate_wait(false);
}

#[test]
fn not_yet_eligible_candidate_wait_moves_with_original_token_when_lease_start_arrives() {
    exercise_no_candidate_wait(true);
}
