//! Actual V2 provider -> original RunDue/Store/Timeline, not selector-only proof.
use mycity_core::{transaction::{Action,Caller,Envelope},timeline::Command as T,venue_runtime::Command as V,facility_authority::Command as F,continuous_supply::Command as S};
use mycity_storage::Store;
use mycity_headless::{required_dispatch::{self,SupplySelection,DueError},sim_client::Client,sim_host::Command as C};
use std::{fs,path::{Path,PathBuf},time::{Duration,SystemTime,UNIX_EPOCH}};
#[path="../../../tests/fixtures/water-treatment-runtime/opening.rs"]mod fixture;
fn apply(s:&mut Store,who:Caller,action:Action)->Envelope{
    let e=Envelope{world:1,branch:1,operation_id:s.committed_sequence()+1,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![action]};
    assert!(s.execute(who,e.clone()).unwrap().model.is_ok());e
}
#[test]
fn actual_v2_rundue_preserves_original_reservation_p5_material_and_retry_after_sim_restart(){ run_water_due(false); }
#[test]
fn actual_v2_normal_role_retains_production_move_save_and_returns_original_pool(){ run_water_due(true); }
fn run_water_due(pooled:bool){
    let root=fs::canonicalize(std::env::var_os("MYCITY_WATER_TEST_ROOT").expect("explicit retained SSD root")).unwrap();assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("water-rundue-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&p).unwrap();
    let world=p.join("world");let raw=serde_json::to_vec(&fixture::input()).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    let rules=format!("sim-w3:{}",env!("MYCITY_SOURCE_SHA256"));
    let mut s=Store::create_spatial_v2(&world,&raw,fixture::resources(),&rules,include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});
    for use_id in [1,2]{apply(&mut s,Caller::Account(3),Action::Venue{use_id,command:V::Accept});let token=s.engine().unwrap().world().view().venue.unwrap().uses.iter().find(|v|v.use_id==use_id).unwrap().token.number;
        apply(&mut s,Caller::System,Action::Venue{use_id,command:V::Enter{event_id:1000+use_id,token_number:token,occurred_at:0}});apply(&mut s,Caller::System,Action::FacilityWork{command:F::Begin{contract_id:use_id}});
    }
    apply(&mut s,Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}});
    if pooled {
        loop { apply(&mut s,Caller::System,Action::Timeline{command:T::Advance{target:3,limit:256}}); if s.engine().unwrap().timeline().unwrap().progress.target.is_none(){break;} }
        let from=s.engine().unwrap().world().stock_ref(202).unwrap();let to=s.engine().unwrap().world().stock_ref(205).unwrap();
        apply(&mut s,Caller::Account(3),Action::MoveStock{from,to,quantity:mycity_core::units::Quantity::new(49000).unwrap()});
        s.checkpoint().unwrap();
    }
    apply(&mut s,Caller::System,Action::Timeline{command:T::Advance{target:5500,limit:1}});
    let stale=match required_dispatch::select_supply_continuation(&s,false).unwrap(){SupplySelection::Selected(v)=>v,v=>panic!("{v:?}")};
    assert_eq!(stale.key().unwrap().source_version,2);assert_eq!(stale.page_limit(),1);drop(s);
    let executable=PathBuf::from(std::env::var_os("MYCITY_WATER_HEADLESS_EXE").expect("frozen original headless executable")).canonicalize().unwrap();
    assert!(executable.starts_with(Path::new("/Volumes/HP P900/dev/MyCity/work")));
    let pool_root=p.join("normal-pool");
    let mut pool=if pooled {Some(mycity_headless::normal_pool::Process::spawn(&executable,&pool_root,Some(&p.join("pool-heaps.jsonl"))).unwrap())}else{None};
    let start=|client:&mut Client|{if pooled {client.start_in_pool(&executable,&world,false,false,&pool_root).unwrap();}else{client.start_probe(&executable,&world,false).unwrap();}client.next(Duration::from_secs(15)).unwrap()};
    let mut client=Client::new();let ready=start(&mut client);assert_eq!(ready.kind,"ready");
    let mut memory=vec![serde_json::to_value(&ready).unwrap()];
    let mut tickets=Vec::new();let mut log=Vec::new();let mut complete=false;
    for id in 1..129{
        client.submit(id,C::RunDue).unwrap();let mut selected=None;let mut terminal=None;
        for _ in 0..16{let event=client.next(Duration::from_secs(15)).unwrap();assert_eq!(event.id,id);
            if event.kind=="supply_continuation_selected"{assert!(selected.is_none());selected=Some(event);}
            else if event.kind=="result"{terminal=Some(event);break;}else{panic!("unexpected event {event:?}");}
        }
        let selected=selected.expect("one original supply reservation");let terminal=terminal.expect("actual original result");
        let e:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();
        assert_eq!(e.operation_id,0xc000_0000_0000_0000|(selected.payload["before_sequence"].as_u64().unwrap()+1));
        let progress=&terminal.payload["Ok"]["timeline"];assert!(progress.is_object(),"{terminal:?}");
        complete=progress["target"].is_null();tickets.push(e);log.push(serde_json::json!({"selected":selected,"result":terminal}));
        if id==1{client.stop().unwrap();client=Client::new();let ready=start(&mut client);assert_eq!(ready.kind,"ready");memory.push(serde_json::to_value(ready).unwrap());}
        if complete{break;}
    }
    if pooled {
        client.submit(1001,C::Checkpoint).unwrap();let saved=client.next(Duration::from_secs(15)).unwrap();assert_eq!(saved.kind,"result");assert_eq!(saved.payload["Ok"]["checkpointed"],true);
        client.submit(1002,C::Status).unwrap();let status=client.next(Duration::from_secs(15)).unwrap();assert_eq!(status.kind,"result");assert!(status.payload["Ok"].is_object());memory.push(serde_json::to_value(status).unwrap());
        client.submit(1003,C::Stop).unwrap();assert_eq!(client.next(Duration::from_secs(15)).unwrap().kind,"result");client.stop().unwrap();
        client=Client::new();let ready=start(&mut client);assert_eq!(ready.kind,"ready");memory.push(serde_json::to_value(ready).unwrap());
        client.submit(1004,C::Status).unwrap();let status=client.next(Duration::from_secs(15)).unwrap();assert!(status.payload["Ok"].is_object());memory.push(serde_json::to_value(status).unwrap());
        client.submit(1005,C::Stop).unwrap();assert_eq!(client.next(Duration::from_secs(15)).unwrap().kind,"result");client.stop().unwrap();
        for event in &memory { let payload=event.get("payload").unwrap();let v=payload.get("Ok").unwrap_or(payload);let a=&v["runtime"]["allocator_boundary"]["snapshot"];assert!(a["activation"].is_object(),"{a}");assert_eq!(a["allocation_failures"],0);assert!(a["arena_used_bytes"].as_u64().unwrap()<=a["arena_usable_bytes"].as_u64().unwrap()); }
        fs::write(p.join("actual-role-memory.json"),serde_json::to_vec_pretty(&memory).unwrap()).unwrap();
        let balance=mycity_headless::normal_pool::status(&pool_root).unwrap();assert_eq!(balance.available_touched_bytes,mycity_headless::normal_pool::BYTES);assert_eq!(balance.workspace_grants,balance.workspace_returns);assert_eq!(balance.grants,balance.returns);
        fs::write(p.join("actual-pool-return.json"),serde_json::to_vec_pretty(&balance).unwrap()).unwrap();pool.as_mut().unwrap().finish().unwrap();
    }
    client.stop().unwrap();fs::write(p.join("actual-rundue-events.json"),serde_json::to_vec_pretty(&log).unwrap()).unwrap();assert!(complete,"finite original continuation");
    let mut s=Store::open(&world,&rules).unwrap();assert_eq!(required_dispatch::validate_supply_continuation(&s,&stale,false),Err(DueError::SelectionChanged));
    let hash=s.state_hash().unwrap();let seq=s.committed_sequence();for e in tickets{assert!(s.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);}
    let v=s.engine().unwrap().world().view();let product=v.stocks.iter().find(|s|s.id==202).unwrap();assert_eq!((product.quantity.value(),product.basis.value()),(if pooled{97_951_000}else{98_000_000},if pooled{176758}else{177548}));
    assert_eq!(v.accounts.iter().find(|a|a.id==2).unwrap().cash.value(),211002);assert_eq!(s.engine().unwrap().timeline().unwrap().progress.last_complete,5500);
    s.checkpoint().unwrap();drop(s);let reopened=Store::open(&world,&rules).unwrap();assert_eq!(reopened.state_hash().unwrap(),hash);
    println!("RETAINED_ACTUAL_WATER_RUNDUE={}",p.display());
}
