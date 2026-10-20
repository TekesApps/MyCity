use mycity_core::{
    boot::{Command as B, Facility},
    exchange::Status,
    transaction::Action,
};
use mycity_headless::joint_reference::{self as joint, Reference};
#[test]
fn seven_day_direct_cpu_reference_has_real_wages_tax_arrears_loan_and_freight() {
    let p = joint::policy().unwrap();
    let mut r = Reference::new().unwrap();
    for action in joint::setup(&p).unwrap() {
        r.execute(vec![action]).unwrap();
    }
    r.drive(60).unwrap();
    r.execute(vec![Action::Boot {
        command: B::Start {
            facility: Facility::Generator,
            reverse_request: false,
        },
    }])
    .unwrap();
    r.drive(120).unwrap();
    let exchange = r.engine.world().view().exchange.unwrap();
    for q in exchange.quotes.iter().take(2) {
        assert_eq!(q.status, Status::Delivered);
    }
    let mut observed_arrears = 0;
    for day in 1..=7 {
        r.drive(day * 86400).unwrap();
        if day == 2 {
            r.execute(vec![Action::RegionActivate { city: 2 }]).unwrap();
        }
        if day == 3 {
            r.execute(vec![Action::RegionActivate { city: 1 }]).unwrap();
        }
        if day == 4 {
            let before = r.summary(&p).unwrap();
            observed_arrears = before["tax"]["arrears_cents"].as_i64().unwrap();
            assert!(observed_arrears > 0);
            r.execute(joint::loan(&p).unwrap()).unwrap();
            r.execute(vec![Action::RegionSettleDebt {
                obligation: p.tax.obligation_id,
            }])
            .unwrap();
            assert_eq!(r.summary(&p).unwrap()["tax"]["arrears_cents"], 0);
        }
    }
    let report = r.summary(&p).unwrap();
    assert_eq!(report["tax"]["occurrences"], 7);
    assert_eq!(report["tax"]["paid_cents"], 1400000);
    assert_eq!(report["cash_total"], 3000000);
    assert_eq!(report["accounts"][0]["cash"], 2400000);
    assert_eq!(report["payroll"][0]["paid"], 336);
    assert_eq!(report["payroll"][1]["paid"], 528);
    assert_eq!(report["claims"][0]["paid"], 1000);
    assert_eq!(report["claims"][1]["principal"], 850000);
    assert_eq!(report["claims"][1]["paid"], 0);
    assert_eq!(report["claims"][1]["due"], 864000);
    println!("W4_DIRECT_CPU_ONLY arrears_seen={observed_arrears} report={report}; endurance and process-group resources NOT RUN");
}

#[test]
fn loan_disbursement_and_principal_roll_back_together_and_retry_is_once() {
    use mycity_core::{
        timeline::Command as T,
        transaction::{Caller, Envelope},
        units::Money,
    };
    let mut r = Reference::new().unwrap();
    r.execute(vec![Action::Timeline { command: T::Enable }])
        .unwrap();
    let cash_before = r.engine.world().view().accounts;
    assert!(r
        .execute(vec![Action::Timeline {
            command: T::Lend {
                id: 1,
                lender: 4,
                borrower: 3,
                principal: Money::new(2000000).unwrap(),
                due: 86400
            }
        }])
        .is_err());
    assert_eq!(r.engine.world().view().accounts, cash_before);
    assert!(r.engine.timeline().unwrap().claims.is_empty());
    let env = Envelope {
        world: 1,
        branch: 1,
        operation_id: 1000,
        expected_revision: r.engine.world().view().revision,
        actions: vec![Action::Timeline {
            command: T::Lend {
                id: 1,
                lender: 4,
                borrower: 3,
                principal: Money::new(1000).unwrap(),
                due: 86400,
            },
        }],
    };
    let before = r.engine.state_hash();
    assert!(r
        .engine
        .execute_recorded(Caller::System, env.clone(), |_, _| Err("injected_io"))
        .is_err());
    assert_eq!(r.engine.state_hash(), before);
    assert!(r.engine.timeline().unwrap().claims.is_empty());
    let first = r.engine.execute(Caller::System, env.clone()).unwrap();
    let after = r.engine.state_hash();
    let again = r
        .engine
        .execute_recorded(Caller::System, env, |_, _| -> Result<(), ()> {
            panic!("duplicate loan must not commit")
        })
        .unwrap()
        .unwrap();
    assert_eq!(first, again);
    assert_eq!(r.engine.state_hash(), after);
    let claim = r.engine.timeline().unwrap().claims[0];
    assert_eq!(
        (claim.payer, claim.payee, claim.principal.value()),
        (3, 4, 1000)
    );
    let accounts = r.engine.world().view().accounts;
    assert_eq!(accounts[2].cash.value(), cash_before[2].cash.value() + 1000);
    assert_eq!(accounts[3].cash.value(), cash_before[3].cash.value() - 1000);
}

#[test]
fn full_business_comparison_detects_capacity_differences_outside_world_view() {
    use mycity_core::transaction::Engine;
    let e = joint::engine().unwrap();
    let initial = joint::business_digest(&e).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
    value["resources"][0]["capacity"] = serde_json::json!(2);
    let other = Engine::restore_checkpoint(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(e.world().view()).unwrap(),
        serde_json::to_value(other.world().view()).unwrap()
    );
    assert_ne!(initial, joint::business_digest(&other).unwrap());
}
