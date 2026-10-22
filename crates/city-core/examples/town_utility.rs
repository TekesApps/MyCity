//! Exercise real Boot startup, calendar advancement, and Town electricity settlement.
//!
//! The fixture has one household, one employer/shop, one raw-input stock, and a
//! Boot-owned electricity stock. The pump consumes the initial electricity buffer;
//! the day-one market settles while the generator is still unfinished. Only the
//! generator's completed day-two output can supply production. The fixed regional
//! obligation and Exchange profile remain part of the opening world; no additional
//! utility, sales, or cash ledger is created here.
use mycity_core::{
    assembly::assemble,
    boot::{Command as BootCommand, Facility},
    timeline::{Command as TimelineCommand, Cycle, PayrollSpec, DAY},
    transaction::{Action, Caller, Engine, Envelope, Receipt, Resource},
    units::{Money, Quantity},
};
use std::env;

const OPENING: &[u8] = include_bytes!("../../../fixtures/core-town-utility-opening.json");
const COMPANY: u32 = 7;
const HOUSEHOLD: u32 = 5;
const OPERATOR: u32 = 3;
const ELECTRICITY_STOCK: u32 = 2;
const RAW_STOCK: u32 = 15;
const PRODUCT_STOCK: u32 = 16;

fn resources() -> Result<Vec<Resource>, String> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2_000),
        (202, 2_000),
    ]
    .into_iter()
    .map(|(id, capacity)| {
        Resource::new(id, capacity).map_err(|error| format!("resource:{id}:{error:?}"))
    })
    .collect()
}

fn envelope(engine: &Engine, action: Action) -> Envelope {
    let view = engine.world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: 0x7400_0000 + view.revision + 1,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn issue(engine: &mut Engine, action: Action) -> Result<Receipt, String> {
    engine
        .execute(Caller::System, envelope(engine, action))
        .map_err(|error| format!("engine:{error:?}"))
}

fn advance_timeline_to(engine: &mut Engine, target: u64) -> Result<(), String> {
    for _ in 0..100 {
        issue(
            engine,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        )?;
        if engine
            .timeline()
            .is_some_and(|timeline| timeline.progress.target.is_none())
        {
            return Ok(());
        }
    }
    Err(format!("timeline_target_not_reached:{target}"))
}

fn stock(engine: &Engine, id: u32) -> Result<(Quantity, Money), String> {
    let view = engine.world().view();
    let row = view
        .stocks
        .iter()
        .find(|row| row.id == id)
        .ok_or_else(|| format!("missing_stock:{id}"))?;
    Ok((row.quantity, row.basis))
}

fn cash(engine: &Engine, id: u32) -> Result<Money, String> {
    engine
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.cash)
        .ok_or_else(|| format!("missing_account:{id}"))
}

fn cash_total(engine: &Engine) -> i128 {
    engine
        .world()
        .view()
        .accounts
        .iter()
        .map(|account| i128::from(account.cash.value()))
        .sum()
}

fn start_day_one_calendar(engine: &mut Engine) -> Result<(), String> {
    issue(
        engine,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    )?;
    issue(
        engine,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: COMPANY,
                    payee: HOUSEHOLD,
                    home_city: 1,
                    hourly_cents: Money::new(10).map_err(|error| format!("money:{error:?}"))?,
                    cycle: Cycle::Daily,
                    ends_at: 7 * DAY,
                },
            },
        },
    )?;
    advance_timeline_to(engine, DAY)
}

fn run(generator_failure: bool) -> Result<(), String> {
    let world = assemble(OPENING).map_err(|error| format!("opening:{error}"))?;
    let mut engine =
        Engine::new(world, resources()?).map_err(|error| format!("engine:{error:?}"))?;
    let initial_cash = i128::from(
        engine
            .world()
            .view()
            .boot
            .ok_or("missing_boot")?
            .initial_cash
            .value(),
    );
    let initial_electricity = stock(&engine, ELECTRICITY_STOCK)?.0;

    // The real pump consumes the opening electricity at admission. Its completed
    // water output is made available by the same authoritative calendar advance.
    issue(
        &mut engine,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    )?;
    let opening_power = stock(&engine, ELECTRICITY_STOCK)?.0;
    start_day_one_calendar(&mut engine)?;
    let pumped_water = stock(&engine, 3)?.0;

    issue(
        &mut engine,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    )?;
    let day_one = issue(&mut engine, Action::TownSettle { profile_id: 1 })?
        .town
        .filter(|summary| summary.day == 1)
        .ok_or("missing_day_one_summary")?;
    let day_one_power = stock(&engine, ELECTRICITY_STOCK)?.0;
    let day_one_local_export = engine
        .world()
        .view()
        .boot
        .ok_or("missing_boot")?
        .local_electricity_exported;

    if generator_failure {
        issue(
            &mut engine,
            Action::Boot {
                command: BootCommand::Fault {
                    facility: Facility::Generator,
                },
            },
        )?;
    }
    advance_timeline_to(&mut engine, DAY + 60)?;
    let generator_electricity = stock(&engine, ELECTRICITY_STOCK)?.0;
    advance_timeline_to(&mut engine, 2 * DAY)?;
    let operator_before_market = cash(&engine, OPERATOR)?;
    let day_two = issue(&mut engine, Action::TownSettle { profile_id: 1 })?
        .town
        .filter(|summary| summary.day == 2)
        .ok_or("missing_day_two_summary")?;
    let operator_after_market = cash(&engine, OPERATOR)?;
    let utility_payment = operator_after_market
        .checked_sub(operator_before_market)
        .map_err(|error| format!("utility_payment:{error:?}"))?;

    let view = engine.world().view();
    let boot = view.boot.ok_or("missing_boot")?;
    let total_electricity_export = boot.exports[0];
    let cross_city_electricity_export = total_electricity_export
        .checked_sub(boot.local_electricity_exported)
        .map_err(|error| format!("export_classification:{error:?}"))?;
    let (remaining_electricity, electricity_basis) = stock(&engine, ELECTRICITY_STOCK)?;
    let (remaining_raw, remaining_raw_basis) = stock(&engine, RAW_STOCK)?;
    let (remaining_product, remaining_product_basis) = stock(&engine, PRODUCT_STOCK)?;
    let expected_output = if generator_failure { 0 } else { 100 };
    let expected_energy = if generator_failure { 0 } else { 400_000 };
    if initial_electricity.value() != 100_000
        || opening_power != Quantity::ZERO
        || pumped_water.value() != 600_000
        || day_one.produced != Quantity::ZERO
        || day_one_power != Quantity::ZERO
        || day_one_local_export != Quantity::ZERO
        || generator_electricity.value() != expected_energy
        || day_two.produced.value() != expected_output
        || day_two.actual_sales.value() != expected_output
        || utility_payment.value() != expected_output
        || day_two.utilities != utility_payment
        || boot.local_electricity_exported.value() != expected_output * 1_000
        || total_electricity_export != boot.local_electricity_exported
        || cross_city_electricity_export != Quantity::ZERO
        || remaining_electricity.value() != expected_energy - expected_output * 1_000
        || remaining_raw.value() != 1_000 - expected_output
    {
        return Err("boot_town_physical_or_payment_reconciliation_failed".into());
    }
    let final_cash = cash_total(&engine);
    let cash_conserved = final_cash == initial_cash;
    if !cash_conserved {
        return Err(format!("cash_conservation:{initial_cash}:{final_cash}"));
    }

    let checkpoint = engine
        .export_checkpoint()
        .map_err(|error| format!("checkpoint_export:{error}"))?;
    let restored = Engine::restore_checkpoint(&checkpoint)
        .map_err(|error| format!("checkpoint_restore:{error}"))?;
    let checkpoint_equal = restored.state_hash() == engine.state_hash();
    if !checkpoint_equal {
        return Err("checkpoint_state_mismatch".into());
    }

    println!(
        "mode={} generator_failure={} opening_electricity={} pump_water={} day1_produced={} day1_electricity={} day1_local_export={} generator_electricity_after_60s={} day2_produced={} day2_sales={} electricity_remaining={} electricity_remaining_basis={} raw_remaining={} raw_remaining_basis={} product_remaining={} product_remaining_basis={} electricity_export_total={} electricity_export_local={} electricity_export_cross_city={} electricity_export_basis_total={} electricity_export_basis_local={} utility_payment_to_operator={} operator_cash_before_market={} operator_cash_after_market={} company_cash_final={} household_cash_final={} cash_initial_total={} cash_final_total={} cash_conserved={} checkpoint_equal={} checkpoint_state_sha256={}",
        if generator_failure { "generator-failure" } else { "normal" },
        generator_failure,
        opening_power.value(),
        pumped_water.value(),
        day_one.produced.value(),
        day_one_power.value(),
        day_one_local_export.value(),
        generator_electricity.value(),
        day_two.produced.value(),
        day_two.actual_sales.value(),
        remaining_electricity.value(),
        electricity_basis.value(),
        remaining_raw.value(),
        remaining_raw_basis.value(),
        remaining_product.value(),
        remaining_product_basis.value(),
        total_electricity_export.value(),
        boot.local_electricity_exported.value(),
        cross_city_electricity_export.value(),
        boot.exported_basis.value(),
        boot.local_electricity_basis.value(),
        utility_payment.value(),
        operator_before_market.value(),
        operator_after_market.value(),
        cash(&engine, COMPANY)?.value(),
        cash(&engine, HOUSEHOLD)?.value(),
        initial_cash,
        final_cash,
        cash_conserved,
        checkpoint_equal,
        restored.state_hash(),
    );
    println!("scope=boot_inventory_electricity live_transport=false days=2 self_funding=not_demonstrated initial_electricity={}", initial_electricity.value());
    Ok(())
}

fn main() -> Result<(), String> {
    let mut generator_failure = false;
    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--generator-failure" => generator_failure = true,
            _ => {
                return Err(format!(
                    "usage: town_utility [--generator-failure] (unknown:{argument})"
                ))
            }
        }
    }
    run(generator_failure)
}
