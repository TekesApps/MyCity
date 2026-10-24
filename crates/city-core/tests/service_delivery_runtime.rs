//! Synthetic capability/state tests only; root's real Store/provider tests are separate.
use mycity_core::{arena,member_qualification,model_rows,service_needs,units};
#[path="../src/service_delivery_runtime.rs"]
mod service_delivery_runtime;
#[path="../../../tests/fixtures/issue-57-delivery/literal.rs"]
mod literal;
use service_delivery_runtime::*;
use service_needs::Kind;
fn opening()->Opening{
    let mut crews=model_rows::Rows::new().unwrap();
    for (index,kind) in Kind::ALL.into_iter().enumerate(){let n=index as u32+1;let start=if index==2{4}else{index as u64*2};
        crews.try_push(Crew{id:n,kind,members:member_qualification::MemberRange{start,end:start+if index==2{4}else{2}},operator:99,
            crew_resource:n*10+1,vehicle_resource:n*10+2,tools_resource:n*10+3}).unwrap();}
    Opening{version:VERSION,policy:Policy::LocalDevelopmentExplicitHostActivityV1,scope:arena::Scope::new(1,2).unwrap(),city_id:7,
        population_origin:[9;32],member_version:1,crews}
}
fn need(kind:Kind)->NeedKey{NeedKey{scope:arena::Scope::new(1,2).unwrap(),city_id:7,household_id:1,household_source:101,
    generation:1,kind,occurrence_day:0}}
fn assignment()->Assignment{Assignment{lease_tokens:[1,2,3],member_activity:57,origin_place:10,target_place:20,route_version:4,
    protected_until:64800,funding_source:[1;32],inputs_source:[2;32],return_source:[3;32]}}
struct Fake{source:Source,prior:Option<Receipt>,fact:Result<Verified,Gate>}
impl Authority for Fake{
    fn prior(&self)->Result<Option<Receipt>,Error>{Ok(self.prior)}
    fn source(&self)->Result<Source,Error>{Ok(self.source)}
    fn verify(&self,_:&Command,_:Option<&Task>)->Result<Verified,Gate>{self.fact}
}
fn fake(r:&Runtime,at:u64,fact:Verified)->Fake{Fake{source:Source{scope:r.opening().scope,city_id:r.opening().city_id,clock:Clock::Development,
    root:[8;32],revision:r.ordinal(),at,operation_id:1000+r.ordinal()%1_000_000},prior:None,fact:Ok(fact)}}
fn command(t:&Task,action:Action)->Command{Command{delivery_id:t.delivery_id,need:t.need,expected_revision:Some(t.revision),action}}
fn ready(r:&mut Runtime,a:Fake,c:Command)->Receipt{
    let candidate=match prepare(r,&a,c).unwrap(){Outcome::Ready(x)=>x,_=>panic!("expected ready")};
    assert!(candidate.matches(r).unwrap());let receipt=candidate.receipt;*r=*candidate.next;r.validate().unwrap();receipt
}
fn accept(r:&mut Runtime,kind:Kind)->u32{
    let c=Command{delivery_id:0,need:need(kind),expected_revision:None,action:Action::Accept{crew_id:kind.index() as u32+1}};
    let a=fake(r,28800,Verified::Admission(Admission{assignment:assignment(),protected_cents:literal::TOTAL_CENTS[kind.index()],contract_end:64800,
        need_generated_operation:55,qualification_root:[7;32]}));
    ready(r,a,c).effect.delivery_id
}
fn host(r:&mut Runtime,id:u32,action:Action,at:u64,from:u64,to:u64)->Receipt{
    let t=*r.task(id).unwrap();let c=command(&t,action);
    let crew=r.opening().crews.iter().find(|x|x.id==t.crew_id).unwrap();
    let mut a=fake(r,at,Verified::Cancellation);
    a.fact=Ok(Verified::Host(HostActivity{event_id:r.last_host_event()+1,operation_id:a.source.operation_id,occurred_at:at,
        members:crew.members,from_place:from,to_place:to,route_version:t.assignment.route_version}));
    ready(r,a,c)
}
fn work(r:&mut Runtime,id:u32,through:u64,seconds:u64,energy:u64,water:u64)->Receipt{
    let t=*r.task(id).unwrap();let c=command(&t,Action::Work);
    let a=fake(r,through.max(r.last_at()),Verified::Work(WorkPrefix{from:t.work_cursor,through,work_seconds:seconds,member_activity:t.assignment.member_activity,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:energy,water_ml:water}));ready(r,a,c)
}
fn control(r:&mut Runtime,id:u32,action:Action,at:u64,fact:Verified)->Receipt{
    let c=command(r.task(id).unwrap(),action);let a=fake(r,at,fact);ready(r,a,c)
}
fn onsite(kind:Kind)->(Runtime,u32){let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,kind);
    host(&mut r,id,Action::Depart,29100,10,10);host(&mut r,id,Action::Arrive,29400,10,20);(r,id)}
#[test]
fn literal_three_recipes_full_lifecycle_no_plans_as_delivery_and_deadline_after_close(){
    for kind in Kind::ALL{let i=kind.index();let (mut r,id)=onsite(kind);
        assert_eq!(r.task(id).unwrap().work_seconds,0);assert_eq!(r.task(id).unwrap().accrued_cents,literal::TOTAL_CENTS[i]/5);
        let end=29400+literal::WORK_SECONDS[i];work(&mut r,id,end,literal::WORK_SECONDS[i],literal::ENERGY_MWH[i],literal::WATER_ML[i]);
        assert_eq!(r.task(id).unwrap().phase,Phase::WorkComplete);
        assert_eq!(r.task(id).unwrap().accrued_cents,literal::TOTAL_CENTS[i]*4/5);
        host(&mut r,id,Action::BeginReturn,end,20,20);host(&mut r,id,Action::Returned,end+600,20,10);
        let c=command(r.task(id).unwrap(),Action::Close);let a=fake(&r,end+600,Verified::Closure);
        assert!(matches!(prepare(&r,&a,c),Err(Error::WrongPhase)));
        host(&mut r,id,Action::ResetComplete,end+1200,10,10);
        assert_eq!(r.task(id).unwrap().accrued_cents,literal::TOTAL_CENTS[i]);
        let closed=control(&mut r,id,Action::Close,end+1200,Verified::Closure);
        assert!(closed.effect.necessary);assert_eq!(r.next_due().unwrap(),Some(115200));
        let seal=control(&mut r,id,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
        assert!(!seal.effect.necessary);assert_eq!(seal.effect.delta_cents,0);
        assert_eq!(r.task(id).unwrap().deadline.unwrap().work_seconds,literal::WORK_SECONDS[i]);
        assert_eq!(r.next_due().unwrap(),None);
    }
}
#[test]
fn literal_partial_carry_checkpoint_and_full_prefix_agree(){
    let (mut split,id)=onsite(Kind::Police);let mut whole=split.clone();let mut end=29400;
    for (seconds,energy,water,fee) in literal::POLICE_PREFIXES{
        end+=seconds;let receipt=work(&mut split,id,end,seconds,energy,water);assert_eq!(receipt.effect.delta_cents,fee);
        let bytes=serde_json::to_vec(&split).unwrap();let mut restored:Runtime=serde_json::from_slice(&bytes).unwrap();
        assert!(!restored.is_ready());restored.validate().unwrap();restored.mark_authenticated().unwrap();split=restored;
    }
    work(&mut whole,id,33000,3600,100000,10000);
    let a=split.task(id).unwrap();let b=whole.task(id).unwrap();
    assert_eq!(a.work_seconds,b.work_seconds);assert_eq!(a.energy_carry,b.energy_carry);assert_eq!(a.water_carry,b.water_carry);
    assert_eq!(a.fee_carry,b.fee_carry);assert_eq!(a.accrued_cents,1600);assert_eq!(a.energy_mwh,100000);assert_eq!(a.water_ml,10000);
    assert_eq!(literal::POLICE_DISPATCH_CENTS+literal::POLICE_RESET_CENTS+1200,2000);
}
#[test]
fn all_missing_real_admission_guarantees_wait_without_consuming_slot(){
    let r=Runtime::new(opening(),0).unwrap();let c=Command{delivery_id:0,need:need(Kind::Police),expected_revision:None,action:Action::Accept{crew_id:2}};
    for gate in [Gate::Unknown,Gate::UnresolvedBudget,Gate::NoQualifiedCrew,Gate::NoCapacity,Gate::NoFunding,Gate::NoInputs,Gate::NoSafeReturn,Gate::ClosedToNew]{
        let mut a=fake(&r,28800,Verified::Cancellation);a.fact=Err(gate);
        assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(g)) if g==gate));assert!(r.tasks().is_empty());
    }
}
#[test]
fn cancel_unstarted_zero_fee_needs_verified_release_and_actual_departure_after_cancel_keeps_responsibility(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);
    let cancelled=control(&mut r,id,Action::Cancel,28801,Verified::Cancellation);
    assert_eq!(cancelled.effect.target_cents,0);assert_eq!(r.task(id).unwrap().phase,Phase::Accepted);
    let c=command(r.task(id).unwrap(),Action::Close);let a=fake(&r,28801,Verified::Cancellation);
    assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));
    host(&mut r,id,Action::Depart,29100,10,10);assert!(r.task(id).unwrap().cancelled);
    assert_eq!(r.task(id).unwrap().phase,Phase::Dispatched);assert_eq!(r.task(id).unwrap().accrued_cents,400);
    host(&mut r,id,Action::BeginReturn,29101,10,10);host(&mut r,id,Action::Returned,29102,10,10);
    host(&mut r,id,Action::ResetComplete,29702,10,10);
    assert_eq!(r.task(id).unwrap().work_seconds,0);assert_eq!(r.task(id).unwrap().accrued_cents,800);
    let closed=control(&mut r,id,Action::Close,29702,Verified::Closure);assert!(closed.effect.necessary);
}
#[test]
fn unstarted_cancel_close_has_zero_fee_but_deadline_due_survives(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Fire);
    control(&mut r,id,Action::Cancel,28801,Verified::Cancellation);
    let closed=control(&mut r,id,Action::Close,28801,Verified::Closure);
    assert_eq!(closed.effect.target_cents,0);assert!(closed.effect.necessary);
    let c=command(r.task(id).unwrap(),Action::Depart);let a=fake(&r,29100,Verified::Cancellation);
    assert!(matches!(prepare(&r,&a,c),Err(Error::WrongPhase)));
}
#[test]
fn deadline_freezes_prefix_once_and_late_work_cannot_rewrite(){
    let (mut r,id)=onsite(Kind::Police);work(&mut r,id,115200,1800,50000,5000);
    control(&mut r,id,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
    let frozen=r.task(id).unwrap().deadline;work(&mut r,id,117000,1800,50000,5000);
    assert_eq!(r.task(id).unwrap().phase,Phase::WorkComplete);assert_eq!(r.task(id).unwrap().deadline,frozen);
    assert_eq!(frozen.unwrap().work_seconds,1800);
    let c=command(r.task(id).unwrap(),Action::SealDeadline);let a=fake(&r,117000,Verified::Deadline{covered_through:117000});
    assert!(matches!(prepare(&r,&a,c),Err(Error::WrongPhase)));
}
#[test]
fn no_inputs_wrong_prefix_and_missing_deadline_do_not_invent_work(){
    let (r,id)=onsite(Kind::Police);let t=r.task(id).unwrap();let c=command(t,Action::Work);
    let p=WorkPrefix{from:t.work_cursor,through:t.work_cursor+1,work_seconds:1,member_activity:t.assignment.member_activity,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:0,water_ml:0};
    let a=fake(&r,p.through,Verified::Work(p));assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));
    let mut p=p;p.from+=1;p.energy_mwh=27;p.water_ml=2;
    let a=fake(&r,p.through,Verified::Work(p));assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));
    p.from=t.work_cursor;p.through=115201;
    let a=fake(&r,p.through,Verified::Work(p));assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::DeadlinePending))));
    assert_eq!(r.task(id).unwrap().work_seconds,0);
}
#[test]
fn replay_first_changed_payload_wrong_clock_source_and_restore_barrier(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);
    let c=command(r.task(id).unwrap(),Action::Cancel);let a=fake(&r,28801,Verified::Cancellation);let receipt=ready(&mut r,a,c);
    let mut a=fake(&r,0,Verified::Closure);a.source.root=[0;32];a.prior=Some(receipt);
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Replay(x)) if x==receipt));
    let mut changed=c;changed.action=Action::Close;assert!(matches!(prepare(&r,&a,changed),Err(Error::Conflict)));
    let bytes=serde_json::to_vec(&r).unwrap();let restored:Runtime=serde_json::from_slice(&bytes).unwrap();
    a.prior=None;assert!(matches!(prepare(&restored,&a,c),Err(Error::Unauthenticated)));
    let c=command(r.task(id).unwrap(),Action::Close);let mut a=fake(&r,28802,Verified::Closure);a.source.city_id=8;
    assert!(matches!(prepare(&r,&a,c),Err(Error::Stale)));
}
#[test]
fn counterfeit_host_wrong_members_route_time_and_reused_event_are_rejected(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);let t=*r.task(id).unwrap();let c=command(&t,Action::Depart);
    let crew=r.opening().crews[1];let base=HostActivity{event_id:1,operation_id:1001,occurred_at:29100,members:crew.members,from_place:10,to_place:10,route_version:4};
    for mode in 0..4{let mut h=base;match mode{0=>h.members.end+=1,1=>h.route_version+=1,2=>h.occurred_at-=1,_=>h.event_id=2}
        let a=fake(&r,29100,Verified::Host(h));assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));}
    host(&mut r,id,Action::Depart,29100,10,10);
    let c=command(r.task(id).unwrap(),Action::Arrive);let mut h=base;h.operation_id=1002;h.from_place=10;h.to_place=20;h.occurred_at=29400;
    let a=fake(&r,29400,Verified::Host(h));assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));
}
#[test]
fn two_tasks_cannot_sell_same_crew_and_zero_day_need_is_legal(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);assert_eq!(r.task(id).unwrap().need.occurrence_day,0);
    let mut key=need(Kind::Police);key.household_id=2;key.household_source=102;
    let c=Command{delivery_id:0,need:key,expected_revision:None,action:Action::Accept{crew_id:2}};
    let a=fake(&r,28801,Verified::Admission(Admission{assignment:assignment(),protected_cents:2000,contract_end:64800,need_generated_operation:56,qualification_root:[7;32]}));
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::NoCapacity))));assert_eq!(r.tasks().len(),1);
}
#[test]
fn ordinal_headroom_reserved_at_accept_allows_cancel_close_and_deadline_at_max(){
    let r=Runtime::new(opening(),0).unwrap();let mut json=serde_json::to_value(&r).unwrap();json["ordinal"]=(u64::MAX-11).into();
    let mut r:Runtime=serde_json::from_value(json).unwrap();r.mark_authenticated().unwrap();let id=accept(&mut r,Kind::Police);
    control(&mut r,id,Action::Cancel,28801,Verified::Cancellation);
    control(&mut r,id,Action::Close,28801,Verified::Closure);
    control(&mut r,id,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
    assert_eq!(r.task(id).unwrap().phase,Phase::Closed);r.validate().unwrap();
}
#[test]
fn actual_late_arrival_keeps_zero_deadline_work_and_cannot_skip_deadline_for_new_work(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);
    host(&mut r,id,Action::Depart,29100,10,10);
    host(&mut r,id,Action::Arrive,115201,10,20);
    assert_eq!(r.task(id).unwrap().arrival_at,Some(115201));assert_eq!(r.task(id).unwrap().work_seconds,0);
    control(&mut r,id,Action::SealDeadline,115201,Verified::Deadline{covered_through:115201});
    let d=r.task(id).unwrap().deadline.unwrap();assert_eq!(d.arrival,None);assert_eq!(d.work_seconds,0);
    work(&mut r,id,115202,1,27,2);assert_eq!(r.task(id).unwrap().deadline,Some(d));
}
#[test]
fn before_departure_or_clock_advance_alone_cannot_produce_arrival_work_or_reset(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);
    let t=*r.task(id).unwrap();let c=command(&t,Action::Arrive);let a=fake(&r,30000,Verified::Cancellation);
    assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));
    let c=command(&t,Action::Work);let a=fake(&r,30000,Verified::Work(WorkPrefix{from:0,through:30000,work_seconds:3600,
        member_activity:57,qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:100000,water_ml:10000}));
    assert!(matches!(prepare(&r,&a,c),Err(Error::WrongPhase)));
    assert_eq!(r.task(id).unwrap().phase,Phase::Accepted);assert_eq!(r.task(id).unwrap().accrued_cents,0);
}
#[test]
fn stale_candidate_cannot_apply_over_cancellation_and_full_metadata_does_not_consume_closing_reserve(){
    let mut r=Runtime::new(opening(),0).unwrap();let id=accept(&mut r,Kind::Police);
    let c=command(r.task(id).unwrap(),Action::Cancel);let a=fake(&r,28801,Verified::Cancellation);
    let stale=match prepare(&r,&a,c).unwrap(){Outcome::Ready(x)=>x,_=>panic!("ready")};
    control(&mut r,id,Action::Cancel,28801,Verified::Cancellation);assert!(!stale.matches(&r).unwrap());
    // Exhausting admission id space cannot prevent necessary close/deadline of an existing task.
    let mut json=serde_json::to_value(&r).unwrap();json["next_delivery"]=u32::MAX.into();
    let mut r:Runtime=serde_json::from_value(json).unwrap();r.mark_authenticated().unwrap();
    control(&mut r,id,Action::Close,28801,Verified::Closure);
    control(&mut r,id,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
    assert_eq!(r.task(id).unwrap().phase,Phase::Closed);
}
#[test]
fn wrong_registration_and_tampered_carry_or_fee_restore_fail(){
    let mut o=opening();o.version=1;assert!(matches!(Runtime::new(o,0),Err(Error::Registration)));
    let mut o=opening();o.crews[2].tools_resource=0;assert!(matches!(Runtime::new(o,0),Err(Error::Registration)));
    let (r,id)=onsite(Kind::Police);let mut json=serde_json::to_value(&r).unwrap();json["tasks"][0]["accrued_cents"]=401.into();
    let invalid:Runtime=serde_json::from_value(json).unwrap();assert_eq!(invalid.validate(),Err(Error::Invalid));
    let mut json=serde_json::to_value(&r).unwrap();json["tasks"][0]["need"]["occurrence_day"]=1.into();
    let invalid:Runtime=serde_json::from_value(json).unwrap();assert_eq!(invalid.validate(),Err(Error::Invalid));
    assert_eq!(r.task(id).unwrap().work_seconds,0);
}
#[test]
fn full_table_preserves_closed_deadline_responsibility_until_one_is_sealed(){
    let mut r=Runtime::new(opening(),0).unwrap();
    for household in 1..=4{
        let mut key=need(Kind::Police);key.household_id=household;key.household_source=100+u64::from(household);
        let c=Command{delivery_id:0,need:key,expected_revision:None,action:Action::Accept{crew_id:2}};
        let a=fake(&r,28800,Verified::Admission(Admission{assignment:assignment(),protected_cents:2000,contract_end:64800,need_generated_operation:55,qualification_root:[7;32]}));
        let id=ready(&mut r,a,c).effect.delivery_id;
        control(&mut r,id,Action::Cancel,28800,Verified::Cancellation);control(&mut r,id,Action::Close,28800,Verified::Closure);
    }
    assert_eq!(r.tasks().len(),4);
    let mut key=need(Kind::Police);key.household_id=5;key.household_source=105;
    let c=Command{delivery_id:0,need:key,expected_revision:None,action:Action::Accept{crew_id:2}};
    let a=fake(&r,28801,Verified::Admission(Admission{assignment:assignment(),protected_cents:2000,contract_end:64800,need_generated_operation:55,qualification_root:[7;32]}));
    assert!(matches!(prepare(&r,&a,c),Err(Error::Budget)));assert_eq!(r.history().reclaimed,0);
    control(&mut r,1,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
    key.occurrence_day=1;let c=Command{need:key,..c};
    let mut assigned=assignment();assigned.protected_until=151200;
    let a=fake(&r,115200,Verified::Admission(Admission{assignment:assigned,protected_cents:2000,contract_end:151200,need_generated_operation:99,qualification_root:[7;32]}));
    let receipt=ready(&mut r,a,c);assert_eq!(receipt.effect.delivery_id,5);assert_eq!(r.tasks().len(),4);
    assert_eq!(r.history().reclaimed,1);assert!(r.task(1).is_none());assert!(r.task(2).unwrap().deadline.is_none());
}
#[test]
fn actual_return_and_reset_do_not_erase_unknown_onsite_work_then_historical_prefix_settles(){
    let (mut r,id)=onsite(Kind::Police);
    // Original [29400,30000) sources are unknown when the real return is observed.
    let c=command(r.task(id).unwrap(),Action::Work);let mut a=fake(&r,30000,Verified::Cancellation);a.fact=Err(Gate::Unknown);
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::Unknown))));
    host(&mut r,id,Action::BeginReturn,30000,20,20);
    assert_eq!(r.task(id).unwrap().onsite_end,Some(30000));
    assert_eq!(r.task(id).unwrap().unsettled_onsite_through(),Some(30000));
    host(&mut r,id,Action::Returned,30600,20,10);host(&mut r,id,Action::ResetComplete,31200,10,10);
    let c=command(r.task(id).unwrap(),Action::Close);let a=fake(&r,31200,Verified::Closure);
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::HistoricalPending))));
    // Physical state and its current clock stay at Reset/31200. Only old certified work settles.
    let old_position=r.task(id).unwrap().position;let c=command(r.task(id).unwrap(),Action::Work);
    let a=fake(&r,31200,Verified::Work(WorkPrefix{from:29400,through:30000,work_seconds:600,member_activity:57,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:16666,water_ml:1666}));
    let receipt=ready(&mut r,a,c);
    assert_eq!(receipt.effect.delta_cents,200);assert_eq!(r.task(id).unwrap().work_seconds,600);
    assert_eq!(r.task(id).unwrap().phase,Phase::Reset);assert_eq!(r.task(id).unwrap().position,old_position);
    assert_eq!(r.last_at(),31200);assert_eq!(r.task(id).unwrap().unsettled_onsite_through(),None);
    assert_eq!(r.task(id).unwrap().accrued_cents,1000); // dispatch400 + work200 + reset400
    control(&mut r,id,Action::Close,31200,Verified::Closure);
}
#[test]
fn source_coverage_without_consumed_prefix_cannot_seal_false_zero_deadline(){
    let (mut r,id)=onsite(Kind::Police);
    host(&mut r,id,Action::BeginReturn,30000,20,20);host(&mut r,id,Action::Returned,30600,20,10);
    host(&mut r,id,Action::ResetComplete,31200,10,10);
    let c=command(r.task(id).unwrap(),Action::SealDeadline);
    let a=fake(&r,115200,Verified::Deadline{covered_through:115200});
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::DeadlinePending))));
    assert_eq!(r.task(id).unwrap().deadline,None);
    // The current root time is already deadline. A historical through=30000 does not rewind it.
    let c=command(r.task(id).unwrap(),Action::Work);
    let a=fake(&r,115200,Verified::Work(WorkPrefix{from:29400,through:30000,work_seconds:600,member_activity:57,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:16666,water_ml:1666}));
    ready(&mut r,a,c);assert_eq!(r.last_at(),115200);assert_eq!(r.task(id).unwrap().phase,Phase::Reset);
    control(&mut r,id,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
    let d=r.task(id).unwrap().deadline.unwrap();assert_eq!(d.work_seconds,600);assert_eq!(d.arrival,Some(29400));
    control(&mut r,id,Action::Close,115200,Verified::Closure);
    assert_eq!(r.task(id).unwrap().deadline,Some(d));
}
#[test]
fn pending_historical_interval_survives_restore_and_cannot_be_extended_past_departure(){
    let (mut r,id)=onsite(Kind::Police);host(&mut r,id,Action::BeginReturn,30000,20,20);
    let bytes=serde_json::to_vec(&r).unwrap();let mut r:Runtime=serde_json::from_slice(&bytes).unwrap();
    r.validate().unwrap();assert!(!r.is_ready());r.mark_authenticated().unwrap();
    assert_eq!(r.task(id).unwrap().unsettled_onsite_through(),Some(30000));
    let c=command(r.task(id).unwrap(),Action::Work);
    let a=fake(&r,31200,Verified::Work(WorkPrefix{from:29400,through:30001,work_seconds:600,member_activity:57,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:16666,water_ml:1666}));
    assert!(matches!(prepare(&r,&a,c),Err(Error::Unauthorized)));
    let mut invalid=serde_json::to_value(&r).unwrap();invalid["tasks"][0]["onsite_end"]=30001.into();
    let invalid:Runtime=serde_json::from_value(invalid).unwrap();assert_eq!(invalid.validate(),Err(Error::Invalid));
}
#[test]
fn completing_certified_work_while_returning_keeps_actual_return_phase(){
    let (mut r,id)=onsite(Kind::Police);host(&mut r,id,Action::BeginReturn,33000,20,20);
    assert_eq!(r.task(id).unwrap().unsettled_onsite_through(),Some(33000));
    work(&mut r,id,33000,3600,100000,10000);
    let t=r.task(id).unwrap();assert_eq!(t.work_seconds,3600);assert_eq!(t.phase,Phase::Returning);
    assert_eq!(t.return_at,Some(33000));assert_eq!(t.position,20);assert_eq!(t.accrued_cents,1600);
    assert_eq!(t.unsettled_onsite_through(),None);
}
#[test]
fn work_complete_still_onsite_consumes_certified_zero_prefix_and_seals_without_fake_return(){
    let (mut r,id)=onsite(Kind::Police);work(&mut r,id,33000,3600,100000,10000);
    assert_eq!(r.task(id).unwrap().phase,Phase::WorkComplete);
    let c=command(r.task(id).unwrap(),Action::SealDeadline);
    let a=fake(&r,115200,Verified::Deadline{covered_through:115200});
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::DeadlinePending))));
    let c=command(r.task(id).unwrap(),Action::Work);
    let mut a=fake(&r,115200,Verified::Cancellation);a.fact=Err(Gate::Unknown);
    assert!(matches!(prepare(&r,&a,c),Ok(Outcome::Wait(Gate::Unknown))));
    assert_eq!(r.task(id).unwrap().work_cursor,33000);
    let receipt=work(&mut r,id,115200,0,0,0);
    assert_eq!(receipt.effect.delta_cents,0);assert_eq!(receipt.effect.consumed_energy_mwh,0);
    let t=r.task(id).unwrap();assert_eq!(t.phase,Phase::WorkComplete);assert_eq!(t.position,20);
    assert_eq!(t.return_at,None);assert_eq!(t.onsite_end,None);assert_eq!(t.work_seconds,3600);
    assert_eq!(t.work_cursor,115200);assert_eq!(t.accrued_cents,1600);
    control(&mut r,id,Action::SealDeadline,115200,Verified::Deadline{covered_through:115200});
    assert_eq!(r.task(id).unwrap().deadline.unwrap().work_seconds,3600);
    assert_eq!(r.task(id).unwrap().phase,Phase::WorkComplete);assert_eq!(r.task(id).unwrap().return_at,None);
}
#[test]
fn restored_work_complete_accepts_only_zero_credit_and_preserves_deadline_cut(){
    let (mut r,id)=onsite(Kind::Police);work(&mut r,id,33000,3600,100000,10000);
    let bytes=serde_json::to_vec(&r).unwrap();let mut restored:Runtime=serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();assert!(!restored.is_ready());restored.mark_authenticated().unwrap();
    let c=command(restored.task(id).unwrap(),Action::Work);
    let a=fake(&restored,33001,Verified::Work(WorkPrefix{from:33000,through:33001,work_seconds:1,member_activity:57,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:27,water_ml:2}));
    assert!(matches!(prepare(&restored,&a,c),Err(Error::Invalid)));
    let a=fake(&restored,115201,Verified::Work(WorkPrefix{from:33000,through:115201,work_seconds:0,member_activity:57,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:0,water_ml:0}));
    assert!(matches!(prepare(&restored,&a,c),Ok(Outcome::Wait(Gate::DeadlinePending))));
    // Keep the current observation at 115201 while consuming only the historical cutoff.
    let a=fake(&restored,115201,Verified::Work(WorkPrefix{from:33000,through:115200,work_seconds:0,member_activity:57,
        qualification_root:[7;32],electric_source:[4;32],water_source:[5;32],energy_mwh:0,water_ml:0}));
    ready(&mut restored,a,c);
    let bytes=serde_json::to_vec(&restored).unwrap();let mut restored:Runtime=serde_json::from_slice(&bytes).unwrap();
    restored.validate().unwrap();restored.mark_authenticated().unwrap();
    control(&mut restored,id,Action::SealDeadline,115201,Verified::Deadline{covered_through:115201});
    let t=restored.task(id).unwrap();assert_eq!(t.phase,Phase::WorkComplete);assert_eq!(t.work_seconds,3600);
    assert_eq!(t.deadline.unwrap().work_seconds,3600);assert_eq!(t.accrued_cents,1600);assert_eq!(t.return_at,None);
}
