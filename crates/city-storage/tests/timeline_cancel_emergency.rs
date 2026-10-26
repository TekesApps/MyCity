//! A completed boundary may cancel its unexecuted target under the original emergency quota.
//! No obligation, actual occupancy or completed clock is cancelled with that request.
use mycity_core::{
    time_capacity::{Command as C, Registration, Unit},
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource},
    units::Money,
};
use mycity_storage::{CapacityPolicy, Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[path = "../../../tests/fixtures/facility-work/opening.rs"]
mod fixture;
const RULES: &str = "timeline-target-emergency149-v1";
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn case(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("MYCITY_TARGET_EMERGENCY_ROOT").expect("retained external SSD root"),
    )
    .canonicalize()
    .unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p = root.join(format!(
        "{name}-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn cap(command: C) -> Action {
    Action::TimeCapacity { command }
}
fn request(s: &Store, op: &mut u64, a: Action) -> Envelope {
    let e = Envelope {
        world: 1,
        branch: 1,
        operation_id: *op,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![a],
    };
    *op += 1;
    e
}
fn apply(s: &mut Store, p: &Path, op: &mut u64, caller: Caller, a: Action) -> Envelope {
    let e = request(s, op, a);
    let result = s.execute(caller, e.clone());
    let output = json!({"request":e,"caller":format!("{caller:?}"),"result":format!("{result:?}")});
    fs::write(
        p.join(format!("operation-{}.json", e.operation_id)),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();
    assert!(result.unwrap().model.is_ok());
    e
}
fn make(p: &Path, loans: bool) -> (Store, u64, u64) {
    let mut s = Store::create(
        &p.join("world"),
        include_bytes!("../../../fixtures/m0-e0-opening.json"),
        vec![Resource::new(701, 2).unwrap()],
        RULES,
    )
    .unwrap();
    let mut op = 1;
    apply(
        &mut s,
        p,
        &mut op,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    if loans {
        for (id, amount, due) in [(1, 17, 3600), (2, 137, 5000)] {
            apply(
                &mut s,
                p,
                &mut op,
                Caller::System,
                Action::Timeline {
                    command: T::Lend {
                        id,
                        lender: 3,
                        borrower: 4,
                        principal: Money::new(amount).unwrap(),
                        due,
                    },
                },
            );
        }
    }
    apply(
        &mut s,
        p,
        &mut op,
        Caller::System,
        cap(C::Register {
            registration: Registration {
                canonical: 70001,
                resource: 701,
                aliases: [Some(1001), Some(1002), None, None],
                unit: Unit::Count,
                owner: 1,
                operator: 3,
                nominal_capacity: 2,
                effective_capacity: 2,
                identity_version: 1,
                policy_version: 1,
                open: true,
            },
        }),
    );
    apply(
        &mut s,
        p,
        &mut op,
        Caller::Account(3),
        cap(C::Reserve {
            alias: 1001,
            account: 3,
            consumer: 301,
            consumer_version: 1,
            start: 0,
            end: 10000,
            units: 2,
        }),
    );
    let token = s
        .engine()
        .unwrap()
        .leases()
        .iter()
        .flatten()
        .next()
        .unwrap()
        .token;
    apply(
        &mut s,
        p,
        &mut op,
        Caller::System,
        cap(C::Enter {
            token,
            occurred_at: 0,
        }),
    );
    (s, op, token)
}
fn full(s: &mut Store) {
    let used = s.capacity_status().unwrap().used_bytes;
    s.reduce_capacity_policy(CapacityPolicy {
        total_bytes: used + 2 * 1024 * 1024,
        emergency_bytes: 2 * 1024 * 1024 + used / 2,
    })
    .unwrap();
    assert!(s.capacity_status().unwrap().normal_blocked);
}
fn world(s: &Store) -> Value {
    let mut v = serde_json::to_value(s.engine().unwrap().world().view()).unwrap();
    v.as_object_mut().unwrap().remove("revision");
    v
}
fn obligations(s: &Store) -> Value {
    let t = s.engine().unwrap().timeline().unwrap();
    json!({"claims":t.claims,"payroll":t.payroll,"claim_sources":t.claim_sources})
}
fn progress(s: &Store) -> mycity_core::timeline::Progress {
    s.engine().unwrap().timeline().unwrap().progress
}
fn actual(s: &Store) -> u32 {
    s.engine()
        .unwrap()
        .resources()
        .iter()
        .find(|r| r.id == 701)
        .unwrap()
        .occupied
}
fn replay(s: &mut Store, e: &Envelope, caller: Caller) {
    let before = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    assert!(s.execute(caller, e.clone()).unwrap().model.is_ok());
    assert_eq!(s.state_hash().unwrap(), before);
    assert_eq!(s.committed_sequence(), seq);
}
fn cold(p: &Path, mode: &str) {
    let cfg = p.join(format!("cold-{mode}.json"));
    fs::write(
        &cfg,
        serde_json::to_vec(&json!({"world":p.join("world"),"mode":mode})).unwrap(),
    )
    .unwrap();
    let mut c = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cold_completed_target_reader",
            "--test-threads=1",
            "--nocapture",
        ])
        .env("MYCITY_TARGET_EMERGENCY_COLD", &cfg)
        .stdout(fs::File::create(p.join(format!("cold-{mode}.stdout"))).unwrap())
        .stderr(fs::File::create(p.join(format!("cold-{mode}.stderr"))).unwrap())
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(x) = c.try_wait().unwrap() {
            break x;
        }
        if start.elapsed() > Duration::from_secs(30) {
            c.kill().unwrap();
            c.wait().unwrap();
            panic!("own cold child deadline; originals retained");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    fs::write(
        p.join(format!("cold-{mode}-result.json")),
        serde_json::to_vec(&json!({"exit_code":status.code(),"config":cfg,"pid":c.id()})).unwrap(),
    )
    .unwrap();
    assert!(status.success());
}
#[test]
fn completed_target_cancellation_preserves_paid_and_future_claims_then_actual_exit_hot_and_cold() {
    for restart_first in [false, true] {
        let p = case(if restart_first {
            "cold-first"
        } else {
            "hot-first"
        });
        let (mut s, mut op, token) = make(&p, true);
        apply(
            &mut s,
            &p,
            &mut op,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
                    target: 7200,
                    limit: 32,
                },
            },
        );
        let before = progress(&s);
        assert_eq!(before.last_complete, 3600);
        assert_eq!(before.target, Some(7200));
        assert!(before.phase.is_none());
        assert_eq!(before.page_paid.value(), 17);
        assert!(s.continuation_status().unwrap().is_none());
        s.checkpoint().unwrap();
        full(&mut s);
        if restart_first {
            drop(s);
            s = Store::open(&p.join("world"), RULES).unwrap();
            assert_eq!(progress(&s), before);
            assert!(s.capacity_status().unwrap().normal_blocked);
        }
        let facts = world(&s);
        let duties = obligations(&s);
        let e = request(
            &s,
            &mut op,
            Action::Timeline {
                command: T::CancelAdvance,
            },
        );
        assert!(e.actions[0].is_responsibility_control());
        assert!(s
            .engine()
            .unwrap()
            .can_advance_existing_responsibility(Caller::System, &e));
        assert!(s.execute(Caller::System, e.clone()).unwrap().model.is_ok());
        assert!(
            s.receipt(e.operation_id)
                .unwrap()
                .unwrap()
                .emergency_eligible
        );
        let after = progress(&s);
        assert_eq!(after.target, None);
        assert_eq!(
            (after.at, after.last_complete, after.serial, after.phase),
            (before.at, before.last_complete, before.serial, None)
        );
        assert_eq!(after.page_paid, Money::ZERO);
        assert_eq!(after.page_sources, 0);
        assert_eq!(world(&s), facts);
        assert_eq!(obligations(&s), duties);
        assert_eq!(actual(&s), 2);
        assert!(s.continuation_status().unwrap().is_none());
        drop(s);
        cold(&p, "cancelled");
        s = Store::open(&p.join("world"), RULES).unwrap();
        replay(&mut s, &e, Caller::System);
        let exit = apply(
            &mut s,
            &p,
            &mut op,
            Caller::System,
            cap(C::Exit {
                token,
                occurred_at: 3600,
            }),
        );
        assert_eq!(actual(&s), 0);
        assert_eq!(obligations(&s), duties);
        drop(s);
        cold(&p, "exited");
        s = Store::open(&p.join("world"), RULES).unwrap();
        replay(&mut s, &exit, Caller::System);
        replay(&mut s, &e, Caller::System);
        fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"paid_claim17_preserved":true,"future_claim137_preserved":true,"clock":progress(&s),"original_cancel":e,"original_exit":exit,"actual":actual(&s),"continuation":s.continuation_status().unwrap(),"capacity":s.capacity_status().unwrap()})).unwrap()).unwrap();
    }
}
#[test]
fn invalid_cancel_never_borrows_emergency_by_its_name() {
    for variant in [
        "no-target",
        "phase",
        "wrong-caller",
        "old-revision",
        "wrong-scope",
    ] {
        let p = case(variant);
        let (mut s, mut op, _) = make(&p, false);
        if variant != "no-target" {
            apply(
                &mut s,
                &p,
                &mut op,
                Caller::System,
                Action::Timeline {
                    command: T::Advance {
                        target: 7200,
                        limit: if variant == "phase" { 1 } else { 32 },
                    },
                },
            );
        }
        if variant == "phase" {
            assert!(progress(&s).phase.is_some());
            assert!(s.continuation_status().unwrap().is_some());
        }
        full(&mut s);
        let before = s.engine().unwrap().export_checkpoint().unwrap();
        let reserved = serde_json::to_value(s.continuation_status().unwrap()).unwrap();
        let mut e = request(
            &s,
            &mut op,
            Action::Timeline {
                command: T::CancelAdvance,
            },
        );
        let caller = if variant == "wrong-caller" {
            Caller::Account(3)
        } else {
            Caller::System
        };
        if variant == "old-revision" {
            e.expected_revision -= 1;
        }
        if variant == "wrong-scope" {
            e.world = 2;
        }
        assert!(!s
            .engine()
            .unwrap()
            .can_advance_existing_responsibility(caller, &e));
        let result = s.execute(caller, e.clone());
        assert!(!result.as_ref().is_ok_and(|r| r.model.is_ok()));
        assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), before);
        assert_eq!(
            serde_json::to_value(s.continuation_status().unwrap()).unwrap(),
            reserved
        );
        assert_eq!(actual(&s), 2);
        fs::write(
            p.join("refusal.json"),
            serde_json::to_vec_pretty(
                &json!({"request":e,"result":format!("{result:?}"),"continuation":reserved}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
fn copy(src: &Path, dst: &Path) {
    fs::create_dir(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let out = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &out);
        } else {
            fs::copy(e.path(), &out).unwrap();
            assert_eq!(fs::read(e.path()).unwrap(), fs::read(out).unwrap());
        }
    }
}
#[test]
fn interrupted_cancel_and_lost_reply_match_complete_uninterrupted_reference_once() {
    for (name, point) in [
        ("prepared", WritePoint::Prepared),
        ("wal-half", WritePoint::WalPayloadHalf),
        ("applied", WritePoint::Applied),
    ] {
        let p = case(name);
        let (mut s, mut op, token) = make(&p, true);
        apply(
            &mut s,
            &p,
            &mut op,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
                    target: 7200,
                    limit: 32,
                },
            },
        );
        full(&mut s);
        let e = request(
            &s,
            &mut op,
            Action::Timeline {
                command: T::CancelAdvance,
            },
        );
        drop(s);
        copy(&p.join("world"), &p.join("reference"));
        let mut normal = Store::open(&p.join("reference"), RULES).unwrap();
        assert!(normal
            .execute(Caller::System, e.clone())
            .unwrap()
            .model
            .is_ok());
        let hash = normal.state_hash().unwrap();
        let seq = normal.committed_sequence();
        let expected = normal.engine().unwrap().export_checkpoint().unwrap();
        drop(normal);
        s = Store::open(&p.join("world"), RULES).unwrap();
        let result = s.execute_with_observer(Caller::System, e.clone(), |at| {
            if at == point {
                Err(io::Error::other("normal cancellation write interruption"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        fs::write(
            p.join("interruption.json"),
            serde_json::to_vec_pretty(
                &json!({"point":format!("{point:?}"),"request":e,"result":format!("{result:?}")}),
            )
            .unwrap(),
        )
        .unwrap();
        drop(s);
        s = Store::open(&p.join("world"), RULES).unwrap();
        assert!(s.execute(Caller::System, e.clone()).unwrap().model.is_ok());
        assert_eq!(s.state_hash().unwrap(), hash);
        assert_eq!(s.committed_sequence(), seq);
        assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), expected);
        replay(&mut s, &e, Caller::System);
        assert_eq!(actual(&s), 2);
        assert!(progress(&s).target.is_none());
        assert!(s.continuation_status().unwrap().is_none());
        drop(s);
        cold(&p, "cancelled");
        s = Store::open(&p.join("world"), RULES).unwrap();
        apply(
            &mut s,
            &p,
            &mut op,
            Caller::System,
            cap(C::Exit {
                token,
                occurred_at: 3600,
            }),
        );
        assert_eq!(actual(&s), 0);
    }
}
#[test]
fn active_existing_work_supply_and_venue_survive_goal_cancel_before_explicit_exit() {
    use mycity_core::{
        continuous_supply::Command as S, facility_authority::Command as F,
        venue_runtime::Command as V,
    };
    let p = case("existing-work");
    let raw = serde_json::to_vec(&fixture::input()).unwrap();
    fs::write(p.join("opening.json"), &raw).unwrap();
    let mut s = Store::create(&p.join("world"), &raw, fixture::resources(), RULES).unwrap();
    let mut op = 1;
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::Account(3),
        Action::Venue {
            use_id: 1,
            command: V::Accept,
        },
    );
    let token = s
        .engine()
        .unwrap()
        .world()
        .view()
        .venue
        .as_ref()
        .unwrap()
        .uses[0]
        .token
        .number;
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::System,
        Action::Venue {
            use_id: 1,
            command: V::Enter {
                event_id: 1,
                token_number: token,
                occurred_at: 0,
            },
        },
    );
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::System,
        Action::FacilityWork {
            command: F::Begin { contract_id: 1 },
        },
    );
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::Account(3),
        Action::ContinuousSupply {
            command: S::Start {
                run_id: 1,
                pumps: 3,
            },
        },
    );
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::System,
        Action::Timeline {
            command: T::Advance {
                target: 7200,
                limit: 32,
            },
        },
    );
    let before = progress(&s);
    assert!(before.phase.is_none());
    assert_eq!(before.target, Some(7200));
    let facts = world(&s);
    let duties = obligations(&s);
    full(&mut s);
    let e = request(
        &s,
        &mut op,
        Action::Timeline {
            command: T::CancelAdvance,
        },
    );
    assert!(s
        .engine()
        .unwrap()
        .can_advance_existing_responsibility(Caller::System, &e));
    assert!(s.execute(Caller::System, e.clone()).unwrap().model.is_ok());
    assert_eq!(world(&s), facts);
    assert_eq!(obligations(&s), duties);
    assert_eq!(
        s.engine()
            .unwrap()
            .resources()
            .iter()
            .find(|r| r.id == 501)
            .unwrap()
            .occupied,
        1
    );
    drop(s);
    s = Store::open(&p.join("world"), RULES).unwrap();
    replay(&mut s, &e, Caller::System);
    assert_eq!(world(&s), facts);
    apply(
        &mut s,
        &p,
        &mut op,
        Caller::System,
        Action::FacilityWork {
            command: F::Stop { contract_id: 1 },
        },
    );
    let exit = apply(
        &mut s,
        &p,
        &mut op,
        Caller::System,
        Action::Venue {
            use_id: 1,
            command: V::SafeExit {
                event_id: 2,
                token_number: token,
                occurred_at: before.last_complete,
            },
        },
    );
    assert_eq!(
        s.engine()
            .unwrap()
            .resources()
            .iter()
            .find(|r| r.id == 501)
            .unwrap()
            .occupied,
        0
    );
    replay(&mut s, &exit, Caller::System);
    fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"completed_clock":before.last_complete,"business_before_cancel":facts,"after_explicit_stop_and_exit":world(&s),"target":progress(&s).target,"capacity":s.capacity_status().unwrap()})).unwrap()).unwrap();
}
#[test]
#[ignore = "only invoked by actual parent cold process"]
fn cold_completed_target_reader() {
    let cfg = PathBuf::from(
        std::env::var_os("MYCITY_TARGET_EMERGENCY_COLD").expect("parent cold config"),
    );
    let v: Value = serde_json::from_slice(&fs::read(cfg).unwrap()).unwrap();
    let s = Store::open(Path::new(v["world"].as_str().unwrap()), RULES).unwrap();
    assert!(progress(&s).target.is_none());
    assert_eq!(progress(&s).last_complete, 3600);
    assert!(s.continuation_status().unwrap().is_none());
    assert_eq!(actual(&s), if v["mode"] == "cancelled" { 2 } else { 0 });
    let t = s.engine().unwrap().timeline().unwrap();
    assert_eq!(t.claims[0].paid.value(), 17);
    assert_eq!(t.claims[1].paid.value(), 0);
    assert_eq!(t.claims[1].principal.value(), 137);
    assert_eq!(t.claims[1].due, 5000);
    let w = world(&s);
    assert_eq!(w["accounts"][2]["cash"], 599863);
    assert_eq!(w["accounts"][3]["cash"], 1200137);
    println!(
        "original obligations/capacity/clock restored {}",
        s.state_hash().unwrap()
    );
}
