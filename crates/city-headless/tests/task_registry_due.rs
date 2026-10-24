use mycity_core::{
    air::Input,
    background::{Command as BackgroundCommand, Failure},
    schedule::Phase,
    timeline::Command as TimelineCommand,
    transaction::{Action, Caller, Envelope, Resource},
};
use mycity_headless::{sim_client::Client, sim_host::Command};
use mycity_storage::Store;
use mycity_headless::required_dispatch::{
    select_required, select_required_with_missing_hint, validate_required, DueError,
    RequiredBlock, Selection, SelectionMode,
};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ROOT_ENV: &str = "MYCITY_ISSUE101_DUE_ROOT";
const EXTERNAL_WORK: &str = "/Volumes/HP P900/dev/MyCity/work";
const RULES: &str = "issue101-due-v168-real-store-test-v1";
const MAX_EVENTS: usize = 512;
const MAX_EVENT_BYTES: usize = 256 * 1024;
const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-exchange-opening.json");
const SPATIAL: &[u8] = include_bytes!("../../../fixtures/m0-exchange-spatial.json");
const TERRAIN: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
const EMPTY_AIR: &[u8] = include_bytes!("../../../tests/fixtures/issue-101/air-empty-sources-30.json");
const EMPTY_AIR_60: &[u8] = include_bytes!("../../../tests/fixtures/issue-101/air-empty-sources.json");

fn evidence_root() -> PathBuf {
    let configured = std::env::var_os(ROOT_ENV)
        .unwrap_or_else(|| panic!("{ROOT_ENV} must point to an existing dedicated HP P900 run root"));
    let root = PathBuf::from(configured)
        .canonicalize()
        .unwrap_or_else(|error| panic!("cannot canonicalize {ROOT_ENV}: {error}"));
    let work = Path::new(EXTERNAL_WORK)
        .canonicalize()
        .unwrap_or_else(|error| panic!("external work volume is unavailable: {error}"));
    assert!(root.starts_with(&work), "evidence root must stay under {work:?}");
    assert_ne!(root, work, "evidence root must be a dedicated existing subdirectory");
    assert!(root.is_dir());
    root
}

fn new_case(name: &str) -> PathBuf {
    let root = evidence_root();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before Unix epoch")
        .as_nanos();
    let case = root.join(format!("{name}-{}-{stamp}", std::process::id()));
    fs::create_dir(&case).expect("create unique case without overwriting prior evidence");
    let manifest = json!({
        "schema": "mycity-issue101-due-v168-test-run-v1",
        "case": name,
        "executable": executable().canonicalize().expect("canonicalize fixed test executable"),
        "evidence_root": case,
        "preserve_world_and_transcript": true
    });
    let manifest_path = case.join("run.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(manifest_path)
        .expect("create retained manifest without overwrite");
    file.write_all(&serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    file.sync_all().unwrap();
    case
}

fn create_store(path: &Path) -> Store {
    let resources = [(101, 1), (102, 1), (103, 1), (104, 1), (201, 2000), (202, 2000)]
        .into_iter()
        .map(|(id, capacity)| Resource::new(id, capacity).unwrap())
        .collect();
    Store::create_spatial(path, OPENING, resources, RULES, SPATIAL, TERRAIN).unwrap()
}

fn envelope(store: &Store, action: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: store.committed_sequence() + 1,
        expected_revision: store.engine().unwrap().world().view().revision,
        actions: vec![action],
    }
}

fn execute(store: &mut Store, action: Action) {
    let request = envelope(store, action);
    let outcome = store.execute(Caller::System, request).unwrap();
    assert!(outcome.model.is_ok(), "{outcome:?}");
    assert!(outcome.has_persistent_receipt);
}

fn prepare_p4(store: &mut Store) {
    execute(
        store,
        Action::Timeline {
            command: TimelineCommand::Enable,
        },
    );
    execute(
        store,
        Action::Background {
            command: BackgroundCommand::Enable,
        },
    );
    for _ in 0..96 {
        execute(
            store,
            Action::Timeline {
                command: TimelineCommand::Advance {
                    target: 30,
                    limit: 256,
                },
            },
        );
        if store.engine().unwrap().timeline().unwrap().progress.phase == Some(Phase::P4) {
            let fixture = Input::parse(EMPTY_AIR).unwrap();
            assert_eq!(fixture.target, 30);
            assert!(fixture.sources.is_empty());
            let pending = store.engine().unwrap().background().unwrap().pending().unwrap();
            assert_eq!(pending.input.identity(), fixture.identity());
            return;
        }
    }
    panic!("P4 was not reached within the bounded advance count");
}

fn empty_air_identity() -> String {
    let input = Input::parse(EMPTY_AIR).unwrap();
    assert_eq!(input.target, 30);
    assert!(input.sources.is_empty());
    input.identity()
}

fn empty_air_60_identity() -> String {
    let input = Input::parse(EMPTY_AIR_60).unwrap();
    assert_eq!(input.target, 60);
    assert!(input.sources.is_empty());
    input.identity()
}

fn execute_background(store: &mut Store, command: BackgroundCommand) {
    execute(store, Action::Background { command });
}

#[test]
fn empty_store_is_no_work_and_real_pending_selection_is_fallback_safe_and_revalidated() {
    let case = new_case("selector-stale");
    let world = case.join("world");
    let mut store = create_store(&world);
    let empty_sequence = store.committed_sequence();
    assert_eq!(select_required(&store, false).unwrap(), Selection::NoWork);
    assert_eq!(store.committed_sequence(), empty_sequence);

    prepare_p4(&mut store);
    assert_eq!(select_required(&store, true).unwrap(), Selection::Busy);
    let selected = match select_required(&store, false).unwrap() {
        Selection::Selected(selected) => selected,
        other => panic!("expected candidate selection, got {other:?}"),
    };
    assert_eq!(selected.job(), 1);
    assert_eq!(selected.mode(), SelectionMode::RegistrySelected);
    validate_required(&store, &selected, false).unwrap();

    let fallback = match select_required_with_missing_hint(&store, false).unwrap() {
        Selection::Selected(selected) => selected,
        other => panic!("missing hint must preserve required work, got {other:?}"),
    };
    assert_eq!(fallback.job(), 1);
    assert_eq!(fallback.mode(), SelectionMode::MandatoryFallback);
    validate_required(&store, &fallback, false).unwrap();

    let job_before_sequence_only = store
        .engine()
        .unwrap()
        .background()
        .unwrap()
        .pending()
        .unwrap()
        .clone();
    execute_background(&mut store, BackgroundCommand::Restart);
    let job_after_sequence_only = store
        .engine()
        .unwrap()
        .background()
        .unwrap()
        .pending()
        .unwrap()
        .clone();
    assert_eq!(job_after_sequence_only, job_before_sequence_only);
    assert_eq!(
        validate_required(&store, &fallback, false),
        Err(DueError::SelectionChanged)
    );

    let binding_stale = match select_required_with_missing_hint(&store, false).unwrap() {
        Selection::Selected(selected) => selected,
        other => panic!("real pending duty must remain selectable after unrelated commit: {other:?}"),
    };
    execute_background(&mut store, BackgroundCommand::RebindLease { job: 1 });
    assert_eq!(
        validate_required(&store, &binding_stale, false),
        Err(DueError::SelectionChanged)
    );
}

#[test]
fn recovered_running_job_with_no_live_owner_is_mandatory_fallback() {
    let case = new_case("recovered-running-fallback");
    let world = case.join("world");
    let mut store = create_store(&world);
    prepare_p4(&mut store);
    execute_background(&mut store, BackgroundCommand::RebindLease { job: 1 });
    let running = store.engine().unwrap().background().unwrap().pending().unwrap().clone();
    assert!(running.running);
    drop(store);

    let store = Store::open(&world, RULES).unwrap();
    let selected = match select_required(&store, false).unwrap() {
        Selection::Selected(selected) => selected,
        other => panic!("persisted running job without live owner must remain required: {other:?}"),
    };
    assert_eq!(selected.job(), running.id);
    assert_eq!(selected.mode(), SelectionMode::MandatoryFallback);
    validate_required(&store, &selected, false).unwrap();
}

#[test]
fn equal_sequence_scope_and_window_do_not_match_a_different_full_failure_binding() {
    let case = new_case("full-binding-mismatch");
    let world_a = case.join("world-a");
    let world_b = case.join("world-b");
    let mut a = create_store(&world_a);
    let mut b = create_store(&world_b);
    prepare_p4(&mut a);
    prepare_p4(&mut b);
    execute_background(&mut a, BackgroundCommand::RebindLease { job: 1 });
    execute_background(&mut b, BackgroundCommand::RebindLease { job: 1 });
    execute_background(
        &mut a,
        BackgroundCommand::Fail {
            job: 1,
            generation: 1,
            failure: Failure::Transient,
        },
    );
    execute_background(
        &mut b,
        BackgroundCommand::Fail {
            job: 1,
            generation: 1,
            failure: Failure::Cancelled,
        },
    );
    assert_eq!(a.committed_sequence(), b.committed_sequence());
    let scope_a = a.engine().unwrap().world().scope_revision().0;
    let scope_b = b.engine().unwrap().world().scope_revision().0;
    assert_eq!(scope_a, scope_b);
    let job_a = a.engine().unwrap().background().unwrap().pending().unwrap();
    let job_b = b.engine().unwrap().background().unwrap().pending().unwrap();
    assert_eq!(job_a.id, job_b.id);
    assert_eq!(job_a.start, job_b.start);
    assert_eq!(job_a.end, job_b.end);
    assert_eq!(job_a.base_revision, job_b.base_revision);
    assert_eq!(job_a.field_version, job_b.field_version);
    assert_eq!(job_a.generation, job_b.generation);
    assert_eq!(job_a.attempt, job_b.attempt);
    assert_eq!(job_a.incarnation, job_b.incarnation);
    assert_eq!(job_a.checkpoint_sha256, job_b.checkpoint_sha256);
    assert_eq!(job_a.input.identity(), job_b.input.identity());
    assert_eq!(job_a.running, job_b.running);
    assert_eq!(job_a.failure, Some(Failure::Transient));
    assert_eq!(job_b.failure, Some(Failure::Cancelled));
    let selection = match select_required_with_missing_hint(&a, false).unwrap() {
        Selection::Selected(selection) => selection,
        other => panic!("source store must retain its required job: {other:?}"),
    };
    assert_eq!(
        validate_required(&b, &selection, false),
        Err(DueError::SelectionChanged)
    );
}

#[test]
fn hard_attempt_limit_retains_the_real_pending_obligation_as_backpressure() {
    let case = new_case("attempt-backpressure");
    let world = case.join("world");
    let mut store = create_store(&world);
    prepare_p4(&mut store);
    for generation in 1..=3 {
        execute_background(&mut store, BackgroundCommand::RebindLease { job: 1 });
        execute_background(
            &mut store,
            BackgroundCommand::Fail {
                job: 1,
                generation,
                failure: Failure::Cancelled,
            },
        );
    }
    assert_eq!(
        select_required(&store, false).unwrap(),
        Selection::Backpressure(RequiredBlock::AttemptLimitReached)
    );
    assert_eq!(select_required(&store, true).unwrap(), Selection::Busy);
    assert_eq!(store.engine().unwrap().background().unwrap().pending().unwrap().attempt, 3);
    assert_eq!(RequiredBlock::AttemptLimitReached.as_code(), "attempt_limit_reached");
}

#[test]
fn invalid_input_failure_is_backpressure_without_consuming_the_job() {
    let case = new_case("invalid-input-backpressure");
    let world = case.join("world");
    let mut store = create_store(&world);
    prepare_p4(&mut store);
    execute_background(&mut store, BackgroundCommand::RebindLease { job: 1 });
    execute_background(
        &mut store,
        BackgroundCommand::Fail {
            job: 1,
            generation: 1,
            failure: Failure::InvalidInput,
        },
    );
    assert_eq!(
        select_required(&store, false).unwrap(),
        Selection::Backpressure(RequiredBlock::InvalidInputFailure)
    );
    let pending = store.engine().unwrap().background().unwrap().pending().unwrap();
    assert_eq!(pending.id, 1);
    assert_eq!(pending.failure, Some(Failure::InvalidInput));
    assert_eq!(RequiredBlock::InvalidInputFailure.as_code(), "invalid_input_failure");
}

struct SimRun {
    client: Client,
    request_id: u64,
    output: BufWriter<File>,
    event_count: usize,
    transcript_bytes: usize,
    worker_started_count: usize,
}

impl SimRun {
    fn start(executable: &Path, world: &Path, transcript: &Path, create: bool) -> Self {
        let (file, event_count, transcript_bytes) = if create {
            let file =
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(transcript)
                .expect("create retained transcript without overwrite");
            (file, 0, 0)
        } else {
            let bytes = fs::read(transcript).expect("read only this case's retained transcript");
            let event_count = bytes.iter().filter(|byte| **byte == b'\n').count();
            let transcript_bytes = bytes.len();
            assert!(event_count <= MAX_EVENTS, "retained transcript already exceeds event bound");
            assert!(transcript_bytes <= MAX_TRANSCRIPT_BYTES, "retained transcript already exceeds byte bound");
            let file = OpenOptions::new()
                .append(true)
                .open(transcript)
                .expect("append to retained transcript");
            (file, event_count, transcript_bytes)
        };
        let mut client = Client::new();
        client
            .start_probe(executable, world, create)
            .expect("start only this test's isolated Sim");
        Self {
            client,
            request_id: 0,
            output: BufWriter::new(file),
            event_count,
            transcript_bytes,
            worker_started_count: 0,
        }
    }

    fn record(&mut self, value: &Value) {
        self.event_count += 1;
        assert!(self.event_count <= MAX_EVENTS, "bounded transcript event limit exceeded");
        let bytes = serde_json::to_vec(value).unwrap();
        assert!(bytes.len() <= MAX_EVENT_BYTES, "bounded event size exceeded");
        self.transcript_bytes = self.transcript_bytes.saturating_add(bytes.len() + 1);
        assert!(self.transcript_bytes <= MAX_TRANSCRIPT_BYTES, "bounded transcript size exceeded");
        self.output.write_all(&bytes).unwrap();
        self.output.write_all(b"\n").unwrap();
        self.output.flush().unwrap();
    }

    fn next(&mut self) -> Value {
        let event = self.client.next(Duration::from_secs(10)).expect("bounded Sim event wait");
        let value = serde_json::to_value(event).unwrap();
        if value["kind"] == "worker_started" {
            self.worker_started_count += 1;
            assert!(self.worker_started_count <= 8, "bounded worker count exceeded");
        }
        self.record(&value);
        value
    }

    fn call(&mut self, command: Command) -> Value {
        self.submit(command);
        self.wait_result()
    }

    fn submit(&mut self, command: Command) {
        self.request_id += 1;
        let id = self.request_id;
        self.record(&json!({"request_id": id, "command": command}));
        self.client.submit(id, command).unwrap();
    }

    fn wait_result(&mut self) -> Value {
        for _ in 0..24 {
            let event = self.next();
            if event["kind"] == "result" {
                return event["payload"].clone();
            }
        }
        panic!("bounded Sim result event count exceeded");
    }

    fn execute_action(&mut self, action: Action) -> Value {
        let before = self.call(Command::Status)["Ok"].clone();
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: before["sequence"].as_u64().unwrap() + 1,
            expected_revision: before["revision"].as_u64().unwrap(),
            actions: vec![action],
        };
        let result = self.call(Command::Execute { envelope });
        assert!(result.get("Ok").is_some(), "Sim Execute failed: {result}");
        assert!(
            result["Ok"]["model"].get("Ok").is_some(),
            "model action failed: {result}"
        );
        result["Ok"].clone()
    }

    fn prepare_p4(&mut self) {
        self.execute_action(Action::Timeline {
            command: TimelineCommand::Enable,
        });
        self.execute_action(Action::Background {
            command: BackgroundCommand::Enable,
        });
        for _ in 0..30 {
            self.execute_action(Action::Timeline {
                command: TimelineCommand::Advance {
                    target: 30,
                    limit: 256,
                },
            });
            let status = self.call(Command::Status)["Ok"].clone();
            if status["timeline"]["phase"] == "P4" {
                assert_eq!(status["background"]["pending"]["start"], 0);
                assert_eq!(status["background"]["pending"]["end"], 30);
                return;
            }
        }
        panic!("Sim Timeline did not reach P4 in bounded advances");
    }

    fn wait_event(&mut self, kind: &str) -> Value {
        for _ in 0..16 {
            let event = self.next();
            if event["kind"] == kind {
                return event;
            }
            assert_ne!(event["kind"], "result", "operation completed before {kind}");
        }
        panic!("bounded event scan did not find {kind}");
    }

    fn stop(&mut self) {
        let _ = self.call(Command::Stop);
        self.client.stop().expect("reap this test's Sim");
        self.output.flush().unwrap();
        self.output.get_ref().sync_all().unwrap();
    }
}

fn executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_mycity-headless"))
}

#[test]
fn run_due_no_work_is_read_only_and_p4_with_an_idle_child_still_runs_required() {
    let case = new_case("run-due-idle-child");
    let world = case.join("world");
    let transcript = case.join("events.jsonl");
    let exe = executable();
    let mut run = SimRun::start(exe, &world, &transcript, true);
    assert_eq!(run.next()["kind"], "ready");
    let before = run.call(Command::Status)["Ok"].clone();
    let worker_count_before_no_work = run.worker_started_count;
    let no_work = run.call(Command::RunDue);
    assert_eq!(no_work["Ok"]["dispatch"], "required_registry_v1");
    assert_eq!(no_work["Ok"]["state"], "no_work");
    assert_eq!(no_work["Ok"]["required_preserved"], true);
    let after = run.call(Command::Status)["Ok"].clone();
    assert_eq!(after["sequence"], before["sequence"]);
    assert_eq!(run.worker_started_count, worker_count_before_no_work);

    run.prepare_p4();
    let before_due = run.call(Command::Status)["Ok"].clone();
    run.submit(Command::RunDue);
    let first_started = run.wait_event("worker_started");
    assert_eq!(first_started["payload"]["registry"]["required_preserved"], true);
    let first_ticket = first_started["payload"]["ticket"].clone();
    let first_result = run.wait_event("result");
    assert!(first_result["payload"].get("Ok").is_some(), "RunDue failed: {first_result}");
    assert_eq!(first_result["payload"]["Ok"]["replayed"], false);
    assert_eq!(first_ticket["job"], 1);
    assert_eq!(first_ticket["start"], 0);
    assert_eq!(first_ticket["end"], 30);
    assert_eq!(first_ticket["input_sha256"], empty_air_identity());
    assert_eq!(first_result["payload"]["Ok"]["record"]["operation_id"], 0xe000_0000_0000_0001u64);
    let completed = run.call(Command::Status)["Ok"].clone();
    assert_eq!(completed["sequence"], before_due["sequence"].as_u64().unwrap() + 2);
    assert!(completed["background"]["pending"].is_null());
    assert_eq!(completed["timeline"]["target"], 30);
    assert_eq!(completed["timeline"]["last_complete"], 0);

    // RunDue accepts the current P4 obligation only. The owner explicitly
    // resumes the already-open Timeline target before starting a later window.
    for _ in 0..30 {
        let progress = run.call(Command::Status)["Ok"]["timeline"].clone();
        if progress["target"].is_null() && progress["last_complete"] == 30 {
            break;
        }
        run.execute_action(Action::Timeline {
            command: TimelineCommand::Advance {
                target: 30,
                limit: 256,
            },
        });
    }
    let drained = run.call(Command::Status)["Ok"]["timeline"].clone();
    assert!(drained["target"].is_null());
    assert_eq!(drained["last_complete"], 30);

    // The first accepted worker can leave an idle Child. A second real P4
    // occurrence must be selected from Store without treating that Child as busy.
    for _ in 0..30 {
        run.execute_action(Action::Timeline {
            command: TimelineCommand::Advance {
                target: 60,
                limit: 256,
            },
        });
        let status = run.call(Command::Status)["Ok"].clone();
        if status["timeline"]["phase"] == "P4" {
            break;
        }
    }
    let second_pending = run.call(Command::Status)["Ok"].clone();
    assert_eq!(second_pending["background"]["pending"]["id"], 2);
    assert_eq!(second_pending["background"]["pending"]["start"], 30);
    assert_eq!(second_pending["background"]["pending"]["end"], 60);
    run.submit(Command::RunDue);
    let second_started = run.wait_event("worker_started");
    assert_eq!(second_started["payload"]["ticket"]["job"], 2);
    assert_eq!(second_started["payload"]["ticket"]["input_sha256"], empty_air_60_identity());
    let second_result = run.wait_event("result");
    assert!(second_result["payload"].get("Ok").is_some(), "second RunDue failed: {second_result}");
    run.stop();
}

#[test]
fn run_due_after_cancel_and_cold_reopen_rebinds_the_actual_current_attempt() {
    let case = new_case("run-due-cold-rebind");
    let world = case.join("world");
    let transcript = case.join("events.jsonl");
    let exe = executable();
    let mut run = SimRun::start(exe, &world, &transcript, true);
    assert_eq!(run.next()["kind"], "ready");
    run.prepare_p4();
    run.request_id += 1;
    let id = run.request_id;
    run.record(&json!({"request_id":id,"command":{"kind":"probe_required","job":1,"point":"worker_cells"}}));
    run.client
        .submit(
            id,
            Command::ProbeRequired {
                job: 1,
                point: mycity_headless::sim_host::ProbePoint::WorkerCells,
            },
        )
        .unwrap();
    let started = run.wait_event("worker_started");
    let original_ticket = started["payload"]["ticket"].clone();
    let barrier = run.wait_event("probe_barrier");
    assert_eq!(barrier["payload"]["point"], "worker_cells");
    run.record(&json!({"probe_ack":{"point":"worker_cells","cancel":true}}));
    run.client
        .acknowledge_probe(mycity_headless::sim_host::ProbePoint::WorkerCells, true)
        .unwrap();
    let cancelled = run.wait_event("result");
    assert!(cancelled["payload"].get("Err").is_some());
    let status = run.call(Command::Status)["Ok"].clone();
    assert_eq!(status["timeline"]["phase"], "P4");
    assert_eq!(status["background"]["pending"]["failure"], "cancelled");
    assert_eq!(status["background"]["time"], 0);
    assert_eq!(status["background"]["pending"]["attempt"], 1);
    assert_eq!(status["background"]["pending"]["generation"], 1);
    run.stop();

    let mut reopened = SimRun::start(exe, &world, &transcript, false);
    assert_eq!(reopened.next()["kind"], "ready");
    reopened.submit(Command::RunDue);
    let rebound = reopened.wait_event("worker_started");
    let current_ticket = rebound["payload"]["ticket"].clone();
    assert_eq!(current_ticket["job"], original_ticket["job"]);
    assert_eq!(current_ticket["start"], original_ticket["start"]);
    assert_eq!(current_ticket["end"], original_ticket["end"]);
    assert_eq!(current_ticket["attempt"], 2);
    assert_eq!(current_ticket["generation"], 2);
    assert_ne!(current_ticket["attempt"], original_ticket["attempt"]);
    assert_ne!(current_ticket["generation"], original_ticket["generation"]);
    assert_ne!(current_ticket["incarnation"], original_ticket["incarnation"]);
    let result = reopened.wait_event("result");
    assert!(result["payload"].get("Ok").is_some(), "rebound RunDue failed: {result}");
    reopened.stop();
}

#[test]
fn run_due_attempt_limit_backpressure_preserves_the_live_sim_obligation() {
    let case = new_case("run-due-attempt-limit");
    let world = case.join("world");
    let transcript = case.join("events.jsonl");
    let mut run = SimRun::start(executable(), &world, &transcript, true);
    assert_eq!(run.next()["kind"], "ready");
    run.prepare_p4();

    for expected_attempt in 1..=3 {
        run.submit(Command::ProbeRequired {
            job: 1,
            point: mycity_headless::sim_host::ProbePoint::WorkerCells,
        });
        let started = run.wait_event("worker_started");
        assert_eq!(started["payload"]["ticket"]["attempt"], expected_attempt);
        assert_eq!(started["payload"]["ticket"]["generation"], expected_attempt);
        let barrier = run.wait_event("probe_barrier");
        assert_eq!(barrier["payload"]["point"], "worker_cells");
        run.record(&json!({"probe_ack":{"point":"worker_cells","cancel":true}}));
        run.client
            .acknowledge_probe(mycity_headless::sim_host::ProbePoint::WorkerCells, true)
            .unwrap();
        let cancelled = run.wait_event("result");
        assert!(cancelled["payload"].get("Err").is_some());
        let status = run.call(Command::Status)["Ok"].clone();
        assert_eq!(status["timeline"]["phase"], "P4");
        assert_eq!(status["background"]["time"], 0);
        assert_eq!(status["background"]["pending"]["id"], 1);
        assert_eq!(status["background"]["pending"]["attempt"], expected_attempt);
        assert_eq!(status["background"]["pending"]["generation"], expected_attempt);
        assert_eq!(status["background"]["pending"]["failure"], "cancelled");
    }

    let before = run.call(Command::Status)["Ok"].clone();
    let worker_count = run.worker_started_count;
    let blocked = run.call(Command::RunDue);
    assert_eq!(
        blocked["Err"],
        "sim_dispatch_backpressure:attempt_limit_reached"
    );
    let after = run.call(Command::Status)["Ok"].clone();
    assert_eq!(after["sequence"], before["sequence"]);
    assert_eq!(after["timeline"]["phase"], "P4");
    assert_eq!(after["background"]["time"], 0);
    assert_eq!(after["background"]["pending"]["id"], 1);
    assert_eq!(after["background"]["pending"]["attempt"], 3);
    assert_eq!(after["background"]["pending"]["generation"], 3);
    assert_eq!(after["background"]["pending"]["failure"], "cancelled");
    assert_eq!(run.worker_started_count, worker_count, "backpressure must not start another worker");
    run.stop();
}
