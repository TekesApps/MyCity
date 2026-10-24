//! Real Housing acceptance -> original Timeline -> original Store; no service delivery claim.
use mycity_core::{assembly, housing::{GenesisLeaseOffer,LeaseTerms,lease_terms_sha256,genesis_lease_offer_sha256},
    service_needs as n,timeline::Command as T,transaction::{Action,Caller,Envelope,Engine,EngineSlot},units::{GameTime,Money}};
use mycity_storage::Store;
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},process::{Command,Stdio},sync::atomic::{AtomicU64,Ordering},time::{Duration,Instant}};
const RULES:&str="service-natural-needs-v1";
const BASE:&[u8]=include_bytes!("fixtures/education-store/free-offline-v1.json");
static NEXT:AtomicU64=AtomicU64::new(0);
fn dir(name:&str)->PathBuf{
    let root=fs::canonicalize(std::env::var_os("MYCITY_SERVICE_TEST_WORK").expect("external evidence root")).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir(&p).unwrap();println!("RETAINED_SERVICE_WORLD={}",p.display());p
}
fn offer(at:u64)->GenesisLeaseOffer{
    let terms=LeaseTerms{lease_id:901,clause_version:1,contract_source_id:9101,landlord_authorization_id:9201,
        tenant_consent_id:9301,rent_per_cycle:Money::new(1000).unwrap(),cycle:mycity_core::timeline::Cycle::Monthly{anchor:1},
        deposit:Money::ZERO,escrow_account:3,starts_at:GameTime::new(at as i64).unwrap()};
    GenesisLeaseOffer{offer_id:901,world_id:1,branch_id:1,unit_id:71,building_id:70,owner_account:1,tenant_account:2,
        household_id:7,valid_from:GameTime::ZERO,valid_until:GameTime::new((400*n::DAY) as i64).unwrap(),asset_source_id:7002,
        authority_source_id:8001,contract_source_id:terms.contract_source_id,landlord_authorization_id:terms.landlord_authorization_id,
        tenant_consent_id:terms.tenant_consent_id,lease_id:terms.lease_id,clause_version:1,rent_per_cycle:terms.rent_per_cycle,
        cycle:terms.cycle,deposit:Money::ZERO,escrow_account:3,starts_at:terms.starts_at,terms_sha256:lease_terms_sha256(terms).unwrap()}
}
fn input(at:u64,enabled:bool)->Vec<u8>{
    let mut v:Value=serde_json::from_slice(BASE).unwrap();v.as_object_mut().unwrap().remove("education");v.as_object_mut().unwrap().remove("venue");
    v["bootstrap"]["seed"]=json!(107);v["housing"]["city_id"]=json!(1);v["housing"]["lease_offers"]=json!([offer(at)]);
    if !enabled{v["housing"].as_object_mut().unwrap().remove("city_id");}
    if enabled{v["service_needs"]=json!({"version":1,"profile":"LocalDevelopmentNaturalNeedsV1","scope":{"world":1,"branch":1},"seed":107,"city_id":1,
        "households":[{"household_id":7,"source_id":7001,"generation":1,"people":10}]});}
    serde_json::to_vec(&v).unwrap()
}
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope{
    let(scope,revision)=s.engine().unwrap().world().scope_revision();
    let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]};
    let out=s.execute(caller,e.clone()).unwrap();assert!(out.has_persistent_receipt&&out.model.is_ok(),"{out:?}");e
}
fn advance(s:&mut Store,target:u64){
    for _ in 0..8192{
        let p=s.engine().unwrap().timeline().unwrap().progress;if p.last_complete==target&&p.target.is_none(){return;}
        let limit=s.continuation_status().unwrap().map_or(256,|reserved|reserved.page_limit);
        apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit}});
    }panic!("original Timeline failed to drain");
}
fn state(s:&Store)->Value{json!({"hash":s.state_hash().unwrap(),"service":s.engine().unwrap().service_needs_summary(),
    "registration":s.engine().unwrap().service_needs_registration(),"timeline":s.engine().unwrap().timeline().unwrap().progress})}
fn probe(path:&Path,world:&Path,expected:&Value,replay:&Envelope,checkpoint:bool){
    let p=path.join(format!("fresh-{}",NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();
    let file=p.join("probe.json");fs::write(&file,serde_json::to_vec(&json!({"world":world,"expected":expected,"replay":replay,"checkpoint":checkpoint})).unwrap()).unwrap();
    fs::write(p.join("launch.json"),serde_json::to_vec(&json!({"timeout_seconds":60,"functional_only":true})).unwrap()).unwrap();
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","service_store_child","--ignored","--nocapture"])
        .env("MYCITY_SERVICE_PROBE",&file).stdout(Stdio::from(fs::File::create(p.join("stdout.log")).unwrap()))
        .stderr(Stdio::from(fs::File::create(p.join("stderr.log")).unwrap())).spawn().unwrap();
    fs::write(p.join("start.json"),serde_json::to_vec(&json!({"pid":child.id()})).unwrap()).unwrap();
    let start=Instant::now();let mut timed_out=false;let status=loop{
        if let Some(status)=child.try_wait().unwrap(){break status;}
        if start.elapsed()>Duration::from_secs(60){timed_out=true;child.kill().unwrap();break child.wait().unwrap();}
        std::thread::sleep(Duration::from_millis(20));
    };
    fs::write(p.join("exit.json"),serde_json::to_vec(&json!({"pid":child.id(),"code":status.code(),"success":status.success(),"timed_out":timed_out})).unwrap()).unwrap();
    assert!(status.success()&&!timed_out,"fresh probe {}",p.display());
}
#[test]#[ignore]
fn service_store_child(){
    let file=std::env::var_os("MYCITY_SERVICE_PROBE").expect("explicit probe");let spec:Value=serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    let mut s=Store::open(Path::new(spec["world"].as_str().unwrap()),RULES).unwrap();assert!(s.engine().unwrap().service_needs_is_authenticated());assert_eq!(state(&s),spec["expected"]);
    let before=s.committed_sequence();let e:Envelope=serde_json::from_value(spec["replay"].clone()).unwrap();
    let out=s.execute(Caller::System,e).unwrap();assert!(out.has_persistent_receipt&&out.model.is_ok());assert_eq!(s.committed_sequence(),before);assert_eq!(state(&s),spec["expected"]);
    if spec["checkpoint"].as_bool().unwrap(){s.checkpoint().unwrap();}assert_eq!(state(&s),spec["expected"]);
}
#[test]
fn real_lease_due_deadline_partial_p5_and_cold_recovery(){
    for spatial in [false,true]{
        let path=dir(if spatial{"spatial"}else{"classic"});let world=path.join("world");let bytes=input(0,true);fs::write(path.join("opening.json"),&bytes).unwrap();
        let mut spatial_input:Value=serde_json::from_slice(include_bytes!("../../../fixtures/m0-exchange-spatial.json")).unwrap();
        spatial_input["bootstrap"]["seed"]=json!(107);
        let spatial_input=serde_json::to_vec(&spatial_input).unwrap();fs::write(path.join("spatial-opening.json"),&spatial_input).unwrap();
        let mut s=if spatial{Store::create_spatial_v2(&world,&bytes,vec![],RULES,&spatial_input,include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap()}
            else{Store::create(&world,&bytes,vec![],RULES).unwrap()};
        let h=n::Household{household_id:7,source_id:7001,generation:1,people:10};
        assert_eq!(n::stable_token(mycity_core::arena::Scope::new(1,1).unwrap(),h),3_571_304_955_809_615_142);
        apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});
        assert!(s.engine().unwrap().service_needs_summary().unwrap().households[0].entry.is_none());
        let accepted=apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:genesis_lease_offer_sha256(offer(0))});
        let entry=s.engine().unwrap().service_needs_summary().unwrap().households[0].entry.unwrap();assert_eq!(entry.at,0);assert_eq!(entry.operation_id,accepted.operation_id);
        let partial=loop{
            let e=apply(&mut s,Caller::System,Action::Timeline{command:T::Advance{target:n::EIGHT_AM,limit:1}});
            if s.engine().unwrap().service_needs_summary().unwrap().pending.is_some(){break e;}
        };
        let expected=state(&s);assert!(s.engine().unwrap().timeline().unwrap().progress.last_complete<n::EIGHT_AM);
        fs::write(path.join("partial-checkpoint.json"),s.engine().unwrap().export_checkpoint().unwrap()).unwrap();
        fs::write(path.join("layout.json"),serde_json::to_vec_pretty(&s.engine().unwrap().world().layout()).unwrap()).unwrap();
        drop(s);probe(&path,&world,&expected,&partial,true);probe(&path,&world,&expected,&partial,false);
        let mut s=Store::open(&world,RULES).unwrap();advance(&mut s,n::EIGHT_AM);
        let summary=s.engine().unwrap().service_needs_summary().unwrap();assert!(summary.households[0].current[1].is_waiting());assert_eq!(summary.households[0].summaries[1].generated,1);
        // No service supplier, admission, money transfer or fake completed work exists.
        let before:Value=serde_json::from_slice(&s.engine().unwrap().export_checkpoint().unwrap()).unwrap();
        let before=before["world"]["accounts"].clone();
        advance(&mut s,n::DAY+n::EIGHT_AM);let summary=s.engine().unwrap().service_needs_summary().unwrap();
        assert!(summary.households[0].current[1].is_expired());assert_eq!(summary.households[0].summaries[1].expired,1);let after:Value=serde_json::from_slice(&s.engine().unwrap().export_checkpoint().unwrap()).unwrap();assert_eq!(before,after["world"]["accounts"]);
        let replay=apply(&mut s,Caller::System,Action::Timeline{command:T::Advance{target:n::DAY+n::EIGHT_AM+1,limit:256}});
        advance(&mut s,n::DAY+n::EIGHT_AM+1);let expected=state(&s);s.checkpoint().unwrap();drop(s);
        probe(&path,&world,&expected,&replay,true);probe(&path,&world,&expected,&replay,false);
        let mut s=Store::open(&world,RULES).unwrap();advance(&mut s,181*n::DAY+n::EIGHT_AM);
        let row=s.engine().unwrap().service_needs_summary().unwrap().households[0];
        assert_eq!(row.summaries.map(|s|s.generated),[2,2,1]);assert_eq!(row.summaries.map(|s|s.expired),[2,2,1]);
        assert_eq!(row.entry,Some(entry));assert_eq!(row.current[1].occurrence_day,180);s.checkpoint().unwrap();
    }
}
#[test]
fn failed_lease_does_not_enter_and_exact_eight_am_entry_skips_equal_occurrence(){
    let path=dir("entry-boundary");let world=path.join("world");let bytes=input(n::EIGHT_AM,true);
    let mut s=Store::create(&world,&bytes,vec![],RULES).unwrap();apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});
    let action=Action::HousingLease{offer_id:901,offer_sha256:genesis_lease_offer_sha256(offer(n::EIGHT_AM))};
    let(scope,revision)=s.engine().unwrap().world().scope_revision();let e=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]};
    let out=s.execute(Caller::Account(3),e).unwrap();assert!(out.model.is_err());assert!(s.engine().unwrap().service_needs_summary().unwrap().households[0].entry.is_none());
    advance(&mut s,n::EIGHT_AM);let accepted=apply(&mut s,Caller::Account(2),action);let entry=s.engine().unwrap().service_needs_summary().unwrap().households[0].entry;
    let seq=s.committed_sequence();let out=s.execute(Caller::Account(2),accepted).unwrap();assert!(out.model.is_ok());assert_eq!(s.committed_sequence(),seq);
    advance(&mut s,n::DAY+n::EIGHT_AM);let row=s.engine().unwrap().service_needs_summary().unwrap().households[0];
    assert!(row.current[1].is_empty());assert_eq!(row.next_days[1],Some(180));assert_eq!(row.entry,entry);s.checkpoint().unwrap();
}
#[test]
fn legacy_disabled_registration_gates_and_direct_restore_cannot_self_authenticate_entry(){
    let path=dir("gates");let good=input(0,true);let mut s=Store::create(&path.join("valid"),&good,vec![],RULES).unwrap();
    apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:genesis_lease_offer_sha256(offer(0))});
    let cp=s.engine().unwrap().export_checkpoint().unwrap();let restored=Engine::restore_checkpoint(&cp).unwrap();assert!(!restored.service_needs_is_authenticated());
    let mut slot=EngineSlot::new().unwrap();slot.restore_into(&cp).unwrap();assert!(slot.engine().unwrap().service_needs_registration().is_some());
    let legacy=Engine::new(assembly::assemble(&input(0,false)).unwrap(),vec![]).unwrap();slot.restore_into(&legacy.export_checkpoint().unwrap()).unwrap();assert!(slot.engine().unwrap().service_needs_registration().is_none());
    let mut bare:Value=serde_json::from_slice(&good).unwrap();bare.as_object_mut().unwrap().remove("service_needs");
    assert!(assembly::assemble(&serde_json::to_vec(&bare).unwrap()).err().unwrap().contains("housing_city"));
    for case in ["city","no-city","seed","people","source","generation","scope","version","unknown-household"]{
        let mut v:Value=serde_json::from_slice(&good).unwrap();match case{
            "city"=>v["service_needs"]["city_id"]=json!(2),"no-city"=>{v["housing"].as_object_mut().unwrap().remove("city_id");},
            "seed"=>v["service_needs"]["seed"]=json!(108),"people"=>v["service_needs"]["households"][0]["people"]=json!(9),
            "source"=>v["service_needs"]["households"][0]["source_id"]=json!(7003),"generation"=>v["service_needs"]["households"][0]["generation"]=json!(2),
            "scope"=>v["service_needs"]["scope"]["branch"]=json!(2),"version"=>v["service_needs"]["version"]=json!(2),
            "unknown-household"=>v["service_needs"]["households"][0]["household_id"]=json!(8),_=>unreachable!()}
        let bytes=serde_json::to_vec(&v).unwrap();fs::write(path.join(format!("{case}.json")),&bytes).unwrap();assert!(Store::create(&path.join(case),&bytes,vec![],RULES).is_err(),"accepted {case}");
    }
}
