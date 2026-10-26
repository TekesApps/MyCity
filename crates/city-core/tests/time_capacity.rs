//! Original Engine transactions; window peaks have literal independent expectations.
use mycity_core::{assembly::assemble, time_capacity::{Command as C, Registration, Unit}, timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope, Resource, EngineSlot, ENGINE_BYTE_BUDGET}};
fn engine()->Engine {Engine::new(assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),vec![Resource::new(701,80).unwrap()]).unwrap()}
fn run(e:&mut Engine,caller:Caller,a:Action)->Envelope {let request=Envelope{world:1,branch:1,operation_id:e.journal().iter().map(|r|r.operation_id).max().unwrap_or(0)+1,expected_revision:e.world().view().revision,actions:vec![a]};let result=e.execute(caller,request.clone());assert!(result.is_ok(),"{request:?}: {result:?}");request}
fn c(command:C)->Action {Action::TimeCapacity{command}}
fn registration(unit:Unit)->Registration {Registration{canonical:701,resource:701,aliases:[Some(11),Some(12),None,None],unit,owner:1,operator:3,nominal_capacity:80,effective_capacity:80,identity_version:1,policy_version:1,open:true}}
fn setup(e:&mut Engine,unit:Unit){run(e,Caller::System,Action::Timeline{command:T::Enable});run(e,Caller::System,c(C::Register{registration:registration(unit)}));}
#[test]
fn power_peak_uses_original_integer_units_and_disjoint_time_can_reuse_one_pool() {
    let mut e=engine();setup(&mut e,Unit::Kilowatts);
    run(&mut e,Caller::Account(3),c(C::Reserve{alias:11,account:3,consumer:1,consumer_version:1,start:0,end:10,units:65}));
    let before=e.business_comparison_checkpoint().unwrap();let request=Envelope{world:1,branch:1,operation_id:e.journal().iter().map(|r|r.operation_id).max().unwrap_or(0)+1,expected_revision:e.world().view().revision,actions:vec![c(C::Reserve{alias:12,account:4,consumer:2,consumer_version:1,start:9,end:20,units:20})]};let denied=e.execute(Caller::Account(4),request);assert!(matches!(denied,Err(mycity_core::transaction::TxError::Capacity)),"{denied:?}");assert_eq!(before,e.business_comparison_checkpoint().unwrap());
    run(&mut e,Caller::Account(4),c(C::Reserve{alias:12,account:4,consumer:2,consumer_version:1,start:10,end:20,units:20}));assert_eq!(e.resources()[0].reserved,65);
    let bytes=e.export_checkpoint().unwrap();let restored=Engine::restore_checkpoint(&bytes).unwrap();assert_eq!(e.state_hash(),restored.state_hash());assert_eq!(restored.resources()[0].reserved,65);assert_eq!(restored.leases().iter().flatten().map(|l|l.units).sum::<u32>(),85);
    let mut slot=EngineSlot::new().unwrap();slot.copy_from_into(&e).unwrap();let copy=slot.into_engine().unwrap();assert_eq!(copy.state_hash(),e.state_hash());assert!(e.owned_capacity_bytes()<=ENGINE_BYTE_BUDGET);
}
#[test]
fn item_rate_profile_and_old_save_without_profile_keep_distinct_semantics() {
    let mut old=engine();run(&mut old,Caller::Account(3),Action::ReserveCapacity{resource:701,units:80});let saved=old.export_checkpoint().unwrap();assert!(!String::from_utf8(saved.clone()).unwrap().contains("time_capacity"));let restored=Engine::restore_checkpoint(&saved).unwrap();assert_eq!(restored.resources()[0].reserved,80);assert!(restored.time_capacity_projection(11).is_none());
    let mut e=engine();setup(&mut e,Unit::ItemsPerSecond);let request=run(&mut e,Caller::Account(3),c(C::Reserve{alias:11,account:3,consumer:3,consumer_version:1,start:0,end:10,units:80}));let hash=e.state_hash();assert!(e.execute(Caller::Account(3),request).is_ok());assert_eq!(hash,e.state_hash());
    assert_eq!(e.time_capacity_projection(12).unwrap().reserved_peak,80);
}

#[test]
fn full_width_canonical_id_roundtrips_through_original_tagged_action_codec() {
    let mut r=registration(Unit::Count);r.canonical=u128::MAX;
    let action=c(C::Register{registration:r});let json=serde_json::to_string(&action).unwrap();
    assert!(json.contains("340282366920938463463374607431768211455"));
    let decoded:Action=serde_json::from_str(&json).unwrap();assert_eq!(decoded,action);
}

#[test]
fn existing_exchange_resource_cannot_change_to_timed_mode_even_without_leases() {
    let resources=[(101,1),(102,1),(103,1),(104,1),(201,2000),(202,2000)].into_iter().map(|(id,n)|Resource::new(id,n).unwrap()).collect();
    let mut e=Engine::new(assemble(include_bytes!("../../../fixtures/m0-exchange-opening.json")).unwrap(),resources).unwrap();
    let mut r=registration(Unit::Count);r.resource=101;r.nominal_capacity=1;r.effective_capacity=1;
    let before=e.business_comparison_checkpoint().unwrap();let request=Envelope{world:1,branch:1,operation_id:1,expected_revision:0,actions:vec![c(C::Register{registration:r})]};
    assert!(matches!(e.execute(Caller::System,request),Err(mycity_core::transaction::TxError::Resource)));assert_eq!(before,e.business_comparison_checkpoint().unwrap());assert!(e.time_capacity_projection(11).is_none());
}

#[path="../../../tests/fixtures/water-treatment-runtime/opening.rs"]mod v2;
#[test]
fn idle_v2_purifier_receivers_and_storage_cannot_be_reclassified_as_timed_resources() {
    for resource in [304,601,602,603,604] {
        let mut e=v2::engine(v2::input());let mut r=registration(Unit::Count);r.resource=resource;r.nominal_capacity=1;r.effective_capacity=1;
        assert!(!e.leases().iter().flatten().any(|l|l.resource==resource));let before=e.business_comparison_checkpoint().unwrap();
        let request=Envelope{world:1,branch:1,operation_id:1,expected_revision:0,actions:vec![c(C::Register{registration:r})]};
        assert!(matches!(e.execute(Caller::System,request),Err(mycity_core::transaction::TxError::Resource)),"role {resource}");assert_eq!(before,e.business_comparison_checkpoint().unwrap());
    }
}
