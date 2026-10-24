//! Root-owned adapter. Only the original transaction writer publishes these candidates.
//! Host facts are explicit game-host inputs, never transport/venue presence certificates.
use crate::{assembly::World, arena::Scope, education as e, education_runtime as r,
    member_activity as m, timeline::Timeline, transaction::{Caller, Envelope, JournalRecord, TxError},
    units::GameTime, venue_capacity as v, venue_runtime};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};

const MEMBER_EPOCH: u64 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Command {
    /// Sequence is a CAS on the admitted education stream, not a new clock.
    Event { expected_epoch: u64, expected_sequence: u64, command: r::Command,
        /// Explicit System-host historical coverage, independent of venue presence.
        actual: Option<e::Window> },
    Compact { expected_runtime: [u8;32], anchor: r::ReceiptAnchor },
}
#[derive(Debug,PartialEq,Eq,Serialize)]
pub struct View {
    pub strategy_version: u32, pub fingerprint: [u8;32],
    pub stream_epoch: u64, pub consumed_sequence: u64,
    pub progress: [Option<(u64,e::Progress)>;r::MAX_COHORTS],
    pub pending: [Option<u64>;r::MAX_COHORTS], pub awaiting: [Option<u64>;r::MAX_COHORTS],
    pub prefix_anchor: Option<r::ReceiptAnchor>,
}
#[derive(Debug,PartialEq,Eq,Serialize)]
pub struct MemberView { pub fingerprint: [u8;32], pub active_count: usize, pub next_ordinal: Option<u64> }

pub(crate) fn view(runtime: &r::Runtime) -> View {
    let mut progress=[None;r::MAX_COHORTS];let mut pending=[None;r::MAX_COHORTS];let mut awaiting=[None;r::MAX_COHORTS];
    for (i,reg) in runtime.opening().courses.iter().enumerate() {
        if let Some(reg)=reg {
            let id=reg.enrollment.cohort;
            progress[i]=runtime.progress(id).expect("validated course").map(|p|(id,p));
            if runtime.pending(id).expect("validated course") {pending[i]=Some(id);}
            if runtime.awaiting_settlement(id).expect("validated course") {awaiting[i]=Some(id);}
        }
    }
    View {strategy_version:2,fingerprint:runtime.fingerprint().expect("bounded runtime"),
        stream_epoch:runtime.opening().stream_epoch,consumed_sequence:runtime.consumed_sequence(),progress,pending,awaiting,
        prefix_anchor:runtime.prefix_anchor()}
}
pub(crate) fn member_view(ledger: &m::Ledger) -> MemberView {
    MemberView {fingerprint:ledger.fingerprint(),active_count:ledger.active().len(),next_ordinal:ledger.next_ordinal().ok()}
}
fn fail(_: impl std::fmt::Debug) -> TxError {TxError::InvalidOperation}
fn rule_error(e:r::Error)->TxError {
    match e {r::Error::Limit|r::Error::NeedsCompaction=>TxError::WorkingLimit,_=>TxError::InvalidOperation}
}
fn time(tick:u64)->Result<GameTime,TxError> {GameTime::new(i64::try_from(tick).map_err(|_|TxError::Overflow)?).map_err(fail)}
pub(crate) fn completed(timeline:Option<&Timeline>)->Result<u64,TxError> {
    let t=timeline.ok_or(TxError::InvalidOperation)?;
    if t.progress.phase.is_some() || t.progress.target.is_some() {return Err(TxError::InvalidOperation);}
    Ok(t.progress.last_complete)
}
fn people(world:&World)->Result<u64,TxError> {
    world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening.households.iter()
        .try_fold(0u64,|n,h|n.checked_add(u64::from(h.people)).ok_or(TxError::Overflow))
}
fn registered_offer(venue:&venue_runtime::Runtime,use_id:u64)->Result<r::RegisteredOffer,TxError> {
    let offer=venue.opening.offers.iter().find(|o|o.use_id==use_id).ok_or(TxError::InvalidOperation)?;
    let def=venue.opening.venues.iter().find(|v|v.venue_id==offer.venue_id).ok_or(TxError::InvalidOperation)?;
    if offer.purpose!=def.purpose || !matches!(def.purpose.0,1|2) {return Err(TxError::InvalidOperation);}
    Ok(r::RegisteredOffer {use_id,venue_id:offer.venue_id,group_id:offer.group_id,resource_id:def.resource_id,
        members:r::Members {source:offer.partition.partition_source_id,version:u32::try_from(offer.partition.partition_version).map_err(|_|TxError::Overflow)?,
            start:offer.partition.member_range_start,end:offer.partition.member_range_end},
        learners:offer.population.learners,teachers:offer.population.teachers,staff:offer.population.staff,
        visitors:offer.population.visitors,enrollment:offer.population.enrollment,
        window:e::Window{start:time(offer.window.start)?,end:time(offer.window.end)?}})
}
pub(crate) fn initialize(opening:r::Opening,world:&World)->Result<(Box<r::Runtime>,Box<m::Ledger>),String> {
    let venue=world.venue.as_deref().ok_or("education_venue_missing")?;
    let population=people(world).map_err(|_|"education_population_missing")?;
    if opening.identity.world!=world.scope.world() || opening.identity.branch!=world.scope.branch()
        || opening.identity.scope==0 || opening.population_origin!=venue.population_origin_sha256 {
        return Err("education_registration_origin".into());
    }
    let mut offers=Vec::with_capacity(r::MAX_COHORTS);
    let mut qualifications=Vec::with_capacity(r::MAX_COHORTS);let mut free=Vec::with_capacity(r::MAX_COHORTS);
    for reg in opening.courses.iter().flatten() {
        let offer=registered_offer(venue,reg.use_id).map_err(|_|"education_offer_missing")?;
        if !offers.contains(&offer) {offers.push(offer);}
        // These are explicitly versioned immutable game registrations, not an OS,
        // external certification, or arbitrary callback's claim of qualification.
        if !qualifications.contains(&reg.qualification) {qualifications.push(reg.qualification);}
        if !free.contains(&reg.free_contract) {free.push(reg.free_contract);}
    }
    let authority=r::VerifiedOpening::after_root_opening_validation(opening.identity,opening.fingerprint().map_err(|_|"education_opening_hash")?,
        opening.registration_origin().map_err(|_|"education_origin_hash")?,venue.population_origin_sha256,population,
        &offers,&qualifications,&free);
    let ledger=m::Ledger::new(world.scope,venue.population_origin_sha256,population,MEMBER_EPOCH,0).map_err(|_|"education_ledger")?;
    let runtime=r::Runtime::new(opening,authority).map_err(|_|"education_registration_invalid")?;
    Ok((Box::new(runtime),Box::new(ledger)))
}
fn kind(k:r::ActivityKind)->Result<m::Kind,TxError> {
    match k {r::ActivityKind::Preparation=>Ok(m::Kind::EducationPreparation),
        r::ActivityKind::Teaching=>Ok(m::Kind::EducationInstruction),_=>Err(TxError::InvalidOperation)}
}
fn members(a:r::SharedActivity,b:Option<r::SharedActivity>)->[Option<m::Members>;2] {
    [Some(m::Members{start:a.members.start,end:a.members.end}),b.map(|b|m::Members{start:b.members.start,end:b.members.end})]
}
fn find_active<'a>(ledger:&'a m::Ledger,a:r::SharedActivity,b:Option<r::SharedActivity>)->Result<&'a m::Active,TxError> {
    let k=kind(a.kind)?;
    let mut matching=ledger.active().iter().filter(|row|row.owner==a.key.cohort && row.kind==k
        && row.start_operation_id==a.source.operation_id && row.started==a.start.value() as u64
        && row.settled_through==a.settled_through.value() as u64 && row.members==members(a,b));
    let row=matching.next().ok_or(TxError::InvalidOperation)?;
    if matching.next().is_some() {return Err(TxError::InvalidOperation);} Ok(row)
}
/// Project only education's exact original claims. Global exclusion is checked by
/// the ONE ledger prepare_begin; no fabricated Origin is made for foreign domains.
fn projection(runtime:&r::Runtime,ledger:&m::Ledger)->Result<Vec<r::SharedActivity>,TxError> {
    let mut rows=Vec::with_capacity(2*r::MAX_COHORTS);let mut expected=0;
    for (teacher,learners) in runtime.pending_claims().map_err(fail)?.into_iter().flatten() {
        find_active(ledger,teacher,learners)?;expected+=1;rows.push(teacher);if let Some(l)=learners {rows.push(l);}
    }
    if ledger.active().iter().filter(|a|matches!(a.kind,m::Kind::EducationPreparation|m::Kind::EducationInstruction)).count()!=expected {
        return Err(TxError::InvalidOperation);
    } Ok(rows)
}
pub(crate) fn validate(world:&World,timeline:Option<&Timeline>)->Result<(),TxError> {
    validate_parts(world,timeline,world.education.as_deref(),world.member_activity.as_deref(),None)
}
pub(crate) fn validate_candidate(world:&World,timeline:Option<&Timeline>,candidate:Option<&Candidate>,
    venue:Option<&venue_runtime::UseCandidate>)->Result<(),TxError> {
    validate_parts(world,timeline,candidate.map(|c|&*c.runtime).or(world.education.as_deref()),
        candidate.map(|c|&*c.ledger).or(world.member_activity.as_deref()),venue)
}
fn validate_parts(world:&World,timeline:Option<&Timeline>,education:Option<&r::Runtime>,member:Option<&m::Ledger>,
    proposed_venue:Option<&venue_runtime::UseCandidate>)->Result<(),TxError> {
    match (education,member) {
        (None,None)=>Ok(()),
        (Some(runtime),Some(ledger))=>{
            let venue=world.venue.as_deref().ok_or(TxError::InvalidOperation)?;
            let tick=if timeline.is_none() && runtime.consumed_sequence()==0 {0} else {
                timeline.ok_or(TxError::InvalidOperation)?.progress.last_complete
            };
            let input=runtime.opening();
            if input.identity.world!=world.scope.world() || input.identity.branch!=world.scope.branch()
                || input.population_origin!=venue.population_origin_sha256
                || runtime.last_source().is_some_and(|s|s.occurred_at.value() as u64>tick || s.revision>world.revision) {
                return Err(TxError::InvalidOperation);
            }
            ledger.validate_binding(world.scope,venue.population_origin_sha256,people(world)?,MEMBER_EPOCH,tick).map_err(fail)?;
            for reg in input.courses.iter().flatten() {
                registered_offer(venue,reg.use_id)?;
                let states=proposed_venue.map_or(&venue.states[..],|v|&v.states[..]);
                let facts=proposed_venue.map_or(&venue.facts[..],|v|&v.facts[..]);
                if runtime.pending(reg.enrollment.cohort).map_err(fail)? && states.iter().find(|s|s.use_id==reg.use_id)
                    .is_none_or(|s|s.status!=v::UseStatus::Actual) {return Err(TxError::InvalidOperation);}
                if let Some(stopped)=runtime.stopped_activity(reg.enrollment.cohort).map_err(fail)? {
                    let cutoff=stopped.cutoff;
                    let row=states.iter().find(|s|s.use_id==reg.use_id).ok_or(TxError::InvalidOperation)?;
                    let fact=facts.iter().find(|f|f.use_id==reg.use_id).and_then(|f|f.exited).ok_or(TxError::InvalidOperation)?;
                    if row.status!=v::UseStatus::Released || row.token.number!=cutoff.token_number
                        || fact.occurred_at!=cutoff.exited_at.value() as u64 || fact.operation_id!=cutoff.exit_operation_id
                        || fact.event_id!=cutoff.exit_event_id || fact.transaction_source.revision!=cutoff.exit_revision {
                        return Err(TxError::InvalidOperation);
                    }
                }
            }
            projection(runtime,ledger)?;Ok(())
        },_=>Err(TxError::InvalidOperation)
    }
}
fn origin(world:&World,runtime:&r::Runtime,env:&Envelope,event_id:u64,tick:u64)->Result<r::Origin,TxError> {
    let text=world.state_hash();let mut root=[0;32];
    for (i,byte) in root.iter_mut().enumerate() {*byte=u8::from_str_radix(&text[2*i..2*i+2],16).map_err(fail)?;}
    Ok(r::Origin {identity:runtime.opening().identity,root,revision:world.revision,clock:e::Clock::Development,
        operation_id:env.operation_id,event_id,stream_epoch:runtime.opening().stream_epoch,
        stream_sequence:runtime.next_sequence().map_err(fail)?,occurred_at:time(tick)?})
}
fn coverage(world:&World,use_id:u64,root:[u8;32],candidate:Option<&venue_runtime::UseCandidate>)->Result<r::VenueCoverage,TxError> {
    let venue=world.venue.as_deref().ok_or(TxError::InvalidOperation)?;let offer=registered_offer(venue,use_id)?;
    let states=candidate.map_or(&venue.states[..],|c|&c.states[..]);let facts=candidate.map_or(&venue.facts[..],|c|&c.facts[..]);
    let state=states.iter().find(|s|s.use_id==use_id).ok_or(TxError::Lease)?;
    let facts=facts.iter().find(|s|s.use_id==use_id).ok_or(TxError::InvalidOperation)?;
    if !matches!(state.status,v::UseStatus::Actual|v::UseStatus::Released) {return Err(TxError::InvalidOperation);}
    Ok(r::VenueCoverage {root,use_id,venue_id:offer.venue_id,group_id:offer.group_id,resource_id:offer.resource_id,
        token_number:state.token.number,members:offer.members,offer_window:offer.window,
        entered_at:time(facts.entered.ok_or(TxError::InvalidOperation)?.occurred_at)?,
        exited_at:facts.exited.map(|f|time(f.occurred_at)).transpose()?})
}
pub(crate) struct Candidate { pub runtime:Box<r::Runtime>, pub ledger:Box<m::Ledger> }
pub(crate) fn candidate_bytes()->usize {r::Runtime::owned_bytes()+m::Ledger::owned_bytes()+m::Ledger::candidate_bytes()
    +2*r::MAX_COHORTS*std::mem::size_of::<r::SharedActivity>()+std::mem::size_of::<r::Candidate>()}
/// Original full ticket remains borrowed from Store's charged receipt workspace.
/// Only core's extra typed replay and streaming authentication workspace is added.
pub(crate) fn restore_workspace_bytes()->usize {
    r::Runtime::restore_workspace_bytes()+std::mem::size_of::<Envelope>()
        +std::mem::size_of::<crate::transaction::Action>()+std::mem::size_of::<HashSink>()
}
fn stage(world:&World,runtime:&mut r::Runtime,ledger:&mut m::Ledger,source:r::Origin,
    command:r::Command,venue:Option<r::VenueCoverage>,stopped:Option<r::StoppedCoverage>)->Result<(),TxError> {
    let rows=projection(runtime,ledger)?;
    let event=r::VerifiedEvent::after_system_host_validation(command,source,source.occurred_at).map_err(fail)?;
    let proposal=runtime.plan(r::Snapshot::after_root_source_validation(event,runtime.opening().fingerprint().map_err(fail)?,
        runtime.opening().population_origin,venue,&rows,r::LedgerCoverage::EducationOnlyV1,stopped)).map_err(rule_error)?;
    let next=match proposal.activity_delta {
        None=>None,
        Some(r::ActivityDelta::Acquire {teacher,learners})=>{
            let input=m::Source{scope:world.scope,population_origin:runtime.opening().population_origin,epoch:MEMBER_EPOCH,
                ordinal:ledger.next_ordinal().map_err(fail)?,operation_id:source.operation_id,occurred_at:source.occurred_at.value() as u64};
            Some(ledger.prepare_begin(input,input.occurred_at,teacher.key.cohort,kind(teacher.kind)?,members(teacher,learners)).map_err(fail)?)
        },
        Some(r::ActivityDelta::Settle {teacher,from,through,close,..})=>{
            let (a,b)=runtime.pending_claims().map_err(fail)?.into_iter().flatten().find(|(a,_)|a.key==teacher).ok_or(TxError::InvalidOperation)?;
            let row=*find_active(ledger,a,b)?;
            if row.settled_through!=from.value() as u64 || through!=source.occurred_at {return Err(TxError::InvalidOperation);}
            let input=m::Source{scope:world.scope,population_origin:runtime.opening().population_origin,epoch:MEMBER_EPOCH,
                ordinal:ledger.next_ordinal().map_err(fail)?,operation_id:source.operation_id,occurred_at:source.occurred_at.value() as u64};
            Some(ledger.prepare_settle(input,input.occurred_at,row.id,row.owner,row.kind,close).map_err(fail)?)
        }
    };
    let next=next.map(|c|c.into_next(ledger)).transpose().map_err(fail)?;
    runtime.apply_candidate(proposal).map_err(fail)?;if let Some(next)=next {*ledger=next;}
    projection(runtime,ledger)?;Ok(())
}

fn copy_candidate(world:&World)->Result<Candidate,TxError> {
    let runtime=world.education.as_ref().ok_or(TxError::InvalidOperation)?;
    if !runtime.is_ready() {return Err(TxError::InvalidOperation);}
    let ledger=world.member_activity.as_ref().ok_or(TxError::InvalidOperation)?;
    Ok(Candidate {runtime:runtime.clone(),ledger:ledger.clone()})
}
pub(crate) fn prepare(world:&World,timeline:Option<&Timeline>,caller:Caller,env:&Envelope,
    command:Command,receipt:Option<&JournalRecord>)->Result<Candidate,TxError> {
    if !matches!(caller,Caller::System) {return Err(TxError::Unauthorized);}
    if env.actions.len()!=1 {return Err(TxError::InvalidOperation);}
    validate(world,timeline)?;let tick=completed(timeline)?;let mut candidate=copy_candidate(world)?;
    match command {
        Command::Event {expected_epoch,expected_sequence,command,actual}=>{
            if matches!(command.action,r::Action::BeginPreparation|r::Action::BeginLesson|r::Action::ActivateNoInstruction)
                && crate::qualification_authority::completed_course(world,command.cohort) {
                return Err(TxError::InvalidOperation);
            }
            let source=origin(world,&candidate.runtime,env,env.operation_id,tick)?;
            if expected_epoch!=source.stream_epoch || expected_sequence!=source.stream_sequence
                || matches!(command.action,r::Action::StopForExit{..}) {return Err(TxError::InvalidOperation);}
            let reg=*candidate.runtime.opening().courses.iter().flatten().find(|r|r.enrollment.cohort==command.cohort)
                .ok_or(TxError::InvalidOperation)?;
            let venue=if matches!(command.action,r::Action::ActivateNoInstruction|r::Action::NoInstruction{..}) {None}
                else {Some(coverage(world,reg.use_id,source.root,None)?)};
            let stopped=if let r::Action::SettleAfterExit {actual_through,..}=command.action {
                let prior=candidate.runtime.stopped_activity(command.cohort).map_err(fail)?.ok_or(TxError::InvalidOperation)?;
                let facts=world.venue.as_deref().ok_or(TxError::InvalidOperation)?.facts.iter()
                    .find(|f|f.use_id==reg.use_id).ok_or(TxError::InvalidOperation)?;
                let exit=facts.exited.ok_or(TxError::InvalidOperation)?;
                let venue=venue.ok_or(TxError::InvalidOperation)?;
                Some(r::StoppedCoverage {identity:source.identity,root:source.root,use_id:reg.use_id,
                    token_number:venue.token_number,exited_at:time(exit.occurred_at)?,exit_operation_id:exit.operation_id,
                    exit_event_id:exit.event_id,exit_revision:exit.transaction_source.revision,
                    no_instruction:e::Window {start:actual_through,end:source.occurred_at},
                    actual:actual.map(|window|r::ActualCoverage {identity:source.identity,root:source.root,
                        cohort:command.cohort,generation:reg.enrollment.generation,original_begin_event_id:prior.begin.event_id,
                        kind:prior.kind,window,source_operation_id:source.operation_id,source_event_id:source.event_id,
                        source_revision:source.revision,observed_at:source.occurred_at})})
            } else {if actual.is_some() {return Err(TxError::InvalidOperation);}None};
            stage(world,&mut candidate.runtime,&mut candidate.ledger,source,command,venue,stopped)?;
        },
        Command::Compact {expected_runtime,anchor}=>{
            if expected_runtime!=candidate.runtime.fingerprint().map_err(fail)? || anchor.store_ref==env.operation_id
                || anchor.through!=candidate.runtime.consumed_sequence() {return Err(TxError::InvalidOperation);}
            let source=candidate.runtime.last_source().ok_or(TxError::InvalidOperation)?;
            authenticate_anchor(world.scope,anchor,source,receipt.ok_or(TxError::InvalidOperation)?)?;
            let rows=projection(&candidate.runtime,&candidate.ledger)?;
            candidate.runtime.compact_prefix(r::VerifiedCompaction::after_durable_prefix_validation(expected_runtime,anchor),&rows).map_err(fail)?;
        }
    }
    Ok(candidate)
}
pub(crate) fn validate_compaction(world:&World,expected:[u8;32],anchor:r::ReceiptAnchor,
    record:&JournalRecord)->Result<(),TxError> {
    let runtime=world.education.as_deref().ok_or(TxError::InvalidOperation)?;
    if !runtime.is_ready() || expected!=runtime.fingerprint().map_err(fail)?
        || anchor.through!=runtime.consumed_sequence() {return Err(TxError::InvalidOperation);}
    authenticate_anchor(world.scope,anchor,runtime.last_source().ok_or(TxError::InvalidOperation)?,record)
}
/// The original SafeExit prepares capacity and these cutoffs in ONE transaction.
/// Physical release is independent of whether teaching history is yet known.
pub(crate) fn stop_for_exit(world:&World,timeline:Option<&Timeline>,env:&Envelope,use_id:u64,
    event_id:u64,venue_candidate:&venue_runtime::UseCandidate)->Result<Option<Candidate>,TxError> {
    let Some(current)=world.education.as_deref() else {return Ok(None)};
    let affected:Vec<_>=current.opening().courses.iter().flatten()
        .filter(|r|r.use_id==use_id && current.pending(r.enrollment.cohort).unwrap_or(false))
        .map(|r|r.enrollment.cohort).collect();
    if affected.is_empty() {return Ok(None);}
    let tick=completed(timeline)?;let mut candidate=copy_candidate(world)?;
    for cohort in affected {
        let source=origin(world,&candidate.runtime,env,event_id,tick)?;
        let venue=coverage(world,use_id,source.root,Some(venue_candidate))?;
        let (a,_)=candidate.runtime.pending_claims().map_err(fail)?.into_iter().flatten()
            .find(|(a,_)|a.key.cohort==cohort).ok_or(TxError::InvalidOperation)?;
        let stop=r::StoppedCoverage {identity:source.identity,root:source.root,use_id,token_number:venue.token_number,
            exited_at:source.occurred_at,exit_operation_id:env.operation_id,exit_event_id:event_id,
            exit_revision:world.revision,no_instruction:e::Window{start:source.occurred_at,end:source.occurred_at},actual:None};
        stage(world,&mut candidate.runtime,&mut candidate.ledger,source,r::Command {cohort,
            action:r::Action::StopForExit{started_event_id:a.source.event_id,exited_at:source.occurred_at,exit_operation_id:env.operation_id}},
            Some(venue),Some(stop))?;
    }
    Ok(Some(candidate))
}

/// Fixed domain and stable typed JournalRecord encoding; no Debug, partial payload,
/// full archive copy or persisted "verified" flag. Encoding scratch is streaming.
struct HashSink {hash:Sha256,bytes:usize}
impl std::io::Write for HashSink {
        fn write(&mut self,b:&[u8])->std::io::Result<usize> {
            if self.bytes.checked_add(b.len()).is_none_or(|n|n>32*1024) {return Err(std::io::Error::other("education_receipt_encoding_limit"));}
            self.bytes+=b.len();self.hash.update(b);Ok(b.len())
        }
        fn flush(&mut self)->std::io::Result<()> {Ok(())}
}
fn typed_digest(value:&impl Serialize,domain:&[u8])->Result<[u8;32],String> {
    let mut sink=HashSink{hash:Sha256::new(),bytes:0};sink.hash.update(domain);
    serde_json::to_writer(&mut sink,value).map_err(|e|e.to_string())?;Ok(sink.hash.finalize().into())
}
pub fn receipt_digest(record:&JournalRecord)->Result<[u8;32],String> {
    typed_digest(record,b"mycity-education-full-journal-record-v1\0")
}
fn authenticate_anchor(scope:Scope,anchor:r::ReceiptAnchor,source:r::Origin,record:&JournalRecord)->Result<(),TxError> {
    if source.identity.world!=scope.world() || source.identity.branch!=scope.branch()
        || anchor.epoch!=source.stream_epoch || anchor.through!=source.stream_sequence
        || anchor.store_ref!=source.operation_id || record.operation_id!=anchor.store_ref
        || record.actor!=0 || record.expected_revision!=source.revision || record.outcome.is_err()
        || record.actions[0].is_none() || record.actions[1..].iter().any(Option::is_some)
        || receipt_digest(record).map_err(fail)?!=anchor.root {return Err(TxError::InvalidOperation);}
    match record.actions[0].ok_or(TxError::InvalidOperation)? {
        crate::transaction::Action::Education {command:Command::Event{expected_epoch,expected_sequence,..}}
            if expected_epoch==anchor.epoch && expected_sequence==anchor.through && source.event_id==record.operation_id=>{},
        crate::transaction::Action::Venue {command:venue_runtime::Command::SafeExit{event_id,occurred_at,..},..}
            if event_id==source.event_id && occurred_at==source.occurred_at.value() as u64=>{},
        _=>return Err(TxError::InvalidOperation),
    }
    // Recover the original scope-bearing Envelope digest, not just matching op IDs.
    let envelope=Envelope {world:scope.world(),branch:scope.branch(),operation_id:record.operation_id,
        expected_revision:record.expected_revision,actions:vec![record.actions[0].unwrap()]};
    let digest=typed_digest(&envelope,b"").map_err(fail)?;
    if digest!=record.payload_sha256 {return Err(TxError::InvalidOperation);}Ok(())
}
pub(crate) fn restore(world:&World,receipt:Option<&JournalRecord>)->Result<Option<Box<r::Runtime>>,TxError> {
    let Some(runtime)=world.education.as_deref() else {return Ok(None)};
    let ledger=world.member_activity.as_deref().ok_or(TxError::InvalidOperation)?;
    if let Some(anchor)=runtime.prefix_anchor() {
        authenticate_anchor(world.scope,anchor,runtime.prefix_source().ok_or(TxError::InvalidOperation)?,receipt.ok_or(TxError::InvalidOperation)?)?;
    } else if receipt.is_some() {return Err(TxError::InvalidOperation);}
    let rows=projection(runtime,ledger)?;let hash=runtime.fingerprint().map_err(fail)?;
    let next=runtime.clone().restore(r::VerifiedRestore::after_original_store_validation(hash,
        runtime.opening().fingerprint().map_err(fail)?,runtime.opening().registration_origin().map_err(fail)?,runtime.prefix_anchor()),&rows).map_err(fail)?;
    Ok(Some(Box::new(next)))
}

impl crate::restore_into::Restore for Option<Box<r::Runtime>> {
    fn restore<'de,D:serde::Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error> {
        *self=Option::<Box<r::Runtime>>::deserialize(d)?;Ok(())
    }
}
impl crate::restore_into::Restore for Option<Box<m::Ledger>> {
    fn restore<'de,D:serde::Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error> {
        *self=Option::<Box<m::Ledger>>::deserialize(d)?;Ok(())
    }
}
