//! #21 local Boot profile plus existing service/Town, same-slot cold restore.
use mycity_core::{boot::{Command as B,Facility},housing,timeline::{Command as T,Cycle,PayrollSpec,DAY},
 service_delivery_authority::{Command as D,Motion},transaction::{Action,Caller,Envelope},units::Money};
use mycity_storage::{Store,WritePoint};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},process::{Command,Stdio},time::{Duration,Instant}};
#[path="../../../tests/fixtures/finite-city/opening.rs"]mod fixture;
const RULES:&str="finite-city-local-boot-v1";
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope {
 let(scope,revision)=s.engine().unwrap().world().scope_revision();let env=Envelope{world:scope.world(),branch:scope.branch(),operation_id:revision+1,expected_revision:revision,actions:vec![action]};
 let r=s.execute(caller,env.clone()).unwrap();assert!(r.model.is_ok(),"{r:?}");env
}
fn advance(s:&mut Store,target:u64)->Envelope {
 for _ in 0..8192 {let limit=s.continuation_status().unwrap().map_or(256,|r|r.page_limit);let last=apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit}});let p=s.engine().unwrap().timeline().unwrap().progress;if p.target.is_none(){assert_eq!(p.last_complete,target);return last;}}panic!("unfinished");
}
fn cold(root:&Path,s:Store,replay:&Envelope,label:&str) {
 let input=root.join(format!("{label}-cold.json"));fs::write(&input,json!({"world":root.join("world"),"hash":s.state_hash().unwrap(),"sequence":s.committed_sequence(),"replay":replay}).to_string()).unwrap();
 drop(s);
 let mut child=Command::new(std::env::current_exe().unwrap()).args(["--ignored","--exact","finite_city_cold_probe","--test-threads=1","--nocapture"]).env("MYCITY_FINITE_CITY_COLD",&input).stdout(Stdio::from(fs::File::create(root.join(format!("{label}-cold.log"))).unwrap())).stderr(Stdio::from(fs::File::create(root.join(format!("{label}-cold.stderr"))).unwrap())).spawn().unwrap();
 let start=Instant::now();loop {if let Some(status)=child.try_wait().unwrap(){fs::write(root.join(format!("{label}-cold-result.json")),json!({"pid":child.id(),"exit":status.code(),"seconds":start.elapsed().as_secs_f64()}).to_string()).unwrap();assert!(status.success());return;}if start.elapsed()>Duration::from_secs(60){child.kill().unwrap();child.wait().unwrap();panic!("cold timeout");}std::thread::sleep(Duration::from_millis(20));}
}
#[test]#[ignore]
fn finite_city_cold_probe() {
 let input:Value=serde_json::from_slice(&fs::read(std::env::var_os("MYCITY_FINITE_CITY_COLD").unwrap()).unwrap()).unwrap();let mut s=Store::open(Path::new(input["world"].as_str().unwrap()),RULES).unwrap();
 assert!(s.engine().unwrap().service_needs_is_authenticated());assert_eq!(serde_json::to_value(s.state_hash().unwrap()).unwrap(),input["hash"]);
 let e:Envelope=serde_json::from_value(input["replay"].clone()).unwrap();assert!(s.execute(Caller::System,e).unwrap().model.is_ok());
 assert_eq!(s.committed_sequence(),input["sequence"].as_u64().unwrap());assert_eq!(serde_json::to_value(s.state_hash().unwrap()).unwrap(),input["hash"]);
}
fn shared_store(raw:Value,label:&str,expected_electric:i64) {
 let base=PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ROOT").expect("retained SSD evidence")).canonicalize().unwrap();assert!(base.starts_with("/Volumes/HP P900/dev/MyCity/work"));let root=base.join(label);fs::create_dir(&root).unwrap();
 let bytes=serde_json::to_vec(&raw).unwrap();fs::write(root.join("opening.json"),&bytes).unwrap();let mut s=Store::create(&root.join("world"),&bytes,fixture::resources(),RULES).unwrap();
 apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
 apply(&mut s,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:21,payer:7,payee:2,home_city:0,hourly_cents:Money::new(10).unwrap(),cycle:Cycle::Daily,ends_at:3*DAY}}});
 for(facility,until)in[(Facility::Pump,60),(Facility::Generator,120)]{apply(&mut s,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut s,until);}
 advance(&mut s,28800);apply(&mut s,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});advance(&mut s,33900);assert_eq!(s.engine().unwrap().service_needs_summary().unwrap().delivery.unwrap().trips[0].motion,Motion::Finished);
 advance(&mut s,DAY);let first=apply(&mut s,Caller::System,Action::TownSettle{profile_id:1});cold(&root,s,&first,"wal-day1");
 let mut s=Store::open(&root.join("world"),RULES).unwrap();advance(&mut s,2*DAY);let second=apply(&mut s,Caller::System,Action::TownSettle{profile_id:1});s.checkpoint().unwrap();
 let e=s.engine().unwrap();let v=e.world().view();assert_eq!(v.boot.unwrap().time,2*DAY);assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);assert_eq!(v.stocks.iter().find(|s|s.id==3002).unwrap().quantity.value(),expected_electric);assert_eq!(v.town.unwrap().state.tax_cursors.as_slice()[0].cumulative_assessed.value(),720);
 cold(&root,s,&second,"checkpoint-day2");
}

#[test]fn actual_service_and_market_day_restore_from_wal_then_checkpoint_without_double_cost(){shared_store(fixture::input(),"store",200000);}
#[test]fn actual_boot_supply_and_service_restore_from_same_wal_and_checkpoint(){shared_store(fixture::boot_service_input(),"boot-store",70000);}

#[test]
fn boot_use_survives_actual_task_reclamation_and_new_consumption() {
 // This focused consumer-history case has no Town producer. The separate two-day
 // same-Store case checks Town coexistence; no frozen load/profile is reduced.
 let mut raw=fixture::boot_service_input();raw.as_object_mut().unwrap().remove("town");
 raw["qualification"]["facts"]["authority"]["valid_until"]=json!(100000000);raw["qualification"]["facts"]["baseline_sources"][0]["valid_until"]=json!(100000000);
 for w in raw["service_needs"]["profile"]["LocalDevelopmentWithDeliveryV2"]["crews"][0]["wages"].as_array_mut().unwrap().iter_mut().filter(|w|!w.is_null()){w["valid_until"]=json!(100000000);}
 let root=PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ROOT").unwrap()).join("reclaimed");fs::create_dir(&root).unwrap();let bytes=serde_json::to_vec(&raw).unwrap();fs::write(root.join("opening.json"),&bytes).unwrap();
 let mut s=Store::create(&root.join("world"),&bytes,fixture::resources(),RULES).unwrap();apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});
 let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
 for(facility,until)in[(Facility::Pump,60),(Facility::Generator,120),(Facility::Generator,180)]{apply(&mut s,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut s,until);}
 for n in 0..5u64 {
  let at=n*180*DAY+28800;advance(&mut s,at);apply(&mut s,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});let completed=advance(&mut s,at+5100);
  let cp:Value=serde_json::from_slice(&s.engine().unwrap().export_checkpoint().unwrap()).unwrap();let d=&cp["world"]["service_needs"]["delivery"];
  assert_eq!(d["boot_use"]["quantity"],json!([(n+1)*130000,(n+1)*10000]));assert_eq!(cp["world"]["boot"]["exports"],d["boot_use"]["quantity"]);
  assert_eq!(cp["world"]["boot"]["exported_basis"].as_u64().unwrap(),d["boot_use"]["basis"][0].as_u64().unwrap()+d["boot_use"]["basis"][1].as_u64().unwrap());
  if n==4 {assert_eq!(d["domain"]["history"]["reclaimed"],1);assert_eq!(d["trips"].as_array().unwrap().len(),4);}
  fs::write(root.join(format!("completed-{n}.json")),serde_json::to_vec(&cp).unwrap()).unwrap();println!("ACTUAL_BOOT_JOB {n} at {at}");
  if n==0||n==4 {s.checkpoint().unwrap();cold(&root,s,&completed,if n==0{"before-reclaim"}else{"after-reclaim"});s=Store::open(&root.join("world"),RULES).unwrap();}
 }
 let v=s.engine().unwrap().world().view();assert_eq!(v.stocks.iter().find(|s|s.id==3002).unwrap().quantity.value(),150000);assert_eq!(v.stocks.iter().find(|s|s.id==3003).unwrap().quantity.value(),430000);
}

#[test]
fn interrupted_consumption_keeps_boot_stock_and_service_meter_on_one_original_ticket() {
 for lost_reply in [false,true] {
  let root=PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ROOT").unwrap()).join(if lost_reply{"lost-reply"}else{"partial-wal"});fs::create_dir(&root).unwrap();let raw=fixture::boot_service_input();let bytes=serde_json::to_vec(&raw).unwrap();fs::write(root.join("opening.json"),&bytes).unwrap();let mut s=Store::create(&root.join("world"),&bytes,fixture::resources(),RULES).unwrap();
  apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
  for(facility,until)in[(Facility::Pump,60),(Facility::Generator,120)]{apply(&mut s,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut s,until);}
  advance(&mut s,28800);apply(&mut s,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});advance(&mut s,29100);s.checkpoint().unwrap();
  let before=s.state_hash().unwrap();let seq=s.committed_sequence();let(scope,revision)=s.engine().unwrap().world().scope_revision();let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:revision+1,expected_revision:revision,actions:vec![Action::Timeline{command:T::Advance{target:29160,limit:256}}]};
  let fault=if lost_reply{WritePoint::CurrentRenamed}else{WritePoint::WalPayloadHalf};let result=s.execute_with_observer(Caller::System,e.clone(),|point|if point==fault{Err(std::io::Error::other("retained interrupted original consumption"))}else{Ok(())});assert!(result.is_err());fs::write(root.join("write-failure.txt"),format!("{result:?}")).unwrap();drop(s);
  let mut s=Store::open(&root.join("world"),RULES).unwrap();if lost_reply{assert_eq!(s.committed_sequence(),seq+1);assert_ne!(s.state_hash().unwrap(),before);}else{assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),before);}
  assert!(s.execute(Caller::System,e.clone()).unwrap().model.is_ok());let progress=s.engine().unwrap().timeline().unwrap().progress;if progress.target.is_some()||progress.last_complete!=29160{advance(&mut s,29160);}
  let cp:Value=serde_json::from_slice(&s.engine().unwrap().export_checkpoint().unwrap()).unwrap();assert_eq!(cp["world"]["service_needs"]["delivery"]["boot_use"]["quantity"],json!([3000,0]));assert_eq!(cp["world"]["boot"]["exports"],json!([3000,0]));
  let after=s.state_hash().unwrap();let committed=s.committed_sequence();assert!(s.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),committed);assert_eq!(s.state_hash().unwrap(),after);s.checkpoint().unwrap();
 }
}
