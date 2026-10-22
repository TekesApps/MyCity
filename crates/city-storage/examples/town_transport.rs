//! Run one real Store-backed transport-limited market day.
use mycity_core::{
    construction::{
        EngineCommand as ConstructionCommand, Gate, GateCategory, GateDecision, Kind, Material,
        OPERATION_PREFIX, Project,
    },
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, RoadCapacityEffectBinding,
        TripPartition,
    },
    timeline::{Command as TimelineCommand, DAY},
    transaction::{Action, Caller, Envelope, Resource},
    transport::{Entrance, Node, OdDemand, Opening, RoadEdge},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::{CapacityPolicy, Error as StorageError, Store};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, env, fs, path::Path};

const RULES: &str = "m0-town-transport-fixed-profile-v1";
const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SHIFT_END: u64 = 17 * 3_600;

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
fn err(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn transport_proof_from_receipt(receipt: &mycity_core::transaction::Receipt) -> Option<[u8; 32]> {
    receipt
        .actions
        .iter()
        .flatten()
        .find_map(|action| match action {
            Action::TownTransportSettle { proof_sha256, .. } => Some(*proof_sha256),
            _ => None,
        })
}

fn open(capacity: u32, is_open: bool) -> Opening {
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
                capacity_people: capacity,
                mode_mask: 1,
                open: is_open,
                boundary_source: None,
            },
            RoadEdge {
                id: 11,
                from: 2,
                to: 1,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: capacity,
                mode_mask: 1,
                open: is_open,
                boundary_source: None,
            },
        ],
        entrances: vec![
            Entrance {
                id: 1,
                owner: 2,
                node: 1,
                mode_mask: 1,
                authorized: true,
            },
            Entrance {
                id: 2,
                owner: 3,
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

#[derive(Serialize)]
struct Allocation<'a> {
    schema: u16,
    world_id: u64,
    branch_id: u64,
    market_profile_id: u32,
    period_seconds: u32,
    allocation_scope: AllocationScope,
    demand_repeat_policy: DemandRepeatPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    road_capacity_effect: Option<RoadCapacityEffectBinding>,
    opening: &'a Opening,
    assignment: &'a mycity_core::transport::Assignment,
    partitions: &'a [TripPartition],
    fee_policy: FeePolicyInput,
}

#[derive(Default)]
struct Road28Totals {
    sales: i128,
    produced: i128,
    unmet: i128,
    goods_payments: i128,
    freight: i128,
    utilities: i128,
    disposal: i128,
    labor_basis: i128,
    wages_accrued: i128,
    idle_labor: i128,
    days_with_sales: u32,
    last_sales_day: u32,
}

impl Road28Totals {
    fn add(&mut self, day: u32, summary: mycity_core::town_runtime::MarketSummary) {
        self.sales += i128::from(summary.actual_sales.value());
        self.produced += i128::from(summary.produced.value());
        self.unmet += i128::from(summary.unmet_market_demand.value());
        self.goods_payments += i128::from(summary.goods_payments.value());
        self.freight += i128::from(summary.freight.value());
        self.utilities += i128::from(summary.utilities.value());
        self.disposal += i128::from(summary.disposal.value());
        self.labor_basis += i128::from(summary.labor_basis.value());
        self.wages_accrued += i128::from(summary.wages_accrued.value());
        self.idle_labor += i128::from(summary.idle_labor_expense.value());
        if summary.actual_sales > Quantity::ZERO {
            self.days_with_sales += 1;
            self.last_sales_day = day;
        }
    }
}

fn opening_with_market(allocation_hash: [u8; 32]) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":6,"kind":"inventory_capital","amount":1000}),
        serde_json::json!({"id":7,"kind":"inventory_capital","amount":200}),
    ]);
    for source in value["sources"].as_array_mut().unwrap() {
        if source["id"] == 2 {
            source["amount"] = 1_000_000.into();
        }
    }
    value["accounts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .for_each(|account| {
            if account["id"] == 2 {
                account["cash"] = 1_000_000.into();
            }
        });
    value["stocks"].as_array_mut().unwrap().extend([
        serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":1000,"basis":1000,"source":6}),
        serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":200,"basis":200,"source":7}),
    ]);
    value["town"] = serde_json::json!({
        "schema_version":1,
        "id":1,
        "transport_allocation_sha256":allocation_hash,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":200,"reference_price_per_q":1000}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":200,"freight_per_q":1,"utility_surcharge_per_q":0}]
    });
    serde_json::to_vec(&value).unwrap()
}

fn opening_with_road_project(
    allocation_hash: [u8; 32],
    project: &Project,
    production_and_suspend: bool,
) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&opening_with_market(allocation_hash)).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    // The two-day road-only case needs at least 240 real goods, not a second free refill.
    // The joint case instead starts with no finished goods and actually produces them.
    let finished = if production_and_suspend { 0 } else { 400 };
    for source in value["sources"].as_array_mut().unwrap() {
        if source["id"] == 7 {
            source["amount"] = finished.into();
        }
    }
    for stock in value["stocks"].as_array_mut().unwrap() {
        if stock["id"] == 3 {
            stock["quantity"] = finished.into();
            stock["basis"] = finished.into();
        }
    }
    if production_and_suspend {
        // A separate contractor avoids counting the factory payroll as road-crew labor.
        value["sources"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":8,"kind":"cash_capital","amount":0}));
        value["accounts"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":5,"role":"company","cash":0,"source":8}));
        value["town"]["recipes"] = serde_json::json!([{
            "recipe_id":1,"employer":3,"assigned_workers":1,
            "input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,
            "output_per_worker_day":400,"waste_per_output_q":0,
            "available_utility_per_day":1000,"utility_per_output_q":1,
            "utility_cost_per_output_q":1,"freight_cost_per_input_q":0,
            "disposal_cost_per_input_q":0,"utility_account":1,
            "freight_account":1,"disposal_account":1
        }]);
    }
    serde_json::to_vec(&value).unwrap()
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
    outcome.model.map_err(err)
}

fn advance_to(store: &mut Store, target: u64) -> Result<(), String> {
    for _ in 0..128 {
        execute_action(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8192,
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
    Err("timeline did not reach requested boundary".into())
}

fn project_effect_id(store: &Store) -> Result<u64, String> {
    let project = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .construction
        .ok_or("road project missing")?;
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
        project_effect_id(store)?
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
    outcome.model.map_err(err)
}

/// Store-backed causal scenario: the project pays for a real road-capacity resource, but
/// traffic remains at the base epoch until inspection is accepted at the settled day boundary.
fn run_road_effect(
    dir: &str,
    production_and_suspend: bool,
    road_28: bool,
    road_withdraw: bool,
) -> Result<(), String> {
    if Path::new(dir).exists() {
        return Err("store directory already exists".into());
    }
    let mut opening = open(2, true);
    // The project improves the bottleneck direction; the return leg has spare capacity already.
    opening.edges[1].capacity_people = 10;
    let assignment = opening.assign().map_err(err)?;
    let partitions = [TripPartition {
        quote_id: 1,
        outbound_source: 101,
        return_source: 102,
        trip_count: 10,
        goods_per_trip: Quantity::new(20).map_err(err)?,
    }];
    let allocation = Allocation {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        market_profile_id: 1,
        period_seconds: DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: Some(RoadCapacityEffectBinding {
            project_id: 7,
            resource_id: 81,
            edge_id: 10,
            base_capacity_people: 2,
            added_capacity_people: 8,
        }),
        opening: &opening,
        assignment: &assignment,
        partitions: &partitions,
        fee_policy: FeePolicyInput {
            provider_account: 1,
            cents_per_travel_second: 1,
        },
    };
    let allocation_bytes = serde_json::to_vec(&allocation).map_err(err)?;
    let contractor = if production_and_suspend { 5 } else { 3 };
    let mut project = Project::new(
        7,
        Kind::Build,
        4,
        contractor,
        1,
        80,
        1,
        1,
        Money::new(1_000_000).map_err(err)?,
        Money::new(3_600).map_err(err)?,
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
            required: Quantity::new(1_000).map_err(err)?,
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(400).map_err(err)?,
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .map_err(err)?;
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 8;
    project.operating_edge_id = 10;
    if road_withdraw {
        project.waste_receiver_account = 3;
        project.waste_receiver_resource = 82;
        project.waste_units = 2;
        project.waste_fee = Money::new(250).map_err(err)?;
    }
    let opening_bytes =
        opening_with_road_project(hash(&allocation_bytes), &project, production_and_suspend);
    let mut store = Store::create_with_transport_allocation_profile(
        Path::new(dir),
        &opening_bytes,
        {
            let mut resources = vec![
                Resource::new(80, 1).map_err(err)?,
                Resource::new(81, 2).map_err(err)?,
            ];
            if road_withdraw {
                resources.push(Resource::new(82, 10).map_err(err)?);
            }
            resources
        },
        RULES,
        &allocation_bytes,
    )
    .map_err(err)?;
    let initial_cash_total: i128 = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum();
    let opening_view = store.engine().map_err(err)?.world().view();
    let opening_company_cash = opening_view
        .accounts
        .iter()
        .find(|row| row.id == 3)
        .ok_or("factory company account missing at opening")?
        .cash;
    let opening_raw = opening_view
        .stocks
        .iter()
        .find(|row| row.id == 2)
        .ok_or("factory raw stock missing at opening")?;
    let opening_finished = opening_view
        .stocks
        .iter()
        .find(|row| row.id == 3)
        .ok_or("factory finished stock missing at opening")?;

    for (caller, command) in [
        (Caller::Account(4), ConstructionCommand::File),
        (
            Caller::Account(2),
            ConstructionCommand::DecideGate {
                gate_id: 1,
                decision: GateDecision::Accepted,
                evidence_id: 701,
            },
        ),
        (
            Caller::Account(1),
            ConstructionCommand::DecideGate {
                gate_id: 2,
                decision: GateDecision::Accepted,
                evidence_id: 702,
            },
        ),
        (Caller::Account(4), ConstructionCommand::Prepare),
        (Caller::Account(4), ConstructionCommand::Start),
    ] {
        construction_action(&mut store, caller, command)?;
    }
    execute_action(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    if production_and_suspend {
        execute_action(
            &mut store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::AddPayroll {
                    spec: mycity_core::timeline::PayrollSpec {
                        id: 1,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: Money::new(100).map_err(err)?,
                        cycle: mycity_core::timeline::Cycle::Daily,
                        ends_at: if road_28 { 28 * DAY } else { 7 * DAY },
                    },
                },
            },
        )?;
    }
    advance_to(&mut store, SHIFT_END)?;
    construction_action(
        &mut store,
        Caller::Account(contractor),
        ConstructionCommand::Work {
            at: GameTime::new(SHIFT_END as i64).map_err(err)?,
            elapsed_seconds: SHIFT_END as u32,
            worker_seconds: 28_800,
        },
    )?;
    construction_action(
        &mut store,
        Caller::Account(contractor),
        ConstructionCommand::SubmitInspection,
    )?;

    let midday_state = store.state_hash().map_err(err)?;
    let midday_view = store.engine().map_err(err)?.world().view();
    let midday = store.execute(
        Caller::Account(1),
        Envelope {
            world: midday_view.scope.world(),
            branch: midday_view.scope.branch(),
            operation_id: store.committed_sequence() + 1,
            expected_revision: midday_view.revision,
            actions: vec![Action::Construction {
                command: ConstructionCommand::AcceptInspection {
                    inspector: 1,
                    accepted: true,
                },
            }],
        },
    );
    let midday_rejected = match midday {
        Ok(outcome) => outcome.model.is_err(),
        Err(_) => true,
    };
    if !midday_rejected || store.state_hash().map_err(err)? != midday_state {
        return Err("midday inspection acceptance changed the world".into());
    }
    advance_to(&mut store, DAY)?;
    let unsettled_state = store.state_hash().map_err(err)?;
    let unsettled_view = store.engine().map_err(err)?.world().view();
    let unsettled = store.execute(
        Caller::Account(1),
        Envelope {
            world: unsettled_view.scope.world(),
            branch: unsettled_view.scope.branch(),
            operation_id: store.committed_sequence() + 1,
            expected_revision: unsettled_view.revision,
            actions: vec![Action::Construction {
                command: ConstructionCommand::AcceptInspection {
                    inspector: 1,
                    accepted: true,
                },
            }],
        },
    );
    let unsettled_rejected = match unsettled {
        Ok(outcome) => outcome.model.is_err(),
        Err(_) => true,
    };
    if !unsettled_rejected || store.state_hash().map_err(err)? != unsettled_state {
        return Err("inspection before day-one market settlement changed the world".into());
    }
    let before_first_day = store.engine().map_err(err)?.world().view();
    let first_provider_cash = before_first_day
        .accounts
        .iter()
        .find(|account| account.id == 1)
        .ok_or("tariff provider account missing")?
        .cash
        .value();
    store
        .settle_town_market(80_001)
        .map_err(err)?
        .model
        .map_err(err)?;
    let day_one = store.engine().map_err(err)?.world().view();
    let day_one_market = day_one.town.ok_or("town missing")?;
    if day_one_market.state.last_summary.actual_sales.value() != 40
        || day_one.construction.unwrap().operating_published
        || store
            .engine()
            .map_err(err)?
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .ok_or("road resource missing")?
            .capacity
            != 2
    {
        return Err("road capacity became effective before inspection acceptance".into());
    }
    let first_proof = day_one_market
        .state
        .last_transport
        .unwrap()
        .issued_proof_sha256;
    construction_action(
        &mut store,
        Caller::Account(1),
        ConstructionCommand::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
    )?;
    let published = store.engine().map_err(err)?.world().view();
    if published.construction.unwrap().operating_published_at
        != Some(GameTime::new(DAY as i64).map_err(err)?)
        || store
            .engine()
            .map_err(err)?
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .ok_or("road resource missing after publication")?
            .capacity
            != 10
    {
        return Err("accepted road project did not publish atomically".into());
    }
    if store.settle_town_market(80_002).is_ok() {
        return Err("second settlement for the same completed day was accepted".into());
    }
    if !road_withdraw {
        let demolition = construction_action(
            &mut store,
            Caller::Account(4),
            ConstructionCommand::BeginDemolition { waste_receiver: 3 },
        );
        if demolition.is_ok() {
            return Err("irreversible road effect allowed demolition".into());
        }
    }
    advance_to(&mut store, 2 * DAY)?;
    store
        .settle_town_market(80_003)
        .map_err(err)?
        .model
        .map_err(err)?;
    let day_two = store.engine().map_err(err)?.world().view();
    let day_two_market = day_two.town.ok_or("town missing after day two")?;
    if day_two_market.state.last_summary.actual_sales.value() != 200
        || day_two_market.state.last_transport.unwrap().issued_day != 2
        || day_two_market
            .state
            .last_transport
            .unwrap()
            .consumed_generation
            != 2
    {
        return Err("next-day road epoch did not unlock the expected 200 sales".into());
    }
    let second_proof = day_two_market
        .state
        .last_transport
        .unwrap()
        .issued_proof_sha256;
    let day_two_provider_cash = day_two
        .accounts
        .iter()
        .find(|account| account.id == 1)
        .ok_or("tariff provider account missing after day two")?
        .cash
        .value();
    if day_two_provider_cash - first_provider_cash
        != day_one_market.state.last_summary.freight.value()
            + day_one_market.state.last_summary.utilities.value()
            + day_one_market.state.last_summary.disposal.value()
            + day_two_market.state.last_summary.freight.value()
            + day_two_market.state.last_summary.utilities.value()
            + day_two_market.state.last_summary.disposal.value()
        || first_proof == second_proof
    {
        return Err("road service freight or daily proof generation did not reconcile".into());
    }
    let construction = day_two.construction.unwrap();
    if construction.materials[0].consumed != Quantity::new(1_000).map_err(err)?
        || day_two
            .stocks
            .iter()
            .find(|stock| stock.id == 1)
            .unwrap()
            .quantity
            .value()
            != 599_000
    {
        return Err("construction material did not debit the real inventory".into());
    }
    let cash_total = |accounts: &[mycity_core::assembly::AccountView]| -> i128 {
        accounts
            .iter()
            .map(|account| i128::from(account.cash.value()))
            .sum()
    };
    if initial_cash_total != cash_total(&day_two.accounts) {
        return Err("construction and market transactions did not conserve city cash".into());
    }
    let mut withdrawal_report = Value::Null;
    let mut third_proof = None;
    if road_withdraw {
        construction_action(
            &mut store,
            Caller::Account(4),
            ConstructionCommand::BeginDemolition { waste_receiver: 3 },
        )?;
        construction_action(
            &mut store,
            Caller::Account(2),
            ConstructionCommand::DecideGate {
                gate_id: 1,
                decision: GateDecision::Accepted,
                evidence_id: 703,
            },
        )?;
        construction_action(
            &mut store,
            Caller::Account(1),
            ConstructionCommand::DecideGate {
                gate_id: 2,
                decision: GateDecision::Accepted,
                evidence_id: 704,
            },
        )?;
        construction_action(&mut store, Caller::Account(4), ConstructionCommand::Prepare)?;
        construction_action(&mut store, Caller::Account(4), ConstructionCommand::Start)?;
        let withdrawn = store.engine().map_err(err)?.world().view();
        let project = withdrawn
            .construction
            .ok_or("road project missing after withdrawal")?;
        let road_capacity = store
            .engine()
            .map_err(err)?
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .ok_or("road resource missing after withdrawal")?
            .capacity;
        if project.kind != Kind::Demolish
            || project.operating_published
            || project.operating_published_at != Some(GameTime::new(DAY as i64).map_err(err)?)
            || project.operating_withdrawn_at != Some(GameTime::new((2 * DAY) as i64).map_err(err)?)
            || road_capacity != 2
        {
            return Err("approved demolition did not withdraw only the added road capacity".into());
        }

        advance_to(&mut store, 3 * DAY)?;
        store
            .settle_town_market(80_004)
            .map_err(err)?
            .model
            .map_err(err)?;
        let day_three = store.engine().map_err(err)?.world().view();
        let day_three_market = day_three.town.ok_or("town missing after withdrawal")?;
        let day_three_proof = day_three_market
            .state
            .last_transport
            .ok_or("day-three transport proof missing")?;
        if day_three_proof.issued_day != 3
            || day_three_proof.consumed_generation != 3
            || day_three_proof.issued_proof_sha256 == second_proof
            || day_three_market.state.last_summary.actual_sales.value() != 40
            || store
                .engine()
                .map_err(err)?
                .resources()
                .iter()
                .find(|resource| resource.id == 81)
                .ok_or("road resource missing on day three")?
                .capacity
                != 2
        {
            return Err(
                "withdrawn road capacity or day-three historical proof is incorrect".into(),
            );
        }
        third_proof = Some(day_three_proof.issued_proof_sha256);

        let work_end = 3 * DAY + SHIFT_END;
        advance_to(&mut store, work_end)?;
        construction_action(
            &mut store,
            Caller::Account(contractor),
            ConstructionCommand::Work {
                at: GameTime::new(work_end as i64).map_err(err)?,
                elapsed_seconds: u32::try_from(work_end - 2 * DAY).map_err(err)?,
                worker_seconds: 28_800,
            },
        )?;
        construction_action(
            &mut store,
            Caller::Account(4),
            ConstructionCommand::AcceptWaste,
        )?;
        construction_action(&mut store, Caller::Account(4), ConstructionCommand::Close)?;
        let closed = store.engine().map_err(err)?.world().view();
        if closed
            .construction
            .ok_or("road project missing after close")?
            .phase
            != mycity_core::construction::Phase::Closed
            || store
                .engine()
                .map_err(err)?
                .resources()
                .iter()
                .find(|resource| resource.id == 81)
                .ok_or("road resource missing after close")?
                .capacity
                != 2
            || store
                .engine()
                .map_err(err)?
                .resources()
                .iter()
                .find(|resource| resource.id == 80)
                .ok_or("site resource missing after close")?
                .occupied
                != 0
            || store
                .engine()
                .map_err(err)?
                .resources()
                .iter()
                .find(|resource| resource.id == 82)
                .ok_or("waste resource missing after close")?
                .occupied
                != 2
            || initial_cash_total != cash_total(&closed.accounts)
        {
            return Err(
                "completed road demolition did not preserve the base capacity and release site"
                    .into(),
            );
        }
        withdrawal_report = serde_json::json!({
            "demolition_gates_and_start":"accepted_at_settled_day_2_boundary",
            "capacity_after_start":road_capacity,
            "withdrawal_timestamp":2 * DAY,
            "day_two_expanded_sales":day_two_market.state.last_summary.actual_sales.value(),
            "day_three_base_sales":day_three_market.state.last_summary.actual_sales.value(),
            "day_two_and_day_three_proofs_distinct":true,
            "demolition_work_and_waste":"completed_and_accepted",
            "site":"released",
            "project":"closed"
        });
    }
    store.checkpoint().map_err(err)?;
    let committed = store.state_hash().map_err(err)?;
    drop(store);
    let mut recovered = Store::open(Path::new(dir), RULES).map_err(err)?;
    if recovered.state_hash().map_err(err)? != committed
        || recovered.receipt(80_001).map_err(err)?.is_none()
        || recovered.receipt(80_003).map_err(err)?.is_none()
    {
        return Err("cold recovery did not preserve road publication and historical proofs".into());
    }
    if road_withdraw {
        let day_two_receipt = recovered
            .receipt(80_003)
            .map_err(err)?
            .ok_or("day-two settlement receipt missing after withdrawal cold open")?;
        let day_three_receipt = recovered
            .receipt(80_004)
            .map_err(err)?
            .ok_or("day-three settlement receipt missing after withdrawal cold open")?;
        let third_proof = third_proof.ok_or("day-three road proof was not captured")?;
        let reopened = recovered.engine().map_err(err)?.world().view();
        let reopened_project = reopened
            .construction
            .ok_or("closed road project missing after withdrawal cold open")?;
        let reopened_capacity = recovered
            .engine()
            .map_err(err)?
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .ok_or("base road resource missing after withdrawal cold open")?
            .capacity;
        if transport_proof_from_receipt(&day_two_receipt) != Some(second_proof)
            || transport_proof_from_receipt(&day_three_receipt) != Some(third_proof)
            || reopened
                .town
                .and_then(|town| town.state.last_transport)
                .is_none_or(|proof| {
                    proof.issued_proof_sha256 != third_proof || proof.issued_day != 3
                })
            || reopened_project.kind != Kind::Demolish
            || reopened_project.phase != mycity_core::construction::Phase::Closed
            || reopened_project.operating_published_at
                != Some(GameTime::new(DAY as i64).map_err(err)?)
            || reopened_project.operating_withdrawn_at
                != Some(GameTime::new((2 * DAY) as i64).map_err(err)?)
            || reopened_capacity != 2
        {
            return Err(
                "cold reopen did not preserve both historical road-capacity epoch proofs".into(),
            );
        }
    }
    let mut suspension_report = Value::Null;
    let mut road_28_report = Value::Null;
    if road_28 {
        if day_one_market.state.last_summary.produced.value() != 400 {
            return Err(
                "road-28 did not produce goods from actual payroll and opening raw stock".into(),
            );
        }
        let mut totals = Road28Totals::default();
        totals.add(1, day_one_market.state.last_summary);
        totals.add(2, day_two_market.state.last_summary);
        let mut proof_hashes = BTreeSet::new();
        proof_hashes.insert(first_proof);
        proof_hashes.insert(second_proof);
        if proof_hashes.len() != 2 {
            return Err("day-one and day-two transport proofs are not unique".into());
        }
        let mut midpoint_hash = String::new();
        let mut midpoint_sequence = 0;
        let mut midpoint_op = 0;
        for day in 3..=28_u32 {
            advance_to(&mut recovered, u64::from(day) * DAY)?;
            let settlement_op = 80_100 + u64::from(day);
            recovered
                .settle_town_market(settlement_op)
                .map_err(err)?
                .model
                .map_err(err)?;
            if recovered.receipt(settlement_op).map_err(err)?.is_none() {
                return Err(format!("day-{day} transport settlement receipt missing"));
            }
            let daily = recovered.engine().map_err(err)?.world().view();
            let market = daily.town.ok_or("town missing during road-28")?;
            let transport = market
                .state
                .last_transport
                .ok_or("daily road-28 transport proof missing")?;
            if transport.issued_day != u64::from(day) || transport.consumed_generation != 2 {
                return Err(format!("day-{day} used a stale traffic proof"));
            }
            if !proof_hashes.insert(transport.issued_proof_sha256) {
                return Err(format!("day-{day} reused a prior transport proof"));
            }
            totals.add(day, market.state.last_summary);
            if day == 14 {
                midpoint_op = settlement_op;
                recovered.checkpoint().map_err(err)?;
                midpoint_hash = recovered.state_hash().map_err(err)?;
                midpoint_sequence = recovered.committed_sequence();
                drop(recovered);
                recovered = Store::open(Path::new(dir), RULES).map_err(err)?;
                let reopened = recovered.engine().map_err(err)?.world().view();
                let reopened_market = reopened
                    .town
                    .ok_or("town missing after day-14 cold reopen")?;
                if recovered.state_hash().map_err(err)? != midpoint_hash
                    || recovered.committed_sequence() != midpoint_sequence
                    || recovered.receipt(midpoint_op).map_err(err)?.is_none()
                    || reopened_market
                        .state
                        .last_transport
                        .ok_or("day-14 proof missing after reopen")?
                        .issued_day
                        != 14
                {
                    return Err("midpoint cold reopen lost road-28 state or proof".into());
                }
            }
        }
        let final_view = recovered.engine().map_err(err)?.world().view();
        let final_market = final_view
            .town
            .as_ref()
            .ok_or("town missing at road-28 close")?;
        let final_raw = final_view
            .stocks
            .iter()
            .find(|stock| stock.id == 2)
            .ok_or("factory raw stock missing at road-28 close")?;
        let final_finished = final_view
            .stocks
            .iter()
            .find(|stock| stock.id == 3)
            .ok_or("factory finished stock missing at road-28 close")?;
        let final_payroll = recovered
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("road-28 payroll missing")?
            .payroll[0];
        let final_company_cash = final_view
            .accounts
            .iter()
            .find(|account| account.id == 3)
            .ok_or("factory company account missing at road-28 close")?
            .cash
            .value();
        let opening_stock_basis =
            i128::from(opening_raw.basis.value()) + i128::from(opening_finished.basis.value());
        let final_stock_basis =
            i128::from(final_raw.basis.value()) + i128::from(final_finished.basis.value());
        let opening_restricted_book_projection =
            i128::from(opening_company_cash.value()) + opening_stock_basis;
        let final_wage_outstanding =
            i128::from(final_payroll.accrued.value()) - i128::from(final_payroll.paid.value());
        let final_restricted_book_projection =
            i128::from(final_company_cash) + final_stock_basis - final_wage_outstanding;
        let final_provider_cash = final_view
            .accounts
            .iter()
            .find(|account| account.id == 1)
            .ok_or("tariff provider account missing at road-28 close")?
            .cash
            .value();
        let raw_consumed = i128::from(opening_raw.quantity.value() - final_raw.quantity.value());
        let finished_balance =
            i128::from(opening_finished.quantity.value()) + totals.produced - totals.sales;
        if totals.days_with_sales == 0
            || totals.sales < 0
            || totals.produced < 0
            || totals.unmet < 0
            || raw_consumed != totals.produced
            || finished_balance != i128::from(final_finished.quantity.value())
            || totals.wages_accrued != i128::from(final_payroll.accrued.value())
            || totals.labor_basis + totals.idle_labor != totals.wages_accrued
            || final_payroll.spec.ends_at != 28 * DAY
            || final_payroll.working
            || final_payroll.next_due.is_some()
            || final_payroll.paid > final_payroll.accrued
            || final_provider_cash - first_provider_cash
                != i64::try_from(totals.freight + totals.utilities + totals.disposal)
                    .map_err(err)?
            || initial_cash_total
                != final_view
                    .accounts
                    .iter()
                    .map(|account| i128::from(account.cash.value()))
                    .sum::<i128>()
        {
            return Err(
                "road-28 cumulative money, goods, wages, or proof reconciliation failed".into(),
            );
        }
        let expected_receipts = std::iter::once(80_001_u64)
            .chain(std::iter::once(80_003_u64))
            .chain((3..=28_u64).map(|day| 80_100 + day));
        for operation_id in expected_receipts {
            if recovered.receipt(operation_id).map_err(err)?.is_none() {
                return Err(format!("historical market receipt {operation_id} missing"));
            }
        }
        recovered.checkpoint().map_err(err)?;
        let final_hash = recovered.state_hash().map_err(err)?;
        let final_sequence = recovered.committed_sequence();
        drop(recovered);
        recovered = Store::open(Path::new(dir), RULES).map_err(err)?;
        if recovered.state_hash().map_err(err)? != final_hash
            || recovered.committed_sequence() != final_sequence
        {
            return Err("final road-28 cold reopen changed state or sequence".into());
        }
        for operation_id in std::iter::once(80_001_u64)
            .chain(std::iter::once(80_003_u64))
            .chain((3..=28_u64).map(|day| 80_100 + day))
        {
            if recovered.receipt(operation_id).map_err(err)?.is_none() {
                return Err(format!(
                    "final cold reopen lost market receipt {operation_id}"
                ));
            }
        }
        road_28_report = serde_json::json!({
            "daily_settlements":28,
            "transport_proofs_unique_and_consumed":proof_hashes.len(),
            "fixed_repeat_od_batch_per_day":true,
            "initial_cash_company_3":opening_company_cash.value(),
            "initial_cash_citywide":initial_cash_total,
            "opening_stock_basis":opening_stock_basis,
            "opening_restricted_book_projection":opening_restricted_book_projection,
            "initial_raw_quantity":opening_raw.quantity.value(),
            "initial_raw_basis":opening_raw.basis.value(),
            "initial_finished_quantity":opening_finished.quantity.value(),
            "final_company_cash":final_company_cash,
            "final_raw_quantity":final_raw.quantity.value(),
            "final_stock_basis":final_stock_basis,
            "final_wage_outstanding":final_wage_outstanding,
            "final_restricted_book_projection":final_restricted_book_projection,
            "restricted_book_projection_delta":final_restricted_book_projection-opening_restricted_book_projection,
            "final_finished_quantity":final_finished.quantity.value(),
            "cumulative_produced":totals.produced,
            "cumulative_actual_sales":totals.sales,
            "cumulative_unmet_demand":totals.unmet,
            "days_with_sales":totals.days_with_sales,
            "last_day_with_sales":totals.last_sales_day,
            "cumulative_goods_payments":totals.goods_payments,
            "cumulative_freight_cost":totals.freight,
            "cumulative_utility_cost":totals.utilities,
            "cumulative_disposal_cost":totals.disposal,
            "cumulative_labor_basis":totals.labor_basis,
            "cumulative_wages_accrued":totals.wages_accrued,
            "cumulative_idle_labor_cost":totals.idle_labor,
            "payroll_contract_end_day":final_payroll.spec.ends_at / DAY,
            "payroll_accrued":final_payroll.accrued.value(),
            "payroll_paid":final_payroll.paid.value(),
            "payroll_outstanding":final_payroll.accrued.value()-final_payroll.paid.value(),
            "construction_spent":construction.spent.value(),
            "cash_conservation":"passed",
            "raw_to_output_balance":"passed",
            "output_stock_balance":"passed",
            "midpoint_day_14_cold_reopen":"passed",
            "final_cold_reopen":"passed",
            "opening_capital_present":opening_company_cash.value() > 0,
            "opening_inventory_consumed":raw_consumed > 0,
            "opening_raw_inventory_consumed":raw_consumed,
            "continuous_self_funding":"not_demonstrated; finite opening raw inventory is consumed",
            "book_projection_scope":"restricted cash plus inventory basis less unpaid wages; excludes unmodeled liabilities/assets and is not a full balance sheet",
            "continuous_sales_all_28_days":totals.days_with_sales==28,
            "utility_input":"explicit_capacity_and_unit_cost_profile_not_live_grid",
            "traffic_demand":"repeat_fixed_daily_OD_not_endogenous_to_market_shortage",
            "legal_company_close":false
        });
    }
    if production_and_suspend && !road_28 {
        if day_one_market.state.last_summary.produced.value() != 400 {
            return Err(
                "joint case did not produce the goods from actual payroll and raw stock".into(),
            );
        }
        let before_suspend = recovered.engine().map_err(err)?.world().view();
        let inventory_before: Vec<_> = before_suspend
            .stocks
            .iter()
            .filter(|stock| stock.owner == 3)
            .map(|stock| (stock.id, stock.quantity, stock.basis))
            .collect();
        let company_cash = before_suspend
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .ok_or("joint company account missing")?
            .cash;
        let accrued = recovered
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("joint timeline missing")?
            .payroll[0]
            .accrued;
        let suspend = Envelope {
            world: before_suspend.scope.world(),
            branch: before_suspend.scope.branch(),
            operation_id: recovered.committed_sequence() + 1,
            expected_revision: before_suspend.revision,
            actions: vec![Action::TownSuspendOperations {
                profile_id: 1,
                company_account: 3,
            }],
        };
        let outcome = recovered
            .execute(Caller::System, suspend.clone())
            .map_err(err)?;
        outcome.model.map_err(err)?;
        if !outcome.has_persistent_receipt {
            return Err("joint suspension missing durable receipt".into());
        }
        let stopped_hash = recovered.state_hash().map_err(err)?;
        recovered.checkpoint().map_err(err)?;
        drop(recovered);
        recovered = Store::open(Path::new(dir), RULES).map_err(err)?;
        if recovered.state_hash().map_err(err)? != stopped_hash {
            return Err("joint suspension cold recovery changed state".into());
        }
        let retry = recovered.execute(Caller::System, suspend).map_err(err)?;
        retry.model.map_err(err)?;
        if !retry.has_persistent_receipt || recovered.state_hash().map_err(err)? != stopped_hash {
            return Err("joint suspension replay changed state".into());
        }
        advance_to(&mut recovered, 3 * DAY)?;
        recovered
            .settle_town_market(80_004)
            .map_err(err)?
            .model
            .map_err(err)?;
        let after = recovered.engine().map_err(err)?.world().view();
        let state = after
            .town
            .ok_or("joint town missing after suspension")?
            .state;
        let inventory_after: Vec<_> = after
            .stocks
            .iter()
            .filter(|stock| stock.owner == 3)
            .map(|stock| (stock.id, stock.quantity, stock.basis))
            .collect();
        let payroll = recovered
            .engine()
            .map_err(err)?
            .timeline()
            .ok_or("joint payroll missing after suspension")?
            .payroll[0];
        if state.last_summary.actual_sales != Quantity::ZERO
            || state.last_summary.produced != Quantity::ZERO
            || state.last_summary.wages_accrued != Money::ZERO
            || state.last_summary.freight != Money::ZERO
            || state
                .last_transport
                .ok_or("joint day-three proof missing")?
                .issued_day
                != 3
            || inventory_after != inventory_before
            || after
                .accounts
                .iter()
                .find(|row| row.id == 3)
                .ok_or("joint company account lost")?
                .cash
                != company_cash
            || payroll.working
            || payroll.next_due.is_some()
            || payroll.accrued != accrued
            || cash_total(&after.accounts) != initial_cash_total
        {
            return Err(
                "joint suspended company continued operations or changed money/materials".into(),
            );
        }
        recovered.checkpoint().map_err(err)?;
        let final_hash = recovered.state_hash().map_err(err)?;
        drop(recovered);
        recovered = Store::open(Path::new(dir), RULES).map_err(err)?;
        if recovered.state_hash().map_err(err)? != final_hash
            || [80_001, 80_003, 80_004]
                .iter()
                .any(|id| recovered.receipt(*id).ok().flatten().is_none())
        {
            return Err("joint final recovery lost a historical transport settlement".into());
        }
        suspension_report = serde_json::json!({
            "day_one_real_production":day_one_market.state.last_summary.produced.value(),
            "day_three_actual_sales":state.last_summary.actual_sales.value(),
            "day_three_produced":state.last_summary.produced.value(),
            "day_three_new_wages":state.last_summary.wages_accrued.value(),
            "inventory_and_company_cash_after_stop":"unchanged",
            "suspension_checkpoint_and_same_op_retry":"passed",
            "historical_day_one_two_three_proofs":"cold_restored",
            "construction_contractor_account":contractor,
            "factory_and_payroll_payer":3,
            "utility_supply":"explicit_capacity_and_cost_profile_not_live_grid",
            "traffic_demand":"repeat_fixed_daily_OD_not_endogenous_to_shop_closure",
            "legal_company_close":false
        });
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scenario":if road_withdraw {"road-withdraw"} else if road_28 {"road-28"} else if production_and_suspend {"road-suspend"} else {"road-effect"},
            "production_and_suspension":suspension_report,
            "road_28":road_28_report,
            "road_withdrawal":withdrawal_report,
            "day_one_sales_before_acceptance":day_one_market.state.last_summary.actual_sales.value(),
            "day_two_sales_after_acceptance":day_two_market.state.last_summary.actual_sales.value(),
            "capacity_base":2,
            "capacity_after_accepted_inspection":10,
            "published_at":DAY,
            "old_day_proof_hash":first_proof,
            "new_day_proof_hash":second_proof,
            "same_day_duplicate":"rejected",
            "midday_inspection":"rejected_without_state_change",
            "unsettled_boundary_inspection":"rejected_without_state_change",
            "demolition":if road_withdraw {"approved_then_completed"} else {"rejected"},
            "real_material_debit":construction.materials[0].consumed.value(),
            "cash_conservation":"passed",
            "cold_recovery":"passed"
        }))
        .map_err(err)?
    );
    Ok(())
}

fn assert_untrusted_action_rejected(
    store: &mut Store,
    operation_id: u64,
    scope_world: u64,
    scope_branch: u64,
    action: Action,
) -> Result<(), String> {
    let view = store.engine().map_err(err)?.world().view();
    let outcome = store.execute(
        Caller::System,
        Envelope {
            world: scope_world,
            branch: scope_branch,
            operation_id,
            expected_revision: view.revision,
            actions: vec![action],
        },
    );
    match outcome {
        Ok(outcome) if outcome.model.is_err() => Ok(()),
        Err(StorageError::Invalid("town_transport_requires_verified_proof")) => Ok(()),
        Ok(_) => Err("caller-supplied transport proof command was accepted".into()),
        Err(error) => Err(err(error)),
    }
}

fn verify_capacity_rejection(dir: &str, opening: &[u8], allocation: &[u8]) -> Result<(), String> {
    if Path::new(dir).exists() {
        return Err("capacity probe directory already exists".into());
    }
    let mut store = Store::create_with_transport_allocation_profile(
        Path::new(dir),
        opening,
        vec![],
        RULES,
        allocation,
    )
    .map_err(err)?;
    let operate = |store: &mut Store, action| -> Result<(), String> {
        let view = store.engine().map_err(err)?.world().view();
        let outcome = store
            .execute(
                Caller::System,
                Envelope {
                    world: view.scope.world(),
                    branch: view.scope.branch(),
                    operation_id: store.committed_sequence() + 1,
                    expected_revision: view.revision,
                    actions: vec![Action::Timeline { command: action }],
                },
            )
            .map_err(err)?;
        outcome.model.map_err(err)?;
        Ok(())
    };
    operate(&mut store, TimelineCommand::Enable)?;
    for _ in 0..128 {
        operate(
            &mut store,
            TimelineCommand::Advance {
                target: DAY,
                limit: 8192,
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            break;
        }
    }
    let before_policy = store.capacity_status().map_err(err)?;
    let quota_slack = 4 * 64 * 1024;
    let small_quota = CapacityPolicy {
        total_bytes: before_policy.used_bytes + quota_slack,
        emergency_bytes: quota_slack,
    };
    store.reduce_capacity_policy(small_quota).map_err(err)?;
    let before_attempt = store.capacity_status().map_err(err)?;
    let before_emergency = before_attempt.total_bytes - before_attempt.normal_limit;
    let before_sequence = store.committed_sequence();
    let before_state = store.state_hash().map_err(err)?;
    let blocked = store.settle_town_market(50_001);
    if !matches!(blocked, Err(StorageError::Capacity { .. })) {
        return Err(format!(
            "proof storage quota did not reject settlement: {blocked:?}"
        ));
    }
    let after_attempt = store.capacity_status().map_err(err)?;
    let market = store.engine().map_err(err)?.world().view().town.unwrap();
    let proof_exists = fs::read_dir(dir)
        .map_err(err)?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("town-proof-")
        });
    if store.committed_sequence() != before_sequence
        || store.state_hash().map_err(err)? != before_state
        || market.state.settled_day != 0
        || market.state.last_transport.is_some()
        || proof_exists
        || after_attempt.used_bytes != before_attempt.used_bytes
        || after_attempt.total_bytes - after_attempt.normal_limit != before_emergency
    {
        return Err("capacity rejection changed proof files, cursor, state, or reserve".into());
    }
    Ok(())
}

/// Read an existing copied Store using the current reader and compare its canonical state
/// identity with a hash recorded by the frozen writer. This path intentionally performs no
/// checkpoint, transaction, or proof regeneration.
fn run_reopen_check(dir: &str, expected_rules: &str, expected_hash: &str) -> Result<(), String> {
    if !Path::new(dir).is_dir() {
        return Err("existing_store_dir_required".into());
    }
    if expected_rules.is_empty() || expected_rules.len() > 128 {
        return Err("expected_rules_identity_required".into());
    }
    if expected_hash.len() != 64
        || !expected_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("expected_state_sha256_must_be_64_lowercase_hex_chars".into());
    }
    let store = Store::open(Path::new(dir), expected_rules).map_err(err)?;
    let observed_hash = store.state_hash().map_err(err)?;
    if observed_hash != expected_hash {
        return Err(format!(
            "state_hash_mismatch expected={expected_hash} observed={observed_hash}"
        ));
    }
    let engine = store.engine().map_err(err)?;
    let view = engine.world().view();
    let project = view.construction;
    let accepted_project_count = u8::from(project.is_some_and(|project| {
        project.inspected && project.phase == mycity_core::construction::Phase::Operational
    }));
    let transport = view.town.and_then(|town| town.state.last_transport);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scenario": "reopen-check",
            "rules_identity": expected_rules,
            "expected_state_sha256": expected_hash,
            "observed_state_sha256": observed_hash,
            "state_hash_match": true,
            "committed_sequence": store.committed_sequence(),
            "construction_project_count": u8::from(project.is_some()),
            "accepted_operating_project_count": accepted_project_count,
            "construction_project_phase": project.map(|value| format!("{:?}", value.phase)),
            "construction_inspected": project.map(|value| value.inspected),
            "construction_operating_published": project.map(|value| value.operating_published),
            "construction_published_at": project.map(|value| value.operating_published_at.map(|at| at.value())),
            "transport_settlement_present": transport.is_some(),
            "transport_service_day": transport.map(|receipt| receipt.issued_day),
            "transport_generation": transport.map(|receipt| receipt.issued_generation),
            "transport_proof_sha256": transport.map(|receipt| hex(&receipt.issued_proof_sha256))
        }))
        .map_err(err)?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("town_transport: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let scenario = args.next().ok_or(
        "usage: town_transport <normal|closed|bottleneck|expanded|road-effect|road-suspend|road-28|road-withdraw> <new-store-dir> | reopen-check <existing-copied-world-dir> <expected-rules-identity> <expected-state-sha256>",
    )?;
    let dir = args.next().ok_or("missing_store_dir")?;
    if scenario == "reopen-check" {
        let expected_rules = args.next().ok_or("missing_expected_rules_identity")?;
        let expected_hash = args.next().ok_or("missing_expected_state_sha256")?;
        if args.next().is_some() {
            return Err("unexpected_reopen_check_argument".into());
        }
        return run_reopen_check(&dir, &expected_rules, &expected_hash);
    }
    if args.next().is_some() {
        return Err("unexpected_scenario_argument".into());
    }
    if matches!(
        scenario.as_str(),
        "road-effect" | "road-suspend" | "road-28" | "road-withdraw"
    ) {
        let production = matches!(scenario.as_str(), "road-suspend" | "road-28");
        return run_road_effect(
            &dir,
            production,
            scenario == "road-28",
            scenario == "road-withdraw",
        );
    }
    let (capacity, is_open, expected) = match scenario.as_str() {
        "normal" => (5, true, 100),
        "closed" => (10, false, 0),
        "bottleneck" => (2, true, 40),
        "expanded" => (10, true, 200),
        _ => return Err("unknown scenario".into()),
    };
    if Path::new(&dir).exists() {
        return Err("store directory already exists".into());
    }
    let opening = open(capacity, is_open);
    let assignment = opening.assign().map_err(err)?;
    let partitions = [TripPartition {
        quote_id: 1,
        outbound_source: 101,
        return_source: 102,
        trip_count: 10,
        goods_per_trip: Quantity::new(20).map_err(err)?,
    }];
    let allocation = Allocation {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        market_profile_id: 1,
        period_seconds: DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: None,
        opening: &opening,
        assignment: &assignment,
        partitions: &partitions,
        fee_policy: FeePolicyInput {
            provider_account: 1,
            cents_per_travel_second: 1,
        },
    };
    let allocation_bytes = serde_json::to_vec(&allocation).map_err(err)?;

    // The profile hash binds the complete Opening and Assignment. Store creation rejects
    // changed assignments and wrong allocation scopes before creating any durable files.
    let mut forged_assignment = assignment.clone();
    forged_assignment.requested_people += 1;
    let forged = Allocation {
        assignment: &forged_assignment,
        ..allocation
    };
    let forged_bytes = serde_json::to_vec(&forged).map_err(err)?;
    let forged_dir = format!("{dir}-forged-assignment");
    if Store::create_with_transport_allocation_profile(
        Path::new(&forged_dir),
        &opening_with_market(hash(&forged_bytes)),
        vec![],
        RULES,
        &forged_bytes,
    )
    .is_ok()
        || Path::new(&forged_dir).exists()
    {
        return Err("forged transport assignment was not rejected before persistence".into());
    }
    for (suffix, world_id, branch_id, profile_id) in [
        ("world", 2, 1, 1),
        ("branch", 1, 2, 1),
        ("profile", 1, 1, 2),
    ] {
        let wrong_scope = Allocation {
            world_id,
            branch_id,
            market_profile_id: profile_id,
            ..allocation
        };
        let wrong_scope_bytes = serde_json::to_vec(&wrong_scope).map_err(err)?;
        let wrong_scope_dir = format!("{dir}-wrong-{suffix}");
        if Store::create_with_transport_allocation_profile(
            Path::new(&wrong_scope_dir),
            &opening_with_market(hash(&wrong_scope_bytes)),
            vec![],
            RULES,
            &wrong_scope_bytes,
        )
        .is_ok()
            || Path::new(&wrong_scope_dir).exists()
        {
            return Err(format!("wrong transport allocation {suffix} was accepted"));
        }
    }
    let opening_bytes = opening_with_market(hash(&allocation_bytes));
    let mut store = Store::create_with_transport_allocation_profile(
        Path::new(&dir),
        &opening_bytes,
        vec![],
        RULES,
        &allocation_bytes,
    )
    .map_err(err)?;
    let timeline = |store: &mut Store, action| -> Result<(), String> {
        let view = store.engine().map_err(err)?.world().view();
        let envelope = Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: store.committed_sequence() + 1,
            expected_revision: view.revision,
            actions: vec![Action::Timeline { command: action }],
        };
        let outcome = store.execute(Caller::System, envelope).map_err(err)?;
        if outcome.model.is_err() {
            return Err("timeline action rejected".into());
        }
        Ok(())
    };
    timeline(&mut store, TimelineCommand::Enable)?;
    for _ in 0..128 {
        timeline(
            &mut store,
            TimelineCommand::Advance {
                target: DAY,
                limit: 8192,
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            break;
        }
    }
    verify_capacity_rejection(
        &format!("{dir}-quota-probe"),
        &opening_bytes,
        &allocation_bytes,
    )?;
    let current = store.engine().map_err(err)?.world().view();
    let tariff_provider_cash_before = current
        .accounts
        .iter()
        .find(|account| account.id == 1)
        .ok_or("tariff provider account missing")?
        .cash
        .value();
    assert_untrusted_action_rejected(
        &mut store,
        9_990,
        current.scope.world(),
        current.scope.branch(),
        Action::TownSettle { profile_id: 1 },
    )?;
    let untrusted_action = Action::TownTransportSettle {
        profile_id: 1,
        service_day: 1,
        generation: 1,
        proof_sha256: [9; 32],
    };
    assert_untrusted_action_rejected(
        &mut store,
        9_991,
        current.scope.world(),
        current.scope.branch(),
        Action::TownTransportSettle {
            profile_id: 2,
            service_day: 1,
            generation: 1,
            proof_sha256: [9; 32],
        },
    )?;
    assert_untrusted_action_rejected(
        &mut store,
        9_992,
        current.scope.world() + 1,
        current.scope.branch(),
        untrusted_action,
    )?;
    assert_untrusted_action_rejected(
        &mut store,
        9_993,
        current.scope.world(),
        current.scope.branch() + 1,
        untrusted_action,
    )?;
    let proof_fault = store.settle_town_market_with_observer(10_001, |point| {
        if point == mycity_storage::WritePoint::TransportProofPublished {
            return Err(std::io::Error::other(
                "injected proof publication response loss",
            ));
        }
        Ok(())
    });
    if proof_fault.is_ok() {
        return Err("proof publication fault was not observed".into());
    }
    if store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .town
        .unwrap()
        .state
        .settled_day
        != 0
    {
        return Err("pre-WAL proof fault advanced the market cursor".into());
    }
    let applied_fault = store.settle_town_market_with_observer(10_001, |point| {
        if point == mycity_storage::WritePoint::Applied {
            return Err(std::io::Error::other("injected Applied response loss"));
        }
        Ok(())
    });
    if applied_fault.is_ok() {
        return Err("Applied response loss was not observed".into());
    }
    drop(store);
    let mut store = Store::open(Path::new(&dir), RULES).map_err(err)?;
    let receipt = store
        .receipt(10_001)
        .map_err(err)?
        .ok_or("settlement receipt missing after recovery")?
        .outcome
        .map_err(err)?;
    let replay = store.settle_town_market(10_001).map_err(err)?;
    if replay.model.map_err(err)? != receipt {
        return Err("same-operation replay mismatch".into());
    }
    let view = store.engine().map_err(err)?.world().view();
    let market = view.town.unwrap();
    let summary = market.state.last_summary;
    if summary.actual_sales.value() != expected {
        return Err(format!(
            "sales mismatch: expected {expected}, got {}",
            summary.actual_sales.value()
        ));
    }
    let first_proof_hash = market.state.last_transport.unwrap().issued_proof_sha256;
    let first_provider_cash = view
        .accounts
        .iter()
        .find(|account| account.id == 1)
        .ok_or("tariff provider account missing after day one")?
        .cash
        .value();
    if first_provider_cash - tariff_provider_cash_before != summary.freight.value() {
        return Err("day-one freight did not settle to the configured tariff provider".into());
    }
    let record = store.receipt(10_001).map_err(err)?.unwrap();
    let mut changed_actions: Vec<_> = record.actions.into_iter().flatten().collect();
    if let Action::TownTransportSettle { proof_sha256, .. } = &mut changed_actions[0] {
        *proof_sha256 = [0; 32];
    } else {
        return Err("unexpected settlement action".into());
    }
    let replay_view = store.engine().map_err(err)?.world().view();
    let changed = store
        .execute(
            Caller::System,
            Envelope {
                world: replay_view.scope.world(),
                branch: replay_view.scope.branch(),
                operation_id: 10_001,
                expected_revision: record.expected_revision,
                actions: changed_actions,
            },
        )
        .map_err(err)?;
    if changed.model != Err(mycity_core::transaction::TxError::OperationConflict) {
        return Err("same-operation changed proof was not rejected".into());
    }
    if store.settle_town_market(10_002).is_ok() {
        return Err("same-day second settlement was accepted".into());
    }
    for _ in 0..128 {
        timeline(
            &mut store,
            TimelineCommand::Advance {
                target: 2 * DAY,
                limit: 8192,
            },
        )?;
        if store
            .engine()
            .map_err(err)?
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            break;
        }
    }
    let second = store.settle_town_market(10_002).map_err(err)?;
    second.model.map_err(err)?;
    let second_market = store.engine().map_err(err)?.world().view().town.unwrap();
    let second_summary = second_market.state.last_summary;
    let second_proof_hash = second_market
        .state
        .last_transport
        .unwrap()
        .issued_proof_sha256;
    let second_provider_cash = store
        .engine()
        .map_err(err)?
        .world()
        .view()
        .accounts
        .into_iter()
        .find(|account| account.id == 1)
        .ok_or("tariff provider account missing after day two")?
        .cash
        .value();
    if second_provider_cash - first_provider_cash != second_summary.freight.value() {
        return Err("day-two freight did not settle to the configured tariff provider".into());
    }
    if second_market.state.last_transport.unwrap().issued_day != 2
        || second_market
            .state
            .last_transport
            .unwrap()
            .consumed_generation
            != 2
        || second_proof_hash == first_proof_hash
    {
        return Err("second day did not issue a fresh proof generation".into());
    }
    let current_revision = store.engine().map_err(err)?.world().view().revision;
    let cross_day = store.execute(
        Caller::System,
        Envelope {
            world: replay_view.scope.world(),
            branch: replay_view.scope.branch(),
            operation_id: 10_003,
            expected_revision: current_revision,
            actions: vec![Action::TownTransportSettle {
                profile_id: 1,
                service_day: 1,
                generation: 1,
                proof_sha256: first_proof_hash,
            }],
        },
    );
    if cross_day.is_ok() {
        return Err("raw cross-day proof was accepted without Store issuance".into());
    }
    store.checkpoint().map_err(err)?;
    if store.checkpoint_status().map_err(err)?.hot_receipts != 0 {
        return Err("checkpoint did not archive the settlement receipts".into());
    }
    let committed_hash = store.state_hash().map_err(err)?;
    drop(store);
    let mut store = Store::open(Path::new(&dir), RULES).map_err(err)?;
    if store.state_hash().map_err(err)? != committed_hash {
        return Err("cold restore state hash mismatch".into());
    }
    let archived = store
        .receipt(10_002)
        .map_err(err)?
        .ok_or("archived settlement receipt missing after cold recovery")?;
    let cold_replay = store.settle_town_market(10_002).map_err(err)?;
    if cold_replay.model.map_err(err)? != archived.outcome.map_err(err)? {
        return Err("cold archived same-operation replay mismatch".into());
    }
    let proof_path = Path::new(&dir).join(format!("town-proof-{}.json", hex(&second_proof_hash)));
    let original_proof = fs::read(&proof_path).map_err(err)?;
    let mut corrupt_proof = original_proof.clone();
    corrupt_proof[0] ^= 1;
    fs::write(&proof_path, &corrupt_proof).map_err(err)?;
    drop(store);
    if Store::open(Path::new(&dir), RULES).is_ok() {
        return Err("tampered checkpointed proof was accepted by cold recovery".into());
    }
    fs::write(&proof_path, &original_proof).map_err(err)?;
    let first_proof_path =
        Path::new(&dir).join(format!("town-proof-{}.json", hex(&first_proof_hash)));
    let first_proof = fs::read(&first_proof_path).map_err(err)?;
    fs::remove_file(&first_proof_path).map_err(err)?;
    if Store::open(Path::new(&dir), RULES).is_ok() {
        return Err("missing archived proof was accepted by cold recovery".into());
    }
    fs::write(&first_proof_path, first_proof).map_err(err)?;
    let store = Store::open(Path::new(&dir), RULES).map_err(err)?;
    let output = serde_json::json!({
        "scenario":scenario,
        "service_day":second_market.state.settled_day,
        "day_one_actual_sales":summary.actual_sales.value(),
        "day_two_actual_sales":second_summary.actual_sales.value(),
        "unmet_market_demand":second_summary.unmet_market_demand.value(),
        "freight_paid":second_summary.freight.value(),
        "settled_generation":second_market.state.last_transport.unwrap().consumed_generation,
        "proof_sha256":second_proof_hash,
        "store_sequence":store.committed_sequence(),
        "state_sha256":store.state_hash().map_err(err)?,
        "same_operation_replay":"passed",
        "cold_restore":"passed",
        "fault_recovery":"passed",
        "cross_day_reissue":"rejected",
        "same_day_double_settlement":"rejected",
        "tampered_checkpointed_proof":"rejected",
        "missing_archived_proof":"rejected",
        "archived_operation_replay":"passed"
    });
    println!("{}", serde_json::to_string_pretty(&output).map_err(err)?);
    let _ = fs::metadata(&dir).map_err(err)?;
    Ok(())
}
