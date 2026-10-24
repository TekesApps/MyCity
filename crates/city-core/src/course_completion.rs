//! Optional free Electric/Water completion rules. No qualification writer or progress ledger.
//! Data/serde/hash is not authority. The original root authenticates every crate-private
//! marker from Opening, System ingress, original runtime and the ONE qualification state.
use crate::{education as e, member_qualification::{Background,BaseQualification,MemberRange,ReadSource,Skill},units::GameTime};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::io::{self,Write};

pub const VERSION:u32=1;
pub const MAX_CONTENT_PARTS:u32=32;
const HASH_INPUT_LIMIT:usize=4096;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Profile {FreeElectricWaterV1}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version:u32,pub profile:Profile,pub reference:u64,
    pub issuer:u32,pub charter_ref:u64,pub authorization_ref:u64,
    pub course_ref:u64,pub course_version:u32,pub result:BaseQualification,
    pub free_contract_ref:u64,pub free_entitlement_ref:u64,
    pub exam_rule_ref:u64,pub exam_rule_version:u32,pub syllabus:u32,
    pub required_background:Background,pub required_skill:Skill,
    pub valid_from:GameTime,pub valid_until:GameTime,
}
impl Policy {
    pub(crate) fn validate(self)->Result<(),Reject> {
        if self.version!=VERSION || self.course_version!=1 || self.exam_rule_version==0
            || !matches!(self.result,BaseQualification::Electric|BaseQualification::Water)
            || self.required_background!=Background::General || self.required_skill!=Skill::Technical {
            return Err(Reject::UnsupportedProfile);
        }
        if [self.reference,self.charter_ref,self.authorization_ref,self.course_ref,self.free_contract_ref,
            self.free_entitlement_ref,self.exam_rule_ref].contains(&0) || self.issuer==0 || self.syllabus==0
            || self.syllabus.count_ones()>MAX_CONTENT_PARTS || self.valid_from>=self.valid_until {return Err(Reject::Policy);}
        Ok(())
    }
    pub fn binding(&self)->Result<[u8;32],Reject> {hash(b"course-completion-policy-v1",self)}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum GroupResult {Pass,Fail,Partial}
/// A bitset is distinct by representation; list ingress must reject duplicate IDs BEFORE
/// conversion, rather than silently deduplicating. Coverage counts content, never people.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Examination {
    pub population_origin:[u8;32],pub member_version:u32,
    pub cohort:u64,pub generation:u64,pub course_ref:u64,pub course_version:u32,pub roster_ref:u64,
    pub members:MemberRange,pub issuer:u32,pub charter_ref:u64,pub authorization_ref:u64,
    pub rule_ref:u64,pub rule_version:u32,pub occurred_at:GameTime,
    pub content:u32,pub result:GroupResult,pub progress_binding:[u8;32],
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExaminationOrigin {pub operation_id:u64,pub event_id:u64,pub epoch:u64,pub sequence:u64}
/// Persistent DATA owned by root's reserved result slot/receipt chain; no "verified" flag.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExaminationRecord {
    examination:Examination,origin:ExaminationOrigin,source:ReadSource,policy_binding:[u8;32],
}
impl ExaminationRecord {
    pub fn examination(&self)->Examination {self.examination}
    pub fn origin(&self)->ExaminationOrigin {self.origin}
    pub fn source(&self)->ReadSource {self.source}
    pub fn policy_binding(&self)->[u8;32] {self.policy_binding}
    pub fn binding(&self)->Result<[u8;32],Reject> {hash(b"course-examination-record-v1",self)}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum ActivityState {Settled,Pending,Awaiting}
/// Original read, not an external checkpoint DTO. Root supplies the actual existing Cohort.
pub(crate) struct CurrentCourse<'a> {cohort:&'a e::Cohort,source:ReadSource,members:MemberRange,activity:ActivityState}
impl<'a> CurrentCourse<'a> {
    pub(crate) fn after_original_runtime_read(cohort:&'a e::Cohort,source:ReadSource,members:MemberRange,activity:ActivityState)->Result<Self,Reject> {
        validate_source(source)?;
        let c=cohort.checkpoint();
        if c.identity.world!=source.scope.world() || c.identity.branch!=source.scope.branch()
            || members.start>=members.end || members.end-members.start!=u64::from(c.enrollment.learners)
            || c.cursor>source.as_of || c.enrollment.started>source.as_of {return Err(Reject::CourseSource);}
        Ok(Self {cohort,source,members,activity})
    }
}
/// Root must resolve exact issuer/charter/course/result/rule/window/free entitlement
/// against its original Opening directory BEFORE calling, not merely parse nonzero IDs.
/// This marker is neither public nor serializable; it is reconstructed at each current root.
pub(crate) struct AuthorizedPolicy {policy:Policy,source:ReadSource,opening_policy_binding:[u8;32]}
impl AuthorizedPolicy {
    pub(crate) fn after_root_issuer_resolution(policy:Policy,source:ReadSource,opening_policy_binding:[u8;32])->Result<Self,Reject> {
        validate_source(source)?;policy.validate()?;
        if opening_policy_binding!=policy.binding()? {return Err(Reject::Policy);}
        Ok(Self {policy,source,opening_policy_binding})
    }
}
/// Caller has authenticated original System host event, exact population/issuer/rule,
/// stable completed Development tick and original Envelope identity. System alone is
/// insufficient; the root must authenticate the ACTUAL examination ingress.
pub(crate) struct VerifiedExamination {examination:Examination,source:ReadSource,origin:ExaminationOrigin}
impl VerifiedExamination {
    pub(crate) fn after_root_system_examination(examination:Examination,source:ReadSource,origin:ExaminationOrigin)->Result<Self,Reject> {
        validate_source(source)?;
        if [origin.operation_id,origin.event_id,origin.epoch,origin.sequence].contains(&0)
            || examination.occurred_at!=source.as_of {return Err(Reject::ExaminationSource);}
        Ok(Self {examination,source,origin})
    }
}
/// Root resolves this exact saved record from its accepted result slot/original receipt.
/// Deserializing ExaminationRecord alone does not create this marker.
pub(crate) struct RecordedExamination<'a> {record:&'a ExaminationRecord,current_source:ReadSource}
impl<'a> RecordedExamination<'a> {
    pub(crate) fn after_root_record_resolution(record:&'a ExaminationRecord,current_source:ReadSource,authenticated_record_binding:[u8;32])->Result<Self,Reject> {
        validate_source(current_source)?;
        if record.binding()?!=authenticated_record_binding
            || [record.origin.operation_id,record.origin.event_id,record.origin.epoch,record.origin.sequence].contains(&0) {
            return Err(Reject::ExaminationSource);
        }
        Ok(Self {record,current_source})
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum PrerequisiteResult {Met,NotMet,Unknown}
/// Root maps the current labor_qualification/unique-state result for the EXACT members
/// and General/Technical requirement. There is no second capability table here.
pub(crate) struct Prerequisites {source:ReadSource,members:MemberRange,required_background:Background,required_skill:Skill,result:PrerequisiteResult}
impl Prerequisites {
    pub(crate) fn after_root_prerequisite_read(source:ReadSource,members:MemberRange,required_background:Background,required_skill:Skill,result:PrerequisiteResult)->Result<Self,Reject> {
        validate_source(source)?;
        if members.start>=members.end {return Err(Reject::Members);}
        Ok(Self {source,members,required_background,required_skill,result})
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Reject {UnsupportedProfile,Policy,Source,CourseSource,Members,Subject,Clock,PolicyWindow,ExaminationSource,
    PartialResult,UnregisteredContent,StaleProgress,Encoding,Oracle}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum NotMet {PendingActivity,AwaitingHistory,ExaminationFailed,ContentCoverage,Prerequisite,ObservationWindow,ActualHours,Calendar,Oracle}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Unknown {Prerequisite}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticKey {
    pub scope:crate::arena::Scope,pub population_origin:[u8;32],pub member_version:u32,pub members:MemberRange,
    pub course_scope:u64,pub cohort:u64,pub generation:u64,pub course_ref:u64,pub result:BaseQualification,
}
impl SemanticKey {pub fn binding(&self)->Result<[u8;32],Reject> {hash(b"course-completion-semantic-v1",self)}}
/// Candidate DATA: original writer must recheck current root/Cohort, reserve result,
/// semantically deduplicate and atomically commit completion+grant once. No writer here.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Candidate {
    pub key:SemanticKey,pub source:ReadSource,pub policy_binding:[u8;32],pub examination_binding:[u8;32],
    pub examination_origin:ExaminationOrigin,pub expected_progress_binding:[u8;32],
    pub oracle:e::QualificationCandidate,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Decision {Candidate(Candidate),NotMet(NotMet),Unknown(Unknown)}
fn validate_source(s:ReadSource)->Result<(),Reject> {
    if s.population_origin==[0;32] || s.root==[0;32] || s.member_version!=1 {return Err(Reject::Source);} Ok(())
}
struct HashWriter {sha:Sha256,bytes:usize}
impl Write for HashWriter {
    fn write(&mut self,input:&[u8])->io::Result<usize> {
        if input.len()>HASH_INPUT_LIMIT.saturating_sub(self.bytes) {return Err(io::Error::new(io::ErrorKind::InvalidData,"completion_hash_limit"));}
        self.sha.update(input);self.bytes+=input.len();Ok(input.len())
    }
    fn flush(&mut self)->io::Result<()> {Ok(())}
}
fn hash<T:Serialize>(domain:&[u8],value:&T)->Result<[u8;32],Reject> {
    let mut w=HashWriter {sha:Sha256::new(),bytes:0};w.sha.update(domain);
    serde_json::to_writer(&mut w,value).map_err(|_| Reject::Encoding)?;Ok(w.sha.finalize().into())
}
/// Full ORIGINAL v2 checkpoint binding, independent of exam payload/current World read
/// revision. This does not mutate, reserialize into v2 storage, or change v2 fingerprints.
pub(crate) fn progress_binding(c:&e::Cohort)->Result<[u8;32],Reject> {hash(b"course-completion-progress-v1",&c.checkpoint())}
fn context(read:&CurrentCourse<'_>,authority:&AuthorizedPolicy)->Result<(),Reject> {
    if read.source!=authority.source || authority.opening_policy_binding!=authority.policy.binding()? {return Err(Reject::Source);}
    let c=read.cohort.checkpoint();let p=authority.policy;
    if c.course.reference!=p.course_ref || c.course.path!=e::Path::Retraining || c.course.cents_per_hour.value()!=0
        || c.enrollment.fee_contract_ref!=p.free_contract_ref {return Err(Reject::Subject);}
    if read.source.as_of<p.valid_from || read.source.as_of>=p.valid_until {return Err(Reject::PolicyWindow);} Ok(())
}
fn examination_subject(read:&CurrentCourse<'_>,p:Policy,x:Examination)->Result<(),Reject> {
    let en=read.cohort.enrollment();
    if x.population_origin!=read.source.population_origin || x.member_version!=read.source.member_version
        || x.members!=read.members {return Err(Reject::Members);}
    if x.cohort!=en.cohort || x.generation!=en.generation || x.roster_ref!=en.roster_ref || x.course_ref!=p.course_ref
        || x.course_version!=p.course_version || x.issuer!=p.issuer || x.charter_ref!=p.charter_ref
        || x.authorization_ref!=p.authorization_ref || x.rule_ref!=p.exam_rule_ref || x.rule_version!=p.exam_rule_version {
        return Err(Reject::Subject);
    }
    if x.progress_binding!=progress_binding(read.cohort)? {return Err(Reject::StaleProgress);}
    if x.result==GroupResult::Partial {return Err(Reject::PartialResult);}
    if x.content & !p.syllabus !=0 {return Err(Reject::UnregisteredContent);} Ok(())
}
pub(crate) fn record_examination(read:&CurrentCourse<'_>,authority:&AuthorizedPolicy,event:VerifiedExamination)->Result<ExaminationRecord,Reject> {
    context(read,authority)?;
    if event.source!=read.source || event.examination.occurred_at!=read.source.as_of {return Err(Reject::Clock);}
    examination_subject(read,authority.policy,event.examination)?;
    Ok(ExaminationRecord {examination:event.examination,origin:event.origin,source:event.source,policy_binding:authority.policy.binding()?})
}
pub(crate) fn evaluate_completion(read:&CurrentCourse<'_>,authority:&AuthorizedPolicy,
    examination:RecordedExamination<'_>,prerequisites:Prerequisites)->Result<Decision,Reject> {
    context(read,authority)?;
    if examination.current_source!=read.source || prerequisites.source!=read.source {return Err(Reject::Source);}
    if prerequisites.members!=read.members {return Err(Reject::Members);}
    if prerequisites.required_background!=authority.policy.required_background
        || prerequisites.required_skill!=authority.policy.required_skill {return Err(Reject::Policy);}
    let record=examination.record;let x=record.examination;
    validate_source(record.source)?;
    if record.source.scope!=read.source.scope || record.source.population_origin!=read.source.population_origin
        || record.source.member_version!=read.source.member_version || record.source.revision>read.source.revision
        || record.source.as_of!=x.occurred_at || x.occurred_at>read.source.as_of
        || x.occurred_at<authority.policy.valid_from || x.occurred_at>=authority.policy.valid_until
        || record.policy_binding!=authority.policy.binding()? {return Err(Reject::ExaminationSource);}
    examination_subject(read,authority.policy,x)?;
    match read.activity {ActivityState::Pending=>return Ok(Decision::NotMet(NotMet::PendingActivity)),
        ActivityState::Awaiting=>return Ok(Decision::NotMet(NotMet::AwaitingHistory)),ActivityState::Settled=>{}}
    if x.result==GroupResult::Fail {return Ok(Decision::NotMet(NotMet::ExaminationFailed));}
    let covered=x.content.count_ones();let total=authority.policy.syllabus.count_ones();
    if covered*100<total*90 {return Ok(Decision::NotMet(NotMet::ContentCoverage));}
    match prerequisites.result {PrerequisiteResult::Unknown=>return Ok(Decision::Unknown(Unknown::Prerequisite)),
        PrerequisiteResult::NotMet=>return Ok(Decision::NotMet(NotMet::Prerequisite)),PrerequisiteResult::Met=>{}}
    // Reuse the old positive completed-observation window; no fake future second.
    let now=read.source.as_of.value();
    if now==0 || now-1<read.cohort.cursor().value() {return Ok(Decision::NotMet(NotMet::ObservationWindow));}
    let start=GameTime::new(now-1).map_err(|_| Reject::Clock)?;
    let w=e::Window {start,end:read.source.as_of};let cp=read.cohort.checkpoint();
    let view=e::View {identity:cp.identity,root:read.source.root,version:e::VERSION,clock:e::Clock::Development,
        as_of:read.source.as_of,max_age_seconds:0,policy_ref:authority.policy.authorization_ref};
    let mut input=e::Completion {view,window:w,calendar:None,elapsed_academic_years:0,examination:None,
        covered_parts:covered,total_parts:total,prerequisite:None};
    let binding=e::completion_binding(read.cohort,&input);
    // These private bridge facts follow authenticated issuer/exam/current prerequisite
    // markers above. No external Fact/JSON or model output can supply them here.
    let fact=|kind,source_ref| Some(e::Fact {view,source_ref,subject:cp.enrollment.cohort,kind,coverage:w,
        observed_at:read.source.as_of,value:true,binding});
    input.calendar=fact(e::Kind::Calendar,authority.policy.authorization_ref);
    input.examination=fact(e::Kind::Examination,record.origin.event_id);
    input.prerequisite=fact(e::Kind::Prerequisite,authority.policy.free_entitlement_ref);
    let Some(oracle)=e::qualification(read.cohort,input).map_err(|_| Reject::Oracle)? else {
        let reason=if cp.progress.actual_seconds_per_learner<cp.course.required_seconds() {NotMet::ActualHours}
            else if (start.value()-cp.enrollment.started.value())/86400 < cp.course.minimum_days() as i64 {NotMet::Calendar}
            else {NotMet::Oracle};return Ok(Decision::NotMet(reason));
    };
    let key=SemanticKey {scope:read.source.scope,population_origin:read.source.population_origin,member_version:read.source.member_version,
        members:read.members,course_scope:cp.identity.scope,cohort:cp.enrollment.cohort,generation:cp.enrollment.generation,
        course_ref:cp.course.reference,result:authority.policy.result};
    Ok(Decision::Candidate(Candidate {key,source:read.source,policy_binding:record.policy_binding,examination_binding:record.binding()?,
        examination_origin:record.origin,expected_progress_binding:progress_binding(read.cohort)?,oracle}))
}

// Inline tests are appended here by the issue-owned worker; author runs nothing.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arena::Scope, member_qualification::{BaseQualification, MemberRange}};

    const DAY: i64 = 86_400;
    const PEOPLE: MemberRange = MemberRange { start: 10, end: 12 };

    fn time(n: i64) -> GameTime { GameTime::new(n).unwrap() }
    fn source(as_of: i64) -> ReadSource {
        ReadSource { scope: Scope::new(1, 2).unwrap(), population_origin: [7; 32], member_version: 1,
            root: [8; 32], revision: 4, as_of: time(as_of) }
    }
    fn cohort(actual_seconds: u64) -> e::Cohort {
        let started = 0;
        e::Cohort::from_checkpoint(e::CohortCheckpoint {
            identity: e::Identity { world: 1, branch: 2, scope: 3 },
            enrollment: e::Enrollment { cohort: 41, generation: 3, roster_ref: 51, class: 61, school: 71,
                learners: 2, started: time(started), fee_contract_ref: 81, obligations_ref: 91 },
            course: e::Course { reference: 101, path: e::Path::Retraining, cents_per_hour: crate::units::Money::ZERO },
            progress: e::Progress { actual_seconds_per_learner: actual_seconds, actual_teaching_days: 60,
                fee_cents_per_learner: crate::units::Money::ZERO, fee_remainder: 0 },
            cursor: time(59 * DAY + actual_seconds.saturating_sub(59 * 7_200).min(7_200) as i64), sequence: 60, day: 59,
            day_seconds: actual_seconds.saturating_sub(59 * 7_200).min(7_200),
            status: e::Status::Active, last_root: [9; 32],
        }).unwrap()
    }
    fn policy(syllabus: u32) -> Policy {
        Policy { version: VERSION, profile: Profile::FreeElectricWaterV1, reference: 1, issuer: 2,
            charter_ref: 3, authorization_ref: 4, course_ref: 101, course_version: 1,
            result: BaseQualification::Electric, free_contract_ref: 81, free_entitlement_ref: 5,
            exam_rule_ref: 6, exam_rule_version: 1, syllabus,
            required_background: Background::General, required_skill: Skill::Technical,
            valid_from: time(0), valid_until: time(1_000 * DAY) }
    }
    fn current(c: &e::Cohort, s: ReadSource, activity: ActivityState) -> CurrentCourse<'_> {
        CurrentCourse::after_original_runtime_read(c, s, PEOPLE, activity).unwrap()
    }
    fn authority(p: Policy, s: ReadSource) -> AuthorizedPolicy {
        AuthorizedPolicy::after_root_issuer_resolution(p, s, p.binding().unwrap()).unwrap()
    }
    fn exam(c: &e::Cohort, p: Policy, s: ReadSource) -> Examination {
        let en = c.enrollment();
        Examination { population_origin: s.population_origin, member_version: s.member_version,
            cohort: en.cohort, generation: en.generation, course_ref: p.course_ref,
            course_version: p.course_version, roster_ref: en.roster_ref, members: PEOPLE,
            issuer: p.issuer, charter_ref: p.charter_ref, authorization_ref: p.authorization_ref,
            rule_ref: p.exam_rule_ref, rule_version: p.exam_rule_version, occurred_at: s.as_of,
            content: p.syllabus, result: GroupResult::Pass, progress_binding: progress_binding(c).unwrap() }
    }
    fn record(c: &e::Cohort, p: Policy, s: ReadSource) -> ExaminationRecord {
        let ev = VerifiedExamination::after_root_system_examination(exam(c,p,s), s,
            ExaminationOrigin { operation_id: 1, event_id: 2, epoch: 3, sequence: 4 }).unwrap();
        record_examination(&current(c,s,ActivityState::Settled), &authority(p,s), ev).unwrap()
    }
    fn decision(c: &e::Cohort, p: Policy, s: ReadSource, activity: ActivityState,
        saved: &ExaminationRecord, prereq: PrerequisiteResult) -> Result<Decision, Reject> {
        let read = current(c,s,activity); let auth = authority(p,s);
        let saved = RecordedExamination::after_root_record_resolution(saved,s,saved.binding()?)?;
        let prereq = Prerequisites::after_root_prerequisite_read(s,PEOPLE,Background::General,Skill::Technical,prereq)?;
        evaluate_completion(&read,&auth,saved,prereq)
    }
    fn candidate(d: Decision) -> Candidate { match d { Decision::Candidate(c) => c, other => panic!("expected candidate, got {other:?}") } }

    #[test]
    fn legacy_observation_window_and_exact_hours_are_preserved() {
        let p = policy(0b1); let c = cohort(432_000); let before = source(90 * DAY);
        let at_cursor=source(c.cursor().value()); let saved=record(&c,p,at_cursor);
        assert_eq!(decision(&c,p,at_cursor,ActivityState::Settled,&saved,PrerequisiteResult::Met).unwrap(),
            Decision::NotMet(NotMet::ObservationWindow));
        let saved = record(&c,p,before);
        assert_eq!(decision(&c,p,before,ActivityState::Settled,&saved,PrerequisiteResult::Met).unwrap(),
            Decision::NotMet(NotMet::Calendar));
        let after = source(90 * DAY + 1); let saved = record(&c,p,after);
        assert!(matches!(decision(&c,p,after,ActivityState::Settled,&saved,PrerequisiteResult::Met), Ok(Decision::Candidate(_))));
        let short = cohort(431_999); let saved = record(&short,p,after);
        assert_eq!(decision(&short,p,after,ActivityState::Settled,&saved,PrerequisiteResult::Met).unwrap(),
            Decision::NotMet(NotMet::ActualHours));
    }

    #[test]
    fn activity_prerequisite_exam_and_syllabus_rules_are_explicit() {
        let p = policy(0b1111); let c = cohort(432_000); let s = source(90 * DAY + 1); let saved = record(&c,p,s);
        assert_eq!(decision(&c,p,s,ActivityState::Pending,&saved,PrerequisiteResult::Met).unwrap(), Decision::NotMet(NotMet::PendingActivity));
        assert_eq!(decision(&c,p,s,ActivityState::Awaiting,&saved,PrerequisiteResult::Met).unwrap(), Decision::NotMet(NotMet::AwaitingHistory));
        assert_eq!(decision(&c,p,s,ActivityState::Settled,&saved,PrerequisiteResult::Unknown).unwrap(), Decision::Unknown(Unknown::Prerequisite));
        assert_eq!(decision(&c,p,s,ActivityState::Settled,&saved,PrerequisiteResult::NotMet).unwrap(), Decision::NotMet(NotMet::Prerequisite));
        let wrong_criteria=Prerequisites::after_root_prerequisite_read(s,PEOPLE,Background::Vocational,Skill::Technical,PrerequisiteResult::Met).unwrap();
        let saved_marker=RecordedExamination::after_root_record_resolution(&saved,s,saved.binding().unwrap()).unwrap();
        assert!(evaluate_completion(&current(&c,s,ActivityState::Settled),&authority(p,s),saved_marker,wrong_criteria).is_err());
        let mut failed = saved; failed.examination.result = GroupResult::Fail;
        assert_eq!(decision(&c,p,s,ActivityState::Settled,&failed,PrerequisiteResult::Met).unwrap(), Decision::NotMet(NotMet::ExaminationFailed));
        let mut partial = exam(&c,p,s); partial.result = GroupResult::Partial;
        assert_eq!(record_examination(&current(&c,s,ActivityState::Settled),&authority(p,s),
            VerifiedExamination::after_root_system_examination(partial,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap()), Err(Reject::PartialResult));
        let mut outside = exam(&c,p,s); outside.content |= 0b1_0000;
        assert_eq!(record_examination(&current(&c,s,ActivityState::Settled),&authority(p,s),
            VerifiedExamination::after_root_system_examination(outside,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap()), Err(Reject::UnregisteredContent));
        let dense=policy(u32::MAX); let mut sparse_exam=exam(&c,dense,s); sparse_exam.content=(1u32<<28)-1;
        let sparse_event=VerifiedExamination::after_root_system_examination(sparse_exam,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap();
        let sparse_record=record_examination(&current(&c,s,ActivityState::Settled),&authority(dense,s),sparse_event).unwrap();
        assert_eq!(decision(&c,dense,s,ActivityState::Settled,&sparse_record,PrerequisiteResult::Met).unwrap(),Decision::NotMet(NotMet::ContentCoverage));
    }

    #[test]
    fn content_threshold_uses_syllabus_bits_and_enforces_32_part_bound() {
        let c = cohort(432_000); let s = source(90 * DAY + 1);
        let p = policy(u32::MAX); let mut x = exam(&c,p,s); x.content = (1u32 << 28)-1;
        let event = VerifiedExamination::after_root_system_examination(x,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap();
        let saved = record_examination(&current(&c,s,ActivityState::Settled),&authority(p,s),event).unwrap();
        assert_eq!(decision(&c,p,s,ActivityState::Settled,&saved,PrerequisiteResult::Met).unwrap(),Decision::NotMet(NotMet::ContentCoverage));
        x.content |= 1u32 << 28;
        let event = VerifiedExamination::after_root_system_examination(x,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap();
        let saved = record_examination(&current(&c,s,ActivityState::Settled),&authority(p,s),event).unwrap();
        assert!(matches!(decision(&c,p,s,ActivityState::Settled,&saved,PrerequisiteResult::Met),Ok(Decision::Candidate(_))));
        assert_eq!(policy(u32::MAX).syllabus.count_ones(),32);
        assert_eq!(Policy { syllabus: 0, ..policy(1) }.validate(),Err(Reject::Policy));
        let sparse=policy(0b1011); let mut out_of_syllabus=exam(&c,sparse,s); out_of_syllabus.content=0b10000;
        assert_eq!(record_examination(&current(&c,s,ActivityState::Settled),&authority(sparse,s),
            VerifiedExamination::after_root_system_examination(out_of_syllabus,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap()),Err(Reject::UnregisteredContent));
    }

    #[test]
    fn semantic_key_ignores_read_revision_but_separates_course_identity_and_result() {
        let c = cohort(432_000); let p = policy(1); let s = source(90 * DAY + 1); let saved = record(&c,p,s);
        let first = candidate(decision(&c,p,s,ActivityState::Settled,&saved,PrerequisiteResult::Met).unwrap());
        let later = ReadSource { revision: 99, root: [33;32], ..s };
        let later_record = RecordedExamination::after_root_record_resolution(&saved,later,saved.binding().unwrap()).unwrap();
        let later_candidate = candidate(evaluate_completion(&current(&c,later,ActivityState::Settled),&authority(p,later),later_record,
            Prerequisites::after_root_prerequisite_read(later,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()).unwrap());
        assert_eq!(first.key,later_candidate.key);
        let revised_event=VerifiedExamination::after_root_system_examination(exam(&c,p,s),s,
            ExaminationOrigin { operation_id: 70, event_id: 80, epoch: 90, sequence: 100 }).unwrap();
        let revised_record=record_examination(&current(&c,s,ActivityState::Settled),&authority(p,s),revised_event).unwrap();
        let revised=candidate(decision(&c,p,s,ActivityState::Settled,&revised_record,PrerequisiteResult::Met).unwrap());
        assert_eq!(first.key,revised.key);
        assert_ne!(first.key.binding().unwrap(),candidate(decision(&cohort_with_ids(42,3),p,s,ActivityState::Settled,&record(&cohort_with_ids(42,3),p,s),PrerequisiteResult::Met).unwrap()).key.binding().unwrap());
        let water = Policy { result: BaseQualification::Water, ..p };
        assert_ne!(first.key, candidate(decision(&c,water,s,ActivityState::Settled,&record(&c,water,s),PrerequisiteResult::Met).unwrap()).key);
    }
    fn cohort_with_ids(cohort_id:u64,generation:u64)->e::Cohort {
        let mut c=cohort(432_000).checkpoint(); c.enrollment.cohort=cohort_id; c.enrollment.generation=generation;
        e::Cohort::from_checkpoint(c).unwrap()
    }

    #[test]
    fn only_free_electric_or_water_policy_shape_is_supported() {
        let c=cohort(432_000); let s=source(90*DAY+1);
        for q in [BaseQualification::Electric,BaseQualification::Water] {
            let p=Policy {result:q,..policy(1)}; assert!(authority(p,s).policy.validate().is_ok());
        }
        let unsupported=Policy {result:BaseQualification::Medical,..policy(1)};
        assert!(matches!(AuthorizedPolicy::after_root_issuer_resolution(unsupported,s,unsupported.binding().unwrap()),Err(Reject::UnsupportedProfile)));
        for bad in [Policy {course_ref:202,..policy(1)}, Policy {free_contract_ref:82,..policy(1)}] {
            let altered=c;
            let auth=authority(bad,s);
            assert_eq!(record_examination(&current(&altered,s,ActivityState::Settled),&auth,
                VerifiedExamination::after_root_system_examination(exam(&altered,bad,s),s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap()),Err(Reject::Subject));
        }
    }

    #[test]
    fn persistent_record_is_data_and_all_current_bindings_must_match() {
        let c=cohort(432_000); let p=policy(1); let s=source(90*DAY+1); let saved=record(&c,p,s);
        let raw=serde_json::to_vec(&saved).unwrap();
        let decoded:ExaminationRecord=serde_json::from_slice(&raw).unwrap();
        assert_eq!(decoded,saved);
        assert!(RecordedExamination::after_root_record_resolution(&decoded,s,saved.binding().unwrap()).is_ok());
        assert!(matches!(RecordedExamination::after_root_record_resolution(&decoded,s,[0;32]),Err(Reject::ExaminationSource)));
        for invalid in [ReadSource {population_origin:[0;32],..s},ReadSource {member_version:2,..s},
            ReadSource {root:[0;32],..s}] {
            assert!(CurrentCourse::after_original_runtime_read(&c,invalid,PEOPLE,ActivityState::Settled).is_err());
        }
        assert!(CurrentCourse::after_original_runtime_read(&c,s,MemberRange{start:10,end:11},ActivityState::Settled).is_err());
        for stale in [ReadSource {population_origin:[10;32],..s},ReadSource {revision:3,..s},ReadSource {as_of:time(90*DAY),..s}] {
            let read=CurrentCourse::after_original_runtime_read(&c,stale,PEOPLE,ActivityState::Settled).unwrap();
            let marker=RecordedExamination::after_root_record_resolution(&saved,stale,saved.binding().unwrap()).unwrap();
            let prereq=Prerequisites::after_root_prerequisite_read(stale,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap();
            assert_eq!(evaluate_completion(&read,&authority(p,stale),marker,prereq),Err(Reject::ExaminationSource));
        }
        let mut wrong=exam(&c,p,s); wrong.issuer+=1;
        assert!(matches!(VerifiedExamination::after_root_system_examination(wrong,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}),Ok(_)));
        let current_marker=current(&c,s,ActivityState::Settled); let current_authority=authority(p,s);
        assert_eq!(record_examination(&current_marker,&current_authority,
            VerifiedExamination::after_root_system_examination(wrong,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap()),Err(Reject::Subject));

        // A same-sized but different current range reaches subject binding and is rejected.
        let moved=MemberRange{start:11,end:13};
        let moved_read=CurrentCourse::after_original_runtime_read(&c,s,moved,ActivityState::Settled).unwrap();
        let saved=record(&c,p,s); let saved_marker=RecordedExamination::after_root_record_resolution(&saved,s,saved.binding().unwrap()).unwrap();
        assert_eq!(evaluate_completion(&moved_read,&authority(p,s),saved_marker,
            Prerequisites::after_root_prerequisite_read(s,moved,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::Members));
        for change in [
            Examination { roster_ref: 999, ..saved.examination() },
            Examination { issuer: 999, ..saved.examination() },
            Examination { rule_ref: 999, ..saved.examination() },
        ] {
            let altered=ExaminationRecord { examination: change, ..saved };
            let marker=RecordedExamination::after_root_record_resolution(&altered,s,altered.binding().unwrap()).unwrap();
            assert_eq!(evaluate_completion(&current(&c,s,ActivityState::Settled),&authority(p,s),marker,
                Prerequisites::after_root_prerequisite_read(s,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::Subject));
        }
    }

    #[test]
    fn markers_reject_stale_progress_and_timestamp_mismatch_without_grant_authority() {
        let p=policy(1); let c=cohort(432_000); let s=source(90*DAY+1); let mut stale=exam(&c,p,s); stale.progress_binding[0]^=1;
        assert_eq!(record_examination(&current(&c,s,ActivityState::Settled),&authority(p,s),
            VerifiedExamination::after_root_system_examination(stale,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}).unwrap()),Err(Reject::StaleProgress));
        let mut wrong_time=exam(&c,p,s); wrong_time.occurred_at=time(s.as_of.value()-1);
        assert!(matches!(VerifiedExamination::after_root_system_examination(wrong_time,s,ExaminationOrigin{operation_id:1,event_id:2,epoch:3,sequence:4}),Err(Reject::ExaminationSource)));
        assert!(matches!(VerifiedExamination::after_root_system_examination(exam(&c,p,s),s,
            ExaminationOrigin{operation_id:0,event_id:2,epoch:3,sequence:4}),Err(Reject::ExaminationSource)));
        let saved=record(&c,p,s); let d=decision(&c,p,s,ActivityState::Settled,&saved,PrerequisiteResult::Met).unwrap();
        assert!(matches!(d,Decision::Candidate(_)));
        // Candidate exposes only the reused education proposal; no capability write exists here.
        assert_eq!(candidate(d).oracle.course_ref,101);
        let changed_entitlement=Policy {free_entitlement_ref:99,..p};
        assert!(matches!(decision(&c,changed_entitlement,s,ActivityState::Settled,&saved,PrerequisiteResult::Met),Err(Reject::ExaminationSource)));
    }

    #[test]
    fn saved_exam_rejects_changed_cohort_generation_population_version_and_revision() {
        let p=policy(1); let c=cohort(432_000); let s=source(90*DAY+1); let saved=record(&c,p,s);
        for changed in [cohort_with_ids(42,3),cohort_with_ids(41,4)] {
            let marker=RecordedExamination::after_root_record_resolution(&saved,s,saved.binding().unwrap()).unwrap();
            assert_eq!(evaluate_completion(&current(&changed,s,ActivityState::Settled),&authority(p,s),marker,
                Prerequisites::after_root_prerequisite_read(s,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::Subject));
        }
        let changed_population=ReadSource {population_origin:[11;32],..s};
        let marker=RecordedExamination::after_root_record_resolution(&saved,changed_population,saved.binding().unwrap()).unwrap();
        assert_eq!(evaluate_completion(&current(&c,changed_population,ActivityState::Settled),&authority(p,changed_population),marker,
            Prerequisites::after_root_prerequisite_read(changed_population,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::ExaminationSource));
        let mut changed_version=saved; changed_version.source.member_version=2;
        let marker=RecordedExamination::after_root_record_resolution(&changed_version,s,changed_version.binding().unwrap()).unwrap();
        assert_eq!(evaluate_completion(&current(&c,s,ActivityState::Settled),&authority(p,s),marker,
            Prerequisites::after_root_prerequisite_read(s,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::Source));
        let mut changed_time=saved; changed_time.source.as_of=time(s.as_of.value()-1);
        let marker=RecordedExamination::after_root_record_resolution(&changed_time,s,changed_time.binding().unwrap()).unwrap();
        assert_eq!(evaluate_completion(&current(&c,s,ActivityState::Settled),&authority(p,s),marker,
            Prerequisites::after_root_prerequisite_read(s,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::ExaminationSource));
        let older_revision=ReadSource {revision:s.revision-1,..s};
        let marker=RecordedExamination::after_root_record_resolution(&saved,older_revision,saved.binding().unwrap()).unwrap();
        assert_eq!(evaluate_completion(&current(&c,older_revision,ActivityState::Settled),&authority(p,older_revision),marker,
            Prerequisites::after_root_prerequisite_read(older_revision,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()),Err(Reject::ExaminationSource));
        let new_root=ReadSource {root:[44;32],revision:s.revision+1,..s};
        let marker=RecordedExamination::after_root_record_resolution(&saved,new_root,saved.binding().unwrap()).unwrap();
        let result=evaluate_completion(&current(&c,new_root,ActivityState::Settled),&authority(p,new_root),marker,
            Prerequisites::after_root_prerequisite_read(new_root,PEOPLE,Background::General,Skill::Technical,PrerequisiteResult::Met).unwrap()).unwrap();
        assert!(matches!(result,Decision::Candidate(_)));
    }
}
