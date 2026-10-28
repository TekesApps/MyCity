//! #151 real Engine construction and TownSettle path. Policy inputs are explicit
//! laboratory fixtures; assertions below are independent integer constants.
//! Store WAL/checkpoint cold recovery is a separate acceptance gate.
use mycity_core::{
    assembly::assemble,
    construction::{EngineCommand as C, Gate, GateCategory, GateDecision, Kind, Material,
        Project, OPERATION_PREFIX},
    timeline::{Command as T, DAY},
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::{GameTime, Money, Quantity},
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

const SHIFT: u64 = 61_200;
static NEXT: AtomicU64 = AtomicU64::new(1_000_000);

fn opening(residual: i64) -> Vec<u8> {
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
            "cost_quantum":1000,"residual":residual}}
    });
    { let capital=input["town"].as_object_mut().unwrap().remove("asset_capital"); if let Some(capital)=capital { input["asset_capital"]=capital; } serde_json::to_vec(&input).unwrap() }
}

fn engine(residual: i64) -> Engine {
    Engine::new(assemble(&opening(residual)).unwrap(), vec![
        Resource::new(80,1).unwrap(), Resource::new(81,2).unwrap(),
        Resource::new(82,10).unwrap(),
    ]).unwrap()
}

fn request(e: &Engine, action: Action, id: u64) -> Envelope {
    let v = e.world().view();
    Envelope { world:v.scope.world(), branch:v.scope.branch(), operation_id:id,
        expected_revision:v.revision, actions:vec![action] }
}

fn run(e: &mut Engine, caller: Caller, action: Action) {
    let envelope = request(e, action, NEXT.fetch_add(1,Ordering::Relaxed));
    e.execute(caller,envelope).unwrap();
}

fn effect(e: &Engine, command: C) -> Envelope {
    let p = e.world().view().construction.unwrap();
    request(e, Action::Construction { command },
        OPERATION_PREFIX | (p.id << 32) | u64::from(p.next_effect))
}

fn command(e: &mut Engine, actor: u32, command: C) {
    run(e,Caller::Account(actor),Action::Construction { command });
}

fn approve(e: &mut Engine) {
    command(e,2,C::DecideGate { gate_id:1, decision:GateDecision::Accepted, evidence_id:11 });
    command(e,1,C::DecideGate { gate_id:2, decision:GateDecision::Accepted, evidence_id:12 });
    command(e,3,C::Prepare);
}

fn advance(e: &mut Engine, at: u64) {
    let progress = &e.timeline().unwrap().progress;
    if progress.last_complete == at && progress.target.is_none() && progress.phase.is_none() {
        return;
    }
    for _ in 0..100 {
        run(e,Caller::System,Action::Timeline { command:T::Advance { target:at,limit:8192 } });
        if e.timeline().unwrap().progress.target.is_none() { return; }
    }
    panic!("timeline did not reach {at}");
}

fn ready_for_inspection(e: &mut Engine) {
    run(e,Caller::System,Action::Timeline { command:T::Enable });
    command(e,3,C::File);
    approve(e);
    let start = effect(e,C::Start);
    e.execute(Caller::Account(3),start).unwrap();
    advance(e,SHIFT);
    let work = effect(e,C::Work { at:GameTime::new(SHIFT as i64).unwrap(),
        elapsed_seconds:SHIFT as u32,worker_seconds:28_800 });
    e.execute(Caller::Account(4),work).unwrap();
    let submit = effect(e,C::SubmitInspection);
    e.execute(Caller::Account(4),submit).unwrap();
}

fn commission(e: &mut Engine) -> Envelope {
    let envelope = effect(e,C::AcceptInspection { inspector:1,accepted:true });
    e.execute(Caller::Account(1),envelope.clone()).unwrap();
    envelope
}

fn settle(e: &mut Engine, day: u64) -> Envelope {
    advance(e,day*DAY);
    let envelope = request(e,Action::TownSettle { profile_id:1 },
        NEXT.fetch_add(1,Ordering::Relaxed));
    let receipt = e.execute(Caller::System,envelope.clone()).unwrap();
    let summary = receipt.town.unwrap();
    assert_eq!(summary.actual_sales,Quantity::new(10).unwrap());
    assert_eq!(summary.goods_payments,Money::new(1000).unwrap());
    assert_eq!(summary.wages_accrued,Money::ZERO);
    envelope
}

fn capital(e: &Engine) -> Value {
    serde_json::to_value(e.asset_capital_summary()).unwrap()
}

fn cash(e: &Engine, account: u32) -> i64 {
    e.world().view().accounts.iter().find(|a|a.id==account).unwrap().cash.value()
}

fn assert_capital(e: &Engine, at: u64, charged: i64, stop: Option<u64>) {
    let c = capital(e);
    assert_eq!(c["commissioned_at"],json!(at));
    assert_eq!(c["basis"],json!(29_200));
    assert_eq!(c["charged"],json!(charged));
    assert_eq!(c["stopped_at"],json!(stop));
}

fn assert_tax(e: &Engine, day: u64, basis: i64, assessed: i64, retained: i64) {
    let town = e.world().view().town.unwrap();
    let cursor = &town.state.tax_cursors.as_slice()[0];
    assert_eq!(cursor.through_day,day);
    assert_eq!(cursor.retained_earnings_cents,retained);
    let timeline = e.timeline().unwrap();
    if assessed == 0 {
        assert!(!timeline.claims.iter().any(|claim|timeline.tax_source_for_claim(claim).is_some_and(|s|s.period==day)));
        return;
    }
    let claim = timeline.claims.iter().find(|claim|
        timeline.tax_source_for_claim(claim).is_some_and(|s|s.period==day)).unwrap();
    let source = timeline.tax_source_for_claim(claim).unwrap();
    assert_eq!(source.taxable_basis,Money::new(basis).unwrap());
    assert_eq!(source.assessed,Money::new(assessed).unwrap());
    assert_eq!(claim.principal,Money::new(assessed).unwrap());
    assert_eq!(claim.paid,Money::ZERO);
    assert_eq!(claim.due,(day+1)*DAY);
}

#[test]
fn actual_construction_basis_flows_into_daily_tax_without_second_cash_charge() {
    let mut e = engine(28_000);
    ready_for_inspection(&mut e);
    assert_eq!(e.world().view().construction.unwrap().spent,Money::new(29_200).unwrap());
    let stock = e.world().view().stocks.into_iter().find(|s|s.id==1).unwrap();
    assert_eq!((stock.quantity.value(),stock.basis.value()),(599_000,239_600));
    assert_eq!(cash(&e,3),571_200);
    assert_eq!(cash(&e,4),1_228_800);
    assert!(capital(&e).is_null());
    let original = commission(&mut e);
    assert_capital(&e,SHIFT,0,None);
    assert_eq!(capital(&e)["commission_operation"],json!(original.operation_id));
    assert_eq!(e.resources().iter().find(|r|r.id==81).unwrap().capacity,4);
    let before = e.state_hash();
    e.execute(Caller::Account(1),original).unwrap();
    assert_eq!(e.state_hash(),before);
    for (day,total,basis,tax,retained,company,treasury) in [
        (1,116,784,392,392,572_200,1_000_000),
        (2,516,500,250,642,572_808,1_000_392),
        (3,916,500,250,892,573_558,1_000_642),
        (4,1200,616,308,1200,574_308,1_000_892),
    ] {
        let original = settle(&mut e,day);
        assert_capital(&e,SHIFT,total,None);
        assert_tax(&e,day,basis,tax,retained);
        assert_eq!(cash(&e,3),company);
        assert_eq!(cash(&e,1),treasury);
        assert_eq!(cash(&e,4),1_228_800);
        assert_eq!(cash(&e,2),200_000-1000*day as i64);
        assert_eq!(e.world().view().accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_000_000);
        let before = e.state_hash();
        e.execute(Caller::System,original).unwrap();
        assert_eq!(e.state_hash(),before);
        let duplicate = request(&e,Action::TownSettle { profile_id:1 },
            NEXT.fetch_add(1,Ordering::Relaxed));
        assert!(e.execute(Caller::System,duplicate).is_err());
        assert_eq!(e.state_hash(),before);
    }
}

#[test]
fn commission_at_day_boundary_has_zero_same_instant_depreciation() {
    let mut e = engine(28_000);
    ready_for_inspection(&mut e);
    advance(&mut e,DAY);
    commission(&mut e);
    settle(&mut e,1);
    assert_capital(&e,DAY,0,None);
    assert_tax(&e,1,900,450,450);
    settle(&mut e,2);
    assert_capital(&e,DAY,400,None);
    assert_tax(&e,2,500,250,700);

    // The reverse legal order at the same instant must not rewrite a closed day.
    let mut after_close = engine(28_000);
    ready_for_inspection(&mut after_close);
    advance(&mut after_close,DAY);
    settle(&mut after_close,1);
    assert_tax(&after_close,1,900,450,450);
    commission(&mut after_close);
    assert_capital(&after_close,DAY,0,None);
    assert_tax(&after_close,1,900,450,450);
    let restored = Engine::restore_checkpoint(&after_close.export_checkpoint().unwrap()).unwrap();
    assert_tax(&restored,1,900,450,450);
    assert_eq!(capital(&restored),capital(&after_close));
    settle(&mut after_close,2);
    assert_capital(&after_close,DAY,400,None);
    assert_tax(&after_close,2,500,250,700);
}

#[test]
fn nonroad_withdrawal_preserves_unassessed_cost_and_freezes_original_basis() {
    let mut e = engine(28_000);
    ready_for_inspection(&mut e);
    commission(&mut e);
    settle(&mut e,1);
    advance(&mut e,DAY+DAY/2);
    command(&mut e,3,C::BeginDemolition { waste_receiver:4 });
    approve(&mut e);
    assert_capital(&e,SHIFT,116,None);
    let withdrawal = effect(&e,C::Start);
    e.execute(Caller::Account(3),withdrawal.clone()).unwrap();
    assert_capital(&e,SHIFT,116,Some(129_600));
    assert_eq!(e.resources().iter().find(|r|r.id==81).unwrap().capacity,2);
    let before = e.state_hash();
    e.execute(Caller::Account(3),withdrawal).unwrap();
    assert_eq!(e.state_hash(),before);
    settle(&mut e,2);
    assert_capital(&e,SHIFT,316,Some(129_600));
    assert_tax(&e,2,700,350,742);
    advance(&mut e,2*DAY+SHIFT);
    let work = effect(&e,C::Work { at:GameTime::new((2*DAY+SHIFT) as i64).unwrap(),
        elapsed_seconds:104_400,worker_seconds:28_800 });
    e.execute(Caller::Account(4),work).unwrap();
    assert_eq!(e.world().view().construction.unwrap().spent,Money::new(58_000).unwrap());
    assert_capital(&e,SHIFT,316,Some(129_600));
    assert_eq!(capital(&e)["subsequent_expense"],json!(28_800));
    settle(&mut e,3);
    assert_capital(&e,SHIFT,316,Some(129_600));
    assert_eq!(capital(&e)["subsequent_expense"],json!(28_800));
    assert_eq!(capital(&e)["charged_subsequent_expense"],json!(28_800));
    let town = e.world().view().town.unwrap();
    let cursor = &town.state.tax_cursors.as_slice()[0];
    assert_eq!(cursor.through_day,3);
    assert_eq!(cursor.cumulative_assessed,Money::new(742).unwrap());
    assert_eq!(cursor.retained_earnings_cents,-27_158);
    let timeline = e.timeline().unwrap();
    assert!(!timeline.claims.iter().any(|claim|
        timeline.tax_source_for_claim(claim).is_some_and(|source|source.period==3)));
}

#[test]
fn invalid_inspector_and_invalid_residual_do_not_publish_capital_or_capacity() {
    let mut e = engine(28_000);
    ready_for_inspection(&mut e);
    let before = e.state_hash();
    let bad = request(&e,Action::Construction {
        command:C::AcceptInspection { inspector:2,accepted:true }
    },NEXT.fetch_add(1,Ordering::Relaxed));
    assert_eq!(e.execute(Caller::Account(2),bad),Err(TxError::Unauthorized));
    assert_eq!(e.state_hash(),before);
    assert!(capital(&e).is_null());
    commission(&mut e);
    assert_capital(&e,SHIFT,0,None);

    let mut bad_residual = engine(29_201);
    ready_for_inspection(&mut bad_residual);
    let before = bad_residual.state_hash();
    let publish = effect(&bad_residual,C::AcceptInspection { inspector:1,accepted:true });
    assert!(bad_residual.execute(Caller::Account(1),publish).is_err());
    assert_eq!(bad_residual.state_hash(),before);
    assert!(capital(&bad_residual).is_null());
    assert_eq!(bad_residual.resources().iter().find(|r|r.id==81).unwrap().capacity,2);
    assert_eq!(cash(&bad_residual,3),571_200);
}


// Boundary additions for #151. These use only existing Engine/assembly paths.
// Prepared/role byte-budget rejection has no public per-call limit here and is
// deliberately not simulated by an unrelated construction cash-budget failure.
fn boundary_engine(input: Value) -> Engine {
    Engine::new(assemble(&serde_json::to_vec(&input).unwrap()).unwrap(),vec![
        Resource::new(80,1).unwrap(),Resource::new(81,2).unwrap(),
        Resource::new(82,10).unwrap(),
    ]).unwrap()
}

#[test]
fn old_opening_without_capital_configuration_keeps_original_market_tax_path() {
    let mut input: Value = serde_json::from_slice(&opening(28_000)).unwrap();
    input.as_object_mut().unwrap().remove("asset_capital");
    let mut e = boundary_engine(input);
    ready_for_inspection(&mut e);
    commission(&mut e);
    assert!(capital(&e).is_null());
    settle(&mut e,1);
    assert!(capital(&e).is_null());
    assert_tax(&e,1,900,450,450);
    assert_eq!(cash(&e,3),572_200);
    settle(&mut e,2);
    assert!(capital(&e).is_null());
    assert_tax(&e,2,900,450,900);
    assert_eq!(cash(&e,3),572_750);
    assert_eq!(cash(&e,1),1_000_450);
}

#[test]
fn invalid_or_missing_bound_capital_policy_and_wrong_binding_are_rejected() {
    let valid: Value = serde_json::from_slice(&opening(28_000)).unwrap();
    // First establish the unchanged positive control for this exact fixture.
    assert!(assemble(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for (field,value) in [
        ("method",json!("Unknown")),
        ("life_seconds",json!(0)),
        ("source",json!({"id":0,"revision":1})),
        ("source",json!({"id":151,"revision":0})),
    ] {
        let mut input = valid.clone();
        input["asset_capital"]["policy"][field] = value;
        assert!(assemble(&serde_json::to_vec(&input).unwrap()).is_err(),"{field}");
    }
    for missing in ["life_seconds","source"] {
        let mut input = valid.clone();
        input["asset_capital"]["policy"].as_object_mut().unwrap().remove(missing);
        assert!(assemble(&serde_json::to_vec(&input).unwrap()).is_err(),"missing {missing}");
    }
    for (field,value) in [("owner",4),("project",8)] {
        let mut input = valid.clone();
        input["asset_capital"][field] = json!(value);
        assert!(assemble(&serde_json::to_vec(&input).unwrap()).is_err(),"wrong {field}");
    }
}

#[test]
fn valid_region_fixture_rejects_the_local_only_capital_configuration() {
    let region: Value = serde_json::from_slice(
        include_bytes!("../../../fixtures/m0-region-opening.json")).unwrap();
    let mut input: Value = serde_json::from_slice(&opening(28_000)).unwrap();
    let policy = input.as_object_mut().unwrap().remove("asset_capital").unwrap();
    input["region"] = region["region"].clone();
    // This positive control distinguishes the explicit local-only policy
    // rejection from a malformed Region or unrelated opening failure.
    assert!(assemble(&serde_json::to_vec(&input).unwrap()).is_ok());
    input["asset_capital"] = policy;
    assert!(assemble(&serde_json::to_vec(&input).unwrap()).is_err());
}

#[test]
fn occupied_commissioned_project_blocks_authorized_town_suspend_and_exit_atomically() {
    let mut input: Value = serde_json::from_slice(&opening(28_000)).unwrap();
    input["town"]["exit_policy"] = json!({
        "policy_id":151,"company_account":3,"requester":2,"executor":1,
        "legal_clearance":"cleared",
        "priority_policy":{
            "order":["tax","wage","secured_debt","unsecured_debt","other"],
            "within_tier":"pro_rata_by_outstanding_stable_id"
        },
        "claims_complete":true,"tax_assessment_complete":true,
        "asset_register_complete":true,"employment_exit_terms_complete":true,
        "claims":[],"payroll":[]
    });
    let mut e = boundary_engine(input);
    ready_for_inspection(&mut e);
    commission(&mut e);
    settle(&mut e,1);
    assert!(e.leases().iter().flatten().any(|lease|lease.actor==3 && lease.occupied));
    for (caller,action) in [
        (Caller::System,Action::TownSuspendOperations { profile_id:1,company_account:3 }),
        (Caller::Account(2),Action::BeginExit { company:3,policy_id:151 }),
    ] {
        let before = e.state_hash();
        let before_capital = capital(&e);
        let envelope = request(&e,action,NEXT.fetch_add(1,Ordering::Relaxed));
        assert_eq!(e.execute(caller,envelope),Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(),before);
        assert_eq!(capital(&e),before_capital);
        assert_capital(&e,SHIFT,116,None);
    }
}

#[test]
fn timeline_partition_preserves_actual_capital_cost_tax_and_cash() {
    let mut direct = engine(28_000);
    ready_for_inspection(&mut direct);
    commission(&mut direct);
    let mut partitioned = engine(28_000);
    ready_for_inspection(&mut partitioned);
    commission(&mut partitioned);
    advance(&mut partitioned,72_000);
    assert_capital(&partitioned,SHIFT,0,None);
    assert_eq!(cash(&partitioned,3),571_200);
    settle(&mut direct,1);
    settle(&mut partitioned,1);
    for e in [&direct,&partitioned] {
        assert_capital(e,SHIFT,116,None);
        assert_tax(e,1,784,392,392);
        assert_eq!(cash(e,3),572_200);
    }
    settle(&mut direct,2);
    settle(&mut partitioned,2);
    for e in [&direct,&partitioned] {
        assert_capital(e,SHIFT,516,None);
        assert_tax(e,2,500,250,642);
        assert_eq!(cash(e,3),572_808);
        assert_eq!(cash(e,1),1_000_392);
        assert_eq!(cash(e,2),198_000);
        assert_eq!(cash(e,4),1_228_800);
    }
}

#[test]
fn optional_asset_same_slot_restore_copy_null_and_owned_accounting() {
    use mycity_core::transaction::EngineSlot;
    let asset = engine(28_000);
    let mut legacy_input: Value = serde_json::from_slice(&opening(28_000)).unwrap();
    legacy_input.as_object_mut().unwrap().remove("asset_capital");
    let legacy = boundary_engine(legacy_input);
    let asset_bytes = asset.export_checkpoint().unwrap();
    let legacy_bytes = legacy.export_checkpoint().unwrap();
    let mut slot = EngineSlot::new().unwrap();
    let root = slot.identity().unwrap().world;
    slot.copy_from_into(&asset).unwrap();
    assert!(slot.engine().unwrap().asset_capital_registration().is_some());
    let asset_owned = slot.identity().unwrap().owned_model_bytes;
    slot.restore_into(&legacy_bytes).unwrap();
    assert!(slot.engine().unwrap().asset_capital_registration().is_none());
    assert_eq!(slot.engine().unwrap().export_checkpoint().unwrap(),legacy_bytes);
    let legacy_owned = slot.identity().unwrap().owned_model_bytes;
    let layout = asset.world().layout();
    assert_eq!(asset_owned - legacy_owned,
        layout["asset_capital_owned_bytes"].as_u64().unwrap() as usize
        + layout["operating_models_header_bytes"].as_u64().unwrap() as usize);
    slot.restore_into(&asset_bytes).unwrap();
    assert!(slot.engine().unwrap().asset_capital_registration().is_some());
    let mut null: Value = serde_json::from_slice(&legacy_bytes).unwrap();
    null["world"]["asset_capital"] = Value::Null;
    slot.restore_into(&serde_json::to_vec(&null).unwrap()).unwrap();
    assert!(slot.engine().unwrap().asset_capital_registration().is_none());
    assert_eq!(slot.identity().unwrap().owned_model_bytes,legacy_owned);
    slot.copy_from_into(&asset).unwrap();
    slot.copy_from_into(&legacy).unwrap();
    assert_eq!(slot.engine().unwrap().export_checkpoint().unwrap(),legacy_bytes);
    assert_eq!(slot.identity().unwrap().world,root);
    assert_eq!(slot.identity().unwrap().owned_model_bytes,legacy_owned);
}

#[test]
fn unpublished_inline_asset_candidate_is_explicitly_rejected() {
    let mut input: Value = serde_json::from_slice(&opening(28_000)).unwrap();
    let policy=input.as_object_mut().unwrap().remove("asset_capital").unwrap();
    input["town"]["asset_capital"]=policy;
    assert!(assemble(&serde_json::to_vec(&input).unwrap()).is_err());
    let mut checkpoint:Value=serde_json::from_slice(&engine(28_000).export_checkpoint().unwrap()).unwrap();
    let runtime=checkpoint["world"].as_object_mut().unwrap().remove("asset_capital").unwrap();
    checkpoint["world"]["town"]["profile"]["asset_capital"]=runtime["opening"].clone();
    assert!(Engine::restore_checkpoint(&serde_json::to_vec(&checkpoint).unwrap()).is_err());
}

#[test]
fn actual_demolition_close_consumes_remaining_basis_once_without_recovery_cash() {
    let mut e=engine(28_000);
    ready_for_inspection(&mut e);commission(&mut e);settle(&mut e,1);
    advance(&mut e,DAY+DAY/2);
    command(&mut e,3,C::BeginDemolition{waste_receiver:4});approve(&mut e);
    let start=effect(&e,C::Start);e.execute(Caller::Account(3),start).unwrap();
    assert!(capital(&e)["retired_at"].is_null());
    settle(&mut e,2);advance(&mut e,2*DAY+SHIFT);
    let work=effect(&e,C::Work{at:GameTime::new((2*DAY+SHIFT)as i64).unwrap(),elapsed_seconds:104_400,worker_seconds:28_800});
    e.execute(Caller::Account(4),work).unwrap();
    // The original effect id is an immutable retry identity. Exercise the rejected
    // Close on a separate same-slot copy, never reuse its ticket for AcceptWaste.
    {
        let mut slot=mycity_core::transaction::EngineSlot::new().unwrap();slot.copy_from_into(&e).unwrap();
        let mut rejected=slot.into_engine().unwrap();let refused=effect(&rejected,C::Close);let hash=rejected.state_hash();
        assert!(rejected.execute(Caller::Account(3),refused).is_err());assert_eq!(rejected.state_hash(),hash);
    }
    let waste=effect(&e,C::AcceptWaste);e.execute(Caller::Account(4),waste).unwrap();
    assert!(capital(&e)["retired_at"].is_null());
    let before=cash(&e,3);let close=effect(&e,C::Close);
    e.execute(Caller::Account(3),close.clone()).unwrap();
    assert_eq!(cash(&e,3),before);
    assert_eq!(e.world().view().construction.unwrap().phase,mycity_core::construction::Phase::Closed);
    assert_eq!(e.resources().iter().find(|r|r.id==80).unwrap().occupied,0);
    assert_eq!(capital(&e)["retirement_cost"],json!(28_884));
    assert_eq!(capital(&e)["charged_retirement_cost"],json!(0));
    let hash=e.state_hash();e.execute(Caller::Account(3),close).unwrap();assert_eq!(e.state_hash(),hash);
    settle(&mut e,3);
    assert_eq!(capital(&e)["charged"],json!(316));
    assert_eq!(capital(&e)["charged_retirement_cost"],json!(28_884));
    assert_eq!(capital(&e)["charged_subsequent_expense"],json!(29_050));
    assert_tax(&e,3,0,0,-56_292);
    settle(&mut e,4);
    assert_tax(&e,4,900,450,-55_842);
    assert_eq!(capital(&e)["basis"],json!(29_200));
    assert_eq!(capital(&e)["charged_retirement_cost"],json!(28_884));
}

#[test]
fn original_sixteen_action_receipts_keep_success_failure_and_atomic_retry() {
    use mycity_core::transaction::{JournalRecord,MAX_ACTIONS};
    let mut e=Engine::new(assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![]).unwrap();
    let from=e.world().account_ref(2).unwrap();let to=e.world().account_ref(3).unwrap();
    let transfer=Action::Transfer{from,to,amount:Money::new(1).unwrap()};
    let mut envelope=request(&e,transfer,NEXT.fetch_add(1,Ordering::Relaxed));
    envelope.actions=vec![transfer;MAX_ACTIONS];
    let mut original=None;
    let result=e.execute_recorded(Caller::Account(2),envelope.clone(),|_,record|{original=Some(*record);Ok::<_,()>(())}).unwrap();
    assert!(result.is_ok());assert_eq!(cash(&e,2),199_984);assert_eq!(cash(&e,3),600_016);
    let original=original.unwrap();
    let decoded:JournalRecord=serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(decoded.actions.into_iter().flatten().collect::<Vec<_>>(),envelope.actions);
    assert_eq!(decoded.outcome,result);
    let hash=e.state_hash();e.execute(Caller::Account(2),envelope).unwrap();assert_eq!(e.state_hash(),hash);
    let transfer=Action::Transfer{from,to,amount:Money::new(100_000).unwrap()};
    let mut failed=request(&e,transfer,NEXT.fetch_add(1,Ordering::Relaxed));failed.actions=vec![transfer;MAX_ACTIONS];
    let hash=e.state_hash();let mut original=None;
    let result=e.execute_recorded(Caller::Account(2),failed.clone(),|_,record|{original=Some(*record);Ok::<_,()>(())}).unwrap();
    assert_eq!(result,Err(TxError::InsufficientFunds));assert_eq!(e.state_hash(),hash);
    let original=original.unwrap();let decoded:JournalRecord=serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(decoded.actions.into_iter().flatten().collect::<Vec<_>>(),failed.actions);assert_eq!(decoded.outcome,result);
    assert_eq!(e.execute(Caller::Account(2),failed),result);assert_eq!(e.state_hash(),hash);
}

#[test]
fn configured_asset_carrier_is_charged_and_original_housing_budget_refuses_atomically() {
    let legacy:Value=serde_json::from_slice(include_bytes!("../../../tests/fixtures/facility-work/legacy-housing18-opening.json")).unwrap();
    let mut input:Value=serde_json::from_slice(&opening(28_000)).unwrap();
    input["housing"]=legacy["housing"].clone();
    // Keep all eighteen dwellings and original relocation inputs. The existing
    // local-city registration supplies their actual source binding without Region.
    let household=&legacy["housing"]["households"][0];
    input["town"]["households"][0]["id"]=household["household_id"].clone();
    input["town"]["delivery"][0]["household_group"]=household["household_id"].clone();
    input["service_needs"]=json!({"version":1,"profile":"LocalDevelopmentNaturalNeedsV1",
        "scope":mycity_core::arena::Scope::new(1,1).unwrap(),"seed":input["bootstrap"]["seed"],
        "city_id":legacy["housing"]["city_id"],"households":[{
            "household_id":household["household_id"],"source_id":household["source_id"],
            "generation":1,"people":household["people"]}]});
    let mut e=boundary_engine(input);
    let before=e.export_checkpoint().unwrap();let hash=e.state_hash();
    let command=request(&e,Action::Timeline{command:T::Enable},NEXT.fetch_add(1,Ordering::Relaxed));
    assert_eq!(e.execute(Caller::System,command),Err(TxError::WorkingLimit));
    assert_eq!(e.state_hash(),hash);assert_eq!(e.export_checkpoint().unwrap(),before);
    assert!(e.asset_capital_registration().is_some());assert!(e.asset_capital_summary().is_none());
    assert!(e.timeline().is_none());
}
