//! Exercise demolition clearance for a partially built, never-commissioned project.
use mycity_core::{
    assembly::assemble,
    construction::{
        AbortClearanceMaterialRule, AbortClearanceSpec, EngineCommand as C, Gate, GateCategory,
        GateDecision, Kind, Material, Phase, Project, OPERATION_PREFIX,
    },
    timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SITE: u32 = 80;
const OPERATING_RESOURCE: u32 = 81;
const WASTE_RESOURCE: u32 = 82;
const PROJECT_ID: u64 = 7;
const SHIFT_END: u64 = 17 * 3_600;
const NEXT_SHIFT_END: u64 = SHIFT_END + 86_400;
const BUILD_WORK_SECONDS: u32 = 14_400;
const CLEARANCE_WORK_SECONDS: u32 = 2_400;
const DISPOSAL_FEE: i64 = 250;
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(2_000_000);

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
    engine
        .execute(caller, envelope(engine, id, action))
        .unwrap_or_else(|error| panic!("action {id} failed: {error:?}"));
}

fn construction(engine: &mut Engine, caller: Caller, command: C, operation_id: u64) {
    engine
        .execute(
            caller,
            envelope(engine, operation_id, Action::Construction { command }),
        )
        .unwrap_or_else(|error| {
            panic!("construction {command:?}, operation {operation_id} failed: {error:?}")
        });
}

fn command(engine: &mut Engine, caller: Caller, command: C) {
    let operation_id = ordinary(engine);
    construction(engine, caller, command, operation_id);
}

fn effect_command(engine: &mut Engine, caller: Caller, command: C) {
    let operation_id = effect_id(engine);
    construction(engine, caller, command, operation_id);
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
    panic!("timeline did not reach the scheduled work date");
}

fn account_cash(engine: &Engine, id: u32) -> i64 {
    engine
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == id)
        .unwrap()
        .cash
        .value()
}

fn main() {
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
    project.operating_resource = OPERATING_RESOURCE;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 3;
    project.waste_receiver_resource = WASTE_RESOURCE;
    project.waste_units = 2;
    project.waste_fee = Money::new(DISPOSAL_FEE).unwrap();
    project.abort_clearance_spec = Some(AbortClearanceSpec {
        base_worker_seconds: 600,
        material_rules: [
            AbortClearanceMaterialRule {
                stock_id: 1,
                quantity_per_waste_capacity_unit: 500,
                worker_seconds_per_thousand_quantity: 3_600,
            },
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
        ],
    });

    let mut opening: Value = serde_json::from_slice(OPENING).unwrap();
    opening["construction"] = serde_json::to_value(project).unwrap();
    let world = assemble(&serde_json::to_vec(&opening).unwrap()).unwrap();
    let mut engine = Engine::new(
        world,
        vec![
            Resource::new(SITE, 1).unwrap(),
            Resource::new(OPERATING_RESOURCE, 2).unwrap(),
            Resource::new(WASTE_RESOURCE, 10).unwrap(),
        ],
    )
    .unwrap();

    command(&mut engine, Caller::Account(4), C::File);
    command(
        &mut engine,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 101,
        },
    );
    command(
        &mut engine,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 102,
        },
    );
    command(&mut engine, Caller::Account(4), C::Prepare);
    effect_command(&mut engine, Caller::Account(4), C::Start);
    let original_site_token = engine
        .world()
        .view()
        .construction
        .unwrap()
        .site_token
        .unwrap();

    run(
        &mut engine,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance(&mut engine, SHIFT_END);
    effect_command(
        &mut engine,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(SHIFT_END as i64).unwrap(),
            elapsed_seconds: SHIFT_END as u32,
            worker_seconds: BUILD_WORK_SECONDS,
        },
    );
    command(&mut engine, Caller::Account(3), C::Pause);
    let partially_built = engine.world().view().construction.unwrap();
    assert_eq!(partially_built.phase, Phase::Paused);
    assert_eq!(
        partially_built.completed_worker_seconds,
        u64::from(BUILD_WORK_SECONDS)
    );
    assert_eq!(
        partially_built.materials[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert!(!partially_built.operating_published);
    assert_eq!(partially_built.site_token, Some(original_site_token));
    let stock_after_build = engine
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == 1)
        .unwrap()
        .quantity;
    let project_spent_before_clearance = partially_built.spent;
    let contractor_cash_before_clearance = account_cash(&engine, 3);

    command(
        &mut engine,
        Caller::Account(4),
        C::BeginDemolition { waste_receiver: 3 },
    );
    command(
        &mut engine,
        Caller::Account(2),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 201,
        },
    );
    command(
        &mut engine,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 202,
        },
    );
    command(&mut engine, Caller::Account(4), C::Prepare);
    command(&mut engine, Caller::Account(4), C::Start);
    let clearing = engine.world().view().construction.unwrap();
    assert_eq!(clearing.phase, Phase::Demolition);
    assert!(clearing.is_uncommissioned_clearance());
    assert_eq!(
        clearing.planned_worker_seconds,
        u64::from(CLEARANCE_WORK_SECONDS)
    );
    assert_eq!(clearing.effective_waste_units(), 1);
    assert_eq!(clearing.site_token, Some(original_site_token));
    assert_eq!(clearing.materials[0].consumed, Quantity::new(500).unwrap());
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|resource| resource.id == OPERATING_RESOURCE)
            .unwrap()
            .capacity,
        2
    );
    assert!(!clearing.operating_published);

    advance(&mut engine, NEXT_SHIFT_END);
    effect_command(
        &mut engine,
        Caller::Account(3),
        C::Work {
            at: GameTime::new(NEXT_SHIFT_END as i64).unwrap(),
            elapsed_seconds: 86_400,
            worker_seconds: CLEARANCE_WORK_SECONDS,
        },
    );
    let completed_clearance = engine.world().view().construction.unwrap();
    assert_eq!(
        completed_clearance.materials[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        engine
            .world()
            .view()
            .stocks
            .iter()
            .find(|stock| stock.id == 1)
            .unwrap()
            .quantity,
        stock_after_build,
        "clearance must not consume the construction material a second time"
    );
    assert_eq!(
        completed_clearance.spent.value() - project_spent_before_clearance.value(),
        i64::from(CLEARANCE_WORK_SECONDS),
        "clearance labor is charged at the configured hourly wage"
    );

    command(&mut engine, Caller::Account(3), C::AcceptWaste);
    assert_eq!(
        account_cash(&engine, 3) - contractor_cash_before_clearance,
        i64::from(CLEARANCE_WORK_SECONDS) + DISPOSAL_FEE,
        "contractor receives clearance wages and the full flat disposal fee"
    );
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|resource| resource.id == WASTE_RESOURCE)
            .unwrap()
            .occupied,
        1
    );
    let clearing_token = engine.world().view().construction.unwrap().site_token;
    assert_eq!(clearing_token, Some(original_site_token));
    effect_command(&mut engine, Caller::Account(4), C::Close);
    let final_project = engine.world().view().construction.unwrap();
    assert_eq!(final_project.phase, Phase::Closed);
    assert_eq!(final_project.site_token, None);
    assert_eq!(
        final_project.spent.value() - project_spent_before_clearance.value(),
        i64::from(CLEARANCE_WORK_SECONDS) + DISPOSAL_FEE,
        "clearance labor and disposal fee remain recorded as costs"
    );
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|resource| resource.id == SITE)
            .unwrap()
            .occupied,
        0,
        "the original site lease is released exactly once at Close"
    );
    assert_eq!(
        engine
            .resources()
            .iter()
            .find(|resource| resource.id == OPERATING_RESOURCE)
            .unwrap()
            .capacity,
        2
    );
    assert!(!final_project.operating_published);
    assert!(final_project.operating_published_at.is_none());
    println!(
        "unfinished clearance closed: project={} built_work={}/28800 clearance_work={} waste_units={} disposal_fee={} original_stock_after_build={} site_occupied={} operating_capacity={} published={}",
        PROJECT_ID,
        final_project
            .abort_clearance
            .unwrap()
            .original_completed_worker_seconds,
        final_project
            .abort_clearance
            .unwrap()
            .required_worker_seconds,
        final_project.effective_waste_units(),
        final_project.effective_waste_fee().value(),
        stock_after_build.value(),
        engine
            .resources()
            .iter()
            .find(|resource| resource.id == SITE)
            .unwrap()
            .occupied,
        engine
            .resources()
            .iter()
            .find(|resource| resource.id == OPERATING_RESOURCE)
            .unwrap()
            .capacity,
        final_project.operating_published,
    );
    println!(
        "summary={}",
        json!({
            "project": PROJECT_ID,
            "result": "closed",
            "original_site_token_retained_until_close": true,
            "construction_material_refunded": false,
            "construction_material_consumed_twice": false,
            "base_operating_capacity": 2,
            "operating_capacity_published": false,
            "model_only_not_full_legal_process": true,
        })
    );
}
