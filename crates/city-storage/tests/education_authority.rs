//! Real Opening/Engine/Store authority, separate from synthetic runtime witnesses.
use mycity_core::{education as e, education_authority::{self as a,Command as C}, education_runtime as r,
    timeline::Command as T, transaction::{Action,Caller,Envelope,Resource,Engine}, units::{GameTime,Money},
    venue_runtime::Command as V};
use mycity_storage::{Store,WritePoint};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io,path::{Path,PathBuf},process::{Command,Stdio},sync::atomic::{AtomicU64,Ordering},time::{Duration,Instant}};
const RULES:&str="issue54-actual-free-offline-education-v2";
const INPUT:&[u8]=include_bytes!("fixtures/education-store/free-offline-v1.json");
static NEXT:AtomicU64=AtomicU64::new(0);
fn t(n:u64)->GameTime {GameTime::new(n.try_into().unwrap()).unwrap()}
fn directory(name:&str)->PathBuf {
    let root=fs::canonicalize(std::env::var_os("MYCITY_EDUCATION_TEST_WORK").unwrap()).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work") && root.is_dir());
    let path=root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir(&path).unwrap();println!("RETAINED_EDUCATION_WORLD={}",path.display());path
}
fn create(path:&Path)->Store {
    fs::write(path.join("opening.json"),INPUT).unwrap();
    let store=Store::create(&path.join("world"),INPUT,
        vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES).unwrap();
    assert!(store.engine().unwrap().owned_capacity_bytes()<=mycity_core::transaction::ENGINE_BYTE_BUDGET);
    let layout=store.engine().unwrap().world().layout();
    fs::write(path.join("layout.json"),serde_json::to_vec_pretty(&layout).unwrap()).unwrap();
    println!("EDUCATION_LAYOUT={layout}");store
}
fn envelope(s:&Store,action:Action)->Envelope {
    let (scope,revision)=s.engine().unwrap().world().scope_revision();
    Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,
        expected_revision:revision,actions:vec![action]}
}
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope {
    let env=envelope(s,action);let out=s.execute(caller,env.clone()).unwrap();
    assert!(out.has_persistent_receipt && out.model.is_ok(),"{out:?}");env
}
fn event(s:&Store,cohort:u64,action:r::Action,actual:Option<e::Window>)->Action {
    let view=s.engine().unwrap().world().view().education.unwrap();
    Action::Education {command:C::Event{expected_epoch:view.stream_epoch,expected_sequence:view.consumed_sequence+1,
        command:r::Command{cohort,action},actual}}
}
fn advance(s:&mut Store,target:u64) {
    for _ in 0..1024 {
        let p=s.engine().unwrap().timeline().unwrap().progress;
        if p.target.is_none() && p.last_complete==target {return;}
        apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}});
    } panic!("bounded Timeline did not finish {target}");
}
fn setup(s:&mut Store) {
    apply(s,Caller::System,Action::Timeline{command:T::Enable});
    apply(s,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});
    let token=s.engine().unwrap().world().view().venue.unwrap().uses[0].token.number;
    apply(s,Caller::System,Action::Venue{use_id:1,command:V::Enter{event_id:1000,token_number:token,occurred_at:0}});
    advance(s,1);let action=event(s,1,r::Action::ActivateNoInstruction,None);apply(s,Caller::System,action);
}
fn cash(s:&Store)->Vec<(u32,i64)> {s.engine().unwrap().world().view().accounts.iter().map(|a|(a.id,a.cash.value())).collect()}
fn compact_envelope(s:&mut Store,prior:u64)->Envelope {
    let view=s.engine().unwrap().world().view().education.unwrap();
    let record=s.receipt(prior).unwrap().unwrap();
    let mut legacy=Sha256::new();legacy.update(b"mycity-education-full-journal-record-v1\0");
    legacy.update(serde_json::to_vec(&record).unwrap());
    assert_eq!(a::receipt_digest(&record).unwrap(),<[u8;32]>::from(legacy.finalize()));
    let anchor=r::ReceiptAnchor{store_ref:prior,root:a::receipt_digest(&record).unwrap(),epoch:view.stream_epoch,through:view.consumed_sequence};
    envelope(s,Action::Education{command:C::Compact{expected_runtime:view.fingerprint,anchor}})
}
fn reject(s:&mut Store,caller:Caller,action:Action) {
    let before=s.engine().unwrap().business_comparison_checkpoint().unwrap();
    let out=s.execute(caller,envelope(s,action)).unwrap();assert!(out.model.is_err(),"{out:?}");
    assert_eq!(s.engine().unwrap().business_comparison_checkpoint().unwrap(),before);
}

#[derive(Serialize,Deserialize)]
struct Probe {world:PathBuf,expected_hash:String,envelope:Envelope,checkpoint:bool,result:PathBuf}
fn child(path:&Path,world:&Path,hash:&str,envelope:&Envelope,checkpoint:bool) {
    let root=path.join(format!("child-{}",NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();
    let probe=Probe{world:world.to_owned(),expected_hash:hash.to_owned(),envelope:envelope.clone(),checkpoint,result:root.join("probe-result.json")};
    let input=root.join("probe.json");fs::write(&input,serde_json::to_vec(&probe).unwrap()).unwrap();
    fs::write(root.join("launch.json"),serde_json::to_vec(&json!({"test":"education_store_child_probe","timeout_seconds":60})).unwrap()).unwrap();
    let mut process=Command::new(std::env::current_exe().unwrap()).args(["--exact","education_store_child_probe","--ignored","--nocapture"])
        .env("MYCITY_EDUCATION_CHILD_PROBE",&input).stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(root.join("stdout.txt")).unwrap()))
        .stderr(Stdio::from(fs::File::create(root.join("stderr.txt")).unwrap())).spawn().unwrap();
    fs::write(root.join("pid.txt"),process.id().to_string()).unwrap();let start=Instant::now();
    let (status,timeout)=loop {
        if let Some(s)=process.try_wait().unwrap() {break (s,false);}
        if start.elapsed()>Duration::from_secs(60) {process.kill().unwrap();break(process.wait().unwrap(),true);}
        std::thread::sleep(Duration::from_millis(10));
    };
    fs::write(root.join("exit.json"),serde_json::to_vec(&json!({"pid":process.id(),"success":status.success(),"code":status.code(),"timeout":timeout})).unwrap()).unwrap();
    assert!(!timeout && status.success(),"retained child {}",root.display());assert!(probe.result.exists());
}
#[test]
#[ignore="parent invokes a real fresh process with retained request/output files"]
fn education_store_child_probe() {
    let input=std::env::var_os("MYCITY_EDUCATION_CHILD_PROBE").unwrap();let p:Probe=serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    let mut s=Store::open(&p.world,RULES).unwrap();assert!(s.engine().unwrap().education_is_authenticated());
    assert_eq!(s.state_hash().unwrap(),p.expected_hash);let seq=s.committed_sequence();
    let out=s.execute(Caller::System,p.envelope).unwrap();assert!(out.has_persistent_receipt && out.model.is_ok());
    assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),p.expected_hash);
    if p.checkpoint {s.checkpoint().unwrap();assert_eq!(s.state_hash().unwrap(),p.expected_hash);}
    fs::write(p.result,serde_json::to_vec(&json!({"pid":std::process::id(),"sequence":seq,"hash":s.state_hash().unwrap(),"authenticated":true})).unwrap()).unwrap();
}

#[test]
fn actual_preparation_instruction_shared_members_and_compact_lost_reply_recover() {
    let path=directory("lesson");let world=path.join("world");let mut s=create(&path);let initial_cash=cash(&s);
    setup(&mut s);let action=event(&s,1,r::Action::BeginPreparation,None);let begin=apply(&mut s,Caller::System,action);
    assert_eq!(s.engine().unwrap().world().view().member_activity.unwrap().active_count,1);
    let action=event(&s,2,r::Action::ActivateNoInstruction,None);apply(&mut s,Caller::System,action);
    let action=event(&s,2,r::Action::BeginPreparation,None);reject(&mut s,Caller::System,action);
    advance(&mut s,7201);
    let action=event(&s,1,r::Action::SettlePreparation{started_event_id:begin.operation_id,close:true},None);apply(&mut s,Caller::System,action);
    assert_eq!(s.engine().unwrap().world().view().education.unwrap().progress[0].unwrap().1.actual_seconds_per_learner,0);
    let action=event(&s,1,r::Action::BeginLesson,None);let lesson=apply(&mut s,Caller::System,action);
    advance(&mut s,10801);
    let action=event(&s,1,r::Action::SettleLesson{started_event_id:lesson.operation_id,close:false},None);apply(&mut s,Caller::System,action);
    assert_eq!(s.engine().unwrap().world().view().education.unwrap().progress[0].unwrap().1.actual_seconds_per_learner,3600);
    let h=s.state_hash().unwrap();drop(s);let mut s=Store::open(&world,RULES).unwrap();assert_eq!(s.state_hash().unwrap(),h);
    assert_eq!(s.engine().unwrap().world().view().member_activity.unwrap().active_count,1);
    advance(&mut s,14401);
    let action=event(&s,1,r::Action::SettleLesson{started_event_id:lesson.operation_id,close:true},None);let final_lesson=apply(&mut s,Caller::System,action);
    let view=s.engine().unwrap().world().view();let progress=view.education.unwrap().progress[0].unwrap().1;
    assert_eq!(progress.actual_seconds_per_learner,7200);assert_eq!(progress.actual_teaching_days,1);assert_eq!(progress.fee_cents_per_learner,Money::ZERO);
    assert_eq!(view.member_activity.unwrap().active_count,0);assert_eq!(cash(&s),initial_cash);
    assert_eq!(initial_cash.iter().map(|(_,n)|n).sum::<i64>(),3_000_000);
    assert_eq!(s.engine().unwrap().resources().iter().find(|r|r.id==501).unwrap().occupied,3);
    let env=compact_envelope(&mut s,final_lesson.operation_id);let outcome=s.execute_with_observer(Caller::System,env.clone(),|point|{
        if point==WritePoint::CurrentRenamed {Err(io::Error::other("education original compact response lost"))}else{Ok(())}
    });assert!(outcome.is_err());drop(s);
    let mut s=Store::open(&world,RULES).unwrap();let hash=s.state_hash().unwrap();let seq=s.committed_sequence();
    let out=s.execute(Caller::System,env.clone()).unwrap();assert!(out.has_persistent_receipt && out.model.is_ok());
    assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(cash(&s),initial_cash);
    assert!(s.engine().unwrap().world().view().education.unwrap().prefix_anchor.is_some());drop(s);
    child(&path,&world,&hash,&env,true);child(&path,&world,&hash,&env,false);
    assert_eq!(fs::read(path.join("opening.json")).unwrap(),INPUT);
}

#[test]
fn safe_exit_retains_unknown_history_and_authenticated_endpoint_through_compaction() {
    let path=directory("history");let world=path.join("world");let mut s=create(&path);setup(&mut s);
    let action=event(&s,1,r::Action::BeginPreparation,None);let begin=apply(&mut s,Caller::System,action);
    let end=3*86400+1;advance(&mut s,end);let token=s.engine().unwrap().world().view().venue.unwrap().uses[0].token.number;
    let exited=apply(&mut s,Caller::System,Action::Venue{use_id:1,command:V::SafeExit{event_id:2000,token_number:token,occurred_at:end}});
    let view=s.engine().unwrap().world().view();assert_eq!(view.member_activity.unwrap().active_count,0);
    assert_eq!(view.education.as_ref().unwrap().awaiting[0],Some(1));assert_eq!(view.education.unwrap().progress[0].unwrap().1.actual_seconds_per_learner,0);
    assert_eq!(s.engine().unwrap().resources().iter().find(|r|r.id==501).unwrap().occupied,0);
    let env=compact_envelope(&mut s,exited.operation_id);assert!(s.execute(Caller::System,env).unwrap().model.is_ok());
    s.checkpoint().unwrap();drop(s);let mut s=Store::open(&world,RULES).unwrap();advance(&mut s,end+1000);
    let history=|through,finalize|r::Action::SettleAfterExit{started_event_id:begin.operation_id,exited_at:t(end),exit_operation_id:exited.operation_id,actual_through:t(through),finalize};
    let actual=Some(e::Window{start:t(1),end:t(end)});
    let action=event(&s,1,history(86401,false),actual);let first=apply(&mut s,Caller::System,action);
    let env=compact_envelope(&mut s,first.operation_id);assert!(s.execute(Caller::System,env).unwrap().model.is_ok());s.checkpoint().unwrap();drop(s);
    let mut s=Store::open(&world,RULES).unwrap();let action=event(&s,1,history(86401,true),None);reject(&mut s,Caller::System,action);
    assert_eq!(s.engine().unwrap().world().view().education.unwrap().awaiting[0],Some(1));
    let action=event(&s,1,history(172801,false),actual);apply(&mut s,Caller::System,action);
    let action=event(&s,1,history(end,true),actual);apply(&mut s,Caller::System,action);
    let view=s.engine().unwrap().world().view();assert!(view.education.as_ref().unwrap().awaiting.iter().all(Option::is_none));
    assert_eq!(view.education.unwrap().progress[0].unwrap().1.actual_seconds_per_learner,0);
    assert_eq!(view.member_activity.unwrap().active_count,0);let hash=s.state_hash().unwrap();s.checkpoint().unwrap();drop(s);
    let mut s=Store::open(&world,RULES).unwrap();assert_eq!(s.state_hash().unwrap(),hash);
}

#[test]
fn caller_epoch_sequence_and_raw_engine_without_receipt_cannot_create_authority() {
    let path=directory("gates");let mut s=create(&path);setup(&mut s);
    let action=event(&s,1,r::Action::BeginPreparation,None);reject(&mut s,Caller::Account(3),action);
    let action=Action::Education{command:C::Event{expected_epoch:99,expected_sequence:2,command:r::Command{cohort:1,action:r::Action::BeginPreparation},actual:None}};reject(&mut s,Caller::System,action);
    let action=Action::Education{command:C::Event{expected_epoch:7,expected_sequence:1,command:r::Command{cohort:1,action:r::Action::BeginPreparation},actual:None}};reject(&mut s,Caller::System,action);
    let action=event(&s,1,r::Action::BeginLesson,None);reject(&mut s,Caller::System,action);
    let last=s.receipt(5).unwrap(); // Deliberately not used as authority; operation IDs may have gaps.
    drop(last);
    let mut engine=Engine::new(mycity_core::assembly::assemble(INPUT).unwrap(),
        vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()]).unwrap();
    let env=|engine:&Engine,op,action|Envelope{world:1,branch:1,operation_id:op,expected_revision:engine.world().scope_revision().1,actions:vec![action]};
    engine.execute(Caller::System,env(&engine,1,Action::Timeline{command:T::Enable})).unwrap();
    for op in 2..128 {let p=engine.timeline().unwrap().progress;if p.target.is_none()&&p.last_complete==1 {break;}
        engine.execute(Caller::System,env(&engine,op,Action::Timeline{command:T::Advance{target:1,limit:256}})).unwrap();}
    let mut record=None;let input=env(&engine,200,Action::Education{command:C::Event{expected_epoch:7,expected_sequence:1,command:r::Command{cohort:1,action:r::Action::ActivateNoInstruction},actual:None}});
    assert!(engine.execute_recorded(Caller::System,input,|_,r|{record=Some(*r);Ok::<_,io::Error>(())}).unwrap().is_ok());
    let view=engine.world().view().education.unwrap();let original=record.unwrap();let anchor=r::ReceiptAnchor{store_ref:200,root:a::receipt_digest(&original).unwrap(),epoch:7,through:1};
    let input=env(&engine,201,Action::Education{command:C::Compact{expected_runtime:view.fingerprint,anchor}});
    assert!(engine.execute(Caller::System,input).is_err());assert_eq!(engine.world().view().education.unwrap().fingerprint,view.fingerprint);
}

fn copy_world(from:&Path,to:&Path) {fs::create_dir(to).unwrap();for entry in fs::read_dir(from).unwrap() {let entry=entry.unwrap();assert!(entry.file_type().unwrap().is_file());fs::copy(entry.path(),to.join(entry.file_name())).unwrap();}}
#[test]
fn damaged_or_missing_original_anchor_archive_refuses_cold_recovery() {
    let path=directory("anchor-faults");let world=path.join("world");let mut s=create(&path);setup(&mut s);
    let view=s.engine().unwrap().world().view().education.unwrap();let prior=s.committed_sequence();
    let env=compact_envelope(&mut s,prior);assert!(s.execute(Caller::System,env).unwrap().model.is_ok());
    let valid_hash=s.state_hash().unwrap();s.checkpoint().unwrap();drop(s);
    let mut source=None;
    for file in fs::read_dir(&world).unwrap() {let file=file.unwrap();let name=file.file_name().to_string_lossy().into_owned();
        if name.starts_with("checkpoint-") && !name.starts_with("checkpoint-index-") && name.ends_with(".json") {
            let bytes=fs::read(file.path()).unwrap();let json:Value=serde_json::from_slice(&bytes).unwrap();
            if json["receipts"].as_array().is_some_and(|rows|rows.iter().any(|r|r["operation_id"]==prior)) {source=Some((name,bytes));break;}
        }
    }
    let (name,bytes)=source.expect("the actual protected anchor receipt segment");fs::write(path.join("anchor-original.json"),&bytes).unwrap();
    let missing=path.join("missing-world");copy_world(&world,&missing);fs::remove_file(missing.join(&name)).unwrap();
    assert!(Store::open(&missing,RULES).is_err());
    let damaged=path.join("damaged-world");copy_world(&world,&damaged);fs::write(damaged.join(&name),b"{\"damaged_anchor\":true}").unwrap();
    assert!(Store::open(&damaged,RULES).is_err());
    let mut original=Store::open(&world,RULES).unwrap();assert_eq!(original.state_hash().unwrap(),valid_hash);
    assert_eq!(original.engine().unwrap().world().view().education.unwrap().consumed_sequence,view.consumed_sequence);
}

#[test]
fn reused_slot_legacy_restore_clears_both_new_optional_backings() {
    use mycity_core::transaction::EngineSlot;
    let path=directory("legacy-slot");let s=create(&path);let new=s.engine().unwrap().export_checkpoint().unwrap();drop(s);
    let old=Engine::new(mycity_core::assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![]).unwrap();
    let bytes=old.export_checkpoint().unwrap();let json:Value=serde_json::from_slice(&bytes).unwrap();
    assert!(json["world"].get("education").is_none()&&json["world"].get("member_activity").is_none());
    let mut slot=EngineSlot::new().unwrap();slot.restore_into(&new).unwrap();assert!(slot.engine().unwrap().world().view().education.is_some());
    slot.restore_into(&bytes).unwrap();let view=slot.engine().unwrap().world().view();assert!(view.education.is_none()&&view.member_activity.is_none());
    assert_eq!(old.business_comparison_checkpoint().unwrap(),slot.engine().unwrap().business_comparison_checkpoint().unwrap());
    fs::write(path.join("legacy-checkpoint.json"),bytes).unwrap();
}

#[test]
fn v2_original_receipt_chain_survives_gc_and_refuses_damaged_anchor() {
    let path=directory("v2-anchor-gc");let world=path.join("world");
    fs::write(path.join("opening.json"),INPUT).unwrap();
    let mut s=Store::create_spatial_v2(&world,INPUT,
        vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    setup(&mut s);let prior=s.committed_sequence();let env=compact_envelope(&mut s,prior);
    assert!(s.execute(Caller::System,env.clone()).unwrap().model.is_ok());
    s.checkpoint().unwrap();
    // A newer real event and another checkpoint retain an older prefix ticket.
    advance(&mut s,2);let action=event(&s,1,r::Action::NoInstruction{window:e::Window{start:t(1),end:t(2)}},None);
    apply(&mut s,Caller::System,action);s.checkpoint().unwrap();
    let hash=s.state_hash().unwrap();let seq=s.committed_sequence();drop(s);
    // An unreachable GC candidate is retained outside the world before the
    // original open/validation/GC path removes only its in-world copy.
    fs::write(path.join("unreachable-original.json"),b"{}").unwrap();
    fs::copy(path.join("unreachable-original.json"),world.join("world-999999.json")).unwrap();
    let mut s=Store::open(&world,RULES).unwrap();assert!(!world.join("world-999999.json").exists());
    assert!(s.engine().unwrap().education_is_authenticated());assert_eq!(s.state_hash().unwrap(),hash);
    let original=s.receipt(prior).unwrap().unwrap();assert_eq!(original.operation_id,prior);
    assert!(s.execute(Caller::System,env.clone()).unwrap().model.is_ok());
    assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);drop(s);
    child(&path,&world,&hash,&env,true);
    let mut source=None;
    for file in fs::read_dir(&world).unwrap() {let file=file.unwrap();let name=file.file_name().to_string_lossy().into_owned();
        if name.starts_with("receipts-") && name.ends_with(".json") {
            let bytes=fs::read(file.path()).unwrap();let json:Value=serde_json::from_slice(&bytes).unwrap();
            if json["receipts"].as_array().is_some_and(|rows|rows.iter().any(|r|r["operation_id"]==prior)) {source=Some((name,bytes));break;}
        }
    }
    let (name,bytes)=source.expect("original committed v2 anchor body");
    fs::write(path.join("anchor-v2-original.json"),&bytes).unwrap();
    let missing=path.join("missing-world");copy_world(&world,&missing);fs::remove_file(missing.join(&name)).unwrap();
    assert!(Store::open(&missing,RULES).is_err());
    let damaged=path.join("damaged-world");copy_world(&world,&damaged);
    fs::write(damaged.join(&name),b"{\"damaged_original_v2_receipt\":true}").unwrap();
    assert!(Store::open(&damaged,RULES).is_err());
    assert_eq!(Store::open(&world,RULES).unwrap().state_hash().unwrap(),hash);
}
