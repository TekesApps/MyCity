//! #151 finite normal Sim role exercise, not native 16GB, speed or M0 acceptance.
//! Creator/reference Store and executable hashing run in the parent tool process;
//! their memory is not reported as shipping Sim role memory.
use mycity_core::{
    construction::{EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material,
        Project, OPERATION_PREFIX},
    timeline::{Command as T, DAY},
    transaction::{Action, Caller, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use mycity_headless::{normal_pool, sim_client::Client, sim_host::{Command, Event}};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{fs, fs::OpenOptions, io::Write, path::{Path, PathBuf}, time::Duration};

const SHIFT: u64 = 61_200;

fn opening() -> Vec<u8> {
    // Same explicit actual-capital opening as asset_capital_runtime/store tests.
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

fn request(s: &Store, action: Action, id: u64) -> Envelope {
    let (scope,revision) = s.engine().unwrap().world().scope_revision();
    Envelope { world:scope.world(),branch:scope.branch(),operation_id:id,
        expected_revision:revision,actions:vec![action] }
}

fn apply(s: &mut Store, actor: u32, action: Action) {
    let envelope = request(s,action,s.committed_sequence()+1);
    let caller = if actor==0 { Caller::System } else { Caller::Account(actor) };
    let result = s.execute(caller,envelope).unwrap();
    assert!(result.model.is_ok(),"{result:?}");
}

fn construction(s: &mut Store, actor: u32, command: C, has_effect: bool) {
    let p = s.engine().unwrap().world().view().construction.unwrap();
    let id = if has_effect { OPERATION_PREFIX | (p.id << 32) | u64::from(p.next_effect) }
        else { s.committed_sequence()+1 };
    let envelope = request(s,Action::Construction { command },id);
    let result = s.execute(Caller::Account(actor),envelope).unwrap();
    assert!(result.model.is_ok(),"{result:?}");
}

fn commission(s: &mut Store) {
    apply(s,0,Action::Timeline { command:T::Enable });
    construction(s,3,C::File,false);
    construction(s,2,C::DecideGate { gate_id:1,decision:GateDecision::Accepted,evidence_id:11 },false);
    construction(s,1,C::DecideGate { gate_id:2,decision:GateDecision::Accepted,evidence_id:12 },false);
    construction(s,3,C::Prepare,false);
    construction(s,3,C::Start,true);
    for _ in 0..100 {
        apply(s,0,Action::Timeline { command:T::Advance { target:SHIFT,limit:8192 } });
        if s.engine().unwrap().timeline().unwrap().progress.target.is_none() { break; }
    }
    assert_eq!(s.engine().unwrap().timeline().unwrap().progress.last_complete,SHIFT);
    construction(s,4,C::Work { at:GameTime::new(SHIFT as i64).unwrap(),
        elapsed_seconds:SHIFT as u32,worker_seconds:28_800 },true);
    construction(s,4,C::SubmitInspection,true);
    construction(s,1,C::AcceptInspection { inspector:1,accepted:true },true);
}

fn copy_world(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() { copy_world(&entry.path(),&destination); }
        else {
            fs::copy(entry.path(),&destination).unwrap();
            assert_eq!(fs::read(entry.path()).unwrap(),fs::read(destination).unwrap());
        }
    }
}

fn event(c: &mut Client, evidence: &Path) -> Event {
    let event = c.next(Duration::from_secs(15)).unwrap();
    writeln!(OpenOptions::new().create(true).append(true)
        .open(evidence.join("events.jsonl")).unwrap(),"{}",serde_json::to_string(&event).unwrap()).unwrap();
    event
}

fn execute(c: &mut Client, reference: &mut Store, evidence: &Path, id: &mut u64,
    envelope: Envelope) {
    c.submit(*id,Command::Execute { envelope:envelope.clone() }).unwrap();
    let out = event(c,evidence);
    assert_eq!(out.id,*id);
    assert_eq!(out.kind,"result");
    *id += 1;
    let expected = reference.execute(Caller::System,envelope).unwrap();
    assert!(expected.model.is_ok(),"{expected:?}");
    assert_eq!(out.payload["Ok"]["model"],serde_json::to_value(expected.model).unwrap());
}

fn status(c: &mut Client, reference: &Store, evidence: &Path, id: &mut u64,
    observations: &mut Vec<Value>) {
    c.submit(*id,Command::Status).unwrap();
    let out = event(c,evidence);
    assert_eq!(out.id,*id);
    *id += 1;
    let payload = out.payload.get("Ok").unwrap();
    assert_eq!(payload["source_sha256"],env!("MYCITY_SOURCE_SHA256"));
    assert_eq!(payload["state_sha256"],reference.state_hash().unwrap());
    assert_eq!(payload["sequence"],reference.committed_sequence());
    observations.push(serde_json::to_value(out).unwrap());
}

fn day(c: &mut Client, reference: &mut Store, evidence: &Path, id: &mut u64,
    number: u64) -> Envelope {
    let target = number*DAY;
    for _ in 0..100 {
        let progress = reference.engine().unwrap().timeline().unwrap().progress;
        if progress.last_complete==target && progress.target.is_none() { break; }
        let envelope = request(reference,Action::Timeline {
            command:T::Advance { target,limit:256 } },reference.committed_sequence()+1);
        execute(c,reference,evidence,id,envelope);
    }
    let progress = reference.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(progress.last_complete,target);
    assert!(progress.target.is_none() && progress.phase.is_none());
    let envelope = request(reference,Action::TownSettle { profile_id:1 },reference.committed_sequence()+1);
    execute(c,reference,evidence,id,envelope.clone());
    envelope
}

fn assert_business(s: &Store, day: u64) -> Value {
    let world = s.engine().unwrap().world().view();
    let town = world.town.unwrap();
    let capital = serde_json::to_value(s.engine().unwrap().asset_capital_summary()).unwrap();
    let (charged,retained,assessed,cash_rows,goods) = match day {
        1 => (116,392,392,[1_000_000,199_000,572_200,1_228_800],(90,900)),
        2 => (516,642,642,[1_000_392,198_000,572_808,1_228_800],(80,800)),
        _ => panic!("expected finite day1/day2"),
    };
    assert_eq!(capital["basis"],json!(29_200));
    assert_eq!(capital["commissioned_at"],json!(61_200));
    assert_eq!(capital["charged"],json!(charged));
    assert_eq!(capital["stopped_at"],Value::Null);
    assert_eq!(capital["subsequent_expense"],json!(0));
    let cursor = &town.state.tax_cursors.as_slice()[0];
    assert_eq!(cursor.through_day,day);
    assert_eq!(cursor.retained_earnings_cents,retained);
    assert_eq!(cursor.cumulative_assessed,Money::new(assessed).unwrap());
    for (index,amount) in cash_rows.into_iter().enumerate() {
        assert_eq!(world.accounts.iter().find(|a|a.id==index as u32+1).unwrap().cash.value(),amount);
    }
    assert_eq!(world.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_000_000);
    let raw = world.stocks.iter().find(|stock|stock.id==1).unwrap();
    assert_eq!((raw.quantity.value(),raw.basis.value()),(599_000,239_600));
    let product = world.stocks.iter().find(|stock|stock.id==2).unwrap();
    assert_eq!((product.quantity.value(),product.basis.value()),goods);
    let timeline = s.engine().unwrap().timeline().unwrap();
    for (period,basis,tax) in [(1,784,392),(2,500,250)] {
        if period<=day {
            let claim = timeline.claims.iter().find(|claim|
                timeline.tax_source_for_claim(claim).is_some_and(|source|source.period==period)).unwrap();
            let source = timeline.tax_source_for_claim(claim).unwrap();
            assert_eq!(source.taxable_basis,Money::new(basis).unwrap());
            assert_eq!(source.assessed,Money::new(tax).unwrap());
            assert_eq!(claim.principal,Money::new(tax).unwrap());
            assert_eq!(claim.paid,if period<day { Money::new(tax).unwrap() } else { Money::ZERO });
        }
    }
    json!({"day":day,"capital":capital,"tax_cursor":cursor,"accounts":world.accounts,
        "state_sha256":s.state_hash().unwrap(),"sequence":s.committed_sequence()})
}

#[test]
fn actual_asset_capital_day_settlement_restart_and_retry_fit_original_normal_role() {
    let base = fs::canonicalize("/Volumes/HP P900/dev/MyCity/work").expect("original SSD work");
    let evidence = PathBuf::from(std::env::var_os("MYCITY_ASSET_CAPITAL_ROLE_ROOT").unwrap())
        .canonicalize().unwrap();
    assert!(evidence.starts_with(&base) && evidence!=base);
    let world = PathBuf::from(std::env::var_os("MYCITY_ASSET_CAPITAL_ROLE_WORLD").unwrap());
    assert!(world.parent().unwrap().canonicalize().unwrap().starts_with(&base));
    assert!(!world.exists());
    let pool_root = PathBuf::from(std::env::var_os("MYCITY_ASSET_CAPITAL_POOL_ROOT").unwrap());
    assert!(pool_root.parent().unwrap().canonicalize().unwrap().starts_with(&base));
    assert!(!pool_root.exists());
    assert!(pool_root.to_str().unwrap().len()<72,"explicit short Unix socket pool path");
    let executable = PathBuf::from(std::env::var_os("MYCITY_ASSET_CAPITAL_HEADLESS_EXE").unwrap())
        .canonicalize().unwrap();
    let rules = format!("sim-w3:{}",env!("MYCITY_SOURCE_SHA256"));
    fs::write(evidence.join("identity.json"),serde_json::to_vec_pretty(&json!({
        "source_sha256":env!("MYCITY_SOURCE_SHA256"),"rules":rules,"world":world,"pool":pool_root,
        "executable":executable,"executable_sha256":mycity_core::sha256(&fs::read(&executable).unwrap()),
        "creator_reference_and_hashing_outside_shipping_role":true,
        "native_16gb_speed_and_m0_acceptance":"not_run"})).unwrap()).unwrap();
    let input = opening();
    fs::write(evidence.join("opening.json"),&input).unwrap();
    let spatial = include_bytes!("../../../fixtures/m0-exchange-spatial.json");
    fs::write(evidence.join("spatial-opening.json"),spatial).unwrap();
    let mut creator = Store::create_spatial_v2(&world,&input,vec![
        Resource::new(80,1).unwrap(),Resource::new(81,2).unwrap(),Resource::new(82,10).unwrap()],
        &rules,spatial,include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
    commission(&mut creator);
    creator.checkpoint().unwrap();
    drop(creator);
    let reference_world = evidence.join("reference-world");
    copy_world(&world,&reference_world);
    let mut reference = Store::open(&reference_world,&rules).unwrap();
    let mut pool = normal_pool::Process::spawn(&executable,&pool_root,
        Some(&evidence.join("pool-heaps.jsonl"))).unwrap();
    assert_eq!(normal_pool::status(&pool_root).unwrap().available_touched_bytes,64*1024*1024);
    let mut observations = Vec::new();
    let mut id = 1;
    let mut client = Client::new();
    client.start_in_pool(&executable,&world,false,false,&pool_root).unwrap();
    let ready = event(&mut client,&evidence);
    assert_eq!(ready.kind,"ready");
    observations.push(serde_json::to_value(ready).unwrap());
    let day1 = day(&mut client,&mut reference,&evidence,&mut id,1);
    status(&mut client,&reference,&evidence,&mut id,&mut observations);
    client.stop().unwrap();
    let balance1 = normal_pool::status(&pool_root).unwrap();
    assert_eq!(balance1.workspace_grants,1);
    assert_eq!(balance1.workspace_returns,1);
    assert_eq!(balance1.available_touched_bytes,64*1024*1024);
    let restored = Store::open(&world,&rules).unwrap();
    assert_eq!(restored.state_hash().unwrap(),reference.state_hash().unwrap());
    fs::write(evidence.join("day1-business.json"),serde_json::to_vec_pretty(&assert_business(&restored,1)).unwrap()).unwrap();
    drop(restored);

    client = Client::new();
    client.start_in_pool(&executable,&world,false,false,&pool_root).unwrap();
    let ready = event(&mut client,&evidence);
    assert_eq!(ready.kind,"ready");
    observations.push(serde_json::to_value(ready).unwrap());
    let sequence = reference.committed_sequence();
    execute(&mut client,&mut reference,&evidence,&mut id,day1);
    assert_eq!(reference.committed_sequence(),sequence);
    status(&mut client,&reference,&evidence,&mut id,&mut observations);
    let day2 = day(&mut client,&mut reference,&evidence,&mut id,2);
    let sequence = reference.committed_sequence();
    execute(&mut client,&mut reference,&evidence,&mut id,day2.clone());
    assert_eq!(reference.committed_sequence(),sequence);
    status(&mut client,&reference,&evidence,&mut id,&mut observations);
    client.stop().unwrap();
    let mut restored = Store::open(&world,&rules).unwrap();
    assert_eq!(restored.state_hash().unwrap(),reference.state_hash().unwrap());
    fs::write(evidence.join("day2-business.json"),serde_json::to_vec_pretty(&assert_business(&restored,2)).unwrap()).unwrap();
    let hash = restored.state_hash().unwrap();
    let sequence = restored.committed_sequence();
    assert!(restored.execute(Caller::System,day2).unwrap().model.is_ok());
    assert_eq!(restored.committed_sequence(),sequence);
    assert_eq!(restored.state_hash().unwrap(),hash);

    let mut arena_peak = 0;
    for event in &observations {
        let payload = event["payload"].get("Ok").unwrap_or(&event["payload"]);
        assert_eq!(payload["source_sha256"],env!("MYCITY_SOURCE_SHA256"));
        let arena = &payload["runtime"]["allocator_boundary"]["snapshot"];
        assert_eq!(arena["activation"]["grant_bytes"],59*1024*1024);
        assert_eq!(arena["allocation_failures"],0);
        let used = arena["arena_used_bytes"].as_u64().unwrap();
        assert!(used<=arena["arena_usable_bytes"].as_u64().unwrap());
        arena_peak = arena_peak.max(used);
    }
    let balance = normal_pool::status(&pool_root).unwrap();
    assert_eq!(balance.workspace_grants,2);
    assert_eq!(balance.workspace_returns,2);
    assert_eq!(balance.available_touched_bytes,64*1024*1024);
    pool.finish().unwrap();
    let report: Value = serde_json::from_slice(&fs::read(pool_root.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["balanced_after_owned_wait"],true);
    fs::write(evidence.join("observations.json"),serde_json::to_vec_pretty(&observations).unwrap()).unwrap();
    fs::write(evidence.join("resources.json"),serde_json::to_vec_pretty(&json!({
        "source_sha256":env!("MYCITY_SOURCE_SHA256"),"arena_observed_peak_bytes":arena_peak,
        "role_grant_bytes":59*1024*1024,"pool_capacity_bytes":64*1024*1024,
        "after_first_owned_wait":balance1,"after_second_owned_wait":balance,
        "balanced_after_owned_wait":report["balanced_after_owned_wait"],
        "engine_owned_bytes":restored.engine().unwrap().owned_capacity_bytes(),
        "store_owned_bytes":restored.owned_capacity_bytes().unwrap(),
        "creator_reference_and_hashing_outside_shipping_role":true,
        "native_16gb_speed_and_m0_acceptance":"not_run"})).unwrap()).unwrap();
}
