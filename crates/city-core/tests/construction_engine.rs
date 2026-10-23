use mycity_core::{
    assembly::assemble,
    construction::{
        AbortClearanceMaterialRule, AbortClearanceSpec, ConstructionCompensation,
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
    opening_with_basis(required, budget, 400, None)
}
fn opening_with_basis(
    required: i64,
    budget: i64,
    estimated_basis_per_unit: i64,
    actual_stock_basis: Option<i64>,
) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(INPUT).unwrap();
    let mut profile = Project::new(
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
            basis_per_unit: Money::new(estimated_basis_per_unit).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    profile.operating_resource = 81;
    profile.operating_base_capacity = 2;
    profile.operating_units = 2;
    profile.waste_receiver_account = 3;
    profile.waste_receiver_resource = 82;
    profile.waste_units = 2;
    profile.waste_fee = Money::new(250).unwrap();
    value["construction"] = serde_json::to_value(profile).unwrap();
    if let Some(actual_basis) = actual_stock_basis {
        value["stocks"][0]["basis"] = json!(actual_basis);
        value["sources"][4]["amount"] = json!(actual_basis);
    }
    serde_json::to_vec(&value).unwrap()
}
fn engine(required: i64, budget: i64, site_capacity: u32) -> Engine {
    Engine::new(
        assemble(&opening(required, budget)).unwrap(),
        vec![
            Resource::new(80, site_capacity).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
    )
    .unwrap()
}
fn compensation_engine(amount: i64, budget: i64) -> Engine {
    let mut value: Value = serde_json::from_slice(&opening(1_000, budget)).unwrap();
    let mut project: Project = serde_json::from_value(value["construction"].clone()).unwrap();
    project.gates[2] = Gate {
        id: 3,
        category: GateCategory::Compensation,
        evidence_id: 0,
        party: 2,
        decided: GateDecision::Pending,
    };
    project.gate_count = 3;
    project.compensation =
        Some(ConstructionCompensation::new(3, 2, Money::new(amount).unwrap()).unwrap());
    value["construction"] = serde_json::to_value(project).unwrap();
    Engine::new(
        assemble(&serde_json::to_vec(&value).unwrap()).unwrap(),
        vec![
            Resource::new(80, 10).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
    )
    .unwrap()
}
fn compensated_operational_engine() -> Engine {
    let mut e = compensation_engine(10_000, 1_000_000);
    construction_auto(&mut e, Caller::Account(4), C::File).unwrap();
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 11,
        },
    )
    .unwrap();
    construction_auto(
        &mut e,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 12,
        },
    )
    .unwrap();
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 3,
            decision: GateDecision::Accepted,
            evidence_id: 13,
        },
    )
    .unwrap();
    construction_auto(&mut e, Caller::Account(4), C::Prepare).unwrap();
    start(&mut e).unwrap();
    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);
    work(&mut e).unwrap();
    let submit = effect_id(&e);
    construction(&mut e, Caller::Account(3), C::SubmitInspection, submit).unwrap();
    let accept = effect_id(&e);
    construction(
        &mut e,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
        accept,
    )
    .unwrap();
    assert_eq!(
        e.world().view().construction.unwrap().phase,
        Phase::Operational
    );
    e
}
fn priced_engine(
    required: i64,
    budget: i64,
    site_capacity: u32,
    estimated_basis_per_unit: i64,
    actual_stock_basis: Option<i64>,
) -> Engine {
    Engine::new(
        assemble(&opening_with_basis(
            required,
            budget,
            estimated_basis_per_unit,
            actual_stock_basis,
        ))
        .unwrap(),
        vec![
            Resource::new(80, site_capacity).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
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
fn modern_unstarted_road_can_cancel_while_legacy_rejection_is_preserved() {
    for legacy in [false, true] {
        let mut value: Value = serde_json::from_slice(&opening(1_000, 1_000_000)).unwrap();
        value["construction"]["operating_edge_id"] = json!(10);
        if legacy {
            value["construction"]
                .as_object_mut()
                .unwrap()
                .remove("execution_policy");
        }
        let mut e = Engine::new(
            assemble(&serde_json::to_vec(&value).unwrap()).unwrap(),
            vec![
                Resource::new(80, 1).unwrap(),
                Resource::new(81, 2).unwrap(),
                Resource::new(82, 10).unwrap(),
            ],
        )
        .unwrap();
        let before_hash = e.state_hash();
        let before_cash = cash(&e, 4);
        let before_stock = stock(&e, 1);
        let outcome = construction_auto(&mut e, Caller::Account(4), C::Cancel);
        if legacy {
            assert_eq!(outcome, Err(TxError::InvalidOperation));
            assert_eq!(e.state_hash(), before_hash);
        } else {
            outcome.unwrap();
            assert_eq!(
                e.world().view().construction.unwrap().phase,
                Phase::Cancelled
            );
        }
        assert_eq!(cash(&e, 4), before_cash);
        assert_eq!(stock(&e, 1), before_stock);
        assert!(e.leases().iter().all(Option::is_none));
        assert_eq!(
            e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
            2
        );
        assert_eq!(
            Engine::restore_checkpoint(&e.export_checkpoint().unwrap())
                .unwrap()
                .state_hash(),
            e.state_hash()
        );
    }
}

#[test]
fn clearance_metadata_start_is_atomic_and_rejects_a_future_restored_clock() {
    let mut value: Value = serde_json::from_slice(&opening(1_000, 1_000_000)).unwrap();
    let mut project: Project = serde_json::from_value(value["construction"].clone()).unwrap();
    let mut spec = AbortClearanceSpec {
        base_worker_seconds: 600,
        ..Default::default()
    };
    spec.material_rules[0] = AbortClearanceMaterialRule {
        stock_id: 1,
        quantity_per_waste_capacity_unit: 500,
        worker_seconds_per_thousand_quantity: 3_600,
    };
    project.abort_clearance_spec = Some(spec);
    value["construction"] = serde_json::to_value(project).unwrap();
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&value).unwrap()).unwrap(),
        vec![
            Resource::new(80, 1).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
    )
    .unwrap();
    file_and_approve(&mut e);
    start(&mut e).unwrap();
    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);
    let id = effect_id(&e);
    construction(
        &mut e,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(END_OF_SHIFT as i64).unwrap(),
            elapsed_seconds: END_OF_SHIFT as u32,
            worker_seconds: (BUILD_SECONDS / 2) as u32,
        },
        id,
    )
    .unwrap();
    construction_auto(&mut e, Caller::Account(3), C::Pause).unwrap();
    construction_auto(
        &mut e,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    )
    .unwrap();
    for (party, gate) in [(2, 1), (1, 2)] {
        construction_auto(
            &mut e,
            Caller::Account(party),
            C::DecideGate {
                gate_id: gate,
                decision: GateDecision::Accepted,
                evidence_id: 200 + u64::from(gate),
            },
        )
        .unwrap();
    }
    construction_auto(&mut e, Caller::Account(4), C::Prepare).unwrap();
    let before = e.export_checkpoint().unwrap();
    let before_hash = e.state_hash();
    let env = envelope(
        &e,
        ordinary_id(&e),
        Action::Construction { command: C::Start },
    );
    assert_eq!(
        e.execute_recorded(Caller::Account(4), env.clone(), |_, _| Err::<(), _>("disk")),
        Err("disk")
    );
    assert_eq!(e.export_checkpoint().unwrap(), before);
    assert_eq!(e.state_hash(), before_hash);

    let original_token = e.world().view().construction.unwrap().site_token;
    let before_cash = cash(&e, 4);
    let before_stock = stock(&e, 1);
    let receipt = e.execute(Caller::Account(4), env.clone()).unwrap();
    let after = e.export_checkpoint().unwrap();
    assert_eq!(e.execute(Caller::Account(4), env).unwrap(), receipt);
    let reopened = Engine::restore_checkpoint(&after).unwrap();
    // Engine checkpoints omit the hot retry journal. Durable lost-response
    // retries belong to Store's immutable receipt archive tests, not this API.
    assert_eq!(reopened.state_hash(), e.state_hash());
    assert_eq!(reopened.export_checkpoint().unwrap(), after);
    let clearing = reopened.world().view().construction.unwrap();
    assert_eq!(clearing.phase, Phase::Demolition);
    assert_eq!(clearing.site_token, original_token);
    assert_eq!(
        clearing
            .abort_clearance
            .unwrap()
            .original_completed_worker_seconds,
        BUILD_SECONDS / 2
    );
    assert_eq!(clearing.planned_worker_seconds, 2_400);
    assert!(!clearing.operating_published);
    assert_eq!(cash(&reopened, 4), before_cash);
    assert_eq!(stock(&reopened, 1), before_stock);

    // Keep Project's internal timestamp ordering valid while moving the whole
    // clearance into the future. The Engine's restored calendar must reject it.
    let mut forged: Value = serde_json::from_slice(&after).unwrap();
    forged["world"]["construction"]["started_at"] = json!(END_OF_SHIFT + 1);
    forged["world"]["construction"]["last_work_at"] = json!(END_OF_SHIFT + 1);
    assert!(Engine::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()).is_err());
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
    let accept_id = effect_id(&e);
    construction(
        &mut e,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
        accept_id,
    )
    .unwrap();
    let view = e.world().view();
    let operating = view.construction.unwrap();
    assert_eq!(operating.phase, Phase::Operational);
    assert!(operating.operating_published);
    assert_eq!(operating.operating_base_capacity, 2);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
        4
    );
    let checkpoint = e.export_checkpoint().unwrap();
    let mut forged: Value = serde_json::from_slice(&checkpoint).unwrap();
    forged["world"]["construction"]["operating_base_capacity"] = json!(3);
    assert!(Engine::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()).is_err());
    let mut legacy: Value = serde_json::from_slice(&checkpoint).unwrap();
    legacy["world"]["construction"]["operating_resource"] = json!(0);
    legacy["world"]["construction"]["operating_base_capacity"] = json!(0);
    legacy["world"]["construction"]["operating_units"] = json!(0);
    legacy["world"]["construction"]["operating_published"] = json!(false);
    let mut legacy = Engine::restore_checkpoint(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    let begin = envelope(
        &legacy,
        ordinary_id(&legacy),
        Action::Construction {
            command: C::BeginDemolition { waste_receiver: 3 },
        },
    );
    assert_eq!(
        legacy.execute(Caller::Account(4), begin),
        Err(TxError::InvalidOperation)
    );
    assert_eq!(
        legacy
            .resources()
            .iter()
            .find(|r| r.id == 81)
            .unwrap()
            .capacity,
        4
    );
    assert_eq!(cash(&e, 4), after_work_cash);
    assert_eq!(stock(&e, 1), after_work_stock);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 80).unwrap().occupied,
        1
    );
}

#[test]
fn retrofit_publication_and_rejected_demolition_keep_the_operating_capacity() {
    let mut value: Value = serde_json::from_slice(&opening(1_000, 1_000_000)).unwrap();
    value["construction"]["kind"] = serde_json::to_value(Kind::Retrofit).unwrap();
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&value).unwrap()).unwrap(),
        vec![
            Resource::new(80, 1).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
    )
    .unwrap();
    file_and_approve(&mut e);
    start(&mut e).unwrap();
    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);
    work(&mut e).unwrap();
    let inspect = effect_id(&e);
    construction(&mut e, Caller::Account(3), C::SubmitInspection, inspect).unwrap();
    let accept = effect_id(&e);
    construction(
        &mut e,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
        accept,
    )
    .unwrap();
    let before = e.world().view().construction.unwrap();
    assert_eq!(before.kind, Kind::Retrofit);
    assert_eq!(before.phase, Phase::Operational);
    assert!(before.operating_published);
    let owner_cash = cash(&e, 4);
    construction_auto(
        &mut e,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    )
    .unwrap();
    assert_eq!(e.world().view().construction.unwrap().phase, Phase::Review);
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Rejected,
            evidence_id: 0,
        },
    )
    .unwrap();
    let retained = e.world().view().construction.unwrap();
    assert_eq!(retained.phase, Phase::Operational);
    assert!(retained.operating_published);
    assert_eq!(retained.site_token, before.site_token);
    assert_eq!(cash(&e, 4), owner_cash);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
        4
    );
    assert!(Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).is_ok());
}

#[test]
fn operating_capacity_is_published_and_withdrawn_only_when_consumers_allow_it() {
    const NEXT_SHIFT: u64 = 86_400 + END_OF_SHIFT;
    let mut e = engine(1_000, 1_000_000, 1);
    file_and_approve(&mut e);
    start(&mut e).unwrap();
    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);
    work(&mut e).unwrap();
    let inspect = effect_id(&e);
    construction(&mut e, Caller::Account(3), C::SubmitInspection, inspect).unwrap();
    let accept = effect_id(&e);
    construction(
        &mut e,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
        accept,
    )
    .unwrap();
    let project = e.world().view().construction.unwrap();
    assert!(project.operating_published);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
        4
    );

    construction_auto(
        &mut e,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    )
    .unwrap();
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Rejected,
            evidence_id: 0,
        },
    )
    .unwrap();
    let project = e.world().view().construction.unwrap();
    assert_eq!(project.phase, Phase::Operational);
    assert!(project.operating_published);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
        4
    );

    construction_auto(
        &mut e,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    )
    .unwrap();
    let reserve_id = ordinary_id(&e);
    let reserve = envelope(
        &e,
        reserve_id,
        Action::ReserveCapacity {
            resource: 81,
            units: 2,
        },
    );
    let token = e
        .execute(Caller::Account(2), reserve)
        .unwrap()
        .created_tokens[0]
        .unwrap();
    run(&mut e, Caller::Account(2), Action::OccupyCapacity { token });
    let pending_consumer = envelope(
        &e,
        ordinary_id(&e),
        Action::ReserveCapacity {
            resource: 81,
            units: 1,
        },
    );
    let pending_token = e
        .execute(Caller::Account(2), pending_consumer)
        .unwrap()
        .created_tokens[0]
        .unwrap();
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 21,
        },
    )
    .unwrap();
    construction_auto(
        &mut e,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 22,
        },
    )
    .unwrap();
    construction_auto(&mut e, Caller::Account(4), C::Prepare).unwrap();
    assert_eq!(e.world().view().construction.unwrap().phase, Phase::Ready);
    assert!(e.world().view().construction.unwrap().operating_published);
    assert_eq!(
        construction_auto(&mut e, Caller::Account(4), C::Start),
        Err(TxError::Capacity)
    );
    assert_eq!(
        e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
        4
    );
    assert!(e.world().view().construction.unwrap().operating_published);

    run(
        &mut e,
        Caller::Account(2),
        Action::ReleaseCapacity { token },
    );
    run(
        &mut e,
        Caller::Account(2),
        Action::ReleaseCapacity {
            token: pending_token,
        },
    );
    let retry_id = ordinary_id(&e);
    construction(&mut e, Caller::Account(4), C::Start, retry_id).unwrap();
    assert_eq!(
        e.resources().iter().find(|r| r.id == 81).unwrap().capacity,
        2
    );
    assert!(!e.world().view().construction.unwrap().operating_published);
    assert_eq!(
        e.world().view().construction.unwrap().phase,
        Phase::Demolition
    );
    construction_auto(&mut e, Caller::Account(3), C::Pause).unwrap();
    assert_eq!(e.world().view().construction.unwrap().phase, Phase::Paused);
    let paused_checkpoint = e.export_checkpoint().unwrap();
    let mut e = Engine::restore_checkpoint(&paused_checkpoint).unwrap();
    assert_eq!(e.world().view().construction.unwrap().phase, Phase::Paused);
    construction_auto(&mut e, Caller::Account(3), C::Resume).unwrap();
    assert_eq!(
        e.world().view().construction.unwrap().phase,
        Phase::Demolition
    );
    advance(&mut e, NEXT_SHIFT);
    let demolish_work = effect_id(&e);
    construction(
        &mut e,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(NEXT_SHIFT as i64).unwrap(),
            elapsed_seconds: 86_400,
            worker_seconds: BUILD_SECONDS as u32,
        },
        demolish_work,
    )
    .unwrap();
    let owner = e.world().account_ref(4).unwrap();
    let protected = Money::new(cash(&e, 4) - 200).unwrap();
    run(
        &mut e,
        Caller::Account(4),
        Action::Protect {
            account: owner,
            amount: protected,
        },
    );
    let next_effect = e.world().view().construction.unwrap().next_effect;
    let owner_cash = cash(&e, 4);
    let receiver_cash = cash(&e, 3);
    let waste_occupied = e.resources().iter().find(|r| r.id == 82).unwrap().occupied;
    assert_eq!(
        construction_auto(&mut e, Caller::Account(3), C::AcceptWaste),
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert_eq!(cash(&e, 4), owner_cash);
    assert_eq!(cash(&e, 3), receiver_cash);
    assert_eq!(
        e.world().view().construction.unwrap().next_effect,
        next_effect
    );
    assert_eq!(
        e.resources().iter().find(|r| r.id == 82).unwrap().occupied,
        waste_occupied
    );
    run(
        &mut e,
        Caller::System,
        Action::ReleaseProtection {
            account: owner,
            amount: protected,
        },
    );

    let reserve = envelope(
        &e,
        ordinary_id(&e),
        Action::ReserveCapacity {
            resource: 82,
            units: 9,
        },
    );
    let receiver_lease = e
        .execute(Caller::Account(2), reserve)
        .unwrap()
        .created_tokens[0]
        .unwrap();
    run(
        &mut e,
        Caller::Account(2),
        Action::OccupyCapacity {
            token: receiver_lease,
        },
    );
    let next_effect = e.world().view().construction.unwrap().next_effect;
    let owner_cash = cash(&e, 4);
    let receiver_cash = cash(&e, 3);
    assert_eq!(
        construction_auto(&mut e, Caller::Account(3), C::AcceptWaste),
        Err(TxError::Capacity)
    );
    assert_eq!(cash(&e, 4), owner_cash);
    assert_eq!(cash(&e, 3), receiver_cash);
    assert_eq!(
        e.world().view().construction.unwrap().next_effect,
        next_effect
    );
    assert_eq!(
        e.resources().iter().find(|r| r.id == 82).unwrap().occupied,
        9
    );
    run(
        &mut e,
        Caller::Account(2),
        Action::ReleaseCapacity {
            token: receiver_lease,
        },
    );
    construction_auto(&mut e, Caller::Account(3), C::AcceptWaste).unwrap();
    assert_eq!(
        e.resources().iter().find(|r| r.id == 82).unwrap().occupied,
        2
    );
    assert_eq!(cash(&e, 4), 1_200_000 - 28_800 - 28_800 - 250);
    let close = effect_id(&e);
    construction(&mut e, Caller::Account(4), C::Close, close).unwrap();
    assert_eq!(e.world().view().construction.unwrap().phase, Phase::Closed);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 80).unwrap().occupied,
        0
    );
    assert_eq!(
        e.resources().iter().find(|r| r.id == 82).unwrap().occupied,
        2
    );
}

#[test]
fn generic_release_cannot_break_project_site_binding_and_cold_restore_can_continue_work() {
    let mut e = engine(1_000, 1_000_000, 1);
    file_and_approve(&mut e);
    start(&mut e).unwrap();
    let before_view = e.world().view();
    let before_cash = cash(&e, 4);
    let before_stocks = stock(&e, 1);
    let before_resources = resource_state(&e);
    let before_leases = lease_state(&e);
    let token = before_view
        .construction
        .unwrap()
        .site_token
        .expect("started project reserves its site");
    let request = envelope(
        &e,
        ordinary_id(&e),
        Action::ReleaseCapacity {
            token: mycity_core::transaction::CapacityToken {
                world: before_view.scope.world(),
                branch: before_view.scope.branch(),
                number: token,
            },
        },
    );
    assert_eq!(e.execute(Caller::Account(4), request), Err(TxError::Lease));
    assert_eq!(e.world().view(), before_view);
    assert_eq!(cash(&e, 4), before_cash);
    assert_eq!(stock(&e, 1), before_stocks);
    assert_eq!(resource_state(&e), before_resources);
    assert_eq!(lease_state(&e), before_leases);

    let checkpoint = e.export_checkpoint().unwrap();
    let mut reopened = Engine::restore_checkpoint(&checkpoint).unwrap();
    enable_timeline(&mut reopened);
    advance(&mut reopened, END_OF_SHIFT);
    work(&mut reopened).unwrap();
    let project = reopened.world().view().construction.unwrap();
    assert_eq!(project.completed_worker_seconds, BUILD_SECONDS);
    assert_eq!(project.phase, Phase::Building);
    assert_eq!(stock(&reopened, 1), (599_000, 239_600));
}

#[test]
fn engine_uses_actual_source_lot_basis_for_budget_and_rejects_actual_overrun_atomically() {
    let mut lower_actual = priced_engine(1_000, 30_000, 1, 100_000, None);
    file_and_approve(&mut lower_actual);
    start(&mut lower_actual).unwrap();
    enable_timeline(&mut lower_actual);
    advance(&mut lower_actual, END_OF_SHIFT);
    work(&mut lower_actual).unwrap();
    let project = lower_actual.world().view().construction.unwrap();
    assert_eq!(project.spent, Money::new(29_200).unwrap());
    assert_eq!(stock(&lower_actual, 1), (599_000, 239_600));

    let mut higher_actual = priced_engine(1_000, 30_000, 1, 1, Some(18_000_000));
    file_and_approve(&mut higher_actual);
    start(&mut higher_actual).unwrap();
    enable_timeline(&mut higher_actual);
    advance(&mut higher_actual, END_OF_SHIFT);
    let before_view = higher_actual.world().view();
    let before_hash = higher_actual.state_hash();
    let before_cash = cash(&higher_actual, 4);
    let before_payee_cash = cash(&higher_actual, 3);
    let before_stock = stock(&higher_actual, 1);
    let before_resources = resource_state(&higher_actual);
    let before_leases = lease_state(&higher_actual);
    assert_eq!(
        work(&mut higher_actual),
        Err(TxError::Construction(
            mycity_core::construction::Error::Funds
        ))
    );
    assert_eq!(higher_actual.state_hash(), before_hash);
    assert_eq!(higher_actual.world().view(), before_view);
    assert_eq!(cash(&higher_actual, 4), before_cash);
    assert_eq!(cash(&higher_actual, 3), before_payee_cash);
    assert_eq!(stock(&higher_actual, 1), before_stock);
    assert_eq!(resource_state(&higher_actual), before_resources);
    assert_eq!(lease_state(&higher_actual), before_leases);
}

#[test]
fn authoritative_work_skips_untrusted_estimate_even_when_its_sum_overflows() {
    let mut e = priced_engine(1_000, 30_000, 1, i64::MAX, None);
    file_and_approve(&mut e);
    start(&mut e).unwrap();
    enable_timeline(&mut e);
    advance(&mut e, END_OF_SHIFT);

    work(&mut e).unwrap();
    let project = e.world().view().construction.unwrap();
    assert_eq!(project.spent, Money::new(29_200).unwrap());
    assert_eq!(stock(&e, 1), (599_000, 239_600));
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

#[test]
fn compensation_payment_requires_owner_filing_and_party_acceptance_and_is_exactly_once() {
    let mut e = compensation_engine(10_000, 1_000_000);
    assert_eq!(
        construction_auto(&mut e, Caller::System, C::File),
        Err(TxError::Unauthorized)
    );
    assert_eq!(
        construction_auto(&mut e, Caller::Account(3), C::File),
        Err(TxError::Unauthorized)
    );
    construction_auto(&mut e, Caller::Account(4), C::File).unwrap();

    let decide = C::DecideGate {
        gate_id: 3,
        decision: GateDecision::Accepted,
        evidence_id: 13,
    };
    assert_eq!(
        construction_auto(&mut e, Caller::Account(3), decide),
        Err(TxError::Unauthorized)
    );
    assert_eq!(
        construction_auto(&mut e, Caller::System, decide),
        Err(TxError::Unauthorized)
    );

    let payer_before = cash(&e, 4);
    let payee_before = cash(&e, 2);
    let op = 1_234_567;
    let request = envelope(&e, op, Action::Construction { command: decide });
    e.execute(Caller::Account(2), request.clone()).unwrap();
    let after_payment = e.world().view();
    let project = after_payment.construction.unwrap();
    assert_eq!(cash(&e, 4), payer_before - 10_000);
    assert_eq!(cash(&e, 2), payee_before + 10_000);
    assert_eq!(project.spent, Money::new(10_000).unwrap());
    assert_eq!(
        project.compensation.unwrap().settled_operation_id(),
        Some(op)
    );
    assert_eq!(project.gates[2].decided, GateDecision::Accepted);

    let checkpoint = e.export_checkpoint().unwrap();
    let mut recovered = Engine::restore_checkpoint(&checkpoint).unwrap();
    // Engine-only checkpoints do not carry the Store receipt archive. The stale
    // request must remain non-mutating; Store cold replay is covered separately.
    assert_eq!(
        recovered.execute(Caller::Account(2), request),
        Err(TxError::StaleRevision)
    );
    assert_eq!(cash(&recovered, 4), payer_before - 10_000);
    assert_eq!(cash(&recovered, 2), payee_before + 10_000);

    let retry = envelope(&recovered, op + 1, Action::Construction { command: decide });
    assert_eq!(
        recovered.execute(Caller::Account(2), retry),
        Err(TxError::Construction(
            mycity_core::construction::Error::InvalidInput
        ))
    );
    assert_eq!(cash(&recovered, 4), payer_before - 10_000);
    assert_eq!(cash(&recovered, 2), payee_before + 10_000);

    // A later rejection does not reverse an already settled compensation.
    construction_auto(
        &mut recovered,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Rejected,
            evidence_id: 0,
        },
    )
    .unwrap();
    assert_eq!(
        recovered.world().view().construction.unwrap().phase,
        Phase::Rejected
    );
    assert_eq!(cash(&recovered, 4), payer_before - 10_000);
    assert_eq!(cash(&recovered, 2), payee_before + 10_000);
}

#[test]
fn compensation_insufficient_and_protected_funds_are_atomic() {
    let mut insufficient = compensation_engine(2_000_000, 3_000_000);
    construction_auto(&mut insufficient, Caller::Account(4), C::File).unwrap();
    let before = insufficient.world().view();
    assert_eq!(
        construction_auto(
            &mut insufficient,
            Caller::Account(2),
            C::DecideGate {
                gate_id: 3,
                decision: GateDecision::Accepted,
                evidence_id: 1,
            },
        ),
        Err(TxError::InsufficientFunds)
    );
    let after = insufficient.world().view();
    assert_eq!(after.accounts, before.accounts);
    assert_eq!(after.construction, before.construction);

    let mut protected = compensation_engine(10_000, 1_000_000);
    construction_auto(&mut protected, Caller::Account(4), C::File).unwrap();
    let owner = protected.world().account_ref(4).unwrap();
    let owner_cash = cash(&protected, 4);
    run(
        &mut protected,
        Caller::Account(4),
        Action::Protect {
            account: owner,
            amount: Money::new(owner_cash - 9_999).unwrap(),
        },
    );
    let before = protected.world().view();
    assert_eq!(
        construction_auto(
            &mut protected,
            Caller::Account(2),
            C::DecideGate {
                gate_id: 3,
                decision: GateDecision::Accepted,
                evidence_id: 1,
            },
        ),
        Err(TxError::InsufficientFunds)
    );
    let after = protected.world().view();
    assert_eq!(after.accounts, before.accounts);
    assert_eq!(after.construction, before.construction);
}

#[test]
fn demolition_checkpoint_resolves_historic_compensation_payee_without_current_gate() {
    let mut e = compensated_operational_engine();
    construction_auto(
        &mut e,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    )
    .unwrap();
    let project = e.world().view().construction.unwrap();
    assert_eq!(project.review_round(), 2);
    assert_eq!(project.phase, Phase::Review);
    assert!(Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).is_ok());

    let checkpoint = e.export_checkpoint().unwrap();
    let mut forged: Value = serde_json::from_slice(&checkpoint).unwrap();
    forged["world"]["construction"]["compensation"]["payee"] = json!(99_999);
    assert!(matches!(
        Engine::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()),
        Err(error) if error == "checkpoint_construction_compensation_payee"
    ));
}

#[test]
fn checkpoint_cannot_relabel_paid_initial_review_as_demolition_review() {
    let mut e = compensation_engine(10_000, 1_000_000);
    construction_auto(&mut e, Caller::Account(4), C::File).unwrap();
    construction_auto(
        &mut e,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 3,
            decision: GateDecision::Accepted,
            evidence_id: 13,
        },
    )
    .unwrap();
    let project = e.world().view().construction.unwrap();
    assert_eq!(project.phase, Phase::Review);
    assert_eq!(project.review_round(), 1);
    assert!(project
        .compensation
        .unwrap()
        .settled_operation_id()
        .is_some());

    let checkpoint = e.export_checkpoint().unwrap();
    assert!(Engine::restore_checkpoint(&checkpoint).is_ok());
    let mut forged: Value = serde_json::from_slice(&checkpoint).unwrap();
    forged["world"]["construction"]["review_round"] = json!(2);
    forged["world"]["construction"]["gates"][2]["decided"] = json!("pending");
    forged["world"]["construction"]["gates"][2]["evidence_id"] = json!(0);
    assert!(matches!(
        Engine::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()),
        Err(error) if error == "checkpoint_construction_profile"
    ));

    let mut forged_draft: Value = serde_json::from_slice(&checkpoint).unwrap();
    forged_draft["world"]["construction"]["review_round"] = json!(2);
    forged_draft["world"]["construction"]["phase"] = json!("draft");
    assert!(matches!(
        Engine::restore_checkpoint(&serde_json::to_vec(&forged_draft).unwrap()),
        Err(error) if error == "checkpoint_construction_profile"
    ));
}
