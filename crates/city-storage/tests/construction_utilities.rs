//! Store-level tests for Boot-backed construction utilities.
use mycity_core::{
    boot::{Boot, Command as BootCommand, Facility},
    construction::{
        AbortClearanceMaterialRule, AbortClearanceSpec, ConstructionUtilityContract,
        ConstructionUtilitySupply, EngineCommand as C, Gate, GateCategory, GateDecision, Kind,
        Material, Project,
    },
    exchange::Command as X,
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource},
    units::{GameTime, Money, Quantity},
};
use mycity_storage::{Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPENING: &[u8] = include_bytes!("../../../fixtures/core-town-utility-opening.json");
const RULES: &str = "construction-utilities-store-v1";
const OWNER: u32 = 7;
const CONTRACTOR: u32 = 4;
const OPERATOR: u32 = 3;
const ELECTRICITY: u32 = 2;
const WATER: u32 = 3;
const NEXT_OPERATION_ID: u64 = 70_000_000;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(NEXT_OPERATION_ID);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "mycity-construction-utilities-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        eprintln!("CONSTRUCTION_UTILITIES_ARTIFACT={}", path.display());
        Self(path)
    }

    fn world(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn retain_recovery_inputs(&self) {
        if std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT").is_some() {
            fs::write(
                self.0.join("KEEP-RESTORE-INPUTS"),
                b"Retain this case's complete Store, WAL and checkpoint as recovery evidence.\n",
            )
            .unwrap();
        }
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if std::thread::panicking() || self.0.join("KEEP-RESTORE-INPUTS").exists() {
            eprintln!("CONSTRUCTION_UTILITIES_PRESERVED={}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn resources() -> Vec<Resource> {
    [
        (80, 1),
        (101, 100_000),
        (102, 100_000),
        (103, 100_000),
        (104, 100_000),
        (201, 2_000),
        (202, 2_000),
    ]
    .into_iter()
    .map(|(id, capacity)| Resource::new(id, capacity).unwrap())
    .collect()
}

fn lifecycle_resources() -> Vec<Resource> {
    let mut rows = resources();
    rows.push(Resource::new(81, 2).unwrap());
    rows.push(Resource::new(82, 10).unwrap());
    rows.sort_by_key(|resource| resource.id);
    rows
}

fn opening(electricity: i64, water: i64) -> Vec<u8> {
    let project = Project::new(
        7,
        Kind::Build,
        OWNER,
        CONTRACTOR,
        1,
        80,
        1,
        1,
        Money::new(100_000).unwrap(),
        Money::new(3_600).unwrap(),
        7_200,
        &[
            Gate {
                id: 1,
                category: GateCategory::OwnerConsent,
                evidence_id: 0,
                party: OWNER,
                decided: GateDecision::Pending,
            },
            Gate {
                id: 2,
                category: GateCategory::Permit,
                evidence_id: 0,
                party: 1,
                decided: GateDecision::Pending,
            },
        ],
        &[Material {
            stock_id: 15,
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(1_000).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap();
    let mut project = project;
    project.utility_contract = Some(ConstructionUtilityContract {
        electricity: Some(ConstructionUtilitySupply {
            operator: OPERATOR,
            stock_id: ELECTRICITY,
            rate_per_3600_worker_seconds: Quantity::new(1_000).unwrap(),
            tariff_per_quantity_unit: Money::new(20).unwrap(),
            quantity_unit: Quantity::new(1_000).unwrap(),
        }),
        water: Some(ConstructionUtilitySupply {
            operator: OPERATOR,
            stock_id: WATER,
            rate_per_3600_worker_seconds: Quantity::new(1_000).unwrap(),
            tariff_per_quantity_unit: Money::new(10).unwrap(),
            quantity_unit: Quantity::new(1_000).unwrap(),
        }),
    });

    project.enable_material_accounting().unwrap();
    let mut value: Value = serde_json::from_slice(OPENING).unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    for (stock_id, source_id, quantity, basis) in [
        (
            ELECTRICITY,
            6,
            electricity,
            if electricity == 0 { 0 } else { 100 },
        ),
        (WATER, 7, water, if water == 0 { 0 } else { 100 }),
    ] {
        let stock = value["stocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["id"] == json!(stock_id))
            .unwrap();
        stock["quantity"] = json!(quantity);
        stock["basis"] = json!(basis);
        let source = value["sources"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["id"] == json!(source_id))
            .unwrap();
        source["amount"] = json!(if quantity == 0 { 0 } else { 100 });
    }
    serde_json::to_vec(&value).unwrap()
}

fn create(path: &Path, electricity: i64, water: i64) -> Store {
    Store::create_spatial(
        path,
        &opening(electricity, water),
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}

fn lifecycle_opening(electricity: i64, water: i64) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&opening(electricity, water)).unwrap();
    let mut project: Project = serde_json::from_value(value["construction"].clone()).unwrap();
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = OPERATOR;
    project.waste_receiver_resource = 82;
    project.waste_units = 2;
    project.waste_fee = Money::new(250).unwrap();
    project.abort_clearance_spec = Some(AbortClearanceSpec {
        base_worker_seconds: 3_600,
        material_rules: [
            AbortClearanceMaterialRule {
                stock_id: 15,
                quantity_per_waste_capacity_unit: 500,
                worker_seconds_per_thousand_quantity: 3_600,
            },
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
        ],
    });
    project.validate_profile().unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    serde_json::to_vec(&value).unwrap()
}

fn create_lifecycle(path: &Path) -> Store {
    Store::create_spatial(
        path,
        &lifecycle_opening(100_000, 20_000),
        lifecycle_resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
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

fn next_operation() -> u64 {
    NEXT_OPERATION.fetch_add(1, Ordering::Relaxed)
}

fn execute(store: &mut Store, caller: Caller, action: Action) {
    let request = envelope(store, next_operation(), action);
    let outcome = store.execute(caller, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
}

fn construction(store: &mut Store, caller: Caller, command: C) {
    execute(store, caller, Action::Construction { command });
}

fn setup(store: &mut Store) {
    execute(
        store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance_to(store, 9 * 3_600);
    prepare_construction(store);
}

fn prepare_construction(store: &mut Store) {
    construction(store, Caller::Account(OWNER), C::File);
    construction(
        store,
        Caller::Account(OWNER),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 7001,
        },
    );
    construction(
        store,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 7002,
        },
    );
    construction(store, Caller::Account(OWNER), C::Prepare);
    construction(store, Caller::Account(OWNER), C::Start);
}

fn approve_demolition(store: &mut Store) {
    construction(
        store,
        Caller::Account(OWNER),
        C::BeginDemolition {
            waste_receiver: OPERATOR,
        },
    );
    construction(
        store,
        Caller::Account(OWNER),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 7101,
        },
    );
    construction(
        store,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 7102,
        },
    );
    construction(store, Caller::Account(OWNER), C::Prepare);
    construction(store, Caller::Account(OWNER), C::Start);
}

fn advance_to(store: &mut Store, target: u64) {
    for _ in 0..128 {
        execute(
            store,
            Caller::System,
            Action::Timeline {
                command: T::Advance {
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

fn work_request(store: &Store, operation_id: u64, at: u64, elapsed: u32) -> Envelope {
    work_request_with_seconds(store, operation_id, at, elapsed, 3_600)
}

fn work_request_with_seconds(
    store: &Store,
    operation_id: u64,
    at: u64,
    elapsed: u32,
    worker_seconds: u32,
) -> Envelope {
    envelope(
        store,
        operation_id,
        Action::Construction {
            command: C::Work {
                at: GameTime::new(i64::try_from(at).unwrap()).unwrap(),
                elapsed_seconds: elapsed,
                worker_seconds,
            },
        },
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    sequence: u64,
    state_hash: String,
    cash: Vec<(u32, i64, i64)>,
    stocks: Vec<(u32, i64, i64, u32)>,
    boot: Boot,
    project: Project,
    timeline_last_complete: u64,
}

fn snapshot(store: &Store) -> Snapshot {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    Snapshot {
        sequence: store.committed_sequence(),
        state_hash: store.state_hash().unwrap(),
        cash: view
            .accounts
            .iter()
            .map(|row| (row.id, row.cash.value(), row.protected.value()))
            .collect(),
        stocks: view
            .stocks
            .iter()
            .map(|row| (row.id, row.quantity.value(), row.basis.value(), row.lot_id))
            .collect(),
        boot: view.boot.unwrap(),
        project: view.construction.unwrap(),
        timeline_last_complete: engine
            .timeline()
            .map_or(0, |clock| clock.progress.last_complete),
    }
}

fn work_state_unchanged(before: &Snapshot, after: &Snapshot) {
    assert_eq!(after.cash, before.cash);
    assert_eq!(after.stocks, before.stocks);
    assert_eq!(after.boot, before.boot);
    assert_eq!(after.project, before.project);
    assert_eq!(after.timeline_last_complete, before.timeline_last_complete);
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SharedSupplySnapshot {
    store: Snapshot,
    town_settled_day: u64,
    town_produced: Quantity,
    town_sales: Quantity,
    electricity_quote_status: Option<mycity_core::exchange::Status>,
}

fn shared_supply_snapshot(store: &Store) -> SharedSupplySnapshot {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    let town = view.town.unwrap();
    let quote_status = view.exchange.and_then(|exchange| {
        exchange
            .quotes
            .iter()
            .find(|quote| quote.spec.id == 6)
            .map(|quote| quote.status)
    });
    SharedSupplySnapshot {
        store: snapshot(store),
        town_settled_day: town.state.settled_day,
        town_produced: town.state.last_summary.produced,
        town_sales: town.state.last_summary.actual_sales,
        electricity_quote_status: quote_status,
    }
}

fn shared_supply_unchanged_except_receipt(
    before: &SharedSupplySnapshot,
    after: &SharedSupplySnapshot,
) {
    let mut expected = before.clone();
    expected.store.sequence = after.store.sequence;
    assert_eq!(*after, expected);
    assert_eq!(after.store.sequence, before.store.sequence + 1);
}

fn snapshot_cash(snapshot: &SharedSupplySnapshot, account: u32) -> i64 {
    snapshot
        .store
        .cash
        .iter()
        .find(|row| row.0 == account)
        .unwrap()
        .1
}

fn snapshot_cash_total(snapshot: &SharedSupplySnapshot) -> i128 {
    snapshot
        .store
        .cash
        .iter()
        .map(|row| i128::from(row.1))
        .sum()
}

fn create_with_electricity_quote(
    path: &Path,
    electricity: i64,
    water: i64,
    quote_quantity: i64,
) -> Store {
    let mut value: Value = serde_json::from_slice(&opening(electricity, water)).unwrap();
    let quote = value["exchange"]["quotes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|quote| quote["id"] == json!(6))
        .unwrap();
    // Quote 6 expires at 190 seconds, so dispatch it before the timeline starts.
    quote["quantity"] = json!(quote_quantity);
    Store::create_spatial(
        path,
        &serde_json::to_vec(&value).unwrap(),
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}

fn start_town_day(store: &mut Store) {
    execute(
        store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    execute(
        store,
        Caller::System,
        Action::Timeline {
            command: T::AddPayroll {
                spec: mycity_core::timeline::PayrollSpec {
                    id: 1,
                    payer: OWNER,
                    payee: 5,
                    home_city: 1,
                    hourly_cents: Money::new(10).unwrap(),
                    cycle: mycity_core::timeline::Cycle::Daily,
                    ends_at: 7 * 86_400,
                },
            },
        },
    );
    advance_to(store, 9 * 3_600);
    prepare_construction(store);
    advance_to(store, 86_400);
}

fn work_at_first_day_close(store: &Store, operation_id: u64) -> Envelope {
    work_request_with_seconds(store, operation_id, 86_400, 86_400 - 9 * 3_600, 3_600)
}

fn expected_town_output_from_shared_inventory(store: &Store) -> Quantity {
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    let town = view.town.unwrap();
    let recipe = town.profile.recipes.as_slice()[0];
    let electricity = view
        .stocks
        .iter()
        .find(|stock| stock.id == ELECTRICITY)
        .unwrap()
        .quantity
        .value();
    let raw = view
        .stocks
        .iter()
        .find(|stock| stock.id == 15)
        .unwrap()
        .quantity
        .value();
    let binding = town.profile.utility_supply.unwrap();
    let electricity_per_output = recipe
        .utility_per_output_q
        .value()
        .checked_mul(i64::from(binding.stock_quantity_per_utility_unit))
        .unwrap();
    let electricity_capacity = electricity / electricity_per_output;
    let raw_capacity = raw / recipe.input_per_output.value();
    let worker_capacity = recipe.output_per_worker_day.value() * i64::from(recipe.assigned_workers);
    let market_demand: i64 = town
        .profile
        .households
        .as_slice()
        .iter()
        .filter(|household| household.need_good == recipe.output_good)
        .map(|household| i64::from(household.households) * household.need_per_household.value())
        .sum();
    Quantity::new(
        electricity_capacity
            .min(raw_capacity)
            .min(worker_capacity)
            .min(market_demand),
    )
    .unwrap()
}

fn town_settle_request(store: &Store) -> Envelope {
    envelope(
        store,
        next_operation(),
        Action::TownSettle { profile_id: 1 },
    )
}

#[test]
fn construction_utilities_store_work_replays_coldopens_and_pause_does_not_reset_meter() {
    let temp = Temp::new();
    let path = temp.world("normal");
    let mut store = create(&path, 100_000, 20_000);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);

    let first = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let outcome = store
        .execute(Caller::Account(CONTRACTOR), first.clone())
        .unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
    let after_first = snapshot(&store);
    assert_eq!(after_first.project.utility_worker_seconds, 3_600);
    assert_eq!(
        after_first.project.utility_consumed,
        [Quantity::new(1_000).unwrap(); 2]
    );
    assert_eq!(
        after_first.project.utility_charged,
        [Money::new(20).unwrap(), Money::new(10).unwrap()]
    );
    assert_eq!(
        after_first.boot.local_construction_exported,
        [Quantity::new(1_000).unwrap(); 2]
    );

    let duplicate = store.execute(Caller::Account(CONTRACTOR), first).unwrap();
    assert!(duplicate.model.is_ok());
    assert!(duplicate.has_persistent_receipt);
    assert_eq!(snapshot(&store), after_first);

    construction(&mut store, Caller::Account(CONTRACTOR), C::Pause);
    advance_to(&mut store, 11 * 3_600);
    construction(&mut store, Caller::Account(CONTRACTOR), C::Resume);
    advance_to(&mut store, 12 * 3_600);
    let second = work_request(&store, next_operation(), 12 * 3_600, 3_600);
    let outcome = store
        .execute(Caller::Account(CONTRACTOR), second.clone())
        .unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    let complete = snapshot(&store);
    assert_eq!(complete.project.utility_worker_seconds, 7_200);
    assert_eq!(
        complete.project.utility_consumed,
        [Quantity::new(2_000).unwrap(); 2]
    );
    assert_eq!(
        complete.project.utility_charged,
        [Money::new(40).unwrap(), Money::new(20).unwrap()]
    );
    assert_eq!(complete.boot.exports, [Quantity::new(2_000).unwrap(); 2]);
    assert_eq!(
        complete.boot.local_construction_exported,
        complete.boot.exports
    );

    store.checkpoint().unwrap();
    let checkpointed = snapshot(&store);
    drop(store);
    let mut reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened), checkpointed);
    let replay = reopened
        .execute(Caller::Account(CONTRACTOR), second)
        .unwrap();
    assert!(replay.model.is_ok());
    assert_eq!(snapshot(&reopened), checkpointed);
    reopened.checkpoint().unwrap();
    let after_checkpoint = snapshot(&reopened);
    drop(reopened);
    let reopened_again = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened_again), after_checkpoint);
}

#[test]
fn construction_utility_store_prepared_failure_is_unchanged_and_retryable() {
    let temp = Temp::new();
    let path = temp.world("prepared-failure");
    let mut store = create(&path, 100_000, 20_000);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);
    let request = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let before = snapshot(&store);
    let failure =
        store.execute_with_observer(Caller::Account(CONTRACTOR), request.clone(), |point| {
            if point == WritePoint::Prepared {
                Err(io::Error::other("injected before durable write"))
            } else {
                Ok(())
            }
        });
    assert!(failure.is_err());
    drop(store);

    let mut reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened), before);
    let outcome = reopened
        .execute(Caller::Account(CONTRACTOR), request)
        .unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
    let after = snapshot(&reopened);
    assert_eq!(after.project.utility_worker_seconds, 3_600);
    assert_eq!(
        after.boot.local_construction_exported,
        [Quantity::new(1_000).unwrap(); 2]
    );
}

#[test]
fn construction_utility_store_applied_lost_reply_reopens_and_replays_once() {
    let temp = Temp::new();
    let path = temp.world("applied-failure");
    let mut store = create(&path, 100_000, 20_000);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);
    let request = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let failure =
        store.execute_with_observer(Caller::Account(CONTRACTOR), request.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other("injected after commit, before reply"))
            } else {
                Ok(())
            }
        });
    assert!(failure.is_err());
    drop(store);

    let mut reopened = Store::open(&path, RULES).unwrap();
    let committed = snapshot(&reopened);
    assert_eq!(committed.project.utility_worker_seconds, 3_600);
    assert_eq!(
        committed.boot.local_construction_exported,
        [Quantity::new(1_000).unwrap(); 2]
    );
    let replay = reopened
        .execute(Caller::Account(CONTRACTOR), request)
        .unwrap();
    assert!(replay.model.is_ok());
    assert!(replay.has_persistent_receipt);
    assert_eq!(snapshot(&reopened), committed);
    reopened.checkpoint().unwrap();
    let checkpointed = snapshot(&reopened);
    drop(reopened);
    let cold = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&cold), checkpointed);
}

#[test]
fn construction_utility_store_shortage_keeps_old_failure_and_new_id_works_after_boot_refill() {
    let temp = Temp::new();
    let path = temp.world("utility-refill");
    let mut store = create(&path, 100_000, 0);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);

    // Electricity exists but water does not. The single Work transaction must roll
    // back its earlier electricity export when the second utility line fails.
    let failed_request = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let before = snapshot(&store);
    let failed = store
        .execute(Caller::Account(CONTRACTOR), failed_request.clone())
        .unwrap();
    assert!(failed.model.is_err());
    assert!(failed.has_persistent_receipt);
    assert_eq!(failed.committed_sequence, before.sequence + 1);
    let after_failure = snapshot(&store);
    work_state_unchanged(&before, &after_failure);

    // Pump uses the same Boot-owned electricity stock, then yields water. A generator
    // consumes the produced water and restores electricity before the fresh Work ID.
    execute(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    );
    advance_to(&mut store, 10 * 3_600 + 60);
    assert!(
        store
            .engine()
            .unwrap()
            .world()
            .view()
            .stocks
            .iter()
            .find(|row| row.id == WATER)
            .unwrap()
            .quantity
            > Quantity::ZERO
    );
    execute(
        &mut store,
        Caller::System,
        Action::Boot {
            command: BootCommand::Start {
                facility: Facility::Generator,
                reverse_request: false,
            },
        },
    );
    advance_to(&mut store, 10 * 3_600 + 120);

    let before_old_replay = snapshot(&store);
    let old_replay = store
        .execute(Caller::Account(CONTRACTOR), failed_request)
        .unwrap();
    assert!(old_replay.model.is_err());
    assert!(old_replay.has_persistent_receipt);
    assert_eq!(snapshot(&store), before_old_replay);

    let retry = work_request(&store, next_operation(), 10 * 3_600 + 120, 3_600 + 120);
    let success = store
        .execute(Caller::Account(CONTRACTOR), retry.clone())
        .unwrap();
    assert!(success.model.is_ok(), "{success:?}");
    let after = snapshot(&store);
    assert_eq!(after.project.utility_worker_seconds, 3_600);
    assert_eq!(
        after.project.utility_consumed,
        [Quantity::new(1_000).unwrap(); 2]
    );
    assert_eq!(
        after.boot.local_construction_exported,
        [Quantity::new(1_000).unwrap(); 2]
    );
    let replay = store.execute(Caller::Account(CONTRACTOR), retry).unwrap();
    assert!(replay.model.is_ok());
    assert_eq!(snapshot(&store), after);
    store.checkpoint().unwrap();
    let checkpointed = snapshot(&store);
    drop(store);
    let cold = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&cold), checkpointed);
}

#[test]
fn construction_utility_store_protection_covers_combined_utilities_and_labor() {
    let temp = Temp::new();
    let path = temp.world("protected-funds");
    let mut store = create(&path, 100_000, 20_000);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);
    let owner_cash = store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == OWNER)
        .unwrap()
        .cash;
    let owner_handle = store.engine().unwrap().world().account_ref(OWNER).unwrap();
    let available = Money::new(3_629).unwrap();
    execute(
        &mut store,
        Caller::System,
        Action::Protect {
            account: owner_handle,
            amount: owner_cash.checked_sub(available).unwrap(),
        },
    );
    let request = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let before = snapshot(&store);
    let failure = store.execute(Caller::Account(CONTRACTOR), request).unwrap();
    assert!(failure.model.is_err());
    assert!(failure.has_persistent_receipt);
    let after = snapshot(&store);
    work_state_unchanged(&before, &after);

    // The combined invoice is $36.30: neither utility nor labor may be committed
    // alone when only $36.29 is spendable.
    execute(
        &mut store,
        Caller::System,
        Action::ReleaseProtection {
            account: owner_handle,
            amount: owner_cash.checked_sub(available).unwrap(),
        },
    );
    let retry = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let success = store.execute(Caller::Account(CONTRACTOR), retry).unwrap();
    assert!(success.model.is_ok(), "{success:?}");
    let after_retry = snapshot(&store);
    assert_eq!(after_retry.project.utility_worker_seconds, 3_600);
    assert_eq!(
        after_retry.boot.local_construction_exported,
        [Quantity::new(1_000).unwrap(); 2]
    );
}

#[test]
fn partial_build_clearance_keeps_material_sink_and_continues_utility_meter_through_cold_replay() {
    let temp = Temp::new();
    let path = temp.world("partial-clearance");
    let mut store = create_lifecycle(&path);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);

    let partial_work = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let result = store
        .execute(Caller::Account(CONTRACTOR), partial_work.clone())
        .unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    let partial = snapshot(&store);
    assert_eq!(
        partial.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        partial.project.material_accounting.unwrap().rows[0].basis,
        Money::new(500).unwrap()
    );
    assert_eq!(partial.project.utility_worker_seconds, 3_600);
    assert_eq!(
        partial.boot.local_construction_exported,
        [Quantity::new(1_000).unwrap(); 2]
    );
    let replay = store
        .execute(Caller::Account(CONTRACTOR), partial_work)
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), partial);

    approve_demolition(&mut store);
    let after_clearance_start = snapshot(&store);
    assert_eq!(after_clearance_start.project.kind, Kind::Demolish);
    assert_eq!(
        after_clearance_start
            .project
            .abort_clearance
            .unwrap()
            .original_consumed[0],
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        after_clearance_start
            .project
            .material_accounting
            .unwrap()
            .rows[0]
            .consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(after_clearance_start.project.material_count, 1);
    advance_to(&mut store, 17 * 3_600);

    let clearance_work =
        work_request_with_seconds(&store, next_operation(), 17 * 3_600, 7 * 3_600, 5_400);
    let lost_reply = store.execute_with_observer(
        Caller::Account(CONTRACTOR),
        clearance_work.clone(),
        |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other("clearance committed before lost response"))
            } else {
                Ok(())
            }
        },
    );
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let cleared = snapshot(&store);
    assert_eq!(cleared.project.utility_worker_seconds, 9_000);
    assert_eq!(
        cleared.project.utility_consumed,
        [Quantity::new(2_500).unwrap(); 2]
    );
    assert_eq!(
        cleared.boot.local_construction_exported,
        [Quantity::new(2_500).unwrap(); 2]
    );
    assert_eq!(
        cleared.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        cleared.project.material_accounting.unwrap().rows[0].basis,
        Money::new(500).unwrap()
    );
    let replay = store
        .execute(Caller::Account(CONTRACTOR), clearance_work)
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), cleared);
    store.checkpoint().unwrap();
    let checkpointed = snapshot(&store);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&store), checkpointed);

    let owner_before_waste = store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == OWNER)
        .unwrap()
        .cash;
    construction(&mut store, Caller::Account(OPERATOR), C::AcceptWaste);
    let owner_after_waste = store
        .engine()
        .unwrap()
        .world()
        .view()
        .accounts
        .iter()
        .find(|row| row.id == OWNER)
        .unwrap()
        .cash;
    assert_eq!(
        owner_before_waste.checked_sub(owner_after_waste).unwrap(),
        Money::new(250).unwrap()
    );
    construction(&mut store, Caller::Account(OWNER), C::Close);
    let closed = snapshot(&store);
    assert_eq!(
        closed.project.phase,
        mycity_core::construction::Phase::Closed
    );
    assert_eq!(
        closed.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        closed.boot.local_construction_exported,
        [Quantity::new(2_500).unwrap(); 2]
    );
    store.checkpoint().unwrap();
    drop(store);
    let reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened), closed);
}

#[test]
fn published_build_demolition_preserves_full_material_sink_and_cumulative_utility_usage() {
    let temp = Temp::new();
    let path = temp.world("operational-demolition");
    let mut store = create_lifecycle(&path);
    setup(&mut store);
    advance_to(&mut store, 10 * 3_600);
    let first_work = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let result = store
        .execute(Caller::Account(CONTRACTOR), first_work)
        .unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    advance_to(&mut store, 11 * 3_600);
    let final_build_work = work_request(&store, next_operation(), 11 * 3_600, 3_600);
    let result = store
        .execute(Caller::Account(CONTRACTOR), final_build_work.clone())
        .unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    let built = snapshot(&store);
    assert_eq!(
        built.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        built.project.material_accounting.unwrap().rows[0].basis,
        Money::new(1_000).unwrap()
    );
    assert_eq!(built.project.utility_worker_seconds, 7_200);
    assert_eq!(
        built.project.utility_consumed,
        [Quantity::new(2_000).unwrap(); 2]
    );
    assert_eq!(
        built.project.utility_charged,
        [Money::new(40).unwrap(), Money::new(20).unwrap()]
    );
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&store), built);
    assert!(store
        .execute(Caller::Account(CONTRACTOR), final_build_work)
        .unwrap()
        .model
        .is_ok());
    assert_eq!(snapshot(&store), built);

    construction(&mut store, Caller::Account(CONTRACTOR), C::SubmitInspection);
    construction(
        &mut store,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
    );
    let operational = snapshot(&store);
    assert!(operational.project.operating_published);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|r| r.id == 81)
            .unwrap()
            .capacity,
        4
    );

    approve_demolition(&mut store);
    let demolition = snapshot(&store);
    assert_eq!(
        demolition.project.phase,
        mycity_core::construction::Phase::Demolition
    );
    assert!(!demolition.project.operating_published);
    assert_eq!(demolition.project.material_count, 0);
    assert_eq!(
        demolition.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|r| r.id == 81)
            .unwrap()
            .capacity,
        2
    );

    advance_to(&mut store, 12 * 3_600);
    let demolition_work_1 = work_request(&store, next_operation(), 12 * 3_600, 3_600);
    let result = store
        .execute(Caller::Account(CONTRACTOR), demolition_work_1)
        .unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    advance_to(&mut store, 13 * 3_600);
    let demolition_work_2 = work_request(&store, next_operation(), 13 * 3_600, 3_600);
    let lost_reply = store.execute_with_observer(
        Caller::Account(CONTRACTOR),
        demolition_work_2.clone(),
        |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other(
                    "demolition work committed before lost response",
                ))
            } else {
                Ok(())
            }
        },
    );
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let demolished = snapshot(&store);
    assert_eq!(demolished.project.utility_worker_seconds, 14_400);
    assert_eq!(
        demolished.project.utility_consumed,
        [Quantity::new(4_000).unwrap(); 2]
    );
    assert_eq!(
        demolished.project.utility_charged,
        [Money::new(80).unwrap(), Money::new(40).unwrap()]
    );
    assert_eq!(
        demolished.boot.local_construction_exported,
        [Quantity::new(4_000).unwrap(); 2]
    );
    assert_eq!(
        demolished.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        demolished.project.material_accounting.unwrap().rows[0].basis,
        Money::new(1_000).unwrap()
    );
    let replay = store
        .execute(Caller::Account(CONTRACTOR), demolition_work_2)
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), demolished);
    store.checkpoint().unwrap();
    let checkpointed = snapshot(&store);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&store), checkpointed);

    let waste_id = next_operation();
    let waste = envelope(
        &store,
        waste_id,
        Action::Construction {
            command: C::AcceptWaste,
        },
    );
    let lost_waste_reply =
        store.execute_with_observer(Caller::Account(OPERATOR), waste.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other(
                    "demolition waste committed before lost response",
                ))
            } else {
                Ok(())
            }
        });
    assert!(lost_waste_reply.is_err());
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let after_waste = snapshot(&store);
    let replay = store.execute(Caller::Account(OPERATOR), waste).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), after_waste);
    construction(&mut store, Caller::Account(OWNER), C::Close);
    let closed = snapshot(&store);
    assert_eq!(
        closed.project.phase,
        mycity_core::construction::Phase::Closed
    );
    assert_eq!(
        closed.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        closed.boot.local_construction_exported,
        [Quantity::new(4_000).unwrap(); 2]
    );
    store.checkpoint().unwrap();
    drop(store);
    let reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened), closed);
}

#[test]
fn construction_shared_exchange_raw_sink_is_durable_without_duplicate_material_payment() {
    let temp = Temp::new();
    let path = temp.world("shared-raw");
    let mut value: Value = serde_json::from_slice(&opening(100_000, 100_000)).unwrap();
    let mut project: Project = serde_json::from_value(value["construction"].take()).unwrap();
    project.owner = 4;
    project.contractor = 2;
    project.gates[0].party = 4;
    project.planned_worker_seconds = 28_800;
    project.materials[0].stock_id = 7;
    project.materials[0].basis_per_unit = Money::new(400).unwrap();
    project.material_accounting = None;
    project.enable_material_accounting().unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    let mut store = Store::create_spatial(
        &path,
        &serde_json::to_vec(&value).unwrap(),
        resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap();
    execute(
        &mut store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance_to(&mut store, 9 * 3_600);
    for (actor, command) in [
        (4, C::File),
        (
            4,
            C::DecideGate {
                gate_id: 1,
                decision: GateDecision::Accepted,
                evidence_id: 7001,
            },
        ),
        (
            1,
            C::DecideGate {
                gate_id: 2,
                decision: GateDecision::Accepted,
                evidence_id: 7002,
            },
        ),
        (4, C::Prepare),
        (4, C::Start),
    ] {
        construction(&mut store, Caller::Account(actor), command);
    }
    advance_to(&mut store, 10 * 3_600);
    let before = snapshot(&store);
    let request = work_request(&store, next_operation(), 10 * 3_600, 3_600);
    let result = store.execute(Caller::Account(2), request.clone()).unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    let after = snapshot(&store);
    let row = after.project.material_accounting.unwrap().rows[0];
    assert_eq!(row.stock_id, 7);
    assert_eq!(row.consumed, Quantity::new(125).unwrap());
    assert_eq!(row.basis, Money::new(50).unwrap());
    assert_eq!(after.project.spent, Money::new(3_680).unwrap());
    let owner_cash = |snapshot: &Snapshot| snapshot.cash.iter().find(|x| x.0 == 4).unwrap().1;
    assert_eq!(owner_cash(&before) - owner_cash(&after), 3_630);
    let stock = after.stocks.iter().find(|x| x.0 == 7).unwrap();
    assert_eq!((stock.1, stock.2), (4_875, 1_950));
    // Both WAL replay before checkpoint and checkpoint cold-open preserve the
    // unique Project sink; the consumed material basis is not another cash fee.
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&store), after);
    assert!(store
        .execute(Caller::Account(2), request.clone())
        .unwrap()
        .model
        .is_ok());
    assert_eq!(snapshot(&store), after);
    store.checkpoint().unwrap();
    let checkpointed = snapshot(&store);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&store), checkpointed);
    assert!(store
        .execute(Caller::Account(2), request)
        .unwrap()
        .model
        .is_ok());
    assert_eq!(snapshot(&store), checkpointed);
}

#[test]
fn shared_boot_electricity_construction_work_before_town_market_uses_one_inventory_pool() {
    let temp = Temp::new();
    let path = temp.world("construction-before-town");
    let mut store = create(&path, 100_000, 20_000);
    start_town_day(&mut store);
    let before_work = shared_supply_snapshot(&store);
    let work = work_at_first_day_close(&store, next_operation());
    let lost_reply =
        store.execute_with_observer(Caller::Account(CONTRACTOR), work.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other(
                    "construction Work committed before lost reply",
                ))
            } else {
                Ok(())
            }
        });
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let after_work = shared_supply_snapshot(&store);
    assert_eq!(after_work.store.project.utility_worker_seconds, 3_600);
    assert_eq!(after_work.store.project.spent, Money::new(4_130).unwrap());
    assert_eq!(
        after_work.store.project.utility_consumed[0],
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        after_work.store.boot.local_construction_exported[0],
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        after_work
            .store
            .stocks
            .iter()
            .find(|row| row.0 == ELECTRICITY)
            .unwrap()
            .1,
        99_000
    );
    assert_eq!(
        after_work
            .store
            .stocks
            .iter()
            .find(|row| row.0 == 15)
            .unwrap()
            .1,
        500
    );
    let raw_stock = after_work
        .store
        .stocks
        .iter()
        .find(|row| row.0 == 15)
        .unwrap();
    assert_eq!(
        raw_stock.2, 500,
        "raw basis comes from source stock 15's 1000/1000 lot"
    );
    let material_sink = after_work.store.project.material_accounting.unwrap().rows[0];
    assert_eq!(material_sink.stock_id, 15);
    assert_eq!(material_sink.consumed, Quantity::new(500).unwrap());
    assert_eq!(material_sink.basis, Money::new(500).unwrap());
    assert_eq!(
        snapshot_cash(&before_work, OWNER) - snapshot_cash(&after_work, OWNER),
        3_630
    );
    assert_eq!(
        snapshot_cash(&after_work, CONTRACTOR) - snapshot_cash(&before_work, CONTRACTOR),
        3_600
    );
    assert_eq!(
        snapshot_cash(&after_work, OPERATOR) - snapshot_cash(&before_work, OPERATOR),
        30
    );
    assert_eq!(
        snapshot_cash_total(&after_work),
        snapshot_cash_total(&before_work)
    );
    let replay = store.execute(Caller::Account(CONTRACTOR), work).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(shared_supply_snapshot(&store), after_work);

    let expected_output = expected_town_output_from_shared_inventory(&store);
    assert_eq!(expected_output, Quantity::new(99).unwrap());
    let settlement = town_settle_request(&store);
    let before_town = shared_supply_snapshot(&store);
    let result = store.execute(Caller::System, settlement.clone()).unwrap();
    let summary = result.model.unwrap().town.unwrap();
    assert_eq!(summary.produced, expected_output);
    assert_eq!(summary.actual_sales, expected_output);
    let after_town = shared_supply_snapshot(&store);
    assert_eq!(after_town.town_settled_day, 1);
    assert_eq!(
        after_town.store.boot.exports[0],
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(
        after_town.store.boot.local_construction_exported[0],
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        after_town.store.boot.local_electricity_exported,
        Quantity::new(99_000).unwrap()
    );
    assert_eq!(
        after_town
            .store
            .stocks
            .iter()
            .find(|row| row.0 == ELECTRICITY)
            .unwrap()
            .1,
        0
    );
    assert_eq!(
        after_town
            .store
            .stocks
            .iter()
            .find(|row| row.0 == 15)
            .unwrap()
            .1,
        401
    );
    let remaining_raw = after_town
        .store
        .stocks
        .iter()
        .find(|row| row.0 == 15)
        .unwrap();
    assert_eq!((remaining_raw.1, remaining_raw.2), (401, 401));
    assert_eq!(summary.goods_payments, Money::new(99).unwrap());
    assert_eq!(summary.utilities, Money::new(99).unwrap());
    assert_eq!(
        snapshot_cash(&after_town, OWNER) - snapshot_cash(&before_town, OWNER),
        0
    );
    assert_eq!(
        snapshot_cash(&after_town, 5) - snapshot_cash(&before_town, 5),
        -99
    );
    assert_eq!(
        snapshot_cash(&after_town, OPERATOR) - snapshot_cash(&before_town, OPERATOR),
        99
    );
    assert_eq!(
        snapshot_cash_total(&after_town),
        snapshot_cash_total(&before_town)
    );

    store.checkpoint().unwrap();
    let checkpointed = shared_supply_snapshot(&store);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(shared_supply_snapshot(&store), checkpointed);
    let replay = store.execute(Caller::System, settlement).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(shared_supply_snapshot(&store), checkpointed);
}

#[test]
fn shared_boot_electricity_town_market_before_construction_leaves_no_work_supply() {
    let temp = Temp::new();
    let path = temp.world("town-before-construction");
    let mut store = create(&path, 100_000, 20_000);
    start_town_day(&mut store);

    let expected_output = expected_town_output_from_shared_inventory(&store);
    assert_eq!(expected_output, Quantity::new(100).unwrap());
    let settlement = town_settle_request(&store);
    let before_town = shared_supply_snapshot(&store);
    let lost_reply = store.execute_with_observer(Caller::System, settlement.clone(), |point| {
        if point == WritePoint::Applied {
            Err(io::Error::other("TownSettle committed before lost reply"))
        } else {
            Ok(())
        }
    });
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let after_town = shared_supply_snapshot(&store);
    assert_eq!(after_town.town_settled_day, 1);
    assert_eq!(after_town.town_produced, expected_output);
    assert_eq!(after_town.town_sales, expected_output);
    assert_eq!(
        snapshot_cash_total(&after_town),
        snapshot_cash_total(&before_town)
    );
    assert_eq!(
        snapshot_cash(&after_town, OWNER) - snapshot_cash(&before_town, OWNER),
        0
    );
    assert_eq!(
        snapshot_cash(&after_town, 5) - snapshot_cash(&before_town, 5),
        -100
    );
    assert_eq!(
        snapshot_cash(&after_town, OPERATOR) - snapshot_cash(&before_town, OPERATOR),
        100
    );
    assert_eq!(
        after_town.store.boot.local_electricity_exported,
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(
        after_town
            .store
            .stocks
            .iter()
            .find(|row| row.0 == ELECTRICITY)
            .unwrap()
            .1,
        0
    );
    assert_eq!(
        after_town
            .store
            .stocks
            .iter()
            .find(|row| row.0 == 15)
            .unwrap()
            .1,
        900
    );
    let remaining_raw = after_town
        .store
        .stocks
        .iter()
        .find(|row| row.0 == 15)
        .unwrap();
    assert_eq!((remaining_raw.1, remaining_raw.2), (900, 900));
    let replay = store.execute(Caller::System, settlement).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(shared_supply_snapshot(&store), after_town);
    store.checkpoint().unwrap();
    let checkpointed = shared_supply_snapshot(&store);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(shared_supply_snapshot(&store), checkpointed);

    let before_work = shared_supply_snapshot(&store);
    let failed_work = work_at_first_day_close(&store, next_operation());
    let failure = store
        .execute(Caller::Account(CONTRACTOR), failed_work.clone())
        .unwrap();
    assert!(failure.model.is_err());
    assert!(failure.has_persistent_receipt);
    let after_failure = shared_supply_snapshot(&store);
    shared_supply_unchanged_except_receipt(&before_work, &after_failure);
    let committed_failure_sequence = after_failure.store.sequence;
    store.checkpoint().unwrap();
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let recovered_failure = shared_supply_snapshot(&store);
    assert_eq!(recovered_failure, after_failure);
    let replay = store
        .execute(Caller::Account(CONTRACTOR), failed_work)
        .unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_err());
    assert_eq!(store.committed_sequence(), committed_failure_sequence);
    assert_eq!(shared_supply_snapshot(&store), recovered_failure);
}

#[test]
fn shared_boot_electricity_cross_city_dispatch_then_work_and_town_share_source_stock() {
    let temp = Temp::new();
    let path = temp.world("cross-city-before-construction-and-town");
    let mut store = create_with_electricity_quote(&path, 100_000, 20_000, 40_000);

    // This uses the fixture's real electricity quote and configured edges 3/4.
    // Town delivery still uses fixture route_source=77 as a routing substitute;
    // this test is not evidence of live road assignment or transport coupling.
    let before_dispatch = shared_supply_snapshot(&store);
    let dispatch = envelope(
        &store,
        next_operation(),
        Action::Exchange {
            command: X::Dispatch { quote: 6 },
        },
    );
    let lost_reply = store.execute_with_observer(Caller::System, dispatch.clone(), |point| {
        if point == WritePoint::Applied {
            Err(io::Error::other(
                "cross-city dispatch committed before lost reply",
            ))
        } else {
            Ok(())
        }
    });
    assert!(lost_reply.is_err());
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    let after_dispatch = shared_supply_snapshot(&store);
    assert_eq!(
        after_dispatch.electricity_quote_status,
        Some(mycity_core::exchange::Status::Transit)
    );
    assert_eq!(
        after_dispatch.store.boot.exports[0],
        Quantity::new(40_000).unwrap()
    );
    assert_eq!(
        after_dispatch.store.boot.local_electricity_exported,
        Quantity::ZERO
    );
    assert_eq!(
        after_dispatch
            .store
            .stocks
            .iter()
            .find(|row| row.0 == ELECTRICITY)
            .unwrap()
            .1,
        60_000
    );
    assert_eq!(
        snapshot_cash_total(&after_dispatch),
        snapshot_cash_total(&before_dispatch)
    );
    let replay = store.execute(Caller::System, dispatch).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(shared_supply_snapshot(&store), after_dispatch);

    start_town_day(&mut store);
    let before_work = shared_supply_snapshot(&store);
    let work = work_at_first_day_close(&store, next_operation());
    let result = store
        .execute(Caller::Account(CONTRACTOR), work.clone())
        .unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    let after_work = shared_supply_snapshot(&store);
    assert_eq!(after_work.store.project.utility_worker_seconds, 3_600);
    assert_eq!(after_work.store.project.spent, Money::new(4_130).unwrap());
    assert_eq!(
        after_work.store.boot.local_construction_exported[0],
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        after_work
            .store
            .stocks
            .iter()
            .find(|row| row.0 == ELECTRICITY)
            .unwrap()
            .1,
        59_000
    );
    assert_eq!(
        after_work
            .store
            .stocks
            .iter()
            .find(|row| row.0 == 15)
            .unwrap()
            .1,
        500
    );
    let raw_stock = after_work
        .store
        .stocks
        .iter()
        .find(|row| row.0 == 15)
        .unwrap();
    assert_eq!(
        raw_stock.2, 500,
        "raw basis comes from source stock 15's 1000/1000 lot"
    );
    let material_sink = after_work.store.project.material_accounting.unwrap().rows[0];
    assert_eq!(material_sink.stock_id, 15);
    assert_eq!(material_sink.consumed, Quantity::new(500).unwrap());
    assert_eq!(material_sink.basis, Money::new(500).unwrap());
    assert_eq!(
        snapshot_cash(&before_work, OWNER) - snapshot_cash(&after_work, OWNER),
        3_630
    );
    assert_eq!(
        snapshot_cash(&after_work, CONTRACTOR) - snapshot_cash(&before_work, CONTRACTOR),
        3_600
    );
    assert_eq!(
        snapshot_cash(&after_work, OPERATOR) - snapshot_cash(&before_work, OPERATOR),
        30
    );
    assert_eq!(
        snapshot_cash_total(&after_work),
        snapshot_cash_total(&before_work)
    );
    let replay = store.execute(Caller::Account(CONTRACTOR), work).unwrap();
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(shared_supply_snapshot(&store), after_work);

    let expected_output = expected_town_output_from_shared_inventory(&store);
    assert_eq!(expected_output, Quantity::new(59).unwrap());
    let settlement = town_settle_request(&store);
    let before_town = shared_supply_snapshot(&store);
    let result = store.execute(Caller::System, settlement.clone()).unwrap();
    let summary = result.model.unwrap().town.unwrap();
    assert_eq!(summary.produced, expected_output);
    assert_eq!(summary.actual_sales, expected_output);
    let after_town = shared_supply_snapshot(&store);
    assert_eq!(after_town.town_settled_day, 1);
    assert_eq!(
        after_town.store.boot.exports[0],
        Quantity::new(100_000).unwrap()
    );
    assert_eq!(
        after_town.store.boot.local_electricity_exported,
        Quantity::new(59_000).unwrap()
    );
    assert_eq!(
        after_town.store.boot.local_construction_exported[0],
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        after_town
            .store
            .stocks
            .iter()
            .find(|row| row.0 == ELECTRICITY)
            .unwrap()
            .1,
        0
    );
    assert_eq!(
        after_town
            .store
            .stocks
            .iter()
            .find(|row| row.0 == 15)
            .unwrap()
            .1,
        441
    );
    let remaining_raw = after_town
        .store
        .stocks
        .iter()
        .find(|row| row.0 == 15)
        .unwrap();
    assert_eq!((remaining_raw.1, remaining_raw.2), (441, 441));
    assert_eq!(summary.goods_payments, Money::new(59).unwrap());
    assert_eq!(summary.utilities, Money::new(59).unwrap());
    assert_eq!(
        snapshot_cash(&after_town, OWNER) - snapshot_cash(&before_town, OWNER),
        0
    );
    assert_eq!(
        snapshot_cash(&after_town, 5) - snapshot_cash(&before_town, 5),
        -59
    );
    assert_eq!(
        snapshot_cash(&after_town, OPERATOR) - snapshot_cash(&before_town, OPERATOR),
        59
    );
    assert_eq!(
        snapshot_cash_total(&after_town),
        snapshot_cash_total(&before_town)
    );
    store.checkpoint().unwrap();
    let checkpointed = shared_supply_snapshot(&store);
    drop(store);

    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(shared_supply_snapshot(&store), checkpointed);
    let replay = store.execute(Caller::System, settlement).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(shared_supply_snapshot(&store), checkpointed);
}

fn shared_raw_fixture_totals() -> (i128, i128) {
    let value: Value = serde_json::from_slice(OPENING).unwrap();
    value["stocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|stock| {
            let id = stock["id"].as_u64().unwrap();
            (7..=12).contains(&id)
        })
        .fold((0_i128, 0_i128), |(quantity, basis), stock| {
            (
                quantity + i128::from(stock["quantity"].as_i64().unwrap()),
                basis + i128::from(stock["basis"].as_i64().unwrap()),
            )
        })
}

fn fixture_raw7_basis_for(consumed: i64) -> Money {
    let value: Value = serde_json::from_slice(OPENING).unwrap();
    let lot = value["stocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stock| stock["id"] == json!(7))
        .unwrap();
    let quantity = i128::from(lot["quantity"].as_i64().unwrap());
    let basis = i128::from(lot["basis"].as_i64().unwrap());
    let numerator = basis * i128::from(consumed);
    assert_eq!(
        numerator % quantity,
        0,
        "fixture lot basis must divide exactly"
    );
    Money::new(i64::try_from(numerator / quantity).unwrap()).unwrap()
}

/// Independently re-sums raw stocks, in-flight raw Exchange fragments, and
/// Project's permanent material sink against the immutable fixture lot. Store
/// validation also enforces this invariant at every commit and restore.
fn assert_exchange_raw_conservation(store: &Store) {
    let initial = shared_raw_fixture_totals();
    let engine = store.engine().unwrap();
    let view = engine.world().view();
    let exchange = view.exchange.unwrap();
    assert_eq!(i128::from(exchange.initial_raw.value()), initial.0);

    let mut quantity = 0_i128;
    let mut basis = 0_i128;
    for stock in view
        .stocks
        .iter()
        .filter(|stock| (7..=12).contains(&stock.id))
    {
        quantity += i128::from(stock.quantity.value());
        basis += i128::from(stock.basis.value());
    }
    for shipment in exchange.shipments.iter().flatten() {
        let quote = exchange.quotes[shipment.quote_index];
        if quote.spec.kind == mycity_core::exchange::Kind::Freight
            && (7..=12).contains(&quote.spec.source)
        {
            quantity += i128::from(shipment.fragment.quantity.value());
            basis += i128::from(shipment.fragment.basis.value());
        }
    }
    if let Some(accounting) = view
        .construction
        .and_then(|project| project.material_accounting)
    {
        for row in accounting
            .rows
            .iter()
            .take(usize::from(accounting.row_count))
        {
            if (7..=12).contains(&row.stock_id) {
                quantity += i128::from(row.consumed.value());
                basis += i128::from(row.basis.value());
            }
        }
    }
    assert_eq!(quantity, initial.0, "Exchange raw quantity conservation");
    assert_eq!(basis, initial.1, "Exchange raw basis conservation");
}

fn shared_raw_lifecycle_opening() -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&opening(100_000, 100_000)).unwrap();
    let mut project: Project = serde_json::from_value(value["construction"].clone()).unwrap();
    let source_lot = value["stocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stock| stock["id"] == json!(7))
        .unwrap();
    let raw_quantity = source_lot["quantity"].as_i64().unwrap();
    let raw_basis = source_lot["basis"].as_i64().unwrap();
    let required = project.materials[0].required.value();
    let material_basis = i128::from(raw_basis) * i128::from(required) / i128::from(raw_quantity);
    assert_eq!(
        i128::from(raw_basis) * i128::from(required) % i128::from(raw_quantity),
        0,
        "fixture raw lot must produce an exact Project material estimate"
    );

    project.owner = 4;
    project.contractor = 2;
    project.gates[0].party = 4;
    project.planned_worker_seconds = 28_800;
    project.materials[0].stock_id = 7;
    project.materials[0].basis_per_unit =
        Money::new(i64::try_from(material_basis).unwrap()).unwrap();
    project.material_accounting = None;
    project.enable_material_accounting().unwrap();
    project.operating_resource = 81;
    project.operating_base_capacity = 2;
    project.operating_units = 2;
    project.waste_receiver_account = OPERATOR;
    project.waste_receiver_resource = 82;
    project.waste_units = 2;
    project.waste_fee = Money::new(250).unwrap();
    project.abort_clearance_spec = Some(AbortClearanceSpec {
        base_worker_seconds: 3_600,
        material_rules: [
            AbortClearanceMaterialRule {
                stock_id: 7,
                quantity_per_waste_capacity_unit: 500,
                worker_seconds_per_thousand_quantity: 3_600,
            },
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
            AbortClearanceMaterialRule::default(),
        ],
    });
    project.validate_profile().unwrap();
    value["construction"] = serde_json::to_value(project).unwrap();
    serde_json::to_vec(&value).unwrap()
}

fn create_shared_raw_lifecycle(path: &Path) -> Store {
    Store::create_spatial(
        path,
        &shared_raw_lifecycle_opening(),
        lifecycle_resources(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    )
    .unwrap()
}

fn setup_shared_raw_project(store: &mut Store) {
    execute(
        store,
        Caller::System,
        Action::Timeline { command: T::Enable },
    );
    advance_to(store, 9 * 3_600);
    construction(store, Caller::Account(4), C::File);
    construction(
        store,
        Caller::Account(4),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 7201,
        },
    );
    construction(
        store,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 7202,
        },
    );
    construction(store, Caller::Account(4), C::Prepare);
    construction(store, Caller::Account(4), C::Start);
    assert_exchange_raw_conservation(store);
}

fn approve_shared_raw_demolition(store: &mut Store) {
    construction(
        store,
        Caller::Account(4),
        C::BeginDemolition {
            waste_receiver: OPERATOR,
        },
    );
    construction(
        store,
        Caller::Account(4),
        C::DecideGate {
            gate_id: 1,
            decision: GateDecision::Accepted,
            evidence_id: 7301,
        },
    );
    construction(
        store,
        Caller::Account(1),
        C::DecideGate {
            gate_id: 2,
            decision: GateDecision::Accepted,
            evidence_id: 7302,
        },
    );
    construction(store, Caller::Account(4), C::Prepare);
    construction(store, Caller::Account(4), C::Start);
    assert_exchange_raw_conservation(store);
}

#[test]
fn exchange_raw7_half_build_clearance_and_close_preserve_raw_sink_through_recovery() {
    let temp = Temp::new();
    temp.retain_recovery_inputs();
    let path = temp.world("exchange-raw7-half-clearance");
    let mut store = create_shared_raw_lifecycle(&path);
    setup_shared_raw_project(&mut store);
    assert_eq!(shared_raw_fixture_totals(), (5_000, 2_000));

    // The fixture's actual stock-7 lot is 5,000 quantity / 2,000 basis.
    // At 28,800 planned worker-seconds each 3,600-second Work consumes 125
    // quantity and the actual lot offset contributes 50 basis.
    for completed_shift in 1..=4_u64 {
        let at = (9 + completed_shift) * 3_600;
        advance_to(&mut store, at);
        let request = work_request(&store, next_operation(), at, 3_600);
        let result = store.execute(Caller::Account(2), request.clone()).unwrap();
        assert!(result.model.is_ok(), "{result:?}");
        let worked = snapshot(&store);
        let consumed = i64::try_from(completed_shift * 125).unwrap();
        let consumed_basis = fixture_raw7_basis_for(consumed);
        let row = worked.project.material_accounting.unwrap().rows[0];
        assert_eq!(row.stock_id, 7);
        assert_eq!(row.consumed, Quantity::new(consumed).unwrap());
        assert_eq!(row.basis, consumed_basis);
        let raw = worked.stocks.iter().find(|stock| stock.0 == 7).unwrap();
        assert_eq!(raw.1, 5_000 - consumed);
        assert_eq!(raw.2, 2_000 - consumed_basis.value());
        assert_exchange_raw_conservation(&store);

        drop(store);
        store = Store::open(&path, RULES).unwrap();
        assert_eq!(snapshot(&store), worked);
        let replay = store.execute(Caller::Account(2), request).unwrap();
        assert!(replay.has_persistent_receipt);
        assert!(replay.model.is_ok(), "{replay:?}");
        assert_eq!(snapshot(&store), worked);
        assert_exchange_raw_conservation(&store);
    }

    let half_built = snapshot(&store);
    assert_eq!(half_built.project.completed_worker_seconds, 14_400);
    assert_eq!(
        half_built.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        half_built.project.material_accounting.unwrap().rows[0].basis,
        fixture_raw7_basis_for(500)
    );
    approve_shared_raw_demolition(&mut store);
    let clearing = snapshot(&store);
    assert_eq!(clearing.project.kind, Kind::Demolish);
    assert!(!clearing.project.operating_published);
    assert_eq!(clearing.project.operating_withdrawn_at, None);
    assert_eq!(clearing.project.abort_clearance.unwrap().waste_units, 1);
    assert_exchange_raw_conservation(&store);

    const CLEARANCE_WORKER_SECONDS: u32 = 5_400;
    const CLEARANCE_AT: u64 = 20 * 3_600;
    advance_to(&mut store, CLEARANCE_AT);
    let clearance = work_request_with_seconds(
        &store,
        next_operation(),
        CLEARANCE_AT,
        7 * 3_600,
        CLEARANCE_WORKER_SECONDS,
    );
    let lost_reply = store.execute_with_observer(Caller::Account(2), clearance.clone(), |point| {
        if point == WritePoint::Applied {
            Err(io::Error::other(
                "raw7 clearance committed before lost reply",
            ))
        } else {
            Ok(())
        }
    });
    assert!(lost_reply.is_err());
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let cleared = snapshot(&store);
    assert_eq!(cleared.project.completed_worker_seconds, 5_400);
    assert_eq!(cleared.project.utility_worker_seconds, 19_800);
    assert_eq!(
        cleared.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        cleared.project.material_accounting.unwrap().rows[0].basis,
        fixture_raw7_basis_for(500)
    );
    assert_exchange_raw_conservation(&store);
    let replay = store.execute(Caller::Account(2), clearance).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), cleared);
    assert_exchange_raw_conservation(&store);
    store.checkpoint().unwrap();
    let checkpointed = snapshot(&store);
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&store), checkpointed);
    assert_exchange_raw_conservation(&store);

    let waste = envelope(
        &store,
        next_operation(),
        Action::Construction {
            command: C::AcceptWaste,
        },
    );
    let waste_lost_reply =
        store.execute_with_observer(Caller::Account(OPERATOR), waste.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other("raw7 waste committed before lost reply"))
            } else {
                Ok(())
            }
        });
    assert!(waste_lost_reply.is_err());
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let after_waste = snapshot(&store);
    assert_eq!(after_waste.project.abort_clearance.unwrap().waste_units, 1);
    assert_exchange_raw_conservation(&store);
    let replay = store.execute(Caller::Account(OPERATOR), waste).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), after_waste);
    assert_exchange_raw_conservation(&store);

    construction(&mut store, Caller::Account(4), C::Close);
    let closed = snapshot(&store);
    assert_eq!(
        closed.project.phase,
        mycity_core::construction::Phase::Closed
    );
    assert_eq!(
        closed.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(500).unwrap()
    );
    assert_eq!(
        closed.project.material_accounting.unwrap().rows[0].basis,
        fixture_raw7_basis_for(500)
    );
    assert_exchange_raw_conservation(&store);
    store.checkpoint().unwrap();
    drop(store);
    let reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened), closed);
    assert_exchange_raw_conservation(&reopened);
}

#[test]
fn exchange_raw7_complete_build_and_operational_demolition_preserve_sink_through_recovery() {
    let temp = Temp::new();
    temp.retain_recovery_inputs();
    let path = temp.world("exchange-raw7-operational-demolition");
    let mut store = create_shared_raw_lifecycle(&path);
    setup_shared_raw_project(&mut store);
    assert_eq!(shared_raw_fixture_totals(), (5_000, 2_000));

    for completed_shift in 1..=8_u64 {
        let at = (9 + completed_shift) * 3_600;
        advance_to(&mut store, at);
        let request = work_request(&store, next_operation(), at, 3_600);
        let result = store.execute(Caller::Account(2), request.clone()).unwrap();
        assert!(result.model.is_ok(), "{result:?}");
        let worked = snapshot(&store);
        let consumed = i64::try_from(completed_shift * 125).unwrap();
        let basis = fixture_raw7_basis_for(consumed);
        let row = worked.project.material_accounting.unwrap().rows[0];
        assert_eq!(
            (row.stock_id, row.consumed, row.basis),
            (7, Quantity::new(consumed).unwrap(), basis)
        );
        let raw = worked.stocks.iter().find(|stock| stock.0 == 7).unwrap();
        assert_eq!((raw.1, raw.2), (5_000 - consumed, 2_000 - basis.value()));
        assert_exchange_raw_conservation(&store);

        if completed_shift == 8 {
            store.checkpoint().unwrap();
            let checkpointed = snapshot(&store);
            drop(store);
            store = Store::open(&path, RULES).unwrap();
            assert_eq!(snapshot(&store), checkpointed);
            assert_exchange_raw_conservation(&store);
        }
        let replay = store.execute(Caller::Account(2), request).unwrap();
        assert!(replay.has_persistent_receipt);
        assert!(replay.model.is_ok(), "{replay:?}");
        assert_eq!(snapshot(&store), worked);
        assert_exchange_raw_conservation(&store);
    }

    let built = snapshot(&store);
    assert_eq!(built.project.completed_worker_seconds, 28_800);
    assert_eq!(
        built.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        built.project.material_accounting.unwrap().rows[0].basis,
        fixture_raw7_basis_for(1_000)
    );
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .unwrap()
            .capacity,
        2
    );
    construction(&mut store, Caller::Account(2), C::SubmitInspection);
    construction(
        &mut store,
        Caller::Account(1),
        C::AcceptInspection {
            inspector: 1,
            accepted: true,
        },
    );
    let operational = snapshot(&store);
    assert!(operational.project.operating_published);
    assert_exchange_raw_conservation(&store);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .unwrap()
            .capacity,
        4
    );

    approve_shared_raw_demolition(&mut store);
    let demolition = snapshot(&store);
    assert!(!demolition.project.operating_published);
    // Only road-edge contributions carry the road withdrawal clock.
    // This fixture withdraws resource 81; verify its real capacity below.
    assert_eq!(demolition.project.operating_edge_id, 0);
    assert!(demolition.project.operating_withdrawn_at.is_none());
    assert_eq!(demolition.project.material_count, 0);
    assert_eq!(
        demolition.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        demolition.project.material_accounting.unwrap().rows[0].basis,
        fixture_raw7_basis_for(1_000)
    );
    assert_exchange_raw_conservation(&store);
    assert_eq!(
        store
            .engine()
            .unwrap()
            .resources()
            .iter()
            .find(|resource| resource.id == 81)
            .unwrap()
            .capacity,
        2
    );

    // Build ends at 17:00. The same crew cannot work outside the actual
    // 09:00-17:00 calendar window, and refusal must not consume anything.
    advance_to(&mut store, 18 * 3_600);
    let before_night = snapshot(&store);
    let night = work_request(&store, next_operation(), 18 * 3_600, 3_600);
    let refused = store.execute(Caller::Account(2), night.clone()).unwrap();
    assert!(matches!(
        refused.model,
        Err(mycity_core::transaction::TxError::Construction(
            mycity_core::construction::Error::WorkLimit
        ))
    ));
    assert!(refused.has_persistent_receipt);
    let after_night = snapshot(&store);
    work_state_unchanged(&before_night, &after_night);
    let replay = store.execute(Caller::Account(2), night).unwrap();
    assert!(replay.model.is_err());
    assert_eq!(snapshot(&store), after_night);
    assert_exchange_raw_conservation(&store);

    for demolition_shift in 1..=8_u64 {
        let at = 86_400 + (9 + demolition_shift) * 3_600;
        let last = snapshot(&store).project.last_work_at.unwrap().value();
        let elapsed = u32::try_from(i64::try_from(at).unwrap() - last).unwrap();
        advance_to(&mut store, at);
        let request = work_request(&store, next_operation(), at, elapsed);
        let result = store.execute(Caller::Account(2), request.clone()).unwrap();
        assert!(result.model.is_ok(), "{result:?}");
        let worked = snapshot(&store);
        assert_eq!(worked.project.material_count, 0);
        assert_eq!(
            worked.project.material_accounting.unwrap().rows[0].consumed,
            Quantity::new(1_000).unwrap()
        );
        assert_eq!(
            worked.project.material_accounting.unwrap().rows[0].basis,
            fixture_raw7_basis_for(1_000)
        );
        assert_eq!(
            worked.project.utility_worker_seconds,
            28_800 + demolition_shift * 3_600
        );
        assert_exchange_raw_conservation(&store);

        if demolition_shift == 8 {
            store.checkpoint().unwrap();
            let checkpointed = snapshot(&store);
            drop(store);
            store = Store::open(&path, RULES).unwrap();
            assert_eq!(snapshot(&store), checkpointed);
            assert_exchange_raw_conservation(&store);
        }
        let replay = store.execute(Caller::Account(2), request).unwrap();
        assert!(replay.has_persistent_receipt);
        assert!(replay.model.is_ok(), "{replay:?}");
        assert_eq!(snapshot(&store), worked);
        assert_exchange_raw_conservation(&store);
    }

    let demolished = snapshot(&store);
    assert_eq!(demolished.project.utility_worker_seconds, 57_600);
    assert_eq!(
        demolished.project.utility_consumed,
        [Quantity::new(16_000).unwrap(); 2]
    );
    assert_eq!(
        demolished.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_exchange_raw_conservation(&store);
    let waste = envelope(
        &store,
        next_operation(),
        Action::Construction {
            command: C::AcceptWaste,
        },
    );
    let lost_reply =
        store.execute_with_observer(Caller::Account(OPERATOR), waste.clone(), |point| {
            if point == WritePoint::Applied {
                Err(io::Error::other(
                    "raw7 demolition waste committed before lost reply",
                ))
            } else {
                Ok(())
            }
        });
    assert!(lost_reply.is_err());
    drop(store);
    let mut store = Store::open(&path, RULES).unwrap();
    let after_waste = snapshot(&store);
    assert_exchange_raw_conservation(&store);
    let replay = store.execute(Caller::Account(OPERATOR), waste).unwrap();
    assert!(replay.has_persistent_receipt);
    assert!(replay.model.is_ok(), "{replay:?}");
    assert_eq!(snapshot(&store), after_waste);
    assert_exchange_raw_conservation(&store);

    construction(&mut store, Caller::Account(4), C::Close);
    let closed = snapshot(&store);
    assert_eq!(
        closed.project.phase,
        mycity_core::construction::Phase::Closed
    );
    assert_eq!(
        closed.project.material_accounting.unwrap().rows[0].consumed,
        Quantity::new(1_000).unwrap()
    );
    assert_eq!(
        closed.project.material_accounting.unwrap().rows[0].basis,
        fixture_raw7_basis_for(1_000)
    );
    assert_exchange_raw_conservation(&store);
    store.checkpoint().unwrap();
    drop(store);
    let reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(snapshot(&reopened), closed);
    assert_exchange_raw_conservation(&reopened);
}
