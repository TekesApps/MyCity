use mycity_core::{
    assembly::assemble,
    boot::{Command as B, Facility},
    exchange::{Command as X, Error as XE},
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError},
    units::{Money, Quantity},
};
use serde_json::{json, Value};
const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-exchange-opening.json");
fn resources() -> Vec<Resource> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
    ]
    .into_iter()
    .map(|(id, cap)| Resource::new(id, cap).unwrap())
    .collect()
}
fn engine() -> Engine {
    Engine::new(assemble(INPUT).unwrap(), resources()).unwrap()
}
fn variant(f: impl FnOnce(&mut Value)) -> Engine {
    let mut v: Value = serde_json::from_slice(INPUT).unwrap();
    f(&mut v);
    Engine::new(
        assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),
        resources(),
    )
    .unwrap()
}
fn env(e: &Engine, actions: Vec<Action>) -> Envelope {
    let v = e.world().view();
    Envelope {
        world: 1,
        branch: 1,
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: v.revision,
        actions,
    }
}
fn apply(e: &mut Engine, a: Action) {
    let c = env(e, vec![a]);
    e.execute(Caller::System, c).unwrap();
    check(e);
}
fn x(c: X) -> Action {
    Action::Exchange { command: c }
}
fn start(facility: Facility) -> Action {
    Action::Boot {
        command: B::Start {
            facility,
            reverse_request: true,
        },
    }
}
fn dispatch(id: u32) -> Action {
    x(X::Dispatch { quote: id })
}
fn advance(e: &mut Engine, t: u32) {
    apply(e, Action::RegionAdvance { seconds: t })
}
fn reject(e: &mut Engine, a: Action) -> TxError {
    let c = env(e, vec![a]);
    let before = e.state_hash();
    let err = e.execute(Caller::System, c).unwrap_err();
    assert_eq!(e.state_hash(), before);
    check(e);
    err
}
fn stock(e: &Engine, id: u32) -> (i64, i64) {
    let w = e.world().view();
    let s = w.stocks.iter().find(|s| s.id == id).unwrap();
    (s.quantity.value(), s.basis.value())
}
fn prepare_power(e: &mut Engine) {
    apply(e, start(Facility::Pump));
    advance(e, 60);
    apply(e, start(Facility::Generator));
    advance(e, 60);
}
fn check(e: &Engine) {
    let w = e.world().view();
    let r = w.region.unwrap();
    let b = w.boot.unwrap();
    let x = w.exchange.unwrap();
    assert_eq!(b.time, r.interaction_time);
    assert_eq!(
        w.accounts.iter().map(|a| a.cash.value()).sum::<i64>(),
        b.initial_cash.value()
    );
    let mut energy = 0_i128;
    let mut water = 0_i128;
    let mut raw = 0_i128;
    let mut basis = i128::from(b.failed_expense.value() + x.loss_expense.value());
    for s in &w.stocks {
        basis += i128::from(s.basis.value());
        match s.commodity {
            mycity_core::assembly::Commodity::Fuel => {
                energy += i128::from(s.quantity.value()) * 1000
            }
            mycity_core::assembly::Commodity::Electricity
            | mycity_core::assembly::Commodity::Heat => energy += i128::from(s.quantity.value()),
            mycity_core::assembly::Commodity::Water
            | mycity_core::assembly::Commodity::RawWater
            | mycity_core::assembly::Commodity::WasteWater => {
                water += i128::from(s.quantity.value())
            }
            mycity_core::assembly::Commodity::Raw => raw += i128::from(s.quantity.value()),
            _ => {}
        }
    }
    for j in b.jobs {
        energy += i128::from(j.inputs[0].value()) * 1000 + i128::from(j.inputs[1].value());
        water += i128::from(j.inputs[2].value() + j.inputs[3].value());
        basis += i128::from(j.cost.value());
    }
    for s in x.shipments.iter().flatten() {
        basis += i128::from(s.fragment.basis.value() + s.freight.value());
        match x.quotes[s.quote_index].spec.kind {
            mycity_core::exchange::Kind::Freight => raw += i128::from(s.fragment.quantity.value()),
            mycity_core::exchange::Kind::Electricity => {
                energy += i128::from(s.fragment.quantity.value())
            }
            mycity_core::exchange::Kind::Water => water += i128::from(s.fragment.quantity.value()),
        }
    }
    energy += i128::from(x.energy_loss.value());
    water += i128::from(x.water_loss.value());
    assert_eq!(energy, 5100000);
    assert_eq!(water, 6000000);
    assert_eq!(raw, 5000);
    assert_eq!(
        basis,
        i128::from(
            x.initial_basis.value() + b.fees.value() + x.sales.value() + x.freight_income.value()
                - x.cogs.value()
        )
    );
    for res in e.resources() {
        assert!(res.reserved + res.occupied <= res.capacity)
    }
    assert_eq!(
        r.cities
            .iter()
            .flatten()
            .map(|c| u64::from(c.population))
            .sum::<u64>()
            + r.transits
                .iter()
                .flatten()
                .map(|t| u64::from(t.actual))
                .sum::<u64>(),
        4400
    );
}
#[test]
fn accepted_boot_finishes_while_closed_then_real_multihop_goods_and_utilities_settle_once() {
    let mut e = engine();
    let old = e
        .world()
        .view()
        .region
        .unwrap()
        .city(1)
        .unwrap()
        .development_elapsed;
    apply(&mut e, start(Facility::Pump));
    apply(&mut e, Action::RegionActivate { city: 2 });
    advance(&mut e, 60);
    assert_eq!(stock(&e, 3).0, 600000);
    assert_eq!(
        e.world()
            .view()
            .region
            .unwrap()
            .city(1)
            .unwrap()
            .development_elapsed,
        old
    );
    assert_eq!(
        reject(&mut e, start(Facility::Generator)),
        TxError::Exchange(XE::Clock)
    );
    apply(&mut e, Action::RegionActivate { city: 1 });
    apply(&mut e, start(Facility::Generator));
    apply(&mut e, Action::RegionActivate { city: 2 });
    advance(&mut e, 60);
    apply(&mut e, dispatch(1));
    apply(&mut e, dispatch(6));
    apply(&mut e, dispatch(7));
    assert_eq!(stock(&e, 8).0, 0);
    assert_eq!(stock(&e, 13).0, 0);
    assert_eq!(stock(&e, 7).0, 4000);
    advance(&mut e, 62);
    assert_eq!(stock(&e, 13), (98010, 118));
    assert_eq!(stock(&e, 8).0, 0);
    advance(&mut e, 58);
    assert_eq!(stock(&e, 8), (1000, 2020));
    advance(&mut e, 964);
    assert_eq!(stock(&e, 14), (98010, 216));
    let v = e.world().view();
    let exchange = v.exchange.unwrap();
    assert_eq!(exchange.energy_loss.value(), 1990);
    assert_eq!(exchange.water_loss.value(), 1990);
    assert!(exchange.shipments.iter().all(Option::is_none));
    assert!(v.accounts.iter().all(|a| a.protected == Money::ZERO));
    let cash = v.accounts;
    let stocks = v.stocks;
    for city in [1, 3, 2, 1, 2] {
        apply(&mut e, Action::RegionActivate { city })
    }
    assert_eq!(e.world().view().accounts, cash);
    assert_eq!(e.world().view().stocks, stocks);
}
#[test]
fn population_freight_boot_and_utility_compete_in_actual_shared_pools() {
    let mut e = engine();
    apply(
        &mut e,
        Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 2000,
            tolerance: 2,
        },
    );
    assert_eq!(reject(&mut e, dispatch(1)), TxError::Capacity);
    assert_eq!(
        e.resources().iter().find(|r| r.id == 201).unwrap().occupied,
        1998
    );
    advance(&mut e, 120);
    apply(&mut e, dispatch(1));
    assert_eq!(
        reject(
            &mut e,
            Action::RegionMigrate {
                from: 1,
                to: 2,
                requested: 1,
                tolerance: 0
            }
        ),
        TxError::Region(mycity_core::region::Error::Shortfall)
    );
    let mut e = engine();
    prepare_power(&mut e);
    apply(&mut e, start(Facility::Pump));
    assert_eq!(reject(&mut e, dispatch(6)), TxError::Capacity);
    advance(&mut e, 60);
    apply(&mut e, dispatch(6));
    assert_eq!(reject(&mut e, start(Facility::Pump)), TxError::Capacity);
    advance(&mut e, 62);
    apply(&mut e, start(Facility::Pump));
}
#[test]
fn broken_link_pauses_actual_goods_and_people_without_erasing_reservations_or_fees() {
    let mut e = engine();
    apply(&mut e, dispatch(1));
    advance(&mut e, 30);
    apply(
        &mut e,
        x(X::SetLink {
            edge: 2,
            open: false,
        }),
    );
    advance(&mut e, 200);
    assert_eq!(stock(&e, 8).0, 0);
    let xstate = e.world().view().exchange.unwrap();
    assert_eq!(
        xstate.shipments.iter().flatten().next().unwrap().remaining,
        90
    );
    assert_eq!(xstate.freight_income.value(), 20);
    assert_eq!(
        reject(&mut e, x(X::Cancel { quote: 1 })),
        TxError::Exchange(XE::State)
    );
    apply(
        &mut e,
        x(X::SetLink {
            edge: 2,
            open: true,
        }),
    );
    advance(&mut e, 89);
    assert_eq!(stock(&e, 8).0, 0);
    advance(&mut e, 1);
    assert_eq!(stock(&e, 8).0, 1000);
    let mut e = engine();
    apply(
        &mut e,
        Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 2000,
            tolerance: 2,
        },
    );
    advance(&mut e, 30);
    apply(
        &mut e,
        x(X::SetLink {
            edge: 1,
            open: false,
        }),
    );
    advance(&mut e, 300);
    assert_eq!(
        e.world().view().region.unwrap().city(2).unwrap().population,
        100
    );
    apply(
        &mut e,
        x(X::SetLink {
            edge: 1,
            open: true,
        }),
    );
    advance(&mut e, 90);
    assert_eq!(
        e.world().view().region.unwrap().city(2).unwrap().population,
        2098
    );
}
#[test]
fn full_shipments_expiry_distance_storage_loss_and_insufficient_funds_reject_atomically() {
    let mut e = engine();
    for i in 1..=4 {
        apply(&mut e, dispatch(i))
    }
    assert_eq!(reject(&mut e, dispatch(5)), TxError::Exchange(XE::Slots));
    advance(&mut e, 120);
    assert_eq!(reject(&mut e, dispatch(5)), TxError::Exchange(XE::Expired));
    assert_eq!(stock(&e, 7).0, 1000);
    let mut e = engine();
    prepare_power(&mut e);
    assert_eq!(reject(&mut e, dispatch(8)), TxError::Exchange(XE::Distance));
    let mut e = variant(|v| {
        v["accounts"][1]["cash"] = json!(1);
        v["sources"][1]["amount"] = json!(1)
    });
    assert_eq!(reject(&mut e, dispatch(1)), TxError::InsufficientFunds);
    let mut e = variant(|v| v["exchange"]["stocks"][7]["limit"] = json!(999));
    assert_eq!(reject(&mut e, dispatch(1)), TxError::Exchange(XE::Storage));
    let mut e = variant(|v| v["exchange"]["energy_loss_limit"] = json!(1));
    prepare_power(&mut e);
    assert_eq!(reject(&mut e, dispatch(6)), TxError::Exchange(XE::Storage));
}
#[test]
fn protections_lease_ownership_clock_scope_hook_and_late_error_are_real_gates() {
    let mut e = engine();
    let before = e.state_hash();
    let c = env(&e, vec![dispatch(1)]);
    assert_eq!(e.execute(Caller::Account(2), c), Err(TxError::Unauthorized));
    let c = env(&e, vec![dispatch(1)]);
    assert_eq!(
        e.execute_recorded(Caller::System, c, |_, _| Err("disk")),
        Err("disk")
    );
    assert_eq!(e.state_hash(), before);
    let c = env(&e, vec![dispatch(1), dispatch(1)]);
    assert!(e.execute(Caller::System, c).is_err());
    assert_eq!(e.state_hash(), before);
    apply(&mut e, dispatch(1));
    let ship = e.world().view().exchange.unwrap().shipments[0].unwrap();
    let token = mycity_core::transaction::CapacityToken {
        world: 1,
        branch: 1,
        number: ship.tokens[0].unwrap(),
    };
    assert_eq!(
        reject(&mut e, Action::ReleaseCapacity { token }),
        TxError::Exchange(XE::Invariant)
    );
    let buyer = e.world().account_ref(2).unwrap();
    reject(
        &mut e,
        Action::ReleaseProtection {
            account: buyer,
            amount: Money::new(2000).unwrap(),
        },
    );
    assert_eq!(
        reject(
            &mut e,
            Action::Boot {
                command: B::Advance { seconds: 1 }
            }
        ),
        TxError::Exchange(XE::Clock)
    );
    let from = e.world().stock_ref(7).unwrap();
    let to = e.world().stock_ref(8).unwrap();
    reject(
        &mut e,
        Action::MoveStock {
            from,
            to,
            quantity: Quantity::new(1000).unwrap(),
        },
    );
    advance(&mut e, 120);
}
#[test]
fn expiry_keeps_old_debt_and_already_accepted_delivery_while_split_steps_keep_business_results() {
    let mut a = engine();
    let mut b = engine();
    prepare_power(&mut a);
    prepare_power(&mut b);
    apply(&mut a, dispatch(6));
    apply(&mut b, dispatch(6));
    advance(&mut a, 62);
    advance(&mut b, 31);
    advance(&mut b, 31);
    assert_eq!(a.world().view().accounts, b.world().view().accounts);
    assert_eq!(a.world().view().stocks, b.world().view().stocks);
    assert_eq!(a.world().view().exchange, b.world().view().exchange);
    let mut e = variant(|v| v["region"]["obligations"][0]["amount"] = json!(700000));
    apply(&mut e, dispatch(1));
    advance(&mut e, 120);
    assert_eq!(stock(&e, 8).0, 1000);
    let r = e.world().view().region.unwrap();
    assert_eq!(r.obligations[0].unwrap().arrears.value(), 1500000);
    assert_eq!(r.obligations[0].unwrap().next_due, None);
    apply(&mut e, Action::RegionActivate { city: 1 });
    advance(&mut e, 120);
    assert_eq!(
        e.world().view().region.unwrap().obligations[0]
            .unwrap()
            .arrears
            .value(),
        1500000
    );
}
#[test]
fn malformed_geometry_custody_type_and_missing_pool_are_rejected() {
    for i in 0..4 {
        let mut v: Value = serde_json::from_slice(INPUT).unwrap();
        match i {
            0 => v["exchange"]["edges"][0]["a_cm"][0] = json!(1),
            1 => v["exchange"]["stocks"][7]["city"] = json!(1),
            2 => v["exchange"]["quotes"][0]["edges"][0] = json!(3),
            _ => v["exchange"]["maps"][1]["origin_x_cm"] = json!(0),
        }
        assert!(assemble(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    assert!(Engine::new(assemble(INPUT).unwrap(), vec![]).is_err());
}

#[test]
fn resource_type_swaps_cannot_bypass_boot_and_original_lot_intervals_survive_buyer_costs() {
    let mut v: Value = serde_json::from_slice(INPUT).unwrap();
    v["exchange"]["edges"][0]["resource"] = json!(101);
    v["exchange"]["edges"][2]["resource"] = json!(201);
    assert_eq!(
        assemble(&serde_json::to_vec(&v).unwrap()).err().unwrap(),
        "exchange_resource_domain"
    );
    let mut e = engine();
    apply(&mut e, dispatch(1));
    apply(&mut e, dispatch(2));
    advance(&mut e, 120);
    let x = e.world().view().exchange.unwrap();
    for (i, offset) in [(0, 0), (1, 1000)] {
        let f = x.quotes[i].source_fragment.unwrap();
        assert_eq!(
            (
                f.lot_id,
                f.offset.value(),
                f.origin_quantity.value(),
                f.origin_basis.value(),
                f.basis.value()
            ),
            (11, offset, 5000, 2000, 400)
        );
    }
    assert_eq!(stock(&e, 8).1, 2020);
}
#[test]
fn freight_blocks_otherwise_eligible_population_and_closed_city_fault_preserves_losses() {
    let mut e = variant(|v| v["region"]["cities"][0]["willing"] = json!(4000));
    apply(&mut e, dispatch(1));
    assert_eq!(
        reject(
            &mut e,
            Action::RegionMigrate {
                from: 1,
                to: 2,
                requested: 1998,
                tolerance: 0
            }
        ),
        TxError::Capacity
    );
    let mut e = engine();
    apply(&mut e, start(Facility::Pump));
    apply(&mut e, Action::RegionActivate { city: 2 });
    advance(&mut e, 30);
    apply(
        &mut e,
        Action::Boot {
            command: B::Fault {
                facility: Facility::Pump,
            },
        },
    );
    assert_eq!(e.world().view().boot.unwrap().failed_expense.value(), 300);
    apply(
        &mut e,
        Action::Boot {
            command: B::ClearFailure {
                facility: Facility::Pump,
            },
        },
    );
    assert_eq!(stock(&e, 5).0, 600000);
}

#[test]
fn electric_and_water_link_failure_keeps_buffers_money_and_capacity_then_delivers_after_expiry() {
    for (quote, edge, dest, duration) in [(6, 3, 13, 62), (7, 5, 14, 1084)] {
        let mut e = variant(|v| v["exchange"]["quotes"][quote - 1]["expires_at"] = json!(190));
        prepare_power(&mut e);
        apply(&mut e, dispatch(quote as u32));
        advance(&mut e, 10);
        apply(&mut e, x(X::SetLink { edge, open: false }));
        advance(&mut e, 200);
        assert_eq!(stock(&e, dest).0, 0);
        let v = e.world().view();
        let ex = v.exchange.unwrap();
        assert_eq!(ex.energy_loss.value() + ex.water_loss.value(), 0);
        assert_eq!(ex.freight_income.value(), 20);
        assert!(v.accounts.iter().find(|a| a.id == 2).unwrap().protected > Money::ZERO);
        assert!(ex
            .shipments
            .iter()
            .flatten()
            .any(|s| s.arrives_at.is_none()));
        apply(&mut e, x(X::SetLink { edge, open: true }));
        advance(&mut e, duration - 10);
        assert_eq!(stock(&e, dest).0, 98010);
        assert_eq!(
            e.world().view().exchange.unwrap().quotes[quote - 1].status,
            mycity_core::exchange::Status::Delivered
        );
        assert!(e
            .world()
            .view()
            .accounts
            .iter()
            .all(|a| a.protected == Money::ZERO));
    }
}
#[test]
fn opening_capital_cannot_reuse_generated_purchase_lot_namespace() {
    let mut v: Value = serde_json::from_slice(INPUT).unwrap();
    v["sources"][0]["id"] = json!(0xc0000001_u32);
    v["accounts"][0]["source"] = json!(0xc0000001_u32);
    assert_eq!(
        assemble(&serde_json::to_vec(&v).unwrap()).err().unwrap(),
        "exchange_reserved_lot_namespace"
    );
}

#[test]
fn old_due_at_arrival_cannot_spend_later_delivery_income_or_silently_repay_old_arrears() {
    let mut e = variant(|v| {
        v["region"]["obligations"][0]["period_seconds"] = json!(182);
        v["region"]["obligations"][0]["next_due"] = json!(182);
        v["region"]["obligations"][0]["expires_at"] = json!(182);
        v["region"]["obligations"][0]["amount"] = json!(600000)
    });
    prepare_power(&mut e);
    apply(&mut e, dispatch(6));
    advance(&mut e, 62);
    let v = e.world().view();
    assert_eq!(
        v.region.unwrap().obligations[0].unwrap().arrears.value(),
        500
    );
    assert_eq!(
        v.accounts.iter().find(|a| a.id == 3).unwrap().cash.value(),
        98
    );
}
