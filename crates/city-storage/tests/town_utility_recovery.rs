//! Store recovery/idempotency for Boot-backed local electricity used by a Town recipe.
use mycity_core::{
    boot::{Command as BootCommand, Facility, Phase as BootPhase},
    timeline::{Command as TimelineCommand, Cycle, Payroll, PayrollSpec, DAY},
    town_runtime::{MarketSummary, PayrollCursor, RecipeOutputRemainder},
    transaction::{Action, Caller, Envelope, Resource},
    units::{Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/core-town-utility-opening.json");
const RULES: &str = "town-utility-recovery-engine-v1";
const COMPANY: u32 = 7;
const HOUSEHOLD: u32 = 5;
const OPERATOR: u32 = 3;
const ELECTRICITY_STOCK: u32 = 2;
const RAW_STOCK: u32 = 15;
const PRODUCT_STOCK: u32 = 16;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-town-utility-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("TOWN_UTILITY_ARTIFACT_CREATED={}", path.display());
        Self(path)
    }

    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("TOWN_UTILITY_ARTIFACT_PRESERVED={}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
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

fn envelope(store: &Store, operation_id: u64, action: Action) -> Envelope {
    let view = store.engine().unwrap().world().view();
    Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id,
        expected_revision: view.revision,
        actions: vec![action],
    }
}

fn execute(store: &mut Store, action: Action) {
    let request = envelope(store, store.committed_sequence() + 1, action);
    let outcome = store.execute(Caller::System, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
}

fn advance_to(store: &mut Store, target: u64) {
    for _ in 0..128 {
        execute(
            store,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target,
                    limit: 8_192,
                },
            },
        );
        if store
            .engine()
            .unwrap()
            .timeline()
            .unwrap()
            .progress
            .target
            .is_none()
        {
            return;
        }
    }
    panic!("timeline did not reach {target}");
}

fn stock(store: &Store, id: u32) -> (Quantity, Money) {
    let view = store.engine().unwrap().world().view();
    let row = view.stocks.iter().find(|row| row.id == id).unwrap();
    (row.quantity, row.basis)
}

fn town_summary(store: &Store) -> MarketSummary {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .town
        .unwrap()
        .state
        .last_summary
}

fn local_electricity_exported(store: &Store) -> Quantity {
    store
        .engine()
        .unwrap()
        .world()
        .view()
        .boot
        .unwrap()
        .local_electricity_exported
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BootEvidence {
    time: u64,
    jobs: [mycity_core::boot::Job; 2],
    fuel_used: Quantity,
    electricity_exports: Quantity,
    exported_basis: Money,
    local_electricity_exported: Quantity,
    local_electricity_basis: Money,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EconomicState {
    cash: Vec<(u32, i64)>,
    stocks: Vec<(u32, i64, i64, u32)>,
    boot: BootEvidence,
    settled_day: u64,
    summary: MarketSummary,
    market_payroll: Vec<PayrollCursor>,
    output_remainders: Vec<RecipeOutputRemainder>,
    timeline_payroll: Vec<Payroll>,
    timeline_last_complete: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    sequence: u64,
    state_hash: String,
    economy: EconomicState,
}

fn snapshot(store: &Store) -> Snapshot {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    let boot = view.boot.unwrap();
    let town = view.town.unwrap();
    let timeline = engine.timeline().unwrap();
    Snapshot {
        sequence: store.committed_sequence(),
        state_hash: store.state_hash().unwrap(),
        economy: EconomicState {
            cash: view
                .accounts
                .iter()
                .map(|row| (row.id, row.cash.value()))
                .collect(),
            stocks: view
                .stocks
                .iter()
                .map(|row| (row.id, row.quantity.value(), row.basis.value(), row.lot_id))
                .collect(),
            boot: BootEvidence {
                time: boot.time,
                jobs: boot.jobs,
                fuel_used: boot.fuel_used,
                electricity_exports: boot.exports[0],
                exported_basis: boot.exported_basis,
                local_electricity_exported: boot.local_electricity_exported,
                local_electricity_basis: boot.local_electricity_basis,
            },
            settled_day: town.state.settled_day,
            summary: town.state.last_summary,
            market_payroll: town.state.payroll.as_slice().to_vec(),
            output_remainders: town.state.recipe_output_remainders.as_slice().to_vec(),
            timeline_payroll: timeline.payroll.iter().copied().collect(),
            timeline_last_complete: timeline.progress.last_complete,
        },
    }
}

fn setup_before_day_two(path: &Path) -> Store {
    let mut store = Store::create_spatial(
        path,
        OPENING,
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap();
    assert_eq!(
        stock(&store, ELECTRICITY_STOCK).0,
        Quantity::new(100_000).unwrap()
    );

    execute(
        &mut store,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    );
    assert_eq!(stock(&store, ELECTRICITY_STOCK).0, Quantity::ZERO);
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    execute(
        &mut store,
        Action::Timeline {
            command: TimelineCommand::AddPayroll {
                spec: PayrollSpec {
                    id: 1,
                    payer: COMPANY,
                    payee: HOUSEHOLD,
                    home_city: 1,
                    hourly_cents: Money::new(10).unwrap(),
                    cycle: Cycle::Daily,
                    ends_at: 7 * DAY,
                },
            },
        },
    );
    advance_to(&mut store, DAY);
    assert_eq!(stock(&store, 3).0, Quantity::new(600_000).unwrap());
    execute(
        &mut store,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    );
    let boot = store.engine().unwrap().world().view().boot.unwrap();
    assert_eq!(boot.jobs[1].phase, BootPhase::Running);
    assert_eq!(boot.jobs[1].completed, 0);
    let day_one_request = envelope(
        &store,
        store.committed_sequence() + 1,
        Action::TownSettle { profile_id: 1 },
    );
    let day_one = store.execute(Caller::System, day_one_request).unwrap();
    assert!(day_one.model.unwrap().town.is_some());
    assert_eq!(town_summary(&store).day, 1);
    assert_eq!(town_summary(&store).produced, Quantity::ZERO);
    assert_eq!(stock(&store, ELECTRICITY_STOCK).0, Quantity::ZERO);
    assert_eq!(local_electricity_exported(&store), Quantity::ZERO);
    advance_to(&mut store, DAY + 60);
    advance_to(&mut store, 2 * DAY);
    assert_eq!(
        stock(&store, ELECTRICITY_STOCK).0,
        Quantity::new(400_000).unwrap()
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .town
            .unwrap()
            .state
            .settled_day,
        1
    );
    store
}

fn day_two_request(store: &Store) -> Envelope {
    envelope(
        store,
        store.committed_sequence() + 1,
        Action::TownSettle { profile_id: 1 },
    )
}

fn assert_day_two_effect(before: &Snapshot, after: &Snapshot) {
    assert_eq!(after.economy.settled_day, 2);
    assert_eq!(after.economy.summary.day, 2);
    assert_eq!(after.economy.summary.produced, Quantity::new(100).unwrap());
    assert_eq!(
        after.economy.summary.actual_sales,
        Quantity::new(100).unwrap()
    );
    assert_eq!(after.economy.summary.utilities, Money::new(100).unwrap());
    assert_eq!(
        after.economy.boot.local_electricity_exported.value()
            - before.economy.boot.local_electricity_exported.value(),
        100_000
    );
    assert_eq!(
        after.economy.boot.electricity_exports.value()
            - before.economy.boot.electricity_exports.value(),
        100_000
    );
    assert_eq!(
        after.economy.boot.electricity_exports, after.economy.boot.local_electricity_exported,
        "all electricity export in this fixture must be local to the bound Town"
    );
    assert_eq!(
        after.economy.boot.exported_basis.value() - before.economy.boot.exported_basis.value(),
        after.economy.boot.local_electricity_basis.value()
            - before.economy.boot.local_electricity_basis.value()
    );
    assert!(
        after.economy.boot.local_electricity_basis > before.economy.boot.local_electricity_basis
    );
    assert_eq!(
        stock_quantity(&after.economy, ELECTRICITY_STOCK)
            - stock_quantity(&before.economy, ELECTRICITY_STOCK),
        -100_000
    );
    assert_eq!(
        stock_basis(&before.economy, ELECTRICITY_STOCK)
            - stock_basis(&after.economy, ELECTRICITY_STOCK),
        after.economy.boot.local_electricity_basis.value()
            - before.economy.boot.local_electricity_basis.value()
    );
    assert_eq!(
        stock_quantity(&before.economy, RAW_STOCK) - stock_quantity(&after.economy, RAW_STOCK),
        100
    );
    assert_eq!(
        stock_quantity(&before.economy, PRODUCT_STOCK) + after.economy.summary.produced.value()
            - after.economy.summary.actual_sales.value(),
        stock_quantity(&after.economy, PRODUCT_STOCK)
    );
    assert_eq!(
        cash_value(&after.economy, OPERATOR) - cash_value(&before.economy, OPERATOR),
        100
    );
    assert_eq!(cash_total(&before.economy), cash_total(&after.economy));
    assert_eq!(
        after.economy.timeline_payroll,
        before.economy.timeline_payroll
    );
    assert_eq!(
        after.economy.market_payroll.len(),
        before.economy.market_payroll.len()
    );
    assert_eq!(
        after.economy.market_payroll[0].work_seconds
            - before.economy.market_payroll[0].work_seconds,
        28_800
    );
    assert_eq!(after.economy.summary.wages_accrued, Money::new(80).unwrap());
    assert_eq!(
        after.economy.output_remainders.len(),
        before.economy.output_remainders.len()
    );
}

fn stock_quantity(state: &EconomicState, id: u32) -> i64 {
    state.stocks.iter().find(|row| row.0 == id).unwrap().1
}

fn stock_basis(state: &EconomicState, id: u32) -> i64 {
    state.stocks.iter().find(|row| row.0 == id).unwrap().2
}

fn cash_value(state: &EconomicState, id: u32) -> i64 {
    state.cash.iter().find(|row| row.0 == id).unwrap().1
}

fn cash_total(state: &EconomicState) -> i128 {
    state.cash.iter().map(|row| i128::from(row.1)).sum()
}

fn settle_fault_and_replay(path: &Path, point: WritePoint) {
    let mut store = setup_before_day_two(path);
    let before = snapshot(&store);
    let request = day_two_request(&store);
    let failure = store.execute_with_observer(Caller::System, request.clone(), |current| {
        if current == point {
            Err(std::io::Error::other("injected TownSettle write fault"))
        } else {
            Ok(())
        }
    });
    assert!(failure.is_err());
    assert!(store.engine().is_err());
    drop(store);

    let mut store = Store::open(path, RULES).unwrap();
    let recovered = snapshot(&store);
    if point == WritePoint::Prepared {
        assert_eq!(
            recovered, before,
            "precommit failure must leave day two untouched"
        );
    } else {
        assert_eq!(recovered.sequence, request.operation_id);
        assert_day_two_effect(&before, &recovered);
    }

    let replay = store.execute(Caller::System, request.clone()).unwrap();
    assert!(replay.has_persistent_receipt);
    let committed = snapshot(&store);
    if point == WritePoint::Prepared {
        assert_day_two_effect(&before, &committed);
    } else {
        assert_eq!(
            committed, recovered,
            "same-operation replay must be a no-op"
        );
    }

    let new_operation = envelope(
        &store,
        store.committed_sequence() + 1,
        Action::TownSettle { profile_id: 1 },
    );
    let outcome = store.execute(Caller::System, new_operation).unwrap();
    assert!(
        outcome.model.is_err(),
        "second operation for day two succeeded"
    );
    assert_eq!(snapshot(&store).economy, committed.economy);

    let before_checkpoint = snapshot(&store);
    store.checkpoint().unwrap();
    drop(store);
    let store = Store::open(path, RULES).unwrap();
    assert_eq!(snapshot(&store), before_checkpoint);
    assert_day_two_effect(&before, &snapshot(&store));
}

#[test]
fn town_utility_prepared_failure_and_same_day_retry_are_exactly_once() {
    let temp = Temp::new();
    settle_fault_and_replay(&temp.world(), WritePoint::Prepared);
}

#[test]
fn town_utility_applied_lost_reply_and_same_day_retry_are_exactly_once() {
    let temp = Temp::new();
    settle_fault_and_replay(&temp.world(), WritePoint::Applied);
}
