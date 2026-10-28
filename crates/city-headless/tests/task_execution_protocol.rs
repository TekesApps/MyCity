//! Versioned bound requests use real original routes, receipts and cold worlds.
use mycity_core::{timeline::Command as T, transaction::{Action, Caller, Envelope, Resource}};
use mycity_headless::{sim_client::Client, sim_host::{Command, Event, BoundTaskRequest, TaskExecutionIntent, TaskExecutionLocation}};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{fs, io::Write, path::{Path,PathBuf}, sync::atomic::{AtomicU64,Ordering}, time::Duration};
static NEXT:AtomicU64=AtomicU64::new(0);
#[path="../../../tests/fixtures/continuous-supply/opening.rs"] mod supply_opening;
fn case(label:&str)->PathBuf {
    let root=PathBuf::from(std::env::var_os("MYCITY_TASK_REPORT_ROOT").unwrap()).canonicalize().unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("{label}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();p
}
fn rules()->String {format!("sim-w3:{}",env!("MYCITY_SOURCE_SHA256"))}
fn exe()->PathBuf {PathBuf::from(std::env::var_os("MYCITY_TASK_REPORT_EXE").unwrap())}
fn event(c:&mut Client,p:&Path)->Event {
    let e=c.next(Duration::from_secs(15)).unwrap();
    let mut f=fs::OpenOptions::new().create(true).append(true).open(p.join("events.jsonl")).unwrap();
    writeln!(f,"{}",serde_json::to_string(&e).unwrap()).unwrap();e
}
fn call(c:&mut Client,p:&Path,id:u64,command:Command)->Value {
    c.submit(id,command).unwrap();let e=event(c,p);assert_eq!(e.kind,"result");assert!(e.payload["Err"].is_null(),"{e:?}");e.payload["Ok"].clone()
}
fn copy(source:&Path,target:&Path) {
    fs::create_dir(target).unwrap();for entry in fs::read_dir(source).unwrap(){let e=entry.unwrap();let dest=target.join(e.file_name());
        if e.file_type().unwrap().is_dir(){copy(&e.path(),&dest)}else{fs::copy(e.path(),&dest).unwrap();assert_eq!(fs::read(e.path()).unwrap(),fs::read(dest).unwrap());}}
}
fn apply(s:&mut Store,action:Action) {
    let(scope,revision)=s.engine().unwrap().world().scope_revision();
    let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]};
    assert!(s.execute(Caller::System,e).unwrap().model.is_ok());
}
fn prepare(p:&Path,zero:bool)->(PathBuf,Store) {
    let world=p.join("world");let resources=if zero {vec![]}else{[(101,1),(102,1),(103,1),(104,1),(201,2000),(202,2000)].into_iter().map(|(id,n)|Resource::new(id,n).unwrap()).collect()};
    let opening:&[u8]=if zero{include_bytes!("../../../fixtures/m0-economy-opening.json")}else{include_bytes!("../../../fixtures/m0-exchange-opening.json")};
    fs::write(p.join("opening.json"),opening).unwrap();
    let mut s=Store::create_spatial(&world,opening,resources,&rules(),include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});apply(&mut s,Action::Timeline{command:T::Advance{target:if zero{3600}else{30},limit:1}});
    s.checkpoint().unwrap();drop(s);copy(&world,&p.join("reference"));(world,Store::open(&p.join("reference"),&rules()).unwrap())
}
fn start(world:&Path,p:&Path)->Client {
    let mut c=Client::new();c.start_probe(&exe(),world,false).unwrap();let ready=event(&mut c,p);assert_eq!(ready.kind,"ready");
    assert_eq!(ready.payload["runtime"]["task_execution_observation"]["schema"],"mycity-task-execution-observation-v1");
    assert!(ready.payload["runtime"]["task_execution_observation"]["sim"].is_null());c
}
fn lease_offer()->mycity_core::housing::GenesisLeaseOffer {
    use mycity_core::{housing::{GenesisLeaseOffer,LeaseTerms,lease_terms_sha256},units::{Money,GameTime},timeline::Cycle};
    let t=LeaseTerms{lease_id:901,clause_version:1,contract_source_id:9101,landlord_authorization_id:9201,tenant_consent_id:9301,rent_per_cycle:Money::new(1000).unwrap(),cycle:Cycle::Monthly{anchor:1},deposit:Money::ZERO,escrow_account:3,starts_at:GameTime::ZERO};
    GenesisLeaseOffer{offer_id:901,world_id:1,branch_id:1,unit_id:71,building_id:70,owner_account:1,tenant_account:2,household_id:7,valid_from:GameTime::ZERO,valid_until:GameTime::new((400*mycity_core::service_needs::DAY)as i64).unwrap(),asset_source_id:7002,authority_source_id:8001,contract_source_id:t.contract_source_id,landlord_authorization_id:t.landlord_authorization_id,tenant_consent_id:t.tenant_consent_id,lease_id:t.lease_id,clause_version:t.clause_version,rent_per_cycle:t.rent_per_cycle,cycle:t.cycle,deposit:t.deposit,escrow_account:t.escrow_account,starts_at:t.starts_at,terms_sha256:lease_terms_sha256(t).unwrap()}
}
fn joint_input()->serde_json::Value {
    use serde_json::json;
    let mut v:serde_json::Value=serde_json::from_slice(include_bytes!("../../city-storage/tests/fixtures/education-store/free-offline-v1.json")).unwrap();
    v.as_object_mut().unwrap().remove("education");v.as_object_mut().unwrap().remove("venue");
    v["bootstrap"]["seed"]=107.into();v["housing"]["city_id"]=1.into();v["housing"]["lease_offers"]=json!([lease_offer()]);
    v["service_needs"]=json!({"version":1,"profile":"LocalDevelopmentNaturalNeedsV1","scope":{"world":1,"branch":1},"seed":107,"city_id":1,"households":[{"household_id":7,"source_id":7001,"generation":1,"people":10}]});
    let supply=supply_opening::input();
    for source in supply["sources"].as_array().unwrap().iter().filter(|x|x["id"].as_u64().unwrap()>=10){v["sources"].as_array_mut().unwrap().push(source.clone());}
    for stock in supply["stocks"].as_array().unwrap(){let mut stock=stock.clone();stock["owner"]=3.into();v["stocks"].as_array_mut().unwrap().push(stock);}
    v["continuous_supply"]=supply["continuous_supply"].clone();v["continuous_supply"]["owner"]=3.into();v["continuous_supply"]["start_supplier"]=4.into();v["continuous_supply"]["city_id"]=1.into();v["continuous_supply"]["source_id"]=json!(0x534e000000000001u64);v
}


fn describe(c:&mut Client,p:&Path,id:u64)->Value {call(c,p,id,Command::DescribeTaskExecution)}
fn binding(d:&Value)->BoundTaskRequest {serde_json::from_value(d["request"].clone()).unwrap()}
fn reject(c:&mut Client,p:&Path,id:u64,b:BoundTaskRequest,expected:&str) {
    c.submit(id,Command::RunBoundTask{binding:b}).unwrap();let e=event(c,p);
    assert_eq!(e.kind,"result");assert!(e.payload["Err"].as_str().unwrap().contains(expected),"{e:?}");
}
fn run_bound_page(c:&mut Client,p:&Path,id:u64,d:&Value,reference:&mut Store,provider:&str)->Envelope {
    c.submit(id,Command::RunBoundTask{binding:binding(d)}).unwrap();
    let stamp=event(c,p);assert_eq!(stamp.kind,"bound_task_selected");assert_eq!(stamp.payload["recovery"],d["recovery"]);
    let selected=event(c,p);assert_eq!(selected.kind,format!("{provider}_continuation_selected"));
    assert_eq!(selected.payload["envelope"],d["recovery"]["envelope"]);
    let e:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();
    assert!(reference.execute(Caller::System,e.clone()).unwrap().model.is_ok());
    let result=event(c,p);assert_eq!(result.kind,"result");assert!(result.payload["Ok"].is_object(),"{result:?}");
    assert_eq!(result.payload["Ok"]["bound_request"],stamp.payload);
    assert_eq!(result.payload["Ok"]["execution_observation"]["sim"]["observation"]["context"]["source"]["provider"],provider);
    assert_eq!(result.payload["Ok"]["after_serial"],reference.engine().unwrap().timeline().unwrap().progress.serial);e
}
#[test]
fn bound_timeline_read_only_description_exact_reference_repeat_and_cold_recovery() {
    let p=case("bound-timeline");let(world,mut reference)=prepare(&p,false);let mut c=start(&world,&p);
    let before=call(&mut c,&p,1,Command::Status);let d=describe(&mut c,&p,2);assert_eq!(d,describe(&mut c,&p,3));
    let after=call(&mut c,&p,4,Command::Status);for k in ["sequence","state_sha256","business_sha256","timeline"]{assert_eq!(before[k],after[k]);}
    assert_eq!(d["selection"]["source"]["provider"],"timeline");assert_eq!(d["capabilities"]["scratch_peak"],"unknown_not_instrumented");
    let ticket=run_bound_page(&mut c,&p,5,&d,&mut reference,"timeline");reject(&mut c,&p,6,binding(&d),"task_binding_stale");
    call(&mut c,&p,7,Command::Checkpoint);c.stop().unwrap();
    let mut s=Store::open(&world,&rules()).unwrap();assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());
    let seq=s.committed_sequence();let hash=s.state_hash().unwrap();assert!(s.receipt(ticket.operation_id).unwrap().unwrap().outcome.is_ok());
    assert!(s.execute(Caller::System,ticket).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);drop(s);
    let mut c=start(&world,&p);reject(&mut c,&p,8,binding(&d),"task_binding_stale");c.stop().unwrap();
}
#[test]
fn zero_width_none_is_legal_and_unsupported_requests_do_not_consume_the_original_page() {
    let p=case("bound-zero");let(world,mut reference)=prepare(&p,true);let mut c=start(&world,&p);let d=describe(&mut c,&p,1);
    assert!(d["selection"]["task"].is_null());assert_eq!(d["selection"]["progress"]["at"],0);
    let original=binding(&d);let mut b=original.clone();b.capability_version+=1;reject(&mut c,&p,2,b,"capability_version_unsupported");
    let mut b=original.clone();b.execution=TaskExecutionLocation::Gpu;reject(&mut c,&p,3,b,"gpu_unsupported");
    let mut b=original.clone();b.strategy_version+=1;reject(&mut c,&p,4,b,"strategy_mismatch");
    let mut b=original.clone();b.binding_sha256[0]^=1;reject(&mut c,&p,5,b,"binding_stale");
    let mut b=original.clone();b.intent=TaskExecutionIntent::RequestRefinement{strategy:"detail".into(),strategy_version:1};reject(&mut c,&p,6,b,"refinement_unsupported");
    let mut b=original.clone();b.intent=TaskExecutionIntent::RequestResources{cpu_nanoseconds:Some(1),additional_bytes:Some(1)};reject(&mut c,&p,7,b,"resource_change_unresolved");
    let mut b=original.clone();b.execution=TaskExecutionLocation::AirWorker;reject(&mut c,&p,8,b,"execution_or_strategy_mismatch");
    let mut b=original.clone();b.intent=TaskExecutionIntent::ExecuteExactPages{plan_version:mycity_headless::air_turns::PLAN_VERSION,
        quota:mycity_headless::air_turns::Quota{source_limit:1,cell_limit:17,pages:1}};
    reject(&mut c,&p,9,b,"task_exact_pages_requires_air");
    assert_eq!(d,describe(&mut c,&p,10));let e=run_bound_page(&mut c,&p,11,&d,&mut reference,"timeline");c.stop().unwrap();
    let mut s=Store::open(&world,&rules()).unwrap();assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());assert!(s.receipt(e.operation_id).unwrap().is_some());
    let mut old=d["request"].clone();old["grant_bytes"]=json!(2*1024*1024);assert!(serde_json::from_value::<BoundTaskRequest>(old).is_err());
}
#[test]
fn unrelated_real_transaction_invalidates_binding_without_losing_the_accepted_target() {
    use mycity_core::units::Money;
    let p=case("bound-sequence");let(world,reference)=prepare(&p,false);let(scope,revision)=reference.engine().unwrap().world().scope_revision();
    let from=reference.engine().unwrap().world().account_ref(1).unwrap();let to=reference.engine().unwrap().world().account_ref(2).unwrap();
    let envelope=Envelope{world:scope.world(),branch:scope.branch(),operation_id:1000,expected_revision:revision,actions:vec![Action::Transfer{from,to,amount:Money::new(1).unwrap()}]};drop(reference);
    let mut c=start(&world,&p);let d=describe(&mut c,&p,1);call(&mut c,&p,2,Command::Execute{envelope});
    reject(&mut c,&p,3,binding(&d),"binding_stale");let changed=describe(&mut c,&p,4);assert_ne!(d["request"]["binding_sha256"],changed["request"]["binding_sha256"]);
    assert_eq!(d["selection"]["progress"],changed["selection"]["progress"]);c.stop().unwrap();
    assert_eq!(Store::open(&world,&rules()).unwrap().engine().unwrap().timeline().unwrap().progress.target,Some(30));
}
#[test]
fn unconsumed_terminal_recovers_the_disclosed_original_operation_without_new_describe_retry() {
    let p=case("bound-lost-reply");let(world,mut reference)=prepare(&p,false);let mut c=start(&world,&p);let d=describe(&mut c,&p,1);
    let ticket:Envelope=serde_json::from_value(d["recovery"]["envelope"].clone()).unwrap();
    c.submit(2,Command::RunBoundTask{binding:binding(&d)}).unwrap();assert!(c.submit(3,Command::Status).is_err());
    assert_eq!(event(&mut c,&p).kind,"bound_task_selected");assert_eq!(event(&mut c,&p).kind,"timeline_continuation_selected");
    c.stop().unwrap(); // owns and waits for Sim; deliberately does not consume terminal
    assert!(reference.execute(Caller::System,ticket.clone()).unwrap().model.is_ok());
    let mut s=Store::open(&world,&rules()).unwrap();assert!(s.receipt(ticket.operation_id).unwrap().unwrap().outcome.is_ok());assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());
    let seq=s.committed_sequence();let hash=s.state_hash().unwrap();assert!(s.execute(Caller::System,ticket).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);
    s.checkpoint().unwrap();drop(s);assert_eq!(Store::open(&world,&rules()).unwrap().state_hash().unwrap(),hash);
}
#[test]
fn real_supply_uses_bound_original_source_and_one_original_advance() {
    let p=case("bound-supply");let world=p.join("world");let raw=serde_json::to_vec(&supply_opening::input()).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    let mut s=Store::create_spatial(&world,&raw,supply_opening::resources(),&rules(),include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});
    let(scope,revision)=s.engine().unwrap().world().scope_revision();let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![Action::ContinuousSupply{command:mycity_core::continuous_supply::Command::Start{run_id:1,pumps:3}}]};
    assert!(s.execute(Caller::Account(1),e).unwrap().model.is_ok());
    let mut pending=false;for _ in 0..128 {apply(&mut s,Action::Timeline{command:T::Advance{target:30,limit:1}});if s.engine().unwrap().timeline().unwrap().progress.phase==Some(mycity_core::schedule::Phase::P5){pending=true;break;}}assert!(pending);
    s.checkpoint().unwrap();drop(s);copy(&world,&p.join("reference"));let mut reference=Store::open(&p.join("reference"),&rules()).unwrap();
    let mut c=start(&world,&p);let d=describe(&mut c,&p,1);assert_eq!(d["selection"]["source"]["provider"],"supply");run_bound_page(&mut c,&p,2,&d,&mut reference,"supply");c.stop().unwrap();
    assert_eq!(Store::open(&world,&rules()).unwrap().state_hash().unwrap(),reference.state_hash().unwrap());
}
#[test]
fn real_service_and_supply_share_one_original_p5_page_not_two_grants() {
    use mycity_core::{continuous_supply::Command as S,housing::genesis_lease_offer_sha256,schedule::Phase};
    let p=case("bound-service-joint");let world=p.join("world");let raw=serde_json::to_vec(&joint_input()).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    let mut spatial:Value=serde_json::from_slice(include_bytes!("../../../fixtures/m0-exchange-spatial.json")).unwrap();spatial["bootstrap"]["seed"]=107.into();let spatial=serde_json::to_vec(&spatial).unwrap();fs::write(p.join("spatial.json"),&spatial).unwrap();
    let mut s=Store::create_spatial_v2(&world,&raw,supply_opening::resources(),&rules(),&spatial,include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});
    for(caller,action)in [(Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:genesis_lease_offer_sha256(lease_offer())}),(Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}})] {
        let(scope,revision)=s.engine().unwrap().world().scope_revision();let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]};assert!(s.execute(caller,e).unwrap().model.is_ok());
    }
    let mut reached=false;for _ in 0..4096 {apply(&mut s,Action::Timeline{command:T::Advance{target:3600,limit:1}});let t=s.engine().unwrap().timeline().unwrap().progress;if t.phase==Some(Phase::P5)&&t.at==3600{reached=true;break;}}assert!(reached);
    let dir=mycity_headless::task_registry_directory::collect(&s,false).unwrap();assert_eq!(dir.entries()[1].key,dir.entries()[2].key);let seq=s.committed_sequence();s.checkpoint().unwrap();drop(s);
    copy(&world,&p.join("reference"));let mut reference=Store::open(&p.join("reference"),&rules()).unwrap();let mut c=start(&world,&p);let d=describe(&mut c,&p,1);assert_eq!(d["selection"]["source"]["provider"],"service");
    run_bound_page(&mut c,&p,2,&d,&mut reference,"service");c.stop().unwrap();let s=Store::open(&world,&rules()).unwrap();assert_eq!(s.committed_sequence(),seq+1);assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());
}
#[test]
fn real_air_binding_keeps_zero_source_revision_original_job_receipt_and_normal_role_grants() {
    use mycity_core::background::Command as B;
    let p=case("bound-air-pool");let world=p.join("world");let resources=[(101,1),(102,1),(103,1),(104,1),(201,2000),(202,2000)].into_iter().map(|(id,n)|Resource::new(id,n).unwrap()).collect();
    let mut s=Store::create_spatial(&world,include_bytes!("../../../fixtures/m0-exchange-opening.json"),resources,&rules(),include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});apply(&mut s,Action::Background{command:B::Enable});let mut pending=false;
    for _ in 0..96 {apply(&mut s,Action::Timeline{command:T::Advance{target:30,limit:256}});if s.engine().unwrap().timeline().unwrap().progress.phase==Some(mycity_core::schedule::Phase::P4){pending=true;break;}}assert!(pending);s.checkpoint().unwrap();drop(s);
    let root=PathBuf::from(std::env::var_os("MYCITY_TASK_PROTOCOL_POOL_ROOT").unwrap()).canonicalize().unwrap();assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));let root=root.join(format!("p{}",std::process::id()));
    let mut pool=mycity_headless::normal_pool::Process::spawn(&exe(),&root,Some(&p.join("pool-heaps.jsonl"))).unwrap();
    let mut c=Client::new();c.start_in_pool(&exe(),&world,false,false,&root).unwrap();assert_eq!(event(&mut c,&p).kind,"ready");let d=describe(&mut c,&p,1);
    assert_eq!(d["selection"]["source"]["provider"],"air");assert_eq!(d["selection"]["source"]["binding"]["field_version"],0);
    assert_eq!(d["capabilities"]["candidate_transport_limit_bytes"],8*1024*1024);assert_eq!(d["capabilities"]["current_air_business_asset_limit_bytes"],2*1024*1024);assert_eq!(d["capabilities"]["control_frame_limit_bytes"],64*1024);assert_eq!(d["capabilities"]["limits_are_memory_grants"],false);
    let before=mycity_headless::normal_pool::status(&root).unwrap();assert_eq!(d,describe(&mut c,&p,2));let after=mycity_headless::normal_pool::status(&root).unwrap();assert_eq!(before.workspace_grants,after.workspace_grants);assert_eq!(before.workspace_returns,after.workspace_returns);
    c.submit(3,Command::RunBoundTask{binding:binding(&d)}).unwrap();assert_eq!(event(&mut c,&p).kind,"bound_task_selected");let mut result=None;
    for _ in 0..512 {let e=event(&mut c,&p);if e.kind=="result"{result=Some(e.payload);break;}}
    let result=result.unwrap();assert!(result["Ok"].is_object(),"{result}");assert_eq!(result["Ok"]["record"]["operation_id"],d["recovery"]["accept_operation"]);assert_eq!(result["Ok"]["bound_request"]["recovery"],d["recovery"]);
    reject(&mut c,&p,4,binding(&d),"task_");
    let status=call(&mut c,&p,5,Command::Status);fs::write(p.join("role-status.json"),serde_json::to_vec_pretty(&status).unwrap()).unwrap();
    let a=&status["runtime"]["allocator_boundary"]["snapshot"];assert_eq!(a["activation"]["grant_bytes"],59*1024*1024);assert_eq!(a["allocation_failures"],0);assert!(a["arena_used_bytes"].as_u64().unwrap()<=a["arena_usable_bytes"].as_u64().unwrap());
    call(&mut c,&p,6,Command::Checkpoint);call(&mut c,&p,7,Command::Stop);c.stop().unwrap();let balance=mycity_headless::normal_pool::status(&root).unwrap();assert_eq!(balance.workspace_grants,balance.workspace_returns);assert_eq!(balance.available_touched_bytes,mycity_headless::normal_pool::BYTES);fs::write(p.join("pool-return.json"),serde_json::to_vec_pretty(&balance).unwrap()).unwrap();pool.finish().unwrap();
    let mut s=Store::open(&world,&rules()).unwrap();let b=s.engine().unwrap().background().unwrap();assert_eq!(b.time(),30);assert!(b.pending().is_none());assert!(s.receipt(d["recovery"]["accept_operation"].as_u64().unwrap()).unwrap().unwrap().outcome.is_ok());
}
#[test]
fn failed_bound_send_keeps_original_client_owned_until_actual_retirement() {
    let p=case("bound-send-failure");let(world,reference)=prepare(&p,false);let before=reference.state_hash().unwrap();drop(reference);
    let mut c=start(&world,&p);let d=describe(&mut c,&p,1);let pid=c.pid();
    // Only the exact child created above belongs to this test.
    assert!(std::process::Command::new("kill").args(["-KILL",&pid.to_string()]).status().unwrap().success());
    let until=std::time::Instant::now()+Duration::from_secs(5);
    loop {let out=std::process::Command::new("ps").args(["-p",&pid.to_string(),"-o","stat="]).output().unwrap();if !out.status.success()||String::from_utf8_lossy(&out.stdout).trim().starts_with('Z'){break;}assert!(std::time::Instant::now()<until);std::thread::sleep(Duration::from_millis(10));}
    assert!(c.submit(2,Command::RunBoundTask{binding:binding(&d)}).is_err());assert!(c.submit(3,Command::Status).is_err());assert!(c.submit(4,Command::RunBoundTask{binding:binding(&d)}).is_err());assert!(!c.is_empty());
    c.stop().unwrap();assert!(c.is_empty());assert_eq!(Store::open(&world,&rules()).unwrap().state_hash().unwrap(),before);
}
#[test]
fn bound_selection_does_not_replace_original_operation_collision_admission() {
    let p=case("bound-original-collision");let(world,reference)=prepare(&p,false);drop(reference);
    let mut s=Store::open(&world,&rules()).unwrap();let(scope,revision)=s.engine().unwrap().world().scope_revision();
    let collision=Envelope{world:scope.world(),branch:scope.branch(),operation_id:0xc000000000000000|(s.committed_sequence()+2),expected_revision:revision,actions:vec![Action::Timeline{command:T::Advance{target:30,limit:1}}]};
    assert!(s.execute(Caller::System,collision).unwrap().model.is_ok());let hash=s.state_hash().unwrap();let seq=s.committed_sequence();drop(s);
    let mut c=start(&world,&p);let d=describe(&mut c,&p,1);assert_eq!(d["selection_is_business_admission"],false);
    c.submit(2,Command::RunBoundTask{binding:binding(&d)}).unwrap();assert_eq!(event(&mut c,&p).kind,"bound_task_selected");let refused=event(&mut c,&p);assert_eq!(refused.kind,"result");assert!(refused.payload["Err"].as_str().unwrap().starts_with("timeline_due_namespace_collision:"));c.stop().unwrap();
    let s=Store::open(&world,&rules()).unwrap();assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);
}
