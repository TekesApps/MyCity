//! Store-backed joint road, construction, Boot utility, and Town market scenario.
//!
//! Fixed aggregate OD cohorts and one construction work-shift are explicit interface
//! inputs, not independent person or traffic simulations. The real Store owns the
//! allocation profile and daily proofs. Pass a fresh --store-dir for each run; the
//! completed directory is retained so the cold-open receipt remains inspectable.
use mycity_core::{
    boot::{Command as BootCommand, Facility},
    construction::{
        ConstructionUtilityContract, ConstructionUtilitySupply,
        EngineCommand as ConstructionCommand, Gate, GateCategory, GateDecision, Kind, Material,
        Project, OPERATION_PREFIX,
    },
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, RoadCapacityEffectBinding,
        TransportAllocationProfile, TripPartition,
    },
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    town_runtime::MarketSummary,
    transaction::{Action, Caller, Envelope, JournalRecord, Resource, TxError},
    transport::{Entrance, Node, OdDemand, Opening, RoadEdge},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::Store;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const RULES: &str = "m0-town-lifelines-v1";
const FIXTURE: &[u8] = include_bytes!("../../../fixtures/core-town-utility-opening.json");
const COMPANY: u32 = 7;
const CONTRACTOR: u32 = 4;
const HOUSEHOLD: u32 = 5;
const PERMIT_AUTHORITY: u32 = 1;
const OPERATOR: u32 = 3;
const RAW_STOCK: u32 = 15;
const ELECTRICITY_STOCK: u32 = 2;
const WATER_STOCK: u32 = 3;
const PRODUCT_STOCK: u32 = 16;
const SITE_RESOURCE: u32 = 80;
const ROAD_RESOURCE: u32 = 81;
const SHIFT_END: u64 = 17 * 3_600;
const CONSTRUCTION_SHORTAGE_OPERATION: u64 = 81_003;
const CONSTRUCTION_WORK_LIMIT_OPERATION: u64 = 81_004;
const NEW_EMPLOYER: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmploymentScenario {
    None,
    Control,
    Change,
}

impl EmploymentScenario {
    fn enabled(self) -> bool {
        self != Self::None
    }

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Control => "employment-control",
            Self::Change => "employment-change",
        }
    }
}

#[derive(Clone, Copy)]
struct Options {
    hold_road_inspection: bool,
    generator_failure: bool,
    construction_utilities: bool,
    employment: EmploymentScenario,
}

fn err(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(digest: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn opening() -> Opening {
    Opening {
        revision: 1,
        nodes: vec![Node { id: 1, city_id: 1 }, Node { id: 2, city_id: 1 }],
        edges: vec![
            RoadEdge {
                id: 10,
                from: 1,
                to: 2,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: 2,
                mode_mask: 1,
                open: true,
                boundary_source: None,
            },
            RoadEdge {
                id: 11,
                from: 2,
                to: 1,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: 10,
                mode_mask: 1,
                open: true,
                boundary_source: None,
            },
        ],
        entrances: vec![
            Entrance {
                id: 1,
                owner: HOUSEHOLD,
                node: 1,
                mode_mask: 1,
                authorized: true,
            },
            Entrance {
                id: 2,
                owner: COMPANY,
                node: 2,
                mode_mask: 1,
                authorized: true,
            },
        ],
        demands: vec![
            OdDemand {
                source_id: 101,
                origin_entrance: 1,
                destination_entrance: 2,
                mode: 1,
                people: 10,
            },
            OdDemand {
                source_id: 102,
                origin_entrance: 2,
                destination_entrance: 1,
                mode: 1,
                people: 10,
            },
        ],
    }
}

fn allocation_profile(opening: Opening) -> Result<Vec<u8>, String> {
    let assignment = opening.assign().map_err(err)?;
    let profile = TransportAllocationProfile {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        market_profile_id: 1,
        period_seconds: DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: Some(RoadCapacityEffectBinding {
            project_id: 7,
            resource_id: ROAD_RESOURCE,
            edge_id: 10,
            base_capacity_people: 2,
            added_capacity_people: 8,
        }),
        opening,
        assignment,
        partitions: vec![TripPartition {
            quote_id: 1,
            outbound_source: 101,
            return_source: 102,
            trip_count: 10,
            goods_per_trip: Quantity::new(10).map_err(err)?,
        }],
        fee_policy: FeePolicyInput {
            provider_account: PERMIT_AUTHORITY,
            cents_per_travel_second: 1,
        },
    };
    serde_json::to_vec(&profile).map_err(err)
}

fn project(construction_utilities: bool) -> Result<Project, String> {
    let mut project = Project::new(
        7,
        Kind::Build,
        COMPANY,
        CONTRACTOR,
        PERMIT_AUTHORITY,
        SITE_RESOURCE,
        1,
        1,
        Money::new(1_000_000).map_err(err)?,
        Money::new(3_600).map_err(err)?,
        28_800,
        &[
            Gate {
                id: 1,
                category: GateCategory::OccupantConsent,
                evidence_id: 0,
                party: HOUSEHOLD,
                decided: GateDecision::Pending,
            },
            Gate {
                id: 2,
                category: GateCategory::Permit,
                evidence_id: 0,
                party: PERMIT_AUTHORITY,
                decided: GateDecision::Pending,
            },
        ],
        &[Material {
            stock_id: RAW_STOCK,
            required: Quantity::new(500).map_err(err)?,
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(1_000).map_err(err)?,
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .map_err(err)?;
    project.operating_resource = ROAD_RESOURCE;
    project.operating_base_capacity = 2;
    project.operating_units = 8;
    project.operating_edge_id = 10;
    if construction_utilities {
        project.utility_contract = Some(ConstructionUtilityContract {
            electricity: Some(ConstructionUtilitySupply {
                operator: OPERATOR,
                stock_id: ELECTRICITY_STOCK,
                // A full 8-hour work shift requires exactly 400,000 electricity units.
                rate_per_3600_worker_seconds: Quantity::new(50_000).map_err(err)?,
                tariff_per_quantity_unit: Money::new(20).map_err(err)?,
                quantity_unit: Quantity::new(1_000).map_err(err)?,
            }),
            water: Some(ConstructionUtilitySupply {
                operator: OPERATOR,
                stock_id: WATER_STOCK,
                // A full 8-hour work shift requires exactly 400,000 water units.
                rate_per_3600_worker_seconds: Quantity::new(50_000).map_err(err)?,
                tariff_per_quantity_unit: Money::new(10).map_err(err)?,
                quantity_unit: Quantity::new(1_000).map_err(err)?,
            }),
        });
        project.enable_material_accounting().map_err(err)?;
    }
    Ok(project)
}

fn opening_bytes(
    profile_sha256: [u8; 32],
    project: &Project,
    employment: EmploymentScenario,
) -> Result<Vec<u8>, String> {
    let mut value: Value = serde_json::from_slice(FIXTURE).map_err(err)?;
    if employment.enabled() {
        // Both comparison modes use the same opening. Move the original 1,000 cash
        // capital from the household to a new, idle employer so wages remain funded
        // without injecting extra aggregate cash into the experiment.
        value["sources"][18]["amount"] = 0.into();
        value["sources"]
            .as_array_mut()
            .ok_or("fixture_sources_not_array")?
            .push(serde_json::json!({
                "id": 25,
                "kind": "cash_capital",
                "amount": 1000
            }));
        value["accounts"][4]["cash"] = 0.into();
        value["accounts"]
            .as_array_mut()
            .ok_or("fixture_accounts_not_array")?
            .push(serde_json::json!({
                "id": NEW_EMPLOYER,
                "role": "company",
                "cash": 1000,
                "source": 25
            }));
    }
    value["town"]["transport_allocation_sha256"] =
        serde_json::to_value(profile_sha256).map_err(err)?;
    value["construction"] = serde_json::to_value(project).map_err(err)?;
    serde_json::to_vec(&value).map_err(err)
}

fn resources() -> Result<Vec<Resource>, String> {
    [
        (SITE_RESOURCE, 1),
        (ROAD_RESOURCE, 2),
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2_000),
        (202, 2_000),
    ]
    .into_iter()
    .map(|(id, capacity)| Resource::new(id, capacity).map_err(err))
    .collect()
}

fn execute_action(store: &mut Store, caller: Caller, action: Action) -> Result<(), String> {
    execute_action_with_id(store, caller, action).map(|_| ())
}

fn execute_action_with_id(
    store: &mut Store,
    caller: Caller,
    action: Action,
) -> Result<u64, String> {
    let view = store.engine().map_err(err)?.world().view();
    let operation_id = store.committed_sequence() + 1;
    let outcome = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id,
                expected_revision: view.revision,
                actions: vec![action],
            },
        )
        .map_err(err)?;
    outcome.model.map(|_| operation_id).map_err(err)
}

fn construction_effect_id(store: &Store) -> Result<u64, String> {
    let project = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction project missing")?;
    Ok(OPERATION_PREFIX | (project.id << 32) | u64::from(project.next_effect))
}

fn construction_action(
    store: &mut Store,
    caller: Caller,
    command: ConstructionCommand,
) -> Result<(), String> {
    construction_action_with_id(store, caller, command).map(|_| ())
}

fn construction_action_with_id(
    store: &mut Store,
    caller: Caller,
    command: ConstructionCommand,
) -> Result<u64, String> {
    let view = store.engine().map_err(err)?.world().view();
    let operation_id = if matches!(
        command,
        ConstructionCommand::Start
            | ConstructionCommand::Work { .. }
            | ConstructionCommand::SubmitInspection
            | ConstructionCommand::AcceptInspection { accepted: true, .. }
    ) {
        construction_effect_id(store)?
    } else {
        store.committed_sequence() + 1
    };
    let outcome = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id,
                expected_revision: view.revision,
                actions: vec![Action::Construction { command }],
            },
        )
        .map_err(err)?;
    outcome.model.map(|_| operation_id).map_err(err)
}

fn replay_record_exact(store: &mut Store, record: JournalRecord) -> Result<(), String> {
    let view = store.engine().map_err(err)?.world().view();
    let actions = record.actions.into_iter().flatten().collect();
    let caller = if record.actor == 0 {
        Caller::System
    } else {
        Caller::Account(record.actor)
    };
    let replay = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: record.operation_id,
                expected_revision: record.expected_revision,
                actions,
            },
        )
        .map_err(err)?;
    if replay.model != record.outcome || !replay.has_persistent_receipt {
        return Err(format!("journal_replay_changed:{}", record.operation_id));
    }
    Ok(())
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..128 {
        execute_action(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline missing")?
            .progress
            .target
            .is_none()
        {
            return Ok(());
        }
    }
    Err(format!("timeline_target_not_reached:{target}"))
}

fn verify_proof(
    dir: &Path,
    store: &Store,
    service_day: u64,
    expected_outbound_capacity: u32,
) -> Result<[u8; 32], String> {
    let receipt = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("town market missing")?
        .state
        .last_transport
        .ok_or("transport receipt missing")?;
    if receipt.issued_day != service_day || receipt.issued_generation != service_day {
        return Err(format!("transport_receipt_boundary:{service_day}"));
    }
    let path = dir.join(format!(
        "town-proof-{}.json",
        hex(&receipt.issued_proof_sha256)
    ));
    let bytes = fs::read(&path).map_err(err)?;
    if hash(&bytes) != receipt.issued_proof_sha256 {
        return Err(format!("proof_hash_mismatch:{service_day}"));
    }
    let proof: mycity_core::market_access::TransportProofAsset =
        serde_json::from_slice(&bytes).map_err(err)?;
    if proof.profile_id != 1
        || proof.service_day != service_day
        || proof.generation != service_day
        || !proof.assignment.matches(&proof.opening)
        || proof
            .opening
            .edges
            .iter()
            .find(|edge| edge.id == 10)
            .map(|edge| edge.capacity_people)
            != Some(expected_outbound_capacity)
        || proof
            .opening
            .edges
            .iter()
            .find(|edge| edge.id == 11)
            .map(|edge| edge.capacity_people)
            != Some(10)
    {
        return Err(format!("proof_profile_or_capacity_mismatch:{service_day}"));
    }
    for (source, edge, expected_served) in [(101, 10, expected_outbound_capacity), (102, 11, 10)] {
        let outcome = proof
            .assignment
            .demand_outcomes
            .iter()
            .find(|outcome| outcome.source_id == source)
            .ok_or_else(|| format!("proof_demand_missing:{service_day}:{source}"))?;
        let flow = proof
            .assignment
            .edge_flows
            .iter()
            .find(|flow| flow.edge_id == edge)
            .ok_or_else(|| format!("proof_edge_missing:{service_day}:{edge}"))?;
        if outcome.requested_people != 10
            || outcome.served_people != expected_served
            || outcome.unserved_people != 10 - expected_served
            || flow.people != u64::from(expected_served)
        {
            return Err(format!("proof_flow_mismatch:{service_day}:{source}"));
        }
    }
    Ok(receipt.issued_proof_sha256)
}

fn stock_basis(store: &Store, stock_id: u32) -> Result<Money, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == stock_id)
        .map(|stock| stock.basis)
        .ok_or_else(|| format!("stock_missing:{stock_id}"))
}

fn checkpoint_reopen(mut store: Store, dir: &Path) -> Result<Store, String> {
    let before = store.state_hash().map_err(err)?;
    store.checkpoint().map_err(err)?;
    drop(store);
    let reopened = Store::open(dir, RULES).map_err(err)?;
    if reopened.state_hash().map_err(err)? != before {
        return Err("checkpoint_cold_open_state_hash_mismatch".into());
    }
    Ok(reopened)
}

fn resource_balances(store: &Store) -> Result<Vec<[u32; 4]>, String> {
    Ok(store
        .engine()
        .map_err(err)?
        .resources()
        .iter()
        .map(|row| [row.id, row.capacity, row.reserved, row.occupied])
        .collect())
}

fn settle_three_day_market(
    dir: &Path,
    store: &mut Store,
    operation_id: u64,
    service_day: u64,
    capacity: u32,
) -> Result<(MarketSummary, [u8; 32], Money, Money), String> {
    let operator_before = account_cash(store, OPERATOR)?;
    let basis_before = stock_basis(store, ELECTRICITY_STOCK)?;
    let summary = store
        .settle_town_market(operation_id)
        .map_err(err)?
        .model
        .map_err(err)?
        .town
        .ok_or_else(|| format!("day-{service_day}-market-summary-missing"))?;
    let operator_after = account_cash(store, OPERATOR)?;
    let basis_after = stock_basis(store, ELECTRICITY_STOCK)?;
    let payment = operator_after.checked_sub(operator_before).map_err(err)?;
    let local_basis_used = basis_before.checked_sub(basis_after).map_err(err)?;
    let proof = verify_proof(dir, store, service_day, capacity)?;
    Ok((summary, proof, payment, local_basis_used))
}

fn run_construction_utilities_three_days(
    dir: &Path,
    mut store: Store,
    initial_cash: i128,
    initial_raw: Quantity,
    options: Options,
    initial_payroll_operation_id: u64,
    mut contract_operation_ids: Vec<u64>,
) -> Result<(), String> {
    let hold_road_inspection = options.hold_road_inspection;
    let employment = options.employment;
    let electricity_before_failure = stock_quantity(&store, ELECTRICITY_STOCK)?;
    let water_before_failure = stock_quantity(&store, WATER_STOCK)?;
    if electricity_before_failure != Quantity::ZERO
        || water_before_failure != Quantity::new(600_000).map_err(err)?
    {
        return Err(format!(
            "construction_shortage_prep_mismatch:electricity={electricity_before_failure:?}:water={water_before_failure:?}"
        ));
    }
    let cash_before_failure = cash_total(&store)?;
    let raw_before_failure = stock_quantity(&store, RAW_STOCK)?;
    let before_view = store.engine().map_err(err)?.world().view();
    let resources_before_failure = resource_balances(&store)?;
    let project_before_failure = before_view.construction.ok_or("construction missing")?;
    let boot_before_failure = before_view.boot.ok_or("boot missing")?;
    let shortage = store
        .execute(
            Caller::Account(CONTRACTOR),
            Envelope {
                world: before_view.scope.world(),
                branch: before_view.scope.branch(),
                operation_id: CONSTRUCTION_SHORTAGE_OPERATION,
                expected_revision: before_view.revision,
                actions: vec![Action::Construction {
                    command: ConstructionCommand::Work {
                        at: GameTime::new(SHIFT_END as i64).map_err(err)?,
                        elapsed_seconds: SHIFT_END as u32,
                        worker_seconds: 28_800,
                    },
                }],
            },
        )
        .map_err(err)?;
    let shortage_error = shortage.model.err().ok_or("shortage_work_succeeded")?;
    contract_operation_ids.push(CONSTRUCTION_SHORTAGE_OPERATION);
    if shortage_error != TxError::Boot(mycity_core::boot::Error::Input)
        || !shortage.has_persistent_receipt
    {
        return Err(format!("wrong_day_zero_shortage_result:{shortage_error:?}"));
    }
    let after_shortage = store.engine().map_err(err)?.world().view();
    let project_after_shortage = after_shortage.construction.ok_or("construction missing")?;
    let boot_after_shortage = after_shortage.boot.ok_or("boot missing")?;
    if stock_quantity(&store, ELECTRICITY_STOCK)? != electricity_before_failure
        || stock_quantity(&store, WATER_STOCK)? != water_before_failure
        || stock_quantity(&store, RAW_STOCK)? != raw_before_failure
        || cash_total(&store)? != cash_before_failure
        || after_shortage.accounts != before_view.accounts
        || after_shortage.stocks != before_view.stocks
        || resource_balances(&store)? != resources_before_failure
        || project_after_shortage.completed_worker_seconds
            != project_before_failure.completed_worker_seconds
        || project_after_shortage.spent != project_before_failure.spent
        || project_after_shortage.utility_worker_seconds
            != project_before_failure.utility_worker_seconds
        || project_after_shortage.utility_consumed != project_before_failure.utility_consumed
        || project_after_shortage.utility_charged != project_before_failure.utility_charged
        || boot_after_shortage != boot_before_failure
    {
        return Err("day_zero_shortage_partially_committed".into());
    }

    let pause_operation_id = construction_action_with_id(
        &mut store,
        Caller::Account(CONTRACTOR),
        ConstructionCommand::Pause,
    )?;
    contract_operation_ids.push(pause_operation_id);
    let paused = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing")?;
    if paused.phase != mycity_core::construction::Phase::Paused
        || paused.last_work_at.map(GameTime::value) != Some(SHIFT_END as i64)
    {
        return Err("pause_timestamp_not_recorded".into());
    }
    store = checkpoint_reopen(store, dir)?;
    let paused_recovered = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing after pause recovery")?;
    if paused_recovered.phase != mycity_core::construction::Phase::Paused
        || paused_recovered.last_work_at.map(GameTime::value) != Some(SHIFT_END as i64)
    {
        return Err("pause_lower_bound_lost_after_cold_open".into());
    }

    execute_action(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    )?;
    advance_to(&mut store, SHIFT_END + 60)?;
    if stock_quantity(&store, ELECTRICITY_STOCK)? != Quantity::new(400_000).map_err(err)?
        || stock_quantity(&store, WATER_STOCK)? != Quantity::new(540_000).map_err(err)?
    {
        return Err("day_zero_generator_batch_or_water_mismatch".into());
    }
    let resume_operation_id = construction_action_with_id(
        &mut store,
        Caller::Account(CONTRACTOR),
        ConstructionCommand::Resume,
    )?;
    contract_operation_ids.push(resume_operation_id);
    let resumed = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing")?;
    if resumed.phase != mycity_core::construction::Phase::Building
        || resumed.last_work_at.map(GameTime::value) != Some((SHIFT_END + 60) as i64)
    {
        return Err("resume_timestamp_not_recorded".into());
    }
    store = checkpoint_reopen(store, dir)?;
    let resumed_recovered = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing after resume recovery")?;
    if resumed_recovered.phase != mycity_core::construction::Phase::Building
        || resumed_recovered.last_work_at.map(GameTime::value) != Some((SHIFT_END + 60) as i64)
    {
        return Err("resume_lower_bound_lost_after_cold_open".into());
    }

    let resumed_view = store.engine().map_err(err)?.world().view();
    let resources_before_limited_retry = resource_balances(&store)?;
    let before_limited_retry = resumed_view.construction.ok_or("construction missing")?;
    let boot_before_limited_retry = resumed_view.boot.ok_or("boot missing")?;
    let canonical_effect_before_limited_retry = construction_effect_id(&store)?;
    let limited_retry = store
        .execute(
            Caller::Account(CONTRACTOR),
            Envelope {
                world: resumed_view.scope.world(),
                branch: resumed_view.scope.branch(),
                operation_id: CONSTRUCTION_WORK_LIMIT_OPERATION,
                expected_revision: resumed_view.revision,
                actions: vec![Action::Construction {
                    command: ConstructionCommand::Work {
                        at: GameTime::new((SHIFT_END + 60) as i64).map_err(err)?,
                        elapsed_seconds: 0,
                        worker_seconds: 28_800,
                    },
                }],
            },
        )
        .map_err(err)?;
    let work_limit_error = limited_retry
        .model
        .err()
        .ok_or("same_shift_work_succeeded")?;
    contract_operation_ids.push(CONSTRUCTION_WORK_LIMIT_OPERATION);
    if work_limit_error != TxError::Construction(mycity_core::construction::Error::WorkLimit)
        || !limited_retry.has_persistent_receipt
    {
        return Err(format!("same_shift_retry_wrong_error:{work_limit_error:?}"));
    }
    let after_limited_retry = store.engine().map_err(err)?.world().view();
    let project_after_limited_retry = after_limited_retry
        .construction
        .ok_or("construction missing")?;
    let boot_after_limited_retry = after_limited_retry.boot.ok_or("boot missing")?;
    if project_after_limited_retry != before_limited_retry
        || after_limited_retry.accounts != resumed_view.accounts
        || after_limited_retry.stocks != resumed_view.stocks
        || resource_balances(&store)? != resources_before_limited_retry
        || boot_after_limited_retry != boot_before_limited_retry
        || stock_quantity(&store, ELECTRICITY_STOCK)? != Quantity::new(400_000).map_err(err)?
        || stock_quantity(&store, WATER_STOCK)? != Quantity::new(540_000).map_err(err)?
        || stock_quantity(&store, RAW_STOCK)? != raw_before_failure
        || cash_total(&store)? != cash_before_failure
        || construction_effect_id(&store)? != canonical_effect_before_limited_retry
    {
        return Err("work_limit_retry_changed_project_or_resources".into());
    }

    advance_to(&mut store, DAY)?;
    let day_one_electricity_before = stock_quantity(&store, ELECTRICITY_STOCK)?;
    let (day_one, day_one_proof, day_one_payment, day_one_basis) =
        settle_three_day_market(dir, &mut store, 80_001, 1, 2)?;
    if day_one_electricity_before != Quantity::new(400_000).map_err(err)?
        || stock_quantity(&store, ELECTRICITY_STOCK)? > day_one_electricity_before
    {
        return Err("day_one_generator_batch_not_available_before_market".into());
    }
    let day_one_household_cash = account_cash(&store, HOUSEHOLD)?;
    let mut employment_operation_ids = vec![initial_payroll_operation_id];
    if employment == EmploymentScenario::Change {
        let before_switch = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline missing at employment switch")?;
        if before_switch.progress.last_complete != DAY
            || before_switch.progress.phase.is_some()
            || before_switch.progress.target.is_some()
        {
            return Err("employment_switch_not_at_completed_day_one_boundary".into());
        }
        let old_before = before_switch
            .payroll
            .iter()
            .find(|payroll| payroll.spec.id == 1)
            .copied()
            .ok_or("original_factory_payroll_missing")?;
        if old_before.paid != Money::new(80).map_err(err)?
            || old_before.accrued != old_before.paid
            || old_before.work_seconds != 28_800
        {
            return Err("day_one_original_wage_not_fully_settled".into());
        }
        let terminate_id = execute_action_with_id(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Terminate { id: 1 },
            },
        )?;
        employment_operation_ids.push(terminate_id);
        let after_terminate = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline missing after employment termination")?;
        let old_after = after_terminate
            .payroll
            .iter()
            .find(|payroll| payroll.spec.id == 1)
            .copied()
            .ok_or("terminated_factory_payroll_missing")?;
        if old_after.working
            || old_after.next_due.is_some()
            || old_after.spec.ends_at != DAY
            || old_after.paid != old_before.paid
            || old_after.accrued != old_after.paid
        {
            return Err("old_employer_payroll_not_closed_without_arrears".into());
        }
        let add_id = execute_action_with_id(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::AddPayroll {
                    spec: PayrollSpec {
                        id: 2,
                        payer: NEW_EMPLOYER,
                        payee: HOUSEHOLD,
                        home_city: 1,
                        hourly_cents: Money::new(1).map_err(err)?,
                        cycle: Cycle::Daily,
                        ends_at: 7 * DAY,
                    },
                },
            },
        )?;
        employment_operation_ids.push(add_id);
        let after_hire = store
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("timeline missing after employment start")?;
        let new_payroll = after_hire
            .payroll
            .iter()
            .find(|payroll| payroll.spec.id == 2)
            .copied()
            .ok_or("new_employer_payroll_missing")?;
        if new_payroll.spec.payer != NEW_EMPLOYER
            || new_payroll.spec.payee != HOUSEHOLD
            || new_payroll.spec.hourly_cents != Money::new(1).map_err(err)?
            || new_payroll.started != DAY
            || new_payroll.last_accrual != DAY
            || new_payroll.work_seconds != 0
        {
            return Err("new_employer_payroll_not_started_at_day_one_boundary".into());
        }
    }

    let electricity_after_day_one = stock_quantity(&store, ELECTRICITY_STOCK)?;
    execute_action(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    )?;
    advance_to(&mut store, DAY + 60)?;
    let electricity_before_work = stock_quantity(&store, ELECTRICITY_STOCK)?;
    if electricity_before_work.value()
        != electricity_after_day_one
            .value()
            .checked_add(400_000)
            .ok_or("electricity_overflow")?
        || stock_quantity(&store, WATER_STOCK)? != Quantity::new(480_000).map_err(err)?
    {
        return Err("day_one_second_generator_batch_mismatch".into());
    }
    let work_at = DAY + SHIFT_END;
    advance_to(&mut store, work_at)?;
    let successful_work_id = construction_effect_id(&store)?;
    let success_work_record_id = construction_action_with_id(
        &mut store,
        Caller::Account(CONTRACTOR),
        ConstructionCommand::Work {
            at: GameTime::new(work_at as i64).map_err(err)?,
            elapsed_seconds: (DAY - 60) as u32,
            worker_seconds: 28_800,
        },
    )?;
    if success_work_record_id != successful_work_id {
        return Err("successful_work_operation_id_mismatch".into());
    }
    contract_operation_ids.push(success_work_record_id);
    let project_after_work = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing")?;
    if project_after_work.completed_worker_seconds != 28_800
        || project_after_work.utility_consumed != [Quantity::new(400_000).map_err(err)?; 2]
        || project_after_work.utility_charged
            != [
                Money::new(8_000).map_err(err)?,
                Money::new(4_000).map_err(err)?,
            ]
        || stock_quantity(&store, ELECTRICITY_STOCK)?.value()
            != electricity_before_work.value() - 400_000
        || stock_quantity(&store, WATER_STOCK)? != Quantity::new(80_000).map_err(err)?
    {
        return Err("day_one_full_shift_work_or_utility_meter_mismatch".into());
    }
    let submit_inspection_id = construction_action_with_id(
        &mut store,
        Caller::Account(CONTRACTOR),
        ConstructionCommand::SubmitInspection,
    )?;
    contract_operation_ids.push(submit_inspection_id);

    advance_to(&mut store, 2 * DAY)?;
    let electricity_before_day_two = stock_quantity(&store, ELECTRICITY_STOCK)?;
    let (day_two, day_two_proof, day_two_payment, day_two_basis) =
        settle_three_day_market(dir, &mut store, 80_002, 2, 2)?;
    let day_two_household_cash = account_cash(&store, HOUSEHOLD)?;
    if stock_quantity(&store, ELECTRICITY_STOCK)? > electricity_before_day_two {
        return Err("day_two_settlement_created_electricity".into());
    }
    if !hold_road_inspection {
        let accept_inspection_id = construction_action_with_id(
            &mut store,
            Caller::Account(PERMIT_AUTHORITY),
            ConstructionCommand::AcceptInspection {
                inspector: PERMIT_AUTHORITY,
                accepted: true,
            },
        )?;
        contract_operation_ids.push(accept_inspection_id);
    }
    let published = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing")?;
    let expected_day_three_capacity = if hold_road_inspection { 2 } else { 10 };
    if published.operating_published_at.map(GameTime::value)
        != (!hold_road_inspection).then_some((2 * DAY) as i64)
        || store
            .engine()
            .map_err(err)?
            .resources()
            .iter()
            .find(|resource| resource.id == ROAD_RESOURCE)
            .ok_or("road capacity resource missing")?
            .capacity
            != expected_day_three_capacity
    {
        return Err("inspection_not_published_at_day_two_boundary".into());
    }

    let electricity_before_final_batch = stock_quantity(&store, ELECTRICITY_STOCK)?;
    execute_action(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    )?;
    advance_to(&mut store, 2 * DAY + 60)?;
    if stock_quantity(&store, ELECTRICITY_STOCK)?.value()
        != electricity_before_final_batch
            .value()
            .checked_add(400_000)
            .ok_or("electricity_overflow")?
        || stock_quantity(&store, WATER_STOCK)? != Quantity::new(20_000).map_err(err)?
    {
        return Err("finite_generator_water_remainder_mismatch".into());
    }
    advance_to(&mut store, 3 * DAY)?;
    let day_three_cash_before_market = account_cash(&store, HOUSEHOLD)?;
    let day_three_stock_before_market = stock_quantity(&store, PRODUCT_STOCK)?;
    let (day_three, day_three_proof, day_three_payment, day_three_basis) =
        settle_three_day_market(dir, &mut store, 80_003, 3, expected_day_three_capacity)?;
    let day_three_household_cash = account_cash(&store, HOUSEHOLD)?;
    // Real transport resolution replaces the opening quote's zero freight with a route
    // charge. Day three has no new production or utility fees in this fixed experiment,
    // so its goods and freight payments are both paid by the single household. Do not
    // attribute construction/Boot producer charges to household consumption.
    if day_three.produced != Quantity::ZERO || day_three.utilities != Money::ZERO {
        return Err("day_three_fixture_has_mixed_producer_costs".into());
    }
    let day_three_household_market_payment = day_three
        .goods_payments
        .checked_add(day_three.freight)
        .map_err(err)?;
    let day_three_available_stock = day_three_stock_before_market
        .checked_add(day_three.produced)
        .map_err(err)?;
    if day_three_household_market_payment.value() > day_three_cash_before_market.value()
        || day_three_household_cash.value()
            != day_three_cash_before_market
                .value()
                .checked_sub(day_three_household_market_payment.value())
                .ok_or("day_three_household_cash_underflow")?
        || day_three.actual_sales > day_three_available_stock
    {
        return Err("day_three_purchase_or_stock_conservation_mismatch".into());
    }
    if employment == EmploymentScenario::Change && day_three.actual_sales >= day_three.market_demand
    {
        return Err("day_three_low_wage_did_not_constrain_purchasing_power".into());
    }

    let expected_utility_quantities = [Quantity::new(400_000).map_err(err)?; 2];
    let expected_utility_fees = [
        Money::new(8_000).map_err(err)?,
        Money::new(4_000).map_err(err)?,
    ];
    let total_produced = day_one
        .produced
        .checked_add(day_two.produced)
        .and_then(|value| value.checked_add(day_three.produced))
        .map_err(err)?;
    let total_sales = day_one
        .actual_sales
        .checked_add(day_two.actual_sales)
        .and_then(|value| value.checked_add(day_three.actual_sales))
        .map_err(err)?;
    let local_export = total_produced.value() * 1_000;
    let expected_electricity_remaining = 1_200_000i64
        .checked_sub(400_000)
        .and_then(|value| value.checked_sub(local_export))
        .ok_or("electricity_balance_underflow")?;
    let project_state = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction missing")?;
    let boot = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .boot
        .ok_or("boot missing")?;
    let local_basis_used = day_one_basis
        .checked_add(day_two_basis)
        .and_then(|value| value.checked_add(day_three_basis))
        .map_err(err)?;
    let market_payments = day_one_payment
        .checked_add(day_two_payment)
        .and_then(|value| value.checked_add(day_three_payment))
        .map_err(err)?;
    if project_state.materials[0].consumed != Quantity::new(500).map_err(err)?
        || project_state.utility_consumed != expected_utility_quantities
        || project_state.utility_charged != expected_utility_fees
        || boot.local_construction_exported != expected_utility_quantities
        || boot.local_electricity_exported.value() != local_export
        || boot.exports.first().map(|quantity| quantity.value()) != Some(local_export + 400_000)
        || boot.local_electricity_basis != local_basis_used
        || stock_quantity(&store, ELECTRICITY_STOCK)?.value() != expected_electricity_remaining
        || cash_total(&store)? != initial_cash
        || stock_quantity(&store, RAW_STOCK)?.value() + 500 + total_produced.value()
            != initial_raw.value()
        || stock_quantity(&store, PRODUCT_STOCK)?.value()
            != total_produced.value() - total_sales.value()
    {
        return Err("three_day_physical_or_cash_conservation_mismatch".into());
    }

    let timeline = store
        .engine()
        .map_err(err)?
        .timeline()
        .ok_or("timeline missing")?;
    let payrolls = timeline.payroll.iter().copied().collect::<Vec<_>>();
    let payroll_occurrences = payrolls.iter().try_fold(0u64, |sum, payroll| {
        sum.checked_add(payroll.occurrences)
            .ok_or_else(|| "payroll_occurrence_overflow".to_owned())
    })?;
    let payroll_paid = payrolls.iter().try_fold(Money::ZERO, |sum, payroll| {
        sum.checked_add(payroll.paid).map_err(err)
    })?;
    let old_factory_payroll = payrolls
        .iter()
        .find(|payroll| payroll.spec.id == 1)
        .copied()
        .ok_or("original_factory_payroll_missing_after_run")?;
    let new_employer_payroll = payrolls
        .iter()
        .find(|payroll| payroll.spec.id == 2)
        .copied();
    match employment {
        EmploymentScenario::None | EmploymentScenario::Control => {
            if new_employer_payroll.is_some()
                || old_factory_payroll.occurrences != 3
                || old_factory_payroll.paid != Money::new(240).map_err(err)?
                || old_factory_payroll.work_seconds != 86_400
                || old_factory_payroll.spec.payer != COMPANY
            {
                return Err("control_employment_payroll_mismatch".into());
            }
        }
        EmploymentScenario::Change => {
            let new_payroll = new_employer_payroll.ok_or("new_employer_payroll_missing")?;
            if old_factory_payroll.occurrences != 1
                || old_factory_payroll.paid != Money::new(80).map_err(err)?
                || old_factory_payroll.work_seconds != 28_800
                || old_factory_payroll.spec.ends_at != DAY
                || new_payroll.occurrences != 2
                || new_payroll.paid != Money::new(16).map_err(err)?
                || new_payroll.work_seconds != 57_600
                || new_payroll.spec.payer != NEW_EMPLOYER
            {
                return Err("employment_change_payroll_or_no_overlap_mismatch".into());
            }
        }
    }
    let contractually_paid = payroll_paid;
    if employment.enabled() && cash_total(&store)? != initial_cash {
        return Err("employment_mode_opening_or_cash_conservation_mismatch".into());
    }
    let new_employer_cash = if employment.enabled() {
        account_cash(&store, NEW_EMPLOYER)?
    } else {
        Money::ZERO
    };
    match employment {
        EmploymentScenario::None => {}
        EmploymentScenario::Control if new_employer_cash != Money::new(1_000).map_err(err)? => {
            return Err("control_new_employer_capital_changed_without_payroll".into());
        }
        EmploymentScenario::Change if new_employer_cash != Money::new(984).map_err(err)? => {
            return Err("new_employer_payroll_not_paid_from_its_own_account".into());
        }
        _ => {}
    }

    let before_cold_hash = store.state_hash().map_err(err)?;
    store.checkpoint().map_err(err)?;
    drop(store);
    let mut recovered = Store::open(dir, RULES).map_err(err)?;
    let after_cold_hash = recovered.state_hash().map_err(err)?;
    if after_cold_hash != before_cold_hash {
        return Err("three_day_final_cold_open_hash_mismatch".into());
    }
    let expected_market = [
        (80_001, day_one_proof, day_one),
        (80_002, day_two_proof, day_two),
        (80_003, day_three_proof, day_three),
    ];
    for (operation_id, expected_proof, _) in expected_market {
        let receipt = recovered
            .receipt(operation_id)
            .map_err(err)?
            .ok_or("historical market receipt missing after recovery")?;
        let actual = receipt
            .actions
            .iter()
            .flatten()
            .find_map(|action| match action {
                Action::TownTransportSettle { proof_sha256, .. } => Some(*proof_sha256),
                _ => None,
            });
        if actual != Some(expected_proof) {
            return Err(format!(
                "market_proof_changed_after_recovery:{operation_id}"
            ));
        }
    }
    let shortage_record = recovered
        .receipt(CONSTRUCTION_SHORTAGE_OPERATION)
        .map_err(err)?
        .ok_or("day_zero_shortage_receipt_missing_after_recovery")?;
    let work_limit_record = recovered
        .receipt(CONSTRUCTION_WORK_LIMIT_OPERATION)
        .map_err(err)?
        .ok_or("same_shift_work_limit_receipt_missing_after_recovery")?;
    let success_record = recovered
        .receipt(successful_work_id)
        .map_err(err)?
        .ok_or("successful_work_receipt_missing_after_recovery")?;
    if shortage_record.outcome != Err(TxError::Boot(mycity_core::boot::Error::Input))
        || work_limit_record.outcome
            != Err(TxError::Construction(
                mycity_core::construction::Error::WorkLimit,
            ))
        || success_record.outcome.is_err()
    {
        return Err("construction_failure_or_success_receipt_changed_after_recovery".into());
    }
    let sequence_before_replay = recovered.committed_sequence();
    if !contract_operation_ids.contains(&CONSTRUCTION_SHORTAGE_OPERATION)
        || !contract_operation_ids.contains(&CONSTRUCTION_WORK_LIMIT_OPERATION)
        || !contract_operation_ids.contains(&successful_work_id)
    {
        return Err("construction_replay_set_incomplete".into());
    }
    for operation_id in contract_operation_ids {
        let record = recovered
            .receipt(operation_id)
            .map_err(err)?
            .ok_or("construction_receipt_missing_after_recovery")?;
        replay_record_exact(&mut recovered, record)?;
        if recovered.committed_sequence() != sequence_before_replay
            || recovered.state_hash().map_err(err)? != after_cold_hash
        {
            return Err(format!("construction_replay_changed_state:{operation_id}"));
        }
    }
    for (operation_id, _, expected_summary) in expected_market {
        let replay = recovered.settle_town_market(operation_id).map_err(err)?;
        if !replay.has_persistent_receipt
            || replay.model.map_err(err)?.town != Some(expected_summary)
            || recovered.committed_sequence() != sequence_before_replay
            || recovered.state_hash().map_err(err)? != after_cold_hash
        {
            return Err(format!(
                "three_day_market_replay_changed_state:{operation_id}"
            ));
        }
    }
    let mut employment_records = Vec::with_capacity(employment_operation_ids.len());
    for operation_id in employment_operation_ids {
        let record = recovered
            .receipt(operation_id)
            .map_err(err)?
            .ok_or("employment_payroll_receipt_missing_after_recovery")?;
        if record.outcome.is_err() {
            return Err(format!("employment_payroll_receipt_failed:{operation_id}"));
        }
        employment_records.push(record);
    }
    for record in employment_records {
        let operation_id = record.operation_id;
        replay_record_exact(&mut recovered, record)?;
        if recovered.committed_sequence() != sequence_before_replay
            || recovered.state_hash().map_err(err)? != after_cold_hash
        {
            return Err(format!(
                "employment_payroll_replay_changed_state:{operation_id}"
            ));
        }
    }
    if recovered.committed_sequence() != sequence_before_replay
        || recovered.state_hash().map_err(err)? != after_cold_hash
    {
        return Err("three_day_construction_replay_changed_state".into());
    }

    println!(
        "mode={} employment={} day0_shortage={:?} pause_checkpoint=true resume_checkpoint=true immediate_work_limit={:?} day1_sales={} day2_sales={} day3_sales={} day1_household_cash_after_market={} day2_household_cash_after_market={} day3_household_cash_before_market={} day3_household_cash_after_market={} day3_market_spend={} day3_goods_payment={} day3_freight={} day3_utilities={} day3_stock_available={} day1_produced={} day2_produced={} day3_produced={} day1_capacity=2 day2_capacity=2 day3_capacity={} inspection_held={} day1_proof_sha256={} day2_proof_sha256={} day3_proof_sha256={} construction_elapsed_seconds={} construction_worker_seconds=28800 electricity_used={} water_used={} electricity_fee={} water_fee={} generator_batches=3 electricity_remaining={} water_remaining={} local_electricity_export={} local_electricity_basis={} local_construction_electricity={} local_construction_water={} payroll_occurrences={} payroll_paid={} old_factory_paid={} new_employer_paid={} old_factory_work_seconds={} new_employer_work_seconds={} new_employer_cash_final={} new_employer_has_production_recipe=false employment_operation_replay_exact=true market_payment_total={} cash_conserved=true pause_resume_durable=true final_cold_open_equal=true all_contract_payroll_and_market_receipts_replay_exact=true raw_initial={} raw_remaining={} state_hash={}",
        if hold_road_inspection { "construction-utilities-three-day-held" } else { "construction-utilities-three-day" },
        employment.label(),
        shortage_error,
        work_limit_error,
        day_one.actual_sales.value(),
        day_two.actual_sales.value(),
        day_three.actual_sales.value(),
        day_one_household_cash.value(),
        day_two_household_cash.value(),
        day_three_cash_before_market.value(),
        day_three_household_cash.value(),
        day_three_household_market_payment.value(),
        day_three.goods_payments.value(),
        day_three.freight.value(),
        day_three.utilities.value(),
        day_three_available_stock.value(),
        day_one.produced.value(),
        day_two.produced.value(),
        day_three.produced.value(),
        expected_day_three_capacity,
        hold_road_inspection,
        hex(&day_one_proof),
        hex(&day_two_proof),
        hex(&day_three_proof),
        DAY - 60,
        project_state.utility_consumed[0].value(),
        project_state.utility_consumed[1].value(),
        project_state.utility_charged[0].value(),
        project_state.utility_charged[1].value(),
        stock_quantity(&recovered, ELECTRICITY_STOCK)?.value(),
        stock_quantity(&recovered, WATER_STOCK)?.value(),
        boot.local_electricity_exported.value(),
        boot.local_electricity_basis.value(),
        boot.local_construction_exported[0].value(),
        boot.local_construction_exported[1].value(),
        payroll_occurrences,
        contractually_paid.value(),
        old_factory_payroll.paid.value(),
        new_employer_payroll.map_or(0, |payroll| payroll.paid.value()),
        old_factory_payroll.work_seconds,
        new_employer_payroll.map_or(0, |payroll| payroll.work_seconds),
        new_employer_cash.value(),
        market_payments.value(),
        initial_raw.value(),
        stock_quantity(&recovered, RAW_STOCK)?.value(),
        after_cold_hash,
    );
    println!("scope=store_road_construction_boot_town fixed_od=true construction_labor=explicit_input days=3 employment_assignment=trusted_fixture_input endogenous_job_search=not_implemented legal_employment_approval=not_modeled new_employer_production_recipe=none automatic_outage_pause=not_implemented resume_supply_interlock=not_implemented continuous_supply=not_implemented steady_state=not_demonstrated balance=not_calibrated");
    Ok(())
}

fn cash_total(store: &Store) -> Result<i128, String> {
    Ok(store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum())
}

fn account_cash(store: &Store, account_id: u32) -> Result<Money, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .find(|account| account.id == account_id)
        .map(|account| account.cash)
        .ok_or_else(|| format!("account_missing:{account_id}"))
}

fn stock_quantity(store: &Store, stock_id: u32) -> Result<Quantity, String> {
    store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == stock_id)
        .map(|stock| stock.quantity)
        .ok_or_else(|| format!("stock_missing:{stock_id}"))
}

fn run(dir: &Path, options: Options) -> Result<(), String> {
    if dir.exists() {
        return Err("store directory already exists; use a fresh --store-dir".into());
    }
    let opening = opening();
    let profile_bytes = allocation_profile(opening)?;
    let construction_utilities = options.construction_utilities || options.employment.enabled();
    let project = project(construction_utilities)?;
    let opening_bytes = opening_bytes(hash(&profile_bytes), &project, options.employment)?;
    let mut store = Store::create_spatial_with_transport_allocation_profile(
        dir,
        &opening_bytes,
        resources()?,
        RULES,
        (
            include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
            include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
        ),
        &profile_bytes,
    )
    .map_err(err)?;
    let initial_cash = cash_total(&store)?;
    let initial_raw = stock_quantity(&store, RAW_STOCK)?;
    if options.employment.enabled()
        && (account_cash(&store, HOUSEHOLD)? != Money::ZERO
            || account_cash(&store, NEW_EMPLOYER)? != Money::new(1_000).map_err(err)?)
    {
        return Err("employment_fixture_source_capital_mismatch".into());
    }

    let mut contract_operation_ids = Vec::new();
    for (caller, command) in [
        (Caller::Account(COMPANY), ConstructionCommand::File),
        (
            Caller::Account(HOUSEHOLD),
            ConstructionCommand::DecideGate {
                gate_id: 1,
                decision: GateDecision::Accepted,
                evidence_id: 5101,
            },
        ),
        (
            Caller::Account(PERMIT_AUTHORITY),
            ConstructionCommand::DecideGate {
                gate_id: 2,
                decision: GateDecision::Accepted,
                evidence_id: 5102,
            },
        ),
        (Caller::Account(COMPANY), ConstructionCommand::Prepare),
        (Caller::Account(COMPANY), ConstructionCommand::Start),
    ] {
        contract_operation_ids.push(construction_action_with_id(&mut store, caller, command)?);
    }

    execute_action(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    )?;
    execute_action(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    let initial_payroll_operation_id = store.committed_sequence() + 1;
    execute_action(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: COMPANY,
                    payee: HOUSEHOLD,
                    home_city: 1,
                    hourly_cents: Money::new(10).map_err(err)?,
                    cycle: Cycle::Daily,
                    ends_at: 7 * DAY,
                },
            },
        },
    )?;
    advance_to(&mut store, SHIFT_END)?;
    if construction_utilities {
        return run_construction_utilities_three_days(
            dir,
            store,
            initial_cash,
            initial_raw,
            options,
            initial_payroll_operation_id,
            contract_operation_ids,
        );
    }
    construction_action(
        &mut store,
        Caller::Account(CONTRACTOR),
        ConstructionCommand::Work {
            at: GameTime::new(SHIFT_END as i64).map_err(err)?,
            elapsed_seconds: SHIFT_END as u32,
            worker_seconds: 28_800,
        },
    )?;
    construction_action(
        &mut store,
        Caller::Account(CONTRACTOR),
        ConstructionCommand::SubmitInspection,
    )?;
    advance_to(&mut store, DAY)?;

    execute_action(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    )?;
    let day_one_electricity_before_settlement = stock_quantity(&store, ELECTRICITY_STOCK)?;
    if day_one_electricity_before_settlement != Quantity::ZERO {
        return Err(format!(
            "day-one-electricity-present-before-settlement:{day_one_electricity_before_settlement:?}"
        ));
    }
    let day_one = store
        .settle_town_market(80_001)
        .map_err(err)?
        .model
        .map_err(err)?
        .town
        .ok_or("day-one market summary missing")?;
    if day_one.produced != Quantity::ZERO
        || day_one.actual_sales != Quantity::ZERO
        || stock_quantity(&store, ELECTRICITY_STOCK)? != Quantity::ZERO
    {
        return Err("day-one-market-used-unfinished-generator-output".into());
    }
    let day_one_proof = verify_proof(dir, &store, 1, 2)?;
    if store
        .engine()
        .map_err(err)?
        .resources()
        .iter()
        .find(|resource| resource.id == ROAD_RESOURCE)
        .ok_or("road capacity resource missing")?
        .capacity
        != 2
    {
        return Err("road capacity published before inspection acceptance".into());
    }

    if !options.hold_road_inspection {
        construction_action(
            &mut store,
            Caller::Account(PERMIT_AUTHORITY),
            ConstructionCommand::AcceptInspection {
                inspector: PERMIT_AUTHORITY,
                accepted: true,
            },
        )?;
    }
    let expected_capacity_after_inspection = if options.hold_road_inspection { 2 } else { 10 };
    let capacity_after_inspection = store
        .engine()
        .map_err(err)?
        .resources()
        .iter()
        .find(|resource| resource.id == ROAD_RESOURCE)
        .ok_or("road capacity resource missing")?
        .capacity;
    let published_at_after_inspection = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("construction project missing")?
        .operating_published_at;
    if capacity_after_inspection != expected_capacity_after_inspection
        || published_at_after_inspection.map(GameTime::value)
            != (!options.hold_road_inspection).then_some(DAY as i64)
    {
        return Err(format!(
            "inspection_publication_mismatch:capacity={capacity_after_inspection}:published={published_at_after_inspection:?}"
        ));
    }
    if options.generator_failure {
        execute_action(
            &mut store,
            Caller::System,
            Action::Boot {
                command: BootCommand::Fault {
                    facility: Facility::Generator,
                },
            },
        )?;
    }
    advance_to(&mut store, DAY + 60)?;
    let completed_generator_output = stock_quantity(&store, ELECTRICITY_STOCK)?;
    let expected_generator_output = if options.generator_failure {
        Quantity::ZERO
    } else {
        Quantity::new(400_000).map_err(err)?
    };
    if completed_generator_output != expected_generator_output {
        return Err(format!(
            "generator_output_mismatch:{completed_generator_output:?}"
        ));
    }
    advance_to(&mut store, 2 * DAY)?;
    let operator_before_market = account_cash(&store, OPERATOR)?;
    let electricity_basis_before = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .stocks
        .iter()
        .find(|stock| stock.id == ELECTRICITY_STOCK)
        .map(|stock| stock.basis)
        .ok_or("electricity stock missing")?;
    let day_two = store
        .settle_town_market(80_002)
        .map_err(err)?
        .model
        .map_err(err)?
        .town
        .ok_or("day-two market summary missing")?;
    let operator_after_market = account_cash(&store, OPERATOR)?;
    let utility_payment = operator_after_market
        .checked_sub(operator_before_market)
        .map_err(err)?;

    let expected_sales = if options.generator_failure {
        0
    } else if options.hold_road_inspection {
        20
    } else {
        100
    };
    let expected_produced = if options.generator_failure { 0 } else { 100 };
    let expected_material_remaining = 1_000 - 500 - expected_produced;
    if day_two.actual_sales.value() != expected_sales
        || day_two.produced.value() != expected_produced
        || stock_quantity(&store, RAW_STOCK)?.value() != expected_material_remaining
        || stock_quantity(&store, PRODUCT_STOCK)?.value() != expected_produced - expected_sales
    {
        return Err(format!(
            "day-two-model-mismatch:produced={}:sales={}:raw={}",
            day_two.produced.value(),
            day_two.actual_sales.value(),
            stock_quantity(&store, RAW_STOCK)?.value()
        ));
    }
    let day_two_proof = verify_proof(
        dir,
        &store,
        2,
        if options.hold_road_inspection { 2 } else { 10 },
    )?;

    let view = store.engine().map_err(err)?.world().view();
    let boot = view.boot.ok_or("boot missing")?;
    let electricity_basis_after = view
        .stocks
        .iter()
        .find(|stock| stock.id == ELECTRICITY_STOCK)
        .map(|stock| stock.basis)
        .ok_or("electricity stock missing")?;
    let project_state = view.construction.ok_or("construction missing")?;
    let expected_utility_quantities = [Quantity::ZERO; 2];
    let expected_utility_fees = [Money::ZERO; 2];
    if project_state.utility_consumed != expected_utility_quantities
        || project_state.utility_charged != expected_utility_fees
        || boot.local_construction_exported != expected_utility_quantities
    {
        return Err(format!(
            "construction_utility_ledger_mismatch:consumed={:?}:charged={:?}:boot={:?}",
            project_state.utility_consumed,
            project_state.utility_charged,
            boot.local_construction_exported
        ));
    }
    let resource_capacity = store
        .engine()
        .map_err(err)?
        .resources()
        .iter()
        .find(|resource| resource.id == ROAD_RESOURCE)
        .ok_or("road capacity resource missing")?
        .capacity;
    let project_material_used = project_state.materials[0].consumed.value();
    let local_export = boot.local_electricity_exported.value();
    let expected_local_export = expected_produced * 1_000;
    let expected_utility_payment = expected_produced;
    let remaining_electricity = stock_quantity(&store, ELECTRICITY_STOCK)?.value();
    if project_material_used != 500
        || local_export != expected_local_export
        || boot.exports.first().map(|quantity| quantity.value())
            != Some(expected_local_export + expected_utility_quantities[0].value())
        || boot.local_electricity_basis
            != electricity_basis_before
                .checked_sub(electricity_basis_after)
                .map_err(err)?
        || remaining_electricity != expected_generator_output.value() - expected_local_export
        || utility_payment.value() != expected_utility_payment
    {
        return Err(format!(
            "physical_accounting_mismatch:construction={project_material_used}:local_electricity={local_export}:total_electricity={:?}:basis={}:payment={}:remaining_electricity={remaining_electricity}",
            boot.exports.first(),
            boot.local_electricity_basis.value(),
            utility_payment.value()
        ));
    }
    let final_cash = cash_total(&store)?;
    if final_cash != initial_cash {
        return Err(format!("cash_conservation:{initial_cash}:{final_cash}"));
    }
    let before_cold_hash = store.state_hash().map_err(err)?;
    store.checkpoint().map_err(err)?;
    drop(store);
    let mut recovered = Store::open(dir, RULES).map_err(err)?;
    let after_cold_hash = recovered.state_hash().map_err(err)?;
    if after_cold_hash != before_cold_hash {
        return Err("cold_open_state_hash_mismatch".into());
    }
    let recovered_day_two_proof = recovered
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .ok_or("recovered town missing")?
        .state
        .last_transport
        .ok_or("recovered transport receipt missing")?
        .issued_proof_sha256;
    if recovered_day_two_proof != day_two_proof {
        return Err("cold_open_transport_proof_mismatch".into());
    }
    let construction_shortage_receipt_replay_exact = false;
    for (operation_id, expected) in [(80_001, day_one_proof), (80_002, day_two_proof)] {
        let receipt = recovered
            .receipt(operation_id)
            .map_err(err)?
            .ok_or("historical market receipt missing")?;
        let actual = receipt
            .actions
            .iter()
            .flatten()
            .find_map(|action| match action {
                Action::TownTransportSettle { proof_sha256, .. } => Some(*proof_sha256),
                _ => None,
            });
        if actual != Some(expected) {
            return Err("historical market proof changed after checkpoint".into());
        }
    }

    // Retry both historical service days after checkpoint recovery. Day one must
    // retain its pre-publication road and unfinished-generator result even though
    // the current world has moved on to day two.
    let sequence_before_retries = recovered.committed_sequence();
    for (operation_id, expected_summary) in [(80_001, day_one), (80_002, day_two)] {
        let replay = recovered.settle_town_market(operation_id).map_err(err)?;
        if !replay.has_persistent_receipt
            || replay.model.map_err(err)?.town != Some(expected_summary)
            || recovered.committed_sequence() != sequence_before_retries
            || recovered.state_hash().map_err(err)? != after_cold_hash
        {
            return Err(format!("cold_market_retry_changed_state:{operation_id}"));
        }
    }

    println!(
        "mode={} fixture=core-town-utility-opening-v1 fixed_od_sources=101,102 outbound_edge=10 return_edge=11 trip_limit=10 goods_per_trip=10 construction_work_elapsed_seconds={} construction_work_worker_seconds={} day1_electricity_stock_at_settlement={} day1_produced={} day1_sales={} day1_proof_sha256={} day1_outbound_capacity=2 inspection_held={} construction_utilities={} construction_utility_failure_retry=not_applicable construction_failure_retry_exact_replay={} construction_electricity_used={} construction_water_used={} construction_electricity_charged={} construction_water_charged={} generator_starts_before_day1_settlement={} construction_generator_batch_before_day1={} construction_material_initial={} construction_material_consumed={} raw_initial={} raw_after_day2={} factory_raw_used={} day2_generator_electricity={} day2_produced={} day2_sales={} day2_proof_sha256={} road_capacity_after_inspection={} road_published_at={:?} boot_electricity_export_total={} boot_electricity_export_local={} boot_electricity_export_cross_city={} boot_electricity_basis_total={} boot_local_electricity_basis={} operator_utility_payment={} construction_owner_cash_final={} household_cash_final={} cash_initial_total={} cash_final_total={} cash_conserved={} cold_open_state_hash_equal={} cold_open_day2_proof_equal={} historical_market_retries_exact=true state_hash={}",
        if options.construction_utilities { "construction-utilities" } else if options.generator_failure { "generator-failure" } else if options.hold_road_inspection { "hold-road-inspection" } else { "normal" },
        SHIFT_END + if options.construction_utilities { 60 } else { 0 },
        28_800,
        day_one_electricity_before_settlement.value(),
        day_one.produced.value(),
        day_one.actual_sales.value(),
        hex(&day_one_proof),
        options.hold_road_inspection,
        options.construction_utilities,
        construction_shortage_receipt_replay_exact,
        project_state.utility_consumed[0].value(),
        project_state.utility_consumed[1].value(),
        project_state.utility_charged[0].value(),
        project_state.utility_charged[1].value(),
        if options.construction_utilities { 2 } else { 1 },
        if options.construction_utilities { 400_000 } else { 0 },
        initial_raw.value(),
        project_material_used,
        initial_raw.value(),
        stock_quantity(&recovered, RAW_STOCK)?.value(),
        initial_raw.value() - 500 - stock_quantity(&recovered, RAW_STOCK)?.value(),
        completed_generator_output.value(),
        day_two.produced.value(),
        day_two.actual_sales.value(),
        hex(&day_two_proof),
        resource_capacity,
        project_state.operating_published_at.map(|at| at.value()),
        boot.exports[0].value(),
        local_export,
        boot.exports[0].value() - local_export - boot.local_construction_exported[0].value(),
        boot.exported_basis.value(),
        boot.local_electricity_basis.value(),
        utility_payment.value(),
        account_cash(&recovered, COMPANY)?.value(),
        account_cash(&recovered, HOUSEHOLD)?.value(),
        initial_cash,
        final_cash,
        final_cash == initial_cash,
        after_cold_hash == before_cold_hash,
        recovered_day_two_proof == day_two_proof,
        after_cold_hash,
    );
    println!("scope=store_road_construction_boot_town fixed_od=true construction_labor=explicit_input construction_utilities={} utility=boot_electricity_and_water_buffers days=2 self_funding=not_demonstrated balance=not_calibrated", options.construction_utilities);
    Ok(())
}

fn parse_args() -> Result<(PathBuf, Options), String> {
    let mut store_dir = None;
    let mut options = Options {
        hold_road_inspection: false,
        generator_failure: false,
        construction_utilities: false,
        employment: EmploymentScenario::None,
    };
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--hold-road-inspection" => options.hold_road_inspection = true,
            "--generator-failure" => options.generator_failure = true,
            "--construction-utilities" => options.construction_utilities = true,
            "--employment-control" => {
                if options.employment != EmploymentScenario::None {
                    return Err("choose only one employment scenario".into());
                }
                options.employment = EmploymentScenario::Control;
            }
            "--employment-change" => {
                if options.employment != EmploymentScenario::None {
                    return Err("choose only one employment scenario".into());
                }
                options.employment = EmploymentScenario::Change;
            }
            "--store-dir" => store_dir = args.next().map(PathBuf::from),
            _ => return Err(format!("usage: town_lifelines --store-dir PATH [--hold-road-inspection] [--generator-failure] [--construction-utilities] [--employment-control|--employment-change] (unknown:{argument})")),
        }
    }
    if options.hold_road_inspection && options.generator_failure {
        return Err(
            "choose one scenario mode: --hold-road-inspection or --generator-failure".into(),
        );
    }
    if options.construction_utilities && options.generator_failure {
        return Err("--construction-utilities requires a successful generator".into());
    }
    if options.employment != EmploymentScenario::None && options.generator_failure {
        return Err("employment scenarios require a successful generator".into());
    }
    if options.employment.enabled() && options.hold_road_inspection {
        return Err(
            "employment comparison requires the day-three road-capacity publication".into(),
        );
    }
    Ok((
        store_dir.ok_or("missing_required_argument:--store-dir")?,
        options,
    ))
}

fn main() -> Result<(), String> {
    let (store_dir, options) = parse_args()?;
    run(&store_dir, options)
}
