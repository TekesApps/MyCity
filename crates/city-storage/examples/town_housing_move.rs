//! Demonstrate a persisted household move through the regular Store and Timeline.
//!
//! Usage:
//! - `cargo run -p mycity-storage --example town_housing_move -- <new-world-dir>`
//! - `cargo run -p mycity-storage --example town_housing_move -- --verify-open <world-dir>`
//!
//! `--verify-open` calls `Store::open` with the fixed rules identity. Store recovery may
//! replay or truncate an incomplete WAL tail, so run it only on an isolated working copy,
//! never on the preserved original evidence directory. It issues no model command.
//! The immutable portfolio belongs to city 1 while city 2 is initially active. A first
//! move request creates a wage-evidence Wait; after 31 complete wage days, retrying the
//! same assessment with the Wait operation ID moves into a better vacant unit. The old
//! rent Claim remains in Timeline and is explicitly settled after the move.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, HousingUnit, LeaseTerms, MoveCostQuote, MoveScore, MoveWait,
        OpeningDwelling, OpeningHousehold, OpeningHousing, OpeningMoveAssessment,
        OpeningMoveCandidate, MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
    },
    model_rows::Rows,
    timeline::{Claim, Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

const RULES: &str = "m0-storage-process-fixture-v1";
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
const SETTLE_AT: u64 = 42 * DAY;
const OLD_RENT: i64 = 480_000;
const NEW_RENT: i64 = 40_000;
const WEEKLY_WAGE: i64 = 49_000;
const MOVING_FEE: i64 = 10_000;
const OPENING_TENANT_CASH: i64 = 85_000;

fn money(value: i64) -> Money {
    Money::new(value).expect("non-negative money")
}

fn game_time(value: u64) -> GameTime {
    GameTime::new(i64::try_from(value).expect("game time fits i64")).unwrap()
}

fn score(lower: u8, upper: u8, env: u64, services: u64, at: u64) -> MoveScore {
    MoveScore {
        lower,
        upper,
        fully_covered: true,
        environment_source: env,
        public_service_source: services,
        source_version: 1,
        assessed_at: game_time(at),
        valid_through: game_time(120 * DAY),
        hard_failure_mask: 0,
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

fn opening_housing() -> OpeningHousing {
    let old_offer = offer(OLD_OFFER, OLD_LEASE, OLD_UNIT, 10, OLD_RENT, ACCEPT_AT);
    let new_offer = offer(NEW_OFFER, NEW_LEASE, NEW_UNIT, 11, NEW_RENT, MOVE_AT);
    let mut dwellings = Rows::<OpeningDwelling, MAX_M0_HOUSING_UNITS>::new().unwrap();
    for (unit_id, building_id) in [(OLD_UNIT, 10), (NEW_UNIT, 11)] {
        dwellings
            .try_push(OpeningDwelling {
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
            })
            .unwrap();
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
    move_assessments
        .try_push(OpeningMoveAssessment {
            assessment_id: ASSESSMENT,
            household_id: HOUSEHOLD,
            source_id: 5_005,
            version: 1,
            old_score: score(10, 20, 5_001, 5_002, ACCEPT_AT),
            low_since: Some(GameTime::ZERO),
            covered_through: game_time(120 * DAY),
            continuous_low_and_covered: true,
            cooldown_until: None,
            essential_costs_per_month: Money::ZERO,
        })
        .unwrap();
    let mut move_candidates = Rows::new().unwrap();
    move_candidates
        .try_push(OpeningMoveCandidate {
            assessment_id: ASSESSMENT,
            offer_id: NEW_OFFER,
            score: score(60, 70, 5_003, 5_004, ACCEPT_AT),
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
        })
        .unwrap();
    OpeningHousing {
        unit_limit: 2,
        city_id: Some(CITY),
        dwellings,
        households,
        lease_offers,
        move_assessments,
        move_candidates,
    }
}

fn envelope(store: &Store, action: Action) -> Result<Envelope, String> {
    let revision = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
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

fn execute(store: &mut Store, caller: Caller, action: Action) -> Result<(), String> {
    let request = envelope(store, action)?;
    let outcome = store
        .execute(caller, request)
        .map_err(|e| format!("store execute: {e:?}"))?;
    outcome
        .model
        .map_err(|e| format!("model operation: {e:?}"))?;
    if !outcome.has_persistent_receipt {
        return Err("operation has no persistent receipt".into());
    }
    Ok(())
}

fn drive_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..10_000 {
        let progress = store
            .engine()
            .map_err(|e| format!("store engine: {e:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress;
        if progress.target.is_none() && progress.last_complete == target {
            return Ok(());
        }
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: T::Advance { target, limit: 256 },
            },
        )?;
    }
    Err(format!("Timeline did not complete target {target}"))
}

fn claim(store: &Store, id: u32) -> Result<Claim, String> {
    store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .claims
        .iter()
        .find(|claim| claim.id == u64::from(id))
        .copied()
        .ok_or_else(|| format!("Claim {id} missing"))
}

fn payroll(store: &Store, id: u32) -> Result<mycity_core::timeline::Payroll, String> {
    store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .payroll
        .iter()
        .find(|payroll| payroll.spec.id == id)
        .copied()
        .ok_or_else(|| format!("Payroll {id} missing"))
}

fn account_cash(store: &Store, id: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == id)
        .map(|account| account.cash.value())
        .ok_or_else(|| format!("account {id} missing"))
}

fn stage_report(store: &Store, stage: &str) -> Result<Value, String> {
    let sequence = store.committed_sequence();
    let state_hash = store
        .state_hash()
        .map_err(|e| format!("{stage} state hash: {e:?}"))?;
    let engine = store
        .engine()
        .map_err(|e| format!("{stage} engine: {e:?}"))?;
    let world = engine.world();
    let view = world.view();
    let cash_total = view.accounts.iter().try_fold(0_i64, |sum, account| {
        sum.checked_add(account.cash.value())
            .ok_or_else(|| format!("{stage} account cash total overflow"))
    })?;
    let accounts: Vec<_> = view
        .accounts
        .iter()
        .map(|account| {
            json!({
                "id": account.id,
                "cash": account.cash.value(),
                "protected": account.protected.value(),
            })
        })
        .collect();
    let housing =
        serde_json::to_value(&view.housing).map_err(|e| format!("{stage} housing view: {e}"))?;
    let (waits, history) = engine
        .housing_move_state()
        .map(|state| (state.waits().to_vec(), state.history().to_vec()))
        .unwrap_or_default();
    let timeline = engine
        .timeline()
        .map(|timeline| {
            json!({
                "progress": &timeline.progress,
                "payroll": timeline.payroll.iter().collect::<Vec<_>>(),
                "claims": timeline.claims.iter().collect::<Vec<_>>(),
            })
        })
        .unwrap_or(Value::Null);
    let owned_capacity_bytes = json!({
        "measurement": "owned_capacity_api_only_not_native_peak",
        "store": store.owned_capacity_bytes()
            .map_err(|e| format!("{stage} Store owned capacity: {e:?}"))?,
        "engine": engine.owned_capacity_bytes(),
        "world_components": world.layout(),
    });
    Ok(json!({
        "stage": stage,
        "sequence": sequence,
        "state_hash": state_hash,
        "owned_capacity_bytes": owned_capacity_bytes,
        "account_cash_total": cash_total,
        "accounts": accounts,
        "active_city": view.region.as_ref().map(|region| region.active_city),
        "housing": housing,
        "housing_move": {
            "waits": waits,
            "historical_leases": history,
        },
        "timeline": timeline,
    }))
}

fn assert_cold_restore_matches(before: &Value, after: &Value) -> Result<(), String> {
    for field in [
        "sequence",
        "state_hash",
        "account_cash_total",
        "housing",
        "housing_move",
    ] {
        if before.get(field) != after.get(field) {
            return Err(format!(
                "cold-open changed {field}: before={:?}; after={:?}",
                before.get(field),
                after.get(field)
            ));
        }
    }
    for field in ["claims", "payroll"] {
        if before["timeline"].get(field) != after["timeline"].get(field) {
            return Err(format!(
                "cold-open changed Timeline {field}: before={:?}; after={:?}",
                before["timeline"].get(field),
                after["timeline"].get(field)
            ));
        }
    }
    Ok(())
}

fn run(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "refusing to reuse existing world {}",
            path.display()
        ));
    }
    fs::create_dir_all(path.parent().ok_or("world path needs parent")?)
        .map_err(|e| format!("create parent: {e}"))?;
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-region-opening.json"))
            .map_err(|e| format!("opening fixture: {e}"))?;
    opening["region"]["routes"] = json!([]);
    opening["region"]["obligations"] = json!([]);
    opening["accounts"][1]["cash"] = json!(OPENING_TENANT_CASH);
    opening["sources"][1]["amount"] = json!(OPENING_TENANT_CASH);
    opening["housing"] =
        serde_json::to_value(opening_housing()).map_err(|e| format!("housing opening: {e}"))?;
    let mut store = Store::create(&path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES)
        .map_err(|e| format!("Store create: {e:?}"))?;
    let artifact = fs::canonicalize(&path).map_err(|e| format!("world canonicalize: {e}"))?;
    eprintln!("HOUSING_MOVE_WORLD={}", artifact.display());
    let opening_report = stage_report(&store, "opening")?;

    execute(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    )?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: T::AddPayroll {
                spec: PayrollSpec {
                    id: 700,
                    payer: EMPLOYER,
                    payee: TENANT,
                    home_city: 2,
                    hourly_cents: money(875),
                    cycle: Cycle::Weekly,
                    ends_at: 150 * DAY,
                },
            },
        },
    )?;
    drive_to(&mut store, ACCEPT_AT)?;
    let old_offer = opening_housing().lease_offers[0];
    execute(
        &mut store,
        Caller::Account(TENANT),
        Action::HousingLease {
            offer_id: OLD_OFFER,
            offer_sha256: genesis_lease_offer_sha256(old_offer),
        },
    )?;

    // Persist the initial wait and reopen the world before retrying it.
    let wait_action = Action::HousingMove {
        assessment_id: ASSESSMENT,
        city_id: CITY,
        expected_wait_operation_id: 0,
    };
    execute(&mut store, Caller::Account(TENANT), wait_action)?;
    let wait = store
        .engine()
        .map_err(|e| format!("engine after wait: {e:?}"))?
        .housing_move_state()
        .and_then(|state| state.wait_for(HOUSEHOLD))
        .ok_or_else(|| "expected persisted Move Wait".to_string())?;
    if wait.reason != MoveWait::EvidenceIncomplete {
        return Err(format!("unexpected first wait reason: {:?}", wait.reason));
    }
    let after_wait_report = stage_report(&store, "afterwait")?;
    let wait_operation_id = wait.operation_id;
    store
        .checkpoint()
        .map_err(|e| format!("checkpoint: {e:?}"))?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(|e| format!("cold open: {e:?}"))?;
    drive_to(&mut store, MOVE_AT)?;

    execute(
        &mut store,
        Caller::System,
        Action::RegionActivate { city: 3 },
    )?;
    let window = store
        .engine()
        .map_err(|e| format!("engine wage window: {e:?}"))?
        .housing_paid_wage_window(HOUSEHOLD)
        .ok_or_else(|| "wage-window view missing".to_string())?;
    if window.status != mycity_core::housing::PaidWageWindowStatus::Complete
        || window.actual_paid_wages.value() != 245_000
    {
        return Err(format!("unexpected 31-day paid wage window: {window:?}"));
    }
    let day38_payroll = payroll(&store, 700)?;
    if day38_payroll.last_accrual != MOVE_AT
        || day38_payroll.accrued.value() != 266_000
        || day38_payroll.billed.value() != 245_000
        || day38_payroll.paid.value() != 245_000
        || day38_payroll.work_seconds != 38 * 8 * 3_600
    {
        return Err(format!(
            "unexpected day-38 payroll basis: {day38_payroll:?}"
        ));
    }
    execute(
        &mut store,
        Caller::Account(TENANT),
        Action::HousingMove {
            assessment_id: ASSESSMENT,
            city_id: CITY,
            expected_wait_operation_id: wait_operation_id,
        },
    )?;
    let moved = store
        .engine()
        .map_err(|e| format!("engine after move: {e:?}"))?
        .world()
        .view();
    let units = &moved.housing.ok_or("housing view missing")?.units;
    let old: &HousingUnit = units.iter().find(|unit| unit.unit_id == OLD_UNIT).unwrap();
    let new: &HousingUnit = units.iter().find(|unit| unit.unit_id == NEW_UNIT).unwrap();
    if old.household_id != 0
        || old.lease_id != 0
        || new.household_id != HOUSEHOLD
        || new.lease_id != NEW_LEASE
    {
        return Err("move did not atomically transfer the household to the vacant unit".into());
    }
    let old_claim = claim(&store, OLD_LEASE)?;
    let expected_old_principal = 263_226 + 120_000;
    if old_claim.principal.value() != expected_old_principal || old_claim.paid.value() != 263_226 {
        return Err(format!(
            "old rent Claim changed unexpectedly: {old_claim:?}"
        ));
    }
    if account_cash(&store, PROVIDER)? != 600_000 - 5 * WEEKLY_WAGE + MOVING_FEE {
        return Err("moving fee did not reach its registered provider".into());
    }
    let after_move_report = stage_report(&store, "aftermove")?;

    // Switching away from payroll city 2 suspends new work accrual. At day 42, only the
    // 21,000 cents already earned by day 38 is paid. The closed lease's unpaid earned rent
    // remains on its original Claim and is collected only through explicit SettleClaim.
    drive_to(&mut store, SETTLE_AT)?;
    let day42_payroll = payroll(&store, 700)?;
    if day42_payroll.last_accrual != SETTLE_AT
        || day42_payroll.accrued != day38_payroll.accrued
        || day42_payroll.work_seconds != day38_payroll.work_seconds
        || day42_payroll.billed.value() != 266_000
        || day42_payroll.paid.value() != 266_000
        || day42_payroll.paid.value() - day38_payroll.paid.value() != 21_000
    {
        return Err(format!(
            "unexpected day-42 payroll settlement: {day42_payroll:?}"
        ));
    }
    if account_cash(&store, TENANT)? != 77_774
        || claim(&store, OLD_LEASE)?.paid.value() != 263_226
        || claim(&store, NEW_LEASE)?.paid.value() != 0
    {
        return Err("day-42 cash or rent Claim state differs from the payroll-only oracle".into());
    }
    let day42_report = stage_report(&store, "day42_before_settlement")?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: T::SettleClaim {
                id: u64::from(OLD_LEASE),
            },
        },
    )?;
    let settled = claim(&store, OLD_LEASE)?;
    if settled.paid.value() != 341_000
        || settled.principal.value() - settled.paid.value() != 42_226
        || account_cash(&store, TENANT)? != 0
    {
        return Err(format!(
            "historical Claim settlement differs from oracle: {settled:?}"
        ));
    }
    let final_before_checkpoint = stage_report(&store, "final_before_checkpoint")?;
    store
        .checkpoint()
        .map_err(|e| format!("final checkpoint: {e:?}"))?;
    drop(store);
    let store = Store::open(&path, RULES).map_err(|e| format!("final cold open: {e:?}"))?;
    let final_cold_report = stage_report(&store, "finalcold")?;
    assert_cold_restore_matches(&final_before_checkpoint, &final_cold_report)?;

    let day42_payroll = payroll(&store, 700)?;
    let old_claim = claim(&store, OLD_LEASE)?;
    let new_claim = claim(&store, NEW_LEASE)?;
    let provider_cash = account_cash(&store, PROVIDER)?;
    let tenant_cash = account_cash(&store, TENANT)?;
    let report = json!({
        "mode": "run",
        "world_path": artifact,
        "rules_identity": RULES,
        "owned_capacity_scope": "Rust-owned capacities only; not process RSS or native peak",
        "stages": {
            "opening": opening_report,
            "afterwait": after_wait_report,
            "aftermove": after_move_report,
            "day42_before_settlement": day42_report,
            "final_before_checkpoint": final_before_checkpoint,
            "finalcold": final_cold_report,
        },
        "wages": {
            "day38": day38_payroll,
            "day42": day42_payroll,
            "day42_paid_delta": day42_payroll.paid.value() - day38_payroll.paid.value(),
        },
        "claims": {
            "old_final": old_claim,
            "new_final": new_claim,
            "old_outstanding": old_claim.principal.value() - old_claim.paid.value(),
            "new_outstanding": new_claim.principal.value() - new_claim.paid.value(),
        },
        "moving_fee": {
            "amount": MOVING_FEE,
            "provider_account": PROVIDER,
            "provider_cash_final": provider_cash,
        },
        "account_cash_total_final": final_cold_report["account_cash_total"],
        "tenant_cash_final": tenant_cash,
        "historical_leases_final": final_cold_report["housing_move"]["historical_leases"],
        "waits_final": final_cold_report["housing_move"]["waits"],
        "checkpoint_cold_restore": "passed",
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}

fn verify_open(path: PathBuf) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!(
            "existing world directory not found: {}",
            path.display()
        ));
    }
    let path = fs::canonicalize(&path).map_err(|e| format!("canonicalize world: {e}"))?;
    eprintln!(
        "NOTICE Store::open can replay recovery data or truncate an incomplete WAL tail; verifying only this working copy: {}",
        path.display()
    );
    let store = Store::open(&path, RULES).map_err(|e| format!("Store::open: {e:?}"))?;
    let sequence = store.committed_sequence();
    let state_hash = store
        .state_hash()
        .map_err(|e| format!("state hash: {e:?}"))?;
    let engine = store.engine().map_err(|e| format!("engine: {e:?}"))?;
    let view = engine.world().view();
    let cash_total = view.accounts.iter().try_fold(0_i64, |sum, account| {
        sum.checked_add(account.cash.value())
            .ok_or_else(|| "account cash total overflow".to_string())
    })?;
    let (waits, history) = engine
        .housing_move_state()
        .map(|state| (state.waits().to_vec(), state.history().to_vec()))
        .unwrap_or_default();
    let world_layout = engine.world().layout();
    let engine_owned_capacity_bytes = engine.owned_capacity_bytes();
    let store_owned_capacity_bytes = store
        .owned_capacity_bytes()
        .map_err(|e| format!("Store owned capacity: {e:?}"))?;

    let report = json!({
        "mode": "verify_open",
        "world_path": path,
        "rules_identity": RULES,
        "sequence": sequence,
        "state_hash": state_hash,
        "housing_move": {
            "waits": waits,
            "historical_leases": history,
        },
        "accounts": {
            "count": view.accounts.len(),
            "cash_total": cash_total,
        },
        "owned_capacity_bytes": {
            "store": store_owned_capacity_bytes,
            "engine": engine_owned_capacity_bytes,
            "world_components": world_layout,
        },
        "model_commands_issued": 0,
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}

fn main() {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    let result = match first.as_deref() {
        Some(arg) if arg.to_str() == Some("--verify-open") => match args.next() {
            Some(path) => verify_open(PathBuf::from(path)),
            None => Err("usage: town_housing_move --verify-open <existing-world-dir>".into()),
        },
        Some(arg) if matches!(arg.to_str(), Some("--help" | "-h")) => {
            println!(
                "Usage:\n  town_housing_move <new-world-dir>\n  town_housing_move --verify-open <existing-world-dir>\n\nStore::open can replay recovery data or truncate an incomplete WAL tail; use --verify-open only on an isolated working copy."
            );
            return;
        }
        Some(path) => run(PathBuf::from(path)),
        None => run(PathBuf::from("./town-housing-move-world")),
    };
    if let Err(error) = result {
        eprintln!("town_housing_move: {error}");
        std::process::exit(1);
    }
}
