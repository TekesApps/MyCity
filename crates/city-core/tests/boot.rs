use mycity_core::{
    assembly::assemble,
    boot::{Boot, Command, Facility, Phase},
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::{Money, Quantity},
};
use serde_json::{json, Value};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-boot-opening.json");
fn resources(cap: u32) -> Vec<Resource> {
    vec![
        Resource::new(102, cap).unwrap(),
        Resource::new(101, cap).unwrap(),
    ]
}
fn engine() -> Engine {
    Engine::new(assemble(INPUT).unwrap(), resources(2)).unwrap()
}
fn variant(f: impl FnOnce(&mut Value)) -> Engine {
    let mut v: Value = serde_json::from_slice(INPUT).unwrap();
    f(&mut v);
    Engine::new(
        assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),
        resources(2),
    )
    .unwrap()
}
fn env(e: &Engine, c: Command) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: e.world().view().revision,
        actions: vec![Action::Boot { command: c }],
    }
}
fn apply(e: &mut Engine, c: Command) {
    let cmd = env(e, c);
    e.execute(Caller::System, cmd).unwrap();
    check(e);
}
fn boot(e: &Engine) -> Boot {
    e.world().view().boot.unwrap()
}
fn start(facility: Facility) -> Command {
    Command::Start {
        facility,
        reverse_request: false,
    }
}
fn step(e: &mut Engine) {
    apply(e, Command::Advance { seconds: 60 })
}
fn stock(e: &Engine, id: u32) -> (i64, i64) {
    let v = e.world().view();
    let s = v.stocks.iter().find(|s| s.id == id).unwrap();
    (s.quantity.value(), s.basis.value())
}
fn reject(e: &mut Engine, c: Command) {
    let before = e.state_hash();
    let cmd = env(e, c);
    assert!(e.execute(Caller::System, cmd).is_err());
    assert_eq!(e.state_hash(), before);
    check(e);
}
fn modify(v: &mut Value, id: u32, qty: i64, basis: i64) {
    let s = v["stocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["id"] == id)
        .unwrap();
    s["quantity"] = json!(qty);
    s["basis"] = json!(basis);
    v["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["id"] == id + 4)
        .unwrap()["amount"] = json!(basis);
}
fn check(e: &Engine) {
    let v = e.world().view();
    let b = v.boot.unwrap();
    let mut q = [0_i128; 6];
    let mut basis = i128::from(b.failed_expense.value());
    for (n, id) in b.stocks.iter().enumerate() {
        let s = v.stocks.iter().find(|s| s.id == *id).unwrap();
        q[n] = i128::from(s.quantity.value());
        basis += i128::from(s.basis.value());
        assert!(s.quantity <= b.limits[n]);
    }
    for j in b.jobs {
        for (i, a) in j.inputs.iter().enumerate() {
            q[i] += i128::from(a.value());
        }
        basis += i128::from(j.cost.value());
    }
    assert_eq!(
        q[0] * 1000 + q[1] + q[5],
        i128::from(b.initial[0].value()) * 1000 + i128::from(b.initial[1].value())
    );
    assert_eq!(
        q[2] + q[3] + q[4],
        i128::from(b.initial[2].value()) + i128::from(b.initial[3].value())
    );
    assert_eq!(
        basis,
        i128::from(b.initial_basis.value()) + i128::from(b.fees.value())
    );
    assert_eq!(
        v.accounts.iter().map(|a| a.cash.value()).sum::<i64>(),
        b.initial_cash.value()
    );
    let held = b.jobs.iter().filter(|j| j.phase != Phase::Idle).count() as u32;
    for r in e.resources() {
        assert!(r.reserved + r.occupied <= r.capacity);
        assert!(r.occupied >= held);
    }
}
#[test]
fn no_seed_rejects_both_dependencies_and_time_does_not_create_outputs() {
    let mut e = variant(|v| modify(v, 2, 0, 0));
    reject(&mut e, start(Facility::Pump));
    reject(&mut e, start(Facility::Generator));
    step(&mut e);
    assert_eq!(stock(&e, 2).0, 0);
    assert_eq!(stock(&e, 3).0, 0);
    assert_eq!(boot(&e).fees, Money::ZERO);
}
#[test]
fn battery_then_actual_water_then_generator_and_parallel_cycles_conserve_inputs_and_fees() {
    let mut e = engine();
    reject(&mut e, start(Facility::Generator));
    apply(&mut e, start(Facility::Pump));
    assert_eq!(stock(&e, 2).0, 0);
    assert_eq!(stock(&e, 3).0, 0);
    apply(&mut e, Command::Advance { seconds: 30 });
    assert_eq!(stock(&e, 3).0, 0);
    apply(&mut e, Command::Advance { seconds: 30 });
    assert_eq!(stock(&e, 3).0, 600000);
    apply(&mut e, start(Facility::Generator));
    step(&mut e);
    assert_eq!(stock(&e, 2).0, 400000);
    apply(&mut e, start(Facility::Pump));
    apply(
        &mut e,
        Command::Start {
            facility: Facility::Generator,
            reverse_request: true,
        },
    );
    step(&mut e);
    for (id, qty) in [
        (1, 3000),
        (2, 700000),
        (3, 1080000),
        (4, 4800000),
        (5, 120000),
        (6, 1400000),
    ] {
        assert_eq!(stock(&e, id).0, qty);
    }
    assert_eq!(boot(&e).fees.value(), 1000);
    assert!(e.resources().iter().all(|r| r.occupied == 0));
}
#[test]
fn water_tank_is_an_alternative_real_seed_not_a_future_generator_promise() {
    let mut e = variant(|v| {
        modify(v, 2, 0, 0);
        modify(v, 3, 60000, 60)
    });
    reject(&mut e, start(Facility::Pump));
    apply(&mut e, start(Facility::Generator));
    assert_eq!(stock(&e, 2).0, 0);
    step(&mut e);
    apply(&mut e, start(Facility::Pump));
    step(&mut e);
    assert_eq!(stock(&e, 3).0, 600000);
}
#[test]
fn reverse_requests_use_same_pool_and_failed_request_leaves_no_partial_claim_or_cash() {
    let mut v: Value = serde_json::from_slice(INPUT).unwrap();
    modify(&mut v, 3, 60000, 60);
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),
        resources(1),
    )
    .unwrap();
    apply(&mut e, start(Facility::Pump));
    let failed = env(
        &e,
        Command::Start {
            facility: Facility::Generator,
            reverse_request: true,
        },
    );
    let before = e.state_hash();
    assert_eq!(
        e.execute(Caller::System, failed.clone()),
        Err(TxError::Capacity)
    );
    assert_eq!(e.state_hash(), before);
    step(&mut e);
    apply(&mut e, start(Facility::Generator));
    let before = e.state_hash();
    assert_eq!(e.execute(Caller::System, failed), Err(TxError::Capacity));
    assert_eq!(e.state_hash(), before);
    step(&mut e);
    // First resource available, second fully held by a generic lease: partial claim rolls back.
    let mut generic = env(&e, start(Facility::Pump));
    generic.actions = vec![Action::ReserveCapacity {
        resource: 102,
        units: 1,
    }];
    e.execute(Caller::System, generic).unwrap();
    let before = e.state_hash();
    reject(&mut e, start(Facility::Pump));
    assert_eq!(e.state_hash(), before);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 101).unwrap().occupied,
        0
    );
}
#[test]
fn faults_keep_actual_losses_and_receivers_then_cleanup_releases_only_work_leases() {
    for facility in [Facility::Pump, Facility::Generator] {
        let mut e = engine();
        if facility == Facility::Generator {
            apply(&mut e, start(Facility::Pump));
            step(&mut e);
        }
        apply(&mut e, start(facility));
        apply(&mut e, Command::Advance { seconds: 30 });
        let fees = boot(&e).fees;
        apply(&mut e, Command::Fault { facility });
        let i = if facility == Facility::Pump { 0 } else { 1 };
        assert_eq!(boot(&e).jobs[i].phase, Phase::Failed);
        assert!(boot(&e).failed_expense > Money::ZERO);
        let wastewater = stock(&e, 5);
        let heat = stock(&e, 6);
        assert!(wastewater.0 > 0 && heat.0 > 0);
        reject(&mut e, start(facility));
        apply(&mut e, Command::ClearFailure { facility });
        assert_eq!(stock(&e, 5), wastewater);
        assert_eq!(stock(&e, 6), heat);
        assert_eq!(boot(&e).fees, fees);
        assert!(e.resources().iter().all(|r| r.occupied == 0));
        assert_eq!(boot(&e).jobs[i].failed, 1);
    }
}
#[test]
fn worst_fault_outlet_reservations_reject_new_work_before_any_consumption() {
    for (index, limit) in [(4, 599999), (5, 99999), (2, 599999)] {
        let mut e = variant(|v| v["boot"]["limits"][index] = json!(limit));
        reject(&mut e, start(Facility::Pump));
        assert_eq!(boot(&e).fees, Money::ZERO);
        assert_eq!(stock(&e, 2).0, 100000);
    }
    // Exact room for one failed pump is insufficient for simultaneous generator leftovers.
    let mut e = variant(|v| {
        modify(v, 2, 500000, 500);
        modify(v, 3, 60000, 60);
        v["boot"]["limits"][4] = json!(600000)
    });
    apply(&mut e, start(Facility::Pump));
    reject(&mut e, start(Facility::Generator));
    apply(
        &mut e,
        Command::Fault {
            facility: Facility::Pump,
        },
    );
    assert_eq!(stock(&e, 5).0, 600000);
}
#[test]
fn fuel_aquifer_battery_cash_and_output_capacity_are_finite() {
    let mut e = engine();
    apply(&mut e, start(Facility::Pump));
    step(&mut e);
    for _ in 0..5 {
        apply(&mut e, start(Facility::Generator));
        step(&mut e);
    }
    assert_eq!(stock(&e, 1).0, 0);
    reject(&mut e, start(Facility::Generator));
    let mut e = variant(|v| modify(v, 4, 599000, 0));
    reject(&mut e, start(Facility::Pump));
    let mut e = variant(|v| {
        v["accounts"][2]["cash"] = json!(199);
        v["sources"][2]["amount"] = json!(199)
    });
    reject(&mut e, start(Facility::Pump));
    let mut e = variant(|v| {
        modify(v, 3, 60000, 60);
        v["boot"]["limits"][1] = json!(399999)
    });
    reject(&mut e, start(Facility::Generator));
}
#[test]
fn authorization_hook_failure_late_batch_error_and_generic_bypasses_cannot_partially_publish() {
    let mut e = engine();
    let c = env(&e, start(Facility::Pump));
    let before = e.state_hash();
    assert_eq!(e.execute(Caller::Account(3), c), Err(TxError::Unauthorized));
    let c = env(&e, start(Facility::Pump));
    assert_eq!(
        e.execute_recorded(Caller::System, c, |_, _| Err("disk")),
        Err("disk")
    );
    assert_eq!(e.state_hash(), before);
    let mut c = env(&e, start(Facility::Pump));
    c.actions.push(Action::Boot {
        command: start(Facility::Generator),
    });
    assert!(e.execute(Caller::System, c).is_err());
    assert_eq!(e.state_hash(), before);
    let mut c = env(&e, start(Facility::Pump));
    c.actions = vec![Action::MoveStock {
        from: e.world().stock_ref(2).unwrap(),
        to: e.world().stock_ref(3).unwrap(),
        quantity: Quantity::new(1000).unwrap(),
    }];
    assert_eq!(e.execute(Caller::System, c), Err(TxError::Unauthorized));
    apply(&mut e, start(Facility::Pump));
    let b = boot(&e);
    let mut c = env(&e, Command::Advance { seconds: 1 });
    c.actions = vec![Action::ReleaseCapacity {
        token: mycity_core::transaction::CapacityToken {
            world: 1,
            branch: 1,
            number: b.jobs[0].tokens[0],
        },
    }];
    let before = e.state_hash();
    assert!(e.execute(Caller::System, c).is_err());
    assert_eq!(e.state_hash(), before);
    step(&mut e);
}
#[test]
fn half_steps_match_one_step_and_due_output_is_not_visible_to_earlier_start() {
    let mut a = engine();
    let mut b = engine();
    apply(&mut a, start(Facility::Pump));
    apply(&mut b, start(Facility::Pump));
    step(&mut a);
    apply(&mut b, Command::Advance { seconds: 30 });
    reject(&mut b, start(Facility::Generator));
    apply(&mut b, Command::Advance { seconds: 30 });
    assert_eq!(boot(&a), boot(&b));
    assert_eq!(stock(&a, 3), stock(&b, 3));
}
#[test]
fn malformed_profiles_sources_units_and_shared_clock_mixes_are_rejected() {
    for mutation in 0..4 {
        let mut v: Value = serde_json::from_slice(INPUT).unwrap();
        match mutation {
            0 => v["boot"]["stocks"][1] = json!(1),
            1 => v["boot"]["profile"] = json!("coal-lc38"),
            2 => v["stocks"][1]["quantity"] = json!(100001),
            _ => v["boot"]["resources"][1] = json!(101),
        }
        assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    assert!(Engine::new(assemble(INPUT).unwrap(), vec![]).is_err());
}

#[test]
fn reversed_resource_request_has_identical_tokens_and_releasing_then_new_id_retry_succeeds() {
    let mut a = engine();
    let mut b = engine();
    apply(&mut a, start(Facility::Pump));
    apply(
        &mut b,
        Command::Start {
            facility: Facility::Pump,
            reverse_request: true,
        },
    );
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn restarting_cold_cleared_work_keeps_old_loss_and_requires_new_real_inputs() {
    for facility in [Facility::Pump, Facility::Generator] {
        let mut e = variant(|v| {
            modify(v, 2, 500000, 500);
            modify(v, 3, 120000, 120)
        });
        apply(&mut e, start(facility));
        apply(&mut e, Command::Fault { facility });
        let b = boot(&e);
        let loss = b.failed_expense;
        let fees = b.fees;
        let used = b.fuel_used;
        let i = if facility == Facility::Pump { 0 } else { 1 };
        let identity = b.jobs[i].identity;
        apply(&mut e, Command::ClearFailure { facility });
        apply(&mut e, start(facility));
        assert_eq!(boot(&e).failed_expense, loss);
        assert!(boot(&e).fees > fees);
        assert_eq!(boot(&e).fuel_used, used);
        assert!(boot(&e).jobs[i].identity > identity);
        assert_eq!(boot(&e).jobs[i].failed, 1);
        step(&mut e);
    }
}

#[test]
fn independent_economic_and_regional_clocks_cannot_be_mixed_with_boot() {
    let mut region: Value = serde_json::from_slice(INPUT).unwrap();
    let r: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-region-opening.json")).unwrap();
    region["region"] = r["region"].clone();
    assert_eq!(
        assemble(&serde_json::to_vec(&region).unwrap())
            .err()
            .unwrap(),
        "boot_profile"
    );
    let mut economic: Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/m0-economy-opening.json"))
            .unwrap();
    let v: Value = serde_json::from_slice(INPUT).unwrap();
    economic["boot"] = v["boot"].clone();
    assert_eq!(
        assemble(&serde_json::to_vec(&economic).unwrap())
            .err()
            .unwrap(),
        "boot_profile"
    );
}
