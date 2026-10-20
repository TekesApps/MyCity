use mycity_core::{
    inbox::{Command as I, ExternalFact, ExternalReason},
    schedule::Phase,
    timeline::{Command as T, Cycle, PayrollSpec},
    transaction::{Action, Caller, Envelope, Resource},
    units::Money,
};
use mycity_storage::{CapacityPolicy, Error, Query, Store};
use serde_json::json;
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
            "mycity-timeline-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn m(n: i64) -> Money {
    Money::new(n).unwrap()
}
fn create(path: &Path, zero: bool) -> Store {
    let mut v: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    if zero {
        v["accounts"][2]["cash"] = json!(0);
        v["sources"][2]["amount"] = json!(0);
    }
    Store::create(path, &serde_json::to_vec(&v).unwrap(), vec![], RULES).unwrap()
}
fn envelope(s: &Store, a: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![a],
    }
}
fn a(c: T) -> Action {
    Action::Timeline { command: c }
}
fn apply(s: &mut Store, action: Action) -> Envelope {
    let e = envelope(s, action);
    let r = s.execute(Caller::System, e.clone()).unwrap();
    assert!(r.model.is_ok(), "{r:?}");
    assert!(r.has_persistent_receipt);
    e
}
fn timeline(s: &mut Store, c: T) -> Envelope {
    apply(s, a(c))
}
fn drive(s: &mut Store, to: u64, limit: u16) {
    for _ in 0..100000 {
        timeline(s, T::Advance { target: to, limit });
        if s.engine()
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
    panic!("unfinished");
}
fn cash(s: &Store, id: u32) -> i64 {
    s.engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == id)
        .unwrap()
        .cash
        .value()
}
#[test]
fn dense_actual_payments_checkpoint_mid_page_retry_and_bounded_query() {
    let t = Temp::new();
    let mut s = create(&t.world(), false);
    timeline(&mut s, T::Enable);
    timeline(
        &mut s,
        T::Claims {
            first_id: 1,
            count: 20000,
            payer: 3,
            payee: 2,
            amount: m(1),
            due: 3600,
        },
    );
    let e = timeline(
        &mut s,
        T::Advance {
            target: 3600,
            limit: 256,
        },
    );
    let h = s.state_hash().unwrap();
    let ticket = s.continuation_status().unwrap().unwrap();
    assert!(ticket.remaining_pages <= 96);
    assert_eq!(cash(&s, 2), 200254);
    s.checkpoint().unwrap();
    drop(s);
    s = Store::open(&t.world(), RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), h);
    assert_eq!(s.continuation_status().unwrap(), Some(ticket.clone()));
    assert!(s.execute(Caller::System, e).unwrap().model.is_ok());
    assert_eq!(s.continuation_status().unwrap(), Some(ticket));
    let page = s.query(Query::Timeline, None, 2).unwrap();
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0]["complete"], false);
    let cursor = page.next_cursor.unwrap();
    drive(&mut s, 3600, 256);
    assert!(s.query(Query::Timeline, Some(&cursor), 2).is_err());
    assert_eq!(cash(&s, 2), 220000);
    assert!(s.continuation_status().unwrap().is_none());
    s.checkpoint().unwrap();
    drop(s);
    let s = Store::open(&t.world(), RULES).unwrap();
    assert!(s
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .claims
        .iter()
        .all(|c| c.paid == m(1)));
    println!(
        "T05_DENSE {}",
        json!({"claims":20000,"actual_paid_cents":20000,"engine_owned_bytes":s.engine().unwrap().owned_capacity_bytes(),"checkpoint":s.checkpoint_status().unwrap(),"physical_capacity":s.capacity_status().unwrap()})
    );
}
fn restrict_normal(s: &mut Store) {
    let used = s.capacity_status().unwrap().used_bytes;
    let held = s.continuation_status().unwrap().unwrap().remaining_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: used + held + 256 * 1024,
        emergency_bytes: held + 128 * 1024,
    })
    .unwrap();
    assert!(s.capacity_status().unwrap().normal_blocked);
}
#[test]
fn reserved_pure_phase_and_zero_cash_debt_pages_finish_when_normal_full_then_stop() {
    let t = Temp::new();
    let mut s = create(&t.world(), true);
    timeline(&mut s, T::Enable);
    timeline(
        &mut s,
        T::Claims {
            first_id: 1,
            count: 1,
            payer: 3,
            payee: 2,
            amount: m(1),
            due: 3600,
        },
    );
    timeline(
        &mut s,
        T::Advance {
            target: 7200,
            limit: 1,
        },
    );
    restrict_normal(&mut s);
    drop(s);
    s = Store::open(&t.world(), RULES).unwrap();
    while s.continuation_status().unwrap().is_some() {
        timeline(
            &mut s,
            T::Advance {
                target: 7200,
                limit: 1,
            },
        );
    }
    let p = s.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(p.last_complete, 3600);
    assert_eq!(p.target, Some(7200));
    let c = s.engine().unwrap().timeline().unwrap().claims[0];
    assert!(c.processed);
    assert_eq!(c.paid, Money::ZERO);
    assert_eq!(c.principal, m(1));
    let h = s.state_hash().unwrap();
    let e = envelope(
        &s,
        a(T::Advance {
            target: 7200,
            limit: 1,
        }),
    );
    assert!(matches!(
        s.execute(Caller::System, e),
        Err(Error::Capacity { .. })
    ));
    assert_eq!(s.state_hash().unwrap(), h);
    println!(
        "T05_RESERVE {}",
        json!({"complete_boundary":p.last_complete,"remote_goal_not_completed":p.target,"unpaid_cents":1,"clock_extension_rejected":true,"capacity":s.capacity_status().unwrap()})
    );
}
fn exchange(path: &Path) -> Store {
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
fn fact(event: u32, due: u64) -> ExternalFact {
    ExternalFact {
        source: 1,
        event,
        debtor: 4,
        creditor: 2,
        amount: m(1),
        due,
        reason: ExternalReason::EmergencyRepair,
    }
}
#[test]
fn full_fact_inbox_during_boundary_allows_safe_handoff_and_source_reconciliation() {
    let t = Temp::new();
    let mut s = exchange(&t.world());
    apply(&mut s, Action::Inbox { command: I::Enable });
    s.register_external_source().unwrap();
    for n in 1..=12 {
        let f = fact(n, 0);
        s.queue_external(f).unwrap();
        assert!(s.receive_external(f).unwrap().model.is_ok());
    }
    s.reconcile_external_source(13).unwrap();
    timeline(&mut s, T::Enable);
    timeline(
        &mut s,
        T::Advance {
            target: 30,
            limit: 1,
        },
    );
    let f = fact(13, 30);
    s.queue_external(f).unwrap();
    assert!(matches!(s.receive_external(f), Err(Error::FactBlocked(_))));
    let e = envelope(
        &s,
        a(T::Advance {
            target: 30,
            limit: 1,
        }),
    );
    assert!(matches!(
        s.execute(Caller::System, e),
        Err(Error::SourceSyncRequired)
    ));
    let before = s.engine().unwrap().world().view().accounts;
    apply(
        &mut s,
        Action::Inbox {
            command: I::Handoff {
                sequence: 1,
                manager: 1,
            },
        },
    );
    assert_eq!(before, s.engine().unwrap().world().view().accounts);
    assert!(s.receive_external(f).unwrap().model.is_ok());
    s.reconcile_external_source(14).unwrap();
    drive(&mut s, 30, 1);
    let e = s.engine().unwrap().world().view().exchange.unwrap();
    assert_eq!(e.inbox.retained.iter().flatten().count(), 1);
    assert_eq!(e.inbox.accrued, m(13));
    // A payment during a frozen stage still cannot spend before its designated lane.
    timeline(
        &mut s,
        T::Advance {
            target: 60,
            limit: 1,
        },
    );
    let e = envelope(
        &s,
        Action::Inbox {
            command: I::Settle { sequence: 2 },
        },
    );
    assert!(s.execute(Caller::System, e).unwrap().model.is_err());
}
#[test]
fn continuation_limit_and_receipt_reserve_are_bound_and_retry_never_spends_twice() {
    let t = Temp::new();
    let mut s = create(&t.world(), false);
    timeline(&mut s, T::Enable);
    timeline(
        &mut s,
        T::Claims {
            first_id: 1,
            count: 20000,
            payer: 3,
            payee: 2,
            amount: m(1),
            due: 3600,
        },
    );
    let bad = envelope(
        &s,
        a(T::Advance {
            target: 3600,
            limit: 1,
        }),
    );
    assert!(matches!(
        s.execute(Caller::System, bad),
        Err(Error::Invalid("timeline_page_limit_too_small"))
    ));
    assert!(s
        .engine()
        .unwrap()
        .timeline()
        .unwrap()
        .progress
        .phase
        .is_none());
    let old = timeline(
        &mut s,
        T::Advance {
            target: 3600,
            limit: 256,
        },
    );
    let ticket = s.continuation_status().unwrap();
    let e = envelope(
        &s,
        a(T::Advance {
            target: 3600,
            limit: 8192,
        }),
    );
    assert!(matches!(
        s.execute(Caller::System, e),
        Err(Error::Invalid("timeline_reserved_page_limit"))
    ));
    s.execute(Caller::System, old).unwrap();
    assert_eq!(ticket, s.continuation_status().unwrap());
}
#[test]
fn calendar_payroll_old_interval_and_partial_debt_survive_real_checkpoint_and_reopen() {
    let t = Temp::new();
    let mut s = create(&t.world(), true);
    timeline(&mut s, T::Enable);
    timeline(
        &mut s,
        T::AddPayroll {
            spec: PayrollSpec {
                id: 1,
                payer: 3,
                payee: 2,
                home_city: 0,
                hourly_cents: m(7),
                cycle: Cycle::Daily,
                ends_at: 3 * 86400,
            },
        },
    );
    drive(&mut s, 86400 - 3600, 256);
    let e = timeline(
        &mut s,
        T::Advance {
            target: 86400,
            limit: 1,
        },
    );
    assert_eq!(
        s.engine().unwrap().timeline().unwrap().progress.phase,
        Some(Phase::P0)
    );
    let earned = s.engine().unwrap().timeline().unwrap().payroll[0].accrued;
    assert_eq!(earned, m(56));
    s.checkpoint().unwrap();
    drop(s);
    s = Store::open(&t.world(), RULES).unwrap();
    s.execute(Caller::System, e).unwrap();
    drive(&mut s, 86400, 1);
    let c = s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!(c.accrued, m(56));
    assert_eq!(c.billed, m(56));
    assert_eq!(c.paid, Money::ZERO);
    timeline(&mut s, T::StopWork { id: 1 });
    drive(&mut s, 2 * 86400, 256);
    assert_eq!(
        s.engine().unwrap().timeline().unwrap().payroll[0].accrued,
        m(56)
    );
}
#[test]
fn actual_process_exits_preserve_pure_cursor_zero_cash_and_final_ticket_release() {
    let points = [
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
    let mut exits = 0;
    for phase in [0, 1, 2] {
        for point in points {
            let t = Temp::new();
            let path = t.world();
            let mut s = create(&path, true);
            timeline(&mut s, T::Enable);
            timeline(
                &mut s,
                T::Claims {
                    first_id: 1,
                    count: 1,
                    payer: 3,
                    payee: 2,
                    amount: m(1),
                    due: 3600,
                },
            );
            timeline(
                &mut s,
                T::Advance {
                    target: 3600,
                    limit: 1,
                },
            );
            loop {
                let p = s.engine().unwrap().timeline().unwrap().progress;
                let ready = match phase {
                    0 => p.phase == Some(Phase::P1),
                    1 => p.phase == Some(Phase::P2) && p.lane == 0,
                    2 => p.phase == Some(Phase::P5),
                    _ => false,
                };
                if ready {
                    break;
                }
                timeline(
                    &mut s,
                    T::Advance {
                        target: 3600,
                        limit: 1,
                    },
                );
            }
            restrict_normal(&mut s);
            let serial = s.engine().unwrap().timeline().unwrap().progress.serial;
            let e = envelope(
                &s,
                a(T::Advance {
                    target: 3600,
                    limit: 1,
                }),
            );
            let f = t.0.join("envelope.json");
            fs::write(&f, serde_json::to_vec(&e).unwrap()).unwrap();
            drop(s);
            let out = Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
                .args([
                    "activity-commit",
                    path.to_str().unwrap(),
                    f.to_str().unwrap(),
                    point,
                ])
                .output()
                .unwrap();
            assert_eq!(
                out.status.code(),
                Some(97),
                "{point}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            exits += 1;
            s = Store::open(&path, RULES).unwrap();
            let r = s.execute(Caller::System, e.clone()).unwrap();
            assert!(r.model.is_ok());
            assert_eq!(
                s.engine().unwrap().timeline().unwrap().progress.serial,
                serial + 1
            );
            let ticket = s.continuation_status().unwrap();
            s.execute(Caller::System, e).unwrap();
            assert_eq!(ticket, s.continuation_status().unwrap());
            assert_eq!(cash(&s, 3), 0);
            if phase == 2 {
                assert!(ticket.is_none());
                assert!(s.engine().unwrap().timeline().unwrap().claims[0].processed);
            }
        }
    }
    println!("T05_ACTUAL_PROCESS_EXITS={exits}; pure cursor, zero-cash liability, final boundary under normal disk exhaustion");
    assert_eq!(exits, 33);
}

#[test]
fn stale_advance_flood_cannot_spend_live_ticket_receipt_slots() {
    let t = Temp::new();
    let mut s = create(&t.world(), true);
    timeline(&mut s, T::Enable);
    timeline(
        &mut s,
        T::Claims {
            first_id: 1,
            count: 1,
            payer: 3,
            payee: 2,
            amount: m(1),
            due: 3600,
        },
    );
    timeline(
        &mut s,
        T::Advance {
            target: 3600,
            limit: 1,
        },
    );
    let ticket = s.continuation_status().unwrap();
    let mut rejected = false;
    for id in 10000..12000 {
        let mut e = envelope(
            &s,
            a(T::Advance {
                target: 3600,
                limit: 1,
            }),
        );
        e.operation_id = id;
        e.expected_revision = 0;
        match s.execute(Caller::System, e) {
            Ok(r) => assert!(r.model.is_err()),
            Err(Error::Invalid("timeline_receipt_reserve")) => {
                rejected = true;
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(rejected);
    assert_eq!(ticket, s.continuation_status().unwrap());
    drive(&mut s, 3600, 1);
    assert!(s.continuation_status().unwrap().is_none());
    assert!(s.engine().unwrap().timeline().unwrap().claims[0].processed);
}
