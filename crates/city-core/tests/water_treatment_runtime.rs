//! Root integration gates. These first tests prove strict registration/restore,
//! not treatment, a Water worker, source fulfillment or full #133 acceptance.
use mycity_core::{assembly,transaction::{Engine,Action,Caller,Envelope},continuous_supply};
#[path="../../../tests/fixtures/water-treatment-runtime/opening.rs"]mod fixture;
#[path="../../../tests/fixtures/continuous-supply/opening.rs"]mod legacy;
use serde_json::{Value,json};
fn bytes(v:&Value)->Vec<u8>{serde_json::to_vec(v).unwrap()}
fn apply(e:&mut Engine,who:Caller,action:Action)->mycity_core::transaction::Receipt{
    let env=Envelope{world:1,branch:1,operation_id:e.world().view().revision+1,expected_revision:e.world().view().revision,actions:vec![action]};e.execute(who,env).unwrap()
}
fn admit(e:&mut Engine,water:bool){
    use mycity_core::{timeline::Command as T,venue_runtime::Command as V,facility_authority::Command as F};
    apply(e,Caller::System,Action::Timeline{command:T::Enable});
    for use_id in [1,2]{
        apply(e,Caller::Account(3),Action::Venue{use_id,command:V::Accept});
        let token=e.world().view().venue.unwrap().uses.iter().find(|s|s.use_id==use_id).unwrap().token.number;
        apply(e,Caller::System,Action::Venue{use_id,command:V::Enter{event_id:1000+use_id,token_number:token,occurred_at:0}});
    }
    apply(e,Caller::System,Action::FacilityWork{command:F::Begin{contract_id:1}});
    if water{apply(e,Caller::System,Action::FacilityWork{command:F::Begin{contract_id:2}});}
}
fn advance(e:&mut Engine,target:u64){
    for _ in 0..8192{
        apply(e,Caller::System,Action::Timeline{command:mycity_core::timeline::Command::Advance{target,limit:256}});
        let p=e.timeline().unwrap().progress;
        if p.target.is_none(){assert_eq!(p.last_complete,target);return;}
    }
    panic!("finite timeline did not finish");
}
#[test]
fn actual_p5_tiny_and_twenty_litre_caps_match_independent_original_owner_oracle(){
    for (cap,time,product_basis,wages,nonwage,port_net) in [(1,1,445,2,703,9999),(19600,2,746,4,704,10000)]{
        let mut input=fixture::input();
        input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"]["outlet_capacities"][0]=cap.into();
        let mut e=fixture::engine(input);admit(&mut e,true);
        apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
        advance(&mut e,time);
        let view=e.world().view();let stock=|id|view.stocks.iter().find(|s|s.id==id).unwrap();
        let cp=e.export_checkpoint().unwrap();
        let root=std::fs::canonicalize(std::env::var_os("MYCITY_WATER_TEST_ROOT").expect("explicit retained evidence root")).unwrap();
        assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
        let path=root.join(format!("actual-p5-cap-{cap}-{}",std::process::id()));std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("checkpoint.json"),&cp).unwrap();
        let saved:Value=serde_json::from_slice(&cp).unwrap();let batch=&saved["world"]["continuous_supply"]["v2"]["batch"];
        assert_eq!(batch["product_cost_wip"],if cap==1{json!({"whole":0,"rem":600001,"den":980000})}else{json!({"whole":0,"rem":1,"den":50})});
        if cap==1{assert_eq!(batch["checkpoint"]["physical"]["raw_stream_basis"],json!({"taken_cents":550,"converted":{"whole":280,"rem":30,"den":49},"wip":{"whole":269,"rem":19,"den":49}}));}
        assert_eq!((stock(202).quantity.value(),stock(202).basis.value()),(cap,product_basis));
        for id in [104,105]{assert_eq!(stock(id).quantity.value(),port_net);}
        let a=|id|view.accounts.iter().find(|a|a.id==id).unwrap();
        assert_eq!(a(2).cash.value(),200000+wages);assert_eq!(a(3).cash.value(),600000-wages-nonwage);
        assert_eq!(a(4).cash.value(),1200000+nonwage);assert_eq!(a(3).protected.value(),0);
        assert_eq!(view.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_000_000);
        for row in view.facility_work.unwrap().records.iter().flatten(){assert_eq!(row.phase,"stopped");assert_eq!(row.work_seconds,time);assert_eq!(row.earned_cents,row.paid_cents);}
        println!("RETAINED_ACTUAL_P5={}",path.display());
    }
}
#[test]
fn full_original_batch_split_and_protected_late_pay_keep_material_and_earned_cost(){
    let mut outcomes=Vec::new();
    for deferred in [false,true]{for split in [false,true]{
        let mut input=fixture::input();
        if deferred{input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"]["wage_settlement"]=json!({"RetainProtectedEarned":{"policy_ref":9003,"version":1}});}
        let mut e=fixture::engine(input);admit(&mut e,true);
        apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
        if split{for t in [1,3,4,17,61,3599,3600,4999,5000,5001,5499,5500]{advance(&mut e,t);}}else{advance(&mut e,5500);}
        let view=e.world().view();let s=|id|view.stocks.iter().find(|s|s.id==id).unwrap();
        let root=std::fs::canonicalize(std::env::var_os("MYCITY_WATER_TEST_ROOT").expect("retained evidence root")).unwrap();assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
        let p=root.join(format!("full-p5-deferred-{deferred}-split-{split}-{}",std::process::id()));std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("before-pay-checkpoint.json"),e.export_checkpoint().unwrap()).unwrap();
        assert_eq!((s(202).quantity.value(),s(202).basis.value()),(98_000_000,177548));
        assert_eq!(s(203).quantity.value(),2_000_000);assert_eq!(s(204).quantity.value(),80);
        for id in [103,104,105]{assert_eq!(s(id).quantity.value(),0);}
        let a=|id|view.accounts.iter().find(|a|a.id==id).unwrap();
        assert_eq!(a(3).protected.value(),if deferred{11002}else{0});
        assert_eq!(a(2).cash.value(),if deferred{200000}else{211002});
        assert_eq!(a(3).cash.value(),if deferred{591720}else{580718});assert_eq!(a(4).cash.value(),1208280);
        for row in view.facility_work.unwrap().records.iter().flatten(){assert_eq!((row.phase,row.work_seconds,row.earned_cents,row.paid_cents,row.wage_remainder),("stopped",5500,5501,if deferred{0}else{5501},1900));}
        if deferred{for id in [1,2]{apply(&mut e,Caller::Account(3),Action::FacilityWork{command:mycity_core::facility_authority::Command::Pay{contract_id:id,cents:5501}});}}
        let view=e.world().view();assert_eq!(view.accounts.iter().find(|a|a.id==3).unwrap().protected.value(),0);
        let cp=e.export_checkpoint().unwrap();std::fs::write(p.join("after-pay-checkpoint.json"),&cp).unwrap();
        let cp:Value=serde_json::from_slice(&cp).unwrap();let b=&cp["world"]["continuous_supply"]["v2"]["batch"];
        assert_eq!(b["product_cost_wip"],json!({"whole":0,"rem":0,"den":1}));
        assert_eq!(b["segments"].as_array().unwrap().iter().filter(|x|!x.is_null()).count(),2);
        outcomes.push((view.stocks,view.accounts,view.continuous_supply.unwrap().operator_labor_basis));
        println!("RETAINED_FULL_P5={}",p.display());
    }}
    for x in &outcomes[1..]{assert_eq!(x,&outcomes[0]);}
}
#[test]
fn actual_cash_boundary_refuses_without_partial_work_or_inventory_and_exact_minimum_runs(){
    for cash in [15106,15107]{
        let mut input=fixture::input();input["accounts"][2]["cash"]=cash.into();input["sources"][2]["amount"]=cash.into();
        input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"]["outlet_capacities"][0]=1.into();
        let mut e=fixture::engine(input);admit(&mut e,true);
        apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
        let before=e.state_hash();let request=Envelope{world:1,branch:1,operation_id:100,expected_revision:e.world().view().revision,actions:vec![Action::Timeline{command:mycity_core::timeline::Command::Advance{target:1,limit:256}}]};
        let result=e.execute(Caller::System,request);
        if cash==15106{assert!(result.is_err());assert_eq!(e.state_hash(),before);assert_eq!(e.world().view().stocks.iter().find(|s|s.id==202).unwrap().quantity.value(),0);}
        else{result.unwrap();assert_eq!(e.world().view().stocks.iter().find(|s|s.id==202).unwrap().quantity.value(),1);}
    }
}
fn move_product(e:&mut Engine,n:i64){
    let action=Action::MoveStock{from:e.world().stock_ref(202).unwrap(),to:e.world().stock_ref(205).unwrap(),quantity:mycity_core::units::Quantity::new(n).unwrap()};
    apply(e,Caller::Account(3),action);
}
#[test]
fn immediate_delayed_and_split_original_product_moves_keep_fixed_prefix_quality_and_cost(){
    let mut final_rows=Vec::new();
    for mode in 0..3{
        let mut e=fixture::engine(fixture::input());admit(&mut e,true);
        apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
        advance(&mut e,if mode==1{4}else{3});
        if mode==2{for n in [1,2,48997]{move_product(&mut e,n);}}else{move_product(&mut e,49000);}
        let diagnostic:Value=serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
        let diagnostic_root=std::fs::canonicalize(std::env::var_os("MYCITY_WATER_TEST_ROOT").unwrap()).unwrap();
        std::fs::write(diagnostic_root.join(format!("outflow-prefix-mode-{mode}-checkpoint.json")),serde_json::to_vec(&diagnostic).unwrap()).unwrap();
        println!("OUTFLOW_PREFIX mode={mode} batch={}",diagnostic["world"]["continuous_supply"]["v2"]["batch"]);
        // Independent original-lot/cell sum at input50000mL:
        // raw600 + reagent1/20 + electric(30+150+2) + Water3
        // + operation3 + concentrate1 + sludge1 =790+1/20 cents.
        let receiver=e.world().view().stocks.into_iter().find(|s|s.id==205).unwrap();assert_eq!((receiver.quantity.value(),receiver.basis.value()),(49000,790),"mode {mode}");
        let quality=e.water_product_quality(205).unwrap().unwrap();
        assert_eq!(quality.judgment,mycity_core::water_treatment::QualityJudgment::MeetsGameChemicalStandard(mycity_core::water_treatment::Standard::OrdinaryGameWaterV1));
        use mycity_core::water_treatment::{Mass,Exact};
        assert_eq!(quality.composition.harmful_mq,Mass::Known(Exact::fraction(1,2000).unwrap()));assert_eq!(quality.composition.allowed_dissolved_mq,Mass::Known(Exact::fraction(1,200).unwrap()));
        advance(&mut e,5500);let v=e.world().view();
        assert_eq!(v.stocks.iter().find(|s|s.id==205).unwrap(),&receiver);assert_eq!(e.water_product_quality(205).unwrap().unwrap().composition,quality.composition);
        let product=v.stocks.iter().find(|s|s.id==202).unwrap();assert_eq!((product.quantity.value(),product.basis.value()),(97_951_000,176758));
        let root=std::fs::canonicalize(std::env::var_os("MYCITY_WATER_TEST_ROOT").unwrap()).unwrap();assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));let p=root.join(format!("outflow-{mode}-{}",std::process::id()));std::fs::create_dir(&p).unwrap();
        let cp=e.export_checkpoint().unwrap();std::fs::write(p.join("checkpoint.json"),&cp).unwrap();std::fs::write(p.join("quality.json"),serde_json::to_vec(&quality).unwrap()).unwrap();
        let raw=Engine::restore_checkpoint(&cp).unwrap();assert!(!raw.continuous_supply_is_authenticated());assert!(raw.water_product_quality(205).is_err());
        final_rows.push((v.stocks,v.accounts,e.water_product_quality(205).unwrap().unwrap().composition));
    }
    for x in &final_rows[1..]{assert_eq!(x,&final_rows[0]);}
}
#[test]
fn product_storage_wrong_caller_direction_capacity_quality_and_window_refuse_atomically(){
    for mode in 0..5{
        let mut input=fixture::input();let profile=&mut input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"];
        match mode{2=>profile["product_storage"]["capacity_ml"]=48000.into(),3=>profile["source_quality"]["composition"]["harmful_mq"]["Known"]["whole"]=300.into(),4=>profile["product_storage"]["until"]=3.into(),_=>{}}
        let mut e=fixture::engine(input);admit(&mut e,true);apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});advance(&mut e,4);
        let before=e.state_hash();let(from,to)=if mode==1{(205,202)}else{(202,205)};
        let env=Envelope{world:1,branch:1,operation_id:1000,expected_revision:e.world().view().revision,actions:vec![Action::MoveStock{from:e.world().stock_ref(from).unwrap(),to:e.world().stock_ref(to).unwrap(),quantity:mycity_core::units::Quantity::new(49000).unwrap()}]};
        assert!(e.execute(if mode==0{Caller::Account(4)}else{Caller::Account(3)},env).is_err(),"mode {mode}");assert_eq!(e.state_hash(),before);
    }
}
#[test]
fn receiver_lease_conflict_rolls_back_transfer_then_full_move_retains_actual_storage_responsibility(){
    let mut e=fixture::engine(fixture::input());admit(&mut e,true);apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});advance(&mut e,5500);
    assert_eq!(e.resources().iter().find(|r|r.id==604).unwrap().occupied,0);
    let reserve=apply(&mut e,Caller::Account(4),Action::ReserveCapacity{resource:604,units:1});let token=reserve.created_tokens[0].unwrap();
    apply(&mut e,Caller::Account(4),Action::OccupyCapacity{token});let before=e.state_hash();
    let env=Envelope{world:1,branch:1,operation_id:1000,expected_revision:e.world().view().revision,actions:vec![Action::MoveStock{from:e.world().stock_ref(202).unwrap(),to:e.world().stock_ref(205).unwrap(),quantity:mycity_core::units::Quantity::new(98_000_000).unwrap()}]};
    assert!(e.execute(Caller::Account(3),env).is_err());assert_eq!(e.state_hash(),before);
    apply(&mut e,Caller::Account(4),Action::ReleaseCapacity{token});move_product(&mut e,98_000_000);
    for(id,occupied)in[(301,0),(302,0),(303,0),(304,0),(601,0),(602,1),(603,1),(604,1)]{assert_eq!(e.resources().iter().find(|r|r.id==id).unwrap().occupied,occupied,"resource{id}");}
    advance(&mut e,7201);assert_eq!(e.resources().iter().find(|r|r.id==604).unwrap().occupied,1);
    let s=e.world().view().stocks.into_iter().find(|s|s.id==205).unwrap();assert_eq!((s.quantity.value(),s.basis.value()),(98_000_000,177548));
}
#[test]
fn original_process_port_capacity_stops_at_last_whole_safe_cell_and_retains_backlog(){
    let mut input=fixture::input();input["continuous_supply"]["water_capacities_ml"]=json!([20000,20000]);
    let mut e=fixture::engine(input);admit(&mut e,true);apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
    advance(&mut e,13);let v=e.world().view();let source=v.continuous_supply.unwrap();
    assert_eq!(source.genset.status,mycity_core::continuous_genset::Status::Stopped);assert_eq!(source.genset.active_seconds,12);assert_eq!(source.raw_used_ml,240000);
    for id in [104,105]{assert_eq!(v.stocks.iter().find(|s|s.id==id).unwrap().quantity.value(),20000);}
    assert_eq!(v.stocks.iter().find(|s|s.id==202).unwrap().quantity.value(),196000);
    for row in v.facility_work.unwrap().records.iter().flatten(){assert_eq!((row.phase,row.work_seconds),("stopped",12));}
}
#[test]
fn actual_reagent_residue_receiving_shift_and_qualification_boundaries_cut_before_unlicensed_work(){
    // Original qualification valid_until is inclusive; its legal change is+1,
    // unlike the half-open facility/receiving windows. Do not rewrite that gate.
    for (mode,seconds,product) in [(0,60,980000),(1,3,49000),(2,75,1225000),(3,1,16333),(4,2,32666),(5,4,65333)]{
        let mut input=fixture::input();
        match mode{
            0=>input["stocks"].as_array_mut().unwrap().iter_mut().find(|s|s["id"]==201).unwrap()["quantity"]=1.into(),
            1=>input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"]["outlet_capacities"][1]=1000.into(),
            2=>input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"]["outlet_capacities"][2]=1.into(),
            3=>{let p=&mut input["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"];p["receiving_until"]=1.into();p["product_storage"]["until"]=1.into();},
            4=>input["facility_work"]["work"]["contracts"][1]["end"]=2.into(),
            _=>input["qualification"]["facts"]["baseline_sources"][0]["valid_until"]=3.into(),
        }
        let mut e=fixture::engine(input);admit(&mut e,true);apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
        advance(&mut e,seconds+1);let v=e.world().view();assert_eq!(v.continuous_supply.unwrap().genset.active_seconds,seconds,"mode{mode}");
        assert_eq!(v.stocks.iter().find(|s|s.id==202).unwrap().quantity.value(),product,"mode{mode}");
        for row in v.facility_work.unwrap().records.iter().flatten(){assert_eq!((row.phase,row.work_seconds),("stopped",seconds));}
    }
}
#[test]
fn actual_joint_start_and_zero_work_exit_keep_original_cash_and_responsibility(){
    use mycity_core::{facility_authority::Command as F,venue_runtime::Command as V};
    for mode in 0..4{
        let mut e=fixture::engine(fixture::input());admit(&mut e,true);
        let env=Envelope{world:1,branch:1,operation_id:100,expected_revision:e.world().view().revision,actions:vec![Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}}]};
        let started=e.execute(Caller::Account(3),env.clone()).unwrap();let cp:Value=serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
        assert_eq!(cp["world"]["continuous_supply"]["v2"]["batch"]["config"]["material"]["binding"]["operation"],100);
        assert_eq!(cp["world"]["continuous_supply"]["v2"]["batch"]["start_revision"],started.revision);
        let token=e.world().view().venue.unwrap().uses.iter().find(|s|s.use_id==2).unwrap().token.number;
        let action=match mode{
            0=>Action::ContinuousSupply{command:continuous_supply::Command::Stop},
            1=>Action::ContinuousSupply{command:continuous_supply::Command::Fault{fault_ref:9000}},
            2=>Action::FacilityWork{command:F::Stop{contract_id:2}},
            _=>Action::Venue{use_id:2,command:V::SafeExit{event_id:2000,token_number:token,occurred_at:0}},
        };
        apply(&mut e,if mode<=1{Caller::Account(3)}else if mode==2{Caller::Account(2)}else{Caller::System},action);
        let view=e.world().view();let source=view.continuous_supply.as_ref().unwrap();assert_eq!(source.zero_work_expense.value(),700);assert_eq!(source.produced_ml,[0;2]);
        for row in view.facility_work.unwrap().records.iter().flatten(){assert_eq!(row.phase,"stopped");assert_eq!((row.work_seconds,row.earned_cents,row.paid_cents),(0,0,0));}
        let a=|id|view.accounts.iter().find(|a|a.id==id).unwrap();assert_eq!(a(3).cash.value(),599300);assert_eq!(a(3).protected.value(),0);assert_eq!(a(2).cash.value(),200000);assert_eq!(a(4).cash.value(),1200700);
        assert_eq!(view.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_000_000);
        let hash=e.state_hash();assert_eq!(e.execute(Caller::Account(3),env).unwrap().operation_id,100);assert_eq!(hash,e.state_hash());
        let mut raw=Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();assert!(!raw.continuous_supply_is_authenticated());assert!(!raw.facility_work_is_authenticated());
        assert_eq!(raw.state_hash(),e.state_hash());
        let request=Envelope{world:1,branch:1,operation_id:1000,expected_revision:raw.world().view().revision,actions:vec![Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:2,pumps:3}}]};
        assert!(raw.execute(Caller::Account(3),request).is_err());
    }
}
#[test]
fn missing_actual_water_worker_cannot_start_or_create_free_work(){
    let mut e=fixture::engine(fixture::input());admit(&mut e,false);let before=e.state_hash();
    let request=Envelope{world:1,branch:1,operation_id:100,expected_revision:e.world().view().revision,actions:vec![Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}}]};
    assert!(e.execute(Caller::Account(3),request).is_err());assert_eq!(before,e.state_hash());
    let view=e.world().view();assert_eq!(view.continuous_supply.unwrap().startup_wip.value(),0);
    assert_eq!(view.accounts.iter().find(|a|a.id==3).unwrap().protected.value(),7202);
}
#[test]
fn explicit_v2_registration_retains_original_stocks_but_cannot_run_v1(){
    let mut e=fixture::engine(fixture::input());
    let view=e.world().view().continuous_supply.unwrap();assert_eq!(view.version,2);
    assert_eq!(view.produced_ml,[0;2]);assert_eq!(view.raw_used_ml,0);
    let cp=e.export_checkpoint().unwrap();let v:Value=serde_json::from_slice(&cp).unwrap();
    assert_eq!(v["world"]["continuous_supply"]["v2"]["version"],2);
    assert_eq!(v["world"]["continuous_supply"]["v2"]["outflow"],json!([{"ml":0,"cents":0},{"ml":0,"cents":0}]));
    let stocks=e.world().view().stocks;
    for (id,quantity,cost) in [(101,100000,100000),(102,6000000,18000),(103,100000000,50000),(104,0,0),(105,0,0),(201,100,100),(202,0,0),(203,0,0),(204,0,0)]{
        let s=stocks.iter().find(|s|s.id==id).unwrap();assert_eq!((s.quantity.value(),s.basis.value()),(quantity,cost));
    }
    let mut restored=Engine::restore_checkpoint(&cp).unwrap();
    assert!(restored.continuous_supply_is_authenticated()); // pristine genesis only
    assert_eq!(e.state_hash(),restored.state_hash());
    let layout=continuous_supply::budget_layout();
    let base=layout["runtime_owned_bytes"].as_u64().unwrap();
    let deep=layout["v2_opening_backing_bytes"].as_u64().unwrap()+layout["v2_state_backing_bytes"].as_u64().unwrap();
    assert!(deep>0);let actual=e.world().layout()["continuous_supply_owned_bytes"].as_u64().unwrap();
    assert_eq!(actual,base+deep);assert!(actual<=continuous_supply::MAX_OWNED_BYTES as u64);
    assert_eq!(restored.world().layout()["continuous_supply_owned_bytes"],actual);
    assert_eq!(legacy::engine(legacy::input()).world().layout()["continuous_supply_owned_bytes"],base);
    for root in [&mut e,&mut restored]{
        let hash=root.state_hash();
        let request=Envelope{world:1,branch:1,operation_id:100,expected_revision:root.world().view().revision,actions:vec![Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}}]};
        assert!(root.execute(Caller::Account(3),request).is_err());
        assert_eq!(hash,root.state_hash());
    }
}
#[test]
fn mismatched_missing_and_invented_v2_history_are_not_migrated(){
    let e=fixture::engine(fixture::input());let original:Value=serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
    for mode in 0..6{
        let mut v=original.clone();let r=&mut v["world"]["continuous_supply"];
        match mode{
            0=>{r.as_object_mut().unwrap().remove("v2");},
            1=>r["v2"]=Value::Null,
            2=>r["v2"]["version"]=1.into(),
            3=>r["opening"]["version"]=1.into(),
            4=>r["opening"]["profile"]="ContinuousGensetTwinPumpV1".into(),
            _=>r["v2"]["outflow"][0]["ml"]=1.into(),
        }
        assert!(Engine::restore_checkpoint(&bytes(&v)).is_err(),"case {mode}");
    }
    let old=legacy::engine(legacy::input());let v:Value=serde_json::from_slice(&old.export_checkpoint().unwrap()).unwrap();
    assert!(v["world"]["continuous_supply"].get("v2").is_none());
    assert_eq!(v["world"]["continuous_supply"]["opening"]["profile"],"ContinuousGensetTwinPumpV1");
    let mut injected=v;injected["world"]["continuous_supply"]["v2"]=original["world"]["continuous_supply"]["v2"].clone();
    assert!(Engine::restore_checkpoint(&bytes(&injected)).is_err());
}
#[test]
fn unknown_quality_aliases_and_missing_original_receivers_refuse_genesis(){
    for mode in 0..12{
        let mut v=fixture::input();let o=&mut v["continuous_supply"]["profile"]["ContinuousGensetTwinPumpTreatmentV2"];
        match mode{
            0=>o["source_quality"]["composition"]["harmful_mq"]="Unknown".into(),
            1=>o["source_quality"]["lot_ref"]=113.into(),
            2=>o["reagent_stock"]=104.into(),
            3=>o["outlet_stocks"][1]=202.into(),
            4=>o["receiver_resources"][0]=304.into(),
            5=>o["receiving_permission_ref"]=0.into(),
            6=>o["source_quality"]["references"][0]["coverage_until"]=5.into(),
            7=>o["receiver_account"]=3.into(),
            8=>o["nominal_power_w"]=0.into(),
            9=>o["wage_settlement"]=json!({"RetainProtectedEarned":{"policy_ref":9030,"version":0}}),
            10=>o["wage_settlement"]=Value::Null,
            _=>{o.as_object_mut().unwrap().remove("wage_settlement");},
        }
        assert!(assembly::assemble(&bytes(&v)).is_err(),"case {mode}");
    }
    let world=assembly::assemble(&bytes(&fixture::input())).unwrap();
    assert!(Engine::new(world,legacy::resources()).is_err());
}

#[test]
fn an_actual_old_start_cannot_be_relabelled_as_a_v2_batch(){
    use mycity_core::{timeline::Command as T,venue_runtime::Command as V,facility_authority::Command as F};
    fn apply(e:&mut Engine,who:Caller,a:Action){let request=Envelope{world:1,branch:1,operation_id:e.world().view().revision+1,
        expected_revision:e.world().view().revision,actions:vec![a]};e.execute(who,request).unwrap();}
    let fresh=fixture::input();let mut old=fresh.clone();old["continuous_supply"]["version"]=1.into();old["continuous_supply"]["profile"]="ContinuousGensetTwinPumpV1".into();
    let mut e=fixture::engine(old);
    apply(&mut e,Caller::System,Action::Timeline{command:T::Enable});
    apply(&mut e,Caller::Account(3),Action::Venue{use_id:1,command:V::Accept});
    let token=e.world().view().venue.unwrap().uses[0].token.number;
    apply(&mut e,Caller::System,Action::Venue{use_id:1,command:V::Enter{event_id:1000,token_number:token,occurred_at:0}});
    apply(&mut e,Caller::System,Action::FacilityWork{command:F::Begin{contract_id:1}});
    apply(&mut e,Caller::Account(3),Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id:1,pumps:3}});
    let mut cp:Value=serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
    assert!(Engine::restore_checkpoint(&bytes(&cp)).is_ok());
    let pristine=fixture::engine(fresh.clone());let pristine:Value=serde_json::from_slice(&pristine.export_checkpoint().unwrap()).unwrap();
    cp["world"]["continuous_supply"]["opening"]["version"]=2.into();
    cp["world"]["continuous_supply"]["opening"]["profile"]=fresh["continuous_supply"]["profile"].clone();
    cp["world"]["continuous_supply"]["v2"]=pristine["world"]["continuous_supply"]["v2"].clone();
    // An honest zero-outflow substate is insufficient: the first real V2 batch
    // must already bind that original Start/revision. Never infer it on restore.
    assert!(Engine::restore_checkpoint(&bytes(&cp)).is_err());
}
