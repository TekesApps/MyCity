//! Finite same-Store equity, tax and spatial-access integration scenario.
//!
//! The Engine, Timeline, TownSettle, WAL and checkpoint remain the only state writers.
//! Actual candidate run 246 passed the four-day fixed arithmetic oracle and cold-open replay.
//! This is a microfixture, not whole-city throughput, economic calibration or 16GB acceptance.

use mycity_core::{
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TripPartition,
    },
    spatial_transport_permissions::{
        request_sha256, SpatialAccessEdge, SpatialAccessOperation, SpatialAccessSurface,
        SpatialApprovalRole, SpatialAssetKind, SpatialAssetState, SpatialPermissionCommand,
        SpatialPermissionState, SpatialUpdateApproval, SpatialUpdateRequest, SPATIAL_ALLOW_CLOSE,
        SPATIAL_ALLOW_RESTORE,
    },
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    transport::OdDemand,
    units::{GameTime, Money, Quantity},
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
use mycity_storage::{spatial_transport_updates::*, Store};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fmt::Write as _, fs, path::PathBuf, time::Instant};

const RULES: &str = "joint-equity-spatial-candidate-v1";
const OPENING_FIXTURE: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");
const SPATIAL_FIXTURE: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const HEIGHTMAP_FIXTURE: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");

const TREASURY: u32 = 1;
const HOUSEHOLD: u32 = 2;
const COMPANY: u32 = 3;
const LENDER_AND_ROAD_AUTHORITY: u32 = 4;
// This fixture delegates execution to account 2, distinct from applicant/owner 3 and permit 4.
const ROAD_EXECUTOR: u32 = HOUSEHOLD;
const ROAD_ASSET: u32 = 70;
const DOOR_ASSET: u32 = 71;
const ROAD_EDGE: u32 = 24;

fn m(value: i64) -> Money {
    Money::new(value).expect("bounded fixture cents")
}

fn scene_profile_and_hash() -> (TransportAllocationProfile, [u8; 32]) {
    // Same map/terrain adapter path as spatial_transport_scenario.rs. This is a real
    // map-derived walk graph; the single managed road edge is 24, and the town's assigned
    // consumer/seller are the corresponding home/shop entrances.
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
                owner_id: COMPANY,
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
            provider_account: LENDER_AND_ROAD_AUTHORITY,
            cents_per_travel_second: 0,
        },
    };
    profile.validate().unwrap();
    // Scene::state_hash is the map-side geometry identity used in the F permission genesis.
    let scene_hash = decode_hex_32(scene.state_hash().as_str());
    (profile, scene_hash)
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

fn permission_state(scene_hash: [u8; 32], profile_hash: [u8; 32]) -> SpatialPermissionState {
    SpatialPermissionState::from_genesis(
        1,
        1,
        vec![
            SpatialAssetState {
                asset_id: ROAD_ASSET,
                kind: SpatialAssetKind::Road,
                owner_account: COMPANY,
                permit_authority_account: LENDER_AND_ROAD_AUTHORITY,
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
                permit_authority_account: LENDER_AND_ROAD_AUTHORITY,
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
    // Reuse D's account/source skeleton and tax/equity contract. The sole material adjustment
    // is raw stock 2: 20 q / 200 cents, enough for exactly the day-1 and day-2 production runs.
    let mut value: Value = serde_json::from_slice(OPENING_FIXTURE).unwrap();
    value["sources"].as_array_mut().unwrap().extend([
        json!({"id":6,"kind":"inventory_capital","amount":200}),
        json!({"id":7,"kind":"inventory_capital","amount":0}),
    ]);
    value["stocks"].as_array_mut().unwrap().extend([
        json!({"id":2,"owner":COMPANY,"commodity":"raw","quantity":20,"basis":200,"source":6}),
        json!({"id":3,"owner":COMPANY,"commodity":"product","quantity":0,"basis":0,"source":7}),
    ]);
    value["town"] = json!({
        "schema_version":1,
        "id":1,
        "transport_allocation_sha256":allocation_hash,
        "households":[{"id":1,"account":HOUSEHOLD,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":102}],
        "employers":[{"account":COMPANY,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":COMPANY,"stock_id":3,"good":1,"shelf_price_per_q":100}],
        "recipes":[{"recipe_id":1,"employer":COMPANY,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":10,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":TREASURY,"freight_account":TREASURY,"disposal_account":TREASURY}],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":TREASURY,"utility_account":TREASURY,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}],
        "equity_charters":[{
            "company":COMPANY,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},
            "distribution_authority":{"kind":"person","id":7001},
            "investors":[
                {"party":{"kind":"person","id":7001},"account":HOUSEHOLD,"opening_shares":6002},
                {"party":{"kind":"treasury","id":1},"account":TREASURY,"opening_shares":0}
            ],
            "opening_sources":[
                {"source_id":3,"party":{"kind":"person","id":7001}},
                {"source_id":6,"party":{"kind":"person","id":7001}},
                {"source_id":7,"party":{"kind":"person","id":7001}}
            ]
        }],
        "tax_rules":[{"company":COMPANY,"policy_id":77,"kind":"daily_realized_market_margin","authority_account":TREASURY,"rate_basis_points":5000}],
        "exit_policy":{
            "policy_id":1,"company_account":COMPANY,"requester":HOUSEHOLD,"executor":TREASURY,"legal_clearance":"cleared",
            "priority_policy":{"order":["tax","wage","secured_debt","unsecured_debt","other"],"within_tier":"pro_rata_by_outstanding_stable_id"},
            "claims_complete":true,"tax_assessment_complete":true,"asset_register_complete":true,"employment_exit_terms_complete":true,
            "claims":[{"id":100,"payee":LENDER_AND_ROAD_AUTHORITY,"class":"unsecured_debt"}],
            "payroll":[{"id":1,"payee":HOUSEHOLD}]
        }
    });
    serde_json::to_vec(&value).unwrap()
}

fn create_joint_store(path: &std::path::Path) -> Store {
    let (profile, scene_hash) = scene_profile_and_hash();
    let raw_profile = serde_json::to_vec(&profile).unwrap();
    let profile_hash = Sha256::digest(&raw_profile).into();
    Store::create_spatial_with_transport_permissions(
        path,
        &opening_bytes(profile_hash),
        vec![],
        RULES,
        (SPATIAL_FIXTURE, HEIGHTMAP_FIXTURE),
        &raw_profile,
        permission_state(scene_hash, profile_hash),
    )
    .unwrap()
}

fn envelope(store: &Store, action: Action) -> Envelope {
    let view = store.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn execute(store: &mut Store, caller: Caller, action: Action) -> Envelope {
    let request = envelope(store, action);
    let result = store.execute(caller, request.clone()).unwrap();
    assert!(
        result.model.is_ok(),
        "transaction rejected: {:?}; caller={caller:?}; request={request:?}",
        result.model
    );
    assert!(result.has_persistent_receipt);
    request
}

fn advance_to(store: &mut Store, target: u64) {
    for _ in 0..512 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8192,
                },
            },
        );
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
    panic!("timeline did not finish at {target}");
}

fn request_road_change(
    store: &mut Store,
    world: &std::path::Path,
    operation: SpatialAccessOperation,
    effective_day: u64,
) -> (SpatialUpdateRequest, [u8; 32]) {
    let permission = store.engine().unwrap().spatial_permissions().unwrap();
    let asset = permission
        .assets()
        .iter()
        .find(|row| row.asset_id == ROAD_ASSET)
        .unwrap();
    let view = store.engine().unwrap().world().view();
    let market = view.town.as_ref().unwrap();
    let current = VerifiedAllocationAsset::read_immutable(
        world,
        asset.current_profile_sha256,
        view.scope.world(),
        view.scope.branch(),
        market.profile.id,
    )
    .unwrap();
    let mut proposed = current.profile().opening.clone();
    proposed.revision += 1;
    proposed
        .edges
        .iter_mut()
        .find(|edge| edge.id == ROAD_EDGE)
        .unwrap()
        .open = operation == SpatialAccessOperation::Restore;
    let candidate = preview_profile_asset(&current.as_ref(), proposed.clone()).unwrap();
    let request = SpatialUpdateRequest {
        request_id: permission.request_id_highwater + 1,
        applicant_account: COMPANY,
        executor_account: ROAD_EXECUTOR,
        asset_id: ROAD_ASSET,
        operation,
        surface: SpatialAccessSurface::Road,
        edge_id: ROAD_EDGE,
        expected_revision: asset.revision,
        candidate_profile_sha256: candidate.sha256,
        effective_time: GameTime::new(i64::try_from(effective_day * DAY).unwrap()).unwrap(),
        parent_profile_sha256: asset.current_profile_sha256,
    };
    let revision = permission.revision;
    let authorization_context = permission.authorization_context(request.asset_id).unwrap();
    store
        .stage_spatial_transport_candidate(request, proposed)
        .unwrap();
    let digest = request_sha256(authorization_context, &request).unwrap();
    execute(
        store,
        Caller::Account(COMPANY),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Request {
                expected_state_revision: revision,
                request,
            },
        },
    );
    let revision = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .revision;
    execute(
        store,
        Caller::Account(COMPANY),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Approve {
                expected_state_revision: revision,
                approval: SpatialUpdateApproval {
                    request_id: request.request_id,
                    request_sha256: digest,
                    approver_account: COMPANY,
                    role: SpatialApprovalRole::OwnerConsent,
                    approved: true,
                },
            },
        },
    );
    let revision = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .revision;
    execute(
        store,
        Caller::Account(LENDER_AND_ROAD_AUTHORITY),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Approve {
                expected_state_revision: revision,
                approval: SpatialUpdateApproval {
                    request_id: request.request_id,
                    request_sha256: digest,
                    approver_account: LENDER_AND_ROAD_AUTHORITY,
                    role: SpatialApprovalRole::AuthorityPermit,
                    approved: true,
                },
            },
        },
    );
    (request, digest)
}

fn apply_road_change(store: &mut Store, request_id: u64, digest: [u8; 32]) -> Envelope {
    let expected_state_revision = store
        .engine()
        .unwrap()
        .spatial_permissions()
        .unwrap()
        .revision;
    execute(
        store,
        Caller::Account(ROAD_EXECUTOR),
        Action::SpatialPermission {
            command: SpatialPermissionCommand::Apply {
                expected_state_revision,
                request_id,
                request_sha256: digest,
            },
        },
    )
}

fn settled_sales_tax_basis_and_assessed(
    store: &Store,
    world: &std::path::Path,
    expected_day: u64,
) -> (i64, i64, i64, u32, u32, [u8; 32]) {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    let market = view.town.unwrap();
    assert_eq!(market.state.settled_day, expected_day);
    let summary = market.state.last_summary;
    let timeline = engine.timeline().unwrap();
    let tax_source = timeline.claims.iter().find_map(|claim| {
        timeline
            .tax_source_for_claim(claim)
            .filter(|source| source.period == market.state.settled_day)
    });
    let taxable_basis = tax_source.map_or(0, |source| source.taxable_basis.value());
    let assessed = tax_source.map_or(0, |source| source.assessed.value());
    let transport = market.state.last_transport.unwrap();
    let proof_path = world.join(format!(
        "town-proof-{}.json",
        transport
            .issued_proof_sha256
            .iter()
            .fold(String::with_capacity(64), |mut text, byte| {
                write!(&mut text, "{byte:02x}").expect("String hex writer");
                text
            })
    ));
    let proof_bytes = fs::read(proof_path).unwrap();
    let proof_hash: [u8; 32] = Sha256::digest(&proof_bytes).into();
    assert_eq!(proof_hash, transport.issued_proof_sha256);
    let proof: mycity_core::market_access::TransportProofAsset =
        serde_json::from_slice(&proof_bytes).unwrap();
    assert_eq!(proof.service_day, expected_day);
    let served = |source_id| {
        proof
            .assignment
            .demand_outcomes
            .iter()
            .find(|entry| entry.source_id == source_id)
            .unwrap()
            .served_people
    };
    (
        summary.actual_sales.value(),
        taxable_basis,
        assessed,
        served(101),
        served(102),
        proof_hash,
    )
}

/// Executable scenario sequence. One Store only:
/// 1. issue real treasury/person contributions and a documented zero-interest 500c loan;
/// 2. day 1 open proof -> real sale/tax; day 2 closed proof -> zero sale/tax; day 3 restored
///    proof -> real sale/tax; day 4 pays the due tax and loan claims; distribute retained earnings;
/// 3. checkpoint, cold open, verify same hash and replay the persisted day receipts.
fn run() -> Result<(), String> {
    let started = Instant::now();
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("pass a fresh world directory")?;
    if path.exists() {
        return Err("world path must be fresh".into());
    }
    let mut store = create_joint_store(&path);
    let initial_store_owned = store.owned_capacity_bytes().map_err(|e| format!("{e:?}"))?;
    let initial_cash_sum: i64 = store
        .engine()
        .map_err(|e| format!("{e:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .map(|row| row.cash.value())
        .sum();

    let contribution_a = execute(
        &mut store,
        Caller::Account(TREASURY),
        Action::EquityContribute {
            company: COMPANY,
            amount: m(10_000),
        },
    );
    let contribution_b = execute(
        &mut store,
        Caller::Account(HOUSEHOLD),
        Action::EquityContribute {
            company: COMPANY,
            amount: m(10_000),
        },
    );
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: COMPANY,
                    payee: HOUSEHOLD,
                    home_city: 0,
                    hourly_cents: m(100),
                    cycle: Cycle::Daily,
                    ends_at: 2 * DAY,
                },
            },
        },
    );
    execute(
        &mut store,
        Caller::System,
        Action::Timeline {
            command: TimelineCommand::Lend {
                id: 100,
                lender: LENDER_AND_ROAD_AUTHORITY,
                borrower: COMPANY,
                principal: m(500),
                due: 4 * DAY,
            },
        },
    );

    advance_to(&mut store, DAY);
    let day1_op = store.committed_sequence() + 1;
    let day1 = store
        .settle_town_market(day1_op)
        .map_err(|e| format!("{e:?}"))?
        .model
        .map_err(|e| format!("{e:?}"))?;
    let (day1_sales, day1_tax_basis, day1_tax, day1_outbound, day1_return, day1_proof) =
        settled_sales_tax_basis_and_assessed(&store, &path, 1);
    assert_eq!(
        (
            day1_sales,
            day1_tax_basis,
            day1_tax,
            day1_outbound,
            day1_return
        ),
        (10, 90, 45, 4, 4)
    );

    let (close, close_hash) =
        request_road_change(&mut store, &path, SpatialAccessOperation::Close, 2);
    advance_to(&mut store, 2 * DAY);
    let close_apply = apply_road_change(&mut store, close.request_id, close_hash);
    let day2_op = store.committed_sequence() + 1;
    let day2 = store
        .settle_town_market(day2_op)
        .map_err(|e| format!("{e:?}"))?
        .model
        .map_err(|e| format!("{e:?}"))?;
    let (day2_sales, day2_tax_basis, day2_tax, day2_outbound, day2_return, day2_proof) =
        settled_sales_tax_basis_and_assessed(&store, &path, 2);
    assert_eq!(
        (
            day2_sales,
            day2_tax_basis,
            day2_tax,
            day2_outbound,
            day2_return
        ),
        (0, 0, 0, 0, 4)
    );

    let (restore, restore_hash) =
        request_road_change(&mut store, &path, SpatialAccessOperation::Restore, 3);
    advance_to(&mut store, 3 * DAY);
    let restore_apply = apply_road_change(&mut store, restore.request_id, restore_hash);
    let day3_op = store.committed_sequence() + 1;
    let day3 = store
        .settle_town_market(day3_op)
        .map_err(|e| format!("{e:?}"))?
        .model
        .map_err(|e| format!("{e:?}"))?;
    let (day3_sales, day3_tax_basis, day3_tax, day3_outbound, day3_return, day3_proof) =
        settled_sales_tax_basis_and_assessed(&store, &path, 3);
    assert_eq!(
        (
            day3_sales,
            day3_tax_basis,
            day3_tax,
            day3_outbound,
            day3_return
        ),
        (10, 90, 45, 4, 4)
    );
    assert_ne!(day1_proof, day2_proof);
    assert_ne!(day2_proof, day3_proof);

    // D's actual Timeline claim-backed debt/tax machinery pays the day-1 and day-3 margin
    // assessments and the lender's 500c principal on day 4. No caller directly edits a claim.
    advance_to(&mut store, 4 * DAY);
    let day4_op = store.committed_sequence() + 1;
    let day4 = store
        .settle_town_market(day4_op)
        .map_err(|e| format!("{e:?}"))?
        .model
        .map_err(|e| format!("{e:?}"))?;
    let company_before_dividend = account_cash(&store, COMPANY)?;
    let dividend = execute(
        &mut store,
        Caller::Account(HOUSEHOLD),
        Action::EquityDistributeRetainedEarnings {
            company: COMPANY,
            requested: m(10_000),
        },
    );
    let (dividend_cents, loan_cleared, unpaid_tax) = {
        let engine = store.engine().unwrap();
        let view = engine.world().view();
        let market = view.town.unwrap();
        let dividend = market.state.equity_registers.as_slice()[0]
            .cumulative_dividends_distributed
            .value();
        let timeline = engine.timeline().unwrap();
        let lender_claim = timeline
            .claims
            .iter()
            .find(|claim| claim.id == 100)
            .unwrap();
        let loan_cleared = lender_claim.paid == lender_claim.principal && lender_claim.processed;
        let unpaid_tax = timeline.claims.iter().any(|claim| {
            timeline.tax_source_for_claim(claim).is_some() && claim.paid != claim.principal
        });
        (dividend, loan_cleared, unpaid_tax)
    };
    if !loan_cleared {
        return Err("loan_not_cleared_before_dividend".into());
    }
    if unpaid_tax || dividend_cents != 90 {
        return Err(format!(
            "tax_or_dividend_mismatch:unpaid_tax={unpaid_tax}:dividend={dividend_cents}"
        ));
    }

    let before_cold_hash = store.state_hash().map_err(|e| format!("{e:?}"))?;
    store.checkpoint().map_err(|e| format!("{e:?}"))?;
    drop(store);
    let mut recovered = Store::open(&path, RULES).map_err(|e| format!("{e:?}"))?;
    if recovered.state_hash().map_err(|e| format!("{e:?}"))? != before_cold_hash {
        return Err("cold_open_hash_mismatch".into());
    }

    // Exact same operation identities and payloads must replay the stored receipts without
    // a second share issue, road profile publication, sale, wage, debt or dividend.
    for (caller, envelope) in [
        (Caller::Account(TREASURY), contribution_a),
        (Caller::Account(HOUSEHOLD), contribution_b),
        (Caller::Account(ROAD_EXECUTOR), close_apply),
        (Caller::Account(ROAD_EXECUTOR), restore_apply),
        (Caller::Account(HOUSEHOLD), dividend),
    ] {
        let record = recovered
            .execute(caller, envelope)
            .map_err(|e| format!("{e:?}"))?;
        if !record.has_persistent_receipt || record.model.is_err() {
            return Err("replay_receipt_mismatch".into());
        }
    }
    for (operation_id, expected) in [
        (day1_op, day1),
        (day2_op, day2),
        (day3_op, day3),
        (day4_op, day4),
    ] {
        let replay = recovered
            .settle_town_market(operation_id)
            .map_err(|e| format!("{e:?}"))?;
        if !replay.has_persistent_receipt || replay.model.map_err(|e| format!("{e:?}"))? != expected
        {
            return Err(format!("town_settlement_replay_mismatch:{operation_id}"));
        }
    }
    let (replay_sales, replay_tax_basis, _, _, _, _) =
        settled_sales_tax_basis_and_assessed(&recovered, &path, 4);
    let expected_latest = (day4.town.unwrap().actual_sales.value(), 0);
    if (replay_sales, replay_tax_basis) != expected_latest {
        return Err("latest_replay_state_mismatch".into());
    }

    let final_cash = [TREASURY, HOUSEHOLD, COMPANY, LENDER_AND_ROAD_AUTHORITY]
        .map(|account| account_cash(&recovered, account))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let final_cash_sum: i64 = final_cash.iter().sum();
    if final_cash_sum != initial_cash_sum {
        return Err(format!(
            "cash_conservation_failed:{initial_cash_sum}:{final_cash_sum}"
        ));
    }
    let final_store_owned = recovered
        .owned_capacity_bytes()
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "{}",
        json!({
            "scenario":"core_joint_equity_spatial", "run_completed":true,
            "world":path,
            "initial_cash_sum":initial_cash_sum, "final_cash_sum":final_cash_sum,
            "initial_store_owned_bytes":initial_store_owned,
            "final_store_owned_bytes":final_store_owned,
            "store_owned_delta_bytes":final_store_owned as i128 - initial_store_owned as i128,
            "world_logical_file_bytes":world_logical_file_bytes(&path)?,
            "elapsed_seconds":started.elapsed().as_secs_f64(),
            "resource_scope":"single_microfixture_not_M0_or_real16GB",
            "day1_sales":day1_sales, "day1_taxable_basis":day1_tax_basis, "day1_tax":day1_tax,
            "day2_sales":day2_sales, "day2_taxable_basis":day2_tax_basis, "day2_tax":day2_tax,
            "day3_sales":day3_sales, "day3_taxable_basis":day3_tax_basis, "day3_tax":day3_tax,
            "day4_sales":day4.town.unwrap().actual_sales.value(),
            "company_cash_pre_dividend":company_before_dividend,
            "ordinary_dividend_cents":dividend_cents,
            "final_cash_accounts_1_to_4":final_cash,
            "state_hash":before_cold_hash, "scope":"fixture_scale_joint_contract_only"
        })
    );
    Ok(())
}

fn world_logical_file_bytes(dir: &std::path::Path) -> Result<u64, String> {
    let mut total = 0u64;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let bytes = if meta.is_dir() {
            world_logical_file_bytes(&entry.path())?
        } else {
            meta.len()
        };
        total = total
            .checked_add(bytes)
            .ok_or("world_logical_size_overflow")?;
    }
    Ok(total)
}

fn account_cash(store: &Store, account: u32) -> Result<i64, String> {
    store
        .engine()
        .map_err(|e| format!("{e:?}"))?
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == account)
        .map(|row| row.cash.value())
        .ok_or_else(|| format!("missing_account:{account}"))
}

fn verify_existing(rules: &str) -> Result<(), String> {
    let path = env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("pass existing world after --verify")?;
    let store = Store::open(&path, rules).map_err(|e| format!("{e:?}"))?;
    let cash = [TREASURY, HOUSEHOLD, COMPANY, LENDER_AND_ROAD_AUTHORITY]
        .map(|account| account_cash(&store, account))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let state_hash = store.state_hash().map_err(|e| format!("{e:?}"))?;
    let engine = store.engine().map_err(|e| format!("{e:?}"))?;
    let market = engine.world().view().town.ok_or("missing market")?;
    println!(
        "{}",
        json!({"mode":"fresh_process_verify", "pid":std::process::id(),
        "world":path,"state_hash":state_hash,"sequence":store.committed_sequence(),
        "settled_day":market.state.settled_day,
        "ordinary_dividend_cents":market.state.equity_registers.as_slice()[0].cumulative_dividends_distributed.value(),
        "final_cash_accounts_1_to_4":cash})
    );
    Ok(())
}

fn main() {
    let result = if env::args_os().nth(1).is_some_and(|arg| arg == "--verify") {
        verify_existing(RULES)
    } else if env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--verify-equity-baseline")
    {
        verify_existing("town-equity-cli-v1")
    } else {
        run()
    };
    if let Err(error) = result {
        eprintln!("core_joint_equity_spatial_failed={error}");
        std::process::exit(1);
    }
}

// Independent arithmetic oracle for the fixed input (expected numbers are hypotheses until the
// single-Store run resolves actual inventory lot basis and travel receipt details):
//
// - Opening raw stock 2: 20 q, basis 200c; each of two production runs consumes 10 q and adds
//   10c utility cost. Daily payroll is 800c. Ten sold units gross 1,000c with expected sold basis
//   910c (raw 100 + utility 10 + capitalized labor 800), so taxable margin 90c and 50% tax 45c.
// - Day 1 open: target 10 q demand, expected actual sales 10, taxable basis 90, assessed 45.
// - Day 2 road edge 24 closed: outbound+return trip cannot complete; expected sales 0 and margin
//   assessment 0. A second 10 q product batch remains unsold with its 910c basis.
// - Day 3 edge restored: expected actual sales 10 from the accumulated product buffer, taxable
//   basis 90 and tax 45. Day 4 Timeline pays both tax claims due by then and the 500c zero-interest
//   lender claim. Retained earnings should be 180 - 90 = 90c; a 10,000c requested ordinary
//   dividend is capped at actual retained earnings. With 100 treasury shares and 6,201 personal
//   shares after contributions, largest-remainder payout is expected treasury 1c/person 89c.
// - Before production: household and treasury each contribute 10,000c and receive shares; company
//   receives 20,000c. Supplier/authority 4 lends 500c, and repayment restores its principal.
// - Cross-check total cash and stock quantity+basis across every settlement; assert tax source
//   `taxable_basis`/assessed and proof service_day/generation against the immutable raw proof.
// - If engine's actual margin includes a different capitalized-labor or transport-cost policy,
//   stop and record the observed source breakdown; do not tune demand, taxes, costs or hard-coded
//   expected sales to force this oracle.
//
// Known interfaces/integration risks:
// 1. F's spatial API is absent from v144. This file targets its WIP `create_spatial_with_transport_permissions`,
//    Store spatial catalog, profile staging and permission actions; root must rebase/resolve before
//    compiling it.
// 2. Company 3 is both equity issuer and road owner, account 4 is lender and permit authority. Those
//    are intentional same-account roles, but each action still uses distinct scoped permissions.
// 3. Existing D delivery's route_source 77 is a fallback contract field; prove settlement uses the
//    current immutable profile by checking each day's proof hash and source 101/102 served counts.
// 4. Spatial profile is a map-derived walk proof, not a full traffic or economic calibration.
// 5. Cold-open and replay here are only candidates until a source-frozen Cargo/CLI run.
