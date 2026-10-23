//! Same-Store D/F market/road and E housing/rent integration candidate.
//!
//! Housing's access declaration remains a static opening fact; the road change below affects
//! transport-backed market settlement only. No residential route/access link is claimed.

use mycity_core::{
    calendar::Date,
    housing::{
        genesis_lease_offer_sha256, lease_terms_sha256, DwellingFact, DwellingOriginKind,
        GenesisLeaseOffer, LeaseTerms, OpeningDwelling, OpeningHousehold, OpeningHousing,
        MAX_GENESIS_LEASE_OFFERS, MAX_M0_HOUSING_UNITS,
    },
    market_access::{
        AllocationScope, DemandRepeatPolicy, FeePolicyInput, TransportAllocationProfile,
        TripPartition,
    },
    model_rows::Rows,
    spatial_transport_permissions::{
        request_sha256, SpatialAccessEdge, SpatialAccessOperation, SpatialAccessSurface,
        SpatialApprovalRole, SpatialAssetKind, SpatialAssetState, SpatialPermissionCommand,
        SpatialPermissionState, SpatialUpdateApproval, SpatialUpdateRequest, SPATIAL_ALLOW_CLOSE,
        SPATIAL_ALLOW_RESTORE,
    },
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope, TxError},
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
use std::{fmt::Write as _, fs, path::PathBuf, sync::atomic::AtomicU64};

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
const HOUSING_UNIT: u32 = 101;
const HOUSEHOLD_ID: u32 = 1;
const OFFER_ID: u64 = 9001;
const LEASE_ID: u32 = 9001;
const MONTHLY_RENT_CENTS: i64 = 480_000;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

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
    let mut opening: Value = serde_json::from_slice(&opening_bytes(profile_hash)).unwrap();
    opening["housing"] = serde_json::to_value(opening_housing()).unwrap();
    Store::create_spatial_with_transport_permissions(
        path,
        &serde_json::to_vec(&opening).unwrap(),
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

fn settle_market(store: &mut Store) -> (u64, mycity_core::transaction::Receipt) {
    let operation_id = store.committed_sequence() + 1;
    let outcome = store.settle_town_market(operation_id).unwrap();
    assert!(outcome.has_persistent_receipt);
    let receipt = outcome
        .model
        .unwrap_or_else(|error| panic!("town settlement rejected: {error:?}"));
    (operation_id, receipt)
}

fn advance_to(store: &mut Store, target: u64) {
    for _ in 0..10_000 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: TimelineCommand::Advance { target, limit: 256 },
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
    let proof = transport_proof_for_day(store, world, expected_day);
    let proof_hash = market.state.last_transport.unwrap().issued_proof_sha256;
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

fn transport_proof_for_day(
    store: &Store,
    world: &std::path::Path,
    expected_day: u64,
) -> mycity_core::market_access::TransportProofAsset {
    let transport = store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .last_transport
        .unwrap();
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
    proof
}

fn proof_derived_freight_per_q(
    proof: &mycity_core::market_access::TransportProofAsset,
    quote_id: u64,
) -> i64 {
    assert_eq!(
        proof.partitions.len(),
        1,
        "fixture uses one transport quote"
    );
    let partition = proof
        .partitions
        .iter()
        .find(|partition| partition.quote_id == quote_id)
        .unwrap();
    let outcome = |source_id| {
        proof
            .assignment
            .demand_outcomes
            .iter()
            .find(|entry| entry.source_id == source_id)
            .unwrap()
    };
    let outbound = outcome(partition.outbound_source);
    let inbound = outcome(partition.return_source);
    let allocated_trips = partition
        .trip_count
        .min(outbound.served_people)
        .min(inbound.served_people);
    if allocated_trips == 0 {
        return 0;
    }
    let round_trip_seconds = outbound
        .mean_travel_seconds
        .unwrap()
        .checked_add(inbound.mean_travel_seconds.unwrap())
        .unwrap();
    let total_cents = u128::from(round_trip_seconds)
        .checked_mul(u128::from(proof.fee_policy.cents_per_travel_second))
        .and_then(|cents| cents.checked_mul(u128::from(allocated_trips)))
        .unwrap();
    let goods_per_trip =
        u128::try_from(partition.goods_per_trip.value()).expect("fixture quantity is nonnegative");
    let accessible_quantity = goods_per_trip
        .checked_mul(u128::from(allocated_trips))
        .unwrap();
    let per_q = total_cents.checked_add(accessible_quantity - 1).unwrap() / accessible_quantity;
    i64::try_from(per_q).unwrap()
}

fn assert_market_cash_flow(
    store: &Store,
    before: [i64; 4],
    receipt: mycity_core::transaction::Receipt,
    proof: &mycity_core::market_access::TransportProofAsset,
) -> i64 {
    let summary = receipt.town.expect("market settlement receipt");
    let profile = store.engine().unwrap().world().view().town.unwrap().profile;
    let quote = profile
        .delivery
        .as_slice()
        .iter()
        .find(|quote| quote.quote_id == 1)
        .unwrap();
    let freight_per_q = proof_derived_freight_per_q(proof, quote.quote_id);
    assert_eq!(quote.freight_per_q.value(), 1, "opening fallback quote");
    assert_eq!(proof.fee_policy.provider_account, LENDER_AND_ROAD_AUTHORITY);
    assert_eq!(quote.utility_account, TREASURY);
    assert_eq!(profile.recipes.as_slice()[0].utility_account, TREASURY);
    assert_eq!(profile.shops.as_slice()[0].account, COMPANY);
    // The Store-resolved proof tariff replaces the opening fallback freight quote. Here the
    // immutable proof carries a zero cents-per-travel-second tariff, so actual freight is zero;
    // the household still pays the one-cent utility surcharge per sold unit.
    let sales = summary.actual_sales.value();
    let utility_per_q = quote.utility_surcharge_per_q.value();
    let freight_receipt = sales.checked_mul(freight_per_q).unwrap();
    let utility_receipt = sales.checked_mul(utility_per_q).unwrap();
    let customer_delivery = freight_receipt + utility_receipt;
    let customer_spend = summary.goods_payments.value() + customer_delivery;
    let production_utility = summary
        .produced
        .value()
        .checked_mul(
            profile.recipes.as_slice()[0]
                .utility_cost_per_output_q
                .value(),
        )
        .unwrap();
    let expected_company_delta = summary.goods_payments.value() - production_utility;
    let expected_treasury_delta = production_utility + utility_receipt;
    let after = [TREASURY, HOUSEHOLD, COMPANY, LENDER_AND_ROAD_AUTHORITY]
        .map(|account| account_cash(store, account));
    assert_eq!(before[1] - after[1], customer_spend, "buyer cash outflow");
    assert_eq!(after[2] - before[2], expected_company_delta, "seller net");
    assert_eq!(
        after[0] - before[0],
        expected_treasury_delta,
        "utility/fee receipts"
    );
    assert_eq!(
        after[3] - before[3],
        freight_receipt,
        "transport provider receives only proof-derived freight"
    );
    assert_eq!(after.iter().sum::<i64>(), before.iter().sum::<i64>());
    customer_spend
}

fn opening_housing() -> OpeningHousing {
    let accepted_at = GameTime::new(i64::try_from(14 * DAY).unwrap()).unwrap();
    let fact = DwellingFact {
        unit_id: HOUSING_UNIT,
        building_id: 10,
        owner_account: COMPANY,
        building_unit_capacity: 1,
        person_capacity: 4,
        delivered: true,
        legal_to_occupy: true,
        has_legal_access: true,
        source_version: 1,
    };
    let terms = LeaseTerms {
        lease_id: LEASE_ID,
        clause_version: 1,
        contract_source_id: 9101,
        landlord_authorization_id: 9102,
        tenant_consent_id: 9103,
        rent_per_cycle: m(MONTHLY_RENT_CENTS),
        cycle: Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: TREASURY,
        starts_at: accepted_at,
    };
    let offer = GenesisLeaseOffer {
        offer_id: OFFER_ID,
        world_id: 1,
        branch_id: 1,
        unit_id: HOUSING_UNIT,
        building_id: fact.building_id,
        owner_account: COMPANY,
        tenant_account: HOUSEHOLD,
        household_id: HOUSEHOLD_ID,
        valid_from: GameTime::ZERO,
        valid_until: Date::new(2001, 2, 1).unwrap().midnight(),
        asset_source_id: 7001,
        authority_source_id: 7002,
        contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
        lease_id: terms.lease_id,
        clause_version: terms.clause_version,
        rent_per_cycle: terms.rent_per_cycle,
        cycle: terms.cycle,
        deposit: terms.deposit,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: lease_terms_sha256(terms).unwrap(),
    };
    let mut dwellings = Rows::<OpeningDwelling, MAX_M0_HOUSING_UNITS>::new().unwrap();
    dwellings
        .try_push(OpeningDwelling {
            fact,
            origin: DwellingOriginKind::ImmutableGenesisEndowment,
            asset_source_id: 7001,
        })
        .unwrap();
    let mut households = Rows::<OpeningHousehold, MAX_M0_HOUSING_UNITS>::new().unwrap();
    households
        .try_push(OpeningHousehold {
            household_id: HOUSEHOLD_ID,
            account: HOUSEHOLD,
            people: 3,
            source_id: 8001,
        })
        .unwrap();
    let mut offers = Rows::<GenesisLeaseOffer, MAX_GENESIS_LEASE_OFFERS>::new().unwrap();
    offers.try_push(offer).unwrap();
    OpeningHousing {
        unit_limit: 1,
        dwellings,
        households,
        lease_offers: offers,
    }
}

fn offer_hash() -> [u8; 32] {
    genesis_lease_offer_sha256(opening_housing().lease_offers[0])
}

fn account_cash(store: &Store, account: u32) -> i64 {
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

fn total_cash(store: &Store) -> i64 {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .map(|row| row.cash.value())
        .sum()
}

fn housing_snapshot(store: &Store) -> Value {
    serde_json::from_slice::<Value>(&store.engine().unwrap().export_checkpoint().unwrap()).unwrap()
        ["world"]["housing"]
        .clone()
}

fn print_resource_receipt(
    phase: &str,
    store: &Store,
    sales: Option<[i64; 3]>,
    rent: Option<(i64, i64, i64)>,
    total_cash: Option<i64>,
) {
    let housing = housing_snapshot(store);
    let configured_units = housing["opening"]["unit_limit"].as_u64().unwrap();
    let occupied_unit_rows = housing["state"]["units"].as_array().unwrap().len();
    println!(
        "HOUSING_EQUITY_SPATIAL_RESOURCE={}",
        json!({
            "phase": phase,
            "store_owned_capacity_bytes": store.owned_capacity_bytes().unwrap(),
            "engine_owned_capacity_bytes": store.engine().unwrap().owned_capacity_bytes(),
            "housing_configured_units": configured_units,
            "housing_state_unit_rows": occupied_unit_rows,
            "first_three_market_sales": sales,
            "rent_claim": rent.map(|(principal, paid, arrears)| json!({
                "principal_cents": principal,
                "paid_cents": paid,
                "arrears_cents": arrears,
            })),
            "total_cash_cents": total_cash,
            "native_peak_bytes": null,
            "native_peak_status": "not_measured",
            "scope": "owned model/store capacities for this one-unit microfixture; not process peak or M0 performance",
        })
    );
}

fn lease_claim(store: &Store) -> mycity_core::timeline::Claim {
    *store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claims
        .iter()
        .find(|claim| claim.id == u64::from(LEASE_ID))
        .expect("one rent Claim for the active lease")
}

fn reject(store: &mut Store, caller: Caller, action: Action) -> Envelope {
    let request = envelope(store, action);
    let outcome = store.execute(caller, request.clone()).unwrap();
    assert!(outcome.has_persistent_receipt);
    assert!(matches!(outcome.model, Err(TxError::InvalidOperation)));
    request
}

fn replay_exact(store: &mut Store, caller: Caller, request: Envelope) {
    let replay = store.execute(caller, request).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
}

fn replay_market(
    store: &mut Store,
    operation_id: u64,
    expected: mycity_core::transaction::Receipt,
) {
    let replay = store.settle_town_market(operation_id).unwrap();
    assert!(replay.has_persistent_receipt);
    assert_eq!(replay.model.unwrap(), expected);
}

fn replay_rejected(store: &mut Store, caller: Caller, request: Envelope) {
    let replay = store.execute(caller, request).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(matches!(replay.model, Err(TxError::InvalidOperation)));
}

#[test]
fn housing_rent_market_road_and_equity_share_one_world_and_rent_blocks_distribution_and_exit() {
    let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&root).unwrap();
    let artifact = root.join(format!(
        "mycity-housing-equity-spatial-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir(&artifact).unwrap();
    let artifact = fs::canonicalize(artifact).unwrap();
    let world_path = artifact.join("world");
    eprintln!("HOUSING_EQUITY_SPATIAL_ARTIFACT={}", artifact.display());
    let mut store = create_joint_store(&world_path);
    print_resource_receipt("created", &store, None, None, None);
    let initial_cash = total_cash(&store);
    assert_eq!(initial_cash, 3_000_000);
    let household_opening_cash = account_cash(&store, HOUSEHOLD);
    assert_eq!(household_opening_cash, 200_000);

    let contribution_treasury = execute(
        &mut store,
        Caller::Account(TREASURY),
        Action::EquityContribute {
            company: COMPANY,
            amount: m(10_000),
        },
    );
    let contribution_household = execute(
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

    let mut market_receipts = Vec::new();
    advance_to(&mut store, DAY);
    let day1_cash_before_settlement = [TREASURY, HOUSEHOLD, COMPANY, LENDER_AND_ROAD_AUTHORITY]
        .map(|account| account_cash(&store, account));
    let day1_receipt = settle_market(&mut store);
    market_receipts.push(day1_receipt);
    let day1 = settled_sales_tax_basis_and_assessed(&store, &world_path, 1);
    assert_eq!((day1.0, day1.1, day1.2, day1.3, day1.4), (10, 90, 45, 4, 4));
    let day1_proof = transport_proof_for_day(&store, &world_path, 1);
    assert_eq!(day1_proof.fee_policy.cents_per_travel_second, 0);
    let day1_customer_spend = assert_market_cash_flow(
        &store,
        day1_cash_before_settlement,
        day1_receipt.1,
        &day1_proof,
    );
    assert_eq!(day1_customer_spend, 1_010);

    let (close_request, close_digest) =
        request_road_change(&mut store, &world_path, SpatialAccessOperation::Close, 2);
    advance_to(&mut store, 2 * DAY);
    let close_apply = apply_road_change(&mut store, close_request.request_id, close_digest);
    let close_receipt = (Caller::Account(ROAD_EXECUTOR), close_apply);
    market_receipts.push(settle_market(&mut store));
    let day2 = settled_sales_tax_basis_and_assessed(&store, &world_path, 2);
    assert_eq!((day2.0, day2.1, day2.2, day2.3), (0, 0, 0, 0));
    assert_ne!(day1.5, day2.5);

    let (restore_request, restore_digest) =
        request_road_change(&mut store, &world_path, SpatialAccessOperation::Restore, 3);
    advance_to(&mut store, 3 * DAY);
    let restore_apply = apply_road_change(&mut store, restore_request.request_id, restore_digest);
    let restore_receipt = (Caller::Account(ROAD_EXECUTOR), restore_apply);
    let day3_cash_before_settlement = [TREASURY, HOUSEHOLD, COMPANY, LENDER_AND_ROAD_AUTHORITY]
        .map(|account| account_cash(&store, account));
    let day3_receipt = settle_market(&mut store);
    market_receipts.push(day3_receipt);
    let day3 = settled_sales_tax_basis_and_assessed(&store, &world_path, 3);
    assert_eq!((day3.0, day3.1, day3.2, day3.3, day3.4), (10, 90, 45, 4, 4));
    assert_ne!(day2.5, day3.5);
    let day3_proof = transport_proof_for_day(&store, &world_path, 3);
    assert_eq!(day3_proof.fee_policy.cents_per_travel_second, 0);
    let day3_customer_spend = assert_market_cash_flow(
        &store,
        day3_cash_before_settlement,
        day3_receipt.1,
        &day3_proof,
    );
    assert_eq!(day3_customer_spend, 1_010);

    advance_to(&mut store, 4 * DAY);
    market_receipts.push(settle_market(&mut store));
    let timeline = store.engine().unwrap().timeline().unwrap();
    let loan = timeline
        .claims
        .iter()
        .find(|claim| claim.id == 100)
        .unwrap();
    assert_eq!(loan.paid, loan.principal);
    assert!(loan.processed);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .unwrap()
            .state
            .tax_cursors
            .as_slice()[0]
            .retained_earnings_cents,
        90
    );

    // Keep the one market and housing household identity aligned (group 1/account 2).
    // Housing is accepted after day-14's completed, settled boundary; no runtime caller can
    // change the offer's landlord/tenant, unit, terms, or legal-access fact.
    for day in 5..=14 {
        advance_to(&mut store, day * DAY);
        market_receipts.push(settle_market(&mut store));
    }
    let payroll = store.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!(payroll.spec.payee, HOUSEHOLD);
    assert_eq!(payroll.paid, m(1_600));
    let lease = execute(
        &mut store,
        Caller::Account(HOUSEHOLD),
        Action::HousingLease {
            offer_id: OFFER_ID,
            offer_sha256: offer_hash(),
        },
    );
    let lease_snapshot = housing_snapshot(&store);
    assert_eq!(
        lease_snapshot["state"]["units"][0]["household_id"],
        HOUSEHOLD_ID
    );
    assert_eq!(lease_snapshot["state"]["units"][0]["lease_id"], LEASE_ID);
    assert!(
        lease_snapshot["opening"]["dwellings"][0]["fact"]["has_legal_access"]
            .as_bool()
            .unwrap()
    );
    let initial_rent_claim = lease_claim(&store);
    assert_eq!(initial_rent_claim.principal, m(263_226));
    assert_eq!(initial_rent_claim.paid, Money::ZERO);
    assert_eq!(initial_rent_claim.due, 31 * DAY);

    // Empty inventory days still settle through the same market writer so the day cursor stays
    // contiguous. Rent accrues and is collected by Timeline on the real day-31 P2 boundary.
    for day in 15..=30 {
        advance_to(&mut store, day * DAY);
        market_receipts.push(settle_market(&mut store));
    }
    let tenant_before_rent = account_cash(&store, HOUSEHOLD);
    let landlord_before_rent = account_cash(&store, COMPANY);
    let total_market_spend = day1_customer_spend + day3_customer_spend;
    let actual_market_sales: i64 = market_receipts
        .iter()
        .map(|(_, receipt)| receipt.town.unwrap().actual_sales.value())
        .sum();
    let actual_goods_payments: i64 = market_receipts
        .iter()
        .map(|(_, receipt)| receipt.town.unwrap().goods_payments.value())
        .sum();
    assert_eq!(actual_market_sales, 20);
    assert_eq!(actual_goods_payments, 2_000);
    let household_contribution = 10_000;
    let wages_received = payroll.paid.value();
    assert_eq!(wages_received, 1_600);
    assert_eq!(
        tenant_before_rent,
        household_opening_cash - household_contribution + wages_received - total_market_spend
    );
    assert_eq!(tenant_before_rent, 189_580);
    advance_to(&mut store, 31 * DAY);
    market_receipts.push(settle_market(&mut store));
    let rent = lease_claim(&store);
    assert_eq!(rent.principal, m(263_226));
    assert_eq!(rent.paid, m(189_580));
    assert_eq!(rent.principal.checked_sub(rent.paid).unwrap(), m(73_646));
    assert!(rent.processed);
    assert_eq!(account_cash(&store, HOUSEHOLD), 0);
    assert_eq!(
        account_cash(&store, COMPANY) - landlord_before_rent,
        189_580
    );
    assert_eq!(total_cash(&store), initial_cash);
    let rent_source = store
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .rent_source_for_claim(&rent)
        .unwrap();
    assert_eq!(rent_source.tenant, HOUSEHOLD);
    assert_eq!(rent_source.landlord, COMPANY);
    assert_eq!(rent_source.unit_id, HOUSING_UNIT);

    // The same account earns market profit and rental cash. Existing D distribution safety
    // conservatively blocks while the Rent Claim is still outstanding; rent cash is not
    // mislabelled as town retained earnings. Likewise business exit cannot discard this claim.
    let company_cash_before_rejections = account_cash(&store, COMPANY);
    let register_before_rejections = store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .equity_registers
        .as_slice()[0];
    let blocked_dividend = reject(
        &mut store,
        Caller::Account(HOUSEHOLD),
        Action::EquityDistributeRetainedEarnings {
            company: COMPANY,
            requested: m(90),
        },
    );
    let blocked_exit = reject(
        &mut store,
        Caller::Account(HOUSEHOLD),
        Action::BeginExit {
            company: COMPANY,
            policy_id: 1,
        },
    );
    assert_eq!(
        account_cash(&store, COMPANY),
        company_cash_before_rejections
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .unwrap()
            .state
            .equity_registers
            .as_slice()[0],
        register_before_rejections
    );

    let final_housing_snapshot = housing_snapshot(&store);
    let final_hash = store.state_hash().unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut recovered = Store::open(&world_path, RULES).unwrap();
    assert_eq!(recovered.state_hash().unwrap(), final_hash);
    assert_eq!(lease_claim(&recovered), rent);
    assert_eq!(housing_snapshot(&recovered), final_housing_snapshot);
    assert_eq!(account_cash(&recovered, HOUSEHOLD), 0);
    assert_eq!(
        account_cash(&recovered, COMPANY),
        company_cash_before_rejections
    );
    replay_exact(
        &mut recovered,
        Caller::Account(TREASURY),
        contribution_treasury,
    );
    replay_exact(
        &mut recovered,
        Caller::Account(HOUSEHOLD),
        contribution_household,
    );
    replay_exact(
        &mut recovered,
        Caller::Account(ROAD_EXECUTOR),
        close_receipt.1,
    );
    replay_exact(
        &mut recovered,
        Caller::Account(ROAD_EXECUTOR),
        restore_receipt.1,
    );
    replay_exact(&mut recovered, Caller::Account(HOUSEHOLD), lease);
    for (operation_id, receipt) in market_receipts {
        replay_market(&mut recovered, operation_id, receipt);
    }
    replay_rejected(&mut recovered, Caller::Account(HOUSEHOLD), blocked_dividend);
    replay_rejected(&mut recovered, Caller::Account(HOUSEHOLD), blocked_exit);
    assert_eq!(recovered.state_hash().unwrap(), final_hash);
    assert_eq!(lease_claim(&recovered), rent);
    assert_eq!(total_cash(&recovered), initial_cash);
    let house = housing_snapshot(&recovered);
    assert_eq!(house["state"]["units"][0]["lease_id"], LEASE_ID);
    assert_eq!(house["state"]["units"][0]["arrears_ref"], LEASE_ID);
    assert!(house["opening"]["dwellings"][0]["fact"]["has_legal_access"]
        .as_bool()
        .unwrap());

    print_resource_receipt(
        "final_recovered",
        &recovered,
        Some([day1.0, day2.0, day3.0]),
        Some((
            rent.principal.value(),
            rent.paid.value(),
            rent.principal.value() - rent.paid.value(),
        )),
        Some(total_cash(&recovered)),
    );

    if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_none() {
        let _ = fs::remove_dir_all(&artifact);
    } else {
        eprintln!("HOUSING_EQUITY_SPATIAL_PRESERVED={}", artifact.display());
    }
}
