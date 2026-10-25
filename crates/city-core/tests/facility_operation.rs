use mycity_core::{facility_authority::{Command as F,ContractView},continuous_supply::{self as s,Command as S},transaction::{Action,Caller,Engine,Envelope,Receipt},timeline::Command as T,venue_runtime::Command as V};
#[path="../../../tests/fixtures/facility-work/opening.rs"]mod fixture;
fn apply(e:&mut Engine,caller:Caller,action:Action)->Receipt{let operation_id=e.world().view().revision+1;let env=Envelope{world:1,branch:1,operation_id,expected_revision:e.world().view().revision,actions:vec![action]};e.execute(caller,env).unwrap()}
fn reject(e:&mut Engine,caller:Caller,action:Action){let old=e.state_hash();let env=Envelope{world:1,branch:1,operation_id:100000+e.world().view().revision,expected_revision:e.world().view().revision,actions:vec![action]};assert!(e.execute(caller,env).is_err());assert_eq!(old,e.state_hash());}
fn work(command:F)->Action{Action::FacilityWork{command}}
fn setup(e:&mut Engine){apply(e,Caller::System,Action::Timeline{command:T::Enable});apply(e,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});let token=e.world().view().venue.unwrap().uses[0].token.number;apply(e,Caller::System,Action::Venue{use_id:1,command:V::Enter{event_id:1000,token_number:token,occurred_at:0}});}
fn begin(e:&mut Engine){apply(e,Caller::System,work(F::Begin{contract_id:1}));}
fn start(e:&mut Engine){apply(e,Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}});}
fn advance(e:&mut Engine,target:u64){for _ in 0..8192{apply(e,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}});let p=e.timeline().unwrap().progress;if p.target.is_none(){assert_eq!(p.last_complete,target);return;}}panic!("finite timeline {:?}",e.timeline().unwrap().progress);}
fn row(e:&Engine)->ContractView{e.world().view().facility_work.unwrap().records[0].clone().unwrap()}
fn cash(e:&Engine,id:u32)->i64{e.world().view().accounts.iter().find(|a|a.id==id).unwrap().cash.value()}
fn source(e:&Engine)->s::View{e.world().view().continuous_supply.unwrap()}
#[test]fn admission_requires_real_presence_actor_qualification_and_funding(){
 let mut e=fixture::engine(fixture::input());reject(&mut e,Caller::System,work(F::Begin{contract_id:1}));setup(&mut e);
 reject(&mut e,Caller::Account(3),work(F::Begin{contract_id:1}));reject(&mut e,Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}});
 begin(&mut e);assert_eq!(row(&e).work_seconds,0);assert_eq!(row(&e).earned_cents,0);assert_eq!(cash(&e,2),200000);start(&mut e);
}
#[test]fn actual_p5_wages_original_cash_cost_and_split_remainders(){
 let mut a=fixture::engine(fixture::input());let mut b=fixture::engine(fixture::input());for e in [&mut a,&mut b]{setup(e);begin(e);start(e);}
 advance(&mut a,61);for t in [1,3,4,17,61]{advance(&mut b,t);}
 let x=row(&a);assert_eq!((x.work_seconds,x.earned_cents,x.paid_cents,x.wage_remainder),(61,61,61,61));assert_eq!(x,row(&b));
 assert_eq!(cash(&a,2),200061);assert_eq!(cash(&a,3),599239);assert_eq!(cash(&a,4),1200700);
 assert_eq!(source(&a).operator_labor_basis.value(),61);assert_eq!(source(&a),source(&b));assert_eq!(a.world().view().stocks,b.world().view().stocks);
}
#[test]fn actual_raw_boundary_closes_operator_preserving_fraction(){let mut e=fixture::engine(fixture::input());setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,5001);let r=row(&e);assert_eq!((r.phase,r.work_seconds,r.earned_cents,r.paid_cents,r.wage_remainder),("stopped",5000,5001,5001,1400));assert_eq!(source(&e).raw_used_ml,100000000);assert_eq!(source(&e).operator_labor_basis.value(),5001);}
#[test]fn explicit_stop_keeps_actual_work_and_cold_checkpoint_is_untrusted(){let mut e=fixture::engine(fixture::input());setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,4);apply(&mut e,Caller::Account(2),work(F::Stop{contract_id:1}));let r=row(&e);assert_eq!((r.phase,r.work_seconds,r.wage_remainder),("stopped",4,4));let bytes=e.export_checkpoint().unwrap();let mut raw=Engine::restore_checkpoint(&bytes).unwrap();assert!(!raw.facility_work_is_authenticated());reject(&mut raw,Caller::System,work(F::Begin{contract_id:1}));assert_eq!(row(&raw),r);}
#[test]fn unknown_or_absent_qualification_and_unfunded_window_refuse_atomically(){
 for mode in 0..3{let mut v=fixture::input();match mode{0=>v["qualification"]["facts"]["baselines"][0]["base_known"]=false.into(),1=>v["qualification"]["facts"]["baselines"][0]["base_mask"]=0.into(),_=>{v["accounts"][2]["cash"]=1.into();v["sources"][2]["amount"]=1.into();}}
 if mode==0{v["qualification"]["facts"]["baselines"][0]["base_mask"]=0.into();}
 let mut e=fixture::engine(v);setup(&mut e);reject(&mut e,Caller::System,work(F::Begin{contract_id:1}));assert_eq!(row(&e).phase,"pending");}
}
#[test]fn shift_and_qualification_boundaries_stop_before_unqualified_production(){
 for qualification in [false,true]{let mut v=fixture::input();if qualification{v["qualification"]["facts"]["baseline_sources"][0]["valid_until"]=3.into();}else{v["facility_work"]["work"]["contracts"][0]["end"]=4.into();}
 let mut e=fixture::engine(v);setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,17);let r=row(&e);assert_eq!((r.phase,r.work_seconds,r.earned_cents,r.wage_remainder),("stopped",4,4,4));assert_eq!(source(&e).raw_used_ml,80000);}
}
#[test]fn safe_exit_and_emergency_stop_reduce_real_responsibility(){
 for exit in [false,true]{let mut e=fixture::engine(fixture::input());setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,4);
 let action=if exit{let token=e.world().view().venue.unwrap().uses[0].token.number;Action::Venue{use_id:1,command:V::SafeExit{event_id:1001,token_number:token,occurred_at:4}}}else{work(F::Stop{contract_id:1})};
 let env=Envelope{world:1,branch:1,operation_id:e.world().view().revision+1,expected_revision:e.world().view().revision,actions:vec![action]};assert!(e.can_advance_existing_responsibility(Caller::System,&env));apply(&mut e,Caller::System,action);assert_eq!(row(&e).phase,"stopped");assert_eq!(source(&e).raw_used_ml,80000);assert_eq!(e.world().view().accounts.iter().find(|a|a.id==3).unwrap().protected.value(),0);}
}
#[test]fn operation_identity_does_not_imply_commit_order(){let mut e=fixture::engine(fixture::input());setup(&mut e);for(op,caller,action)in[(100,Caller::System,work(F::Begin{contract_id:1})),(99,Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}})]{let request=Envelope{world:1,branch:1,operation_id:op,expected_revision:e.world().view().revision,actions:vec![action]};e.execute(caller,request).unwrap();}advance(&mut e,4);assert_eq!(row(&e).paid_cents,4);}
#[test]fn payroll_and_facility_admission_are_mutually_exclusive_in_both_orders(){
 use mycity_core::{timeline::{PayrollSpec,Cycle},units::Money};
 for payroll_first in [false,true]{let mut e=fixture::engine(fixture::input());setup(&mut e);let action=Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:1,payer:3,payee:2,home_city:0,hourly_cents:Money::new(3601).unwrap(),cycle:Cycle::Daily,ends_at:7200}}};
 if payroll_first{apply(&mut e,Caller::System,action);reject(&mut e,Caller::System,work(F::Begin{contract_id:1}));}else{begin(&mut e);reject(&mut e,Caller::System,action);}}
}

#[test]fn actually_paid_facility_wages_reach_the_original_complete_housing_window(){
 let raw=fixture::input();assert_eq!(serde_json::to_vec(&raw).unwrap(),include_bytes!("../../../tests/fixtures/facility-work/electric-v2-opening.json"));
 let mut e=fixture::engine(raw);setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,4);apply(&mut e,Caller::Account(2),work(F::Stop{contract_id:1}));assert_eq!(cash(&e,2),200004);
 advance(&mut e,31*86400);let income=e.housing_paid_wage_window(7).unwrap();
 let root=std::fs::canonicalize(std::env::var_os("MYCITY_FACILITY_TEST_ROOT").expect("explicit retained evidence root")).unwrap();assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));let evidence=root.join(format!("income-window-{}",std::process::id()));std::fs::create_dir(&evidence).unwrap();
 std::fs::write(evidence.join("checkpoint.json"),e.export_checkpoint().unwrap()).unwrap();std::fs::write(evidence.join("actual.json"),serde_json::to_vec_pretty(&serde_json::json!({"income_status":format!("{:?}",income.status),"income_paid":income.actual_paid_wages.value(),"qualifying_contracts":income.qualifying_contracts,"active_contracts":income.active_contracts,"cash":cash(&e,2),"facility":row(&e),"clock":e.timeline().unwrap().progress})).unwrap()).unwrap();println!("RETAINED_INCOME_WINDOW={}",evidence.display());
 assert_eq!(income.status,mycity_core::housing::PaidWageWindowStatus::Complete);
 assert_eq!(income.actual_paid_wages.value(),4);assert_eq!(income.qualifying_contracts,1);assert_eq!(income.active_contracts,0);
}

#[test]fn authority_v2_rejects_old_genesis_and_deleted_checkpoint_versions(){
 let old=include_bytes!("../../../tests/fixtures/facility-work/electric-v1-opening.json");assert!(mycity_core::assembly::assemble(old).is_err());
 for v in [0,1,3]{let mut raw=fixture::input();raw["facility_work"]["authority_version"]=v.into();assert!(mycity_core::assembly::assemble(&serde_json::to_vec(&raw).unwrap()).is_err());}
 let mut e=fixture::engine(fixture::input());setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,4);let bytes=e.export_checkpoint().unwrap();
 for mode in [0,1,2,3]{let mut cp:serde_json::Value=serde_json::from_slice(&bytes).unwrap();match mode{0=>{cp["world"]["facility_work"].as_object_mut().unwrap().remove("version");},1=>cp["world"]["facility_work"]["version"]=1.into(),2=>{cp["world"]["facility_work"].as_object_mut().unwrap().remove("income_window_anchor");},_=>{cp["world"]["facility_work"].as_object_mut().unwrap().remove("income_window_anchor");cp["world"]["housing"].as_object_mut().unwrap().remove("paid_wages");}}
 assert!(Engine::restore_checkpoint(&serde_json::to_vec(&cp).unwrap()).is_err(),"case {mode}");}
}
#[test]fn old_payroll_and_facility_share_the_original_income_buckets_without_overlap(){
 use mycity_core::{timeline::{PayrollSpec,Cycle},units::Money};let mut e=fixture::engine(fixture::input());setup(&mut e);begin(&mut e);start(&mut e);advance(&mut e,4);apply(&mut e,Caller::Account(2),work(F::Stop{contract_id:1}));
 advance(&mut e,86400); // Original Payroll accepts a new contract only at midnight.
 apply(&mut e,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:1,payer:3,payee:2,home_city:0,hourly_cents:Money::new(1).unwrap(),cycle:Cycle::Daily,ends_at:32*86400}}});
 advance(&mut e,31*86400);let income=e.housing_paid_wage_window(7).unwrap();assert_eq!(income.actual_paid_wages.value(),4+29*8);assert_eq!(income.qualifying_contracts,2);assert_eq!(income.active_contracts,1);assert_eq!(cash(&e,2),200004+30*8);
}

#[test]
fn legacy_housing_header_stays_legal_before_the_original_wait_boundary(){
 fn engine(raw:&[u8])->Engine{Engine::new(mycity_core::assembly::assemble(raw).unwrap(),vec![]).unwrap()}
 let mut small=engine(include_bytes!("../../../tests/fixtures/facility-work/legacy-housing2-opening.json"));
 apply(&mut small,Caller::System,Action::Timeline{command:T::Enable});
 let window=small.world().layout()["housing_paid_wage_window_bytes"].as_u64().unwrap();
 let mut large=engine(include_bytes!("../../../tests/fixtures/facility-work/legacy-housing18-opening.json"));
 let before=large.world().layout();let owned=before["owned_capacity_bytes"].as_u64().unwrap();let scratch=mycity_core::housing::MAX_VALIDATION_SCRATCH_BYTES as u64;
 let outcome=large.execute(Caller::System,Envelope{world:1,branch:1,operation_id:1,expected_revision:0,actions:vec![Action::Timeline{command:T::Enable}]});
 eprintln!("LEGACY_HOUSING_BUDGET={} ",serde_json::json!({"world_size":std::mem::size_of::<mycity_core::assembly::World>(),"world_align":std::mem::align_of::<mycity_core::assembly::World>(),"before":before,"original_window_bytes":window,"projected_enable_with_scratch":owned+window+scratch,"world_budget":mycity_core::assembly::WORLD_BYTE_BUDGET,"enable_ok":outcome.is_ok()}));
 assert!(outcome.is_ok(),"original18-unit Enable must remain legal before the unchanged96-byte Wait boundary: {outcome:?}");
}
