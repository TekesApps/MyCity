//! Explicit lab prices/stock capital; no sanitary qualification or performance evidence.
use mycity_core::{assembly,continuous_supply,transaction::{Action,Caller,Engine,Envelope,Resource,Receipt},timeline};
use serde_json::{json,Value};
pub fn input()->Value{
    let mut sources=vec![json!({"id":1,"kind":"cash_capital","amount":100000}),json!({"id":2,"kind":"cash_capital","amount":0})];
    let quantities=[100000,6000000,100000000,0,0,0,0];let bases=[100000,18000,50000,0,0,0,0];
    let commodities=["fuel","water","raw_water","water","water","heat","waste_water"];
    let mut stocks=vec![];
    for i in 0..7{sources.push(json!({"id":10+i,"kind":"inventory_capital","amount":bases[i]}));
        stocks.push(json!({"id":101+i,"owner":1,"commodity":commodities[i],"quantity":quantities[i],"basis":bases[i],"source":10+i}));}
    json!({"schema":"mycity-m0-opening-v1","bootstrap":{"schema":"mycity-m0-bootstrap-v1","world":1,"branch":1,"seed":7,"laboratory_only":true},
        "sources":sources,"accounts":[{"id":1,"role":"company","cash":100000,"source":1},{"id":2,"role":"supplier","cash":0,"source":2}],"stocks":stocks,
        "continuous_supply":{"version":1,"profile":"ContinuousGensetTwinPumpV1","city_id":42,"source_id":86,"owner":1,"start_supplier":2,
            "stocks":[101,102,103,104,105,106,107],"resources":[301,302,303],"water_capacities_ml":[72000000,72000000],
            "heat_capacity_mwh":120000000,"wastewater_capacity_ml":7200000,"heat_peak_w":60000,"wastewater_peak_ml_per_second":1000}})
}
pub fn resources()->Vec<Resource>{[301,302,303].map(|id|Resource{id,capacity:1,reserved:0,occupied:0}).to_vec()}
pub fn engine(v:Value)->Engine{Engine::new(assembly::assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),resources()).unwrap()}
pub fn envelope(e:&Engine,operation:u64,action:Action)->Envelope{Envelope{world:1,branch:1,operation_id:operation,expected_revision:e.world().view().revision,actions:vec![action]}}
pub fn execute(e:&mut Engine,operation:u64,caller:Caller,action:Action)->Receipt{
    let env=envelope(e,operation,action);e.execute(caller,env).unwrap()
}
pub fn start(e:&mut Engine,operation:u64,run_id:u64,pumps:u8)->Envelope{
    let env=envelope(e,operation,Action::ContinuousSupply{command:continuous_supply::Command::Start{run_id,pumps}});
    e.execute(Caller::Account(1),env.clone()).unwrap();env
}
pub fn enable(e:&mut Engine,operation:u64){execute(e,operation,Caller::System,Action::Timeline{command:timeline::Command::Enable});}
pub fn advance(e:&mut Engine,target:u64,mut operation:u64){
    for _ in 0..256{
        execute(e,operation,Caller::System,Action::Timeline{command:timeline::Command::Advance{target,limit:1}});operation+=1;
        if e.timeline().unwrap().progress.target.is_none(){assert_eq!(e.timeline().unwrap().progress.last_complete,target);return;}
    }panic!("finite original Timeline did not finish");
}
pub fn supply(e:&Engine)->continuous_supply::View{e.world().view().continuous_supply.unwrap()}
