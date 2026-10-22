//! Store-backed one-shift construction utility transaction.
use mycity_core::{
    construction::{
        ConstructionUtilityContract, ConstructionUtilitySupply, EngineCommand as C, Gate,
        GateCategory, GateDecision, Kind, Material, Project,
    },
    timeline::{Command as T, DAY},
    transaction::{Action, Caller, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf};

const OPENING: &[u8] = include_bytes!("../../../../fixtures/core-town-utility-opening.json");
const RULES: &str = "construction-utilities-cli-v1";
const OWNER: u32 = 7;
const CONTRACTOR: u32 = 4;
const OPERATOR: u32 = 3;
const ELECTRICITY: u32 = 2;
const WATER: u32 = 3;
const WORK_START: u64 = 9 * 3_600;
const WORK_END: u64 = 10 * 3_600;
const WORK_SECONDS: u64 = WORK_END - WORK_START;

fn err(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn opening() -> Result<Vec<u8>, String> {
    let project = Project::new(
        7,
        Kind::Build,
        OWNER,
        CONTRACTOR,
        1,
        80,
        1,
        1,
        Money::new(100_000).map_err(err)?,
        Money::new(3_600).map_err(err)?,
        WORK_SECONDS,
        &[
            Gate {
                id: 1,
                category: GateCategory::OwnerConsent,
                evidence_id: 0,
                party: OWNER,
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
            stock_id: 15,
            required: Quantity::new(1_000).map_err(err)?,
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(1_000).map_err(err)?,
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .map_err(err)?;
    let mut project = project;
    project.utility_contract = Some(ConstructionUtilityContract {
        electricity: Some(ConstructionUtilitySupply {
            operator: OPERATOR,
            stock_id: ELECTRICITY,
            rate_per_3600_worker_seconds: Quantity::new(1_000).map_err(err)?,
            tariff_per_quantity_unit: Money::new(20).map_err(err)?,
            quantity_unit: Quantity::new(1_000).map_err(err)?,
        }),
        water: Some(ConstructionUtilitySupply {
            operator: OPERATOR,
            stock_id: WATER,
            rate_per_3600_worker_seconds: Quantity::new(1_000).map_err(err)?,
            tariff_per_quantity_unit: Money::new(10).map_err(err)?,
            quantity_unit: Quantity::new(1_000).map_err(err)?,
        }),
    });
    project.enable_material_accounting().map_err(err)?;
    let mut value: Value = serde_json::from_slice(OPENING).map_err(err)?;
    value["construction"] = serde_json::to_value(project).map_err(err)?;
    let water = value["stocks"]
        .as_array_mut()
        .ok_or("opening_stocks_missing")?
        .iter_mut()
        .find(|row| row["id"] == json!(WATER))
        .ok_or("opening_water_stock_missing")?;
    water["quantity"] = json!(20_000);
    water["basis"] = json!(100);
    let source = value["sources"]
        .as_array_mut()
        .ok_or("opening_sources_missing")?
        .iter_mut()
        .find(|row| row["id"] == json!(7))
        .ok_or("opening_water_source_missing")?;
    source["amount"] = json!(100);
    serde_json::to_vec(&value).map_err(err)
}

fn resources() -> Result<Vec<Resource>, String> {
    [
        (80, 1),
        (101, 100_000),
        (102, 100_000),
        (103, 100_000),
        (104, 100_000),
        (201, 2_000),
        (202, 2_000),
    ]
    .into_iter()
    .map(|(id, capacity)| Resource::new(id, capacity).map_err(err))
    .collect()
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> Result<(), String> {
    let view = store.engine().map_err(err)?.world().view();
    let result = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: store.committed_sequence() + 1,
                expected_revision: view.revision,
                actions: vec![action],
            },
        )
        .map_err(err)?;
    if !result.has_persistent_receipt {
        return Err("missing_durable_receipt".into());
    }
    result.model.map(|_| ()).map_err(err)
}

fn construction(store: &mut Store, caller: Caller, command: C) -> Result<(), String> {
    execute(store, caller, Action::Construction { command })
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..128 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
                    target,
                    limit: 8_192,
                },
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline_missing")?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("timeline_not_complete:{target}"))
}

fn run(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err("store path already exists; provide a new directory".into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(err)?;
    }
    let mut store = Store::create_spatial(
        &path,
        &opening()?,
        resources()?,
        RULES,
        include_bytes!("../../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../../fixtures/terrain/m0-flat16.png"),
    )
    .map_err(err)?;
    execute(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    )?;
    advance_to(&mut store, WORK_START)?;
    construction(&mut store, Caller::Account(OWNER), C::File)?;
    construction(
        &mut store,
        Caller::Account(OWNER),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 9001,
        },
    )?;
    construction(
        &mut store,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 9002,
        },
    )?;
    construction(&mut store, Caller::Account(OWNER), C::Prepare)?;
    construction(&mut store, Caller::Account(OWNER), C::Start)?;
    advance_to(&mut store, WORK_END)?;
    construction(
        &mut store,
        Caller::Account(CONTRACTOR),
        C::Work {
            at: GameTime::new(i64::try_from(WORK_END).map_err(err)?).map_err(err)?,
            elapsed_seconds: u32::try_from(WORK_SECONDS).map_err(err)?,
            worker_seconds: u32::try_from(WORK_SECONDS).map_err(err)?,
        },
    )?;
    let before_checkpoint = store.state_hash().map_err(err)?;
    let project = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction_missing")?;
    let boot = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .boot
        .ok_or("boot_missing")?;
    if project.utility_worker_seconds != WORK_SECONDS
        || project.utility_consumed != [Quantity::new(1_000).map_err(err)?; 2]
        || project.utility_charged != [Money::new(20).map_err(err)?, Money::new(10).map_err(err)?]
        || boot.local_construction_exported != [Quantity::new(1_000).map_err(err)?; 2]
    {
        return Err("construction_utility_reconciliation_failed".into());
    }
    store.checkpoint().map_err(err)?;
    drop(store);
    let reopened = Store::open(&path, RULES).map_err(err)?;
    let after_reopen = reopened.state_hash().map_err(err)?;
    if after_reopen != before_checkpoint {
        return Err("checkpoint_cold_open_hash_mismatch".into());
    }
    let view = reopened.engine().map_err(err)?.world().view();
    println!(
        "{}",
        json!({
            "schema":"mycity-construction-utilities-cli-v1",
            "scenario":"normal", "store":path,
            "service_day":view.boot.map(|boot|boot.time/DAY),
            "worker_seconds":project.utility_worker_seconds,
            "utility_quantities":project.utility_consumed.map(Quantity::value),
            "utility_fees":project.utility_charged.map(Money::value),
            "boot_exports":view.boot.map(|boot|boot.exports.map(Quantity::value)),
            "construction_local_exports":view.boot.map(|boot|boot.local_construction_exported.map(Quantity::value)),
            "project_spent_cents":project.spent.value(),
            "material_accounting":project.material_accounting,
            "project_inline_bytes":std::mem::size_of::<Project>(),
            "cold_open_state_hash_equal":true, "state_hash":after_reopen,
            "scope":"store_construction_boot_utilities",
            "local_boot_supply":true,
            "town_cross_city_competition":"not_tested"
        })
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let path = PathBuf::from(args.next().ok_or_else(|| {
        "usage: cargo run -p mycity-storage --bin construction_utilities -- <new-store-dir>"
            .to_owned()
    })?);
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    run(path)
}
