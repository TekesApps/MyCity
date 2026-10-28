//! #151 original Store recovery of the real construction/TownSettle path.
//! The explicit laboratory opening matches the frozen core business test.
//! Cold children open actual WAL/checkpoints and retry original envelopes.
use mycity_core::{
    construction::{EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material,
        Project, OPERATION_PREFIX},
    timeline::{Command as T, DAY},
    transaction::{Action, Caller, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const RULES: &str = "asset-capital-original-store-v1";
const PROBE: &str = "MYCITY_ASSET_CAPITAL_COLD_PROBE";
const SHIFT: u64 = 61_200;
static NEXT: AtomicU64 = AtomicU64::new(1_000_000);

fn opening() -> Vec<u8> {
    let mut input: Value = serde_json::from_slice(
        include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    input["stocks"][0]["owner"] = json!(3);
    input["sources"].as_array_mut().unwrap().push(
        json!({"id":6,"kind":"inventory_capital","amount":1000}));
    input["stocks"].as_array_mut().unwrap().push(json!({
        "id":2,"owner":3,"commodity":"product","quantity":100,"basis":1000,"source":6
    }));
    let mut project = Project::new(7, Kind::Build, 3, 4, 1, 80, 1, 1,
        Money::new(1_000_000).unwrap(), Money::new(3_600).unwrap(), 28_800,
        &[
            Gate { id:1, category:GateCategory::OwnerConsent, evidence_id:0,
                party:2, decided:GateDecision::Pending },
            Gate { id:2, category:GateCategory::Permit, evidence_id:0,
                party:1, decided:GateDecision::Pending },
        ],
        &[Material { stock_id:1, required:Quantity::new(1000).unwrap(),
            consumed:Quantity::ZERO, basis_per_unit:Money::new(100_000).unwrap(),
            basis_remainder:0 }], GameTime::ZERO).unwrap();
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = 4;
    project.waste_receiver_resource = 82;
    project.waste_units = 2;
    project.waste_fee = Money::new(250).unwrap();
    input["construction"] = serde_json::to_value(project).unwrap();
    let person = json!({"kind":"person","id":7001});
    let treasury = json!({"kind":"treasury","id":1});
    input["town"] = json!({
        "schema_version":1,"id":1,
        "households":[{"id":1,"account":2,"households":1,"need_good":1,
            "need_per_household":10,"reference_price_per_q":100}],
        "employers":[{"account":3,"worker_limit":1}],
        "shops":[{"shop_id":1,"account":3,"stock_id":2,"good":1,"shelf_price_per_q":100}],
        "recipes":[],
        "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,
            "reachable":true,"route_source":77,"freight_account":1,"utility_account":1,
            "available_capacity":100,"freight_per_q":0,"utility_surcharge_per_q":0}],
        "equity_charters":[{
            "company":3,
            "policy":{"policy_id":77,"version":1,"cents_per_share":100,
                "max_issued_shares":10000,"max_direct_shareholders":8,
                "ordinary_shares_only":true,"cross_company_holding_allowed":false,
                "secondary_market_enabled":false},
            "distribution_authority":person,
            "investors":[{"party":person,"account":2,"opening_shares":8410},
                {"party":treasury,"account":1,"opening_shares":0}],
            "opening_sources":[{"source_id":3,"party":person},
                {"source_id":5,"party":person},{"source_id":6,"party":person}]
        }],
        "tax_rules":[{"company":3,"policy_id":77,"kind":"daily_realized_market_margin",
            "authority_account":1,"rate_basis_points":5000}],
        "asset_capital":{"version":2,"project":7,"owner":3,"disposal":"ZeroRecovery","policy":{
            "revision":1,"source":{"id":151,"revision":1},"method":"StraightLine",
            "clock":"LocalDevelopment","life_seconds":259200,
            "cost_quantum":1000,"residual":28000}}
    });
    { let capital=input["town"].as_object_mut().unwrap().remove("asset_capital"); if let Some(capital)=capital { input["asset_capital"]=capital; } serde_json::to_vec(&input).unwrap() }
}

fn directory(label: &str) -> PathBuf {
    let base = fs::canonicalize("/Volumes/HP P900/dev/MyCity/work").expect("original SSD work");
    let root = PathBuf::from(std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
        .expect("explicit retained artifact root required"));
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    assert!(root.starts_with(&base) && root != base);
    let dir = root.join(format!("asset-capital-{label}-{}-{}",std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&dir).unwrap();
    eprintln!("ASSET_CAPITAL_STORE_RETAINED={}",dir.display());
    dir
}

fn create(world: &Path) -> Store {
    Store::create(world,&opening(),vec![Resource::new(80,1).unwrap(),
        Resource::new(81,2).unwrap(),Resource::new(82,10).unwrap()],RULES).unwrap()
}

fn request(s: &Store, action: Action, id: u64) -> Envelope {
    let v = s.engine().unwrap().world().view();
    Envelope { world:v.scope.world(),branch:v.scope.branch(),operation_id:id,
        expected_revision:v.revision,actions:vec![action] }
}

fn run(s: &mut Store, actor: u32, action: Action) -> Envelope {
    let envelope = request(s,action,NEXT.fetch_add(1,Ordering::Relaxed));
    let caller = if actor==0 { Caller::System } else { Caller::Account(actor) };
    let result = s.execute(caller,envelope.clone()).unwrap();
    assert!(result.model.is_ok(),"{result:?}");
    assert!(result.has_persistent_receipt);
    envelope
}

fn effect(s: &Store, command: C) -> Envelope {
    let p = s.engine().unwrap().world().view().construction.unwrap();
    request(s,Action::Construction { command },
        OPERATION_PREFIX | (p.id << 32) | u64::from(p.next_effect))
}

fn perform_effect(s: &mut Store, actor: u32, command: C) {
    let envelope = effect(s,command);
    let result = s.execute(Caller::Account(actor),envelope).unwrap();
    assert!(result.model.is_ok(),"{result:?}");
}

fn approve(s: &mut Store) {
    run(s,2,Action::Construction { command:C::DecideGate {
        gate_id:1,decision:GateDecision::Accepted,evidence_id:11 } });
    run(s,1,Action::Construction { command:C::DecideGate {
        gate_id:2,decision:GateDecision::Accepted,evidence_id:12 } });
    run(s,3,Action::Construction { command:C::Prepare });
}

fn advance(s: &mut Store, at: u64) {
    let p = &s.engine().unwrap().timeline().unwrap().progress;
    if p.last_complete==at && p.target.is_none() && p.phase.is_none() { return; }
    for _ in 0..100 {
        run(s,0,Action::Timeline { command:T::Advance { target:at,limit:8192 } });
        if s.engine().unwrap().timeline().unwrap().progress.target.is_none() { return; }
    }
    panic!("original Timeline did not reach {at}");
}

fn inspection_ready(s: &mut Store) {
    run(s,0,Action::Timeline { command:T::Enable });
    run(s,3,Action::Construction { command:C::File });
    approve(s);
    perform_effect(s,3,C::Start);
    advance(s,SHIFT);
    perform_effect(s,4,C::Work { at:GameTime::new(SHIFT as i64).unwrap(),
        elapsed_seconds:SHIFT as u32,worker_seconds:28_800 });
    perform_effect(s,4,C::SubmitInspection);
}

fn interrupted(s: &mut Store, actor: u32, envelope: &Envelope, boundary: WritePoint) {
    let mut reached = false;
    let caller = if actor==0 { Caller::System } else { Caller::Account(actor) };
    let result = s.execute_with_observer(caller,envelope.clone(),|point| {
        if point==boundary {
            reached = true;
            Err(std::io::Error::other("asset capital retained write interruption"))
        } else { Ok(()) }
    });
    assert!(reached,"original Store did not reach {boundary:?}: {result:?}");
    assert!(matches!(result,Err(mycity_storage::Error::Io(ref e))
        if e.to_string()=="asset capital retained write interruption"));
}

#[derive(Clone, Serialize, Deserialize)]
struct Retry { actor: u32, envelope: Envelope }

#[derive(Serialize, Deserialize)]
struct Probe {
    world: PathBuf,
    hash: String,
    sequence: u64,
    stage: String,
    retries: Vec<Retry>,
    checkpoint: bool,
    result: PathBuf,
}

fn cash(s: &Store, account: u32) -> i64 {
    s.engine().unwrap().world().view().accounts.iter()
        .find(|a|a.id==account).unwrap().cash.value()
}

fn assert_stage(s: &Store, stage: &str) {
    // All expected business values are literal independent constants, not a
    // snapshot produced by another implementation of the depreciation formula.
    let (day,charged,stop,expense,consumed,retained,assessed,cash_rows,product,spent,capacity) = match stage {
        "commission" => (0,0,None,0,0,0,0,
            [1_000_000,200_000,571_200,1_228_800],(100,1000),29_200,4),
        "day1" => (1,116,None,0,0,392,392,
            [1_000_000,199_000,572_200,1_228_800],(90,900),29_200,4),
        "withdrawal" => (1,116,Some(129_600),0,0,392,392,
            [1_000_000,199_000,572_200,1_228_800],(90,900),29_200,2),
        "unconsumed-expense" => (2,316,Some(129_600),28_800,0,742,742,
            [1_000_392,198_000,544_008,1_257_600],(80,800),58_000,2),
        "day3" => (3,316,Some(129_600),28_800,28_800,-27_158,742,
            [1_000_742,197_000,544_658,1_257_600],(70,700),58_000,2),
        "closed-unconsumed" => (3,316,Some(129_600),29_050,28_800,-27_158,742,
            [1_000_742,197_000,544_408,1_257_850],(70,700),58_250,2),
        "closed-day4" => (4,316,Some(129_600),29_050,29_050,-55_392,742,
            [1_000_742,196_000,545_408,1_257_850],(60,600),58_250,2),
        _ => panic!("unknown business checkpoint {stage}"),
    };
    let world = s.engine().unwrap().world().view();
    let town = world.town.unwrap();
    let capital = serde_json::to_value(s.engine().unwrap().asset_capital_summary()).unwrap();
    assert_eq!(capital["basis"],json!(29_200));
    assert_eq!(capital["commissioned_at"],json!(61_200));
    assert_eq!(capital["charged"],json!(charged));
    assert_eq!(capital["stopped_at"],json!(stop));
    assert_eq!(capital["subsequent_expense"],json!(expense));
    assert_eq!(capital["charged_subsequent_expense"],json!(consumed));
    let retired=stage.starts_with("closed-");
    assert_eq!(capital["retired_at"],if retired {json!(3*DAY)} else {Value::Null});
    assert_eq!(capital["retirement_cost"],json!(if retired {28_884} else {0}));
    assert_eq!(capital["charged_retirement_cost"],json!(if stage=="closed-day4" {28_884} else {0}));
    if retired {
        assert_eq!(world.construction.unwrap().phase,mycity_core::construction::Phase::Closed);
        assert_eq!(s.engine().unwrap().resources().iter().find(|r|r.id==80).unwrap().occupied,0);
    }
    let cursor = &town.state.tax_cursors.as_slice()[0];
    assert_eq!(cursor.through_day,day);
    assert_eq!(cursor.retained_earnings_cents,retained);
    assert_eq!(cursor.cumulative_assessed,Money::new(assessed).unwrap());
    assert_eq!(town.state.settled_day,day);
    assert_eq!(world.construction.unwrap().spent,Money::new(spent).unwrap());
    let raw = world.stocks.iter().find(|stock|stock.id==1).unwrap();
    assert_eq!((raw.quantity.value(),raw.basis.value()),(599_000,239_600));
    let goods = world.stocks.iter().find(|stock|stock.id==2).unwrap();
    assert_eq!((goods.quantity.value(),goods.basis.value()),product);
    for (index,expected) in cash_rows.into_iter().enumerate() {
        assert_eq!(cash(s,index as u32+1),expected,"account {}",index+1);
    }
    assert_eq!(world.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_000_000);
    assert_eq!(s.engine().unwrap().resources().iter().find(|r|r.id==81).unwrap().capacity,capacity);
    let timeline = s.engine().unwrap().timeline().unwrap();
    for (period,basis,tax) in [(1,784,392),(2,700,350)] {
        if day>=period {
            let claim = timeline.claims.iter().find(|claim|
                timeline.tax_source_for_claim(claim).is_some_and(|source|source.period==period)).unwrap();
            let source = timeline.tax_source_for_claim(claim).unwrap();
            assert_eq!(source.taxable_basis,Money::new(basis).unwrap());
            assert_eq!(claim.principal,Money::new(tax).unwrap());
            assert_eq!(source.assessed,Money::new(tax).unwrap());
            assert_eq!(claim.paid,if day>period { Money::new(tax).unwrap() } else { Money::ZERO });
        }
    }
    assert!(!timeline.claims.iter().any(|claim|
        timeline.tax_source_for_claim(claim).is_some_and(|source|source.period==3)));
}

fn cold(dir: &Path, world: &Path, stage: &str, checkpoint: bool, retries: &[Retry]) {
    let name = format!("{stage}-{}",if checkpoint { "wal" } else { "checkpoint" });
    let folder = dir.join(name);
    fs::create_dir(&folder).unwrap();
    let s = Store::open(world,RULES).unwrap();
    assert_stage(&s,stage);
    let config = Probe { world:world.to_owned(),hash:s.state_hash().unwrap(),
        sequence:s.committed_sequence(),stage:stage.to_owned(),retries:retries.to_vec(),
        checkpoint,result:folder.join("result.json") };
    drop(s);
    let input = folder.join("probe.json");
    fs::write(&input,serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact","asset_capital_store_cold_probe","--ignored","--nocapture","--test-threads=1"])
        .env(PROBE,&input).stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(folder.join("stdout.log")).unwrap()))
        .stderr(Stdio::from(fs::File::create(folder.join("stderr.log")).unwrap()))
        .spawn().unwrap();
    fs::write(folder.join("launch.json"),serde_json::to_vec(&json!({
        "pid":child.id(),"timeout_seconds":60,"probe":input})).unwrap()).unwrap();
    let deadline = Instant::now()+Duration::from_secs(60);
    let (status,timed_out) = loop {
        if let Some(status) = child.try_wait().unwrap() { break (status,false); }
        if Instant::now()>=deadline {
            child.kill().unwrap();
            break (child.wait().unwrap(),true);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    fs::write(folder.join("exit.json"),serde_json::to_vec(&json!({
        "pid":child.id(),"code":status.code(),"timed_out":timed_out})).unwrap()).unwrap();
    assert!(!timed_out && status.success(),"retained cold evidence {}",folder.display());
    let result: Value = serde_json::from_slice(&fs::read(&config.result).unwrap()).unwrap();
    assert_eq!(result["sequence"],json!(config.sequence));
    assert_eq!(result["hash"],json!(config.hash));
    assert_ne!(result["pid"],json!(std::process::id()));
}

fn cold_pair(dir: &Path, world: &Path, stage: &str, retries: &[Retry]) {
    // First independent process reads uncheckpointed new WAL and publishes the
    // original Store checkpoint. Second process opens that checkpoint and retries
    // archived original receipts. Both must match independent business constants.
    cold(dir,world,stage,true,retries);
    cold(dir,world,stage,false,retries);
}

#[test]
fn actual_capital_stop_and_unconsumed_expenses_survive_cold_wal_checkpoint_and_lost_replies() {
    let dir = directory("business");
    let world = dir.join("world");
    let mut s = create(&world);
    inspection_ready(&mut s);
    let commission = effect(&s,C::AcceptInspection { inspector:1,accepted:true });
    interrupted(&mut s,1,&commission,WritePoint::Applied);
    drop(s);
    let mut retries = vec![Retry { actor:1,envelope:commission }];
    cold_pair(&dir,&world,"commission",&retries);

    let mut s = Store::open(&world,RULES).unwrap();
    advance(&mut s,DAY);
    let day1 = request(&s,Action::TownSettle { profile_id:1 },NEXT.fetch_add(1,Ordering::Relaxed));
    interrupted(&mut s,0,&day1,WritePoint::Applied);
    drop(s);
    retries.push(Retry { actor:0,envelope:day1 });
    cold_pair(&dir,&world,"day1",&retries);

    let mut s = Store::open(&world,RULES).unwrap();
    advance(&mut s,DAY+DAY/2);
    run(&mut s,3,Action::Construction { command:C::BeginDemolition { waste_receiver:4 } });
    approve(&mut s);
    let withdrawal = effect(&s,C::Start);
    interrupted(&mut s,3,&withdrawal,WritePoint::Applied);
    drop(s);
    retries.push(Retry { actor:3,envelope:withdrawal });
    cold_pair(&dir,&world,"withdrawal",&retries);

    let mut s = Store::open(&world,RULES).unwrap();
    advance(&mut s,2*DAY);
    let day2 = run(&mut s,0,Action::TownSettle { profile_id:1 });
    retries.push(Retry { actor:0,envelope:day2 });
    advance(&mut s,2*DAY+SHIFT);
    let work = effect(&s,C::Work { at:GameTime::new((2*DAY+SHIFT) as i64).unwrap(),
        elapsed_seconds:104_400,worker_seconds:28_800 });
    interrupted(&mut s,4,&work,WritePoint::Applied);
    drop(s);
    retries.push(Retry { actor:4,envelope:work });
    cold_pair(&dir,&world,"unconsumed-expense",&retries);

    let mut s = Store::open(&world,RULES).unwrap();
    advance(&mut s,3*DAY);
    let day3 = request(&s,Action::TownSettle { profile_id:1 },NEXT.fetch_add(1,Ordering::Relaxed));
    interrupted(&mut s,0,&day3,WritePoint::Applied);
    drop(s);
    retries.push(Retry { actor:0,envelope:day3 });
    cold_pair(&dir,&world,"day3",&retries);
    let mut s=Store::open(&world,RULES).unwrap();
    let waste=perform_effect(&mut s,4,C::AcceptWaste);
    let _=waste;
    let close=effect(&s,C::Close);
    interrupted(&mut s,3,&close,WritePoint::Applied);drop(s);
    retries.push(Retry{actor:3,envelope:close});
    cold_pair(&dir,&world,"closed-unconsumed",&retries);
    let mut s=Store::open(&world,RULES).unwrap();advance(&mut s,4*DAY);
    let day4=request(&s,Action::TownSettle{profile_id:1},NEXT.fetch_add(1,Ordering::Relaxed));
    interrupted(&mut s,0,&day4,WritePoint::Applied);drop(s);
    retries.push(Retry{actor:0,envelope:day4});
    cold_pair(&dir,&world,"closed-day4",&retries);

}

#[test]
fn interrupted_capital_wal_payload_never_half_publishes_and_original_retry_is_once() {
    let dir = directory("partial-wal");
    let world = dir.join("world");
    let mut s = create(&world);
    inspection_ready(&mut s);
    let before = s.state_hash().unwrap();
    let sequence = s.committed_sequence();
    let commission = effect(&s,C::AcceptInspection { inspector:1,accepted:true });
    interrupted(&mut s,1,&commission,WritePoint::WalPayloadHalf);
    drop(s);
    let mut s = Store::open(&world,RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(),before);
    assert_eq!(s.committed_sequence(),sequence);
    assert!(s.engine().unwrap().asset_capital_summary().is_none());
    assert_eq!(s.engine().unwrap().resources().iter().find(|r|r.id==81).unwrap().capacity,2);
    let result = s.execute(Caller::Account(1),commission.clone()).unwrap();
    assert!(result.model.is_ok() && result.has_persistent_receipt);
    assert_eq!(s.committed_sequence(),sequence+1);
    assert_stage(&s,"commission");
    drop(s);
    cold_pair(&dir,&world,"commission",&[Retry { actor:1,envelope:commission }]);
}

#[test]
#[ignore = "parent invokes an independent original Store process with retained inputs and outputs"]
fn asset_capital_store_cold_probe() {
    let input = std::env::var_os(PROBE).expect("parent cold probe path");
    let config: Probe = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    assert!(config.world.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let mut s = Store::open(&config.world,RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(),config.hash);
    assert_eq!(s.committed_sequence(),config.sequence);
    assert_stage(&s,&config.stage);
    for retry in config.retries {
        let archived = s.receipt(retry.envelope.operation_id).unwrap().unwrap();
        assert!(archived.outcome.is_ok());
        let caller = if retry.actor==0 { Caller::System } else { Caller::Account(retry.actor) };
        let out = s.execute(caller,retry.envelope).unwrap();
        assert_eq!(out.model,archived.outcome);
        assert!(out.has_persistent_receipt);
        assert_eq!(s.committed_sequence(),config.sequence);
        assert_eq!(s.state_hash().unwrap(),config.hash);
        assert_stage(&s,&config.stage);
    }
    if config.checkpoint {
        s.checkpoint().unwrap();
        assert_eq!(s.state_hash().unwrap(),config.hash);
        assert_stage(&s,&config.stage);
    }
    fs::write(config.result,serde_json::to_vec(&json!({"pid":std::process::id(),
        "sequence":s.committed_sequence(),"hash":s.state_hash().unwrap(),
        "stage":config.stage,"checkpoint_published":config.checkpoint})).unwrap()).unwrap();
}
