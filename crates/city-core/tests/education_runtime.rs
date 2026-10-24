//! Synthetic crate-private root witnesses only. No real Store, Timeline, venue or writer.
//! Root must run separate real Opening/World/receipt/cold-Store acceptance after integration.
pub use mycity_core::units;
#[path = "../src/education.rs"]
mod education;
#[path = "../src/education_runtime.rs"]
mod education_runtime;
use education as e;
use education_runtime::*;
use units::{GameTime,Money};
fn t(s: i64) -> GameTime { GameTime::new(s).unwrap() }
fn members(start: u64,end: u64) -> Members { Members {source:1,version:1,start,end} }
fn identity() -> e::Identity { e::Identity {world:1,branch:2,scope:3} }
fn registration(id: u64, swapped: bool) -> Registration {
    let teacher = if swapped {members(0,1)} else {members(2,3)};
    let learners = if swapped {members(1,3)} else {members(0,2)};
    Registration {enrollment:e::Enrollment {cohort:id,generation:1,roster_ref:10+id,class:100,school:200,
        learners:2,started:t(0),fee_contract_ref:30+id,obligations_ref:40+id},
        course:e::Course {reference:50+id,path:e::Path::Retraining,cents_per_hour:Money::ZERO},
        learners,teacher,use_id:300,
        qualification:QualificationRegistration {origin:QualificationOrigin::ImmutableGameOpeningV1,source_ref:60+id,course_ref:50+id,teacher,valid_from:t(0),valid_until:t(100*86400)},
        free_contract:FreeContractRegistration {basis:FreeBasis::ImmutableGameOpeningEntitlementV1,source_ref:70+id,contract_ref:30+id,course_ref:50+id,roster_ref:10+id}}
}
fn offer() -> RegisteredOffer {
    RegisteredOffer {use_id:300,venue_id:200,group_id:100,resource_id:501,members:members(0,3),
        learners:2,teachers:1,staff:0,visitors:0,enrollment:2,window:e::Window {start:t(0),end:t(100*86400)}}
}
fn opening(two: bool) -> Opening {
    Opening {version:1,profile:Profile::FreeOfflineWholeGroupV1,identity:identity(),stream_epoch:7,population_origin:[8;32],
        courses:[Some(registration(1,false)),if two {Some(registration(2,true))} else {None},None,None]}
}
fn construct(input: Opening) -> Result<Runtime,Error> {
    construct_with_offer(input,offer())
}
fn construct_with_offer(input: Opening,registered: RegisteredOffer) -> Result<Runtime,Error> {
    let qs: Vec<_> = input.courses.iter().flatten().map(|r| r.qualification).collect();
    let fs: Vec<_> = input.courses.iter().flatten().map(|r| r.free_contract).collect();
    let offers = [registered];
    let authority = VerifiedOpening::after_root_opening_validation(identity(),input.fingerprint().unwrap(),input.registration_origin().unwrap(),[8;32],3,&offers,&qs,&fs);
    Runtime::new(input,authority)
}
struct Host { runtime:Runtime, input:Opening, ledger:Vec<SharedActivity>, venue_window:e::Window }
impl Host {
    fn new(two: bool) -> Self {
        let input = opening(two); Self {runtime:construct(input.clone()).unwrap(),input,ledger:Vec::new(),venue_window:offer().window}
    }
    fn late_clock(two: bool) -> Self {
        let mut input=opening(two);
        for r in input.courses.iter_mut().flatten() {r.qualification.valid_until=t(i64::MAX);}
        let mut registered=offer(); registered.window.end=t(i64::MAX);
        Self {runtime:construct_with_offer(input.clone(),registered).unwrap(),input,ledger:Vec::new(),venue_window:registered.window}
    }
    fn source(&self,at: i64) -> Origin {
        let seq = self.runtime.next_sequence().unwrap();
        Origin {identity:identity(),root:[7;32],revision:seq,clock:e::Clock::Development,operation_id:seq.wrapping_add(20000),
            event_id:seq.wrapping_add(1000),stream_epoch:7,stream_sequence:seq,occurred_at:t(at)}
    }
    fn venue(&self) -> VenueCoverage {
        VenueCoverage {root:[7;32],use_id:300,venue_id:200,group_id:100,resource_id:501,token_number:1,
            members:members(0,3),offer_window:self.venue_window,entered_at:t(0),exited_at:None}
    }
    fn plan_with(&self,cohort: u64,action: Action,source: Origin,venue: Option<VenueCoverage>) -> Result<Candidate,Error> {
        let event = VerifiedEvent::after_system_host_validation(Command {cohort,action},source,source.occurred_at)?;
        self.runtime.plan(Snapshot::after_root_source_validation(event,self.input.fingerprint().unwrap(),[8;32],venue,
            &self.ledger,LedgerCoverage::EducationOnlyV1,None))
    }
    fn plan(&self,cohort: u64,action: Action,at: i64) -> Result<Candidate,Error> {
        self.plan_with(cohort,action,self.source(at),Some(self.venue()))
    }
    fn plan_stopped(&self,cohort: u64,action: Action,source: Origin,venue: VenueCoverage,stop: Option<StoppedCoverage>) -> Result<Candidate,Error> {
        let event=VerifiedEvent::after_system_host_validation(Command {cohort,action},source,source.occurred_at)?;
        self.runtime.plan(Snapshot::after_root_source_validation(event,self.input.fingerprint().unwrap(),[8;32],Some(venue),
            &self.ledger,LedgerCoverage::EducationOnlyV1,stop))
    }
    fn exit_proof(&self,source: Origin,at: i64) -> StoppedCoverage {
        StoppedCoverage {identity:identity(),root:source.root,use_id:300,token_number:1,exited_at:t(at),
            exit_operation_id:source.operation_id,exit_event_id:source.event_id,exit_revision:source.revision,
            no_instruction:e::Window {start:t(at),end:t(at)},actual:None}
    }
    fn resolve_plan(&self,cohort: u64,begin: Origin,exit: Origin,through: i64,known_end: i64,now: i64,finalize: bool) -> Result<Candidate,Error> {
        let source=self.source(now); let mut proof=self.exit_proof(exit,exit.occurred_at.value()); proof.root=source.root;
        proof.no_instruction=e::Window {start:t(known_end),end:t(now)};
        proof.actual=Some(ActualCoverage {identity:identity(),root:source.root,cohort,generation:1,
            original_begin_event_id:begin.event_id,kind:self.runtime.stopped_activity(cohort)?.ok_or(Error::Unknown)?.kind,
            window:e::Window {start:begin.occurred_at,end:t(known_end)},source_operation_id:source.operation_id,
            source_event_id:source.event_id,source_revision:source.revision,observed_at:t(now)});
        let mut venue=self.venue(); venue.exited_at=Some(exit.occurred_at);
        self.plan_stopped(cohort,Action::SettleAfterExit {started_event_id:begin.event_id,exited_at:exit.occurred_at,
            exit_operation_id:exit.operation_id,actual_through:t(through),finalize},source,venue,Some(proof))
    }
    fn stop(&mut self,cohort: u64,begin: Origin,source: Origin) {
        let proof=self.exit_proof(source,source.occurred_at.value()); let mut venue=self.venue(); venue.exited_at=Some(source.occurred_at);
        let candidate=self.plan_stopped(cohort,Action::StopForExit {started_event_id:begin.event_id,exited_at:source.occurred_at,
            exit_operation_id:source.operation_id},source,venue,Some(proof)).unwrap(); self.commit(candidate);
    }
    fn commit(&mut self,candidate: Candidate) {
        let delta = candidate.activity_delta;
        self.runtime.apply_candidate(candidate).unwrap();
        // Synthetic single-thread fixture only. Production root prepares BOTH images and
        // receipt, commits once, and never uses this helper as an atomicity implementation.
        match delta {
            Some(ActivityDelta::Acquire {teacher,learners}) => { self.ledger.push(teacher); if let Some(row) = learners {self.ledger.push(row);} }
            Some(ActivityDelta::Settle {teacher,learners,from,through,close}) => {
                for key in [Some(teacher),learners].into_iter().flatten() {
                    let i = self.ledger.iter().position(|row| row.key == key).unwrap();
                    assert_eq!(self.ledger[i].settled_through,from);
                    if close {self.ledger.remove(i);} else {self.ledger[i].settled_through=through;}
                }
            }
            None => {}
        }
    }
    fn apply(&mut self,cohort: u64,action: Action,at: i64) -> Origin {
        let candidate = self.plan(cohort,action,at).unwrap(); let source = candidate.source(); self.commit(candidate); source
    }
    fn activate(&mut self,cohort: u64) { self.apply(cohort,Action::ActivateNoInstruction,1); }
    fn gap(&mut self,cohort: u64,from: i64,through: i64) {
        self.apply(cohort,Action::NoInstruction {window:e::Window {start:t(from),end:t(through)}},through);
    }
    fn compact(&mut self) {
        let anchor = ReceiptAnchor {store_ref:self.runtime.last_source().unwrap().operation_id,root:[9;32],epoch:7,through:self.runtime.consumed_sequence()};
        let witness = VerifiedCompaction::after_durable_prefix_validation(self.runtime.fingerprint().unwrap(),anchor);
        self.runtime.compact_prefix(witness,&self.ledger).unwrap();
    }
    fn reopen(&mut self) {
        let expected = self.runtime.fingerprint().unwrap(); let anchor = self.runtime.prefix_anchor();
        let encoded = serde_json::to_vec(&self.runtime).unwrap();
        let decoded: Runtime = serde_json::from_slice(&encoded).unwrap();
        let witness = VerifiedRestore::after_original_store_validation(expected,self.input.fingerprint().unwrap(),self.input.registration_origin().unwrap(),anchor);
        self.runtime = decoded.restore(witness,&self.ledger).unwrap();
        assert_eq!(self.runtime.fingerprint().unwrap(),expected);
    }
    fn synthetic_prefix_sequence(&mut self, through: u64) {
        self.compact();
        let mut data=serde_json::to_value(&self.runtime).unwrap();
        data["consumed_sequence"]=through.into(); data["prefix"]["through"]=through.into();
        data["prefix"]["anchor"]["through"]=through.into();
        data["last_source"]["stream_sequence"]=through.into(); data["last_source"]["revision"]=through.into();
        data["prefix"]["last_source"]=data["last_source"].clone();
        let raw:Runtime=serde_json::from_value(data).unwrap();
        let auth=VerifiedRestore::after_original_store_validation(raw.fingerprint().unwrap(),self.input.fingerprint().unwrap(),self.input.registration_origin().unwrap(),raw.prefix_anchor());
        self.runtime=raw.restore(auth,&self.ledger).unwrap();
    }
    fn prepared(&mut self) {
        self.activate(1); let begin = self.apply(1,Action::BeginPreparation,1);
        self.apply(1,Action::SettlePreparation {started_event_id:begin.event_id,close:true},7201);
    }
}
#[test]
fn registration_presence_and_preparation_do_not_create_hours_and_tick_zero_cannot_activate() {
    let mut h = Host::new(false);
    assert_eq!(h.runtime.progress(1).unwrap(),None);
    assert!(matches!(h.plan(1,Action::ActivateNoInstruction,0),Err(Error::Clock)));
    assert_eq!(h.runtime.consumed_sequence(),0);
    h.activate(1); assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,0);
    assert!(matches!(h.plan(1,Action::BeginLesson,1),Err(Error::Unknown)));
    let prep = h.apply(1,Action::BeginPreparation,1);
    assert!(h.runtime.pending(1).unwrap()); assert_eq!(h.ledger.len(),1);
    h.apply(1,Action::SettlePreparation {started_event_id:prep.event_id,close:true},7201);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,0);
    let lesson = h.apply(1,Action::BeginLesson,7201);
    assert_eq!(h.ledger.len(),2);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,0);
    h.apply(1,Action::SettleLesson {started_event_id:lesson.event_id,close:true},10801);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap(),e::Progress {actual_seconds_per_learner:3600,actual_teaching_days:1,
        fee_cents_per_learner:Money::ZERO,fee_remainder:0});
    assert!(h.ledger.is_empty()); // NOT an original venue SafeExit or lease release.
    let json = serde_json::to_value(&h.runtime).unwrap();
    assert_eq!(json["states"][0]["cohort"]["enrollment"]["started"],0);
    assert_eq!(json["states"][0]["cohort"]["enrollment"]["obligations_ref"],41);
}
#[test]
fn ongoing_claim_rejects_teacher_learner_overlap_before_either_activity_finishes() {
    let mut h = Host::new(true); h.activate(1); h.activate(2);
    let prep = h.apply(1,Action::BeginPreparation,1);
    h.apply(1,Action::SettlePreparation {started_event_id:prep.event_id,close:true},7201);
    h.gap(2,1,7201);
    let lesson = h.apply(1,Action::BeginLesson,7201);
    let before = h.runtime.fingerprint().unwrap();
    // Course 2's explicit teacher [0,1) is a learner [0,2) of ongoing course 1.
    assert!(matches!(h.plan(2,Action::BeginPreparation,7201),Err(Error::Conflict)));
    assert_eq!(h.runtime.fingerprint().unwrap(),before); assert_eq!(h.ledger.len(),2);
    h.reopen(); assert!(h.runtime.pending(1).unwrap());
    assert!(matches!(h.plan(2,Action::BeginPreparation,7201),Err(Error::Conflict)));
    h.apply(1,Action::SettleLesson {started_event_id:lesson.event_id,close:true},10801);
}
#[test]
fn exited_or_retokened_venue_rejects_credit_without_discarding_pending_responsibility() {
    let mut h = Host::new(false); h.prepared(); let lesson = h.apply(1,Action::BeginLesson,7201);
    let before = h.runtime.fingerprint().unwrap();
    for mut venue in [h.venue(),h.venue()] {
        if venue.token_number == 1 {venue.exited_at=Some(t(8000));}
        assert!(h.plan_with(1,Action::SettleLesson {started_event_id:lesson.event_id,close:true},h.source(10801),Some(venue)).is_err());
    }
    let mut changed = h.venue(); changed.token_number=2;
    assert!(matches!(h.plan_with(1,Action::SettleLesson {started_event_id:lesson.event_id,close:true},h.source(10801),Some(changed)),Err(Error::Unknown)));
    assert_eq!(h.runtime.fingerprint().unwrap(),before); assert!(h.runtime.pending(1).unwrap());
    assert_eq!(h.ledger.len(),2); assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,0);
}
#[test]
fn safe_exit_cuts_future_exclusion_with_unknown_history_then_late_proven_prefix_settles_once() {
    let mut h=Host::new(false); h.prepared(); let began=h.apply(1,Action::BeginLesson,7201);
    let exit_source=h.source(10001); let stop=h.exit_proof(exit_source,10001);
    let mut exited=h.venue(); exited.exited_at=Some(t(10001));
    let action=Action::StopForExit {started_event_id:began.event_id,exited_at:t(10001),exit_operation_id:exit_source.operation_id};
    let before=h.runtime.fingerprint().unwrap(); let mut bad=stop; bad.token_number=99;
    assert!(h.plan_stopped(1,action,exit_source,exited,Some(bad)).is_err());
    assert_eq!(h.runtime.fingerprint().unwrap(),before); assert_eq!(h.ledger.len(),2);
    let cut=h.plan_stopped(1,action,exit_source,exited,Some(stop)).unwrap(); h.commit(cut);
    assert!(!h.runtime.pending(1).unwrap()); assert!(h.runtime.awaiting_settlement(1).unwrap());
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,0);
    assert!(h.ledger.is_empty()); assert!(h.runtime.pending_claims().unwrap().iter().all(Option::is_none));
    assert_eq!(h.runtime.stopped_activity(1).unwrap().unwrap().consumed_through,t(7201));
    h.compact(); h.reopen();
    // Members may start admitted future work after physical cutoff; unresolved OLD
    // course history creates no active claim. This remains a synthetic ledger projection.
    h.ledger.push(SharedActivity {key:ClaimKey {epoch:90,sequence:1,cohort:99,role:Role::Learners},
        members:members(0,2),kind:ActivityKind::Work,start:t(10001),settled_through:t(10001),end:None,source:exit_source});
    let resolved_source=h.source(10801);
    let resolve=Action::SettleAfterExit {started_event_id:began.event_id,exited_at:t(10001),
        exit_operation_id:exit_source.operation_id,actual_through:t(9001),finalize:true};
    let mut evidence=stop; evidence.root=resolved_source.root;
    evidence.no_instruction=e::Window {start:t(9001),end:t(10801)};
    let unchanged=h.runtime.fingerprint().unwrap();
    // Presence/exit alone cannot fill 7201..9001 with teaching.
    assert!(matches!(h.plan_stopped(1,resolve,resolved_source,exited,Some(evidence)),Err(Error::Unknown)));
    assert_eq!(h.runtime.fingerprint().unwrap(),unchanged); assert_eq!(h.ledger.len(),1);
    evidence.actual=Some(ActualCoverage {identity:identity(),root:resolved_source.root,cohort:1,generation:1,
        original_begin_event_id:began.event_id,kind:ActivityKind::Teaching,window:e::Window {start:t(7201),end:t(9001)},
        source_operation_id:resolved_source.operation_id,source_event_id:resolved_source.event_id,
        source_revision:resolved_source.revision,observed_at:resolved_source.occurred_at});
    let candidate=h.plan_stopped(1,resolve,resolved_source,exited,Some(evidence)).unwrap();
    assert_eq!(candidate.activity_delta,None); h.commit(candidate);
    assert!(!h.runtime.awaiting_settlement(1).unwrap()); assert_eq!(h.ledger.len(),1);
    // Literal: only 9001-7201=1800 seconds. 9001..10801 is NOT course instruction,
    // not a statement that these members were idle (their new work remains admitted).
    assert_eq!(h.runtime.progress(1).unwrap().unwrap(),e::Progress {actual_seconds_per_learner:1800,actual_teaching_days:1,
        fee_cents_per_learner:Money::ZERO,fee_remainder:0});
    assert!(matches!(h.plan_stopped(1,resolve,resolved_source,exited,Some(evidence)),Err(Error::Duplicate)));
    h.compact(); h.reopen(); assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,1800);
}
#[test]
fn cutoff_only_stop_rejects_actual_some_without_consuming_source_or_changing_claims() {
    let mut h=Host::new(false); h.activate(1); let begin=h.apply(1,Action::BeginPreparation,1);
    let source=h.source(100); let mut proof=h.exit_proof(source,100);
    proof.actual=Some(ActualCoverage {identity:identity(),root:source.root,cohort:1,generation:1,
        original_begin_event_id:begin.event_id,kind:ActivityKind::Preparation,window:e::Window {start:t(1),end:t(100)},
        source_operation_id:source.operation_id,source_event_id:source.event_id,source_revision:source.revision,observed_at:t(100)});
    let mut venue=h.venue(); venue.exited_at=Some(t(100));
    let before=h.runtime.fingerprint().unwrap(); let ledger=h.ledger.clone(); let sequence=h.runtime.consumed_sequence();
    let action=Action::StopForExit {started_event_id:begin.event_id,exited_at:t(100),exit_operation_id:source.operation_id};
    assert!(matches!(h.plan_stopped(1,action,source,venue,Some(proof)),Err(Error::Invalid)));
    assert_eq!(h.runtime.fingerprint().unwrap(),before); assert_eq!(h.ledger,ledger);
    assert_eq!(h.runtime.consumed_sequence(),sequence);
    assert!(h.runtime.pending(1).unwrap()); assert!(!h.runtime.awaiting_settlement(1).unwrap());
}
#[test]
fn pre_fix_runtime_strategy_is_rejected_without_silent_snapshot_upgrade() {
    let h=Host::new(false);
    let mut absent=serde_json::to_value(&h.runtime).unwrap(); absent.as_object_mut().unwrap().remove("strategy_version");
    assert!(serde_json::from_value::<Runtime>(absent).is_err());
    let mut old=serde_json::to_value(&h.runtime).unwrap(); old["strategy_version"]=1.into();
    let decoded:Runtime=serde_json::from_value(old).unwrap();
    let auth=VerifiedRestore::after_original_store_validation(h.runtime.fingerprint().unwrap(),h.input.fingerprint().unwrap(),
        h.input.registration_origin().unwrap(),h.runtime.prefix_anchor());
    assert!(matches!(decoded.restore(auth,&h.ledger),Err(Error::Restore)));
}
#[test]
fn certified_multiday_preparation_is_consumed_in_historical_batches_without_inventing_a_suffix() {
    let mut h=Host::new(false); h.activate(1); let begin=h.apply(1,Action::BeginPreparation,1);
    let cutoff=3*86400+13; let now=cutoff+7;
    let exit=h.source(cutoff); h.stop(1,begin,exit); h.compact(); h.reopen();
    // Current commit tick is already after all 3+ certified days. The first batch
    // consumes only its own endpoint, not the remaining actual preparation as no-class.
    let first=86401;
    let c=h.resolve_plan(1,begin,exit,first,cutoff,now,false).unwrap(); h.commit(c);
    assert_eq!(h.runtime.stopped_activity(1).unwrap().unwrap().consumed_through,t(first));
    assert_eq!(h.runtime.last_source().unwrap().occurred_at,t(now));
    assert!(h.runtime.awaiting_settlement(1).unwrap()); assert!(h.ledger.is_empty());
    h.compact(); h.reopen();
    assert_eq!(h.runtime.stopped_activity(1).unwrap().unwrap().known_actual_end,t(cutoff));
    let before=h.runtime.fingerprint().unwrap();
    // Cannot truncate a previously certified remaining interval and call it no-class.
    assert!(matches!(h.resolve_plan(1,begin,exit,first,first,now,true),Err(Error::Boundary)));
    assert_eq!(h.runtime.fingerprint().unwrap(),before);
    // A true >1-day remaining prefix must be batched, not silently accepted/truncated.
    assert!(matches!(h.resolve_plan(1,begin,exit,cutoff,cutoff,now,true),Err(Error::Boundary)));
    h.reopen();
    for endpoint in [172801,259201] {
        let c=h.resolve_plan(1,begin,exit,endpoint,cutoff,now,false).unwrap(); h.commit(c);
        assert!(h.runtime.awaiting_settlement(1).unwrap());
        assert_eq!(h.runtime.stopped_activity(1).unwrap().unwrap().consumed_through,t(endpoint));
        h.compact(); h.reopen();
    }
    let c=h.resolve_plan(1,begin,exit,cutoff,cutoff,now,true).unwrap(); h.commit(c);
    assert!(!h.runtime.awaiting_settlement(1).unwrap()); assert!(h.ledger.is_empty());
    let data=serde_json::to_value(&h.runtime).unwrap();
    assert_eq!(data["states"][0]["prepared_day"],3);
    assert_eq!(data["states"][0]["prepared_seconds"],13);
    assert_eq!(data["states"][0]["cohort"]["cursor"],now);
    assert_eq!(data["states"][0]["cohort"]["sequence"],9);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,0);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().fee_cents_per_learner,Money::ZERO);
    h.compact(); h.reopen();
}
#[test]
fn partial_midnight_settlement_keeps_original_claim_across_checkpoint_and_zero_close_has_no_credit() {
    let mut h = Host::new(false); h.activate(1); h.gap(1,1,82800);
    let prep = h.apply(1,Action::BeginPreparation,82800);
    h.apply(1,Action::SettlePreparation {started_event_id:prep.event_id,close:false},86400);
    assert_eq!(h.ledger[0].source.event_id,prep.event_id);
    assert_eq!(h.ledger[0].start,t(82800)); assert_eq!(h.ledger[0].settled_through,t(86400));
    h.compact(); assert_eq!(h.runtime.retained_activities(),1); h.reopen();
    h.apply(1,Action::SettlePreparation {started_event_id:prep.event_id,close:false},90000);
    h.apply(1,Action::SettlePreparation {started_event_id:prep.event_id,close:true},93600);
    assert!(h.ledger.is_empty());
    let lesson = h.apply(1,Action::BeginLesson,93600);
    h.apply(1,Action::SettleLesson {started_event_id:lesson.event_id,close:false},97200);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,3600);
    h.compact(); h.reopen();
    h.apply(1,Action::SettleLesson {started_event_id:lesson.event_id,close:true},97200);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,3600);
    assert!(!h.runtime.pending(1).unwrap()); assert!(h.ledger.is_empty());
}
#[test]
fn more_than_32_lifetime_events_continue_after_exact_prefix_compaction_and_old_stream_cannot_replay() {
    let mut h = Host::new(false); h.prepared();
    let mut first_end = None;
    // 40 real one-minute lesson intervals: literal 2400 seconds per learner, free.
    for n in 0..40 {
        if n%6 == 0 {h.compact();}
        let start = h.apply(1,Action::BeginLesson,7201+n*60);
        let ended = h.apply(1,Action::SettleLesson {started_event_id:start.event_id,close:true},7201+(n+1)*60);
        if first_end.is_none() {first_end=Some(ended);}
    }
    assert_eq!(h.runtime.consumed_sequence(),83);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,2400);
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().fee_cents_per_learner,Money::ZERO);
    h.compact(); h.reopen();
    let mut old = first_end.unwrap(); old.operation_id=99999; old.occurred_at=t(9601);
    assert!(matches!(h.runtime.recent_event(7,old.stream_sequence),Err(Error::Unknown)));
    assert!(matches!(h.plan_with(1,Action::NoInstruction {window:e::Window {start:t(9601),end:t(9602)}},old,None),Err(Error::Duplicate)));
    assert_eq!(h.runtime.progress(1).unwrap().unwrap().actual_seconds_per_learner,2400);
}
#[test]
fn wrong_clock_scope_epoch_sequence_and_qualification_are_rejected_without_mutation() {
    let mut h = Host::new(false); h.prepared();
    let before = h.runtime.fingerprint().unwrap();
    let source = h.source(7201);
    assert!(matches!(VerifiedEvent::after_system_host_validation(Command {cohort:1,action:Action::BeginLesson},source,t(7202)),Err(Error::Clock)));
    for mode in 0..5 {
        let mut bad = source;
        match mode {0=>bad.identity.branch=9,1=>bad.stream_epoch=8,2=>bad.stream_sequence+=1,
            3=>bad.root=[0;32],_=>bad.event_id=0}
        assert!(h.plan_with(1,Action::BeginLesson,bad,Some(h.venue())).is_err());
        assert_eq!(h.runtime.fingerprint().unwrap(),before);
    }
    let mut bad_venue = h.venue(); bad_venue.resource_id=999;
    assert!(matches!(h.plan_with(1,Action::BeginLesson,source,Some(bad_venue)),Err(Error::Unknown)));
    let mut bad_venue = h.venue(); bad_venue.members=members(1,4);
    assert!(matches!(h.plan_with(1,Action::BeginLesson,source,Some(bad_venue)),Err(Error::Unknown)));
}
#[test]
fn explicit_free_contract_and_exact_teacher_identity_are_registration_requirements() {
    let mut paid = opening(false); paid.courses[0].as_mut().unwrap().course.cents_per_hour=Money::new(1).unwrap();
    assert!(matches!(construct(paid),Err(Error::Invalid)));
    let mut missing = opening(false); missing.courses[0].as_mut().unwrap().free_contract.source_ref=0;
    assert!(matches!(construct(missing),Err(Error::Unknown)));
    let mut roles = opening(false); roles.courses[0].as_mut().unwrap().teacher=members(0,1);
    assert!(matches!(construct(roles),Err(Error::Invalid)));
    let input = opening(false); let fs=[input.courses[0].unwrap().free_contract]; let offers=[offer()];
    let witness = VerifiedOpening::after_root_opening_validation(identity(),input.fingerprint().unwrap(),input.registration_origin().unwrap(),[8;32],3,&offers,&[],&fs);
    assert!(matches!(Runtime::new(input,witness),Err(Error::Unknown)));
}
#[test]
fn failed_candidate_is_unpublished_stale_cas_rejected_and_serialized_state_needs_original_anchor() {
    let mut h = Host::new(false); h.prepared();
    let original_hash = h.runtime.fingerprint().unwrap();
    let c1 = h.plan(1,Action::BeginLesson,7201).unwrap();
    let c2 = h.plan(1,Action::BeginLesson,7201).unwrap();
    assert_eq!(h.runtime.fingerprint().unwrap(),original_hash); // prepare has no mutation
    h.commit(c1); let committed = h.runtime.fingerprint().unwrap();
    assert!(matches!(h.runtime.apply_candidate(c2),Err(Error::Cursor)));
    assert_eq!(h.runtime.fingerprint().unwrap(),committed);
    let data = serde_json::to_vec(&h.runtime).unwrap();
    let decoded:Runtime = serde_json::from_slice(&data).unwrap();
    let source = h.source(7202);
    let ev = VerifiedEvent::after_system_host_validation(Command {cohort:1,action:Action::NoInstruction {window:e::Window {start:t(7201),end:t(7202)}}},source,t(7202)).unwrap();
    assert!(matches!(decoded.plan(Snapshot::after_root_source_validation(ev,h.input.fingerprint().unwrap(),[8;32],None,&h.ledger,LedgerCoverage::EducationOnlyV1,None)),Err(Error::Unknown)));
    h.reopen();
    let mut value = serde_json::to_value(&h.runtime).unwrap();
    value["states"][0]["cohort"]["progress"]["actual_seconds_per_learner"] = 3600.into();
    let changed:Runtime = serde_json::from_value(value).unwrap();
    let auth = VerifiedRestore::after_original_store_validation(committed,h.input.fingerprint().unwrap(),h.input.registration_origin().unwrap(),h.runtime.prefix_anchor());
    assert!(matches!(changed.restore(auth,&h.ledger),Err(Error::Restore)));
}
#[test]
fn recent_event_capacity_reserves_a_necessary_close_and_compaction_preserves_ongoing_source() {
    let mut h = Host::new(false); h.activate(1);
    let started = h.apply(1,Action::BeginPreparation,1);
    // Two slots remain: StopForExit and final historical resolution, not only close.
    for n in 1..=28 {h.apply(1,Action::SettlePreparation {started_event_id:started.event_id,close:false},1+n);}
    let before = h.runtime.fingerprint().unwrap();
    assert!(matches!(h.plan(1,Action::SettlePreparation {started_event_id:started.event_id,close:false},30),Err(Error::NeedsCompaction)));
    assert_eq!(h.runtime.fingerprint().unwrap(),before);
    h.reopen();
    let exit=h.source(29); h.stop(1,started,exit); assert!(h.ledger.is_empty());
    assert!(h.runtime.awaiting_settlement(1).unwrap()); h.compact(); h.reopen();
    let final_batch=h.resolve_plan(1,started,exit,29,29,29,true).unwrap(); h.commit(final_batch);
    assert_eq!(h.runtime.consumed_sequence(),32); assert!(h.ledger.is_empty());
    h.compact(); h.reopen();
    h.gap(1,29,30);
    let again = h.apply(1,Action::BeginPreparation,30);
    h.apply(1,Action::SettlePreparation {started_event_id:again.event_id,close:false},31);
    h.compact(); h.reopen();
    assert_eq!(h.ledger[0].source.event_id,again.event_id);
    assert_eq!(h.ledger[0].start,t(30)); assert_eq!(h.ledger[0].settled_through,t(31));
    assert!(h.runtime.pending(1).unwrap());
}
#[test]
fn source_max_boundary_reserves_the_last_ordinal_for_existing_close() {
    let mut insufficient=Host::new(false); insufficient.activate(1); insufficient.synthetic_prefix_sequence(u64::MAX-2);
    // At early game time, MAX-1 Begin would strand a possible delayed Stop path.
    assert!(matches!(insufficient.plan(1,Action::BeginPreparation,1),Err(Error::Limit)));
    assert!(!insufficient.runtime.pending(1).unwrap());
    let mut h=Host::late_clock(false); let start=i64::MAX-86400;
    h.apply(1,Action::ActivateNoInstruction,start); h.synthetic_prefix_sequence(u64::MAX-4);
    let started=h.apply(1,Action::BeginPreparation,start); assert_eq!(started.stream_sequence,u64::MAX-3);
    let exit=h.source(i64::MAX); h.stop(1,started,exit);
    assert_eq!(h.runtime.consumed_sequence(),u64::MAX-2); h.compact(); h.reopen();
    let premature=h.plan(1,Action::NoInstruction {window:e::Window {start:t(start),end:t(i64::MAX)}},i64::MAX);
    assert!(premature.is_err()); // unrelated/intermediate commands cannot spend the tail
    let batch=h.resolve_plan(1,started,exit,i64::MAX,i64::MAX,i64::MAX,false).unwrap(); h.commit(batch);
    assert!(h.runtime.awaiting_settlement(1).unwrap()); h.compact(); h.reopen();
    let end=h.resolve_plan(1,started,exit,i64::MAX,i64::MAX,i64::MAX,true).unwrap(); h.commit(end);
    assert_eq!(h.runtime.consumed_sequence(),u64::MAX); assert!(h.ledger.is_empty());
    assert!(matches!(h.runtime.next_sequence(),Err(Error::Limit))); h.reopen();
}
#[test]
fn maximum_sequence_keeps_a_close_for_each_of_two_admitted_activities_after_restore() {
    let mut h=Host::late_clock(true); let start=i64::MAX-86400;
    h.apply(1,Action::ActivateNoInstruction,start); h.apply(2,Action::ActivateNoInstruction,start);
    h.synthetic_prefix_sequence(u64::MAX-8);
    let a=h.apply(1,Action::BeginPreparation,start);
    let b=h.apply(2,Action::BeginPreparation,start);
    assert_eq!(h.ledger.len(),2);
    assert!(matches!(h.plan(1,Action::SettlePreparation {started_event_id:a.event_id,close:false},start+1),Err(Error::Limit)));
    let exit=h.source(i64::MAX); h.stop(1,a,exit);
    let mut second=h.source(i64::MAX); second.operation_id=exit.operation_id; second.event_id=exit.event_id; second.revision=exit.revision;
    h.stop(2,b,second); assert!(h.ledger.is_empty()); h.compact(); h.reopen();
    for (cohort,begin) in [(1,a),(2,b)] {let c=h.resolve_plan(cohort,begin,exit,i64::MAX,i64::MAX,i64::MAX,false).unwrap(); h.commit(c);}
    h.compact(); h.reopen();
    for (cohort,begin) in [(1,a),(2,b)] {let c=h.resolve_plan(cohort,begin,exit,i64::MAX,i64::MAX,i64::MAX,true).unwrap(); h.commit(c);}
    assert_eq!(h.runtime.consumed_sequence(),u64::MAX); assert!(h.ledger.is_empty()); h.reopen();
}
#[test]
fn bounded_layout_and_encoding_are_future_checks_not_measured_acceptance() {
    let mut h = Host::new(false); h.prepared();
    assert!(Runtime::owned_bytes() <= 64*1024);
    assert!(Runtime::candidate_bytes() <= 4*1024);
    let bytes=serde_json::to_vec(&h.runtime).unwrap(); assert!(bytes.len()<=MAX_JSON_BYTES);
    h.compact(); h.reopen();
    println!("education runtime={} candidate={} encoded={}",Runtime::owned_bytes(),Runtime::candidate_bytes(),bytes.len());
}
