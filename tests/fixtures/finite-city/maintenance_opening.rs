//! #36 finite maintenance profile: one newly completed project-7 support asset.
//! This is an explicit input variant; the frozen asset/M0 fixtures are untouched.
//! The qualification, household employment, stock, and cash sources below are
//! synthetic game-opening facts, not a real credential, payroll, or worksite.
#[path="asset_opening.rs"] mod base;
use mycity_core::{transaction::Resource, timeline::DAY};
use serde_json::{json, Value};

const ELECTRIC: u8 = 1 << 1;
const MAINTENANCE_END: u64 = 30 * DAY + 20 * 3600;

fn id(value: u8) -> [u8; 16] {
    let mut id = [0; 16];
    id[15] = value;
    id
}

/// A separate finite opening for root's future maintenance wiring. It does not
/// contain any maintenance ingress field; that contract is still being fixed.
pub fn base_input() -> Value {
    let mut v = base::input();

    // Project 7 contributes two units only after its real build is inspected.
    // It cannot withdraw an older shared contribution that was never opened here.
    v["construction"]["operating_base_capacity"] = json!(0);
    v["construction"]["operating_units"] = json!(2);

    // The original 10-person household is still household 7 / account 2. In this
    // new profile only members [0,4) receive the explicit Electric baseline fact.
    let facts = &mut v["qualification"]["facts"];
    facts["authority"]["valid_until"] = json!(MAINTENANCE_END);
    facts["baseline_sources"][0]["valid_until"] = json!(MAINTENANCE_END);
    facts["baselines"][0] = json!({
        "members": {"start": 0, "end": 4},
        "source_id": id(2),
        "base_known": true,
        "base_mask": 33 | ELECTRIC,
        "background": "General",
        "skill": "Technical"
    });
    facts["baselines"][1] = json!({
        "members": {"start": 4, "end": 10},
        "source_id": id(2),
        "base_known": true,
        "base_mask": 33,
        "background": "General",
        "skill": "Technical"
    });
    // Preserve the existing Driver/Police baseline: the common city's service
    // remains a real consumer. No course issuer is needed for a declared baseline.
    v["boot"]["profile"] = json!(mycity_core::boot::LOCAL_EXACT_PROFILE);
    v
}

/// Follows the neighboring finite-city fixture convention.
pub fn input() -> Value {
    base_input()
}

/// Retains every base finite resource except the old opening contribution of 2
/// on resource 81; project 7's two new units are added by completed construction.
pub fn resources() -> Vec<Resource> {
    let mut resources = base::resources();
    for resource in &mut resources {
        if resource.id == 81 {
            resource.capacity = 0;
        }
    }
    resources
}

/// Explicit initial four-person servicing bay, distinct from the asset's output
/// capacity. This new game source is not the old production offer or a predicted
/// arrival. Actual Accept/Enter must still be committed by the original host.
pub fn registered_input()->Value {
    let mut v=base_input();let binding=&v["member_activity"];
    v["facility_work"]=json!({"authority_version":3,"work":{"binding":{
        "scope":binding["scope"],"population_origin":binding["population_origin"],
        "member_version":1,"people":10,"epoch":1},"contracts":[{
        "id":22,"version":2,"device_id":7,"role":"ElectricalMaintenance",
        "members":{"start":0,"end":4},"employer_account":7,"employee_account":2,
        "venue_use_id":1591,"hourly_cents_per_member":2000,
        "start":30*DAY+17*3600,"end":MAINTENANCE_END},null,null,null]},
        "maintenance":{"version":1,"project":7,"component":1,"contract_id":22,"material_stock":15}});
    v["venue"]=json!({"profile":"managed_people_v1","population_source":"housing_genesis_v1",
        "venues":[{"venue_id":1591,"resource_id":1592,"owner":7,"operator":7,
            "purpose":159,"permit_version":1,"aliases":[null,null,null,null]}],
        "limits":[{"resource_id":1592,"maximum_people":4}],
        "offers":[{"use_id":1591,"accepted_version":1,"venue_id":1591,"operator":7,
            "group_id":7,"purpose":159,"window":{"start":30*DAY+17*3600,"end":MAINTENANCE_END},
            "population":{"staff":4,"visitors":0,"learners":0,"teachers":0,"enrollment":0},
            "partition":{"group_id":7,"member_range_start":0,"member_range_end":4,"partition_source_id":1,"partition_version":1},
            "source_refs":[{"kind":"genesis","scope":{"world":1,"branch":1},"source_id":7001,
                "revision":0,"operation_id":0,"event_id":0},null,null,null]}]});v
}
pub fn registered_resources()->Vec<Resource>{let mut r=resources();r.push(Resource::new(1592,4).unwrap());r}
