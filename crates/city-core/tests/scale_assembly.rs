//! Full frozen table counts; this is model assembly/correctness, not a timed gate.
use mycity_core::{
    assembly::assemble,
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::Quantity,
};
use serde_json::{json, Value};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-t13a-full-opening-candidate.json");
fn engine() -> Engine {
    Engine::new(assemble(INPUT).unwrap(), resources()).unwrap()
}
fn resources() -> Vec<Resource> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
        (203, 1000),
        (204, 1000),
    ]
    .into_iter()
    .map(|(id, cap)| Resource::new(id, cap).unwrap())
    .collect()
}
fn envelope(e: &Engine, action: Action) -> Envelope {
    let v = e.world().view();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: v.revision,
        actions: vec![action],
    }
}
fn movement(e: &Engine, from: u32, to: u32) -> Action {
    Action::MoveStock {
        from: e.world().stock_ref(from).unwrap(),
        to: e.world().stock_ref(to).unwrap(),
        quantity: Quantity::new(500).unwrap(),
    }
}
#[test]
fn full_world_assembles_and_extra_nonzero_stock_is_outside_managed_conservation_domains() {
    let mut e = engine();
    let view = e.world().view();
    assert_eq!((view.accounts.len(), view.stocks.len()), (64, 128));
    assert_eq!(view.region.unwrap().cities.iter().flatten().count(), 8);
    assert_eq!(view.exchange.unwrap().maps.len(), 8);
    assert_eq!(view.boot.unwrap().initial_cash.value(), 3000000);
    for (from, to) in [(15, 7), (7, 15), (15, 1), (1, 15)] {
        let before = e.state_hash();
        let env = envelope(&e, movement(&e, from, to));
        assert_eq!(e.execute(Caller::System, env), Err(TxError::Unauthorized));
        assert_eq!(e.state_hash(), before);
    }
    let env = envelope(&e, movement(&e, 15, 16));
    e.execute(Caller::System, env).unwrap();
    let view = e.world().view();
    let source = &view.stocks[14];
    let target = &view.stocks[15];
    assert_eq!(
        (
            source.quantity.value(),
            source.basis.value(),
            source.offset.value()
        ),
        (500, 150, 500)
    );
    assert_eq!(
        (
            target.quantity.value(),
            target.basis.value(),
            target.origin_quantity.value(),
            target.origin_basis.value()
        ),
        (500, 150, 1000, 300)
    );
    assert_eq!(source.lot_id, target.lot_id);
    let restored = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
    assert_eq!(restored.world().view(), e.world().view());
    assert_eq!(restored.state_hash(), e.state_hash());
}
#[test]
fn scaled_shape_is_bounded_and_invalid_structure_rejects_both_initialization_and_restore() {
    let base: Value = serde_json::from_slice(INPUT).unwrap();
    for mutate in [
        |v: &mut Value| {
            v["exchange"]["maps"][1]["city"] = json!(1);
        },
        |v: &mut Value| {
            v["exchange"]["maps"][3]["origin_x_cm"] = json!(1);
        },
        |v: &mut Value| {
            v["exchange"]["maps"][3]["city"] = json!(99);
        },
        |v: &mut Value| {
            v["exchange"]["maps"]
                .as_array_mut()
                .unwrap()
                .push(json!({"city":9,"origin_x_cm":819200}));
        },
        |v: &mut Value| {
            v["boot"]["profile"] = json!("pboot-cold-micro-v1");
        },
    ] {
        let mut v = base.clone();
        mutate(&mut v);
        assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    let checkpoint: Value = serde_json::from_slice(&engine().export_checkpoint().unwrap()).unwrap();
    for mutate in [
        |v: &mut Value| {
            v["world"]["exchange"]["maps"][3]["origin_x_cm"] = json!(1);
        },
        |v: &mut Value| {
            v["world"]["exchange"]["maps"][3]["city"] = json!(99);
        },
        |v: &mut Value| {
            v["world"]["exchange"]["stocks"][0]["city"] = json!(2);
        },
        |v: &mut Value| {
            v["world"]["exchange"]["quotes"][0]["spec"]["source"] = json!(15);
        },
        |v: &mut Value| {
            v["world"]["boot"]["stocks"][0] = json!(15);
        },
        |v: &mut Value| {
            v["world"]["boot"]["profile"] = json!("pboot-cold-micro-v1");
        },
    ] {
        let mut v = checkpoint.clone();
        mutate(&mut v);
        assert!(Engine::restore_checkpoint(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
#[test]
fn legacy_three_city_wire_input_stays_valid_and_does_not_accept_expanded_tables() {
    let old = include_bytes!("../../../fixtures/m0-exchange-opening.json");
    let e = Engine::new(assemble(old).unwrap(), resources()).unwrap();
    assert_eq!(e.world().view().exchange.unwrap().maps.len(), 3);
    assert_eq!(
        Engine::restore_checkpoint(&e.export_checkpoint().unwrap())
            .unwrap()
            .world()
            .view(),
        e.world().view()
    );
    let mut expanded: Value = serde_json::from_slice(INPUT).unwrap();
    expanded["exchange"]["profile"] = json!("regional-fulfilment-v1");
    assert!(assemble(&serde_json::to_vec(&expanded).unwrap()).is_err());
}
fn apply(e: &mut Engine, action: Action) {
    e.execute(Caller::System, envelope(e, action)).unwrap();
}
#[test]
fn scale_receiving_cost_pool_keeps_two_purchase_prices_and_rolls_back_failed_commit() {
    use mycity_core::{
        boot::{Command as B, Facility},
        exchange::Command as X,
        inbox::Command as I,
    };
    let mut input: Value = serde_json::from_slice(INPUT).unwrap();
    input["exchange"]["quotes"][4]["unit_price"] = json!(3000);
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&input).unwrap()).unwrap(),
        resources(),
    )
    .unwrap();
    apply(&mut e, Action::Inbox { command: I::Enable });
    apply(
        &mut e,
        Action::Boot {
            command: B::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    );
    apply(&mut e, Action::RegionAdvance { seconds: 60 });
    apply(
        &mut e,
        Action::Boot {
            command: B::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    );
    apply(
        &mut e,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    );
    apply(&mut e, Action::RegionAdvance { seconds: 120 });
    apply(
        &mut e,
        Action::Inbox {
            command: I::Settle { sequence: 1 },
        },
    );
    apply(
        &mut e,
        Action::Inbox {
            command: I::Archive { sequence: 1 },
        },
    );
    e = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
    let first = e.world().view().exchange.unwrap().quotes[0];
    apply(
        &mut e,
        Action::Exchange {
            command: X::Dispatch { quote: 5 },
        },
    );
    let env = envelope(&e, Action::RegionAdvance { seconds: 120 });
    let before = e.export_checkpoint().unwrap();
    let outcome = e.execute_recorded(Caller::System, env.clone(), |_, _| {
        Err::<(), _>("simulated_persistence_failure")
    });
    assert_eq!(outcome, Err("simulated_persistence_failure"));
    assert_eq!(e.export_checkpoint().unwrap(), before);
    e.execute(Caller::System, env.clone()).unwrap();
    let view = e.world().view();
    let dst = &view.stocks[7];
    assert_eq!((dst.quantity.value(), dst.basis.value()), (2000, 5040));
    assert_eq!(
        (
            dst.origin_quantity.value(),
            dst.origin_basis.value(),
            dst.offset.value()
        ),
        (2000, 5040, 0)
    );
    assert_eq!(view.exchange.unwrap().quotes[0], first);
    assert_eq!(
        e.execute(Caller::System, env).unwrap().revision,
        view.revision
    );
    let restored = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
    assert_eq!(restored.world().view(), view);
}
#[test]
fn scale_receiving_pool_capacity_rejects_before_any_second_shipment_side_effect() {
    use mycity_core::exchange::{Command as X, Error as XE};
    let mut input: Value = serde_json::from_slice(INPUT).unwrap();
    input["exchange"]["stocks"][7]["limit"] = json!(1500);
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&input).unwrap()).unwrap(),
        resources(),
    )
    .unwrap();
    use mycity_core::boot::{Command as B, Facility};
    apply(
        &mut e,
        Action::Boot {
            command: B::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    );
    apply(&mut e, Action::RegionAdvance { seconds: 60 });
    apply(
        &mut e,
        Action::Boot {
            command: B::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    );
    apply(
        &mut e,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    );
    apply(&mut e, Action::RegionAdvance { seconds: 120 });
    let before = e.world().view();
    let env = envelope(
        &e,
        Action::Exchange {
            command: X::Dispatch { quote: 5 },
        },
    );
    assert_eq!(
        e.execute(Caller::System, env),
        Err(TxError::Exchange(XE::Storage))
    );
    assert_eq!(e.world().view(), before);
}
#[test]
fn scale_air_rejects_a_freight_source_outside_finite_field_but_not_unemitting_frames() {
    use mycity_core::{background::Command as B, timeline::Command as T};
    let mut input: Value = serde_json::from_slice(INPUT).unwrap();
    // City 1 is the actual freight source; keep the metric layout valid but outside the air extent.
    for frame in input["exchange"]["maps"].as_array_mut().unwrap() {
        frame["origin_x_cm"] = json!(frame["origin_x_cm"].as_i64().unwrap() + 819200);
    }
    for edge in input["exchange"]["edges"].as_array_mut().unwrap() {
        for endpoint in ["a_cm", "b_cm"] {
            edge[endpoint][0] = json!(edge[endpoint][0].as_i64().unwrap() + 819200);
        }
    }
    let mut e = Engine::new(
        assemble(&serde_json::to_vec(&input).unwrap()).unwrap(),
        resources(),
    )
    .unwrap();
    apply(&mut e, Action::Timeline { command: T::Enable });
    let env = envelope(&e, Action::Background { command: B::Enable });
    assert_eq!(
        e.execute(Caller::System, env),
        Err(TxError::InvalidOperation)
    );
    assert!(e.background().is_none());
    let mut valid = engine();
    apply(&mut valid, Action::Timeline { command: T::Enable });
    apply(&mut valid, Action::Background { command: B::Enable });
}
