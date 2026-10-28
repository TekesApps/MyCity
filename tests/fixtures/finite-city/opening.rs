//! #21 common genesis. Explicit finite sources; no claim of perpetual supply.
#[path="../service-delivery/opening.rs"]mod service;
use mycity_core::transaction::Resource;
use serde_json::{json,Value};
pub fn input()->Value {
    let mut v=service::input();
    let source:Value=serde_json::from_slice(include_bytes!("../../../crates/city-core/tests/fixtures/issue-120/canonical/core-town-utility-opening.json")).unwrap();
    // Existing service household/account identity is canonical. These are new,
    // explicitly endowed stocks and one separate enterprise, not copied balances.
    v["accounts"].as_array_mut().unwrap().push(json!({"id":7,"role":"company","cash":100000,"source":21001}));
    for (id,amount,kind) in [(21001,100000,"cash_capital"),(21002,1000,"inventory_capital"),(21003,0,"inventory_capital")] {
        v["sources"].as_array_mut().unwrap().push(json!({"id":id,"kind":kind,"amount":amount}));
    }
    for (id,commodity,quantity,basis,source) in [(15,"raw",1000,1000,21002),(16,"product",0,0,21003)] {
        v["stocks"].as_array_mut().unwrap().push(json!({"id":id,"owner":7,"commodity":commodity,"quantity":quantity,"basis":basis,"source":source}));
    }
    for (i,stock) in source["stocks"].as_array().unwrap()[..6].iter().enumerate() {
        let mut stock=stock.clone();let id=3001+i as u32;let sid=22001+i as u32;
        stock["id"]=json!(id);stock["source"]=json!(sid);
        v["sources"].as_array_mut().unwrap().push(json!({"id":sid,"kind":"inventory_capital","amount":stock["basis"]}));
        v["stocks"].as_array_mut().unwrap().push(stock);
    }
    v["boot"]=source["boot"].clone();v["boot"]["profile"]=json!(mycity_core::boot::LOCAL_PROFILE);v["boot"]["stocks"]=json!([3001,3002,3003,3004,3005,3006]);v["boot"]["resources"]=json!([3101,3102]);
    v["town"]=source["town"].clone();
    v["town"]["households"][0]["id"]=json!(7);v["town"]["households"][0]["account"]=json!(2);
    v["town"]["households"][0]["reference_price_per_q"]=json!(10);
    v["town"]["shops"][0]["shelf_price_per_q"]=json!(10);
    v["town"]["delivery"][0]["household_group"]=json!(7);
    v["town"]["utility_supply"]["boot_stock_id"]=json!(3002);
    let person=json!({"kind":"person","id":7001});
    v["town"]["equity_charters"]=json!([{"company":7,"policy":{"policy_id":21,"version":1,"cents_per_share":100,"max_issued_shares":10000,"max_direct_shareholders":8,"ordinary_shares_only":true,"cross_company_holding_allowed":false,"secondary_market_enabled":false},"distribution_authority":person,"investors":[{"party":person,"account":2,"opening_shares":1010}],"opening_sources":[{"source_id":21001,"party":person},{"source_id":21002,"party":person},{"source_id":21003,"party":person}]}]);
    v["town"]["tax_rules"]=json!([{"company":7,"policy_id":21,"kind":"daily_realized_market_margin","authority_account":1,"rate_basis_points":5000}]);
    v
}
pub fn resources()->Vec<Resource> {
    let mut r=service::resources();r.extend([Resource::new(3101,1).unwrap(),Resource::new(3102,1).unwrap()]);r
}

/// New explicit service strategy consumes the original Boot outputs, not a refill.
pub fn boot_service_input()->Value {
    let mut v=input();
    let delivery=&mut v["service_needs"]["profile"]["LocalDevelopmentWithDeliveryV2"];
    delivery["version"]=json!(3);delivery["profile"]=json!("LocalDevelopmentEdgeDrivenPoliceBootV3");
    for c in delivery["crews"].as_array_mut().unwrap(){c["energy_stock"]=json!(3002);c["water_stock"]=json!(3003);c["police_utility_water_stock"]=json!(3003);}
    v["stocks"].as_array_mut().unwrap().retain(|s|s["id"]!=991&&s["id"]!=992);
    v["sources"].as_array_mut().unwrap().retain(|s|s["id"]!=9010&&s["id"]!=9011);v
}
