//! A real wall-clock sustained run; finite laboratory contracts, no GUI workload.
use mycity_core::{
    boot::{Command as B, Facility},
    exchange::Command as X,
    inbox::{Command as I, ExternalFact, ExternalReason},
    transaction::{Action, Caller, CapacityToken, Engine, Envelope, Receipt, Resource, TxError},
    units::Money,
};
use mycity_storage::{Error, Query, Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
const RULES: &str = env!("MYCITY_SOURCE_SHA256");
const POINTS: [&str; 11] = [
    "Prepared",
    "WalHeaderHalf",
    "WalPayloadHalf",
    "WalChecksumHalf",
    "WalWritten",
    "WalSynced",
    "RootPublished",
    "CurrentRenamed",
    "CurrentSynced",
    "Applied",
    "Responded",
];
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn inbox(command: I) -> Action {
    Action::Inbox { command }
}
fn exchange(command: X) -> Action {
    Action::Exchange { command }
}
fn fact(event: u32) -> ExternalFact {
    ExternalFact {
        source: 1,
        event,
        debtor: 2,
        creditor: 4,
        amount: Money::new(700000).unwrap(),
        due: 50,
        reason: ExternalReason::EmergencyRepair,
    }
}
fn envelope(s: &Store, action: Action) -> Result<Envelope, String> {
    Ok(Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().map_err(err)?.world().view().revision,
        actions: vec![action],
    })
}
fn step(s: &mut Store, action: Action) -> Result<Receipt, String> {
    let e = envelope(s, action)?;
    let result = s.execute(Caller::System, e).map_err(err)?;
    if !result.has_persistent_receipt {
        return Err("missing_persistent_receipt".into());
    }
    result.model.map_err(err)
}
fn reject(s: &mut Store, action: Action, expected: TxError) -> Result<(), String> {
    let e = envelope(s, action)?;
    let before = s.state_hash().map_err(err)?;
    let r = s.execute(Caller::System, e).map_err(err)?;
    if r.model != Err(expected)
        || !r.has_persistent_receipt
        || s.state_hash().map_err(err)? != before
    {
        return Err(format!("wrong rejection: {:?}", r.model));
    }
    Ok(())
}
fn token(r: Receipt) -> Result<CapacityToken, String> {
    r.created_tokens[0].ok_or("missing_capacity_token".into())
}
fn reopen(s: Store, path: &Path) -> Result<Store, String> {
    let hash = s.state_hash().map_err(err)?;
    drop(s);
    let mut s = Store::open(path, RULES).map_err(err)?;
    if s.state_hash().map_err(err)? != hash {
        return Err("restart_hash_changed".into());
    }
    s.reconcile_external_source(6).map_err(err)?;
    Ok(s)
}
fn audit(s: &Store) -> Result<Value, String> {
    let engine = s.engine().map_err(err)?;
    let bytes = engine.export_checkpoint()?;
    let restored = Engine::restore_checkpoint(&bytes)?;
    if restored.state_hash() != engine.state_hash() {
        return Err("invariant_roundtrip_changed".into());
    }
    let w = engine.world().view();
    let cash: i64 = w.accounts.iter().map(|a| a.cash.value()).sum();
    if cash != 3000000 {
        return Err("cash_not_conserved".into());
    }
    let e = w.exchange.ok_or("missing_exchange")?;
    let debt: i64 = e.inbox.all().map(|f| f.fee.value() - f.paid.value()).sum();
    if debt < 0 || debt + e.inbox.paid.value() != e.inbox.accrued.value() {
        return Err("liability_not_conserved".into());
    }
    Ok(
        json!({"cash_cents":cash,"outstanding_cents":debt,"owned_capacity_bytes":s.owned_capacity_bytes().map_err(err)?,"checkpoint":s.checkpoint_status().map_err(err)?,"source":s.source_status().map_err(err)?,"interaction_time":w.region.unwrap().interaction_time,"state_sha256":s.state_hash().map_err(err)?}),
    )
}
pub fn worker(args: &[String]) -> Result<(), String> {
    if args.len() != 5 {
        return Err("continuity worker args".into());
    }
    let dir = Path::new(&args[2]);
    let env: Envelope = serde_json::from_slice(&fs::read(&args[3]).map_err(err)?).map_err(err)?;
    let point = &args[4];
    let mut s = Store::open(dir, RULES).map_err(err)?;
    s.execute_with_observer(Caller::System, env, |p: WritePoint| {
        if format!("{p:?}") == *point {
            std::process::exit(97);
        }
        Ok(())
    })
    .map_err(err)?;
    if point == "Responded" {
        std::process::exit(97);
    }
    Err("crash_point_not_reached".into())
}
fn crash(s: Store, path: &Path, parent: &Path, point: &str) -> Result<Store, String> {
    let env = envelope(&s, Action::RegionActivate { city: 3 })?;
    let old_hash = s.engine().map_err(err)?.state_hash();
    let old_seq = s.committed_sequence();
    let mut candidate = Engine::restore_checkpoint(&s.engine().map_err(err)?.export_checkpoint()?)?;
    let expected = candidate
        .execute(Caller::System, env.clone())
        .map_err(err)?;
    let new_hash = candidate.state_hash();
    let command = parent.join("crash-envelope.json");
    fs::write(&command, serde_json::to_vec(&env).map_err(err)?).map_err(err)?;
    drop(s);
    let out = Command::new(std::env::current_exe().map_err(err)?)
        .args([
            "continuity-worker",
            path.to_str().ok_or("path")?,
            command.to_str().ok_or("path")?,
            point,
        ])
        .output()
        .map_err(err)?;
    if out.status.code() != Some(97) {
        return Err(format!(
            "child:{point}:{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let mut s = Store::open(path, RULES).map_err(err)?;
    let recovered = s.engine().map_err(err)?.state_hash();
    if recovered != old_hash && recovered != new_hash {
        return Err("partial_crash_root".into());
    }
    if s.execute(Caller::System, env).map_err(err)?.model != Ok(expected)
        || s.engine().map_err(err)?.state_hash() != new_hash
        || s.committed_sequence() != old_seq + 1
    {
        return Err("crash_retry_not_exact".into());
    }
    s.reconcile_external_source(6).map_err(err)?;
    Ok(s)
}
pub fn run(path: &Path, seconds: u64) -> Result<Value, String> {
    if seconds > 3600 {
        return Err("duration_above_one_hour".into());
    }
    fs::create_dir(path).map_err(err)?;
    let started = Instant::now();
    let world = path.join("world");
    let resources = [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
    ]
    .into_iter()
    .map(|(id, n)| Resource::new(id, n).unwrap())
    .collect();
    let mut s = Store::create_spatial(
        &world,
        include_bytes!("../../../fixtures/m0-exchange-opening.json"),
        resources,
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .map_err(err)?;
    step(&mut s, inbox(I::Enable))?;
    step(
        &mut s,
        Action::Boot {
            command: B::Start {
                facility: Facility::Pump,
                reverse_request: true,
            },
        },
    )?;
    step(
        &mut s,
        Action::Boot {
            command: B::Fault {
                facility: Facility::Pump,
            },
        },
    )?;
    step(
        &mut s,
        Action::Boot {
            command: B::ClearFailure {
                facility: Facility::Pump,
            },
        },
    )?;
    let hold = token(step(
        &mut s,
        Action::ReserveCapacity {
            resource: 201,
            units: 1920,
        },
    )?)?;
    for quote in 1..=4 {
        step(&mut s, exchange(X::Dispatch { quote }))?;
    }
    reject(
        &mut s,
        exchange(X::Dispatch { quote: 5 }),
        TxError::InboxFull,
    )?;
    step(
        &mut s,
        exchange(X::SetLink {
            edge: 1,
            open: false,
        }),
    )?;
    for quote in 1..=4 {
        step(&mut s, inbox(I::Incident { quote }))?;
    }
    s.register_external_source().map_err(err)?;
    for event in 1..=5 {
        s.queue_external(fact(event)).map_err(err)?;
    }
    for event in 1..=4 {
        s.receive_external(fact(event)).map_err(err)?;
    }
    if !matches!(
        s.receive_external(fact(5)),
        Err(Error::FactBlocked("world_fact_inbox_full"))
    ) {
        return Err("expected_external_backpressure".into());
    }
    let blocked = audit(&s)?;
    let advance = envelope(&s, Action::RegionAdvance { seconds: 1 })?;
    if !matches!(
        s.execute(Caller::System, advance),
        Err(Error::SourceSyncRequired)
    ) {
        return Err("missing_source_barrier".into());
    }
    // Crash/reopen while an actual external event remains unacknowledged.
    let hash = s.state_hash().map_err(err)?;
    drop(s);
    s = Store::open(&world, RULES).map_err(err)?;
    if s.state_hash().map_err(err)? != hash
        || s.source_status().map_err(err)?.pending != vec![fact(5)]
    {
        return Err("pending_lost".into());
    }
    step(
        &mut s,
        inbox(I::Handoff {
            sequence: 9,
            manager: 1,
        }),
    )?;
    s.receive_external(fact(5)).map_err(err)?;
    s.reconcile_external_source(6).map_err(err)?;
    step(&mut s, Action::RegionActivate { city: 2 })?;
    step(
        &mut s,
        exchange(X::SetLink {
            edge: 1,
            open: true,
        }),
    )?;
    step(&mut s, Action::RegionAdvance { seconds: 120 })?;
    for sequence in 1..=8 {
        step(&mut s, inbox(I::Settle { sequence }))?;
        step(&mut s, inbox(I::Archive { sequence }))?;
    }
    for sequence in 10..=13 {
        step(
            &mut s,
            inbox(I::Handoff {
                sequence,
                manager: 1,
            }),
        )?;
    }
    step(&mut s, inbox(I::Settle { sequence: 9 }))?;
    step(&mut s, Action::ReleaseCapacity { token: hold })?;
    s.checkpoint().map_err(err)?;
    s = reopen(s, &world)?;
    let initial = audit(&s)?;
    let workload_started = Instant::now();
    let mut heartbeat = fs::File::create(path.join("heartbeat.jsonl")).map_err(err)?;
    let mut next_report = Duration::ZERO;
    let mut cycles = 0u64;
    let mut crashes = 0u64;
    let mut restarts = 2u64;
    let mut max_owned = 0u64;
    let mut max_audit_us = 0u128;
    let mut max_cycle_us = 0u128;
    while cycles == 0 || workload_started.elapsed() < Duration::from_secs(seconds) {
        let cycle_start = Instant::now();
        step(
            &mut s,
            Action::RegionActivate {
                city: [1, 2, 3][cycles as usize % 3],
            },
        )?;
        let t = token(step(
            &mut s,
            Action::ReserveCapacity {
                resource: 201,
                units: 2000,
            },
        )?)?;
        reject(
            &mut s,
            Action::ReserveCapacity {
                resource: 201,
                units: 1,
            },
            TxError::Capacity,
        )?;
        step(&mut s, Action::OccupyCapacity { token: t })?;
        step(&mut s, Action::ReleaseCapacity { token: t })?;
        step(
            &mut s,
            exchange(X::SetLink {
                edge: 1,
                open: false,
            }),
        )?;
        step(
            &mut s,
            exchange(X::SetLink {
                edge: 1,
                open: true,
            }),
        )?;
        step(&mut s, Action::RegionAdvance { seconds: 30 })?;
        if cycles % 25 == 24 {
            s = reopen(s, &world)?;
            restarts += 1;
        }
        if cycles % 50 == 49 {
            s = crash(s, &world, path, POINTS[crashes as usize % POINTS.len()])?;
            crashes += 1;
            restarts += 1;
        }
        let check = Instant::now();
        let sample = audit(&s)?;
        max_audit_us = max_audit_us.max(check.elapsed().as_micros());
        max_owned = max_owned.max(sample["owned_capacity_bytes"].as_u64().ok_or("owned")?);
        for q in [
            Query::Contracts,
            Query::Fulfilment,
            Query::Arrears,
            Query::Capacity,
            Query::Blocked,
            Query::Recovery,
            Query::Receipts,
        ] {
            let page = s.query(q, None, 3).map_err(err)?;
            if let Some(cursor) = page.next_cursor {
                s.query(q, Some(&cursor), 3).map_err(err)?;
            }
        }
        cycles += 1;
        max_cycle_us = max_cycle_us.max(cycle_start.elapsed().as_micros());
        if started.elapsed() >= next_report {
            let sample = json!({"wall_elapsed_seconds":started.elapsed().as_secs_f64(),"cycles":cycles,"actual_process_exits":crashes,"restarts":restarts,"sample":sample});
            writeln!(heartbeat, "{sample}").map_err(err)?;
            heartbeat.flush().map_err(err)?;
            eprintln!(
                "continuity: wall={:.1}s cycles={cycles} sequence={} crashes={crashes} disk={}B",
                started.elapsed().as_secs_f64(),
                s.committed_sequence(),
                s.capacity_status().map_err(err)?.used_bytes
            );
            next_report = started.elapsed() + Duration::from_secs(30);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    s.checkpoint().map_err(err)?;
    s = reopen(s, &world)?;
    restarts += 1;
    let final_sample = audit(&s)?;
    let elapsed = started.elapsed().as_secs_f64();
    let workload_elapsed = workload_started.elapsed().as_secs_f64();
    Ok(
        json!({"schema":"mycity-m0-continuity-v1","source_sha256":RULES,"lock_sha256":env!("MYCITY_LOCK_SHA256"),"rustc":env!("MYCITY_RUSTC"),"target":env!("MYCITY_TARGET"),"profile":env!("MYCITY_PROFILE"),"requested_wall_seconds":seconds,"actual_wall_seconds":elapsed,"workload_wall_seconds":workload_elapsed,"cycles":cycles,"committed_sequence":s.committed_sequence(),"actual_process_exits":crashes,"restart_count":restarts,"crash_points":POINTS,"all_crash_points_exercised":crashes>=11,"max_owned_capacity_bytes":max_owned,"max_cycle_us":max_cycle_us,"max_invariant_check_us":max_audit_us,"initial_full_and_blocked":blocked,"after_physical_delivery_and_handoff":initial,"final":final_sample,"wall_duration_gate":(1800..=3600).contains(&seconds)&&workload_elapsed>=1800.0,"sixteen_gb_acceptance":false,"scope":"same finite three-city profile; initial four actual deliveries and external debts, recurring shared-capacity exhaustion, map clocks, link faults, queries and durable recovery; no new contracts/steady economic balance/UI/GPU"}),
    )
}
