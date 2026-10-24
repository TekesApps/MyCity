use mycity_core::{
    air::Input,
    background::Command as BackgroundCommand,
    schedule::Phase,
    timeline::Command as TimelineCommand,
    transaction::{Action, Envelope},
};
use mycity_headless::{sim_client::Client, sim_host::Command};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ROOT_ENV: &str = "MYCITY_ISSUE101_DISPATCH_ROOT";
const EXTERNAL_WORK: &str = "/Volumes/HP P900/dev/MyCity/work";
const ACCEPT_PREFIX: u64 = 0xe000_0000_0000_0000;
const AIR_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/issue-101/air-empty-sources-30.json");
const MAX_TRANSCRIPT_EVENTS: usize = 512;
const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 256 * 1024;

fn issue_root() -> PathBuf {
    let configured = std::env::var_os(ROOT_ENV)
        .unwrap_or_else(|| panic!("{ROOT_ENV} must point to an existing dedicated HP P900 run root"));
    let root = PathBuf::from(configured)
        .canonicalize()
        .unwrap_or_else(|error| panic!("cannot canonicalize {ROOT_ENV}: {error}"));
    let work = Path::new(EXTERNAL_WORK)
        .canonicalize()
        .unwrap_or_else(|error| panic!("external work volume is unavailable: {error}"));
    assert!(root.starts_with(&work), "dispatch evidence root must remain under {work:?}");
    assert_ne!(root, work, "dispatch root must be a dedicated existing subdirectory");
    assert!(root.is_dir(), "dispatch evidence root must already exist");
    root
}

fn new_case_root(root: &Path, name: &str, executable: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_nanos();
    let case = root.join(format!("{name}-{}-{timestamp}", std::process::id()));
    fs::create_dir(&case).expect("create unique case directory without overwrite");
    let world = case.join("world");
    let transcript = case.join("events.jsonl");
    let executable = executable.canonicalize().expect("canonicalize test executable");
    let manifest = json!({
        "schema": "mycity-issue101-dispatch-test-run-v1",
        "case": name,
        "executable": executable,
        "world": world,
        "evidence_root": case,
        "world_preserved": true,
        "transcript_preserved": true
    });
    let manifest_path = case.join("run.json");
    let mut manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(manifest_path)
        .expect("create manifest without overwriting prior evidence");
    manifest_file
        .write_all(&serde_json::to_vec_pretty(&manifest).unwrap())
        .unwrap();
    manifest_file.sync_all().unwrap();
    (case, world, transcript)
}

struct Run {
    client: Client,
    command_id: u64,
    transcript: BufWriter<File>,
    event_count: usize,
    transcript_bytes: usize,
    worker_tickets: Vec<Value>,
}

impl Run {
    fn new(executable: &Path, world: &Path, transcript: &Path, create: bool) -> Self {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(transcript)
            .or_else(|error| {
                if !create && error.kind() == std::io::ErrorKind::AlreadyExists {
                    OpenOptions::new().append(true).open(transcript)
                } else {
                    Err(error)
                }
            })
            .expect("open append-only retained event transcript");
        let mut client = Client::new();
        client.start_probe(executable, world, create).expect("spawn isolated probe Sim");
        Self {
            client,
            command_id: 0,
            transcript: BufWriter::new(file),
            event_count: 0,
            transcript_bytes: 0,
            worker_tickets: Vec::new(),
        }
    }

    fn write_record(&mut self, record: &Value) {
        self.event_count += 1;
        assert!(self.event_count <= MAX_TRANSCRIPT_EVENTS, "bounded transcript event count exceeded");
        let line = serde_json::to_vec(record).unwrap();
        assert!(line.len() <= MAX_EVENT_BYTES, "bounded transcript event size exceeded");
        self.transcript_bytes = self.transcript_bytes.saturating_add(line.len() + 1);
        assert!(self.transcript_bytes <= MAX_TRANSCRIPT_BYTES, "bounded transcript size exceeded");
        self.transcript.write_all(&line).unwrap();
        self.transcript.write_all(b"\n").unwrap();
        self.transcript.flush().unwrap();
    }

    fn next(&mut self) -> Value {
        let event = self.client.next(Duration::from_secs(10)).expect("bounded Sim event wait");
        let value = serde_json::to_value(&event).unwrap();
        if event.kind == "worker_started" {
            assert!(self.worker_tickets.len() < 8, "bounded worker ticket collection exceeded");
            self.worker_tickets.push(event.payload["ticket"].clone());
        }
        self.write_record(&json!({"event": value}));
        value
    }

    fn submit(&mut self, command: Command) {
        self.command_id += 1;
        let id = self.command_id;
        self.write_record(&json!({"request_id": id, "command": command}));
        self.client.submit(id, command).expect("submit one serialized Sim command");
    }

    fn result(&mut self) -> Value {
        for _ in 0..24 {
            let event = self.next();
            if event["kind"] == "result" {
                return event["payload"].clone();
            }
        }
        panic!("bounded result event limit exceeded");
    }

    fn call(&mut self, command: Command) -> Value {
        self.submit(command);
        let result = self.result();
        assert!(result.get("Ok").is_some(), "Sim command failed: {result}");
        result["Ok"].clone()
    }

    fn execute(&mut self, action: Action) {
        let status = self.call(Command::Status);
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: status["sequence"].as_u64().unwrap() + 1,
            expected_revision: status["revision"].as_u64().unwrap(),
            actions: vec![action],
        };
        let outcome = self.call(Command::Execute { envelope });
        assert!(outcome["model"].get("Ok").is_some(), "model action failed: {outcome}");
    }

    fn prepare_empty_air_interval(&mut self) {
        self.execute(Action::Timeline { command: TimelineCommand::Enable });
        self.execute(Action::Background { command: BackgroundCommand::Enable });
        for _ in 0..30 {
            self.execute(Action::Timeline {
                command: TimelineCommand::Advance { target: 30, limit: 256 },
            });
            let status = self.call(Command::Status);
            if serde_json::from_value::<Option<Phase>>(status["timeline"]["phase"].clone()).unwrap()
                == Some(Phase::P4)
            {
                let pending = &status["background"]["pending"];
                assert_eq!(pending["id"], 1);
                assert_eq!(pending["start"], 0);
                assert_eq!(pending["end"], 30);
                return;
            }
        }
        panic!("P4 not reached within bounded timeline advances");
    }

    fn worker_started(&mut self) -> Value {
        for _ in 0..8 {
            let event = self.next();
            if event["kind"] == "worker_started" {
                return event;
            }
            assert_ne!(event["kind"], "result", "required operation ended before worker start");
        }
        panic!("worker_started event absent within bounded scan");
    }

    fn probe_barrier(&mut self) -> Value {
        for _ in 0..16 {
            let event = self.next();
            if event["kind"] == "probe_barrier" {
                return event;
            }
            assert_ne!(event["kind"], "result", "required operation ended before WorkerCells barrier");
        }
        panic!("WorkerCells probe barrier absent within bounded scan");
    }

    fn acknowledge(&mut self, cancel: bool) {
        self.write_record(&json!({
            "probe_ack": {"point":"worker_cells", "cancel":cancel, "request_id":self.command_id}
        }));
        self.client
            .acknowledge_probe(mycity_headless::sim_host::ProbePoint::WorkerCells, cancel)
            .expect("acknowledge only this Sim's WorkerCells probe");
    }

    fn replace_after_stop(&mut self, executable: &Path, world: &Path) {
        self.client.start_probe(executable, world, false).expect("reopen preserved world");
        let ready = self.next();
        assert_eq!(ready["kind"], "ready");
    }

    fn stop(&mut self) {
        let _ = self.call(Command::Stop);
        self.client.stop().expect("reap this Sim only");
        self.transcript.flush().unwrap();
        self.transcript.get_ref().sync_all().unwrap();
    }
}

fn empty_air_identity() -> String {
    let input = Input::parse(AIR_FIXTURE).unwrap();
    assert_eq!(input.target, 30);
    assert!(input.sources.is_empty());
    input.identity()
}

fn assert_registry_observation(event: &Value, observed_attempt: u64) {
    let registry = &event["payload"]["registry"];
    assert_eq!(registry["schema"], "mycity-task-registry-observation-v1");
    assert_eq!(registry["state"], "Candidate");
    assert_eq!(registry["snapshot_token_durable"], false);
    assert_eq!(registry["required_preserved"], true);
    assert_eq!(registry["world"], 1);
    assert_eq!(registry["branch"], 1);
    assert_eq!(registry["clock"], "world");
    assert_eq!(registry["now"], 30);
    assert_eq!(registry["job"], 1);
    assert_eq!(registry["start"], 0);
    assert_eq!(registry["end"], 30);
    assert_eq!(registry["generation"], observed_attempt);
    assert_eq!(registry["attempt"], observed_attempt);
    assert_eq!(registry["rebound"]["state"], "NoHint(AlreadyRunning)");
    assert_eq!(registry["rebound"]["required_preserved"], true);
}

fn assert_ticket_scope_window_and_empty_input(ticket: &Value, expected_attempt: u64) {
    assert_eq!(ticket["world"], 1);
    assert_eq!(ticket["branch"], 1);
    assert_eq!(ticket["job"], 1);
    assert_eq!(ticket["start"], 0);
    assert_eq!(ticket["end"], 30);
    assert_eq!(ticket["attempt"], expected_attempt);
    assert_eq!(ticket["generation"], expected_attempt);
    assert_eq!(ticket["field_version"], 0);
    assert_eq!(ticket["input_sha256"], empty_air_identity());
}

#[test]
fn actual_required_dispatch_emits_observation_and_cold_replay_keeps_one_receipt() {
    let root = issue_root();
    let executable = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let (_case, world, transcript) = new_case_root(&root, "empty-air-required", executable);
    let mut run = Run::new(executable, &world, &transcript, true);
    assert_eq!(run.next()["kind"], "ready");
    run.prepare_empty_air_interval();

    let before = run.call(Command::Status);
    let sequence_before_required = before["sequence"].as_u64().unwrap();
    run.submit(Command::RunRequired { job: 1 });
    let started = run.worker_started();
    assert_registry_observation(&started, 0);
    let ticket = started["payload"]["ticket"].clone();
    assert_ticket_scope_window_and_empty_input(&ticket, 1);
    let registry = &started["payload"]["registry"];
    assert_eq!(registry["rebound"]["generation"], ticket["generation"]);
    assert_eq!(registry["rebound"]["attempt"], ticket["attempt"]);
    assert_eq!(registry["rebound"]["job"], ticket["job"]);
    assert_eq!(registry["rebound"]["start"], ticket["start"]);
    assert_eq!(registry["rebound"]["end"], ticket["end"]);
    assert!(run.worker_tickets.len() == 1);
    let result = run.result();
    assert!(result.get("Ok").is_some(), "required run failed: {result}");
    let first = result["Ok"].clone();
    assert_eq!(first["replayed"], false);
    let literal_operation = ACCEPT_PREFIX | 1;
    assert_eq!(first["record"]["operation_id"], literal_operation);
    assert_eq!(first["record"]["outcome"]["Ok"]["operation_id"], literal_operation);

    let committed = run.call(Command::Status);
    assert_eq!(committed["sequence"], sequence_before_required + 2);
    assert_eq!(committed["background"]["time"], 30);
    assert_eq!(committed["background"]["version"], 1);
    assert!(committed["background"]["pending"].is_null());
    run.call(Command::Checkpoint);
    run.stop();

    run.replace_after_stop(executable, &world);
    let before_replay = run.call(Command::Status);
    let worker_count_before = run.worker_tickets.len();
    let replayed = run.call(Command::RunRequired { job: 1 });
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["record"], first["record"]);
    let after_replay = run.call(Command::Status);
    assert_eq!(after_replay["sequence"], before_replay["sequence"]);
    assert_eq!(run.worker_tickets.len(), worker_count_before, "receipt replay must not start another worker");
    run.stop();
}

#[test]
fn cancelled_required_attempt_preserves_obligation_then_retries_with_fresh_ticket() {
    let root = issue_root();
    let executable = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let (_case, world, transcript) = new_case_root(&root, "cancelled-air-required", executable);
    let mut run = Run::new(executable, &world, &transcript, true);
    assert_eq!(run.next()["kind"], "ready");
    run.prepare_empty_air_interval();
    let sequence_before_cancelled_required = run.call(Command::Status)["sequence"].as_u64().unwrap();

    run.submit(Command::ProbeRequired {
        job: 1,
        point: mycity_headless::sim_host::ProbePoint::WorkerCells,
    });
    let cancelled_started = run.worker_started();
    assert_registry_observation(&cancelled_started, 0);
    let cancelled_ticket = cancelled_started["payload"]["ticket"].clone();
    assert_ticket_scope_window_and_empty_input(&cancelled_ticket, 1);
    let registry = &cancelled_started["payload"]["registry"];
    assert_eq!(registry["rebound"]["generation"], cancelled_ticket["generation"]);
    assert_eq!(registry["rebound"]["attempt"], cancelled_ticket["attempt"]);
    assert_eq!(registry["rebound"]["state"], "NoHint(AlreadyRunning)");

    let barrier = run.probe_barrier();
    assert_eq!(barrier["payload"]["point"], "worker_cells");
    run.acknowledge(true);
    let cancelled_result = run.result();
    assert!(cancelled_result.get("Err").is_some(), "cancel acknowledgement must fail this attempt");
    assert_eq!(cancelled_result["Err"], "sim_attempt_cancelled");

    let after_cancel = run.call(Command::Status);
    assert_eq!(after_cancel["background"]["time"], 0);
    assert_eq!(after_cancel["background"]["pending"]["id"], 1);
    assert_eq!(after_cancel["background"]["pending"]["start"], 0);
    assert_eq!(after_cancel["background"]["pending"]["end"], 30);
    assert_eq!(after_cancel["background"]["pending"]["failure"], "cancelled");
    assert_eq!(after_cancel["background"]["pending"]["generation"], 1);
    assert_eq!(after_cancel["background"]["pending"]["attempt"], 1);
    assert_eq!(after_cancel["sequence"], sequence_before_cancelled_required + 2);
    assert_eq!(after_cancel["timeline"]["phase"], "P4");

    run.submit(Command::RunRequired { job: 1 });
    let retry_started = run.worker_started();
    assert_registry_observation(&retry_started, 1);
    let retry_ticket = retry_started["payload"]["ticket"].clone();
    let retry_registry = &retry_started["payload"]["registry"];
    assert_eq!(retry_registry["required_preserved"], true);
    assert_eq!(retry_registry["rebound"]["state"], "NoHint(AlreadyRunning)");
    assert_eq!(retry_registry["rebound"]["generation"], retry_ticket["generation"]);
    assert_eq!(retry_registry["rebound"]["attempt"], retry_ticket["attempt"]);
    assert_ticket_scope_window_and_empty_input(&retry_ticket, 2);
    assert_eq!(retry_ticket["job"], cancelled_ticket["job"]);
    assert_eq!(retry_ticket["start"], cancelled_ticket["start"]);
    assert_eq!(retry_ticket["end"], cancelled_ticket["end"]);
    assert_eq!(retry_ticket["input_sha256"], cancelled_ticket["input_sha256"]);
    assert_ne!(retry_ticket["generation"], cancelled_ticket["generation"]);
    assert_ne!(retry_ticket["attempt"], cancelled_ticket["attempt"]);
    assert_ne!(retry_ticket["incarnation"], cancelled_ticket["incarnation"]);
    assert!(run.result().get("Ok").is_some());

    let committed = run.call(Command::Status);
    assert_eq!(committed["sequence"], sequence_before_cancelled_required + 4);
    assert_eq!(committed["background"]["time"], 30);
    assert_eq!(committed["background"]["version"], 1);
    assert!(committed["background"]["pending"].is_null());
    let receipt = run.call(Command::RunRequired { job: 1 });
    assert_eq!(receipt["replayed"], true);
    assert_eq!(receipt["record"]["operation_id"], ACCEPT_PREFIX | 1);
    run.stop();
}
