//! #21 incremental coexistence gate; full finite-city contract stays open.
use mycity_core::{assembly,boot::{Command as B,Facility},housing,
    service_delivery_authority::{Command as D,Motion},timeline::{Command as T,Cycle,PayrollSpec,DAY},
    transaction::{Action,Caller,Engine,Envelope},units::Money};
#[path="../../../tests/fixtures/finite-city/opening.rs"]mod fixture;
fn apply(e:&mut Engine,caller:Caller,action:Action) {
    let (scope,revision)=e.world().scope_revision();
    e.execute(caller,Envelope{world:scope.world(),branch:scope.branch(),operation_id:revision+1,expected_revision:revision,actions:vec![action]}).unwrap();
}
fn advance(e:&mut Engine,target:u64) {
    for _ in 0..8192 {
        apply(e,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}});
        let p=e.timeline().unwrap().progress;if p.target.is_none(){assert_eq!(p.last_complete,target);return;}
    }panic!("finite timeline unfinished");
}
#[test]
fn original_service_and_two_market_days_share_one_clock_cash_and_utility_stock() {
    let input=fixture::input();let bytes=serde_json::to_vec(&input).unwrap();
    let root=std::path::PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ROOT").expect("retained SSD evidence root")).canonicalize().unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));std::fs::write(root.join("opening.json"),&bytes).unwrap();
    let mut e=Engine::new(assembly::assemble(&bytes).unwrap(),fixture::resources()).unwrap();
    apply(&mut e,Caller::System,Action::Timeline{command:T::Enable});
    let offer:housing::GenesisLeaseOffer=serde_json::from_value(input["housing"]["lease_offers"][0].clone()).unwrap();
    apply(&mut e,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
    apply(&mut e,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:21,payer:7,payee:2,home_city:0,hourly_cents:Money::new(10).unwrap(),cycle:Cycle::Daily,ends_at:3*DAY}}});
    for (facility,until) in [(Facility::Pump,60),(Facility::Generator,120)] {apply(&mut e,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut e,until);}
    advance(&mut e,28800);
    apply(&mut e,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});
    advance(&mut e,33900);
    assert_eq!(e.service_needs_summary().unwrap().delivery.unwrap().trips[0].motion,Motion::Finished);
    for day in 1..=2 {
        advance(&mut e,day*DAY);apply(&mut e,Caller::System,Action::TownSettle{profile_id:1});
        let v=e.world().view();assert_eq!(v.boot.unwrap().time,day*DAY);assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);
        assert_eq!(v.stocks.iter().find(|s|s.id==15).unwrap().quantity.value(),1000-100*day as i64);
        assert_eq!(v.stocks.iter().find(|s|s.id==3002).unwrap().quantity.value(),400000-100000*day as i64);
        let cursor=&v.town.as_ref().unwrap().state.tax_cursors.as_slice()[0];
        // Each day: sales1000 - raw100 - utility100 - earned/paid labor80 =720;
        // original 50% daily rule assesses360, leaving360 retained.
        assert_eq!(cursor.cumulative_assessed.value(),360*day as i64);
        assert_eq!(cursor.retained_earnings_cents,360*day as i64);
        std::fs::write(root.join(format!("day{day}-checkpoint.json")),e.export_checkpoint().unwrap()).unwrap();
    }
    let p=&e.timeline().unwrap().payroll[0];assert_eq!(p.work_seconds,16*3600);assert_eq!(p.paid.value(),160);
}

#[test]
fn original_boot_profiles_still_refuse_the_new_unrelated_stock_shape() {
    for profile in [mycity_core::boot::PROFILE,mycity_core::boot::SCALE_PROFILE] {
        let mut v=fixture::input();v["boot"]["profile"]=serde_json::json!(profile);
        assert_eq!(assembly::assemble(&serde_json::to_vec(&v).unwrap()).err().unwrap(),"boot_profile");
    }
}

#[test]
fn local_profile_refuses_region_instead_of_silently_selecting_another_clock() {
    let mut input=fixture::input();
    let regional:serde_json::Value=serde_json::from_slice(include_bytes!("fixtures/issue-120/canonical/core-town-utility-opening.json")).unwrap();
    input["region"]=regional["region"].clone();
    assert_eq!(assembly::assemble(&serde_json::to_vec(&input).unwrap()).err().unwrap(),"boot_profile");
}
