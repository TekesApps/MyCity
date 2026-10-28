//! #21 finite joint input; original fixtures and frozen M0 inputs stay intact.
//! Contractor 4 supplies the existing aggregate construction service. These
//! worker-seconds do not assert local household member identity or geometry.
#[path="opening.rs"]mod base;
use mycity_core::{construction::{Project,Kind,Gate,GateCategory,GateDecision,Material,
    ConstructionUtilityContract,ConstructionUtilitySupply},transaction::Resource,units::{GameTime,Money,Quantity},timeline::DAY};
use serde_json::{json,Value};
pub fn input()->Value{
    let mut v=base::boot_service_input();
    v["payroll_members"]=json!({"version":1,"profile":"LocalDevelopmentActualJobGroupV1",
        "scope":v["member_activity"]["scope"],"population_origin":v["member_activity"]["population_origin"],"member_version":1,
        "bindings":[{"spec":{"id":21,"payer":7,"payee":2,"home_city":0,"hourly_cents":10,"cycle":{"kind":"daily"},"ends_at":3*DAY},
            "members":{"start":0,"end":1},"source":{"id":15501,"version":1}}]});
    let mut p=Project::new(7,Kind::Build,7,4,1,80,1,1,Money::new(50_000).unwrap(),Money::new(3600).unwrap(),28_800,
        &[Gate{id:1,category:GateCategory::OwnerConsent,evidence_id:0,party:2,decided:GateDecision::Pending},
          Gate{id:2,category:GateCategory::Permit,evidence_id:0,party:1,decided:GateDecision::Pending}],
        &[Material{stock_id:15,required:Quantity::new(100).unwrap(),consumed:Quantity::ZERO,basis_per_unit:Money::new(1000).unwrap(),basis_remainder:0}],GameTime::ZERO).unwrap();
    p.operating_resource=81;p.operating_base_capacity=2;p.operating_units=2;
    p.waste_receiver_account=4;p.waste_receiver_resource=82;p.waste_units=2;p.waste_fee=Money::new(250).unwrap();
    let utility=|stock_id|ConstructionUtilitySupply{operator:3,stock_id,rate_per_3600_worker_seconds:Quantity::new(1000).unwrap(),tariff_per_quantity_unit:Money::new(1).unwrap(),quantity_unit:Quantity::new(1000).unwrap()};
    p.utility_contract=Some(ConstructionUtilityContract{electricity:Some(utility(3002)),water:Some(utility(3003))});
    v["construction"]=serde_json::to_value(p).unwrap();
    v["asset_capital"]=json!({"version":2,"project":7,"owner":7,"disposal":"ZeroRecovery","policy":{"revision":1,"source":{"id":151,"revision":1},"method":"StraightLine","clock":"LocalDevelopment","life_seconds":3*DAY,"cost_quantum":1000,"residual":27716}});
    v
}
pub fn resources()->Vec<Resource>{let mut r=base::resources();r.extend([Resource::new(80,1).unwrap(),Resource::new(81,2).unwrap(),Resource::new(82,10).unwrap()]);r}

/// Separate, explicitly pre-approved whole lifecycle contract: actual build and
/// demolition total 57,982 cents. The 50,000-cent operating case stays intact.
/// This changes no cash endowment, resource budget, load or frozen M0 criterion.
pub fn closing_input()->Value{let mut v=input();v["construction"]["budget"]=json!(60000);v}
