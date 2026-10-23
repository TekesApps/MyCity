//! Store-level one-shot lease acceptance against the shared housing/Timeline integration.
//!
//! This is a finite housing prototype. It verifies a real immutable-opening offer, actual payroll
//! cash, first-period proration, recurring Claim accrual/settlement, and Store recovery.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, LeaseTerms, OpeningDwelling, OpeningHousehold, OpeningHousing,
        MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
    },
    model_rows::Rows,
    schedule::Phase,
    timeline::{Claim, ClaimSource, Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_storage::{Error as StorageError, Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "m0-storage-process-fixture-v1";
static ID: AtomicU64 = AtomicU64::new(0);
const TENANT: u32 = 2;
const EMPLOYER: u32 = 3;
const LANDLORD: u32 = 4;
const UNIT: u32 = 101;
const HOUSEHOLD: u32 = 501;
const OFFER: u64 = 9001;
const LEASE: u32 = 9001;
const ACCEPT_AT: u64 = 14 * DAY;
const FIRST_DUE: u64 = 31 * DAY;
const SECOND_DUE: u64 = 59 * DAY;
const MONTHLY_RENT_CENTS: i64 = 480_000;
const FIRST_INVOICE_CENTS: i64 = 263_226; // ceil(480000 * 17 occupied days / 31 days)
const PERIOD_PAY_CENTS: i64 = 44_800; // 800 cents/hour * 56 hours

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-housing-runtime-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("HOUSING_RUNTIME_ARTIFACT_CREATED={}", path.display());
        Self(path)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("HOUSING_RUNTIME_ARTIFACT_PRESERVED={}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn m(value: i64) -> Money {
    Money::new(value).unwrap()
}

fn opening_housing() -> OpeningHousing {
    let fact = DwellingFact {
        unit_id: UNIT,
        building_id: 10,
        owner_account: LANDLORD,
        building_unit_capacity: 1,
        person_capacity: 4,
        delivered: true,
        legal_to_occupy: true,
        has_legal_access: true,
        source_version: 1,
    };
    let terms = LeaseTerms {
        lease_id: LEASE,
        clause_version: 1,
        contract_source_id: 9101,
        landlord_authorization_id: 9102,
        tenant_consent_id: 9103,
        rent_per_cycle: m(MONTHLY_RENT_CENTS),
        cycle: Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: 1,
        starts_at: GameTime::new(ACCEPT_AT as i64).unwrap(),
    };
    let offer = GenesisLeaseOffer {
        offer_id: OFFER,
        world_id: 1,
        branch_id: 1,
        unit_id: UNIT,
        building_id: fact.building_id,
        owner_account: LANDLORD,
        tenant_account: TENANT,
        household_id: HOUSEHOLD,
        valid_from: GameTime::ZERO,
        valid_until: Date::new(2001, 2, 1).unwrap().midnight(),
        asset_source_id: 7001,
        authority_source_id: 7002,
        contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
        lease_id: LEASE,
        clause_version: terms.clause_version,
        rent_per_cycle: terms.rent_per_cycle,
        cycle: terms.cycle,
        deposit: Money::ZERO,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: lease_terms_sha256(terms).unwrap(),
    };

    let mut dwellings = Rows::<OpeningDwelling, MAX_M0_HOUSING_UNITS>::new().unwrap();
    dwellings
        .try_push(OpeningDwelling {
            fact,
            origin: DwellingOriginKind::ImmutableGenesisEndowment,
            asset_source_id: 7001,
        })
        .unwrap();
    let mut households = Rows::<OpeningHousehold, MAX_M0_HOUSING_UNITS>::new().unwrap();
    households
        .try_push(OpeningHousehold {
            household_id: HOUSEHOLD,
            account: TENANT,
            people: 3,
            source_id: 8001,
        })
        .unwrap();
    let mut lease_offers = Rows::<GenesisLeaseOffer, MAX_GENESIS_LEASE_OFFERS>::new().unwrap();
    lease_offers.try_push(offer).unwrap();

    OpeningHousing {
        unit_limit: 1,
        dwellings,
        households,
        lease_offers,
    }
}

fn create_with_household_cash(path: &Path, household_cash: i64) -> Store {
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    opening["accounts"][1]["cash"] = json!(household_cash);
    opening["sources"][1]["amount"] = json!(household_cash);
    opening["housing"] = serde_json::to_value(opening_housing()).unwrap();
    Store::create(path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES).unwrap()
}

fn envelope(s: &Store, action: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![action],
    }
}

fn execute(s: &mut Store, caller: Caller, action: Action) {
    let request = envelope(s, action);
    let outcome = s.execute(caller, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
}

fn timeline(command: T) -> Action {
    Action::Timeline { command }
}

fn drive(s: &mut Store, target: u64) {
    drive_with_limit(s, target, 256);
}

fn drive_with_limit(s: &mut Store, target: u64, limit: u16) {
    for _ in 0..10_000 {
        execute(s, Caller::System, timeline(T::Advance { target, limit }));
        if s.engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("Timeline did not complete target {target}");
}

/// Completes the preceding hour, then uses one fixed-size page limit for the entire final-hour
/// continuation. Stops after P1 accrues rent but before P2 settles Claims.
fn advance_to_rent_p2(s: &mut Store, target: u64) -> Envelope {
    let last_hour = target.checked_sub(3600).expect("due is after one hour");
    let before = s.engine().unwrap().timeline().unwrap().progress;
    assert!(
        before.target.is_none(),
        "no earlier Advance may remain pending"
    );
    assert!(
        before.last_complete < target,
        "target must be in the future"
    );
    if before.last_complete < last_hour {
        drive(s, last_hour);
    }
    let staged = s.engine().unwrap().timeline().unwrap().progress;
    assert!(
        staged.target.is_none(),
        "pre-boundary target must be complete"
    );
    assert!(staged.last_complete >= last_hour && staged.last_complete < target);

    let mut last_operation = None;
    for _ in 0..10_000 {
        let progress = s.engine().unwrap().timeline().unwrap().progress;
        if progress.at == target
            && progress.phase == Some(Phase::P2)
            && progress.lane == 0
            && progress.cursor == 0
        {
            return last_operation.expect("P1 boundary operation was recorded");
        }
        let operation = envelope(s, timeline(T::Advance { target, limit: 1 }));
        let outcome = s.execute(Caller::System, operation.clone()).unwrap();
        assert!(outcome.model.is_ok(), "{outcome:?}");
        assert!(outcome.has_persistent_receipt);
        last_operation = Some(operation);
    }
    panic!("Timeline did not stop before P2 at {target}");
}

fn household_cash(s: &Store) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == TENANT)
        .unwrap()
        .cash
        .value()
}

fn employer_cash(s: &Store) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == EMPLOYER)
        .unwrap()
        .cash
        .value()
}

fn landlord_cash(s: &Store) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == LANDLORD)
        .unwrap()
        .cash
        .value()
}

fn total_cash(s: &Store) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| account.cash.value())
        .sum()
}

fn rent_claim(s: &Store) -> Claim {
    *s.engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claims
        .iter()
        .find(|claim| claim.id == u64::from(LEASE))
        .expect("lease has one native Timeline Claim")
}

fn housing_json(s: &Store) -> Value {
    let bytes = s.engine().unwrap().export_checkpoint().unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["world"]["housing"].clone()
}

fn offer_sha256() -> [u8; 32] {
    genesis_lease_offer_sha256(opening_housing().lease_offers[0])
}

fn lease_action(sha256: [u8; 32]) -> Action {
    Action::HousingLease {
        offer_id: OFFER,
        offer_sha256: sha256,
    }
}

fn setup_at_lease_acceptance_with_cash(path: &Path, opening_household_cash: i64) -> Store {
    let mut s = create_with_household_cash(path, opening_household_cash);
    execute(&mut s, Caller::System, timeline(T::Enable));
    execute(
        &mut s,
        Caller::System,
        timeline(T::AddPayroll {
            spec: PayrollSpec {
                id: 700,
                payer: EMPLOYER,
                payee: TENANT,
                home_city: 0,
                hourly_cents: m(800),
                cycle: Cycle::Weekly,
                ends_at: 150 * DAY,
            },
        }),
    );
    drive(&mut s, ACCEPT_AT);
    assert_eq!(
        household_cash(&s),
        opening_household_cash + 2 * PERIOD_PAY_CENTS
    );
    assert_eq!(employer_cash(&s), 600_000 - 2 * PERIOD_PAY_CENTS);
    s
}

fn setup_at_lease_acceptance(path: &Path) -> Store {
    setup_at_lease_acceptance_with_cash(path, 0)
}

fn reject(s: &mut Store, caller: Caller, action: Action) {
    let before = s.business_digest().unwrap();
    let outcome = s.execute(caller, envelope(s, action)).unwrap();
    assert!(
        outcome.model.is_err(),
        "invalid lease unexpectedly accepted"
    );
    assert!(outcome.has_persistent_receipt);
    assert_eq!(
        s.business_digest().unwrap(),
        before,
        "rejected lease changed business state"
    );
}

#[test]
fn genesis_offer_lease_proration_payroll_p2_and_cold_recovery_are_atomic() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = setup_at_lease_acceptance(&path);
    let valid_sha = offer_sha256();
    assert_eq!(total_cash(&s), 2_800_000);

    reject(&mut s, Caller::Account(EMPLOYER), lease_action(valid_sha));
    let mut altered_sha = valid_sha;
    altered_sha[0] ^= 0x80;
    reject(&mut s, Caller::Account(TENANT), lease_action(altered_sha));

    // A failure before WAL admission leaves the same operation retryable and no lease/Claim.
    let request = envelope(&s, lease_action(valid_sha));
    let sequence_before_precommit = s.committed_sequence();
    let before_precommit = s.business_digest().unwrap();
    assert!(s
        .execute_with_observer(Caller::Account(TENANT), request.clone(), |at| {
            if at == WritePoint::Prepared {
                Err(std::io::Error::other("injected housing precommit failure"))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(matches!(s.engine(), Err(StorageError::RecoveryRequired)));
    drop(s);

    // Prepared is before WAL append. The observer error poisons this Store instance, so recover
    // from the same world directory and prove the operation was not committed before retry.
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), sequence_before_precommit);
    assert_eq!(s.business_digest().unwrap(), before_precommit);
    assert!(s
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claims
        .iter()
        .all(|claim| claim.id != u64::from(LEASE)));
    let recovered_housing = housing_json(&s);
    assert_eq!(
        recovered_housing["state"]["units"].as_array().unwrap()[0]["household_id"],
        0
    );

    assert!(s
        .execute(Caller::Account(TENANT), request)
        .unwrap()
        .model
        .is_ok());

    let claim = rent_claim(&s);
    assert_eq!(claim.id, u64::from(LEASE));
    assert_eq!(claim.payer, TENANT);
    assert_eq!(claim.payee, LANDLORD);
    assert_eq!(claim.due, FIRST_DUE);
    assert_eq!(claim.principal, m(FIRST_INVOICE_CENTS));
    assert_eq!(claim.paid, Money::ZERO);
    assert!(!claim.processed);
    let rent_source = s
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claim_sources
        .iter()
        .find(|source| source.claim_id == claim.id)
        .unwrap();
    assert_eq!(
        rent_source.source,
        ClaimSource::Rent {
            details: mycity_core::timeline::RentClaimSource {
                landlord: LANDLORD,
                tenant: TENANT,
                lease_id: LEASE,
                unit_id: UNIT,
            }
        }
    );
    assert_eq!(household_cash(&s), 2 * PERIOD_PAY_CENTS);
    assert_eq!(landlord_cash(&s), 1_200_000);

    let housing = housing_json(&s);
    let units = housing["state"]["units"].as_array().unwrap();
    assert_eq!(units.len(), 1, "one registered physical unit only");
    assert_eq!(units[0]["unit_id"], UNIT);
    assert_eq!(units[0]["household_id"], HOUSEHOLD);
    assert_eq!(units[0]["lease_id"], LEASE);
    assert_eq!(housing["state"]["consumed_offer_ids"][0], OFFER);

    // Reusing a successfully consumed offer under a new operation cannot create another lease.
    reject(&mut s, Caller::Account(TENANT), lease_action(valid_sha));
    assert_eq!(rent_claim(&s), claim);

    // CP reopen retains the offer receipt, future due and original Claim without charging early.
    let lease_sequence = s.committed_sequence();
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), lease_sequence);
    assert_eq!(rent_claim(&s), claim);
    assert_eq!(household_cash(&s), 2 * PERIOD_PAY_CENTS);
    assert_eq!(housing_json(&s)["state"]["consumed_offer_ids"][0], OFFER);

    // The first invoice is due at the existing Gregorian Cycle boundary, after payroll has
    // deposited four weekly wages. Inject a lost response after durable application, then cold
    // open and replay the exact same operation envelope.
    let due = envelope(
        &s,
        timeline(T::Advance {
            target: FIRST_DUE,
            limit: 256,
        }),
    );
    let due_operation = due.operation_id;
    let sequence_before_due = s.committed_sequence();
    assert!(s
        .execute_with_observer(Caller::System, due.clone(), |at| {
            if at == WritePoint::Applied {
                Err(std::io::Error::other("injected lost housing P2 reply"))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(matches!(s.engine(), Err(StorageError::RecoveryRequired)));
    drop(s);

    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.committed_sequence(), sequence_before_due + 1);
    let partial_progress = s.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(partial_progress.target, Some(FIRST_DUE));
    assert!(partial_progress.last_complete < FIRST_DUE);
    let hash_at_partial_cursor = s.state_hash().unwrap();
    let replay = s.execute(Caller::System, due.clone()).unwrap();
    assert!(replay.has_persistent_receipt);
    let replay_receipt = replay.model.unwrap();
    assert_eq!(replay_receipt.operation_id, due_operation);
    assert_eq!(replay_receipt.timeline, Some(partial_progress));
    assert_eq!(s.state_hash().unwrap(), hash_at_partial_cursor);
    assert_eq!(
        s.engine().unwrap().timeline().unwrap().progress,
        partial_progress,
        "same-envelope replay advanced the persisted partial cursor"
    );

    // Continue the same target through fresh, bounded operations. One 256-step envelope is a
    // resumable slice, not a promise that a 17-day interval fits in one call.
    drive(&mut s, FIRST_DUE);
    let completed_progress = s.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(completed_progress.target, None);
    assert_eq!(completed_progress.last_complete, FIRST_DUE);
    let settled = rent_claim(&s);
    assert_eq!(settled.principal, m(FIRST_INVOICE_CENTS));
    assert_eq!(settled.paid, m(4 * PERIOD_PAY_CENTS));
    assert_eq!(settled.principal.value() - settled.paid.value(), 84_026);
    assert_eq!(settled.due, FIRST_DUE);
    assert!(settled.processed);
    assert_eq!(household_cash(&s), 0);
    assert_eq!(employer_cash(&s), 600_000 - 4 * PERIOD_PAY_CENTS);
    assert_eq!(landlord_cash(&s), 1_200_000 + 4 * PERIOD_PAY_CENTS);
    assert_eq!(
        total_cash(&s),
        2_800_000,
        "payroll and rent must conserve cash"
    );
    let business_after_due = s.state_hash().unwrap();
    let business_digest_after_due = s.business_digest().unwrap();
    let replay = s.execute(Caller::System, due).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.model.unwrap().operation_id, due_operation);
    assert_eq!(s.state_hash().unwrap(), business_after_due);
    assert_eq!(s.business_digest().unwrap(), business_digest_after_due);
    assert_eq!(rent_claim(&s), settled, "replay charged the rent twice");

    // The second Gregorian occurrence accrues into the same Claim in P1. Stop at the durable
    // P1/P2 boundary, then checkpoint and cold-open before any second-period cash is collected.
    let second_p1 = advance_to_rent_p2(&mut s, SECOND_DUE);
    let pending = s.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(pending.at, SECOND_DUE);
    assert_eq!(pending.target, Some(SECOND_DUE));
    assert_eq!(pending.phase, Some(Phase::P2));
    assert_eq!(pending.lane, 0);
    assert_eq!(pending.cursor, 0);
    let accrued = rent_claim(&s);
    assert_eq!(
        accrued.principal.value(),
        FIRST_INVOICE_CENTS + MONTHLY_RENT_CENTS
    );
    assert_eq!(accrued.paid.value(), 4 * PERIOD_PAY_CENTS);
    assert_eq!(accrued.principal.value() - accrued.paid.value(), 564_026);
    assert_eq!(
        accrued.due, FIRST_DUE,
        "oldest unpaid occurrence keeps its due date"
    );
    assert!(
        !accrued.processed,
        "P2 has not collected the second occurrence yet"
    );
    assert_eq!(household_cash(&s), 4 * PERIOD_PAY_CENTS);
    let rent_source_count = s
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claim_sources
        .iter()
        .filter(|source| source.claim_id == u64::from(LEASE))
        .count();
    assert_eq!(
        rent_source_count, 1,
        "recurrence must reuse the original Rent source"
    );
    let housing = housing_json(&s);
    let unit = &housing["state"]["units"][0];
    assert_eq!(unit["last_accounted_at"], SECOND_DUE);
    assert_eq!(unit["next_due_at"], 90 * DAY);
    assert_eq!(unit["arrears_ref"], LEASE);

    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.engine().unwrap().timeline().unwrap().progress, pending);
    assert_eq!(rent_claim(&s), accrued);
    assert_eq!(household_cash(&s), 4 * PERIOD_PAY_CENTS);
    let p1_state_hash = s.state_hash().unwrap();
    let replay = s.execute(Caller::System, second_p1.clone()).unwrap();
    assert!(replay.has_persistent_receipt);
    let receipt = replay.model.unwrap();
    assert_eq!(receipt.operation_id, second_p1.operation_id);
    assert_eq!(receipt.timeline, Some(pending));
    assert_eq!(s.state_hash().unwrap(), p1_state_hash);
    assert_eq!(
        rent_claim(&s),
        accrued,
        "P1 replay added the same month twice"
    );

    // P2 pays only the actual outstanding delta from the second month's four payroll events.
    drive_with_limit(&mut s, SECOND_DUE, 1);
    let second_paid = rent_claim(&s);
    assert_eq!(
        second_paid.principal.value(),
        FIRST_INVOICE_CENTS + MONTHLY_RENT_CENTS
    );
    assert_eq!(second_paid.paid.value(), 8 * PERIOD_PAY_CENTS);
    assert_eq!(
        second_paid.principal.value() - second_paid.paid.value(),
        384_826
    );
    assert_eq!(
        second_paid.due, FIRST_DUE,
        "second P2 must retain the earliest unpaid due"
    );
    assert!(second_paid.processed);
    assert_eq!(household_cash(&s), 0);
    assert_eq!(employer_cash(&s), 600_000 - 8 * PERIOD_PAY_CENTS);
    assert_eq!(landlord_cash(&s), 1_200_000 + 8 * PERIOD_PAY_CENTS);
    assert_eq!(
        total_cash(&s),
        2_800_000,
        "second rent P2 must conserve cash"
    );
    let after_second_payment = s.state_hash().unwrap();
    let replay = s.execute(Caller::System, second_p1).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(s.state_hash().unwrap(), after_second_payment);
    assert_eq!(
        rent_claim(&s),
        second_paid,
        "lost-reply P1 replay changed settled rent"
    );

    let opening = opening_housing();
    let opening_capacity = opening.owned_capacity_bytes();
    let state_capacity = mycity_core::housing::HousingState::from_opening(&opening)
        .unwrap()
        .owned_capacity_bytes();
    println!(
        "HOUSING_RUNTIME_ORACLE={{\"actual_store\":true,\"units\":1,\"household\":501,\"offer_consumed_once\":true,\"rent_monthly_cents\":480000,\"prorated_days\":17,\"calendar_interval_days\":31,\"first_invoice_cents\":263226,\"first_period_payroll_cents\":179200,\"first_period_paid_cents\":179200,\"first_period_arrears_cents\":84026,\"second_invoice_cents\":480000,\"second_period_payroll_cents\":179200,\"second_period_claim_principal_cents\":743226,\"second_period_claim_paid_cents\":358400,\"second_period_arrears_cents\":384826,\"earliest_due_day\":31,\"one_rent_source_reused\":true,\"p1_pending_p2_cold_open_and_replay\":true,\"checkpoint_and_cold_open\":true,\"applied_lost_reply_recovered\":true,\"configured_housing_units\":1,\"opening_owned_capacity_bytes\":{},\"housing_state_owned_capacity_bytes\":{}}}",
        opening_capacity, state_capacity,
    );
}

#[test]
fn fully_paid_first_rent_occurrence_moves_new_claim_due_to_second_calendar_boundary() {
    let temp = Temp::new();
    let path = temp.world();
    let mut s = setup_at_lease_acceptance_with_cash(&path, 100_000);
    execute(
        &mut s,
        Caller::Account(TENANT),
        lease_action(offer_sha256()),
    );
    drive(&mut s, FIRST_DUE);

    let first = rent_claim(&s);
    assert_eq!(first.principal.value(), FIRST_INVOICE_CENTS);
    assert_eq!(first.paid.value(), FIRST_INVOICE_CENTS);
    assert_eq!(first.due, FIRST_DUE);
    assert!(first.processed);
    let cash_after_first_rent = 100_000 + 4 * PERIOD_PAY_CENTS - FIRST_INVOICE_CENTS;
    assert_eq!(cash_after_first_rent, 15_974);
    assert_eq!(household_cash(&s), cash_after_first_rent);

    let _second_p1 = advance_to_rent_p2(&mut s, SECOND_DUE);
    let second = rent_claim(&s);
    assert_eq!(
        second.principal.value(),
        FIRST_INVOICE_CENTS + MONTHLY_RENT_CENTS
    );
    assert_eq!(second.paid.value(), FIRST_INVOICE_CENTS);
    assert_eq!(
        second.principal.value() - second.paid.value(),
        MONTHLY_RENT_CENTS
    );
    assert_eq!(
        second.due, SECOND_DUE,
        "a fully paid prior period must not age the new debt"
    );
    assert!(!second.processed);
    let second_period_cash = cash_after_first_rent + 4 * PERIOD_PAY_CENTS;
    assert_eq!(second_period_cash, 195_174);
    assert_eq!(household_cash(&s), second_period_cash);

    drive_with_limit(&mut s, SECOND_DUE, 1);
    let second_p2 = rent_claim(&s);
    let expected_cumulative_paid = FIRST_INVOICE_CENTS + second_period_cash;
    assert_eq!(expected_cumulative_paid, 458_400);
    assert_eq!(second_p2.paid.value(), expected_cumulative_paid);
    assert_eq!(
        second_p2.principal.value() - second_p2.paid.value(),
        284_826
    );
    assert_eq!(second_p2.due, SECOND_DUE);
    assert!(second_p2.processed);
    assert_eq!(
        household_cash(&s),
        0,
        "available rent funds were not settled"
    );
    assert_eq!(
        total_cash(&s),
        2_900_000,
        "opening cash, wages and rent must reconcile"
    );
}
