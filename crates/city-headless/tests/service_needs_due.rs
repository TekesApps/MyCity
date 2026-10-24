use mycity_core::{
    housing::{genesis_lease_offer_sha256, GenesisLeaseOffer, LeaseTerms, lease_terms_sha256},
    schedule::Phase,
    service_needs as needs,
    timeline::Command as TimelineCommand,
    transaction::{Action, Caller, Envelope},
    units::{GameTime, Money},
};
use mycity_headless::{required_dispatch::{self, DueError, SelectionMode, ServiceSelection}, sim_client::Client, sim_host::Command};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::Duration};

const SPEC: &[u8] = include_bytes!("../../../tests/fixtures/issue-101/service-needs-p5-due-v182.json");
const BASE: &[u8] = include_bytes!("../../city-storage/tests/fixtures/education-store/free-offline-v1.json");
const RULES_PREFIX: &str = "sim-w3";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn spec() -> Value { serde_json::from_slice(SPEC).unwrap() }

fn rules() -> String {
    format!("{RULES_PREFIX}:{}", env!("MYCITY_SOURCE_SHA256"))
}

fn evidence_root() -> PathBuf {
    let root = fs::canonicalize(std::env::var_os("MYCITY_ISSUE101_SERVICE_DUE_ROOT")
        .expect("set MYCITY_ISSUE101_SERVICE_DUE_ROOT to an existing retained HP P900 work root"))
        .expect("canonicalize retained service due root");
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"), "unexpected evidence root: {}", root.display());
    root
}

fn retained_case(name: &str) -> (PathBuf, PathBuf) {
    let root = evidence_root();
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let case = root.join(format!("issue101-service-due-{name}-{}-{id}", std::process::id()));
    fs::create_dir(&case).expect("create unique retained case directory");
    let world = case.join("world");
    println!("RETAINED_SERVICE_DUE_CASE={}", case.display());
    (case, world)
}

fn offer(at: u64) -> GenesisLeaseOffer {
    let terms = LeaseTerms {
        lease_id: 901,
        clause_version: 1,
        contract_source_id: 9101,
        landlord_authorization_id: 9201,
        tenant_consent_id: 9301,
        rent_per_cycle: Money::new(1000).unwrap(),
        cycle: mycity_core::timeline::Cycle::Monthly { anchor: 1 },
        deposit: Money::ZERO,
        escrow_account: 3,
        starts_at: GameTime::new(at as i64).unwrap(),
    };
    GenesisLeaseOffer {
        offer_id: 901,
        world_id: 1,
        branch_id: 1,
        unit_id: 71,
        building_id: 70,
        owner_account: 1,
        tenant_account: 2,
        household_id: 7,
        valid_from: GameTime::ZERO,
        valid_until: GameTime::new((400 * needs::DAY) as i64).unwrap(),
        asset_source_id: 7002,
        authority_source_id: 8001,
        contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
        lease_id: terms.lease_id,
        clause_version: terms.clause_version,
        rent_per_cycle: terms.rent_per_cycle,
        cycle: terms.cycle,
        deposit: terms.deposit,
        escrow_account: terms.escrow_account,
        starts_at: terms.starts_at,
        terms_sha256: lease_terms_sha256(terms).unwrap(),
    }
}

fn opening(at: u64) -> Vec<u8> {
    let spec = spec();
    let mut value: Value = serde_json::from_slice(BASE).unwrap();
    value.as_object_mut().unwrap().remove("education");
    value.as_object_mut().unwrap().remove("venue");
    value["bootstrap"]["seed"] = json!(spec["seed"]);
    value["housing"]["city_id"] = json!(spec["city_id"]);
    value["housing"]["lease_offers"] = json!([offer(at)]);
    value["service_needs"] = json!({
        "version": 1,
        "profile": "LocalDevelopmentNaturalNeedsV1",
        "scope": {"world": 1, "branch": 1},
        "seed": spec["seed"],
        "city_id": spec["city_id"],
        "households": [{
            "household_id": spec["household_id"],
            "source_id": spec["household_source_id"],
            "generation": 1,
            "people": 10
        }]
    });
    serde_json::to_vec(&value).unwrap()
}

fn apply(store: &mut Store, caller: Caller, action: Action) -> Envelope {
    let (scope, revision) = store.engine().unwrap().world().scope_revision();
    let envelope = Envelope {
        world: scope.world(),
        branch: scope.branch(),
        operation_id: store.committed_sequence() + 1,
        expected_revision: revision,
        actions: vec![action],
    };
    let outcome = store.execute(caller, envelope.clone()).unwrap();
    assert!(outcome.has_persistent_receipt, "{outcome:?}");
    assert!(outcome.model.is_ok(), "{outcome:?}");
    envelope
}

fn create_store(world: &Path, case: &Path) -> Store {
    let mut spatial: Value = serde_json::from_slice(include_bytes!("../../../fixtures/m0-exchange-spatial.json")).unwrap();
    spatial["bootstrap"]["seed"] = json!(spec()["seed"]);
    let spatial = serde_json::to_vec(&spatial).unwrap();
    fs::write(case.join("opening.json"), opening(0)).unwrap();
    fs::write(case.join("spatial-opening.json"), &spatial).unwrap();
    Store::create_spatial_v2(
        world,
        &opening(0),
        vec![],
        &rules(),
        &spatial,
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    ).unwrap()
}

fn prepare_p5(store: &mut Store) {
    apply(store, Caller::System, Action::Timeline { command: TimelineCommand::Enable });
    let lease = offer(0);
    apply(store, Caller::Account(2), Action::HousingLease {
        offer_id: lease.offer_id,
        offer_sha256: genesis_lease_offer_sha256(lease),
    });
    let target = spec()["target"].as_u64().unwrap();
    let mut reached = false;
    for _ in 0..4096 {
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        if progress.phase == Some(Phase::P5) && progress.at == target {
            assert_eq!(progress.at, target);
            reached = true;
            break;
        }
        let limit = store.continuation_status().unwrap().map_or(1, |c| c.page_limit);
        apply(store, Caller::System, Action::Timeline {
            command: TimelineCommand::Advance { target, limit },
        });
    }
    assert!(reached, "bounded original Timeline never reached P5");
}

fn open_due_world(name: &str) -> (PathBuf, PathBuf, Store) {
    let (case, world) = retained_case(name);
    let mut store = create_store(&world, &case);
    assert!(matches!(
        required_dispatch::select_service_continuation(&store, false).unwrap(),
        ServiceSelection::NoWork
    ));
    prepare_p5(&mut store);
    let progress = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(progress.phase, Some(Phase::P5));
    assert_eq!(progress.last_complete, spec()["window_start"].as_u64().unwrap());
    assert_eq!(store.engine().unwrap().service_needs_due_state().unwrap().unwrap().pending, None);
    (case, world, store)
}

fn selected(selection: ServiceSelection) -> required_dispatch::SelectedServiceContinuation {
    match selection {
        ServiceSelection::Selected(value) => value,
        other => panic!("expected selected service continuation, got {other:?}"),
    }
}

fn result(client: &mut Client, id: u64, command: Command) -> Value {
    client.submit(id, command).unwrap();
    for _ in 0..16 {
        let event = client.next(Duration::from_secs(15)).unwrap();
        assert_eq!(event.id, id);
        if event.kind == "result" { return event.payload; }
        assert_ne!(event.kind, "service_continuation_selected", "unexpected internal service selection");
    }
    panic!("bounded Sim result missing")
}

fn sim(executable: &Path, world: &Path) -> Client {
    let mut client = Client::new();
    client.start_probe(executable, world, false).unwrap();
    assert_eq!(client.next(Duration::from_secs(15)).unwrap().kind, "ready");
    client
}

#[test]
fn p5_snapshot_missing_hint_stale_page_cold_resume_and_no_future_advance() {
    let fixture = spec();
    assert_eq!(fixture["schema"], "mycity-service-needs-due-v182");
    assert_eq!(fixture["clock"], "development");
    assert_eq!(fixture["source_id_hex"], "534e000000000001");
    assert_eq!(fixture["source_version"], 1);
    assert_eq!(fixture["unstarted_cursor"], 0);
    assert_eq!(fixture["deadlines_row_zero_cursor"], 1);
    assert_eq!(fixture["occurrences_row_zero_cursor"], 2);
    assert_eq!(fixture["expected_delivery_claim"], false);

    let (_case, world, mut store) = open_due_world("snapshot");
    assert!(matches!(
        required_dispatch::select_service_continuation(&store, true).unwrap(),
        ServiceSelection::Busy
    ));
    let first = selected(required_dispatch::select_service_continuation(&store, false).unwrap());
    assert_eq!(first.mode(), SelectionMode::RegistrySelected);
    assert_eq!(first.key().source_id, required_dispatch::SERVICE_NEEDS_SOURCE_ID);
    assert_eq!(first.key().source_id.to_be_bytes(), [0x53, 0x4e, 0, 0, 0, 0, 0, 1]);
    assert_eq!(first.key().source_version, fixture["source_version"].as_u64().unwrap());
    assert_eq!(first.key().scope.city_id, Some(fixture["city_id"].as_u64().unwrap() as u32));
    assert_eq!(first.key().clock, mycity_core::task_registry::ClockKind::Development { city_id: 1 });
    assert_eq!(first.key().window.start, fixture["window_start"].as_u64().unwrap());
    assert_eq!(first.key().window.end, fixture["target"].as_u64().unwrap());
    assert_eq!(first.key().cursor, fixture["unstarted_cursor"].as_u64().unwrap());
    assert_eq!(first.target(), fixture["target"].as_u64().unwrap());
    assert_eq!(first.page_limit(), fixture["page_limit"].as_u64().unwrap() as u16);
    required_dispatch::validate_service_continuation(&store, &first, false).unwrap();

    let missing = selected(required_dispatch::select_service_continuation_with_missing_hint(&store, false).unwrap());
    assert_eq!(missing.mode(), SelectionMode::MandatoryFallback);
    required_dispatch::validate_service_continuation(&store, &missing, false).unwrap();

    let sequence = first.sequence();
    apply(&mut store, Caller::System, Action::Timeline {
        command: TimelineCommand::Advance { target: first.target(), limit: first.page_limit() },
    });
    assert_eq!(store.committed_sequence(), sequence + 1);
    assert_eq!(
        required_dispatch::validate_service_continuation(&store, &first, false),
        Err(DueError::SelectionChanged),
    );
    let after_first_page = selected(required_dispatch::select_service_continuation(&store, false).unwrap());
    assert_eq!(after_first_page.key().cursor, fixture["occurrences_row_zero_cursor"].as_u64().unwrap());
    required_dispatch::validate_service_continuation(&store, &after_first_page, false).unwrap();

    drop(store);
    let mut store = Store::open(&world, &rules()).unwrap();
    let resumed = selected(required_dispatch::select_service_continuation(&store, false).unwrap());
    assert_eq!(resumed.key(), after_first_page.key());
    assert_eq!(resumed.sequence(), after_first_page.sequence());
    required_dispatch::validate_service_continuation(&store, &resumed, false).unwrap();

    apply(&mut store, Caller::System, Action::Timeline {
        command: TimelineCommand::Advance { target: resumed.target(), limit: resumed.page_limit() },
    });
    let completed = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(completed.last_complete, resumed.target());
    assert_eq!(completed.phase, None);
    let sequence_after_boundary = store.committed_sequence();
    assert!(matches!(
        required_dispatch::select_service_continuation(&store, false).unwrap(),
        ServiceSelection::NoWork
    ));
    assert_eq!(store.committed_sequence(), sequence_after_boundary, "selection must not advance future time");
}

#[test]
fn sim_rundue_lost_terminal_response_replays_only_the_original_page_receipt() {
    let fixture = spec();
    let (case, world, store) = open_due_world("sim-lost-terminal");
    let before_sequence = store.committed_sequence();
    let before_serial = store.engine().unwrap().timeline().unwrap().progress.serial;
    drop(store);

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")).canonicalize().unwrap();
    fs::write(case.join("sim-executable.txt"), executable.to_string_lossy().as_bytes()).unwrap();
    fs::write(case.join("before.json"), serde_json::to_vec(&json!({
        "sequence": before_sequence, "serial": before_serial,
    })).unwrap()).unwrap();
    let mut client = Client::new();
    client.start_probe(&executable, &world, false).unwrap();
    assert_eq!(client.next(Duration::from_secs(15)).unwrap().kind, "ready");
    client.submit(1, Command::RunDue).unwrap();
    let mut selection_event = None;
    for _ in 0..16 {
        let event = client.next(Duration::from_secs(15)).unwrap();
        assert_eq!(event.id, 1);
        if event.kind == "service_continuation_selected" { selection_event = Some(event); break; }
        assert_ne!(event.kind, "result", "RunDue returned before recording the selected envelope");
    }
    let selection_event = selection_event.expect("bounded service selection event missing");
    fs::write(case.join("service-continuation-selected.json"), serde_json::to_vec_pretty(&selection_event).unwrap()).unwrap();
    assert_eq!(selection_event.payload["selection_mode"], "registry_selected");
    assert_eq!(selection_event.payload["before_sequence"], before_sequence);
    assert_eq!(selection_event.payload["before_serial"], before_serial);
    let envelope: Envelope = serde_json::from_value(selection_event.payload["envelope"].clone()).unwrap();
    match envelope.actions.as_slice() {
        [Action::Timeline { command: TimelineCommand::Advance { target, limit } }] => {
            assert_eq!(*target, fixture["target"].as_u64().unwrap());
            assert_eq!(*limit, fixture["page_limit"].as_u64().unwrap() as u16);
        }
        other => panic!("service RunDue must preserve the selected original Advance envelope: {other:?}"),
    }
    let expected_operation = 0xc000_0000_0000_0000 | (before_sequence + 1);
    assert_eq!(envelope.operation_id, expected_operation, "service continuation has its own checked C namespace");
    fs::write(case.join("selected-envelope.json"), serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
    // Stop without consuming the terminal result; the real Sim finishes/retire itself.
    client.stop().unwrap();

    let mut store = Store::open(&world, &rules()).unwrap();
    assert_eq!(store.committed_sequence(), before_sequence + 1);
    let committed = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(committed.serial, before_serial + 1);
    let record = store.receipt(envelope.operation_id).unwrap()
        .expect("cold Store must recover the exact original C operation receipt");
    assert!(record.outcome.is_ok(), "{record:?}");
    let original_receipt = record.outcome.as_ref().unwrap();
    assert_eq!(original_receipt.timeline.as_ref().unwrap().serial, before_serial + 1);
    fs::write(case.join("original-receipt.json"), serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    let due = store.engine().unwrap().service_needs_due_state().unwrap().unwrap();
    assert_eq!(due.pending.unwrap().at, fixture["target"].as_u64().unwrap());

    let replay = store.execute(Caller::System, envelope).unwrap();
    assert!(replay.has_persistent_receipt);
    let receipt = replay.model.unwrap();
    assert_eq!(receipt.timeline.unwrap().serial, committed.serial);
    assert_eq!(store.committed_sequence(), before_sequence + 1, "exact receipt replay must not commit a second page");
    assert_eq!(store.engine().unwrap().timeline().unwrap().progress.serial, committed.serial);
    fs::write(case.join("replay-proof.json"), serde_json::to_vec_pretty(&json!({
        "sequence": store.committed_sequence(),
        "serial": store.engine().unwrap().timeline().unwrap().progress.serial,
        "has_persistent_receipt": replay.has_persistent_receipt,
        "terminal_result_consumed_by_client": false,
    })).unwrap()).unwrap();
}

#[test]
fn sim_execute_c_namespace_is_reserved_and_an_occupied_due_operation_fails_closed() {
    let (case, world, store) = open_due_world("namespace");
    drop(store);
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")).canonicalize().unwrap();
    let mut client = sim(&executable, &world);
    let before = result(&mut client, 1, Command::Status)["Ok"].clone();
    let sequence = before["sequence"].as_u64().unwrap();
    let envelope = Envelope {
        world: 1,
        branch: 1,
        operation_id: 0xc000_0000_0000_0000 | (sequence + 1),
        expected_revision: before["revision"].as_u64().unwrap(),
        actions: vec![Action::Timeline { command: TimelineCommand::Advance {
            target: spec()["target"].as_u64().unwrap(),
            limit: spec()["page_limit"].as_u64().unwrap() as u16,
        }}],
    };
    let rejected = result(&mut client, 2, Command::Execute { envelope });
    assert_eq!(rejected["Err"], "sim_internal_control_reserved");
    let after = result(&mut client, 3, Command::Status)["Ok"].clone();
    assert_eq!(after["sequence"], before["sequence"]);
    assert_eq!(after["timeline"], before["timeline"]);
    assert_eq!(after["state_sha256"], before["state_sha256"]);
    fs::write(case.join("generic-c-reserved.json"), serde_json::to_vec_pretty(&json!({
        "before": before,
        "rejected": rejected,
        "after": after,
    })).unwrap()).unwrap();
    client.stop().unwrap();

    let mut store = Store::open(&world, &rules()).unwrap();
    let initial_sequence = store.committed_sequence();
    let page_limit = store.continuation_status().unwrap().unwrap().page_limit;
    let (scope, revision) = store.engine().unwrap().world().scope_revision();
    let occupied_operation = 0xc000_0000_0000_0000 | (initial_sequence + 2);
    let occupied = Envelope {
        world: scope.world(),
        branch: scope.branch(),
        operation_id: occupied_operation,
        expected_revision: revision,
        actions: vec![Action::Timeline { command: TimelineCommand::Advance {
            target: spec()["target"].as_u64().unwrap(), limit: page_limit,
        }}],
    };
    let occupied_outcome = store.execute(Caller::System, occupied).unwrap();
    assert!(occupied_outcome.has_persistent_receipt);
    assert!(occupied_outcome.model.is_ok());
    assert_eq!(store.committed_sequence(), initial_sequence + 1);
    let before_collision = store.state_hash().unwrap();
    let before_progress = store.engine().unwrap().timeline().unwrap().progress;
    assert!(store.engine().unwrap().service_needs_due_state().unwrap().unwrap().pending.is_some());
    fs::write(case.join("occupied-before.json"), serde_json::to_vec_pretty(&json!({
        "sequence": store.committed_sequence(),
        "state_sha256": store.state_hash().unwrap(),
        "progress": store.engine().unwrap().timeline().unwrap().progress,
        "due": store.engine().unwrap().service_needs_due_state().unwrap().unwrap(),
    })).unwrap()).unwrap();
    drop(store);

    let mut client = sim(&executable, &world);
    let status = result(&mut client, 4, Command::Status)["Ok"].clone();
    assert_eq!(status["sequence"], initial_sequence + 1);
    client.submit(5, Command::RunDue).unwrap();
    let mut collision_events = Vec::new();
    let mut collision = None;
    for _ in 0..16 {
        let event = client.next(Duration::from_secs(15)).unwrap();
        assert_eq!(event.id, 5);
        assert_ne!(event.kind, "service_continuation_selected", "occupied receipt must be checked before candidate event");
        let terminal = event.kind == "result";
        collision_events.push(event.clone());
        if terminal { collision = Some(event.payload); break; }
    }
    fs::write(case.join("collision-events.json"), serde_json::to_vec_pretty(&collision_events).unwrap()).unwrap();
    let collision = collision.expect("bounded namespace collision result missing");
    assert!(collision["Err"].as_str().unwrap().starts_with("service_due_namespace_collision"));
    fs::write(case.join("collision-result.json"), serde_json::to_vec_pretty(&collision).unwrap()).unwrap();
    client.stop().unwrap();

    let mut store = Store::open(&world, &rules()).unwrap();
    assert_eq!(store.state_hash().unwrap(), before_collision);
    assert_eq!(store.committed_sequence(), initial_sequence + 1);
    assert_eq!(store.engine().unwrap().timeline().unwrap().progress, before_progress);
    assert!(store.engine().unwrap().service_needs_due_state().unwrap().unwrap().pending.is_some());
    fs::write(case.join("collision-after.json"), serde_json::to_vec_pretty(&json!({
        "sequence": store.committed_sequence(),
        "state_sha256": store.state_hash().unwrap(),
        "progress": store.engine().unwrap().timeline().unwrap().progress,
        "due": store.engine().unwrap().service_needs_due_state().unwrap().unwrap(),
    })).unwrap()).unwrap();

    // The original reservation remains usable through an ordinary caller op;
    // the selector never changes IDs or retries against a different receipt.
    apply(&mut store, Caller::System, Action::Timeline { command: TimelineCommand::Advance {
        target: spec()["target"].as_u64().unwrap(), limit: page_limit,
    }});
    assert_eq!(store.committed_sequence(), initial_sequence + 2);
}
