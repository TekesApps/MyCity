//! Explicit finite laboratory input. This defines game sources; no real police/medical certificate.
use mycity_core::{assembly,arena::Scope,housing::{GenesisLeaseOffer,LeaseTerms,lease_terms_sha256},
 member_authority,member_qualification::{MemberRange,Background,Skill},qualification_authority as q,
 qualification_state as f,service_needs as n,transaction::Resource,units::{GameTime,Money}};
use serde_json::{json,Value};
fn id(n:u8)->[u8;16]{let mut id=[0;16];id[15]=n;id}
fn offer()->GenesisLeaseOffer{
 let terms=LeaseTerms{lease_id:901,clause_version:1,contract_source_id:9101,landlord_authorization_id:9201,tenant_consent_id:9301,rent_per_cycle:Money::new(1000).unwrap(),cycle:mycity_core::timeline::Cycle::Monthly{anchor:1},deposit:Money::ZERO,escrow_account:3,starts_at:GameTime::ZERO};
 GenesisLeaseOffer{offer_id:901,world_id:1,branch_id:1,unit_id:71,building_id:70,owner_account:1,tenant_account:2,household_id:7,
 valid_from:GameTime::ZERO,valid_until:GameTime::new((400*n::DAY)as i64).unwrap(),asset_source_id:7002,authority_source_id:8001,contract_source_id:terms.contract_source_id,landlord_authorization_id:terms.landlord_authorization_id,tenant_consent_id:9301,
 lease_id:terms.lease_id,clause_version:1,rent_per_cycle:terms.rent_per_cycle,cycle:terms.cycle,deposit:Money::ZERO,escrow_account:3,starts_at:GameTime::ZERO,terms_sha256:lease_terms_sha256(terms).unwrap()}
}
pub fn input()->Value{
 let mut raw:Value=serde_json::from_slice(include_bytes!("../../../crates/city-storage/tests/fixtures/education-store/free-offline-v1.json")).unwrap();
 let origin:[u8;32]=serde_json::from_value(raw["education"]["population_origin"].clone()).unwrap();
 raw.as_object_mut().unwrap().remove("education");raw.as_object_mut().unwrap().remove("venue");raw["bootstrap"]["seed"]=json!(107);raw["housing"]["city_id"]=json!(1);raw["housing"]["lease_offers"]=json!([offer()]);
 let scope=Scope::new(1,1).unwrap();
 raw["member_activity"]=serde_json::to_value(member_authority::Registration{version:1,scope,population_origin:origin,total_people:10,epoch:1}).unwrap();
 let zero=GameTime::ZERO;let until=GameTime::new(200000).unwrap();
 let facts=f::Opening{version:1,profile:f::Profile::ImmutableGameOpeningV1,scope,member_version:1,population_origin:origin,total_people:10,
 authority:f::OpeningAuthority{id:id(1),scope,population_origin:origin,member_version:1,profile:f::Profile::ImmutableGameOpeningV1,issued_at:zero,valid_until:until,permitted_results:0},
 issuers:[None;4],rules:[None;4],course_results:[None;4],
 baseline_sources:std::array::from_fn(|i|if i==0{Some(f::BaselineSourceRegistration{id:id(2),authority_id:id(1),scope,population_origin:origin,member_version:1,valid_from:zero,valid_until:until})}else{None}),
 baselines:std::array::from_fn(|i|if i==0{Some(f::BaselineFact{members:MemberRange{start:0,end:10},source_id:id(2),base_known:true,base_mask:33,background:Some(Background::General),skill:Some(Skill::Technical)})}else{None})};
 raw["qualification"]=serde_json::to_value(q::Opening{protocol_version:3,facts,policies:[None;4]}).unwrap();
 for(id,commodity,quantity,basis,source)in[(991,"electricity",200000,2000,9010),(992,"water",20000,200,9011)]{
 raw["stocks"].as_array_mut().unwrap().push(json!({"id":id,"owner":3,"commodity":commodity,"quantity":quantity,"basis":basis,"source":source}));raw["sources"].as_array_mut().unwrap().push(json!({"id":source,"kind":"inventory_capital","amount":basis}));}
 let mut edges=vec![];for(id,from,to,seconds)in[(1,1,2,120),(2,2,3,180),(3,3,2,180),(4,2,1,120)]{edges.push(json!({"id":id,"from":from,"to":to,"city_from":1,"city_to":1,"free_flow_seconds":seconds,"capacity_people":1000,"mode_mask":2,"open":true,"boundary_source":null}));}
 let path=|a:u32,b:u32|{let mut p=vec![Value::Null;16];p[0]=json!(a);p[1]=json!(b);p};
 let delivery=json!({"version":1,"profile":"LocalDevelopmentEdgeDrivenPoliceV1","domain":{"version":2,"policy":"LocalDevelopmentExplicitHostActivityV1","scope":{"world":1,"branch":1},"city_id":1,"population_origin":origin,"member_version":1,"crews":[{"id":1,"kind":"Police","members":{"start":0,"end":2},"operator":3,"crew_resource":1001,"vehicle_resource":1002,"tools_resource":1003}]},
  "graph":{"revision":1,"nodes":[{"id":1,"city_id":1},{"id":2,"city_id":1},{"id":3,"city_id":1}],"edges":edges,"entrances":[{"id":101,"owner":3,"node":1,"mode_mask":2,"authorized":true},{"id":102,"owner":1,"node":3,"mode_mask":2,"authorized":true}],"demands":[]},
  "edges":[{"edge":1,"resource":2101,"energy_mwh_per_second":50},{"edge":2,"resource":2102,"energy_mwh_per_second":50},{"edge":3,"resource":2102,"energy_mwh_per_second":50},{"edge":4,"resource":2101,"energy_mwh_per_second":50}],
  "crews":[{"crew":1,"station_entrance":101,"driver":0,"energy_stock":991,"water_stock":992,"police_utility_water_stock":992,"employer":3,"wages":[{"member":0,"account":2,"cents_per_hour":1200,"valid_from":0,"valid_until":100000},{"member":1,"account":2,"cents_per_hour":1200,"valid_from":0,"valid_until":100000},null,null]}],
  "routes":[{"household":7,"target_entrance":102,"fee_payer":2,"fee_payee":3,"out":path(1,2),"back":path(3,4)}]});
 raw["service_needs"]=json!({"version":2,"profile":{"LocalDevelopmentWithDeliveryV2":delivery},"scope":{"world":1,"branch":1},"seed":107,"city_id":1,"households":[{"household_id":7,"source_id":7001,"generation":1,"people":10}]});raw
}
pub fn resources()->Vec<Resource>{[1001,1002,1003,2101,2102].into_iter().map(|id|Resource::new(id,1).unwrap()).collect()}
pub fn engine()->mycity_core::transaction::Engine{mycity_core::transaction::Engine::new(assembly::assemble(&serde_json::to_vec(&input()).unwrap()).unwrap(),resources()).unwrap()}
