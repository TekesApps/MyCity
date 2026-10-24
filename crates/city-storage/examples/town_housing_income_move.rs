//! Store-backed comparison of two households with different wage histories.
//!
//! The low-wage household accumulates a real rent Claim arrear and cannot afford the
//! candidate unit, so its persisted move request remains a Wait. The higher-wage household
//! pays its first invoice and completes a move after the same 31-day wage window is covered.
//! Move satisfaction/environment/public-service scores are immutable fixture evidence; this
//! scenario exercises the Store/Timeline financial path, not the production of those scores.
//! This is an unrun source candidate until the shared-core owner schedules the Store check.
//!
//! Usage: `cargo run -p mycity-storage --example town_housing_income_move -- <new-world-dir>`.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, LeaseTerms, MoveCostQuote, MoveScore, MoveWait, OpeningDwelling,
        OpeningHousehold, OpeningHousing, OpeningMoveAssessment, OpeningMoveCandidate,
        PaidWageWindowStatus, MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
    },
    model_rows::Rows,
    timeline::{Claim, Command as T, Cycle, PayrollSpec, RentClaimSource, DAY},
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

const RULES: &str = "m0-storage-process-fixture-v1";
const LOW_TENANT: u32 = 2;
const EMPLOYER: u32 = 3;
const LANDLORD: u32 = 4;
const PROVIDER: u32 = 3;
const HIGH_TENANT: u32 = 5;
const LOW_HOUSEHOLD: u32 = 501;
const HIGH_HOUSEHOLD: u32 = 502;
const LOW_UNIT: u32 = 101;
const HIGH_UNIT: u32 = 102;
const LOW_TARGET_UNIT: u32 = 103;
const HIGH_TARGET_UNIT: u32 = 104;
const LOW_OFFER: u64 = 9_001;
const HIGH_OFFER: u64 = 9_002;
const LOW_TARGET_OFFER: u64 = 9_003;
const HIGH_TARGET_OFFER: u64 = 9_004;
const LOW_LEASE: u32 = 9_001;
const HIGH_LEASE: u32 = 9_002;
const LOW_TARGET_LEASE: u32 = 9_003;
const HIGH_TARGET_LEASE: u32 = 9_004;
const LOW_ASSESSMENT: u64 = 501;
const HIGH_ASSESSMENT: u64 = 502;
const LOW_PAYROLL: u32 = 700;
const HIGH_PAYROLL: u32 = 701;
const CITY: u32 = 1;
const ACCEPT_AT: u64 = 14 * DAY;
const MOVE_AT: u64 = 38 * DAY;
const LOW_OLD_RENT: i64 = 480_000;
const HIGH_OLD_RENT: i64 = 120_000;
const LOW_TARGET_RENT: i64 = 150_000;
const HIGH_TARGET_RENT: i64 = 40_000;
const LOW_HOURLY_CENTS: i64 = 100;
const HIGH_HOURLY_CENTS: i64 = 875;
const OPENING_CASH: i64 = 85_000;
const SECOND_TENANT_SOURCE: u32 = 6;
const MOVING_FEE: i64 = 10_000;
const LOW_FIRST_INVOICE: i64 = 263_226; // ceil(480000 * 17 / 31)
const HIGH_FIRST_INVOICE: i64 = 65_807; // ceil(120000 * 17 / 31)
const HIGH_FINAL_ACCRUAL: i64 = 30_000; // 120000 * 7 / 28
const HIGH_TARGET_FIRST_INVOICE: i64 = 30_000; // 40000 * 21 / 28

fn money(value: i64) -> Money {
    Money::new(value).expect("non-negative scenario amount")
}

fn at(seconds: u64) -> GameTime {
    GameTime::new(i64::try_from(seconds).expect("scenario time fits i64")).unwrap()
}

fn score(lower: u8, upper: u8, environment_source: u64, service_source: u64) -> MoveScore {
    MoveScore {
        lower,
        upper,
        fully_covered: true,
        environment_source,
        public_service_source: service_source,
        source_version: 1,
        assessed_at: at(ACCEPT_AT),
        valid_through: at(120 * DAY),
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
    tenant: u32,
    household: u32,
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
        starts_at: at(starts_at),
    };
    GenesisLeaseOffer {
        offer_id,
        world_id: 1,
        branch_id: 1,
        unit_id,
        building_id,
        owner_account: LANDLORD,
        tenant_account: tenant,
        household_id: household,
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
        deposit: terms.deposit,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: lease_terms_sha256(terms).unwrap(),
    }
}

fn opening_housing() -> OpeningHousing {
    let mut dwellings = Rows::<OpeningDwelling, MAX_M0_HOUSING_UNITS>::new().unwrap();
    for (unit, building) in [
        (LOW_UNIT, 10),
        (HIGH_UNIT, 11),
        (LOW_TARGET_UNIT, 12),
        (HIGH_TARGET_UNIT, 13),
    ] {
        dwellings.try_push(dwelling(unit, building)).unwrap();
    }

    let mut households = Rows::<OpeningHousehold, MAX_M0_HOUSING_UNITS>::new().unwrap();
    households
        .try_push(OpeningHousehold {
            household_id: LOW_HOUSEHOLD,
            account: LOW_TENANT,
            people: 2,
            source_id: 8_001,
        })
        .unwrap();
    households
        .try_push(OpeningHousehold {
            household_id: HIGH_HOUSEHOLD,
            account: HIGH_TENANT,
            people: 2,
            source_id: 8_002,
        })
        .unwrap();

    let offers = [
        offer(
            LOW_OFFER,
            LOW_LEASE,
            LOW_UNIT,
            10,
            LOW_TENANT,
            LOW_HOUSEHOLD,
            LOW_OLD_RENT,
            ACCEPT_AT,
        ),
        offer(
            HIGH_OFFER,
            HIGH_LEASE,
            HIGH_UNIT,
            11,
            HIGH_TENANT,
            HIGH_HOUSEHOLD,
            HIGH_OLD_RENT,
            ACCEPT_AT,
        ),
        offer(
            LOW_TARGET_OFFER,
            LOW_TARGET_LEASE,
            LOW_TARGET_UNIT,
            12,
            LOW_TENANT,
            LOW_HOUSEHOLD,
            LOW_TARGET_RENT,
            MOVE_AT,
        ),
        offer(
            HIGH_TARGET_OFFER,
            HIGH_TARGET_LEASE,
            HIGH_TARGET_UNIT,
            13,
            HIGH_TENANT,
            HIGH_HOUSEHOLD,
            HIGH_TARGET_RENT,
            MOVE_AT,
        ),
    ];
    let mut lease_offers = Rows::<GenesisLeaseOffer, MAX_GENESIS_LEASE_OFFERS>::new().unwrap();
    for row in offers {
        lease_offers.try_push(row).unwrap();
    }

    let mut assessments = Rows::new().unwrap();
    for (assessment_id, household_id, source) in [
        (LOW_ASSESSMENT, LOW_HOUSEHOLD, 5_000),
        (HIGH_ASSESSMENT, HIGH_HOUSEHOLD, 5_100),
    ] {
        assessments
            .try_push(OpeningMoveAssessment {
                assessment_id,
                household_id,
                source_id: source,
                version: 1,
                old_score: score(10, 20, source + 1, source + 2),
                low_since: Some(GameTime::ZERO),
                covered_through: at(120 * DAY),
                continuous_low_and_covered: true,
                cooldown_until: None,
                essential_costs_per_month: Money::ZERO,
            })
            .unwrap();
    }

    let mut candidates = Rows::new().unwrap();
    for (assessment_id, offer_id, source, old_lease) in [
        (LOW_ASSESSMENT, LOW_TARGET_OFFER, 5_200, LOW_LEASE),
        (HIGH_ASSESSMENT, HIGH_TARGET_OFFER, 5_300, HIGH_LEASE),
    ] {
        candidates
            .try_push(OpeningMoveCandidate {
                assessment_id,
                offer_id,
                score: score(60, 70, source + 1, source + 2),
                move_cost: MoveCostQuote {
                    source_id: source,
                    valid_through: at(120 * DAY),
                    reachable: true,
                    travel_seconds: 0,
                    moving_charge: money(MOVING_FEE),
                    moving_provider_account: PROVIDER,
                    old_lease_close_due: Money::ZERO,
                    old_close_payee_account: LANDLORD,
                    old_clause_version: 1,
                    old_contract_exit_source_id: 9_100 + u64::from(old_lease),
                    old_escrow_account: 1,
                    deposit_resolution_source_id: 0,
                    old_deposit_refund_authorized: Money::ZERO,
                    old_deposit_deduction_authorized: Money::ZERO,
                    old_deposit_deduction_account: 0,
                },
            })
            .unwrap();
    }

    OpeningHousing {
        unit_limit: 4,
        city_id: Some(CITY),
        dwellings,
        households,
        lease_offers,
        move_assessments: assessments,
        move_candidates: candidates,
    }
}

fn envelope(store: &Store, action: Action) -> Result<Envelope, String> {
    let revision = store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .world()
        .view()
        .revision;
    Ok(Envelope {
        world: 1,
        branch: 1,
        operation_id: store.committed_sequence() + 1,
        expected_revision: revision,
        actions: vec![action],
    })
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> Result<u64, String> {
    let request = envelope(store, action)?;
    let operation_id = request.operation_id;
    let outcome = store
        .execute(caller, request)
        .map_err(|error| format!("Store execute: {error:?}"))?;
    let receipt = outcome
        .model
        .map_err(|error| format!("model operation {operation_id}: {error:?}"))?;
    if !outcome.has_persistent_receipt || receipt.operation_id != operation_id {
        return Err(format!(
            "operation {operation_id} lacks its persistent receipt"
        ));
    }
    Ok(operation_id)
}

fn drive_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..10_000 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: T::Advance { target, limit: 256 },
            },
        )?;
        let progress = store
            .engine()
            .map_err(|error| format!("engine: {error:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress;
        if progress.target.is_none() && progress.last_complete == target {
            return Ok(());
        }
    }
    Err(format!("Timeline did not complete target {target}"))
}

fn add_payroll(store: &mut Store, id: u32, payee: u32, hourly_cents: i64) -> Result<(), String> {
    execute(
        store,
        Caller::System,
        Action::Timeline {
            command: T::AddPayroll {
                spec: PayrollSpec {
                    id,
                    payer: EMPLOYER,
                    payee,
                    home_city: 2,
                    hourly_cents: money(hourly_cents),
                    cycle: Cycle::Weekly,
                    ends_at: 150 * DAY,
                },
            },
        },
    )?;
    Ok(())
}

fn accept_offer(store: &mut Store, account: u32, offer_id: u64) -> Result<(), String> {
    let offer = opening_housing()
        .lease_offers
        .iter()
        .find(|row| row.offer_id == offer_id)
        .copied()
        .ok_or_else(|| format!("unknown fixture offer {offer_id}"))?;
    execute(
        store,
        Caller::Account(account),
        Action::HousingLease {
            offer_id,
            offer_sha256: genesis_lease_offer_sha256(offer),
        },
    )?;
    Ok(())
}

fn wait_operation_id(store: &Store, household_id: u32) -> Result<u64, String> {
    store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .housing_move_state()
        .and_then(|state| state.wait_for(household_id))
        .map(|wait| wait.operation_id)
        .ok_or_else(|| format!("missing persisted wait for household {household_id}"))
}

fn move_wait(store: &Store, household_id: u32) -> Result<Option<(u64, MoveWait)>, String> {
    Ok(store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .housing_move_state()
        .and_then(|state| state.wait_for(household_id))
        .map(|wait| (wait.operation_id, wait.reason)))
}

fn wage_window(
    store: &Store,
    household_id: u32,
) -> Result<mycity_core::housing::PaidWageWindowView, String> {
    store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .housing_paid_wage_window(household_id)
        .ok_or_else(|| format!("missing paid-wage window for household {household_id}"))
}

fn claim(store: &Store, lease_id: u32) -> Result<Claim, String> {
    store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .timeline()
        .and_then(|timeline| {
            timeline
                .claims
                .iter()
                .find(|claim| claim.id == u64::from(lease_id))
        })
        .copied()
        .ok_or_else(|| format!("missing rent Claim for lease {lease_id}"))
}

fn cash(store: &Store, account: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .map(|row| row.cash.value())
        .ok_or_else(|| format!("missing account {account}"))
}

fn total_cash(store: &Store) -> Result<i64, String> {
    store
        .engine()
        .map_err(|error| format!("engine: {error:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .try_fold(0_i64, |total, row| {
            total
                .checked_add(row.cash.value())
                .ok_or_else(|| "account cash total overflow".to_string())
        })
}

fn wage_window_report(store: &Store, household_id: u32) -> Value {
    let Some(window) = store
        .engine()
        .ok()
        .and_then(|engine| engine.housing_paid_wage_window(household_id))
    else {
        return Value::Null;
    };
    let status = match window.status {
        PaidWageWindowStatus::CoverageIncomplete => "coverage_incomplete",
        PaidWageWindowStatus::NoQualifiedPayroll => "no_qualified_payroll",
        PaidWageWindowStatus::BilledArrears => "billed_arrears",
        PaidWageWindowStatus::Complete => "complete",
        PaidWageWindowStatus::Invalid => "invalid",
    };
    json!({
        "status": status,
        "household_id": window.household_id,
        "account": window.account,
        "start_day": window.start_day,
        "end_day": window.end_day,
        "actual_paid_wages_cents": window.actual_paid_wages.value(),
        "qualifying_contracts": window.qualifying_contracts,
        "active_contracts": window.active_contracts,
    })
}

fn state_report(store: &Store, stage: &str, world_path: &std::path::Path) -> Result<Value, String> {
    let engine = store
        .engine()
        .map_err(|error| format!("{stage} engine: {error:?}"))?;
    let world = engine.world();
    let view = world.view();
    let timeline = engine.timeline();
    let claims = timeline
        .as_ref()
        .map(|timeline| timeline.claims.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let (waits, history) = engine
        .housing_move_state()
        .map(|state| (state.waits().to_vec(), state.history().to_vec()))
        .unwrap_or_default();
    let accounts = view
        .accounts
        .iter()
        .map(|account| {
            json!({
                "id": account.id,
                "cash_cents": account.cash.value(),
                "protected_cents": account.protected.value(),
            })
        })
        .collect::<Vec<_>>();
    let cash_total = total_cash(store)?;
    let store_owned = store.owned_capacity_bytes().ok();

    Ok(json!({
        "stage": stage,
        "world_path": world_path,
        "sequence": store.committed_sequence(),
        "state_hash": store.state_hash().map_err(|error| format!("{stage} state hash: {error:?}"))?,
        "housing": serde_json::to_value(&view.housing).map_err(|error| format!("{stage} housing: {error}"))?,
        "housing_move": { "waits": waits, "historical_leases": history },
        "paid_wage_windows": {
            LOW_HOUSEHOLD.to_string(): wage_window_report(store, LOW_HOUSEHOLD),
            HIGH_HOUSEHOLD.to_string(): wage_window_report(store, HIGH_HOUSEHOLD),
        },
        "rent_claims": claims,
        "accounts": accounts,
        "account_cash_total_cents": cash_total,
        "owned_capacity_bytes": {
            "measurement": "owned_capacity_api_only_not_native_peak",
            "store": store_owned,
            "engine": engine.owned_capacity_bytes(),
            "world_components": world.layout(),
        }
    }))
}

fn verify_open(path: PathBuf) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!(
            "working-copy directory not found: {}",
            path.display()
        ));
    }
    let path = fs::canonicalize(&path).map_err(|error| format!("canonicalize: {error}"))?;
    eprintln!(
        "NOTICE Store::open can replay recovery data or truncate an incomplete WAL tail; verifying only this working copy: {}",
        path.display()
    );
    let store = Store::open(&path, RULES).map_err(|error| format!("Store::open: {error:?}"))?;
    let mut report = state_report(&store, "verify_open", &path)?;
    report["mode"] = json!("verify_open");
    report["model_commands_issued"] = json!(0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}

fn run(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "refusing to reuse existing world: {}",
            path.display()
        ));
    }
    fs::create_dir_all(path.parent().ok_or("world path requires a parent")?)
        .map_err(|error| format!("create parent: {error}"))?;
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-region-opening.json"))
            .map_err(|error| format!("opening fixture: {error}"))?;
    opening["region"]["routes"] = json!([]);
    opening["region"]["obligations"] = json!([]);
    opening["accounts"][1]["cash"] = json!(OPENING_CASH);
    opening["sources"][1]["amount"] = json!(OPENING_CASH);
    opening["accounts"].as_array_mut().unwrap().push(json!({
        "id": HIGH_TENANT,
        "role": "household",
        "cash": OPENING_CASH,
        "source": SECOND_TENANT_SOURCE,
    }));
    opening["sources"].as_array_mut().unwrap().push(json!({
        "id": SECOND_TENANT_SOURCE,
        "kind": "cash_capital",
        "amount": OPENING_CASH,
    }));
    opening["housing"] = serde_json::to_value(opening_housing())
        .map_err(|error| format!("housing opening: {error}"))?;

    let mut store = Store::create(&path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES)
        .map_err(|error| format!("Store create: {error:?}"))?;
    let world_path = fs::canonicalize(&path).map_err(|error| format!("canonicalize: {error}"))?;
    eprintln!("HOUSING_INCOME_MOVE_WORLD={}", world_path.display());
    let opening_cash_total = total_cash(&store)?;
    let opening_report = state_report(&store, "opening", &world_path)?;

    execute(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    )?;
    add_payroll(&mut store, LOW_PAYROLL, LOW_TENANT, LOW_HOURLY_CENTS)?;
    add_payroll(&mut store, HIGH_PAYROLL, HIGH_TENANT, HIGH_HOURLY_CENTS)?;
    drive_to(&mut store, ACCEPT_AT)?;
    accept_offer(&mut store, LOW_TENANT, LOW_OFFER)?;
    accept_offer(&mut store, HIGH_TENANT, HIGH_OFFER)?;

    for (household, account) in [(LOW_HOUSEHOLD, LOW_TENANT), (HIGH_HOUSEHOLD, HIGH_TENANT)] {
        execute(
            &mut store,
            Caller::Account(account),
            Action::HousingMove {
                assessment_id: u64::from(household),
                city_id: CITY,
                expected_wait_operation_id: 0,
            },
        )?;
        if move_wait(&store, household)?.map(|(_, reason)| reason)
            != Some(MoveWait::EvidenceIncomplete)
        {
            return Err(format!(
                "household {household} did not persist the initial income-evidence Wait"
            ));
        }
    }
    let low_wait_op = wait_operation_id(&store, LOW_HOUSEHOLD)?;
    let high_wait_op = wait_operation_id(&store, HIGH_HOUSEHOLD)?;
    let after_wait_report = state_report(&store, "after_wait", &world_path)?;

    drive_to(&mut store, MOVE_AT)?;
    let low_income = wage_window(&store, LOW_HOUSEHOLD)?;
    let high_income = wage_window(&store, HIGH_HOUSEHOLD)?;
    if low_income.status != PaidWageWindowStatus::Complete
        || low_income.actual_paid_wages.value() != 28_000
        || low_income.qualifying_contracts != 1
        || high_income.status != PaidWageWindowStatus::Complete
        || high_income.actual_paid_wages.value() != 245_000
        || high_income.qualifying_contracts != 1
    {
        return Err(format!(
            "unexpected 31-day paid wage windows: low={low_income:?}, high={high_income:?}"
        ));
    }

    let low_before_move_claim = claim(&store, LOW_LEASE)?;
    let high_before_move_claim = claim(&store, HIGH_LEASE)?;
    if low_before_move_claim.principal.value() != LOW_FIRST_INVOICE
        || low_before_move_claim.paid >= low_before_move_claim.principal
        || high_before_move_claim.principal.value() != HIGH_FIRST_INVOICE
        || high_before_move_claim.paid != high_before_move_claim.principal
    {
        return Err(format!(
            "rent/P2 result did not distinguish arrears: low={low_before_move_claim:?}, high={high_before_move_claim:?}"
        ));
    }

    execute(
        &mut store,
        Caller::Account(LOW_TENANT),
        Action::HousingMove {
            assessment_id: LOW_ASSESSMENT,
            city_id: CITY,
            expected_wait_operation_id: low_wait_op,
        },
    )?;
    let low_wait = move_wait(&store, LOW_HOUSEHOLD)?;
    if low_wait.map(|(_, reason)| reason) != Some(MoveWait::CandidateNotBetter) {
        return Err(format!(
            "low-income household did not retain the expected affordability Wait: {low_wait:?}"
        ));
    }

    execute(
        &mut store,
        Caller::Account(HIGH_TENANT),
        Action::HousingMove {
            assessment_id: HIGH_ASSESSMENT,
            city_id: CITY,
            expected_wait_operation_id: high_wait_op,
        },
    )?;
    if move_wait(&store, HIGH_HOUSEHOLD)?.is_some() {
        return Err("high-income household retained a Wait after its affordable move".into());
    }
    let housing = store
        .engine()
        .map_err(|error| format!("engine after moves: {error:?}"))?
        .world()
        .view()
        .housing
        .ok_or_else(|| "housing view missing".to_string())?;
    let unit = |id| housing.units.iter().find(|row| row.unit_id == id).copied();
    if unit(LOW_UNIT).map(|row| (row.household_id, row.lease_id))
        != Some((LOW_HOUSEHOLD, LOW_LEASE))
        || unit(HIGH_UNIT).map(|row| (row.household_id, row.lease_id)) != Some((0, 0))
        || unit(LOW_TARGET_UNIT).map(|row| (row.household_id, row.lease_id)) != Some((0, 0))
        || unit(HIGH_TARGET_UNIT).map(|row| (row.household_id, row.lease_id))
            != Some((HIGH_HOUSEHOLD, HIGH_TARGET_LEASE))
    {
        return Err("post-move occupancy differs from the expected wait/success outcomes".into());
    }
    if unit(LOW_UNIT).map(|row| row.arrears_ref) != Some(LOW_LEASE)
        || unit(HIGH_UNIT).map(|row| row.arrears_ref) != Some(0)
    {
        return Err("housing-side rent references disagree with the active debt/lease rows".into());
    }
    let high_target_claim = claim(&store, HIGH_TARGET_LEASE)?;
    if high_target_claim.due != 59 * DAY
        || high_target_claim.principal.value() != HIGH_TARGET_FIRST_INVOICE
        || high_target_claim.paid != Money::ZERO
    {
        return Err(format!(
            "high-income destination rent did not open at its calendar due: {high_target_claim:?}"
        ));
    }

    let pre_checkpoint_hash = store
        .state_hash()
        .map_err(|error| format!("state hash: {error:?}"))?;
    let pre_checkpoint_sequence = store.committed_sequence();
    let low_wait_final = move_wait(&store, LOW_HOUSEHOLD)?;
    let low_wait_report = low_wait_final.map(|(operation_id, reason)| {
        json!({
            "operation_id": operation_id,
            "reason": format!("{reason:?}"),
        })
    });
    let high_old_final_claim = claim(&store, HIGH_LEASE)?;
    let low_final_claim = claim(&store, LOW_LEASE)?;
    let high_old_outstanding = high_old_final_claim
        .principal
        .value()
        .checked_sub(high_old_final_claim.paid.value());
    if low_final_claim != low_before_move_claim
        || high_old_final_claim.paid != money(HIGH_FIRST_INVOICE)
        || high_old_final_claim.principal.value() != HIGH_FIRST_INVOICE + HIGH_FINAL_ACCRUAL
        || high_old_outstanding != Some(HIGH_FINAL_ACCRUAL)
    {
        return Err("migration changed low household rent or lost the high household's historical rent balance".into());
    }
    let low_source = RentClaimSource {
        landlord: LANDLORD,
        tenant: LOW_TENANT,
        lease_id: LOW_LEASE,
        unit_id: LOW_UNIT,
    };
    let high_old_source = RentClaimSource {
        landlord: LANDLORD,
        tenant: HIGH_TENANT,
        lease_id: HIGH_LEASE,
        unit_id: HIGH_UNIT,
    };
    let high_new_source = RentClaimSource {
        landlord: LANDLORD,
        tenant: HIGH_TENANT,
        lease_id: HIGH_TARGET_LEASE,
        unit_id: HIGH_TARGET_UNIT,
    };
    let timeline = store.engine().unwrap().timeline().unwrap();
    if timeline.rent_source_for_claim(&low_final_claim) != Some(low_source)
        || timeline.rent_source_for_claim(&high_old_final_claim) != Some(high_old_source)
        || timeline.rent_source_for_claim(&high_target_claim) != Some(high_new_source)
    {
        return Err("rent Claim sources do not match their original or destination leases".into());
    }
    let _ = timeline;
    let employer_cash = cash(&store, EMPLOYER)?;
    let expected_employer_cash = 600_000 - 28_000 - 245_000 + MOVING_FEE;
    if employer_cash != expected_employer_cash || total_cash(&store)? != opening_cash_total {
        return Err(format!(
            "payroll/moving fee cash flow mismatch: employer={employer_cash}, expected={expected_employer_cash}"
        ));
    }
    let after_move_report = state_report(&store, "after_move", &world_path)?;

    store
        .checkpoint()
        .map_err(|error| format!("checkpoint: {error:?}"))?;
    drop(store);
    let store = Store::open(&path, RULES).map_err(|error| format!("cold open: {error:?}"))?;
    if store
        .state_hash()
        .map_err(|error| format!("cold state hash: {error:?}"))?
        != pre_checkpoint_hash
        || store.committed_sequence() != pre_checkpoint_sequence
        || claim(&store, LOW_LEASE)? != low_final_claim
        || claim(&store, HIGH_LEASE)? != high_old_final_claim
        || claim(&store, HIGH_TARGET_LEASE)? != high_target_claim
        || move_wait(&store, LOW_HOUSEHOLD)? != low_wait_final
        || move_wait(&store, HIGH_HOUSEHOLD)?.is_some()
        || wage_window(&store, LOW_HOUSEHOLD)? != low_income
        || wage_window(&store, HIGH_HOUSEHOLD)? != high_income
        || total_cash(&store)? != opening_cash_total
    {
        return Err(
            "Store cold-open changed wage windows, rent Claims, occupancy Wait or state identity"
                .into(),
        );
    }
    let final_cold_report = state_report(&store, "final_cold_open", &world_path)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scenario": "two_household_paid_income_rent_arrears_move",
            "world_path": world_path,
            "rules_identity": RULES,
            "run_status": "scenario_assertions_passed",
            "acceptance_scope": "candidate_scenario_assertions_only_not_full_product_or_performance_acceptance",
            "timeline_day": MOVE_AT / DAY,
            "income_window_days": 31,
            "households": {
                "low_income": {
                    "household_id": LOW_HOUSEHOLD,
                    "actual_paid_wages_cents": low_income.actual_paid_wages.value(),
                    "candidate_monthly_rent_cents": LOW_TARGET_RENT,
                    "rent_to_observed_wages_percent_floor": LOW_TARGET_RENT * 100 / low_income.actual_paid_wages.value(),
                    "first_rent_claim": low_before_move_claim,
                    "rent_arrears_cents": low_final_claim.principal.value() - low_final_claim.paid.value(),
                    "move_wait": low_wait_report,
                    "cash_cents": cash(&store, LOW_TENANT)?,
                },
                "higher_income": {
                    "household_id": HIGH_HOUSEHOLD,
                    "actual_paid_wages_cents": high_income.actual_paid_wages.value(),
                    "candidate_monthly_rent_cents": HIGH_TARGET_RENT,
                    "rent_to_observed_wages_percent_floor": HIGH_TARGET_RENT * 100 / high_income.actual_paid_wages.value(),
                    "old_rent_claim": high_old_final_claim,
                    "old_rent_arrears_cents": high_old_final_claim.principal.value() - high_old_final_claim.paid.value(),
                    "destination_rent_claim": high_target_claim,
                    "destination_cash_cents": cash(&store, HIGH_TENANT)?,
                    "move_wait": null,
                }
            },
            "checkpoint_cold_open_equal": true,
            "sequence": pre_checkpoint_sequence,
            "state_hash": pre_checkpoint_hash,
            "opening_account_cash_total": opening_cash_total,
            "account_cash_total": total_cash(&store)?,
            "employer_and_move_provider_cash_cents": cash(&store, PROVIDER)?,
            "moving_fee_cents": MOVING_FEE,
            "owned_capacity_measurement": "owned_capacity_api_only_not_native_peak",
            "stage_reports": [opening_report, after_wait_report, after_move_report, final_cold_report],
        }))
        .unwrap()
    );
    Ok(())
}

fn main() {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    let result = match first.as_deref() {
        Some(arg) if arg.to_str() == Some("--verify-open") => match args.next() {
            Some(path) if args.next().is_none() => verify_open(PathBuf::from(path)),
            _ => {
                Err("usage: town_housing_income_move --verify-open <existing-working-copy>".into())
            }
        },
        Some(arg) if matches!(arg.to_str(), Some("--help" | "-h")) => {
            println!(
                "Usage:\n  town_housing_income_move <new-world-dir>\n  town_housing_income_move --verify-open <existing-working-copy>\n\nStore::open can replay recovery data or truncate an incomplete WAL tail; use --verify-open only on an isolated working copy."
            );
            return;
        }
        Some(path) if args.next().is_none() => run(PathBuf::from(path)),
        Some(_) => Err("provide exactly one new-world directory".into()),
        None => Err("usage: town_housing_income_move <new-world-dir>".into()),
    };
    if let Err(error) = result {
        eprintln!("town_housing_income_move: {error}");
        std::process::exit(2);
    }
}
