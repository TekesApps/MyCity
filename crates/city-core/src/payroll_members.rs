//! Root adapter from explicitly registered job groups to the ONE member ledger.
//! Payroll owns money/rounding; this owns only identity, activity and coverage.
use crate::{arena::Scope, assembly::World, member_activity as m,
    member_qualification::MemberRange, model_rows::{Record, Rows},
    timeline::{self as t, PayrollSpec, Timeline}, transaction::{Action, JournalRecord, TxError}};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};

pub const LIMIT: usize = t::PAYROLL_LIMIT;
pub const MAX_RESTORE_REFS: usize = LIMIT * 3;
pub const MAX_SERIALIZED_BYTES: usize = 64 * 1024;
pub const MAX_OWNED_BYTES: usize = 32 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { LocalDevelopmentActualJobGroupV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source { pub id: u64, pub version: u32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding { pub spec: PayrollSpec, pub members: MemberRange, pub source: Source }
impl Record for Binding {
    fn inactive()->Self { Self { spec: PayrollSpec { id:0,payer:0,payee:0,home_city:0,
        hourly_cents:crate::units::Money::ZERO,cycle:t::Cycle::Daily,ends_at:0 },
        members:MemberRange{start:0,end:0},source:Source{id:0,version:0} } }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version:u32, pub profile:Profile, pub scope:Scope,
    pub population_origin:[u8;32], pub member_version:u32, pub bindings:Rows<Binding,LIMIT>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event { operation:u64, revision:u64, at:u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Activity {
    Unadmitted, Idle, WaitingMember{blocked_by:u64}, Holding{activity:u64,begin:Event},
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    payroll:u32, admitted:Option<Event>, coordinated_at:Option<u64>,
    activity:Activity, last_event:Option<Event>,
}
impl Record for State { fn inactive()->Self{Self{payroll:0,admitted:None,coordinated_at:None,activity:Activity::Unadmitted,last_event:None}} }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    opening:Box<Opening>, states:Rows<State,LIMIT>, #[serde(skip)] ready:bool,
}
pub(crate) enum Step { Done, Progress{member_event:bool}, Yield }
fn invalid(_:impl std::fmt::Debug)->TxError{TxError::InvalidOperation}
fn member_error(e:m::Error)->TxError{match e{m::Error::UnresolvedBudget=>TxError::WorkingLimit,m::Error::Overflow=>TxError::Overflow,_=>TxError::InvalidOperation}}
struct HashWriter { hash:Sha256, bytes:usize }
impl std::io::Write for HashWriter {
    fn write(&mut self,b:&[u8])->std::io::Result<usize>{
        let bytes=self.bytes.checked_add(b.len()).filter(|n|*n<=MAX_SERIALIZED_BYTES).ok_or_else(||std::io::Error::other("payroll_encoding_budget"))?;
        self.hash.update(b);self.bytes=bytes;Ok(b.len())
    }
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
fn encoded_hash(value:&impl Serialize)->Result<[u8;32],TxError>{
    let mut out=HashWriter{hash:Sha256::new(),bytes:0};
    serde_json::to_writer(&mut out,value).map_err(|_|TxError::WorkingLimit)?;Ok(out.hash.finalize().into())
}
fn shift_open(at:u64,p:&t::Payroll)->bool{p.working && at<p.spec.ends_at && (9*3600..17*3600).contains(&(at%t::DAY))}
fn members(b:Binding)->[Option<m::Members>;2]{[Some(m::Members{start:b.members.start,end:b.members.end}),None]}
impl Opening {
    pub(crate) fn runtime_owned_bytes(&self)->usize{std::mem::size_of::<Runtime>()+self.owned_bytes()+LIMIT*std::mem::size_of::<State>()}
    fn owned_bytes(&self)->usize{std::mem::size_of::<Self>()+self.bindings.capacity()*std::mem::size_of::<Binding>()}
    fn validate(&self,world:&World)->Result<(),TxError>{
        let housing=&world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening;
        if self.version!=1||self.member_version!=1||self.scope!=world.scope||world.region.is_some()||world.economy.is_some()
            ||self.population_origin!=crate::member_qualification::housing_population_origin(world.scope,housing)
            ||world.member_activity.is_none()||self.bindings.is_empty(){return Err(TxError::InvalidOperation);}
        for(i,b)in self.bindings.iter().enumerate(){
            t::Payroll::new(b.spec,0)?;
            if b.spec.home_city!=0||b.members.start>=b.members.end||b.source.id==0||b.source.version==0
                ||(i>0&&self.bindings[i-1].spec.id>=b.spec.id){return Err(TxError::InvalidOperation);}
            let payer=world.accounts.get(world.account_handle(b.spec.payer).map_err(invalid)?).map_err(invalid)?;
            if payer.role!=crate::assembly::Role::Company{return Err(TxError::Unauthorized);}
            let mut start=0u64;let mut matched=false;
            for h in &housing.households {let end=start.checked_add(u64::from(h.people)).ok_or(TxError::Overflow)?;
                if b.members.start>=start&&b.members.end<=end {matched=h.account==b.spec.payee;break;}start=end;}
            if !matched{return Err(TxError::Unauthorized);}
        }Ok(())
    }
}
pub(crate) fn initialize(opening:Box<Opening>,world:&World)->Result<Box<Runtime>,String>{
    opening.validate(world).map_err(|e|format!("payroll_members_opening:{e:?}"))?;
    let mut states=Rows::new().map_err(str::to_owned)?;
    for b in opening.bindings.iter(){states.try_push(State{payroll:b.spec.id,..State::inactive()}).map_err(str::to_owned)?;}
    Ok(Box::new(Runtime{opening,states,ready:true}))
}
impl Runtime {
    pub(crate) fn owned_bytes(&self)->usize{std::mem::size_of::<Self>()+self.opening.owned_bytes()+self.states.capacity()*std::mem::size_of::<State>()}
    /// Extra scratch only; original and prepared Runtime boxes are charged separately.
    pub(crate) fn workspace_bytes()->usize{2*m::Ledger::owned_bytes()+std::mem::size_of::<HashWriter>()+std::mem::size_of::<RestoreCheck>()+std::mem::size_of::<State>()}
    pub(crate) fn registration_fingerprint(&self)->[u8;32]{
        encoded_hash(&*self.opening).expect("bounded typed Payroll registration")
    }
    fn fingerprint(&self)->Result<[u8;32],TxError>{encoded_hash(self)}
    fn index(&self,id:u32)->Result<usize,TxError>{self.states.iter().position(|s|s.payroll==id).ok_or(TxError::InvalidOperation)}
    pub(crate) fn registered(&self,spec:&PayrollSpec)->bool{self.opening.bindings.iter().any(|b|b.spec.id==spec.id&&b.spec.payer==spec.payer&&b.spec.payee==spec.payee)}
    pub(crate) fn is_ready(&self)->bool{self.ready}
    /// Coordinate each admitted row, possible yield steps, then finish P5.
    pub(crate) fn step_bound(&self)->usize{2*self.states.iter().filter(|s|s.admitted.is_some()).count()+1}
    /// At most one new Begin fits an original Envelope, regardless of page width.
    /// Already-held claims need only settlement; a still-busy source can reduce
    /// this bound, but cannot require more starts than these immutable groups.
    pub(crate) fn begin_page_breaks(&self,timeline:&Timeline,at:u64)->usize{
        self.states.iter().filter(|s|s.admitted.is_some()&&s.coordinated_at!=Some(at)&&!matches!(s.activity,Activity::Holding{..})
            &&timeline.payroll.iter().any(|p|p.spec.id==s.payroll&&shift_open(at,p))).count().saturating_sub(1)
    }

    pub(crate) fn admit(&mut self,spec:PayrollSpec,op:u64,revision:u64,at:u64)->Result<(),TxError>{
        if !self.ready{return Err(TxError::InvalidOperation);}let i=self.index(spec.id)?;
        if self.opening.bindings[i].spec!=spec||self.states[i].admitted.is_some(){return Err(TxError::InvalidOperation);}
        let e=Event{operation:op,revision,at};self.states[i].admitted=Some(e);self.states[i].activity=Activity::Idle;
        self.states[i].coordinated_at=None;self.states[i].last_event=Some(e);Ok(())
    }
    pub(crate) fn dirty(&mut self,id:u32)->Result<(),TxError>{let i=self.index(id)?;self.states[i].coordinated_at=None;Ok(())}
    pub(crate) fn next_shift_boundary(&self,now:u64)->Result<u64,TxError>{
        let day=now/t::DAY*t::DAY;let at=if now%t::DAY<9*3600{day+9*3600}else if now%t::DAY<17*3600{day+17*3600}else{day.checked_add(t::DAY+9*3600).ok_or(TxError::Overflow)?};Ok(at)
    }
    /// Previous completed-boundary admission is the only permission to accrue.
    pub(crate) fn work_allowed(&self,p:&t::Payroll,from:u64,to:u64,ledger:&m::Ledger)->Result<bool,TxError>{
        if !self.ready{return Err(TxError::InvalidOperation);}let i=self.index(p.spec.id)?;let s=self.states[i];
        if s.admitted.is_none()||s.coordinated_at!=Some(from)||to<from{return Err(TxError::InvalidOperation);}
        match s.activity {
            Activity::Holding{activity,..}=>{
                let a=ledger.active().iter().find(|a|a.id==activity).ok_or(TxError::InvalidOperation)?;
                if a.owner!=u64::from(p.spec.id)||a.kind!=m::Kind::PayrollWork||a.members!=members(self.opening.bindings[i])||a.settled_through!=from
                    ||!shift_open(from,p)||to>from/t::DAY*t::DAY+17*3600||to>p.spec.ends_at{return Err(TxError::InvalidOperation);}Ok(true)
            },
            Activity::Idle|Activity::WaitingMember{..}=>Ok(false),Activity::Unadmitted=>Err(TxError::InvalidOperation),
        }
    }
    /// One row per existing source step. Yield happens BEFORE mutating a new row.
    pub(crate) fn coordinate_one(&mut self,timeline:&Timeline,ledger:&mut m::Ledger,at:u64,operation:u64,revision:u64,begin_used:&mut bool)->Result<Step,TxError>{
        if !self.ready{return Err(TxError::InvalidOperation);}
        let Some(i)=self.states.iter().position(|s|s.admitted.is_some()&&s.coordinated_at!=Some(at))else{return Ok(Step::Done);};
        let b=self.opening.bindings[i];let p=timeline.payroll.iter().find(|p|p.spec.id==b.spec.id).ok_or(TxError::InvalidOperation)?;
        let open=shift_open(at,p);let e=Event{operation,revision,at};let source=|ledger:&m::Ledger|->Result<m::Source,TxError>{Ok(m::Source{scope:self.opening.scope,population_origin:self.opening.population_origin,epoch:crate::member_authority::EPOCH,ordinal:ledger.next_ordinal().map_err(member_error)?,operation_id:operation,occurred_at:at})};
        let mut event=false;
        match self.states[i].activity {
            Activity::Holding{activity,begin}=>{
                let old=*ledger.active().iter().find(|a|a.id==activity).ok_or(TxError::InvalidOperation)?;
                if old.settled_through<at||!open {
                    let c=ledger.prepare_settle(source(ledger)?,at,activity,u64::from(b.spec.id),m::Kind::PayrollWork,!open).map_err(member_error)?;
                    *ledger=c.into_next(ledger).map_err(member_error)?;event=true;
                }
                self.states[i].activity=if open{Activity::Holding{activity,begin}}else{Activity::Idle};
            },
            Activity::Idle|Activity::WaitingMember{..}=>{
                if open {
                    let conflict=ledger.active().iter().find(|a|a.members.iter().flatten().any(|m|m.start<b.members.end&&b.members.start<m.end)).map(|a|a.id);
                    if let Some(blocked_by)=conflict{self.states[i].activity=Activity::WaitingMember{blocked_by};}
                    else {
                        if *begin_used{return Ok(Step::Yield);}
                        let source=source(ledger)?;
                        let c=ledger.prepare_begin(source,at,u64::from(b.spec.id),m::Kind::PayrollWork,members(b)).map_err(member_error)?;
                        *ledger=c.into_next(ledger).map_err(member_error)?;*begin_used=true;event=true;
                        self.states[i].activity=Activity::Holding{activity:source.ordinal,begin:e};
                    }
                }else{self.states[i].activity=Activity::Idle;}
            },Activity::Unadmitted=>return Err(TxError::InvalidOperation),
        }
        self.states[i].coordinated_at=Some(at);self.states[i].last_event=Some(e);Ok(Step::Progress{member_event:event})
    }
    pub(crate) fn validate(&self,world:&World,timeline:Option<&Timeline>,ledger:&m::Ledger,max_revision:u64)->Result<(),TxError>{
        self.opening.validate(world)?;if self.states.len()!=self.opening.bindings.len(){return Err(TxError::InvalidOperation);}
        let at=timeline.map_or(0,|t|if t.progress.phase==Some(crate::schedule::Phase::P0){t.progress.last_complete}else if t.progress.phase.is_some(){t.progress.at}else{t.progress.last_complete});
        let people=world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening.households.iter().map(|h|u64::from(h.people)).sum();
        ledger.validate_binding(world.scope,self.opening.population_origin,people,crate::member_authority::EPOCH,at).map_err(member_error)?;
        for(i,s)in self.states.iter().enumerate(){let b=self.opening.bindings[i];if s.payroll!=b.spec.id{return Err(TxError::InvalidOperation);}
            let p=timeline.and_then(|t|t.payroll.iter().find(|p|p.spec.id==s.payroll));
            if let Some(admit)=s.admitted {
                let p=p.ok_or(TxError::InvalidOperation)?;
                if p.started!=admit.at||p.spec.payer!=b.spec.payer||p.spec.payee!=b.spec.payee||p.spec.home_city!=b.spec.home_city||p.spec.hourly_cents!=b.spec.hourly_cents||p.spec.ends_at>b.spec.ends_at
                    ||matches!(s.activity,Activity::Unadmitted){return Err(TxError::InvalidOperation);}
                for e in [Some(admit),s.last_event]{let e=e.ok_or(TxError::InvalidOperation)?;if e.operation==0||e.revision==0||e.revision>max_revision||e.at>at{return Err(TxError::InvalidOperation);}}
                if s.coordinated_at.is_none_or(|c|c>at){return Err(TxError::InvalidOperation);}
                if let Activity::Holding{activity,begin}=s.activity {
                    let a=ledger.active().iter().find(|a|a.id==activity).ok_or(TxError::InvalidOperation)?;
                    if a.owner!=u64::from(s.payroll)||a.kind!=m::Kind::PayrollWork||a.members!=members(b)||a.started!=begin.at||a.start_operation_id!=begin.operation||Some(a.settled_through)!=s.coordinated_at{return Err(TxError::InvalidOperation);}
                }
            }else if p.is_some()||s.activity!=Activity::Unadmitted||s.coordinated_at.is_some()||s.last_event.is_some(){return Err(TxError::InvalidOperation);}
        }
        if timeline.is_some_and(|t|t.payroll.iter().any(|p|!self.registered(&p.spec))){return Err(TxError::InvalidOperation);}
        for a in ledger.active().iter().filter(|a|a.kind==m::Kind::PayrollWork){if !self.states.iter().any(|s|matches!(s.activity,Activity::Holding{activity,..}if activity==a.id)){return Err(TxError::InvalidOperation);}}
        Ok(())
    }
    pub(crate) fn refs(&self)->[Option<u64>;MAX_RESTORE_REFS]{std::array::from_fn(|n|{let s=self.states.get(n/3)?;match n%3{0=>s.admitted,1=>if let Activity::Holding{begin,..}=s.activity{Some(begin)}else{None},_=>s.last_event}.map(|e|e.operation)})}
}
/// Only original Store receipt lookups can complete this marker.
pub struct RestoreCheck{expected:[u8;32],checked:[bool;MAX_RESTORE_REFS]}
impl Runtime {
    pub(crate) fn begin_restore(&self)->Result<RestoreCheck,TxError>{Ok(RestoreCheck{expected:self.fingerprint()?,checked:[false;MAX_RESTORE_REFS]})}
    pub(crate) fn check_restore(&self,c:&mut RestoreCheck,index:usize,r:&JournalRecord)->Result<(),TxError>{
        if index>=MAX_RESTORE_REFS||c.checked[index]||c.expected!=self.fingerprint()?{return Err(TxError::InvalidOperation);}
        let s=*self.states.get(index/3).ok_or(TxError::InvalidOperation)?;let b=self.opening.bindings[index/3];
        let e=match index%3{0=>s.admitted,1=>if let Activity::Holding{begin,..}=s.activity{Some(begin)}else{None},_=>s.last_event}.ok_or(TxError::InvalidOperation)?;
        let receipt=r.outcome.as_ref().map_err(invalid)?;
        if r.operation_id!=e.operation||r.actor!=0||receipt.revision!=e.revision||r.actions.iter().skip(1).any(Option::is_some){return Err(TxError::InvalidOperation);}
        let Some(Action::Timeline{command})=r.actions[0]else{return Err(TxError::InvalidOperation);};
        let allowed=if index%3==0 {matches!(command,t::Command::AddPayroll{spec}if spec==b.spec)}else{match command{
            t::Command::AddPayroll{spec}=>spec==b.spec,
            t::Command::Advance{target,..}=>target>=e.at,
            t::Command::StopWork{id}|t::Command::Terminate{id}|t::Command::EndForExit{id}|t::Command::SettlePayroll{id}|t::Command::ChangeCycle{id,..}=>id==s.payroll,
            _=>false,
        }};if !allowed{return Err(TxError::InvalidOperation);}c.checked[index]=true;Ok(())
    }
    pub(crate) fn finish_restore(&mut self,c:RestoreCheck)->Result<(),TxError>{if c.expected!=self.fingerprint()?||self.refs().iter().enumerate().any(|(i,r)|r.is_some()&&!c.checked[i]){return Err(TxError::InvalidOperation);}self.ready=true;Ok(())}
}

impl crate::restore_into::Restore for Option<Box<Runtime>>{
    fn restore<'de,D:serde::Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error>{
        *self=None;
        *self=Option::<Box<Runtime>>::deserialize(d)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_group_backing_is_charged_deep_copied_and_cleared_by_legacy_restore() {
        let bytes=include_bytes!("../../../tests/fixtures/facility-work/electric-v2-opening.json");
        let mut input:serde_json::Value=serde_json::from_slice(bytes).unwrap();
        input["payroll_members"]=serde_json::json!({"version":1,"profile":"LocalDevelopmentActualJobGroupV1",
            "scope":input["member_activity"]["scope"],"population_origin":input["member_activity"]["population_origin"],"member_version":1,
            "bindings":[{"spec":{"id":21,"payer":3,"payee":2,"home_city":0,"hourly_cents":10,"cycle":{"kind":"daily"},"ends_at":t::DAY},
                "members":{"start":2,"end":3},"source":{"id":15501,"version":1}}]});
        let base=crate::assembly::assemble(bytes).unwrap();
        let active=crate::assembly::assemble(&serde_json::to_vec(&input).unwrap()).unwrap();
        let runtime=active.payroll_members.as_deref().unwrap();
        assert_eq!(active.owned_capacity_bytes()-base.owned_capacity_bytes(),runtime.owned_bytes());
        assert_eq!(runtime.owned_bytes(),runtime.opening.runtime_owned_bytes());
        assert!(runtime.owned_bytes()<=MAX_OWNED_BYTES);
        let wire=serde_json::to_vec(&active).unwrap();
        let raw:World=serde_json::from_slice(&wire).unwrap();
        assert!(!raw.payroll_members.as_deref().unwrap().ready);
        assert_eq!(serde_json::to_vec(&raw).unwrap(),wire);
        let mut reused=base;
        active.copy_into(&mut reused).unwrap();
        let copied=reused.payroll_members.as_deref().unwrap();
        assert!(!std::ptr::eq(copied,runtime));
        assert!(!std::ptr::eq(&*copied.opening,&*runtime.opening));
        assert_eq!(serde_json::to_vec(&reused).unwrap(),wire);
        let empty=crate::assembly::assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        let mut old:serde_json::Value=serde_json::to_value(&empty).unwrap();
        old["payroll_members"]=serde_json::Value::Null;
        let old=serde_json::to_vec(&old).unwrap();let mut d=serde_json::Deserializer::from_slice(&old);
        crate::restore_into::Restore::restore(&mut reused,&mut d).unwrap();d.end().unwrap();
        assert!(reused.payroll_members.is_none());
        assert_eq!(reused.owned_capacity_bytes(),empty.owned_capacity_bytes());
        assert_eq!(serde_json::to_vec(&reused).unwrap(),serde_json::to_vec(&empty).unwrap());
    }
    #[test]
    fn idle_coverage_does_not_request_a_member_event_ordinal(){
        let world=crate::assembly::assemble(include_bytes!("../../../tests/fixtures/facility-work/electric-v2-opening.json")).unwrap();
        let spec=PayrollSpec{id:21,payer:3,payee:2,home_city:0,hourly_cents:crate::units::Money::new(10).unwrap(),cycle:t::Cycle::Daily,ends_at:t::DAY};
        let mut bindings=Rows::new().unwrap();bindings.try_push(Binding{spec,members:MemberRange{start:2,end:3},source:Source{id:15501,version:1}}).unwrap();
        let opening=Box::new(Opening{version:1,profile:Profile::LocalDevelopmentActualJobGroupV1,scope:world.scope,population_origin:crate::member_qualification::housing_population_origin(world.scope,&world.housing.as_deref().unwrap().opening),member_version:1,bindings});
        let mut r=initialize(opening,&world).unwrap();r.admit(spec,1,1,0).unwrap();
        let mut timeline=Timeline::new(0);timeline.payroll.try_push(t::Payroll::new(spec,0).unwrap()).unwrap();
        let mut raw=serde_json::to_value(world.member_activity.as_deref().unwrap()).unwrap();raw["last_ordinal"]=serde_json::json!(u64::MAX);
        let mut ledger:m::Ledger=serde_json::from_value(raw).unwrap();
        let people=10;ledger.validate_binding(world.scope,r.opening.population_origin,people,crate::member_authority::EPOCH,t::DAY).unwrap();
        let mut begin_used=false;
        let mut ended=r.clone();timeline.payroll[0].working=false;
        assert!(matches!(ended.coordinate_one(&timeline,&mut ledger,t::DAY,2,2,&mut begin_used),Ok(Step::Progress{member_event:false})));
        assert!(matches!(ended.coordinate_one(&timeline,&mut ledger,t::DAY,3,3,&mut begin_used),Ok(Step::Done)));
        assert!(ledger.active().is_empty());
        timeline.payroll[0].working=true;
        assert!(matches!(r.coordinate_one(&timeline,&mut ledger,9*3600,2,2,&mut begin_used),Err(TxError::Overflow)));
        assert_eq!(r.states[0].coordinated_at,None);
    }
    #[test]
    fn bounded_streaming_hash_covers_widest_typed_rows_without_heap_encoding(){
        let spec=PayrollSpec{id:u32::MAX,payer:u32::MAX,payee:u32::MAX,home_city:u32::MAX,hourly_cents:crate::units::Money::new(i64::MAX).unwrap(),cycle:t::Cycle::Monthly{anchor:u8::MAX},ends_at:u64::MAX};
        let mut bindings=Rows::new().unwrap();let mut states=Rows::new().unwrap();
        let e=Event{operation:u64::MAX,revision:u64::MAX,at:u64::MAX};
        for _ in 0..LIMIT{bindings.try_push(Binding{spec,members:MemberRange{start:u64::MAX,end:u64::MAX},source:Source{id:u64::MAX,version:u32::MAX}}).unwrap();states.try_push(State{payroll:u32::MAX,admitted:Some(e),coordinated_at:Some(u64::MAX),activity:Activity::Holding{activity:u64::MAX,begin:e},last_event:Some(e)}).unwrap();}
        // Encoding upper bound deliberately includes values wider than any admitted
        // city permits. It is not a source of game facts or a valid population.
        let r=Runtime{opening:Box::new(Opening{version:u32::MAX,profile:Profile::LocalDevelopmentActualJobGroupV1,scope:Scope::new(u64::MAX,u64::MAX).unwrap(),population_origin:[255;32],member_version:u32::MAX,bindings}),states,ready:false};
        let expected=serde_json::to_vec(&r).unwrap();assert!(expected.len()<MAX_SERIALIZED_BYTES);
        assert_eq!(r.fingerprint().unwrap(),<[u8;32]>::from(Sha256::digest(&expected)));
        let mut writer=HashWriter{hash:Sha256::new(),bytes:MAX_SERIALIZED_BYTES};
        assert!(std::io::Write::write(&mut writer,b"x").is_err());assert_eq!(writer.bytes,MAX_SERIALIZED_BYTES);
    }

}
