//! Existing actual asset costs consumed by the common city, not a new model.
use mycity_core::{assembly,boot::{Command as B,Facility},construction::{EngineCommand as C,GateDecision,OPERATION_PREFIX},
    housing,service_delivery_authority::{Command as D,Motion},timeline::{Command as T,PayrollSpec,Cycle,DAY},
    transaction::{Engine,Envelope,Action,Caller},units::{Money,GameTime}};
use serde_json::{Value,json};
use std::sync::atomic::{AtomicU64,Ordering};
#[path="../../../tests/fixtures/finite-city/asset_opening.rs"]mod fixture;
const SHIFT:u64=61200;
static NEXT:AtomicU64=AtomicU64::new(5_000_000);
fn request(e:&Engine,a:Action)->Envelope{let(s,r)=e.world().scope_revision();Envelope{world:s.world(),branch:s.branch(),operation_id:NEXT.fetch_add(1,Ordering::Relaxed),expected_revision:r,actions:vec![a]}}
fn apply(e:&mut Engine,c:Caller,a:Action)->Envelope{let env=request(e,a);e.execute(c,env.clone()).unwrap();env}
fn project(e:&Engine)->mycity_core::construction::Project{e.world().view().construction.unwrap()}
fn effect(e:&mut Engine,actor:u32,c:C)->Envelope{let p=project(e);let mut env=request(e,Action::Construction{command:c});env.operation_id=OPERATION_PREFIX|(p.id<<32)|u64::from(p.next_effect);e.execute(Caller::Account(actor),env.clone()).unwrap();env}
fn command(e:&mut Engine,actor:u32,c:C){apply(e,Caller::Account(actor),Action::Construction{command:c});}
fn advance(e:&mut Engine,target:u64){for _ in 0..8192{let p=e.timeline().unwrap().progress;if p.target.is_none()&&p.last_complete==target{return;}apply(e,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}});}panic!("unfinished {target}");}
fn capital(e:&Engine)->Value{serde_json::to_value(e.asset_capital_summary()).unwrap()}
fn cash(e:&Engine,id:u32)->i64{e.world().view().accounts.iter().find(|a|a.id==id).unwrap().cash.value()}
fn commissioned_city(input:Value)->Engine{
    let mut e=Engine::new(assembly::assemble(&serde_json::to_vec(&input).unwrap()).unwrap(),fixture::resources()).unwrap();
    apply(&mut e,Caller::System,Action::Timeline{command:T::Enable});
    let offer:housing::GenesisLeaseOffer=serde_json::from_value(input["housing"]["lease_offers"][0].clone()).unwrap();
    apply(&mut e,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)});
    apply(&mut e,Caller::System,Action::Timeline{command:T::AddPayroll{spec:PayrollSpec{id:21,payer:7,payee:2,home_city:0,hourly_cents:Money::new(10).unwrap(),cycle:Cycle::Daily,ends_at:3*DAY}}});
    command(&mut e,7,C::File);command(&mut e,2,C::DecideGate{gate_id:1,decision:GateDecision::Accepted,evidence_id:11});command(&mut e,1,C::DecideGate{gate_id:2,decision:GateDecision::Accepted,evidence_id:12});command(&mut e,7,C::Prepare);effect(&mut e,7,C::Start);
    for(facility,until)in[(Facility::Pump,60),(Facility::Generator,120)]{apply(&mut e,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}});advance(&mut e,until);}
    advance(&mut e,28800);apply(&mut e,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}});advance(&mut e,33900);
    assert_eq!(e.service_needs_summary().unwrap().delivery.unwrap().trips[0].motion,Motion::Finished);
    advance(&mut e,SHIFT);effect(&mut e,4,C::Work{at:GameTime::new(SHIFT as i64).unwrap(),elapsed_seconds:SHIFT as u32,worker_seconds:28800});
    assert_eq!(project(&e).spent.value(),28916);assert_eq!(cash(&e,7),71184);assert_eq!(cash(&e,4),1229300);
    effect(&mut e,4,C::SubmitInspection);let commissioned=effect(&mut e,1,C::AcceptInspection{inspector:1,accepted:true});assert_eq!(capital(&e)["basis"],json!(28916));
    let hash=e.state_hash();e.execute(Caller::Account(1),commissioned).unwrap();assert_eq!(e.state_hash(),hash);
    e
}
#[test]
fn actual_work_utilities_and_asset_costs_share_both_city_day_closes(){
    let mut e=commissioned_city(fixture::input());
    for(day,charged,tax,retained,company,raw,electric)in[(1,116,280,281,71955,806,168000),(2,516,440,441,72495,706,68000)]{
        advance(&mut e,day*DAY);apply(&mut e,Caller::System,Action::TownSettle{profile_id:1});
        let c=capital(&e);assert_eq!(c["charged"],json!(charged));assert_eq!(c["basis"],json!(28916));assert_eq!(cash(&e,7),company);
        let v=e.world().view();let cursor=v.town.unwrap().state.tax_cursors.as_slice()[0];assert_eq!((cursor.cumulative_assessed.value(),cursor.retained_earnings_cents),(tax,retained));
        for(id,q)in[(15,raw),(3002,electric),(3003,522000)]{assert_eq!(v.stocks.iter().find(|s|s.id==id).unwrap().quantity.value(),q);}
        assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3100000);
    }
    let p=&e.timeline().unwrap().payroll[0];assert_eq!((p.work_seconds,p.paid.value(),p.remainder),(56100,155,3000));
    let v=e.world().view();let b=v.boot.unwrap();assert_eq!(b.local_construction_exported.iter().map(|q|q.value()).collect::<Vec<_>>(),vec![8000,8000]);
}

#[test]
fn actual_withdrawal_and_close_preserve_joint_supply_and_all_unsettled_costs(){
    let mut e=commissioned_city(fixture::closing_input());advance(&mut e,64800);
    command(&mut e,7,C::BeginDemolition{waste_receiver:4});
    command(&mut e,2,C::DecideGate{gate_id:1,decision:GateDecision::Accepted,evidence_id:21});
    command(&mut e,1,C::DecideGate{gate_id:2,decision:GateDecision::Accepted,evidence_id:22});
    command(&mut e,7,C::Prepare);effect(&mut e,7,C::Start);
    assert_eq!(capital(&e)["stopped_at"],json!(64800));assert_eq!(e.resources().iter().find(|r|r.id==81).unwrap().capacity,2);
    advance(&mut e,DAY);apply(&mut e,Caller::System,Action::TownSettle{profile_id:1});
    assert_eq!(capital(&e)["charged"],json!(16));
    advance(&mut e,147600);effect(&mut e,4,C::Work{at:GameTime::new(147600).unwrap(),elapsed_seconds:82800,worker_seconds:28800});
    effect(&mut e,4,C::AcceptWaste);let closed=effect(&mut e,7,C::Close);
    assert_eq!(project(&e).phase,mycity_core::construction::Phase::Closed);
    let state=capital(&e);assert_eq!(state["subsequent_expense"],json!(29066));assert_eq!(state["retirement_cost"],json!(28900));assert_eq!(state["charged_retirement_cost"],json!(0));
    let hash=e.state_hash();e.execute(Caller::Account(7),closed).unwrap();assert_eq!(e.state_hash(),hash);
    advance(&mut e,2*DAY);apply(&mut e,Caller::System,Action::TownSettle{profile_id:1});
    let state=capital(&e);assert_eq!(state["charged_subsequent_expense"],json!(29066));assert_eq!(state["charged_retirement_cost"],json!(28900));
    let v=e.world().view();let tax=v.town.unwrap().state.tax_cursors.as_slice()[0];assert_eq!((tax.cumulative_assessed.value(),tax.retained_earnings_cents),(330,-56915));assert_eq!(cash(&e,7),43379);
    for(id,q)in[(15,706),(3002,60000),(3003,514000)]{assert_eq!(v.stocks.iter().find(|s|s.id==id).unwrap().quantity.value(),q);}
    assert_eq!(v.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3100000);
}

#[test]
fn original_50000_contract_refuses_unapproved_demolition_cost_atomically(){
    let mut e=commissioned_city(fixture::input());advance(&mut e,64800);
    command(&mut e,7,C::BeginDemolition{waste_receiver:4});
    command(&mut e,2,C::DecideGate{gate_id:1,decision:GateDecision::Accepted,evidence_id:21});
    command(&mut e,1,C::DecideGate{gate_id:2,decision:GateDecision::Accepted,evidence_id:22});
    command(&mut e,7,C::Prepare);effect(&mut e,7,C::Start);
    advance(&mut e,147600);
    let p=project(&e);let mut env=request(&e,Action::Construction{command:C::Work{at:GameTime::new(147600).unwrap(),elapsed_seconds:82800,worker_seconds:28800}});env.operation_id=OPERATION_PREFIX|(p.id<<32)|u64::from(p.next_effect);
    let before=e.export_checkpoint().unwrap();let hash=e.state_hash();
    assert_eq!(e.execute(Caller::Account(4),env),Err(mycity_core::transaction::TxError::Construction(mycity_core::construction::Error::Funds)));
    assert_eq!(e.state_hash(),hash);assert_eq!(e.export_checkpoint().unwrap(),before);
    assert_eq!(project(&e).spent.value(),28916);assert_eq!(project(&e).phase,mycity_core::construction::Phase::Demolition);
    assert!(e.leases().iter().flatten().any(|l|l.actor==7&&l.occupied));
    assert!(capital(&e)["retired_at"].is_null());
}
