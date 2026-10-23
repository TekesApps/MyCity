//! Shared paired-world scenario for the integration test and CLI example.
//! The OD assignment is map-derived and its proof is real, but household wages do not mutate OD.

use mycity_core::{
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TransportProofAsset, TripPartition,
    },
    spatial_transport_permissions::{
        SpatialAccessEdge, SpatialAccessSurface, SpatialAssetKind, SpatialAssetState,
        SpatialPermissionState, SPATIAL_ALLOW_CLOSE, SPATIAL_ALLOW_RESTORE,
    },
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, TaxClaimSource, DAY},
    transaction::{Action, Caller, Envelope, JournalRecord, TxError},
    transport::OdDemand,
    units::{Money, Quantity},
};
use mycity_map::{
    geometry::{Facing, Mode, Port},
    probe,
    scene::Scene,
    terrain::import,
    traffic_adapter::{
        adapt, AdapterIds, AdapterInput, FacilityOperation, RoadDirection, TerminalGrant,
        WALK_MODE_MASK,
    },
    Point,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fmt::Write as _, fs, path::Path};

pub const RULES: &str = "household-wage-spatial-equity-v1";
const OPENING_FIXTURE: &[u8] = include_bytes!("../../../../fixtures/m0-e0-opening.json");
const SPATIAL_FIXTURE: &[u8] = include_bytes!("../../../../fixtures/m0-spatial.json");
const HEIGHTMAP_FIXTURE: &[u8] = include_bytes!("../../../../fixtures/terrain/m0-flat16.png");
const TREASURY: u32 = 1;
const HOUSEHOLD: u32 = 2;
const SELLER: u32 = 3;
const SUPPLIER: u32 = 4;
const WAGE_EMPLOYER: u32 = 5;
const ROAD_ASSET: u32 = 70;
const DOOR_ASSET: u32 = 71;
const ROAD_EDGE: u32 = 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseReport {
    pub opening_store_owned_bytes: usize,
    pub opening_engine_owned_bytes: usize,
    pub opening_world_owned_bytes: u64,
    pub opening_receipt_sequence: u64,
    pub opening_state_hash: String,
    pub sales_by_day: [i64; 2],
    pub household_cash_after_day1: i64,
    pub household_cash_final: i64,
    pub taxable_basis_by_day: [i64; 2],
    pub tax_assessed_by_day: [i64; 2],
    pub retained_earnings_before_distribution: i64,
    pub rejected_dividend_reason: Option<String>,
    pub served_people_by_day: [[i64; 2]; 3],
    pub seller_payroll_paid: i64,
    pub supplemental_payroll_paid: i64,
    pub government_dividend: i64,
    pub household_dividend: i64,
    pub dividend_total: i64,
    pub treasury_after_fiscal_transfer: i64,
    pub supplier_fiscal_receipt: i64,
    pub total_cash: i128,
    pub state_hash: String,
    pub final_cold_store_owned_bytes: usize,
    pub final_cold_engine_owned_bytes: usize,
    pub final_cold_world_owned_bytes: u64,
    pub final_cold_receipt_sequence: u64,
    pub final_cold_state_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairReport {
    pub control: CaseReport,
    pub higher_wage: CaseReport,
}

enum RecordedOperation {
    Action(JournalRecord),
    Market {
        operation_id: u64,
        record: JournalRecord,
    },
}

fn m(value: i64) -> Money {
    Money::new(value).expect("fixture cents fit Money")
}

fn decode_hex_32(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64);
    let mut out = [0_u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => panic!("invalid scene hash"),
        };
        out[index] = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    out
}

fn profile_and_scene_hash() -> (TransportAllocationProfile, [u8; 32]) {
    let input: probe::Input = serde_json::from_slice(SPATIAL_FIXTURE).unwrap();
    let terrain = import(input.terrain, HEIGHTMAP_FIXTURE, 1, 0).unwrap();
    let scene = Scene::assemble(input.scene, &terrain, &terrain).unwrap();
    let demands = [
        OdDemand {
            source_id: 101,
            origin_entrance: 11,
            destination_entrance: 12,
            mode: WALK_MODE_MASK,
            people: 4,
        },
        OdDemand {
            source_id: 102,
            origin_entrance: 12,
            destination_entrance: 11,
            mode: WALK_MODE_MASK,
            people: 4,
        },
    ];
    let opening = adapt(
        &scene,
        AdapterInput {
            expected_map_id: 1,
            city_id: 1,
            revision: 1,
            building_owner_id: HOUSEHOLD,
            ids: AdapterIds {
                building_node: 1,
                anchor_node: 2,
                terminal_node: 3,
                building_entrance: 11,
                connector_forward_edge: 21,
                connector_reverse_edge: 22,
                road_forward_edge: 23,
                road_reverse_edge: ROAD_EDGE,
            },
            terminal: TerminalGrant {
                owner_id: SELLER,
                entrance_id: 12,
                port: Port {
                    point: Point { x: 1000, y: 1000 },
                    z_cm: 0,
                    mode: Mode::Walk,
                    facing: Facing::West,
                    clear_width_cm: 700,
                    can_enter: true,
                    can_exit: true,
                    authorized: true,
                },
            },
            connector: FacilityOperation {
                capacity_people_per_period: 4,
                free_flow_speed_cm_per_second: 100,
                open: true,
            },
            road: FacilityOperation {
                capacity_people_per_period: 4,
                free_flow_speed_cm_per_second: 100,
                open: true,
            },
            road_direction: RoadDirection::Both,
            demands: &demands,
        },
    )
    .unwrap();
    let profile = TransportAllocationProfile {
        schema: 1,
        world_id: 1,
        branch_id: 1,
        market_profile_id: 1,
        period_seconds: DAY as u32,
        allocation_scope: AllocationScope::SingleTownExclusive,
        demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
        road_capacity_effect: None,
        assignment: opening.assign().unwrap(),
        opening,
        partitions: vec![TripPartition {
            quote_id: 1,
            outbound_source: 101,
            return_source: 102,
            trip_count: 4,
            goods_per_trip: Quantity::new(10).unwrap(),
        }],
        fee_policy: FeePolicyInput {
            provider_account: SUPPLIER,
            cents_per_travel_second: 0,
        },
    };
    profile.validate().unwrap();
    let scene_hash = decode_hex_32(scene.state_hash().as_str());
    (profile, scene_hash)
}

fn permission_state(scene_hash: [u8; 32], profile_hash: [u8; 32]) -> SpatialPermissionState {
    SpatialPermissionState::from_genesis(
        1,
        1,
        vec![
            SpatialAssetState {
                asset_id: ROAD_ASSET,
                kind: SpatialAssetKind::Road,
                owner_account: SELLER,
                permit_authority_account: SUPPLIER,
                source_scene_sha256: scene_hash,
                revision: 1,
                current_profile_sha256: profile_hash,
                edges: vec![SpatialAccessEdge {
                    edge_id: ROAD_EDGE,
                    surface: SpatialAccessSurface::Road,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
                }],
            },
            SpatialAssetState {
                asset_id: DOOR_ASSET,
                kind: SpatialAssetKind::Building,
                owner_account: HOUSEHOLD,
                permit_authority_account: SUPPLIER,
                source_scene_sha256: scene_hash,
                revision: 1,
                current_profile_sha256: profile_hash,
                edges: vec![SpatialAccessEdge {
                    edge_id: 22,
                    surface: SpatialAccessSurface::Door,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
                }],
            },
        ],
    )
    .unwrap()
}

fn opening_bytes(allocation_hash: [u8; 32]) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(OPENING_FIXTURE).unwrap();
    value["sources"][1]["amount"] = json!(0);
    value["accounts"][1]["cash"] = json!(0);
    value["sources"].as_array_mut().unwrap().extend([
        json!({"id":6,"kind":"inventory_capital","amount":300}),
        json!({"id":7,"kind":"inventory_capital","amount":0}),
        json!({"id":8,"kind":"cash_capital","amount":5000}),
    ]);
    value["accounts"].as_array_mut().unwrap().push(json!({
        "id":WAGE_EMPLOYER,
        "role":"company",
        "cash":5000,
        "source":8
    }));
    value["stocks"].as_array_mut().unwrap().extend([
        json!({"id":2,"owner":SELLER,"commodity":"raw","quantity":30,"basis":300,"source":6}),
        json!({"id":3,"owner":SELLER,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    value["town"] = json!({
        "schema_version":1,
        "id":1,
        "transport_allocation_sha256":allocation_hash,
        "households":[{"id":1,"account":HOUSEHOLD,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers":[{"account":SELLER,"worker_limit":1},{"account":WAGE_EMPLOYER,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":SELLER,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":SELLER,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":TREASURY,"freight_account":TREASURY,"disposal_account":TREASURY}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":TREASURY,"utility_account":TREASURY,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters":[{
            "company":SELLER,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":{"kind":"person","id":7001},
            "investors":[
                {"party":{"kind":"person","id":7001},"account":HOUSEHOLD,"opening_shares":6003},
                {"party":{"kind":"treasury","id":1},"account":TREASURY,"opening_shares":0}
            ],
            "opening_sources":[
                {"source_id":3,"party":{"kind":"person","id":7001}},
                {"source_id":6,"party":{"kind":"person","id":7001}},
                {"source_id":7,"party":{"kind":"person","id":7001}}
            ]
        }],
        "tax_rules":[{"company":SELLER,"policy_id":77,"kind":"daily_realized_market_margin","authority_account":TREASURY,"rate_basis_points":5000}],
        "exit_policy":{
            "policy_id":1,"company_account":SELLER,"requester":HOUSEHOLD,"executor":TREASURY,"legal_clearance":"cleared",
            "priority_policy":{"order":["tax","wage","secured_debt","unsecured_debt","other"],"within_tier":"pro_rata_by_outstanding_stable_id"},
            "claims_complete":true,"tax_assessment_complete":true,"asset_register_complete":true,"employment_exit_terms_complete":true,
            "claims":[],"payroll":[{"id":1,"payee":HOUSEHOLD}]
        }
    });
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path) -> Store {
    let (profile, scene_hash) = profile_and_scene_hash();
    let profile_bytes = serde_json::to_vec(&profile).unwrap();
    let profile_hash = Sha256::digest(&profile_bytes).into();
    Store::create_spatial_with_transport_permissions(
        path,
        &opening_bytes(profile_hash),
        vec![],
        RULES,
        (SPATIAL_FIXTURE, HEIGHTMAP_FIXTURE),
        &profile_bytes,
        permission_state(scene_hash, profile_hash),
    )
    .unwrap()
}

fn request(store: &Store, action: Action) -> Envelope {
    let view = store.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn run(store: &mut Store, caller: Caller, action: Action) -> RecordedOperation {
    let envelope = request(store, action);
    let operation_id = envelope.operation_id;
    let outcome = store.execute(caller, envelope).unwrap();
    assert!(outcome.model.is_ok(), "operation failed: {outcome:?}");
    assert!(outcome.has_persistent_receipt);
    RecordedOperation::Action(store.receipt(operation_id).unwrap().unwrap())
}

fn replay_exact(store: &mut Store, record: JournalRecord) {
    let view = store.engine().unwrap().world().view();
    let caller = if record.actor == 0 {
        Caller::System
    } else {
        Caller::Account(record.actor)
    };
    let outcome = store
        .execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: record.operation_id,
                expected_revision: record.expected_revision,
                actions: record.actions.into_iter().flatten().collect(),
            },
        )
        .unwrap();
    assert_eq!(outcome.model, record.outcome);
    assert!(outcome.has_persistent_receipt);
}

fn reject_one_cent_dividend(store: &mut Store) -> JournalRecord {
    let envelope = request(
        store,
        Action::EquityDistributeRetainedEarnings {
            company: SELLER,
            requested: m(1),
        },
    );
    let operation_id = envelope.operation_id;
    let sequence_before = store.committed_sequence();
    let state_before = store.state_hash().unwrap();
    let accounts_before = [
        cash(store, TREASURY),
        cash(store, HOUSEHOLD),
        cash(store, SELLER),
        cash(store, SUPPLIER),
        cash(store, WAGE_EMPLOYER),
    ];
    let view = store.engine().unwrap().world().view();
    let equity_before = view.town.unwrap().state.equity_registers.as_slice()[0];
    let outcome = store.execute(Caller::Account(HOUSEHOLD), envelope).unwrap();
    assert_eq!(outcome.model, Err(TxError::InvalidOperation));
    assert!(outcome.has_persistent_receipt);
    assert_eq!(store.committed_sequence(), sequence_before + 1);
    assert_eq!(store.state_hash().unwrap(), state_before);
    assert_eq!(
        [
            cash(store, TREASURY),
            cash(store, HOUSEHOLD),
            cash(store, SELLER),
            cash(store, SUPPLIER),
            cash(store, WAGE_EMPLOYER),
        ],
        accounts_before
    );
    let view = store.engine().unwrap().world().view();
    let equity_after = view.town.unwrap().state.equity_registers.as_slice()[0];
    assert_eq!(equity_after, equity_before);
    let record = store.receipt(operation_id).unwrap().unwrap();
    assert_eq!(record.outcome, Err(TxError::InvalidOperation));
    record
}

fn replay_market(store: &mut Store, operation_id: u64, record: JournalRecord) {
    assert!(record.outcome.is_ok());
    let outcome = store.settle_town_market(operation_id).unwrap();
    assert!(outcome.has_persistent_receipt);
    assert_eq!(outcome.model, record.outcome);
}

fn cash(store: &Store, account: u32) -> i64 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .unwrap()
        .cash
        .value()
}

fn total_cash(store: &Store) -> i128 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum()
}

fn capacity_snapshot(store: &Store) -> (usize, usize, u64, u64, String) {
    let engine = store.engine().unwrap();
    let world_owned_bytes = engine.world().layout()["owned_capacity_bytes"]
        .as_u64()
        .expect("world layout reports owned bytes");
    (
        store.owned_capacity_bytes().unwrap(),
        engine.owned_capacity_bytes(),
        world_owned_bytes,
        store.committed_sequence(),
        store.state_hash().unwrap(),
    )
}

fn advance_to(store: &mut Store, target: u64, records: &mut Vec<RecordedOperation>) {
    for _ in 0..512 {
        records.push(run(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        ));
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("Timeline did not reach {target}");
}

fn add_payrolls(store: &mut Store, high_wage: bool, records: &mut Vec<RecordedOperation>) {
    records.push(run(
        store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    ));
    records.push(run(
        store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: SELLER,
                    payee: HOUSEHOLD,
                    home_city: 0,
                    hourly_cents: m(100),
                    cycle: Cycle::Daily,
                    ends_at: 2 * DAY,
                },
            },
        },
    ));
    if high_wage {
        records.push(run(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::AddPayroll {
                    spec: PayrollSpec {
                        id: 2,
                        payer: WAGE_EMPLOYER,
                        payee: HOUSEHOLD,
                        home_city: 0,
                        hourly_cents: m(30),
                        cycle: Cycle::Daily,
                        ends_at: 2 * DAY,
                    },
                },
            },
        ));
    }
}

fn proof_for_day(store: &Store, world: &Path, day: u64) -> TransportProofAsset {
    let market = store.engine().unwrap().world().view().town.unwrap();
    let transport = market.state.last_transport.unwrap();
    let digest =
        transport
            .issued_proof_sha256
            .iter()
            .fold(String::with_capacity(64), |mut text, byte| {
                write!(&mut text, "{byte:02x}").unwrap();
                text
            });
    let path = world.join(format!("town-proof-{digest}.json"));
    let bytes = fs::read(path).unwrap();
    let found: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(found, transport.issued_proof_sha256);
    let proof: TransportProofAsset = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(proof.service_day, day);
    proof
}

fn served_pair(proof: &TransportProofAsset) -> [i64; 2] {
    [101, 102].map(|source| {
        i64::from(
            proof
                .assignment
                .demand_outcomes
                .iter()
                .find(|outcome| outcome.source_id == source)
                .unwrap()
                .served_people,
        )
    })
}

fn tax_for_period(store: &Store, day: u64) -> Option<TaxClaimSource> {
    let timeline = store.engine().unwrap().timeline().unwrap();
    timeline.claims.iter().find_map(|claim| {
        timeline
            .tax_source_for_claim(claim)
            .filter(|source| source.period == day)
    })
}

fn settle_market(
    store: &mut Store,
    world: &Path,
    day: u64,
    records: &mut Vec<RecordedOperation>,
) -> (i64, [i64; 2]) {
    let operation_id = store.committed_sequence() + 1;
    let outcome = store.settle_town_market(operation_id).unwrap();
    assert!(outcome.has_persistent_receipt);
    let receipt = outcome
        .model
        .unwrap_or_else(|error| panic!("verified town settlement rejected: {error:?}"));
    let sales = receipt
        .town
        .expect("market settlement summary")
        .actual_sales
        .value();
    let proof = proof_for_day(store, world, day);
    let served = served_pair(&proof);
    assert_eq!(served, [4, 4], "fixed input OD should remain reachable");
    assert!(proof
        .assignment
        .edge_flows
        .iter()
        .any(|flow| flow.edge_id == ROAD_EDGE && flow.people > 0));
    let record = store.receipt(operation_id).unwrap().unwrap();
    assert_eq!(record.outcome.as_ref().unwrap(), &receipt);
    records.push(RecordedOperation::Market {
        operation_id,
        record,
    });
    (sales, served)
}

fn execute_case(root: &Path, high_wage: bool) -> CaseReport {
    let world = root.join(if high_wage { "higher-wage" } else { "control" });
    let mut store = create(&world);
    let (
        opening_store_owned_bytes,
        opening_engine_owned_bytes,
        opening_world_owned_bytes,
        opening_receipt_sequence,
        opening_state_hash,
    ) = capacity_snapshot(&store);
    assert_eq!(opening_receipt_sequence, 0);
    assert_eq!(cash(&store, HOUSEHOLD), 0);
    assert_eq!(total_cash(&store), 2_805_000);
    let mut records = Vec::new();
    records.push(run(
        &mut store,
        Caller::Account(TREASURY),
        Action::EquityContribute {
            company: SELLER,
            amount: m(100_000),
        },
    ));
    add_payrolls(&mut store, high_wage, &mut records);

    let mut sales = [0_i64; 2];
    let mut served = [[0_i64; 2]; 3];
    let mut household_after_day1 = 0;
    for day in 1..=3 {
        advance_to(&mut store, day * DAY, &mut records);
        let (actual_sales, actual_served) = settle_market(&mut store, &world, day, &mut records);
        served[(day - 1) as usize] = actual_served;
        if day == 1 {
            sales[0] = actual_sales;
            household_after_day1 = cash(&store, HOUSEHOLD);
        } else if day == 2 {
            sales[1] = actual_sales;
        } else {
            assert_eq!(actual_sales, 0, "both payroll agreements end after day 2");
        }
    }
    assert_eq!(total_cash(&store), 2_805_000);

    let tax_source = |day| tax_for_period(&store, day);
    let day1_tax = tax_source(1);
    let day2_tax = tax_source(2);
    let tax_rows = [day1_tax, day2_tax].map(|source| {
        source.map_or((0, 0), |row| {
            (row.taxable_basis.value(), row.assessed.value())
        })
    });
    let taxable_basis_by_day = [tax_rows[0].0, tax_rows[1].0];
    let tax_assessed_by_day = [tax_rows[0].1, tax_rows[1].1];
    let timeline = store.engine().unwrap().timeline().unwrap();
    for claim in timeline.claims.iter().filter(|claim| {
        timeline
            .tax_source_for_claim(claim)
            .is_some_and(|source| source.company == SELLER)
    }) {
        assert_eq!(claim.paid, claim.principal);
        assert!(claim.processed);
    }
    let payroll = |id| {
        timeline
            .payroll
            .iter()
            .find(|row| row.spec.id == id)
            .unwrap()
            .paid
            .value()
    };
    let seller_payroll_paid = payroll(1);
    let supplemental_payroll_paid = if high_wage { payroll(2) } else { 0 };
    let earnings_cursor = store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .tax_cursors
        .as_slice()[0];
    let retained_earnings_before_distribution = earnings_cursor.retained_earnings_cents;

    let mut government_dividend = 0;
    let mut household_dividend = 0;
    let mut dividend_total = 0;
    let mut rejected_dividend_reason = None;
    let mut supplier_fiscal_receipt = 0;
    let mut treasury_after_fiscal_transfer = cash(&store, TREASURY);
    if high_wage {
        assert_eq!(retained_earnings_before_distribution, 90);
        assert_eq!(earnings_cursor.cumulative_assessed, m(90));
        let before_dividend = cash(&store, TREASURY);
        let household_before_dividend = cash(&store, HOUSEHOLD);
        records.push(run(
            &mut store,
            Caller::Account(HOUSEHOLD),
            Action::EquityDistributeRetainedEarnings {
                company: SELLER,
                requested: m(retained_earnings_before_distribution),
            },
        ));
        government_dividend = cash(&store, TREASURY) - before_dividend;
        household_dividend = cash(&store, HOUSEHOLD) - household_before_dividend;
        dividend_total = government_dividend + household_dividend;
        assert!(government_dividend > 0);
        assert!(household_dividend > 0);
        assert_eq!(dividend_total, retained_earnings_before_distribution);
        let from = store
            .engine()
            .unwrap()
            .world()
            .account_ref(TREASURY)
            .unwrap();
        let to = store
            .engine()
            .unwrap()
            .world()
            .account_ref(SUPPLIER)
            .unwrap();
        records.push(run(
            &mut store,
            Caller::Account(TREASURY),
            Action::Transfer {
                from,
                to,
                amount: m(government_dividend),
            },
        ));
        supplier_fiscal_receipt = government_dividend;
        treasury_after_fiscal_transfer = cash(&store, TREASURY);
        assert_eq!(treasury_after_fiscal_transfer, before_dividend);
    } else {
        assert!(retained_earnings_before_distribution <= 0);
        let rejected = reject_one_cent_dividend(&mut store);
        records.push(RecordedOperation::Action(rejected));
        rejected_dividend_reason = Some(format!("{:?}", TxError::InvalidOperation));
    }
    assert_eq!(total_cash(&store), 2_805_000);

    let state_hash = store.state_hash().unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut recovered = Store::open(&world, RULES).unwrap();
    assert_eq!(recovered.state_hash().unwrap(), state_hash);
    for operation in records {
        match operation {
            RecordedOperation::Action(record) => replay_exact(&mut recovered, record),
            RecordedOperation::Market {
                operation_id,
                record,
            } => replay_market(&mut recovered, operation_id, record),
        }
    }
    assert_eq!(recovered.state_hash().unwrap(), state_hash);
    assert_eq!(total_cash(&recovered), 2_805_000);
    assert_eq!(
        cash(&recovered, HOUSEHOLD),
        cash_after_sales(&sales, high_wage, household_dividend)
    );
    assert_eq!(cash(&recovered, TREASURY), treasury_after_fiscal_transfer);
    let (
        final_cold_store_owned_bytes,
        final_cold_engine_owned_bytes,
        final_cold_world_owned_bytes,
        final_cold_receipt_sequence,
        final_cold_state_hash,
    ) = capacity_snapshot(&recovered);
    assert!(final_cold_receipt_sequence > opening_receipt_sequence);
    assert_eq!(final_cold_state_hash, state_hash);

    CaseReport {
        opening_store_owned_bytes,
        opening_engine_owned_bytes,
        opening_world_owned_bytes,
        opening_receipt_sequence,
        opening_state_hash,
        sales_by_day: sales,
        household_cash_after_day1: household_after_day1,
        household_cash_final: cash(&recovered, HOUSEHOLD),
        taxable_basis_by_day,
        tax_assessed_by_day,
        retained_earnings_before_distribution,
        rejected_dividend_reason,
        served_people_by_day: served,
        seller_payroll_paid,
        supplemental_payroll_paid,
        government_dividend,
        household_dividend,
        dividend_total,
        treasury_after_fiscal_transfer,
        supplier_fiscal_receipt,
        total_cash: total_cash(&recovered),
        state_hash,
        final_cold_store_owned_bytes,
        final_cold_engine_owned_bytes,
        final_cold_world_owned_bytes,
        final_cold_receipt_sequence,
        final_cold_state_hash,
    }
}

fn cash_after_sales(sales: &[i64; 2], high_wage: bool, household_dividend: i64) -> i64 {
    let received_wages = 1_600 + if high_wage { 480 } else { 0 };
    let spending = sales.iter().sum::<i64>() * 101;
    received_wages - spending + household_dividend
}

pub fn run_pair(artifact_root: &Path) -> PairReport {
    fs::create_dir_all(artifact_root).unwrap();
    let control = execute_case(artifact_root, false);
    let higher_wage = execute_case(artifact_root, true);
    PairReport {
        control,
        higher_wage,
    }
}

pub fn report_json(report: &PairReport) -> Value {
    json!({
        "control": {
            "opening_store_owned_bytes": report.control.opening_store_owned_bytes,
            "opening_engine_owned_bytes": report.control.opening_engine_owned_bytes,
            "opening_world_owned_bytes": report.control.opening_world_owned_bytes,
            "opening_receipt_sequence": report.control.opening_receipt_sequence,
            "opening_state_hash": report.control.opening_state_hash,
            "sales_by_day": report.control.sales_by_day,
            "household_cash_after_day1": report.control.household_cash_after_day1,
            "household_cash_final": report.control.household_cash_final,
            "taxable_basis_by_day": report.control.taxable_basis_by_day,
            "tax_assessed_by_day": report.control.tax_assessed_by_day,
            "retained_earnings_before_distribution": report.control.retained_earnings_before_distribution,
            "rejected_dividend_reason": report.control.rejected_dividend_reason,
            "served_people_by_day": report.control.served_people_by_day,
            "seller_payroll_paid": report.control.seller_payroll_paid,
            "supplemental_payroll_paid": report.control.supplemental_payroll_paid,
            "household_dividend": report.control.household_dividend,
            "government_dividend": report.control.government_dividend,
            "dividend_total": report.control.dividend_total,
            "state_hash": report.control.state_hash,
            "final_cold_store_owned_bytes": report.control.final_cold_store_owned_bytes,
            "final_cold_engine_owned_bytes": report.control.final_cold_engine_owned_bytes,
            "final_cold_world_owned_bytes": report.control.final_cold_world_owned_bytes,
            "final_cold_receipt_sequence": report.control.final_cold_receipt_sequence,
            "final_cold_state_hash": report.control.final_cold_state_hash,
        },
        "higher_wage": {
            "opening_store_owned_bytes": report.higher_wage.opening_store_owned_bytes,
            "opening_engine_owned_bytes": report.higher_wage.opening_engine_owned_bytes,
            "opening_world_owned_bytes": report.higher_wage.opening_world_owned_bytes,
            "opening_receipt_sequence": report.higher_wage.opening_receipt_sequence,
            "opening_state_hash": report.higher_wage.opening_state_hash,
            "sales_by_day": report.higher_wage.sales_by_day,
            "household_cash_after_day1": report.higher_wage.household_cash_after_day1,
            "household_cash_final": report.higher_wage.household_cash_final,
            "taxable_basis_by_day": report.higher_wage.taxable_basis_by_day,
            "tax_assessed_by_day": report.higher_wage.tax_assessed_by_day,
            "retained_earnings_before_distribution": report.higher_wage.retained_earnings_before_distribution,
            "rejected_dividend_reason": report.higher_wage.rejected_dividend_reason,
            "served_people_by_day": report.higher_wage.served_people_by_day,
            "seller_payroll_paid": report.higher_wage.seller_payroll_paid,
            "supplemental_payroll_paid": report.higher_wage.supplemental_payroll_paid,
            "government_dividend": report.higher_wage.government_dividend,
            "household_dividend": report.higher_wage.household_dividend,
            "dividend_total": report.higher_wage.dividend_total,
            "supplier_fiscal_receipt": report.higher_wage.supplier_fiscal_receipt,
            "treasury_after_fiscal_transfer": report.higher_wage.treasury_after_fiscal_transfer,
            "state_hash": report.higher_wage.state_hash,
            "final_cold_store_owned_bytes": report.higher_wage.final_cold_store_owned_bytes,
            "final_cold_engine_owned_bytes": report.higher_wage.final_cold_engine_owned_bytes,
            "final_cold_world_owned_bytes": report.higher_wage.final_cold_world_owned_bytes,
            "final_cold_receipt_sequence": report.higher_wage.final_cold_receipt_sequence,
            "final_cold_state_hash": report.higher_wage.final_cold_state_hash,
        },
        "total_cash": report.higher_wage.total_cash,
        "route": "map-derived spatial profile with fixed OD; wages do not alter demand or trip origins",
        "native_peak": "not_measured",
    })
}

/// Open an existing isolated working copy; the caller preserves and verifies the original.
#[cfg_attr(test, allow(dead_code))]
pub fn verify_open(world: &Path) -> Result<Value, String> {
    if !world.is_dir() {
        return Err(format!(
            "existing working copy not found: {}",
            world.display()
        ));
    }
    let world =
        fs::canonicalize(world).map_err(|error| format!("resolve working copy: {error}"))?;
    eprintln!("Store::open may replay or truncate recovery data; using caller-provided isolated working copy: {}", world.display());
    let store =
        Store::open(&world, RULES).map_err(|error| format!("open working copy: {error:?}"))?;
    let snapshot = capacity_snapshot(&store);
    Ok(json!({
        "mode": "verify_open_working_copy",
        "rules": RULES,
        "working_copy": world,
        "model_commands_executed": 0,
        "store_owned_bytes": snapshot.0,
        "engine_owned_bytes": snapshot.1,
        "world_owned_bytes": snapshot.2,
        "receipt_sequence": snapshot.3,
        "state_hash": snapshot.4,
        "native_peak": "not_measured",
    }))
}
