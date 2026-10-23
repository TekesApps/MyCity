//! Store-level checks for actual paid-wage windows used by housing decisions.
//!
//! This is source-only until the frozen wage-window core is integrated into this private tree
//! and root runs the serialized storage validation. The independent numeric expectations are
//! asserted against Engine's persisted read model, not against a second income ledger.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, LeaseTerms, OpeningDwelling, OpeningHousehold, OpeningHousing,
        PaidWageWindowStatus, MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
    },
    model_rows::Rows,
    timeline::{Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_storage::{Error as StorageError, Store, WritePoint};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const RULES: &str = "m0-storage-process-fixture-v1";
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
const TENANT: u32 = 2;
const EMPLOYER: u32 = 3;
const SUPPLIER: u32 = 4;
const HOUSEHOLD: u32 = 501;
const UNIT: u32 = 101;
const BUILDING: u32 = 10;
const OFFER: u64 = 9_001;
const LEASE: u32 = 9_001;
const PAYROLL: u32 = 700;
const MONTHLY_RENT_CENTS: i64 = 480_000;
const DAY_31: u64 = 31 * DAY;
const DAY_32: u64 = 32 * DAY;
const DAY_62: u64 = 62 * DAY;
const MONTHLY_WAGE_AT_DAY_31: i64 = 198_400; // 31 days * 8 hours/day * 800 cents/hour

struct Temp {
    path: PathBuf,
    preserve: bool,
}

impl Temp {
    fn new() -> Self {
        let artifact_root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT");
        let preserve = artifact_root.is_some();
        let root = artifact_root
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-housing-wage-window-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("HOUSING_WAGE_WINDOW_ARTIFACT_CREATED={}", path.display());
        Self { path, preserve }
    }

    fn world(&self) -> PathBuf {
        self.path.join("world")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if self.preserve || std::thread::panicking() {
            eprintln!(
                "HOUSING_WAGE_WINDOW_ARTIFACT_PRESERVED={}",
                self.path.display()
            );
            return;
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn money(value: i64) -> Money {
    Money::new(value).unwrap()
}

fn opening_housing() -> OpeningHousing {
    let fact = DwellingFact {
        unit_id: UNIT,
        building_id: BUILDING,
        owner_account: SUPPLIER,
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
        contract_source_id: 9_101,
        landlord_authorization_id: 9_102,
        tenant_consent_id: 9_103,
        rent_per_cycle: money(MONTHLY_RENT_CENTS),
        cycle: Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: 1,
        starts_at: GameTime::ZERO,
    };
    let offer = GenesisLeaseOffer {
        offer_id: OFFER,
        world_id: 1,
        branch_id: 1,
        unit_id: UNIT,
        building_id: BUILDING,
        owner_account: SUPPLIER,
        tenant_account: TENANT,
        household_id: HOUSEHOLD,
        valid_from: GameTime::ZERO,
        valid_until: Date::new(2001, 4, 1).unwrap().midnight(),
        asset_source_id: 7_001,
        authority_source_id: 7_002,
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
            asset_source_id: 7_001,
        })
        .unwrap();
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
    lease_offers.try_push(offer).unwrap();

    OpeningHousing {
        unit_limit: 1,
        city_id: None,
        dwellings,
        households,
        lease_offers,
        move_assessments: mycity_core::model_rows::Rows::new().unwrap(),
        move_candidates: mycity_core::model_rows::Rows::new().unwrap(),
    }
}

fn create(path: &Path) -> Store {
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    opening["housing"] = serde_json::to_value(opening_housing()).unwrap();
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
    let result = store.execute(caller, request).unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    assert!(result.has_persistent_receipt);
}

fn timeline(command: T) -> Action {
    Action::Timeline { command }
}

fn payroll(cycle: Cycle, hourly_cents: i64, ends_at: u64) -> Action {
    timeline(T::AddPayroll {
        spec: PayrollSpec {
            id: PAYROLL,
            payer: EMPLOYER,
            payee: TENANT,
            home_city: 0,
            hourly_cents: money(hourly_cents),
            cycle,
            ends_at,
        },
    })
}

fn drive_to(store: &mut Store, target: u64) {
    for _ in 0..10_000 {
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        if progress.target.is_none() && progress.at == target {
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

fn drive_daily(store: &mut Store, through_day: u64) {
    let completed_day = store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .progress
        .last_complete
        / DAY;
    for day in completed_day + 1..=through_day {
        drive_to(store, day * DAY);
    }
}

fn accept_opening_lease(store: &mut Store) {
    let offer = opening_housing().lease_offers[0];
    execute(
        store,
        Caller::Account(TENANT),
        Action::HousingLease {
            offer_id: OFFER,
            offer_sha256: genesis_lease_offer_sha256(offer),
        },
    );
}

fn wage_window(store: &Store) -> mycity_core::housing::PaidWageWindowView {
    store
        .engine()
        .unwrap()
        .housing_paid_wage_window(HOUSEHOLD)
        .expect("registered household should expose a wage-window view")
}

fn payroll_state(store: &Store) -> mycity_core::timeline::Payroll {
    *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .payroll
        .iter()
        .find(|row| row.spec.id == PAYROLL)
        .unwrap()
}

fn rent_claim(store: &Store) -> mycity_core::timeline::Claim {
    *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claims
        .iter()
        .find(|row| row.id == u64::from(LEASE))
        .unwrap()
}

fn cash(store: &Store, account_id: u32) -> i64 {
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

fn transfer(store: &mut Store, from: u32, to: u32, amount: i64) {
    let from_ref = store.engine().unwrap().world().account_ref(from).unwrap();
    let to_ref = store.engine().unwrap().world().account_ref(to).unwrap();
    execute(
        store,
        Caller::Account(from),
        Action::Transfer {
            from: from_ref,
            to: to_ref,
            amount: money(amount),
        },
    );
}

fn setup_monthly(store: &mut Store) {
    execute(store, Caller::System, timeline(T::Enable));
    execute(
        store,
        Caller::System,
        payroll(Cycle::Monthly { anchor: 1 }, 800, 150 * DAY),
    );
    accept_opening_lease(store);
}

#[test]
fn monthly_boundary_and_applied_cold_replay_match_daily_advances() {
    let batch_temp = Temp::new();
    let daily_temp = Temp::new();
    let mut batched = create(&batch_temp.world());
    let mut daily = create(&daily_temp.world());
    setup_monthly(&mut batched);
    setup_monthly(&mut daily);

    drive_to(&mut batched, 30 * DAY);
    drive_daily(&mut daily, 30);
    for store in [&batched, &daily] {
        let view = wage_window(store);
        assert_eq!(view.status, PaidWageWindowStatus::CoverageIncomplete);
        assert_eq!(view.start_day, None);
        assert_eq!(view.end_day, None);
        assert_eq!(view.actual_paid_wages, Money::ZERO);
    }

    // The monthly payroll and rent reach their existing Gregorian boundary together at
    // day 31. Inject an Applied lost reply on the batched path, then recover by replaying
    // the exact envelope; its event-time wage delta must not be recorded twice.
    let due = envelope(
        &batched,
        timeline(T::Advance {
            target: DAY_31,
            limit: 256,
        }),
    );
    let operation_id = due.operation_id;
    assert!(batched
        .execute_with_observer(Caller::System, due.clone(), |point| {
            if point == WritePoint::Applied {
                Err(std::io::Error::other("lost monthly payroll boundary reply"))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(matches!(
        batched.engine(),
        Err(StorageError::RecoveryRequired)
    ));
    drop(batched);
    let mut batched = Store::open(&batch_temp.world(), RULES).unwrap();
    assert_eq!(
        batched.receipt(operation_id).unwrap().unwrap().operation_id,
        operation_id
    );
    let recovered_hash = batched.state_hash().unwrap();
    let replay = batched.execute(Caller::System, due).unwrap();
    assert_eq!(replay.model.unwrap().operation_id, operation_id);
    assert_eq!(batched.state_hash().unwrap(), recovered_hash);
    drive_to(&mut batched, DAY_31);

    drive_to(&mut daily, DAY_31);
    for store in [&batched, &daily] {
        let view = wage_window(store);
        assert_eq!(view.status, PaidWageWindowStatus::Complete);
        assert_eq!((view.start_day, view.end_day), (Some(0), Some(31)));
        assert_eq!(view.qualifying_contracts, 1);
        assert_eq!(view.active_contracts, 1);
        assert_eq!(
            view.actual_paid_wages,
            Money::ZERO,
            "salary paid exactly at 00:00 day 31 belongs to the new day and is excluded"
        );
        assert_eq!(payroll_state(store).paid, money(MONTHLY_WAGE_AT_DAY_31));
    }

    // One more complete day moves the half-open window to [day 1, day 32), including
    // the day-31 00:00 payment exactly once. Compare full business outputs across a single
    // multi-day catch-up and 32 sequential day targets.
    drive_to(&mut batched, DAY_32);
    drive_daily(&mut daily, 32);
    assert_eq!(wage_window(&batched), wage_window(&daily));
    assert_eq!(wage_window(&batched).status, PaidWageWindowStatus::Complete);
    assert_eq!(
        (
            wage_window(&batched).start_day,
            wage_window(&batched).end_day
        ),
        (Some(1), Some(32))
    );
    assert_eq!(
        wage_window(&batched).actual_paid_wages,
        money(MONTHLY_WAGE_AT_DAY_31)
    );
    assert_eq!(payroll_state(&batched), payroll_state(&daily));
    assert_eq!(rent_claim(&batched), rent_claim(&daily));
    for account in [1, TENANT, EMPLOYER, SUPPLIER] {
        assert_eq!(cash(&batched, account), cash(&daily, account));
    }
}

#[test]
fn pending_payroll_p2_checkpoint_cold_open_records_wage_once() {
    let temp = Temp::new();
    let mut store = create(&temp.world());
    setup_monthly(&mut store);

    // Complete the preceding hour as its own target, then hold limit=1 for every operation
    // in the due hour. This preserves Store's fixed continuation page-size contract.
    let last_hour = DAY_31 - 3600;
    drive_to(&mut store, last_hour);
    let staged = store.engine().unwrap().timeline().unwrap().progress;
    assert!(staged.target.is_none());
    assert_eq!(staged.last_complete, last_hour);

    let mut pending = None;
    for _ in 0..10_000 {
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        if progress.target == Some(DAY_31)
            && progress.phase == Some(mycity_core::schedule::Phase::P2)
            && progress.lane == 1
            && progress.cursor >= 1
        {
            pending = Some(progress);
            break;
        }
        execute(
            &mut store,
            Caller::System,
            timeline(T::Advance {
                target: DAY_31,
                limit: 1,
            }),
        );
    }
    let pending = pending.expect("must stop after the due payroll page, before Timeline completes");
    assert_eq!(payroll_state(&store).paid, money(MONTHLY_WAGE_AT_DAY_31));

    // Window queries reject an in-flight Timeline boundary rather than exposing a partial
    // wage sum as a complete 31-day observation.
    let incomplete = wage_window(&store);
    assert_eq!(incomplete.status, PaidWageWindowStatus::CoverageIncomplete);
    assert_eq!((incomplete.start_day, incomplete.end_day), (None, None));
    assert_eq!(incomplete.actual_paid_wages, Money::ZERO);

    let committed_hash = store.state_hash().unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&temp.world(), RULES).unwrap();
    assert_eq!(
        store.engine().unwrap().timeline().unwrap().progress,
        pending
    );
    assert_eq!(store.state_hash().unwrap(), committed_hash);
    assert_eq!(payroll_state(&store).paid, money(MONTHLY_WAGE_AT_DAY_31));
    let cold_incomplete = wage_window(&store);
    assert_eq!(
        cold_incomplete.status,
        PaidWageWindowStatus::CoverageIncomplete
    );
    assert_eq!(
        (cold_incomplete.start_day, cold_incomplete.end_day),
        (None, None)
    );
    assert_eq!(cold_incomplete.actual_paid_wages, Money::ZERO);

    // Resume that exact target only with the same limit=1. The payment was already applied
    // before the checkpoint, so completion must preserve one actual delta in the day-31 slot.
    for _ in 0..10_000 {
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        if progress.target.is_none() && progress.last_complete == DAY_31 {
            break;
        }
        assert_eq!(progress.target, Some(DAY_31));
        execute(
            &mut store,
            Caller::System,
            timeline(T::Advance {
                target: DAY_31,
                limit: 1,
            }),
        );
    }
    let complete = wage_window(&store);
    assert_eq!(complete.status, PaidWageWindowStatus::Complete);
    assert_eq!((complete.start_day, complete.end_day), (Some(0), Some(31)));
    assert_eq!(complete.actual_paid_wages, Money::ZERO);
    assert_eq!(payroll_state(&store).paid, money(MONTHLY_WAGE_AT_DAY_31));

    drive_to(&mut store, DAY_32);
    let next_window = wage_window(&store);
    assert_eq!(next_window.status, PaidWageWindowStatus::Complete);
    assert_eq!(
        (next_window.start_day, next_window.end_day),
        (Some(1), Some(32))
    );
    assert_eq!(next_window.actual_paid_wages, money(MONTHLY_WAGE_AT_DAY_31));
    assert_eq!(payroll_state(&store).paid, money(MONTHLY_WAGE_AT_DAY_31));
}

#[test]
fn terminated_at_opening_is_unqualified_but_stopped_contract_is_zero_income() {
    let no_payroll_temp = Temp::new();
    let mut no_payroll = create(&no_payroll_temp.world());
    execute(&mut no_payroll, Caller::System, timeline(T::Enable));
    drive_to(&mut no_payroll, DAY_31);
    let no_contract = wage_window(&no_payroll);
    assert_eq!(no_contract.status, PaidWageWindowStatus::NoQualifiedPayroll);
    assert_eq!(
        (no_contract.start_day, no_contract.end_day),
        (Some(0), Some(31))
    );
    assert_eq!(no_contract.actual_paid_wages, Money::ZERO);
    assert_eq!(no_contract.qualifying_contracts, 0);

    let zero_temp = Temp::new();
    let mut zero_income = create(&zero_temp.world());
    execute(&mut zero_income, Caller::System, timeline(T::Enable));
    execute(
        &mut zero_income,
        Caller::System,
        payroll(Cycle::Weekly, 800, 150 * DAY),
    );
    // End the qualified contract at its opening boundary before any work or cash payment.
    execute(
        &mut zero_income,
        Caller::System,
        timeline(T::Terminate { id: PAYROLL }),
    );
    drive_to(&mut zero_income, DAY_31);
    let terminated_at_opening = wage_window(&zero_income);
    assert_eq!(
        terminated_at_opening.status,
        PaidWageWindowStatus::NoQualifiedPayroll
    );
    assert_eq!(
        (
            terminated_at_opening.start_day,
            terminated_at_opening.end_day
        ),
        (Some(0), Some(31))
    );
    assert_eq!(terminated_at_opening.actual_paid_wages, Money::ZERO);
    assert_eq!(terminated_at_opening.qualifying_contracts, 0);
    assert_eq!(terminated_at_opening.active_contracts, 0);

    // Stop work without terminating the contract. Its term still overlaps [day 0, day 31),
    // but it earns and receives no wages, so this is a true Complete+zero observation.
    let stopped_temp = Temp::new();
    let mut stopped = create(&stopped_temp.world());
    execute(&mut stopped, Caller::System, timeline(T::Enable));
    execute(
        &mut stopped,
        Caller::System,
        payroll(Cycle::Weekly, 800, 150 * DAY),
    );
    execute(
        &mut stopped,
        Caller::System,
        timeline(T::StopWork { id: PAYROLL }),
    );
    drive_to(&mut stopped, DAY_31);
    let observed_zero = wage_window(&stopped);
    assert_eq!(observed_zero.status, PaidWageWindowStatus::Complete);
    assert_eq!(
        (observed_zero.start_day, observed_zero.end_day),
        (Some(0), Some(31))
    );
    assert_eq!(observed_zero.actual_paid_wages, Money::ZERO);
    assert_eq!(observed_zero.qualifying_contracts, 1);
    assert_eq!(observed_zero.active_contracts, 0);
    assert_eq!(payroll_state(&stopped).paid, Money::ZERO);
}

#[test]
fn explicit_settle_payroll_payment_survives_applied_cold_replay_once() {
    let temp = Temp::new();
    let mut store = create(&temp.world());
    execute(&mut store, Caller::System, timeline(T::Enable));
    execute(
        &mut store,
        Caller::System,
        payroll(Cycle::Weekly, 800, 150 * DAY),
    );

    // Deplete the employer through a real transfer so the weekly due boundary records
    // billed-but-unpaid payroll. Replenish exactly the wage from the supplier, then exercise
    // the explicit settlement path that is wired into the same prepared wage-window update.
    let employer_cash = cash(&store, EMPLOYER);
    transfer(&mut store, EMPLOYER, SUPPLIER, employer_cash);
    drive_to(&mut store, DAY_31);
    let unpaid = payroll_state(&store);
    assert_eq!(unpaid.accrued, money(MONTHLY_WAGE_AT_DAY_31));
    assert!(unpaid.billed > Money::ZERO);
    assert_eq!(payroll_state(&store).paid, Money::ZERO);
    let arrears = wage_window(&store);
    assert_eq!(arrears.status, PaidWageWindowStatus::BilledArrears);
    assert_eq!((arrears.start_day, arrears.end_day), (Some(0), Some(31)));
    assert_eq!(arrears.actual_paid_wages, Money::ZERO);
    let amount_due = unpaid.billed.value() - unpaid.paid.value();
    transfer(&mut store, SUPPLIER, EMPLOYER, amount_due);

    let settle = envelope(&store, timeline(T::SettlePayroll { id: PAYROLL }));
    let operation_id = settle.operation_id;
    assert!(store
        .execute_with_observer(Caller::System, settle.clone(), |point| {
            if point == WritePoint::Applied {
                Err(std::io::Error::other(
                    "lost explicit payroll settlement reply",
                ))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(matches!(
        store.engine(),
        Err(StorageError::RecoveryRequired)
    ));
    drop(store);
    let mut store = Store::open(&temp.world(), RULES).unwrap();
    let before_replay = store.state_hash().unwrap();
    let receipt = store.execute(Caller::System, settle).unwrap();
    assert_eq!(receipt.model.unwrap().operation_id, operation_id);
    assert_eq!(store.state_hash().unwrap(), before_replay);
    assert_eq!(payroll_state(&store).paid.value(), unpaid.billed.value());
    assert_eq!(cash(&store, TENANT), 200_000 + unpaid.billed.value());
    // A payment at 00:00 day 31 is retained in the new day's bucket, so the just-completed
    // `[day 0, day 31)` window remains zero after explicit settlement.
    assert_eq!(wage_window(&store).status, PaidWageWindowStatus::Complete);
    assert_eq!(wage_window(&store).actual_paid_wages, Money::ZERO);

    // Terminate at the same complete boundary to bill and pay the final partial weekly
    // interval. Both real settlement deltas share day 31; cold recovery and replay above
    // must leave the first delta recorded exactly once.
    let remaining = payroll_state(&store).accrued.value() - payroll_state(&store).paid.value();
    transfer(&mut store, SUPPLIER, EMPLOYER, remaining);
    execute(
        &mut store,
        Caller::System,
        timeline(T::Terminate { id: PAYROLL }),
    );
    drive_to(&mut store, DAY_62);
    let view = wage_window(&store);
    assert_eq!(view.status, PaidWageWindowStatus::Complete);
    assert_eq!((view.start_day, view.end_day), (Some(31), Some(62)));
    assert_eq!(view.actual_paid_wages, money(MONTHLY_WAGE_AT_DAY_31));
    assert_eq!(payroll_state(&store).paid, money(MONTHLY_WAGE_AT_DAY_31));
    assert_eq!(cash(&store, TENANT), 200_000 + MONTHLY_WAGE_AT_DAY_31);
}
