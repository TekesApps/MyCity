//! Run a complete build, demolition, and waste settlement through the authoritative Engine path.
use mycity_core::{
    assembly::assemble,
    construction::{
        EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material, Phase, Project,
        OPERATION_PREFIX,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::{GameTime, Money, Quantity},
};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SITE: u32 = 80;
const PROJECT_ID: u64 = 7;
const SHIFT_END: u64 = 17 * 3_600;
const NEXT_SHIFT_END: u64 = SHIFT_END + 86_400;
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1_000_000);

fn envelope(engine: &Engine, operation_id: u64, action: Action) -> Envelope {
    let world = engine.world().view();
    Envelope {
        world: world.scope.world(),
        branch: world.scope.branch(),
        operation_id,
        expected_revision: world.revision,
        actions: vec![action],
    }
}
fn ordinary(engine: &Engine) -> u64 {
    let _ = engine;
    NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed)
}
fn run(engine: &mut Engine, caller: Caller, action: Action) {
    let id = ordinary(engine);
    let request = envelope(engine, id, action);
    engine
        .execute(caller, request)
        .unwrap_or_else(|error| panic!("action {id} failed: {error:?}"));
}
fn construction(engine: &mut Engine, caller: Caller, command: C, op: u64) {
    let request = envelope(engine, op, Action::Construction { command });
    engine.execute(caller, request).unwrap_or_else(|error| {
        panic!("construction {command:?} operation {op} failed: {error:?}")
    });
}
fn effect_id(engine: &Engine) -> u64 {
    let project = engine.world().view().construction.unwrap();
    OPERATION_PREFIX | (project.id << 32) | u64::from(project.next_effect)
}
fn advance(engine: &mut Engine, target: u64) {
    for _ in 0..100 {
        run(
            engine,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
        if engine.timeline().unwrap().progress.target.is_none() {
            return;
        }
    }
    panic!("timeline did not reach the construction work date");
}

fn main() {
    // Owner 4 owns raw stock 1; contractor 3 receives wages; inspector 1 is an existing account.
    let mut project = Project::new(
        PROJECT_ID,
        Kind::Build,
        4,
        3,
        1,
        SITE,
        1,
        1,
        Money::new(1_000_000).unwrap(),
        Money::new(3_600).unwrap(),
        28_800,
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
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(400).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 3;
    project.waste_receiver_resource = 82;
    project.waste_units = 2;
    project.waste_fee = Money::new(250).unwrap();
    let mut opening: Value = serde_json::from_slice(OPENING).unwrap();
    opening["construction"] = serde_json::to_value(project).unwrap();
    let world = assemble(&serde_json::to_vec(&opening).unwrap()).unwrap();
    let mut engine = Engine::new(
        world,
        vec![
            Resource::new(SITE, 1).unwrap(),
            Resource::new(81, 2).unwrap(),
            Resource::new(82, 10).unwrap(),
        ],
    )
    .unwrap();

    let id = ordinary(&engine);
    construction(&mut engine, Caller::Account(4), C::File, id);
    let id = ordinary(&engine);
    construction(
        &mut engine,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 101,
        },
        id,
    );
    let id = ordinary(&engine);
    construction(
        &mut engine,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 102,
        },
        id,
    );
    let id = ordinary(&engine);
    construction(&mut engine, Caller::Account(4), C::Prepare, id);
    let start_id = effect_id(&engine);
    construction(&mut engine, Caller::Account(4), C::Start, start_id);

    run(
        &mut engine,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance(&mut engine, SHIFT_END);
    let work_id = effect_id(&engine);
    construction(
        &mut engine,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(SHIFT_END as i64).unwrap(),
            elapsed_seconds: SHIFT_END as u32,
            worker_seconds: 28_800,
        },
        work_id,
    );
    let id = effect_id(&engine);
    construction(&mut engine, Caller::Account(3), C::SubmitInspection, id);
    let id = effect_id(&engine);
    construction(
        &mut engine,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
        id,
    );

    let result = engine.world().view();
    assert_eq!(result.construction.unwrap().phase, Phase::Operational);
    assert!(result.construction.unwrap().operating_published);
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|r| r.id == 81)
            .unwrap()
            .capacity,
        4
    );

    let id = ordinary(&engine);
    construction(
        &mut engine,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
        id,
    );
    let consumer = engine
        .execute(
            Caller::Account(2),
            envelope(
                &engine,
                ordinary(&engine),
                Action::ReserveCapacity {
                    resource: 81,
                    units: 3,
                },
            ),
        )
        .unwrap()
        .created_tokens[0]
        .unwrap();
    run(
        &mut engine,
        Caller::Account(2),
        Action::OccupyCapacity { token: consumer },
    );
    let id = ordinary(&engine);
    construction(
        &mut engine,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 201,
        },
        id,
    );
    let id = ordinary(&engine);
    construction(
        &mut engine,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 202,
        },
        id,
    );
    let id = ordinary(&engine);
    construction(&mut engine, Caller::Account(4), C::Prepare, id);
    assert_eq!(
        engine.execute(
            Caller::Account(4),
            envelope(
                &engine,
                ordinary(&engine),
                Action::Construction { command: C::Start },
            ),
        ),
        Err(TxError::Capacity)
    );
    run(
        &mut engine,
        Caller::Account(2),
        Action::ReleaseCapacity { token: consumer },
    );
    let id = effect_id(&engine);
    construction(&mut engine, Caller::Account(4), C::Start, id);
    assert!(
        !engine
            .world()
            .view()
            .construction
            .unwrap()
            .operating_published
    );
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|r| r.id == 81)
            .unwrap()
            .capacity,
        2
    );
    advance(&mut engine, NEXT_SHIFT_END);
    let id = effect_id(&engine);
    construction(
        &mut engine,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(NEXT_SHIFT_END as i64).unwrap(),
            elapsed_seconds: 86_400,
            worker_seconds: 28_800,
        },
        id,
    );
    let owner_cash_before = engine
        .world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == 4)
        .unwrap()
        .cash
        .value();
    let id = ordinary(&engine);
    construction(&mut engine, Caller::Account(3), C::AcceptWaste, id);
    assert_eq!(
        engine
            .world()
            .view()
            .accounts
            .iter()
            .find(|a| a.id == 4)
            .unwrap()
            .cash
            .value(),
        owner_cash_before - 250
    );
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|r| r.id == 82)
            .unwrap()
            .occupied,
        2
    );
    let id = effect_id(&engine);
    construction(&mut engine, Caller::Account(4), C::Close, id);
    assert!(!engine.world().view().construction.unwrap().site_reserved);
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|r| r.id == SITE)
            .unwrap()
            .occupied,
        0
    );
    // Read the final committed state after waste fees and Close, not the earlier
    // Operational snapshot retained for the construction-stage assertions.
    let final_result = engine.world().view();
    let owner_cash = final_result
        .accounts
        .iter()
        .find(|a| a.id == 4)
        .unwrap()
        .cash
        .value();
    let owner_stock = final_result.stocks.iter().find(|s| s.id == 1).unwrap();
    println!(
        "Engine construction closed: project={} owner_cash={} stock_qty={} site_occupied={} waste_occupied={}",
        PROJECT_ID,
        owner_cash,
        owner_stock.quantity.value(),
        engine
            .resources()
            .iter()
            .find(|r| r.id == SITE)
            .unwrap()
            .occupied,
        engine
            .resources()
            .iter()
            .find(|r| r.id == 82)
            .unwrap()
            .occupied,
    );
}
