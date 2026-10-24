//! Actual production sliced execution; fixed-source root runs retain all artifacts.
use mycity_core::{air::{Input,Source,State,STRATEGY},background::Command as B,exchange::Command as X,
    schedule::Phase,timeline::Command as T,transaction::{Action,Envelope},sha256};
use mycity_headless::{air_turns::{self,Quota,Boundary,Continue},compute_worker::{self as wire,Request as Work,Ticket,Stage},
    sim_client::Client,sim_host::{Command,PageControl}};
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},fs,io::{BufRead,BufReader,Write},process::{Command as Process,Stdio},time::Duration,
    sync::atomic::{AtomicU64,Ordering}};
static NEXT:AtomicU64=AtomicU64::new(0);
fn root(name:&str)->PathBuf {
    let base=PathBuf::from(std::env::var_os("MYCITY_AIR_TURN_TEST_ROOT").expect("dedicated external evidence root required"));
    assert!(base.canonicalize().unwrap().starts_with(Path::new("/Volumes/HP P900/dev/MyCity/work").canonicalize().unwrap()));
    let p=base.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();p
}
fn exe()->PathBuf {std::env::var_os("MYCITY_PROTOCOL_WORKER_EXE").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")))}
fn quota()->Quota {Quota{source_limit:1,cell_limit:17,pages:1}}
fn large()->Quota {Quota{source_limit:8192,cell_limit:1024,pages:16}}
struct Run {client:Client,id:u64,events:Vec<Value>,events_path:PathBuf}
impl Run {
    fn new(folder:PathBuf,create:bool)->Self {
        let mut client=Client::new();client.start(&exe(),&folder.join("world"),create).unwrap();
        let events_path=folder.join(format!("events-session-{}.json",NEXT.fetch_add(1,Ordering::Relaxed)));
        let mut r=Self{client,id:0,events:Vec::new(),events_path};assert_eq!(r.next().kind,"ready");r
    }
    fn pooled(folder:PathBuf,pool:&Path)->Self {
        let mut client=Client::new();client.start_in_pool(&exe(),&folder.join("world"),true,false,pool).unwrap();
        let events_path=folder.join(format!("events-session-{}.json",NEXT.fetch_add(1,Ordering::Relaxed)));
        let mut r=Self{client,id:0,events:Vec::new(),events_path};assert_eq!(r.next().kind,"ready");r
    }
    fn next(&mut self)->mycity_headless::sim_host::Event {
        let e=self.client.next(Duration::from_secs(10)).unwrap();
        self.events.push(serde_json::to_value(&e).unwrap());assert!(self.events.len()<=512);
        fs::write(&self.events_path,serde_json::to_vec_pretty(&self.events).unwrap()).unwrap();e
    }
    fn begin(&mut self,c:Command){self.id+=1;self.client.submit(self.id,c).unwrap();}
    fn call(&mut self,c:Command)->Value {self.begin(c);loop{let e=self.next();if e.kind=="result"{assert!(e.payload.get("Err").is_none(),"{e:?}");return e.payload["Ok"].clone();}}}
    fn action(&mut self,a:Action){let s=self.call(Command::Status);let v=self.call(Command::Execute{envelope:Envelope{
        world:1,branch:1,operation_id:s["sequence"].as_u64().unwrap()+1,expected_revision:s["revision"].as_u64().unwrap(),actions:vec![a]}});assert!(v["model"].get("Ok").is_some(),"{v}");}
    fn prepare(&mut self){
        self.action(Action::Exchange{command:X::Dispatch{quote:1}});
        self.action(Action::Timeline{command:T::Enable});self.action(Action::Background{command:B::Enable});
        for _ in 0..30 {self.action(Action::Timeline{command:T::Advance{target:30,limit:256}});
            if serde_json::from_value::<Option<Phase>>(self.call(Command::Status)["timeline"]["phase"].clone()).unwrap()==Some(Phase::P4){return;}}
        panic!("P4 not reached");
    }
    fn sliced(&mut self){self.begin(Command::RunRequiredSliced{job:1,plan:air_turns::PLAN_ID.into(),quota:quota()});}
    fn boundary(&mut self)->Boundary {loop {let e=self.next();assert_ne!(e.kind,"result","premature {e:?}");if e.kind=="page_boundary"{return serde_json::from_value(e.payload["boundary"].clone()).unwrap();}}}
    fn control(&mut self,c:PageControl){self.client.control_pages(c).unwrap();}
    fn control_result(&mut self)->Value {let e=self.next();assert_eq!(e.kind,"page_control_result");e.payload["Ok"].clone()}
    fn finish_sliced(&mut self,first:Boundary)->Value {
        let mut boundary=first;let mut n=0;
        loop {let q=if n==0 {Quota{source_limit:7,cell_limit:31,pages:1}} else {large()};
            self.control(PageControl::Continue{next:Continue{plan_version:boundary.plan_version,turn:boundary.turn,process_instance:boundary.process_instance.clone(),quota:q}});n+=1;
            loop {let e=self.next();if e.kind=="result"{assert!(e.payload.get("Err").is_none(),"{e:?}");assert!(n>=2);return e.payload["Ok"].clone();}
                if e.kind=="page_boundary"{boundary=serde_json::from_value(e.payload["boundary"].clone()).unwrap();break;}}
        }
    }
    fn stop(&mut self){self.call(Command::Stop);self.client.stop().unwrap();}
}
fn candidate(status:&Value)->Value {status["runtime"]["worker_task_observation"]["data"]["candidate"].clone()}
#[test]
fn real_sim_pauses_answers_queries_changes_quota_and_commits_exact_reference() {
    let mut reference=Run::new(root("reference"),true);reference.prepare();reference.call(Command::RunRequired{job:1});
    let expected=reference.call(Command::Status);reference.stop();
    let mut run=Run::new(root("sliced"),true);run.prepare();run.sliced();let first=run.boundary();assert_eq!(first.turn,1);
    run.control(PageControl::Observe);let paused=run.control_result();assert_eq!(paused["still_unresolved"],true);
    assert_eq!(paused["worker_owner"]["empty"],false);assert_eq!(paused["task_observation"]["turn_waiting"],true);
    assert_eq!(paused["progress"]["background"]["time"],0);assert_eq!(paused["progress"]["background"]["pending"]["id"],1);
    run.control(PageControl::ReceiptProof{operation:0xe000000000000001});assert_eq!(run.control_result()["present"],false);
    // No Continue means no next page; query retained owner again before authorizing it.
    run.control(PageControl::Observe);let again=run.control_result();
    assert_eq!(again["task_observation"]["work_counts"],paused["task_observation"]["work_counts"]);
    assert_eq!(again["task_observation"]["data"],paused["task_observation"]["data"]);
    let first_metrics=paused["task_observation"]["page_execution_metrics"].clone();
    assert!(!first_metrics.is_null(),"{paused}");
    std::thread::sleep(Duration::from_millis(25)); // intentional pause, not numerical work
    run.control(PageControl::Observe);let held=run.control_result();
    assert_eq!(held["task_observation"]["page_execution_metrics"],first_metrics);
    assert!(held["task_observation"]["host_current_pause_ns"].as_u64().unwrap() >= 20_000_000);
    run.finish_sliced(first);let actual=run.call(Command::Status);
    assert_eq!(actual["background"]["time"],30);assert_eq!(candidate(&actual),candidate(&expected));
    assert_eq!(actual["runtime"]["worker_task_observation"]["data"]["stage"],"complete");
    let counts=&actual["runtime"]["worker_task_observation"]["work_counts"];
    assert_eq!(counts["cells"],16384);assert_eq!(counts["integrated_cells"],16384);assert_eq!(counts["segments"],1);
    assert_eq!(run.events.iter().filter(|e|e["kind"]=="worker_started").count(),1);
    let observation=&actual["runtime"]["worker_task_observation"];
    assert!(observation["page_metrics_error"].is_null(),"{observation}");
    assert_eq!(observation["host_turn_response"]["kind"],"continue_send_to_boundary");
    assert!(observation["host_turn_response"]["ns"].as_u64().is_some());
    for e in run.events.iter().filter(|e|e["kind"]=="worker_progress"
        && (e["payload"]["stage"]=="sources" || e["payload"]["stage"]=="cells")) {
        assert!(e["payload"]["task_observation"]["page_execution_metrics"].is_null());
    }
    let terminal:Vec<_>=run.events.iter().filter(|e|e["kind"]=="worker_progress"
        && (e["payload"]["stage"]=="published" || e["payload"]["stage"]=="complete")).collect();
    assert_eq!(terminal.len(),2);assert_eq!(terminal[0]["payload"]["page_metrics"],terminal[1]["payload"]["page_metrics"]);
    assert!(!terminal[0]["payload"]["page_metrics"].is_null());
    run.stop();
}
#[test]
fn rejected_sim_controls_end_without_implicit_attempt_and_allow_explicit_retry() {
    for fault in ["turn","instance","duplicate"] {
        let mut run=Run::new(root(fault),true);run.prepare();run.sliced();let first=run.boundary();
        let mut next=Continue{plan_version:first.plan_version,turn:first.turn,process_instance:first.process_instance.clone(),quota:quota()};
        if fault=="turn" {next.turn+=1;} if fault=="instance" {next.process_instance="1".into();}
        run.control(PageControl::Continue{next:next.clone()});if fault=="duplicate" {run.control(PageControl::Continue{next});}
        loop {let e=run.next();if e.kind=="result"{assert!(e.payload.get("Err").is_some(),"{e:?}");break;}}
        assert_eq!(run.events.iter().filter(|e|e["kind"]=="worker_started").count(),1);
        let state=run.call(Command::Status);assert_eq!(state["background"]["time"],0);
        assert_eq!(state["background"]["pending"]["id"],1);assert_eq!(state["background"]["pending"]["attempt"],1);
        let pages=state["runtime"]["worker_task_observation"]["work_counts"]["pages"].as_u64().unwrap();
        assert_eq!(pages,if fault=="duplicate"{2}else{1});
        let old_pid=first.process_instance;run.sliced();let retry=run.boundary();assert_ne!(retry.process_instance,old_pid);
        run.finish_sliced(retry);assert_eq!(run.call(Command::Status)["background"]["time"],30);run.stop();
    }
}
#[test]
fn cancelled_pause_preserves_required_job_through_cold_restore() {
    let folder=root("cancel-restore");let mut run=Run::new(folder.clone(),true);run.prepare();run.sliced();run.boundary();
    run.control(PageControl::Cancel);loop {let e=run.next();if e.kind=="result"{assert!(e.payload.get("Err").is_some());break;}}
    let pending=run.call(Command::Status);assert_eq!(pending["background"]["time"],0);assert_eq!(pending["background"]["pending"]["failure"],"cancelled");
    run.call(Command::Checkpoint);run.stop();drop(run);
    let mut restored=Run::new(folder,false);let state=restored.call(Command::Status);
    assert_eq!(state["background"]["pending"],pending["background"]["pending"]);
    restored.sliced();let boundary=restored.boundary();restored.finish_sliced(boundary);
    restored.call(Command::Checkpoint);restored.stop();
}
fn raw_request(root:&Path)->Work {
    fs::create_dir(root.join("inputs")).unwrap();fs::create_dir(root.join("candidates")).unwrap();
    let state=State::new(Input{schema:STRATEGY.into(),side:32,target:60,initial_micro:2000000,boundary_micro:[Some(2000000);4],boundary_until:60,
        sources:(0..17).map(|id|Source{id:id+1,cell:id,start:0,end:60,rate_micro:10}).collect()}).unwrap();
    Work{protocol:wire::PROTOCOL.into(),strategy:STRATEGY.into(),controlled:Some(quota()),ticket:Ticket{
        world:1,branch:1,job:1,incarnation:1,generation:1,attempt:1,base_revision:0,field_version:0,start:0,end:60,
        rules_sha256:sha256(b"exact-pages"),source_sha256:env!("MYCITY_SOURCE_SHA256").into(),input_sha256:state.input().identity(),
        checkpoint_sha256:wire::publish(&root.join("inputs"),&state.checkpoint().unwrap()).unwrap()}}
}
#[test]
fn real_child_rejects_stale_duplicate_wrong_ticket_and_probe_controls_without_extra_page() {
    for fault in ["turn","instance","ticket","quota","probe","duplicate","eof"] {
        let root=root(fault);let req=raw_request(&root);let mut child=Process::new(exe()).args(["compute-worker"]).arg(&root)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::from(fs::File::create(root.join("raw-worker.stderr")).unwrap())).spawn().unwrap();
        let mut input=child.stdin.take().unwrap();let mut output=BufReader::new(child.stdout.take().unwrap());
        serde_json::to_writer(&mut input,&req).unwrap();input.write_all(b"\n").unwrap();input.flush().unwrap();
        let mut events=Vec::new();let first=loop{let mut line=String::new();assert!(output.read_line(&mut line).unwrap()>0);
            let event:wire::Event=serde_json::from_str(&line).unwrap();events.push(serde_json::to_value(&event).unwrap());
            if event.stage==Stage::SliceBoundary {break event.turn.unwrap();}};
        let mut command=json!({"protocol":wire::PROTOCOL,"ticket":req.ticket,"continue_turn":Continue{
            plan_version:first.plan_version,turn:first.turn,process_instance:first.process_instance.clone(),quota:quota()}});
        match fault {"turn"=>command["continue_turn"]["turn"]=json!(first.turn+1),"instance"=>command["continue_turn"]["process_instance"]=json!("1"),
            "ticket"=>command["ticket"]["job"]=json!(2),"quota"=>command["continue_turn"]["quota"]["pages"]=json!(0),
            "probe"=>command=json!({"protocol":wire::PROTOCOL,"ticket":req.ticket,"stage":"slice_boundary"}),_=>{}}
        for _ in 0..if fault=="duplicate"{2}else if fault=="eof"{0}else{1} {serde_json::to_writer(&mut input,&command).unwrap();input.write_all(b"\n").unwrap();input.flush().unwrap();}
        drop(input);loop{let mut line=String::new();if output.read_line(&mut line).unwrap()==0{break;}
            let event:wire::Event=serde_json::from_str(&line).unwrap();events.push(serde_json::to_value(&event).unwrap());}
        assert!(!child.wait().unwrap().success());
        let boundaries:Vec<_>=events.iter().filter(|e|e["stage"]=="slice_boundary").collect();assert_eq!(boundaries.len(),if fault=="duplicate"{2}else{1});
        assert!(events.iter().all(|e|e["stage"]!="published"&&e["stage"]!="complete"));assert_eq!(fs::read_dir(root.join("candidates")).unwrap().count(),0);
        fs::write(root.join("raw-control-events.json"),serde_json::to_vec_pretty(&events).unwrap()).unwrap();
    }
}

#[test]
fn real_broker_pause_keeps_original_grant_charged_until_actual_retirement() {
    use mycity_headless::normal_pool::{self,Process,BYTES};
    let folder=root("broker-pause");let mut pool=Process::spawn(&exe(),&folder.join("pool"),None).unwrap();
    let mut run=Run::pooled(folder.clone(),pool.root());run.prepare();run.sliced();let first=run.boundary();
    let before=normal_pool::status(pool.root()).unwrap();assert!(before.grants>before.returns);
    assert!(before.available_touched_bytes<BYTES);
    run.control(PageControl::Observe);let observation=run.control_result();
    assert_eq!(observation["worker_owner"]["empty"],false);
    assert!(observation["worker_owner"]["broker_lease"].as_u64().is_some());
    let paused=normal_pool::status(pool.root()).unwrap();
    assert_eq!(paused.grants,before.grants);assert_eq!(paused.returns,before.returns);
    assert_eq!(paused.available_touched_bytes,before.available_touched_bytes);
    fs::write(folder.join("paused-pool.json"),serde_json::to_vec_pretty(&paused).unwrap()).unwrap();
    run.finish_sliced(first);run.stop();
    let retired=normal_pool::status(pool.root()).unwrap();
    assert_eq!(retired.grants,retired.returns);assert_eq!(retired.available_touched_bytes,BYTES);
    fs::write(folder.join("retired-pool.json"),serde_json::to_vec_pretty(&retired).unwrap()).unwrap();pool.finish().unwrap();
}

fn drive_adaptive(run:&mut Run,retarget_to_force_transition:bool)->Value {
    use mycity_headless::{adaptive_client::{AdaptiveSession,Phase as AutoPhase,Purpose},air_page_policy::{Config,Reason}};
    run.id+=1;let log=run.events_path.with_extension("adaptive.json");let mut rows=Vec::new();
    let q=Quota{source_limit:1,cell_limit:256,pages:1};
    let mut session=AdaptiveSession::start(&mut run.client,run.id,1,q,Config{target_ns:1_000_000_000}).unwrap();
    let mut up=false;let mut down=false;let mut held=false;let mut retargeted=false;let mut queried=false;let mut terminal=false;
    let mut result=None;let mut expected_pages=1u16;let mut last_sent_turn=0;
    for _ in 0..1024 {
        let step=session.step(Duration::from_secs(10)).unwrap();
        if let Some(sent)=session.last_attempted_decision() {
            if sent.boundary.turn!=last_sent_turn {
                assert_eq!(sent.decision.current_quota.source_limit,q.source_limit);
                assert_eq!(sent.decision.next_quota.cell_limit,q.cell_limit);
                assert_eq!(sent.decision.next_quota.source_limit,q.source_limit);
                assert!(matches!(sent.decision.next_quota.pages,1|2|4|8|16));
                up|=sent.decision.reason==Reason::StepUp;down|=sent.decision.reason==Reason::StepDown;
                expected_pages=sent.decision.next_quota.pages;last_sent_turn=sent.boundary.turn;
                rows.push(json!({"actually_attempted_continue":sent}));
            }
        }
        if let Some(report)=step.decision.as_ref() {
            if report.purpose==Purpose::PendingNextTurn {
                assert_eq!(report.decision.current_quota.pages,expected_pages);
                assert_eq!(report.boundary.work.pages,u64::from(expected_pages));
                held|=report.decision.current_quota==report.decision.next_quota;
            } else {
                terminal=true;assert_eq!(report.decision.current_quota,report.decision.next_quota);
                if retargeted {assert!(report.decision.integration_overrun);assert!(report.decision.irreducible_overrun);}
            }
        }
        rows.push(json!({"event":step.event,"decision":step.decision}));
        fs::write(&log,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        if session.phase()==AutoPhase::BoundaryReady {
            if !queried {
                let pending=session.pending_decision().unwrap().clone();
                session.observe().unwrap();assert_eq!(session.phase(),AutoPhase::QueryPending);
                assert!(session.cancel().is_err());assert!(session.retarget(Config{target_ns:1}).is_err());
                let response=session.step(Duration::from_secs(10)).unwrap();assert_eq!(response.event.kind,"page_control_result");
                assert_eq!(session.phase(),AutoPhase::BoundaryReady);assert!(session.last_attempted_decision().is_none());
                assert_eq!(session.pending_decision().unwrap().boundary,pending.boundary);
                rows.push(json!({"query_observe":response.event}));
                session.receipt_proof(0xe000000000000001).unwrap();let proof=session.step(Duration::from_secs(10)).unwrap();
                assert_eq!(proof.event.payload["Ok"]["present"],false);assert_eq!(session.phase(),AutoPhase::BoundaryReady);
                rows.push(json!({"query_proof":proof.event}));queried=true;
            }
            if retarget_to_force_transition && up && !retargeted
                && session.pending_decision().unwrap().decision.known_sample.is_some_and(|s|s.wall_ns>1) {
                let changed=session.retarget(Config{target_ns:1}).unwrap();
                rows.push(json!({"explicit_test_target_change_after_actual_positive_sample":changed}));
                retargeted=true;
            }
        }
        if session.phase()==AutoPhase::Finished {
            assert_eq!(rows.last().unwrap().get("event").map(|e|e["kind"].clone()),Some(json!("result")));
            assert!(step.event.payload.get("Err").is_none(),"{:?}",step.event);
            result=Some(step.event.payload["Ok"].clone());break;
        }
    }
    assert!(terminal&&queried&&held);if retarget_to_force_transition {assert!(up&&down&&retargeted,"up={up} down={down} retargeted={retargeted}");}
    fs::write(&log,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();result.expect("adaptive command completed")
}

#[test]
fn production_adaptive_session_uses_real_samples_for_up_down_hold_and_exact_checkpoint() {
    let mut reference=Run::new(root("adaptive-reference"),true);reference.prepare();reference.call(Command::RunRequired{job:1});
    let expected=reference.call(Command::Status);reference.stop();
    let mut run=Run::new(root("adaptive-transitions"),true);run.prepare();drive_adaptive(&mut run,true);
    let actual=run.call(Command::Status);assert_eq!(actual["background"]["time"],30);assert_eq!(candidate(&actual),candidate(&expected));run.stop();
}
#[test]
fn production_adaptive_cancel_preserves_required_job_through_cold_restore() {
    use mycity_headless::{adaptive_client::{AdaptiveSession,Phase as AutoPhase},air_page_policy::Config};
    let folder=root("adaptive-cancel");let mut run=Run::new(folder.clone(),true);run.prepare();run.id+=1;
    {
        let mut session=AdaptiveSession::start(&mut run.client,run.id,1,quota(),Config{target_ns:1_000_000}).unwrap();
        loop {let s=session.step(Duration::from_secs(10)).unwrap();if s.event.kind=="page_boundary"{break;}}
        session.cancel().unwrap();assert_eq!(session.phase(),AutoPhase::CancelPending);
        assert!(session.pending_decision().is_none());assert!(session.observe().is_err());
        loop {let s=session.step(Duration::from_secs(10)).unwrap();if s.event.kind=="result"{assert!(s.event.payload.get("Err").is_some());break;}}
        assert!(session.last_attempted_decision().is_none());
    }
    let pending=run.call(Command::Status);assert_eq!(pending["background"]["time"],0);assert_eq!(pending["background"]["pending"]["id"],1);
    run.call(Command::Checkpoint);run.stop();drop(run);
    let mut restored=Run::new(folder,false);assert_eq!(restored.call(Command::Status)["background"]["pending"],pending["background"]["pending"]);
    drive_adaptive(&mut restored,false);assert_eq!(restored.call(Command::Status)["background"]["time"],30);restored.stop();
}
#[test]
fn dropping_adaptive_borrow_keeps_original_client_and_job_owned_until_explicit_cancel() {
    use mycity_headless::{adaptive_client::AdaptiveSession,air_page_policy::Config};
    let mut run=Run::new(root("adaptive-drop"),true);run.prepare();run.id+=1;
    {
        let mut session=AdaptiveSession::start(&mut run.client,run.id,1,quota(),Config{target_ns:1_000_000}).unwrap();
        loop {if session.step(Duration::from_secs(10)).unwrap().event.kind=="page_boundary"{break;}}
    }
    assert!(!run.client.is_empty());assert!(run.client.submit(run.id+1,Command::Status).is_err());
    run.client.control_pages(PageControl::Observe).unwrap();let observed=run.next();assert_eq!(observed.kind,"page_control_result");
    assert_eq!(observed.payload["Ok"]["worker_owner"]["empty"],false);assert_eq!(observed.payload["Ok"]["still_unresolved"],true);
    run.client.control_pages(PageControl::Cancel).unwrap();loop {if run.next().kind=="result"{break;}}
    assert_eq!(run.call(Command::Status)["background"]["time"],0);run.stop();
}
fn kill_only_known_test_sim_and_confirm_terminal(pid:u32) {
    // pid is exclusively the Client child created by this test, never another project.
    assert!(Process::new("kill").args(["-KILL",&pid.to_string()]).status().unwrap().success());
    let until=std::time::Instant::now()+Duration::from_secs(5);
    loop {
        let output=Process::new("ps").args(["-p",&pid.to_string(),"-o","stat="]).output().unwrap();
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim().starts_with('Z') {return;}
        assert!(std::time::Instant::now()<until);std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn actual_continue_send_failure_is_consumed_once_and_original_owner_retires() {
    use mycity_headless::{adaptive_client::{AdaptiveSession,Phase as AutoPhase},air_page_policy::Config};
    let mut run=Run::new(root("adaptive-send-failure"),true);run.prepare();run.id+=1;let sim=run.client.pid();
    {
        let mut session=AdaptiveSession::start(&mut run.client,run.id,1,quota(),Config{target_ns:1_000_000}).unwrap();
        loop {if session.step(Duration::from_secs(10)).unwrap().event.kind=="page_boundary"{break;}}
        let boundary=session.pending_decision().unwrap().boundary.clone();kill_only_known_test_sim_and_confirm_terminal(sim);
        assert!(session.step(Duration::from_secs(10)).is_err());assert_eq!(session.phase(),AutoPhase::ControlUncertain);
        assert_eq!(session.last_attempted_decision().unwrap().boundary,boundary);assert!(session.pending_decision().is_none());
        assert!(session.step(Duration::from_secs(10)).is_err());assert_eq!(session.last_attempted_decision().unwrap().boundary,boundary);
        session.retire().unwrap();assert_eq!(session.phase(),AutoPhase::Finished);
    }
    assert!(run.client.is_empty());
    // Deliberately no fake result/refund or implicit new attempt after Sim death.
}
#[test]
fn actual_failed_initial_adaptive_submit_keeps_client_busy_until_original_retirement() {
    use mycity_headless::{adaptive_client::AdaptiveSession,air_page_policy::Config};
    let mut run=Run::new(root("adaptive-start-failure"),true);run.prepare();let sim=run.client.pid();
    kill_only_known_test_sim_and_confirm_terminal(sim);run.id+=1;
    assert!(AdaptiveSession::start(&mut run.client,run.id,1,quota(),Config{target_ns:1_000_000}).is_err());
    assert!(run.client.submit(run.id+1,Command::Status).is_err());assert!(!run.client.is_empty());
    run.client.stop().unwrap();assert!(run.client.is_empty());
}
