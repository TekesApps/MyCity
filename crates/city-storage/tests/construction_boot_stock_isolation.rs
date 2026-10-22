use mycity_core::{
    assembly::{self, World},
    construction::{ExecutionPolicy, Gate, GateCategory, GateDecision, Kind, Material, Project},
    transaction::{Engine, Resource},
    units::{GameTime, Money, Quantity},
};
use serde_json::{json, Value};

const OPENING: &[u8] = include_bytes!("../../../fixtures/core-town-utility-opening.json");

fn resources() -> Vec<Resource> {
    [
        (80, 1),
        (101, 100_000),
        (102, 100_000),
        (103, 100_000),
        (104, 100_000),
        (201, 100_000),
        (202, 100_000),
    ]
    .into_iter()
    .map(|(id, capacity)| Resource::new(id, capacity).unwrap())
    .collect()
}

fn opening(stock_id: u32, legacy: bool) -> Vec<u8> {
    let basis_per_thousand = match stock_id {
        1 => 400,
        2 => 1,
        3 => 100,
        _ => panic!("fixture stock not used by Boot"),
    };
    let mut project = Project::new(
        17,
        Kind::Build,
        3,
        4,
        1,
        80,
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
            stock_id,
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(basis_per_thousand).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    if legacy {
        project.execution_policy = ExecutionPolicy::LegacyV1;
    }

    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    if stock_id == 3 {
        // The utility fixture's Boot water stock is initially empty. Make it a valid
        // non-zero governed material lot while keeping the source basis consistent.
        let stocks = value["stocks"].as_array_mut().unwrap();
        let water = stocks
            .iter_mut()
            .find(|stock| stock["id"] == json!(3))
            .unwrap();
        water["quantity"] = json!(1_000);
        water["basis"] = json!(100);
        let sources = value["sources"].as_array_mut().unwrap();
        let water_source = sources
            .iter_mut()
            .find(|source| source["id"] == json!(7))
            .unwrap();
        water_source["amount"] = json!(100);
    }
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn recoverable_construction_genesis_rejects_all_boot_governed_inventory_kinds() {
    for stock_id in [1, 2, 3] {
        // Project::new defaults to RecoverableV2. Fuel, electricity, and water are
        // all owned by operator 3 and listed in Boot's immutable stock binding.
        match assembly::assemble(&opening(stock_id, false)) {
            Err(error) => assert_eq!(error, "construction_governed_stock"),
            Ok(_) => panic!("Boot-governed stock {stock_id} was accepted at genesis"),
        }
    }
}

fn legacy_engine_with_boot_material() -> Engine {
    let world: World = assembly::assemble(&opening(2, true)).unwrap();
    assert_eq!(
        world.view().construction.unwrap().execution_policy,
        ExecutionPolicy::LegacyV1
    );
    Engine::new(world, resources()).unwrap()
}

#[test]
fn checkpoint_restore_rejects_v2_reinterpretation_of_legacy_boot_material_binding() {
    let engine = legacy_engine_with_boot_material();
    let checkpoint = engine.export_checkpoint().unwrap();
    let mut value: Value = serde_json::from_slice(&checkpoint).unwrap();
    value["world"]["construction"]["execution_policy"] = json!("recoverable_v2");
    let forged_v2_checkpoint = serde_json::to_vec(&value).unwrap();

    match Engine::restore_checkpoint(&forged_v2_checkpoint) {
        Err(error) => assert_eq!(error, "checkpoint_construction_governed_stock"),
        Ok(_) => panic!("restoring a V2 project over Boot stock unexpectedly succeeded"),
    }
}
