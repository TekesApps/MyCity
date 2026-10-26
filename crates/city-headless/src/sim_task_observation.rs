//! Latest-only execution evidence. No queue, grant, durable cursor or refund.
use crate::{compute_worker::{self, Worker}, task_registry_directory::{Entry, SourceBinding}};
use mycity_core::{task_registry::{ClockKind,TaskKey}, timeline::Progress, transaction::{Caller, Envelope}};
use mycity_storage::{Outcome, Store};
use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use std::time::Instant;

pub(super) const REPORT_CAP_BYTES: usize = 12 * 1024;
/// Planning charge, not an allocator grant or proof of native/reporting peak.
pub(super) const METADATA_PLAN_BYTES: usize = 64 * 1024;

#[derive(Clone, Serialize)]
#[serde(tag = "provider", content = "binding", rename_all = "snake_case")]
enum Source {
    Service(mycity_core::service_needs_authority::DueView),
    Supply(mycity_core::continuous_supply::DueView),
    Timeline(Progress),
}
impl Source {
    fn from_entry(entry: &Entry) -> Option<Self> {
        match entry.binding.as_ref()? {
            SourceBinding::Service(value) => Some(Self::Service(*value)),
            SourceBinding::Supply(value) => Some(Self::Supply(*value)),
            SourceBinding::Timeline(value) => Some(Self::Timeline(*value)),
            SourceBinding::Air(_) => None,
        }
    }
}

fn clock_value(clock:ClockKind)->Value {
    match clock {ClockKind::World=>json!({"kind":"world"}),ClockKind::Development{city_id}=>json!({"kind":"development","city_id":city_id}),ClockKind::Interaction=>json!({"kind":"interaction"})}
}
pub(super) fn serialize_clock<S:Serializer>(clock:&Option<ClockKind>,serializer:S)->Result<S::Ok,S::Error> {
    clock.map(clock_value).serialize(serializer)
}
pub(super) fn serialize_key<S: Serializer>(key: &Option<TaskKey>, serializer: S) -> Result<S::Ok, S::Error> {
    key.map(|key| json!({"scope":{"world_id":key.scope.world_id,"branch_id":key.scope.branch_id,"city_id":key.scope.city_id},
        "clock":clock_value(key.clock),"source_id":key.source_id,"source_version":key.source_version,
        "window":{"start":key.window.start,"end":key.window.end},"cursor":key.cursor})).serialize(serializer)
}

#[derive(Serialize)]
struct Context {
    world: u64,
    branch: u64,
    source: Source,
    #[serde(serialize_with = "serialize_clock")]
    clock: Option<ClockKind>,
    #[serde(serialize_with = "serialize_key")]
    task: Option<TaskKey>,
    before_sequence: u64,
    expected_revision: u64,
    operation: u64,
    advance_target: u64,
    advance_limit: u16,
    before: Progress,
}
impl Context {
    fn capture(store: &Store, entry: &Entry, envelope: &Envelope) -> Option<Self> {
        let engine = store.engine().ok()?;
        let scope = engine.world().scope_revision().0;
        let [mycity_core::transaction::Action::Timeline { command:mycity_core::timeline::Command::Advance { target,limit } }]=envelope.actions.as_slice() else {return None;};
        Some(Self { world:scope.world(), branch:scope.branch(), source:Source::from_entry(entry)?,clock:entry.clock,
            task:entry.key, before_sequence:store.committed_sequence(), expected_revision:envelope.expected_revision,
            operation:envelope.operation_id, advance_target:*target, advance_limit:*limit, before:engine.timeline()?.progress })
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Nanoseconds {
    Known { ns: u64 },
    Unknown { reason: &'static str },
}
fn cpu_delta(start: Result<u64, compute_worker::cpu_metrics::UnknownReason>, end: Result<u64, compute_worker::cpu_metrics::UnknownReason>) -> Nanoseconds {
    match (start, end) {
        (Ok(start), Ok(end)) => end.checked_sub(start).map_or(Nanoseconds::Unknown { reason:"clock_went_backwards" }, |ns|Nanoseconds::Known { ns }),
        _ => Nanoseconds::Unknown { reason:"process_clock_unavailable" },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CommitState { Committed, Replayed, ModelRejectedRecorded, RejectedWithoutReceipt, Unconfirmed }
fn classify(result: &Result<Outcome, String>, before_sequence:u64, after_sequence:u64) -> CommitState {
    match result {
        Ok(out) if out.has_persistent_receipt && after_sequence == before_sequence => CommitState::Replayed,
        Ok(out) if out.has_persistent_receipt && out.model.is_ok() && after_sequence > before_sequence => CommitState::Committed,
        Ok(out) if out.has_persistent_receipt && out.model.is_err() => CommitState::ModelRejectedRecorded,
        Ok(out) if !out.has_persistent_receipt && out.model.is_err() => CommitState::RejectedWithoutReceipt,
        _ => CommitState::Unconfirmed,
    }
}

#[derive(Serialize)]
pub(super) struct SimReport {
    context: Context,
    source_sha256: &'static str,
    pid: u32,
    process_instance: String,
    allocation: &'static str,
    commit: CommitState,
    original_receipt_confirmed: bool,
    receipt_sequence: Option<u64>,
    model_outcome: &'static str,
    after_sequence: u64,
    after: Option<Progress>,
    /// A committed Advance operation is not a domain work unit or all-target completion.
    committed_advance_operations: Option<u8>,
    elapsed_ns: Nanoseconds,
    process_cpu_ns: Nanoseconds,
    measurement_scope: &'static str,
    scratch_peak: &'static str,
}

#[derive(Default)]
pub(super) struct Recorder {
    latest: Option<Box<SimReport>>,
    unavailable: Option<&'static str>,
}
impl Recorder {
    /// The bracket is exactly the original execute call, including diagnostic IO.
    /// A failed observation never changes the returned original business result.
    pub(super) fn execute(&mut self, store:&mut Store, caller:Caller, envelope:Envelope,
        diag:&mut Option<crate::cpu_log::CpuLog>, entry:&Entry) -> Result<Outcome,String> {
        self.execute_using(store,caller,envelope,diag,entry,
            compute_worker::cpu_metrics::read_process_cpu_ns,crate::cpu_log::execute)
    }
    // Private dependency seam: production always uses the original clock and
    // execute function. There is no request/env switch for injecting a fault.
    fn execute_using<C,F>(&mut self,store:&mut Store,caller:Caller,envelope:Envelope,
        diag:&mut Option<crate::cpu_log::CpuLog>,entry:&Entry,mut clock:C,execute:F)->Result<Outcome,String>
    where C:FnMut()->Result<u64,compute_worker::cpu_metrics::UnknownReason>,
        F:FnOnce(&mut Store,Caller,Envelope,&mut Option<crate::cpu_log::CpuLog>)->Result<Outcome,String> {
        let context=Context::capture(store,entry,&envelope);
        let instance=crate::heap_meter::clock_reading().instance_unix_ns.to_string();
        let cpu_start=clock();
        let started=Instant::now();
        let result=execute(store,caller,envelope,diag);
        let elapsed=started.elapsed();
        let cpu_end=clock();
        self.latest=None;
        self.unavailable=Some("original_context_unavailable");
        if let Some(context)=context {
            let after_sequence=store.committed_sequence();
            let commit=classify(&result,context.before_sequence,after_sequence);
            let operations=match commit { CommitState::Committed=>Some(1),CommitState::Unconfirmed=>None,_=>Some(0) };
            self.latest=Some(Box::new(SimReport { context, source_sha256:env!("MYCITY_SOURCE_SHA256"),
                pid:std::process::id(),process_instance:instance,allocation:"inherited_sim_role_no_task_grant_or_refund",
                commit,original_receipt_confirmed:result.as_ref().is_ok_and(|out|out.has_persistent_receipt),
                receipt_sequence:result.as_ref().ok().filter(|out|out.has_persistent_receipt).map(|out|out.committed_sequence),
                model_outcome:match &result {Ok(out) if out.model.is_ok()=>"accepted",Ok(_)=>"rejected",Err(_)=>"unconfirmed"},
                after_sequence,after:store.engine().ok().and_then(|e|e.timeline().map(|t|t.progress)),
                committed_advance_operations:operations,
                elapsed_ns:u64::try_from(elapsed.as_nanos()).map_or(Nanoseconds::Unknown { reason:"nanosecond_overflow" },|ns|Nanoseconds::Known { ns }),
                process_cpu_ns:cpu_delta(cpu_start,cpu_end),
                measurement_scope:"same_sim_process_all_threads; execute_including_persistence_and_diagnostics; not_domain_exclusive_cpu_or_queue_age",
                scratch_peak:"unknown_not_instrumented" }));
            self.unavailable=None;
        }
        result
    }
    pub(super) fn report(&self, worker:&Worker) -> Value {
        #[derive(Serialize)]
        #[serde(tag="execution",content="observation",rename_all="snake_case")]
        enum Execution<'a> { AirWorker(compute_worker::AirTaskObservation<'a>), SimTimelinePage(&'a SimReport) }
        // Two bounded latest owners, not a chronological history or two allocations.
        let air=Execution::AirWorker(worker.common_task_observation());
        let sim=self.latest.as_deref().map(Execution::SimTimelinePage);
        let value=json!({"schema":"mycity-task-execution-observation-v1","air":air,"sim":sim,
            "sim_unavailable":self.unavailable,"retained_latest_only":true,"authority":"observation_only_no_store_completion_grant_or_refund",
            "units":"air_sources_cells_integration_segments_pages_separate; sim_committed_advance_operations_not_domain_work",
            "metadata_plan_bytes":METADATA_PLAN_BYTES,"metadata_plan_is_peak_proof":false});
        if serde_json::to_vec(&value).is_ok_and(|v|v.len()<=REPORT_CAP_BYTES) { value }
        else { json!({"schema":"mycity-task-execution-observation-v1","state":"unavailable","reason":"report_bound","authority":"none"}) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycity_core::{timeline::Command as T,transaction::Action};
    use std::{fs,path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering},process::{Command,Stdio},time::Duration};
    static NEXT:AtomicU64=AtomicU64::new(0);
    const RULES:&str="task-report-real-observer-v1";
    fn retained(label:&str)->PathBuf {
        let root=PathBuf::from(std::env::var_os("MYCITY_TASK_REPORT_FAULT_ROOT").expect("explicit retained root")).canonicalize().unwrap();
        assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
        let p=root.join(format!("{label}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();p
    }
    fn copy(source:&Path,target:&Path) {
        fs::create_dir(target).unwrap();for entry in fs::read_dir(source).unwrap(){let entry=entry.unwrap();let dest=target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir(){copy(&entry.path(),&dest)}else{fs::copy(entry.path(),&dest).unwrap();assert_eq!(fs::read(entry.path()).unwrap(),fs::read(dest).unwrap());}}
    }
    fn envelope(s:&Store,operation:u64,command:T)->Envelope {
        let(scope,revision)=s.engine().unwrap().world().scope_revision();Envelope{world:scope.world(),branch:scope.branch(),operation_id:operation,expected_revision:revision,actions:vec![Action::Timeline{command}]}
    }
    fn prepare(p:&Path)->(Store,Store,Entry,Envelope) {
        let input=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../fixtures/m0-economy-opening.json"));fs::write(p.join("opening.json"),input).unwrap();
        let mut s=Store::create_spatial(&p.join("world"),input,vec![],RULES,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../fixtures/m0-exchange-spatial.json")),include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../fixtures/terrain/m0-flat16.png"))).unwrap();
        for command in [T::Enable,T::Advance{target:3600,limit:1}] {let e=envelope(&s,s.committed_sequence()+1,command);assert!(s.execute(Caller::System,e).unwrap().model.is_ok());}
        let directory=crate::task_registry_directory::collect(&s,false).unwrap();assert!(matches!(directory.route(),crate::task_registry_directory::Route::Selected(crate::task_registry_directory::Action::Timeline(_))));
        let entry=directory.entries()[3].clone();let e=envelope(&s,0xc000000000000000|(s.committed_sequence()+1),T::Advance{target:3600,limit:1});
        s.checkpoint().unwrap();drop(s);copy(&p.join("world"),&p.join("reference"));
        (Store::open(&p.join("world"),RULES).unwrap(),Store::open(&p.join("reference"),RULES).unwrap(),entry,e)
    }
    fn cold(p:&Path,s:&Store,e:&Envelope) {
        let input=p.join("cold-input.json");fs::write(&input,serde_json::to_vec_pretty(&json!({"world":p.join("world"),"hash":s.state_hash().unwrap(),"sequence":s.committed_sequence(),"replay":e})).unwrap()).unwrap();
    }
    fn probe(p:&Path) {
        let mut child=Command::new(std::env::current_exe().unwrap()).args(["--ignored","--exact","sim_host::task_observation::tests::cold_probe","--test-threads=1","--nocapture"])
            .env("MYCITY_TASK_REPORT_COLD",p.join("cold-input.json"))
            .stdout(Stdio::from(fs::File::create(p.join("cold.stdout")).unwrap())).stderr(Stdio::from(fs::File::create(p.join("cold.stderr")).unwrap())).spawn().unwrap();
        let started=Instant::now();loop {if let Some(status)=child.try_wait().unwrap(){fs::write(p.join("cold-result.json"),json!({"pid":child.id(),"code":status.code(),"seconds":started.elapsed().as_secs_f64()}).to_string()).unwrap();assert!(status.success());break;}
            if started.elapsed()>Duration::from_secs(60){child.kill().unwrap();child.wait().unwrap();panic!("own bounded cold probe timeout");}std::thread::sleep(Duration::from_millis(20));}
    }
    #[test] #[ignore]
    fn cold_probe() {
        let config:Value=serde_json::from_slice(&fs::read(std::env::var_os("MYCITY_TASK_REPORT_COLD").unwrap()).unwrap()).unwrap();let mut s=Store::open(Path::new(config["world"].as_str().unwrap()),RULES).unwrap();
        assert_eq!(serde_json::to_value(s.state_hash().unwrap()).unwrap(),config["hash"]);assert_eq!(s.committed_sequence(),config["sequence"].as_u64().unwrap());
        let e:Envelope=serde_json::from_value(config["replay"].clone()).unwrap();assert!(s.execute(Caller::System,e).unwrap().model.is_ok());assert_eq!(s.committed_sequence(),config["sequence"].as_u64().unwrap());
        assert_eq!(serde_json::to_value(s.state_hash().unwrap()).unwrap(),config["hash"]);
        assert!(Recorder::default().report(&Worker::new())["sim"].is_null());
    }
    #[test]
    fn unavailable_clock_still_commits_the_real_original_transaction_and_cold_replays_once() {
        let p=retained("clock-unavailable");let(mut s,mut reference,entry,e)=prepare(&p);let expected=reference.execute(Caller::System,e.clone()).unwrap();let mut recorder=Recorder::default();let mut reads=0;
        let out=recorder.execute_using(&mut s,Caller::System,e.clone(),&mut None,&entry,||{reads+=1;Err(compute_worker::cpu_metrics::UnknownReason::ClockReadFailed)},crate::cpu_log::execute).unwrap();
        assert_eq!(reads,2);assert_eq!(serde_json::to_value(out).unwrap(),serde_json::to_value(expected).unwrap());assert_eq!(s.state_hash().unwrap(),reference.state_hash().unwrap());
        let report=recorder.latest.as_ref().unwrap();assert_eq!(report.commit,CommitState::Committed);assert_eq!(report.committed_advance_operations,Some(1));assert!(matches!(report.process_cpu_ns,Nanoseconds::Unknown{..}));
        fs::write(p.join("observation.json"),serde_json::to_vec_pretty(&recorder.report(&Worker::new())).unwrap()).unwrap();s.checkpoint().unwrap();cold(&p,&s,&e);drop(s);probe(&p);
    }
    #[test]
    fn actual_partial_wal_and_lost_commit_reply_remain_unconfirmed_until_original_recovery() {
        use mycity_storage::WritePoint;
        for(point,label,committed)in[(WritePoint::WalPayloadHalf,"partial-wal",false),(WritePoint::CurrentRenamed,"lost-reply",true)] {
            let p=retained(label);let(mut s,mut reference,entry,e)=prepare(&p);let before=s.state_hash().unwrap();let seq=s.committed_sequence();assert!(reference.execute(Caller::System,e.clone()).unwrap().model.is_ok());
            let mut recorder=Recorder::default();let mut hit=false;
            let out=recorder.execute_using(&mut s,Caller::System,e.clone(),&mut None,&entry,compute_worker::cpu_metrics::read_process_cpu_ns,
                |store,caller,envelope,_|store.execute_with_observer(caller,envelope,|at|if at==point{hit=true;Err(std::io::Error::other("retained original write interruption"))}else{Ok(())}).map_err(|error|format!("{error:?}")));
            assert!(hit);assert!(out.is_err());fs::write(p.join("write-error.txt"),format!("{out:?}")).unwrap();let report=recorder.latest.as_ref().unwrap();
            assert_eq!(report.commit,CommitState::Unconfirmed);assert_eq!(report.committed_advance_operations,None);assert!(!report.original_receipt_confirmed);assert_eq!(report.receipt_sequence,None);
            fs::write(p.join("observation.json"),serde_json::to_vec_pretty(&recorder.report(&Worker::new())).unwrap()).unwrap();drop(s);
            let mut recovered=Store::open(&p.join("world"),RULES).unwrap();assert_eq!(recovered.committed_sequence(),seq+u64::from(committed));
            if committed{assert_eq!(recovered.state_hash().unwrap(),reference.state_hash().unwrap());assert!(recovered.receipt(e.operation_id).unwrap().is_some());}else{assert_eq!(recovered.state_hash().unwrap(),before);assert!(recovered.receipt(e.operation_id).unwrap().is_none());}
            assert!(recovered.execute(Caller::System,e.clone()).unwrap().model.is_ok());assert_eq!(recovered.committed_sequence(),seq+1);assert_eq!(recovered.state_hash().unwrap(),reference.state_hash().unwrap());
            let hash=recovered.state_hash().unwrap();assert!(recovered.execute(Caller::System,e.clone()).unwrap().model.is_ok());assert_eq!(recovered.committed_sequence(),seq+1);assert_eq!(recovered.state_hash().unwrap(),hash);
            recovered.checkpoint().unwrap();cold(&p,&recovered,&e);drop(recovered);probe(&p);
        }
    }
    #[test] fn unknown_or_backwards_cpu_is_not_zero_work_or_business_failure() {
        assert_eq!(cpu_delta(Ok(7),Ok(12)),Nanoseconds::Known{ns:5});
        assert!(matches!(cpu_delta(Ok(12),Ok(7)),Nanoseconds::Unknown{..}));
        assert!(matches!(cpu_delta(Err(compute_worker::cpu_metrics::UnknownReason::UnsupportedPlatform),Ok(7)),Nanoseconds::Unknown{..}));
    }
    #[test] fn uncertain_write_cannot_be_reported_as_zero_committed_work() {
        assert_eq!(classify(&Err("lost_publication_reply".into()),7,8),CommitState::Unconfirmed);
    }
    #[test] fn recorded_rejection_and_its_replay_are_not_new_successful_work() {
        let result=Ok(Outcome {model:Err(mycity_core::transaction::TxError::InvalidOperation),has_persistent_receipt:true,committed_sequence:8});
        assert_eq!(classify(&result,7,8),CommitState::ModelRejectedRecorded);
        assert_eq!(classify(&result,8,8),CommitState::Replayed);
        let rejected=Ok(Outcome {model:Err(mycity_core::transaction::TxError::InvalidOperation),has_persistent_receipt:false,committed_sequence:7});
        assert_eq!(classify(&rejected,7,7),CommitState::RejectedWithoutReceipt);
    }
}
