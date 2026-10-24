//! Production supervision of one existing sliced command. No second process owner,
//! grant, scheduler or Store writer. Drop does not cancel or acknowledge retirement.
use crate::{air_page_metrics::Summary, air_page_policy::{Config,Decision,Policy},
    air_turns::{self,Boundary,Continue,Quota}, compute_worker::{self,Ticket,Stage},
    sim_client::{Client,ReceiveError}, sim_host::{Command,Event,PageControl}};
use serde::{Serialize,Deserialize};
use std::time::Duration;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Phase { Running, BoundaryReady, QueryPending, CancelPending, ControlUncertain, Finished }
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Purpose { PendingNextTurn, TerminalObservationOnly }
#[derive(Clone,Debug,Serialize)]
pub struct DecisionRecord { pub ticket:Ticket, pub boundary:Boundary, pub decision:Decision, pub purpose:Purpose }
#[derive(Debug)]
pub struct Step { pub event:Event, pub decision:Option<DecisionRecord> }
#[derive(Clone,Copy)]
enum Query { Observe, Receipt(u64) }
struct Pending { boundary:Boundary, summary:Option<Summary>, next_policy:Policy, record:DecisionRecord }

/// Must live inside the caller's original retained Client scope. After uncertain
/// writes, use `retire`/the original Client cleanup; never start an implicit retry.
/// Receive timeout alone is not retirement: `step` may poll the same owner again.
#[must_use="retain session/Client until original command finishes or explicit retirement is acknowledged"]
pub struct AdaptiveSession<'a> {
    client:&'a mut Client,
    policy:Policy,
    job:u64,
    phase:Phase,
    ticket:Option<Ticket>,
    instance:Option<String>,
    expected_turn:u64,
    quota:Quota,
    wire_boundary:Option<(Boundary,Option<Summary>)>,
    final_sample:Option<(Boundary,Option<Summary>)>,
    final_metrics_status:Option<&'static str>,
    pending:Option<Pending>,
    last_attempted:Option<DecisionRecord>,
    query:Option<Query>,
}
impl<'a> AdaptiveSession<'a> {
    pub fn start(client:&'a mut Client,id:u64,job:u64,quota:Quota,config:Config)->Result<Self,String> {
        if job==0 {return Err("adaptive_job".into());}
        let policy=Policy::new(quota,config).map_err(|e|format!("adaptive_policy:{e:?}"))?;
        // Reserve the command id BEFORE the first write, including a partial write.
        // Failed start leaves Client owned/busy until original explicit stop.
        client.submit_retained(id,Command::RunRequiredSliced{job,plan:air_turns::PLAN_ID.into(),quota})?;
        Ok(Self{client,policy,job,phase:Phase::Running,ticket:None,instance:None,expected_turn:1,quota,
            wire_boundary:None,final_sample:None,final_metrics_status:None,pending:None,last_attempted:None,query:None})
    }
    pub fn phase(&self)->Phase {self.phase}
    pub fn final_metrics_status(&self)->Option<&'static str> {self.final_metrics_status}
    pub fn pending_decision(&self)->Option<&DecisionRecord> {self.pending.as_ref().map(|p|&p.record)}
    /// Attempted means write was attempted, not delivery, execution, release or publication.
    pub fn last_attempted_decision(&self)->Option<&DecisionRecord> {self.last_attempted.as_ref()}
    pub fn retarget(&mut self,config:Config)->Result<&DecisionRecord,String> {
        if self.phase!=Phase::BoundaryReady {return Err("adaptive_not_at_boundary".into());}
        let mut updated=self.policy.clone();
        updated.retarget(config).map_err(|e|format!("adaptive_retarget:{e:?}"))?;
        let p=self.pending.as_ref().ok_or("adaptive_missing_pending")?;
        let boundary=p.boundary.clone();let summary=p.summary;
        let (next_policy,record)=Self::plan(&updated,self.ticket.as_ref().ok_or("adaptive_ticket")?,&boundary,summary);
        self.policy=updated;
        self.pending=Some(Pending{boundary,summary,next_policy,record});
        Ok(&self.pending.as_ref().expect("pending installed").record)
    }
    pub fn observe(&mut self)->Result<(),String> {self.query(PageControl::Observe)}
    pub fn receipt_proof(&mut self,operation:u64)->Result<(),String> {self.query(PageControl::ReceiptProof{operation})}
    fn query(&mut self,control:PageControl)->Result<(),String> {
        if self.phase!=Phase::BoundaryReady {return Err("adaptive_not_at_boundary".into());}
        self.query=Some(match control {PageControl::Observe=>Query::Observe,
            PageControl::ReceiptProof{operation}=>Query::Receipt(operation),_=>return Err("adaptive_query_kind".into())});
        self.phase=Phase::ControlUncertain;
        self.client.control_pages(control)?;
        self.phase=Phase::QueryPending;Ok(())
    }
    pub fn cancel(&mut self)->Result<(),String> {
        if self.phase!=Phase::BoundaryReady {return Err("adaptive_not_at_boundary".into());}
        self.pending=None;
        self.phase=Phase::ControlUncertain;
        self.client.control_pages(PageControl::Cancel)?;
        self.phase=Phase::CancelPending;Ok(())
    }
    pub fn retire(&mut self)->Result<(),String> {
        self.phase=Phase::ControlUncertain;self.pending=None;
        self.client.stop()?;self.phase=Phase::Finished;Ok(())
    }
    fn plan(policy:&Policy,ticket:&Ticket,boundary:&Boundary,summary:Option<Summary>)->(Policy,DecisionRecord) {
        let mut next=policy.clone();let decision=next.decide(summary.as_ref());
        (next,DecisionRecord{ticket:ticket.clone(),boundary:boundary.clone(),decision,purpose:Purpose::PendingNextTurn})
    }
    fn send_pending(&mut self)->Result<(),String> {
        let next_turn=self.expected_turn.checked_add(1).ok_or("adaptive_turn_overflow")?;
        let pending=self.pending.take().ok_or("adaptive_missing_pending")?;
        self.phase=Phase::ControlUncertain;
        let quota=pending.next_policy.current_quota();
        let control=Continue{plan_version:pending.boundary.plan_version,turn:pending.boundary.turn,
            process_instance:pending.boundary.process_instance.clone(),quota};
        self.policy=pending.next_policy;self.quota=quota;self.last_attempted=Some(pending.record);
        // Pending was consumed before write. Any error leaves ControlUncertain;
        // no second step can resend it. Original Client/Sim still own retirement.
        self.client.control_pages(PageControl::Continue{next:control})?;
        self.expected_turn=next_turn;self.phase=Phase::Running;Ok(())
    }
    fn bind_boundary(&mut self,b:&Boundary)->Result<(),String> {
        air_turns::validate_process_instance(&b.process_instance).map_err(|_|"adaptive_instance")?;
        if b.plan_version!=air_turns::PLAN_VERSION || b.turn!=self.expected_turn
            || self.instance.as_ref().is_some_and(|i|i!=&b.process_instance) {return Err("adaptive_boundary_identity".into());}
        self.instance=Some(b.process_instance.clone());Ok(())
    }
    fn receive(&mut self,event:&Event)->Result<Option<DecisionRecord>,String> {
        if matches!(self.phase,Phase::QueryPending|Phase::CancelPending)
            && !matches!(event.kind.as_str(),"page_control_result"|"attempt_failed"|"result") {
            return Err("adaptive_control_response_order".into());
        }
        match event.kind.as_str() {
            "worker_started"=>{
                let ticket:Ticket=serde_json::from_value(event.payload["ticket"].clone()).map_err(|e|e.to_string())?;
                let digests=[&ticket.rules_sha256,&ticket.source_sha256,&ticket.input_sha256,&ticket.checkpoint_sha256];
                if self.ticket.is_some() || ticket.job!=self.job || ticket.world==0 || ticket.branch==0
                    || ticket.incarnation==0 || ticket.generation==0 || !(1..=3).contains(&ticket.attempt)
                    || ticket.start>=ticket.end || ticket.source_sha256!=env!("MYCITY_SOURCE_SHA256")
                    || !digests.iter().all(|s|s.len()==64 && s.bytes().all(|b|b.is_ascii_hexdigit())) {
                    return Err("adaptive_worker_identity".into());
                }
                self.ticket=Some(ticket);
            }
            "worker_progress"=>{
                let mut value=event.payload.clone();value.as_object_mut().ok_or("adaptive_progress")?.remove("task_observation");
                let wire:compute_worker::Event=serde_json::from_value(value).map_err(|e|e.to_string())?;
                if wire.protocol!=compute_worker::PROTOCOL || Some(&wire.ticket)!=self.ticket.as_ref()
                    || Some(wire.pid)!=self.client.worker_pid() {return Err("adaptive_progress_identity".into());}
                match wire.stage {
                    Stage::SliceBoundary=>{
                        let boundary=wire.turn.ok_or("adaptive_boundary_absent")?;self.bind_boundary(&boundary)?;
                        let summary=wire.page_metrics.filter(|s|s.validate().is_ok()&&s.work==boundary.work);
                        if self.wire_boundary.is_some() {return Err("adaptive_duplicate_wire_boundary".into());}
                        self.wire_boundary=Some((boundary,summary));
                    }
                    Stage::Published=>{
                        let boundary=wire.turn.ok_or("adaptive_final_absent")?;self.bind_boundary(&boundary)?;
                        let summary=wire.page_metrics.filter(|s|s.validate().is_ok()&&s.work==boundary.work);
                        self.final_sample=Some((boundary.clone(),summary));
                        let report=DecisionRecord{ticket:wire.ticket,boundary,
                            decision:self.policy.preview_final(summary.as_ref()),purpose:Purpose::TerminalObservationOnly};
                        return Ok(Some(report));
                    }
                    Stage::Complete=>{
                        let boundary=wire.turn.ok_or("adaptive_complete_absent")?;
                        let summary=wire.page_metrics.filter(|s|s.validate().is_ok()&&s.work==boundary.work);
                        if self.accept_final_metrics(&boundary,summary)? {
                            // Auxiliary sampling changed: invalidate the old diagnostic,
                            // not original business completion, and never send Continue.
                            return Ok(Some(DecisionRecord{ticket:wire.ticket,boundary,
                                decision:self.policy.preview_final(None),purpose:Purpose::TerminalObservationOnly}));
                        }
                    }
                    _=>{}
                }
            }
            "page_boundary"=>{
                if self.phase!=Phase::Running {return Err("adaptive_boundary_phase".into());}
                let ticket:Ticket=serde_json::from_value(event.payload["ticket"].clone()).map_err(|e|e.to_string())?;
                let boundary:Boundary=serde_json::from_value(event.payload["boundary"].clone()).map_err(|e|e.to_string())?;
                if Some(&ticket)!=self.ticket.as_ref() || event.payload["plan"]!=air_turns::PLAN_ID {return Err("adaptive_boundary_ticket".into());}
                let (wire,summary)=self.wire_boundary.take().ok_or("adaptive_unmatched_boundary")?;
                if wire!=boundary {return Err("adaptive_boundary_changed".into());}
                let (next_policy,record)=Self::plan(&self.policy,&ticket,&boundary,summary);
                self.pending=Some(Pending{boundary,summary,next_policy,record:record.clone()});
                self.phase=Phase::BoundaryReady;return Ok(Some(record));
            }
            "page_control_result"=>{
                if self.phase!=Phase::QueryPending {return Err("adaptive_unsolicited_query".into());}
                let value=event.payload.get("Ok").ok_or("adaptive_query_failed")?;
                match self.query.take().ok_or("adaptive_query_absent")? {
                    Query::Observe if value["still_unresolved"]==true && value.get("task_observation").is_some()=>{},
                    Query::Receipt(operation) if value["operation"].as_u64()==Some(operation)
                        && value["present"].is_boolean()=>{},
                    _=>return Err("adaptive_query_response_identity".into()),
                }
                self.phase=Phase::BoundaryReady;
            }
            "result"=>{self.pending=None;self.phase=Phase::Finished;}
            "attempt_failed"=>{},
            _=>return Err("adaptive_event_kind".into()),
        }
        Ok(None)
    }
    fn accept_final_metrics(&mut self,boundary:&Boundary,summary:Option<Summary>)->Result<bool,String> {
        let (old,metrics)=self.final_sample.as_mut().ok_or("adaptive_complete_without_published")?;
        if old!=boundary {return Err("adaptive_final_boundary_changed".into());}
        if *metrics!=summary {
            *metrics=None;self.final_metrics_status=Some("changed_or_missing_at_complete");return Ok(true);
        }
        self.final_metrics_status=Some(if summary.is_some(){"frozen_final_summary"}else{"unavailable"});Ok(false)
    }
    /// One event per step. A returned BoundaryReady/Decision is not yet sent.
    /// QueryPending must read its response before any Continue; CancelPending can
    /// only drain original failure/result. A receive timeout does not restart work.
    pub fn step(&mut self,timeout:Duration)->Result<Step,String> {
        match self.phase {
            Phase::BoundaryReady=>self.send_pending()?,
            Phase::ControlUncertain=>return Err("adaptive_control_uncertain_retire_required".into()),
            Phase::Finished=>return Err("adaptive_finished".into()),
            _=>{}
        }
        let event=match self.client.next_typed(timeout) {
            Ok(event)=>event,
            Err(ReceiveError::Timeout)=>return Err("adaptive_receive_timeout".into()),
            Err(ReceiveError::Fault(error))=>{self.phase=Phase::ControlUncertain;return Err(error);}
        };
        let decision=match self.receive(&event) {Ok(d)=>d,Err(e)=>{self.phase=Phase::ControlUncertain;return Err(e);}};
        Ok(Step{event,decision})
    }
}

#[cfg(test)]
#[path="adaptive_client_tests.rs"]
mod tests;
