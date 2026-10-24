use mycity_core::{continuous_supply::Command,continuous_genset::Status,transaction::{Action,Caller,Engine,TxError},units::{Money,Quantity}};
#[path="../../../tests/fixtures/continuous-supply/opening.rs"]mod fixture;
use fixture::*;
#[test]
fn actual_original_stocks_two_pumps_same_window_and_heat_water_cost_close(){
    let mut e=engine(input());enable(&mut e,1);let original=start(&mut e,2,1,3);
    let opening=supply(&e);assert_eq!(opening.genset.active_seconds,0);assert_eq!(opening.startup_wip.value(),700);
    assert_eq!(opening.machine_meters.fuel_wip_thirds_mq,51);
    advance(&mut e,3600,10);let s=supply(&e);
    assert_eq!(s.produced_ml,[36_000_000;2]);assert_eq!(s.raw_used_ml,72_000_000);
    assert_eq!(s.genset.received_mwh,12_000_000);assert_eq!(s.machine_meters.electricity_generated_mwh,24_000_000);
    assert_eq!(s.machine_meters.heat_mwh,48_000_000);assert_eq!(s.heat_mwh,60_000_000); // includes both actual pump heats
    assert_eq!(s.machine_meters.wastewater_ml,3_600_000);assert_eq!(s.output_basis.map(|x|x.value()),[53_750;2]);
    let stocks=e.world().view().stocks;assert_eq!(stocks.iter().find(|s|s.id==101).unwrap().quantity.value(),40_000);
    assert_eq!(stocks.iter().find(|s|s.id==102).unwrap().quantity.value(),2_400_000);
    let hash=e.state_hash();let receipt=e.execute(Caller::Account(1),original).unwrap();assert_eq!(receipt.operation_id,2);assert_eq!(hash,e.state_hash());
    assert!(!s.sanitary_quality_known);
}
#[test]
fn split_prefixes_preserve_fractional_energy_lot_cost_and_one_start_fee(){
    let mut a=engine(input());let mut b=engine(input());enable(&mut a,1);enable(&mut b,1);start(&mut a,2,1,3);start(&mut b,2,1,3);
    advance(&mut a,61,10);advance(&mut b,1,10);let one=supply(&b);
    assert_eq!((one.heat_mwh,one.heat_remainder_numerator),(16_666,2));
    assert_eq!((one.genset.received_mwh,one.genset.received_remainder_numerator),(3_333,1));
    assert_eq!(one.machine_meters.fuel_burned_thirds_mq,50);assert_eq!(one.machine_meters.fuel_wip_thirds_mq,1);
    advance(&mut b,17,100);advance(&mut b,61,200);
    let x=supply(&a);let y=supply(&b);assert_eq!(x,y);
    assert_eq!(evidence_accounts(&a),evidence_accounts(&b));
}
fn evidence_accounts(e:&Engine)->Vec<(u32,Money)>{e.world().view().accounts.into_iter().map(|a|(a.id,a.cash)).collect()}
#[test]
fn single_pump_changes_only_its_output_full_sink_cuts_and_stops_original_timeline(){
    let mut v=input();v["continuous_supply"]["water_capacities_ml"][0]=600_000.into();
    let mut e=engine(v);enable(&mut e,1);start(&mut e,2,1,1);advance(&mut e,90,10);
    let s=supply(&e);assert_eq!(s.produced_ml,[600_000,0]);assert_eq!(s.genset.active_seconds,60);
    assert_eq!(s.genset.status,Status::Stopped);assert_eq!(s.cursor,90);assert_eq!(s.heat_mwh,1_000_000);
    assert_eq!(s.runs[0].unwrap().ended_at,Some(60));
    let env=envelope(&e,500,Action::ContinuousSupply{command:Command::Start{run_id:2,pumps:1}});
    let accounts=evidence_accounts(&e);assert_eq!(e.execute(Caller::Account(1),env),Err(TxError::Capacity));assert_eq!(accounts,evidence_accounts(&e));
}
#[test]
fn zero_work_stop_refunds_only_unconverted_lots_and_keeps_actual_start_expense(){
    let mut e=engine(input());enable(&mut e,1);start(&mut e,2,1,3);
    execute(&mut e,3,Caller::Account(1),Action::ContinuousSupply{command:Command::Stop});
    let s=supply(&e);assert_eq!(s.zero_work_expense.value(),700);assert_eq!(s.startup_wip,Money::ZERO);
    assert_eq!(s.machine_meters.fuel_wip_thirds_mq,0);assert_eq!(s.machine_meters.water_wip_ml,0);
    assert_eq!(s.produced_ml,[0;2]);let stocks=e.world().view().stocks;assert_eq!(stocks[0].quantity.value(),100_000);
    start(&mut e,4,2,1);advance(&mut e,1,10);let x=supply(&e);
    assert_eq!(x.machine_meters.start_fees.value(),600);assert_eq!(x.output_basis[0].value(),524);assert_eq!(x.output_basis[1],Money::ZERO);
    execute(&mut e,99,Caller::Account(1),Action::ContinuousSupply{command:Command::Stop});
    assert_eq!(supply(&e).machine_meters.fuel_wip_thirds_mq,1); // 1/3 mQ is not returned/burned twice
}
#[test]
fn immutable_lots_and_receivers_cannot_be_spent_through_generic_moves_or_wrong_actor(){
    let mut e=engine(input());enable(&mut e,1);
    let c=Action::ContinuousSupply{command:Command::Start{run_id:1,pumps:3}};
    assert_eq!(e.execute(Caller::System,envelope(&e,2,c)),Err(TxError::Unauthorized));
    assert_eq!(e.execute(Caller::Account(2),envelope(&e,3,c)),Err(TxError::Unauthorized));
    let from=e.world().stock_ref(104).unwrap();let to=e.world().stock_ref(105).unwrap();
    assert_eq!(e.execute(Caller::Account(1),envelope(&e,4,Action::MoveStock{from,to,quantity:Quantity::new(1).unwrap()})),Err(TxError::Unauthorized));
}
#[test]
fn raw_checkpoint_preserves_fraction_but_is_not_an_authenticated_provider(){
    let mut e=engine(input());enable(&mut e,1);start(&mut e,2,1,3);advance(&mut e,1,10);
    let bytes=e.export_checkpoint().unwrap();let mut raw=Engine::restore_checkpoint(&bytes).unwrap();
    assert_eq!(supply(&e),supply(&raw));assert!(!raw.continuous_supply_is_authenticated());
    let due=e.continuous_supply_due_state().unwrap().unwrap();
    assert_eq!(due.cursor,supply(&e).cursor);
    assert_eq!(Some(due.registration),e.continuous_supply_registration());
    assert_eq!(raw.continuous_supply_due_state(),Err(TxError::InvalidOperation));
    let env=envelope(&raw,500,Action::ContinuousSupply{command:Command::Stop});
    assert_eq!(raw.execute(Caller::Account(1),env),Err(TxError::InvalidOperation));
}
#[test]
fn second_pump_resource_and_joint_fee_failure_roll_back_all_original_business_rows(){
    let mut e=engine(input());enable(&mut e,1);
    let capacity=execute(&mut e,2,Caller::Account(2),Action::ReserveCapacity{resource:303,units:1});
    let before=e.world().view();let resource_state=serde_json::from_slice::<serde_json::Value>(&e.export_checkpoint().unwrap()).unwrap();
    let env=envelope(&e,3,Action::ContinuousSupply{command:Command::Start{run_id:1,pumps:3}});
    assert_eq!(e.execute(Caller::Account(1),env),Err(TxError::Capacity));
    assert_eq!(before,e.world().view());assert_eq!(resource_state["resources"],serde_json::from_slice::<serde_json::Value>(&e.export_checkpoint().unwrap()).unwrap()["resources"]);
    assert_eq!(resource_state["leases"],serde_json::from_slice::<serde_json::Value>(&e.export_checkpoint().unwrap()).unwrap()["leases"]);
    execute(&mut e,4,Caller::Account(2),Action::ReleaseCapacity{token:capacity.created_tokens[0].unwrap()});
    let mut v=input();v["accounts"][0]["cash"]=600.into();v["sources"][0]["amount"]=600.into();
    let mut poor=engine(v);enable(&mut poor,1);let before=poor.world().view();
    let resources=serde_json::from_slice::<serde_json::Value>(&poor.export_checkpoint().unwrap()).unwrap();
    let env=envelope(&poor,2,Action::ContinuousSupply{command:Command::Start{run_id:1,pumps:3}});
    assert!(poor.execute(Caller::Account(1),env).is_err());assert_eq!(before,poor.world().view());
    let after=serde_json::from_slice::<serde_json::Value>(&poor.export_checkpoint().unwrap()).unwrap();assert_eq!(resources["resources"],after["resources"]);assert_eq!(resources["leases"],after["leases"]);
}
#[test]
fn forged_or_missing_original_receipts_never_authenticate_raw_provider(){
    let mut e=engine(input());enable(&mut e,1);start(&mut e,2,1,3);advance(&mut e,1,10);
    let bytes=e.export_checkpoint().unwrap();let mut raw=Engine::restore_checkpoint(&bytes).unwrap();
    let check=raw.begin_continuous_supply_restore().unwrap();assert!(raw.finish_continuous_supply_restore(check).is_err());
    let mut check=raw.begin_continuous_supply_restore().unwrap();
    let wrong=mycity_core::transaction::JournalRecord{actor:1,emergency_eligible:false,operation_id:2,expected_revision:0,payload_sha256:[0;32],
        outcome:Err(TxError::Unauthorized),actions:[None;mycity_core::transaction::MAX_ACTIONS],qualification_context:None};
    assert!(raw.check_continuous_supply_restore_receipt(&mut check,0,&wrong).is_err());assert!(!raw.continuous_supply_is_authenticated());
}
#[test]
fn original_clock_must_exist_before_any_new_machine_commitment(){
    let mut e=engine(input());let before=e.world().view();
    let env=envelope(&e,1,Action::ContinuousSupply{command:Command::Start{run_id:1,pumps:3}});
    assert_eq!(e.execute(Caller::Account(1),env),Err(TxError::InvalidOperation));assert_eq!(before,e.world().view());
}
