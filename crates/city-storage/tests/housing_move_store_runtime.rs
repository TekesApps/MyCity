//! Store-backed relocation slice using the existing Timeline, Claims, region identity,
//! and checkpoint recovery. This source targets the frozen move API and remains unrun
//! until the shared core candidate is integrated into this checkout.

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
use mycity_storage::{Error as StorageError, Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs, io,
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
const RELET_OFFER: u64 = 9_003;
const OLD_LEASE: u32 = 9_001;
const NEW_LEASE: u32 = 9_002;
const RELET_LEASE: u32 = 9_003;
const ASSESSMENT: u64 = 501;
const RELET_HOUSEHOLD: u32 = 502;
const RELET_TENANT: u32 = 5;
const RELET_CASH_SOURCE: u32 = 6;
const CITY: u32 = 1;
const ACTIVE_CITY_AFTER_SWITCH: u32 = 3;
const ACCEPT_AT: u64 = 14 * DAY;
const MOVE_AT: u64 = 38 * DAY;
const SETTLE_AT: u64 = 42 * DAY;
const OLD_RENT: i64 = 480_000;
const NEW_RENT: i64 = 40_000;
const RELET_RENT: i64 = 20_000;
const WEEKLY_WAGE: i64 = 49_000;
const MOVING_FEE: i64 = 10_000;
const RELET_OPENING_CASH: i64 = 30_000;
const FIRST_OLD_INVOICE: i64 = 263_226; // ceil(480000 * 17 / 31)
const OLD_FINAL_ACCRUAL: i64 = 120_000; // 480000 * 7 / 28, Feb 1 through Feb 8
const NEW_FIRST_INVOICE: i64 = 30_000; // 40000 * 21 / 28, Feb 8 through Mar 1
const OPENING_TENANT_CASH: i64 = 85_000;

struct Temp {
    path: PathBuf,
    preserve: bool,
}

impl Temp {
    fn new() -> Self {
        let configured = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT");
        let preserve = configured.is_some();
        let root = configured
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-housing-move-store-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("HOUSING_MOVE_ARTIFACT_CREATED={}", path.display());
        Self { path, preserve }
    }

    fn world(&self) -> PathBuf {
        self.path.join("world")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if self.preserve || std::thread::panicking() {
            eprintln!("HOUSING_MOVE_ARTIFACT_PRESERVED={}", self.path.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.path);
    }
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
    opening_housing_with_limit(city_id, 2)
}

fn opening_housing_with_relet() -> OpeningHousing {
    // The current prototype retains one consumed offer ID per configured dwelling for the
    // lifetime of the bounded state. This separate 3-unit case is needed for the three
    // acceptances below (old lease, destination lease, and a new lease on the vacated unit).
    // It does not prove repeated re-letting in a 2-unit world.
    let mut housing = opening_housing_with_limit(Some(CITY), 3);
    let mut relet_offer = offer(RELET_OFFER, RELET_LEASE, OLD_UNIT, 10, RELET_RENT, MOVE_AT);
    relet_offer.tenant_account = RELET_TENANT;
    relet_offer.household_id = RELET_HOUSEHOLD;
    housing
        .lease_offers
        .try_push(relet_offer)
        .expect("bounded relet offer");
    housing
        .households
        .try_push(OpeningHousehold {
            household_id: RELET_HOUSEHOLD,
            account: RELET_TENANT,
            people: 2,
            source_id: 8_002,
        })
        .expect("bounded second household");
    housing
}

fn opening_housing_with_limit(city_id: Option<u32>, unit_limit: u8) -> OpeningHousing {
    assert!((2..=MAX_M0_HOUSING_UNITS as u8).contains(&unit_limit));
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
    for extra in 0..usize::from(unit_limit - 2) {
        let unit_id = NEW_UNIT + 1 + extra as u32;
        let building_id = 12 + extra as u32;
        dwellings.try_push(dwelling(unit_id, building_id)).unwrap();
    }
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

fn create(path: &Path, city_id: Option<u32>) -> Store {
    create_with_housing(path, opening_housing(city_id))
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

fn create_with_relet(path: &Path) -> Store {
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-region-opening.json")).unwrap();
    opening["region"]["routes"] = json!([]);
    opening["region"]["obligations"] = json!([]);
    opening["accounts"][1]["cash"] = json!(OPENING_TENANT_CASH);
    opening["sources"][1]["amount"] = json!(OPENING_TENANT_CASH);
    opening["accounts"].as_array_mut().unwrap().push(json!({
        "id": RELET_TENANT,
        "role": "household",
        "cash": RELET_OPENING_CASH,
        "source": RELET_CASH_SOURCE,
    }));
    opening["sources"].as_array_mut().unwrap().push(json!({
        "id": RELET_CASH_SOURCE,
        "kind": "cash_capital",
        "amount": RELET_OPENING_CASH,
    }));
    opening["housing"] = serde_json::to_value(opening_housing_with_relet()).unwrap();
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

fn execute_request(store: &mut Store, caller: Caller, request: Envelope) {
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

fn assert_relet_world_budget(store: &Store, stage: &str) -> usize {
    let layout = store.engine().unwrap().world().layout();
    let owned = layout["owned_capacity_bytes"].as_u64().unwrap() as usize;
    let scratch = mycity_core::housing::MAX_VALIDATION_SCRATCH_BYTES;
    let budget = mycity_core::assembly::WORLD_BYTE_BUDGET;
    assert!(
        owned + scratch <= budget,
        "relet {stage} must remain inside frozen World budget: owned={owned}, scratch={scratch}, budget={budget}, layout={layout}"
    );
    eprintln!(
        "HOUSING_RELET_WORLD_BUDGET stage={stage} owned={owned} scratch={scratch} budget={budget}"
    );
    owned
}

fn move_action(expected_wait_operation_id: u64, city_id: u32) -> Action {
    Action::HousingMove {
        assessment_id: ASSESSMENT,
        city_id,
        expected_wait_operation_id,
    }
}

#[test]
fn wait_move_and_old_claim_settlement_survive_cold_open_and_lost_replies() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path, Some(CITY));
    let opening_cash_total = total_cash(&store);
    add_payroll(&mut store);
    drive_to(&mut store, ACCEPT_AT);
    accept_old_lease(&mut store);

    // The first attempt has only the initial two-week wage history. It creates a durable
    // EvidenceIncomplete Wait. The Wait records a retry CAS token; it is not move authority.
    let wait_request = envelope(&store, move_action(0, CITY));
    let wait_operation_id = wait_request.operation_id;
    execute_request(&mut store, Caller::Account(TENANT), wait_request.clone());
    let wait = store
        .engine()
        .unwrap()
        .housing_move_state()
        .unwrap()
        .wait_for(HOUSEHOLD)
        .unwrap();
    assert_eq!(wait.operation_id, wait_operation_id);
    assert_eq!(wait.city_id, CITY);
    assert_eq!(wait.reason, MoveWait::EvidenceIncomplete);
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let wait_after_open = store
        .engine()
        .unwrap()
        .housing_move_state()
        .unwrap()
        .wait_for(HOUSEHOLD)
        .unwrap();
    assert_eq!(wait_after_open, wait);
    execute_request(&mut store, Caller::Account(TENANT), wait_request.clone());
    assert_eq!(
        store
            .engine()
            .unwrap()
            .housing_move_state()
            .unwrap()
            .wait_for(HOUSEHOLD),
        Some(wait)
    );

    // At day 31 the original rent invoice is partially/currently settled by actual wages;
    // at day 38 the rolling 31-day paid-wage window is complete. The portfolio is registered
    // to city 1 while city 2 was active for payroll. Switch to city 3 before moving to prove
    // the move selector follows immutable portfolio city 1, not whichever map is active.
    drive_to(&mut store, MOVE_AT);
    let window = store
        .engine()
        .unwrap()
        .housing_paid_wage_window(HOUSEHOLD)
        .unwrap();
    assert_eq!(
        window.status,
        mycity_core::housing::PaidWageWindowStatus::Complete
    );
    assert_eq!(window.actual_paid_wages.value(), 245_000);
    assert_eq!(window.start_day, Some(7));
    assert_eq!(window.end_day, Some(38));
    assert_eq!(
        rent_claim(&store, OLD_LEASE).principal.value(),
        FIRST_OLD_INVOICE
    );
    assert_eq!(
        rent_claim(&store, OLD_LEASE).paid.value(),
        FIRST_OLD_INVOICE
    );

    execute(
        &mut store,
        Caller::System,
        Action::RegionActivate {
            city: ACTIVE_CITY_AFTER_SWITCH,
        },
    );
    let day38_payroll = *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .payroll
        .iter()
        .find(|payroll| payroll.spec.id == 700)
        .unwrap();
    assert_eq!(day38_payroll.last_accrual, MOVE_AT);
    assert_eq!(day38_payroll.accrued.value(), 266_000);
    assert_eq!(day38_payroll.billed.value(), 245_000);
    assert_eq!(day38_payroll.paid.value(), 245_000);
    assert_eq!(day38_payroll.work_seconds, 38 * 8 * 3_600);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .region
            .unwrap()
            .active_city,
        ACTIVE_CITY_AFTER_SWITCH
    );

    // A stale retry token and a request for a different region city are hard failures; neither
    // can rewrite the persisted Wait or select another city's opening portfolio.
    let stale_request = envelope(&store, move_action(0, CITY));
    let stale = store
        .execute(Caller::Account(TENANT), stale_request)
        .unwrap();
    assert!(stale.model.is_err());
    let wrong_city_request = envelope(
        &store,
        move_action(wait_operation_id, ACTIVE_CITY_AFTER_SWITCH),
    );
    let wrong_city = store
        .execute(Caller::Account(TENANT), wrong_city_request)
        .unwrap();
    assert!(wrong_city.model.is_err());
    let unknown_city_request = envelope(&store, move_action(wait_operation_id, 999));
    let unknown_city = store
        .execute(Caller::Account(TENANT), unknown_city_request)
        .unwrap();
    assert!(unknown_city.model.is_err());
    assert_eq!(
        store
            .engine()
            .unwrap()
            .housing_move_state()
            .unwrap()
            .wait_for(HOUSEHOLD),
        Some(wait)
    );

    // Simulate a lost reply after the durable Apply. Reopening the same Store must expose the
    // single move transaction; replaying its original envelope returns that receipt unchanged.
    let move_request = envelope(&store, move_action(wait_operation_id, CITY));
    let error = store
        .execute_with_observer(Caller::Account(TENANT), move_request.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other("simulated lost move reply"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert!(matches!(
        error,
        StorageError::RecoveryRequired | StorageError::Io(_)
    ));
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let old_claim_after_move = rent_claim(&store, OLD_LEASE);
    assert_eq!(old_claim_after_move.id, u64::from(OLD_LEASE));
    assert_eq!(old_claim_after_move.payer, TENANT);
    assert_eq!(old_claim_after_move.payee, LANDLORD);
    assert_eq!(old_claim_after_move.due, 31 * DAY);
    assert!(old_claim_after_move.processed);
    assert_eq!(old_claim_after_move.paid.value(), FIRST_OLD_INVOICE);
    assert_eq!(
        old_claim_after_move.principal.value(),
        FIRST_OLD_INVOICE + OLD_FINAL_ACCRUAL
    );
    let old_source = RentClaimSource {
        landlord: LANDLORD,
        tenant: TENANT,
        lease_id: OLD_LEASE,
        unit_id: OLD_UNIT,
    };
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&old_claim_after_move),
        Some(old_source)
    );
    let housing = store.engine().unwrap().world().view().housing.unwrap();
    let old_unit = housing
        .units
        .iter()
        .find(|unit| unit.unit_id == OLD_UNIT)
        .unwrap();
    let new_unit = housing
        .units
        .iter()
        .find(|unit| unit.unit_id == NEW_UNIT)
        .unwrap();
    assert_eq!((old_unit.household_id, old_unit.lease_id), (0, 0));
    assert_eq!(
        (new_unit.household_id, new_unit.lease_id),
        (HOUSEHOLD, NEW_LEASE)
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .housing_move_state()
            .unwrap()
            .wait_for(HOUSEHOLD),
        None
    );
    let history = store
        .engine()
        .unwrap()
        .housing_move_state()
        .unwrap()
        .history()
        .to_vec();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].city_id, CITY);
    assert_eq!(history[0].claim_id, u64::from(OLD_LEASE));
    assert_eq!(history[0].source, old_source);
    assert_eq!(
        rent_claim(&store, NEW_LEASE).principal.value(),
        NEW_FIRST_INVOICE
    );
    assert_eq!(
        account_cash(&store, PROVIDER),
        600_000 - 5 * WEEKLY_WAGE + MOVING_FEE
    );
    assert_eq!(total_cash(&store), opening_cash_total);
    execute_request(&mut store, Caller::Account(TENANT), move_request);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .housing_move_state()
            .unwrap()
            .history(),
        history.as_slice()
    );

    // Switching away from payroll city 2 suspends new work accrual. At day 42, only the
    // 21,000 cents already earned by day 38 is paid; the old due Claim is no longer scanned
    // by daily P2 after move, so its retained rent source authorizes explicit SettleClaim.
    drive_to(&mut store, SETTLE_AT);
    let day42_payroll = *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .payroll
        .iter()
        .find(|payroll| payroll.spec.id == 700)
        .unwrap();
    assert_eq!(day42_payroll.last_accrual, SETTLE_AT);
    assert_eq!(day42_payroll.accrued.value(), day38_payroll.accrued.value());
    assert_eq!(day42_payroll.work_seconds, day38_payroll.work_seconds);
    assert_eq!(day42_payroll.billed.value(), 266_000);
    assert_eq!(day42_payroll.paid.value(), 266_000);
    assert_eq!(
        day42_payroll.paid.value() - day38_payroll.paid.value(),
        21_000
    );
    assert_eq!(account_cash(&store, TENANT), 77_774);
    assert_eq!(
        rent_claim(&store, OLD_LEASE).paid.value(),
        FIRST_OLD_INVOICE
    );
    assert_eq!(rent_claim(&store, NEW_LEASE).paid.value(), 0);
    let settle_request = envelope(
        &store,
        timeline(T::SettleClaim {
            id: u64::from(OLD_LEASE),
        }),
    );
    let error = store
        .execute_with_observer(Caller::System, settle_request.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other("simulated lost settlement reply"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert!(matches!(
        error,
        StorageError::RecoveryRequired | StorageError::Io(_)
    ));
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    // The observer error is injected after the WAL Applied boundary. The durable receipt may
    // still contain a model failure, so replay the exact envelope and require its business
    // result before interpreting any unchanged Claim as a lost successful reply.
    let settled_replay = store
        .execute(Caller::System, settle_request.clone())
        .unwrap();
    assert!(
        settled_replay.model.is_ok(),
        "durable settlement receipt is not successful: request={settle_request:?}; replay={settled_replay:?}"
    );
    assert!(settled_replay.has_persistent_receipt);
    let settled_once = rent_claim(&store, OLD_LEASE);
    assert_eq!(settled_once.id, u64::from(OLD_LEASE));
    assert_eq!(
        settled_once.principal.value(),
        FIRST_OLD_INVOICE + OLD_FINAL_ACCRUAL
    );
    assert_eq!(settled_once.paid.value(), FIRST_OLD_INVOICE + 77_774);
    assert_eq!(
        settled_once.principal.value() - settled_once.paid.value(),
        42_226
    );
    let tenant_cash_after_settlement = account_cash(&store, TENANT);
    assert_eq!(tenant_cash_after_settlement, 0);
    let landlord_cash_after_settlement = account_cash(&store, LANDLORD);
    execute_request(&mut store, Caller::System, settle_request);
    assert_eq!(rent_claim(&store, OLD_LEASE), settled_once);
    assert_eq!(account_cash(&store, TENANT), tenant_cash_after_settlement);
    assert_eq!(
        account_cash(&store, LANDLORD),
        landlord_cash_after_settlement
    );
    assert_eq!(total_cash(&store), opening_cash_total);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&rent_claim(&store, OLD_LEASE)),
        Some(old_source)
    );
}

#[test]
fn historical_rent_remains_settleable_after_unit_is_relet_and_cold_opened() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create_with_relet(&path);
    let opening_cash_total = total_cash(&store);
    assert_eq!(opening_cash_total, 2_915_000);
    assert_eq!(store.engine().unwrap().world().view().accounts.len(), 5);
    let world_owned = assert_relet_world_budget(&store, "opening");
    eprintln!(
        "HOUSING_RELET_FIXTURE_INPUT accounts=5 households=2 units=3 offers=3 opening_world_owned={world_owned}; bounded prototype needs 3 consumed-offer slots for old lease, destination lease, and relet"
    );
    assert_eq!(account_cash(&store, RELET_TENANT), RELET_OPENING_CASH);

    add_payroll(&mut store);
    drive_to(&mut store, ACCEPT_AT);
    accept_old_lease(&mut store);
    execute(&mut store, Caller::Account(TENANT), move_action(0, CITY));
    let wait = store
        .engine()
        .unwrap()
        .housing_move_state()
        .unwrap()
        .wait_for(HOUSEHOLD)
        .unwrap();
    assert_eq!(wait.reason, MoveWait::EvidenceIncomplete);
    assert_relet_world_budget(&store, "after_initial_wait");
    drive_to(&mut store, MOVE_AT);
    execute(
        &mut store,
        Caller::System,
        Action::RegionActivate {
            city: ACTIVE_CITY_AFTER_SWITCH,
        },
    );
    let move_request = envelope(&store, move_action(wait.operation_id, CITY));
    execute_request(&mut store, Caller::Account(TENANT), move_request);
    assert_relet_world_budget(&store, "after_move");

    let old_source = RentClaimSource {
        landlord: LANDLORD,
        tenant: TENANT,
        lease_id: OLD_LEASE,
        unit_id: OLD_UNIT,
    };
    let old_claim_before_relet = rent_claim(&store, OLD_LEASE);
    assert_eq!(
        old_claim_before_relet.principal.value(),
        FIRST_OLD_INVOICE + OLD_FINAL_ACCRUAL
    );
    assert_eq!(old_claim_before_relet.paid.value(), FIRST_OLD_INVOICE);
    assert!(old_claim_before_relet.processed);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&old_claim_before_relet),
        Some(old_source)
    );

    let relet_offer = opening_housing_with_relet()
        .lease_offers
        .iter()
        .find(|offer| offer.offer_id == RELET_OFFER)
        .copied()
        .unwrap();
    execute(
        &mut store,
        Caller::Account(RELET_TENANT),
        Action::HousingLease {
            offer_id: RELET_OFFER,
            offer_sha256: genesis_lease_offer_sha256(relet_offer),
        },
    );
    assert_relet_world_budget(&store, "after_relet");
    let relet_source = RentClaimSource {
        landlord: LANDLORD,
        tenant: RELET_TENANT,
        lease_id: RELET_LEASE,
        unit_id: OLD_UNIT,
    };
    let relet_claim_before_settlement = rent_claim(&store, RELET_LEASE);
    assert_eq!(relet_claim_before_settlement.payer, RELET_TENANT);
    assert_eq!(relet_claim_before_settlement.payee, LANDLORD);
    assert_eq!(relet_claim_before_settlement.due, 59 * DAY);
    // The first partial-month invoice is 20,000 * 21 / 28; its future due date means no
    // arrears or cash movement has occurred by day 42.
    assert_eq!(relet_claim_before_settlement.principal.value(), 15_000);
    assert_eq!(relet_claim_before_settlement.paid.value(), 0);
    assert!(!relet_claim_before_settlement.processed);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&relet_claim_before_settlement),
        Some(relet_source)
    );
    let housing_after_relet = store.engine().unwrap().world().view().housing.unwrap();
    assert_eq!(housing_after_relet.unit_limit, 3);
    assert_eq!(housing_after_relet.units.len(), 3);
    assert_eq!(housing_after_relet.household_registration_count, 2);
    assert_eq!(
        housing_after_relet
            .units
            .iter()
            .find(|unit| unit.unit_id == OLD_UNIT)
            .map(|unit| (unit.household_id, unit.lease_id)),
        Some((RELET_HOUSEHOLD, RELET_LEASE))
    );
    assert_eq!(
        housing_after_relet
            .units
            .iter()
            .find(|unit| unit.unit_id == OLD_UNIT)
            .map(|unit| unit.arrears_ref),
        Some(0)
    );
    assert_eq!(
        housing_after_relet
            .units
            .iter()
            .find(|unit| unit.unit_id == NEW_UNIT + 1)
            .map(|unit| (unit.household_id, unit.lease_id)),
        Some((0, 0))
    );
    assert_eq!(account_cash(&store, RELET_TENANT), RELET_OPENING_CASH);
    let outstanding_before_settlement =
        old_claim_before_relet.principal.value() - old_claim_before_relet.paid.value();
    assert_eq!(outstanding_before_settlement, OLD_FINAL_ACCRUAL);
    let total_before_settlement = total_cash(&store);
    assert_eq!(total_before_settlement, opening_cash_total);

    drive_to(&mut store, SETTLE_AT);
    let payroll_day42 = store.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!(payroll_day42.accrued.value(), 266_000);
    assert_eq!(payroll_day42.paid.value(), 266_000);
    assert_eq!(payroll_day42.work_seconds, 38 * 8 * 3_600);
    assert_eq!(account_cash(&store, TENANT), 77_774);
    assert_eq!(account_cash(&store, RELET_TENANT), RELET_OPENING_CASH);
    assert_eq!(
        rent_claim(&store, RELET_LEASE),
        relet_claim_before_settlement
    );
    assert_eq!(rent_claim(&store, OLD_LEASE), old_claim_before_relet);

    let settle_request = envelope(
        &store,
        timeline(T::SettleClaim {
            id: u64::from(OLD_LEASE),
        }),
    );
    let error = store
        .execute_with_observer(Caller::System, settle_request.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other(
                    "simulated lost historical settlement reply",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert!(matches!(
        error,
        StorageError::RecoveryRequired | StorageError::Io(_)
    ));
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let settled_replay = store
        .execute(Caller::System, settle_request.clone())
        .unwrap();
    assert!(
        settled_replay.model.is_ok(),
        "historical settlement receipt failed: request={settle_request:?}; receipt={settled_replay:?}"
    );
    assert!(settled_replay.has_persistent_receipt);

    let settled_old_claim = rent_claim(&store, OLD_LEASE);
    assert_eq!(
        settled_old_claim.principal,
        old_claim_before_relet.principal
    );
    assert_eq!(settled_old_claim.paid.value(), 341_000);
    assert_eq!(
        settled_old_claim.principal.value() - settled_old_claim.paid.value(),
        42_226
    );
    assert_eq!(
        old_claim_before_relet.principal.value()
            - old_claim_before_relet.paid.value()
            - (settled_old_claim.principal.value() - settled_old_claim.paid.value()),
        77_774
    );
    assert_eq!(
        rent_claim(&store, RELET_LEASE),
        relet_claim_before_settlement
    );
    assert_eq!(account_cash(&store, TENANT), 0);
    assert_eq!(account_cash(&store, RELET_TENANT), RELET_OPENING_CASH);
    assert_eq!(total_cash(&store), opening_cash_total);
    assert_eq!(account_cash(&store, LANDLORD), 1_200_000 + 341_000);
    assert_eq!(
        account_cash(&store, PROVIDER),
        600_000 - 5 * WEEKLY_WAGE + MOVING_FEE - 21_000
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&settled_old_claim),
        Some(old_source)
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&rent_claim(&store, RELET_LEASE)),
        Some(relet_source)
    );
    assert_relet_world_budget(&store, "after_old_settlement");
    let housing_after_settlement = store.engine().unwrap().world().view().housing.unwrap();
    assert_eq!(
        housing_after_settlement
            .units
            .iter()
            .find(|unit| unit.unit_id == OLD_UNIT)
            .map(|unit| (unit.household_id, unit.lease_id)),
        Some((RELET_HOUSEHOLD, RELET_LEASE))
    );
    assert_eq!(
        housing_after_settlement
            .units
            .iter()
            .find(|unit| unit.unit_id == OLD_UNIT)
            .map(|unit| unit.arrears_ref),
        Some(0)
    );
    assert_eq!(
        housing_after_settlement
            .units
            .iter()
            .find(|unit| unit.unit_id == NEW_UNIT)
            .map(|unit| (unit.household_id, unit.lease_id)),
        Some((HOUSEHOLD, NEW_LEASE))
    );
    let history = store
        .engine()
        .unwrap()
        .housing_move_state()
        .unwrap()
        .history()
        .to_vec();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].claim_id, u64::from(OLD_LEASE));
    assert_eq!(history[0].source, old_source);

    execute_request(&mut store, Caller::System, settle_request.clone());
    assert_eq!(rent_claim(&store, OLD_LEASE), settled_old_claim);
    assert_eq!(
        rent_claim(&store, RELET_LEASE),
        relet_claim_before_settlement
    );
    assert_eq!(total_cash(&store), opening_cash_total);
    store.checkpoint().unwrap();
    drop(store);
    let store = Store::open(&path, RULES).unwrap();
    assert_eq!(rent_claim(&store, OLD_LEASE), settled_old_claim);
    assert_eq!(
        rent_claim(&store, RELET_LEASE),
        relet_claim_before_settlement
    );
    assert_eq!(account_cash(&store, RELET_TENANT), RELET_OPENING_CASH);
    assert_eq!(total_cash(&store), opening_cash_total);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&rent_claim(&store, OLD_LEASE)),
        Some(old_source)
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .rent_source_for_claim(&rent_claim(&store, RELET_LEASE)),
        Some(relet_source)
    );
    assert_eq!(
        store.engine().unwrap().world().view().housing.unwrap(),
        housing_after_settlement
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .housing_move_state()
            .unwrap()
            .history(),
        history.as_slice()
    );
}

#[test]
fn over_budget_wait_is_rejected_without_model_changes_and_replays_after_cold_open() {
    const RESERVED_UNITS: u8 = 18;

    // Measure the exact persistent delta for a real Wait on the unchanged, legal two-unit
    // fixture. The 18-unit case below uses this measured row/runtime cost to prove the new
    // Wait would cross the same World gate.
    let calibration_temp = Temp::new();
    let calibration_path = calibration_temp.world();
    let mut calibration = create(&calibration_path, Some(CITY));
    add_payroll(&mut calibration);
    drive_to(&mut calibration, ACCEPT_AT);
    accept_old_lease(&mut calibration);
    let calibration_world = calibration.engine().unwrap().world();
    let calibration_layout_before = calibration_world.layout();
    let calibration_owned_before = calibration_layout_before["owned_capacity_bytes"]
        .as_u64()
        .unwrap() as usize;
    let validation_scratch = mycity_core::housing::MAX_VALIDATION_SCRATCH_BYTES;
    let world_budget = mycity_core::assembly::WORLD_BYTE_BUDGET;
    assert!(calibration_owned_before + validation_scratch <= world_budget);
    let calibration_request = envelope(&calibration, move_action(0, CITY));
    let calibration_outcome = calibration
        .execute(Caller::Account(TENANT), calibration_request)
        .unwrap();
    assert!(calibration_outcome.model.is_ok(), "{calibration_outcome:?}");
    assert!(calibration_outcome.has_persistent_receipt);
    let calibration_wait = calibration
        .engine()
        .unwrap()
        .housing_move_state()
        .unwrap()
        .wait_for(HOUSEHOLD)
        .unwrap();
    assert_eq!(calibration_wait.reason, MoveWait::EvidenceIncomplete);
    let calibration_layout_after = calibration.engine().unwrap().world().layout();
    let calibration_owned_after = calibration_layout_after["owned_capacity_bytes"]
        .as_u64()
        .unwrap() as usize;
    let first_wait_runtime_bytes = calibration_owned_after - calibration_owned_before;
    assert_eq!(first_wait_runtime_bytes, 96);
    assert_eq!(
        calibration_layout_after["housing_move_runtime_bytes"]
            .as_u64()
            .unwrap() as usize,
        first_wait_runtime_bytes
    );
    assert!(calibration_owned_after + validation_scratch <= world_budget);
    eprintln!(
        "HOUSING_WAIT_BUDGET_CALIBRATION owned_before={calibration_owned_before} owned_after={calibration_owned_after} wait_delta={first_wait_runtime_bytes} scratch={validation_scratch} budget={world_budget}"
    );
    drop(calibration);

    let temp = Temp::new();
    let path = temp.world();
    let mut store = create_with_housing(
        &path,
        opening_housing_with_limit(Some(CITY), RESERVED_UNITS),
    );
    add_payroll(&mut store);
    drive_to(&mut store, ACCEPT_AT);
    accept_old_lease(&mut store);

    let world = store.engine().unwrap().world();
    let layout = world.layout();
    let before_world_owned = layout["owned_capacity_bytes"].as_u64().unwrap() as usize;
    let before_engine_owned = store.engine().unwrap().owned_capacity_bytes();
    let engine_budget = mycity_core::transaction::ENGINE_BYTE_BUDGET;
    assert_eq!(world.view().housing.unwrap().unit_limit, RESERVED_UNITS);
    assert!(
        before_world_owned + validation_scratch <= world_budget,
        "fixture must be legal before adding the Wait row: owned={before_world_owned}, scratch={validation_scratch}, budget={world_budget}, layout={layout}"
    );
    assert!(
        before_world_owned + validation_scratch + first_wait_runtime_bytes > world_budget,
        "one measured persisted Wait must cross the World gate: owned={before_world_owned}, scratch={validation_scratch}, wait={first_wait_runtime_bytes}, budget={world_budget}, layout={layout}"
    );
    assert!(
        before_engine_owned + first_wait_runtime_bytes < engine_budget,
        "the narrow failure must be the World gate, not the broader Engine budget: owned={before_engine_owned}, wait={first_wait_runtime_bytes}, budget={engine_budget}"
    );
    eprintln!(
        "HOUSING_WAIT_BUDGET_BOUNDARY world_owned_before={before_world_owned} wait_delta={first_wait_runtime_bytes} scratch={validation_scratch} projected_world_owned_plus_scratch={} world_budget={world_budget} engine_owned_before={before_engine_owned} engine_budget={engine_budget} unit_limit={RESERVED_UNITS}",
        before_world_owned + first_wait_runtime_bytes + validation_scratch,
    );

    let before_engine_hash = store.engine().unwrap().state_hash();
    let before_world_hash = world.state_hash();
    let before_housing = serde_json::to_value(world.view().housing).unwrap();
    let before_timeline =
        serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap();
    let before_cash = total_cash(&store);
    let before_tenant_cash = account_cash(&store, TENANT);
    let before_provider_cash = account_cash(&store, PROVIDER);
    let before_sequence = store.committed_sequence();
    let request = envelope(&store, move_action(0, CITY));

    let failed = store
        .execute(Caller::Account(TENANT), request.clone())
        .unwrap();
    assert_eq!(failed.model, Err(TxError::WorkingLimit));
    assert!(failed.has_persistent_receipt);
    assert_eq!(failed.committed_sequence, before_sequence + 1);

    let assert_model_unchanged = |store: &Store| {
        assert_eq!(store.engine().unwrap().state_hash(), before_engine_hash);
        assert_eq!(
            store.engine().unwrap().world().state_hash(),
            before_world_hash
        );
        assert_eq!(
            serde_json::to_value(store.engine().unwrap().world().view().housing).unwrap(),
            before_housing
        );
        assert_eq!(
            serde_json::to_value(store.engine().unwrap().timeline().unwrap()).unwrap(),
            before_timeline
        );
        assert_eq!(total_cash(store), before_cash);
        assert_eq!(account_cash(store, TENANT), before_tenant_cash);
        assert_eq!(account_cash(store, PROVIDER), before_provider_cash);
        assert!(store
            .engine()
            .unwrap()
            .housing_move_state()
            .is_none_or(|state| state.waits().is_empty() && state.history().is_empty()));
    };
    assert_model_unchanged(&store);

    let replay = store
        .execute(Caller::Account(TENANT), request.clone())
        .unwrap();
    assert_eq!(replay.model, failed.model);
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.committed_sequence, failed.committed_sequence);
    assert_model_unchanged(&store);

    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_model_unchanged(&store);
    let cold_replay = store.execute(Caller::Account(TENANT), request).unwrap();
    assert_eq!(cold_replay.model, failed.model);
    assert!(cold_replay.has_persistent_receipt);
    assert_eq!(cold_replay.committed_sequence, failed.committed_sequence);
    assert_model_unchanged(&store);
}

#[test]
fn legacy_cityless_housing_can_reopen_but_cannot_authorize_a_move() {
    let temp = Temp::new();
    let path = temp.world();
    let mut store = create(&path, None);
    add_payroll(&mut store);
    drive_to(&mut store, ACCEPT_AT);
    accept_old_lease(&mut store);
    let before = store.engine().unwrap().world().view().housing.unwrap();
    let request = envelope(&store, move_action(0, CITY));
    let outcome = store.execute(Caller::Account(TENANT), request).unwrap();
    assert!(outcome.model.is_err());
    store.checkpoint().unwrap();
    drop(store);
    let store = Store::open(&path, RULES).unwrap();
    assert_eq!(
        store.engine().unwrap().world().view().housing.unwrap(),
        before
    );
    assert!(store
        .engine()
        .unwrap()
        .housing_move_state()
        .is_none_or(|state| state.history().is_empty() && state.waits().is_empty()));
}
