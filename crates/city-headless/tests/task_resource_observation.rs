//! Issue #102: retained original-owner facts; root alone builds and runs this preparation.
use mycity_core::{air::{Input, Source, State, STRATEGY}, background::Command as B,
    exchange::Command as X, schedule::Phase, sha256, timeline::Command as T,
    transaction::{Action, Envelope}};
use mycity_headless::{air_turns::{self, Quota}, compute_worker::{self as wire, Request, Stage, Ticket, Worker},
    normal_pool::{self, Process as Pool, Summary, BYTES, GRANT_BYTES}, sim_client::Client,
    sim_host::{Command, Event, PageControl}};
use serde_json::{json, Value};
use std::{fs, io::Write, path::{Path, PathBuf}, process::{Command as Child, Stdio},
    sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
static NEXT: AtomicU64 = AtomicU64::new(0);
const SIM_BYTES: usize = 59 * 1024 * 1024;
fn root(variable: &str, label: &str) -> PathBuf {
    let base = PathBuf::from(std::env::var_os(variable).expect("explicit retained SSD root required"))
        .canonicalize().unwrap();
    assert!(base.starts_with(Path::new("/Volumes/HP P900/dev/MyCity/work").canonicalize().unwrap()));
    let p = base.join(format!("{label}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&p).unwrap(); p
}
fn exe() -> PathBuf { PathBuf::from(std::env::var_os("MYCITY_RESOURCE_EXE").expect("frozen executable required")) }
fn save(p: &Path, name: &str, value: &impl serde::Serialize) {
    fs::write(p.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn no_capabilities(value: &Value) {
    match value {
        Value::Object(fields) => for (key, value) in fields {
            assert!(!["key", "sim_key", "worker_key", "broker_key"].contains(&key.as_str()), "capability field: {key}");
            no_capabilities(value);
        },
        Value::Array(values) => for value in values { no_capabilities(value); },
        _ => (),
    }
}
fn common(status: &Value) -> &Value {
    let report = &status["runtime"]["task_execution_observation"];
    assert_eq!(report["schema"], "mycity-task-execution-observation-v1");
    assert!(report["state"].is_null(), "bounded report unavailable: {report}");
    assert!(serde_json::to_vec(report).unwrap().len() <= 12 * 1024);
    no_capabilities(report); report
}
fn capacity(observation: &Value, state: &str) {
    let c = &observation["current_resources"]["original_contract_capacity"];
    assert_eq!(c["state"], state);
    if state == "known" { assert_eq!(c["bytes"], GRANT_BYTES); assert!(c["source"].as_str().unwrap().contains("original_normal_pool")); }
    else { assert!(c["bytes"].is_null()); }
}
fn returned(summary: Summary) {
    assert_eq!(summary.grants, summary.returns);
    assert_eq!(summary.workspace_grants, summary.workspace_returns);
    assert_eq!(summary.available_touched_bytes, BYTES);
}
struct Run { client: Client, id: u64, folder: PathBuf }
impl Run {
    fn new(folder: PathBuf, pool: Option<&Path>) -> Self {
        let mut client = Client::new();
        match pool {
            Some(pool) => client.start_in_pool(&exe(), &folder.join("world"), true, false, pool).unwrap(),
            None => client.start(&exe(), &folder.join("world"), true).unwrap(),
        }
        let mut r = Self { client, id: 0, folder }; assert_eq!(r.next().kind, "ready"); r
    }
    fn next(&mut self) -> Event {
        let e = self.client.next(Duration::from_secs(15)).unwrap();
        let mut log = fs::OpenOptions::new().create(true).append(true).open(self.folder.join("events.jsonl")).unwrap();
        writeln!(log, "{}", serde_json::to_string(&e).unwrap()).unwrap(); e
    }
    fn begin(&mut self, command: Command) { self.id += 1; self.client.submit(self.id, command).unwrap(); }
    fn call(&mut self, command: Command) -> Value {
        self.begin(command); loop { let e = self.next(); if e.kind == "result" {
            assert!(e.payload.get("Err").is_none(), "{e:?}"); return e.payload["Ok"].clone();
        }}
    }
    fn action(&mut self, action: Action) {
        let status = self.call(Command::Status);
        let result = self.call(Command::Execute { envelope: Envelope { world: 1, branch: 1,
            operation_id: status["sequence"].as_u64().unwrap() + 1,
            expected_revision: status["revision"].as_u64().unwrap(), actions: vec![action] }});
        assert!(result["model"].get("Ok").is_some(), "{result}");
    }
    fn prepare(&mut self) {
        self.action(Action::Exchange { command: X::Dispatch { quote: 1 } });
        self.action(Action::Timeline { command: T::Enable }); self.action(Action::Background { command: B::Enable });
        for _ in 0..30 {
            self.action(Action::Timeline { command: T::Advance { target: 30, limit: 256 } });
            if serde_json::from_value::<Option<Phase>>(self.call(Command::Status)["timeline"]["phase"].clone()).unwrap() == Some(Phase::P4) { return; }
        }
        panic!("original P4 not reached");
    }
    fn pause(&mut self) -> Value {
        self.begin(Command::RunRequiredSliced { job: 1, plan: air_turns::PLAN_ID.into(),
            quota: Quota { source_limit: 1, cell_limit: 17, pages: 1 } });
        loop { let e = self.next(); assert_ne!(e.kind, "result", "{e:?}");
            if e.kind == "page_boundary" { return e.payload; }
        }
    }
    fn observe(&mut self) -> Value {
        self.client.control_pages(PageControl::Observe).unwrap(); let e = self.next();
        assert_eq!(e.kind, "page_control_result"); e.payload["Ok"].clone()
    }
    fn cancel(&mut self) {
        self.client.control_pages(PageControl::Cancel).unwrap();
        loop { let e = self.next(); if e.kind == "result" { assert!(e.payload.get("Err").is_some()); break; } }
    }
    fn stop(&mut self) { self.call(Command::Checkpoint); self.call(Command::Stop); self.client.stop().unwrap(); }
}
#[test]
fn pooled_sim_and_air_observe_existing_credit_and_retirement_history() {
    let folder = root("MYCITY_RESOURCE_TEST_ROOT", "pooled");
    let pool_root = root("MYCITY_RESOURCE_POOL_ROOT", "p").join("p");
    let mut pool = Pool::spawn(&exe(), &pool_root, None).unwrap();
    let mut run = Run::new(folder.clone(), Some(pool.root()));
    let initial = run.call(Command::Status); let report = common(&initial);
    let sim = &report["sim_role_resources"];
    assert_eq!(sim["schema"], "mycity-sim-role-resource-observation-v1");
    assert_eq!(sim["pid"], run.client.pid());
    assert!(sim["process_instance"].as_str().unwrap().parse::<u128>().unwrap() > 0);
    assert!(sim["original_role_epoch"].as_u64().unwrap() > 0);
    let admission = &sim["admission"];
    assert_eq!(admission["source"], "broker_authorize_receipt"); assert_eq!(admission["role_kind"], "sim_codec");
    assert_eq!(admission["authorized_bytes"], SIM_BYTES);
    assert!(admission["pool_boot"].as_str().unwrap().parse::<u128>().unwrap() > 0);
    assert!(admission["generation"].as_u64().unwrap() > 0);
    assert_eq!(sim["arena"]["activation"]["grant_bytes"], SIM_BYTES);
    assert!(sim["sample_start_elapsed_ns"].as_u64().is_some());
    assert!(sim["scope"].as_str().unwrap().contains("not_additive"));
    let before = normal_pool::status(pool.root()).unwrap(); assert_eq!(before.workspace_capacity_bytes, SIM_BYTES);
    assert_eq!(before.granted_capacity_bytes, SIM_BYTES);
    save(&folder, "initial-status.json", &initial); save(&folder, "initial-pool.json", &before);
    run.prepare(); let first = run.pause(); let paused = run.observe(); let air = &paused["task_observation"];
    let current = &air["current_resources"];
    assert_eq!(current["phase"], "original_owner_acknowledged"); capacity(air, "known");
    assert_eq!(current["identity"]["kind"], "broker_lease");
    assert_eq!(current["identity"]["pool_boot"], admission["pool_boot"]);
    assert_eq!(current["identity"]["generation"], admission["generation"]);
    assert_eq!(current["identity"]["lease_id"], paused["worker_owner"]["broker_lease"]);
    assert_eq!(current["identity"], air["data"]["execution"]["allocation"]);
    assert_eq!(current["has_active_execution"], true); assert_eq!(current["ready_to_submit"], true);
    let held = normal_pool::status(pool.root()).unwrap(); assert_eq!(held.workspace_capacity_bytes, SIM_BYTES);
    assert_eq!(held.granted_capacity_bytes, SIM_BYTES + GRANT_BYTES);
    assert_eq!(held.grants, held.returns + 1);
    // Both capacities refer to the original ledger; arena/requested heap is not another grant.
    assert_eq!(held.available_touched_bytes + SIM_BYTES + GRANT_BYTES, BYTES);
    save(&folder, "held-pool.json", &held); save(&folder, "paused-air.json", &paused);
    run.cancel(); let cancelled = run.call(Command::Status); let common_air = &common(&cancelled)["air"]["observation"];
    assert_eq!(common_air["current_resources"]["phase"], "no_current_owner"); capacity(common_air, "unknown");
    assert!(common_air["current_resources"]["pid"].is_null());
    assert_eq!(common_air["data"], air["data"]);
    assert_eq!(common_air["historical_contract_capacity"], air["historical_contract_capacity"]);
    assert_eq!(common_air["historical_contract_capacity"]["bytes"], GRANT_BYTES);
    assert_eq!(common_air["current_resources"], cancelled["runtime"]["worker_task_observation"]["current_resources"]);
    let retired_air = normal_pool::status(pool.root()).unwrap(); assert_eq!(retired_air.grants, retired_air.returns);
    assert_eq!(retired_air.available_touched_bytes + SIM_BYTES, BYTES);
    save(&folder, "cancelled-status.json", &cancelled); save(&folder, "air-retired-pool.json", &retired_air);
    let retry = run.pause(); let retry_observation = run.observe();
    assert_ne!(retry["boundary"]["process_instance"], first["boundary"]["process_instance"]);
    assert_ne!(retry["ticket"], first["ticket"]);
    assert_ne!(retry_observation["task_observation"]["current_resources"]["identity"], current["identity"]);
    assert_eq!(retry_observation["task_observation"]["data"]["execution"]["allocation"],
        retry_observation["task_observation"]["current_resources"]["identity"]);
    run.cancel(); run.stop(); let final_pool = normal_pool::status(pool.root()).unwrap(); returned(final_pool);
    save(&folder, "final-pool.json", &final_pool); pool.finish().unwrap();
}
#[test]
fn standalone_sim_local_arena_and_unpooled_air_have_no_invented_pool_capacity() {
    assert!(std::env::var_os("MYCITY_NORMAL_POOL").is_none(), "standalone test requires clean parent environment");
    let folder = root("MYCITY_RESOURCE_TEST_ROOT", "standalone"); let mut run = Run::new(folder.clone(), None);
    let status = run.call(Command::Status); let sim = &common(&status)["sim_role_resources"];
    assert_eq!(sim["admission"]["source"], "standalone_local_activation");
    assert_eq!(sim["admission"]["local_arena_bytes"], SIM_BYTES);
    assert!(sim["admission"]["authorized_bytes"].is_null()); assert!(sim["admission"]["pool_boot"].is_null());
    run.prepare(); run.pause(); let paused = run.observe(); let air = &paused["task_observation"];
    assert_eq!(air["current_resources"]["phase"], "unpooled_owner");
    assert_eq!(air["current_resources"]["identity"]["kind"], "unpooled"); capacity(air, "unknown");
    assert_eq!(air["historical_contract_capacity"]["state"], "unknown");
    save(&folder, "initial-status.json", &status); save(&folder, "paused-air.json", &paused);
    run.cancel(); common(&run.call(Command::Status)); run.stop();
}
fn raw_request(root: &Path, attempt: u8) -> Request {
    fs::create_dir(root.join("inputs")).unwrap(); fs::create_dir(root.join("candidates")).unwrap();
    let state = State::new(Input { schema: STRATEGY.into(), side: 32, target: 60, initial_micro: 2000000,
        boundary_micro: [Some(2000000); 4], boundary_until: 60,
        sources: (0..17).map(|id| Source { id: id + 1, cell: id, start: 0, end: 60, rate_micro: 10 }).collect() }).unwrap();
    Request { protocol: wire::PROTOCOL.into(), strategy: STRATEGY.into(), controlled: None, ticket: Ticket {
        world: 1, branch: 1, job: 1, incarnation: 1, generation: 1, attempt, base_revision: 0, field_version: 0,
        start: 0, end: 60, rules_sha256: sha256(b"resource-direct"), source_sha256: env!("MYCITY_SOURCE_SHA256").into(),
        input_sha256: state.input().identity(), checkpoint_sha256: wire::publish(&root.join("inputs"), &state.checkpoint().unwrap()).unwrap() } }
}
#[test]
fn direct_air_original_prepare_attach_and_wait_return_contract_capacity() {
    let folder = root("MYCITY_RESOURCE_TEST_ROOT", "direct"); let pool_root = root("MYCITY_RESOURCE_POOL_ROOT", "d").join("p");
    let mut pool = Pool::spawn(&exe(), &pool_root, None).unwrap();
    let before = normal_pool::status(pool.root()).unwrap(); returned(before);
    let mut child = Child::new(std::env::current_exe().unwrap())
        .args(["direct_air_resource_helper", "--ignored", "--exact", "--nocapture"])
        .env("MYCITY_RESOURCE_HELPER_ROOT", &folder).env("MYCITY_NORMAL_POOL", pool.root())
        .env_remove("MYCITY_BROKER_SESSION").env_remove("MYCITY_BROKER_SUPERVISOR")
        .stdout(Stdio::from(fs::File::create(folder.join("helper.stdout")).unwrap()))
        .stderr(Stdio::from(fs::File::create(folder.join("helper.stderr")).unwrap())).spawn().unwrap();
    let started = Instant::now(); let status = loop {
        if let Some(status) = child.try_wait().unwrap() { break status; }
        if started.elapsed() > Duration::from_secs(60) {
            child.kill().unwrap(); child.wait().unwrap(); panic!("direct helper timeout; retained artifacts at {}", folder.display());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    save(&folder, "helper-exit.json", &json!({"pid":child.id(), "code":status.code()}));
    assert!(status.success(), "read retained helper stdout/stderr");
    let after = normal_pool::status(pool.root()).unwrap(); returned(after);
    assert_eq!(after.grants, before.grants + 2); assert_eq!(after.returns, before.returns + 2);
    assert_eq!(after.workspace_grants, before.workspace_grants);
    save(&folder, "initial-pool.json", &before); save(&folder, "final-pool.json", &after); pool.finish().unwrap();
}
#[test]
#[ignore = "spawned only by direct parent with isolated environment"]
fn direct_air_resource_helper() {
    let folder = PathBuf::from(std::env::var_os("MYCITY_RESOURCE_HELPER_ROOT").unwrap());
    assert!(std::env::var_os("MYCITY_BROKER_SESSION").is_none());
    let pool = PathBuf::from(std::env::var_os("MYCITY_NORMAL_POOL").unwrap());
    let mut worker = Worker::new(); let mut previous_identity = Value::Null;
    for attempt in 1..=2 {
        let work_root = folder.join(format!("air-{attempt}")); fs::create_dir(&work_root).unwrap();
        let request = raw_request(&work_root, attempt);
        worker.start_bounded(&exe(), &work_root, None, false).unwrap();
        let held = worker.task_observation(); capacity(&held, "known");
        assert_eq!(held["current_resources"]["phase"], "original_owner_acknowledged");
        assert_eq!(held["current_resources"]["identity"]["kind"], "direct_grant");
        assert_ne!(held["current_resources"]["identity"], previous_identity);
        let pool_held = normal_pool::status(&pool).unwrap(); assert_eq!(pool_held.granted_capacity_bytes, GRANT_BYTES);
        assert_eq!(pool_held.available_touched_bytes + GRANT_BYTES, BYTES);
        worker.submit(&request).unwrap(); let log = folder.join(format!("direct-events-{attempt}.jsonl"));
        loop {
            let e = worker.next(Duration::from_secs(15)).unwrap();
            let mut file = fs::OpenOptions::new().create(true).append(true).open(&log).unwrap();
            writeln!(file, "{}", serde_json::to_string(&e).unwrap()).unwrap();
            if e.stage == Stage::Complete { break; }
        }
        let complete = worker.task_observation(); no_capabilities(&complete);
        assert!(complete["data"].is_object());
        assert_eq!(complete["data"]["execution"]["allocation"], held["current_resources"]["identity"]);
        assert_eq!(complete["data"]["request"]["ticket"]["attempt"], attempt);
        worker.retire(Duration::from_secs(10)).unwrap(); assert!(worker.is_empty());
        let retired = worker.task_observation(); capacity(&retired, "unknown");
        assert_eq!(retired["current_resources"]["phase"], "no_current_owner");
        assert!(retired["current_resources"]["pid"].is_null()); assert_eq!(retired["data"], complete["data"]);
        assert_eq!(retired["historical_contract_capacity"]["bytes"], GRANT_BYTES);
        returned(normal_pool::status(&pool).unwrap());
        save(&folder, &format!("direct-{attempt}.json"), &json!({"held":held,"pool_held":pool_held,"complete":complete,"retired":retired}));
        previous_identity = held["current_resources"]["identity"].clone();
    }
}
