//! Run one real Store-backed transport-limited market day.
use mycity_core::{
    market_access::{AllocationScope, DemandRepeatPolicy, FeePolicyInput, TripPartition},
    timeline::{Command as TimelineCommand, DAY},
    transaction::{Action, Caller, Envelope},
    transport::{Entrance, Node, OdDemand, Opening, RoadEdge},
    units::Quantity,
};
use mycity_storage::{CapacityPolicy, Error as StorageError, Store};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

const RULES: &str = "m0-town-transport-fixed-profile-v1";
const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");

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
    opening: &'a Opening,
    assignment: &'a mycity_core::transport::Assignment,
    partitions: &'a [TripPartition],
    fee_policy: FeePolicyInput,
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

fn main() {
    if let Err(error) = run() {
        eprintln!("town_transport: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let scenario = args
        .next()
        .ok_or("usage: town_transport <normal|closed|bottleneck|expanded> <new-store-dir>")?;
    let dir = args.next().ok_or("missing_store_dir")?;
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
