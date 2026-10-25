//! Root-owned registration of the ONE member activity ledger. Domains own no
//! parallel population/cursor. This exposes no generic Begin/End fact ingress.
use crate::{arena::Scope,assembly::World,member_activity as m,member_qualification,
    timeline::Timeline,transaction::TxError};
use serde::{Serialize,Deserialize};

pub(crate) const EPOCH:u64=1;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub version:u32,pub scope:Scope,pub population_origin:[u8;32],pub total_people:u64,pub epoch:u64,
}
fn expected(world:&World)->Result<Registration,TxError> {
    let housing=&world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening;
    let total_people=housing.households.iter().try_fold(0u64,|n,h|n.checked_add(u64::from(h.people)).ok_or(TxError::Overflow))?;
    Ok(Registration{version:1,scope:world.scope,population_origin:member_qualification::housing_population_origin(world.scope,housing),total_people,epoch:EPOCH})
}
pub(crate) fn initialize(input:Registration,world:&World)->Result<Box<m::Ledger>,String> {
    if input!=expected(world).map_err(|_|"member_population_missing")? {return Err("member_registration_binding".into());}
    Ok(Box::new(m::Ledger::new(input.scope,input.population_origin,input.total_people,input.epoch,0).map_err(|_|"member_initialization")?))
}
pub(crate) fn initialize_education(world:&World)->Result<Box<m::Ledger>,String> {
    // Historical implicit education registration remains supported unchanged.
    world.venue.as_deref().ok_or("education_venue_missing")?;
    initialize(expected(world).map_err(|_|"education_population_missing")?,world)
}
pub(crate) fn validate(world:&World,timeline:Option<&Timeline>,ledger:Option<&m::Ledger>)->Result<(),TxError> {
    validate_with_facility(world,timeline,ledger,world.facility_work.as_deref())
}
pub(crate) fn validate_with_facility(world:&World,timeline:Option<&Timeline>,ledger:Option<&m::Ledger>,facility:Option<&crate::facility_authority::Runtime>)->Result<(),TxError>{
    let Some(ledger)=ledger else {return if facility.is_some(){Err(TxError::InvalidOperation)}else{Ok(())};};
    let registration=expected(world)?;
    ledger.validate_binding(registration.scope,registration.population_origin,registration.total_people,EPOCH,
        timeline.map_or(0,|t|if t.progress.phase==Some(crate::schedule::Phase::P5){t.progress.at}else{t.progress.last_complete})).map_err(|_|TxError::InvalidOperation)?;
    // Every active row needs an actual domain owner, not merely a legal Kind.
    // Until the service runtime is wired there is no ServiceWork consumer.
    if ledger.active().iter().any(|row|row.kind==m::Kind::ServiceWork){return Err(TxError::InvalidOperation);}
    crate::facility_authority::validate_rows(world,facility,Some(ledger),timeline.map_or(0,|t|if t.progress.phase==Some(crate::schedule::Phase::P5){t.progress.at}else{t.progress.last_complete}))?;
    Ok(())
}
pub(crate) fn registration(world:&World)->Option<[u8;32]> {
    world.member_activity.as_deref().map(m::Ledger::registration_fingerprint)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legal_kind_without_actual_consumer_is_not_permission() {
        let mut world=crate::assembly::assemble(include_bytes!("../../city-storage/tests/fixtures/education-store/free-offline-v1.json")).unwrap();
        let registered=expected(&world).unwrap();let ledger=world.member_activity.as_deref().unwrap();
        let source=m::Source{scope:world.scope,population_origin:registered.population_origin,epoch:EPOCH,ordinal:ledger.next_ordinal().unwrap(),operation_id:1,occurred_at:0};
        let candidate=ledger.prepare_begin(source,0,900,m::Kind::ServiceWork,[Some(m::Members{start:0,end:1}),None]).unwrap();
        world.member_activity=Some(Box::new(candidate.into_next(ledger).unwrap()));
        assert!(crate::education_authority::validate(&world,None).is_err());
        world.education=None;assert!(crate::education_authority::validate(&world,None).is_err());
    }
}
