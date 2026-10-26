//! One explicit original-writer tow episode. No second clock, pool or actor ledger.
use super::*;
use crate::transaction::{Lease,Resource};
const RECOVERY_OWNER:u64=0x5400_0000_0000_0001;
const MAX_RECOVERY_PLANS:usize=8;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum RecoveryTarget{Edge{edge:u32},Node{node:u32,pickup_resource:u32}}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPlan{pub id:u32,pub target:RecoveryTarget,pub approach:[Option<u32>;MAX_PATH],pub return_after:[Option<u32>;MAX_PATH]}
impl Record for RecoveryPlan{fn inactive()->Self{Self{id:0,target:RecoveryTarget::Edge{edge:0},approach:[None;MAX_PATH],return_after:[None;MAX_PATH]}}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryOpening{
 pub station_entrance:u32,pub employer:u32,pub wage:Wage,pub resources:[u32;3],pub energy_stock:u32,
 pub energy_mwh_per_second:u64,pub tow_energy_multiplier:u64,pub coupling_seconds:u64,pub fee_cents:u64,
 pub plans:Rows<RecoveryPlan,MAX_RECOVERY_PLANS>,
}
impl RecoveryOpening{
 pub(super) fn owned_bytes(&self)->usize{std::mem::size_of::<Self>()+MAX_RECOVERY_PLANS*std::mem::size_of::<RecoveryPlan>()}
 pub(super) fn validate(&self,o:&Opening)->Result<(),TxError>{
  let station=o.graph.entrances.iter().find(|e|e.id==self.station_entrance).ok_or(TxError::InvalidOperation)?;
  if o.crews.len()!=1||station.owner!=self.employer||!station.authorized||station.mode_mask&DRIVE_MODE==0||self.employer==0||self.wage.account==0||self.wage.account==self.employer||self.wage.valid_from!=0||self.wage.valid_until==0||self.wage.cents_per_hour==0||self.energy_stock==0||self.energy_mwh_per_second==0||self.tow_energy_multiplier!=2||self.coupling_seconds==0||self.fee_cents==0||self.plans.is_empty(){return Err(TxError::InvalidOperation);}
  let primary=o.domain.crews[0];if primary.operator!=self.employer||self.wage.member>=primary.members.start&&self.wage.member<primary.members.end||o.crews[0].station_entrance!=self.station_entrance{return Err(TxError::InvalidOperation);}
  for(i,id)in self.resources.iter().enumerate(){if *id==0||self.resources[..i].contains(id)||[primary.crew_resource,primary.vehicle_resource,primary.tools_resource].contains(id)||o.edges.iter().any(|e|e.resource==*id){return Err(TxError::InvalidOperation);}}
  for(i,p)in self.plans.iter().enumerate(){if p.id==0||self.plans[..i].iter().any(|old|old.id==p.id){return Err(TxError::InvalidOperation);}
   let(node,after)=match p.target{RecoveryTarget::Edge{edge}=>{let(e,_)=o.edge(edge)?;(e.from,e.to)},RecoveryTarget::Node{node,pickup_resource}=>{if pickup_resource==0||self.resources.contains(&pickup_resource)||o.edges.iter().any(|e|e.resource==pickup_resource)||[primary.crew_resource,primary.vehicle_resource,primary.tools_resource].contains(&pickup_resource){return Err(TxError::InvalidOperation);}(node,node)}};
   path_end(o,&p.approach,station.node,node)?;path_end(o,&p.return_after,after,station.node)?;
  }Ok(())
 }
 fn plan(&self,id:u32)->Result<RecoveryPlan,TxError>{self.plans.iter().find(|p|p.id==id).copied().ok_or(TxError::InvalidOperation)}
}
fn path_end(o:&Opening,path:&[Option<u32>;MAX_PATH],mut node:u32,target:u32)->Result<(),TxError>{let mut ended=false;for id in path{if let Some(id)=id{if ended{return Err(TxError::InvalidOperation);}let(e,_)=o.edge(*id)?;if e.from!=node||!e.open||e.mode_mask&DRIVE_MODE==0{return Err(TxError::InvalidOperation);}node=e.to;}else{ended=true;}}if node!=target{return Err(TxError::InvalidOperation);}Ok(())}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum RecoveryPhase{Standby,Approach,Coupling,Towing,Reset,Settlement,Finished}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryState{
 pub phase:RecoveryPhase,pub cursor:u64,pub started_at:u64,pub member_activity:u64,pub resources:[u64;3],
 pub delivery:Option<u32>,pub need:Option<d::NeedKey>,pub need_operation:u64,pub accepted_operation:u64,pub plan:Option<u32>,pub target_offset:u64,pub route_revision:u64,
 pub current_edge:Option<u32>,pub edge_seconds:u64,pub edge_token:Option<u64>,pub pickup_primary_token:Option<u64>,
 pub approach_index:u8,pub return_index:u8,pub tow_prefix_done:bool,pub coupling_work:u64,pub blocked:bool,
 pub energy_reserved:u64,pub energy_mwh:u64,pub energy_basis_cents:u64,pub fee_payer:u32,pub fee_protected:u64,pub fee_paid:u64,
 pub wages_due:u64,pub wages_paid:u64,pub wage_protected:u64,pub wage_carry:u64,
 pub arrived_at:Option<u64>,pub coupled_at:Option<u64>,pub returned_at:Option<u64>,pub physical_end:Option<u64>,
 pub origins:[u64;8],
}
impl RecoveryState{
 pub(super) fn physical(&self)->bool{!matches!(self.phase,RecoveryPhase::Settlement|RecoveryPhase::Finished)}
 pub(super) fn holds(&self,delivery:u32)->bool{self.delivery==Some(delivery)&&matches!(self.phase,RecoveryPhase::Approach|RecoveryPhase::Coupling|RecoveryPhase::Towing)}
}
impl Runtime{
 fn recovery_opening(&self)->Result<&RecoveryOpening,TxError>{self.opening.recovery.as_deref().ok_or(TxError::InvalidOperation)}
 pub(super) fn activate_recovery(&mut self,l:&mut impl Ledger,at:u64)->Result<(),TxError>{
  let o=self.recovery_opening()?.clone();if at!=0||self.recovery.is_some()||!l.permits(o.employer)||!l.driver_qualified(o.wage.member,at)?{return Err(TxError::Unauthorized);}
  let protected=mul(o.wage.cents_per_hour,8)?;l.protect(o.employer,protected)?;let tokens=l.claim(o.employer,o.resources)?;
  let activity=l.begin_owned_members(RECOVERY_OWNER,MemberRange{start:o.wage.member,end:add(o.wage.member,1)?},at)?;
  self.recovery=Some(Box::new(RecoveryState{phase:RecoveryPhase::Standby,cursor:at,started_at:at,member_activity:activity,resources:tokens,delivery:None,need:None,need_operation:0,accepted_operation:0,plan:None,target_offset:0,route_revision:self.opening.graph.revision,current_edge:None,edge_seconds:0,edge_token:None,pickup_primary_token:None,approach_index:0,return_index:0,tow_prefix_done:false,coupling_work:0,blocked:false,energy_reserved:0,energy_mwh:0,energy_basis_cents:0,fee_payer:0,fee_protected:0,fee_paid:0,wages_due:0,wages_paid:0,wage_protected:protected,wage_carry:0,arrived_at:None,coupled_at:None,returned_at:None,physical_end:None,origins:[l.operation(),0,0,0,0,0,l.operation(),0]}));Ok(())
 }
 pub(super) fn dispatch_recovery(&mut self,l:&mut impl Ledger,at:u64,delivery:u32,plan:u32)->Result<(),TxError>{
  let o=self.recovery_opening()?.clone();let plan=o.plan(plan)?;let r=self.recovery.as_deref().ok_or(TxError::InvalidOperation)?;
  let t=*self.trips.iter().find(|t|t.delivery==delivery).ok_or(TxError::InvalidOperation)?;let task=*self.domain.task(delivery).ok_or(TxError::InvalidOperation)?;let crew=self.opening.domain.crews.iter().find(|c|c.id==t.crew).ok_or(TxError::InvalidOperation)?;
  if !l.permits(o.employer)||r.phase!=RecoveryPhase::Standby||r.cursor!=at||!l.driver_qualified(o.wage.member,at)?||!matches!(task.phase,d::Phase::Dispatched|d::Phase::OnSite|d::Phase::WorkComplete|d::Phase::Returning){return Err(TxError::InvalidOperation);}
  // The original operator may also order a real voluntary tow; the primary
  // stays frozen from this committed request until the actual recovery completes.
  let station=self.opening.graph.entrances.iter().find(|e|e.id==o.station_entrance).ok_or(TxError::InvalidOperation)?.node;
  let(offset,pickup)=match plan.target{
   RecoveryTarget::Edge{edge}=>{let(e,_)=self.opening.edge(edge)?;if t.current_edge!=Some(edge)||t.edge_seconds>u64::from(e.free_flow_seconds)||t.edge_token.is_none()||(e.to==station&&t.edge_seconds==u64::from(e.free_flow_seconds)){return Err(TxError::InvalidOperation);}(t.edge_seconds,None)},
   RecoveryTarget::Node{node,pickup_resource}=>{if t.current_edge.is_some()||node==station||!self.opening.graph.entrances.iter().any(|e|u64::from(e.id)==task.position&&e.node==node){return Err(TxError::InvalidOperation);}let token=l.enter_edge(o.employer,pickup_resource,None)?.ok_or(TxError::Capacity)?;(0,Some(token))},
  };
  let approach=self.opening.path_cost(&plan.approach)?.0;let returning=self.opening.path_cost(&plan.return_after)?.0;
  let remaining=match plan.target{RecoveryTarget::Edge{edge}=>u64::from(self.opening.edge(edge)?.0.free_flow_seconds).checked_sub(offset).ok_or(TxError::InvalidOperation)?,_=>0};
  let energy=mul(add(add(approach,offset)?,mul(add(remaining,returning)?,o.tow_energy_multiplier)?)?,o.energy_mwh_per_second)?;
  if l.stock_available(o.energy_stock,o.employer,true)?<add(self.reserved_stock(o.energy_stock)?,energy)?{return Err(TxError::Stock);}
  let route=self.opening.route(t.household)?;l.protect(route.fee_payer,o.fee_cents)?;
  if !task.cancelled{self.apply(l,at,delivery,d::Action::Cancel,d::Verified::Cancellation)?;}
  let r=self.recovery.as_deref_mut().ok_or(TxError::InvalidOperation)?;r.phase=RecoveryPhase::Approach;r.delivery=Some(delivery);r.need=Some(task.need);r.need_operation=task.origins.need;r.accepted_operation=task.origins.accepted;r.plan=Some(plan.id);r.target_offset=offset;r.pickup_primary_token=pickup;r.energy_reserved=energy;r.fee_payer=route.fee_payer;r.fee_protected=o.fee_cents;r.origins[1]=l.operation();r.origins[6]=l.operation();Ok(())
 }
 fn recovery_pay(&self,l:&mut impl Ledger,r:&mut RecoveryState)->Result<(),TxError>{let o=self.recovery_opening()?;let paid=r.wages_due.checked_sub(r.wages_paid).ok_or(TxError::InvalidOperation)?.min(r.wage_protected);if paid>0{l.pay_protected(o.employer,o.wage.account,paid)?;r.wages_paid=add(r.wages_paid,paid)?;r.wage_protected-=paid;r.origins[7]=l.operation();}Ok(())}
 fn recovery_duty(&self,l:&mut impl Ledger,r:&mut RecoveryState,until:u64)->Result<(),TxError>{if until<r.cursor{return Err(TxError::InvalidOperation);}if r.physical(){let o=self.recovery_opening()?;let scaled=u128::from(until-r.cursor).checked_mul(u128::from(o.wage.cents_per_hour)).and_then(|n|n.checked_add(u128::from(r.wage_carry))).ok_or(TxError::Overflow)?;r.wages_due=add(r.wages_due,u64::try_from(scaled/3600).map_err(bad)?)?;r.wage_carry=(scaled%3600)as u64;l.settle_owned_members(RECOVERY_OWNER,r.member_activity,until,false)?;self.recovery_pay(l,r)?;}r.cursor=until;r.origins[6]=l.operation();Ok(())}
 pub(super) fn close_recovery_standby(&mut self,l:&mut impl Ledger,at:u64)->Result<(),TxError>{let mut r=self.recovery.take().ok_or(TxError::InvalidOperation)?;let result=(||{if r.phase!=RecoveryPhase::Standby||!l.permits(self.recovery_opening()?.employer)||self.trips.iter().any(|t|!matches!(t.motion,Motion::Finished|Motion::Settlement)){return Err(TxError::InvalidOperation);}self.recovery_finish(l,&mut r,at)})();self.recovery=Some(r);result}
 fn recovery_finish(&self,l:&mut impl Ledger,r:&mut RecoveryState,at:u64)->Result<(),TxError>{let o=self.recovery_opening()?;if r.physical(){l.release(o.employer,r.resources)?;l.settle_owned_members(RECOVERY_OWNER,r.member_activity,at,true)?;r.physical_end=Some(at);r.energy_reserved=0;r.phase=RecoveryPhase::Settlement;r.origins[5]=l.operation();}if r.wages_paid==r.wages_due{l.release_money(o.employer,r.wage_protected)?;r.wage_protected=0;r.phase=RecoveryPhase::Finished;}Ok(())}
 pub(super) fn fund_recovery_wages(&mut self,l:&mut impl Ledger,at:u64,cents:u64)->Result<(),TxError>{let mut r=self.recovery.take().ok_or(TxError::InvalidOperation)?;let result=(||{let o=self.recovery_opening()?;if cents==0||!l.permits(o.employer)||cents>r.wages_due.checked_sub(r.wages_paid).ok_or(TxError::InvalidOperation)?.saturating_sub(r.wage_protected){return Err(TxError::InvalidOperation);}l.protect(o.employer,cents)?;r.wage_protected=add(r.wage_protected,cents)?;self.recovery_pay(l,&mut r)?;r.origins[6]=l.operation();if r.phase==RecoveryPhase::Settlement{self.recovery_finish(l,&mut r,at)?;}Ok(())})();self.recovery=Some(r);result}
 pub(super) fn advance_recovery(&mut self,l:&mut impl Ledger,until:u64)->Result<(),TxError>{if self.recovery.as_deref().is_none_or(|r|!r.physical()){return Ok(());}let Some(mut r)=self.recovery.take()else{return Ok(());};r.blocked=false;let result=(||{if until<r.cursor{return Err(TxError::InvalidOperation);}let from=r.cursor;self.recovery_motion(l,&mut r,until)?;let motion_cursor=r.cursor;r.cursor=from;self.recovery_duty(l,&mut r,until)?;if r.phase==RecoveryPhase::Reset&&r.returned_at.is_some_and(|at|until>=at.saturating_add(d::RESET_SECONDS)){self.recovery_finish(l,&mut r,until)?;}if motion_cursor>until{return Err(TxError::InvalidOperation);}Ok(())})();self.recovery=Some(r);result}
 fn recovery_motion(&mut self,l:&mut impl Ledger,r:&mut RecoveryState,until:u64)->Result<(),TxError>{
  if !r.physical()||r.phase==RecoveryPhase::Standby{return Ok(());}let o=self.recovery_opening()?.clone();let plan=o.plan(r.plan.ok_or(TxError::InvalidOperation)?)?;
  let index=self.trips.iter().position(|t|Some(t.delivery)==r.delivery).ok_or(TxError::InvalidOperation)?;
  for _ in 0..MAX_PATH*2+8{
   if r.cursor>until{return Err(TxError::InvalidOperation);}if r.blocked{break;}
   match r.phase{
    RecoveryPhase::Approach=>{
     let next=plan.approach.get(usize::from(r.approach_index)).copied().flatten();
     let selected=next.map(|edge|(edge,u64::from(self.opening.edge(edge).map(|v|v.0.free_flow_seconds).unwrap_or(0))));
     let selected=selected.or(match plan.target{RecoveryTarget::Edge{edge}=>Some((edge,r.target_offset)),_=>None});
     if let Some((edge,limit))=selected{
      if !self.recovery_drive(l,r,index,edge,limit,false,until)?{break;}
      if next.is_some(){r.approach_index=r.approach_index.checked_add(1).ok_or(TxError::Overflow)?;continue;}
     }
     if let RecoveryTarget::Node{pickup_resource,..}=plan.target{let Some(token)=l.enter_edge(o.employer,pickup_resource,r.edge_token)?else{r.blocked=true;break;};r.edge_token=Some(token);r.current_edge=None;r.edge_seconds=0;}
     if r.cursor<until{r.cursor=until;}r.arrived_at=Some(r.cursor);r.origins[2]=l.operation();r.phase=RecoveryPhase::Coupling;
    }
    RecoveryPhase::Coupling=>{
     if !l.driver_qualified(o.wage.member,r.cursor)?{r.blocked=true;break;}let dt=(until-r.cursor).min(o.coupling_seconds.checked_sub(r.coupling_work).ok_or(TxError::InvalidOperation)?);r.coupling_work=add(r.coupling_work,dt)?;r.cursor=add(r.cursor,dt)?;
     if r.coupling_work<o.coupling_seconds{break;}if r.cursor<until{return Err(TxError::WorkingLimit);}
     let task=*self.domain.task(r.delivery.ok_or(TxError::InvalidOperation)?).ok_or(TxError::InvalidOperation)?;
     if task.phase!=d::Phase::Returning{self.host(l,r.cursor,task.delivery_id,d::Action::BeginReturn,task.position,task.position)?;}
     if r.fee_protected!=o.fee_cents{return Err(TxError::Protection);}l.pay_protected(r.fee_payer,o.employer,o.fee_cents)?;r.fee_paid=add(r.fee_paid,o.fee_cents)?;r.fee_protected=0;
     r.phase=RecoveryPhase::Towing;r.coupled_at=Some(r.cursor);r.origins[3]=l.operation();r.tow_prefix_done=matches!(plan.target,RecoveryTarget::Node{..});self.trips[index].motion=Motion::Returning;
    }
    RecoveryPhase::Towing=>{
     if !r.tow_prefix_done{let RecoveryTarget::Edge{edge}=plan.target else{return Err(TxError::InvalidOperation);};let limit=u64::from(self.opening.edge(edge)?.0.free_flow_seconds);if !self.recovery_drive(l,r,index,edge,limit,true,until)?{break;}r.tow_prefix_done=true;continue;}
     if let Some(edge)=plan.return_after.get(usize::from(r.return_index)).copied().flatten(){let limit=u64::from(self.opening.edge(edge)?.0.free_flow_seconds);if !self.recovery_drive(l,r,index,edge,limit,true,until)?{break;}r.return_index=r.return_index.checked_add(1).ok_or(TxError::Overflow)?;continue;}
     if r.cursor<until{return Err(TxError::WorkingLimit);}if let Some(token)=self.trips[index].edge_token.take().or(r.pickup_primary_token.take()){l.exit_edge(o.employer,token)?;}if let Some(token)=r.edge_token.take(){l.exit_edge(o.employer,token)?;}
     self.trips[index].current_edge=None;self.trips[index].edge_seconds=0;r.current_edge=None;r.edge_seconds=0;
     let task=*self.domain.task(r.delivery.ok_or(TxError::InvalidOperation)?).ok_or(TxError::InvalidOperation)?;self.host(l,r.cursor,task.delivery_id,d::Action::Returned,task.position,task.assignment.origin_place)?;self.trips[index].motion=Motion::Reset;self.trips[index].last_operation=l.operation();r.returned_at=Some(r.cursor);r.origins[4]=l.operation();r.phase=RecoveryPhase::Reset;
    }
    RecoveryPhase::Reset=>{let end=add(r.returned_at.ok_or(TxError::InvalidOperation)?,d::RESET_SECONDS)?;r.cursor=until.min(end);if r.cursor<end{break;}if r.cursor<until{return Err(TxError::WorkingLimit);}break;}
    _=>break,
   }
   if r.cursor==until{break;}
  }Ok(())
 }
 fn recovery_drive(&mut self,l:&mut impl Ledger,r:&mut RecoveryState,index:usize,edge:u32,limit:u64,tow:bool,until:u64)->Result<bool,TxError>{
  let o=self.recovery_opening()?.clone();let(e,b)=self.opening.edge(edge)?;let i=self.opening.edges.iter().position(|b|b.edge==edge).ok_or(TxError::InvalidOperation)?;
  if !self.edge_open[i]||!l.driver_qualified(o.wage.member,r.cursor)?{r.blocked=true;return Ok(false);}
  if r.current_edge!=Some(edge){
   if tow{let old=[self.trips[index].edge_token.or(r.pickup_primary_token),r.edge_token];let Some(tokens)=l.enter_convoy(o.employer,b.resource,old)?else{r.blocked=true;return Ok(false);};self.trips[index].edge_token=Some(tokens[0]);self.trips[index].current_edge=Some(edge);self.trips[index].edge_seconds=0;r.pickup_primary_token=None;r.edge_token=Some(tokens[1]);}
   else{let Some(token)=l.enter_edge(o.employer,b.resource,r.edge_token)?else{r.blocked=true;return Ok(false);};r.edge_token=Some(token);}
   r.current_edge=Some(edge);r.edge_seconds=0;
  }
  if limit>u64::from(e.free_flow_seconds)||r.edge_seconds>limit{return Err(TxError::InvalidOperation);}let remaining=limit-r.edge_seconds;
  if remaining==0{return Ok(true);}let rate=mul(o.energy_mwh_per_second,if tow{o.tow_energy_multiplier}else{1})?;let available=l.stock_available(o.energy_stock,o.employer,true)?;let dt=(until-r.cursor).min(remaining).min(available/rate);
  if dt==0{if until>r.cursor{r.blocked=true;}return Ok(false);}let quantity=mul(dt,rate)?;let basis=l.consume(o.energy_stock,o.employer,quantity,true)?;r.energy_mwh=add(r.energy_mwh,quantity)?;r.energy_basis_cents=add(r.energy_basis_cents,basis)?;r.energy_reserved=r.energy_reserved.checked_sub(quantity).ok_or(TxError::Protection)?;r.edge_seconds=add(r.edge_seconds,dt)?;r.cursor=add(r.cursor,dt)?;
  if tow{self.trips[index].current_edge=Some(edge);self.trips[index].edge_seconds=r.edge_seconds;self.trips[index].last_operation=l.operation();}Ok(dt==remaining)
 }
 pub(super) fn recovery_next_due(&self,now:u64)->Result<Option<u64>,TxError>{let Some(r)=self.recovery.as_deref()else{return Ok(None);};if !r.physical()||r.phase==RecoveryPhase::Standby||r.blocked{return Ok(None);}let o=self.recovery_opening()?;let p=o.plan(r.plan.ok_or(TxError::InvalidOperation)?)?;
  let dt=match r.phase{RecoveryPhase::Approach=>{if let Some(edge)=p.approach.get(usize::from(r.approach_index)).copied().flatten(){let full=u64::from(self.opening.edge(edge)?.0.free_flow_seconds);full.saturating_sub(if r.current_edge==Some(edge){r.edge_seconds}else{0})}else{match p.target{RecoveryTarget::Edge{..}=>r.target_offset.saturating_sub(r.edge_seconds),_=>0}}},RecoveryPhase::Coupling=>o.coupling_seconds.saturating_sub(r.coupling_work),RecoveryPhase::Towing=>{let edge=if !r.tow_prefix_done{match p.target{RecoveryTarget::Edge{edge}=>Some(edge),_=>None}}else{p.return_after.get(usize::from(r.return_index)).copied().flatten()};if let Some(edge)=edge{u64::from(self.opening.edge(edge)?.0.free_flow_seconds).saturating_sub(if r.current_edge==Some(edge){r.edge_seconds}else{0})}else{0}},RecoveryPhase::Reset=>add(r.returned_at.ok_or(TxError::InvalidOperation)?,d::RESET_SECONDS)?.saturating_sub(now),_=>0};Ok(Some(add(now,dt.max(1))?))
 }
 pub(super) fn recovery_protection(&self,account:u32)->Result<u64,TxError>{let(Some(r),Some(o))=(self.recovery.as_deref(),self.opening.recovery.as_deref())else{return Ok(0);};add(if account==o.employer{r.wage_protected}else{0},if account==r.fee_payer{r.fee_protected}else{0})}
 pub(super) fn recovery_reserved(&self,stock:u32)->u64{self.recovery.as_deref().zip(self.opening.recovery.as_deref()).map_or(0,|(r,o)|if stock==o.energy_stock{r.energy_reserved}else{0})}
 pub(super) fn owns_recovery_member(&self,id:u64,owner:u64)->bool{owner==RECOVERY_OWNER&&self.recovery.as_deref().is_some_and(|r|r.physical()&&r.member_activity==id)}
 pub(super) fn validate_recovery_member(&self,l:&crate::member_activity::Ledger)->Result<(),TxError>{if let Some(r)=self.recovery.as_deref(){let o=self.recovery_opening()?;let found=l.active().iter().find(|a|a.id==r.member_activity);if r.physical(){if !found.is_some_and(|a|a.owner==RECOVERY_OWNER&&a.kind==crate::member_activity::Kind::ServiceWork&&a.members==[Some(crate::member_activity::Members{start:o.wage.member,end:o.wage.member+1}),None]){return Err(TxError::InvalidOperation);}}else if found.is_some(){return Err(TxError::InvalidOperation);}}Ok(())}
 pub(super) fn validate_recovery(&self)->Result<(),TxError>{
  if self.opening.recovery.is_some()&&self.recovery.is_none()&&!self.trips.is_empty(){return Err(TxError::InvalidOperation);}
  if let Some(r)=self.recovery.as_deref(){let o=self.recovery_opening()?;
   if r.cursor>self.covered_through||r.started_at!=0||r.wages_paid>r.wages_due||r.wage_carry>=3600||r.route_revision!=self.opening.graph.revision||r.origins[0]==0||r.member_activity==0||r.coupling_work>o.coupling_seconds||r.resources.contains(&0)||r.resources[0]==r.resources[1]||r.resources[0]==r.resources[2]||r.resources[1]==r.resources[2]{return Err(TxError::InvalidOperation);}
   if let Some(id)=r.delivery{let need=r.need.ok_or(TxError::InvalidOperation)?;if need.scope!=self.opening.domain.scope||need.city_id!=self.opening.domain.city_id||r.need_operation==0||r.accepted_operation==0{return Err(TxError::InvalidOperation);}if let Some(task)=self.domain.task(id){if task.need!=need{return Err(TxError::InvalidOperation);}}else if r.physical()||self.domain.history().reclaimed==0||self.domain.next_delivery_id()<=id{return Err(TxError::InvalidOperation);}o.plan(r.plan.ok_or(TxError::InvalidOperation)?)?;}else if r.need.is_some()||r.need_operation!=0||r.accepted_operation!=0||r.plan.is_some()||r.arrived_at.is_some()||r.coupled_at.is_some()||r.returned_at.is_some()||!matches!(r.phase,RecoveryPhase::Standby|RecoveryPhase::Settlement|RecoveryPhase::Finished){return Err(TxError::InvalidOperation);}
   if let Some(edge)=r.current_edge{if r.edge_token.is_none()||r.edge_seconds>u64::from(self.opening.edge(edge)?.0.free_flow_seconds){return Err(TxError::InvalidOperation);}}
   if r.physical(){if r.physical_end.is_some(){return Err(TxError::InvalidOperation);}}
   else if r.physical_end.is_none()||r.edge_token.is_some()||r.pickup_primary_token.is_some()||r.energy_reserved!=0||r.fee_protected!=0{return Err(TxError::InvalidOperation);}
   if matches!(r.phase,RecoveryPhase::Coupling|RecoveryPhase::Towing|RecoveryPhase::Reset)&&r.arrived_at.is_none(){return Err(TxError::InvalidOperation);}
   if matches!(r.phase,RecoveryPhase::Towing|RecoveryPhase::Reset)&&r.coupled_at.is_none(){return Err(TxError::InvalidOperation);}
   if r.phase==RecoveryPhase::Reset&&r.returned_at.is_none(){return Err(TxError::InvalidOperation);}
   if r.phase==RecoveryPhase::Finished&&(r.wages_paid!=r.wages_due||r.wage_protected!=0){return Err(TxError::InvalidOperation);}
  }Ok(())
 }
 pub(crate) fn validate_recovery_pool(&self,resources:&[Resource],leases:&[Option<Lease>])->Result<(),TxError>{
  let(Some(r),Some(o))=(self.recovery.as_deref(),self.opening.recovery.as_deref())else{return Ok(());};
  for(id,token)in o.resources.iter().zip(r.resources){let found=leases.iter().flatten().find(|l|l.token==token);if r.physical(){if !found.is_some_and(|l|l.actor==o.employer&&l.resource==*id&&l.units==1&&l.occupied){return Err(TxError::Lease);}}else if found.is_some(){return Err(TxError::Lease);}}
  let pickup=if let Some(id)=r.plan{match o.plan(id)?.target{RecoveryTarget::Node{pickup_resource,..}=>Some(pickup_resource),_=>None}}else{None};
  for(token,expected)in [(r.edge_token,if let Some(edge)=r.current_edge{Some(self.opening.edge(edge)?.1.resource)}else{pickup}),(r.pickup_primary_token,pickup)]{
   if let Some(token)=token{if !leases.iter().flatten().any(|l|l.token==token&&l.actor==o.employer&&Some(l.resource)==expected&&l.occupied&&l.units==1&&resources.iter().any(|res|res.id==l.resource)){return Err(TxError::Lease);}}
  }
  if let Some(id)=r.delivery{if let Some(t)=self.trips.iter().find(|t|t.delivery==id){
   if matches!(r.phase,RecoveryPhase::Approach|RecoveryPhase::Coupling){match o.plan(r.plan.ok_or(TxError::InvalidOperation)?)?.target{
    RecoveryTarget::Edge{edge}=>if t.current_edge!=Some(edge)||t.edge_seconds!=r.target_offset{return Err(TxError::InvalidOperation);},
    RecoveryTarget::Node{..}=>if t.current_edge.is_some()||t.edge_token.is_some(){return Err(TxError::InvalidOperation);},
   }}
   if r.phase==RecoveryPhase::Towing&&r.current_edge.is_some()&&(t.current_edge!=r.current_edge||t.edge_seconds!=r.edge_seconds||t.edge_token==r.edge_token){return Err(TxError::Lease);}
  }}Ok(())
 }
 pub(super) fn recovery_paid_source(&self)->Option<crate::housing_income::PaidWageSource>{self.recovery.as_deref().zip(self.opening.recovery.as_deref()).map(|(r,o)|crate::housing_income::PaidWageSource{domain:crate::housing_income::WageSourceDomain::ServiceRecovery,source:1,account:o.wage.account,paid:r.wages_paid})}
 pub(super) fn recovery_wage_contract(&self,account:u32,start:u64,end:u64,now:u64)->Result<(u16,u16,bool),TxError>{let(Some(r),Some(o))=(self.recovery.as_deref(),self.opening.recovery.as_deref())else{return Ok((0,0,false));};if o.wage.account!=account{return Ok((0,0,false));}let through=r.physical_end.unwrap_or(r.cursor);Ok((u16::from(r.started_at<end&&through>start),u16::from(r.physical()&&o.wage.valid_until>now),r.wages_due>r.wages_paid))}
 pub(crate) fn recovery_receipts(&self)->[Option<u64>;10]{let mut refs=[None;10];if let Some(r)=self.recovery.as_deref(){refs[..8].copy_from_slice(&r.origins.map(|v|(v>0).then_some(v)));refs[8]=(r.need_operation>0).then_some(r.need_operation);refs[9]=(r.accepted_operation>0).then_some(r.accepted_operation);}refs}
 pub(crate) fn recovery_history_need(&self)->Option<d::NeedKey>{self.recovery.as_deref().and_then(|r|r.need)}
}
