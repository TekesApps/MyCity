//! Actual local-process entry gates, with complete retained worlds and child logs.
use mycity_core::{transaction::{Action, Caller, Envelope}, units::Money};
use mycity_storage::{Query, Store};
use serde_json::{json, Value};
use std::{fs, io::Write, path::{Path, PathBuf}, process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};

const RULES: &str = env!("MYCITY_SOURCE_SHA256");
static NEXT: AtomicU64 = AtomicU64::new(0);
fn directory(name: &str) -> PathBuf {
    let root = PathBuf::from(std::env::var_os("MYCITY_OBSERVATION_TEST_WORK").unwrap());
    assert!(root.is_dir());
    let path = root.join(format!("entry-{name}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&path).unwrap();
    println!("OBSERVATION_ENTRY_ARTIFACT={}", path.display());
    path
}
fn create(path: &Path) -> Store {
    Store::create(path, include_bytes!("../../../fixtures/m0-region-opening.json"), vec![], RULES).unwrap()
}
fn request(id: u64, command: Value) -> Value { json!({"id":id,"request":command}) }
fn child(root: &Path, args: &[&str], requests: &[Value]) -> (bool, String, String) {
    let path = root.join(format!("child-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&path).unwrap();
    let mut input = fs::File::create(path.join("stdin.jsonl")).unwrap();
    for r in requests { writeln!(input,"{r}").unwrap(); }
    drop(input);
    fs::write(path.join("launch.json"), serde_json::to_vec(&json!({
        "args":args,"timeout_seconds":60,"exe":env!("CARGO_BIN_EXE_mycity-headless")
    })).unwrap()).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_mycity-headless"))
        .args(args).stdin(Stdio::from(fs::File::open(path.join("stdin.jsonl")).unwrap()))
        .stdout(Stdio::from(fs::File::create(path.join("stdout.jsonl")).unwrap()))
        .stderr(Stdio::from(fs::File::create(path.join("stderr.txt")).unwrap()))
        .spawn().unwrap();
    fs::write(path.join("pid.txt"), c.id().to_string()).unwrap();
    let start=Instant::now();
    let (status, timeout) = loop {
        if let Some(s)=c.try_wait().unwrap() {break (s,false);}
        if start.elapsed()>Duration::from_secs(60) {
            c.kill().unwrap(); break (c.wait().unwrap(),true);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    fs::write(path.join("result.json"), serde_json::to_vec(&json!({
        "pid":c.id(),"exit":status.code(),"success":status.success(),"timeout":timeout
    })).unwrap()).unwrap();
    assert!(!timeout,"owned child timeout; retained {}",path.display());
    let stdout=fs::read_to_string(path.join("stdout.jsonl")).unwrap();
    let stderr=fs::read_to_string(path.join("stderr.txt")).unwrap();
    assert!(stdout.len()<=256*1024 && stderr.len()<=256*1024);
    (status.success(), stdout, stderr)
}
fn session(root: &Path, world: &Path, actor: [&str;2], requests: &[Value]) -> Vec<Value> {
    let (ok,out,err)=child(root,&["session",world.to_str().unwrap(),RULES,actor[0],actor[1]],requests);
    assert!(ok,"{err}");
    let rows:Vec<Value>=out.lines().map(|l|serde_json::from_str(l).unwrap()).collect();
    assert_eq!(rows.len(),requests.len()); rows
}
#[test]
fn account_cannot_bypass_operator_query_gate_or_self_elevate() {
    let root=directory("query-gates"); let world=root.join("world");
    let mut store=create(&world);let before=store.state_hash().unwrap();drop(store);
    let mut requests=vec![];
    for command in ["query","observe_query"] {
        for section in ["contracts","fulfilment","arrears","capacity","blocked","recovery","receipts","timeline"] {
            requests.push(request(requests.len() as u64,json!({"command":command,"section":section,"limit":1,"cursor":null})));
        }
    }
    requests.push(json!({"id":90,"caller":"system","request":{"command":"observe_query","section":"receipts","limit":1}}));
    requests.push(request(91,json!({"command":"query","section":"receipts","limit":1,"caller":"system"})));
    let rows=session(&root,&world,["account","2"],&requests);
    for row in &rows[..16] {
        assert_eq!(row["error"],json!({"code":"query_operator_required"}));
        assert!(row.get("result").is_none());
    }
    for row in &rows[16..] { assert_eq!(row["error"]["code"],"invalid_request"); }
    let mut store=Store::open(&world,RULES).unwrap();
    assert_eq!(store.committed_sequence(),0); assert_eq!(store.state_hash().unwrap(),before);
}
#[test]
fn receipt_visibility_preserves_own_success_failure_and_retry_hot_and_archived() {
    let root=directory("receipt-visibility");let world=root.join("world");let mut store=create(&world);
    let transfer=|op,rev,from,to|Envelope{world:1,branch:1,operation_id:op,expected_revision:rev,
        actions:vec![Action::Transfer{from:store.engine().unwrap().world().account_ref(from).unwrap(),
            to:store.engine().unwrap().world().account_ref(to).unwrap(),amount:Money::new(100).unwrap()}]};
    let own=transfer(1,0,2,3);let failed=transfer(2,1,3,2);
    assert!(store.execute(Caller::Account(2),own.clone()).unwrap().model.is_ok());
    assert!(store.execute(Caller::Account(2),failed.clone()).unwrap().model.is_err());
    let sys=Envelope{world:1,branch:1,operation_id:3,expected_revision:1,
        actions:vec![Action::RegionAdvance{seconds:1}]};
    assert!(store.execute(Caller::System,sys).unwrap().model.is_ok());
    let success=serde_json::to_value(store.receipt(1).unwrap().unwrap()).unwrap();
    let failure=serde_json::to_value(store.receipt(2).unwrap().unwrap()).unwrap();
    assert_eq!(success["actor"],2); assert_eq!(failure["actor"],2);
    let hash=store.state_hash().unwrap();let sequence=store.committed_sequence();drop(store);
    let reads:Vec<_>=[1,2,3,999].into_iter().enumerate().map(|(i,op)|request(i as u64,json!({"command":"receipt","operation_id":op}))).collect();
    for archive in [false,true] {
        if archive {
            let mut store=Store::open(&world,RULES).unwrap();store.checkpoint().unwrap();
            assert!(store.engine().unwrap().journal().is_empty());drop(store);
        }
        let own_rows=session(&root,&world,["account","2"],&reads);
        assert_eq!(own_rows[0]["result"]["record"],success);
        assert_eq!(own_rows[1]["result"]["record"],failure);
        assert_eq!(own_rows[2]["result"],own_rows[3]["result"]);
        assert_eq!(own_rows[2]["result"]["state"],"unknown");
        assert!(own_rows[2]["result"]["record"].is_null());
        let other=session(&root,&world,["account","3"],&reads);
        assert!(other.iter().all(|r|r["result"]==other[3]["result"]));
        let operator=session(&root,&world,["operator","system"],&reads);
        assert_eq!(operator[0]["result"]["record"],success);
        assert_eq!(operator[1]["result"]["record"],failure);
        assert_eq!(operator[2]["result"]["record"]["actor"],0);
        let retry=session(&root,&world,["account","2"],&[
            request(50,json!({"command":"submit","envelope":own})),
            request(51,json!({"command":"submit","envelope":failed}))]);
        assert!(retry.iter().all(|r|r["result"]["already_recorded"]==true));
        let mut store=Store::open(&world,RULES).unwrap();
        assert_eq!(store.committed_sequence(),sequence);assert_eq!(store.state_hash().unwrap(),hash);
    }
}
#[test]
fn raw_cli_and_operator_session_remain_exact_while_observations_add_unknowns() {
    let root=directory("operator-compatibility");let world=root.join("world");let mut store=create(&world);
    let expected=serde_json::to_value(store.query(Query::Timeline,None,1).unwrap()).unwrap();
    let hash=store.state_hash().unwrap();drop(store);
    let (ok,raw,err)=child(&root,&["query",world.to_str().unwrap(),RULES,"timeline","1"],&[]);
    assert!(ok,"{err}");assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(),expected);
    let (ok,observed,err)=child(&root,&["observe-query",world.to_str().unwrap(),RULES,"timeline","1"],&[]);
    assert!(ok,"{err}");let observed:Value=serde_json::from_str(&observed).unwrap();
    assert_eq!(observed["schema"],"mycity-operator-observation-v1");assert_eq!(observed["page"],expected);
    for row in observed["row_observations"].as_array().unwrap() {
        assert_eq!(row["conditional_eta"]["status"],"unknown");assert_eq!(row["age_seconds"]["status"],"unknown");
    }
    let rows=session(&root,&world,["operator","system"],&[
        request(1,json!({"command":"query","section":"timeline","limit":1})),
        request(2,json!({"command":"observe_query","section":"timeline","limit":1})),
        request(3,json!({"command":"observe_query","section":"timeline","limit":0})),
        request(4,json!({"command":"query","section":"timeline","limit":0}))]);
    assert_eq!(rows[0]["result"],expected);assert_eq!(rows[1]["result"],observed);
    assert_eq!(rows[2]["error"],rows[3]["error"]);assert_eq!(rows[2]["error"]["reason"],"query_page_limit");
    let mut store=Store::open(&world,RULES).unwrap();assert_eq!(store.state_hash().unwrap(),hash);
}
