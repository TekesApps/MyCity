//! Exercise the real Store wage-window read model across its Gregorian coverage boundary.
//!
//! Usage: `cargo run -p mycity-storage --example town_housing_income -- <new-world-dir>`.
//! This scenario is source-only until a serialized Store run is recorded; it must not be
//! described as passed based on compilation or on the companion test source.

use mycity_core::{
    calendar::Date,
    housing::{
        DwellingFact, DwellingOriginKind, GenesisLeaseOffer, LeaseTerms, OpeningDwelling,
        OpeningHousehold, OpeningHousing, PaidWageWindowStatus, MAX_GENESIS_LEASE_OFFERS,
        MAX_M0_HOUSING_UNITS,
    },
    model_rows::Rows,
    timeline::{Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

const RULES: &str = "m0-storage-process-fixture-v1";
const TENANT: u32 = 2;
const EMPLOYER: u32 = 3;
const LANDLORD: u32 = 4;
const HOUSEHOLD: u32 = 501;
const UNIT: u32 = 101;
const BUILDING: u32 = 10;
const LEASE: u32 = 9001;
const PAYROLL: u32 = 700;
const RENT_CENTS: i64 = 480_000;
const HOURLY_CENTS: i64 = 800;
const DAY_31: u64 = 31 * DAY;
const DAY_32: u64 = 32 * DAY;
const PAID_AT_DAY_31_CENTS: i64 = 31 * 8 * HOURLY_CENTS;
const EXPECTED_WAGE_WINDOW_OWNED_BYTES: u64 = 8_456;

fn money(cents: i64) -> Money {
    Money::new(cents).expect("scenario money is non-negative")
}

fn opening_housing() -> OpeningHousing {
    let fact = DwellingFact {
        unit_id: UNIT,
        building_id: BUILDING,
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
        contract_source_id: 9_101,
        landlord_authorization_id: 9_102,
        tenant_consent_id: 9_103,
        rent_per_cycle: money(RENT_CENTS),
        cycle: Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: 1,
        starts_at: GameTime::ZERO,
    };
    let offer = GenesisLeaseOffer {
        offer_id: 9_001,
        world_id: 1,
        branch_id: 1,
        unit_id: UNIT,
        building_id: BUILDING,
        owner_account: LANDLORD,
        tenant_account: TENANT,
        household_id: HOUSEHOLD,
        valid_from: GameTime::ZERO,
        valid_until: Date::new(2001, 4, 1).unwrap().midnight(),
        asset_source_id: 7_001,
        authority_source_id: 7_002,
        contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
        lease_id: terms.lease_id,
        clause_version: terms.clause_version,
        rent_per_cycle: terms.rent_per_cycle,
        cycle: terms.cycle,
        deposit: terms.deposit,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: mycity_core::housing::lease_terms_sha256(terms).unwrap(),
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

fn envelope(store: &Store, action: Action) -> Result<Envelope, String> {
    let revision = store
        .engine()
        .map_err(|error| format!("store engine: {error:?}"))?
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
    let operation_id = request.operation_id;
    let outcome = store
        .execute(caller, request)
        .map_err(|error| format!("store execute: {error:?}"))?;
    let receipt = outcome
        .model
        .map_err(|error| format!("model operation {operation_id}: {error:?}"))?;
    if !outcome.has_persistent_receipt || receipt.operation_id != operation_id {
        return Err(format!(
            "operation {operation_id} lacks its persistent receipt"
        ));
    }
    Ok(())
}

fn wage_window(store: &Store) -> Result<mycity_core::housing::PaidWageWindowView, String> {
    store
        .engine()
        .map_err(|error| format!("store engine: {error:?}"))?
        .housing_paid_wage_window(HOUSEHOLD)
        .ok_or_else(|| "registered household has no wage-window view".into())
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
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
            .map_err(|error| format!("store engine: {error:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress;
        if progress.target.is_none() && progress.last_complete == target {
            return Ok(());
        }
    }
    Err(format!("Timeline did not complete target {target}"))
}

fn resource_snapshot(
    store: &Store,
    stage: &str,
    artifact_path: &Path,
    elapsed_ms: u128,
) -> Result<Value, String> {
    let engine = store
        .engine()
        .map_err(|error| format!("store engine for {stage}: {error:?}"))?;
    let layout = engine.world().layout();
    let wage_window_bytes = layout
        .get("housing_paid_wage_window_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("world layout lacks housing wage-window bytes at {stage}"))?;
    Ok(json!({
        "stage": stage,
        "elapsed_ms": elapsed_ms,
        "artifact_path": artifact_path.display().to_string(),
        "engine_owned_capacity_bytes": engine.owned_capacity_bytes(),
        "store_owned_capacity_bytes": store
            .owned_capacity_bytes()
            .map_err(|error| format!("Store owned capacity at {stage}: {error:?}"))?,
        "housing_paid_wage_window_owned_bytes": wage_window_bytes
    }))
}

fn run(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "refusing to reuse existing directory: {}",
            path.display()
        ));
    }
    fs::create_dir_all(path.parent().ok_or("world path must have a parent")?)
        .map_err(|error| format!("create parent: {error}"))?;
    let started = Instant::now();

    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
            .map_err(|error| format!("opening fixture: {error}"))?;
    opening["housing"] = serde_json::to_value(opening_housing())
        .map_err(|error| format!("housing opening: {error}"))?;
    let mut store = Store::create(&path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES)
        .map_err(|error| format!("store create: {error:?}"))?;
    let artifact_path = fs::canonicalize(&path)
        .map_err(|error| format!("resolve absolute artifact path: {error}"))?;
    let mut resource_samples = Vec::new();
    resource_samples.push(resource_snapshot(
        &store,
        "store_created_before_timeline_enable",
        &artifact_path,
        started.elapsed().as_millis(),
    )?);

    execute(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    )?;
    let after_enable = resource_snapshot(
        &store,
        "timeline_enabled",
        &artifact_path,
        started.elapsed().as_millis(),
    )?;
    if after_enable["housing_paid_wage_window_owned_bytes"]
        != json!(EXPECTED_WAGE_WINDOW_OWNED_BYTES)
    {
        return Err(format!(
            "unexpected paid-wage-window allocation after Enable: {after_enable:?}"
        ));
    }
    resource_samples.push(after_enable);
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: T::AddPayroll {
                spec: PayrollSpec {
                    id: PAYROLL,
                    payer: EMPLOYER,
                    payee: TENANT,
                    home_city: 0,
                    hourly_cents: money(HOURLY_CENTS),
                    cycle: Cycle::Monthly { anchor: 1 },
                    ends_at: 150 * DAY,
                },
            },
        },
    )?;
    resource_samples.push(resource_snapshot(
        &store,
        "payroll_registered_before_simulation",
        &artifact_path,
        started.elapsed().as_millis(),
    )?);

    advance_to(&mut store, DAY_31)?;
    let boundary = wage_window(&store)?;
    if boundary.status != PaidWageWindowStatus::Complete
        || (boundary.start_day, boundary.end_day) != (Some(0), Some(31))
        || boundary.actual_paid_wages != Money::ZERO
        || boundary.qualifying_contracts != 1
        || boundary.active_contracts != 1
    {
        return Err(format!(
            "day-31 half-open wage window mismatch: {boundary:?}"
        ));
    }

    advance_to(&mut store, DAY_32)?;
    let current = wage_window(&store)?;
    if current.status != PaidWageWindowStatus::Complete
        || (current.start_day, current.end_day) != (Some(1), Some(32))
        || current.actual_paid_wages != money(PAID_AT_DAY_31_CENTS)
        || current.qualifying_contracts != 1
        || current.active_contracts != 1
    {
        return Err(format!("day-32 complete wage window mismatch: {current:?}"));
    }

    let state_hash = store
        .state_hash()
        .map_err(|error| format!("state hash: {error:?}"))?;
    store
        .checkpoint()
        .map_err(|error| format!("checkpoint: {error:?}"))?;
    drop(store);
    let store = Store::open(&path, RULES).map_err(|error| format!("cold open: {error:?}"))?;
    let reopened = wage_window(&store)?;
    let reopened_hash = store
        .state_hash()
        .map_err(|error| format!("reopened state hash: {error:?}"))?;
    if reopened != current || reopened_hash != state_hash {
        return Err(
            "wage window or persisted Store state changed after checkpoint cold-open".into(),
        );
    }
    let cold_open_resources = resource_snapshot(
        &store,
        "cold_open_after_day_32_checkpoint",
        &artifact_path,
        started.elapsed().as_millis(),
    )?;
    if cold_open_resources["housing_paid_wage_window_owned_bytes"]
        != json!(EXPECTED_WAGE_WINDOW_OWNED_BYTES)
    {
        return Err(format!(
            "unexpected paid-wage-window allocation after cold open: {cold_open_resources:?}"
        ));
    }
    resource_samples.push(cold_open_resources);

    println!(
        "{}",
        json!({
            "scenario": "actual_store_paid_wage_window",
            "timeline_day_31": {
                "status": format!("{:?}", boundary.status),
                "window_days": [boundary.start_day, boundary.end_day],
                "paid_wages_cents": boundary.actual_paid_wages.value(),
                "payment_at_midnight_is_excluded": true
            },
            "timeline_day_32": {
                "status": format!("{:?}", current.status),
                "window_days": [current.start_day, current.end_day],
                "qualifying_contracts": current.qualifying_contracts,
                "active_contracts": current.active_contracts,
                "paid_wages_cents": current.actual_paid_wages.value()
            },
            "checkpoint_cold_open_equal": true,
            "expected_paid_wages_cents": PAID_AT_DAY_31_CENTS,
            "resource_samples": resource_samples,
            "elapsed_total_ms": started.elapsed().as_millis(),
            "artifact_path": artifact_path.display().to_string()
        })
    );
    Ok(())
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: town_housing_income <new-world-dir>");
        std::process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("provide exactly one new-world directory");
        std::process::exit(2);
    }
    if let Err(error) = run(path) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
