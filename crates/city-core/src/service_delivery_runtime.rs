//! Bounded persistent local Development service duties. Root owns all actual authentication,
//! original pools/member ledger, money, input consumption, need links and atomic Store writes.
use crate::{arena::Scope, member_qualification::MemberRange, model_rows::{Record, Rows},
    service_needs::Kind, units::Carry};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::{self, Write}, mem::size_of};

pub const VERSION: u32 = 2;
pub const MAX_TASKS: usize = 4;
pub const MAX_SERIALIZED_BYTES: usize = 64 * 1024;
pub const MAX_RESTORE_DEPTH: usize = 16;
pub const PREPARATION_SECONDS: u64 = 300;
pub const RESET_SECONDS: u64 = 600;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Policy { LocalDevelopmentExplicitHostActivityV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Clock { Development }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedKey {
    pub scope: Scope, pub city_id: u32, pub household_id: u32, pub household_source: u64,
    pub generation: u32, pub kind: Kind, pub occurrence_day: u32,
}
impl NeedKey {
    pub fn occurred_at(self) -> Result<u64, Error> {
        u64::from(self.occurrence_day).checked_mul(86_400).and_then(|x|x.checked_add(28_800)).ok_or(Error::Overflow)
    }
    pub fn deadline(self) -> Result<u64, Error> { self.occurred_at()?.checked_add(86_400).ok_or(Error::Overflow) }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crew {
    pub id: u32, pub kind: Kind, pub members: MemberRange, pub operator: u32,
    pub crew_resource: u32, pub vehicle_resource: u32, pub tools_resource: u32,
}
impl Record for Crew {
    fn inactive() -> Self { Self { id:0, kind:Kind::Medical, members:MemberRange{start:0,end:0},
        operator:0, crew_resource:0, vehicle_resource:0, tools_resource:0 } }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version: u32, pub policy: Policy, pub scope: Scope, pub city_id: u32,
    pub population_origin: [u8;32], pub member_version: u32,
    pub crews: Rows<Crew,3>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe { pub people:u64, pub work_seconds:u64, pub energy_mwh:u64, pub water_ml:u64, pub cents:u64 }
pub const fn recipe(kind:Kind)->Recipe {
    match kind {
        Kind::Medical=>Recipe{people:2,work_seconds:3600,energy_mwh:100_000,water_ml:20_000,cents:3000},
        Kind::Police=>Recipe{people:2,work_seconds:3600,energy_mwh:100_000,water_ml:10_000,cents:2000},
        Kind::Fire=>Recipe{people:4,work_seconds:1800,energy_mwh:1_000_000,water_ml:1_000_000,cents:5000},
    }
}
impl Opening {
    pub fn validate(&self)->Result<(),Error> {
        if self.version!=VERSION || self.city_id==0 || self.population_origin==[0;32] || self.member_version==0 || self.crews.is_empty() {return Err(Error::Registration);}
        for (i,c) in self.crews.iter().enumerate() {
            if c.id==0 || c.operator==0 || c.members.start>=c.members.end
                || c.members.end-c.members.start!=recipe(c.kind).people
                || c.crew_resource==0 || c.vehicle_resource==0 || c.tools_resource==0
                || c.crew_resource==c.vehicle_resource || c.crew_resource==c.tools_resource || c.vehicle_resource==c.tools_resource
                || self.crews[..i].iter().any(|old|old.id==c.id || old.kind==c.kind) {return Err(Error::Registration);}
        } Ok(())
    }
    pub fn registration_fingerprint(&self)->Result<[u8;32],Error>{hash(self,b"MyCity/service-delivery/registration/v2\0")}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase { Accepted, Dispatched, OnSite, WorkComplete, Returning, Reset, Closed }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub lease_tokens:[u64;3], pub member_activity:u64,
    pub origin_place:u64, pub target_place:u64, pub route_version:u64,
    pub protected_until:u64, pub funding_source:[u8;32], pub inputs_source:[u8;32], pub return_source:[u8;32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeadlineResult {pub at:u64,pub arrival:Option<u64>,pub work_seconds:u64,pub operation_id:u64}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origins {
    pub need:u64,pub accepted:u64,pub departed:u64,pub arrived:u64,pub return_started:u64,
    pub returned:u64,pub reset:u64,pub closed:u64,pub cancelled:u64,pub last_work:u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub delivery_id:u32, pub need:NeedKey, pub crew_id:u32, pub revision:u64,
    pub assignment:Assignment, pub phase:Phase, pub accepted_at:u64,
    pub position:u64,
    pub accepted_root:[u8;32],pub accepted_world_revision:u64,pub origins:Origins,
    pub last_work_sources:[[u8;32];3],
    pub departure_at:Option<u64>,pub arrival_at:Option<u64>,pub return_at:Option<u64>,pub returned_at:Option<u64>,pub reset_at:Option<u64>,
    pub cancelled:bool,pub work_cursor:u64,pub work_seconds:u64,
    /// Frozen physical end of the onsite interval. Unsettled prefixes survive return/reset.
    pub onsite_end:Option<u64>,
    pub energy_mwh:u64,pub water_ml:u64,pub energy_carry:Carry,pub water_carry:Carry,pub fee_carry:Carry,
    pub accrued_cents:u64,pub deadline:Option<DeadlineResult>,
    pub last_host_event:u64,pub last_operation:u64,
}
impl Record for Task {
    fn inactive()->Self { Self {delivery_id:0,need:NeedKey{scope:Scope::new(1,1).expect("fill scope"),city_id:0,
        household_id:0,household_source:0,generation:0,kind:Kind::Medical,occurrence_day:0},crew_id:0,revision:0,
        assignment:Assignment{lease_tokens:[0;3],member_activity:0,origin_place:0,target_place:0,route_version:0,protected_until:0,
            funding_source:[0;32],inputs_source:[0;32],return_source:[0;32]},phase:Phase::Accepted,accepted_at:0,position:0,
        accepted_root:[0;32],accepted_world_revision:0,origins:Origins::default(),last_work_sources:[[0;32];3],
        departure_at:None,arrival_at:None,return_at:None,returned_at:None,reset_at:None,cancelled:false,work_cursor:0,work_seconds:0,onsite_end:None,
        energy_mwh:0,water_ml:0,energy_carry:Carry::new(0,1).expect("fill carry"),water_carry:Carry::new(0,1).expect("fill carry"),
        fee_carry:Carry::new(0,1).expect("fill carry"),accrued_cents:0,deadline:None,last_host_event:0,last_operation:0} }
}
impl Task {
    /// Necessary accounting responsibility, not a past timestamp to use as the current clock.
    pub fn unsettled_onsite_through(&self)->Option<u64>{self.onsite_end.filter(|end|self.work_cursor<*end)}
    fn deadline_cursor_target(&self)->Result<Option<u64>,Error>{
        let deadline=self.need.deadline()?;
        Ok(self.arrival_at.filter(|arrival|*arrival<=deadline).map(|_|self.onsite_end.map_or(deadline,|end|end.min(deadline))))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {pub closed:u64,pub reclaimed:u64,pub chain:[u8;32],pub last_delivery:u32,pub last_need:Option<NeedKey>,pub last_operation:u64}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    #[serde(skip)] ready:bool,
    opening:Opening, tasks:Rows<Task,MAX_TASKS>, history:History,
    next_delivery:u32, ordinal:u64, last_at:u64,last_host_event:u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {Accept{crew_id:u32},Depart,Arrive,Work,BeginReturn,Returned,ResetComplete,Cancel,SealDeadline,Close}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {pub delivery_id:u32,pub need:NeedKey,pub expected_revision:Option<u64>,pub action:Action}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {Registration,Invalid,Stale,Conflict,Unauthorized,WrongPhase,Overflow,Budget,Unauthenticated}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {Unknown,UnresolvedBudget,NoQualifiedCrew,NoCapacity,NoFunding,NoInputs,NoSafeReturn,ClosedToNew,DeadlinePending,HistoricalPending}
/// No Deserialize: only the original root prepared authority may construct these capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Source {pub scope:Scope,pub city_id:u32,pub clock:Clock,pub root:[u8;32],pub revision:u64,pub at:u64,pub operation_id:u64}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Admission {
    pub assignment:Assignment,pub protected_cents:u64,pub contract_end:u64,
    pub need_generated_operation:u64,pub qualification_root:[u8;32],
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct HostActivity {
    pub event_id:u64,pub operation_id:u64,pub occurred_at:u64,pub members:MemberRange,
    pub from_place:u64,pub to_place:u64,pub route_version:u64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkPrefix {
    pub from:u64,pub through:u64,pub work_seconds:u64,pub member_activity:u64,
    pub qualification_root:[u8;32],pub electric_source:[u8;32],pub water_source:[u8;32],
    pub energy_mwh:u64,pub water_ml:u64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Verified {
    Admission(Admission),Host(HostActivity),Work(WorkPrefix),
    /// Full deadline coverage over canonical actual sources, not lack of solver output.
    Deadline{covered_through:u64},
    /// Root validated cancellation authority. Unstarted duties are later closed atomically.
    Cancellation,
    /// Root prepared original lease/member release AND lawful money/obligation disposition.
    Closure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub delivery_id:u32,pub need:NeedKey,pub action:Action,
    pub before_cents:u64,pub target_cents:u64,pub delta_cents:u64,
    pub consumed_energy_mwh:u64,pub consumed_water_ml:u64,
    pub phase:Phase,pub necessary:bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {pub command:Command,pub operation_id:u64,pub effect:Effect}
pub(crate) trait Authority {
    /// Original Envelope/receipt lookup BEFORE current root/freshness/CAS, never filtered by payload.
    fn prior(&self)->Result<Option<Receipt>,Error>;
    fn source(&self)->Result<Source,Error>;
    /// Must verify complete command and same root against actual original authorities. No
    /// deserialized client object, plan, venue Enter or nonzero ID is actual authentication.
    fn verify(&self,command:&Command,before:Option<&Task>)->Result<Verified,Gate>;
}
pub(crate) struct Candidate {pub expected:[u8;32],pub source:Source,pub next:Box<Runtime>,pub receipt:Receipt}
pub(crate) enum Outcome {Ready(Candidate),Replay(Receipt),Wait(Gate)}
impl Candidate {
    pub fn matches(&self,current:&Runtime)->Result<bool,Error>{Ok(current.ready&&self.next.ready&&self.expected==current.fingerprint()?)}
}
fn remaining(t:&Task)->Result<u64,Error>{
    let physical=match t.phase {Phase::Accepted=>6,Phase::Dispatched=>5,Phase::OnSite|Phase::WorkComplete=>4,
        Phase::Returning=>3,Phase::Reset=>if t.reset_at.is_some(){1}else{2},Phase::Closed=>0};
    Ok(physical+u64::from(t.deadline.is_none())+u64::from(!t.cancelled&&t.phase!=Phase::Closed)+settlement_reserve(t)?)
}
/// Reserve a final aggregate historical prefix on each side of an unsealed deadline.
/// Optional partial Work may consume spare ordinals, never these final closing slots.
fn settlement_reserve(t:&Task)->Result<u64,Error>{
    if t.phase==Phase::Closed{return Ok(0);}
    if t.arrival_at.is_none(){return Ok(if matches!(t.phase,Phase::Accepted|Phase::Dispatched){2}else{0});}
    if let Some(end)=t.onsite_end{
        if t.work_cursor>=end{return Ok(0);}
        return Ok(1+u64::from(t.deadline.is_none()&&t.work_cursor<t.need.deadline()?&&end>t.need.deadline()?));
    }
    Ok(1+u64::from(t.deadline.is_none()&&t.work_cursor<t.need.deadline()?))
}
fn host_remaining(t:&Task)->u64{match t.phase{Phase::Accepted=>5,Phase::Dispatched=>4,
    Phase::OnSite|Phase::WorkComplete=>3,Phase::Returning=>2,Phase::Reset=>u64::from(t.reset_at.is_none()),Phase::Closed=>0}}
impl Runtime {
    pub fn new(opening:Opening,at:u64)->Result<Self,Error>{opening.validate()?;Ok(Self{ready:true,opening,tasks:Rows::new().map_err(|_|Error::Budget)?,
        history:History{closed:0,reclaimed:0,chain:[0;32],last_delivery:0,last_need:None,last_operation:0},next_delivery:1,ordinal:0,last_at:at,last_host_event:0})}
    pub fn opening(&self)->&Opening{&self.opening}
    pub fn tasks(&self)->&[Task]{&self.tasks}
    pub fn task(&self,id:u32)->Option<&Task>{self.tasks.iter().find(|t|t.delivery_id==id)}
    pub fn history(&self)->History{self.history}
    pub fn is_ready(&self)->bool{self.ready}
    pub fn next_delivery_id(&self)->u32{self.next_delivery}
    pub fn ordinal(&self)->u64{self.ordinal}
    pub fn last_host_event(&self)->u64{self.last_host_event}
    pub fn last_at(&self)->u64{self.last_at}
    pub fn registration_fingerprint(&self)->Result<[u8;32],Error>{self.opening.registration_fingerprint()}
    pub fn fingerprint(&self)->Result<[u8;32],Error>{hash(self,b"MyCity/service-delivery/runtime/v2\0")}
    pub(crate) fn mark_authenticated(&mut self)->Result<(),Error>{self.validate()?;self.ready=true;Ok(())}
    pub const fn owned_bytes()->usize{size_of::<Self>()+3*size_of::<Crew>()+MAX_TASKS*size_of::<Task>()}
    /// Extra prepared payload plus bounded reclaim backing and explicit working copies.
    /// Caller must ALSO charge its current Runtime, other root candidates and native stack.
    pub const fn candidate_bytes()->usize{size_of::<Candidate>()+Self::owned_bytes()
        +(MAX_TASKS+4)*size_of::<Task>()+size_of::<Rows<Task,MAX_TASKS>>()+2*size_of::<HashWriter>()}
    pub const fn restore_workspace_bytes()->usize{Self::owned_bytes()+MAX_SERIALIZED_BYTES}
    pub fn next_due(&self)->Result<Option<u64>,Error>{
        let mut due=None;
        for t in self.tasks.iter(){
            if t.deadline.is_none(){minimum(&mut due,t.need.deadline()?);}
            if t.phase==Phase::Reset&&t.reset_at.is_none(){minimum(&mut due,t.returned_at.ok_or(Error::Invalid)?.checked_add(RESET_SECONDS).ok_or(Error::Overflow)?);}
        }Ok(due)
    }
    pub fn validate(&self)->Result<(),Error>{
        self.opening.validate()?;
        if self.next_delivery==0||self.history.reclaimed>self.history.closed{return Err(Error::Invalid);}
        let mut reserve=0u64;let mut host_reserve=0u64;
        for (i,t) in self.tasks.iter().enumerate(){
            validate_task(&self.opening,t,self.last_at)?;
            let conflict=self.tasks[..i].iter().any(|old|{
                let shared=old.crew_id==t.crew_id||old.assignment.member_activity==t.assignment.member_activity
                    ||old.assignment.lease_tokens.iter().any(|token|t.assignment.lease_tokens.contains(token));
                old.delivery_id==t.delivery_id||old.need==t.need||(old.phase!=Phase::Closed&&t.phase!=Phase::Closed&&shared)
            });
            if t.delivery_id>=self.next_delivery||t.last_host_event>self.last_host_event||conflict{return Err(Error::Invalid);}
            reserve=reserve.checked_add(remaining(t)?).ok_or(Error::Overflow)?;
            host_reserve=host_reserve.checked_add(host_remaining(t)).ok_or(Error::Overflow)?;
            if u64::MAX-t.revision<remaining(t)?{return Err(Error::Budget);}
        }
        let open=u64::try_from(self.tasks.iter().filter(|t|t.phase!=Phase::Closed).count()).map_err(|_|Error::Overflow)?;
        if u64::MAX-self.history.closed<open{return Err(Error::Budget);}
        if u64::MAX-self.ordinal<reserve||u64::MAX-self.last_host_event<host_reserve{return Err(Error::Budget);}Ok(())
    }
}
fn validate_task(o:&Opening,t:&Task,at:u64)->Result<(),Error>{
    let c=o.crews.iter().find(|c|c.id==t.crew_id).ok_or(Error::Registration)?;let r=recipe(t.need.kind);
    let deadline=t.need.deadline()?;
    if t.need.scope!=o.scope||t.need.city_id!=o.city_id||t.need.household_id==0||t.need.household_source==0||t.need.generation!=1||t.need.kind!=c.kind
        ||t.delivery_id==0||t.revision==0||t.last_operation==0||t.accepted_at<t.need.occurred_at()?||t.accepted_at>=t.need.deadline()?
        ||t.accepted_at>at||t.position==0||t.accepted_root==[0;32]||t.origins.need==0||t.origins.accepted==0
        ||t.work_seconds>r.work_seconds||t.accrued_cents>r.cents{return Err(Error::Invalid);}
    for (present,op) in [(t.departure_at.is_some(),t.origins.departed),(t.arrival_at.is_some(),t.origins.arrived),
        (t.return_at.is_some(),t.origins.return_started),(t.returned_at.is_some(),t.origins.returned),
        (t.reset_at.is_some(),t.origins.reset),(t.phase==Phase::Closed,t.origins.closed),(t.cancelled,t.origins.cancelled)]{
        if present!=(op!=0){return Err(Error::Invalid);}
    }
    if t.work_seconds>0&&(t.origins.last_work==0||t.last_work_sources.contains(&[0;32])){return Err(Error::Invalid);}
    let a=t.assignment;
    if a.member_activity==0||a.lease_tokens.contains(&0)||a.lease_tokens[0]==a.lease_tokens[1]||a.lease_tokens[0]==a.lease_tokens[2]||a.lease_tokens[1]==a.lease_tokens[2]
        ||a.origin_place==0||a.target_place==0||a.route_version==0||a.funding_source==[0;32]||a.inputs_source==[0;32]||a.return_source==[0;32]||a.protected_until<=t.accepted_at{return Err(Error::Invalid);}
    let mut e=Carry::new(r.energy_mwh,r.work_seconds).map_err(|_|Error::Invalid)?;
    let mut w=Carry::new(r.water_ml,r.work_seconds).map_err(|_|Error::Invalid)?;
    let mut f=Carry::new(r.cents*3/5,r.work_seconds).map_err(|_|Error::Invalid)?;
    let energy=e.accrue(t.work_seconds).map_err(|_|Error::Overflow)?;let water=w.accrue(t.work_seconds).map_err(|_|Error::Overflow)?;
    let work_fee=f.accrue(t.work_seconds).map_err(|_|Error::Overflow)?;
    if e!=t.energy_carry||w!=t.water_carry||f!=t.fee_carry||t.energy_mwh<energy||t.water_ml<water{return Err(Error::Invalid);}
    let expected=work_fee+(if t.departure_at.is_some(){r.cents/5}else{0})+(if t.reset_at.is_some(){r.cents/5}else{0});
    if t.accrued_cents!=expected{return Err(Error::Invalid);}
    if let Some(dep)=t.departure_at{if dep<t.accepted_at.checked_add(PREPARATION_SECONDS).ok_or(Error::Overflow)?||dep>at{return Err(Error::Invalid);}}
    if let Some(arr)=t.arrival_at{if t.departure_at.is_none_or(|dep|arr<dep)||arr>at||t.work_cursor<arr{return Err(Error::Invalid);}}
    if t.work_cursor>at||(t.arrival_at.is_none()&&(t.work_cursor!=0||t.work_seconds!=0)){return Err(Error::Invalid);}
    if t.onsite_end!=t.return_at.filter(|_|t.arrival_at.is_some())
        ||t.onsite_end.is_some_and(|end|t.arrival_at.is_none_or(|arrival|end<arrival)||t.work_cursor>end){return Err(Error::Invalid);}
    if let Some(ret)=t.return_at{if t.departure_at.is_none_or(|dep|ret<dep)||t.arrival_at.is_some_and(|arr|ret<arr)||ret<t.work_cursor||ret>at{return Err(Error::Invalid);}}
    if let Some(home)=t.returned_at{if t.return_at.is_none_or(|ret|home<ret)||home>at{return Err(Error::Invalid);}}
    if let Some(reset)=t.reset_at{if t.returned_at.is_none_or(|home|home.checked_add(RESET_SECONDS).is_none_or(|end|reset<end))||reset>at{return Err(Error::Invalid);}}
    match t.phase{
        Phase::Accepted=>if t.departure_at.is_some()||t.position!=a.origin_place{return Err(Error::Invalid);},
        Phase::Dispatched=>if t.departure_at.is_none()||t.arrival_at.is_some()||t.return_at.is_some()||t.position!=a.origin_place{return Err(Error::Invalid);},
        Phase::OnSite|Phase::WorkComplete=>if t.arrival_at.is_none()||t.return_at.is_some()||t.position!=a.target_place{return Err(Error::Invalid);},
        Phase::Returning=>if t.return_at.is_none()||t.returned_at.is_some(){return Err(Error::Invalid);},
        Phase::Reset=>if t.returned_at.is_none()||t.position!=a.origin_place{return Err(Error::Invalid);},
        Phase::Closed=>if (!(t.cancelled&&t.departure_at.is_none())&&t.reset_at.is_none())||t.unsettled_onsite_through().is_some(){return Err(Error::Invalid);},
    }
    if t.phase==Phase::WorkComplete&&t.work_seconds!=r.work_seconds{return Err(Error::Invalid);}
    if let Some(d)=t.deadline{
        if d.at!=t.need.deadline()?||d.at>at||d.operation_id==0||d.work_seconds>t.work_seconds
            ||d.arrival!=t.arrival_at.filter(|x|*x<=d.at){return Err(Error::Invalid);}
        if t.deadline_cursor_target()?.is_some_and(|target|t.work_cursor<target){return Err(Error::Invalid);}
    }else if t.work_cursor>deadline&&!(t.work_seconds==0&&t.arrival_at.is_some_and(|a|a>deadline)){return Err(Error::Invalid);}
    Ok(())
}

pub(crate) fn prepare(current:&Runtime,a:&impl Authority,command:Command)->Result<Outcome,Error>{
    if let Some(receipt)=a.prior()? {
        return if receipt.command==command {Ok(Outcome::Replay(receipt))} else {Err(Error::Conflict)};
    }
    if !current.ready{return Err(Error::Unauthenticated);}current.validate()?;
    let source=a.source()?;
    if source.scope!=current.opening.scope||source.city_id!=current.opening.city_id||source.root==[0;32]
        ||source.operation_id==0||source.at<current.last_at{return Err(Error::Stale);}
    let key=command.need;
    if key.scope!=source.scope||key.city_id!=source.city_id||key.household_id==0||key.household_source==0||key.generation!=1{return Err(Error::Registration);}
    let before=current.task(command.delivery_id).copied();
    if before.map(|t|t.revision)!=command.expected_revision{return Err(Error::Stale);}
    if before.is_some_and(|t|t.need!=key){return Err(Error::Conflict);}
    if !matches!(command.action,Action::Accept{..})&&before.is_none(){return Err(Error::WrongPhase);}
    let verified=match a.verify(&command,before.as_ref()){Ok(f)=>f,Err(g)=>return Ok(Outcome::Wait(g))};
    let mut next=Box::new(current.clone());
    let before_cents=before.map_or(0,|t|t.accrued_cents);
    let mut delta_energy=0;let mut delta_water=0;
    let mut task=if let Action::Accept{crew_id}=command.action {
        if before.is_some()||command.delivery_id!=0||command.expected_revision.is_some(){return Err(Error::Conflict);}
        if current.tasks.iter().any(|t|t.need==key||(t.crew_id==crew_id&&t.phase!=Phase::Closed)){return Ok(Outcome::Wait(Gate::NoCapacity));}
        // Root's canonical need verification also excludes all older closed need keys.
        if current.history.last_need==Some(key){return Err(Error::Conflict);}
        let crew=current.opening.crews.iter().find(|c|c.id==crew_id&&c.kind==key.kind).ok_or(Error::Registration)?;
        let Verified::Admission(admission)=verified else{return Err(Error::Unauthorized)};
        let r=recipe(key.kind);
        let day_start=(source.at/86_400).checked_mul(86_400).ok_or(Error::Overflow)?;
        let service_end=admission.contract_end.min(day_start.checked_add(64_800).ok_or(Error::Overflow)?);
        let latest=service_end.checked_sub(14_400).ok_or(Error::Invalid)?;
        if source.at<key.occurred_at()?||source.at>=key.deadline()?||source.at<day_start+28_800||source.at>latest
            ||admission.need_generated_operation==0||admission.qualification_root==[0;32]
            ||admission.protected_cents<r.cents||admission.assignment.protected_until<service_end{
            return Err(Error::Unauthorized);
        }
        // Reclaim only durably closed + deadline-sealed rows from the before state.
        // Current transient candidates never grant a spare slot to a concurrent accept.
        if next.tasks.len()==MAX_TASKS {
            let reclaim=next.tasks.iter().position(|t|t.phase==Phase::Closed&&t.deadline.is_some()).ok_or(Error::Budget)?;
            let old=next.tasks[reclaim];let mut rows=Rows::new().map_err(|_|Error::Budget)?;
            for (i,row) in next.tasks.iter().enumerate(){if i!=reclaim{rows.try_push(*row).map_err(|_|Error::Budget)?;}}
            next.tasks=rows;
            next.history.reclaimed=next.history.reclaimed.checked_add(1).ok_or(Error::Overflow)?;
            next.history.chain=hash(&(next.history.chain,old,source.operation_id),b"MyCity/service-delivery/reclaim/v2\0")?;
        }
        let id=next.next_delivery;
        next.next_delivery=id.checked_add(1).ok_or(Error::Overflow)?;
        let t=Task{delivery_id:id,need:key,crew_id:crew.id,revision:1,assignment:admission.assignment,phase:Phase::Accepted,
            accepted_at:source.at,position:admission.assignment.origin_place,departure_at:None,arrival_at:None,return_at:None,
            accepted_root:source.root,accepted_world_revision:source.revision,
            origins:Origins{need:admission.need_generated_operation,accepted:source.operation_id,..Origins::default()},last_work_sources:[[0;32];3],
            returned_at:None,reset_at:None,cancelled:false,work_cursor:0,work_seconds:0,onsite_end:None,energy_mwh:0,water_ml:0,
            energy_carry:Carry::new(r.energy_mwh,r.work_seconds).map_err(|_|Error::Invalid)?,
            water_carry:Carry::new(r.water_ml,r.work_seconds).map_err(|_|Error::Invalid)?,
            fee_carry:Carry::new(r.cents*3/5,r.work_seconds).map_err(|_|Error::Invalid)?,
            accrued_cents:0,deadline:None,last_host_event:0,last_operation:source.operation_id};
        next.tasks.try_push(t).map_err(|_|Error::Budget)?;t
    }else{before.ok_or(Error::WrongPhase)?};
    if !matches!(command.action,Action::Accept{..}){
        if task.phase==Phase::Closed&&command.action!=Action::SealDeadline{return Err(Error::WrongPhase);}
        let r=recipe(key.kind);
        match command.action{
            Action::Accept{..}=>return Err(Error::Invalid),
            Action::Depart|Action::Arrive|Action::BeginReturn|Action::Returned|Action::ResetComplete=>{
                let Verified::Host(host)=verified else{return Err(Error::Unauthorized)};
                let crew=current.opening.crews.iter().find(|c|c.id==task.crew_id).ok_or(Error::Registration)?;
                if host.operation_id!=source.operation_id||host.event_id!=next.last_host_event.checked_add(1).ok_or(Error::Overflow)?
                    ||host.members!=crew.members||host.occurred_at!=source.at||host.route_version!=task.assignment.route_version
                    ||host.from_place==0||host.to_place==0{return Err(Error::Unauthorized);}
                let at=host.occurred_at;
                match command.action{
                    Action::Depart=>{
                        if task.phase!=Phase::Accepted{return Err(Error::WrongPhase);}
                        if at<task.accepted_at.checked_add(PREPARATION_SECONDS).ok_or(Error::Overflow)?
                            ||host.from_place!=task.assignment.origin_place||host.to_place!=host.from_place{return Err(Error::Invalid);}
                        // Existing actual departure is recorded even after cancellation. Authority
                        // must supply actual Host, never a new-permission/closed-city gate here.
                        task.departure_at=Some(at);task.phase=Phase::Dispatched;task.accrued_cents=r.cents/5;
                        task.origins.departed=source.operation_id;
                    }
                    Action::Arrive=>{
                        if task.phase!=Phase::Dispatched{return Err(Error::WrongPhase);}
                        if host.from_place!=task.position||host.to_place!=task.assignment.target_place{return Err(Error::Invalid);}
                        task.arrival_at=Some(at);task.work_cursor=at;task.phase=Phase::OnSite;
                        task.origins.arrived=source.operation_id;
                        // Late actual arrival creates no pre-deadline work or response credit.
                    }
                    Action::BeginReturn=>{
                        if !matches!(task.phase,Phase::Dispatched|Phase::OnSite|Phase::WorkComplete){return Err(Error::WrongPhase);}
                        if host.to_place!=host.from_place||at<task.work_cursor{return Err(Error::Invalid);}
                        task.return_at=Some(at);task.phase=Phase::Returning;
                        task.onsite_end=task.arrival_at.map(|_|at);
                        task.origins.return_started=source.operation_id;
                    }
                    Action::Returned=>{
                        if task.phase!=Phase::Returning{return Err(Error::WrongPhase);}
                        if host.from_place!=task.position||host.to_place!=task.assignment.origin_place{return Err(Error::Invalid);}
                        task.returned_at=Some(at);task.phase=Phase::Reset;
                        task.origins.returned=source.operation_id;
                    }
                    Action::ResetComplete=>{
                        if task.phase!=Phase::Reset||task.reset_at.is_some(){return Err(Error::WrongPhase);}
                        if at<task.returned_at.ok_or(Error::Invalid)?.checked_add(RESET_SECONDS).ok_or(Error::Overflow)?
                            ||host.from_place!=task.assignment.origin_place||host.to_place!=host.from_place{return Err(Error::Invalid);}
                        task.reset_at=Some(at);task.accrued_cents=task.accrued_cents.checked_add(r.cents/5).ok_or(Error::Overflow)?;
                        task.origins.reset=source.operation_id;
                    }
                    _=>return Err(Error::Invalid),
                }
                task.position=host.to_place;task.last_host_event=host.event_id;next.last_host_event=host.event_id;
            }
            Action::Work=>{
                if !matches!(task.phase,Phase::OnSite|Phase::WorkComplete|Phase::Returning|Phase::Reset)||task.arrival_at.is_none(){return Err(Error::WrongPhase);}
                let Verified::Work(prefix)=verified else{return Err(Error::Unauthorized)};
                if prefix.from!=task.work_cursor||prefix.through<=prefix.from||prefix.through>source.at
                    ||prefix.member_activity!=task.assignment.member_activity||prefix.qualification_root==[0;32]
                    ||prefix.electric_source==[0;32]||prefix.water_source==[0;32]
                    ||prefix.work_seconds>prefix.through-prefix.from
                    ||task.onsite_end.is_some_and(|end|prefix.through>end){return Err(Error::Unauthorized);}
                if task.deadline.is_none()&&prefix.through>key.deadline()?{return Ok(Outcome::Wait(Gate::DeadlinePending));}
                let work=task.work_seconds.checked_add(prefix.work_seconds).ok_or(Error::Overflow)?;
                if work>r.work_seconds{return Err(Error::Invalid);}
                let energy=task.energy_carry.accrue(prefix.work_seconds).map_err(|_|Error::Overflow)?;
                let water=task.water_carry.accrue(prefix.work_seconds).map_err(|_|Error::Overflow)?;
                if prefix.energy_mwh<energy||prefix.water_ml<water{return Err(Error::Unauthorized);}
                let fee=task.fee_carry.accrue(prefix.work_seconds).map_err(|_|Error::Overflow)?;
                task.work_cursor=prefix.through;task.work_seconds=work;
                task.origins.last_work=source.operation_id;
                task.last_work_sources=[prefix.qualification_root,prefix.electric_source,prefix.water_source];
                task.energy_mwh=task.energy_mwh.checked_add(prefix.energy_mwh).ok_or(Error::Overflow)?;
                task.water_ml=task.water_ml.checked_add(prefix.water_ml).ok_or(Error::Overflow)?;
                task.accrued_cents=task.accrued_cents.checked_add(fee).ok_or(Error::Overflow)?;
                delta_energy=prefix.energy_mwh;delta_water=prefix.water_ml;
                if work==r.work_seconds&&task.phase==Phase::OnSite{task.phase=Phase::WorkComplete;}
            }
            Action::Cancel=>{
                if !matches!(verified,Verified::Cancellation){return Err(Error::Unauthorized);}
                if task.cancelled{return Err(Error::WrongPhase);}task.cancelled=true;task.origins.cancelled=source.operation_id;
            }
            Action::SealDeadline=>{
                let Verified::Deadline{covered_through}=verified else{return Err(Error::Unauthorized)};
                let deadline=key.deadline()?;
                if task.deadline.is_some()||source.at<deadline||covered_through<deadline||covered_through>source.at{return Err(Error::WrongPhase);}
                // Provider source coverage is not evidence that this duty consumed its prefix.
                // Preserve deadline-pending until all pre-deadline onsite accounting is settled.
                if task.deadline_cursor_target()?.is_some_and(|target|task.work_cursor<target){return Ok(Outcome::Wait(Gate::DeadlinePending));}
                task.deadline=Some(DeadlineResult{at:deadline,arrival:task.arrival_at.filter(|a|*a<=deadline),
                    work_seconds:task.work_seconds,operation_id:source.operation_id});
                if task.arrival_at.is_some_and(|a|a>deadline){task.work_cursor=task.arrival_at.ok_or(Error::Invalid)?;}
            }
            Action::Close=>{
                if !matches!(verified,Verified::Closure){return Err(Error::Unauthorized);}
                if task.unsettled_onsite_through().is_some(){return Ok(Outcome::Wait(Gate::HistoricalPending));}
                if !((task.phase==Phase::Accepted&&task.cancelled)||(task.phase==Phase::Reset&&task.reset_at.is_some())){return Err(Error::WrongPhase);}
                task.phase=Phase::Closed;
                task.origins.closed=source.operation_id;
                next.history.closed=next.history.closed.checked_add(1).ok_or(Error::Overflow)?;
                next.history.last_delivery=task.delivery_id;next.history.last_need=Some(task.need);next.history.last_operation=source.operation_id;
                next.history.chain=hash(&(next.history.chain,task,source.operation_id),b"MyCity/service-delivery/close/v2\0")?;
            }
        }
        task.revision=task.revision.checked_add(1).ok_or(Error::Overflow)?;task.last_operation=source.operation_id;
        let index=next.tasks.iter().position(|t|t.delivery_id==task.delivery_id).ok_or(Error::Invalid)?;next.tasks[index]=task;
    }
    next.ordinal=next.ordinal.checked_add(1).ok_or(Error::Overflow)?;next.last_at=source.at;
    next.validate()?;
    let effect=Effect{delivery_id:task.delivery_id,need:key,action:command.action,before_cents,target_cents:task.accrued_cents,
        delta_cents:task.accrued_cents.checked_sub(before_cents).ok_or(Error::Invalid)?,consumed_energy_mwh:delta_energy,
        consumed_water_ml:delta_water,phase:task.phase,necessary:task.phase!=Phase::Closed||task.deadline.is_none()};
    Ok(Outcome::Ready(Candidate{expected:current.fingerprint()?,source,next,receipt:Receipt{command,operation_id:source.operation_id,effect}}))
}
fn minimum(slot:&mut Option<u64>,at:u64){*slot=Some(slot.map_or(at,|old|old.min(at)));}
struct HashWriter{state:Sha256,count:usize}
impl Write for HashWriter{
    fn write(&mut self,bytes:&[u8])->io::Result<usize>{self.count=self.count.checked_add(bytes.len()).ok_or_else(||io::Error::other("delivery_bytes"))?;
        if self.count>MAX_SERIALIZED_BYTES{return Err(io::Error::other("delivery_bytes"));}self.state.update(bytes);Ok(bytes.len())}
    fn flush(&mut self)->io::Result<()>{Ok(())}
}
fn hash(value:&impl Serialize,domain:&[u8])->Result<[u8;32],Error>{let mut w=HashWriter{state:Sha256::new(),count:0};w.state.update(domain);
    serde_json::to_writer(&mut w,value).map_err(|_|Error::Budget)?;Ok(w.state.finalize().into())}
