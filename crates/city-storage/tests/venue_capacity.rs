//! Actual original Engine/Store gate, separate from the pure candidate's test fixture.
use mycity_core::{timeline::Command as T, transaction::{Action, Caller, CapacityToken, Envelope, Resource},
    venue_capacity::UseStatus, venue_runtime::Command as V};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{fs, io, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};
const RULES: &str = "issue118-managed-venue-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new(name: &str) -> Self {
        let base = fs::canonicalize("/Volumes/HP P900/dev/MyCity/work").expect("existing external SSD");
        let configured = std::env::var_os("MYCITY_VENUE_TEST_ROOT").expect("set retained external test root");
        let root = fs::canonicalize(configured).unwrap();
        assert!(root != base && root.starts_with(&base) && root.is_dir());
        let path = root.join(format!("{name}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        println!("retained venue world {}", path.display()); Self(path)
    }
    fn world(&self) -> PathBuf { self.0.join("world") }
}
fn opening() -> Value {
    let mut input: Value = serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    // Existing original registration, not venue-created people or an account alias.
    input["housing"] = json!({"unit_limit":1,"dwellings":[{
        "fact":{"unit_id":71,"building_id":70,"owner_account":1,"building_unit_capacity":1,
            "person_capacity":10,"delivered":true,"legal_to_occupy":true,"has_legal_access":true,"source_version":1},
        "origin":"immutable_genesis_endowment","asset_source_id":7002}],"households":[

        {"household_id":7,"account":2,"people":10,"source_id":7001}],"lease_offers":[]});
    let source = json!({"kind":"genesis","scope":{"world":1,"branch":1},"revision":0,
        "operation_id":0,"event_id":0,"source_id":7001});
    let offer = |id: u64, venue: u64, group: u64, start: u64, end: u64, lo: u64| json!({
        "use_id":id,"venue_id":venue,"group_id":group,
        "partition":{"group_id":group,"partition_source_id":1,"partition_version":1,
            "member_range_start":lo,"member_range_end":lo+3},
        "population":{"learners":2,"teachers":0,"staff":0,"visitors":1,"enrollment":2},
        "purpose":1,"operator":3,"window":{"start":start,"end":end},
        "source_refs":[source.clone(),null,null,null],"accepted_version":1});
    input["venue"] = json!({"profile":"managed_people_v1","population_source":"housing_genesis_v1",
        "venues":[
            {"venue_id":1001,"resource_id":501,"aliases":[2001,null,null,null],"owner":1,"operator":3,"purpose":1,"permit_version":1},
            {"venue_id":1002,"resource_id":502,"aliases":[2002,null,null,null],"owner":1,"operator":3,"purpose":1,"permit_version":1}],
        "limits":[{"resource_id":501,"maximum_people":4},{"resource_id":502,"maximum_people":4}],
        "offers":[offer(1,1001,1,0,10,0),offer(2,1001,2,0,10,3),
                  offer(3,1002,1,0,10,0),offer(4,1001,3,20,30,6)]}); input
}
fn create(path: &Path, input: &Value) -> Store {
    fs::write(path.parent().unwrap().join("opening.json"), serde_json::to_vec(input).unwrap()).unwrap();
    let store=Store::create(path, &serde_json::to_vec(input).unwrap(),
        vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()], RULES).unwrap();
    let engine_owned=store.engine().unwrap().owned_capacity_bytes();
    assert!(engine_owned <= mycity_core::transaction::ENGINE_BYTE_BUDGET);
    println!("venue logical bytes engine={engine_owned} store={} engine_budget={} candidate_header={} candidate_payload={} runtime_box_handle={}",
        store.owned_capacity_bytes().unwrap(), mycity_core::transaction::ENGINE_BYTE_BUDGET,
        2*std::mem::size_of::<mycity_core::model_rows::Rows<mycity_core::venue_capacity::UseState,64>>(),
        64*(std::mem::size_of::<mycity_core::venue_capacity::UseState>()+std::mem::size_of::<mycity_core::venue_runtime::ActivityFacts>()),
        std::mem::size_of::<Box<mycity_core::venue_runtime::Opening>>());
    let layout=store.engine().unwrap().world().layout();
    println!("venue actual ABI layout {layout}");
    fs::write(path.parent().unwrap().join("layout.json"),serde_json::to_vec_pretty(&layout).unwrap()).unwrap();
    store
}
fn envelope(s: &Store, id: u64, action: Action) -> Envelope {
    let (scope, revision) = s.engine().unwrap().world().scope_revision();
    Envelope { world:scope.world(),branch:scope.branch(),operation_id:id,expected_revision:revision,actions:vec![action] }
}
fn venue(use_id: u64, command: V) -> Action { Action::Venue { use_id, command } }
fn execute(s: &mut Store, caller: Caller, id: u64, action: Action) {
    let env = envelope(s,id,action); let out = s.execute(caller,env).unwrap(); assert!(out.model.is_ok(),"{out:?}");
}
fn reject(s: &mut Store, caller: Caller, id: u64, action: Action) {
    let before = s.engine().unwrap().business_comparison_checkpoint().unwrap();
    let env = envelope(s,id,action); let out = s.execute(caller,env).unwrap(); assert!(out.model.is_err(),"{out:?}");
    assert_eq!(before,s.engine().unwrap().business_comparison_checkpoint().unwrap());
}
fn enable(s: &mut Store) { execute(s,Caller::System,1,Action::Timeline { command:T::Enable }); }
fn token(s: &Store, use_id: u64) -> CapacityToken {
    s.engine().unwrap().world().view().venue.unwrap().uses.iter().find(|row|row.use_id==use_id).unwrap().token
}
fn counters(s: &Store, resource: u32) -> (u32,u32) {
    let r=s.engine().unwrap().resources().iter().find(|row|row.id==resource).unwrap(); (r.reserved,r.occupied)
}
fn advance(s: &mut Store, target:u64, first_id:u64) {
    for n in 0..128 {
        execute(s,Caller::System,first_id+n,Action::Timeline { command:T::Advance { target,limit:128 } });
        if s.engine().unwrap().timeline().unwrap().progress.target.is_none() { return; }
    } panic!("bounded advance failed to finish");
}

#[test]
fn actual_entry_due_safe_exit_cold_restore_and_retry_preserve_original_lease() {
    let t=Temp::new("lifecycle"); let path=t.world(); let mut s=create(&path,&opening()); enable(&mut s);
    execute(&mut s,Caller::Account(3),2,venue(1,V::Accept)); let tok=token(&s,1);
    assert_eq!(counters(&s,501),(3,0));
    reject(&mut s,Caller::Account(3),3,venue(1,V::Enter { event_id:100,token_number:tok.number,occurred_at:0 }));
    let entered=envelope(&s,4,venue(1,V::Enter { event_id:100,token_number:tok.number,occurred_at:0 }));
    assert!(s.execute(Caller::System,entered.clone()).unwrap().model.is_ok());
    assert_eq!(counters(&s,501),(0,3)); s.checkpoint().unwrap(); drop(s);
    let mut s=Store::open(&path,RULES).unwrap();
    assert!(s.execute(Caller::System,entered).unwrap().model.is_ok()); assert_eq!(counters(&s,501),(0,3));
    advance(&mut s,20,1000); assert_eq!(s.engine().unwrap().venue_exit_due(1),Some(true));
    assert_eq!(counters(&s,501),(0,3));
    reject(&mut s,Caller::System,2000,Action::ReleaseCapacity { token:tok });
    reject(&mut s,Caller::System,2001,venue(1,V::SafeExit { event_id:101,token_number:tok.number,occurred_at:19 }));
    execute(&mut s,Caller::System,2002,venue(1,V::SafeExit { event_id:101,token_number:tok.number,occurred_at:20 }));
    assert_eq!(counters(&s,501),(0,0));
    reject(&mut s,Caller::System,2003,venue(1,V::SafeExit { event_id:102,token_number:tok.number,occurred_at:20 }));
    s.checkpoint().unwrap(); drop(s); let s=Store::open(&path,RULES).unwrap();
    let view=s.engine().unwrap().world().view().venue.unwrap();
    assert_eq!(view.uses[0].status,UseStatus::Released); assert_eq!(view.activities[0].entered.unwrap().event_id,100);
    assert_eq!(view.activities[0].exited.unwrap().event_id,101); assert_eq!(counters(&s,501),(0,0));
}

#[test]
fn competing_uses_group_exclusion_cancel_authority_and_no_generic_bypass() {
    let t=Temp::new("conflicts"); let mut s=create(&t.world(),&opening()); enable(&mut s);
    reject(&mut s,Caller::Account(4),2,venue(1,V::Accept));
    execute(&mut s,Caller::Account(3),3,venue(1,V::Accept)); let a=token(&s,1);
    reject(&mut s,Caller::Account(3),4,venue(2,V::Accept)); assert_eq!(counters(&s,501),(3,0));
    // Future nonoverlap does not defeat conservative immediate holding; not real shortage.
    reject(&mut s,Caller::Account(3),5,venue(4,V::Accept));
    execute(&mut s,Caller::Account(3),6,venue(3,V::Accept)); let b=token(&s,3);
    execute(&mut s,Caller::System,7,venue(1,V::Enter { event_id:100,token_number:a.number,occurred_at:0 }));
    reject(&mut s,Caller::System,8,venue(3,V::Enter { event_id:101,token_number:b.number,occurred_at:0 }));
    reject(&mut s,Caller::Account(4),9,venue(3,V::CancelReserved));
    reject(&mut s,Caller::System,10,Action::OccupyCapacity { token:b });
    reject(&mut s,Caller::System,11,Action::ReleaseCapacity { token:b });
    reject(&mut s,Caller::System,12,Action::ReserveCapacity { resource:501,units:1 });
    execute(&mut s,Caller::Account(3),13,venue(3,V::CancelReserved)); assert_eq!(counters(&s,502),(0,0));
    execute(&mut s,Caller::System,14,venue(1,V::SafeExit { event_id:102,token_number:a.number,occurred_at:0 }));
    execute(&mut s,Caller::Account(3),15,venue(2,V::Accept)); let c=token(&s,2);
    // Raw event identity survives even after its use became terminal and last_event changed.
    reject(&mut s,Caller::System,16,venue(2,V::Enter { event_id:100,token_number:c.number,occurred_at:0 }));
    execute(&mut s,Caller::System,17,venue(2,V::Enter { event_id:103,token_number:c.number,occurred_at:0 }));
    // A legacy non-managed resource retains its own ordinary path.
    execute(&mut s,Caller::Account(3),18,Action::ReserveCapacity { resource:503,units:1 });
    assert_eq!(counters(&s,503),(1,0));
}

#[test]
fn pre_persist_failure_publishes_neither_original_capacity_nor_venue_metadata() {
    let t=Temp::new("prepare-failure"); let path=t.world(); let mut s=create(&path,&opening()); enable(&mut s);
    let request=envelope(&s,2,venue(1,V::Accept));
    let result=s.execute_with_observer(Caller::Account(3),request.clone(),|point| {
        if point==WritePoint::Prepared { Err(io::Error::other("venue retained pre-persist fault")) } else { Ok(()) }
    }); assert!(result.is_err()); fs::write(t.0.join("injected-error.txt"),format!("{result:?}")).unwrap(); drop(s);
    let mut s=Store::open(&path,RULES).unwrap(); assert_eq!(counters(&s,501),(0,0));
    assert!(s.engine().unwrap().world().view().venue.unwrap().uses.is_empty()); assert!(s.receipt(2).unwrap().is_none());
    assert!(s.execute(Caller::Account(3),request).unwrap().model.is_ok()); assert_eq!(counters(&s,501),(3,0));
}

#[test]
fn lost_actual_response_cold_replay_never_reenters_or_loses_event_identity() {
    let t=Temp::new("lost-response"); let path=t.world(); let mut s=create(&path,&opening()); enable(&mut s);
    execute(&mut s,Caller::Account(3),2,venue(1,V::Accept)); let tok=token(&s,1);
    let request=envelope(&s,3,venue(1,V::Enter { event_id:900,token_number:tok.number,occurred_at:0 }));
    let result=s.execute_with_observer(Caller::System,request.clone(),|point| {
        if point==WritePoint::Applied { Err(io::Error::other("venue retained lost response")) } else { Ok(()) }
    }); assert!(result.is_err()); fs::write(t.0.join("injected-error.txt"),format!("{result:?}")).unwrap(); drop(s);
    let mut s=Store::open(&path,RULES).unwrap(); assert_eq!(counters(&s,501),(0,3));
    assert!(s.execute(Caller::System,request).unwrap().model.is_ok());
    let view=s.engine().unwrap().world().view().venue.unwrap(); assert_eq!(view.uses.len(),1); assert_eq!(view.activities.len(),1);
    assert_eq!(view.activities[0].entered.unwrap().event_id,900); assert_eq!(counters(&s,501),(0,3));
}

#[test]
fn original_population_coverage_alias_and_registration_limits_fail_closed() {
    let control=Temp::new("invalid-case-legal-control"); let _control=create(&control.world(),&opening());
    for (name, mutate) in [
        ("population-outside", 0), ("source-mismatch", 1), ("duplicate-alias", 2),
        ("limit-mismatch", 3), ("population-missing", 4), ("overlap-group", 5),
    ] {
        let t=Temp::new(name); let mut input=opening();
        match mutate {
            0 => input["venue"]["offers"][0]["partition"]["member_range_end"]=json!(11),
            1 => input["venue"]["offers"][0]["source_refs"][0]["source_id"]=json!(999),
            2 => input["venue"]["venues"][1]["aliases"][0]=json!(2001),
            3 => input["venue"]["limits"][0]["maximum_people"]=json!(5),
            4 => { input.as_object_mut().unwrap().remove("housing"); },
            5 => input["venue"]["offers"][1]["partition"]["member_range_start"]=json!(1),
            _ => unreachable!(),
        }
        let result=Store::create(&t.world(),&serde_json::to_vec(&input).unwrap(),
            vec![Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES);
        assert!(result.is_err(),"invalid {name} accepted");
    }
}

#[test]
fn missing_clock_multiaction_and_late_enter_cannot_fabricate_presence() {
    let t=Temp::new("clock-gates"); let mut s=create(&t.world(),&opening());
    reject(&mut s,Caller::Account(3),2,venue(1,V::Accept)); enable(&mut s);
    let mut request=envelope(&s,3,venue(1,V::Accept)); request.actions.push(venue(3,V::Accept));
    assert!(s.execute(Caller::Account(3),request).unwrap().model.is_err());
    execute(&mut s,Caller::Account(3),4,venue(1,V::Accept)); let tok=token(&s,1);
    advance(&mut s,10,1000);
    reject(&mut s,Caller::System,2000,venue(1,V::Enter { event_id:50,token_number:tok.number,occurred_at:10 }));
    assert_eq!(counters(&s,501),(3,0)); execute(&mut s,Caller::Account(3),2001,venue(1,V::CancelReserved));
    assert_eq!(counters(&s,501),(0,0));
}


#[test]
fn restore_rejects_wrong_current_and_historical_activity_operation_sources() {
    let t=Temp::new("restore-facts"); let mut s=create(&t.world(),&opening()); enable(&mut s);
    execute(&mut s,Caller::Account(3),2,venue(1,V::Accept)); let tok=token(&s,1);
    execute(&mut s,Caller::System,3,venue(1,V::Enter { event_id:100,token_number:tok.number,occurred_at:0 }));
    let original=s.engine().unwrap().export_checkpoint().unwrap(); fs::write(t.0.join("actual-checkpoint.json"),&original).unwrap();
    for (index, field) in ["operation_id", "occurred_at"].into_iter().enumerate() {
        let mut corrupt:Value=serde_json::from_slice(&original).unwrap();
        corrupt["world"]["venue"]["facts"][0]["entered"][field]=json!(9999);
        let bytes=serde_json::to_vec(&corrupt).unwrap(); fs::write(t.0.join(format!("wrong-actual-{index}.json")),&bytes).unwrap();
        assert!(mycity_core::transaction::Engine::restore_checkpoint(&bytes).is_err());
    }
    execute(&mut s,Caller::System,4,venue(1,V::SafeExit { event_id:101,token_number:tok.number,occurred_at:0 }));
    let original=s.engine().unwrap().export_checkpoint().unwrap(); fs::write(t.0.join("released-checkpoint.json"),&original).unwrap();
    for (index, role) in ["entered","exited"].into_iter().enumerate() {
        let mut corrupt:Value=serde_json::from_slice(&original).unwrap();
        corrupt["world"]["venue"]["facts"][0][role]["operation_id"]=json!(9999);
        let bytes=serde_json::to_vec(&corrupt).unwrap(); fs::write(t.0.join(format!("wrong-released-{index}.json")),&bytes).unwrap();
        assert!(mycity_core::transaction::Engine::restore_checkpoint(&bytes).is_err());
    }
    let mut future:Value=serde_json::from_slice(&original).unwrap();
    future["world"]["venue"]["facts"][0]["entered"]["transaction_source"]["revision"]=json!(u64::MAX);
    let bytes=serde_json::to_vec(&future).unwrap(); fs::write(t.0.join("future-historical-source.json"),&bytes).unwrap();
    assert!(mycity_core::transaction::Engine::restore_checkpoint(&bytes).is_err());
    let mut corrupt:Value=serde_json::from_slice(&original).unwrap();
    corrupt["world"]["venue"]["facts"][0]["exited"]["event_id"]=json!(100);
    assert!(mycity_core::transaction::Engine::restore_checkpoint(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(counters(&s,501),(0,0));
}

#[test]
fn existing_boot_resource_cannot_be_reregistered_as_people_even_when_unoccupied() {
    let t=Temp::new("boot-domain-conflict");
    let mut input:Value=serde_json::from_slice(include_bytes!("../../../fixtures/m0-boot-opening.json")).unwrap();
    let template=opening(); input["housing"]=template["housing"].clone(); input["venue"]=template["venue"].clone();
    let control=Temp::new("boot-domain-legal-control");
    let positive=Store::create(&control.world(),&serde_json::to_vec(&input).unwrap(),
        vec![Resource::new(101,4).unwrap(),Resource::new(102,4).unwrap(),Resource::new(501,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES);
    assert!(positive.is_ok(), "legal joint Boot/venue profile must succeed");
    input["venue"]["venues"][0]["resource_id"]=json!(101);
    input["venue"]["limits"][0]["resource_id"]=json!(101);
    fs::write(t.0.join("opening.json"),serde_json::to_vec(&input).unwrap()).unwrap();
    let result=Store::create(&t.world(),&serde_json::to_vec(&input).unwrap(),
        vec![Resource::new(101,4).unwrap(),Resource::new(102,4).unwrap(),Resource::new(502,4).unwrap(),Resource::new(503,5).unwrap()],RULES);
    assert!(result.is_err());
}


#[test]
fn original_failed_checkpoint_archive_reopens_and_retains_exact_venue_receipts() {
    let t=Temp::new("original-checkpoint-regression"); let path=t.world(); fs::create_dir(&path).unwrap();
    let files: &[(&str,&[u8])] = &[
        ("CURRENT.json",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/CURRENT.json")),
        ("LOCK",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/LOCK")),
        ("checkpoint-5.json",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/checkpoint-5.json")),
        ("checkpoint-index-5.json",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/checkpoint-index-5.json")),
        ("genesis.json",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/genesis.json")),
        ("root-0.json",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/root-0.json")),
        ("root-1.json",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/root-1.json")),
        ("wal-5.bin",include_bytes!("fixtures/venue-checkpoint-before-receipt-fix/wal-5.bin")),
    ];
    for (name,bytes) in files { fs::write(path.join(name),bytes).unwrap(); }
    let mut store=Store::open(&path,RULES).unwrap(); assert_eq!(store.committed_sequence(),4);
    assert_eq!(counters(&store,501),(0,3));
    for id in 2..=4 {
        let record=store.receipt(id).unwrap().unwrap();
        assert!(matches!(record.actions[0],Some(Action::Venue { use_id:1,.. })));
        assert_eq!(record.operation_id,id);
    }
    let view=store.engine().unwrap().world().view().venue.unwrap();
    assert_eq!(view.uses[0].status,UseStatus::Actual); assert_eq!(view.activities[0].entered.unwrap().event_id,100);
    let before=store.engine().unwrap().business_comparison_checkpoint().unwrap();
    store.checkpoint().unwrap(); drop(store);
    let store=Store::open(&path,RULES).unwrap(); assert_eq!(before,store.engine().unwrap().business_comparison_checkpoint().unwrap());
}

#[test]
fn reused_original_engine_slot_clears_venue_when_restoring_legacy_absent_field() {
    use mycity_core::transaction::{Engine,EngineSlot};
    let t=Temp::new("old-slot-clear"); let store=create(&t.world(),&opening());
    let venue=store.engine().unwrap().export_checkpoint().unwrap();
    let legacy=Engine::new(mycity_core::assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![]).unwrap();
    let legacy_bytes=legacy.export_checkpoint().unwrap();
    let parsed:Value=serde_json::from_slice(&legacy_bytes).unwrap(); assert!(parsed["world"].get("venue").is_none());
    let mut slot=EngineSlot::new().unwrap();slot.restore_into(&venue).unwrap();
    assert!(slot.engine().unwrap().world().view().venue.is_some());
    slot.restore_into(&legacy_bytes).unwrap();
    assert!(slot.engine().unwrap().world().view().venue.is_none());
    assert_eq!(legacy.business_comparison_checkpoint().unwrap(),slot.engine().unwrap().business_comparison_checkpoint().unwrap());
    fs::write(t.0.join("old-wire-checkpoint.json"),legacy_bytes).unwrap();
}
