//! Original root adapter for facility work. No externally supplied work/paid flag.
use crate::{assembly::World, facility_work as w, member_activity as m,
    qualification_authority as q, timeline::Timeline, transaction::{Caller,TxError},
    units::Money, venue_capacity::UseStatus};
use serde::{Serialize,Deserialize,Deserializer};
use sha2::{Digest,Sha256};

pub const MAX_OWNED_BYTES:usize=8*1024;
pub const MAX_WORKSPACE_BYTES:usize=64*1024;
pub const AUTHORITY_VERSION:u32=2;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {pub authority_version:u32,pub work:w::Opening}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Command { Begin{contract_id:u64}, Stop{contract_id:u64}, Pay{contract_id:u64,cents:u64} }
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime { version:u32, pub(crate) state:w::Runtime, begin_revisions:[Option<u64>;4], runs:[Option<StartedRun>;4], income_window_anchor:Option<IncomeWindowAnchor>, #[serde(skip)] ready:bool }
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct IncomeWindowAnchor {operation_id:u64,revision:u64,at:u64,hash:[u8;32]}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct StartedRun {contract_id:u64,begin:w::EventRef,begin_revision:u64,run_id:u64,start_operation:u64,start_revision:u64,started_at:u64}
fn root_encoded_bound()->Result<usize,TxError>{
    // Noncausal fixed maximum-width sizing only, never installed as game facts.
    let x=StartedRun{contract_id:u64::MAX,begin:w::EventRef{ordinal:u64::MAX,operation_id:u64::MAX,occurred_at:u64::MAX},begin_revision:u64::MAX,run_id:u64::MAX,start_operation:u64::MAX,start_revision:u64::MAX,started_at:u64::MAX};
    let income=IncomeWindowAnchor{operation_id:u64::MAX,revision:u64::MAX,at:u64::MAX,hash:[255;32]};
    let envelope=serde_json::json!({"version":AUTHORITY_VERSION,"begin_revisions":([Some(u64::MAX);4]),"runs":([Some(x);4]),"income_window_anchor":Some(income)});
    let n=serde_json::to_vec(&envelope).map_err(fail)?.len();n.checked_add(16).ok_or(TxError::Overflow) // adding state key/value; count all root keys together.

}
impl Runtime {
    pub(crate) const fn owned_bytes()->usize{std::mem::size_of::<Self>()}
    pub(crate) fn is_ready(&self)->bool{self.ready}
    pub(crate) fn registration(&self)->Result<[u8;32],TxError>{digest(&("facility-work-authority-v2",self.version,self.state.opening()))}
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
pub struct ContractView {
    pub id:u64,pub device_id:u64,pub members:crate::member_qualification::MemberRange,
    pub phase:&'static str,pub started:u64,pub settled:u64,pub work_seconds:u64,
    pub earned_cents:u64,pub paid_cents:u64,pub wage_remainder:u16,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
pub struct View {pub records:[Option<ContractView>;4],pub last_operation:u64}
pub(crate) fn view(r:&Runtime)->View {
    View{records:std::array::from_fn(|i|r.state.opening().contracts[i].zip(r.state.records()[i]).map(|(c,a)|ContractView{
        id:c.id,device_id:c.device_id,members:c.members,phase:match a.phase{w::Phase::Pending=>"pending",w::Phase::Active=>"active",w::Phase::Stopped=>"stopped"},
        started:a.started,settled:a.settled,work_seconds:a.work_seconds,earned_cents:a.earned,paid_cents:a.paid,wage_remainder:a.remainder})),
        last_operation:r.state.last_source().map_or(0,|s|s.operation_id)}
}
/// Bounded original-transaction result, persisted inside its existing receipt.
/// Digests commit exact interval/payment fields; they are not external grants.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {pub contract_id:u64,pub begin:Option<[u8;32]>,pub stopped:Option<[u8;32]>,pub interval:Option<[u8;32]>,pub payment:Option<[u8;32]>}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {pub contracts:[Option<Effect>;4],#[serde(default,skip_serializing_if="Option::is_none")]pub income_window:Option<[u8;32]>}
pub(crate) fn effects(r:Option<&Runtime>,op:u64)->Result<Option<Effects>,TxError>{
    let Some(r)=r else{return Ok(None);};let mut rows=[None;4];
    for(i,c)in r.state.opening().contracts.iter().enumerate(){if let Some(c)=c{let a=r.state.records()[i].ok_or(TxError::InvalidOperation)?;
        let begin=a.begin.filter(|v|v.operation_id==op).map(|_|digest(&("facility-admission-v2",c.id,a.begin,a.qualification_source,a.started,a.activity_id))).transpose()?;
        let stopped=a.stopped.filter(|v|v.operation_id==op).map(|_|digest(&("facility-stop-v2",c.id,a.stopped,a.settled,a.work_seconds,a.earned,a.remainder))).transpose()?;
        let interval=a.last_interval.filter(|v|v.source.operation_id==op).map(|v|digest(&("facility-actual-interval-v2",c.id,v))).transpose()?;
        let payment=a.last_payment.filter(|v|v.source.operation_id==op).map(|v|digest(&("facility-actual-payment-v2",c.id,v))).transpose()?;
        if begin.is_some()||stopped.is_some()||interval.is_some()||payment.is_some(){rows[i]=Some(Effect{contract_id:c.id,begin,stopped,interval,payment});}
    }}let income_window=r.income_window_anchor.filter(|a|a.operation_id==op).map(|a|a.hash);Ok((income_window.is_some()||rows.iter().any(Option::is_some)).then_some(Effects{contracts:rows,income_window}))
}
#[derive(Clone,Copy)]
pub(crate) struct PaidSlot {contract_id:u64,paid:u64}
pub(crate) type PaidSnapshot=[Option<PaidSlot>;4];
pub(crate) fn paid_snapshot(r:&Runtime)->PaidSnapshot{std::array::from_fn(|i|r.state.records()[i].map(|a|PaidSlot{contract_id:a.contract_id,paid:a.paid}))}
pub(crate) fn paid_sources(r:Option<&Runtime>)->[Option<crate::housing_income::PaidWageSource>;4]{
    r.map_or([None;4],|r|std::array::from_fn(|i|r.state.opening().contracts[i].zip(r.state.records()[i]).map(|(c,a)|crate::housing_income::PaidWageSource{account:c.employee_account,paid:a.paid})))
}
/// Capture only the actual successful cash payment delta at this original tick.
/// All changes remain inside the same Prepared; a window error rolls back cash/work.
pub(crate) fn record_actual_wages(world:&World,before:PaidSnapshot,r:&Runtime,op:u64,at:u64,window:&mut Option<Box<crate::housing_income::PaidWageWindow>>)->Result<(),TxError>{
    for(i,c)in r.state.opening().contracts.iter().enumerate(){let Some(c)=c else{continue;};let a=r.state.records()[i].ok_or(TxError::InvalidOperation)?;let old=before[i].filter(|b|b.contract_id==c.id).ok_or(TxError::InvalidOperation)?;
        let delta=a.paid.checked_sub(old.paid).ok_or(TxError::InvalidOperation)?;if delta==0{continue;}
        let payment=a.last_payment.ok_or(TxError::InvalidOperation)?;
        if payment.source.operation_id!=op||payment.source.occurred_at!=at||payment.cash_operation_id!=op{return Err(TxError::InvalidOperation);}
        let opening=&world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening;
        let window=window.as_deref_mut().ok_or(TxError::InvalidOperation)?;window.begin_coverage(at).map_err(fail)?;
        window.record_paid_delta(opening,c.employee_account,money(delta)?,at).map_err(fail)?;
    }Ok(())
}
/// Contract eligibility is separate from actual cash received. Late pay remains
/// income on the payment day even if the old contract no longer overlaps a window.
pub(crate) fn wage_contracts(r:Option<&Runtime>,account:u32,start:u64,end:u64,now:u64)->Result<(u16,u16,bool),TxError>{
    let Some(r)=r else{return Ok((0,0,false));};if !r.ready||r.version!=AUTHORITY_VERSION{return Err(TxError::InvalidOperation);}
    let mut qualifying=0u16;let mut active=0u16;let mut arrears=false;
    for c in r.state.opening().contracts.iter().flatten(){if c.employee_account!=account{continue;}let(_,a)=contract(r,c.id)?;if a.phase==w::Phase::Pending{continue;}
        let through=if a.phase==w::Phase::Stopped{a.settled}else{c.end};if a.started<end&&through>start{qualifying=qualifying.checked_add(1).ok_or(TxError::Overflow)?;}
        if a.phase==w::Phase::Active&&c.end>now{active=active.checked_add(1).ok_or(TxError::Overflow)?;}arrears|=a.earned>a.paid;
    }Ok((qualifying,active,arrears))
}
fn window_hash(r:&Runtime,window:&crate::housing_income::PaidWageWindow)->Result<[u8;32],TxError>{digest(&("original-household-paid-window-v2",r.version,r.state.opening().binding,window))}
pub(crate) fn stamp_income_window(r:&mut Runtime,window:&crate::housing_income::PaidWageWindow,op:u64,revision:u64,at:u64)->Result<(),TxError>{
    let hash=window_hash(r,window)?;if r.income_window_anchor.is_none_or(|a|a.hash!=hash){r.income_window_anchor=Some(IncomeWindowAnchor{operation_id:op,revision,at,hash});}Ok(())
}
pub(crate) fn validate_income_window(r:Option<&Runtime>,window:Option<&crate::housing_income::PaidWageWindow>,timeline:Option<&Timeline>)->Result<(),TxError>{
    let Some(r)=r else{return Ok(());};match(r.income_window_anchor,window,timeline){
        (None,None,None)if r.state.last_source().is_none()&&r.runs.iter().all(Option::is_none)&&r.begin_revisions.iter().all(Option::is_none)=>Ok(()),
        (Some(a),Some(window),Some(t))if a.operation_id>0&&a.revision>0&&a.at<=t.progress.at&&window_hash(r,window)?==a.hash=>Ok(()),_=>Err(TxError::InvalidOperation)}
}
pub(crate) struct Candidate {pub runtime:Box<Runtime>,pub ledger:Box<m::Ledger>}
pub(crate) trait Funds {
    fn protect_wage(&mut self,account:u32,cents:u64)->Result<(),TxError>;
    fn pay_wage(&mut self,from:u32,to:u32,cents:u64)->Result<(),TxError>;
    fn release_wage(&mut self,account:u32,cents:u64)->Result<(),TxError>;
}
fn fail(_:impl std::fmt::Debug)->TxError{TxError::InvalidOperation}
fn rule(e:w::Error)->TxError{if matches!(e,w::Error::UnresolvedBudget){TxError::WorkingLimit}else if matches!(e,w::Error::Overflow){TxError::Overflow}else{fail(e)}}
fn digest(v:&impl Serialize)->Result<[u8;32],TxError>{Ok(Sha256::digest(serde_json::to_vec(v).map_err(fail)?).into())}
fn money(n:u64)->Result<Money,TxError>{Money::new(i64::try_from(n).map_err(|_|TxError::Overflow)?).map_err(fail)}
fn tick(t:Option<&Timeline>)->u64{t.map_or(0,|t|if t.progress.phase==Some(crate::schedule::Phase::P5){t.progress.at}else{t.progress.last_complete})}
fn contract(r:&Runtime,id:u64)->Result<(w::Contract,w::Record),TxError>{
    r.state.opening().contracts.iter().enumerate().find_map(|(i,c)|c.filter(|c|c.id==id).zip(r.state.records()[i])).ok_or(TxError::InvalidOperation)
}
fn source(r:&Runtime,op:u64,at:u64)->Result<w::Source,TxError>{Ok(w::Source{binding:r.state.opening().binding,ordinal:r.state.next_ordinal().map_err(rule)?,operation_id:op,occurred_at:at})}
fn member_source(r:&Runtime,l:&m::Ledger,op:u64,at:u64)->Result<m::Source,TxError>{
    let b=r.state.opening().binding;Ok(m::Source{scope:b.scope,population_origin:b.population_origin,epoch:b.epoch,ordinal:l.next_ordinal().map_err(fail)?,operation_id:op,occurred_at:at})
}
fn expected(world:&World)->Result<w::Binding,TxError>{
    let h=&world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening;
    let q=world.qualification.as_deref().ok_or(TxError::InvalidOperation)?;
    let people=h.households.iter().try_fold(0u64,|n,h|n.checked_add(u64::from(h.people)).ok_or(TxError::Overflow))?;
    Ok(w::Binding{scope:world.scope,population_origin:crate::member_qualification::housing_population_origin(world.scope,h),
        member_version:q.state.opening.member_version,people,epoch:crate::member_authority::EPOCH})
}
fn employee(world:&World,c:w::Contract)->Result<(),TxError>{
    world.account_handle(c.employer_account).map_err(fail)?;world.account_handle(c.employee_account).map_err(fail)?;
    let mut start=0u64;
    for h in &world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening.households {
        let end=start.checked_add(u64::from(h.people)).ok_or(TxError::Overflow)?;
        if c.members.start>=start&&c.members.end<=end {return if h.account==c.employee_account{Ok(())}else{Err(TxError::Unauthorized)};}
        start=end;
    }Err(TxError::InvalidOperation)
}
fn registered_venue(world:&World,c:w::Contract)->Result<(),TxError>{
    let v=world.venue.as_deref().ok_or(TxError::InvalidOperation)?;
    let o=v.opening.offers.iter().find(|o|o.use_id==c.venue_use_id).ok_or(TxError::InvalidOperation)?;
    let d=v.opening.venues.iter().find(|d|d.venue_id==o.venue_id).ok_or(TxError::InvalidOperation)?;
    if o.operator!=c.employer_account||d.operator!=c.employer_account||o.partition.member_range_start!=c.members.start||o.partition.member_range_end!=c.members.end
        ||o.window.start>c.start||o.window.end<c.end||u64::from(o.population.staff)!=c.members.end-c.members.start
        ||o.population.learners!=0||o.population.teachers!=0||o.population.visitors!=0 {return Err(TxError::InvalidOperation);}
    Ok(())
}
fn presence(world:&World,c:w::Contract,from:u64,until:u64,proposed:Option<&crate::venue_runtime::UseCandidate>)->Result<(),TxError>{
    registered_venue(world,c)?;let v=world.venue.as_deref().ok_or(TxError::InvalidOperation)?;
    let states=proposed.map_or(&v.states[..],|p|&p.states[..]);let facts=proposed.map_or(&v.facts[..],|p|&p.facts[..]);
    let row=states.iter().find(|s|s.use_id==c.venue_use_id).ok_or(TxError::Lease)?;
    let fact=facts.iter().find(|s|s.use_id==c.venue_use_id).ok_or(TxError::InvalidOperation)?;
    if !matches!(row.status,UseStatus::Actual|UseStatus::Released)||fact.entered.is_none_or(|f|f.occurred_at>from)
        ||fact.exited.is_some_and(|f|f.occurred_at<until) {return Err(TxError::InvalidOperation);}
    Ok(())
}
fn provider<'a>(world:&'a World,c:w::Contract,proposed:Option<&'a crate::continuous_supply::Runtime>)->Result<&'a crate::continuous_supply::Runtime,TxError>{
    let s=proposed.or(world.continuous_supply.as_deref()).ok_or(TxError::InvalidOperation)?;
    let matches=match c.role{
        w::Role::Electric=>c.device_id==u64::from(s.opening().resources[0]),
        w::Role::Water=>s.treatment_device()==Some(c.device_id),
    };
    // Water is bound only to the explicit original V2 owner and its actual
    // segment coverage, never qualification alone or an unrelated plant ID.
    if !matches||c.employer_account!=s.opening().owner{return Err(TxError::InvalidOperation);}
    Ok(s)
}
fn qualified(world:&World,c:w::Contract,at:u64)->Result<q::JobCheck,TxError>{
    q::check_job_at_boundary(world,at,q::JobRequirements{members:c.members,minimum_background:None,minimum_skill:None,required_base_mask:c.role.required_base().bit(),required_endorsement_mask:0})
}
fn covered(s:&crate::continuous_supply::Runtime,device:u64,from:u64,until:u64)->Result<u64,TxError>{s.work_covered(device,from,until)}
fn initial_funding(c:w::Contract,r:w::Record)->Result<u64,TxError>{let mut terms=c;terms.start=r.started;terms.maximum_earned_cents().map_err(rule)}
pub(crate) fn required_protection(r:Option<&Runtime>,account:u32)->Result<Money,TxError>{
    let Some(r)=r else{return Ok(Money::ZERO);};let mut n=0u64;
    for c in r.state.opening().contracts.iter().flatten(){let(_,a)=contract(r,c.id)?;if c.employer_account!=account{continue;}
        let v=match a.phase{w::Phase::Pending=>0,w::Phase::Active=>initial_funding(*c,a)?.checked_sub(a.paid).ok_or(TxError::Protection)?,w::Phase::Stopped=>a.due()};n=n.checked_add(v).ok_or(TxError::Overflow)?;
    }money(n)
}
pub(crate) fn initialize(input:Opening,world:&World)->Result<Box<Runtime>,String>{
    if input.authority_version!=AUTHORITY_VERSION{return Err("facility_authority_version".into());}let opening=input.work;
    if opening.binding!=expected(world).map_err(|_|"facility_binding_missing")?{return Err("facility_opening_binding".into());}
    for c in opening.contracts.iter().flatten(){employee(world,*c).map_err(|_|"facility_employee")?;registered_venue(world,*c).map_err(|_|"facility_venue")?;}
    let encoded=w::Runtime::terminal_encoded_bound_bytes(&opening).map_err(|_|"facility_encoded_bound")?.checked_add(root_encoded_bound().map_err(|_|"facility_root_encoded_bound")?).ok_or("facility_encoded_overflow")?; // {"state": ...}; ready is skipped.
    if encoded>MAX_OWNED_BYTES{return Err("facility_encoded_budget".into());}
    let state=w::Runtime::new(opening).map_err(|_|"facility_opening_state")?;
    if Runtime::owned_bytes()>MAX_OWNED_BYTES{return Err("facility_owned_budget".into());}
    Ok(Box::new(Runtime{version:AUTHORITY_VERSION,state,begin_revisions:[None;4],runs:[None;4],income_window_anchor:None,ready:true}))
}
pub(crate) fn check_peak(base:usize,limit:usize)->Result<(),TxError>{
    if base.checked_add(Runtime::owned_bytes()).and_then(|n|n.checked_add(m::Ledger::owned_bytes())).and_then(|n|n.checked_add(MAX_WORKSPACE_BYTES)).is_none_or(|n|n>limit){Err(TxError::WorkingLimit)}else{Ok(())}
}
pub(crate) fn validate_rows(world:&World,r:Option<&Runtime>,l:Option<&m::Ledger>,completed:u64)->Result<(),TxError>{
    let Some(r)=r else{return if l.is_some_and(|l|l.active().iter().any(|a|a.kind==m::Kind::FacilityOperation)){Err(TxError::InvalidOperation)}else{Ok(())};};
    if r.version!=AUTHORITY_VERSION{return Err(TxError::InvalidOperation);}
    if r.state.opening().binding!=expected(world)?{return Err(TxError::InvalidOperation);}
    r.state.validate_binding(r.state.opening(),completed).map_err(rule)?;
    let l=l.ok_or(TxError::InvalidOperation)?;let mut count=0;
    for c in r.state.opening().contracts.iter().flatten(){let(_,a)=contract(r,c.id)?;
        if a.phase==w::Phase::Active {
            count+=1;let b=a.begin.ok_or(TxError::InvalidOperation)?;
            if l.active().iter().filter(|row|row.id==a.activity_id&&row.owner==c.id&&row.kind==m::Kind::FacilityOperation
                &&row.members==[Some(c.members),None]&&row.started==a.started&&row.settled_through==a.settled&&row.start_operation_id==b.operation_id).count()!=1{return Err(TxError::InvalidOperation);}
        }
    }
    if l.active().iter().filter(|a|a.kind==m::Kind::FacilityOperation).count()!=count{return Err(TxError::InvalidOperation);}Ok(())
}
pub(crate) fn validate(world:&World,r:Option<&Runtime>,l:Option<&m::Ledger>,timeline:Option<&Timeline>,supply:Option<&crate::continuous_supply::Runtime>,venue:Option<&crate::venue_runtime::UseCandidate>)->Result<(),TxError>{
    validate_rows(world,r,l,tick(timeline))?;let Some(r)=r else{
        if supply.or(world.continuous_supply.as_deref()).is_some_and(|s|s.view().is_ok_and(|v|v.operator_labor_basis!=Money::ZERO)){return Err(TxError::InvalidOperation);}return Ok(());
    };
    for c in r.state.opening().contracts.iter().flatten(){employee(world,*c)?;registered_venue(world,*c)?;let(_,a)=contract(r,c.id)?;
        if a.phase==w::Phase::Pending{continue;}
        presence(world,*c,a.started,a.settled,venue)?;
        let source=provider(world,*c,supply)?;
        if covered(source,c.device_id,a.started,a.settled)?!=a.work_seconds{return Err(TxError::InvalidOperation);}
    }
    if let Some(s)=supply.or(world.continuous_supply.as_deref()) {
        let labor=r.state.records().iter().flatten().try_fold(0u64,|n,a|n.checked_add(a.earned).ok_or(TxError::Overflow))?;
        let device=s.view()?;
        if s.all_labor_basis()?!=money(labor)?{return Err(TxError::Stock);}
        // Persisted segment terms are a cache of immutable original contracts,
        // never a second authority for staffing, rates or wage carry.
        if let Some(batch)=s.treatment_batch(){for segment in batch.segments.iter().flatten(){
            for (id,water,rate,members,carry) in [
                (segment.electric_contract,false,segment.electric_hourly_cents,segment.electric_members,segment.electric_wage_carry),
                (segment.water_contract,true,segment.water_hourly_cents,segment.water_members,segment.water_wage_carry),
            ]{
                let(c,a)=contract(r,id)?;provider(world,c,Some(s))?;
                let prior=covered(s,c.device_id,a.started,segment.started_at)?;
                let n=u128::from(c.hourly_cents_per_member).checked_mul(u128::from(c.members.end-c.members.start))
                    .and_then(|v|v.checked_mul(u128::from(prior))).ok_or(TxError::Overflow)?;
                if (c.role==w::Role::Water)!=water||c.hourly_cents_per_member!=rate||c.members.end-c.members.start!=members
                    ||a.started>segment.started_at||carry!=u16::try_from(n%3600).map_err(fail)?
                    ||!r.runs.iter().flatten().any(|link|link.contract_id==id&&link.run_id==segment.source_run&&link.start_revision==segment.source_start_revision){return Err(TxError::InvalidOperation);}
            }
        }}
        // Reverse proof: a source history cannot retain production while its
        // operator responsibility is removed or relabeled as Pending.
        for run in device.runs.iter().flatten() {
            let end=run.started_at.checked_add(run.production_seconds).ok_or(TxError::Overflow)?;
            let links:Vec<_>=r.runs.iter().flatten().filter(|v|v.run_id==run.id&&v.start_operation==run.operation).collect();
            if links.len()!=if s.treatment_device().is_some(){2}else{1}{return Err(TxError::InvalidOperation);}
            let mut electric=0;let mut water=0;
            for link in links{let(c,a)=contract(r,link.contract_id)?;provider(world,c,Some(s))?;
            if c.role==w::Role::Electric{electric+=1;}else{water+=1;}
            let actual=covered(s,c.device_id,run.started_at,end)?;
            if a.phase==w::Phase::Pending||a.begin!=Some(link.begin)
                ||a.started>run.started_at||link.started_at!=run.started_at||link.begin_revision>=link.start_revision
                ||covered(s,c.device_id,run.started_at,a.settled)?!=actual||money(a.earned)?!=s.labor_basis_for_run(run.id,c.role==w::Role::Water)?{return Err(TxError::InvalidOperation);}
            }
            if electric!=1||water!=usize::from(s.treatment_device().is_some()){return Err(TxError::InvalidOperation);}
        }
        for(i,link)in r.runs.iter().enumerate(){if let Some(link)=link{
            let terms=r.state.opening().contracts[i].ok_or(TxError::InvalidOperation)?;
            if terms.id!=link.contract_id||r.begin_revisions[i]!=Some(link.begin_revision)||!device.runs.iter().flatten().any(|v|v.id==link.run_id&&v.operation==link.start_operation){return Err(TxError::InvalidOperation);}
        }}
    }
    for(i,a)in r.state.records().iter().enumerate(){if a.is_some_and(|a|a.begin.is_some())!=r.begin_revisions[i].is_some(){return Err(TxError::InvalidOperation);}}
    if let Some(t)=timeline {for p in &t.payroll{check_payroll(Some(r),p.spec.payee,p.started,p.spec.ends_at,p.working)?;}}
    Ok(())
}
pub(crate) fn prepare(world:&World,timeline:Option<&Timeline>,caller:Caller,command:Command,op:u64,funds:&mut impl Funds)->Result<Candidate,TxError>{
    let base=world.facility_work.as_deref().ok_or(TxError::InvalidOperation)?;if !base.ready{return Err(TxError::InvalidOperation);}
    let at=crate::education_authority::completed(timeline)?;
    let id=match command{Command::Begin{contract_id}|Command::Stop{contract_id}|Command::Pay{contract_id,..}=>contract_id};
    let(c,a)=contract(base,id)?;
    if !matches!(caller,Caller::System)&&!matches!(caller,Caller::Account(id)if id==c.employer_account||matches!(command,Command::Stop{..})&&id==c.employee_account){return Err(TxError::Unauthorized);}
    if matches!(command,Command::Begin{..})&&!matches!(caller,Caller::System){return Err(TxError::Unauthorized);}
    let mut r=Box::new(base.clone());let mut l=world.member_activity.clone().ok_or(TxError::InvalidOperation)?;
    match command {
        Command::Begin{..}=>{
            let s=provider(world,c,None)?;if s.view()?.genset.status==crate::continuous_genset::Status::Running{return Err(TxError::InvalidOperation);}
            if r.state.opening().contracts.iter().flatten().any(|other|other.id!=id&&other.device_id==c.device_id&&contract(&r,other.id).is_ok_and(|(_,r)|r.phase==w::Phase::Active)){return Err(TxError::InvalidOperation);}
            presence(world,c,at,at,None)?;let v=world.venue.as_deref().ok_or(TxError::InvalidOperation)?;
            if !v.states.iter().any(|s|s.use_id==c.venue_use_id&&s.status==UseStatus::Actual){return Err(TxError::InvalidOperation);}
            if timeline.is_some_and(|t|t.payroll.iter().any(|p|p.spec.payee==c.employee_account&&p.working&&p.started<c.end&&p.spec.ends_at>at)){return Err(TxError::InvalidOperation);}
            let q=qualified(world,c,at)?;if q.status!=q::JobStatus::Eligible{return Err(TxError::Unauthorized);}
            let event=member_source(&r,&l,op,at)?;let pc=l.prepare_begin(event,at,id,m::Kind::FacilityOperation,[Some(c.members),None]).map_err(fail)?;
            *l=pc.into_next(&l).map_err(fail)?;
            let p=r.state.prepare(source(&r,op,at)?,w::Command::Begin(w::Admission::from_root(c,event.ordinal,q.source))).map_err(rule)?;r.state=p.into_next(&r.state).map_err(rule)?;
            let(_,now)=contract(&r,id)?;funds.protect_wage(c.employer_account,initial_funding(c,now)?)?;
        },
        Command::Stop{..}=>close(&mut r,&mut l,c,op,at,funds)?,
        Command::Pay{cents,..}=>pay(&mut r,c,cents,op,at,funds)?,
    }
    if matches!(command,Command::Begin{..}) {let i=r.state.opening().contracts.iter().position(|c|c.is_some_and(|c|c.id==id)).ok_or(TxError::InvalidOperation)?;r.begin_revisions[i]=Some(world.revision.checked_add(1).ok_or(TxError::Overflow)?);}
    Ok(Candidate{runtime:r,ledger:l})
}
fn pay(r:&mut Runtime,c:w::Contract,cents:u64,op:u64,at:u64,funds:&mut impl Funds)->Result<(),TxError>{
    let p=r.state.confirm_payment(source(r,op,at)?,w::Payment::from_root(c,cents,op)).map_err(rule)?;
    funds.pay_wage(c.employer_account,c.employee_account,cents)?;r.state=p.into_next(&r.state).map_err(rule)?;Ok(())
}
pub(crate) fn close(r:&mut Runtime,l:&mut m::Ledger,c:w::Contract,op:u64,at:u64,funds:&mut impl Funds)->Result<(),TxError>{
    let(_,a)=contract(r,c.id)?;let before=required_protection(Some(r),c.employer_account)?;
    let p=r.state.prepare(source(r,op,at)?,w::Command::Stop{contract_id:c.id,activity_id:a.activity_id}).map_err(rule)?;
    let event=member_source(r,l,op,at)?;let member=l.prepare_settle(event,at,a.activity_id,c.id,m::Kind::FacilityOperation,true).map_err(fail)?;
    r.state=p.into_next(&r.state).map_err(rule)?;*l=member.into_next(l).map_err(fail)?;
    let after=required_protection(Some(r),c.employer_account)?;funds.release_wage(c.employer_account,u64::try_from(before.checked_sub(after).map_err(fail)?.value()).map_err(fail)?)?;Ok(())
}
pub(crate) fn authorize_start(world:&World,r:Option<&Runtime>,at:u64)->Result<(),TxError>{
    let Some(r)=r else{return Ok(());};if !r.ready{return Err(TxError::InvalidOperation);}
    let s=world.continuous_supply.as_deref().ok_or(TxError::InvalidOperation)?;let device=u64::from(s.opening().resources[0]);
    let mut n=0;let mut water=0;
    for c in r.state.opening().contracts.iter().flatten().filter(|c|c.device_id==device||Some(c.device_id)==s.treatment_device()){let(_,a)=contract(r,c.id)?;if a.phase!=w::Phase::Active{continue;}
        provider(world,*c,Some(s))?;
        if at<c.start||at>=c.end||a.settled!=at||qualified(world,*c,at)?.status!=q::JobStatus::Eligible{return Err(TxError::Unauthorized);}presence(world,*c,at,at,None)?;n+=1;
        if c.role==w::Role::Water{water+=1;}
    }if n!=1+usize::from(s.treatment_device().is_some())||water!=usize::from(s.treatment_device().is_some()){return Err(TxError::InvalidOperation);}Ok(())
}
pub(crate) fn bind_started(r:&mut Runtime,source:&crate::continuous_supply::Runtime,op:u64,revision:u64,at:u64)->Result<(),TxError>{
    let view=source.view()?;let run=view.runs.iter().flatten().find(|v|v.operation==op&&v.started_at==at).ok_or(TxError::InvalidOperation)?;
    let device=u64::from(source.opening().resources[0]);
    let indices:[Option<usize>;4]=std::array::from_fn(|i|r.state.opening().contracts[i].filter(|c|c.device_id==device||Some(c.device_id)==source.treatment_device()).and_then(|_|r.state.records()[i].filter(|a|a.phase==w::Phase::Active)).map(|_|i));
    if indices.iter().flatten().count()!=1+usize::from(source.treatment_device().is_some()){return Err(TxError::InvalidOperation);}
    for i in indices.into_iter().flatten(){
    if r.runs[i].is_some(){return Err(TxError::InvalidOperation);}let a=r.state.records()[i].ok_or(TxError::InvalidOperation)?;let c=r.state.opening().contracts[i].ok_or(TxError::InvalidOperation)?;
    r.runs[i]=Some(StartedRun{contract_id:c.id,begin:a.begin.ok_or(TxError::InvalidOperation)?,begin_revision:r.begin_revisions[i].ok_or(TxError::InvalidOperation)?,run_id:run.id,start_operation:run.operation,start_revision:revision,started_at:run.started_at});
    }Ok(())
}
pub(crate) struct JointTerms {pub electric:w::Contract,pub water:w::Contract,pub electric_carry:u16,pub water_carry:u16}
pub(crate) fn joint_terms(world:&World,r:Option<&Runtime>,at:u64)->Result<JointTerms,TxError>{
    authorize_start(world,r,at)?;let r=r.ok_or(TxError::InvalidOperation)?;
    let s=world.continuous_supply.as_deref().ok_or(TxError::InvalidOperation)?;
    let device=s.treatment_device().ok_or(TxError::InvalidOperation)?;
    let mut electric=None;let mut water=None;
    for c in r.state.opening().contracts.iter().flatten(){let(_,a)=contract(r,c.id)?;if a.phase!=w::Phase::Active{continue;}
        if c.device_id==u64::from(s.opening().resources[0]){electric=Some((*c,a.remainder));}
        if c.device_id==device{water=Some((*c,a.remainder));}
    }
    let(e,ec)=electric.ok_or(TxError::InvalidOperation)?;let(w,wc)=water.ok_or(TxError::InvalidOperation)?;
    Ok(JointTerms{electric:e,water:w,electric_carry:ec,water_carry:wc})
}
pub(crate) fn check_payroll(r:Option<&Runtime>,payee:u32,start:u64,end:u64,working:bool)->Result<(),TxError>{
    if !working{return Ok(());}if let Some(r)=r{for c in r.state.opening().contracts.iter().flatten(){let(_,a)=contract(r,c.id)?;if a.phase==w::Phase::Active&&c.employee_account==payee&&a.started<end&&start<c.end{return Err(TxError::InvalidOperation);}}}Ok(())
}
pub(crate) fn can_continue(world:&World,r:&Runtime,at:u64)->Result<bool,TxError>{
    let s=world.continuous_supply.as_deref().ok_or(TxError::InvalidOperation)?;let device=u64::from(s.opening().resources[0]);let mut count=0;
    for c in r.state.opening().contracts.iter().flatten().filter(|c|c.device_id==device||Some(c.device_id)==s.treatment_device()){let(_,a)=contract(r,c.id)?;if a.phase!=w::Phase::Active{continue;}
        if at<c.start||at>=c.end||qualified(world,*c,at)?.status!=q::JobStatus::Eligible||presence(world,*c,at,at,None).is_err(){return Ok(false);}count+=1;
    }Ok(count==1+usize::from(s.treatment_device().is_some()))
}
pub(crate) fn close_for_device(r:&mut Runtime,l:&mut m::Ledger,device:u64,op:u64,at:u64,funds:&mut impl Funds)->Result<(),TxError>{
    for i in 0..w::MAX_CONTRACTS{if let Some(c)=r.state.opening().contracts[i]{if c.device_id==device&&contract(r,c.id)?.1.phase==w::Phase::Active{close(r,l,c,op,at,funds)?;}}}Ok(())
}
pub(crate) fn close_for_source(r:&mut Runtime,l:&mut m::Ledger,s:&crate::continuous_supply::Runtime,op:u64,at:u64,funds:&mut impl Funds)->Result<(),TxError>{
    close_for_device(r,l,u64::from(s.opening().resources[0]),op,at,funds)?;
    if let Some(device)=s.treatment_device(){close_for_device(r,l,device,op,at,funds)?;}Ok(())
}
pub(crate) fn active_device_for_venue(r:&Runtime,use_id:u64)->Option<u64>{r.state.opening().contracts.iter().flatten().find(|c|c.venue_use_id==use_id&&contract(r,c.id).is_ok_and(|(_,a)|a.phase==w::Phase::Active)).map(|c|c.device_id)}
pub(crate) fn next_boundary(world:&World,r:Option<&Runtime>,t:&Timeline,supply:Option<&crate::continuous_supply::Runtime>)->Result<Option<u64>,TxError>{
    let Some(r)=r else{return Ok(None);};let now=t.progress.last_complete;let mut next=None;
    for c in r.state.opening().contracts.iter().flatten(){let(_,a)=contract(r,c.id)?;if a.phase!=w::Phase::Active{continue;}
        let s=provider(world,*c,supply)?;
        let due=if now>=c.end||s.view()?.genset.status!=crate::continuous_genset::Status::Running||qualified(world,*c,now)?.status!=q::JobStatus::Eligible{now}else{c.end};
        next=Some(next.map_or(due,|n:u64|n.min(due)));
    }
    if next.is_some(){if let Some(due)=q::next_source_change(world,now)?{next=Some(next.map_or(due,|n|n.min(due)));}}
    Ok(next)
}
/// Original P5 executes an already admitted real device interval. All downstream
/// mutations remain transaction-local; dry preparation failures commit no work.
pub(crate) fn settle_device(world:&World,r:&mut Runtime,l:&mut m::Ledger,source_device:&crate::continuous_supply::Runtime,op:u64,at:u64,funds:&mut impl Funds)->Result<Money,TxError>{
    let defer=source_device.treatment_opening().is_some_and(|o|matches!(o.wage_settlement,crate::continuous_supply::v2::WageSettlement::RetainProtectedEarned{..}));
    let mut earned=0u64;
    for i in 0..w::MAX_CONTRACTS {
        let Some(c)=r.state.opening().contracts[i]else{continue;};let(_,a)=contract(r,c.id)?;if a.phase!=w::Phase::Active{continue;}
        provider(world,c,Some(source_device))?;
        if a.settled<at {
            let seconds=covered(source_device,c.device_id,a.settled,at)?;
            if seconds!=0 {
                if seconds!=at-a.settled||at>c.end{return Err(TxError::InvalidOperation);}
                presence(world,c,a.settled,at,None)?;
                let e=member_source(r,l,op,at)?;let member=l.prepare_settle(e,at,a.activity_id,c.id,m::Kind::FacilityOperation,false).map_err(fail)?;
                let interval=member.settlement.ok_or(TxError::InvalidOperation)?;if interval.from!=a.settled||interval.through!=at{return Err(TxError::InvalidOperation);}
                let p=r.state.record_interval(source(r,op,at)?,w::ObservedInterval::from_root(c,a.activity_id,a.settled,at,op,op,op)).map_err(rule)?;
                let delta=p.next().records()[i].ok_or(TxError::InvalidOperation)?.earned.checked_sub(a.earned).ok_or(TxError::InvalidOperation)?;
                // Root preflights complete work+payment+close while effects remain
                // staged; a refused candidate never licenses real production.
                r.state=p.into_next(&r.state).map_err(rule)?;*l=member.into_next(l).map_err(fail)?;
                if delta>0{if !defer{pay(r,c,delta,op,at,funds)?;}earned=earned.checked_add(delta).ok_or(TxError::Overflow)?;}
            }
        }
        if at>=c.end||source_device.view()?.genset.status!=crate::continuous_genset::Status::Running {close(r,l,c,op,at,funds)?;}
    }money(earned)
}

pub const MAX_ANCHORS:usize=22;
pub struct RestoreCheck {fingerprint:[u8;32],checked:[bool;MAX_ANCHORS]}
fn anchors(r:&Runtime)->[Option<w::EventRef>;MAX_ANCHORS]{
    let mut out=[None;MAX_ANCHORS];
    for(i,row)in r.state.records().iter().enumerate(){if let Some(a)=row{out[4*i]=a.begin;out[4*i+1]=a.last_interval.map(|v|v.source);out[4*i+2]=a.last_payment.map(|v|v.source);out[4*i+3]=a.stopped;}}
    out[16]=r.state.last_source().map(Into::into);
    for(i,v)in r.runs.iter().enumerate(){out[17+i]=v.map(|v|w::EventRef{ordinal:v.start_revision,operation_id:v.start_operation,occurred_at:v.started_at});}
    out[21]=r.income_window_anchor.map(|a|w::EventRef{ordinal:a.revision,operation_id:a.operation_id,occurred_at:a.at});out
}
pub(crate) fn restore_operations(world:&World)->[Option<u64>;MAX_ANCHORS]{world.facility_work.as_deref().map_or([None;MAX_ANCHORS],|r|anchors(r).map(|a|a.map(|v|v.operation_id)))}
pub(crate) fn begin_restore(world:&World)->Result<RestoreCheck,TxError>{let r=world.facility_work.as_deref().ok_or(TxError::InvalidOperation)?;Ok(RestoreCheck{fingerprint:digest(r)?,checked:[false;MAX_ANCHORS]})}
fn coupled_stop_record(world:&World,r:&Runtime,target:w::Contract,record:&crate::transaction::JournalRecord)->bool{
    use crate::transaction::Action;
    let Some(s)=world.continuous_supply.as_deref().filter(|s|s.treatment_device().is_some())else{return false;};
    if provider(world,target,Some(s)).is_err(){return false;}
    // The exact target's stopped effect was already authenticated above. This
    // selector permits only the same original V2 source's other real contract
    // or venue to trigger its coupled stop; it is not a generic stop grant.
    match record.actions[0]{
        Some(Action::FacilityWork{command:Command::Stop{contract_id}})=>r.state.opening().contracts.iter().flatten().any(|c|c.id==contract_id
            &&provider(world,*c,Some(s)).is_ok()&&(record.actor==0||record.actor==c.employer_account||record.actor==c.employee_account)),
        Some(Action::Venue{use_id,command:crate::venue_runtime::Command::SafeExit{..}})=>record.actor==0&&r.state.opening().contracts.iter().flatten().any(|c|c.venue_use_id==use_id&&provider(world,*c,Some(s)).is_ok()),
        _=>false,
    }
}
pub(crate) fn check_restore(world:&World,c:&mut RestoreCheck,index:usize,record:&crate::transaction::JournalRecord)->Result<(),TxError>{
    use crate::transaction::Action;
    let r=world.facility_work.as_deref().ok_or(TxError::InvalidOperation)?;
    if index>=MAX_ANCHORS||c.checked[index]||digest(r)?!=c.fingerprint||record.actions[1..].iter().any(Option::is_some){return Err(TxError::InvalidOperation);}
    let a=anchors(r)[index].ok_or(TxError::InvalidOperation)?;let receipt=record.outcome.map_err(fail)?;
    if a.operation_id!=record.operation_id||receipt.operation_id!=record.operation_id||receipt.timeline.is_none_or(|p|p.at!=a.occurred_at){return Err(TxError::InvalidOperation);}
    let timeline=matches!(record.actions[0],Some(Action::Timeline{command:crate::timeline::Command::Advance{..}}))&&record.actor==0;
    if index==16 {
        if !anchors(r)[..16].iter().flatten().any(|v|*v==a){return Err(TxError::InvalidOperation);}
        // The matching concrete event anchor is also mandatory and checked.
    } else if index==21 {
        let anchor=r.income_window_anchor.ok_or(TxError::InvalidOperation)?;
        if receipt.revision!=anchor.revision||record.facility_context.and_then(|c|c.income_window)!=Some(anchor.hash){return Err(TxError::InvalidOperation);}
        let eligible=matches!(record.actions[0],Some(Action::Timeline{..}))&&record.actor==0
            ||matches!(record.actions[0],Some(Action::SettleExit{..}))
            ||matches!(record.actions[0],Some(Action::FacilityWork{command:Command::Pay{..}}));
        if !eligible{return Err(TxError::InvalidOperation);}
        let window=world.housing.as_deref().and_then(|h|h.paid_wages.as_deref()).ok_or(TxError::InvalidOperation)?;
        if window_hash(r,window)?!=anchor.hash{return Err(TxError::InvalidOperation);}
    } else if index>=17 {
        let link=r.runs[index-17].ok_or(TxError::InvalidOperation)?;
        let(terms,row)=contract(r,link.contract_id)?;
        if row.begin!=Some(link.begin)||receipt.revision!=link.start_revision||link.begin_revision>=link.start_revision||record.actor!=terms.employer_account
            ||!matches!(record.actions[0],Some(Action::ContinuousSupply{command:crate::continuous_supply::Command::Start{run_id,..}})if run_id==link.run_id){return Err(TxError::InvalidOperation);}
    } else {
        let i=index/4;let terms=r.state.opening().contracts[i].ok_or(TxError::InvalidOperation)?;let row=r.state.records()[i].ok_or(TxError::InvalidOperation)?;
        if index%4==0&&r.begin_revisions[i]!=Some(receipt.revision){return Err(TxError::InvalidOperation);}
        if index%4==1&&row.last_interval.is_none_or(|v|v.ledger_operation_id!=record.operation_id||v.device_operation_id!=record.operation_id||v.timeline_operation_id!=record.operation_id){return Err(TxError::InvalidOperation);}
        if index%4==2&&row.last_payment.is_none_or(|v|v.cash_operation_id!=record.operation_id){return Err(TxError::InvalidOperation);}
        {

            let effect=record.facility_context.and_then(|v|v.contracts[i]).filter(|v|v.contract_id==terms.id).ok_or(TxError::InvalidOperation)?;
            if index%4==0&&effect.begin!=Some(digest(&("facility-admission-v2",terms.id,row.begin,row.qualification_source,row.started,row.activity_id))?){return Err(TxError::InvalidOperation);}
            if index%4==3&&effect.stopped!=Some(digest(&("facility-stop-v2",terms.id,row.stopped,row.settled,row.work_seconds,row.earned,row.remainder))?){return Err(TxError::InvalidOperation);}
            if index%4==1&&effect.interval!=Some(digest(&("facility-actual-interval-v2",terms.id,row.last_interval.ok_or(TxError::InvalidOperation)?))?){return Err(TxError::InvalidOperation);}
            if index%4==2&&effect.payment!=Some(digest(&("facility-actual-payment-v2",terms.id,row.last_payment.ok_or(TxError::InvalidOperation)?))?){return Err(TxError::InvalidOperation);}
        }
        let accepted=match index%4 {
            0=>record.actor==0&&matches!(record.actions[0],Some(Action::FacilityWork{command:Command::Begin{contract_id}})if contract_id==terms.id),
            1=>timeline,
            2=>timeline||matches!(record.actions[0],Some(Action::FacilityWork{command:Command::Pay{contract_id,cents}})if contract_id==terms.id&&row.last_payment.is_some_and(|p|p.cents==cents)&&(record.actor==0||record.actor==terms.employer_account)),
            3=>timeline||matches!(record.actions[0],Some(Action::FacilityWork{command:Command::Stop{contract_id}})if contract_id==terms.id&&(record.actor==0||record.actor==terms.employer_account||record.actor==terms.employee_account))
                ||matches!(record.actions[0],Some(Action::ContinuousSupply{command:crate::continuous_supply::Command::Stop|crate::continuous_supply::Command::Fault{..}})if record.actor==terms.employer_account)
                ||matches!(record.actions[0],Some(Action::Venue{use_id,command:crate::venue_runtime::Command::SafeExit{..}})if record.actor==0&&use_id==terms.venue_use_id)
                ||coupled_stop_record(world,r,terms,record),
            _=>false,
        };if !accepted{return Err(TxError::InvalidOperation);}
    }c.checked[index]=true;Ok(())
}
pub(crate) fn finish_restore(world:&mut World,c:RestoreCheck)->Result<(),TxError>{
    let r=world.facility_work.as_deref().ok_or(TxError::InvalidOperation)?;
    if digest(r)?!=c.fingerprint||anchors(r).iter().enumerate().any(|(i,a)|a.is_some()&&!c.checked[i]){return Err(TxError::InvalidOperation);}
    if world.continuous_supply.as_deref().is_some_and(|s|!s.is_ready()){return Err(TxError::InvalidOperation);}
    world.facility_work_mut().ok_or(TxError::InvalidOperation)?.ready=true;Ok(())
}
pub(crate) fn authenticate_empty(world:&mut World)->Result<(),TxError>{
    if world.facility_work.as_deref().is_some_and(|r|anchors(r).iter().all(Option::is_none)) {let c=begin_restore(world)?;finish_restore(world,c)?;}Ok(())
}
impl crate::restore_into::Restore for Option<Box<Runtime>>{
    fn restore<'de,D:Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error>{*self=Option::<Box<Runtime>>::deserialize(d)?;Ok(())}
}
