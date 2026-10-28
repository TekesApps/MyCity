//! Exact-page intent goes through the ordinary task binding and page-control path.
//! Frozen executable, evidence roots and all events are retained by the harness.
use mycity_core::{background::Command as B, exchange::Command as X,
    timeline::Command as T, transaction::{Action, Envelope}};
use mycity_headless::{air_turns::{self, Boundary, Continue, Quota}, sim_client::Client,
    sim_host::{BoundTaskRequest, Command, Event, PageControl}};
use serde::{Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::Duration};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn evidence(name: &str) -> PathBuf {
    let root = PathBuf::from(std::env::var_os("MYCITY_TASK_REPORT_ROOT")
        .expect("frozen task evidence root required")).canonicalize().unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let path = root.join(format!("exact-pages-{name}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&path).unwrap(); path
}
fn exe() -> PathBuf { PathBuf::from(std::env::var_os("MYCITY_TASK_REPORT_EXE").expect("frozen current headless executable required")).canonicalize().unwrap() }
fn rules() -> String { format!("sim-w3:{}", env!("MYCITY_SOURCE_SHA256")) }
fn q(source_limit: u32, cell_limit: u32, pages: u16) -> Quota { Quota { source_limit, cell_limit, pages } }

struct Run { client: Client, id: u64, folder: PathBuf, events_path:PathBuf, events: Vec<Value> }
impl Run {
    fn start(folder: PathBuf, pool: &Path, create: bool) -> Self {
        let mut client = Client::new();
        client.start_in_pool(&exe(), &folder.join("world"), create, false, pool).unwrap();
        let events_path=folder.join(format!("events-session-{}.json",NEXT.fetch_add(1,Ordering::Relaxed)));
        let mut run = Self { client, id: 0, folder, events_path, events: Vec::new() };
        assert_eq!(run.next().kind, "ready"); run
    }
    fn next(&mut self) -> Event {
        let event = self.client.next(Duration::from_secs(20)).unwrap();
        self.events.push(serde_json::to_value(&event).unwrap());
        fs::write(&self.events_path, serde_json::to_vec_pretty(&self.events).unwrap()).unwrap();
        event
    }
    fn begin(&mut self, command: Command) { self.id += 1; self.client.submit(self.id, command).unwrap(); }
    fn call(&mut self, command: Command) -> Value {
        self.begin(command);
        loop { let e = self.next(); if e.kind == "result" { assert!(e.payload.get("Err").is_none(), "{e:?}"); return e.payload["Ok"].clone(); } }
    }
    fn action(&mut self, action: Action) {
        let status = self.call(Command::Status);
        let result = self.call(Command::Execute { envelope: Envelope { world: 1, branch: 1,
            operation_id: status["sequence"].as_u64().unwrap() + 1,
            expected_revision: status["revision"].as_u64().unwrap(), actions: vec![action] } });
        assert!(result["model"].get("Ok").is_some(), "{result}");
    }
    fn prepare(&mut self) {
        self.action(Action::Exchange { command: X::Dispatch { quote: 1 } });
        self.action(Action::Timeline { command: T::Enable });
        self.action(Action::Background { command: B::Enable });
        for _ in 0..32 {
            self.action(Action::Timeline { command: T::Advance { target: 30, limit: 256 } });
            if self.call(Command::Status)["timeline"]["phase"] == "P4" { return; }
        }
        panic!("required Air job did not reach P4");
    }
    fn boundary(&mut self) -> Boundary {
        loop { let e = self.next(); assert_ne!(e.kind, "result", "premature {e:?}");
            if e.kind == "page_boundary" { return serde_json::from_value(e.payload["boundary"].clone()).unwrap(); } }
    }
    fn control(&mut self, control: PageControl) { self.client.control_pages(control).unwrap(); }
    fn control_result(&mut self) -> Value { let e = self.next(); assert_eq!(e.kind, "page_control_result"); e.payload["Ok"].clone() }
    fn stop(&mut self) { self.call(Command::Stop); self.client.stop().unwrap(); }
}

fn make_pool(path: &Path, events: &Path) -> mycity_headless::normal_pool::Process {
    mycity_headless::normal_pool::Process::spawn(&exe(), path, Some(events)).unwrap()
}
fn exact_intent(request: &mut Value, version: u16, quota: Quota) {
    request["intent"] = json!({"kind":"execute_exact_pages", "plan_version":version, "quota":quota});
}
#[derive(Serialize)]
struct Accepted<'a> { encoding: &'static str, request: &'a BoundTaskRequest }
fn selected(run: &mut Run, request: Value) -> Value {
    let typed: BoundTaskRequest = serde_json::from_value(request).unwrap();
    let bytes = serde_json::to_vec(&Accepted { encoding:"mycity-accepted-task-execution-v1", request:&typed }).unwrap();
    let expected: [u8;32] = Sha256::digest(&bytes).into();
    run.begin(Command::RunBoundTask { binding: typed });
    let e = run.next(); assert_eq!(e.kind, "bound_task_selected", "{e:?}");
    assert_eq!(e.payload["accepted_execution_sha256"], json!(expected));
    assert_eq!(e.payload["accepted_execution_encoding"], "mycity-accepted-task-execution-v1");
    e.payload
}
fn clone_world(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap(); let dst = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() { clone_world(&entry.path(), &dst); }
        else { fs::copy(entry.path(), &dst).unwrap(); assert_eq!(fs::read(entry.path()).unwrap(), fs::read(dst).unwrap()); }
    }
}
fn candidate(status: &Value) -> Value { status["runtime"]["worker_task_observation"]["data"]["candidate"].clone() }
fn finish_exact(run:&mut Run,mut boundary:Boundary)->Value {
    for turn in 0..64 {
        let quota=if turn==0 {q(7,31,1)}else{q(8192,1024,16)};
        run.control(PageControl::Continue{next:Continue{plan_version:boundary.plan_version,turn:boundary.turn,
            process_instance:boundary.process_instance.clone(),quota}});
        loop {let e=run.next();
            if e.kind=="result" {assert!(e.payload.get("Err").is_none(),"{e:?}");return e.payload["Ok"].clone();}
            if e.kind=="page_boundary" {boundary=serde_json::from_value(e.payload["boundary"].clone()).unwrap();break;}
        }
    }
    panic!("finite exact-page request did not complete");
}

#[test]
fn exact_air_pages_pause_observe_prove_continue_and_match_original_required_candidate() {
    let folder = evidence("pause-reference"); let pool_root = PathBuf::from(std::env::var_os("MYCITY_TASK_PROTOCOL_POOL_ROOT").expect("dedicated SSD pool root required")).canonicalize().unwrap();
    assert!(pool_root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let pool_path = pool_root.join(format!("exact-pages-{}", std::process::id()));
    let mut pool = make_pool(&pool_path, &folder.join("pool-events.jsonl"));
    let mut reference = Run::start(evidence("ordinary"), pool.root(), true); reference.prepare();
    reference.call(Command::RunRequired { job: 1 }); let expected = candidate(&reference.call(Command::Status)); reference.stop();

    let mut run = Run::start(folder.clone(), pool.root(), true); run.prepare();
    let described = run.call(Command::DescribeTaskExecution);
    assert_eq!(described["request"]["intent"]["kind"], "execute_original");
    let mut request = described["request"].clone(); exact_intent(&mut request, air_turns::PLAN_VERSION, q(1, 17, 1));
    let first = selected(&mut run, request.clone());
    assert_eq!(first["initial_intent"]["kind"], "execute_exact_pages");
    let grant_at_accept = mycity_headless::normal_pool::status(pool.root()).unwrap();
    let boundary: Boundary = loop { let e=run.next(); assert_ne!(e.kind,"result","{e:?}");
        if e.kind=="page_boundary" { break serde_json::from_value(e.payload["boundary"].clone()).unwrap(); } };
    run.control(PageControl::Observe); let observed = run.control_result();
    assert_eq!(observed["still_unresolved"], true); assert_eq!(observed["task_observation"]["turn_waiting"], true);
    assert_eq!(observed["worker_owner"]["empty"],false);
    let grant_paused = mycity_headless::normal_pool::status(pool.root()).unwrap();
    assert_eq!(grant_paused.workspace_grants, grant_at_accept.workspace_grants);
    assert_eq!(grant_paused.workspace_returns, grant_at_accept.workspace_returns);
    run.control(PageControl::ReceiptProof { operation: 0xe000000000000001 }); assert_eq!(run.control_result()["present"], false);
    let final_result=finish_exact(&mut run,boundary);assert_eq!(final_result["bound_request"],first);
    let actual = candidate(&run.call(Command::Status)); assert_eq!(actual, expected);
    assert_eq!(run.events.iter().filter(|e|e["kind"]=="worker_started").count(), 1);
    let still_owned = mycity_headless::normal_pool::status(pool.root()).unwrap();
    assert!(still_owned.workspace_grants > still_owned.workspace_returns);
    run.stop();
    let retired = mycity_headless::normal_pool::status(pool.root()).unwrap();
    assert_eq!(retired.workspace_grants, retired.workspace_returns);
    assert_eq!(retired.available_touched_bytes,mycity_headless::normal_pool::BYTES);
    fs::write(folder.join("pool-observations.json"),serde_json::to_vec_pretty(&json!({"accepted":grant_at_accept,"paused":grant_paused,"before_sim_exit":still_owned,"retired":retired})).unwrap()).unwrap();
    pool.finish().unwrap();
}

#[test]
fn exact_pages_cancel_preserves_original_air_obligation_for_cold_explicit_resume() {
    let folder = evidence("cancel-restore"); let pool_root = PathBuf::from(std::env::var_os("MYCITY_TASK_PROTOCOL_POOL_ROOT").expect("dedicated SSD pool root required")).canonicalize().unwrap();
    assert!(pool_root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let pool_path = pool_root.join(format!("exact-pages-cancel-{}", std::process::id())); let mut pool = make_pool(&pool_path, &folder.join("pool-events.jsonl"));
    let mut run = Run::start(folder.clone(), pool.root(), true); run.prepare();
    let mut request = run.call(Command::DescribeTaskExecution)["request"].clone(); exact_intent(&mut request, air_turns::PLAN_VERSION, q(1, 17, 1));
    selected(&mut run, request); run.boundary(); run.control(PageControl::Cancel);
    loop { let e = run.next(); if e.kind == "result" { assert!(e.payload.get("Err").is_some(), "cancel must not report fulfillment: {e:?}"); break; } }
    let pending = run.call(Command::Status); assert_eq!(pending["background"]["time"], 0);
    assert_eq!(pending["background"]["pending"]["failure"], "cancelled");
    run.call(Command::Checkpoint);run.stop();
    clone_world(&folder.join("world"),&folder.join("cancelled-original"));drop(run);
    let mut restored = Run::start(folder.clone(), pool.root(), false); let recovered = restored.call(Command::Status);
    assert_eq!(recovered["background"]["pending"], pending["background"]["pending"]);
    let mut request = restored.call(Command::DescribeTaskExecution)["request"].clone(); exact_intent(&mut request, air_turns::PLAN_VERSION, q(1, 17, 1));
    selected(&mut restored, request); let boundary = restored.boundary();
    finish_exact(&mut restored,boundary);
    let final_status = restored.call(Command::Status); assert_eq!(final_status["background"]["time"], 30);
    assert!(final_status["background"]["pending"].is_null());
    restored.call(Command::Checkpoint); restored.stop();
    let retired = mycity_headless::normal_pool::status(pool.root()).unwrap();
    assert_eq!(retired.workspace_grants, retired.workspace_returns);
    assert_eq!(retired.available_touched_bytes,mycity_headless::normal_pool::BYTES);
    fs::write(folder.join("retired-pool.json"),serde_json::to_vec_pretty(&retired).unwrap()).unwrap();
    pool.finish().unwrap();
}

#[test]
fn exact_page_rejections_and_changed_plan_stamps_have_no_business_side_effects() {
    let folder = evidence("reject"); let pool_root = PathBuf::from(std::env::var_os("MYCITY_TASK_PROTOCOL_POOL_ROOT").expect("dedicated SSD pool root required")).canonicalize().unwrap();
    assert!(pool_root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let pool_path = pool_root.join(format!("exact-pages-reject-{}", std::process::id())); let mut pool = make_pool(&pool_path, &folder.join("pool-events.jsonl"));
    let mut run = Run::start(folder.clone(), pool.root(), true); run.prepare();
    run.call(Command::Checkpoint);clone_world(&folder.join("world"),&folder.join("reference-before-rejections"));
    let reference=mycity_storage::Store::open(&folder.join("reference-before-rejections"),&rules()).unwrap();
    let from=reference.engine().unwrap().world().account_ref(1).unwrap();let to=reference.engine().unwrap().world().account_ref(2).unwrap();drop(reference);
    let before = run.call(Command::Status); let described = run.call(Command::DescribeTaskExecution);
    let base = described["request"].clone(); let mut cases = Vec::new();
    for (version, quota, marker) in [(air_turns::PLAN_VERSION + 1, q(1,17,1), "task_exact_pages_plan"),
        (air_turns::PLAN_VERSION, q(0,17,1), "task_exact_pages_plan"),
        (air_turns::PLAN_VERSION, q(1,0,1), "task_exact_pages_plan"),
        (air_turns::PLAN_VERSION, q(1,17,0), "task_exact_pages_plan")] {
        let mut bad = base.clone(); exact_intent(&mut bad, version, quota); cases.push((bad, marker));
    }
    let mut legacy = base.clone(); legacy["capability_version"] = json!(1); exact_intent(&mut legacy, air_turns::PLAN_VERSION, q(1,17,1)); cases.push((legacy,"task_capability_version_unsupported"));
    let mut non_air = base.clone(); non_air["execution"] = json!("sim_timeline");
    exact_intent(&mut non_air, air_turns::PLAN_VERSION, q(1,17,1)); cases.push((non_air,"task_exact_pages_requires_air"));
    for (request, marker) in cases {
        run.begin(Command::RunBoundTask { binding: serde_json::from_value(request).unwrap() }); let e = run.next();
        assert_eq!(e.kind,"result"); assert!(e.payload["Err"].as_str().unwrap().contains(marker),"{e:?}");
    }
    let after_invalid=run.call(Command::Status);
    for key in ["sequence","state_sha256","business_sha256","background"] {assert_eq!(before[key],after_invalid[key]);}
    let mut stale = base.clone(); exact_intent(&mut stale, air_turns::PLAN_VERSION, q(1,17,1));
    // The original pending P4 obligation correctly blocks this business write.
    // Its recorded refusal advances Store sequence, and must stale the binding
    // without pretending the transfer occurred or loosening the original gate.
    let current=run.call(Command::Status);
    let refused=run.call(Command::Execute{envelope:Envelope{world:1,branch:1,
        operation_id:current["sequence"].as_u64().unwrap()+1,expected_revision:current["revision"].as_u64().unwrap(),
        actions:vec![Action::Transfer{from,to,amount:mycity_core::units::Money::new(1).unwrap()}]}});
    assert_eq!(refused["model"]["Err"]["timeline"],"busy");assert_eq!(refused["has_persistent_receipt"],true);
    let before_reject = run.call(Command::Status);
    run.begin(Command::RunBoundTask { binding: serde_json::from_value(stale).unwrap() });
    let rejected = run.next(); assert_eq!(rejected.kind,"result");
    assert!(rejected.payload["Err"].as_str().unwrap().contains("task_binding_stale_recover_original_result"),"{rejected:?}");
    let after_reject = run.call(Command::Status);
    for key in ["sequence", "state_sha256", "business_sha256", "background"] { assert_eq!(before_reject[key], after_reject[key]); }
    assert_eq!(run.events.iter().filter(|e|e["kind"]=="worker_started").count(),0);
    run.stop(); pool.finish().unwrap();
}
