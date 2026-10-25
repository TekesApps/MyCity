//! Real original Timeline/Store/Sim directory gates; retain complete game worlds.
use mycity_core::{transaction::{Action,Caller,Envelope,Resource},timeline::Command as T};
use mycity_headless::{task_registry_directory::{self as directory,Route,Action as RouteAction,Error},sim_client::Client,sim_host::{Command,Event}};
use mycity_storage::Store;
use std::{fs::{self,OpenOptions},io::Write,path::{Path,PathBuf},time::{Duration,SystemTime,UNIX_EPOCH}};
#[path="../../../tests/fixtures/continuous-supply/opening.rs"]mod supply_opening;

fn case(name:&str)->PathBuf {
    let root=fs::canonicalize(std::env::var_os("MYCITY_DIRECTORY_TEST_ROOT").expect("explicit retained SSD root")).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("{name}-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&p).unwrap();println!("RETAINED_DIRECTORY_WORLD={}",p.display());p
}
fn rules()->String {format!("sim-w3:{}",env!("MYCITY_SOURCE_SHA256"))}
fn create(path:&Path)->Store {
    let resources=[(101,1),(102,1),(103,1),(104,1),(201,2000),(202,2000)].into_iter().map(|(id,n)|Resource::new(id,n).unwrap()).collect();
    Store::create_spatial(path,include_bytes!("../../../fixtures/m0-exchange-opening.json"),resources,&rules(),
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap()
}
fn apply(s:&mut Store,action:Action)->Envelope {
    apply_as(s,Caller::System,action)
}
fn apply_as(s:&mut Store,caller:Caller,action:Action)->Envelope {
    let e=Envelope{world:1,branch:1,operation_id:s.committed_sequence()+1,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![action]};
    assert!(s.execute(caller,e.clone()).unwrap().model.is_ok());e
}
fn accepted(s:&mut Store) {
    apply(s,Action::Timeline{command:T::Enable});
    apply(s,Action::Timeline{command:T::Advance{target:30,limit:1}});
    assert_eq!(s.engine().unwrap().timeline().unwrap().progress.target,Some(30));
    assert!(s.engine().unwrap().background().is_none());
    assert!(s.engine().unwrap().service_needs_due_state().unwrap().is_none());
    assert!(s.engine().unwrap().continuous_supply_due_state().unwrap().is_none());
}
fn event(c:&mut Client,p:&Path)->Event {
    let e=c.next(Duration::from_secs(15)).unwrap();
    let bytes=serde_json::to_vec(&e).unwrap();assert!(bytes.len()<256*1024);
    let path=p.join("events.jsonl");let mut f=OpenOptions::new().append(true).create(true).open(path).unwrap();f.write_all(&bytes).unwrap();f.write_all(b"\n").unwrap();
    assert!(f.metadata().unwrap().len()<8*1024*1024);e
}
fn executable()->PathBuf {fs::canonicalize(std::env::var_os("MYCITY_DIRECTORY_HEADLESS_EXE").expect("frozen headless")).unwrap()}
fn copy_world(source:&Path,target:&Path) {
    fs::create_dir(target).unwrap();
    for f in fs::read_dir(source).unwrap(){let f=f.unwrap();let next=target.join(f.file_name());
        if f.file_type().unwrap().is_dir(){copy_world(&f.path(),&next);}else{fs::copy(f.path(),&next).unwrap();assert_eq!(fs::read(f.path()).unwrap(),fs::read(next).unwrap());}}
}
fn drain(s:&mut Store,target:u64,limit:u16) {
    for _ in 0..4096 {apply(s,Action::Timeline{command:T::Advance{target,limit}});if s.engine().unwrap().timeline().unwrap().progress.target.is_none(){return;}}
    panic!("finite original timeline continuation");
}
fn pending_air(s:&mut Store,target:u64) {
    for _ in 0..96 {apply(s,Action::Timeline{command:T::Advance{target,limit:256}});
        if s.engine().unwrap().timeline().unwrap().progress.phase==Some(mycity_core::schedule::Phase::P4){assert!(s.engine().unwrap().background().unwrap().pending().is_some());return;}}
    panic!("original P4 pending job");
}

#[test]
fn supply_schema_content_and_unrelated_sequence_are_distinct_and_cold_rebuild_is_exact(){
    use mycity_core::continuous_supply::Command as S;
    use directory::SourceBinding;
    let p=case("source-binding");let raw=serde_json::to_vec(&supply_opening::input()).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    let mut s=Store::create_spatial(&p.join("world"),&raw,supply_opening::resources(),&rules(),include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});let registered=directory::collect(&s,false).unwrap();
    apply_as(&mut s,Caller::Account(1),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}});let running=directory::collect(&s,false).unwrap();
    assert_eq!(registered.entries()[2].registry_version,Some(1));assert_eq!(running.entries()[2].registry_version,Some(1));
    let get=|d:&directory::Directory|match &d.entries()[2].binding{Some(SourceBinding::Supply(v))=>*v,_=>panic!("original supply binding")};
    let a=get(&registered);let b=get(&running);assert_eq!(a.registration,b.registration);assert_ne!(a.fingerprint,b.fingerprint);assert_eq!(b.status,mycity_core::continuous_genset::Status::Running);
    let from=s.engine().unwrap().world().account_ref(1).unwrap();let to=s.engine().unwrap().world().account_ref(2).unwrap();
    apply_as(&mut s,Caller::Account(1),Action::Transfer{from,to,amount:mycity_core::units::Money::new(1).unwrap()});let unrelated=directory::collect(&s,false).unwrap();
    assert_eq!(running.entries()[2],unrelated.entries()[2]);assert_eq!(running.validate(&s,false),Err(Error::Stale));
    apply_as(&mut s,Caller::Account(1),Action::ContinuousSupply{command:S::Stop});let stopped=directory::collect(&s,false).unwrap();assert_ne!(get(&stopped).fingerprint,b.fingerprint);assert_eq!(get(&stopped).registration,b.registration);
    assert!(stopped.entries().iter().all(|x|x.cache_observation_age_seconds.is_none()));
    fs::write(p.join("before-after-directory.json"),serde_json::to_vec_pretty(&serde_json::json!([registered.report().unwrap(),running.report().unwrap(),unrelated.report().unwrap(),stopped.report().unwrap()])).unwrap()).unwrap();
    let hash=s.state_hash().unwrap();s.checkpoint().unwrap();drop(s);let s=Store::open(&p.join("world"),&rules()).unwrap();assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(directory::collect(&s,false).unwrap(),stopped);
}

#[test]
fn air_real_field_versions_advance_while_task_schema_stays_stable_and_old_point_is_not_cache(){
    use mycity_core::background::Command as B;
    let p=case("air-versions");let world=p.join("world");let mut s=create(&world);
    apply(&mut s,Action::Timeline{command:T::Enable});apply(&mut s,Action::Background{command:B::Enable});
    let exe=executable();let mut observations=Vec::new();
    for (expected_version,target) in [(0,30),(1,60)] {
        pending_air(&mut s,target);let d=directory::collect(&s,false).unwrap();let row=&d.entries()[0];
        assert!(matches!(d.route(),Route::Selected(RouteAction::Air(_))));assert_eq!(row.registry_version,Some(expected_version+1));assert_eq!(row.required_registry_version,Some(expected_version+1));
        let key=row.key.unwrap();assert_eq!(key.source_version,1);assert_eq!((key.window.start,key.window.end),(target-30,target));assert_eq!(row.cache_observation_age_seconds,None);assert!(row.registry_wake);
        if target==30 {assert_eq!(row.coverage,directory::Coverage::Unknown);}else{assert_eq!(row.coverage,directory::Coverage::Point(30));}
        observations.push(d.report().unwrap());drop(s);
        let mut c=Client::new();c.start_probe(&exe,&world,false).unwrap();assert_eq!(event(&mut c,&p).kind,"ready");c.submit(target,Command::RunDue).unwrap();
        let mut done=false;for _ in 0..512 {let e=event(&mut c,&p);if e.kind=="result"{assert!(e.payload["Ok"].is_object(),"{e:?}");assert_eq!(e.payload["Ok"]["record"]["operation_id"],0xe000000000000000|(target/30));done=true;break;}}assert!(done);c.stop().unwrap();
        s=Store::open(&world,&rules()).unwrap();let b=s.engine().unwrap().background().unwrap();assert_eq!(b.version(),expected_version+1);assert_eq!(u64::from(b.time()),target);assert!(b.pending().is_none());
        drain(&mut s,target,256);
    }
    fs::write(p.join("actual-version-observations.json"),serde_json::to_vec_pretty(&observations).unwrap()).unwrap();
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

#[test]
fn same_store_same_clock_same_key_providers_remain_separate_but_original_p5_page_commits_once(){
    run_joint(false)
}
#[test]
fn same_key_joint_p5_in_original_normal_role_observes_allocation_and_returns_exact_grant(){
    run_joint(true)
}
fn run_joint(pooled:bool){
    use mycity_core::{continuous_supply::Command as S,housing::genesis_lease_offer_sha256,schedule::Phase};
    let p=case("same-key-joint");let world=p.join("world");let raw=serde_json::to_vec(&joint_input()).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    let mut spatial:serde_json::Value=serde_json::from_slice(include_bytes!("../../../fixtures/m0-exchange-spatial.json")).unwrap();spatial["bootstrap"]["seed"]=107.into();let spatial=serde_json::to_vec(&spatial).unwrap();fs::write(p.join("spatial-opening.json"),&spatial).unwrap();
    let mut s=Store::create_spatial_v2(&world,&raw,supply_opening::resources(),&rules(),&spatial,include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});let before=s.engine().unwrap().service_needs_due_state().unwrap().unwrap();
    apply_as(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:genesis_lease_offer_sha256(lease_offer())});let entered=s.engine().unwrap().service_needs_due_state().unwrap().unwrap();
    assert_eq!((entered.version,entered.covered_through,entered.next_due),(1,0,Some(28800)));assert_eq!(before.registration,entered.registration);assert_ne!(before.fingerprint,entered.fingerprint);
    assert!(matches!(directory::collect(&s,false).unwrap().route(),Route::NoWork)); // no future target is manufactured
    apply_as(&mut s,Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}});
    let mut reached=false;for _ in 0..4096 {apply(&mut s,Action::Timeline{command:T::Advance{target:3600,limit:1}});let t=s.engine().unwrap().timeline().unwrap().progress;if t.phase==Some(Phase::P5)&&t.at==3600{reached=true;break;}}assert!(reached);
    let selected=directory::collect(&s,false).unwrap();assert_eq!(selected.entries()[1].key,selected.entries()[2].key);assert!(selected.entries()[1].key.is_some());
    assert_eq!(selected.entries()[1].clock,selected.entries()[2].clock);assert_ne!(selected.entries()[1].provider,selected.entries()[2].provider);
    assert!(selected.entries()[1].registry_wake&&selected.entries()[2].registry_wake);assert!(matches!(selected.route(),Route::Selected(RouteAction::Service(_))));
    assert_eq!(selected.entries()[1].coverage_lag_game_seconds,Some(3600));assert_eq!(selected.entries()[2].coverage_lag_game_seconds,Some(3600));assert!(selected.entries().iter().all(|x|x.cache_observation_age_seconds.is_none()));
    fs::write(p.join("same-key-before.json"),serde_json::to_vec_pretty(&selected.report().unwrap()).unwrap()).unwrap();
    let mut seq=s.committed_sequence();let mut serial=s.engine().unwrap().timeline().unwrap().progress.serial;drop(s);copy_world(&world,&p.join("reference"));let mut reference=Store::open(&p.join("reference"),&rules()).unwrap();
    let pool_root=if pooled {let root=fs::canonicalize(std::env::var_os("MYCITY_DIRECTORY_POOL_ROOT").expect("explicit short retained SSD pool root")).unwrap();assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));Some(root.join(format!("p{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())))}else{None};
    let mut pool=pool_root.as_ref().map(|root|mycity_headless::normal_pool::Process::spawn(&executable(),root,Some(&p.join("pool-heaps.jsonl"))).unwrap());
    let mut c=Client::new();if let Some(root)=&pool_root {c.start_in_pool(&executable(),&world,false,false,root).unwrap();}else{c.start_probe(&executable(),&world,false).unwrap();}
    let ready=event(&mut c,&p);assert_eq!(ready.kind,"ready");let mut memory=vec![serde_json::to_value(&ready).unwrap()];let mut complete=false;
    for id in 1..65 {
        c.submit(id,Command::RunDue).unwrap();let selected=event(&mut c,&p);assert!(matches!(selected.kind.as_str(),"service_continuation_selected"|"supply_continuation_selected"|"timeline_continuation_selected"));
        let e:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();assert_eq!(e.actions,vec![Action::Timeline{command:T::Advance{target:3600,limit:1}}]);assert!(reference.execute(Caller::System,e).unwrap().model.is_ok());
        let result=event(&mut c,&p);assert!(result.payload["Ok"].is_object(),"{result:?}");assert_eq!(result.payload["Ok"]["after_serial"],serial+1);serial+=1;seq+=1;
        complete=result.payload["Ok"]["timeline"]["target"].is_null();if complete{break;}
    }
    assert!(complete);
    if pooled {
        c.submit(1001,Command::Checkpoint).unwrap();assert_eq!(event(&mut c,&p).payload["Ok"]["checkpointed"],true);
        c.submit(1002,Command::Status).unwrap();let status=event(&mut c,&p);assert!(status.payload["Ok"].is_object());memory.push(serde_json::to_value(&status).unwrap());
        for e in &memory {let v=e["payload"].get("Ok").unwrap_or(&e["payload"]);let a=&v["runtime"]["allocator_boundary"]["snapshot"];assert_eq!(a["activation"]["grant_bytes"],59*1024*1024);assert_eq!(a["allocation_failures"],0);assert!(a["arena_used_bytes"].as_u64().unwrap()<=a["arena_usable_bytes"].as_u64().unwrap());}
        fs::write(p.join("joint-role-observations.json"),serde_json::to_vec_pretty(&memory).unwrap()).unwrap();
        c.submit(1003,Command::Stop).unwrap();assert_eq!(event(&mut c,&p).kind,"result");
    }
    c.stop().unwrap();
    if let Some(pool)=pool.as_mut(){let balance=mycity_headless::normal_pool::status(pool.root()).unwrap();assert_eq!(balance.available_touched_bytes,mycity_headless::normal_pool::BYTES);assert_eq!(balance.workspace_grants,balance.workspace_returns);assert_eq!(balance.workspace_grants,1);fs::write(p.join("joint-pool-return.json"),serde_json::to_vec_pretty(&balance).unwrap()).unwrap();pool.finish().unwrap();}
    let s=Store::open(&world,&rules()).unwrap();assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());
    assert_eq!(s.engine().unwrap().service_needs_due_state().unwrap().unwrap().covered_through,3600);
    let view=s.engine().unwrap().world().view().continuous_supply.unwrap();assert_eq!((view.cursor,view.raw_used_ml,view.produced_ml),(3600,72_000_000,[36_000_000,36_000_000]));
    fs::write(p.join("same-key-final.json"),serde_json::to_vec_pretty(&serde_json::json!({"seq":seq,"serial":serial,"actual_original_reference_equal":true,"supply":view})).unwrap()).unwrap();
}

#[test]
fn original_same_time_economy_zero_window_is_a_control_not_a_fabricated_positive_window(){
    let p=case("zero-window");let world=p.join("world");let mut s=Store::create_spatial(&world,include_bytes!("../../../fixtures/m0-economy-opening.json"),vec![],&rules(),include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});apply(&mut s,Action::Timeline{command:T::Advance{target:3600,limit:1}});
    let before=s.engine().unwrap().timeline().unwrap().progress;assert_eq!((before.last_complete,before.at),(0,0));let d=directory::collect(&s,false).unwrap();assert!(matches!(d.route(),Route::Selected(RouteAction::Timeline(_))));assert!(d.entries()[3].key.is_none());drop(s);
    let mut c=Client::new();c.start_probe(&executable(),&world,false).unwrap();event(&mut c,&p);c.submit(1,Command::RunDue).unwrap();let selected=event(&mut c,&p);assert_eq!(selected.kind,"timeline_continuation_selected");assert_eq!(selected.payload["zero_width"],true);
    let e:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();let result=event(&mut c,&p);assert!(result.payload["Ok"].is_object(),"{result:?}");assert_eq!(result.payload["Ok"]["task_directory"]["providers"][3]["task"],serde_json::Value::Null);c.stop().unwrap();
    let mut s=Store::open(&world,&rules()).unwrap();let after=s.engine().unwrap().timeline().unwrap().progress;assert_eq!((after.last_complete,after.at),(0,0));assert_eq!(after.serial,before.serial+1);assert_ne!((after.phase,after.lane,after.cursor),(before.phase,before.lane,before.cursor));
    let seq=s.committed_sequence();let hash=s.state_hash().unwrap();assert!(s.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);
}

#[test]
fn original_timeline_c_collision_and_lost_terminal_preserve_the_same_responsibility(){
    let p=case("timeline-c-recovery");let world=p.join("world");let mut s=create(&world);accepted(&mut s);
    let collision=Envelope{world:1,branch:1,operation_id:0xc000000000000000|(s.committed_sequence()+2),expected_revision:s.engine().unwrap().world().view().revision,actions:vec![Action::Timeline{command:T::Advance{target:30,limit:1}}]};assert!(s.execute(Caller::System,collision).unwrap().model.is_ok());
    let seq=s.committed_sequence();let hash=s.state_hash().unwrap();let before=s.engine().unwrap().timeline().unwrap().progress;drop(s);
    let mut c=Client::new();c.start_probe(&executable(),&world,false).unwrap();event(&mut c,&p);c.submit(1,Command::RunDue).unwrap();let rejected=event(&mut c,&p);assert_eq!(rejected.kind,"result");assert!(rejected.payload["Err"].as_str().unwrap().starts_with("timeline_due_namespace_collision:"));c.stop().unwrap();
    let mut s=Store::open(&world,&rules()).unwrap();assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.engine().unwrap().timeline().unwrap().progress,before);
    apply(&mut s,Action::Timeline{command:T::Advance{target:30,limit:1}});let before=s.engine().unwrap().timeline().unwrap().progress;let seq=s.committed_sequence();drop(s);
    c=Client::new();c.start_probe(&executable(),&world,false).unwrap();event(&mut c,&p);c.submit(2,Command::RunDue).unwrap();let selected=event(&mut c,&p);assert_eq!(selected.kind,"timeline_continuation_selected");let e:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();
    c.stop().unwrap(); // application deliberately does not consume terminal; original owned exit is awaited
    let mut s=Store::open(&world,&rules()).unwrap();let record=s.receipt(e.operation_id).unwrap().unwrap();assert!(record.outcome.is_ok());assert_eq!(s.committed_sequence(),seq+1);assert_eq!(s.engine().unwrap().timeline().unwrap().progress.serial,before.serial+1);
    let hash=s.state_hash().unwrap();let seq=s.committed_sequence();assert!(s.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);
}

#[test]
fn original_admission_refusal_proxy_keeps_accepted_target_and_directory_uses_original_safe_quote(){
    use mycity_core::units::Money;
    let p=case("admission-budget-proxy");let world=p.join("world");let mut s=create(&world);apply(&mut s,Action::Timeline{command:T::Enable});
    // Explicit original Claims laboratory proxy: it validates admission budgeting,
    // not a real population/workload or legal-contract product acceptance.
    apply(&mut s,Action::Timeline{command:T::Claims{first_id:1,count:1,payer:3,payee:4,amount:Money::new(1).unwrap(),due:30}});
    apply(&mut s,Action::Timeline{command:T::Claims{first_id:2,count:128,payer:3,payee:4,amount:Money::new(1).unwrap(),due:60}});
    for _ in 0..128 {apply(&mut s,Action::Timeline{command:T::Advance{target:60,limit:1}});let t=s.engine().unwrap().timeline().unwrap().progress;if t.last_complete==30&&t.phase.is_none(){break;}}
    let before=s.engine().unwrap().timeline().unwrap().progress;assert_eq!((before.last_complete,before.target,before.phase),(30,Some(60),None));assert!(s.continuation_status().unwrap().is_none());
    let seq=s.committed_sequence();let hash=s.state_hash().unwrap();let e=Envelope{world:1,branch:1,operation_id:seq+1,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![Action::Timeline{command:T::Advance{target:60,limit:1}}]};
    let error=s.execute(Caller::System,e).unwrap_err();assert!(format!("{error:?}").contains("timeline_page_limit_too_small"));assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
    let d=directory::collect(&s,false).unwrap();let Route::Selected(RouteAction::Timeline(selected))=d.route()else{panic!("original target cannot be NoWork")};assert!(selected.is_admission());assert_eq!(selected.page_limit(),32);drop(s);
    let mut c=Client::new();c.start_probe(&executable(),&world,false).unwrap();event(&mut c,&p);c.submit(1,Command::RunDue).unwrap();let selected=event(&mut c,&p);assert_eq!(selected.kind,"timeline_continuation_selected");assert_eq!(selected.payload["plan"],"original_next_boundary_admission");let result=event(&mut c,&p);assert!(result.payload["Ok"].is_object(),"{result:?}");c.stop().unwrap();
    let mut s=Store::open(&world,&rules()).unwrap();if s.engine().unwrap().timeline().unwrap().progress.target.is_some(){drain(&mut s,60,32);}
    assert_eq!(s.engine().unwrap().timeline().unwrap().progress.last_complete,60);assert!(s.engine().unwrap().timeline().unwrap().claims.iter().all(|x|x.processed&&x.paid==Money::new(1).unwrap()));
    fs::write(p.join("proxy-scope.json"),serde_json::to_vec_pretty(&serde_json::json!({"kind":"original_claims_admission_budget_proxy","claims":129,"not_population_or_real_contract_business_acceptance":true,"original_rejected_width":1,"new_boundary_quote_width":32,"target_preserved":60})).unwrap()).unwrap();
}

#[test]
fn original_accepted_timeline_without_optional_models_is_not_no_work_and_stale_dirty_is_read_only(){
    let p=case("directory-owner");let mut s=create(&p.join("world"));
    assert!(matches!(directory::collect(&s,false).unwrap().route(),Route::NoWork));
    accepted(&mut s);let hash=s.state_hash().unwrap();let seq=s.committed_sequence();
    let selected=directory::collect(&s,false).unwrap();
    assert!(matches!(selected.route(),Route::Selected(RouteAction::Timeline(_))));
    assert_eq!(directory::collect(&s,false).unwrap(),selected);
    assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
    assert!(selected.entries()[3].dirty==Some(true));assert!(selected.entries()[3].registry_wake);
    assert!(matches!(directory::collect(&s,true).unwrap().route(),Route::Busy));
    assert!(directory::collect(&s,true).unwrap().validate(&s,true).is_ok());
    fs::write(p.join("actual-directory.json"),serde_json::to_vec_pretty(&selected.report().unwrap()).unwrap()).unwrap();
    let ticket=apply(&mut s,Action::Timeline{command:T::Advance{target:30,limit:1}});
    assert_eq!(selected.validate(&s,false),Err(Error::Stale));
    let hash=s.state_hash().unwrap();let seq=s.committed_sequence();s.checkpoint().unwrap();drop(s);
    let mut s=Store::open(&p.join("world"),&rules()).unwrap();let recovered=directory::collect(&s,false).unwrap();
    assert!(matches!(recovered.route(),Route::Selected(RouteAction::Timeline(_))));
    assert!(s.execute(Caller::System,ticket).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
    assert_eq!(directory::collect(&s,false).unwrap(),recovered);
}

#[test]
fn actual_sim_directory_continues_original_timeline_and_cold_retry_without_optional_models(){
    let p=case("directory-sim");let world=p.join("world");let mut s=create(&world);accepted(&mut s);
    let mut reference=create(&p.join("reference"));accepted(&mut reference);assert_eq!(reference.state_hash().unwrap(),s.state_hash().unwrap());drop(s);
    let exe=fs::canonicalize(std::env::var_os("MYCITY_DIRECTORY_HEADLESS_EXE").expect("frozen current headless")).unwrap();assert!(exe.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let mut c=Client::new();c.start_probe(&exe,&world,false).unwrap();assert_eq!(event(&mut c,&p).kind,"ready");
    let mut tickets=Vec::new();let mut complete=false;
    for id in 1..129 {
        c.submit(id,Command::RunDue).unwrap();let selected=event(&mut c,&p);assert_eq!(selected.id,id);assert_eq!(selected.kind,"timeline_continuation_selected");
        let e:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();
        assert_eq!(e.operation_id,0xc000_0000_0000_0000|(reference.committed_sequence()+1));
        assert_eq!(e.actions,vec![Action::Timeline{command:T::Advance{target:30,limit:1}}]);
        assert!(reference.execute(Caller::System,e.clone()).unwrap().model.is_ok());tickets.push(e);
        let result=event(&mut c,&p);assert_eq!(result.id,id);assert_eq!(result.kind,"result");assert!(result.payload["Ok"].is_object(),"{result:?}");
        assert_eq!(result.payload["Ok"]["task_directory"]["providers"][3]["provider"],"timeline");
        complete=result.payload["Ok"]["timeline"]["target"].is_null();
        if id==1 {c.stop().unwrap();c=Client::new();c.start_probe(&exe,&world,false).unwrap();assert_eq!(event(&mut c,&p).kind,"ready");}
        if complete {break;}
    }
    assert!(complete);c.submit(1001,Command::Checkpoint).unwrap();assert_eq!(event(&mut c,&p).payload["Ok"]["checkpointed"],true);
    c.stop().unwrap();let mut s=Store::open(&world,&rules()).unwrap();
    assert_eq!(s.engine().unwrap().timeline().unwrap().progress.last_complete,30);
    assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());
    let hash=s.state_hash().unwrap();let seq=s.committed_sequence();
    for ticket in tickets {assert!(s.execute(Caller::System,ticket).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);}
    assert!(matches!(directory::collect(&s,false).unwrap().route(),Route::NoWork));drop(s);
    c=Client::new();c.start_probe(&exe,&world,false).unwrap();let ready=event(&mut c,&p);assert_eq!(ready.payload["state_sha256"],serde_json::to_value(&hash).unwrap());
    c.submit(1002,Command::RunDue).unwrap();let none=event(&mut c,&p);assert_eq!(none.payload["Ok"]["state"],"no_work");
    c.stop().unwrap();fs::write(p.join("final-proof.json"),serde_json::to_vec_pretty(&serde_json::json!({"hash":hash,"sequence":seq,"last_complete":30,"original_manual_reference_equal":true})).unwrap()).unwrap();
}
