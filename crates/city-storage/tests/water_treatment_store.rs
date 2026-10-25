//! Actual original V2 Start/dual-worker/zero-work exit receipt gates. These do
//! not replace the still-pending joint P5 material/funding/late-pay acceptance.
use mycity_core::{transaction::{Action,Caller,Envelope},timeline::Command as T,venue_runtime::Command as V,
    facility_authority::Command as F,continuous_supply::Command as S};
use mycity_storage::{Store,WritePoint};
use std::{fs,path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
#[path="../../../tests/fixtures/water-treatment-runtime/opening.rs"]mod fixture;
const RULES:&str="actual-water-treatment-runtime-v2";
fn dir()->PathBuf{
    let root=fs::canonicalize(std::env::var_os("MYCITY_WATER_TEST_ROOT").expect("explicit retained SSD root")).unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("water-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&p).unwrap();println!("RETAINED_WATER_WORLD={}",p.display());p
}
fn apply(s:&mut Store,caller:Caller,action:Action)->Envelope{
    let env=Envelope{world:1,branch:1,operation_id:s.committed_sequence()+1,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![action]};
    let out=s.execute(caller,env.clone()).unwrap();assert!(out.model.is_ok(),"{out:?}");env
}
fn create(p:&Path)->Store{
    create_input(p,fixture::input())
}
fn create_input(p:&Path,input:serde_json::Value)->Store{
    let raw=serde_json::to_vec(&input).unwrap();fs::write(p.join("opening.json"),&raw).unwrap();
    Store::create_spatial_v2(&p.join("world"),&raw,fixture::resources(),RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap()
}
fn advance(s:&mut Store,target:u64)->Envelope{
    for _ in 0..8192{
        let ticket=apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}});
        let p=s.engine().unwrap().timeline().unwrap().progress;
        if p.target.is_none(){assert_eq!(p.last_complete,target);return ticket;}
    }panic!("finite original timeline");
}
#[test]
fn actual_production_wip_pump_off_stop_and_late_payment_survive_original_wal_checkpoint_and_cold(){
    for deferred in [false,true]{
        let p=dir();let mut input=fixture::input();
        if deferred{input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"]["wage_settlement"]=serde_json::json!({"RetainProtectedEarned":{"policy_ref":9003,"version":1}});}
        let mut s=create_input(&p,input);setup(&mut s);
        for (time,checkpoint,label) in [(4,false,"production-wal"),(5000,true,"pump-off-checkpoint"),(5500,false,"completed-wal")]{
            let ticket=advance(&mut s,time);let hash=s.state_hash().unwrap();let seq=s.committed_sequence();
            fs::write(p.join(format!("{label}-checkpoint.json")),s.engine().unwrap().export_checkpoint().unwrap()).unwrap();
            if checkpoint{s.checkpoint().unwrap();}drop(s);cold(&p,&hash,label);
            s=Store::open(&p.join("world"),RULES).unwrap();authenticated(&s);assert_eq!(s.state_hash().unwrap(),hash);
            assert!(s.execute(Caller::System,ticket).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);
        }
        if deferred{for id in [1,2]{let ticket=apply(&mut s,Caller::Account(3),Action::FacilityWork{command:F::Pay{contract_id:id,cents:5501}});
            let hash=s.state_hash().unwrap();let seq=s.committed_sequence();assert!(s.execute(Caller::Account(3),ticket).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
        }}
        let v=s.engine().unwrap().world().view();let product=v.stocks.iter().find(|s|s.id==202).unwrap();assert_eq!((product.quantity.value(),product.basis.value()),(98_000_000,177548));
        assert_eq!(v.accounts.iter().find(|a|a.id==2).unwrap().cash.value(),211002);assert_eq!(v.accounts.iter().find(|a|a.id==3).unwrap().protected.value(),0);
        let hash=s.state_hash().unwrap();s.checkpoint().unwrap();drop(s);cold(&p,&hash,"paid-checkpoint");
    }
}
#[test]
fn original_product_move_receipt_wal_and_checkpoint_keep_both_stocks_and_quality(){
    for at in [3,4]{
        let p=dir();let mut s=create(&p);setup(&mut s);advance(&mut s,at);
        let from=s.engine().unwrap().world().stock_ref(202).unwrap();let to=s.engine().unwrap().world().stock_ref(205).unwrap();
        let ticket=apply(&mut s,Caller::Account(3),Action::MoveStock{from,to,quantity:mycity_core::units::Quantity::new(49000).unwrap()});
        let hash=s.state_hash().unwrap();let seq=s.committed_sequence();let quality=s.engine().unwrap().water_product_quality(205).unwrap().unwrap();
        fs::write(p.join("actual-moved-checkpoint.json"),s.engine().unwrap().export_checkpoint().unwrap()).unwrap();drop(s);cold(&p,&hash,"moved-wal");
        s=Store::open(&p.join("world"),RULES).unwrap();authenticated(&s);assert_eq!(s.engine().unwrap().water_product_quality(205).unwrap().unwrap(),quality);
        assert!(s.execute(Caller::Account(3),ticket.clone()).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
        advance(&mut s,5500);let q=s.engine().unwrap().water_product_quality(205).unwrap().unwrap();assert_eq!(q.composition,quality.composition);
        let v=s.engine().unwrap().world().view();let stock=|id|v.stocks.iter().find(|s|s.id==id).unwrap();assert_eq!((stock(205).quantity.value(),stock(205).basis.value()),(49000,790));assert_eq!((stock(202).quantity.value(),stock(202).basis.value()),(97_951_000,176758));
        let hash=s.state_hash().unwrap();s.checkpoint().unwrap();drop(s);cold(&p,&hash,"moved-then-production-checkpoint");
        s=Store::open(&p.join("world"),RULES).unwrap();let seq=s.committed_sequence();assert!(s.execute(Caller::Account(3),ticket).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),hash);
    }
}
#[test]
fn interrupted_save_and_retry_keep_complete_game_state_without_double_settlement(){
    use std::io;
    for moved in [false,true]{
        let reference_dir=dir();let mut reference=create(&reference_dir);setup(&mut reference);
        if moved{advance(&mut reference,3);}
        let before=reference.state_hash().unwrap();let before_seq=reference.committed_sequence();
        let action=if moved{Action::MoveStock{from:reference.engine().unwrap().world().stock_ref(202).unwrap(),to:reference.engine().unwrap().world().stock_ref(205).unwrap(),quantity:mycity_core::units::Quantity::new(49000).unwrap()}}
            else{Action::Timeline{command:T::Advance{target:4,limit:256}}};
        let caller=if moved{Caller::Account(3)}else{Caller::System};
        let request=Envelope{world:1,branch:1,operation_id:before_seq+1,expected_revision:reference.engine().unwrap().world().view().revision,actions:vec![action]};
        assert!(reference.execute(caller,request.clone()).unwrap().model.is_ok());let expected=reference.state_hash().unwrap();
        reference.checkpoint().unwrap();drop(reference);cold(&reference_dir,&expected,"normal-save-reference");
        for cut in [WritePoint::WalPayloadHalf,WritePoint::WalSynced,WritePoint::RootPublished,WritePoint::CurrentRenamed]{
            let p=dir();let mut s=create(&p);setup(&mut s);if moved{advance(&mut s,3);}
            assert_eq!(s.state_hash().unwrap(),before);assert_eq!(s.committed_sequence(),before_seq);
            let out=s.execute_with_observer(caller,request.clone(),|point|if point==cut{Err(io::Error::other("interrupted game save"))}else{Ok(())});
            assert!(out.is_err());drop(s);
            let mut s=Store::open(&p.join("world"),RULES).unwrap();authenticated(&s);
            let recovered=s.state_hash().unwrap();assert_eq!(&recovered,if cut==WritePoint::CurrentRenamed{&expected}else{&before},"cut {cut:?}");
            assert!(s.execute(caller,request.clone()).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),expected);assert_eq!(s.committed_sequence(),before_seq+1);
            assert!(s.execute(caller,request.clone()).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),expected);assert_eq!(s.committed_sequence(),before_seq+1);
            fs::write(p.join("save-retry-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"move_product":moved,"cut":format!("{cut:?}"),"before":before,"recovered":recovered,"after_retry":expected,"sequence":s.committed_sequence(),"duplicate_settlement":false})).unwrap()).unwrap();
            s.checkpoint().unwrap();drop(s);cold(&p,&expected,"recovered-and-retried-save");
        }
    }
}
fn setup(s:&mut Store)->Envelope{
    apply(s,Caller::System,Action::Timeline{command:T::Enable});
    for use_id in [1,2]{
        apply(s,Caller::Account(3),Action::Venue{use_id,command:V::Accept});
        let token=s.engine().unwrap().world().view().venue.unwrap().uses.iter().find(|s|s.use_id==use_id).unwrap().token.number;
        apply(s,Caller::System,Action::Venue{use_id,command:V::Enter{event_id:1000+use_id,token_number:token,occurred_at:0}});
        apply(s,Caller::System,Action::FacilityWork{command:F::Begin{contract_id:use_id}});
    }
    apply(s,Caller::Account(3),Action::ContinuousSupply{command:S::Start{run_id:1,pumps:3}})
}
fn authenticated(s:&Store){assert!(s.engine().unwrap().continuous_supply_is_authenticated());assert!(s.engine().unwrap().facility_work_is_authenticated());}
#[test]
fn real_two_worker_start_and_each_coupled_exit_restore_original_receipts_once(){
    for mode in 0..4{
        let p=dir();let mut s=create(&p);let start=setup(&mut s);let active=s.state_hash().unwrap();let seq=s.committed_sequence();drop(s);
        cold(&p,&active,"start-wal-cold");
        let mut s=Store::open(&p.join("world"),RULES).unwrap();authenticated(&s);assert_eq!(s.state_hash().unwrap(),active);
        assert!(s.execute(Caller::Account(3),start).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),seq);assert_eq!(s.state_hash().unwrap(),active);
        let token=s.engine().unwrap().world().view().venue.unwrap().uses.iter().find(|s|s.use_id==2).unwrap().token.number;
        let action=match mode{0=>Action::ContinuousSupply{command:S::Stop},1=>Action::ContinuousSupply{command:S::Fault{fault_ref:9000}},
            2=>Action::FacilityWork{command:F::Stop{contract_id:2}},_=>Action::Venue{use_id:2,command:V::SafeExit{event_id:2000,token_number:token,occurred_at:0}}};
        let caller=if mode<2{Caller::Account(3)}else if mode==2{Caller::Account(2)}else{Caller::System};
        let stop=apply(&mut s,caller,action);let stopped=s.state_hash().unwrap();let seq=s.committed_sequence();
        let cp=s.engine().unwrap().export_checkpoint().unwrap();fs::write(p.join("valid-stopped-checkpoint.json"),&cp).unwrap();
        s.checkpoint().unwrap();drop(s);cold(&p,&stopped,"stop-checkpoint-cold");
        let mut s=Store::open(&p.join("world"),RULES).unwrap();authenticated(&s);assert_eq!(s.state_hash().unwrap(),stopped);
        assert!(s.execute(caller,stop).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),stopped);assert_eq!(s.committed_sequence(),seq);
        let v=s.engine().unwrap().world().view();let a=v.accounts.iter().find(|a|a.id==3).unwrap();assert_eq!((a.cash.value(),a.protected.value()),(599300,0));
        for row in v.facility_work.unwrap().records.iter().flatten(){assert_eq!(row.phase,"stopped");assert_eq!((row.work_seconds,row.earned_cents,row.paid_cents),(0,0,0));}
        assert_eq!(v.continuous_supply.unwrap().zero_work_expense.value(),700);
    }
}
#[test]#[ignore]
fn water_cold_probe(){
    let cfg:serde_json::Value=serde_json::from_str(&std::env::var("MYCITY_WATER_COLD_PROBE").expect("explicit cold config")).unwrap();
    let s=Store::open(Path::new(cfg["world"].as_str().unwrap()),RULES).unwrap();authenticated(&s);
    assert_eq!(serde_json::to_value(s.state_hash().unwrap()).unwrap(),cfg["hash"]);
}
fn cold(p:&Path,hash:&impl serde::Serialize,label:&str){
    use std::{process::{Command,Stdio},time::{Duration,Instant},thread};
    let config=serde_json::json!({"world":p.join("world"),"hash":hash});
    let stdout=fs::File::create(p.join(format!("{label}-stdout.log"))).unwrap();let stderr=fs::File::create(p.join(format!("{label}-stderr.log"))).unwrap();
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--ignored","--exact","water_cold_probe","--test-threads=1","--nocapture"])
        .env("MYCITY_WATER_COLD_PROBE",config.to_string()).stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr)).spawn().unwrap();
    let deadline=Instant::now()+Duration::from_secs(60);
    loop{if let Some(status)=child.try_wait().unwrap(){
        fs::write(p.join(format!("{label}-result.json")),serde_json::to_vec(&serde_json::json!({"pid":child.id(),"exit_code":status.code(),"timed_out":false,"config":config})).unwrap()).unwrap();assert!(status.success());break;
    }if Instant::now()>=deadline{child.kill().unwrap();let _=child.wait();fs::write(p.join(format!("{label}-timeout.json")),config.to_string()).unwrap();panic!("own cold child timeout, originals retained");}thread::sleep(Duration::from_millis(20));}
}
