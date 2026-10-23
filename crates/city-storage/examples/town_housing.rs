//! Run one real Store lease across first and recurring existing-Timeline rent boundaries.
//!
//! Usage: `cargo run -p mycity-storage --example town_housing -- <new-world-dir>`.
//! This finite flow covers one immutable opening offer, first-period proration, second-period
//! accrual into the same Claim, and cold recovery at the P1/P2 boundary. Broader housing workflows
//! remain outside this bounded scenario.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, HousingState, LeaseTerms, OpeningDwelling, OpeningHousehold,
        OpeningHousing, MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
        MAX_VALIDATION_SCRATCH_BYTES,
    },
    model_rows::Rows,
    schedule::Phase,
    timeline::{Claim, Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope, Receipt},
    units::{GameTime, Money},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

const RULES: &str = "m0-storage-process-fixture-v1";
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
const RENT_CENTS: i64 = 480_000;
const FIRST_INVOICE_CENTS: i64 = 263_226;
const WEEKLY_PAY_CENTS: i64 = 44_800;

fn m(value: i64) -> Money {
    Money::new(value).expect("non-negative scenario amount")
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
        rent_per_cycle: m(RENT_CENTS),
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
    let mut offers = Rows::<GenesisLeaseOffer, MAX_GENESIS_LEASE_OFFERS>::new().unwrap();
    offers.try_push(offer).unwrap();
    OpeningHousing {
        unit_limit: 1,
        city_id: None,
        dwellings,
        households,
        lease_offers: offers,
        move_assessments: mycity_core::model_rows::Rows::new().unwrap(),
        move_candidates: mycity_core::model_rows::Rows::new().unwrap(),
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
    execute_envelope(store, caller, request).map(|_| ())
}

fn execute_envelope(
    store: &mut Store,
    caller: Caller,
    request: Envelope,
) -> Result<Receipt, String> {
    let expected_operation_id = request.operation_id;
    let outcome = store
        .execute(caller, request)
        .map_err(|e| format!("store execute: {e:?}"))?;
    let receipt = outcome
        .model
        .map_err(|e| format!("model operation: {e:?}"))?;
    if !outcome.has_persistent_receipt {
        return Err("operation did not receive a persistent receipt".into());
    }
    if receipt.operation_id != expected_operation_id {
        return Err(format!(
            "receipt operation {} differs from submitted envelope {}",
            receipt.operation_id, expected_operation_id
        ));
    }
    Ok(receipt)
}

fn drive(store: &mut Store, target: u64) -> Result<(), String> {
    drive_with_limit(store, target, 256)
}

fn drive_with_limit(store: &mut Store, target: u64, limit: u16) -> Result<(), String> {
    for _ in 0..10_000 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: T::Advance { target, limit },
            },
        )?;
        if store
            .engine()
            .map_err(|e| format!("store engine: {e:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("Timeline did not complete target {target}"))
}

fn advance_to_rent_p2(store: &mut Store, target: u64) -> Result<Envelope, String> {
    // Finish the preceding hour as a separate target. Every continuation in the due hour uses
    // the same limit=1 so the Store's fixed page-limit contract remains intact across recovery.
    let last_hour = target
        .checked_sub(3600)
        .ok_or_else(|| "rent due must be after one hour".to_string())?;
    let before = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .progress;
    if before.target.is_some() || before.last_complete >= target {
        return Err(format!(
            "unexpected preexisting Timeline cursor: {before:?}"
        ));
    }
    if before.last_complete < last_hour {
        drive(store, last_hour)?;
    }
    let staged = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .progress;
    if staged.target.is_some() || staged.last_complete < last_hour || staged.last_complete >= target
    {
        return Err(format!("previous-hour target not complete: {staged:?}"));
    }

    let mut last_operation = None;
    for _ in 0..10_000 {
        let progress = store
            .engine()
            .map_err(|e| format!("store engine: {e:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress;
        if progress.at == target
            && progress.phase == Some(Phase::P2)
            && progress.lane == 0
            && progress.cursor == 0
        {
            return last_operation.ok_or_else(|| "P1 operation receipt missing".into());
        }
        let operation = envelope(
            store,
            Action::Timeline {
                command: T::Advance { target, limit: 1 },
            },
        )?;
        let receipt = execute_envelope(store, Caller::System, operation.clone())?;
        last_operation = Some(operation);
        let current = store
            .engine()
            .map_err(|e| format!("store engine: {e:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress;
        if current.at == target
            && current.phase == Some(Phase::P2)
            && receipt.timeline != Some(current)
        {
            return Err("P1 operation receipt differs from pending P2 cursor".into());
        }
    }
    Err(format!("Timeline did not stop before P2 at {target}"))
}

fn cash(store: &Store, account: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .map(|row| row.cash.value())
        .ok_or_else(|| format!("account {account} missing"))
}

fn total_cash(store: &Store) -> Result<i64, String> {
    Ok(store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .map(|row| row.cash.value())
        .sum())
}

fn claim(store: &Store) -> Result<Claim, String> {
    store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .claims
        .iter()
        .find(|row| row.id == u64::from(LEASE))
        .copied()
        .ok_or_else(|| "rent Claim missing".to_string())
}

fn run(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "refusing to reuse existing directory: {}",
            path.display()
        ));
    }
    fs::create_dir_all(path.parent().ok_or("world path must have a parent")?)
        .map_err(|e| format!("create parent: {e}"))?;

    let housing = opening_housing();
    let opening_owned_capacity_bytes = housing.owned_capacity_bytes();
    let initial_state_owned_capacity_bytes = HousingState::from_opening(&housing)
        .map_err(|e| format!("housing state init: {e:?}"))?
        .owned_capacity_bytes();
    let mut opening: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
            .map_err(|e| format!("opening fixture: {e}"))?;
    opening["accounts"][1]["cash"] = json!(0);
    opening["sources"][1]["amount"] = json!(0);
    opening["housing"] =
        serde_json::to_value(housing).map_err(|e| format!("housing opening encode: {e}"))?;

    let mut store = Store::create(&path, &serde_json::to_vec(&opening).unwrap(), vec![], RULES)
        .map_err(|e| format!("store create: {e:?}"))?;
    if total_cash(&store)? != 2_800_000 {
        return Err("opening household/payroll fixture cash does not reconcile".into());
    }
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
                    home_city: 0,
                    hourly_cents: m(800),
                    cycle: Cycle::Weekly,
                    ends_at: 150 * DAY,
                },
            },
        },
    )?;
    drive(&mut store, ACCEPT_AT)?;
    if cash(&store, TENANT)? != 2 * WEEKLY_PAY_CENTS {
        return Err("expected two actual weekly salary payments before lease acceptance".into());
    }

    let offer = opening_housing().lease_offers[0];
    let offer_sha = genesis_lease_offer_sha256(offer);
    let bad_tenant = envelope(
        &store,
        Action::HousingLease {
            offer_id: OFFER,
            offer_sha256: offer_sha,
        },
    )?;
    let bad_tenant = store
        .execute(Caller::Account(EMPLOYER), bad_tenant)
        .map_err(|e| format!("wrong-tenant operation: {e:?}"))?;
    if bad_tenant.model.is_ok() {
        return Err("wrong tenant unexpectedly accepted immutable offer".into());
    }
    let mut wrong_hash = offer_sha;
    wrong_hash[0] ^= 1;
    let wrong_hash = envelope(
        &store,
        Action::HousingLease {
            offer_id: OFFER,
            offer_sha256: wrong_hash,
        },
    )?;
    let wrong_hash = store
        .execute(Caller::Account(TENANT), wrong_hash)
        .map_err(|e| format!("altered-hash operation: {e:?}"))?;
    if wrong_hash.model.is_ok() {
        return Err("altered offer SHA unexpectedly accepted".into());
    }

    let precommit = envelope(
        &store,
        Action::HousingLease {
            offer_id: OFFER,
            offer_sha256: offer_sha,
        },
    )?;
    let sequence_before_precommit = store.committed_sequence();
    let before_precommit = store
        .business_digest()
        .map_err(|e| format!("business digest: {e:?}"))?;
    if store
        .execute_with_observer(Caller::Account(TENANT), precommit.clone(), |at| {
            if at == WritePoint::Prepared {
                Err(std::io::Error::other("injected precommit failure"))
            } else {
                Ok(())
            }
        })
        .is_ok()
    {
        return Err("injected precommit failure was not observed".into());
    }
    drop(store);

    // Prepared is before WAL append, but the observer error poisons this Store instance. Reopen
    // the same world, prove there is no rent Claim or occupied household, then retry the exact
    // envelope and operation ID.
    let mut store = Store::open(&path, RULES).map_err(|e| format!("precommit recovery: {e:?}"))?;
    if store.committed_sequence() != sequence_before_precommit {
        return Err("Prepared failure advanced the committed sequence".into());
    }
    if store
        .business_digest()
        .map_err(|e| format!("business digest: {e:?}"))?
        != before_precommit
    {
        return Err("precommit failure changed business state".into());
    }
    if store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .claims
        .iter()
        .any(|row| row.id == u64::from(LEASE))
    {
        return Err("Prepared failure left a rent Claim".into());
    }
    let checkpoint = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .export_checkpoint()
        .map_err(|e| format!("checkpoint snapshot: {e}"))?;
    let snapshot: Value =
        serde_json::from_slice(&checkpoint).map_err(|e| format!("checkpoint JSON: {e}"))?;
    if snapshot["world"]["housing"]["state"]["units"][0]["household_id"] != 0 {
        return Err("Prepared failure left the dwelling occupied".into());
    }
    let lease = store
        .execute(Caller::Account(TENANT), precommit)
        .map_err(|e| format!("lease retry: {e:?}"))?;
    lease.model.map_err(|e| format!("housing lease: {e:?}"))?;
    if !lease.has_persistent_receipt {
        return Err("housing lease receipt missing".into());
    }

    let registered = claim(&store)?;
    if registered.due != FIRST_DUE
        || registered.principal.value() != FIRST_INVOICE_CENTS
        || registered.paid != Money::ZERO
        || registered.processed
    {
        return Err(format!("first future rent Claim mismatch: {registered:?}"));
    }
    let checkpoint = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .export_checkpoint()
        .map_err(|e| format!("checkpoint snapshot: {e}"))?;
    let view: Value =
        serde_json::from_slice(&checkpoint).map_err(|e| format!("checkpoint JSON: {e}"))?;
    let housing = &view["world"]["housing"];
    if housing["state"]["units"].as_array().map_or(0, Vec::len) != 1
        || housing["state"]["units"][0]["household_id"] != HOUSEHOLD
        || housing["state"]["consumed_offer_ids"][0] != OFFER
    {
        return Err("accepted offer did not produce one persistent unique occupancy".into());
    }

    let rejected_replay = envelope(
        &store,
        Action::HousingLease {
            offer_id: OFFER,
            offer_sha256: offer_sha,
        },
    )?;
    let rejected_replay = store
        .execute(Caller::Account(TENANT), rejected_replay)
        .map_err(|e| format!("consumed-offer replay: {e:?}"))?;
    if rejected_replay.model.is_ok() {
        return Err("consumed genesis offer was accepted a second time".into());
    }

    store
        .checkpoint()
        .map_err(|e| format!("checkpoint: {e:?}"))?;
    drop(store);
    let mut store = Store::open(&path, RULES).map_err(|e| format!("cold open: {e:?}"))?;
    if claim(&store)? != registered {
        return Err("cold open changed the future rent Claim".into());
    }

    let due = envelope(
        &store,
        Action::Timeline {
            command: T::Advance {
                target: FIRST_DUE,
                limit: 256,
            },
        },
    )?;
    let due_operation = due.operation_id;
    if store
        .execute_with_observer(Caller::System, due.clone(), |at| {
            if at == WritePoint::Applied {
                Err(std::io::Error::other("injected post-commit lost reply"))
            } else {
                Ok(())
            }
        })
        .is_ok()
    {
        return Err("Applied lost-reply injection was not observed".into());
    }
    drop(store);
    let mut store =
        Store::open(&path, RULES).map_err(|e| format!("reopen after Applied: {e:?}"))?;
    if store.committed_sequence() != due_operation {
        return Err("Applied Advance receipt sequence did not survive cold open".into());
    }
    let partial_progress = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .progress;
    if partial_progress.target != Some(FIRST_DUE) || partial_progress.last_complete >= FIRST_DUE {
        return Err(format!(
            "expected persisted partial Timeline cursor, got {partial_progress:?}"
        ));
    }
    let partial_hash = store
        .state_hash()
        .map_err(|e| format!("state hash: {e:?}"))?;
    let replay = store
        .execute(Caller::System, due.clone())
        .map_err(|e| format!("partial lost-reply replay: {e:?}"))?;
    let replay_receipt = replay
        .model
        .map_err(|e| format!("partial replay model: {e:?}"))?;
    if !replay.has_persistent_receipt
        || replay_receipt.operation_id != due_operation
        || replay_receipt.timeline != Some(partial_progress)
        || store
            .state_hash()
            .map_err(|e| format!("state hash: {e:?}"))?
            != partial_hash
        || store
            .engine()
            .map_err(|e| format!("store engine: {e:?}"))?
            .timeline()
            .ok_or_else(|| "Timeline unavailable".to_string())?
            .progress
            != partial_progress
    {
        return Err("replaying the original partial Advance changed the cursor/state".into());
    }

    // Resume in new 256-step operations until the same target finishes; never enlarge a slice
    // to force an entire multi-day interval into one operation.
    drive(&mut store, FIRST_DUE)?;
    let completed = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .progress;
    if completed.target.is_some() || completed.last_complete != FIRST_DUE {
        return Err(format!(
            "Timeline did not reach exact due boundary: {completed:?}"
        ));
    }
    let paid = claim(&store)?;
    let arrears = paid.principal.value() - paid.paid.value();
    if paid.principal.value() != FIRST_INVOICE_CENTS
        || paid.paid.value() != 4 * WEEKLY_PAY_CENTS
        || arrears != 84_026
        || paid.due != FIRST_DUE
        || !paid.processed
    {
        return Err(format!("P2 rent settlement mismatch: {paid:?}"));
    }
    if cash(&store, TENANT)? != 0
        || cash(&store, EMPLOYER)? != 600_000 - 4 * WEEKLY_PAY_CENTS
        || cash(&store, LANDLORD)? != 1_200_000 + 4 * WEEKLY_PAY_CENTS
        || total_cash(&store)? != 2_800_000
    {
        return Err("rent settlement did not conserve the expected cash transfers".into());
    }
    let hash_after_application = store
        .state_hash()
        .map_err(|e| format!("state hash: {e:?}"))?;
    let replay = store
        .execute(Caller::System, due)
        .map_err(|e| format!("lost-reply replay: {e:?}"))?;
    let replay_receipt = replay.model.map_err(|e| format!("replay model: {e:?}"))?;
    if !replay.has_persistent_receipt
        || replay_receipt.operation_id != due_operation
        || replay_receipt.timeline != Some(partial_progress)
        || store
            .state_hash()
            .map_err(|e| format!("state hash: {e:?}"))?
            != hash_after_application
        || claim(&store)? != paid
    {
        return Err("replayed P2 operation changed committed rent state".into());
    }

    // The second Gregorian occurrence must add one invoice to the same Claim in P1. Persist and
    // cold-open at the P1/P2 seam, before the second-period payroll cash is collected.
    let second_p1 = advance_to_rent_p2(&mut store, SECOND_DUE)?;
    let pending = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .progress;
    if pending.at != SECOND_DUE
        || pending.target != Some(SECOND_DUE)
        || pending.phase != Some(Phase::P2)
        || pending.lane != 0
        || pending.cursor != 0
    {
        return Err(format!(
            "expected P1-complete/P2-pending cursor, got {pending:?}"
        ));
    }
    let accrued = claim(&store)?;
    if accrued.principal.value() != FIRST_INVOICE_CENTS + RENT_CENTS
        || accrued.paid.value() != 4 * WEEKLY_PAY_CENTS
        || accrued.principal.value() - accrued.paid.value() != 564_026
        || accrued.due != FIRST_DUE
        || accrued.processed
        || cash(&store, TENANT)? != 4 * WEEKLY_PAY_CENTS
    {
        return Err(format!("second P1 rent accrual mismatch: {accrued:?}"));
    }
    let timeline = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?;
    let rent_sources = timeline
        .claim_sources
        .iter()
        .filter(|row| row.claim_id == u64::from(LEASE))
        .count();
    if rent_sources != 1 {
        return Err(format!(
            "expected one reused rent source, got {rent_sources}"
        ));
    }
    let housing_snapshot = store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .export_checkpoint()
        .map_err(|e| format!("housing snapshot: {e}"))?;
    let housing_snapshot: Value =
        serde_json::from_slice(&housing_snapshot).map_err(|e| format!("checkpoint JSON: {e}"))?;
    let unit = &housing_snapshot["world"]["housing"]["state"]["units"][0];
    if unit["last_accounted_at"] != SECOND_DUE
        || unit["next_due_at"] != 90 * DAY
        || unit["arrears_ref"] != LEASE
    {
        return Err(format!("housing occurrence cursor mismatch: {unit}"));
    }
    store
        .checkpoint()
        .map_err(|e| format!("second P1 checkpoint: {e:?}"))?;
    drop(store);
    let mut store =
        Store::open(&path, RULES).map_err(|e| format!("cold open before second P2: {e:?}"))?;
    if store
        .engine()
        .map_err(|e| format!("store engine: {e:?}"))?
        .timeline()
        .ok_or_else(|| "Timeline unavailable".to_string())?
        .progress
        != pending
        || claim(&store)? != accrued
        || cash(&store, TENANT)? != 4 * WEEKLY_PAY_CENTS
    {
        return Err("cold open changed the pending second P2 rent state".into());
    }
    let p1_hash = store
        .state_hash()
        .map_err(|e| format!("P1 state hash: {e:?}"))?;
    let replay = store
        .execute(Caller::System, second_p1.clone())
        .map_err(|e| format!("second P1 replay: {e:?}"))?;
    let replay_receipt = replay
        .model
        .map_err(|e| format!("second P1 replay model: {e:?}"))?;
    if !replay.has_persistent_receipt
        || replay_receipt.operation_id != second_p1.operation_id
        || replay_receipt.timeline != Some(pending)
        || store
            .state_hash()
            .map_err(|e| format!("P1 replay hash: {e:?}"))?
            != p1_hash
        || claim(&store)? != accrued
    {
        return Err("replayed second P1 changed the accrued claim".into());
    }

    drive_with_limit(&mut store, SECOND_DUE, 1)?;
    let recurring = claim(&store)?;
    let recurring_arrears = recurring.principal.value() - recurring.paid.value();
    if recurring.principal.value() != FIRST_INVOICE_CENTS + RENT_CENTS
        || recurring.paid.value() != 8 * WEEKLY_PAY_CENTS
        || recurring_arrears != 384_826
        || recurring.due != FIRST_DUE
        || !recurring.processed
        || cash(&store, TENANT)? != 0
        || cash(&store, EMPLOYER)? != 600_000 - 8 * WEEKLY_PAY_CENTS
        || cash(&store, LANDLORD)? != 1_200_000 + 8 * WEEKLY_PAY_CENTS
        || total_cash(&store)? != 2_800_000
    {
        return Err(format!("second P2 rent settlement mismatch: {recurring:?}"));
    }
    let after_recurring_p2 = store
        .state_hash()
        .map_err(|e| format!("second P2 state hash: {e:?}"))?;
    let replay = store
        .execute(Caller::System, second_p1)
        .map_err(|e| format!("settled second P1 replay: {e:?}"))?;
    if !replay.has_persistent_receipt
        || store
            .state_hash()
            .map_err(|e| format!("settled replay hash: {e:?}"))?
            != after_recurring_p2
        || claim(&store)? != recurring
    {
        return Err("late replay double-billed or repaid the second occurrence".into());
    }

    println!(
        "{}",
        json!({
            "scenario": "housing-genesis-lease-first-and-recurring-rent",
            "actual_store_and_timeline": true,
            "configured_units": 1,
            "household_id": HOUSEHOLD,
            "offer_consumed_once": true,
            "wrong_tenant_rejected": true,
            "altered_offer_sha_rejected": true,
            "precommit_rollback_and_retry": true,
            "rent_monthly_cents": RENT_CENTS,
            "first_invoice_cents": FIRST_INVOICE_CENTS,
            "occupied_days": 17,
            "calendar_interval_days": 31,
            "registered_due_seconds": FIRST_DUE,
            "salary_paid_before_due_cents": 2 * WEEKLY_PAY_CENTS,
            "salary_paid_by_due_cents": 4 * WEEKLY_PAY_CENTS,
            "rent_paid_at_p2_cents": paid.paid.value(),
            "arrears_cents": arrears,
            "earliest_due_seconds": paid.due,
            "checkpoint_cold_open": true,
            "applied_lost_reply_replayed_once": true,
            "monthly_recurrence": {
                "second_invoice_cents": RENT_CENTS,
                "p1_principal_cents": FIRST_INVOICE_CENTS + RENT_CENTS,
                "p1_paid_cents": 4 * WEEKLY_PAY_CENTS,
                "p1_earliest_due_seconds": FIRST_DUE,
                "p1_pending_p2_checkpoint_cold_open_replay": true,
                "p2_cumulative_paid_cents": 8 * WEEKLY_PAY_CENTS,
                "p2_arrears_cents": recurring_arrears,
                "p2_reuses_single_claim_and_rent_source": true,
                "same_occurrence_replay_no_double_charge": true
            },
            "housing_opening_owned_capacity_bytes": opening_owned_capacity_bytes,
            "housing_state_owned_capacity_bytes": initial_state_owned_capacity_bytes,
            "housing_validation_scratch_budget_bytes": MAX_VALIDATION_SCRATCH_BYTES
        })
    );
    Ok(())
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: town_housing <new-world-dir>");
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
