//! #21 finite joint Store acceptance: actual Payroll/service/Town plus existing asset costs.
//! Worker-seconds belong to the aggregate construction operation and do not identify a resident.
//! Run only through the root's serial HP P900 acceptance plan.
use mycity_core::{
    boot::{Command as B, Facility},
    construction::{self, EngineCommand as C, GateDecision},
    housing,
    service_delivery_authority::{Command as D, Motion},
    timeline::{Command as T, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[path = "../../../tests/fixtures/finite-city/asset_opening.rs"]
mod fixture;
const RULES: &str = "finite-city-local-boot-v1";
const SHIFT: u64 = 61_200;
static NEXT: AtomicU64 = AtomicU64::new(7_000_000);
fn payroll_spec() -> PayrollSpec {
    PayrollSpec {
        id: 21, payer: 7, payee: 2, home_city: 0,
        hourly_cents: Money::new(10).unwrap(), cycle: Cycle::Daily, ends_at: 3 * DAY,
    }
}
fn evidence_root() -> PathBuf {
    let root = PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ASSET_ROOT").expect("retained HP P900 evidence root"));
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    root
}
fn create(raw: &Value, root: &Path) -> Store {
    fs::create_dir(root).unwrap();
    let bytes = serde_json::to_vec(raw).unwrap();
    fs::write(root.join("opening.json"), &bytes).unwrap();
    Store::create(&root.join("world"), &bytes, fixture::resources(), RULES).unwrap()
}
fn request(s: &Store, action: Action) -> Envelope {
    let (scope, revision) = s.engine().unwrap().world().scope_revision();
    Envelope {
        world: scope.world(), branch: scope.branch(),
        operation_id: NEXT.fetch_add(1, Ordering::Relaxed),
        expected_revision: revision, actions: vec![action],
    }
}
fn apply(s: &mut Store, caller: Caller, action: Action) -> Envelope {
    let ticket = request(s, action);
    let result = s.execute(caller, ticket.clone()).unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    ticket
}
fn effect_ticket(s: &Store, command: C) -> Envelope {
    let project = s.engine().unwrap().world().view().construction.unwrap();
    let mut ticket = request(s, Action::Construction { command });
    ticket.operation_id = construction::OPERATION_PREFIX
        | (u64::from(project.id) << 32)
        | u64::from(project.next_effect);
    ticket
}
fn effect(s: &mut Store, actor: u32, command: C) -> Envelope {
    let ticket = effect_ticket(s, command);
    let result = s.execute(Caller::Account(actor), ticket.clone()).unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    ticket
}
fn command(s: &mut Store, actor: u32, command: C) {
    apply(s, Caller::Account(actor), Action::Construction { command });
}
fn advance(s: &mut Store, target: u64) -> Envelope {
    for _ in 0..8192 {
        let limit = s.continuation_status().unwrap().map_or(256, |r| r.page_limit);
        let ticket = apply(s, Caller::System, Action::Timeline { command: T::Advance { target, limit } });
        let p = s.engine().unwrap().timeline().unwrap().progress;
        if p.target.is_none() { assert_eq!(p.last_complete, target); return ticket; }
    }
    panic!("finite timeline unfinished at {target}")
}
fn cash(s: &Store, account: u32) -> i64 {
    s.engine().unwrap().world().view().accounts.iter().find(|a| a.id == account).unwrap().cash.value()
}
fn capital(s: &Store) -> Value {
    serde_json::to_value(s.engine().unwrap().asset_capital_summary()).unwrap()
}
fn begin_world(label: &str) -> (PathBuf, Store) {
    begin_world_from(label, fixture::input())
}
fn begin_world_from(label: &str, raw: Value) -> (PathBuf, Store) {
    let run = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = evidence_root().join(format!("{label}-{}-{run}", std::process::id()));
    let mut s = create(&raw, &root);
    apply(&mut s, Caller::System, Action::Timeline { command: T::Enable });
    let offer: housing::GenesisLeaseOffer = serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();
    apply(&mut s, Caller::Account(2), Action::HousingLease {
        offer_id: 901, offer_sha256: housing::genesis_lease_offer_sha256(offer),
    });
    apply(&mut s, Caller::System, Action::Timeline { command: T::AddPayroll { spec: payroll_spec() } });
    command(&mut s, 7, C::File);
    command(&mut s, 2, C::DecideGate { gate_id: 1, decision: GateDecision::Accepted, evidence_id: 11 });
    command(&mut s, 1, C::DecideGate { gate_id: 2, decision: GateDecision::Accepted, evidence_id: 12 });
    command(&mut s, 7, C::Prepare);
    effect(&mut s, 7, C::Start);
    for (facility, until) in [(Facility::Pump, 60), (Facility::Generator, 120)] {
        apply(&mut s, Caller::System, Action::Boot { command: B::Start { facility, reverse_request: false } });
        advance(&mut s, until);
    }
    (root, s)
}
fn cold(root: &Path, s: Store, replay: &Envelope, actor: u32, label: &str, expect: Value) {
    let input = root.join(format!("{label}-cold.json"));
    fs::write(&input, json!({
        "world": root.join("world"), "hash": s.state_hash().unwrap(),
        "sequence": s.committed_sequence(), "replay": replay,
        "actor": actor, "expect": expect,
    }).to_string()).unwrap();
    drop(s);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "finite_city_asset_cold_probe", "--test-threads=1", "--nocapture"])
        .env("MYCITY_FINITE_CITY_ASSET_COLD", &input)
        .stdout(Stdio::from(fs::File::create(root.join(format!("{label}-cold.log"))).unwrap()))
        .stderr(Stdio::from(fs::File::create(root.join(format!("{label}-cold.stderr"))).unwrap()))
        .spawn().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            fs::write(root.join(format!("{label}-cold-result.json")), json!({"pid":child.id(),"exit":status.code(),"seconds":start.elapsed().as_secs_f64()}).to_string()).unwrap();
            assert!(status.success()); return;
        }
        if start.elapsed() > Duration::from_secs(60) {
            child.kill().unwrap(); child.wait().unwrap(); panic!("cold restore timeout");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore]
fn finite_city_asset_cold_probe() {
    let input: Value = serde_json::from_slice(&fs::read(std::env::var_os("MYCITY_FINITE_CITY_ASSET_COLD").unwrap()).unwrap()).unwrap();
    let mut s = Store::open(Path::new(input["world"].as_str().unwrap()), RULES).unwrap();
    assert!(s.engine().unwrap().asset_capital_is_ready());
    assert!(s.engine().unwrap().payroll_members_is_ready());
    assert!(s.engine().unwrap().service_needs_is_authenticated());
    assert_eq!(s.committed_sequence(), input["sequence"].as_u64().unwrap());
    assert_eq!(s.state_hash().unwrap(), input["hash"].as_str().unwrap());
    let actor = u32::try_from(input["actor"].as_u64().unwrap()).unwrap();
    let caller = if actor == 0 { Caller::System } else { Caller::Account(actor) };
    let ticket: Envelope = serde_json::from_value(input["replay"].clone()).unwrap();
    assert!(s.execute(caller, ticket).unwrap().model.is_ok());
    assert_eq!(s.committed_sequence(), input["sequence"].as_u64().unwrap());
    assert_eq!(s.state_hash().unwrap(), input["hash"].as_str().unwrap());
    let e = s.engine().unwrap();
    let capital = serde_json::to_value(e.asset_capital_summary()).unwrap();
    assert_eq!(capital["basis"], input["expect"]["capital"]["basis"]);
    assert_eq!(capital["charged"], input["expect"]["capital"]["charged"]);
    if let Some(at) = input["expect"]["capital"]["commissioned_at"].as_u64() {
        assert_eq!(capital["commissioned_at"], json!(at));
    }
    assert_eq!(e.world().view().accounts.iter().find(|a|a.id==7).unwrap().cash.value(), input["expect"]["company_cash"].as_i64().unwrap());
    if let Some(expected) = input["expect"]["contractor_cash"].as_i64() {
        assert_eq!(e.world().view().accounts.iter().find(|a|a.id==4).unwrap().cash.value(), expected);
    }
}

/// Actual project, original payroll and service, Boot work, and two Town closes share one Store.
#[test]
fn finite_asset_business_runs_with_actual_service_payroll_and_two_town_closes() {
    let (root, mut s) = begin_world("asset-joint");
    advance(&mut s, 8 * 3600);
    apply(&mut s, Caller::Account(3), Action::ServiceDelivery { command: D::Accept { household: 7, crew: 1 } });
    advance(&mut s, 33_900);
    assert_eq!(s.engine().unwrap().service_needs_summary().unwrap().delivery.unwrap().trips[0].motion, Motion::Finished);

    advance(&mut s, SHIFT);
    let work = effect_ticket(&s, C::Work {
        at: GameTime::new(SHIFT as i64).unwrap(), elapsed_seconds: SHIFT as u32, worker_seconds: 28_800,
    });
    let write_before = s.state_hash().unwrap(); let sequence_before = s.committed_sequence();
    let mut reached = false;
    let result = s.execute_with_observer(Caller::Account(4), work.clone(), |point| {
        if point == WritePoint::WalPayloadHalf { reached = true; Err(std::io::Error::other("retained construction work partial WAL")) } else { Ok(()) }
    });
    assert!(reached, "original Work did not reach partial WAL: {result:?}");
    assert!(result.is_err()); fs::write(root.join("work-partial-wal-failure.txt"), format!("{result:?}")).unwrap(); drop(s);
    let mut s = Store::open(&root.join("world"), RULES).unwrap();
    assert_eq!(s.committed_sequence(), sequence_before); assert_eq!(s.state_hash().unwrap(), write_before);
    assert!(s.execute(Caller::Account(4), work.clone()).unwrap().model.is_ok());
    assert_eq!(s.engine().unwrap().world().view().construction.unwrap().spent.value(), 28_916);
    assert_eq!(cash(&s, 7), 71_184);
    assert_eq!(cash(&s, 4), 1_229_300);
    assert_eq!(capital(&s), Value::Null);
    assert_eq!(s.engine().unwrap().world().view().construction.unwrap().spent.value(), 28_916);

    let inspection = effect(&mut s, 4, C::SubmitInspection);
    let commission = effect_ticket(&s, C::AcceptInspection { inspector: 1, accepted: true });
    assert_eq!(capital(&s), Value::Null);
    let commission_hash = s.state_hash().unwrap(); let commission_seq = s.committed_sequence();
    let mut reached = false;
    let lost = s.execute_with_observer(Caller::Account(1), commission.clone(), |point| {
        if point == WritePoint::CurrentRenamed { reached = true; Err(std::io::Error::other("retained commission lost reply")) } else { Ok(()) }
    });
    assert!(reached, "commission did not reach current publication: {lost:?}");
    assert!(lost.is_err()); fs::write(root.join("commission-lost-reply.txt"), format!("{lost:?}")).unwrap(); drop(s);
    let mut s = Store::open(&root.join("world"), RULES).unwrap();
    assert_eq!(s.committed_sequence(), commission_seq + 1); assert_ne!(s.state_hash().unwrap(), commission_hash);
    let commissioned_hash = s.state_hash().unwrap();
    assert!(s.execute(Caller::Account(1), commission.clone()).unwrap().model.is_ok());
    assert_eq!(s.state_hash().unwrap(), commissioned_hash);
    assert_eq!(capital(&s)["basis"], json!(28_916));
    cold(&root, s, &commission, 1, "commission-replay", json!({
        "capital": {"basis":28_916,"charged":0,"commissioned_at":SHIFT},
        "company_cash":71_184, "contractor_cash":1_229_300,
    }));
    let mut s = Store::open(&root.join("world"), RULES).unwrap();
    // The inspection receipt is also the actual original ticket; replay cannot publish a second cost.
    let _ = inspection;
    assert_eq!(capital(&s)["basis"], json!(28_916));

    for (day, charged, tax, retained, company_cash, raw, electric, water) in [
        (1, 116, 280, 281, 71_955, 806, 168_000, 522_000),
        (2, 516, 440, 441, 72_495, 706, 68_000, 522_000),
    ] {
        advance(&mut s, day * DAY);
        let settlement = apply(&mut s, Caller::System, Action::TownSettle { profile_id: 1 });
        let c = capital(&s);
        assert_eq!(c["charged"], json!(charged)); assert_eq!(c["basis"], json!(28_916));
        assert_eq!(cash(&s, 7), company_cash); assert_eq!(cash(&s, 4), 1_229_300);
        let v = s.engine().unwrap().world().view();
        let cursor = v.town.unwrap().state.tax_cursors.as_slice()[0];
        assert_eq!((cursor.cumulative_assessed.value(), cursor.retained_earnings_cents), (tax, retained));
        for (id, expected) in [(15, raw), (3_002, electric), (3_003, water)] {
            assert_eq!(v.stocks.iter().find(|stock|stock.id==id).unwrap().quantity.value(), expected);
        }
        assert_eq!(v.accounts.iter().map(|account|account.cash.value()).sum::<i64>(), 3_100_000);
        if day == 1 {
            cold(&root, s, &settlement, 0, "wal-day1", json!({
                "capital": {"basis":28_916,"charged":116,"commissioned_at":SHIFT},
                "company_cash":71_955,"contractor_cash":1_229_300,
            }));
            s = Store::open(&root.join("world"), RULES).unwrap();
        } else {
            s.checkpoint().unwrap();
            let p = &s.engine().unwrap().timeline().unwrap().payroll[0];
            assert_eq!((p.work_seconds, p.paid.value(), p.remainder), (56_100, 155, 3_000));
            let v=s.engine().unwrap().world().view();let b=v.boot.unwrap();
            assert_eq!(b.local_construction_exported.iter().map(|q|q.value()).collect::<Vec<_>>(),vec![8_000,8_000]);
            cold(&root, s, &settlement, 0, "checkpoint-day2", json!({
                "capital": {"basis":28_916,"charged":516,"commissioned_at":SHIFT},
                "company_cash":72_495,"contractor_cash":1_229_300,
            }));
            return;
        }
    }
}

/// Actual withdrawal and close keep the existing supply and unsettled cost path intact.
#[test]
fn finite_asset_withdrawal_and_close_preserve_joint_supply_and_costs() {
    let (root, mut s) = begin_world_from("asset-close-joint", fixture::closing_input());
    advance(&mut s, 8 * 3600);
    apply(&mut s, Caller::Account(3), Action::ServiceDelivery { command: D::Accept { household: 7, crew: 1 } });
    advance(&mut s, 33_900);
    advance(&mut s, SHIFT);
    effect(&mut s, 4, C::Work { at: GameTime::new(SHIFT as i64).unwrap(), elapsed_seconds: SHIFT as u32, worker_seconds: 28_800 });
    effect(&mut s, 4, C::SubmitInspection);
    effect(&mut s, 1, C::AcceptInspection { inspector: 1, accepted: true });
    advance(&mut s, 64_800);

    command(&mut s, 7, C::BeginDemolition { waste_receiver: 4 });
    command(&mut s, 2, C::DecideGate { gate_id: 1, decision: GateDecision::Accepted, evidence_id: 21 });
    command(&mut s, 1, C::DecideGate { gate_id: 2, decision: GateDecision::Accepted, evidence_id: 22 });
    command(&mut s, 7, C::Prepare);
    let withdrawal = effect(&mut s, 7, C::Start);
    assert_eq!(capital(&s)["stopped_at"], json!(64_800));
    assert_eq!(s.engine().unwrap().resources().iter().find(|r|r.id==81).unwrap().capacity, 2);

    advance(&mut s, DAY);
    let day1 = apply(&mut s, Caller::System, Action::TownSettle { profile_id: 1 });
    assert_eq!(capital(&s)["charged"], json!(16));
    assert_eq!(cash(&s, 7), 71_955);
    assert_eq!(cash(&s, 4), 1_229_300);
    let v=s.engine().unwrap().world().view();
    let town=v.town.unwrap();let cursor=town.state.tax_cursors.as_slice()[0];
    assert_eq!((cursor.cumulative_assessed.value(),cursor.retained_earnings_cents),(330,331));
    assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);
    cold(&root, s, &day1, 0, "withdrawal-day1", json!({
        "capital":{"basis":28_916,"charged":16,"commissioned_at":SHIFT},
        "company_cash":71_955,"contractor_cash":1_229_300,
    }));
    let mut s=Store::open(&root.join("world"),RULES).unwrap();
    assert!(s.execute(Caller::Account(7),withdrawal).unwrap().model.is_ok());

    advance(&mut s, 147_600);
    let work=effect(&mut s,4,C::Work{at:GameTime::new(147_600).unwrap(),elapsed_seconds:82_800,worker_seconds:28_800});
    assert_eq!(s.engine().unwrap().world().view().construction.unwrap().spent.value(),57_732);
    let waste=effect(&mut s,4,C::AcceptWaste);
    assert_eq!(s.engine().unwrap().world().view().construction.unwrap().spent.value(),57_982);
    let close=effect(&mut s,7,C::Close);
    let c=capital(&s);
    assert_eq!(c["subsequent_expense"],json!(29_066));
    assert_eq!(c["retirement_cost"],json!(28_900));
    assert_eq!(c["charged_retirement_cost"],json!(0));
    let closed_hash=s.state_hash().unwrap();let closed_seq=s.committed_sequence();
    assert!(s.execute(Caller::Account(7),close.clone()).unwrap().model.is_ok());
    assert_eq!(s.state_hash().unwrap(),closed_hash);
    s.checkpoint().unwrap();
    cold(&root,s,&close,7,"closed-checkpoint-replay",json!({
        "capital":{"basis":28_916,"charged":16,"commissioned_at":SHIFT},
        "company_cash":42_889,
    }));
    let mut s=Store::open(&root.join("world"),RULES).unwrap();
    assert_eq!(s.committed_sequence(),closed_seq);
    assert!(s.execute(Caller::Account(4),work).unwrap().model.is_ok());
    assert_eq!(s.engine().unwrap().world().view().construction.unwrap().spent.value(),57_982);
    assert_eq!(s.state_hash().unwrap(),closed_hash);
    assert_eq!(s.committed_sequence(),closed_seq);
    assert!(s.execute(Caller::Account(4),waste).unwrap().model.is_ok());
    assert_eq!(s.state_hash().unwrap(),closed_hash);
    assert_eq!(s.committed_sequence(),closed_seq);
    assert_eq!(capital(&s)["retirement_cost"],json!(28_900));
    advance(&mut s,2*DAY);
    let day2=apply(&mut s,Caller::System,Action::TownSettle{profile_id:1});
    let c=capital(&s);assert_eq!(c["charged_subsequent_expense"],json!(29_066));assert_eq!(c["charged_retirement_cost"],json!(28_900));
    assert_eq!(cash(&s,7),43_379);
    let v=s.engine().unwrap().world().view();let tax=v.town.unwrap().state.tax_cursors.as_slice()[0];
    assert_eq!((tax.cumulative_assessed.value(),tax.retained_earnings_cents),(330,-56_915));
    for(id,q)in[(15,706),(3_002,60_000),(3_003,514_000)]{assert_eq!(v.stocks.iter().find(|x|x.id==id).unwrap().quantity.value(),q);}
    assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);
    cold(&root,s,&day2,0,"closed-day2-replay",json!({
        "capital":{"basis":28_916,"charged":16,"commissioned_at":SHIFT},
        "company_cash":43_379,
    }));
}
