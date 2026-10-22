//! Joint Engine contract tests for Boot-backed electricity used by Town production.
//! These are source-only cases until the production binding/settlement bridge lands.
use mycity_core::{
    assembly::{assemble, Commodity},
    boot::SCALE_PROFILE as BOOT_SCALE_PROFILE,
    exchange::SCALE_PROFILE as EXCHANGE_SCALE_PROFILE,
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec},
    transaction::{Action, Caller, Engine, Envelope, Resource},
    units::{Money, Quantity},
};
use serde_json::{json, Value};

const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-background-joint-opening.json");
const DAY: u64 = 86_400;
const COMPANY_CASH: i64 = 100_000;
const HOUSEHOLD_CASH: i64 = 1_000;
const ELECTRICITY_STOCK: i64 = 100_000;
const UTILITY_SCALE: u32 = 1_000;

fn opening(company_cash: i64, recipe_count: usize) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["boot"]["profile"] = json!(BOOT_SCALE_PROFILE);
    value["exchange"]["profile"] = json!(EXCHANGE_SCALE_PROFILE);

    // Existing household source remains its sole capital provenance.
    set_row(
        &mut value["sources"],
        "id",
        19,
        "amount",
        json!(HOUSEHOLD_CASH),
    );
    set_row(
        &mut value["accounts"],
        "id",
        5,
        "cash",
        json!(HOUSEHOLD_CASH),
    );
    value["sources"].as_array_mut().unwrap().extend([
        json!({"id":21,"kind":"cash_capital","amount":company_cash}),
        json!({"id":22,"kind":"inventory_capital","amount":1000}),
        json!({"id":23,"kind":"inventory_capital","amount":0}),
        json!({"id":24,"kind":"inventory_capital","amount":0}),
    ]);
    value["accounts"].as_array_mut().unwrap().push(json!({
        "id":7,"role":"company","cash":company_cash,"source":21
    }));
    value["stocks"].as_array_mut().unwrap().extend([
        json!({"id":15,"owner":7,"commodity":"raw","quantity":1000,"basis":1000,"source":22}),
        json!({"id":16,"owner":7,"commodity":"product","quantity":0,"basis":0,"source":23}),
        json!({"id":17,"owner":7,"commodity":"product","quantity":0,"basis":0,"source":24}),
    ]);

    let mut recipes = Vec::new();
    let mut shops = Vec::new();
    let mut delivery = Vec::new();
    for index in 0..recipe_count {
        let recipe_id = index as u32 + 1;
        let stock_id = index as u32 + 16;
        recipes.push(json!({
            "recipe_id":recipe_id,
            "employer":7,
            "assigned_workers":1,
            "input_stock_id":15,
            "input_per_output":1,
            "output_good":1,
            "output_stock_id":stock_id,
            "output_per_worker_day":100,
            "waste_per_output_q":0,
            "available_utility_per_day":0,
            "utility_per_output_q":1,
            "utility_cost_per_output_q":0,
            "freight_cost_per_input_q":0,
            "disposal_cost_per_input_q":0,
            "utility_account":3,
            "freight_account":1,
            "disposal_account":1
        }));
        shops.push(json!({
            "shop_id":recipe_id,
            "account":7,
            "stock_id":stock_id,
            "good":1,
            "shelf_price_per_q":1
        }));
        delivery.push(json!({
            "quote_id":recipe_id,
            "shop":recipe_id,
            "household_group":1,
            "good":1,
            "reachable":true,
            "route_source":77,
            "freight_account":1,
            "utility_account":1,
            "available_capacity":1000,
            "freight_per_q":0,
            "utility_surcharge_per_q":0
        }));
    }
    value["town"] = json!({
        "schema_version":1,
        "id":1,
        "households":[{"id":1,"account":5,"households":1,"need_good":1,"need_per_household":100,"reference_price_per_q":100}],
        "employers":[{"account":7,"worker_limit":recipe_count}],
        "shops":shops,
        "recipes":recipes,
        "delivery":delivery,
        "utility_supply":{
            "boot_operator":3,
            "boot_stock_id":2,
            "stock_quantity_per_utility_unit":UTILITY_SCALE,
            "tariff_per_utility_unit":1
        }
    });
    serde_json::to_vec(&value).unwrap()
}

fn opening_with_cross_city_electricity_quote() -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&opening(COMPANY_CASH, 1)).unwrap();
    let quote = value["exchange"]["quotes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|quote| quote["id"] == 6)
        .unwrap();
    // Quote 6 consumes 40% of the shared electricity pool before Town's
    // local production settlement. It remains a real Exchange quote/path.
    quote["quantity"] = json!(40_000);
    serde_json::to_vec(&value).unwrap()
}

fn set_row(rows: &mut Value, key: &str, id: u32, field: &str, value: Value) {
    let row = rows
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row[key].as_u64() == Some(u64::from(id)))
        .unwrap();
    row[field] = value;
}

fn engine(company_cash: i64, recipe_count: usize) -> Engine {
    Engine::new(
        assemble(&opening(company_cash, recipe_count)).unwrap(),
        resources(),
    )
    .unwrap()
}

fn resources() -> Vec<Resource> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2_000),
        (202, 2_000),
    ]
    .into_iter()
    .map(|(id, capacity)| Resource::new(id, capacity).unwrap())
    .collect()
}

fn envelope(engine: &Engine, operation_id: u64, actions: Vec<Action>) -> Envelope {
    let view = engine.world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions,
    }
}

fn issue(engine: &mut Engine, action: Action) -> mycity_core::transaction::Receipt {
    let id = 0x7000_0000 + engine.world().view().revision + 1;
    engine
        .execute(Caller::System, envelope(engine, id, vec![action]))
        .unwrap()
}

fn start_day(engine: &mut Engine, workers: usize) {
    issue(
        engine,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    for index in 0..workers {
        issue(
            engine,
            Action::Timeline {
                command: TimelineCommand::AddPayroll {
                    spec: PayrollSpec {
                        id: index as u32 + 1,
                        payer: 7,
                        payee: 5,
                        home_city: 1,
                        hourly_cents: Money::new(10).unwrap(),
                        cycle: Cycle::Daily,
                        ends_at: 7 * DAY,
                    },
                },
            },
        );
    }
    advance_to(engine, DAY);
}

fn advance_to(engine: &mut Engine, target: u64) {
    for _ in 0..100 {
        issue(
            engine,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        );
        if engine.timeline().unwrap().progress.target.is_none() {
            return;
        }
    }
    panic!("timeline did not reach target {target}");
}

fn stock_quantity(engine: &Engine, id: u32) -> i64 {
    engine
        .world()
        .view()
        .stocks
        .iter()
        .find(|row| row.id == id)
        .unwrap()
        .quantity
        .value()
}

fn account_cash(engine: &Engine, id: u32) -> i64 {
    engine
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == id)
        .unwrap()
        .cash
        .value()
}

#[test]
fn actual_town_day_exports_only_consumed_electricity_and_does_not_refill_next_day() {
    let mut engine = engine(COMPANY_CASH, 1);
    assert_eq!(stock_quantity(&engine, 2), ELECTRICITY_STOCK);
    assert_eq!(stock_quantity(&engine, 15), 1_000);
    start_day(&mut engine, 1);
    // The unchanged regional fixture charges operator 3 a 200,000 daily obligation
    // at midnight before this market settlement. Keep that real transfer visible.
    let operator_before_market = account_cash(&engine, 3);
    assert_eq!(operator_before_market, 600_000 - 200_000);
    assert_eq!(account_cash(&engine, 1), 1_000_000 + 200_000);
    let obligation = engine.world().view().region.unwrap().obligations[0].unwrap();
    assert_eq!(obligation.next_due, Some(2 * DAY));
    assert_eq!(obligation.arrears, Money::ZERO);

    let settled = issue(&mut engine, Action::TownSettle { profile_id: 1 });
    let summary = settled.town.unwrap();
    assert_eq!(summary.day, 1);
    assert_eq!(summary.produced.value(), 100);
    assert_eq!(
        engine.world().view().boot.unwrap().exports[0].value(),
        ELECTRICITY_STOCK
    );
    assert_eq!(
        engine
            .world()
            .view()
            .boot
            .unwrap()
            .local_electricity_exported,
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(
        engine.world().view().boot.unwrap().local_electricity_basis,
        Money::new(100).unwrap()
    );
    assert_eq!(stock_quantity(&engine, 2), 0);
    assert_eq!(stock_quantity(&engine, 15), 900);
    // The configured workday is 09:00-17:00, not all 24 calendar hours.
    let payroll = engine.timeline().unwrap().payroll[0];
    assert_eq!(payroll.work_seconds, 8 * 3_600);
    assert_eq!(payroll.accrued, Money::new(80).unwrap());
    assert_eq!(payroll.paid, Money::new(80).unwrap());
    assert_eq!(summary.wages_accrued, Money::new(80).unwrap());
    assert_eq!(account_cash(&engine, 7), COMPANY_CASH - 80 - 100 + 100);
    assert_eq!(account_cash(&engine, 3), operator_before_market + 100);

    advance_to(&mut engine, 2 * DAY);
    let next_day = issue(&mut engine, Action::TownSettle { profile_id: 1 });
    assert_eq!(next_day.town.unwrap().produced, Quantity::ZERO);
    assert_eq!(
        engine.world().view().boot.unwrap().exports[0].value(),
        ELECTRICITY_STOCK
    );
    assert_eq!(
        engine
            .world()
            .view()
            .boot
            .unwrap()
            .local_electricity_exported,
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(stock_quantity(&engine, 2), 0);
}

#[test]
fn two_recipes_share_one_boot_stock_pool_and_cannot_over_export() {
    let mut engine = engine(COMPANY_CASH, 2);
    start_day(&mut engine, 2);
    let summary = issue(&mut engine, Action::TownSettle { profile_id: 1 })
        .town
        .unwrap();
    assert_eq!(summary.produced.value(), 100);
    assert_eq!(
        engine.world().view().boot.unwrap().exports[0].value(),
        ELECTRICITY_STOCK
    );
    assert_eq!(
        engine
            .world()
            .view()
            .boot
            .unwrap()
            .local_electricity_exported,
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(stock_quantity(&engine, 2), 0);
}

#[test]
fn only_completed_generator_output_can_supply_the_next_market_day() {
    use mycity_core::boot::{Command as B, Facility};
    for fail_generator in [false, true] {
        let mut engine = engine(COMPANY_CASH, 1);
        // The real pump consumes the entire initial electricity buffer.
        issue(
            &mut engine,
            Action::Boot {
                command: B::Start {
                    facility: Facility::Pump,
                    reverse_request: false,
                },
            },
        );
        assert_eq!(stock_quantity(&engine, 2), 0);
        start_day(&mut engine, 1);
        assert_eq!(stock_quantity(&engine, 3), 600_000);
        issue(
            &mut engine,
            Action::Boot {
                command: B::Start {
                    facility: Facility::Generator,
                    reverse_request: false,
                },
            },
        );
        let before_completion = issue(&mut engine, Action::TownSettle { profile_id: 1 });
        assert_eq!(before_completion.town.unwrap().produced, Quantity::ZERO);
        assert_eq!(
            engine
                .world()
                .view()
                .boot
                .unwrap()
                .local_electricity_exported,
            Quantity::ZERO
        );
        if fail_generator {
            issue(
                &mut engine,
                Action::Boot {
                    command: B::Fault {
                        facility: Facility::Generator,
                    },
                },
            );
        }
        advance_to(&mut engine, DAY + 60);
        assert_eq!(
            stock_quantity(&engine, 2),
            if fail_generator { 0 } else { 400_000 }
        );
        advance_to(&mut engine, 2 * DAY);
        let settled = issue(&mut engine, Action::TownSettle { profile_id: 1 });
        assert_eq!(
            settled.town.unwrap().produced.value(),
            if fail_generator { 0 } else { 100 }
        );
        let boot = engine.world().view().boot.unwrap();
        assert_eq!(
            boot.local_electricity_exported.value(),
            if fail_generator { 0 } else { 100_000 }
        );
        assert_eq!(
            stock_quantity(&engine, 2),
            if fail_generator { 0 } else { 300_000 }
        );
        assert_eq!(
            stock_quantity(&engine, 15),
            if fail_generator { 1_000 } else { 900 }
        );
        let checkpoint = engine.export_checkpoint().unwrap();
        assert_eq!(
            Engine::restore_checkpoint(&checkpoint)
                .unwrap()
                .state_hash(),
            engine.state_hash()
        );
    }
}

#[test]
fn insufficient_company_cash_skips_production_without_blocking_settlement() {
    // Timeline accrues 8 scheduled work hours (09:00-17:00) at 10 cents/hour,
    // so the actual daily wage is 80 cents. The company starts with 150 cents;
    // after wages its 70 cents cannot cover 100 cents of electricity. The
    // authoritative planner skips production, while the market day still settles.
    let mut engine = engine(150, 1);
    start_day(&mut engine, 1);
    assert_eq!(account_cash(&engine, 7), 70);
    assert_eq!(account_cash(&engine, 5), HOUSEHOLD_CASH + 80);
    let settled = issue(&mut engine, Action::TownSettle { profile_id: 1 });
    assert_eq!(settled.town.unwrap().produced, Quantity::ZERO);
    assert_eq!(account_cash(&engine, 7), 70);
    assert_eq!(account_cash(&engine, 5), HOUSEHOLD_CASH + 80);
    assert_eq!(stock_quantity(&engine, 15), 1_000);
    assert_eq!(
        engine
            .world()
            .view()
            .boot
            .unwrap()
            .local_electricity_exported,
        Quantity::ZERO
    );
    assert_eq!(stock_quantity(&engine, 2), ELECTRICITY_STOCK);
}

#[test]
fn metadata_precommit_failure_and_checkpoint_retry_export_exactly_once() {
    let mut engine = engine(COMPANY_CASH, 1);
    start_day(&mut engine, 1);
    let before = engine.export_checkpoint().unwrap();
    let request = envelope(
        &engine,
        0x7200_0001,
        vec![Action::TownSettle { profile_id: 1 }],
    );
    let failed = engine.execute_recorded(Caller::System, request.clone(), |_, _| {
        Err::<(), _>("metadata_precommit_failure")
    });
    assert!(failed.is_err());
    assert_eq!(engine.export_checkpoint().unwrap(), before);

    let mut restored = Engine::restore_checkpoint(&before).unwrap();
    let committed = restored
        .execute_recorded(Caller::System, request.clone(), |_, _| Ok::<(), ()>(()))
        .unwrap()
        .unwrap();
    assert_eq!(committed.town.unwrap().produced.value(), 100);
    assert_eq!(
        restored.world().view().boot.unwrap().exports[0].value(),
        ELECTRICITY_STOCK
    );
    assert_eq!(
        restored
            .world()
            .view()
            .boot
            .unwrap()
            .local_electricity_exported,
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(
        restored
            .world()
            .view()
            .boot
            .unwrap()
            .local_electricity_basis,
        Money::new(100).unwrap()
    );
    assert_eq!(stock_quantity(&restored, 2), 0);
    let after_commit = restored.state_hash();
    let duplicate = restored.execute(Caller::System, request).unwrap();
    assert_eq!(duplicate.town.unwrap().produced.value(), 100);
    assert_eq!(restored.state_hash(), after_commit);
    assert_eq!(
        restored.world().view().boot.unwrap().exports[0].value(),
        ELECTRICITY_STOCK
    );
}

#[test]
fn accepted_cross_city_quote_uses_shared_pool_before_local_market_and_survives_checkpoint() {
    let world = assemble(&opening_with_cross_city_electricity_quote()).unwrap();
    let mut engine = Engine::new(world, resources()).unwrap();
    issue(
        &mut engine,
        Action::Exchange {
            command: mycity_core::exchange::Command::Dispatch { quote: 6 },
        },
    );
    let accepted_status = engine
        .world()
        .view()
        .exchange
        .unwrap()
        .quotes
        .iter()
        .find(|quote| quote.spec.id == 6)
        .map(|quote| quote.status)
        .unwrap();
    assert_eq!(accepted_status, mycity_core::exchange::Status::Transit);
    let after_cross_city_dispatch = engine.world().view().boot.unwrap();
    assert_eq!(after_cross_city_dispatch.exports[0].value(), 40_000);
    assert_eq!(
        after_cross_city_dispatch.local_electricity_exported,
        Quantity::ZERO
    );
    assert_eq!(
        after_cross_city_dispatch.local_electricity_basis,
        Money::ZERO
    );

    start_day(&mut engine, 1);
    let settled = issue(&mut engine, Action::TownSettle { profile_id: 1 });
    assert_eq!(settled.town.unwrap().produced.value(), 60);
    let boot = engine.world().view().boot.unwrap();
    assert_eq!(
        boot.local_electricity_exported,
        Quantity::new(60_000).unwrap()
    );
    assert_eq!(boot.local_electricity_basis, Money::new(60).unwrap());
    assert_eq!(boot.exports[0].value(), ELECTRICITY_STOCK);
    assert_eq!(stock_quantity(&engine, 2), 0);
    assert_eq!(stock_quantity(&engine, 15), 940);

    // The checkpoint preserves the distinct cross-city and local classifications.
    let checkpoint = engine.export_checkpoint().unwrap();
    let restored = Engine::restore_checkpoint(&checkpoint).unwrap();
    let restored_boot = restored.world().view().boot.unwrap();
    assert_eq!(restored_boot.exports[0].value(), ELECTRICITY_STOCK);
    assert_eq!(
        restored_boot.local_electricity_exported,
        Quantity::new(60_000).unwrap()
    );
    assert_eq!(
        restored_boot.local_electricity_basis,
        Money::new(60).unwrap()
    );
    assert_eq!(restored.state_hash(), engine.state_hash());
}

fn rejected(value: Value) -> bool {
    match assemble(&serde_json::to_vec(&value).unwrap()) {
        Ok(world) => Engine::new(world, resources()).is_err(),
        Err(_) => true,
    }
}

#[test]
fn invalid_boot_binding_and_stale_legacy_power_controls_are_rejected() {
    let valid: Value = serde_json::from_slice(&opening(COMPANY_CASH, 1)).unwrap();

    let mut wrong_stock = valid.clone();
    wrong_stock["town"]["utility_supply"]["boot_stock_id"] = json!(15);
    assert!(rejected(wrong_stock));

    let mut wrong_owner = valid.clone();
    set_row(&mut wrong_owner["stocks"], "id", 2, "owner", json!(4));
    assert!(rejected(wrong_owner));

    let mut wrong_commodity = valid.clone();
    wrong_commodity["town"]["utility_supply"]["boot_stock_id"] = json!(1);
    assert!(rejected(wrong_commodity));

    let mut wrong_conversion = valid.clone();
    wrong_conversion["town"]["utility_supply"]["stock_quantity_per_utility_unit"] = json!(0);
    assert!(rejected(wrong_conversion));

    let mut stale_legacy_controls_with_binding = valid;
    stale_legacy_controls_with_binding["town"]["recipes"][0]["available_utility_per_day"] =
        json!(1);
    stale_legacy_controls_with_binding["town"]["recipes"][0]["utility_cost_per_output_q"] =
        json!(1);
    stale_legacy_controls_with_binding["town"]["delivery"][0]["utility_surcharge_per_q"] = json!(1);
    assert!(rejected(stale_legacy_controls_with_binding));
}

#[test]
fn legacy_profile_without_boot_binding_remains_compatible() {
    let mut legacy: Value = serde_json::from_slice(&opening(COMPANY_CASH, 1)).unwrap();
    legacy["town"]
        .as_object_mut()
        .unwrap()
        .remove("utility_supply");
    let world = assemble(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(Engine::new(world, resources()).is_ok());
}

#[test]
fn opening_utility_stock_is_the_existing_electricity_commodity_and_boot_owner() {
    let world = assemble(&opening(COMPANY_CASH, 1)).unwrap();
    let view = world.view();
    let stock = view.stocks.iter().find(|stock| stock.id == 2).unwrap();
    assert_eq!(stock.owner, 3);
    assert_eq!(stock.commodity, Commodity::Electricity);
    assert_eq!(stock.quantity, Quantity::new(ELECTRICITY_STOCK).unwrap());
    let boot = view.boot.unwrap();
    assert_eq!(boot.operator, 3);
    assert_eq!(boot.stocks[1], 2);
}
