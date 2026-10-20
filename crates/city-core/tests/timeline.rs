use mycity_core::{
    assembly::assemble,
    calendar::Date,
    schedule::Phase,
    timeline::{Command as T, Cycle, PayrollSpec},
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::Money,
};
use serde_json::{json, Value};
fn money(n: i64) -> Money {
    Money::new(n).unwrap()
}
fn engine() -> Engine {
    Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),
        vec![],
    )
    .unwrap()
}
fn env(e: &Engine, a: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: e.world().view().revision + 1,
        expected_revision: e.world().view().revision,
        actions: vec![a],
    }
}
fn action(c: T) -> Action {
    Action::Timeline { command: c }
}
fn run(e: &mut Engine, c: T) {
    let v = env(e, action(c));
    e.execute(Caller::System, v).unwrap();
}
fn advance(e: &mut Engine, to: u64, limit: u16) {
    for _ in 0..100000 {
        if e.journal().len() >= 900 {
            *e = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
        }
        run(e, T::Advance { target: to, limit });
        let p = e.timeline().unwrap().progress;
        if p.target.is_none() {
            return;
        }
    }
    panic!("unbounded scheduling");
}
fn cash(e: &Engine, id: u32) -> i64 {
    e.world()
        .view()
        .accounts
        .iter()
        .find(|a| a.id == id)
        .unwrap()
        .cash
        .value()
}
fn clean_world(e: &Engine) -> Value {
    let mut v = serde_json::to_value(e.world().view()).unwrap();
    v.as_object_mut().unwrap().remove("revision");
    v
}
#[test]
fn real_twenty_thousand_payments_page_and_checkpoint_without_duplicate_cash() {
    let mut e = engine();
    run(&mut e, T::Enable);
    run(
        &mut e,
        T::Claims {
            first_id: 1,
            count: 20000,
            payer: 3,
            payee: 2,
            amount: money(1),
            due: 3600,
        },
    );
    run(
        &mut e,
        T::Advance {
            target: 3600,
            limit: 8192,
        },
    );
    let p = e.timeline().unwrap().progress;
    assert_eq!(p.phase, Some(Phase::P2));
    assert_eq!(p.lane, 0);
    assert_eq!(p.cursor, 8190);
    assert_eq!(cash(&e, 2), 208190);
    let bytes = e.export_checkpoint().unwrap();
    let hash = e.state_hash();
    e = Engine::restore_checkpoint(&bytes).unwrap();
    assert_eq!(e.state_hash(), hash);
    advance(&mut e, 3600, 8192);
    assert_eq!(cash(&e, 2), 220000);
    assert!(e
        .timeline()
        .unwrap()
        .claims
        .iter()
        .all(|c| c.processed && c.paid == money(1)));
    println!(
        "TIMELINE_20000 {}",
        json!({"actual_claims":20000,"cash_paid_cents":20000,"checkpoint_bytes":bytes.len(),"engine_owned_bytes":e.owned_capacity_bytes(),"progress":e.timeline().unwrap().progress})
    );
}
#[test]
fn economic_dynamic_same_time_work_wage_output_and_tax_match_legacy_thirty_days() {
    let input = include_bytes!("../../../fixtures/m0-economy-opening.json");
    let mut a = Engine::new(assemble(input).unwrap(), vec![]).unwrap();
    let mut b = Engine::new(assemble(input).unwrap(), vec![]).unwrap();
    run(&mut b, T::Enable);
    for _ in 0..30 * 24 {
        let v = env(
            &a,
            Action::Economy {
                command: mycity_core::economy::Command::Advance { seconds: 3600 },
            },
        );
        a.execute(Caller::System, v).unwrap();
    }
    advance(&mut b, 30 * 86400, 256);
    assert_eq!(clean_world(&a), clean_world(&b));
    assert_eq!(cash(&b, 3), 816000);
}
#[test]
fn mid_wage_page_is_valid_and_p4_never_changes_money() {
    let mut e = Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-economy-opening.json")).unwrap(),
        vec![],
    )
    .unwrap();
    run(&mut e, T::Enable);
    let target = 9 * 3600;
    let mut saw_wage = false;
    for _ in 0..500 {
        run(&mut e, T::Advance { target, limit: 1 });
        let p = e.timeline().unwrap().progress;
        let econ = e.world().view().economy.unwrap();
        if econ.phase == mycity_core::economy::Phase::Produce {
            saw_wage = true;
            let h = e.state_hash();
            e = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
            assert_eq!(h, e.state_hash());
        }
        if p.target.is_none() {
            break;
        }
    }
    assert!(saw_wage);
    // Jump close to day 30 without overflowing the finite hot receipt table.
    e = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
    advance(&mut e, 30 * 86400 - 3600, 256);
    e = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
    let target = 30 * 86400;
    let mut observed = false;
    for _ in 0..100 {
        let before = e.world().view().accounts;
        let p = e.timeline().unwrap().progress;
        run(&mut e, T::Advance { target, limit: 1 });
        if p.phase == Some(Phase::P4) {
            assert_eq!(before, e.world().view().accounts);
            observed = true;
        }
        if e.timeline().unwrap().progress.target.is_none() {
            break;
        }
    }
    assert!(observed);
}
fn date(y: u16, m: u8, d: u8) -> u64 {
    Date::new(y, m, d).unwrap().midnight().value() as u64
}
fn spec(id: u32, cycle: Cycle, end: u64) -> PayrollSpec {
    PayrollSpec {
        id,
        payer: 3,
        payee: 2,
        home_city: 0,
        hourly_cents: money(7),
        cycle,
        ends_at: end,
    }
}
#[test]
fn daily_weekly_monthly_same_earned_wages_fraction_carry_and_exit_debt() {
    let mut results = vec![];
    for cycle in [Cycle::Daily, Cycle::Weekly, Cycle::Monthly { anchor: 31 }] {
        let mut e = engine();
        run(&mut e, T::Enable);
        run(
            &mut e,
            T::AddPayroll {
                spec: spec(1, cycle, 40 * 86400),
            },
        );
        advance(&mut e, 3 * 86400 + 9 * 3600 + 1, 256);
        let before = e.timeline().unwrap().payroll[0];
        assert_eq!(before.accrued, money(168));
        assert_eq!(before.remainder, 7);
        run(&mut e, T::Terminate { id: 1 });
        let c = e.timeline().unwrap().payroll[0];
        assert_eq!(c.paid, c.accrued);
        assert_eq!(c.remainder, 7);
        results.push((c.accrued, c.work_seconds, c.remainder));
    }
    assert!(results.windows(2).all(|p| p[0] == p[1]));
}
#[test]
fn monthly_anchor_leap_century_year_end_stop_and_frequency_change_keep_old_due() {
    for (y, m, d, next, after) in [
        (2001, 1, 31, date(2001, 2, 28), date(2001, 3, 31)),
        (2004, 1, 31, date(2004, 2, 29), date(2004, 3, 31)),
        (2100, 1, 31, date(2100, 2, 28), date(2100, 3, 31)),
        (2400, 1, 31, date(2400, 2, 29), date(2400, 3, 31)),
        (2001, 12, 31, date(2002, 1, 31), date(2002, 2, 28)),
    ] {
        assert_eq!(
            Cycle::Monthly { anchor: 31 }.next(date(y, m, d)).unwrap(),
            next
        );
        assert_eq!(Cycle::Monthly { anchor: 31 }.next(next).unwrap(), after);
    }
    let mut e = engine();
    run(&mut e, T::Enable);
    advance(&mut e, date(2004, 1, 31), 8192);
    run(
        &mut e,
        T::AddPayroll {
            spec: spec(1, Cycle::Monthly { anchor: 31 }, date(2004, 4, 1)),
        },
    );
    run(
        &mut e,
        T::ChangeCycle {
            id: 1,
            cycle: Cycle::Weekly,
        },
    );
    advance(&mut e, date(2004, 2, 28), 8192);
    assert_eq!(e.timeline().unwrap().payroll[0].paid, Money::ZERO);
    advance(&mut e, date(2004, 2, 29), 8192);
    assert_eq!(
        e.timeline().unwrap().payroll[0].next_due,
        Some(date(2004, 3, 7))
    );
    let earned = e.timeline().unwrap().payroll[0].accrued;
    run(&mut e, T::StopWork { id: 1 });
    advance(&mut e, date(2004, 3, 7), 8192);
    assert_eq!(e.timeline().unwrap().payroll[0].accrued, earned);
}
#[test]
fn inactive_city_gets_no_autonomous_work_and_payment_insufficiency_keeps_claim() {
    let mut e = Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-region-opening.json")).unwrap(),
        vec![],
    )
    .unwrap();
    run(&mut e, T::Enable);
    let mut c = spec(1, Cycle::Daily, 3 * 86400);
    c.home_city = 1;
    run(&mut e, T::AddPayroll { spec: c });
    advance(&mut e, 86400, 256);
    assert_eq!(e.timeline().unwrap().payroll[0].work_seconds, 0);
    let v = env(&e, Action::RegionActivate { city: 1 });
    e.execute(Caller::System, v).unwrap();
    advance(&mut e, 2 * 86400, 256);
    let c = e.timeline().unwrap().payroll[0];
    assert_eq!(c.work_seconds, 28800);
    assert_eq!(c.accrued, money(56));
    assert_eq!(c.paid, Money::ZERO);
    assert_eq!(c.billed, money(56));
}
#[test]
fn one_clock_no_new_action_in_frozen_stage_and_target_cannot_change() {
    let mut e = engine();
    run(&mut e, T::Enable);
    run(
        &mut e,
        T::Advance {
            target: 3600,
            limit: 1,
        },
    );
    let h = e.state_hash();
    for a in [
        action(T::Advance {
            target: 7200,
            limit: 256,
        }),
        action(T::Claims {
            first_id: 1,
            count: 1,
            payer: 3,
            payee: 2,
            amount: money(1),
            due: 7200,
        }),
        Action::RegionAdvance { seconds: 1 },
    ] {
        let v = Envelope {
            world: 1,
            branch: 1,
            operation_id: e.journal().len() as u64 + 1,
            expected_revision: e.world().view().revision,
            actions: vec![a],
        };
        assert!(e.execute(Caller::System, v).is_err());
        assert_eq!(e.state_hash(), h);
    }
    let mut v = env(
        &e,
        action(T::Advance {
            target: 3600,
            limit: 256,
        }),
    );
    v.operation_id = 99;
    assert_eq!(e.execute(Caller::Account(2), v), Err(TxError::Unauthorized));
}
#[test]
fn joint_boot_transport_periodic_debt_and_population_use_real_dynamic_sources() {
    let mut e = Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-exchange-opening.json")).unwrap(),
        [
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2000),
            (202, 2000),
        ]
        .into_iter()
        .map(|(i, n)| Resource::new(i, n).unwrap())
        .collect(),
    )
    .unwrap();
    for a in [
        Action::Boot {
            command: mycity_core::boot::Command::Start {
                facility: mycity_core::boot::Facility::Pump,
                reverse_request: false,
            },
        },
        Action::Exchange {
            command: mycity_core::exchange::Command::Dispatch { quote: 1 },
        },
        Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 10,
            tolerance: 0,
        },
    ] {
        let v = env(&e, a);
        e.execute(Caller::System, v).unwrap();
    }
    run(&mut e, T::Enable);
    advance(&mut e, 120, 256);
    let v = e.world().view();
    assert_eq!(v.region.unwrap().interaction_time, 120);
    assert!(v.region.unwrap().transits.iter().all(Option::is_none));
    assert_eq!(
        v.exchange.unwrap().quotes[0].status,
        mycity_core::exchange::Status::Delivered
    );
    assert!(e.leases().iter().all(Option::is_none));
}

#[test]
fn later_page_can_touch_all_sixty_four_admitted_accounts() {
    let mut v: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    for id in 6..=65u32 {
        v["sources"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":id,"kind":"cash_capital","amount":100}));
        v["accounts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":id,"role":"household","cash":100,"source":id}));
    }
    let mut e = Engine::new(assemble(&serde_json::to_vec(&v).unwrap()).unwrap(), vec![]).unwrap();
    run(&mut e, T::Enable);
    run(
        &mut e,
        T::Claims {
            first_id: 1,
            count: 300,
            payer: 3,
            payee: 2,
            amount: money(1),
            due: 3600,
        },
    );
    for (i, payee) in (1..=65u32).filter(|id| *id != 3 && *id != 5).enumerate() {
        run(
            &mut e,
            T::Claims {
                first_id: 301 + i as u64,
                count: 1,
                payer: 3,
                payee,
                amount: money(1),
                due: 3600,
            },
        );
    }
    run(
        &mut e,
        T::Advance {
            target: 3600,
            limit: 256,
        },
    );
    assert!(e.timeline().unwrap().progress.phase.is_some());
    run(
        &mut e,
        T::Advance {
            target: 3600,
            limit: 256,
        },
    );
    assert!(e.timeline().unwrap().progress.target.is_none());
    assert_eq!(cash(&e, 3), 600000 - 363);
    assert!(e
        .timeline()
        .unwrap()
        .claims
        .iter()
        .all(|c| c.paid == c.principal));
    Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
}

#[test]
fn diagnostics_preserve_complete_state_quotes_retries_and_failed_persistence() {
    use mycity_core::diagnostics::{Class, Role, Trace};
    let mut plain = engine();
    run(&mut plain, T::Enable);
    run(
        &mut plain,
        T::Claims {
            first_id: 1,
            count: 20000,
            payer: 3,
            payee: 2,
            amount: money(1),
            due: 3600,
        },
    );
    let mut measured = Engine::restore_checkpoint(&plain.export_checkpoint().unwrap()).unwrap();
    let command = env(
        &plain,
        action(T::Advance {
            target: 3600,
            limit: 8192,
        }),
    );
    let before = measured.export_checkpoint().unwrap();
    let mut quote = Trace::enabled();
    assert!(measured
        .timeline_quote_diagnosed(Caller::System, &command, &mut quote)
        .is_some());
    assert_eq!(measured.export_checkpoint().unwrap(), before);
    assert_eq!(
        quote.summary.roles[Role::Execution.index()][Class::Claims as usize].calls,
        0
    );
    assert!(quote.summary.roles[Role::Quote.index()][Class::Claims as usize].work > 0);
    let mut failed = Trace::enabled();
    let failure = measured.execute_recorded_diagnosed(
        Caller::System,
        command.clone(),
        None,
        |_, _| Err::<(), _>("disk failure"),
        &mut failed,
    );
    assert_eq!(failure, Err("disk failure"));
    assert!(!failed.summary.hook_succeeded && !failed.summary.applied);
    assert_eq!(measured.export_checkpoint().unwrap(), before);
    let expected = plain.execute(Caller::System, command.clone()).unwrap();
    let mut trace = Trace::enabled();
    assert_eq!(
        measured
            .execute_recorded_diagnosed(
                Caller::System,
                command.clone(),
                None,
                |_, _| Ok::<(), ()>(()),
                &mut trace
            )
            .unwrap()
            .unwrap(),
        expected
    );
    assert!(trace.summary.hook_succeeded && trace.summary.applied);
    assert_eq!(trace.summary.roles[0][Class::Claims as usize].work, 8190);
    assert_eq!(
        plain.export_checkpoint().unwrap(),
        measured.export_checkpoint().unwrap()
    );
    let mut duplicate = Trace::enabled();
    measured
        .execute_recorded_diagnosed(
            Caller::System,
            command,
            None,
            |_, _| panic!("duplicate must not persist"),
            &mut duplicate,
        )
        .unwrap_or_else(|_: ()| unreachable!())
        .unwrap();
    assert!(duplicate.summary.replayed && !duplicate.summary.applied);
    assert_eq!(duplicate.summary.roles[0][Class::Prepare as usize].calls, 0);
    assert_eq!(
        plain.export_checkpoint().unwrap(),
        measured.export_checkpoint().unwrap()
    );
}

#[test]
fn diagnostics_distinguish_empty_scan_from_zero_cash_debt_processing() {
    use mycity_core::diagnostics::{Class, Trace};
    let mut value: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    value["accounts"][2]["cash"] = json!(0);
    value["sources"][2]["amount"] = json!(0);
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&value).unwrap()).unwrap(),
        vec![],
    )
    .unwrap();
    run(&mut e, T::Enable);
    run(
        &mut e,
        T::Claims {
            first_id: 1,
            count: 10,
            payer: 3,
            payee: 2,
            amount: money(1),
            due: 3600,
        },
    );
    for (target, expected_work) in [(1800, 0), (3600, 10)] {
        let command = env(
            &e,
            action(T::Advance {
                target,
                limit: 8192,
            }),
        );
        let mut trace = Trace::enabled();
        e.execute_recorded_diagnosed(
            Caller::System,
            command,
            None,
            |_, _| Ok::<(), ()>(()),
            &mut trace,
        )
        .unwrap()
        .unwrap();
        let claims = trace.summary.roles[0][Class::Claims as usize];
        assert_eq!(claims.scanned, 10);
        assert_eq!(claims.work, expected_work);
        assert_eq!(claims.paid, 0);
    }
    assert!(e
        .timeline()
        .unwrap()
        .claims
        .iter()
        .all(|c| c.processed && c.paid == money(0)));
}
