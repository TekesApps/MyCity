//! Actual authority tests: original teaching, original Store tickets, one grant state.
use mycity_core::{arena::Scope, course_completion::{self as c, GroupResult}, education as e,
    education_authority::{self as a,Command as Education}, education_runtime as er,
    member_qualification::{Background,Skill,BaseQualification,MemberRange}, qualification_authority::{self as q,Command as Q},
    qualification_state as f, timeline::Command as T, transaction::{Action,Caller,Envelope,Engine,EngineSlot,Resource},
    units::{GameTime,Money}, venue_runtime::Command as V};
use mycity_storage::{Store,WritePoint};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io,path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering},process::{Command,Stdio},time::{Duration,Instant}};
const RULES:&str="actual-free-course-qualification-v2";
const DAY:u64=86400;
static NEXT:AtomicU64=AtomicU64::new(0);
fn time(n:u64)->GameTime {GameTime::new(n.try_into().unwrap()).unwrap()}
fn id(n:u64)->[u8;16] {let mut out=[0;16];out[8..].copy_from_slice(&n.to_be_bytes());out}
fn directory(name:&str)->PathBuf {
    let root=fs::canonicalize(std::env::var_os("MYCITY_QUALIFICATION_TEST_WORK").unwrap()).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir(&p).unwrap();println!("RETAINED_QUALIFICATION_WORLD={}",p.display());p
}
fn input()->Vec<u8> {
    // New dedicated legal profile. Original teaching/M0 fixture bytes remain unchanged.
    let mut raw:Value=serde_json::from_slice(include_bytes!("fixtures/education-store/free-offline-v1.json")).unwrap();
    for row in raw["education"]["courses"].as_array_mut().unwrap().iter_mut().filter(|r|!r.is_null()) {
        row["qualification"]["valid_until"]=json!(200*DAY);
    }
    raw["venue"]["offers"][0]["window"]["end"]=json!(200*DAY);
    let edu:er::Opening=serde_json::from_value(raw["education"].clone()).unwrap();let reg=edu.courses[0].unwrap();
    let mut h=Sha256::new();h.update(b"qualification-original-roster-v1\0");h.update(serde_json::to_vec(&(reg.enrollment,reg.learners,reg.course)).unwrap());
    let roster=h.finalize().into();let scope=Scope::new(1,1).unwrap();let origin=edu.population_origin;
    let p=c::Policy{version:1,profile:c::Profile::FreeElectricWaterV1,reference:9001,issuer:1,charter_ref:8001,authorization_ref:8001,
        course_ref:reg.course.reference,course_version:1,result:BaseQualification::Electric,free_contract_ref:reg.free_contract.contract_ref,
        free_entitlement_ref:reg.free_contract.source_ref,exam_rule_ref:9002,exam_rule_version:1,syllabus:1023,
        required_background:Background::General,required_skill:Skill::Technical,valid_from:time(0),valid_until:time(200*DAY)};
    let facts=f::Opening{version:1,profile:f::Profile::ImmutableGameOpeningV1,scope,member_version:1,population_origin:origin,total_people:10,
        authority:f::OpeningAuthority{id:id(8001),scope,population_origin:origin,member_version:1,profile:f::Profile::ImmutableGameOpeningV1,
            issued_at:time(0),valid_until:time(200*DAY),permitted_results:2},
        issuers:[Some(f::IssuerRegistration{id:id(1),authority_id:id(8001),scope,population_origin:origin,member_version:1,
            valid_from:time(0),valid_until:time(200*DAY),permitted_results:2}),None,None,None],
        baseline_sources:std::array::from_fn(|i|if i==0 {Some(f::BaselineSourceRegistration{id:id(8002),authority_id:id(8001),scope,
            population_origin:origin,member_version:1,valid_from:time(0),valid_until:time(200*DAY)})}else{None}),
        rules:[Some(f::RuleRegistration{id:id(9002),issuer_id:id(1),course:f::Course::Electric,syllabus_mask:1023,minimum_covered_parts:9}),None,None,None],
        baselines:std::array::from_fn(|i|if i==0 {Some(f::BaselineFact{members:MemberRange{start:0,end:2},source_id:id(8002),
            base_known:true,base_mask:0,background:Some(Background::General),skill:Some(Skill::Technical)})}else{None}),
        course_results:[Some(f::CourseResultRegistration{course:f::Course::Electric,cohort_id:reg.enrollment.cohort,generation:reg.enrollment.generation,
            members:MemberRange{start:0,end:2},roster_hash:roster,rule_id:id(9002),issuer_id:id(1),free_entitlement_id:id(reg.free_contract.source_ref)}),None,None,None]};
    raw["qualification"]=serde_json::to_value(q::Opening{protocol_version:2,facts,policies:[Some(p),None,None,None]}).unwrap();
    serde_json::to_vec(&raw).unwrap()
}
fn create(path:&Path)->Store {
    let bytes=input();fs::write(path.join("opening.json"),&bytes).unwrap();
    let s=Store::create(&path.join("world"),&bytes,vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES).unwrap();
    fs::write(path.join("layout.json"),serde_json::to_vec_pretty(&s.engine().unwrap().world().layout()).unwrap()).unwrap();s
}
fn create_v2(path:&Path)->Store {
    let bytes=input();fs::write(path.join("opening.json"),&bytes).unwrap();
    Store::create_spatial_v2(&path.join("world"),&bytes,
        vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap()
}
fn env(s:&Store,action:Action)->Envelope {
    let (scope,revision)=s.engine().unwrap().world().scope_revision();
    Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]}
}
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope {
    let input=env(s,action);let out=s.execute(caller,input.clone()).unwrap();assert!(out.has_persistent_receipt&&out.model.is_ok(),"{out:?}");input
}
fn reject(s:&mut Store,caller:Caller,action:Action) {
    let old=s.engine().unwrap().business_comparison_checkpoint().unwrap();let input=env(s,action);let op=input.operation_id;let out=s.execute(caller,input).unwrap();
    assert!(out.model.is_err(),"{out:?}");
    if let Some(ticket)=s.receipt(op).unwrap(){assert!(ticket.qualification_context().is_none());}
    assert_eq!(s.engine().unwrap().business_comparison_checkpoint().unwrap(),old);
}
fn event(s:&Store,action:er::Action)->Action {
    let view=s.engine().unwrap().world().view().education.unwrap();
    Action::Education{command:Education::Event{expected_epoch:view.stream_epoch,expected_sequence:view.consumed_sequence+1,command:er::Command{cohort:1,action},actual:None}}
}
fn advance(s:&mut Store,target:u64) {
    for _ in 0..1024 {
        let p=s.engine().unwrap().timeline().unwrap().progress;if p.target.is_none()&&p.last_complete==target{return;}
        apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}});
    }panic!("bounded Timeline did not finish");
}
fn setup(s:&mut Store) {
    apply(s,Caller::System,Action::Timeline{command:T::Enable});apply(s,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});
    let token=s.engine().unwrap().world().view().venue.unwrap().uses[0].token.number;
    apply(s,Caller::System,Action::Venue{use_id:1,command:V::Enter{event_id:1000,token_number:token,occurred_at:0}});
    advance(s,1);let action=event(s,er::Action::ActivateNoInstruction);apply(s,Caller::System,action);
}
fn exam(s:&Store,attempt:u32,content:u32,result:GroupResult)->Action {
    let op=s.committed_sequence()+1;let record=s.engine().unwrap().qualification_examination_input(0,attempt,op,op,content,result).unwrap();
    Action::Qualification{command:Q::Examination{slot:0,attempt,expected_root:record.source().root,progress_binding:record.examination().progress_binding,content,result}}
}
fn complete(s:&Store)->Action {Action::Qualification{command:Q::Complete{slot:0,expected_root:s.engine().unwrap().qualification_source().unwrap().root,
    examination:s.engine().unwrap().qualification_examination_anchor(0).unwrap()}}}
fn job(s:&Store,endorsement:u64)->q::JobCheck {
    s.engine().unwrap().check_job_qualification(q::JobRequirements{members:MemberRange{start:0,end:2},minimum_background:Some(Background::General),
        minimum_skill:Some(Skill::Technical),required_base_mask:2,required_endorsement_mask:endorsement}).unwrap()
}
fn compact(s:&mut Store,prior:u64) {
    let view=s.engine().unwrap().world().view().education.unwrap();let original=s.receipt(prior).unwrap().unwrap();
    let anchor=er::ReceiptAnchor{store_ref:prior,root:a::receipt_digest(&original).unwrap(),epoch:view.stream_epoch,through:view.consumed_sequence};
    apply(s,Caller::System,Action::Education{command:Education::Compact{expected_runtime:view.fingerprint,anchor}});
}
fn teach_days(s:&mut Store)->u64 {
    let mut cursor=1;let mut last=0;
    for day in 0..60 {
        let start=day*DAY+1;
        if cursor<start {advance(s,start);let action=event(s,er::Action::NoInstruction{window:e::Window{start:time(cursor),end:time(start)}});apply(s,Caller::System,action);}
        let action=event(s,er::Action::BeginPreparation);let prepared=apply(s,Caller::System,action);
        advance(s,start+7200);let action=event(s,er::Action::SettlePreparation{started_event_id:prepared.operation_id,close:true});apply(s,Caller::System,action);
        let action=event(s,er::Action::BeginLesson);let lesson=apply(s,Caller::System,action);
        advance(s,start+14400);let action=event(s,er::Action::SettleLesson{started_event_id:lesson.operation_id,close:true});last=apply(s,Caller::System,action).operation_id;
        cursor=start+14400;if day%2==1 {compact(s,last);}
    }
    let progress=s.engine().unwrap().world().view().education.unwrap().progress[0].unwrap().1;
    assert_eq!(progress.actual_seconds_per_learner,120*3600);assert_eq!(progress.actual_teaching_days,60);assert_eq!(progress.fee_cents_per_learner,Money::ZERO);last
}
#[test]
fn original_teaching_exam_grant_replay_revoke_and_old_exam_substitution() {
    let path=directory("real-course");let world=path.join("world");let mut s=create_v2(&path);setup(&mut s);
    assert_eq!(job(&s,0).status,q::JobStatus::Ineligible);
    let invalid=exam(&s,1,0,GroupResult::Fail);reject(&mut s,Caller::Account(3),invalid);
    let action=exam(&s,1,0,GroupResult::Fail);apply(&mut s,Caller::System,action);let action=complete(&s);reject(&mut s,Caller::System,action);
    teach_days(&mut s);
    let action=exam(&s,2,1023,GroupResult::Pass);apply(&mut s,Caller::System,action);
    let action=complete(&s);reject(&mut s,Caller::System,action); // 120h alone cannot pass90 calendar days.
    advance(&mut s,90*DAY+1);let action=exam(&s,3,0,GroupResult::Pass);apply(&mut s,Caller::System,action);
    let action=complete(&s);reject(&mut s,Caller::System,action); // real zero-coverage Pass retained, not a credential.
    advance(&mut s,90*DAY+2);let action=exam(&s,4,1023,GroupResult::Pass);let earlier=apply(&mut s,Caller::System,action);
    let old_checkpoint:Value=serde_json::from_slice(&s.engine().unwrap().export_checkpoint().unwrap()).unwrap();
    advance(&mut s,90*DAY+3);let action=exam(&s,5,1023,GroupResult::Pass);let latest=apply(&mut s,Caller::System,action);
    let original=env(&s,complete(&s));
    let lost=s.execute_with_observer(Caller::System,original.clone(),|point|if point==WritePoint::CurrentRenamed {Err(io::Error::other("qualification response lost"))}else{Ok(())});
    assert!(lost.is_err());drop(s);let mut s=Store::open(&world,RULES).unwrap();assert!(s.engine().unwrap().qualification_is_authenticated());
    assert_eq!(job(&s,0).status,q::JobStatus::Eligible);assert_eq!(job(&s,1).status,q::JobStatus::Unknown);
    let sequence=s.committed_sequence();let hash=s.state_hash().unwrap();assert!(s.execute(Caller::System,original.clone()).unwrap().model.is_ok());
    assert_eq!(s.committed_sequence(),sequence);assert_eq!(s.state_hash().unwrap(),hash);
    drop(s);probe(&path,&world,&hash,&original,q::JobStatus::Eligible,false);
    let mut s=Store::open(&world,RULES).unwrap();s.checkpoint().unwrap();let hash=s.state_hash().unwrap();drop(s);
    fs::write(path.join("unreachable-original.json"),b"{}").unwrap();fs::copy(path.join("unreachable-original.json"),world.join("world-999999.json")).unwrap();
    probe(&path,&world,&hash,&original,q::JobStatus::Eligible,true);assert!(!world.join("world-999999.json").exists());
    let mut s=Store::open(&world,RULES).unwrap();
    let action=complete(&s);reject(&mut s,Caller::System,action);
    let action=event(&s,er::Action::BeginPreparation);reject(&mut s,Caller::System,action);
    let action=event(&s,er::Action::BeginLesson);reject(&mut s,Caller::System,action);
    // Both original Pass tickets are real. A forged checkpoint must not switch which
    // examination the original Complete consumed, even if each ticket is authentic.
    let valid=s.engine().unwrap().export_checkpoint().unwrap();let mut forged:Value=serde_json::from_slice(&valid).unwrap();
    let old=old_checkpoint["world"]["qualification"]["state"]["slots"][0]["latest_exam"].clone();
    let slot=&mut forged["world"]["qualification"]["state"]["slots"][0];slot["latest_exam"]=old.clone();
    slot["grant"]["exam_source"]=old["source"].clone();slot["grant"]["progress_binding"]=old["progress_binding"].clone();
    let forged=serde_json::to_vec(&forged).unwrap();fs::write(path.join("forged-older-exam-checkpoint.json"),&forged).unwrap();
    let mut target=EngineSlot::new().unwrap();target.restore_into(&forged).unwrap();assert!(!target.engine().unwrap().qualification_is_authenticated());
    let operations=target.engine().unwrap().qualification_receipt_operations().unwrap();assert_eq!(operations[0],Some(earlier.operation_id));
    let mut check=target.engine().unwrap().begin_qualification_restore().unwrap().unwrap();
    let exam_ticket=s.receipt(operations[0].unwrap()).unwrap().unwrap();target.engine().unwrap().check_qualification_restore_receipt(&mut check,0,&exam_ticket).unwrap();
    // Context alone cannot relabel an authentic compact request. Preserve the
    // original receipt and checkpoint; mutate only a detached test copy.
    let mut tampered=serde_json::to_value(exam_ticket).unwrap();
    tampered["qualification_context"]["record"]["source"]["root"]=json!([9;32].to_vec());
    let tampered=serde_json::from_value(tampered).unwrap();
    let mut bad_check=target.engine().unwrap().begin_qualification_restore().unwrap().unwrap();
    assert!(target.engine().unwrap().check_qualification_restore_receipt(&mut bad_check,0,&tampered).is_err());
    let grant_ticket=s.receipt(operations[1].unwrap()).unwrap().unwrap();assert!(target.engine().unwrap().check_qualification_restore_receipt(&mut check,1,&grant_ticket).is_err());
    assert!(!target.engine().unwrap().qualification_is_authenticated());
    fs::write(path.join("legitimate-completed-checkpoint.json"),valid).unwrap();
    let source=s.engine().unwrap().qualification_source().unwrap();let revoked=apply(&mut s,Caller::System,Action::Qualification{command:Q::Revoke{slot:0,expected_root:source.root}});
    assert_eq!(job(&s,0).status,q::JobStatus::Ineligible);s.checkpoint().unwrap();let hash=s.state_hash().unwrap();drop(s);
    let mut s=Store::open(&world,RULES).unwrap();assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(job(&s,0).status,q::JobStatus::Ineligible);
    assert_eq!(s.engine().unwrap().world().view().accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_000_000);
    assert!(s.engine().unwrap().owned_capacity_bytes()<=mycity_core::transaction::ENGINE_BYTE_BUDGET);drop(s);
    probe(&path,&world,&hash,&revoked,q::JobStatus::Ineligible,false);
    let source=fs::read_dir(&world).unwrap().map(Result::unwrap).find(|entry|{
        let name=entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("receipts-")||!name.ends_with(".json"){return false;}
        let value:Value=serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
        value["receipts"].as_array().is_some_and(|rows|rows.iter().any(|row|row["operation_id"]==latest.operation_id))
    }).expect("original latest-examination segment");
    let name=source.file_name();fs::copy(source.path(),path.join("original-examination-segment.json")).unwrap();
    let missing=path.join("missing-original-world");copy_world(&world,&missing);
    drop(Store::open(&missing,RULES).unwrap());fs::remove_file(missing.join(&name)).unwrap();assert!(Store::open(&missing,RULES).is_err());
    let corrupt=path.join("corrupt-original-world");copy_world(&world,&corrupt);
    drop(Store::open(&corrupt,RULES).unwrap());fs::write(corrupt.join(&name),b"{\"corrupt_original_examination\":true}").unwrap();assert!(Store::open(&corrupt,RULES).is_err());
    let mut original=Store::open(&world,RULES).unwrap();assert_eq!(original.state_hash().unwrap(),hash);assert_eq!(job(&original,0).status,q::JobStatus::Ineligible);
}
#[test]
fn mismatched_immutable_registration_cannot_publish_and_reused_legacy_slot_clears_qualification() {
    let path=directory("opening-gates");let good=input();let mut bad:Value=serde_json::from_slice(&good).unwrap();
    bad["qualification"]["policies"][0]["course_ref"]=json!(999999);
    let mut old_protocol:Value=serde_json::from_slice(&input()).unwrap();old_protocol["qualification"].as_object_mut().unwrap().remove("protocol_version");
    assert!(mycity_core::assembly::assemble(&serde_json::to_vec(&old_protocol).unwrap()).is_err());
    assert!(Store::create(&path.join("bad"),&serde_json::to_vec(&bad).unwrap(),vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES).is_err());
    assert!(!path.join("bad/CURRENT.json").exists());let s=create(&path);let checkpoint=s.engine().unwrap().export_checkpoint().unwrap();drop(s);
    let old=Engine::new(mycity_core::assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![]).unwrap();
    let mut slot=EngineSlot::new().unwrap();slot.restore_into(&checkpoint).unwrap();assert!(slot.engine().unwrap().world().view().qualification.is_some());
    slot.restore_into(&old.export_checkpoint().unwrap()).unwrap();assert!(slot.engine().unwrap().world().view().qualification.is_none());
}
#[test]
fn host_event_identity_is_original_operation_and_stale_sources_do_not_relabel_exams() {
    let path=directory("event-gates");let mut s=create(&path);setup(&mut s);let op=s.committed_sequence()+1;
    assert!(s.engine().unwrap().qualification_examination_input(0,1,op+1,op,1023,GroupResult::Pass).is_err());
    let action=exam(&s,1,1023,GroupResult::Pass);let original=apply(&mut s,Caller::System,action);
    advance(&mut s,2);let mut json=serde_json::to_value(exam(&s,2,1023,GroupResult::Pass)).unwrap();
    json["command"]["event_id"]=json!(original.operation_id);
    assert!(serde_json::from_value::<Action>(json).is_err());
    let mut wrong=serde_json::to_value(exam(&s,2,1023,GroupResult::Pass)).unwrap();
    wrong["command"]["progress_binding"]=json!([0;32].to_vec());
    reject(&mut s,Caller::System,serde_json::from_value(wrong).unwrap());
    let before=s.engine().unwrap().world().view().qualification.unwrap().fingerprint;
    let mut changed=original;changed.operation_id=s.committed_sequence()+1;changed.expected_revision=s.engine().unwrap().world().scope_revision().1;
    let out=s.execute(Caller::System,changed).unwrap();assert!(out.model.is_err());
    assert_eq!(before,s.engine().unwrap().world().view().qualification.unwrap().fingerprint);
}

fn copy_world(src:&Path,dst:&Path) {
    fs::create_dir(dst).unwrap();
    for entry in fs::read_dir(src).unwrap(){let entry=entry.unwrap();let to=dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir(){copy_world(&entry.path(),&to);}else{fs::copy(entry.path(),to).unwrap();}}
}
#[derive(Serialize,Deserialize)]
struct Probe {world:PathBuf,hash:String,envelope:Envelope,eligible:bool,checkpoint:bool,result:PathBuf}
fn probe(root:&Path,world:&Path,hash:&str,envelope:&Envelope,expected:q::JobStatus,checkpoint:bool) {
    let path=root.join(format!("fresh-{}",NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&path).unwrap();
    let input=Probe{world:world.to_owned(),hash:hash.into(),envelope:envelope.clone(),eligible:expected==q::JobStatus::Eligible,checkpoint,result:path.join("result.json")};
    let file=path.join("probe.json");fs::write(&file,serde_json::to_vec(&input).unwrap()).unwrap();
    fs::write(path.join("launch.json"),serde_json::to_vec(&json!({"timeout_seconds":60,"purpose":"fresh original Store reader; no performance claim"})).unwrap()).unwrap();
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","qualification_store_child","--ignored","--nocapture"])
        .env("MYCITY_QUALIFICATION_PROBE",&file).stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(path.join("stdout.txt")).unwrap())).stderr(Stdio::from(fs::File::create(path.join("stderr.txt")).unwrap())).spawn().unwrap();
    fs::write(path.join("start.json"),serde_json::to_vec(&json!({"pid":child.id()})).unwrap()).unwrap();let start=Instant::now();let mut timed_out=false;
    let status=loop{if let Some(status)=child.try_wait().unwrap(){break status;}
        if start.elapsed()>Duration::from_secs(60){timed_out=true;child.kill().unwrap();break child.wait().unwrap();}
        std::thread::sleep(Duration::from_millis(20));};
    fs::write(path.join("exit.json"),serde_json::to_vec(&json!({"pid":child.id(),"code":status.code(),"success":status.success(),"timed_out":timed_out})).unwrap()).unwrap();
    assert!(!timed_out&&status.success(),"fresh helper failed: {}",path.display());assert!(input.result.exists());
}
#[test]
#[ignore]
fn qualification_store_child() {
    let file=std::env::var_os("MYCITY_QUALIFICATION_PROBE").expect("explicit owned probe input");let p:Probe=serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    let mut s=Store::open(&p.world,RULES).unwrap();assert!(s.engine().unwrap().qualification_is_authenticated());assert_eq!(s.state_hash().unwrap(),p.hash);
    let status=if p.eligible{q::JobStatus::Eligible}else{q::JobStatus::Ineligible};assert_eq!(job(&s,0).status,status);
    let before=s.committed_sequence();let replay=s.execute(Caller::System,p.envelope).unwrap();assert!(replay.has_persistent_receipt&&replay.model.is_ok());
    assert_eq!(s.committed_sequence(),before);assert_eq!(s.state_hash().unwrap(),p.hash);
    if p.checkpoint{s.checkpoint().unwrap();}assert_eq!(job(&s,0).status,status);
    fs::write(p.result,serde_json::to_vec(&json!({"pid":std::process::id(),"hash":s.state_hash().unwrap(),"status":format!("{status:?}"),"replayed":true,"checkpoint":p.checkpoint})).unwrap()).unwrap();
}

/// Explicit new game Opening, not a credential inferred from simulated lessons.
fn baseline_input()->Vec<u8> {
    let mut raw:Value=serde_json::from_slice(&input()).unwrap();
    raw.as_object_mut().unwrap().remove("education");raw.as_object_mut().unwrap().remove("venue");
    let q=&mut raw["qualification"];q["protocol_version"]=json!(3);
    q["policies"]=json!([null,null,null,null]);
    for key in ["course_results","rules","issuers"] {q["facts"][key]=json!([null,null,null,null]);}
    q["facts"]["authority"]["permitted_results"]=json!(0);
    q["facts"]["baselines"][0]["base_known"]=json!(false);
    q["facts"]["baselines"][0]["base_mask"]=json!(BaseQualification::Electric.bit()|BaseQualification::Police.bit());
    serde_json::to_vec(&raw).unwrap()
}
fn baseline_roundtrip(spatial:bool) {
    let path=directory(if spatial{"baseline-v3-spatial"}else{"baseline-v3"});let world=path.join("world");let bytes=baseline_input();
    fs::write(path.join("opening.json"),&bytes).unwrap();
    let mut s=if spatial {Store::create_spatial_v2(&world,&bytes,vec![],RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap()}
        else {Store::create(&world,&bytes,vec![],RULES).unwrap()};
    assert!(s.engine().unwrap().world().view().education.is_none());
    assert!(s.engine().unwrap().world().view().venue.is_none());
    assert!(s.engine().unwrap().world().view().member_activity.is_none());
    assert!(s.engine().unwrap().qualification_source().is_err()); // No invented clock.
    let enabled=apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});
    let request=q::JobRequirements{members:MemberRange{start:0,end:2},minimum_background:Some(Background::General),minimum_skill:Some(Skill::Technical),
        required_base_mask:BaseQualification::Police.bit(),required_endorsement_mask:0};
    assert_eq!(s.engine().unwrap().check_job_qualification(request).unwrap().status,q::JobStatus::Eligible);
    assert_eq!(s.engine().unwrap().check_job_qualification(q::JobRequirements{required_base_mask:BaseQualification::Driver.bit(),..request}).unwrap().status,q::JobStatus::Unknown);
    assert_eq!(s.engine().unwrap().check_job_qualification(q::JobRequirements{required_endorsement_mask:1,..request}).unwrap().status,q::JobStatus::Unknown);
    assert!(s.engine().unwrap().qualification_examination_input(0,1,2,2,1023,GroupResult::Pass).is_err());
    let source=s.engine().unwrap().qualification_source().unwrap();
    reject(&mut s,Caller::System,Action::Qualification{command:Q::Revoke{slot:0,expected_root:source.root}});
    fs::write(path.join("layout.json"),serde_json::to_vec_pretty(&s.engine().unwrap().world().layout()).unwrap()).unwrap();
    let cp=s.engine().unwrap().export_checkpoint().unwrap();let unready=Engine::restore_checkpoint(&cp).unwrap();
    assert!(!unready.qualification_is_authenticated());assert!(unready.check_job_qualification(request).is_err());
    let hash=s.state_hash().unwrap();drop(s);
    probe(&path,&world,&hash,&enabled,q::JobStatus::Eligible,true);
    probe(&path,&world,&hash,&enabled,q::JobStatus::Eligible,false);
    let s=Store::open(&world,RULES).unwrap();assert!(s.engine().unwrap().qualification_is_authenticated());
    assert_eq!(s.engine().unwrap().check_job_qualification(request).unwrap().status,q::JobStatus::Eligible);
    let mut slot=EngineSlot::new().unwrap();slot.restore_into(&cp).unwrap();assert!(!slot.engine().unwrap().qualification_is_authenticated());
    let old=Engine::new(mycity_core::assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![]).unwrap();
    slot.restore_into(&old.export_checkpoint().unwrap()).unwrap();assert!(slot.engine().unwrap().world().view().qualification.is_none());
}
#[test]
fn baseline_only_without_teaching_or_venue_retains_original_store_authority() {baseline_roundtrip(false);}
#[test]
fn baseline_only_spatial_same_slot_recovery_does_not_trust_serde_ready() {baseline_roundtrip(true);}
#[test]
fn baseline_only_rejects_unbound_population_and_dormant_course_authority() {
    let path=directory("baseline-gates");let bytes=baseline_input();
    // A valid control reaches the real business path before testing counterexamples.
    drop(Store::create(&path.join("valid"),&bytes,vec![],RULES).unwrap());
    let original:Value=serde_json::from_slice(&input()).unwrap();let base:Value=serde_json::from_slice(&bytes).unwrap();
    for case in ["legacy-v2","future-version","no-housing","wrong-origin","wrong-count","wrong-scope","range","source","course","policy","issuer","rule","grant-mask"] {
        let mut bad=base.clone();match case {
            "legacy-v2"=>bad["qualification"]["protocol_version"]=json!(2),
            "future-version"=>bad["qualification"]["protocol_version"]=json!(4),
            "no-housing"=>{bad.as_object_mut().unwrap().remove("housing");},
            "wrong-origin"=>{
                let mut origin=bad["qualification"]["facts"]["population_origin"].clone();
                origin[0]=json!(origin[0].as_u64().unwrap()^1);
                bad["qualification"]["facts"]["population_origin"]=origin.clone();
                bad["qualification"]["facts"]["authority"]["population_origin"]=origin.clone();
                for source in bad["qualification"]["facts"]["baseline_sources"].as_array_mut().unwrap().iter_mut().filter(|x|!x.is_null()) {
                    source["population_origin"]=origin.clone();
                }
            },
            "wrong-count"=>bad["qualification"]["facts"]["total_people"]=json!(11),
            "wrong-scope"=>{
                let scope=serde_json::to_value(Scope::new(2,1).unwrap()).unwrap();
                bad["qualification"]["facts"]["scope"]=scope.clone();
                bad["qualification"]["facts"]["authority"]["scope"]=scope.clone();
                for source in bad["qualification"]["facts"]["baseline_sources"].as_array_mut().unwrap().iter_mut().filter(|x|!x.is_null()) {source["scope"]=scope.clone();}
            },
            "range"=>bad["qualification"]["facts"]["baselines"][0]["members"]["end"]=json!(11),
            "source"=>bad["qualification"]["facts"]["baselines"][0]["source_id"]=serde_json::to_value([0u8;16]).unwrap(),
            "course"=>bad["qualification"]["facts"]["course_results"]=original["qualification"]["facts"]["course_results"].clone(),
            "policy"=>bad["qualification"]["policies"]=original["qualification"]["policies"].clone(),
            "issuer"=>bad["qualification"]["facts"]["issuers"]=original["qualification"]["facts"]["issuers"].clone(),
            "rule"=>bad["qualification"]["facts"]["rules"]=original["qualification"]["facts"]["rules"].clone(),
            "grant-mask"=>bad["qualification"]["facts"]["authority"]["permitted_results"]=json!(2),
            _=>unreachable!(),
        }
        if matches!(case,"wrong-origin"|"wrong-scope") {
            // Self-consistent fake registry must pass pure State validation:
            // only the original World/Housing binding can reject these inputs.
            let facts:f::Opening=serde_json::from_value(bad["qualification"]["facts"].clone()).unwrap();facts.validate().unwrap();
        }
        let bytes=serde_json::to_vec(&bad).unwrap();fs::write(path.join(format!("{case}.json")),&bytes).unwrap();
        assert!(Store::create(&path.join(case),&bytes,vec![],RULES).is_err(),"accepted {case}");assert!(!path.join(case).join("CURRENT.json").exists());
    }
}
