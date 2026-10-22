//! Store-backed joint road, construction, Boot utility, and Town market scenario.
//!
//! Fixed aggregate OD cohorts and one construction work-shift are explicit interface
//! inputs, not independent person or traffic simulations. The real Store owns the
//! allocation profile and daily proofs. Pass a fresh --store-dir for each run; the
//! completed directory is retained so the cold-open receipt remains inspectable.
use mycity_core::{
    boot::{Command as BootCommand, Facility},
    construction::{
        EngineCommand as ConstructionCommand, Gate, GateCategory, GateDecision, Kind, Material,
        Project, OPERATION_PREFIX,
    },
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, RoadCapacityEffectBinding,
        TransportAllocationProfile, TripPartition,
    },
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope, Resource},
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
const PRODUCT_STOCK: u32 = 16;
const SITE_RESOURCE: u32 = 80;
const ROAD_RESOURCE: u32 = 81;
const SHIFT_END: u64 = 17 * 3_600;

#[derive(Clone, Copy)]
struct Options {
    hold_road_inspection: bool,
    generator_failure: bool,
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

fn project() -> Result<Project, String> {
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
    Ok(project)
}

fn opening_bytes(profile_sha256: [u8; 32], project: &Project) -> Result<Vec<u8>, String> {
    let mut value: Value = serde_json::from_slice(FIXTURE).map_err(err)?;
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
    let view = store.engine().map_err(err)?.world().view();
    let outcome = store
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
    outcome.model.map(|_| ()).map_err(err)
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
    outcome.model.map(|_| ()).map_err(err)
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
    let project = project()?;
    let opening_bytes = opening_bytes(hash(&profile_bytes), &project)?;
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
        construction_action(&mut store, caller, command)?;
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
        || boot.exports.first().map(|quantity| quantity.value()) != Some(expected_local_export)
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
        "mode={} fixture=core-town-utility-opening-v1 fixed_od_sources=101,102 outbound_edge=10 return_edge=11 trip_limit=10 goods_per_trip=10 fixed_construction_elapsed_seconds={} fixed_construction_worker_seconds={} day1_produced={} day1_sales={} day1_proof_sha256={} day1_outbound_capacity=2 inspection_held={} construction_material_initial={} construction_material_consumed={} raw_initial={} raw_after_day2={} factory_raw_used={} day2_generator_electricity={} day2_produced={} day2_sales={} day2_proof_sha256={} road_capacity_after_inspection={} road_published_at={:?} boot_electricity_export_total={} boot_electricity_export_local={} boot_electricity_export_cross_city={} boot_electricity_basis_total={} boot_local_electricity_basis={} operator_utility_payment={} construction_owner_cash_final={} household_cash_final={} cash_initial_total={} cash_final_total={} cash_conserved={} cold_open_state_hash_equal={} cold_open_day2_proof_equal={} historical_market_retries_exact=true state_hash={}",
        if options.generator_failure { "generator-failure" } else if options.hold_road_inspection { "hold-road-inspection" } else { "normal" },
        SHIFT_END,
        28_800,
        day_one.produced.value(),
        day_one.actual_sales.value(),
        hex(&day_one_proof),
        options.hold_road_inspection,
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
        boot.exports[0].value() - local_export,
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
    println!("scope=store_road_construction_boot_town fixed_od=true construction_labor=explicit_input utility=boot_electricity_buffer days=2 self_funding=not_demonstrated balance=not_calibrated");
    Ok(())
}

fn parse_args() -> Result<(PathBuf, Options), String> {
    let mut store_dir = None;
    let mut options = Options {
        hold_road_inspection: false,
        generator_failure: false,
    };
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--hold-road-inspection" => options.hold_road_inspection = true,
            "--generator-failure" => options.generator_failure = true,
            "--store-dir" => store_dir = args.next().map(PathBuf::from),
            _ => return Err(format!("usage: town_lifelines --store-dir PATH [--hold-road-inspection] [--generator-failure] (unknown:{argument})")),
        }
    }
    if options.hold_road_inspection && options.generator_failure {
        return Err(
            "choose one scenario mode: --hold-road-inspection or --generator-failure".into(),
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
