//! #159 opening-source gate only; not maintenance delivery or site acceptance.
use mycity_core::{assembly, member_qualification::{BaseQualification,MemberRange},
    qualification_authority::{JobRequirements,JobStatus},timeline,transaction::{Engine,Caller,Envelope,Action}};
#[path="../../../tests/fixtures/finite-city/maintenance_opening.rs"] mod fixture;
#[path="../../../tests/fixtures/finite-city/asset_opening.rs"] mod original;
#[test]
fn explicit_maintenance_sources_preserve_people_money_stocks_and_old_service_qualification() {
    let raw=fixture::base_input();let old=original::input();
    for key in ["accounts","sources","stocks","member_activity","housing"] {assert_eq!(raw[key],old[key],"no new endowment or people: {key}");}
    let mut engine=Engine::new(assembly::assemble(&serde_json::to_vec(&raw).unwrap()).unwrap(),fixture::resources()).unwrap();
    let (scope,revision)=engine.world().scope_revision();
    engine.execute(Caller::System,Envelope{world:scope.world(),branch:scope.branch(),operation_id:1,expected_revision:revision,
        actions:vec![Action::Timeline{command:timeline::Command::Enable}]}).unwrap();
    for (end,required) in [(4,BaseQualification::Electric.bit()),(2,BaseQualification::Driver.bit()|BaseQualification::Police.bit())] {
        let read=engine.check_job_qualification(JobRequirements{members:MemberRange{start:0,end},minimum_background:None,
            minimum_skill:None,required_base_mask:required,required_endorsement_mask:0}).unwrap();
        assert!(matches!(read.status,JobStatus::Eligible),"{read:?}");
    }
    assert_eq!(engine.world().view().accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);
    assert_eq!(fixture::resources().iter().find(|r|r.id==81).unwrap().capacity,0);
    assert!(raw.get("venue").is_none(),"no worksite is claimed by this initial source gate");
}

#[test]
fn registered_maintenance_uses_its_own_bay_and_a_distinct_source_version() {
    let raw=fixture::registered_input();
    let engine=Engine::new(assembly::assemble(&serde_json::to_vec(&raw).unwrap()).unwrap(),fixture::registered_resources()).unwrap();
    let venue=engine.world().view().venue.unwrap();
    assert_eq!(venue.venue_count,1);
    let registration=&raw["venue"]["venues"][0];
    assert_eq!(registration["venue_id"],1591);
    assert_eq!(registration["owner"],7);
    assert_eq!(registration["operator"],7);
    assert!(venue.uses.is_empty()); // Registration is not actual entry or work.
    assert!(engine.owned_capacity_bytes()<mycity_core::transaction::ENGINE_BYTE_BUDGET);
    // This purpose has no general permission outside its explicit maintenance binding.
    let mut unrelated=raw.clone();unrelated.as_object_mut().unwrap().remove("facility_work");
    assert!(Engine::new(assembly::assemble(&serde_json::to_vec(&unrelated).unwrap()).unwrap(),fixture::registered_resources()).is_err());
    for (field,value) in [("authority_version",serde_json::json!(2)),("maintenance",serde_json::Value::Null)] {
        let mut bad=raw.clone();bad["facility_work"][field]=value;
        assert!(assembly::assemble(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn unused_maintenance_backing_keeps_original_same_slot_restore_and_copy() {
    use mycity_core::transaction::EngineSlot;
    let raw=fixture::registered_input();
    let registered=Engine::new(assembly::assemble(&serde_json::to_vec(&raw).unwrap()).unwrap(),fixture::registered_resources()).unwrap();
    let old=Engine::new(assembly::assemble(&serde_json::to_vec(&fixture::base_input()).unwrap()).unwrap(),fixture::resources()).unwrap();
    let saved=registered.export_checkpoint().unwrap();let old_saved=old.export_checkpoint().unwrap();
    let mut slot=EngineSlot::new().unwrap();let root=slot.identity().unwrap().world;
    slot.copy_from_into(&registered).unwrap();
    let configured_layout=slot.engine().unwrap().world().layout();
    let maintenance_bytes=configured_layout["maintenance_owned_bytes"].as_u64().unwrap();
    assert!(maintenance_bytes>0);
    assert!(configured_layout["facility_work_owned_bytes"].as_u64().unwrap()>maintenance_bytes);
    let registration=registered.facility_work_registration();assert!(registration.is_some());
    slot.restore_into(&old_saved).unwrap();let legacy=slot.identity().unwrap().owned_model_bytes;
    assert!(slot.engine().unwrap().world().layout().get("maintenance_owned_bytes").is_none());
    assert!(slot.engine().unwrap().facility_work_registration().is_none());
    slot.restore_into(&saved).unwrap();
    assert_eq!(slot.engine().unwrap().facility_work_registration(),registration);
    assert_eq!(slot.engine().unwrap().export_checkpoint().unwrap(),saved);
    assert_eq!(slot.engine().unwrap().world().layout()["maintenance_owned_bytes"],maintenance_bytes);
    assert!(slot.identity().unwrap().owned_model_bytes<=mycity_core::transaction::ENGINE_BYTE_BUDGET);
    let mut null:serde_json::Value=serde_json::from_slice(&old_saved).unwrap();
    null["world"]["facility_work"]=serde_json::Value::Null;
    slot.restore_into(&serde_json::to_vec(&null).unwrap()).unwrap();
    slot.copy_from_into(&registered).unwrap();slot.copy_from_into(&old).unwrap();
    assert_eq!(slot.identity().unwrap().world,root);
    assert!(slot.engine().unwrap().world().layout().get("facility_work_owned_bytes").is_none());
    assert!(slot.identity().unwrap().owned_model_bytes<=mycity_core::transaction::ENGINE_BYTE_BUDGET);
    eprintln!("legacy restored retained={legacy}; final copied retained={}",slot.identity().unwrap().owned_model_bytes);
    assert_eq!(slot.engine().unwrap().export_checkpoint().unwrap(),old_saved);
}

#[test]
fn maintenance_origin_is_created_only_by_the_original_successful_commission() {
    use mycity_core::{boot::{Command as B,Facility},construction::{EngineCommand as C,GateDecision,OPERATION_PREFIX},
        housing,service_delivery_authority::{Command as D,Motion},timeline::Command as T,units::GameTime};
    fn apply(e:&mut Engine,caller:Caller,action:Action,effect:bool)->Envelope {
        let(scope,revision)=e.world().scope_revision();
        let operation_id=if effect {let p=e.world().view().construction.unwrap();OPERATION_PREFIX|(p.id<<32)|u64::from(p.next_effect)}else{10_000_000+revision};
        let env=Envelope{world:scope.world(),branch:scope.branch(),operation_id,expected_revision:revision,actions:vec![action]};
        e.execute(caller,env.clone()).unwrap();env
    }
    fn advance(e:&mut Engine,target:u64){for _ in 0..8192{let p=e.timeline().unwrap().progress;if p.target.is_none()&&p.last_complete==target{return;}
        apply(e,Caller::System,Action::Timeline{command:T::Advance{target,limit:256}},false);}panic!("unfinished {target}");}
    let raw=fixture::registered_input();
    let mut e=Engine::new(assembly::assemble(&serde_json::to_vec(&raw).unwrap()).unwrap(),fixture::registered_resources()).unwrap();
    assert!(e.maintenance_summary().is_none());
    apply(&mut e,Caller::System,Action::Timeline{command:T::Enable},false);
    let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();
    apply(&mut e,Caller::Account(2),Action::HousingLease{offer_id:901,offer_sha256:housing::genesis_lease_offer_sha256(offer)},false);
    for(actor,command)in[(7,C::File),(2,C::DecideGate{gate_id:1,decision:GateDecision::Accepted,evidence_id:11}),
        (1,C::DecideGate{gate_id:2,decision:GateDecision::Accepted,evidence_id:12}),(7,C::Prepare)] {
        apply(&mut e,Caller::Account(actor),Action::Construction{command},false);
    }
    apply(&mut e,Caller::Account(7),Action::Construction{command:C::Start},true);
    for(facility,target)in[(Facility::Pump,60),(Facility::Generator,120)] {
        apply(&mut e,Caller::System,Action::Boot{command:B::Start{facility,reverse_request:false}},false);advance(&mut e,target);
    }
    advance(&mut e,28800);apply(&mut e,Caller::Account(3),Action::ServiceDelivery{command:D::Accept{household:7,crew:1}},false);
    advance(&mut e,33900);assert_eq!(e.service_needs_summary().unwrap().delivery.unwrap().trips[0].motion,Motion::Finished);
    advance(&mut e,61200);
    apply(&mut e,Caller::Account(4),Action::Construction{command:C::Work{at:GameTime::new(61200).unwrap(),elapsed_seconds:61200,worker_seconds:28800}},true);
    assert!(e.maintenance_summary().is_none());
    apply(&mut e,Caller::Account(4),Action::Construction{command:C::SubmitInspection},true);
    let original=apply(&mut e,Caller::Account(1),Action::Construction{command:C::AcceptInspection{inspector:1,accepted:true}},true);
    let m=e.maintenance_summary().unwrap();assert_eq!((m.project,m.component,m.commissioned_at,m.commission_operation,m.condition_bps),(7,1,61200,original.operation_id,10000));
    assert_eq!(e.resources().iter().find(|r|r.id==81).unwrap().capacity,2);
    let saved=e.export_checkpoint().unwrap();let hash=e.state_hash();e.execute(Caller::Account(1),original).unwrap();
    assert_eq!(e.state_hash(),hash);assert_eq!(e.export_checkpoint().unwrap(),saved);
    let restored=Engine::restore_checkpoint(&saved).unwrap();assert_eq!(restored.maintenance_summary(),Some(m));
    assert_eq!(restored.export_checkpoint().unwrap(),saved);
    // This covers the commissioning-only checkpoint, not actual maintenance or Store recovery.
    assert!(restored.world().layout()["maintenance_owned_bytes"].as_u64().unwrap()>0);
    assert!(restored.owned_capacity_bytes()<=mycity_core::transaction::ENGINE_BYTE_BUDGET);
}
