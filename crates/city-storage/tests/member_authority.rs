//! Original shared ledger registration and Store recovery, not service fulfillment.
use mycity_core::{assembly,education_authority::Command as Education,education_runtime as er,
    timeline::Command as Timeline,transaction::{Action,Caller,Envelope,Engine,EngineSlot,Resource},venue_runtime::Command as Venue};
use mycity_storage::Store;
use serde_json::{Value,json};
use std::{fs,path::{Path,PathBuf},process::{Command,Stdio},sync::atomic::{AtomicU64,Ordering},time::{Duration,Instant}};
const INPUT:&[u8]=include_bytes!("fixtures/education-store/free-offline-v1.json");
const RULES:&str="original-member-activity-opening-v1";
static NEXT:AtomicU64=AtomicU64::new(0);
fn directory(name:&str)->PathBuf {
    let root=fs::canonicalize(std::env::var_os("MYCITY_MEMBER_TEST_WORK").expect("explicit external evidence root")).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();println!("RETAINED_MEMBER_WORLD={}",p.display());p
}
fn input(education:bool)->Vec<u8> {
    let mut raw:Value=serde_json::from_slice(INPUT).unwrap();
    raw["member_activity"]=json!({"version":1,"scope":{"world":1,"branch":1},
        "population_origin":raw["education"]["population_origin"],"total_people":10,"epoch":1});
    if !education {raw.as_object_mut().unwrap().remove("education");raw.as_object_mut().unwrap().remove("venue");}
    serde_json::to_vec(&raw).unwrap()
}
fn resources(education:bool)->Vec<Resource> {
    if education {vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()]}else{vec![]}
}
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope {
    let (scope,revision)=s.engine().unwrap().world().scope_revision();
    let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]};
    let out=s.execute(caller,e.clone()).unwrap();assert!(out.has_persistent_receipt&&out.model.is_ok(),"{out:?}");e
}
fn advance(s:&mut Store,target:u64) {
    for _ in 0..1024 {let p=s.engine().unwrap().timeline().unwrap().progress;if p.target.is_none()&&p.last_complete==target {return;}
        apply(s,Caller::System,Action::Timeline{command:Timeline::Advance{target,limit:256}});}
    panic!("original Timeline did not finish");
}
fn event(s:&mut Store,action:er::Action)->Envelope {
    let v=s.engine().unwrap().world().view().education.unwrap();
    apply(s,Caller::System,Action::Education{command:Education::Event{expected_epoch:v.stream_epoch,expected_sequence:v.consumed_sequence+1,
        command:er::Command{cohort:1,action},actual:None}})
}
fn probe(path:&Path,world:&Path,expected:&Value,replay:&Envelope,checkpoint:bool) {
    let p=path.join(format!("fresh-{}",NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();
    let spec=json!({"world":world,"expected":expected,"replay":replay,"checkpoint":checkpoint});let file=p.join("probe.json");fs::write(&file,serde_json::to_vec(&spec).unwrap()).unwrap();
    fs::write(p.join("launch.json"),serde_json::to_vec(&json!({"timeout_seconds":60,"functional_only":true})).unwrap()).unwrap();
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","member_store_child","--ignored","--nocapture"])
        .env("MYCITY_MEMBER_PROBE",&file).stdout(Stdio::from(fs::File::create(p.join("stdout.log")).unwrap()))
        .stderr(Stdio::from(fs::File::create(p.join("stderr.log")).unwrap())).spawn().unwrap();
    fs::write(p.join("start.json"),serde_json::to_vec(&json!({"pid":child.id()})).unwrap()).unwrap();let start=Instant::now();let mut timed_out=false;
    let status=loop {if let Some(s)=child.try_wait().unwrap(){break s;}if start.elapsed()>Duration::from_secs(60){timed_out=true;child.kill().unwrap();break child.wait().unwrap();}std::thread::sleep(Duration::from_millis(20));};
    fs::write(p.join("exit.json"),serde_json::to_vec(&json!({"pid":child.id(),"code":status.code(),"success":status.success(),"timed_out":timed_out})).unwrap()).unwrap();
    assert!(status.success()&&!timed_out,"fresh probe {}",p.display());
}
fn state(s:&Store)->Value {
    json!({"hash":s.state_hash().unwrap(),"member":s.engine().unwrap().member_activity_summary(),"registration":s.engine().unwrap().member_activity_registration()})
}
#[test]
#[ignore]
fn member_store_child() {
    let file=std::env::var_os("MYCITY_MEMBER_PROBE").expect("explicit owned probe");let spec:Value=serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    let mut s=Store::open(Path::new(spec["world"].as_str().unwrap()),RULES).unwrap();assert_eq!(state(&s),spec["expected"]);
    let before=s.committed_sequence();let envelope:Envelope=serde_json::from_value(spec["replay"].clone()).unwrap();
    let out=s.execute(Caller::System,envelope).unwrap();assert!(out.has_persistent_receipt&&out.model.is_ok());assert_eq!(s.committed_sequence(),before);assert_eq!(state(&s),spec["expected"]);
    if spec["checkpoint"].as_bool().unwrap(){s.checkpoint().unwrap();}assert_eq!(state(&s),spec["expected"]);
}
#[test]
fn standalone_member_registration_preserves_classic_and_spatial_recovery() {
    for spatial in [false,true] {
        let path=directory(if spatial{"standalone-spatial"}else{"standalone-classic"});let world=path.join("world");let bytes=input(false);fs::write(path.join("opening.json"),&bytes).unwrap();
        let mut s=if spatial {Store::create_spatial_v2(&world,&bytes,vec![],RULES,include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap()}
            else {Store::create(&world,&bytes,vec![],RULES).unwrap()};
        let view=s.engine().unwrap().world().view();assert_eq!(view.schema,"mycity-m0-state-v12");assert!(view.education.is_none());
        let member=view.member_activity.unwrap();assert_eq!(member.active_count,0);assert_eq!(member.next_ordinal,Some(1));
        let enabled=apply(&mut s,Caller::System,Action::Timeline{command:Timeline::Enable});
        let cp=s.engine().unwrap().export_checkpoint().unwrap();fs::write(path.join("engine-checkpoint.json"),&cp).unwrap();
        fs::write(path.join("layout.json"),serde_json::to_vec_pretty(&s.engine().unwrap().world().layout()).unwrap()).unwrap();
        for case in ["orphan","unowned-history","wrong-epoch"] {
            let mut forged:Value=serde_json::from_slice(&cp).unwrap();let l=&mut forged["world"]["member_activity"];
            match case {"orphan"=>{l["last_ordinal"]=json!(1);l["active"]=json!([{"id":1,"owner":1,"kind":"service_work","members":[{"start":0,"end":1},null],"started":0,"settled_through":0,"start_operation_id":1}]);},
                "unowned-history"=>l["last_ordinal"]=json!(1),"wrong-epoch"=>l["epoch"]=json!(2),_=>unreachable!()}
            let bytes=serde_json::to_vec(&forged).unwrap();fs::write(path.join(format!("{case}.json")),&bytes).unwrap();assert!(Engine::restore_checkpoint(&bytes).is_err(),"accepted {case}");
        }
        let expected=state(&s);drop(s);probe(&path,&world,&expected,&enabled,true);probe(&path,&world,&expected,&enabled,false);
        let mut slot=EngineSlot::new().unwrap();slot.restore_into(&cp).unwrap();assert!(slot.engine().unwrap().member_activity_registration().is_some());
        let legacy=Engine::new(assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![]).unwrap();slot.restore_into(&legacy.export_checkpoint().unwrap()).unwrap();assert!(slot.engine().unwrap().member_activity_registration().is_none());
    }
}
#[test]
fn education_borrows_one_ledger_and_keeps_active_consumption_across_store_reopen() {
    let path=directory("education-shared");let bytes=input(true);fs::write(path.join("opening.json"),&bytes).unwrap();
    let old=Engine::new(assembly::assemble(INPUT).unwrap(),resources(true)).unwrap();
    let new=Engine::new(assembly::assemble(&bytes).unwrap(),resources(true)).unwrap();assert_eq!(new.export_checkpoint().unwrap(),old.export_checkpoint().unwrap());
    let world=path.join("world");let mut s=Store::create(&world,&bytes,resources(true),RULES).unwrap();let registration=s.engine().unwrap().member_activity_registration();
    apply(&mut s,Caller::System,Action::Timeline{command:Timeline::Enable});apply(&mut s,Caller::Account(3),Action::Venue{use_id:1,command:Venue::Accept});
    let token=s.engine().unwrap().world().view().venue.unwrap().uses[0].token.number;
    apply(&mut s,Caller::System,Action::Venue{use_id:1,command:Venue::Enter{event_id:1000,token_number:token,occurred_at:0}});advance(&mut s,1);
    event(&mut s,er::Action::ActivateNoInstruction);let begun=event(&mut s,er::Action::BeginPreparation);advance(&mut s,61);
    let settled=event(&mut s,er::Action::SettlePreparation{started_event_id:begun.operation_id,close:false});
    let member=s.engine().unwrap().member_activity_summary().unwrap();assert_eq!(member.active_count,1);assert_eq!(member.next_ordinal,Some(3));assert_eq!(s.engine().unwrap().member_activity_registration(),registration);
    let expected=state(&s);drop(s);probe(&path,&world,&expected,&settled,true);probe(&path,&world,&expected,&settled,false);
    let mut s=Store::open(&world,RULES).unwrap();advance(&mut s,121);event(&mut s,er::Action::SettlePreparation{started_event_id:begun.operation_id,close:true});
    let member=s.engine().unwrap().member_activity_summary().unwrap();assert_eq!(member.active_count,0);assert_eq!(member.next_ordinal,Some(4));assert_eq!(s.engine().unwrap().member_activity_registration(),registration);s.checkpoint().unwrap();
}
#[test]
fn member_opening_rejects_false_namespace_and_unsupported_registration() {
    let path=directory("registration-gates");let good=input(false);drop(Store::create(&path.join("valid"),&good,vec![],RULES).unwrap());
    for case in ["version","epoch","scope","population","people","no-housing"] {
        let mut raw:Value=serde_json::from_slice(&good).unwrap();match case {
            "version"=>raw["member_activity"]["version"]=json!(2),"epoch"=>raw["member_activity"]["epoch"]=json!(2),
            "scope"=>raw["member_activity"]["scope"]["world"]=json!(2),"population"=>{
                let old=raw["member_activity"]["population_origin"][0].as_u64().unwrap();raw["member_activity"]["population_origin"][0]=json!(old^1);
            },
            "people"=>raw["member_activity"]["total_people"]=json!(11),"no-housing"=>{raw.as_object_mut().unwrap().remove("housing");},_=>unreachable!()}
        let bytes=serde_json::to_vec(&raw).unwrap();fs::write(path.join(format!("{case}.json")),&bytes).unwrap();assert!(Store::create(&path.join(case),&bytes,vec![],RULES).is_err(),"accepted {case}");assert!(!path.join(case).join("CURRENT.json").exists());
    }
}
