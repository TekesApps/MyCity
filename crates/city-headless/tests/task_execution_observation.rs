//! Real original RunDue, retained worlds and independent direct-Advance reference.
use mycity_core::{timeline::Command as T, transaction::{Action, Caller, Envelope, Resource}};
use mycity_headless::{sim_client::Client, sim_host::{Command, Event}};
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
fn run_page(c:&mut Client,p:&Path,id:u64,reference:&mut Store,provider:&str)->(Envelope,Value) {
    let before=reference.engine().unwrap().timeline().unwrap().progress;
    let binding=match provider {"timeline"=>serde_json::to_value(before).unwrap(),"supply"=>serde_json::to_value(reference.engine().unwrap().continuous_supply_due_state().unwrap().unwrap()).unwrap(),_=>panic!("expected original provider")};
    let kind=match provider {"timeline"=>"timeline_continuation_selected","supply"=>"supply_continuation_selected",_=>unreachable!()};
    let sequence=reference.committed_sequence();c.submit(id,Command::RunDue).unwrap();
    let selected=event(c,p);assert_eq!(selected.kind,kind);
    let envelope:Envelope=serde_json::from_value(selected.payload["envelope"].clone()).unwrap();
    assert!(reference.execute(Caller::System,envelope.clone()).unwrap().model.is_ok());
    let result=event(c,p);assert_eq!(result.kind,"result");assert!(result.payload["Ok"].is_object(),"{result:?}");
    let report=&result.payload["Ok"]["execution_observation"];
    assert_eq!(report["schema"],"mycity-task-execution-observation-v1");assert!(serde_json::to_vec(report).unwrap().len()<=12*1024);
    assert_eq!(report["sim"]["execution"],"sim_timeline_page");let sim=&report["sim"]["observation"];
    assert_eq!(sim["context"]["source"]["provider"],provider);assert_eq!(sim["context"]["source"]["binding"],binding);
    if provider=="timeline" {assert_eq!(sim["context"]["clock"],json!({"kind":"world"}));}else{assert_eq!(sim["context"]["clock"],json!({"kind":"development","city_id":42}));}
    assert_eq!(sim["context"]["before_sequence"],sequence);assert_eq!(sim["context"]["before"],serde_json::to_value(before).unwrap());
    assert_eq!(sim["context"]["operation"],envelope.operation_id);assert_eq!(sim["context"]["world"],envelope.world);assert_eq!(sim["context"]["branch"],envelope.branch);
    assert_eq!(sim["commit"],"committed");assert_eq!(sim["committed_advance_operations"],1);assert_eq!(sim["original_receipt_confirmed"],true);assert_eq!(sim["model_outcome"],"accepted");
    assert_eq!(sim["after"],serde_json::to_value(reference.engine().unwrap().timeline().unwrap().progress).unwrap());
    assert_eq!(sim["allocation"],"inherited_sim_role_no_task_grant_or_refund");assert_eq!(sim["scratch_peak"],"unknown_not_instrumented");
    assert!(sim["process_instance"].as_str().unwrap().parse::<u128>().unwrap()>0);assert!(sim["pid"].as_u64().unwrap()>0);
    (envelope,report.clone())
}
#[test]
fn actual_timeline_report_replay_restart_and_reference_keep_original_business() {
    let p=case("timeline");let(world,mut reference)=prepare(&p,false);let mut c=start(&world,&p);let mut tickets=vec![];let mut done=false;
    for page in 0..64u64 {
        let(e,report)=run_page(&mut c,&p,10+page*4,&mut reference,"timeline");tickets.push(e.clone());
        if page==0 {
            assert!(!report["sim"]["observation"]["after"]["target"].is_null());
            let seq=reference.committed_sequence();let status=call(&mut c,&p,12,Command::Status);assert_eq!(status["sequence"],seq);assert_eq!(status["runtime"]["task_execution_observation"]["sim"],report["sim"]);
            c.stop().unwrap();
            // C tickets belong to the original Store; public Execute deliberately
            // excludes that namespace. Do not weaken the real command gate.
            let mut restored=Store::open(&world,&rules()).unwrap();let hash=restored.state_hash().unwrap();
            assert!(restored.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(restored.committed_sequence(),seq);assert_eq!(restored.state_hash().unwrap(),hash);drop(restored);
            c=start(&world,&p); // no CPU reconstruction from the old ticket
        }
        if reference.engine().unwrap().timeline().unwrap().progress.target.is_none(){done=true;break;}
    }
    assert!(done);call(&mut c,&p,1001,Command::Checkpoint);c.stop().unwrap();
    let mut restored=Store::open(&world,&rules()).unwrap();assert_eq!(restored.state_hash().unwrap(),reference.state_hash().unwrap());
    let sequence=restored.committed_sequence();let hash=restored.state_hash().unwrap();
    for ticket in tickets {assert!(restored.execute(Caller::System,ticket).unwrap().model.is_ok());assert_eq!(restored.committed_sequence(),sequence);assert_eq!(restored.state_hash().unwrap(),hash);}
    fs::write(p.join("final-proof.json"),serde_json::to_vec_pretty(&json!({"hash":hash,"sequence":sequence,"reference_equal":true})).unwrap()).unwrap();
}
#[test]
fn same_time_required_page_has_no_invented_task_window_or_completion() {
    let p=case("zero-window");let(world,mut reference)=prepare(&p,true);let before=reference.engine().unwrap().timeline().unwrap().progress;
    assert_eq!((before.last_complete,before.at),(0,0));let mut c=start(&world,&p);let(_,report)=run_page(&mut c,&p,1,&mut reference,"timeline");
    let sim=&report["sim"]["observation"];assert!(sim["context"]["task"].is_null());assert_eq!(sim["after"]["at"],0);assert_eq!(sim["after"]["last_complete"],0);
    assert!(!sim["after"]["phase"].is_null());assert_eq!(sim["after"]["serial"].as_u64().unwrap(),before.serial+1);c.stop().unwrap();
    assert_eq!(Store::open(&world,&rules()).unwrap().state_hash().unwrap(),reference.state_hash().unwrap());
}
#[test]
fn actual_supply_selection_reports_the_original_source_and_one_operation() {
    let p=case("supply");let world=p.join("world");let raw=serde_json::to_vec(&supply_opening::input()).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    let mut s=Store::create_spatial(&world,&raw,supply_opening::resources(),&rules(),include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    apply(&mut s,Action::Timeline{command:T::Enable});
    let(scope,revision)=s.engine().unwrap().world().scope_revision();let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![Action::ContinuousSupply{command:mycity_core::continuous_supply::Command::Start{run_id:1,pumps:3}}]};
    assert!(s.execute(Caller::Account(1),e).unwrap().model.is_ok());
    let mut pending=false;for _ in 0..128 {apply(&mut s,Action::Timeline{command:T::Advance{target:30,limit:1}});if s.engine().unwrap().timeline().unwrap().progress.phase==Some(mycity_core::schedule::Phase::P5){pending=true;break;}}
    assert!(pending);s.checkpoint().unwrap();drop(s);copy(&world,&p.join("reference"));let mut reference=Store::open(&p.join("reference"),&rules()).unwrap();
    let mut c=start(&world,&p);let(e,_)=run_page(&mut c,&p,1,&mut reference,"supply");c.stop().unwrap();let mut restored=Store::open(&world,&rules()).unwrap();assert_eq!(restored.state_hash().unwrap(),reference.state_hash().unwrap());
    let sequence=restored.committed_sequence();assert!(restored.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(restored.committed_sequence(),sequence);
}
