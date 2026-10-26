//! Normal original-pool reservation/admission/exit, with complete retained save originals.
use mycity_core::{time_capacity::{Command as C, Registration, Unit}, timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource, CapacityToken}};
use mycity_storage::{Store, WritePoint};
use std::{fs::{self, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
const RULES: &str = "original-timed-pool-v1";
fn case(name: &str) -> PathBuf {
    let root=fs::canonicalize(std::env::var_os("MYCITY_TIME_CAPACITY_ROOT").expect("retained SSD root")).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("{name}-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&p).unwrap();println!("RETAINED_TIMED_POOL={}",p.display());p
}
fn create(p: &Path) -> Store {
    fs::write(p.join("opening.json"),include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    Store::create(&p.join("world"), include_bytes!("../../../fixtures/m0-e0-opening.json"), vec![Resource::new(701,2).unwrap(),Resource::new(702,3).unwrap()],RULES).unwrap()
}
fn env(s: &Store, action: Action) -> Envelope { Envelope { world:1,branch:1,operation_id:s.committed_sequence()+1,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![action] } }
fn action(command: C) -> Action { Action::TimeCapacity { command } }
fn execute(s: &mut Store, p: &Path, caller: Caller, a: Action) -> Envelope {
    let e=env(s,a);let out=s.execute(caller,e.clone()).unwrap();
    let mut f=OpenOptions::new().append(true).create(true).open(p.join("operations.jsonl")).unwrap();writeln!(f,"{}",serde_json::json!({"caller":format!("{caller:?}"),"request":e,"model":format!("{:?}",out.model)})).unwrap();
    assert!(out.model.is_ok(),"{out:?}");e
}
fn reject(s: &mut Store, p: &Path, caller: Caller, a: Action) {
    let before=s.engine().unwrap().business_comparison_checkpoint().unwrap();let e=env(s,a);let result=s.execute(caller,e.clone()).unwrap();
    assert!(result.model.is_err(),"{result:?}");assert_eq!(before,s.engine().unwrap().business_comparison_checkpoint().unwrap());
    let mut f=OpenOptions::new().append(true).create(true).open(p.join("operations.jsonl")).unwrap();writeln!(f,"{}",serde_json::json!({"caller":format!("{caller:?}"),"request":e,"rejected":format!("{:?}",result.model)})).unwrap();
}
fn registration() -> Registration { Registration { canonical:70001,resource:701,aliases:[Some(1001),Some(1002),None,None],unit:Unit::Count,owner:1,operator:3,nominal_capacity:2,effective_capacity:2,identity_version:1,policy_version:1,open:true } }
fn setup(s:&mut Store,p:&Path) { execute(s,p,Caller::System,Action::Timeline{command:T::Enable});execute(s,p,Caller::System,action(C::Register{registration:registration()})); }
fn reserve(s:&mut Store,p:&Path,account:u32,consumer:u64,alias:u64,start:u64,end:u64)->(Envelope,CapacityToken) {
    let e=execute(s,p,Caller::Account(account),action(C::Reserve{alias,account,consumer,consumer_version:1,start,end,units:2}));
    let l=s.engine().unwrap().leases().iter().flatten().max_by_key(|l|l.token).unwrap();
    (e,CapacityToken{world:1,branch:1,number:l.token})
}
fn advance(s:&mut Store,p:&Path,target:u64) {
    for _ in 0..64 { execute(s,p,Caller::System,Action::Timeline{command:T::Advance{target,limit:32}});if s.engine().unwrap().timeline().unwrap().progress.target.is_none(){assert_eq!(s.engine().unwrap().timeline().unwrap().progress.last_complete,target);return;} }panic!("finite original timeline");
}
fn counters(s:&Store)->(u32,u32) {let r=s.engine().unwrap().resources().iter().find(|r|r.id==701).unwrap();(r.reserved,r.occupied)}
fn replay(s:&mut Store,e:Envelope,caller:Caller) {let hash=s.state_hash().unwrap();let seq=s.committed_sequence();assert!(s.execute(caller,e).unwrap().model.is_ok());assert_eq!(seq,s.committed_sequence());assert_eq!(hash,s.state_hash().unwrap());}

#[test]
fn two_original_consumers_share_aliases_and_adjacent_windows_wait_for_actual_exit_after_cold_restore() {
    let p=case("adjacent-consumers");let mut s=create(&p);setup(&mut s,&p);
    // Original company3 and supplier4 use one transfer-gate slot pool, not new people or a travel proof.
    let (a,at)=reserve(&mut s,&p,3,301,1001,0,10);let (b,bt)=reserve(&mut s,&p,4,401,1002,10,20);
    assert_ne!(at,bt);assert_eq!(counters(&s),(2,0));assert_eq!(s.engine().unwrap().leases().iter().flatten().filter(|l|l.resource==701).count(),2);
    reject(&mut s,&p,Caller::Account(4),action(C::Reserve{alias:1002,account:4,consumer:402,consumer_version:1,start:9,end:11,units:1}));
    let entered=execute(&mut s,&p,Caller::System,action(C::Enter{token:at.number,occurred_at:0}));assert_eq!(counters(&s),(2,2));assert!(s.engine().unwrap().time_capacity_projection(1002).unwrap().pending_conflict);
    s.checkpoint().unwrap();drop(s);cold_check(&p,(2,2));let mut s=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(counters(&s),(2,2));replay(&mut s,a,Caller::Account(3));replay(&mut s,b,Caller::Account(4));replay(&mut s,entered,Caller::System);
    advance(&mut s,&p,10);reject(&mut s,&p,Caller::System,action(C::Enter{token:bt.number,occurred_at:10}));assert_eq!(counters(&s),(2,2));
    reject(&mut s,&p,Caller::System,action(C::Cancel{token:at.number}));
    let exited=execute(&mut s,&p,Caller::System,action(C::Exit{token:at.number,occurred_at:10}));assert_eq!(counters(&s),(2,0));
    execute(&mut s,&p,Caller::System,action(C::Enter{token:bt.number,occurred_at:10}));assert_eq!(counters(&s),(0,2));
    advance(&mut s,&p,25);assert_eq!(counters(&s),(0,2));s.checkpoint().unwrap();drop(s);cold_check(&p,(0,2));let mut s=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(counters(&s),(0,2));replay(&mut s,exited,Caller::System);
    execute(&mut s,&p,Caller::System,action(C::Exit{token:bt.number,occurred_at:25}));assert_eq!(counters(&s),(0,0));
    fs::write(p.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"original_accounts":[3,4],"distinct_consumers":[301,401],"aliases_same_original_resource":701,"adjacent_accepted":true,"actual_overhang_waited":true,"actual_exit_unblocked":true,"expiry_did_not_release":true,"original_ticket_cold_retry_once":true,"engine_owned_bytes":s.engine().unwrap().owned_capacity_bytes(),"store_owned_bytes":s.owned_capacity_bytes().unwrap()})).unwrap()).unwrap();
}

#[test]
fn closure_and_effective_reduction_retain_obligations_and_legacy_path_stays_separate() {
    let p=case("closure-legacy");let mut s=create(&p);setup(&mut s,&p);let (_,t)=reserve(&mut s,&p,3,301,1001,0,10);
    execute(&mut s,&p,Caller::System,action(C::Enter{token:t.number,occurred_at:0}));
    execute(&mut s,&p,Caller::System,action(C::SetPolicy{alias:1002,expected_version:1,effective_capacity:1,open:false}));assert_eq!(counters(&s),(0,2));assert!(s.engine().unwrap().time_capacity_projection(1001).unwrap().effective_overcommit);
    reject(&mut s,&p,Caller::Account(3),Action::ReserveCapacity{resource:701,units:1});reject(&mut s,&p,Caller::System,Action::ReleaseCapacity{token:t});
    execute(&mut s,&p,Caller::Account(3),Action::ReserveCapacity{resource:702,units:3});let lease=*s.engine().unwrap().leases().iter().flatten().find(|l|l.resource==702).unwrap();
    reject(&mut s,&p,Caller::Account(4),Action::ReserveCapacity{resource:702,units:1});
    execute(&mut s,&p,Caller::Account(3),Action::ReleaseCapacity{token:CapacityToken{world:1,branch:1,number:lease.token}});
    s.checkpoint().unwrap();drop(s);cold_check(&p,(0,2));let mut s=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(counters(&s),(0,2));
    execute(&mut s,&p,Caller::System,action(C::Exit{token:t.number,occurred_at:0}));assert_eq!(counters(&s),(0,0));
}

#[test]
fn interrupted_reservation_and_lost_entry_reply_recover_original_lease_and_window_once() {
    for (name,point,enter) in [("before-write",WritePoint::Prepared,false),("wal-half",WritePoint::WalPayloadHalf,false),("reply-lost",WritePoint::Applied,true)] {
        let p=case(name);let mut s=create(&p);setup(&mut s,&p);
        let request=if enter {let (_,t)=reserve(&mut s,&p,3,301,1001,0,10);env(&s,action(C::Enter{token:t.number,occurred_at:0}))} else {env(&s,action(C::Reserve{alias:1001,account:3,consumer:301,consumer_version:1,start:0,end:10,units:2}))};
        let caller=if enter{Caller::System}else{Caller::Account(3)};
        // A normal uninterrupted world is the reference; no save bytes are modified.
        drop(s);copy_world(&p.join("world"),&p.join("reference"));
        let mut normal=Store::open(&p.join("reference"),RULES).unwrap();assert!(normal.execute(caller,request.clone()).unwrap().model.is_ok());
        let expected=normal.state_hash().unwrap();let expected_sequence=normal.committed_sequence();let expected_engine=normal.engine().unwrap().export_checkpoint().unwrap();drop(normal);
        fs::write(p.join("normal-reference.json"),serde_json::to_vec_pretty(&serde_json::json!({"hash":expected,"sequence":expected_sequence})).unwrap()).unwrap();
        s=Store::open(&p.join("world"),RULES).unwrap();
        let result=s.execute_with_observer(caller,request.clone(),|at|if at==point{Err(io::Error::other("retained normal write interruption"))}else{Ok(())});assert!(result.is_err());fs::write(p.join("failure.txt"),format!("{result:?}")).unwrap();fs::write(p.join("request.json"),serde_json::to_vec_pretty(&request).unwrap()).unwrap();drop(s);
        let mut s=Store::open(&p.join("world"),RULES).unwrap();assert!(s.execute(caller,request.clone()).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),expected);assert_eq!(s.committed_sequence(),expected_sequence);assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(),expected_engine);replay(&mut s,request,caller);assert_eq!(counters(&s),if enter{(0,2)}else{(2,0)});assert_eq!(s.engine().unwrap().leases().iter().flatten().filter(|l|l.resource==701).count(),1);s.checkpoint().unwrap();drop(s);cold_check(&p,if enter{(0,2)}else{(2,0)});let s=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(counters(&s),if enter{(0,2)}else{(2,0)});
    }
}

fn cold_check(p:&Path,expected:(u32,u32)) { cold_check_alias(p,expected,1001); }
fn cold_check_alias(p:&Path,expected:(u32,u32),alias:u64) {
    let id=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();let config=p.join(format!("cold-{id}.json"));
    fs::write(&config,serde_json::to_vec(&serde_json::json!({"world":p.join("world"),"reserved":expected.0,"occupied":expected.1,"alias":alias})).unwrap()).unwrap();
    let out=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","cold_process_reads_original_timed_pool","--test-threads=1","--nocapture"]).env("MYCITY_TIMED_COLD_CONFIG",&config).output().unwrap();
    fs::write(p.join(format!("cold-{id}.stdout")),&out.stdout).unwrap();fs::write(p.join(format!("cold-{id}.stderr")),&out.stderr).unwrap();
    fs::write(p.join(format!("cold-{id}-result.json")),serde_json::to_vec(&serde_json::json!({"exit_code":out.status.code(),"config":config,"executable":std::env::current_exe().unwrap()})).unwrap()).unwrap();assert!(out.status.success(),"cold process: {}",String::from_utf8_lossy(&out.stderr));
}
#[test]
fn cold_process_reads_original_timed_pool() {
    let Some(config)=std::env::var_os("MYCITY_TIMED_COLD_CONFIG") else{return;};
    let input:serde_json::Value=serde_json::from_slice(&fs::read(config).unwrap()).unwrap();let s=Store::open(Path::new(input["world"].as_str().unwrap()),RULES).unwrap();
    assert_eq!(counters(&s),(input["reserved"].as_u64().unwrap() as u32,input["occupied"].as_u64().unwrap() as u32));
    assert!(s.engine().unwrap().time_capacity_projection(input["alias"].as_u64().unwrap_or(1001)).is_some());println!("cold original pool read {}",s.state_hash().unwrap());
}

#[test]
fn full_normal_disk_budget_still_allows_only_actual_cancel_or_exit_progress() {
    for actual in [false,true] {
        let p=case(if actual{"disk-exit"}else{"disk-cancel"});let mut s=create(&p);setup(&mut s,&p);let (_,t)=reserve(&mut s,&p,3,301,1001,0,10);
        if actual {execute(&mut s,&p,Caller::System,action(C::Enter{token:t.number,occurred_at:0}));}
        let used=s.capacity_status().unwrap().used_bytes;
        s.reduce_capacity_policy(mycity_storage::CapacityPolicy{total_bytes:used+2*1024*1024,emergency_bytes:2*1024*1024+used/2}).unwrap();assert!(s.capacity_status().unwrap().normal_blocked);
        let policy=env(&s,action(C::SetPolicy{alias:1001,expected_version:1,effective_capacity:2,open:false}));assert!(!s.engine().unwrap().can_advance_existing_responsibility(Caller::System,&policy));
        let request=env(&s,action(if actual{C::Exit{token:t.number,occurred_at:0}}else{C::Cancel{token:t.number}}));assert!(s.engine().unwrap().can_advance_existing_responsibility(Caller::System,&request));
        assert!(s.execute(Caller::System,request.clone()).unwrap().model.is_ok());assert_eq!(counters(&s),(0,0));let hash=s.state_hash().unwrap();let seq=s.committed_sequence();drop(s);cold_check(&p,(0,0));let mut s=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);replay(&mut s,request,Caller::System);
    }
}

#[test]
fn original_lease_limit_refuses_new_bookings_and_cancel_releases_only_one_token() {
    let p=case("lease-limit");let mut s=create(&p);setup(&mut s,&p);let mut first=None;
    for i in 0..64 {let (_,t)=reserve(&mut s,&p,3,1000+i,1001,i*10,i*10+10);first.get_or_insert(t);}
    assert_eq!(counters(&s),(2,0));assert_eq!(s.engine().unwrap().leases().iter().flatten().count(),64);
    reject(&mut s,&p,Caller::Account(3),action(C::Reserve{alias:1002,account:3,consumer:2000,consumer_version:1,start:640,end:650,units:2}));
    let first=first.unwrap();execute(&mut s,&p,Caller::Account(3),action(C::Cancel{token:first.number}));let (_,next)=reserve(&mut s,&p,3,2000,1002,640,650);assert!(next.number>64);assert_ne!(next,first);
    s.checkpoint().unwrap();drop(s);cold_check(&p,(2,0));let s=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(s.engine().unwrap().leases().iter().flatten().count(),64);
    fs::write(p.join("classification.json"),b"{\"scope\":\"finite lease-table admission limit, not population/M0/throughput acceptance\"}").unwrap();
}
#[test]
fn missing_clock_wrong_actor_late_entry_and_stale_policy_preserve_original_reservation() {
    let p=case("boundary-controls");let mut s=create(&p);execute(&mut s,&p,Caller::System,action(C::Register{registration:registration()}));
    reject(&mut s,&p,Caller::Account(3),action(C::Reserve{alias:1001,account:3,consumer:301,consumer_version:1,start:10,end:20,units:2}));execute(&mut s,&p,Caller::System,Action::Timeline{command:T::Enable});
    reject(&mut s,&p,Caller::Account(4),action(C::Reserve{alias:1001,account:3,consumer:301,consumer_version:1,start:10,end:20,units:2}));let (_,t)=reserve(&mut s,&p,3,301,1001,10,20);
    reject(&mut s,&p,Caller::Account(4),action(C::Cancel{token:t.number}));reject(&mut s,&p,Caller::System,action(C::Enter{token:t.number,occurred_at:0}));
    reject(&mut s,&p,Caller::System,action(C::SetPolicy{alias:1002,expected_version:99,effective_capacity:2,open:false}));
    advance(&mut s,&p,20);reject(&mut s,&p,Caller::System,action(C::Enter{token:t.number,occurred_at:20}));assert_eq!(counters(&s),(2,0));
    execute(&mut s,&p,Caller::Account(3),action(C::Cancel{token:t.number}));assert_eq!(counters(&s),(0,0));
}

fn copy_world(source:&Path,destination:&Path) {
    fs::create_dir(destination).unwrap();
    for item in fs::read_dir(source).unwrap(){let item=item.unwrap();let target=destination.join(item.file_name());if item.file_type().unwrap().is_dir(){copy_world(&item.path(),&target);}else{fs::copy(item.path(),&target).unwrap();assert_eq!(fs::read(item.path()).unwrap(),fs::read(target).unwrap());}}
}

#[path="../../../tests/fixtures/water-treatment-runtime/opening.rs"]mod mixed_v2;
fn create_mixed(p:&Path)->Store {
    let input=serde_json::to_vec(&mixed_v2::input()).unwrap();fs::write(p.join("opening.json"),&input).unwrap();let mut resources=mixed_v2::resources();resources.push(Resource::new(701,2).unwrap());
    Store::create(&p.join("world"),&input,resources,RULES).unwrap()
}
fn mixed_setup(s:&mut Store,p:&Path){execute(s,p,Caller::System,Action::Timeline{command:T::Enable});let mut r=registration();r.aliases=[Some(70011),Some(70012),None,None];execute(s,p,Caller::System,action(C::Register{registration:r}));}
#[test]
fn original_venue_and_v2_registration_coexist_with_adjacent_timed_obligations_and_cold_restore() {
    use mycity_core::venue_runtime::Command as V;
    let p=case("mixed-venue-v2");let mut s=create_mixed(&p);mixed_setup(&mut s,&p);
    let (_,a)=reserve(&mut s,&p,3,301,70011,0,10);let (_,b)=reserve(&mut s,&p,4,401,70012,10,20);assert_eq!(counters(&s),(2,0));
    execute(&mut s,&p,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});let vt=s.engine().unwrap().world().view().venue.unwrap().uses[0].token.number;
    execute(&mut s,&p,Caller::System,Action::Venue{use_id:1,command:V::Enter{event_id:50001,token_number:vt,occurred_at:0}});
    execute(&mut s,&p,Caller::System,action(C::Enter{token:a.number,occurred_at:0}));s.checkpoint().unwrap();drop(s);cold_check_alias(&p,(2,2),70011);let mut s=Store::open(&p.join("world"),RULES).unwrap();
    advance(&mut s,&p,10);reject(&mut s,&p,Caller::System,action(C::Enter{token:b.number,occurred_at:10}));execute(&mut s,&p,Caller::System,action(C::Exit{token:a.number,occurred_at:10}));
    execute(&mut s,&p,Caller::System,action(C::Enter{token:b.number,occurred_at:10}));execute(&mut s,&p,Caller::System,Action::Venue{use_id:1,command:V::SafeExit{event_id:50002,token_number:vt,occurred_at:10}});
    s.checkpoint().unwrap();drop(s);cold_check_alias(&p,(0,2),70011);let mut s=Store::open(&p.join("world"),RULES).unwrap();advance(&mut s,&p,25);execute(&mut s,&p,Caller::System,action(C::Exit{token:b.number,occurred_at:25}));assert_eq!(counters(&s),(0,0));
    assert!(!s.engine().unwrap().leases().iter().flatten().any(|l|l.resource==501||l.resource==701));
}
#[test]
fn legacy_venue_still_uses_original_full_lease_table_after_timed_view_filtering() {
    use mycity_core::venue_runtime::Command as V;
    let p=case("mixed-global-slot-limit");let mut s=create_mixed(&p);mixed_setup(&mut s,&p);let mut first=None;
    for i in 0..64{let (_,t)=reserve(&mut s,&p,3,1000+i,70011,i*10,i*10+10);first.get_or_insert(t);}
    reject(&mut s,&p,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});assert_eq!(s.engine().unwrap().leases().iter().flatten().count(),64);assert!(s.engine().unwrap().world().view().venue.unwrap().uses.is_empty());
    execute(&mut s,&p,Caller::Account(3),action(C::Cancel{token:first.unwrap().number}));execute(&mut s,&p,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});
    assert_eq!(s.engine().unwrap().leases().iter().flatten().count(),64);s.checkpoint().unwrap();drop(s);cold_check_alias(&p,(2,0),70011);
}
