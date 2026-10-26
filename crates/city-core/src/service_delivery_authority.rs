//! Original-writer service bridge. The graph supplies legal edges, not arrival facts.
//! This runtime is staged with the existing needs, member, Resource/Lease and money state.
use crate::{arena::Scope, member_qualification::MemberRange, model_rows::{Record,Rows},
    service_delivery_runtime as d, service_needs::Kind, transaction::TxError, transport};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};

pub const VERSION:u32=1;
pub const DRIVE_MODE:u8=2;
pub const MAX_EDGES:usize=32;
pub const MAX_ROUTES:usize=32;
pub const MAX_PATH:usize=16;
pub const MAX_OWNED_BYTES:usize=128*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Profile { LocalDevelopmentEdgeDrivenPoliceV1 }
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeBinding {pub edge:u32,pub resource:u32,pub energy_mwh_per_second:u64}
impl Record for EdgeBinding {fn inactive()->Self{Self{edge:0,resource:0,energy_mwh_per_second:0}}}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wage {pub member:u64,pub account:u32,pub cents_per_hour:u64,pub valid_from:u64,pub valid_until:u64}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrewBinding {
    pub crew:u32,pub station_entrance:u32,pub driver:u64,pub energy_stock:u32,pub water_stock:u32,
    /// A finite original Opening rule, explicitly non-potable police utility use only.
    pub police_utility_water_stock:u32,pub employer:u32,pub wages:[Option<Wage>;4],
}
impl Record for CrewBinding {fn inactive()->Self{Self{crew:0,station_entrance:0,driver:0,energy_stock:0,water_stock:0,police_utility_water_stock:0,employer:0,wages:[None;4]}}}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub household:u32,pub target_entrance:u32,pub fee_payer:u32,pub fee_payee:u32,
    pub out:[Option<u32>;MAX_PATH],pub back:[Option<u32>;MAX_PATH],
}
impl Record for Route {fn inactive()->Self{Self{household:0,target_entrance:0,fee_payer:0,fee_payee:0,out:[None;MAX_PATH],back:[None;MAX_PATH]}}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version:u32,pub profile:Profile,pub domain:d::Opening,
    pub graph:transport::Opening,pub edges:Rows<EdgeBinding,MAX_EDGES>,
    pub crews:Rows<CrewBinding,3>,pub routes:Rows<Route,MAX_ROUTES>,
}
fn bad(_:impl std::fmt::Debug)->TxError{TxError::InvalidOperation}
fn add(a:u64,b:u64)->Result<u64,TxError>{a.checked_add(b).ok_or(TxError::Overflow)}
fn mul(a:u64,b:u64)->Result<u64,TxError>{a.checked_mul(b).ok_or(TxError::Overflow)}
impl Opening {
    pub fn validate(&self)->Result<(),TxError>{
        self.domain.validate().map_err(bad)?;self.graph.validate().map_err(bad)?;
        if self.version!=VERSION || self.domain.crews.len()!=self.crews.len() || !self.graph.demands.is_empty()
            || self.edges.is_empty() || self.routes.is_empty(){return Err(TxError::InvalidOperation);}
        for (i,e) in self.edges.iter().enumerate(){
            let edge=self.graph.edges.iter().find(|r|r.id==e.edge).ok_or(TxError::InvalidOperation)?;
            if e.resource==0 || e.energy_mwh_per_second==0 || edge.mode_mask&DRIVE_MODE==0
                || edge.city_from!=self.domain.city_id || edge.city_to!=self.domain.city_id
                || self.edges[..i].iter().any(|r|r.edge==e.edge){return Err(TxError::InvalidOperation);}
        }
        for (i,c) in self.crews.iter().enumerate(){
            let crew=self.domain.crews.iter().find(|r|r.id==c.crew).ok_or(TxError::InvalidOperation)?;
            let entrance=self.graph.entrances.iter().find(|r|r.id==c.station_entrance).ok_or(TxError::InvalidOperation)?;
            if crew.kind!=Kind::Police || c.driver<crew.members.start || c.driver>=crew.members.end
                || c.energy_stock==0 || c.water_stock==0 || c.water_stock==c.energy_stock
                || c.police_utility_water_stock!=c.water_stock || c.employer!=crew.operator
                || !entrance.authorized || entrance.owner!=crew.operator || entrance.mode_mask&DRIVE_MODE==0
                || self.crews[..i].iter().any(|r|r.crew==c.crew){return Err(TxError::InvalidOperation);}
            let n=usize::try_from(crew.members.end-crew.members.start).map_err(bad)?;
            for (j,wage) in c.wages.iter().enumerate(){match wage{
                Some(w) if j<n&&w.member==crew.members.start+j as u64&&w.account!=0&&w.account!=c.employer
                    &&w.cents_per_hour>0&&w.valid_from<w.valid_until=>{},
                None if j>=n=>{},_=>return Err(TxError::InvalidOperation),
            }}
        }
        for (i,r) in self.routes.iter().enumerate(){
            if r.household==0||r.fee_payer==0||r.fee_payee==0||r.fee_payer==r.fee_payee
                ||self.routes[..i].iter().any(|p|p.household==r.household){return Err(TxError::InvalidOperation);}
            let target=self.graph.entrances.iter().find(|p|p.id==r.target_entrance).ok_or(TxError::InvalidOperation)?;
            if !target.authorized||target.mode_mask&DRIVE_MODE==0{return Err(TxError::InvalidOperation);}
            for c in self.crews.iter(){
                let origin=self.graph.entrances.iter().find(|p|p.id==c.station_entrance).ok_or(TxError::InvalidOperation)?;
                self.check_path(&r.out,origin.node,target.node)?;self.check_path(&r.back,target.node,origin.node)?;
            }
        }
        Ok(())
    }
    fn check_path(&self,path:&[Option<u32>;MAX_PATH],mut node:u32,target:u32)->Result<(),TxError>{
        let mut ended=false;let mut n=0;
        for id in path {if let Some(id)=id{
            if ended{return Err(TxError::InvalidOperation);}
            let edge=self.graph.edges.iter().find(|e|e.id==*id).ok_or(TxError::InvalidOperation)?;
            if edge.from!=node||!edge.open||edge.mode_mask&DRIVE_MODE==0
                ||!self.edges.iter().any(|e|e.edge==*id){return Err(TxError::InvalidOperation);}
            node=edge.to;n+=1;
        }else{ended=true;}}
        if n==0||node!=target{return Err(TxError::InvalidOperation);}Ok(())
    }
    pub(crate) fn owned_bytes(&self)->usize{std::mem::size_of::<Self>()+self.graph.nodes.capacity()*std::mem::size_of::<transport::Node>()+self.graph.edges.capacity()*std::mem::size_of::<transport::RoadEdge>()+self.graph.entrances.capacity()*std::mem::size_of::<transport::Entrance>()+self.graph.demands.capacity()*std::mem::size_of::<transport::OdDemand>()+MAX_EDGES*std::mem::size_of::<EdgeBinding>()+3*std::mem::size_of::<CrewBinding>()+MAX_ROUTES*std::mem::size_of::<Route>()+3*std::mem::size_of::<d::Crew>()}
    fn route(&self,household:u32)->Result<Route,TxError>{self.routes.iter().find(|r|r.household==household).copied().ok_or(TxError::InvalidOperation)}
    fn crew(&self,id:u32)->Result<CrewBinding,TxError>{self.crews.iter().find(|r|r.crew==id).copied().ok_or(TxError::InvalidOperation)}
    fn edge(&self,id:u32)->Result<(transport::RoadEdge,EdgeBinding),TxError>{Ok((
        *self.graph.edges.iter().find(|r|r.id==id).ok_or(TxError::InvalidOperation)?,
        *self.edges.iter().find(|r|r.edge==id).ok_or(TxError::InvalidOperation)?))}
    fn path_cost(&self,path:&[Option<u32>;MAX_PATH])->Result<(u64,u64),TxError>{
        path.iter().flatten().try_fold((0,0),|(t,q),id|{let(e,b)=self.edge(*id)?;Ok((add(t,u64::from(e.free_flow_seconds))?,add(q,mul(u64::from(e.free_flow_seconds),b.energy_mwh_per_second)?)?))})
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Motion { Preparing,Outbound,OnSite,Returning,Reset,Finished }
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trip {
    pub delivery:u32,pub crew:u32,pub household:u32,pub motion:Motion,pub route_revision:u64,
    pub current_edge:Option<u32>,pub edge_seconds:u64,pub edge_token:Option<u64>,pub path_index:u8,
    pub cursor:u64,pub blocked:bool,pub energy_basis_cents:u64,pub water_basis_cents:u64,pub drive_energy_mwh:u64,pub need_generated_operation:u64,
    pub fee_paid:u64,pub fee_protected:u64,pub wage_protected:u64,pub wages_paid:[u64;4],pub wages_due:[u64;4],pub wage_carry:[u64;4],
    pub stock_energy_reserved:u64,pub stock_water_reserved:u64,pub last_operation:u64,
}
impl Record for Trip {fn inactive()->Self{Self{delivery:0,crew:0,household:0,motion:Motion::Finished,route_revision:0,current_edge:None,edge_seconds:0,edge_token:None,path_index:0,cursor:0,blocked:false,energy_basis_cents:0,water_basis_cents:0,drive_energy_mwh:0,need_generated_operation:0,fee_paid:0,fee_protected:0,wage_protected:0,wages_paid:[0;4],wages_due:[0;4],wage_carry:[0;4],stock_energy_reserved:0,stock_water_reserved:0,last_operation:0}}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    #[serde(skip)] ready:bool,
    opening:Box<Opening>,domain:Box<d::Runtime>,trips:Rows<Trip,{d::MAX_TASKS}>,
    closed_to_new:bool,wake:bool,edge_open:[bool;MAX_EDGES],covered_through:u64,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Command { Accept{household:u32,crew:u32},Cancel{delivery:u32},BeginReturn{delivery:u32},SetOpen{open:bool},SetEdgeOpen{edge:u32,open:bool} }
/// Only root's transaction-local adapter implements this over original staged tables.
/// No method accepts caller-provided arrival, receipts, qualification or a replacement pool.
pub(crate) trait Ledger {
    fn scope(&self)->Scope;
    fn root(&self)->[u8;32];
    fn revision(&self)->u64;
    fn operation(&self)->u64;
    fn permits(&self,operator:u32)->bool;
    fn generated_need(&self,household:u32,kind:Kind)->Result<(d::NeedKey,u64),TxError>;
    fn link_need(&mut self,key:d::NeedKey,delivery:u32)->Result<(),TxError>;
    fn qualified(&self,members:MemberRange,driver:u64,at:u64)->Result<bool,TxError>;
    fn claim(&mut self,operator:u32,resources:[u32;3])->Result<[u64;3],TxError>;
    fn release(&mut self,operator:u32,tokens:[u64;3])->Result<(),TxError>;
    fn enter_edge(&mut self,operator:u32,resource:u32,old_token:Option<u64>)->Result<Option<u64>,TxError>;
    fn exit_edge(&mut self,operator:u32,token:u64)->Result<(),TxError>;
    fn begin_members(&mut self,delivery:u32,members:MemberRange,at:u64)->Result<u64,TxError>;
    fn settle_members(&mut self,delivery:u32,activity:u64,at:u64,close:bool)->Result<(),TxError>;
    fn available(&mut self,account:u32)->Result<u64,TxError>;
    fn protect(&mut self,account:u32,cents:u64)->Result<(),TxError>;
    fn release_money(&mut self,account:u32,cents:u64)->Result<(),TxError>;
    fn pay_protected(&mut self,from:u32,to:u32,cents:u64)->Result<(),TxError>;
    fn stock_available(&mut self,stock:u32,owner:u32,electric:bool)->Result<u64,TxError>;
    fn consume(&mut self,stock:u32,owner:u32,quantity:u64,electric:bool)->Result<u64,TxError>;
}
struct Grant {source:d::Source,verified:d::Verified}
impl d::Authority for Grant {
    fn prior(&self)->Result<Option<d::Receipt>,d::Error>{Ok(None)} // Original Engine performs Envelope dedup first.
    fn source(&self)->Result<d::Source,d::Error>{Ok(self.source)}
    fn verify(&self,_:&d::Command,_:Option<&d::Task>)->Result<d::Verified,d::Gate>{Ok(self.verified)}
}
impl Runtime {
    pub(crate) fn new(opening:Opening)->Result<Self,TxError>{opening.validate()?;let mut edge_open=[false;MAX_EDGES];for(i,b)in opening.edges.iter().enumerate(){edge_open[i]=opening.edge(b.edge)?.0.open;}
        let domain=Box::new(d::Runtime::new(opening.domain.clone(),0).map_err(bad)?);
        Ok(Self{ready:true,opening:Box::new(opening),domain,trips:Rows::new().map_err(bad)?,closed_to_new:false,wake:false,edge_open,covered_through:0})}
    pub(crate) fn wake_after_input_change(&mut self){self.wake=self.trips.iter().any(|t|t.motion!=Motion::Finished);for t in self.trips.iter_mut(){t.blocked=false;}}
    pub(crate) fn mark_authenticated(&mut self)->Result<(),TxError>{self.validate()?;self.domain.mark_authenticated().map_err(bad)?;self.ready=true;Ok(())}
    pub(crate) fn validate_member_rows(&self,ledger:&crate::member_activity::Ledger)->Result<(),TxError>{
        for t in self.trips.iter(){let task=self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;
            let c=self.opening.domain.crews.iter().find(|c|c.id==t.crew).ok_or(TxError::InvalidOperation)?;
            let active=ledger.active().iter().find(|r|r.id==task.assignment.member_activity);
            if t.motion==Motion::Finished{if active.is_some(){return Err(TxError::InvalidOperation);}}
            else if !active.is_some_and(|r|r.owner==service_member_owner(t.delivery)&&r.kind==crate::member_activity::Kind::ServiceWork&&r.members==[Some(crate::member_activity::Members{start:c.members.start,end:c.members.end}),None]){return Err(TxError::InvalidOperation);}
        }Ok(())
    }
    pub(crate) fn owns_member(&self,id:u64,owner:u64)->bool{self.trips.iter().filter(|t|t.motion!=Motion::Finished).any(|t|owner==service_member_owner(t.delivery)&&self.domain.task(t.delivery).is_some_and(|r|r.assignment.member_activity==id))}
    pub fn opening(&self)->&Opening{&self.opening}
    pub fn trips(&self)->&[Trip]{&self.trips}
    pub fn tasks(&self)->&[d::Task]{self.domain.tasks()}
    pub fn covered_through(&self)->u64{self.covered_through}
    pub fn is_ready(&self)->bool{self.ready&&self.domain.is_ready()}
    pub(crate) fn owned_bytes(&self)->usize{std::mem::size_of::<Self>()+self.opening.owned_bytes()+d::Runtime::owned_bytes()+d::MAX_TASKS*std::mem::size_of::<Trip>()}
    pub fn fingerprint(&self)->Result<[u8;32],TxError>{Ok(Sha256::digest(serde_json::to_vec(self).map_err(bad)?).into())}
    pub(crate) fn validate(&self)->Result<(),TxError>{self.opening.validate()?;self.domain.validate().map_err(bad)?;
        if self.owned_bytes()>MAX_OWNED_BYTES{return Err(TxError::WorkingLimit);}
        for t in self.trips.iter(){let task=self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;
            if t.crew!=task.crew_id||t.household!=task.need.household_id||t.cursor>self.covered_through
                ||t.route_revision!=self.opening.graph.revision||t.edge_token.is_some()!=t.current_edge.is_some(){return Err(TxError::InvalidOperation);}}
        Ok(())}
    fn apply(&mut self,l:&impl Ledger,at:u64,id:u32,action:d::Action,verified:d::Verified)->Result<d::Receipt,TxError>{
        let task=self.domain.task(id);let need=task.map(|t|t.need).ok_or(TxError::InvalidOperation)?;
        let g=Grant{source:d::Source{scope:l.scope(),city_id:self.opening.domain.city_id,clock:d::Clock::Development,root:l.root(),revision:l.revision(),at,operation_id:l.operation()},verified};
        match d::prepare(&self.domain,&g,d::Command{delivery_id:id,need,expected_revision:task.map(|t|t.revision),action}).map_err(bad)?{
            d::Outcome::Ready(c)=>{let(next,receipt)=c.into_parts(&self.domain).map_err(bad)?;self.domain=next;Ok(receipt)},
            d::Outcome::Replay(receipt)=>Ok(receipt),d::Outcome::Wait(_)=>Err(TxError::InvalidOperation),
        }
    }
    fn host(&mut self,l:&impl Ledger,at:u64,id:u32,action:d::Action,from:u64,to:u64)->Result<d::Receipt,TxError>{
        let task=self.domain.task(id).ok_or(TxError::InvalidOperation)?;
        let crew=self.opening.domain.crews.iter().find(|c|c.id==task.crew_id).ok_or(TxError::InvalidOperation)?;
        let host=d::HostActivity{event_id:self.domain.last_host_event().checked_add(1).ok_or(TxError::Overflow)?,operation_id:l.operation(),occurred_at:at,members:crew.members,from_place:from,to_place:to,route_version:self.opening.graph.revision};
        self.apply(l,at,id,action,d::Verified::Host(host))
    }
    pub(crate) fn command(&mut self,l:&mut impl Ledger,command:Command,at:u64)->Result<(),TxError>{
        if !self.is_ready()||at!=self.covered_through{return Err(TxError::InvalidOperation);}
        match command{
            Command::Accept{household,crew}=>self.accept(l,at,household,crew),
            Command::SetOpen{open}=>{if !self.opening.crews.iter().all(|c|l.permits(c.employer)){return Err(TxError::Unauthorized);}self.closed_to_new=!open;Ok(())},
            Command::SetEdgeOpen{edge,open}=>{if !self.opening.crews.iter().all(|c|l.permits(c.employer)){return Err(TxError::Unauthorized);}let i=self.opening.edges.iter().position(|e|e.edge==edge).ok_or(TxError::InvalidOperation)?;self.edge_open[i]=open;Ok(())},
            Command::Cancel{delivery}=>{let task=*self.domain.task(delivery).ok_or(TxError::InvalidOperation)?;let crew=self.opening.crew(task.crew_id)?;if !l.permits(crew.employer){return Err(TxError::Unauthorized);}self.apply(l,at,delivery,d::Action::Cancel,d::Verified::Cancellation)?;Ok(())},
            Command::BeginReturn{delivery}=>{let task=*self.domain.task(delivery).ok_or(TxError::InvalidOperation)?;let crew=self.opening.crew(task.crew_id)?;if !l.permits(crew.employer)||task.phase!=d::Phase::OnSite{return Err(TxError::InvalidOperation);}self.host(l,at,delivery,d::Action::BeginReturn,task.position,task.position)?;let i=self.trips.iter().position(|t|t.delivery==delivery).ok_or(TxError::InvalidOperation)?;self.trips[i].motion=Motion::Returning;self.trips[i].path_index=0;Ok(())},
        }
    }
    fn accept(&mut self,l:&mut impl Ledger,at:u64,household:u32,crew:u32)->Result<(),TxError>{
        if self.closed_to_new{return Err(TxError::InvalidOperation);}let route=self.opening.route(household)?;let binding=self.opening.crew(crew)?;
        let group=*self.opening.domain.crews.iter().find(|c|c.id==crew).ok_or(TxError::InvalidOperation)?;
        if !l.permits(binding.employer)||!l.qualified(group.members,binding.driver,at)?{return Err(TxError::Unauthorized);}
        let(key,generated)=l.generated_need(household,group.kind)?;let(out_s,out_q)=self.opening.path_cost(&route.out)?;let(back_s,back_q)=self.opening.path_cost(&route.back)?;let recipe=d::recipe(group.kind);
        let duration=add(add(add(add(d::PREPARATION_SECONDS,out_s)?,recipe.work_seconds)?,back_s)?,d::RESET_SECONDS)?;
        let end=add(at,duration)?;let legal_end=binding.wages.iter().flatten().map(|w|w.valid_until).min().ok_or(TxError::InvalidOperation)?;
        let guaranteed_end=legal_end.min(add(at,14_400)?).min(add((at/86_400)*86_400,64_800)?);
        if binding.wages.iter().flatten().any(|w|at<w.valid_from)||end>guaranteed_end||duration>8*3600{return Err(TxError::InvalidOperation);}
        let energy=add(add(out_q,back_q)?,recipe.energy_mwh)?;
        if l.stock_available(binding.energy_stock,binding.employer,true)?<energy||l.stock_available(binding.water_stock,binding.employer,false)?<recipe.water_ml{return Err(TxError::Stock);}
        let wage_max=binding.wages.iter().flatten().try_fold(0,|sum,w|add(sum,mul(w.cents_per_hour,8)?))?;
        if l.available(route.fee_payer)?<recipe.cents||l.available(binding.employer)?<wage_max{return Err(TxError::InsufficientFunds);}
        let tokens=l.claim(binding.employer,[group.crew_resource,group.vehicle_resource,group.tools_resource])?;
        let id=self.domain.next_delivery_id();let activity=l.begin_members(id,group.members,at)?;
        l.protect(route.fee_payer,recipe.cents)?;l.protect(binding.employer,wage_max)?;
        let grant=Grant{source:d::Source{scope:l.scope(),city_id:self.opening.domain.city_id,clock:d::Clock::Development,root:l.root(),revision:l.revision(),at,operation_id:l.operation()},verified:d::Verified::Admission(d::Admission{assignment:d::Assignment{lease_tokens:tokens,member_activity:activity,origin_place:u64::from(binding.station_entrance),target_place:u64::from(route.target_entrance),route_version:self.opening.graph.revision,protected_until:guaranteed_end,funding_source:l.root(),inputs_source:l.root(),return_source:l.root()},protected_cents:recipe.cents,contract_end:guaranteed_end,need_generated_operation:generated,qualification_root:l.root()})};
        let c=d::Command{delivery_id:0,need:key,expected_revision:None,action:d::Action::Accept{crew_id:crew}};
        let receipt=match d::prepare(&self.domain,&grant,c).map_err(bad)?{d::Outcome::Ready(c)=>{let(next,receipt)=c.into_parts(&self.domain).map_err(bad)?;self.domain=next;receipt},_=>return Err(TxError::InvalidOperation)};
        if receipt.effect.delivery_id!=id{return Err(TxError::InvalidOperation);}l.link_need(key,id)?;
        let mut trip=Trip::inactive();trip.delivery=id;trip.crew=crew;trip.household=household;trip.motion=Motion::Preparing;trip.route_revision=self.opening.graph.revision;trip.cursor=at;trip.need_generated_operation=generated;trip.fee_protected=recipe.cents;trip.wage_protected=wage_max;trip.stock_energy_reserved=energy;trip.stock_water_reserved=recipe.water_ml;trip.last_operation=l.operation();self.trips.try_push(trip).map_err(bad)?;Ok(())
    }
}
impl Runtime {
    fn pay_fee(&mut self,l:&mut impl Ledger,index:usize,receipt:d::Receipt)->Result<(),TxError>{
        let cents=receipt.effect.delta_cents;if cents==0{return Ok(());}let t=&mut self.trips[index];let r=self.opening.route(t.household)?;
        if cents>t.fee_protected{return Err(TxError::Protection);}l.pay_protected(r.fee_payer,r.fee_payee,cents)?;t.fee_paid=add(t.fee_paid,cents)?;t.fee_protected-=cents;Ok(())
    }
    fn duty(&mut self,l:&mut impl Ledger,index:usize,until:u64)->Result<(),TxError>{
        let t=&mut self.trips[index];if until<t.cursor{return Err(TxError::InvalidOperation);}let c=self.opening.crew(t.crew)?;let dt=until-t.cursor;
        for (i,w) in c.wages.iter().enumerate(){if let Some(w)=w{
            let scaled=u128::from(dt).checked_mul(u128::from(w.cents_per_hour)).and_then(|v|v.checked_add(u128::from(t.wage_carry[i]))).ok_or(TxError::Overflow)?;
            let earned=u64::try_from(scaled/3600).map_err(bad)?;t.wage_carry[i]=u64::try_from(scaled%3600).map_err(bad)?;
            t.wages_due[i]=add(t.wages_due[i],earned)?;
            let paid=(t.wages_due[i]-t.wages_paid[i]).min(t.wage_protected);
            if paid>0{l.pay_protected(c.employer,w.account,paid)?;t.wages_paid[i]=add(t.wages_paid[i],paid)?;t.wage_protected-=paid;}
        }}
        let task=self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;
        l.settle_members(t.delivery,task.assignment.member_activity,until,false)?;
        t.cursor=until;t.last_operation=l.operation();Ok(())
    }
    pub(crate) fn required_protection(&self,account:u32)->Result<u64,TxError>{
        self.trips.iter().filter(|t|t.motion!=Motion::Finished).try_fold(0,|sum,t|{
            let r=self.opening.route(t.household)?;let c=self.opening.crew(t.crew)?;
            add(add(sum,if r.fee_payer==account{t.fee_protected}else{0})?,if c.employer==account{t.wage_protected}else{0})
        })
    }
    pub(crate) fn reserved_stock(&self,stock:u32)->Result<u64,TxError>{
        self.trips.iter().filter(|t|t.motion!=Motion::Finished).try_fold(0,|sum,t|{let c=self.opening.crew(t.crew)?;
            add(add(sum,if c.energy_stock==stock{t.stock_energy_reserved}else{0})?,if c.water_stock==stock{t.stock_water_reserved}else{0})})
    }
    /// A boundary, never a promise of arrival. Blocked physical work needs an original
    /// input/resource change; callers may query can-enter on original pool state to wake it.
    pub(crate) fn next_due(&self,now:u64)->Result<Option<u64>,TxError>{
        let mut next=self.domain.next_due().map_err(bad)?.filter(|t|*t>now);
        if self.wake{next=Some(add(now,1)?);}
        for t in self.trips.iter(){
            if t.blocked{continue;}
            let task=self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;
            let at=match t.motion{
                Motion::Preparing=>Some(add(task.accepted_at,d::PREPARATION_SECONDS)?),
                Motion::Outbound|Motion::Returning=>if let Some(id)=t.current_edge{let(e,_)=self.opening.edge(id)?;Some(add(now,u64::from(e.free_flow_seconds).saturating_sub(t.edge_seconds))?)}else{None},
                Motion::OnSite=>Some(add(now,d::recipe(task.need.kind).work_seconds.saturating_sub(task.work_seconds))?.min(task.need.deadline().map_err(bad)?)),
                Motion::Reset=>task.returned_at.map(|at|add(at,d::RESET_SECONDS)).transpose()?,Motion::Finished=>None,
            };
            if let Some(at)=at.filter(|at|*at>now){next=Some(next.map_or(at,|v|v.min(at)));}
        }Ok(next)
    }
    /// Called only on transaction-local copies by the original Timeline P5 writer.
    /// All substeps use actual original stock/member/lease mutations in the SAME Prepared.
    pub(crate) fn advance(&mut self,l:&mut impl Ledger,until:u64)->Result<(),TxError>{
        if !self.is_ready()||until<self.covered_through{return Err(TxError::InvalidOperation);}
        self.wake=false;
        for index in 0..self.trips.len(){
            let mut steps=0;
            while self.trips[index].cursor<=until && self.trips[index].motion!=Motion::Finished{
                steps+=1;if steps>MAX_PATH*2+16{return Err(TxError::WorkingLimit);}
                let t=self.trips[index];let task=*self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;
                let c=self.opening.crew(t.crew)?;let route=self.opening.route(t.household)?;
                let group=*self.opening.domain.crews.iter().find(|c|c.id==t.crew).ok_or(TxError::InvalidOperation)?;
                match t.motion{
                    Motion::Preparing=>{
                        if task.cancelled{self.finish(l,index,t.cursor)?;if self.trips[index].motion!=Motion::Finished{self.trips[index].blocked=true;self.duty(l,index,until)?;break;}continue;}
                        let end=add(task.accepted_at,d::PREPARATION_SECONDS)?.max(t.cursor);self.duty(l,index,until.min(end))?;
                        if until>=end{
                            if !l.qualified(group.members,c.driver,end)?{self.trips[index].blocked=true;self.duty(l,index,until)?;break;}
                            let receipt=self.host(l,end,t.delivery,d::Action::Depart,task.position,task.position)?;self.pay_fee(l,index,receipt)?;
                            self.trips[index].motion=Motion::Outbound;self.trips[index].path_index=0;
                        }else{break;}
                    }
                    Motion::Outbound|Motion::Returning=>{
                        let path=if t.motion==Motion::Outbound{route.out}else{route.back};
                        let Some(id)=path.get(usize::from(t.path_index)).copied().flatten()else{
                            if let Some(token)=t.edge_token{l.exit_edge(c.employer,token)?;}
                            self.trips[index].current_edge=None;self.trips[index].edge_token=None;self.trips[index].edge_seconds=0;
                            let (action,to,motion)=if t.motion==Motion::Outbound{(d::Action::Arrive,task.assignment.target_place,Motion::OnSite)}else{(d::Action::Returned,task.assignment.origin_place,Motion::Reset)};
                            self.host(l,t.cursor,t.delivery,action,task.position,to)?;self.trips[index].motion=motion;continue;
                        };
                        let(edge,b)=self.opening.edge(id)?;let edge_index=self.opening.edges.iter().position(|e|e.edge==id).ok_or(TxError::InvalidOperation)?;
                        if !self.edge_open[edge_index]||!l.qualified(group.members,c.driver,t.cursor)?{self.trips[index].blocked=true;self.duty(l,index,until)?;break;}
                        if t.current_edge!=Some(id){
                            let Some(token)=l.enter_edge(c.employer,b.resource,t.edge_token)?else{self.trips[index].blocked=true;self.duty(l,index,until)?;break;};
                            self.trips[index].current_edge=Some(id);self.trips[index].edge_token=Some(token);self.trips[index].edge_seconds=0;
                        }
                        let t=self.trips[index];let remaining=u64::from(edge.free_flow_seconds).checked_sub(t.edge_seconds).ok_or(TxError::InvalidOperation)?;
                        let available=l.stock_available(c.energy_stock,c.employer,true)?;
                        let dt=(until-t.cursor).min(remaining).min(available/b.energy_mwh_per_second);
                        if dt==0{if until>t.cursor{self.trips[index].blocked=true;self.duty(l,index,until)?;}break;}
                        self.trips[index].blocked=false;let energy=mul(dt,b.energy_mwh_per_second)?;let basis=l.consume(c.energy_stock,c.employer,energy,true)?;self.trips[index].energy_basis_cents=add(self.trips[index].energy_basis_cents,basis)?;
                        self.trips[index].stock_energy_reserved=self.trips[index].stock_energy_reserved.checked_sub(energy).ok_or(TxError::Protection)?;
                        self.trips[index].drive_energy_mwh=add(t.drive_energy_mwh,energy)?;self.trips[index].edge_seconds=add(t.edge_seconds,dt)?;
                        self.duty(l,index,add(t.cursor,dt)?)?;
                        if dt==remaining{
                            // Retain this real edge lease until the next edge can be entered or
                            // the destination handover commits. No invisible queue at a node.
                            self.trips[index].path_index=t.path_index.checked_add(1).ok_or(TxError::Overflow)?;
                        }
                    }
                    Motion::OnSite=>{
                        if task.cancelled||task.work_seconds==d::recipe(task.need.kind).work_seconds||t.cursor>=task.need.deadline().map_err(bad)?{
                            self.host(l,t.cursor,t.delivery,d::Action::BeginReturn,task.position,task.position)?;self.trips[index].motion=Motion::Returning;self.trips[index].path_index=0;continue;
                        }
                        let through=until.min(task.need.deadline().map_err(bad)?).min(add(t.cursor,d::recipe(task.need.kind).work_seconds-task.work_seconds)?);
                        let mut dt=through-t.cursor;
                        let available_e=l.stock_available(c.energy_stock,c.employer,true)?;let available_w=l.stock_available(c.water_stock,c.employer,false)?;
                        let mut low=0;let mut high=dt;
                        while low<high{let mid=low+(high-low+1)/2;let mut ec=task.energy_carry;let mut wc=task.water_carry;
                            if ec.accrue(mid).map_err(bad)?<=available_e&&wc.accrue(mid).map_err(bad)?<=available_w{low=mid}else{high=mid-1}}
                        dt=low;
                        if !l.qualified(group.members,c.driver,t.cursor)?{self.trips[index].blocked=true;self.duty(l,index,until)?;break;}
                        if dt==0{if until>t.cursor{self.trips[index].blocked=true;self.duty(l,index,until)?;}break;}
                        self.trips[index].blocked=false;let through=add(t.cursor,dt)?;let mut ec=task.energy_carry;let mut wc=task.water_carry;let energy=ec.accrue(dt).map_err(bad)?;let water=wc.accrue(dt).map_err(bad)?;
                        let basis=l.consume(c.energy_stock,c.employer,energy,true)?;self.trips[index].energy_basis_cents=add(self.trips[index].energy_basis_cents,basis)?;let basis=l.consume(c.water_stock,c.employer,water,false)?;self.trips[index].water_basis_cents=add(self.trips[index].water_basis_cents,basis)?;
                        let prefix=d::WorkPrefix{from:task.work_cursor,through,work_seconds:dt,member_activity:task.assignment.member_activity,qualification_root:l.root(),electric_source:l.root(),water_source:l.root(),energy_mwh:energy,water_ml:water};
                        let receipt=self.apply(l,through,t.delivery,d::Action::Work,d::Verified::Work(prefix))?;self.pay_fee(l,index,receipt)?;
                        self.trips[index].stock_energy_reserved=self.trips[index].stock_energy_reserved.checked_sub(energy).ok_or(TxError::Protection)?;
                        self.trips[index].stock_water_reserved=self.trips[index].stock_water_reserved.checked_sub(water).ok_or(TxError::Protection)?;
                        self.duty(l,index,through)?;
                    }
                    Motion::Reset=>{
                        let end=add(task.returned_at.ok_or(TxError::InvalidOperation)?,d::RESET_SECONDS)?;let through=until.min(end.max(t.cursor));self.duty(l,index,through)?;
                        if through>=end{
                            if task.reset_at.is_none(){let receipt=self.host(l,through,t.delivery,d::Action::ResetComplete,task.position,task.position)?;self.pay_fee(l,index,receipt)?;}
                            self.finish(l,index,through)?;
                            if self.trips[index].motion!=Motion::Finished{self.trips[index].blocked=true;self.duty(l,index,until)?;break;}
                        }else{break;}
                    }
                    Motion::Finished=>break,
                }
            }
        }
        self.covered_through=until;
        let ids:Vec<_>=self.domain.tasks().iter().filter(|t|t.deadline.is_none()&&t.need.deadline().is_ok_and(|d|d<=until)&&t.unsettled_onsite_through().is_none()).map(|t|t.delivery_id).collect();
        for id in ids{self.apply(l,until,id,d::Action::SealDeadline,d::Verified::Deadline{covered_through:until})?;}
        self.validate()?;Ok(())
    }
    fn finish(&mut self,l:&mut impl Ledger,index:usize,at:u64)->Result<(),TxError>{
        let t=self.trips[index];let task=*self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;let c=self.opening.crew(t.crew)?;
        if task.unsettled_onsite_through().is_some()||t.wages_paid!=t.wages_due{return Ok(());}
        let route=self.opening.route(t.household)?;
        l.release(c.employer,task.assignment.lease_tokens)?;l.settle_members(t.delivery,task.assignment.member_activity,at,true)?;
        l.release_money(c.employer,t.wage_protected)?;l.release_money(route.fee_payer,t.fee_protected)?;
        self.apply(l,at,t.delivery,d::Action::Close,d::Verified::Closure)?;
        let t=&mut self.trips[index];t.motion=Motion::Finished;t.wage_protected=0;t.fee_protected=0;t.stock_energy_reserved=0;t.stock_water_reserved=0;Ok(())
    }
}

pub(crate) fn service_member_owner(delivery:u32)->u64{0x5300_0000_0000_0000|u64::from(delivery)}
/// Genesis registration uses original identities; runtime activity still needs its actual
/// original crew qualification, membership, stock and lease gates at each transition.
pub(crate) fn validate_opening(world:&crate::assembly::World,input:&Opening)->Result<(),TxError>{
    input.validate()?;
    let housing=&world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening;
    if world.region.is_some()||input.domain.scope!=world.scope||housing.city_id!=Some(input.domain.city_id)
        ||input.domain.population_origin!=crate::member_qualification::housing_population_origin(world.scope,housing)
        ||world.member_activity.is_none(){return Err(TxError::InvalidOperation);}
    for r in input.routes.iter(){let household=housing.household(r.household).map_err(bad)?;
        if household.account!=r.fee_payer{return Err(TxError::InvalidOperation);}world.account_handle(r.fee_payee).map_err(bad)?;
    }
    for c in input.crews.iter(){
        for (id,electric) in [(c.energy_stock,true),(c.water_stock,false)]{
            let h=world.stock_handle(id).map_err(bad)?;let s=world.stocks.get(h).map_err(bad)?;let owner=world.accounts.get(s.owner).map_err(bad)?;
            if owner.id!=c.employer||s.commodity!=if electric{crate::assembly::Commodity::Electricity}else{crate::assembly::Commodity::Water}{return Err(TxError::Stock);}
        }
        for w in c.wages.iter().flatten(){world.account_handle(w.account).map_err(bad)?;}
    }Ok(())
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
pub struct View {pub version:u32,pub closed_to_new:bool,pub covered_through:u64,pub trips:Vec<Trip>,pub tasks:Vec<d::Task>}
impl Runtime {pub(crate) fn view(&self)->View{View{version:VERSION,closed_to_new:self.closed_to_new,covered_through:self.covered_through,trips:self.trips.to_vec(),tasks:self.domain.tasks().to_vec()}}}
impl Runtime {
    /// Predict only the next mandatory split from actual original input quantities.
    /// This quote performs no movement, arrival, consumption or capacity grant.
    pub(crate) fn input_boundary(&self,l:&mut impl Ledger,now:u64)->Result<Option<u64>,TxError>{
        let mut next=self.next_due(now)?;
        for t in self.trips.iter().filter(|t|!t.blocked&&t.motion!=Motion::Finished){let task=self.domain.task(t.delivery).ok_or(TxError::InvalidOperation)?;let c=self.opening.crew(t.crew)?;
            if let Some(end)=c.wages.iter().flatten().map(|w|w.valid_until).min().filter(|v|*v>now){next=Some(next.map_or(end,|v|v.min(end)));}
            let duration=match t.motion{
                Motion::Outbound|Motion::Returning=>if let Some(id)=t.current_edge{let(e,b)=self.opening.edge(id)?;let remaining=u64::from(e.free_flow_seconds).saturating_sub(t.edge_seconds);let q=l.stock_available(c.energy_stock,c.employer,true)?;Some(remaining.min(q/b.energy_mwh_per_second))}else{None},
                Motion::OnSite=>{let mut low=0;let mut high=d::recipe(task.need.kind).work_seconds.saturating_sub(task.work_seconds);let e=l.stock_available(c.energy_stock,c.employer,true)?;let w=l.stock_available(c.water_stock,c.employer,false)?;
                    while low<high{let m=low+(high-low+1)/2;let mut ec=task.energy_carry;let mut wc=task.water_carry;if ec.accrue(m).map_err(bad)?<=e&&wc.accrue(m).map_err(bad)?<=w{low=m}else{high=m-1}}Some(low)},_=>None};
            if let Some(dt)=duration.filter(|v|*v>0){let at=add(now,dt)?;next=Some(next.map_or(at,|v|v.min(at)));}
        }Ok(next)
    }
}
