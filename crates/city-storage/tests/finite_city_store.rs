//! #21 local Boot profile plus existing service/Town, same-slot cold restore.
use mycity_core::{boot::{Command as B,Facility},housing,timeline::{Command as T,Cycle,PayrollSpec,DAY},
 service_delivery_authority::{Command as D,Motion},transaction::{Action,Caller,Envelope},units::Money};
use mycity_storage::Store;
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},process::{Command,Stdio},time::{Duration,Instant}};
#[path="../../../tests/fixtures/finite-city/opening.rs"]mod fixture;
const RULES:&str="finite-city-local-boot-v1";
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope {
 let(scope,revision)=s.engine().unwrap().world().scope_revision();let env=Envelope{world:scope.world(),branch:scope.branch(),operation_id:revision+1,expected_revision:revision,actions:vec![action]};
 let r=s.execute(caller,env.clone()).unwrap();assert!(r.model.is_ok(),"{r:?}");env
}
fn advance(s:&mut Store,target:u64) {
 for _ in 0..8192 {let limit=s.continuation_status().unwrap().map_or(256,|r|r.page_limit);apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit}});let p=s.engine().unwrap().timeline().unwrap().progress;if p.target.is_none(){assert_eq!(p.last_complete,target);return;}}panic!("unfinished");
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
#[test]
fn actual_service_and_market_day_restore_from_wal_then_checkpoint_without_double_cost() {
 let base=PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ROOT").expect("retained SSD evidence")).canonicalize().unwrap();assert!(base.starts_with("/Volumes/HP P900/dev/MyCity/work"));let root=base.join("store");fs::create_dir(&root).unwrap();
 let raw=fixture::input();let bytes=serde_json::to_vec(&raw).unwrap();fs::write(root.join("opening.json"),&bytes).unwrap();let mut s=Store::create(&root.join("world"),&bytes,fixture::resources(),RULES).unwrap();
 apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
 apply(&mut s,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:21,payer:7,payee:2,home_city:0,hourly_cents:Money::new(10).unwrap(),cycle:Cycle::Daily,ends_at:3*DAY}}});
 for(facility,until)in[(Facility::Pump,60),(Facility::Generator,120)]{apply(&mut s,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut s,until);}
 advance(&mut s,28800);apply(&mut s,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});advance(&mut s,33900);assert_eq!(s.engine().unwrap().service_needs_summary().unwrap().delivery.unwrap().trips[0].motion,Motion::Finished);
 advance(&mut s,DAY);let first=apply(&mut s,Caller::System,Action::TownSettle{profile_id:1});cold(&root,s,&first,"wal-day1");
 let mut s=Store::open(&root.join("world"),RULES).unwrap();advance(&mut s,2*DAY);let second=apply(&mut s,Caller::System,Action::TownSettle{profile_id:1});s.checkpoint().unwrap();
 let e=s.engine().unwrap();let v=e.world().view();assert_eq!(v.boot.unwrap().time,2*DAY);assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);assert_eq!(v.stocks.iter().find(|s|s.id==3002).unwrap().quantity.value(),200000);assert_eq!(v.town.unwrap().state.tax_cursors.as_slice()[0].cumulative_assessed.value(),720);
 cold(&root,s,&second,"checkpoint-day2");
}
