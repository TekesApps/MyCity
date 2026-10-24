use mycity_core::{
    continuous_supply::{self, Command as SupplyCommand},
    continuous_genset::Status,
    schedule::Phase,
    timeline,
    transaction::{Action, Caller, Envelope},
};
use mycity_headless::{
    required_dispatch::{
    self, DueError, SupplyPlanKind, SupplySelection, SupplySelectionMode, SupplyZeroReason,
    },
    sim_client::Client,
    sim_host::Command as SimCommand,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};

#[path = "../../../tests/fixtures/continuous-supply/opening.rs"]
mod opening;

const SPEC: &[u8] = include_bytes!("../../../tests/fixtures/issue-101/continuous-supply-due-v184.json");
const SPATIAL: &[u8] = include_bytes!("../../../fixtures/m0-exchange-spatial.json");
const TERRAIN: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
const RULES: &str = "continuous-supply-original-store-v1";
const APPROVED_ROOT: &str = "/Volumes/HP P900/dev/MyCity/work";

fn spec() -> Value { serde_json::from_slice(SPEC).unwrap() }

fn evidence_case(label: &str) -> (PathBuf, PathBuf) {
    let root = fs::canonicalize(std::env::var_os("MYCITY_ISSUE101_CONTINUOUS_SUPPLY_DUE_ROOT")
        .expect("set MYCITY_ISSUE101_CONTINUOUS_SUPPLY_DUE_ROOT to an existing retained HP P900 work root"))
        .expect("canonicalize existing evidence root");
    let approved = fs::canonicalize(APPROVED_ROOT).expect("HP P900 work volume must be mounted");
    assert_eq!(approved, Path::new(APPROVED_ROOT));
    assert!(root.starts_with(&approved) && root != approved, "unexpected evidence root: {}", root.display());
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(fs::metadata("/Volumes/HP P900").unwrap().dev(), fs::metadata("/Volumes").unwrap().dev());
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let case = root.join(format!("issue101-continuous-supply-v184-{label}-{}-{nonce}", std::process::id()));
    fs::create_dir(&case).expect("create unique retained case directory");
    let world = case.join("world");
    println!("RETAINED_CONTINUOUS_SUPPLY_CASE={}", case.display());
    (case, world)
}

fn apply(store: &mut Store, caller: Caller, action: Action) -> Envelope {
    let (scope, revision) = store.engine().unwrap().world().scope_revision();
    let envelope = Envelope {
        world: scope.world(),
        branch: scope.branch(),
        operation_id: store.committed_sequence().checked_add(1).unwrap(),
        expected_revision: revision,
        actions: vec![action],
    };
    let result = store.execute(caller, envelope.clone()).unwrap();
    assert!(result.has_persistent_receipt, "{result:?}");
    assert!(result.model.is_ok(), "{result:?}");
    envelope
}

fn create_store_with_rules(world: &Path, rules: &str) -> Store {
    Store::create(
        world,
        &serde_json::to_vec(&opening::input()).unwrap(),
        opening::resources(),
        rules,
    ).unwrap()
}

fn setup_status(world: &Path, status: &str, rules: &str) -> Store {
    let mut store = create_store_with_rules(world, rules);
    apply_status(&mut store, status);
    store
}

fn setup_spatial_status(world: &Path, status: &str, rules: &str, case: &Path) -> Store {
    let opening_bytes = serde_json::to_vec(&opening::input()).unwrap();
    let spatial: Value = serde_json::from_slice(SPATIAL).unwrap();
    assert_eq!(spatial["bootstrap"]["seed"], opening::input()["bootstrap"]["seed"]);
    assert_eq!(spatial["bootstrap"]["seed"], 7);
    fs::write(case.join("opening.json"), &opening_bytes).unwrap();
    fs::write(case.join("spatial-opening.json"), SPATIAL).unwrap();
    let terrain_dir = case.join("terrain");
    fs::create_dir(&terrain_dir).unwrap();
    fs::write(terrain_dir.join("m0-flat16.png"), TERRAIN).unwrap();
    let mut store = Store::create_spatial(world, &opening_bytes, opening::resources(), rules, SPATIAL, TERRAIN).unwrap();
    apply_status(&mut store, status);
    store
}

fn apply_status(store: &mut Store, status: &str) {
    apply(store, Caller::System, Action::Timeline { command: timeline::Command::Enable });
    apply(store, Caller::Account(1), Action::ContinuousSupply {
        command: SupplyCommand::Start { run_id: 1, pumps: 3 },
    });
    match status {
        "running" => assert_eq!(store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap().status, Status::Running),
        "stopped" => {
            apply(store, Caller::Account(1), Action::ContinuousSupply { command: SupplyCommand::Stop });
            assert_eq!(store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap().status, Status::Stopped);
        }
        "faulted" => {
            apply(store, Caller::Account(1), Action::ContinuousSupply {
                command: SupplyCommand::Fault { fault_ref: 7001 },
            });
            assert_eq!(store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap().status, Status::Faulted { fault_ref: 7001 });
        }
        other => panic!("unknown fixture status {other}"),
    }
}

fn sim_rules() -> String {
    format!("sim-w3:{}", env!("MYCITY_SOURCE_SHA256"))
}

fn selected(value: SupplySelection) -> required_dispatch::SelectedSupplyContinuation {
    match value {
        SupplySelection::Selected(selected) => selected,
        other => panic!("expected a selected supply responsibility, got {other:?}"),
    }
}

fn key_json(key: Option<&mycity_core::task_registry::TaskKey>) -> Value {
    key.map(|key| json!({
        "scope": {
            "world_id": key.scope.world_id,
            "branch_id": key.scope.branch_id,
            "city_id": key.scope.city_id,
        },
        "clock": { "kind": "development", "city_id": 42 },
        "source_id": key.source_id,
        "source_version": key.source_version,
        "window": { "start": key.window.start, "end": key.window.end },
        "cursor": key.cursor,
    })).unwrap_or(Value::Null)
}

fn advance(store: &mut Store, target: u64, page_limit: u16) -> Envelope {
    apply(store, Caller::System, Action::Timeline {
        command: timeline::Command::Advance { target, limit: page_limit },
    })
}

fn start_sim(executable: &Path, world: &Path) -> Client {
    let mut client = Client::new();
    client.start_probe(executable, world, false).unwrap();
    let ready = client.next(Duration::from_secs(15)).unwrap();
    assert_eq!(ready.kind, "ready");
    client
}

fn run_due_events(client: &mut Client, id: u64) -> (Vec<Value>, Value) {
    client.submit(id, SimCommand::RunDue).unwrap();
    let mut events = Vec::new();
    for _ in 0..16 {
        let event = client.next(Duration::from_secs(15)).unwrap();
        assert_eq!(event.id, id);
        let is_result = event.kind == "result";
        let payload = event.payload;
        events.push(json!({"kind": event.kind, "id": event.id, "payload": payload.clone()}));
        if is_result { return (events, payload); }
    }
    panic!("bounded RunDue terminal result missing")
}

#[test]
fn accepted_supply_target_survives_all_genset_statuses_and_next_boundary_admits_originally() {
    let fixture = spec();
    assert_eq!(fixture["schema"], "mycity-continuous-supply-due-v184");
    assert_eq!(fixture["city_id"], 42);
    assert_eq!(fixture["source_id"], 86);
    assert_eq!(fixture["source_version"], continuous_supply::VERSION);
    assert_eq!(fixture["clock"], "development");
    assert_eq!(fixture["accepted_target"], 7200);
    assert_eq!(fixture["funded_page_limit"], 1);
    assert_eq!(fixture["admission_policy_version"], required_dispatch::SUPPLY_ADMISSION_POLICY_VERSION);
    assert_eq!(fixture["admission_page_limit"], required_dispatch::SUPPLY_ADMISSION_PAGE_LIMIT);
    assert_eq!(fixture["delivery_claim"], false);

    for status in fixture["statuses"].as_array().unwrap() {
        let status = status.as_str().unwrap();
        let (case, world) = evidence_case(status);
        let mut store = setup_status(&world, status, RULES);
        assert!(matches!(required_dispatch::select_supply_continuation(&store, false).unwrap(), SupplySelection::NoWork));
        advance(&mut store, fixture["accepted_target"].as_u64().unwrap(), fixture["funded_page_limit"].as_u64().unwrap() as u16);

        let original_sequence = store.committed_sequence();
        let first = selected(required_dispatch::select_supply_continuation(&store, false).unwrap());
        assert_eq!(first.plan_kind(), SupplyPlanKind::FundedContinuation);
        assert_eq!(first.page_limit(), store.continuation_status().unwrap().unwrap().page_limit);
        assert_eq!(first.page_limit(), fixture["funded_page_limit"].as_u64().unwrap() as u16);
        assert!(matches!(first.mode(), SupplySelectionMode::RegistrySelected | SupplySelectionMode::MandatoryFallback));
        assert_eq!(first.policy_version(), None);
        assert_eq!(first.zero_reason(), None);
        let key = first.key().expect("nonzero funded timeline interval has a task key");
        let mut seen_phases = std::collections::BTreeSet::new();
        if let Some(phase) = store.engine().unwrap().timeline().unwrap().progress.phase { seen_phases.insert(phase); }
        assert_eq!(key.scope.city_id, Some(fixture["city_id"].as_u64().unwrap() as u32));
        assert_eq!(key.source_id, fixture["source_id"].as_u64().unwrap());
        assert_eq!(key.source_version, fixture["source_version"].as_u64().unwrap());
        assert_eq!(key.clock, mycity_core::task_registry::ClockKind::Development { city_id: 42 });
        assert_eq!(first.target(), fixture["accepted_target"].as_u64().unwrap());
        required_dispatch::validate_supply_continuation(&store, &first, false).unwrap();
        assert_eq!(required_dispatch::select_supply_continuation(&store, true).unwrap(), SupplySelection::Busy);

        let missing = selected(required_dispatch::select_supply_continuation_with_missing_hint(&store, false).unwrap());
        assert_eq!(missing.mode(), SupplySelectionMode::MandatoryFallback);
        assert_eq!(missing.key(), first.key());
        required_dispatch::validate_supply_continuation(&store, &missing, false).unwrap();

        let receipt_operation = store.committed_sequence();
        let raw = store.continuation_status().unwrap().unwrap();
        let receipt_envelope = advance(&mut store, first.target(), first.page_limit());
        assert_eq!(store.committed_sequence(), receipt_operation + 1);
        assert_eq!(receipt_envelope.operation_id, receipt_operation + 1);
        assert_eq!(required_dispatch::validate_supply_continuation(&store, &first, false), Err(DueError::SelectionChanged));
        fs::write(case.join("funded-selection.json"), serde_json::to_vec_pretty(&json!({
            "selected_key": key_json(first.key()), "mode": format!("{:?}", first.mode()),
            "plan_kind": format!("{:?}", first.plan_kind()), "page_limit": first.page_limit(),
            "original_continuation": raw, "original_envelope": receipt_envelope,
            "before_sequence": original_sequence, "after_sequence": store.committed_sequence(),
        })).unwrap()).unwrap();

        drop(store);
        let mut store = Store::open(&world, RULES).unwrap();
        let recovered = store.receipt(receipt_operation + 1).unwrap().expect("cold Store recovers exact original Advance receipt");
        assert!(recovered.outcome.is_ok(), "{recovered:?}");
        let current = selected(required_dispatch::select_supply_continuation(&store, false).unwrap());
        assert_eq!(current.plan_kind(), SupplyPlanKind::FundedContinuation);
        assert_eq!(current.page_limit(), store.continuation_status().unwrap().unwrap().page_limit);
        required_dispatch::validate_supply_continuation(&store, &current, false).unwrap();

        let mut reached_admission = false;
        for _ in 0..128 {
            let progress = store.engine().unwrap().timeline().unwrap().progress;
            if progress.phase.is_none() && progress.target.is_some() && store.continuation_status().unwrap().is_none() {
                reached_admission = true;
                break;
            }
            if let Some(phase) = progress.phase { seen_phases.insert(phase); }
            let selection = selected(required_dispatch::select_supply_continuation(&store, false).unwrap());
            assert_eq!(selection.plan_kind(), SupplyPlanKind::FundedContinuation);
            assert_eq!(selection.page_limit(), store.continuation_status().unwrap().unwrap().page_limit);
            required_dispatch::validate_supply_continuation(&store, &selection, false).unwrap();
            advance(&mut store, selection.target(), selection.page_limit());
        }
        assert!(reached_admission, "bounded original funded pages must expose the next accepted-boundary admission window");
        for phase in [Phase::P1, Phase::P2, Phase::P3, Phase::P4, Phase::P5] {
            assert!(seen_phases.contains(&phase), "accepted supply responsibility must remain selectable through {phase:?}");
        }
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        assert_eq!(progress.last_complete, 3600);
        assert_eq!(progress.at, progress.last_complete);
        let admission = selected(required_dispatch::select_supply_continuation(&store, false).unwrap());
        assert_eq!(admission.plan_kind(), SupplyPlanKind::NextBoundaryAdmission);
        assert_eq!(admission.key(), None);
        assert_eq!(admission.mode(), SupplySelectionMode::MandatoryZeroBoundary);
        assert_eq!(admission.zero_reason(), Some(SupplyZeroReason::ZeroWidthBoundary));
        assert_eq!(admission.policy_version(), Some(fixture["admission_policy_version"].as_u64().unwrap() as u32));
        assert_eq!(admission.page_limit(), fixture["admission_page_limit"].as_u64().unwrap() as u16);
        assert_eq!(admission.target(), fixture["accepted_target"].as_u64().unwrap());
        required_dispatch::validate_supply_continuation(&store, &admission, false).unwrap();
        let seq_before_admit = store.committed_sequence();
        advance(&mut store, admission.target(), admission.page_limit());
        assert_eq!(store.committed_sequence(), seq_before_admit + 1);
        assert_eq!(required_dispatch::validate_supply_continuation(&store, &admission, false), Err(DueError::SelectionChanged));
        fs::write(case.join("next-boundary-admission.json"), serde_json::to_vec_pretty(&json!({
            "plan_kind": format!("{:?}", admission.plan_kind()),
            "policy_version": admission.policy_version(), "page_limit": admission.page_limit(),
            "zero_reason": format!("{:?}", admission.zero_reason()),
            "before": progress, "after": store.engine().unwrap().timeline().unwrap().progress,
            "sequence_before": seq_before_admit, "sequence_after": store.committed_sequence(),
            "store_continuation_after_original_admit": store.continuation_status().unwrap(),
        })).unwrap()).unwrap();
    }
}

#[test]
fn equal_sequence_scope_and_timeline_still_reject_a_different_authenticated_supply_binding() {
    let (case_a, world_a) = evidence_case("full-binding-stopped");
    let (case_b, world_b) = evidence_case("full-binding-faulted");
    let mut stopped = setup_status(&world_a, "stopped", RULES);
    let mut faulted = setup_status(&world_b, "faulted", RULES);
    let target = spec()["accepted_target"].as_u64().unwrap();
    let limit = spec()["funded_page_limit"].as_u64().unwrap() as u16;
    advance(&mut stopped, target, limit);
    advance(&mut faulted, target, limit);

    assert_eq!(stopped.committed_sequence(), faulted.committed_sequence());
    let stopped_progress = stopped.engine().unwrap().timeline().unwrap().progress;
    let faulted_progress = faulted.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(stopped_progress, faulted_progress);
    let stopped_due = stopped.engine().unwrap().continuous_supply_due_state().unwrap().unwrap();
    let faulted_due = faulted.engine().unwrap().continuous_supply_due_state().unwrap().unwrap();
    assert_ne!(stopped_due, faulted_due);
    let selected = selected(required_dispatch::select_supply_continuation(&stopped, false).unwrap());
    assert_eq!(selected.sequence(), faulted.committed_sequence());
    assert_eq!(
        required_dispatch::validate_supply_continuation(&faulted, &selected, false),
        Err(DueError::SelectionChanged),
    );
    fs::write(case_a.join("selected-stopped-view.json"), serde_json::to_vec_pretty(&json!({
        "sequence": stopped.committed_sequence(), "progress": stopped_progress, "due": stopped_due,
    })).unwrap()).unwrap();
    fs::write(case_b.join("rejected-faulted-view.json"), serde_json::to_vec_pretty(&json!({
        "sequence": faulted.committed_sequence(), "progress": faulted_progress, "due": faulted_due,
        "revalidation": "selection_changed",
    })).unwrap()).unwrap();
}

#[test]
fn sim_rundue_admits_the_next_boundary_once_and_recovers_the_original_c_receipt_without_reply() {
    let (case, world) = evidence_case("sim-lost-terminal");
    let rules = sim_rules();
    let mut store = setup_spatial_status(&world, "running", &rules, &case);
    let target = spec()["accepted_target"].as_u64().unwrap();
    let funded_limit = spec()["sim_funded_page_limit"].as_u64().unwrap() as u16;
    advance(&mut store, target, funded_limit);
    let funded = selected(required_dispatch::select_supply_continuation(&store, false).unwrap());
    assert_eq!(funded.plan_kind(), SupplyPlanKind::FundedContinuation);
    assert_eq!(funded.page_limit(), funded_limit);
    let funded_before_sequence = store.committed_sequence();
    let funded_before = store.engine().unwrap().timeline().unwrap().progress;
    let funded_continuation = store.continuation_status().unwrap().unwrap();
    assert_eq!(funded_continuation.page_limit, funded_limit);
    drop(store);

    // Exercise the already-funded continuation through the real Sim seam
    // before reaching the next-boundary admission used for lost-response proof.
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")).canonicalize().unwrap();
    fs::write(case.join("sim-executable.txt"), executable.to_string_lossy().as_bytes()).unwrap();
    let mut funded_client = start_sim(&executable, &world);
    funded_client.submit(1, SimCommand::RunDue).unwrap();
    let mut funded_selected_event = None;
    let mut funded_terminal = None;
    for _ in 0..16 {
        let event = funded_client.next(Duration::from_secs(15)).unwrap();
        assert_eq!(event.id, 1);
        match event.kind.as_str() {
            "supply_continuation_selected" => funded_selected_event = Some(event),
            "result" => { funded_terminal = Some(event); break; }
            other => panic!("unexpected funded RunDue event: {other}"),
        }
    }
    let funded_selected_event = funded_selected_event.expect("funded RunDue must emit its original reservation");
    let funded_terminal = funded_terminal.expect("funded RunDue terminal result missing");
    assert_eq!(funded_selected_event.payload["dispatch"], "required_registry_v3");
    assert_eq!(funded_selected_event.payload["plan_kind"], "FundedContinuation");
    assert_eq!(funded_selected_event.payload["before_sequence"], funded_before_sequence);
    assert_eq!(funded_selected_event.payload["before_serial"], funded_before.serial);
    let funded_envelope: Envelope = serde_json::from_value(funded_selected_event.payload["envelope"].clone()).unwrap();
    match funded_envelope.actions.as_slice() {
        [Action::Timeline { command: timeline::Command::Advance { target: selected_target, limit } }] => {
            assert_eq!(*selected_target, target);
            assert_eq!(*limit, funded_limit, "RunDue must preserve the stored width-1 reservation");
        }
        other => panic!("funded RunDue must retain the original Timeline envelope: {other:?}"),
    }
    assert_eq!(funded_envelope.operation_id, 0xc000_0000_0000_0000 | (funded_before_sequence + 1));
    let funded_timeline = funded_terminal.payload["Ok"]["timeline"].as_object()
        .expect("funded RunDue must return its original Timeline progress");
    assert_eq!(funded_timeline["serial"].as_u64().unwrap(), funded_before.serial + 1);
    assert!(funded_timeline["target"].as_u64() == Some(target));
    fs::write(case.join("funded-run-due.json"), serde_json::to_vec_pretty(&json!({
        "selected": funded_selected_event, "result": funded_terminal,
        "original_envelope": funded_envelope, "stored_page_limit": funded_limit,
    })).unwrap()).unwrap();
    funded_client.stop().unwrap();

    let mut store = Store::open(&world, &rules).unwrap();
    assert_eq!(store.committed_sequence(), funded_before_sequence + 1);
    let funded_record = store.receipt(funded_envelope.operation_id).unwrap().expect("cold Store retains funded RunDue receipt");
    assert!(funded_record.outcome.is_ok(), "{funded_record:?}");
    assert_eq!(funded_record.outcome.unwrap().timeline.unwrap().serial, funded_before.serial + 1);
    let funded_replay = store.execute(Caller::System, funded_envelope.clone()).unwrap();
    assert!(funded_replay.has_persistent_receipt);
    assert_eq!(store.committed_sequence(), funded_before_sequence + 1);

    let mut reached_admission = false;
    for _ in 0..128 {
        let progress = store.engine().unwrap().timeline().unwrap().progress;
        if progress.phase.is_none() && progress.target == Some(target) && store.continuation_status().unwrap().is_none() {
            reached_admission = true;
            break;
        }
        let continuation = store.continuation_status().unwrap().expect("funded Timeline phase retains original continuation");
        advance(&mut store, continuation.target, continuation.page_limit);
    }
    assert!(reached_admission, "bounded original Store continuation must reach the next admission boundary");
    let selected = selected(required_dispatch::select_supply_continuation(&store, false).unwrap());
    assert_eq!(selected.plan_kind(), SupplyPlanKind::NextBoundaryAdmission);
    assert_eq!(selected.target(), target);
    assert_eq!(selected.page_limit(), required_dispatch::SUPPLY_ADMISSION_PAGE_LIMIT);
    let before_sequence = store.committed_sequence();
    let before = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(before.phase, None);
    assert_eq!(before.target, Some(target));
    assert_eq!(before.at, before.last_complete);
    let before_due = store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap();
    fs::write(case.join("before-run-due.json"), serde_json::to_vec_pretty(&json!({
        "sequence": before_sequence, "timeline": before, "due": before_due,
        "selection_mode": format!("{:?}", selected.mode()),
        "plan_kind": format!("{:?}", selected.plan_kind()),
        "policy_version": selected.policy_version(), "page_limit": selected.page_limit(),
    })).unwrap()).unwrap();
    drop(store);

    let mut client = start_sim(&executable, &world);
    client.submit(2, SimCommand::RunDue).unwrap();
    let mut event_log = Vec::new();
    let mut selected_event = None;
    for _ in 0..16 {
        let event = client.next(Duration::from_secs(15)).unwrap();
        assert_eq!(event.id, 2);
        let is_selected = event.kind == "supply_continuation_selected";
        let is_result = event.kind == "result";
        event_log.push(serde_json::to_value(&event).unwrap());
        if is_selected { selected_event = Some(event); break; }
        assert!(!is_result, "RunDue must publish the selected original envelope before executing it");
    }
    fs::write(case.join("events-through-selection.json"), serde_json::to_vec_pretty(&event_log).unwrap()).unwrap();
    let selected_event = selected_event.expect("bounded RunDue did not emit the supply selection event");
    assert_eq!(selected_event.payload["dispatch"], "required_registry_v3");
    assert_eq!(selected_event.payload["plan_kind"], "NextBoundaryAdmission");
    assert_eq!(selected_event.payload["policy_version"], 1);
    assert_eq!(selected_event.payload["before_sequence"], before_sequence);
    assert_eq!(selected_event.payload["before_serial"], before.serial);
    let envelope: Envelope = serde_json::from_value(selected_event.payload["envelope"].clone()).unwrap();
    assert_eq!(envelope.operation_id, 0xc000_0000_0000_0000 | (before_sequence + 1));
    match envelope.actions.as_slice() {
        [Action::Timeline { command: timeline::Command::Advance { target: selected_target, limit } }] => {
            assert_eq!(*selected_target, target);
            assert_eq!(*limit, required_dispatch::SUPPLY_ADMISSION_PAGE_LIMIT);
        }
        other => panic!("RunDue must preserve one original Timeline Advance envelope: {other:?}"),
    }
    fs::write(case.join("selected-envelope.json"), serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
    // Deliberately stop without consuming the terminal event; inspect the exact
    // original operation after this owned Sim has exited and Store is cold.
    client.stop().unwrap();

    let mut store = Store::open(&world, &rules).unwrap();
    assert_eq!(store.committed_sequence(), before_sequence + 1);
    let record = store.receipt(envelope.operation_id).unwrap().expect("cold Store recovers the exact selected C receipt");
    assert!(record.outcome.is_ok(), "{record:?}");
    let outcome = record.outcome.as_ref().unwrap();
    let committed = outcome.timeline.as_ref().unwrap();
    assert_eq!(committed.serial, before.serial + 1);
    assert!(committed.last_complete > before.last_complete && committed.last_complete < target);
    assert_eq!(committed.target, Some(target), "the running source's real input boundary must preserve the accepted target");
    fs::write(case.join("original-receipt.json"), serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    let replay = store.execute(Caller::System, envelope.clone()).unwrap();
    assert!(replay.has_persistent_receipt);
    let replayed = replay.model.unwrap();
    assert_eq!(replayed.timeline.unwrap().serial, committed.serial);
    assert_eq!(store.committed_sequence(), before_sequence + 1, "exact envelope replay must not add a second page");
    assert_eq!(store.engine().unwrap().timeline().unwrap().progress, *committed);
    fs::write(case.join("replay-proof.json"), serde_json::to_vec_pretty(&json!({
        "sequence": store.committed_sequence(),
        "serial": store.engine().unwrap().timeline().unwrap().progress.serial,
        "original_terminal_result_consumed": false,
        "exact_original_envelope_replayed": true,
    })).unwrap()).unwrap();

    // Continue the same accepted target through the real running-input stop
    // boundary. Each RunDue is one original C-envelope and one serial step;
    // the source may stop itself when the raw-water lot is exhausted while the
    // Timeline still retains the user's target.
    let mut expected_sequence = store.committed_sequence();
    let mut expected_serial = store.engine().unwrap().timeline().unwrap().progress.serial;
    assert_eq!(store.engine().unwrap().timeline().unwrap().progress.target, Some(target));
    drop(store);
    let mut client = start_sim(&executable, &world);
    let mut continuation_events = Vec::new();
    let mut completed = false;
    for id in 3..67 {
        client.submit(id, SimCommand::RunDue).unwrap();
        let mut selected = None;
        let mut terminal = None;
        for _ in 0..16 {
            let event = client.next(Duration::from_secs(15)).unwrap();
            assert_eq!(event.id, id);
            match event.kind.as_str() {
                "supply_continuation_selected" => {
                    assert!(selected.is_none(), "one RunDue may select only one page");
                    selected = Some(event);
                }
                "result" => { terminal = Some(event); break; }
                other => panic!("unexpected event during bounded supply continuation: {other}"),
            }
        }
        let selected_event = selected.expect("required target must retain a next RunDue page");
        let result_event = terminal.expect("bounded RunDue terminal response missing");
        assert_eq!(selected_event.payload["dispatch"], "required_registry_v3");
        assert_eq!(selected_event.payload["before_sequence"], expected_sequence);
        assert_eq!(selected_event.payload["before_serial"], expected_serial);
        let next_envelope: Envelope = serde_json::from_value(selected_event.payload["envelope"].clone()).unwrap();
        assert_eq!(next_envelope.operation_id, 0xc000_0000_0000_0000 | (expected_sequence + 1));
        let next_progress = result_event.payload.get("Ok").expect("RunDue page must succeed")
            .get("timeline").expect("original Timeline outcome is retained");
        let next_serial = next_progress["serial"].as_u64().unwrap();
        assert_eq!(next_serial, expected_serial + 1);
        let next_target = next_progress["target"].as_u64();
        assert!(next_target == Some(target) || next_target.is_none(), "RunDue changed or discarded the accepted target");
        continuation_events.push(json!({
            "id": id, "selected": selected_event, "result": result_event,
            "operation": next_envelope.operation_id,
            "before_sequence": expected_sequence, "after_sequence": expected_sequence + 1,
            "before_serial": expected_serial, "after_serial": next_serial,
            "target_after": next_target,
        }));
        expected_sequence += 1;
        expected_serial = next_serial;
        if next_target.is_none() { completed = true; break; }
    }
    assert!(completed, "bounded RunDue pages should finish the accepted target");
    client.stop().unwrap();
    fs::write(case.join("continued-run-due-events.json"), serde_json::to_vec_pretty(&continuation_events).unwrap()).unwrap();

    let mut store = Store::open(&world, &rules).unwrap();
    let final_progress = store.engine().unwrap().timeline().unwrap().progress;
    assert_eq!(store.committed_sequence(), expected_sequence);
    assert_eq!(final_progress.serial, expected_serial);
    assert_eq!(final_progress.last_complete, target);
    assert_eq!(final_progress.target, None);
    let source = opening::supply(store.engine().unwrap());
    assert_eq!(source.cursor, target);
    assert_eq!(source.raw_used_ml, 100_000_000, "original raw-water ledger reaches its real input boundary");
    assert_eq!(source.produced_ml, [50_000_000, 50_000_000]);
    assert_eq!(source.genset.active_seconds, 5_000);
    assert_eq!(source.runs[0].as_ref().unwrap().production_seconds, 5_000);
    assert_eq!(source.genset.status, Status::Stopped);
    fs::write(case.join("completed-source-ledger.json"), serde_json::to_vec_pretty(&json!({
        "timeline": final_progress, "source": source,
        "expected_source_cursor": target,
        "raw_input_boundary_preserved_target_until_completion": true,
    })).unwrap()).unwrap();
}

#[test]
fn sim_rundue_c_namespace_collision_preserves_the_original_target_for_manual_advance() {
    let (case, world) = evidence_case("sim-collision");
    let rules = sim_rules();
    let mut store = setup_spatial_status(&world, "stopped", &rules, &case);
    let target = spec()["accepted_target"].as_u64().unwrap();
    advance(&mut store, target, spec()["funded_page_limit"].as_u64().unwrap() as u16);
    let before_sequence = store.committed_sequence();
    let continuation = store.continuation_status().unwrap().expect("accepted target has a real reservation");
    let (scope, revision) = store.engine().unwrap().world().scope_revision();
    let occupied_operation = 0xc000_0000_0000_0000 | (before_sequence + 2);
    let occupied_envelope = Envelope {
        world: scope.world(),
        branch: scope.branch(),
        operation_id: occupied_operation,
        expected_revision: revision,
        actions: vec![Action::Timeline { command: timeline::Command::Advance {
            target: continuation.target,
            limit: continuation.page_limit,
        }}],
    };
    let occupied_receipt = store.execute(Caller::System, occupied_envelope).unwrap();
    assert!(occupied_receipt.has_persistent_receipt);
    assert!(occupied_receipt.model.is_ok(), "{occupied_receipt:?}");
    assert_eq!(store.committed_sequence(), before_sequence + 1);
    assert!(store.continuation_status().unwrap().is_some(), "one limit-1 page must preserve the accepted target");
    let before_collision_sequence = store.committed_sequence();
    let before_collision = store.state_hash().unwrap();
    let before_progress = store.engine().unwrap().timeline().unwrap().progress;
    let before_due = store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap();
    let before_continuation = store.continuation_status().unwrap().unwrap();
    assert_eq!(occupied_operation, 0xc000_0000_0000_0000 | (before_collision_sequence + 1));
    fs::write(case.join("collision-before.json"), serde_json::to_vec_pretty(&json!({
        "sequence": before_collision_sequence, "state_sha256": before_collision,
        "timeline": before_progress, "due": before_due, "continuation": before_continuation,
        "occupied_operation": occupied_operation,
    })).unwrap()).unwrap();
    drop(store);

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")).canonicalize().unwrap();
    fs::write(case.join("sim-executable.txt"), executable.to_string_lossy().as_bytes()).unwrap();
    let mut client = start_sim(&executable, &world);
    let (events, collision) = run_due_events(&mut client, 2);
    assert!(events.iter().all(|event| event["kind"] != "supply_continuation_selected"), "an occupied C operation must be rejected before selection event");
    assert!(collision["Err"].as_str().unwrap().starts_with("supply_due_namespace_collision"), "{collision}");
    fs::write(case.join("collision-events.json"), serde_json::to_vec_pretty(&events).unwrap()).unwrap();
    fs::write(case.join("collision-result.json"), serde_json::to_vec_pretty(&collision).unwrap()).unwrap();
    client.stop().unwrap();

    let mut store = Store::open(&world, &rules).unwrap();
    assert_eq!(store.committed_sequence(), before_collision_sequence);
    assert_eq!(store.state_hash().unwrap(), before_collision);
    assert_eq!(store.engine().unwrap().timeline().unwrap().progress, before_progress);
    assert_eq!(store.engine().unwrap().continuous_supply_due_state().unwrap().unwrap(), before_due);
    assert_eq!(store.continuation_status().unwrap().unwrap(), before_continuation);
    assert!(store.receipt(occupied_operation).unwrap().is_some());

    // The owner's ordinary original Store path can continue the same target;
    // no fresh RunDue ID or alternate receipt is selected after collision.
    let ordinary = advance(&mut store, continuation.target, continuation.page_limit);
    assert_eq!(store.committed_sequence(), before_collision_sequence + 1);
    assert_eq!(ordinary.operation_id, before_collision_sequence + 1);
    assert_eq!(store.engine().unwrap().timeline().unwrap().progress.target, Some(target));
    assert!(store.continuation_status().unwrap().is_some());
    fs::write(case.join("manual-advance-proof.json"), serde_json::to_vec_pretty(&json!({
        "ordinary_operation": ordinary.operation_id,
        "sequence": store.committed_sequence(),
        "timeline": store.engine().unwrap().timeline().unwrap().progress,
        "continuation": store.continuation_status().unwrap(),
        "target_preserved": true,
    })).unwrap()).unwrap();
}
