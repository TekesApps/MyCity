//! #21 actual map allocation in the original finite asset/Payroll/service city.
use mycity_core::{boot::{Command as B,Facility},construction::{EngineCommand as C,GateDecision},housing,
    service_delivery_authority::Command as D,timeline::{Command as T,PayrollSpec,Cycle,DAY},
    transaction::{Action,Caller,Envelope},units::{Money,GameTime},market_access::TransportProofAsset};
use mycity_storage::{Store,WritePoint};
use serde_json::{Value,json};
use std::{fs,path::{Path,PathBuf},process::{Command,Stdio},time::{SystemTime,UNIX_EPOCH,Instant,Duration}};
#[path="../../../tests/fixtures/finite-city/transport_opening.rs"]mod fixture;
const RULES:&str="finite-city-road-v1";
fn root()->PathBuf{let p=PathBuf::from(std::env::var_os("MYCITY_FINITE_CITY_ROAD_ROOT").unwrap()).canonicalize().unwrap();assert!(p.starts_with("/Volumes/HP P900/dev/MyCity/work"));p}
fn apply(s:&mut Store,caller:Caller,action:Action){let(scope,revision)=s.engine().unwrap().world().scope_revision();let ticket=Envelope{world:scope.world(),branch:scope.branch(),operation_id:s.committed_sequence()+1,expected_revision:revision,actions:vec![action]};let r=s.execute(caller,ticket).unwrap();assert!(r.model.is_ok(),"{r:?}");}
fn construction(s:&mut Store,actor:u32,command:C){apply(s,Caller::Account(actor),Action::Construction{command});}
fn advance(s:&mut Store,target:u64){for _ in 0..8192{let limit=s.continuation_status().unwrap().map_or(256,|r|r.page_limit);apply(s,Caller::System,Action::Timeline{command:T::Advance{target,limit}});if s.engine().unwrap().timeline().unwrap().progress.target.is_none(){assert_eq!(s.engine().unwrap().timeline().unwrap().progress.last_complete,target);return;}}panic!("timeline unfinished");}
fn create(label:&str,scenario:fixture::Scenario)->(PathBuf,Store){let p=root().join(format!("{label}-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&p).unwrap();let(raw,allocation,spatial)=fixture::opening(scenario);let input=serde_json::to_vec(&raw).unwrap();for(name,b)in[("opening.json",input.as_slice()),("allocation.json",allocation.as_slice()),("spatial.json",spatial.as_slice())]{fs::write(p.join(name),b).unwrap();}let mut s=Store::create_spatial_with_transport_allocation_profile(&p.join("world"),&input,fixture::resources(),RULES,(&spatial,fixture::HEIGHTMAP),&allocation).unwrap();
 apply(&mut s,Caller::System,Action::Timeline{command:T::Enable});let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();apply(&mut s,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
 apply(&mut s,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:21,payer:7,payee:2,home_city:0,hourly_cents:Money::new(10).unwrap(),cycle:Cycle::Daily,ends_at:3*DAY}}});
 construction(&mut s,7,C::File);construction(&mut s,2,C::DecideGate{gate_id:1,decision:GateDecision::Accepted,evidence_id:11});construction(&mut s,1,C::DecideGate{gate_id:2,decision:GateDecision::Accepted,evidence_id:12});construction(&mut s,7,C::Prepare);construction(&mut s,7,C::Start);
 for(facility,until)in[(Facility::Pump,60),(Facility::Generator,120)]{apply(&mut s,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut s,until);}
 advance(&mut s,28800);apply(&mut s,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});advance(&mut s,61200);construction(&mut s,4,C::Work{at:GameTime::new(61200).unwrap(),elapsed_seconds:61200,worker_seconds:28800});construction(&mut s,4,C::SubmitInspection);construction(&mut s,1,C::AcceptInspection{inspector:1,accepted:true});(p,s)
}
fn facts(s:&Store)->Value{let e=s.engine().unwrap();let v=e.world().view();let town=v.town.unwrap();let tax=town.state.tax_cursors.as_slice()[0];let payroll=&e.timeline().unwrap().payroll[0];json!({"sales":town.state.last_summary.actual_sales.value(),"summary":town.state.last_summary,"tax":tax.cumulative_assessed.value(),"retained":tax.retained_earnings_cents,"company_cash":v.accounts.iter().find(|a|a.id==7).unwrap().cash.value(),"stocks":v.stocks.iter().filter(|x|matches!(x.id,15|16|3002|3003)).map(|x|(x.id,x.quantity.value())).collect::<Vec<_>>(),"stock_basis":v.stocks.iter().filter(|x|matches!(x.id,15|16)).map(|x|(x.id,x.basis.value())).collect::<Vec<_>>(),"capital":e.asset_capital_summary(),"payroll":[payroll.work_seconds,payroll.paid.value()as u64,payroll.remainder],"cash_total":v.accounts.iter().map(|a|a.cash.value()).sum::<i64>()})}
fn cold(p:&Path,s:Store,operation:u64,label:&str){let input=p.join(format!("{label}-cold.json"));fs::write(&input,json!({"world":p.join("world"),"hash":s.state_hash().unwrap(),"sequence":s.committed_sequence(),"operation":operation,"facts":facts(&s)}).to_string()).unwrap();drop(s);let mut child=Command::new(std::env::current_exe().unwrap()).args(["--ignored","--exact","finite_city_road_cold_probe","--test-threads=1","--nocapture"]).env("MYCITY_FINITE_CITY_ROAD_COLD",&input).stdout(Stdio::from(fs::File::create(p.join(format!("{label}-cold.log"))).unwrap())).stderr(Stdio::from(fs::File::create(p.join(format!("{label}-cold.stderr"))).unwrap())).spawn().unwrap();let t=Instant::now();loop{if let Some(status)=child.try_wait().unwrap(){fs::write(p.join(format!("{label}-cold-result.json")),json!({"exit":status.code(),"seconds":t.elapsed().as_secs_f64()}).to_string()).unwrap();assert!(status.success());return;}if t.elapsed()>Duration::from_secs(60){child.kill().unwrap();child.wait().unwrap();panic!("cold timeout");}std::thread::sleep(Duration::from_millis(20));}}
#[test]#[ignore]fn finite_city_road_cold_probe(){let input:Value=serde_json::from_slice(&fs::read(std::env::var_os("MYCITY_FINITE_CITY_ROAD_COLD").unwrap()).unwrap()).unwrap();let mut s=Store::open(Path::new(input["world"].as_str().unwrap()),RULES).unwrap();assert_eq!(s.state_hash().unwrap(),input["hash"].as_str().unwrap());assert_eq!(facts(&s),input["facts"]);let out=s.settle_town_market(input["operation"].as_u64().unwrap()).unwrap();assert!(out.model.is_ok());assert_eq!(s.state_hash().unwrap(),input["hash"].as_str().unwrap());assert_eq!(s.committed_sequence(),input["sequence"].as_u64().unwrap());assert_eq!(facts(&s),input["facts"]);}
#[test]fn actual_spatial_access_changes_common_city_sales_and_preserves_original_recovery(){for(label,scenario,served)in[("open",fixture::Scenario::Open,4),("closed",fixture::Scenario::RoadClosed,0),("door",fixture::Scenario::DoorCannotEnter,0)]{let(p,mut s)=create(label,scenario);let hash=s.state_hash().unwrap();let seq=s.committed_sequence();assert!(s.settle_town_market(seq+1).is_err());assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);
 for day in 1..=2{advance(&mut s,day*DAY);let operation=s.committed_sequence()+1;let old_hash=s.state_hash().unwrap();let old_seq=s.committed_sequence();let point=if day==1{WritePoint::WalPayloadHalf}else{WritePoint::CurrentRenamed};let mut reached=false;let out=s.settle_town_market_with_observer(operation,|p|if p==point{reached=true;Err(std::io::Error::other("retained road-town publication interruption"))}else{Ok(())});assert!(reached,"{out:?}");assert!(out.is_err());fs::write(p.join(format!("day{day}-interruption.txt")),format!("{out:?}")).unwrap();drop(s);s=Store::open(&p.join("world"),RULES).unwrap();if day==1{assert_eq!(s.committed_sequence(),old_seq);assert_eq!(s.state_hash().unwrap(),old_hash);}else{assert_eq!(s.committed_sequence(),old_seq+1);assert_ne!(s.state_hash().unwrap(),old_hash);}
 assert!(s.settle_town_market(operation).unwrap().model.is_ok());let f=facts(&s);assert_eq!(f["cash_total"],json!(3100000));assert_eq!(f["capital"]["basis"],json!(28916));assert_eq!(f["capital"]["charged"],json!(if day==1{116}else{516}));assert_eq!(f["sales"].as_i64().unwrap()>0,served>0);
 let (sales,freight,produced,company,tax,retained,raw,product,electric)=if served>0{if day==1{(94,658,94,71955,280,281,806,0,168000)}else{(100,700,100,72495,440,441,706,0,68000)}}else if day==1{(0,0,94,71015,0,-116,806,94,168000)}else{(0,0,0,70935,0,-596,806,94,168000)};
 assert_eq!(f["sales"],json!(sales));assert_eq!(f["summary"]["freight"],json!(freight));assert_eq!(f["summary"]["produced"],json!(produced));assert_eq!(f["company_cash"],json!(company));assert_eq!(f["tax"],json!(tax));assert_eq!(f["retained"],json!(retained));assert_eq!(f["stocks"],json!([[15,raw],[16,product],[3002,electric],[3003,522000]]));if served==0{assert_eq!(f["stock_basis"],json!([[15,806],[16,263]]));}
 let reference=s.engine().unwrap().world().view().town.unwrap().state.last_transport.unwrap();let h=reference.issued_proof_sha256.iter().map(|b|format!("{b:02x}")).collect::<String>();let proof:TransportProofAsset=serde_json::from_slice(&fs::read(p.join("world").join(format!("town-proof-{h}.json"))).unwrap()).unwrap();assert_eq!(proof.service_day,day);assert!(proof.assignment.matches(&proof.opening));for source in[101,102]{let expected=if matches!(scenario,fixture::Scenario::DoorCannotEnter)&&source==101{4}else{served};assert_eq!(proof.assignment.demand_outcomes.iter().find(|d|d.source_id==source).unwrap().served_people,expected);}
 fs::write(p.join(format!("day{day}-facts.json")),serde_json::to_vec_pretty(&f).unwrap()).unwrap();let hash=s.state_hash().unwrap();let seq=s.committed_sequence();assert!(s.settle_town_market(operation).unwrap().model.is_ok());assert_eq!(s.state_hash().unwrap(),hash);assert_eq!(s.committed_sequence(),seq);if day==2{s.checkpoint().unwrap();}cold(&p,s,operation,&format!("day{day}"));s=Store::open(&p.join("world"),RULES).unwrap();}
}}

/// Fixed opening endowments are finite. Exhaustion limits new output, while
/// actual wages, inventory cost, tax and depreciation keep their own clocks.
#[test]
fn depleted_power_and_expired_payroll_keep_actual_costs_due_payments_and_restore() {
    let (p, mut s) = create("depletion", fixture::Scenario::Open);
    for (day, sales, freight, produced, cash, tax, retained, raw, electric, charged) in [
        (1, 94, 658, 94, 71_955, 280, 281, 806, 168_000, 116),
        (2, 100, 700, 100, 72_495, 440, 441, 706, 68_000, 516),
        (3, 68, 476, 68, 72_867, 472, 473, 638, 0, 916),
        (4, 0, 0, 0, 72_835, 472, 189, 638, 0, 1_200),
    ] {
        advance(&mut s, day * DAY);
        let operation = s.committed_sequence() + 1;
        assert!(s.settle_town_market(operation).unwrap().model.is_ok());
        let f = facts(&s);
        assert_eq!(f["sales"], json!(sales));
        assert_eq!(f["summary"]["freight"], json!(freight));
        assert_eq!(f["summary"]["produced"], json!(produced));
        assert_eq!(f["company_cash"], json!(cash));
        assert_eq!(f["tax"], json!(tax));
        assert_eq!(f["retained"], json!(retained));
        assert_eq!(f["stocks"], json!([[15, raw], [16, 0], [3002, electric], [3003, 522000]]));
        assert_eq!(f["capital"]["basis"], json!(28_916));
        assert_eq!(f["capital"]["charged"], json!(charged));
        assert_eq!(f["cash_total"], json!(3_100_000));
        if day >= 3 {
            let e = s.engine().unwrap();
            let t = e.timeline().unwrap();
            let payroll = &t.payroll[0];
            assert_eq!((payroll.work_seconds, payroll.paid.value(), payroll.remainder), (84_900, 235, 3_000));
            assert!(!payroll.working);
            assert_eq!(payroll.next_due, None);
            assert_eq!((payroll.accrued.value(), payroll.billed.value()), (235, 235));
            assert_eq!(f["summary"]["reference_need"], json!(100));
            assert_eq!(f["summary"]["labor_basis"], json!(if day == 3 { 54 } else { 0 }));
            assert_eq!(f["summary"]["idle_labor_expense"], json!(if day == 3 { 26 } else { 0 }));
            let rent = t.claims.iter().find(|c| c.id == 901).unwrap();
            assert_eq!((rent.due, rent.principal.value(), rent.paid.value(), rent.processed), (31 * DAY, 1000, 0, false));
            let rent_source = t.rent_source_for_claim(rent).unwrap();
            assert_eq!((rent_source.landlord, rent_source.tenant, rent_source.lease_id, rent_source.unit_id), (1, 2, 901, 71));
            let taxes: Vec<_> = t.claims.iter().filter_map(|c| t.tax_source_for_claim(c).map(|source| (source, c))).collect();
            assert_eq!(taxes.len(), 3);
            assert_eq!(taxes.iter().map(|(_, c)| c.principal.value()).sum::<i64>(), 472);
            assert_eq!(taxes.iter().map(|(_, c)| c.paid.value()).sum::<i64>(), if day == 3 { 440 } else { 472 });
            for (period, assessed) in [(1, 280), (2, 160), (3, 32)] {
                let (source, claim) = taxes.iter().find(|(source, _)| source.period == period).unwrap();
                assert_eq!((source.company, source.policy_id, source.assessed.value()), (7, 21, assessed));
                assert_eq!(claim.due, (period + 1) * DAY);
            }
            // There is still ample spendable cash in this fixed case. Every
            // real claim due by this complete boundary must have been paid.
            assert!(t.claims.iter().all(|c| c.due > day * DAY || c.paid == c.principal));
            fs::write(p.join(format!("day{day}-responsibilities.json")), serde_json::to_vec_pretty(&json!({
                "claims": t.claims,
                "payroll": payroll,
                "service": e.service_needs_summary(),
            })).unwrap()).unwrap();
        }
        fs::write(p.join(format!("day{day}-facts.json")), serde_json::to_vec_pretty(&f).unwrap()).unwrap();
        let hash = s.state_hash().unwrap();
        let sequence = s.committed_sequence();
        assert!(s.settle_town_market(operation).unwrap().model.is_ok());
        assert_eq!(s.state_hash().unwrap(), hash);
        assert_eq!(s.committed_sequence(), sequence);
        if day >= 3 {
            if day == 4 { s.checkpoint().unwrap(); }
            cold(&p, s, operation, &format!("day{day}"));
            s = Store::open(&p.join("world"), RULES).unwrap();
        }
    }
}

fn refill_observation(s: &Store) -> Value {
    let view = s.engine().unwrap().world().view();
    let boot = view.boot.unwrap();
    let stock = |id| view.stocks.iter().find(|x| x.id == id).unwrap();
    let cash = |id| view.accounts.iter().find(|x| x.id == id).unwrap().cash.value();
    let all_basis = view.stocks.iter().filter(|x| boot.stocks.contains(&x.id)).map(|x| x.basis.value()).sum::<i64>()
        + boot.exported_basis.value() + boot.failed_expense.value()
        + boot.jobs.iter().map(|job| job.cost.value()).sum::<i64>();
    json!({"boot":boot,"fuel":stock(3001),"electric":stock(3002),"water":stock(3003),
        "raw_water":stock(3004),"waste":stock(3005),"heat":stock(3006),
        "cash3":cash(3),"cash4":cash(4),"facts":facts(s),"all_boot_basis":all_basis})
}

/// A late batch may finish after the market close. Its planned electricity is
/// never used by that close, and finishing cannot rewrite the original tax.
#[test]
fn actual_generator_refill_after_consumption_preserves_pool_costs_and_cannot_supply_the_past() {
    for late in [false, true] {
        let (p, mut s) = create(if late { "refill-late" } else { "refill-ontime" }, fixture::Scenario::Open);
        let mut last_market = 0;
        for day in 1..=2 {
            advance(&mut s, day * DAY);
            last_market = s.committed_sequence() + 1;
            assert!(s.settle_town_market(last_market).unwrap().model.is_ok());
        }
        let before = refill_observation(&s);
        assert_eq!(before["electric"]["quantity"], json!(68_000));
        assert_eq!(before["water"]["quantity"], json!(522_000));
        assert_eq!(before["fuel"]["quantity"], json!(4_000));
        assert_eq!(before["all_boot_basis"], json!(2_600));
        let old_e = before["electric"]["basis"].as_i64().unwrap();
        let old_w = before["water"]["basis"].as_i64().unwrap();
        assert!((261..=266).contains(&old_w));
        fs::write(p.join("before-refill.json"), serde_json::to_vec_pretty(&before).unwrap()).unwrap();
        let start = if late { 3 * DAY - 30 } else { 2 * DAY };
        if late { advance(&mut s, start); }
        let (scope, revision) = s.engine().unwrap().world().scope_revision();
        let ticket = Envelope { world: scope.world(), branch: scope.branch(),
            operation_id: s.committed_sequence() + 1, expected_revision: revision,
            actions: vec![Action::Boot { command: B::Start { facility: Facility::Generator, reverse_request: true } }] };
        assert!(s.execute(Caller::System, ticket.clone()).unwrap().model.is_ok());
        let running = refill_observation(&s);
        assert_eq!(running["electric"], before["electric"]);
        assert_eq!(running["fuel"]["quantity"], json!(3_000));
        assert_eq!(running["fuel"]["basis"], json!(1_200));
        assert_eq!((running["fuel"]["origin_quantity"].as_i64(), running["fuel"]["origin_basis"].as_i64(), running["fuel"]["offset"].as_i64()), (Some(5000), Some(2000), Some(2000)));
        assert_eq!(running["water"]["quantity"], json!(462_000));
        assert_eq!(running["water"]["basis"], json!(old_w - 30));
        assert_eq!(running["cash3"].as_i64().unwrap(), before["cash3"].as_i64().unwrap() - 300);
        assert_eq!(running["cash4"].as_i64().unwrap(), before["cash4"].as_i64().unwrap() + 300);
        assert_eq!(running["boot"]["fees"], json!(800));
        assert_eq!(running["boot"]["fuel_used"], json!(1000));
        let job = &running["boot"]["jobs"][1];
        assert_eq!(job["phase"], json!("Running"));
        assert_eq!(job["cost"], json!(730));
        assert_eq!(job["inputs"], json!([1000, 0, 60000, 0, 0, 0]));
        assert_eq!((job["started"].as_u64(), job["due"].as_u64()), (Some(start), Some(start + 60)));
        assert_eq!(running["all_boot_basis"], json!(2900));
        assert_eq!(running["boot"]["exports"], before["boot"]["exports"]);
        for id in [3101, 3102] {
            assert_eq!(s.engine().unwrap().resources().iter().find(|r| r.id == id).unwrap().occupied, 1);
        }
        let hash = s.state_hash().unwrap(); let sequence = s.committed_sequence();
        assert!(s.execute(Caller::System, ticket.clone()).unwrap().model.is_ok());
        assert_eq!(s.state_hash().unwrap(), hash); assert_eq!(s.committed_sequence(), sequence);
        fs::write(p.join("running.json"), serde_json::to_vec_pretty(&running).unwrap()).unwrap();
        cold(&p, s, last_market, "running");
        s = Store::open(&p.join("world"), RULES).unwrap();
        assert_eq!(refill_observation(&s), running);

        if late {
            advance(&mut s, 3 * DAY);
            last_market = s.committed_sequence() + 1;
            assert!(s.settle_town_market(last_market).unwrap().model.is_ok());
            let before_finish = refill_observation(&s);
            assert_eq!(before_finish["facts"]["sales"], json!(68));
            assert_eq!(before_finish["facts"]["tax"], json!(472));
            assert_eq!(before_finish["facts"]["company_cash"], json!(72867));
            assert_eq!(before_finish["electric"]["quantity"], json!(0));
            assert_eq!(before_finish["electric"]["basis"], json!(0));
            assert_eq!(before_finish["boot"]["jobs"][1]["cost"], json!(730));
            cold(&p, s, last_market, "late-close-still-running");
            s = Store::open(&p.join("world"), RULES).unwrap();
            assert_eq!(refill_observation(&s), before_finish);
        }
        advance(&mut s, start + 59);
        let pre_due = refill_observation(&s);
        assert_eq!(pre_due["electric"]["quantity"], json!(if late { 0 } else { 68_000 }));
        assert_eq!(pre_due["boot"]["jobs"][1]["phase"], json!("Running"));
        cold(&p, s, last_market, "one-second-before-due");
        s = Store::open(&p.join("world"), RULES).unwrap();
        assert_eq!(refill_observation(&s), pre_due);
        advance(&mut s, start + 60);
        let finished = refill_observation(&s);
        let added_basis = if late { 730 } else { old_e + 730 };
        let added_quantity = if late { 400_000 } else { 468_000 };
        assert_eq!(finished["electric"]["quantity"], json!(added_quantity));
        assert_eq!(finished["electric"]["basis"], json!(added_basis));
        assert_eq!(finished["electric"]["origin_quantity"], json!(added_quantity));
        assert_eq!(finished["electric"]["origin_basis"], json!(added_basis));
        assert_eq!(finished["electric"]["offset"], json!(0));
        for key in ["id", "owner", "commodity", "lot_id", "opening_source_id", "opening_basis"] {
            assert_eq!(finished["electric"][key], before["electric"][key]);
        }
        assert_eq!(finished["boot"]["jobs"][1]["phase"], json!("Idle"));
        assert_eq!(finished["boot"]["jobs"][1]["completed"], json!(2));
        assert_eq!(finished["boot"]["jobs"][1]["inputs"], json!([0, 0, 0, 0, 0, 0]));
        assert_eq!(finished["boot"]["jobs"][1]["cost"], json!(0));
        assert_eq!(finished["boot"]["jobs"][1]["tokens"], json!([0, 0]));
        assert_eq!(finished["boot"]["fuel_used"], json!(2000));
        assert_eq!(finished["waste"]["quantity"], json!(120000));
        assert_eq!(finished["heat"]["quantity"], json!(1300000));
        assert_eq!(finished["all_boot_basis"], json!(2900));
        for id in [3101, 3102] {
            assert_eq!(s.engine().unwrap().resources().iter().find(|r| r.id == id).unwrap().occupied, 0);
        }
        let hash = s.state_hash().unwrap(); let sequence = s.committed_sequence();
        assert!(s.execute(Caller::System, ticket.clone()).unwrap().model.is_ok());
        assert_eq!(s.state_hash().unwrap(), hash); assert_eq!(s.committed_sequence(), sequence);
        assert_eq!(refill_observation(&s), finished);
        fs::write(p.join("finished.json"), serde_json::to_vec_pretty(&finished).unwrap()).unwrap();
        s.checkpoint().unwrap(); cold(&p, s, last_market, "finished-checkpoint");
        s = Store::open(&p.join("world"), RULES).unwrap();
        for day in if late { vec![4] } else { vec![3, 4] } {
            advance(&mut s, day * DAY);
            last_market = s.committed_sequence() + 1;
            assert!(s.settle_town_market(last_market).unwrap().model.is_ok());
            let f = facts(&s);
            let (sales, freight, tax, retained, cash) = match (late, day) {
                (false, 3) => (100, 700, 600, 601, 73155),
                (false, 4) => (0, 0, 600, 317, 72995),
                (true, 4) => (0, 0, 472, 189, 72835),
                _ => unreachable!(),
            };
            assert_eq!(f["sales"], json!(sales)); assert_eq!(f["summary"]["freight"], json!(freight));
            assert_eq!((f["tax"].as_i64(), f["retained"].as_i64(), f["company_cash"].as_i64()), (Some(tax), Some(retained), Some(cash)));
            assert_eq!(f["cash_total"], json!(3100000));
            fs::write(p.join(format!("day{day}-facts.json")), serde_json::to_vec_pretty(&f).unwrap()).unwrap();
        }
        let end = refill_observation(&s);
        let consumed_basis = if late { old_e } else { (old_e + 730) * 100000 / 468000 };
        assert_eq!(end["electric"]["quantity"], json!(if late { 400000 } else { 368000 }));
        assert_eq!(end["electric"]["basis"], json!(if late { 730 } else { old_e + 730 - consumed_basis }));
        assert_eq!(end["facts"]["stocks"], json!([[15, if late { 638 } else { 606 }], [16, 0], [3002, if late { 400000 } else { 368000 }], [3003, 462000]]));
        assert_eq!(end["boot"]["exports"], json!([if late { 400000 } else { 432000 }, 18000]));
        assert_eq!(end["boot"]["exported_basis"].as_i64().unwrap(), before["boot"]["exported_basis"].as_i64().unwrap() + consumed_basis);
        assert_eq!(end["boot"]["local_electricity_basis"].as_i64().unwrap(), before["boot"]["local_electricity_basis"].as_i64().unwrap() + consumed_basis);
        assert_eq!(end["boot"]["local_construction_exported"], before["boot"]["local_construction_exported"]);
        assert_eq!(end["boot"]["local_construction_basis"], before["boot"]["local_construction_basis"]);
        assert_eq!(end["all_boot_basis"], json!(2900));
        let t = s.engine().unwrap().timeline().unwrap();
        assert_eq!((t.payroll[0].work_seconds, t.payroll[0].paid.value(), t.payroll[0].remainder), (84900, 235, 3000));
        assert!(!t.payroll[0].working); assert_eq!(t.payroll[0].next_due, None);
        assert!(t.claims.iter().all(|c| c.due > 4 * DAY || c.paid == c.principal));
        fs::write(p.join("end.json"), serde_json::to_vec_pretty(&end).unwrap()).unwrap();
        s.checkpoint().unwrap(); cold(&p, s, last_market, "day4-checkpoint");
    }
}
