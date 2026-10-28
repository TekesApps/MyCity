//! Requests bind the original current selection; they are never grants.
use crate::{compute_worker::{self,Worker},task_registry_directory::{self as directory,Directory,Route,Action as Selected,SourceBinding},task_registry_adapter::ActualJobBinding};
use mycity_core::{background,task_registry::{ClockKind,TaskKey},timeline::Progress};
use mycity_storage::{Store,Continuation};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use super::task_observation::{serialize_clock,serialize_key};

pub const VERSION:u16=2;
pub(super) const METADATA_PLAN_BYTES:usize=64*1024;
const CANONICAL_CAP:usize=8*1024;
const DESCRIPTION_CAP:usize=16*1024;
const SIM_STRATEGY:&str="original-timeline-advance-v1";

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TaskExecutionLocation { AirWorker, SimTimeline, Gpu }
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum TaskExecutionIntent {
    ExecuteOriginal,
    /// Equivalent numerical work; only its original Air page boundary changes.
    ExecuteExactPages { plan_version:u16, quota:crate::air_turns::Quota },
    RequestRefinement { strategy:String, strategy_version:u16 },
    RequestResources { cpu_nanoseconds:Option<u64>, additional_bytes:Option<u64> },
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundTaskRequest {
    pub capability_version:u16,
    pub binding_sha256:[u8;32],
    pub execution:TaskExecutionLocation,
    pub strategy:String,
    pub strategy_version:u16,
    pub intent:TaskExecutionIntent,
}

#[derive(Serialize)]
struct Scope { world_id:u64,branch_id:u64,city_id:Option<u32> }
#[derive(Serialize)]
struct AirSource<'a> {
    scope:Scope,
    #[serde(serialize_with="serialize_clock")]
    clock:Option<ClockKind>,
    id:u64,base_revision:u64,field_version:u64,start:u32,end:u32,
    input_identity:&'a str,checkpoint_sha256:[u8;32],incarnation:u64,generation:u32,
    attempt:u8,running:bool,failure:Option<background::Failure>,
}
impl<'a> From<&'a ActualJobBinding> for AirSource<'a> {
    fn from(b:&'a ActualJobBinding)->Self {Self {scope:Scope{world_id:b.scope.world_id,branch_id:b.scope.branch_id,city_id:b.scope.city_id},clock:Some(b.clock),
        id:b.id,base_revision:b.base_revision,field_version:b.field_version,start:b.start,end:b.end,input_identity:&b.input_identity,checkpoint_sha256:b.checkpoint_sha256,
        incarnation:b.incarnation,generation:b.generation,attempt:b.attempt,running:b.running,failure:b.failure}}
}
#[derive(Serialize)]
#[serde(tag="provider",content="binding",rename_all="snake_case")]
enum Source<'a> {
    Air(AirSource<'a>),
    Service(&'a mycity_core::service_needs_authority::DueView),
    Supply(&'a mycity_core::continuous_supply::DueView),
    Timeline(&'a Progress),
}
impl<'a> Source<'a> {
    fn from_binding(b:&'a SourceBinding)->Self {match b {SourceBinding::Air(v)=>Self::Air(v.into()),SourceBinding::Service(v)=>Self::Service(v),SourceBinding::Supply(v)=>Self::Supply(v),SourceBinding::Timeline(v)=>Self::Timeline(v)}}
}
#[derive(Serialize)]
#[serde(tag="kind",rename_all="snake_case")]
enum Plan { OriginalAir { job:u64 }, OriginalAdvance { target:u64,page_limit:u16 } }
#[derive(Serialize)]
struct WorkerState { active:bool,owner_empty:bool,pid:Option<u32> }
// This is the explicitly versioned typed serde-JSON encoding, not Directory's
// presentation JSON or Debug strings. Field order/enum spelling is this format;
// locked build source binds its encoder. No elapsed/heap/cache display enters it.
#[derive(Serialize)]
struct Canonical<'a> {
    encoding:&'static str,capability_version:u16,sim_protocol:&'static str,source_sha256:&'static str,
    application_instance:String,worker:WorkerState,scope:Scope,store_sequence:u64,
    progress:Option<Progress>,continuation:Option<Continuation>,source:Source<'a>,
    #[serde(serialize_with="serialize_clock")]
    clock:Option<ClockKind>,
    #[serde(serialize_with="serialize_key")]
    task:Option<TaskKey>,
    directory_policy_version:u32,execution:TaskExecutionLocation,strategy:&'static str,strategy_version:u16,plan:Plan,
}

#[derive(Serialize)]
struct AcceptedExecution<'a> {
    encoding:&'static str,
    request:&'a BoundTaskRequest,
}

fn capabilities(execution:Option<TaskExecutionLocation>)->Value {
    let air=execution==Some(TaskExecutionLocation::AirWorker);
    json!({"version":VERSION,"supported_request":"execute_original_current_selection",
        "exact_pages":if air {json!({"intent":"execute_exact_pages","plan":crate::air_turns::PLAN_ID,
            "plan_version":crate::air_turns::PLAN_VERSION,"effect":"equivalent_execution_granularity_not_model_refinement",
            "quota_bounds":{"source_limit":[1,8192],"cell_limit":[1,1024],"pages":[1,16]},
            "continuation":"original_PageControl_ticket_turn_instance; no_automatic_Continue"})}else{Value::Null},
        "gpu":"unsupported","refinement_request":"unsupported_model_change; exact_pages_is_equivalent_execution_only",
        "additional_resource_request":"unresolved_no_new_grant_or_shortage_claim",
        "air_wire":compute_worker::PROTOCOL,"control_frame_limit_bytes":compute_worker::CONTROL_BYTES,
        "candidate_transport_limit_bytes":air.then_some(mycity_core::air::CHECKPOINT_BYTES),
        "current_air_business_asset_limit_bytes":air.then_some(background::ASSET_LIMIT),
        "limits_are_memory_grants":false,"scratch_peak":"unknown_not_instrumented","residency_and_switch_cost":"unknown_not_calibrated",
        "allocation":"original_sim_role_or_original_air_owner_only; no_per_task_sim_grant_or_refund",
        "metadata_plan_bytes":METADATA_PLAN_BYTES,"metadata_plan_is_peak_proof":false})
}

pub(super) struct Prepared {
    request:BoundTaskRequest,
    canonical:Value,
    recovery:Value,
}
impl Prepared {
    pub(super) fn new(store:&Store,worker:&Worker,dir:&Directory)->Result<Option<Self>,String> {
        let(index,plan,execution,strategy)=match dir.route() {
            Route::Selected(Selected::Air(s))=>(0,Plan::OriginalAir{job:s.job()},TaskExecutionLocation::AirWorker,mycity_core::air::STRATEGY),
            Route::Selected(Selected::Service(s))=>(1,Plan::OriginalAdvance{target:s.target(),page_limit:s.page_limit()},TaskExecutionLocation::SimTimeline,SIM_STRATEGY),
            Route::Selected(Selected::Supply(s))=>(2,Plan::OriginalAdvance{target:s.target(),page_limit:s.page_limit()},TaskExecutionLocation::SimTimeline,SIM_STRATEGY),
            Route::Selected(Selected::Timeline(s))=>(3,Plan::OriginalAdvance{target:s.target(),page_limit:s.page_limit()},TaskExecutionLocation::SimTimeline,SIM_STRATEGY),
            _=>return Ok(None),
        };
        let scope=store.engine().map_err(|e|format!("{e:?}"))?.world().scope_revision().0;let entry=&dir.entries()[index];
        let recovery=match &plan {
            Plan::OriginalAir{job}=>json!({"kind":"original_air_job_and_accept_receipt","world":scope.world(),"branch":scope.branch(),"job":job,
                "accept_operation":background::accept_operation_id(*job)?,"inspect_job_and_original_owner_also":true,"absence_of_accept_is_not_no_attempt_proof":true}),
            Plan::OriginalAdvance{target,page_limit}=>{
                let envelope=super::continuation_envelope(store,*target,*page_limit)?;
                json!({"kind":"original_store_operation","envelope":envelope,"no_blind_retry_or_new_describe_as_retry":true})
            },
        };
        let engine=store.engine().map_err(|e|format!("{e:?}"))?;
        let canonical=Canonical {encoding:"mycity-bound-task-typed-json-v2",capability_version:VERSION,sim_protocol:super::PROTOCOL,source_sha256:env!("MYCITY_SOURCE_SHA256"),
            application_instance:crate::heap_meter::clock_reading().instance_unix_ns.to_string(),worker:WorkerState{active:worker.has_active_execution(),owner_empty:worker.is_empty(),pid:worker.pid_opt()},
            scope:Scope{world_id:scope.world(),branch_id:scope.branch(),city_id:None},store_sequence:store.committed_sequence(),progress:engine.timeline().map(|t|t.progress),
            continuation:store.continuation_status().map_err(|e|format!("{e:?}"))?,source:Source::from_binding(entry.binding.as_ref().ok_or("task_source_unavailable")?),
            clock:entry.clock,task:entry.key,directory_policy_version:directory::POLICY_VERSION,execution,strategy,strategy_version:1,plan};
        let bytes=serde_json::to_vec(&canonical).map_err(|e|e.to_string())?;if bytes.len()>CANONICAL_CAP{return Err("task_binding_bound".into());}
        let digest:[u8;32]=Sha256::digest(&bytes).into();let canonical=serde_json::to_value(canonical).map_err(|e|e.to_string())?;
        Ok(Some(Self {request:BoundTaskRequest{capability_version:VERSION,binding_sha256:digest,execution,strategy:strategy.into(),strategy_version:1,intent:TaskExecutionIntent::ExecuteOriginal},canonical,recovery}))
    }
    // The host validates supported request fields once at its input boundary.
    // Here only the recomputed current selection is compared. Original handlers
    // remain responsible for business admission, including operation collisions.
    pub(super) fn validate_binding(&self,request:&BoundTaskRequest)->Result<(),String> {
        if request.execution!=self.request.execution||request.strategy!=self.request.strategy||request.strategy_version!=self.request.strategy_version{return Err("task_execution_or_strategy_mismatch".into());}
        if request.binding_sha256!=self.request.binding_sha256{return Err("task_binding_stale_recover_original_result".into());}Ok(())
    }
    pub(super) fn stamp(&self,request:&BoundTaskRequest)->Result<Value,String> {
        // The base Describe digest covers selection. This digest also covers the
        // actual requested initial plan/quota; subsequent controls keep their
        // original ticket/turn/instance authority rather than rewriting it.
        let accepted=AcceptedExecution{encoding:"mycity-accepted-task-execution-v1",request};
        let bytes=serde_json::to_vec(&accepted).map_err(|e|e.to_string())?;
        let digest:[u8;32]=Sha256::digest(&bytes).into();
        Ok(json!({"capability_version":VERSION,"binding_sha256":self.request.binding_sha256,
            "accepted_execution_sha256":digest,"accepted_execution_encoding":"mycity-accepted-task-execution-v1",
            "initial_intent":request.intent,"recovery":self.recovery,"meaning":"validated_selection_not_business_completion_or_resource_grant"}))
    }
    fn description(&self)->Value {json!({"schema":"mycity-task-execution-capabilities-v2","state":"ready","capabilities":capabilities(Some(self.request.execution)),
        "selection":self.canonical,"selection_is_business_admission":false,"request":self.request,"recovery":self.recovery})}
}
pub(super) fn validate_supported_request(request:&BoundTaskRequest)->Result<Option<crate::air_turns::Quota>,String> {
    if request.capability_version!=VERSION{return Err("task_capability_version_unsupported".into());}
    if request.execution==TaskExecutionLocation::Gpu{return Err("task_gpu_unsupported_no_cpu_fallback".into());}
    if request.strategy.is_empty()||request.strategy.len()>64||!request.strategy.bytes().all(|b|b.is_ascii_graphic()){return Err("task_strategy_label".into());}
    match request.intent {
        TaskExecutionIntent::ExecuteOriginal=>Ok(None),
        TaskExecutionIntent::ExecuteExactPages{plan_version,quota}=>{
            if request.execution!=TaskExecutionLocation::AirWorker{return Err("task_exact_pages_requires_air".into());}
            if plan_version!=crate::air_turns::PLAN_VERSION||quota.validate().is_err(){return Err("task_exact_pages_plan".into());}
            Ok(Some(quota))
        },
        TaskExecutionIntent::RequestRefinement{..}=>Err("task_refinement_unsupported_no_strategy_substitution".into()),
        TaskExecutionIntent::RequestResources{..}=>Err("task_resource_change_unresolved_no_grant_or_shortage_claim".into()),
    }
}
pub(super) fn describe(store:&Store,worker:&Worker)->Result<Value,String> {
    let dir=directory::collect(store,worker.has_active_execution()).map_err(|e|e.to_string())?;
    let result=if let Some(prepared)=Prepared::new(store,worker,&dir)?{prepared.description()}else {
        let state=match dir.route(){Route::NoWork=>"no_work",Route::Busy=>"busy",Route::Blocked(_)=>"blocked",_=>unreachable!()};
        json!({"schema":"mycity-task-execution-capabilities-v2","state":state,"capabilities":capabilities(None),"request":Value::Null,"required_preserved":true})
    };
    if serde_json::to_vec(&result).map_err(|e|e.to_string())?.len()>DESCRIPTION_CAP{return Err("task_description_bound".into());}Ok(result)
}

#[cfg(test)]
mod exact_request_binding_tests {
    use super::*;
    #[test]
    fn accepted_digest_binds_every_initial_quota_field_without_changing_selection() {
        // Encoding contract only, not evidence of a real owner or grant.
        let request=BoundTaskRequest{capability_version:VERSION,binding_sha256:[7;32],execution:TaskExecutionLocation::AirWorker,
            strategy:mycity_core::air::STRATEGY.into(),strategy_version:1,intent:TaskExecutionIntent::ExecuteOriginal};
        let prepared=Prepared{request:request.clone(),canonical:Value::Null,recovery:Value::Null};
        let original=prepared.stamp(&request).unwrap();let mut seen=vec![original["accepted_execution_sha256"].clone()];
        for quota in [crate::air_turns::Quota{source_limit:1,cell_limit:17,pages:1},
            crate::air_turns::Quota{source_limit:2,cell_limit:17,pages:1},crate::air_turns::Quota{source_limit:1,cell_limit:18,pages:1},
            crate::air_turns::Quota{source_limit:1,cell_limit:17,pages:2}] {
            let mut changed=request.clone();changed.intent=TaskExecutionIntent::ExecuteExactPages{plan_version:crate::air_turns::PLAN_VERSION,quota};
            assert_eq!(validate_supported_request(&changed).unwrap(),Some(quota));let stamp=prepared.stamp(&changed).unwrap();
            assert_eq!(stamp["binding_sha256"],original["binding_sha256"]);assert!(!seen.contains(&stamp["accepted_execution_sha256"]));
            assert_eq!(stamp["initial_intent"],serde_json::to_value(&changed.intent).unwrap());seen.push(stamp["accepted_execution_sha256"].clone());
        }
    }
}
