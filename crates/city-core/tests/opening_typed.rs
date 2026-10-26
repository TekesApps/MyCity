use mycity_core::{
    assembly,
    boot::{Command, Facility},
    opening_loader::OpeningInput,
    transaction::{Action, Caller, Engine, EngineSlot, Envelope, Resource},
    units::Money,
    INPUT_LIMIT,
};
use serde_json::{json, Value};
const BASE: &[u8] = include_bytes!("../../../tests/fixtures/opening-typed/base.json");
const BOOT: &[u8] = include_bytes!("../../../tests/fixtures/opening-typed/boot.json");
fn init(bytes: &[u8], resources: &[Resource]) -> Engine {
    let input = OpeningInput::from_json(bytes).unwrap();
    let mut slot = EngineSlot::new().unwrap();
    input.initialize_into(&mut slot, resources).unwrap();
    slot.into_engine().unwrap()
}
fn base() -> Value {
    serde_json::from_slice(BASE).unwrap()
}
fn reject(v: &Value, resources: &[Resource]) -> String {
    let bytes = serde_json::to_vec(v).unwrap();
    let mut slot = EngineSlot::new().unwrap();
    let error = slot.initialize_into(&bytes, resources).unwrap_err();
    assert!(slot.engine().is_err());
    error
}
#[test]
fn independent_capital_stock_oracle_and_original_transfer_once() {
    let mut e = init(BASE, &[]);
    let v = e.world().view();
    assert_eq!((v.scope.world(), v.scope.branch()), (9, 3));
    assert_eq!(
        v.accounts
            .iter()
            .map(|a| a.cash.value())
            .collect::<Vec<_>>(),
        [5000, 1000]
    );
    assert_eq!(v.stocks[0].quantity.value(), 3000);
    assert_eq!(v.stocks[0].basis.value(), 700);
    let from = e.world().account_ref(1).unwrap();
    let to = e.world().account_ref(2).unwrap();
    let req = Envelope {
        world: 9,
        branch: 3,
        operation_id: 77,
        expected_revision: 0,
        actions: vec![Action::Transfer {
            from,
            to,
            amount: Money::new(137).unwrap(),
        }],
    };
    e.execute(Caller::Account(1), req.clone()).unwrap();
    let hash = e.state_hash();
    e.execute(Caller::Account(1), req).unwrap();
    assert_eq!(hash, e.state_hash());
    assert_eq!(
        e.world()
            .view()
            .accounts
            .iter()
            .map(|a| a.cash.value())
            .collect::<Vec<_>>(),
        [4863, 1137]
    );
}
#[test]
fn typed_requires_complete_engine_context_and_successful_loader_is_not_reinitialized() {
    assert!(matches!(assembly::assemble(BASE),Err(e) if e.contains("requires_engine_context")));
    let mut slot = EngineSlot::new().unwrap();
    let input = OpeningInput::from_json(BASE).unwrap();
    input.initialize_into(&mut slot, &[]).unwrap();
    let hash = slot.engine().unwrap().state_hash();
    assert!(input.initialize_into(&mut slot, &[]).is_err());
    assert_eq!(hash, slot.engine().unwrap().state_hash());
    let mut v = base();
    v["resources"] = json!([{"id":701,"version":1,"capacity":2,"unit":"count"}]);
    assert!(reject(&v, &[]).contains("resource_context"));
    let resources = [Resource::new(701, 2).unwrap()];
    assert_eq!(
        init(&serde_json::to_vec(&v).unwrap(), &resources).resources()[0].capacity,
        2
    );
    assert!(reject(&v, &[Resource::new(701, 3).unwrap()]).contains("resource_context"));
    let mut occupied = resources;
    occupied[0].occupied = 1;
    assert!(reject(&v, &occupied).contains("resource_context"));
}
#[test]
fn each_reference_scope_domain_target_and_catalog_version_is_required() {
    for pointer in ["/accounts/0/source", "/stocks/0/owner", "/stocks/0/source"] {
        for (field, value) in [
            ("domain", json!("resource")),
            ("id", json!(99999)),
            ("version", json!(2)),
            ("scope", json!({"world":9,"branch":4})),
        ] {
            let mut v = base();
            v.pointer_mut(pointer).unwrap()[field] = value;
            assert!(reject(&v, &[]).contains("reference"), "{pointer}/{field}");
        }
    }
    let mut v = base();
    v["sources"][0]["kind"] = json!("inventory_capital");
    assert!(reject(&v, &[]).contains("reference"));
    let mut v = base();
    v["accounts"][1]["source"] = v["accounts"][0]["source"].clone();
    assert!(reject(&v, &[]).contains("capital_source_reused_or_mismatch"));
}
#[test]
fn absent_unknown_unit_profile_provider_clock_and_overflow_reject_without_defaults() {
    for (pointer, value) in [
        ("/schema", json!("mycity-typed-opening-v2")),
        ("/provider/kind", json!("network")),
        ("/provider/version", json!(2)),
        ("/profile/version", json!(2)),
        ("/profile/kind", json!("town")),
        ("/clock/kind", json!("development")),
        ("/sources/0/amount/unit", json!("dollar")),
        ("/stocks/0/quantity/unit", json!("millilitre")),
        ("/accounts/0/cash/value", json!(u64::MAX)),
        ("/sources/0/version", json!(0)),
    ] {
        let mut v = base();
        *v.pointer_mut(pointer).unwrap() = value;
        reject(&v, &[]);
    }
    for field in ["provider", "resources", "clock", "profile"] {
        let mut v = base();
        v.as_object_mut().unwrap().remove(field);
        reject(&v, &[]);
    }
    let mut v = base();
    v["sources"][0]["amount"]
        .as_object_mut()
        .unwrap()
        .remove("unit");
    reject(&v, &[]);
}
fn boot_apply(e: &mut Engine, command: Command) {
    let req = Envelope {
        world: 1,
        branch: 1,
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: e.world().view().revision,
        actions: vec![Action::Boot { command }],
    };
    e.execute(Caller::System, req).unwrap();
}
#[test]
fn actual_original_boot_clock_and_typed_ports_drive_completion() {
    let resources = [
        Resource::new(101, 2).unwrap(),
        Resource::new(102, 2).unwrap(),
    ];
    let mut e = init(BOOT, &resources);
    boot_apply(
        &mut e,
        Command::Start {
            facility: Facility::Pump,
            reverse_request: false,
        },
    );
    boot_apply(&mut e, Command::Advance { seconds: 30 });
    assert_eq!(e.world().view().boot.unwrap().time, 30);
    assert_eq!(
        e.world()
            .view()
            .stocks
            .iter()
            .find(|s| s.id == 3)
            .unwrap()
            .quantity
            .value(),
        0
    );
    boot_apply(&mut e, Command::Advance { seconds: 30 });
    assert_eq!(e.world().view().boot.unwrap().time, 60);
    assert_eq!(
        e.world()
            .view()
            .stocks
            .iter()
            .find(|s| s.id == 3)
            .unwrap()
            .quantity
            .value(),
        600000
    );
    for (pointer, value) in [
        ("/clock/initial", json!(1)),
        ("/clock/version", json!(2)),
        ("/clock/scope/branch", json!(2)),
        ("/profile/stocks/0/domain", json!("resource")),
        ("/profile/resources/0/id", json!(999)),
        ("/profile/limits/0/unit", json!("millilitre")),
    ] {
        let mut v: Value = serde_json::from_slice(BOOT).unwrap();
        *v.pointer_mut(pointer).unwrap() = value;
        reject(&v, &resources);
    }
}
#[test]
fn raw_size_and_duplicate_json_fields_are_rejected_and_old_v1_remains_legal() {
    let mut exact = BASE.to_vec();
    exact.resize(INPUT_LIMIT as usize, b' ');
    init(&exact, &[]);
    exact.push(b' ');
    assert!(OpeningInput::from_json(&exact).is_err());
    let duplicate = String::from_utf8(BASE.to_vec()).unwrap().replacen(
        "\"provider\":",
        "\"provider\":{\"kind\":\"embedded_laboratory\",\"version\":1},\"provider\":",
        1,
    );
    let mut slot = EngineSlot::new().unwrap();
    assert!(slot.initialize_into(duplicate.as_bytes(), &[]).is_err());
    assert!(slot.engine().is_err());
    assert!(assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).is_ok());
}

#[test]
fn maximum_account_stock_and_resource_tables_accept_n_and_refuse_n_plus_one() {
    let mut accounts = base();
    accounts["accounts"] = json!([]);
    accounts["sources"] = json!([{"id":12,"version":1,"kind":"inventory_capital","amount":{"value":700,"unit":"cent"}}]);
    for id in 1..=64 {
        let mut a = base()["accounts"][1].clone();
        a["id"] = json!(id);
        a["cash"]["value"] = json!(0);
        a["source"]["id"] = json!(1000 + id);
        accounts["accounts"].as_array_mut().unwrap().push(a);
        accounts["sources"].as_array_mut().unwrap().push(json!({"id":1000+id,"version":1,"kind":"cash_capital","amount":{"value":0,"unit":"cent"}}));
    }
    let bytes = serde_json::to_vec(&accounts).unwrap();
    assert!(bytes.len() < INPUT_LIMIT as usize);
    assert_eq!(init(&bytes, &[]).world().view().accounts.len(), 64);
    let extra = accounts["accounts"][0].clone();
    accounts["accounts"].as_array_mut().unwrap().push(extra);
    assert!(reject(&accounts, &[]).contains("opening_count_limit"));
    let mut stocks = base();
    stocks["stocks"] = json!([]);
    stocks["sources"].as_array_mut().unwrap().pop();
    for id in 1..=128 {
        let mut s = base()["stocks"][0].clone();
        s["id"] = json!(id);
        s["quantity"]["value"] = json!(1);
        s["basis"]["value"] = json!(0);
        s["source"]["id"] = json!(2000 + id);
        stocks["stocks"].as_array_mut().unwrap().push(s);
        stocks["sources"].as_array_mut().unwrap().push(json!({"id":2000+id,"version":1,"kind":"inventory_capital","amount":{"value":0,"unit":"cent"}}));
    }
    let bytes = serde_json::to_vec(&stocks).unwrap();
    assert!(bytes.len() < INPUT_LIMIT as usize);
    assert_eq!(init(&bytes, &[]).world().view().stocks.len(), 128);
    let extra = stocks["stocks"][0].clone();
    stocks["stocks"].as_array_mut().unwrap().push(extra);
    assert!(reject(&stocks, &[]).contains("opening_count_limit"));
    let mut resources = base();
    resources["resources"] = json!((1..=16)
        .map(|id| json!({"id":id,"version":1,"capacity":2,"unit":"count"}))
        .collect::<Vec<_>>());
    let actual = (1..=16)
        .map(|id| Resource::new(id, 2).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        init(&serde_json::to_vec(&resources).unwrap(), &actual)
            .resources()
            .len(),
        16
    );
    resources["resources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":17,"version":1,"capacity":2,"unit":"count"}));
    assert!(reject(&resources, &actual).contains("opening_count_limit"));
}

#[test]
fn original_sum_overflow_and_real_reader_capacity_are_not_silent_conversions() {
    let mut v = base();
    for index in [0, 1] {
        v["sources"][index]["amount"]["value"] = json!(i64::MAX);
        v["accounts"][index]["cash"]["value"] = json!(i64::MAX);
    }
    assert!(reject(&v, &[]).contains("opening_cash_overflow"));
    let mut exact = BASE.to_vec();
    exact.resize(INPUT_LIMIT as usize, b' ');
    let input = OpeningInput::read_named_json(exact.as_slice(), "actual-reader-capacity").unwrap();
    let owned = input.source().owned_capacity_bytes();
    assert!(
        owned
            >= input.source().bytes().len()
                + std::mem::size_of::<mycity_core::opening_loader::OpeningSource>()
    );
    println!(
        "input_bytes={}; actual_source_owned_capacity={owned}; counted_outside_borrowed_decode",
        input.source().bytes().len()
    );
    let mut slot = EngineSlot::new().unwrap();
    input.initialize_into(&mut slot, &[]).unwrap();
}
