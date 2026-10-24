//! Synthetic source-contract fixtures only. No provider, Store or certificate acceptance.
//! Path inclusion deliberately avoids changing the shared lib.rs export surface.
pub use mycity_core::units;
#[path = "../src/education.rs"]
mod education;
use education::*;
use units::{GameTime, Money};

fn t(s: i64) -> GameTime { GameTime::new(s).unwrap() }
fn window(a: i64,b: i64) -> Window { Window { start:t(a),end:t(b) } }
fn view(at: i64) -> View {
    View { identity:Identity {world:1,branch:2,scope:3},root:[7;32],version:1,
        clock:Clock::Development,as_of:t(at),max_age_seconds:0,policy_ref:11 }
}
fn proof(v: View,w: Window,kind: Kind,subject: u64,binding: [u8;32]) -> Option<Fact> {
    Some(Fact {view:v,source_ref:99,subject,kind,coverage:w,observed_at:v.as_of,value:true,binding})
}
fn begin(path: Path,rate: i64) -> Cohort {
    let e = Enrollment {cohort:1,generation:1,roster_ref:2,class:3,school:4,
        learners:10,started:t(0),fee_contract_ref:5,obligations_ref:6};
    let c = Course {reference:10,path,cents_per_hour:Money::new(rate).unwrap()};
    let b = enrollment_binding(e,c);
    Cohort::begin(view(1),e,c,proof(view(1),window(0,1),Kind::Curriculum,10,b),
        proof(view(1),window(0,1),Kind::Roster,2,b)).unwrap()
}
fn attendance(c: &Cohort,end: i64,attended: bool) -> Attendance {
    let w = Window {start:c.cursor(),end:t(end)};
    let v = view(end);
    let mut a = Attendance {view:v,window:w,sequence:c.sequence()+1,teacher:8,remote:false,
        school_open:None,reservation:None,teacher_qualified:None,teacher_present:None,
        teacher_exclusive:None,preparation:None,actual_attendance:None,learner_exclusive:None,
        network:None,fee_service_authorized:None};
    let b = attendance_binding(c,&a);
    let e = c.enrollment();
    a.school_open = proof(v,w,Kind::SchoolOpen,e.school,b);
    a.reservation = proof(v,w,Kind::CapacityReservation,e.class,b);
    a.teacher_qualified = proof(v,w,Kind::TeacherQualified,8,b);
    a.teacher_present = proof(v,w,Kind::TeacherPresent,8,b);
    a.teacher_exclusive = proof(v,w,Kind::TeacherExclusive,8,b);
    a.preparation = proof(v,w,Kind::Preparation,8,b);
    a.actual_attendance = proof(v,w,Kind::ActualAttendance,e.roster_ref,b);
    a.actual_attendance.as_mut().unwrap().value = attended;
    a.learner_exclusive = proof(v,w,Kind::LearnerExclusive,e.roster_ref,b);
    a.fee_service_authorized = proof(v,w,Kind::FeeServiceAuthorized,e.fee_contract_ref,b);
    a
}
fn no_instruction(c: &Cohort,end: i64) -> NoInstruction {
    let mut input = NoInstruction {view:view(end),window:Window {start:c.cursor(),end:t(end)},
        sequence:c.sequence()+1,proof:None};
    let binding = no_instruction_binding(c,&input);
    input.proof = proof(input.view,input.window,Kind::NoInstruction,c.enrollment().cohort,binding);
    input
}
fn completion(c: &Cohort,at: i64,years: u16,covered: u32) -> Completion {
    let w = window(at,at+1);
    let v = view(at+1);
    let mut q = Completion {view:v,window:w,calendar:None,elapsed_academic_years:years,
        examination:None,covered_parts:covered,total_parts:100,prerequisite:None};
    let b = completion_binding(c,&q);
    q.calendar = proof(v,w,Kind::Calendar,c.enrollment().cohort,b);
    q.examination = proof(v,w,Kind::Examination,c.enrollment().cohort,b);
    q.prerequisite = proof(v,w,Kind::Prerequisite,c.enrollment().cohort,b);
    q
}
fn training() -> Cohort {
    let mut c = begin(Path::Retraining,1000);
    // Literal oracle: 60 real teaching days * 7200 s = 432000 s, 120 h.
    for day in 0..60 {
        if day > 0 {
            let a = attendance(&c,day*86400,false);
            c = advance(&c,a).unwrap().next;
        }
        let a = attendance(&c,day*86400+7200,true);
        c = advance(&c,a).unwrap().next;
    }
    c
}
#[test]
fn literal_hours_fee_and_calendar_are_distinct_gates() {
    let c = training();
    assert_eq!(c.progress(),Progress {actual_seconds_per_learner:432000,actual_teaching_days:60,
        fee_cents_per_learner:Money::new(120000).unwrap(),fee_remainder:0});
    assert!(qualification(&c,completion(&c,89*86400,0,100)).unwrap().is_none());
    assert!(qualification(&c,completion(&c,90*86400,0,89)).unwrap().is_none());
    let q = qualification(&c,completion(&c,90*86400,0,90)).unwrap().unwrap();
    assert_eq!((q.cohort,q.generation,q.course_ref,q.examination_source),(1,1,10,99));
    let untouched = begin(Path::Retraining,1000);
    assert!(qualification(&untouched,completion(&untouched,900*86400,0,100)).unwrap().is_none());
}
#[test]
fn free_school_and_fractional_fees_do_not_create_hours_or_cash() {
    let c = begin(Path::Retraining,0);
    let u = advance(&c,attendance(&c,7200,true)).unwrap();
    assert!(u.matches(&c,[7;32]));
    assert!(!u.matches(&c.with_status(Status::Paused),[7;32]));
    assert!(!u.matches(&c,[9;32]));
    assert_eq!(u.fee_delta,Money::ZERO);
    assert_eq!(u.learner_seconds,72000);
    assert_eq!(u.next.progress().actual_seconds_per_learner,7200);
    let c = begin(Path::Retraining,7);
    let whole = advance(&c,attendance(&c,3600,true)).unwrap();
    let first = advance(&c,attendance(&c,1801,true)).unwrap();
    assert_eq!(first.next.progress().fee_cents_per_learner.value(),3);
    assert_eq!(first.next.progress().fee_remainder,1807);
    let resumed = first.next; // in-memory copy, explicitly not durable restoration
    let second = advance(&resumed,attendance(&resumed,3600,true)).unwrap();
    assert_eq!(second.next.progress(),whole.next.progress());
    assert_eq!(first.fee_delta.value()+second.fee_delta.value(),70);
}
#[test]
fn missing_stale_wrong_root_scope_or_incomplete_sources_are_unknown() {
    let c = begin(Path::Retraining,0);
    for mode in 0..7 {
        let mut a = attendance(&c,3600,true);
        match mode {
            0 => a.teacher_present = None,
            1 => a.teacher_present.as_mut().unwrap().view.root = [9;32],
            2 => a.teacher_present.as_mut().unwrap().view.identity.scope = 7,
            3 => a.teacher_present.as_mut().unwrap().coverage.end = t(3599),
            4 => a.teacher_present.as_mut().unwrap().observed_at = t(3599),
            5 => a.teacher_present.as_mut().unwrap().view.version = 2,
            _ => a.teacher_present.as_mut().unwrap().binding = [0;32],
        }
        assert_eq!(advance(&c,a),Err(Error::Unknown(Kind::TeacherPresent)));
        assert_eq!(c.cursor(),t(0));
    }
    let mut a = attendance(&c,3600,true);
    a.view.as_of = t(3602);
    // Update all source view envelopes, but retain old observations, yielding stale facts.
    for f in [&mut a.school_open,&mut a.reservation,&mut a.teacher_qualified,
        &mut a.teacher_present,&mut a.teacher_exclusive,&mut a.preparation,
        &mut a.actual_attendance,&mut a.learner_exclusive,&mut a.fee_service_authorized] {
        f.as_mut().unwrap().view = a.view;
    }
    assert_eq!(advance(&c,a),Err(Error::Unknown(Kind::SchoolOpen)));
}
#[test]
fn no_teacher_conflicting_work_travel_closure_or_arrears_cannot_count_learning() {
    let c = begin(Path::Retraining,1000);
    for kind in [Kind::SchoolOpen,Kind::TeacherQualified,Kind::TeacherPresent,
        Kind::TeacherExclusive,Kind::Preparation,Kind::ActualAttendance,
        Kind::LearnerExclusive,Kind::FeeServiceAuthorized,Kind::CapacityReservation] {
        let mut a = attendance(&c,7200,true);
        let f = match kind {
            Kind::SchoolOpen => &mut a.school_open,
            Kind::TeacherQualified => &mut a.teacher_qualified,
            Kind::TeacherPresent => &mut a.teacher_present,
            Kind::TeacherExclusive => &mut a.teacher_exclusive,
            Kind::Preparation => &mut a.preparation,
            Kind::ActualAttendance => &mut a.actual_attendance,
            Kind::LearnerExclusive => &mut a.learner_exclusive,
            Kind::FeeServiceAuthorized => &mut a.fee_service_authorized,
            _ => &mut a.reservation,
        };
        f.as_mut().unwrap().value = false;
        let u = advance(&c,a).unwrap();
        assert_eq!(u.outcome,Outcome::Held(kind));
        assert_eq!(u.next.progress(),c.progress());
        assert_eq!(u.fee_delta,Money::ZERO);
        assert_eq!(u.next.cursor(),t(7200));
    }
    let mut a = attendance(&c,7200,true);
    a.remote = true;
    // A remote flag change invalidates the old business binding before network lookup.
    assert_eq!(advance(&c,a),Err(Error::Unknown(Kind::SchoolOpen)));
    let b = attendance_binding(&c,&a);
    for f in [&mut a.school_open,&mut a.reservation,&mut a.teacher_qualified,
        &mut a.teacher_present,&mut a.teacher_exclusive,&mut a.preparation,
        &mut a.actual_attendance,&mut a.learner_exclusive,&mut a.fee_service_authorized] {
        f.as_mut().unwrap().binding = b;
    }
    assert_eq!(advance(&c,a),Err(Error::Unknown(Kind::Network)));
    a.network = proof(a.view,a.window,Kind::Network,c.enrollment().class,b);
    a.network.as_mut().unwrap().value = false;
    assert_eq!(advance(&c,a).unwrap().outcome,Outcome::Held(Kind::Network));
}
#[test]
fn late_arrival_hard_daily_limit_midnight_and_replay_are_not_averaged() {
    let c = begin(Path::Retraining,1000);
    let absent = advance(&c,attendance(&c,3600,false)).unwrap().next;
    let a = attendance(&absent,7200,true);
    let u = advance(&absent,a).unwrap();
    assert_eq!(u.next.progress().actual_seconds_per_learner,3600);
    assert_eq!(u.fee_delta.value(),10000);
    assert_eq!(advance(&u.next,a),Err(Error::Cursor));
    let full = advance(&c,attendance(&c,7200,true)).unwrap().next;
    assert_eq!(advance(&full,attendance(&full,7201,true)),Err(Error::Boundary));
    assert_eq!(advance(&c,attendance(&c,86401,false)),Err(Error::Boundary));
}
#[test]
fn pause_transfer_preserves_original_time_carry_and_obligations() {
    let c = begin(Path::Retraining,7);
    let c = advance(&c,attendance(&c,1801,true)).unwrap().next;
    let paused = c.with_status(Status::ClosedAwaitingTransfer);
    let u = advance(&paused,attendance(&paused,3600,true)).unwrap();
    assert_eq!(u.outcome,Outcome::HeldStatus(Status::ClosedAwaitingTransfer));
    assert_eq!(u.next.progress(),c.progress());
    let c = u.next;
    let b = transfer_binding(&c,20,30);
    let p = proof(view(3601),window(3600,3601),Kind::Transfer,1,b);
    assert_eq!(c.transfer(view(3601),21,30,p),Err(Error::Unknown(Kind::Transfer)));
    let moved = c.transfer(view(3601),20,30,p).unwrap();
    assert_eq!(moved.progress(),c.progress());
    assert_eq!(moved.cursor(),c.cursor());
    assert_eq!(moved.enrollment().started,t(0));
    assert_eq!(moved.enrollment().obligations_ref,6);
    assert_eq!(moved.enrollment().fee_contract_ref,5);
    assert_eq!(moved.enrollment().roster_ref,2);
}
#[test]
fn no_teacher_or_reservation_multiday_pause_then_resume_only_counts_actual_lesson() {
    let c = begin(Path::Retraining,7);
    let first = advance(&c,attendance(&c,1801,true)).unwrap();
    let paused = first.next.with_status(Status::Paused);
    // This input has no teacher, reservation, preparation, attendance or fee-service fields.
    let gap = no_instruction(&paused,3*86400);
    let u = advance_no_instruction(&paused,gap).unwrap();
    assert_eq!(u.outcome,Outcome::Held(Kind::NoInstruction));
    assert_eq!(u.fee_delta,Money::ZERO);
    assert_eq!(u.learner_seconds,0);
    assert_eq!(u.next.progress(),paused.progress());
    assert_eq!(u.next.enrollment(),paused.enrollment());
    assert_eq!(u.next.status(),Status::Paused);
    assert_eq!(u.next.cursor(),t(3*86400));
    assert_eq!(u.next.sequence(),paused.sequence()+1);
    assert!(u.matches(&paused,[7;32]));
    assert!(!u.matches(&paused.with_status(Status::Active),[7;32]));
    assert_eq!(advance_no_instruction(&u.next,gap),Err(Error::Cursor));
    let resumed = u.next.with_status(Status::Active); // original writer must authorize resume
    let lesson = advance(&resumed,attendance(&resumed,3*86400+1799,true)).unwrap();
    assert_eq!(lesson.learner_seconds,17990);
    assert_eq!(lesson.next.progress(),Progress {actual_seconds_per_learner:3600,
        actual_teaching_days:2,fee_cents_per_learner:Money::new(7).unwrap(),fee_remainder:0});
    assert_eq!(first.fee_delta.value()+u.fee_delta.value()+lesson.fee_delta.value(),70);
    assert_eq!(lesson.next.enrollment().started,t(0));
    assert_eq!(lesson.next.enrollment().obligations_ref,6);
}
#[test]
fn closed_school_multiday_gap_then_legal_transfer_preserves_progress_and_fee_carry() {
    let c = begin(Path::Retraining,7);
    let first = advance(&c,attendance(&c,1801,true)).unwrap();
    let closed = first.next.with_status(Status::ClosedAwaitingTransfer);
    let gap = no_instruction(&closed,4*86400);
    let u = advance_no_instruction(&closed,gap).unwrap();
    assert_eq!(u.next.status(),Status::ClosedAwaitingTransfer);
    assert_eq!(u.next.progress(),closed.progress());
    let at = 4*86400;
    let binding = transfer_binding(&u.next,20,30);
    let p = proof(view(at+1),window(at,at+1),Kind::Transfer,1,binding);
    let moved = u.next.transfer(view(at+1),20,30,p).unwrap();
    assert_eq!(moved.progress(),closed.progress());
    assert_eq!(moved.enrollment().fee_contract_ref,5);
    assert_eq!(moved.enrollment().obligations_ref,6);
    assert_eq!(moved.enrollment().started,t(0));
    assert_eq!(moved.enrollment().roster_ref,2);
    assert_eq!(moved.cursor(),t(at));
    let a = attendance(&moved,at+1799,true);
    let lesson = advance(&moved,a).unwrap();
    assert_eq!(lesson.next.progress().actual_seconds_per_learner,3600);
    assert_eq!(lesson.next.progress().fee_cents_per_learner.value(),7);
    assert_eq!(lesson.next.progress().fee_remainder,0);
    assert_eq!(u.learner_seconds,0);
    assert_eq!(first.fee_delta.value()+u.fee_delta.value()+lesson.fee_delta.value(),70);
    assert_eq!(advance(&lesson.next,a),Err(Error::Cursor));
}
#[test]
fn no_instruction_requires_complete_current_bound_evidence_even_while_paused() {
    let c = begin(Path::Retraining,7).with_status(Status::Paused);
    let end = 5*86400;
    for mode in 0..10 {
        let mut input = no_instruction(&c,end);
        match mode {
            0 => input.proof = None,
            1 => input.proof.as_mut().unwrap().view.root = [9;32],
            2 => input.proof.as_mut().unwrap().coverage.end = t(end-1),
            3 => input.proof.as_mut().unwrap().binding = [0;32],
            4 => input.proof.as_mut().unwrap().observed_at = t(end-1),
            5 => input.proof.as_mut().unwrap().subject = 99,
            6 => input.proof.as_mut().unwrap().kind = Kind::SchoolOpen,
            7 => input.proof.as_mut().unwrap().view.version = 2,
            8 => input.proof.as_mut().unwrap().source_ref = 0,
            _ => input.proof.as_mut().unwrap().coverage.start = t(1),
        }
        assert_eq!(advance_no_instruction(&c,input),Err(Error::Unknown(Kind::NoInstruction)));
        assert_eq!(c.cursor(),t(0));
    }
    let mut input = no_instruction(&c,end);
    input.proof.as_mut().unwrap().value = false;
    assert_eq!(advance_no_instruction(&c,input),Err(Error::Invalid));
    let mut input = no_instruction(&c,end);
    input.view.identity.branch = 20;
    assert_eq!(advance_no_instruction(&c,input),Err(Error::Invalid));
    let mut input = no_instruction(&c,end);
    input.sequence += 1;
    assert_eq!(advance_no_instruction(&c,input),Err(Error::Cursor));
    let mut input = no_instruction(&c,end);
    input.window.start = t(1);
    assert_eq!(advance_no_instruction(&c,input),Err(Error::Cursor));
    let mut input = no_instruction(&c,end);
    input.window.end = input.window.start;
    assert_eq!(advance_no_instruction(&c,input),Err(Error::Window));
}
#[test]
fn zero_instruction_keeps_day_counter_and_never_bypasses_normal_teaching_sources() {
    let c = begin(Path::Retraining,7);
    let full = advance(&c,attendance(&c,7200,true)).unwrap().next;
    // Same-day gap must not reset the two-hour cap or manufacture another teaching day.
    let u = advance_no_instruction(&full,no_instruction(&full,14400)).unwrap();
    assert_eq!(u.next.progress(),full.progress());
    assert_eq!(advance(&u.next,attendance(&u.next,14401,true)),Err(Error::Boundary));
    let active = advance_no_instruction(&full,no_instruction(&full,2*86400)).unwrap().next;
    let mut a = attendance(&active,2*86400+3600,true);
    a.teacher = 0;
    assert_eq!(advance(&active,a),Err(Error::Invalid));
    let mut a = attendance(&active,2*86400+3600,true);
    a.teacher_present = None;
    assert_eq!(advance(&active,a),Err(Error::Unknown(Kind::TeacherPresent)));
    let mut a = attendance(&active,2*86400+3600,true);
    a.reservation = None;
    assert_eq!(advance(&active,a),Err(Error::Unknown(Kind::CapacityReservation)));
    // A no-instruction proof cannot be substituted for normal attendance evidence.
    let mut a = attendance(&active,2*86400+3600,true);
    a.actual_attendance = no_instruction(&active,2*86400+3600).proof;
    assert_eq!(advance(&active,a),Err(Error::Unknown(Kind::ActualAttendance)));
}
#[test]
fn academic_rules_keep_four_years_and_proven_calendar_separate_from_training() {
    let c = begin(Path::Basic,0);
    assert!(qualification(&c,completion(&c,2000*86400,4,100)).unwrap().is_none());
    let rules = [(Path::Basic,4,11520000),(Path::Secondary,4,11520000),
        (Path::Vocational,1,2880000),(Path::University,3,8640000),
        (Path::Retraining,0,432000),(Path::Endorsement,0,864000)];
    for (path,years,seconds) in rules {
        let course = Course {reference:1,path,cents_per_hour:Money::ZERO};
        assert_eq!(course.academic_years(),years);
        assert_eq!(course.required_seconds(),seconds);
    }
    assert_eq!(TEACHING_DAYS_PER_YEAR,200);
}
#[test]
fn four_year_reference_needs_real_hours_and_original_calendar_proof() {
    let mut c = begin(Path::Basic,0);
    for day in 0..800 {
        if day > 0 { c = advance(&c,attendance(&c,day*86400,false)).unwrap().next; }
        c = advance(&c,attendance(&c,day*86400+14400,true)).unwrap().next;
    }
    assert_eq!(c.progress().actual_seconds_per_learner,11520000);
    assert_eq!(c.progress().actual_teaching_days,800);
    assert!(qualification(&c,completion(&c,1500*86400,3,100)).unwrap().is_none());
    assert!(qualification(&c,completion(&c,1500*86400,4,90)).unwrap().is_some());
    let mut q = completion(&c,1500*86400,3,100);
    q.elapsed_academic_years = 4;
    assert_eq!(qualification(&c,q),Err(Error::Unknown(Kind::Calendar)));
}
#[test]
fn missing_failed_exam_and_prerequisite_cannot_issue_a_candidate() {
    let c = training();
    let mut q = completion(&c,90*86400,0,100);
    q.examination = None;
    assert_eq!(qualification(&c,q),Err(Error::Unknown(Kind::Examination)));
    let mut q = completion(&c,90*86400,0,100);
    q.prerequisite.as_mut().unwrap().value = false;
    assert!(qualification(&c,q).unwrap().is_none());
    let mut q = completion(&c,90*86400,0,100);
    q.examination.as_mut().unwrap().value = false;
    assert!(qualification(&c,q).unwrap().is_none());
}
#[test]
fn bounded_inline_shapes_are_only_a_future_layout_assertion() {
    assert!(std::mem::size_of::<Cohort>() <= 512);
    assert!(std::mem::size_of::<Attendance>() + 2*std::mem::size_of::<Update>() <= 8192);
}
