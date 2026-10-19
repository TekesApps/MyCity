use mycity_core::{
    exchange::Command as X,
    inbox::Command as I,
    transaction::{Action, Caller, Envelope, Resource, TxError},
};
use mycity_storage::{CapacityPolicy, Error, Store};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static ID: AtomicU64 = AtomicU64::new(0);
const RULES: &str = "m0-storage-process-fixture-v1";
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-inbox-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn create(path: &Path) -> Store {
    Store::create_spatial(
        path,
        include_bytes!("../../../fixtures/m0-exchange-opening.json"),
        [
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2000),
            (202, 2000),
        ]
        .into_iter()
        .map(|(id, n)| Resource::new(id, n).unwrap())
        .collect(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}
fn env(s: &Store, action: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![action],
    }
}
fn run(s: &mut Store, action: Action) {
    let e = env(s, action);
    let r = s.execute(Caller::System, e).unwrap();
    assert!(r.model.is_ok(), "{action:?}: {:?}", r.model);
    assert!(r.has_persistent_receipt);
}
fn i(command: I) -> Action {
    Action::Inbox { command }
}
fn x(command: X) -> Action {
    Action::Exchange { command }
}
fn full(s: &mut Store) {
    run(s, i(I::Enable));
    for quote in 1..=4 {
        run(s, x(X::Dispatch { quote }));
    }
    run(
        s,
        x(X::SetLink {
            edge: 1,
            open: false,
        }),
    );
    for quote in 1..=4 {
        run(s, i(I::Incident { quote }));
    }
}
#[test]
fn full_inbox_retains_obligations_handoff_does_not_erase_debt_and_delivery_uses_reserved_slots() {
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    let full_state = s.engine().unwrap().world().view();
    let inbox = full_state.exchange.unwrap().inbox;
    assert_eq!(inbox.facts.iter().flatten().count(), 8);
    assert_eq!(inbox.accrued.value(), 100);
    let e = env(&s, x(X::Dispatch { quote: 5 }));
    let failed = e.clone();
    assert_eq!(
        s.execute(Caller::System, e).unwrap().model,
        Err(TxError::InboxFull)
    );
    let bad = env(&s, i(I::Archive { sequence: 1 }));
    assert_eq!(
        s.execute(Caller::System, bad).unwrap().model,
        Err(TxError::UnsettledResponsibility)
    );
    run(
        &mut s,
        i(I::Handoff {
            sequence: 5,
            manager: 1,
        }),
    );
    let before = s.engine().unwrap().world().view().exchange.unwrap().inbox;
    assert_eq!(
        before.all().find(|f| f.sequence == 5).unwrap().debtor,
        inbox.facts[4].unwrap().debtor
    );
    assert_eq!(before.accrued, inbox.accrued);
    let hash = s.state_hash().unwrap();
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    run(
        &mut s,
        x(X::SetLink {
            edge: 1,
            open: true,
        }),
    );
    run(&mut s, Action::RegionAdvance { seconds: 120 });
    let after = s.engine().unwrap().world().view();
    let b = after.exchange.unwrap().inbox;
    assert_eq!(b.accrued.value(), 120);
    assert!(b.facts.iter().flatten().all(|f| f.received_at.is_some()));
    assert!(after
        .exchange
        .unwrap()
        .shipments
        .iter()
        .all(Option::is_none));
    let cash: i64 = after.accounts.iter().map(|a| a.cash.value()).sum();
    assert_eq!(cash, 3000000);
    let receipt = env(&s, i(I::Receive { quote: 1 }));
    let receipt_result = s.execute(Caller::System, receipt.clone()).unwrap().model;
    let hash = s.state_hash().unwrap();
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    assert_eq!(
        s.execute(Caller::System, receipt).unwrap().model,
        receipt_result
    );
    assert_eq!(s.state_hash().unwrap(), hash);
    for sequence in 1..=8 {
        run(&mut s, i(I::Settle { sequence }));
        run(&mut s, i(I::Archive { sequence }));
    }
    let b = s.engine().unwrap().world().view().exchange.unwrap().inbox;
    assert_eq!(b.accrued, b.paid);
    assert!(b.facts.iter().all(Option::is_none));
    assert_eq!(b.closed, 8);
    // Notification under a fresh request ID is still the same canonical delivery.
    run(&mut s, i(I::Receive { quote: 1 }));
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .exchange
            .unwrap()
            .inbox
            .accrued,
        b.accrued
    );
    assert_eq!(
        s.execute(Caller::System, failed).unwrap().model,
        Err(TxError::InboxFull)
    );
    println!("inbox: normal=4 emergency=4 full=8; four paused deliveries resume in preallocated slots; 120c liabilities paid once; handoff retains debtor/creditor; restart and old/fresh notification retries exact");
}
#[test]
fn normal_disk_exhaustion_preserves_emergency_exit_and_quota_on_reopen() {
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: used + 160 * 1024,
        emergency_bytes: 96 * 1024,
    })
    .unwrap();
    // Fill normal storage with real persisted business rejections. No history deletion.
    let mut refused = 0;
    for _ in 0..100 {
        let e = env(&s, Action::RegionActivate { city: 2 });
        match s.execute(Caller::System, e) {
            Ok(_) => {}
            Err(Error::Capacity { .. }) => {
                refused += 1;
                break;
            }
            Err(e) => panic!("{e:?}"),
        }
    }
    assert_eq!(refused, 1);
    assert!(s.capacity_status().unwrap().normal_blocked);
    run(&mut s, x(X::Cancel { quote: 5 }));
    run(
        &mut s,
        i(I::Handoff {
            sequence: 5,
            manager: 1,
        }),
    );
    run(
        &mut s,
        x(X::SetLink {
            edge: 1,
            open: true,
        }),
    );
    run(&mut s, Action::RegionAdvance { seconds: 120 });
    for sequence in 1..=8 {
        run(&mut s, i(I::Settle { sequence }));
        run(&mut s, i(I::Archive { sequence }));
    }
    let status = s.capacity_status().unwrap();
    let hash = s.state_hash().unwrap();
    drop(s);
    let s = Store::open(&t.path(), RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.capacity_status().unwrap().total_bytes, status.total_bytes);
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .exchange
            .unwrap()
            .inbox
            .accrued,
        s.engine()
            .unwrap()
            .world()
            .view()
            .exchange
            .unwrap()
            .inbox
            .paid
    );
    println!("disk normal full: cancellation, repair, handoff, delivery, payment and release succeeded using persisted reserve; used={}",status.used_bytes);
}
#[test]
fn physical_facts_and_exit_survive_real_process_exits_and_exact_retries() {
    let t = Temp::new();
    let seed = t.0.join("seed");
    let mut s = create(&seed);
    full(&mut s);
    run(
        &mut s,
        x(X::SetLink {
            edge: 1,
            open: true,
        }),
    );
    let cmd = env(&s, Action::RegionAdvance { seconds: 120 });
    let old = s.state_hash().unwrap();
    let result = s.execute(Caller::System, cmd.clone()).unwrap().model;
    let new = s.state_hash().unwrap();
    drop(s);
    // Rebuild baseline for each crash; actual CURRENT may confirm old or new root.
    for point in [
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
    ] {
        let p = t.0.join(point);
        let mut s = create(&p);
        full(&mut s);
        run(
            &mut s,
            x(X::SetLink {
                edge: 1,
                open: true,
            }),
        );
        assert_eq!(s.state_hash().unwrap(), old);
        drop(s);
        let command = t.0.join(format!("{point}.json"));
        fs::write(&command, serde_json::to_vec(&cmd).unwrap()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
            .args([
                "activity-commit",
                p.to_str().unwrap(),
                command.to_str().unwrap(),
                point,
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(97),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut s = Store::open(&p, RULES).unwrap();
        let current = s.state_hash().unwrap();
        assert!(current == old || current == new);
        assert_eq!(
            s.execute(Caller::System, cmd.clone()).unwrap().model,
            result
        );
        assert_eq!(s.state_hash().unwrap(), new);
    }
    println!("inbox_delivery_actual_process_exits=11; facts, four invoices, materials, capacity and once-only handling liabilities share one durable root");
}

fn external(event: u32) -> mycity_core::inbox::ExternalFact {
    mycity_core::inbox::ExternalFact {
        source: 1,
        event,
        debtor: 2,
        creditor: 4,
        amount: mycity_core::units::Money::new(700000).unwrap(),
        due: 50,
        reason: mycity_core::inbox::ExternalReason::EmergencyRepair,
    }
}
#[test]
fn external_pending_full_inbox_restart_unpaid_handoff_and_source_alignment() {
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    s.register_external_source().unwrap();
    for event in 1..=5 {
        s.queue_external(external(event)).unwrap();
    }
    for event in 1..=4 {
        s.receive_external(external(event)).unwrap();
    }
    let before = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    assert!(matches!(
        s.receive_external(external(5)),
        Err(Error::FactBlocked("world_fact_inbox_full"))
    ));
    assert_eq!(s.state_hash().unwrap(), before);
    assert_eq!(s.committed_sequence(), seq);
    let advance = env(&s, Action::RegionAdvance { seconds: 1 });
    assert!(matches!(
        s.execute(Caller::System, advance),
        Err(Error::SourceSyncRequired)
    ));
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    assert_eq!(s.source_status().unwrap().pending, vec![external(5)]);
    assert!(!s.source_status().unwrap().reconciled);
    // Unpaid liability is moved intact to a separately bounded collection ledger.
    let fact = s
        .engine()
        .unwrap()
        .world()
        .view()
        .exchange
        .unwrap()
        .inbox
        .all()
        .find(|f| f.external == Some(external(1)))
        .copied()
        .unwrap();
    let bad = env(
        &s,
        i(I::Archive {
            sequence: fact.sequence,
        }),
    );
    assert_eq!(
        s.execute(Caller::System, bad).unwrap().model,
        Err(TxError::UnsettledResponsibility)
    );
    run(
        &mut s,
        i(I::Handoff {
            sequence: fact.sequence,
            manager: 1,
        }),
    );
    let retained = s
        .engine()
        .unwrap()
        .world()
        .view()
        .exchange
        .unwrap()
        .inbox
        .retained
        .into_iter()
        .flatten()
        .find(|f| f.sequence == fact.sequence)
        .unwrap();
    assert_eq!(retained.debtor, fact.debtor);
    assert_eq!(retained.creditor, fact.creditor);
    assert_eq!(retained.fee, fact.fee);
    s.receive_external(external(5)).unwrap();
    s.reconcile_external_source(6).unwrap();
    let hash = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    s.receive_external(external(5)).unwrap();
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.committed_sequence(), seq);
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    assert!(!s.source_status().unwrap().reconciled);
    assert!(matches!(
        s.reconcile_external_source(7),
        Err(Error::SourceSyncRequired)
    ));
    s.reconcile_external_source(6).unwrap();
    run(
        &mut s,
        i(I::Settle {
            sequence: fact.sequence,
        }),
    );
    let view = s.engine().unwrap().world().view();
    let after = view
        .exchange
        .unwrap()
        .inbox
        .retained
        .into_iter()
        .flatten()
        .find(|f| f.sequence == fact.sequence)
        .unwrap();
    assert!(after.paid < after.fee);
    assert!(after.paid.value() > 0);
    assert_eq!(
        view.accounts.iter().map(|a| a.cash.value()).sum::<i64>(),
        3000000
    );
    println!("external: 12/12 working full; event5 NoAck and source pending survive restart; unpaid handoff frees one working slot without erasing 700000c claim; replay once; explicit upstream frontier required");
}
#[test]
fn source_publication_crashes_retry_from_durable_pending_and_missing_source_is_not_ignored() {
    use mycity_storage::SourcePoint;
    for point in [
        SourcePoint::Queued,
        SourcePoint::WorldCommitted,
        SourcePoint::Acknowledged,
    ] {
        let t = Temp::new();
        let mut s = create(&t.path());
        run(&mut s, i(I::Enable));
        s.register_external_source().unwrap();
        if point == SourcePoint::Queued {
            assert!(s
                .queue_external_with_observer(external(1), |p| if p == point {
                    Err(std::io::Error::other("injected source uncertainty"))
                } else {
                    Ok(())
                })
                .is_err());
        } else {
            s.queue_external(external(1)).unwrap();
            assert!(s
                .receive_external_with_observer(external(1), |p| if p == point {
                    Err(std::io::Error::other("injected ack uncertainty"))
                } else {
                    Ok(())
                })
                .is_err());
        }
        drop(s);
        let mut s = Store::open(&t.path(), RULES).unwrap();
        s.receive_external(external(1)).unwrap();
        s.reconcile_external_source(2).unwrap();
        assert_eq!(
            s.engine()
                .unwrap()
                .world()
                .view()
                .exchange
                .unwrap()
                .inbox
                .accrued
                .value(),
            700000
        );
        let seq = s.committed_sequence();
        s.receive_external(external(1)).unwrap();
        assert_eq!(s.committed_sequence(), seq);
        drop(s);
        fs::remove_file(t.path().join("producer-outbox.json")).unwrap();
        assert!(Store::open(&t.path(), RULES).is_err());
    }
}
#[test]
fn invalid_and_unchanged_controls_cannot_spend_emergency_storage() {
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: used + 112 * 1024,
        emergency_bytes: 96 * 1024,
    })
    .unwrap();
    // Make sure even a short failure frame is beyond normal quota, not merely the max-frame preflight.
    for _ in 0..200 {
        let e = env(&s, i(I::Receive { quote: 1 }));
        match s.execute(Caller::Account(2), e) {
            Ok(_) => {}
            Err(Error::Capacity { .. }) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    let before = s.committed_sequence();
    let used = s.capacity_status().unwrap().used_bytes;
    let invalid = env(&s, i(I::Incident { quote: 1 }));
    assert!(matches!(
        s.execute(Caller::Account(2), invalid),
        Err(Error::Capacity { .. })
    ));
    let noop = env(&s, i(I::Incident { quote: 1 }));
    assert!(matches!(
        s.execute(Caller::System, noop),
        Err(Error::Capacity { .. })
    ));
    assert_eq!(s.committed_sequence(), before);
    assert_eq!(s.capacity_status().unwrap().used_bytes, used);
    run(
        &mut s,
        i(I::Handoff {
            sequence: 5,
            manager: 1,
        }),
    );
}
#[test]
fn query_pages_are_complete_provenanced_and_reject_stale_cursor() {
    use mycity_storage::Query;
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    let first = s.query(Query::Contracts, None, 3).unwrap();
    assert_eq!(first.rows.len(), 3);
    let second = s
        .query(Query::Contracts, first.next_cursor.as_deref(), 3)
        .unwrap();
    assert_eq!(second.rows.len(), 3);
    let third = s
        .query(Query::Contracts, second.next_cursor.as_deref(), 3)
        .unwrap();
    assert_eq!(third.rows.len(), 2);
    assert!(third.next_cursor.is_none());
    let page = s.query(Query::Receipts, None, 2).unwrap();
    assert!(page.rows[0]["record"]["actions"].is_array());
    s.checkpoint().unwrap();
    let mut cursor = None;
    let mut count = 0;
    loop {
        let p = s.query(Query::Receipts, cursor.as_deref(), 3).unwrap();
        count += p.rows.len();
        cursor = p.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, s.committed_sequence() as usize);
    assert!(s
        .query(Query::Contracts, first.next_cursor.as_deref(), 3)
        .is_err());
    for q in [
        Query::Fulfilment,
        Query::Arrears,
        Query::Capacity,
        Query::Blocked,
        Query::Recovery,
    ] {
        assert!(!s.query(q, None, 64).unwrap().rows.is_empty());
    }
    s.register_external_source().unwrap();
    let p = s.query(Query::Blocked, None, 1).unwrap();
    s.queue_external(external(1)).unwrap();
    assert!(s
        .query(Query::Blocked, p.next_cursor.as_deref(), 1)
        .is_err());
}

#[test]
fn actual_external_producer_and_ack_process_exits_preserve_exactly_once() {
    for point in ["Queued", "WorldCommitted", "Acknowledged"] {
        let t = Temp::new();
        let mut s = create(&t.path());
        run(&mut s, i(I::Enable));
        s.register_external_source().unwrap();
        if point != "Queued" {
            s.queue_external(external(1)).unwrap();
        }
        drop(s);
        let fact = t.0.join("fact.json");
        fs::write(&fact, serde_json::to_vec(&external(1)).unwrap()).unwrap();
        let cmd = if point == "Queued" {
            "source-queue"
        } else {
            "source-receive"
        };
        let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
            .args([
                cmd,
                t.path().to_str().unwrap(),
                fact.to_str().unwrap(),
                point,
            ])
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(97),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut s = Store::open(&t.path(), RULES).unwrap();
        s.receive_external(external(1)).unwrap();
        s.reconcile_external_source(2).unwrap();
        let hash = s.state_hash().unwrap();
        let seq = s.committed_sequence();
        s.receive_external(external(1)).unwrap();
        assert_eq!(s.state_hash().unwrap(), hash);
        assert_eq!(s.committed_sequence(), seq);
        let mut changed = external(1);
        changed.due += 1;
        assert!(s.receive_external(changed).is_err());
        assert_eq!(s.state_hash().unwrap(), hash);
    }
    println!("external_source_actual_process_exits=3; durable pending before application; world-root and source-ack interruption retries once");
}
#[test]
fn producer_is_bounded_and_pauses_without_dropping_the_seventeenth_fact() {
    let t = Temp::new();
    let mut s = create(&t.path());
    run(&mut s, i(I::Enable));
    s.register_external_source().unwrap();
    for event in 1..=16 {
        s.queue_external(external(event)).unwrap();
    }
    assert!(matches!(
        s.queue_external(external(17)),
        Err(Error::FactBlocked("producer_pending_full"))
    ));
    assert_eq!(s.source_status().unwrap().pending.len(), 16);
    assert_eq!(s.source_status().unwrap().next_event, 17);
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    assert_eq!(s.source_status().unwrap().pending.len(), 16);
    assert!(matches!(
        s.reconcile_external_source(17),
        Err(Error::SourceSyncRequired)
    ));
}

#[test]
fn normal_rotation_at_threshold_cannot_consume_the_emergency_reserve() {
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    while s.engine().unwrap().journal().len() < 512 {
        run(&mut s, Action::RegionActivate { city: 2 });
    }
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: used + 112 * 1024,
        emergency_bytes: 96 * 1024,
    })
    .unwrap();
    let before = s.capacity_status().unwrap().used_bytes;
    let generation = s.checkpoint_status().unwrap().generation;
    assert!(matches!(
        s.checkpoint(),
        Err(Error::Capacity {
            emergency: false,
            ..
        })
    ));
    assert_eq!(s.capacity_status().unwrap().used_bytes, before);
    assert_eq!(s.checkpoint_status().unwrap().generation, generation);
    let e = env(&s, Action::RegionActivate { city: 3 });
    assert!(matches!(
        s.execute(Caller::System, e),
        Err(Error::Capacity {
            emergency: false,
            ..
        })
    ));
    run(
        &mut s,
        i(I::Handoff {
            sequence: 5,
            manager: 1,
        }),
    );
    assert_eq!(s.checkpoint_status().unwrap().generation, generation + 1);
    assert_eq!(s.engine().unwrap().journal().len(), 513);
}

#[test]
fn no_room_for_necessary_fact_stops_advancement_and_retains_source_without_false_ack() {
    let t = Temp::new();
    let mut s = create(&t.path());
    run(&mut s, i(I::Enable));
    s.register_external_source().unwrap();
    s.queue_external(external(1)).unwrap();
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: used + 8192,
        emergency_bytes: 4096,
    })
    .unwrap();
    let hash = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    assert!(matches!(
        s.receive_external(external(1)),
        Err(Error::Capacity { .. })
    ));
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.committed_sequence(), seq);
    assert_eq!(s.source_status().unwrap().pending, vec![external(1)]);
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    let advance = env(&s, Action::RegionAdvance { seconds: 1 });
    assert!(matches!(
        s.execute(Caller::System, advance),
        Err(Error::SourceSyncRequired)
    ));
    assert_eq!(s.source_status().unwrap().pending, vec![external(1)]);
    assert!(!s
        .query(mycity_storage::Query::Blocked, None, 64)
        .unwrap()
        .rows
        .is_empty());
}

#[test]
fn external_cannot_bypass_registered_pending_or_take_the_incident_reserve() {
    let t = Temp::new();
    let mut s = create(&t.path());
    run(&mut s, i(I::Enable));
    let mut e = env(&s, i(I::External { fact: external(1) }));
    e.operation_id = external(1).operation_id();
    assert!(matches!(
        s.execute(Caller::System, e.clone()),
        Err(Error::FactBlocked(_))
    ));
    let mut ordinary = env(&s, Action::RegionActivate { city: 2 });
    ordinary.operation_id = external(1).operation_id();
    assert_eq!(
        s.execute(Caller::System, ordinary).unwrap().model,
        Err(TxError::InvalidOperation)
    );
    for quote in 1..=4 {
        run(&mut s, x(X::Dispatch { quote }));
    }
    s.register_external_source().unwrap();
    for event in 1..=5 {
        s.queue_external(external(event)).unwrap();
    }
    for event in 1..=4 {
        s.receive_external(external(event)).unwrap();
    }
    assert!(matches!(
        s.receive_external(external(5)),
        Err(Error::FactBlocked("world_fact_inbox_full"))
    ));
    run(
        &mut s,
        x(X::SetLink {
            edge: 1,
            open: false,
        }),
    );
    for quote in 1..=4 {
        run(&mut s, i(I::Incident { quote }));
    }
    assert_eq!(
        s.engine()
            .unwrap()
            .world()
            .view()
            .exchange
            .unwrap()
            .inbox
            .facts
            .iter()
            .flatten()
            .count(),
        12
    );
    let hash = s.state_hash().unwrap();
    let mut malformed = env(&s, i(I::External { fact: external(5) }));
    malformed.operation_id = external(5).operation_id();
    malformed.expected_revision = 0;
    assert!(matches!(
        s.execute(Caller::System, malformed),
        Err(Error::FactBlocked(_))
    ));
    assert_eq!(s.state_hash().unwrap(), hash);
}

#[test]
fn paused_clock_and_repeated_link_toggles_are_not_unbounded_emergency_work() {
    let t = Temp::new();
    let mut s = create(&t.path());
    full(&mut s);
    run(&mut s, Action::RegionAdvance { seconds: 120 });
    let e = env(&s, Action::RegionAdvance { seconds: 1 });
    assert!(!s
        .engine()
        .unwrap()
        .can_advance_existing_responsibility(Caller::System, &e));
    let repair = env(
        &s,
        x(X::SetLink {
            edge: 1,
            open: true,
        }),
    );
    assert!(s
        .engine()
        .unwrap()
        .can_advance_existing_responsibility(Caller::System, &repair));
    run(
        &mut s,
        x(X::SetLink {
            edge: 1,
            open: true,
        }),
    );
    let again = env(
        &s,
        x(X::SetLink {
            edge: 1,
            open: false,
        }),
    );
    assert!(!s
        .engine()
        .unwrap()
        .can_advance_existing_responsibility(Caller::System, &again));
}

#[test]
fn archived_fact_creation_and_actual_cash_settlement_remain_directly_traceable() {
    let t = Temp::new();
    let mut s = create(&t.path());
    run(&mut s, i(I::Enable));
    let create = env(&s, x(X::Dispatch { quote: 1 }));
    let created = s
        .execute(Caller::System, create.clone())
        .unwrap()
        .model
        .unwrap();
    assert_eq!(created.created_facts[0], Some(1));
    run(&mut s, Action::RegionAdvance { seconds: 120 });
    let settle = env(&s, i(I::Settle { sequence: 1 }));
    let receipt = s
        .execute(Caller::System, settle.clone())
        .unwrap()
        .model
        .unwrap();
    assert_eq!(receipt.settled[0].unwrap().value(), 5);
    run(&mut s, i(I::Archive { sequence: 1 }));
    s.checkpoint().unwrap();
    drop(s);
    let mut s = Store::open(&t.path(), RULES).unwrap();
    let hash = s.state_hash().unwrap();
    assert_eq!(
        s.execute(Caller::System, settle)
            .unwrap()
            .model
            .unwrap()
            .settled[0]
            .unwrap()
            .value(),
        5
    );
    assert_eq!(s.state_hash().unwrap(), hash);
    let rows = s
        .query(mycity_storage::Query::Receipts, None, 64)
        .unwrap()
        .rows;
    let creation = rows
        .iter()
        .find(|row| row["record"]["operation_id"] == create.operation_id)
        .unwrap();
    assert_eq!(creation["record"]["outcome"]["Ok"]["created_facts"][0], 1);
    let payment = rows
        .iter()
        .find(|row| row["record"]["outcome"]["Ok"]["settled"][0] == 5)
        .unwrap();
    assert_eq!(payment["record"]["actions"][0]["command"]["sequence"], 1);
}
