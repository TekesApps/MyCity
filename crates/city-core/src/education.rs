//! Issue #54 bounded, deterministic education proposals. Not an authority or proof verifier.
//! A trusted adapter must authenticate every source, reservation and exclusive-time claim.
//! No Engine/Store, certificate publication, money movement or capacity pool lives here.
use crate::units::{GameTime, Money};
use sha2::{Digest, Sha256};

pub const VERSION: u32 = 1;
pub const MAX_CLASS: u16 = 30;
pub const TEACHING_DAYS_PER_YEAR: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock { Development }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity { pub world: u64, pub branch: u64, pub scope: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window { pub start: GameTime, pub end: GameTime }
impl Window {
    fn seconds(self) -> Result<u64, Error> {
        if self.start >= self.end { return Err(Error::Window); }
        Ok((self.end.value() - self.start.value()) as u64)
    }
}
/// Caller freezes the age policy under policy_ref; no host clock or silent default age.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub identity: Identity,
    pub root: [u8; 32],
    pub version: u32,
    pub clock: Clock,
    pub as_of: GameTime,
    pub max_age_seconds: u64,
    pub policy_ref: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Curriculum, Roster, SchoolOpen, CapacityReservation, TeacherQualified,
    TeacherPresent, TeacherExclusive, Preparation, ActualAttendance,
    LearnerExclusive, Network, FeeServiceAuthorized, Calendar, Examination,
    Prerequisite, Transfer, NoInstruction,
}
/// Opaque references are not signatures. Boolean false is a known observation;
/// None, wrong scope/root/version/coverage/age is Unknown, never a real shortage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    pub view: View,
    pub source_ref: u64,
    pub subject: u64,
    pub kind: Kind,
    pub coverage: Window,
    pub observed_at: GameTime,
    pub value: bool,
    /// Hash of the exact business query. Authenticated source must attest this binding.
    pub binding: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Unknown(Kind), Invalid, Window, Cursor, Boundary, Overflow }
fn fact(input: Option<Fact>, view: View, window: Window, kind: Kind, subject: u64, binding: [u8;32]) -> Result<bool, Error> {
    window.seconds()?;
    let f = input.ok_or(Error::Unknown(kind))?;
    if view.version != VERSION || view.policy_ref == 0 || view.root == [0; 32]
        || view.identity.world == 0 || view.identity.branch == 0 || view.identity.scope == 0
        || f.view != view || f.kind != kind || f.subject != subject || f.source_ref == 0 || f.binding != binding
        || f.coverage.start > window.start || f.coverage.end < window.end
        || f.observed_at < window.end || f.observed_at > view.as_of
        || (view.as_of.value() - f.observed_at.value()) as u64 > view.max_age_seconds
    { return Err(Error::Unknown(kind)); }
    Ok(f.value)
}
fn digest(domain: &[u8], words: &[u64]) -> [u8;32] {
    let mut h = Sha256::new();
    h.update(b"mycity-education-query-v1");
    h.update(domain);
    for word in words { h.update(word.to_le_bytes()); }
    h.finalize().into()
}
pub fn enrollment_binding(e: Enrollment, c: Course) -> [u8;32] {
    digest(b"enrollment", &[e.cohort,e.generation,e.roster_ref,e.class,e.school,
        u64::from(e.learners),e.started.value() as u64,e.fee_contract_ref,e.obligations_ref,
        c.reference,c.path as u64,c.cents_per_hour.value() as u64])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path { Basic, Secondary, Vocational, University, Retraining, Endorsement }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Course {
    pub reference: u64,
    pub path: Path,
    /// Exact cents per actual learner-hour; zero is legal free/subsidized education.
    pub cents_per_hour: Money,
}
impl Course {
    pub fn academic_years(self) -> u16 {
        match self.path { Path::Basic | Path::Secondary => 4, Path::Vocational => 1,
            Path::University => 3, _ => 0 }
    }
    pub fn required_seconds(self) -> u64 {
        match self.path { Path::Retraining => 120 * 3600, Path::Endorsement => 240 * 3600,
            // L03 synthetic full-day reference: 200 teaching days/year, four hours/day.
            // This reference is not a real school curriculum or graduation certificate.
            _ => u64::from(self.academic_years()) * u64::from(TEACHING_DAYS_PER_YEAR) * 4 * 3600 }
    }
    pub fn minimum_days(self) -> u64 {
        match self.path { Path::Retraining => 90, Path::Endorsement => 180, _ => 0 }
    }
    fn daily_seconds(self) -> u64 {
        if self.path == Path::Retraining { 2 * 3600 } else { 4 * 3600 }
    }
    fn seats(self) -> u16 { if self.path == Path::Retraining { 10 } else { MAX_CLASS } }
}

/// One homogeneous cohort, not one resident agent. roster_ref resolves exact membership
/// externally. Different progress, fees, attendance or ownership requires an original-writer
/// conservative split, which this module deliberately does not implement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enrollment {
    pub cohort: u64,
    pub generation: u64,
    pub roster_ref: u64,
    pub class: u64,
    pub school: u64,
    pub learners: u16,
    pub started: GameTime,
    pub fee_contract_ref: u64,
    pub obligations_ref: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub actual_seconds_per_learner: u64,
    pub actual_teaching_days: u32,
    pub fee_cents_per_learner: Money,
    pub fee_remainder: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status { Active, Paused, ClosedAwaitingTransfer }
/// Private fields prevent arbitrary progress reset/injection via the public API.
/// Clone is only an in-memory continuation candidate, NOT durable Store recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cohort {
    identity: Identity,
    enrollment: Enrollment,
    course: Course,
    progress: Progress,
    cursor: GameTime,
    sequence: u64,
    day: i64,
    day_seconds: u64,
    status: Status,
    last_root: [u8; 32],
}
impl Cohort {
    pub fn begin(view: View, enrollment: Enrollment, course: Course,
        curriculum: Option<Fact>, roster: Option<Fact>) -> Result<Self, Error> {
        let e = enrollment;
        if [e.cohort,e.generation,e.roster_ref,e.class,e.school,e.fee_contract_ref,e.obligations_ref,course.reference]
            .contains(&0) || e.learners == 0 || e.learners > course.seats() { return Err(Error::Invalid); }
        let end = GameTime::new(e.started.value().checked_add(1).ok_or(Error::Overflow)?)
            .map_err(|_| Error::Overflow)?;
        let w = Window { start: e.started, end };
        let binding = enrollment_binding(e,course);
        if !fact(curriculum,view,w,Kind::Curriculum,course.reference,binding)?
            || !fact(roster,view,w,Kind::Roster,e.roster_ref,binding)? { return Err(Error::Invalid); }
        Ok(Self { identity: view.identity, enrollment:e, course,
            progress:Progress { actual_seconds_per_learner:0,actual_teaching_days:0,
                fee_cents_per_learner:Money::ZERO,fee_remainder:0 },
            cursor:e.started,sequence:0,day:-1,day_seconds:0,status:Status::Active,last_root:view.root })
    }
    pub fn progress(&self) -> Progress { self.progress }
    pub fn enrollment(&self) -> Enrollment { self.enrollment }
    pub fn cursor(&self) -> GameTime { self.cursor }
    pub fn sequence(&self) -> u64 { self.sequence }
    pub fn status(&self) -> Status { self.status }
    /// Suspension/closure is a proposal only. It retains all progress, fees and obligation refs.
    /// Permission and mandatory notice/transfer delivery remain original writer responsibilities.
    pub fn with_status(self, status: Status) -> Self { Self { status, ..self } }
    /// No attendance or deadline is inferred during pauses/closed-city gaps.
    pub fn transfer(self, view: View, school: u64, class: u64, proof: Option<Fact>) -> Result<Self, Error> {
        if view.identity != self.identity || school == 0 || class == 0 { return Err(Error::Invalid); }
        let end = GameTime::new(self.cursor.value().checked_add(1).ok_or(Error::Overflow)?)
            .map_err(|_| Error::Overflow)?;
        if !fact(proof,view,Window {start:self.cursor,end},Kind::Transfer,self.enrollment.cohort,
            transfer_binding(&self,school,class))? {
            return Err(Error::Invalid);
        }
        // Same course, cohort/generation, roster, original start, cursor, carry and obligations.
        let mut result = self;
        result.enrollment.school = school;
        result.enrollment.class = class;
        result.status = Status::Active;
        result.last_root = view.root;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attendance {
    pub view: View,
    pub window: Window,
    pub sequence: u64,
    pub teacher: u64,
    pub remote: bool,
    /// Exact class/teacher/day and roster-bound claims from trusted source adapters.
    pub school_open: Option<Fact>,
    pub reservation: Option<Fact>,
    pub teacher_qualified: Option<Fact>,
    pub teacher_present: Option<Fact>,
    pub teacher_exclusive: Option<Fact>,
    /// At least two actual preparation hours for this teacher/class/day, disjoint from
    /// teaching/work/travel. Provider must enforce total teacher daily/weekly employment.
    pub preparation: Option<Fact>,
    pub actual_attendance: Option<Fact>,
    /// Same roster, entire interval: no overlapping work, travel or another course.
    pub learner_exclusive: Option<Fact>,
    pub network: Option<Fact>,
    /// Lawful service can be free/paid/subsidized; arrears policy is a separate fact.
    pub fee_service_authorized: Option<Fact>,
}
/// A trusted source attests that this entire cohort received no instruction in the
/// complete interval. Paused status alone is insufficient. This is not a request
/// to cancel teaching, release capacity, waive debt or discard accepted obligations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoInstruction {
    pub view: View,
    pub window: Window,
    pub sequence: u64,
    pub proof: Option<Fact>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Credited, Held(Kind), HeldStatus(Status) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Update {
    pub expected_sequence: u64,
    pub expected_cursor: GameTime,
    pub expected_root: [u8; 32],
    pub expected_state: [u8;32],
    pub next: Cohort,
    pub outcome: Outcome,
    /// Accrual candidate only; not a receipt, payment or existing debt replacement.
    pub fee_delta: Money,
    pub learner_seconds: u64,
}
impl Update {
    /// Full state fingerprint also catches a transfer/status change with the same cursor.
    /// This helper grants no authority and performs no durable commit.
    pub fn matches(&self, current: &Cohort, root: [u8;32]) -> bool {
        self.expected_root == root && self.expected_state == state_binding(current)
    }
}
fn state_binding(c: &Cohort) -> [u8;32] {
    let mut h = Sha256::new();
    h.update(enrollment_binding(c.enrollment,c.course));
    h.update(digest(b"state", &[c.identity.world,c.identity.branch,c.identity.scope,
        c.cursor.value() as u64,c.sequence,c.progress.actual_seconds_per_learner,
        u64::from(c.progress.actual_teaching_days),c.progress.fee_cents_per_learner.value() as u64,
        u64::from(c.progress.fee_remainder),c.day as u64,c.day_seconds,c.status as u64]));
    h.update(c.last_root);
    h.finalize().into()
}
pub fn attendance_binding(c: &Cohort, a: &Attendance) -> [u8;32] {
    let mut h = Sha256::new();
    h.update(state_binding(c));
    h.update(digest(b"attendance", &[a.window.start.value() as u64,a.window.end.value() as u64,
        a.sequence,a.teacher,u64::from(a.remote)]));
    h.finalize().into()
}
pub fn transfer_binding(c: &Cohort, school: u64, class: u64) -> [u8;32] {
    let mut h = Sha256::new();
    h.update(state_binding(c));
    h.update(digest(b"transfer", &[school,class]));
    h.finalize().into()
}
pub fn no_instruction_binding(c: &Cohort, input: &NoInstruction) -> [u8;32] {
    let mut h = Sha256::new();
    h.update(state_binding(c));
    h.update(digest(b"no-instruction", &[input.window.start.value() as u64,
        input.window.end.value() as u64,input.sequence]));
    h.finalize().into()
}
/// Zero-accrual proposal for a proven no-instruction interval, including multiple
/// midnights. No teacher or reservation is required because no teaching is credited.
/// The original writer must authenticate the whole interval and CAS this Update
/// exactly as for advance; missing evidence leaves the original cursor unchanged.
pub fn advance_no_instruction(current: &Cohort, input: NoInstruction) -> Result<Update, Error> {
    input.window.seconds()?;
    if input.view.identity != current.identity { return Err(Error::Invalid); }
    if input.window.start != current.cursor
        || input.sequence != current.sequence.checked_add(1).ok_or(Error::Overflow)? {
        return Err(Error::Cursor);
    }
    if !fact(input.proof,input.view,input.window,Kind::NoInstruction,current.enrollment.cohort,
        no_instruction_binding(current,&input))? { return Err(Error::Invalid); }
    let mut next = *current;
    // Deliberately preserve progress, carry, day/day_seconds, enrollment and status.
    next.cursor = input.window.end;
    next.sequence = input.sequence;
    next.last_root = input.view.root;
    Ok(Update { expected_sequence:current.sequence,expected_cursor:current.cursor,
        expected_root:input.view.root,expected_state:state_binding(current),next,
        outcome:Outcome::Held(Kind::NoInstruction),fee_delta:Money::ZERO,learner_seconds:0 })
}
/// Pure compare-and-commit proposal. Caller must CAS the original progress AND source root,
/// recheck authority, and atomically persist progress/cursor/fee carry/accrual once.
/// Unknown inputs never consume the event; known failed delivery consumes time but adds no hours.
pub fn advance(current: &Cohort, input: Attendance) -> Result<Update, Error> {
    let w = input.window;
    let seconds = w.seconds()?;
    if input.view.identity != current.identity || input.teacher == 0 { return Err(Error::Invalid); }
    if w.start != current.cursor || input.sequence != current.sequence.checked_add(1).ok_or(Error::Overflow)? {
        return Err(Error::Cursor);
    }
    let day = w.start.value() / 86400;
    if (w.end.value() - 1) / 86400 != day { return Err(Error::Boundary); }
    let e = current.enrollment;
    let binding = attendance_binding(current,&input);
    // Validate every required fact before proposing any mutation, including known holds.
    let checks = [
        (input.school_open,Kind::SchoolOpen,e.school),
        (input.reservation,Kind::CapacityReservation,e.class),
        (input.teacher_qualified,Kind::TeacherQualified,input.teacher),
        (input.teacher_present,Kind::TeacherPresent,input.teacher),
        (input.teacher_exclusive,Kind::TeacherExclusive,input.teacher),
        (input.preparation,Kind::Preparation,input.teacher),
        (input.actual_attendance,Kind::ActualAttendance,e.roster_ref),
        (input.learner_exclusive,Kind::LearnerExclusive,e.roster_ref),
        (input.fee_service_authorized,Kind::FeeServiceAuthorized,e.fee_contract_ref),
    ];
    let mut hold = None;
    for (f,kind,subject) in checks {
        if !fact(f,input.view,w,kind,subject,binding)? && hold.is_none() { hold = Some(kind); }
    }
    if input.remote && !fact(input.network,input.view,w,Kind::Network,e.class,binding)? && hold.is_none() {
        hold = Some(Kind::Network);
    }
    let mut next = *current;
    next.cursor = w.end;
    next.sequence = input.sequence;
    next.last_root = input.view.root;
    let outcome = if current.status != Status::Active { Outcome::HeldStatus(current.status) }
        else if let Some(kind) = hold { Outcome::Held(kind) } else { Outcome::Credited };
    let mut fee_delta = Money::ZERO;
    let mut learner_seconds = 0;
    if outcome == Outcome::Credited {
        let previous = if current.day == day { current.day_seconds } else { 0 };
        next.day_seconds = previous.checked_add(seconds).ok_or(Error::Overflow)?;
        if next.day_seconds > current.course.daily_seconds() { return Err(Error::Boundary); }
        if previous == 0 {
            next.progress.actual_teaching_days = current.progress.actual_teaching_days.checked_add(1).ok_or(Error::Overflow)?;
        }
        next.day = day;
        next.progress.actual_seconds_per_learner = current.progress.actual_seconds_per_learner
            .checked_add(seconds).ok_or(Error::Overflow)?;
        let numerator = u128::from(seconds) * (current.course.cents_per_hour.value() as u128)
            + u128::from(current.progress.fee_remainder);
        let cents = i64::try_from(numerator / 3600).map_err(|_| Error::Overflow)?;
        next.progress.fee_remainder = (numerator % 3600) as u16;
        next.progress.fee_cents_per_learner = current.progress.fee_cents_per_learner
            .checked_add(Money::new(cents).map_err(|_| Error::Overflow)?).map_err(|_| Error::Overflow)?;
        // Carry is per learner: splitting a time window cannot round the household bill twice.
        fee_delta = Money::new(cents).map_err(|_| Error::Overflow)?
            .checked_mul(u64::from(e.learners)).map_err(|_| Error::Overflow)?;
        learner_seconds = seconds.checked_mul(u64::from(e.learners)).ok_or(Error::Overflow)?;
    }
    Ok(Update { expected_sequence:current.sequence,expected_cursor:current.cursor,
        expected_root:input.view.root,expected_state:state_binding(current),next,outcome,fee_delta,learner_seconds })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub view: View,
    pub window: Window,
    pub calendar: Option<Fact>,
    /// Proven whole academic years since ORIGINAL enrollment using the existing school
    /// calendar (200 teaching days/year). No Gregorian-year or housing-level substitution.
    pub elapsed_academic_years: u16,
    pub examination: Option<Fact>,
    pub covered_parts: u32,
    pub total_parts: u32,
    pub prerequisite: Option<Fact>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QualificationCandidate {
    pub identity: Identity,
    pub root: [u8;32],
    pub cohort: u64,
    pub generation: u64,
    pub roster_ref: u64,
    pub course_ref: u64,
    pub sequence: u64,
    pub calendar_source: u64,
    pub examination_source: u64,
    pub prerequisite_source: u64,
    pub as_of: GameTime,
    pub progress_binding: [u8;32],
}
pub fn completion_binding(c: &Cohort, a: &Completion) -> [u8;32] {
    let mut h = Sha256::new();
    h.update(state_binding(c));
    h.update(digest(b"completion", &[a.window.start.value() as u64,a.window.end.value() as u64,
        u64::from(a.elapsed_academic_years),u64::from(a.covered_parts),u64::from(a.total_parts)]));
    h.finalize().into()
}
/// No skill mutation or certification. An S03/S12 writer must resolve exact members,
/// inspect provenance and deduplicate by cohort/generation/course before publishing rights.
pub fn qualification(current: &Cohort, input: Completion) -> Result<Option<QualificationCandidate>, Error> {
    let w = input.window;
    w.seconds()?;
    if input.view.identity != current.identity || w.start < current.cursor
        || w.end > input.view.as_of || input.total_parts == 0 || input.covered_parts > input.total_parts {
        return Err(Error::Invalid);
    }
    let subject = current.enrollment.cohort;
    let binding = completion_binding(current,&input);
    let calendar = fact(input.calendar,input.view,w,Kind::Calendar,subject,binding)?;
    let exam = fact(input.examination,input.view,w,Kind::Examination,subject,binding)?;
    let prerequisite = fact(input.prerequisite,input.view,w,Kind::Prerequisite,subject,binding)?;
    let days = (w.start.value() - current.enrollment.started.value()) / 86400;
    if !calendar || !exam || !prerequisite
        || input.elapsed_academic_years < current.course.academic_years()
        || (days as u64) < current.course.minimum_days()
        || current.progress.actual_seconds_per_learner < current.course.required_seconds()
        || u64::from(input.covered_parts) * 100 < u64::from(input.total_parts) * 90 {
        return Ok(None);
    }
    // Fields are extracted only after all three Some facts were validated.
    Ok(Some(QualificationCandidate { identity:current.identity,root:input.view.root,
        cohort:subject,generation:current.enrollment.generation,roster_ref:current.enrollment.roster_ref,
        course_ref:current.course.reference,sequence:current.sequence,
        calendar_source:input.calendar.ok_or(Error::Unknown(Kind::Calendar))?.source_ref,
        examination_source:input.examination.ok_or(Error::Unknown(Kind::Examination))?.source_ref,
        prerequisite_source:input.prerequisite.ok_or(Error::Unknown(Kind::Prerequisite))?.source_ref,
        as_of:input.view.as_of,progress_binding:state_binding(current) }))
}
