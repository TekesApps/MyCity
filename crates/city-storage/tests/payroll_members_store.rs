//! #155 real Store acceptance inputs for actual Payroll/member ownership.
//! Run only through the root's serial HP P900 evidence plan.
use mycity_core::{
    boot::{Command as B, Facility},
    housing,
    service_delivery_authority::Motion,
    service_delivery_authority::Command as D,
    timeline::{Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[path = "../../../tests/fixtures/finite-city/opening.rs"]
mod fixture;
const RULES: &str = "finite-city-local-boot-v1";
fn payroll_spec() -> PayrollSpec {
    PayrollSpec {
    id: 21,
    payer: 7,
    payee: 2,
    home_city: 0,
    hourly_cents: Money::new(10).unwrap(),
    cycle: Cycle::Daily,
    ends_at: 3 * DAY,
}
}

fn evidence_root() -> PathBuf {
    let p = PathBuf::from(std::env::var_os("MYCITY_PAYROLL_MEMBERS_ROOT").expect("retained SSD evidence root"));
    let p = p.canonicalize().unwrap();
    assert!(p.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    p
}
fn registered_input(members: (u64, u64)) -> Value {
    let mut raw = fixture::boot_service_input();
    raw["payroll_members"] = json!({
        "version": 1,
        "profile": "LocalDevelopmentActualJobGroupV1",
        "scope": raw["member_activity"]["scope"],
        "population_origin": raw["member_activity"]["population_origin"],
        "member_version": 1,
        "bindings": [{
            "spec": payroll_spec(),
            "members": {"start": members.0, "end": members.1},
            "source": {"id": 15501, "version": 1}
        }]
    });
    raw
}
fn create_store(raw: &Value, root: &Path) -> Store {
    fs::create_dir(root).unwrap();
    let bytes = serde_json::to_vec(raw).unwrap();
    fs::write(root.join("opening.json"), &bytes).unwrap();
    Store::create(&root.join("world"), &bytes, fixture::resources(), RULES).unwrap()
}
fn apply(s: &mut Store, caller: Caller, action: Action) -> Envelope {
    let (scope, revision) = s.engine().unwrap().world().scope_revision();
    let env = Envelope {
        world: scope.world(), branch: scope.branch(), operation_id: s.committed_sequence() + 1,
        expected_revision: revision, actions: vec![action],
    };
    let r = s.execute(caller, env.clone()).unwrap();
    assert!(r.model.is_ok(), "{r:?}");
    env
}
fn advance(s: &mut Store, target: u64) -> Envelope {
    for _ in 0..8192 {
        let limit = s.continuation_status().unwrap().map_or(256, |r| r.page_limit);
        let last = apply(s, Caller::System, Action::Timeline { command: T::Advance { target, limit } });
        let p = s.engine().unwrap().timeline().unwrap().progress;
        if p.target.is_none() { assert_eq!(p.last_complete, target); return last; }
    }
    panic!("finite timeline unfinished")
}
fn advance_page(s: &mut Store, target: u64) -> Envelope {
    apply(s, Caller::System, Action::Timeline { command: T::Advance { target, limit: 1 } })
}
fn start_world(raw: &Value, members: (u64, u64), label: &str) -> (PathBuf, Store) {
    let root = evidence_root().join(label);
    let mut s = create_store(&registered_input(members), &root);
    apply(&mut s, Caller::System, Action::Timeline { command: T::Enable });
    let offer: housing::GenesisLeaseOffer = serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();
    apply(&mut s, Caller::Account(2), Action::HousingLease {
        offer_id: 901, offer_sha256: housing::genesis_lease_offer_sha256(offer),
    });
    apply(&mut s, Caller::System, Action::Timeline { command: T::AddPayroll { spec: payroll_spec() } });
    for (facility, until) in [(Facility::Pump, 60), (Facility::Generator, 120)] {
        apply(&mut s, Caller::System, Action::Boot { command: B::Start { facility, reverse_request: false } });
        advance(&mut s, until);
    }
    assert!(s.engine().unwrap().payroll_members_is_ready());
    (root, s)
}
fn cold(root: &Path, s: Store, replay: &Envelope, label: &str, seconds: u64, wages: i64) {
    let input = root.join(format!("{label}-cold.json"));
    fs::write(&input, json!({
        "world": root.join("world"), "hash": s.state_hash().unwrap(),
        "sequence": s.committed_sequence(), "replay": replay,
        "seconds": seconds, "wages": wages, "registration":s.engine().unwrap().payroll_members_registration(),
    }).to_string()).unwrap();
    drop(s);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "payroll_members_cold_probe", "--test-threads=1", "--nocapture"])
        .env("MYCITY_PAYROLL_MEMBERS_COLD", &input)
        .stdout(Stdio::from(fs::File::create(root.join(format!("{label}-cold.log"))).unwrap()))
        .stderr(Stdio::from(fs::File::create(root.join(format!("{label}-cold.stderr"))).unwrap()))
        .spawn().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            fs::write(root.join(format!("{label}-cold-result.json")), json!({"pid":child.id(),"exit":status.code(),"seconds":start.elapsed().as_secs_f64()}).to_string()).unwrap();
            assert!(status.success()); return;
        }
        if start.elapsed() > Duration::from_secs(60) { child.kill().unwrap(); child.wait().unwrap(); panic!("cold restore timeout"); }
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore]
fn payroll_members_cold_probe() {
    let input: Value = serde_json::from_slice(&fs::read(std::env::var_os("MYCITY_PAYROLL_MEMBERS_COLD").unwrap()).unwrap()).unwrap();
    let mut s = Store::open(Path::new(input["world"].as_str().unwrap()), RULES).unwrap();
    let e = s.engine().unwrap(); assert!(e.payroll_members_is_ready());
    assert_eq!(serde_json::to_value(e.payroll_members_registration()).unwrap(),input["registration"]);
    assert!(e.service_needs_is_authenticated());
    assert_eq!(serde_json::to_value(s.state_hash().unwrap()).unwrap(),input["hash"]);
    assert_eq!(s.committed_sequence(),input["sequence"].as_u64().unwrap());
    let p = &e.timeline().unwrap().payroll[0];
    assert_eq!(p.work_seconds, input["seconds"].as_u64().unwrap());
    assert_eq!(p.paid.value(), input["wages"].as_i64().unwrap());
    let hash = s.state_hash().unwrap(); let seq = s.committed_sequence();
    let replay: Envelope = serde_json::from_value(input["replay"].clone()).unwrap();
    assert!(s.execute(Caller::System, replay).unwrap().model.is_ok());
    assert_eq!(s.committed_sequence(), seq); assert_eq!(s.state_hash().unwrap(), hash);
}

/// [2,3) and the service crew's [0,2) do not overlap; both complete a normal two-day cycle.
#[test]
fn registered_payroll_group_coexists_with_service_and_restores_from_wal_and_checkpoint() {
    let opening = registered_input((2, 3));
    let (root, mut s) = start_world(&opening, (2, 3), "independent-group");
    advance(&mut s, 8 * 3600);
    let accept = apply(&mut s, Caller::Account(3), Action::ServiceDelivery { command: D::Accept { household: 7, crew: 1 } });
    let completed = advance(&mut s, 13 * 3600 + 1500);
    assert_eq!(s.engine().unwrap().timeline().unwrap().payroll[0].work_seconds, 4 * 3600 + 1500);
    assert_eq!(s.engine().unwrap().service_needs_summary().unwrap().delivery.unwrap().trips[0].motion, Motion::Finished);
    advance(&mut s,17*3600);
    let mut partial=advance_page(&mut s,DAY);
    for _ in 0..256{let p=s.engine().unwrap().timeline().unwrap().progress;if p.at==DAY&&p.phase==Some(mycity_core::schedule::Phase::P3){break;}partial=advance_page(&mut s,DAY);}
    let p=s.engine().unwrap().timeline().unwrap().progress;assert_eq!((p.at,p.phase),(DAY,Some(mycity_core::schedule::Phase::P3)));
    cold(&root,s,&partial,"partial-p3-paid",8*3600,80);
    let mut s=Store::open(&root.join("world"),RULES).unwrap();
    advance(&mut s, DAY);
    let settled = apply(&mut s, Caller::System, Action::TownSettle { profile_id: 1 });
    let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!((p.work_seconds, p.paid.value(), p.remainder), (8 * 3600, 80, 0));
    assert_eq!(s.engine().unwrap().world().view().town.unwrap().state.last_summary.produced.value(), 100);
    cold(&root, s, &settled, "wal-day1", 8 * 3600, 80);
    let mut s = Store::open(&root.join("world"), RULES).unwrap();
    advance(&mut s, 2 * DAY);
    let second = apply(&mut s, Caller::System, Action::TownSettle { profile_id: 1 });
    s.checkpoint().unwrap();
    let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!((p.work_seconds, p.paid.value(), p.remainder), (16 * 3600, 160, 0));
    assert_eq!(s.engine().unwrap().world().view().town.unwrap().state.last_summary.produced.value(), 100);
    cold(&root, s, &second, "checkpoint-day2", 16 * 3600, 160);
    let _ = (accept, completed);
}

/// The [0,1) job waits through the actual [0,2) public-service claim and starts at 09:25.
#[test]
fn overlapping_payroll_waits_without_backpay_then_resumes_across_store_recovery() {
    let opening = registered_input((0, 1));
    let (root, mut s) = start_world(&opening, (0, 1), "overlapping-group");
    advance(&mut s, 8 * 3600);
    apply(&mut s, Caller::Account(3), Action::ServiceDelivery { command: D::Accept { household: 7, crew: 1 } });

    // Persist a genuine in-progress P5 boundary before payroll's member acquisition.
    let mut last = advance_page(&mut s, 9 * 3600);
    for _ in 0..512 {
        let p = s.engine().unwrap().timeline().unwrap().progress;
        if p.at == 9 * 3600 && format!("{:?}", p.phase) == "Some(P5)" { break; }
        last = advance_page(&mut s, 9 * 3600);
    }
    let progress = s.engine().unwrap().timeline().unwrap().progress;
    assert_eq!((progress.at, format!("{:?}", progress.phase)), (9 * 3600, "Some(P5)".to_owned()));
    cold(&root, s, &last, "partial-p5", 0, 0);
    let mut s = Store::open(&root.join("world"), RULES).unwrap();
    advance(&mut s, 9 * 3600);
    let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!((p.work_seconds, p.accrued.value(), p.remainder), (0, 0, 0));
    assert_eq!(s.engine().unwrap().world().view().member_activity.unwrap().active_count, 1);

    advance(&mut s, 33_900);
    let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!((p.work_seconds, p.accrued.value(), p.remainder), (0, 0, 0));
    assert_eq!(s.engine().unwrap().world().view().member_activity.unwrap().active_count, 1);
    advance(&mut s, DAY);
    let settled = apply(&mut s, Caller::System, Action::TownSettle { profile_id: 1 });
    let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!((p.work_seconds, p.accrued.value(), p.paid.value(), p.remainder), (27_300, 75, 75, 3_000));
    let town = s.engine().unwrap().world().view().town.unwrap();
    assert_eq!(town.state.last_summary.produced.value(), 94);
    assert_eq!(town.state.recipe_output_remainders.as_slice()[0].numerator, 22_800);
    assert_eq!(town.state.tax_cursors.as_slice()[0].through_day, 1);
    cold(&root, s, &settled, "wal-day1", 27_300, 75);

    let mut s = Store::open(&root.join("world"), RULES).unwrap();
    advance(&mut s, 2 * DAY);
    let day2_settlement = apply(&mut s, Caller::System, Action::TownSettle { profile_id: 1 });
    let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
    assert_eq!((p.work_seconds, p.accrued.value(), p.paid.value(), p.remainder), (56_100, 155, 155, 3_000));
    let e = s.engine().unwrap(); let v = e.world().view();
    assert_eq!(v.stocks.iter().find(|s|s.id==15).unwrap().quantity.value(), 806);
    assert_eq!(v.stocks.iter().find(|s|s.id==3002).unwrap().quantity.value(), 76_000);
    let town = v.town.unwrap();
    let tax = town.state.tax_cursors.as_slice()[0];
    assert_eq!(tax.through_day, 2);
    assert_eq!((tax.cumulative_assessed.value(), tax.retained_earnings_cents), (698, 699));
    assert_eq!(town.state.last_summary.produced.value(), 100);
    assert_eq!(town.state.recipe_output_remainders.as_slice()[0].numerator, 22_800);
    s.checkpoint().unwrap();
    cold(&root, s, &day2_settlement, "checkpoint-day2", 56_100, 155);
}

/// Verify Store failures preserve the original receipt/stock and permit one normal retry.
#[test]
fn interrupted_member_and_payroll_write_is_recovered_only_by_original_ticket() {
    for (label, fault) in [("partial-wal", WritePoint::WalPayloadHalf), ("lost-reply", WritePoint::CurrentRenamed)] {
        let raw = registered_input((2, 3));
        let (root, mut s) = start_world(&raw, (2, 3), &format!("write-{label}"));
        advance(&mut s,9*3600);
        s.checkpoint().unwrap();
        let before = s.state_hash().unwrap(); let seq = s.committed_sequence();
        let (scope, revision) = s.engine().unwrap().world().scope_revision();
        let ticket = Envelope { world: scope.world(), branch: scope.branch(), operation_id: s.committed_sequence() + 1,
            expected_revision: revision, actions: vec![Action::Timeline { command: T::Advance { target: 10*3600, limit: 1 } }] };
        let result = s.execute_with_observer(Caller::System, ticket.clone(), |point| if point == fault { Err(std::io::Error::other("retained #155 write interruption")) } else { Ok(()) });
        assert!(result.is_err()); fs::write(root.join("write-failure.txt"), format!("{result:?}")).unwrap(); drop(s);
        let mut s = Store::open(&root.join("world"), RULES).unwrap();
        if fault == WritePoint::WalPayloadHalf { assert_eq!(s.committed_sequence(), seq); assert_eq!(s.state_hash().unwrap(), before); }
        else { assert_eq!(s.committed_sequence(), seq + 1); assert_ne!(s.state_hash().unwrap(), before); }
        let retry = s.execute(Caller::System, ticket.clone()).unwrap(); assert!(retry.model.is_ok());
        let committed = s.committed_sequence(); let hash = s.state_hash().unwrap();
        assert!(s.execute(Caller::System, ticket).unwrap().model.is_ok());
        assert_eq!(s.committed_sequence(), committed); assert_eq!(s.state_hash().unwrap(), hash);
        let p=&s.engine().unwrap().timeline().unwrap().payroll[0];
        assert_eq!((p.work_seconds,p.accrued.value(),p.paid.value()),(3600,10,0));
        advance(&mut s,10*3600);
        assert_eq!(s.engine().unwrap().member_activity_summary().unwrap().active_count,1);
        s.checkpoint().unwrap();
    }
}


#[test]
fn two_groups_resume_a_committed_first_begin_from_the_same_p5_checkpoint(){
    let mut raw=registered_input((2,3));
    let mut second=raw["payroll_members"]["bindings"][0].clone();
    second["spec"]["id"]=json!(22);second["members"]=json!({"start":3,"end":4});second["source"]["id"]=json!(15502);
    raw["payroll_members"]["bindings"].as_array_mut().unwrap().push(second);
    let root=evidence_root().join("two-groups-p5");let mut s=create_store(&raw,&root);
    apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});
    for id in [21,22]{apply(&mut s,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id,..payroll_spec()}}});}
    advance(&mut s,8*3600);
    let(scope,revision)=s.engine().unwrap().world().scope_revision();
    let narrow=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![Action::Timeline{command:T::Advance{target:9*3600,limit:1}}]};
    let hash=s.state_hash().unwrap();let seq=s.committed_sequence();
    assert!(matches!(s.execute(Caller::System,narrow),Err(mycity_storage::Error::Invalid("timeline_page_limit_too_small"))));
    assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
    assert!(s.continuation_status().unwrap().is_none());
    let mut last=None;
    for _ in 0..64{
        last=Some(apply(&mut s,Caller::System,Action::Timeline{command:T::Advance{target:9*3600,limit:256}}));
        let p=s.engine().unwrap().timeline().unwrap().progress;
        if p.phase==Some(mycity_core::schedule::Phase::P5)&&p.at==9*3600&&s.engine().unwrap().member_activity_summary().unwrap().active_count==1{break;}
    }
    let progress=s.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(progress.phase,Some(mycity_core::schedule::Phase::P5));assert_eq!(progress.at,9*3600);
    assert!(progress.last_complete<9*3600);
    s.checkpoint().unwrap();
    cold(&root,s,&last.unwrap(),"first-begin-p5",0,0);
    let mut s=Store::open(&root.join("world"),RULES).unwrap();
    let hash=s.state_hash().unwrap();let(scope,revision)=s.engine().unwrap().world().scope_revision();
    let refused=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![Action::TownSettle{profile_id:1}]};
    assert!(s.execute(Caller::System,refused).unwrap().model.is_err());assert_eq!(s.state_hash().unwrap(),hash);
    advance(&mut s,9*3600);assert_eq!(s.engine().unwrap().member_activity_summary().unwrap().active_count,2);
    let last=advance(&mut s,10*3600);
    for p in &s.engine().unwrap().timeline().unwrap().payroll{assert_eq!((p.work_seconds,p.accrued.value(),p.remainder),(3600,10,0));}
    cold(&root,s,&last,"two-actual-groups-wal",3600,0);
}


#[test]
fn original_96_page_ceiling_is_reserved_exactly_and_excess_is_refused_before_work(){
    for count in [4,5]{
        let raw=registered_input((2,3));let(root,mut s)=start_world(&raw,(2,3),&format!("pages-{}",92+count));
        advance(&mut s,8*3600);
        let first_id=s.engine().unwrap().timeline().unwrap().claims.last().map_or(1,|c|c.id+1);
        apply(&mut s,Caller::System,Action::Timeline{command:T::Claims{first_id,count,payer:3,payee:2,amount:Money::new(1).unwrap(),due:9*3600}});
        let(scope,revision)=s.engine().unwrap().world().scope_revision();
        let env=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![Action::Timeline{command:T::Advance{target:9*3600,limit:1}}]};
        let(progress,steps)=s.engine().unwrap().timeline_quote(Caller::System,&env).unwrap();assert_eq!((progress.at,steps),(9*3600,92+count as usize));
        let hash=s.state_hash().unwrap();let seq=s.committed_sequence();
        if count==5{
            assert!(matches!(s.execute(Caller::System,env),Err(mycity_storage::Error::Invalid("timeline_page_limit_too_small"))));
            assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);assert!(s.continuation_status().unwrap().is_none());
        }else{
            assert!(s.execute(Caller::System,env.clone()).unwrap().model.is_ok());
            assert_eq!(s.continuation_status().unwrap().unwrap().remaining_pages,95);
            s.checkpoint().unwrap();cold(&root,s,&env,"partial-p0-exact-bound",0,0);
            let mut s=Store::open(&root.join("world"),RULES).unwrap();
            assert_eq!(s.continuation_status().unwrap().unwrap().remaining_pages,95);
            advance(&mut s,9*3600);assert!(s.continuation_status().unwrap().is_none());
            assert_eq!(s.engine().unwrap().member_activity_summary().unwrap().active_count,1);
        }
    }
}

/// Read-only replay of a retained normal-game checkpoint; never rewrites it.
#[test]#[ignore]
fn retained_payroll_checkpoint_restore_stage_probe(){
    let cp:Value=serde_json::from_slice(&fs::read(std::env::var_os("MYCITY_PAYROLL_CHECKPOINT_PROBE").unwrap()).unwrap()).unwrap();
    let bytes:Vec<u8>=serde_json::from_value(cp["engine"].clone()).unwrap();
    let mut e=mycity_core::transaction::Engine::restore_checkpoint(&bytes).expect("checkpoint decode");
    let mut check=e.begin_service_needs_restore().expect("begin needs source lookup");
    for(index,operation)in e.service_needs_receipt_operations().into_iter().enumerate(){if let Some(operation)=operation{
        let original=cp["receipts"].as_array().unwrap().iter().find(|r|r["operation_id"]==operation).unwrap();
        let original:mycity_core::transaction::JournalRecord=serde_json::from_value(original.clone()).unwrap();
        e.check_service_needs_restore_receipt(&mut check,index,&original).expect("original needs receipt");
        println!("NEEDS_REF index={index} operation={operation}");
    }}
    e.finish_service_needs_restore(check).expect("finish needs source lookup");
}
