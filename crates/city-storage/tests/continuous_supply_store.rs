//! Real original Store WAL/checkpoint/receipt recovery, retained on the approved SSD.
use mycity_core::{continuous_supply::Command as SupplyCommand,timeline,transaction::{Action,Caller,Envelope}};
use mycity_storage::Store;
use serde_json::{json,Value};
use std::{fs::{self,OpenOptions},path::{Path,PathBuf},process::{Command,Stdio},thread,time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
#[path="../../../tests/fixtures/continuous-supply/opening.rs"]mod fixture;
const RULES:&str="continuous-supply-original-store-v1";
const BASE:&str="/Volumes/HP P900/dev/MyCity/work";
const PROBE:&str="MYCITY_CONTINUOUS_SUPPLY_PROBE";
fn run_dir()->PathBuf{
    let root=fs::canonicalize(std::env::var("MYCITY_CONTINUOUS_SUPPLY_TEST_ROOT").expect("explicit evidence root required")).unwrap();
    let approved=fs::canonicalize(BASE).expect("HP P900 must be mounted");assert_eq!(approved,Path::new(BASE));
    #[cfg(unix)]{use std::os::unix::fs::MetadataExt;assert_ne!(fs::metadata("/Volumes/HP P900").unwrap().dev(),fs::metadata("/Volumes").unwrap().dev());}
    assert!(root.starts_with(approved)&&root!=Path::new(BASE));
    let dir=root.join(format!("supply-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&dir).unwrap();println!("retained continuous supply original Store: {}",dir.display());dir
}
fn write(path:&Path,v:&Value){use std::io::Write;OpenOptions::new().create_new(true).write(true).open(path).unwrap().write_all(&serde_json::to_vec_pretty(v).unwrap()).unwrap();}
fn action(s:&mut Store,id:u64,caller:Caller,a:Action)->Envelope{
    let e=fixture::envelope(s.engine().unwrap(),id,a);assert!(s.execute(caller,e.clone()).unwrap().model.is_ok());e
}
fn advance(s:&mut Store,target:u64,mut id:u64){for _ in 0..128{
    action(s,id,Caller::System,Action::Timeline{command:timeline::Command::Advance{target,limit:1}});id+=1;
    if s.engine().unwrap().timeline().unwrap().progress.target.is_none(){return;}
}panic!("bounded original Timeline");}
fn cold(dir:&Path,world:&Path,name:&str,checkpoint:bool){
    let config=json!({"world":world,"checkpoint":checkpoint});
    let out=OpenOptions::new().create_new(true).write(true).open(dir.join(format!("{name}-stdout.log"))).unwrap();
    let err=OpenOptions::new().create_new(true).write(true).open(dir.join(format!("{name}-stderr.log"))).unwrap();
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--ignored","--exact","continuous_supply_cold_probe","--test-threads=1","--nocapture"])
        .env(PROBE,config.to_string()).stdout(Stdio::from(out)).stderr(Stdio::from(err)).spawn().unwrap();
    let pid=child.id();write(&dir.join(format!("{name}-launch.json")),&json!({"pid":pid,"probe":config,"timeout_seconds":60}));
    let deadline=Instant::now()+Duration::from_secs(60);let mut timeout=false;let mut wait_error=None;
    let status=loop{match child.try_wait(){Ok(Some(x))=>break Some(x),Ok(None)if Instant::now()<deadline=>thread::sleep(Duration::from_millis(20)),
        Ok(None)=>{timeout=true;child.kill().unwrap();break child.wait().ok();},Err(e)=>{wait_error=Some(e.to_string());let _=child.kill();break child.wait().ok();}}};
    write(&dir.join(format!("{name}-result.json")),&json!({"pid":pid,"exit_code":status.as_ref().and_then(|s|s.code()),"timed_out":timeout,"wait_error":wait_error}));
    assert!(!timeout&&wait_error.is_none()&&status.is_some_and(|s|s.success()),"cold child evidence retained");
}
#[test]
fn original_wal_checkpoint_cold_process_retries_and_fractional_inputs_are_once(){
    let dir=run_dir();let world=dir.join("world");
    let mut store=Store::create(&world,&serde_json::to_vec(&fixture::input()).unwrap(),fixture::resources(),RULES).unwrap();
    action(&mut store,1,Caller::System,Action::Timeline{command:timeline::Command::Enable});
    let start=action(&mut store,2,Caller::Account(1),Action::ContinuousSupply{command:SupplyCommand::Start{run_id:1,pumps:3}});
    advance(&mut store,1,10);let expected=serde_json::to_value(fixture::supply(store.engine().unwrap())).unwrap();
    write(&dir.join("expected-prefix.json"),&expected);write(&dir.join("original-start-envelope.json"),&serde_json::to_value(&start).unwrap());
    let sequence=store.committed_sequence();drop(store);
    // First new process reads real uncheckpointed WAL, then checkpoints that same original state.
    cold(&dir,&world,"wal-reader",true);
    // Second new process reads the checkpoint and authenticates original archived receipts.
    cold(&dir,&world,"checkpoint-reader",false);
    let mut reopened=Store::open(&world,RULES).unwrap();assert_eq!(sequence,reopened.committed_sequence());
    assert_eq!(expected,serde_json::to_value(fixture::supply(reopened.engine().unwrap())).unwrap());
    let retry=reopened.execute(Caller::Account(1),start).unwrap();assert!(retry.model.is_ok());assert_eq!(retry.committed_sequence,sequence);
    advance(&mut reopened,3,100);let s=fixture::supply(reopened.engine().unwrap());
    assert_eq!(s.genset.active_seconds,3);assert_eq!(s.produced_ml,[30_000;2]);assert_eq!(s.machine_meters.fuel_burned_thirds_mq,150);
    assert_eq!(s.machine_meters.start_fees.value(),300);assert_eq!(s.output_basis.map(|m|m.value()),[394,395]);
    action(&mut reopened,200,Caller::Account(1),Action::ContinuousSupply{command:SupplyCommand::Stop});
    let final_supply=serde_json::to_value(fixture::supply(reopened.engine().unwrap())).unwrap();reopened.checkpoint().unwrap();drop(reopened);
    let mut final_open=Store::open(&world,RULES).unwrap();assert_eq!(final_supply,serde_json::to_value(fixture::supply(final_open.engine().unwrap())).unwrap());
    write(&dir.join("final-prefix.json"),&final_supply);
    action(&mut final_open,201,Caller::Account(1),Action::ContinuousSupply{command:SupplyCommand::Start{run_id:2,pumps:2}});
    advance(&mut final_open,4,300);action(&mut final_open,400,Caller::Account(1),Action::ContinuousSupply{command:SupplyCommand::Stop});
    let restarted=fixture::supply(final_open.engine().unwrap());assert_eq!(restarted.produced_ml,[30_000,40_000]);
    assert_eq!(restarted.output_basis.map(|m|m.value()),[394,919]);assert_eq!(restarted.machine_meters.start_fees.value(),600);
    final_open.checkpoint().unwrap();drop(final_open);let after_restart=Store::open(&world,RULES).unwrap();
    assert_eq!(restarted,fixture::supply(after_restart.engine().unwrap()));write(&dir.join("restarted-prefix.json"),&serde_json::to_value(restarted).unwrap());
}
#[test]
#[ignore="fresh original Store process launched by parent"]
fn continuous_supply_cold_probe(){
    let config:Value=serde_json::from_str(&std::env::var(PROBE).expect("parent probe context")).unwrap();
    let world=PathBuf::from(config["world"].as_str().unwrap());assert!(world.starts_with(BASE));
    let expected:Value=serde_json::from_slice(&fs::read(world.parent().unwrap().join("expected-prefix.json")).unwrap()).unwrap();
    let mut store=Store::open(&world,RULES).unwrap();assert!(store.engine().unwrap().continuous_supply_is_authenticated());
    assert_eq!(expected,serde_json::to_value(fixture::supply(store.engine().unwrap())).unwrap());
    if config["checkpoint"]==true{store.checkpoint().unwrap();}
}
