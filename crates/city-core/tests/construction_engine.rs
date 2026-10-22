use mycity_core::{
    assembly::assemble,
    construction::{
        EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material, Phase, Project,
        OPERATION_PREFIX, STATE_BUDGET_BYTES,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::{GameTime, Money, Quantity},
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const BUILD_SECONDS: u64 = 28_800;
const END_OF_SHIFT: u64 = 17 * 3_600;
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1_000_000);

fn opening(required: i64, budget: i64) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(INPUT).unwrap();
    let profile = Project::new(
        7,
        Kind::Build,
        4,
        3,
        1,
        80,
        1,
        1,
        Money::new(budget).unwrap(),
        Money::new(3_600).unwrap(),
        BUILD_SECONDS,
        &[
            Gate {
                id: 1,
                category: GateCategory::OwnerConsent,
                evidence_id: 0,
                party: 2,
                decided: GateDecision::Pending,
            },
            Gate {
                id: 2,
                category: GateCategory::Permit,
                evidence_id: 0,
                party: 1,
                decided: GateDecision::Pending,
            },
        ],
        &[Material {
            stock_id: 1,
            required: Quantity::new(required).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(400).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    value["construction"] = serde_json::to_value(profile).unwrap();
    serde_json::to_vec(&value).unwrap()
}
fn engine(required: i64, budget: i64, site_capacity: u32) -> Engine {
    Engine::new(
        assemble(&opening(required, budget)).unwrap(),
        vec![Resource::new(80, site_capacity).unwrap()],
    )
    .unwrap()
}
fn envelope(e: &Engine, operation_id: u64, action: Action) -> Envelope {
    let view = e.world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![action],
    }
}
fn ordinary_id(_: &Engine) -> u64 {
    NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed)
}
fn run(e: &mut Engine, caller: Caller, action: Action) {
    let id = ordinary_id(e);
    let request = envelope(e, id, action);
    e.execute(caller, request).unwrap();
}
fn construction(
    e: &mut Engine,
    caller: Caller,
    command: C,
    operation_id: u64,
) -> Result<(), TxError> {
    let request = envelope(e, operation_id, Action::Construction { command });
    e.execute(caller, request).map(|_| ())
}
fn construction_auto(e: &mut Engine, caller: Caller, command: C) -> Result<(), TxError> {
    let id = ordinary_id(e);
    construction(e, caller, command, id)
}
fn effect_id(e: &Engine) -> u64 {
    let p = e.world().view().construction.unwrap();
    OPERATION_PREFIX | (p.id << 32) | u64::from(p.next_effect)
}
fn file_and_approve(e: &mut Engine) {
    construction_auto(e, Caller::Account(4), C::File).unwrap();
    construction_auto(
        e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 11,
        },
    )
    .unwrap();
    construction_auto(
        e,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 12,
        },
    )
    .unwrap();
    construction_auto(e, Caller::Account(4), C::Prepare).unwrap();
}
fn start(e: &mut Engine) -> Result<(), TxError> {
    let id = effect_id(e);
    construction(e, Caller::Account(4), C::Start, id)
}
fn enable_timeline(e: &mut Engine) {
    run(e, Caller::System, Action::Timeline { command: T::Enable });
}
fn advance(e: &mut Engine, target: u64) {
    for _ in 0..100 {
        run(
            e,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        if e.timeline().unwrap().progress.target.is_none() {
            return;
        }
    }
    panic!("timeline failed to reach construction work time");
}
fn work(e: &mut Engine) -> Result<(), TxError> {
    let id = effect_id(e);
    construction(
        e,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
            elapsed_seconds: END_OF_SHIFT as u32,
            worker_seconds: BUILD_SECONDS as u32,
        },
        id,
    )
}
fn cash(e: &Engine, account: u32) -> i64 {
    e.world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == account)
        .unwrap()
        .cash
        .value()
}
fn stock(e: &Engine, id: u32) -> (i64, i64) {
    let s = e
        .world()
        .view()
        .stocks
        .into_iter()
        .find(|s| s.id == id)
        .unwrap();
    (s.quantity.value(), s.basis.value())
}
fn resource_state(e: &Engine) -> Vec<(u32, u32, u32, u32)> {
    e.resources()
        .iter()
        .map(|r| (r.id, r.capacity, r.reserved, r.occupied))
        .collect()
}
type LeaseState = Vec<Option<(u64, u32, u32, u32, bool)>>;
fn lease_state(e: &Engine) -> LeaseState {
    e.leases()
        .iter()
        .map(|l| l.map(|l| (l.token, l.actor, l.resource, l.units, l.occupied)))
        .collect()
}

#[test]
fn insufficient_site_capacity_funds_and_materials_leave_candidate_domains_unchanged() {
    let mut no_site = engine(1_000, 1_000_000, 1);
    file_and_approve(&mut no_site);
    run(
        &mut no_site,
        Caller::Account(2),
        Action::ReserveCapacity {
            resource: 80,
            units: 1,
        },
    );
    let before_hash = no_site.state_hash();
    let before_view = no_site.world().view();
    let before_resources = resource_state(&no_site);
    let before_leases = lease_state(&no_site);
    assert_eq!(start(&mut no_site), Err(TxError::Capacity));
    assert_eq!(no_site.state_hash(), before_hash);
    assert_eq!(no_site.world().view(), before_view);
    assert_eq!(resource_state(&no_site), before_resources);
    assert_eq!(lease_state(&no_site), before_leases);

    let mut no_cash = engine(1_000, 1_000_000, 1);
    file_and_approve(&mut no_cash);
    start(&mut no_cash).unwrap();
    enable_timeline(&mut no_cash);
    advance(&mut no_cash, END_OF_SHIFT);
    let owner = no_cash.world().account_ref(4).unwrap();
    run(
        &mut no_cash,
        Caller::System,
        Action::Protect {
            account: owner,
            amount: Money::new(1_200_000).unwrap(),
        },
    );
    let before_hash = no_cash.state_hash();
    let before_view = no_cash.world().view();
    let before_resources = resource_state(&no_cash);
    let before_leases = lease_state(&no_cash);
    assert_eq!(
        work(&mut no_cash),
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert_eq!(no_cash.state_hash(), before_hash);
    assert_eq!(no_cash.world().view(), before_view);
    assert_eq!(resource_state(&no_cash), before_resources);
    assert_eq!(lease_state(&no_cash), before_leases);

    let mut no_material = engine(700_000, 1_000_000, 1);
    file_and_approve(&mut no_material);
    start(&mut no_material).unwrap();
    enable_timeline(&mut no_material);
    advance(&mut no_material, END_OF_SHIFT);
    let before_hash = no_material.state_hash();
    let before_view = no_material.world().view();
    let before_resources = resource_state(&no_material);
    let before_leases = lease_state(&no_material);
    let before_cash = cash(&no_material, 4);
    let before_stock = stock(&no_material, 1);
    assert_eq!(
        work(&mut no_material),
        Err(TxError::Construction(
            mycity_core::construction::Error::Materials
        ))
    );
    assert_eq!(no_material.state_hash(), before_hash);
    assert_eq!(no_material.world().view(), before_view);
    assert_eq!(resource_state(&no_material), before_resources);
    assert_eq!(lease_state(&no_material), before_leases);
    assert_eq!(cash(&no_material, 4), before_cash);
    assert_eq!(stock(&no_material, 1), before_stock);
}

#[test]
fn durable_hook_failure_does_not_publish_and_retry_after_commit_returns_original_receipt() {
    let mut e = engine(1_000, 1_000_000, 1);
    file_and_approve(&mut e);
    let env = envelope(
        &e,
        effect_id(&e),
        Action::Construction { command: C::Start },
    );
    let before = e.export_checkpoint().unwrap();
    let before_hash = e.state_hash();
    let failed = e.execute_recorded(Caller::Account(4), env.clone(), |_, _| Err::<(), _>("disk"));
    assert_eq!(failed, Err("disk"));
    assert_eq!(e.export_checkpoint().unwrap(), before);
    assert_eq!(e.state_hash(), before_hash);
    assert!(e.leases().iter().all(Option::is_none));

    // Model a durable commit whose response is lost: keep the exact request and retry it.
    let _discarded_response = e
        .execute_recorded(Caller::Account(4), env.clone(), |_, _| Ok::<(), ()>(()))
        .unwrap()
        .unwrap();
    let after = e.export_checkpoint().unwrap();
    let post_hash = e.state_hash();
    let record_count = e.journal().len();
    let original_receipt = e.journal().last().unwrap().outcome.unwrap();
    let occupied = e.resources().iter().find(|r| r.id == 80).unwrap().occupied;
    let site_token = e.world().view().construction.unwrap().site_token.unwrap();
    let lease = e
        .leases()
        .iter()
        .flatten()
        .find(|l| l.token == site_token)
        .unwrap();
    assert_eq!(
        (lease.actor, lease.resource, lease.units, lease.occupied),
        (4, 80, 1, true)
    );
    let replay = e.execute_recorded(Caller::Account(4), env, |_, _| -> Result<(), ()> {
        panic!("hot duplicate must not persist again")
    });
    assert_eq!(replay.unwrap().unwrap(), original_receipt);
    assert_eq!(e.journal().len(), record_count);
    assert_eq!(e.export_checkpoint().unwrap(), after);
    assert_eq!(e.state_hash(), post_hash);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 80).unwrap().occupied,
        occupied
    );
}

#[test]
fn non_draft_checkpoint_restores_exact_site_lease_and_continues_to_operational_without_double_charge(
) {
    let mut e = engine(1_000, 1_000_000, 1);
    file_and_approve(&mut e);
    start(&mut e).unwrap();
    let after_start_cash = cash(&e, 4);
    let after_start_stock = stock(&e, 1);
    let checkpoint = e.export_checkpoint().unwrap();
    let mut forged: Value = serde_json::from_slice(&checkpoint).unwrap();
    forged["world"]["construction"]["site_token"] = json!(u64::MAX);
    assert!(Engine::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()).is_err());
    let mut e = Engine::restore_checkpoint(&checkpoint).unwrap();
    let p = e.world().view().construction.unwrap();
    assert_eq!(p.phase, Phase::Building);
    let token = p.site_token.unwrap();
    assert!(e.leases().iter().flatten().any(|l| l.token == token
        && l.actor == p.owner
        && l.resource == p.site_resource
        && l.units == p.site_units
        && l.occupied));
    assert_eq!(cash(&e, 4), after_start_cash);
    assert_eq!(stock(&e, 1), after_start_stock);

    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);
    work(&mut e).unwrap();
    let after_work_cash = cash(&e, 4);
    let after_work_stock = stock(&e, 1);
    assert_eq!(after_work_cash, after_start_cash - 28_800);
    assert_eq!(after_work_stock, (599_000, 239_600));
    assert!(after_work_cash < after_start_cash);
    assert!(after_work_stock.0 < after_start_stock.0);
    let inspect_id = effect_id(&e);
    construction(&mut e, Caller::Account(3), C::SubmitInspection, inspect_id).unwrap();
    construction_auto(
        &mut e,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
    )
    .unwrap();
    let view = e.world().view();
    assert_eq!(view.construction.unwrap().phase, Phase::Operational);
    assert_eq!(cash(&e, 4), after_work_cash);
    assert_eq!(stock(&e, 1), after_work_stock);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 80).unwrap().occupied,
        1
    );
}

#[test]
fn forged_gate_or_inspector_identity_is_rejected_before_domain_mutation() {
    let mut e = engine(1_000, 1_000_000, 1);
    construction_auto(&mut e, Caller::Account(4), C::File).unwrap();
    let before = e.world().view();
    assert_eq!(
        construction_auto(
            &mut e,
            Caller::Account(3),
            C::DecideGate {
                gate_id: 1,
                decision: GateDecision::Accepted,
                evidence_id: 7
            }
        ),
        Err(TxError::Unauthorized)
    );
    assert_eq!(e.world().view(), before);
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 7,
        },
    )
    .unwrap();
    construction_auto(
        &mut e,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 8,
        },
    )
    .unwrap();
    construction_auto(&mut e, Caller::Account(4), C::Prepare).unwrap();
    start(&mut e).unwrap();
    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);
    work(&mut e).unwrap();
    let inspect_id = effect_id(&e);
    construction(&mut e, Caller::Account(3), C::SubmitInspection, inspect_id).unwrap();
    let before = e.world().view();
    assert_eq!(
        construction_auto(
            &mut e,
            Caller::Account(2),
            C::AcceptInspection {
                inspector: 2,
                accepted: true
            }
        ),
        Err(TxError::Unauthorized)
    );
    assert_eq!(e.world().view(), before);
}

#[test]
fn construction_action_and_journal_records_stay_inside_frozen_engine_budget() {
    use mycity_core::transaction::{Action, JournalRecord, ENGINE_BYTE_BUDGET};
    use std::mem::size_of;
    let e = engine(1_000, 1_000_000, 1);
    assert!(
        size_of::<Action>() <= 128,
        "Action grew beyond compact journal envelope"
    );
    assert!(
        size_of::<JournalRecord>() <= 4096,
        "journal receipt footprint unexpectedly expanded"
    );
    assert!(size_of::<Project>() <= STATE_BUDGET_BYTES);
    assert!(e.owned_capacity_bytes() <= ENGINE_BYTE_BUDGET);
    assert_eq!(ENGINE_BYTE_BUDGET, 4 * 1024 * 1024);
}
