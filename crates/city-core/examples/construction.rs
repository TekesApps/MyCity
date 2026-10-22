//! Run a complete, bounded build through the authoritative Engine transaction path.
use mycity_core::{
    assembly::assemble,
    construction::{
        EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material, Phase, Project,
        OPERATION_PREFIX,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use serde_json::Value;

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SITE: u32 = 80;
const PROJECT_ID: u64 = 7;
const SHIFT_END: u64 = 17 * 3_600;

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
    engine.world().view().revision + 100
}
fn run(engine: &mut Engine, caller: Caller, action: Action) {
    let id = ordinary(engine);
    let request = envelope(engine, id, action);
    engine.execute(caller, request).unwrap();
}
fn construction(engine: &mut Engine, caller: Caller, command: C, op: u64) {
    let request = envelope(engine, op, Action::Construction { command });
    engine.execute(caller, request).unwrap();
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
    let project = Project::new(
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
    let mut opening: Value = serde_json::from_slice(OPENING).unwrap();
    opening["construction"] = serde_json::to_value(project).unwrap();
    let world = assemble(&serde_json::to_vec(&opening).unwrap()).unwrap();
    let mut engine = Engine::new(world, vec![Resource::new(SITE, 1).unwrap()]).unwrap();

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
    let id = ordinary(&engine);
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
    let owner_cash = result
        .accounts
        .iter()
        .find(|a| a.id == 4)
        .unwrap()
        .cash
        .value();
    let owner_stock = result.stocks.iter().find(|s| s.id == 1).unwrap();
    println!(
        "Engine construction operational: project={} owner_cash={} stock_qty={} occupied_site={}",
        PROJECT_ID,
        owner_cash,
        owner_stock.quantity.value(),
        engine
            .resources()
            .iter()
            .find(|r| r.id == SITE)
            .unwrap()
            .occupied
    );
}
