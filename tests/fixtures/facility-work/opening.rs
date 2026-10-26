//! Dedicated original-source labor fixture; laboratory prices, not certification/performance.
use mycity_core::{
    arena::Scope,
    assembly, facility_work as w, member_authority,
    member_qualification::{Background, MemberRange, Skill},
    qualification_authority as q, qualification_state as f,
    transaction::{Engine, Resource},
    units::GameTime,
};
use serde_json::{json, Value};
#[path = "../continuous-supply/opening.rs"]
mod supply;
fn id(n: u8) -> [u8; 16] {
    let mut a = [0; 16];
    a[15] = n;
    a
}
pub fn input() -> Value {
    let mut raw: Value = serde_json::from_slice(include_bytes!(
        "../../../crates/city-storage/tests/fixtures/education-store/free-offline-v1.json"
    ))
    .unwrap();
    let origin = serde_json::from_value(raw["education"]["population_origin"].clone()).unwrap();
    raw.as_object_mut().unwrap().remove("education");
    let base = supply::input();
    raw["continuous_supply"] = base["continuous_supply"].clone();
    raw["continuous_supply"]["owner"] = json!(3);
    raw["continuous_supply"]["start_supplier"] = json!(4);
    for (i, s) in base["stocks"].as_array().unwrap().iter().enumerate() {
        let mut s = s.clone();
        s["owner"] = json!(3);
        s["source"] = json!(110 + i);
        raw["stocks"].as_array_mut().unwrap().push(s);
        raw["sources"].as_array_mut().unwrap().push(
            json!({"id":110+i,"kind":"inventory_capital","amount":base["stocks"][i]["basis"]}),
        );
    }
    let scope = Scope::new(1, 1).unwrap();
    let until = GameTime::new(10000).unwrap();
    let zero = GameTime::new(0).unwrap();
    let facts = f::Opening {
        version: 1,
        profile: f::Profile::ImmutableGameOpeningV1,
        scope,
        member_version: 1,
        population_origin: origin,
        total_people: 10,
        authority: f::OpeningAuthority {
            id: id(1),
            scope,
            population_origin: origin,
            member_version: 1,
            profile: f::Profile::ImmutableGameOpeningV1,
            issued_at: zero,
            valid_until: until,
            permitted_results: 0,
        },
        issuers: [None; 4],
        rules: [None; 4],
        course_results: [None; 4],
        baseline_sources: std::array::from_fn(|i| {
            if i == 0 {
                Some(f::BaselineSourceRegistration {
                    id: id(2),
                    authority_id: id(1),
                    scope,
                    population_origin: origin,
                    member_version: 1,
                    valid_from: zero,
                    valid_until: until,
                })
            } else {
                None
            }
        }),
        baselines: std::array::from_fn(|i| {
            if i == 0 {
                Some(f::BaselineFact {
                    members: MemberRange { start: 0, end: 10 },
                    source_id: id(2),
                    base_known: true,
                    base_mask: 2,
                    background: Some(Background::General),
                    skill: Some(Skill::Technical),
                })
            } else {
                None
            }
        }),
    };
    raw["qualification"] = serde_json::to_value(q::Opening {
        protocol_version: 3,
        facts,
        policies: [None; 4],
    })
    .unwrap();
    raw["member_activity"] = serde_json::to_value(member_authority::Registration {
        version: 1,
        scope,
        population_origin: origin,
        total_people: 10,
        epoch: 1,
    })
    .unwrap();
    let offer = &mut raw["venue"]["offers"][0];
    offer["partition"]["member_range_end"] = json!(1);
    offer["population"] = json!({"enrollment":0,"learners":0,"staff":1,"teachers":0,"visitors":0});
    offer["window"]["end"] = json!(7200);
    raw["facility_work"] = serde_json::to_value(mycity_core::facility_authority::Opening {
        authority_version: 2,
        work: w::Opening {
            binding: w::Binding {
                scope,
                population_origin: origin,
                member_version: 1,
                people: 10,
                epoch: 1,
            },
            contracts: [
                Some(w::Contract {
                    id: 1,
                    version: 1,
                    device_id: 301,
                    role: w::Role::Electric,
                    members: MemberRange { start: 0, end: 1 },
                    employer_account: 3,
                    employee_account: 2,
                    venue_use_id: 1,
                    hourly_cents_per_member: 3601,
                    start: 0,
                    end: 7200,
                }),
                None,
                None,
                None,
            ],
        },
    })
    .unwrap();
    raw
}
pub fn resources() -> Vec<Resource> {
    [301, 302, 303, 501, 502]
        .into_iter()
        .map(|id| Resource::new(id, if id >= 501 { 4 } else { 1 }).unwrap())
        .collect()
}
pub fn engine(v: Value) -> Engine {
    Engine::new(
        assembly::assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),
        resources(),
    )
    .unwrap()
}
