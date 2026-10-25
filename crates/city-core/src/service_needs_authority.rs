//! Root-owned bridge: original Housing transaction and Timeline are the only writers.
use crate::{assembly::World, service_needs as n, timeline::Timeline,
    transaction::{Action, JournalRecord, TxError}};
use serde::{Serialize,Deserialize};
/// Explicit 32-household prototype allocation inside original Engine/role totals.
pub const MAX_OWNED_BYTES:usize=16*1024;

/// Small authenticated scheduling observation. It grants no execution authority;
/// the original Store continuation and revision must still be revalidated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DueView {
    pub version: u32,
    pub city_id: u32,
    pub fingerprint: [u8;32],
    pub registration: [u8;32],
    pub covered_through: u64,
    pub pending: Option<n::DueRun>,
    pub next_due: Option<u64>,
}
pub(crate) fn due_view(runtime:&n::Runtime)->Result<DueView,TxError> {
    if !runtime.is_ready() { return Err(TxError::InvalidOperation); }
    Ok(DueView { version:n::VERSION, city_id:runtime.opening().city_id,
        fingerprint:runtime.fingerprint().map_err(bad)?,
        registration:runtime.registration_fingerprint().map_err(bad)?,
        covered_through:runtime.covered_through(), pending:runtime.pending(),
        next_due:runtime.next_due().map_err(bad)? })
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub version: u32, pub city_id: u32, pub fingerprint: [u8;32],
    pub covered_through: u64, pub pending: Option<n::DueRun>,
    pub households: Vec<n::HouseholdState>,
}
pub(crate) fn view(runtime:&n::Runtime)->View {
    View{version:n::VERSION,city_id:runtime.opening().city_id,
        fingerprint:runtime.fingerprint().expect("validated bounded needs"),
        covered_through:runtime.covered_through(),pending:runtime.pending(),
        households:runtime.households().to_vec()}
}
fn bad(_:impl std::fmt::Debug)->TxError{TxError::InvalidOperation}
pub(crate) fn allows_local_city(input:&n::Opening,scope:crate::arena::Scope,seed:u64,housing:&crate::housing::OpeningHousing)->bool{
    input.validate().is_ok() && input.scope==scope && input.seed==seed && housing.city_id==Some(input.city_id)
        && input.households.len()==housing.households.len()
        && input.households.iter().zip(housing.households.iter()).all(|(reg,original)|
            reg.household_id==original.household_id && reg.source_id==original.source_id
            && reg.people==original.people && reg.generation==1)
}
fn binding(world:&World,runtime:&n::Runtime)->Result<(),TxError>{
    runtime.validate().map_err(bad)?;
    let input=runtime.opening();
    let housing=world.housing.as_deref().ok_or(TxError::InvalidOperation)?;
    if world.region.is_some() || !allows_local_city(input,world.scope,world.seed,&housing.opening) {return Err(TxError::InvalidOperation);}
    Ok(())
}
pub(crate) fn initialize(input:n::Opening,world:&World)->Result<Box<n::Runtime>,String>{
    let runtime=Box::new(n::Runtime::new(input,0).map_err(|e|format!("service_needs_opening:{e:?}"))?);
    binding(world,&runtime).map_err(|_|"service_needs_origin_binding")?;
    Ok(runtime)
}
pub(crate) fn validate(world:&World,runtime:Option<&n::Runtime>,timeline:Option<&Timeline>,
    region:Option<&crate::region::Region>)->Result<(),TxError>{
    let Some(runtime)=runtime else{return Ok(());};
    binding(world,runtime)?;
    if region.is_some(){return Err(TxError::InvalidOperation);}
    let Some(t)=timeline else{
        if runtime.covered_through()!=0 || runtime.pending().is_some() || runtime.households().iter().any(|h|h.entry.is_some()){
            return Err(TxError::InvalidOperation);
        }
        return Ok(());
    };
    // Other original lanes may still be processing this boundary. Only P5 may own a due-run.
    if runtime.covered_through()!=t.progress.last_complete {
        // The service page can complete before the original P5 publishes its endpoint,
        // but both are committed by the same original transaction.
        if t.progress.phase!=Some(crate::schedule::Phase::P5)
            || runtime.covered_through()!=t.progress.at {return Err(TxError::InvalidOperation);}
    }
    if let Some(run)=runtime.pending(){
        if t.progress.phase!=Some(crate::schedule::Phase::P5) || run.at!=t.progress.at
            || run.at<t.progress.last_complete {return Err(TxError::InvalidOperation);}
    }
    Ok(())
}
pub(crate) fn check_peak(existing_plus_staged:usize,limit:usize)->Result<(),TxError>{
    if existing_plus_staged.checked_add(n::Runtime::candidate_bytes()).is_none_or(|peak|peak>limit){return Err(TxError::WorkingLimit);}
    Ok(())
}
pub(crate) fn due_page(runtime:&mut Option<Box<n::Runtime>>,at:u64,operation:u64,working_base:usize,limit:usize)->Result<bool,TxError>{
    let Some(current)=runtime.as_deref() else{return Ok(true);};
    check_peak(working_base,limit)?;
    let candidate=current.prepare_due(at,n::MAX_PAGE_HOUSEHOLDS,operation).map_err(bad)?;
    if !candidate.matches(current).map_err(bad)?{return Err(TxError::InvalidOperation);}
    let complete=candidate.page.complete;
    *runtime=Some(candidate.next);
    Ok(complete)
}
pub(crate) fn record_entry(world:&World,runtime:&mut Option<Box<n::Runtime>>,offer_id:u64,
    timeline:&Timeline,operation:u64)->Result<(),TxError>{
    let Some(current)=runtime.as_deref() else{return Ok(());};
    if timeline.progress.phase.is_some() || timeline.progress.target.is_some()
        || timeline.progress.at!=timeline.progress.last_complete {return Err(TxError::InvalidOperation);}
    let offer=world.housing.as_deref().ok_or(TxError::InvalidOperation)?.opening.lease_offer(offer_id).map_err(bad)?;
    let candidate=current.record_legal_entry(offer.household_id,timeline.progress.last_complete,operation).map_err(bad)?;
    if !candidate.matches(current).map_err(bad)?{return Err(TxError::InvalidOperation);}
    *runtime=Some(candidate.next);Ok(())
}

/// Unforgeable completion carrier; original Store supplies bounded original receipts.
pub struct RestoreCheck { expected:[u8;32], checked:[bool;n::MAX_HOUSEHOLDS] }
pub(crate) fn begin_restore(world:&World)->Result<RestoreCheck,TxError>{
    let r=world.service_needs.as_deref().ok_or(TxError::InvalidOperation)?;
    Ok(RestoreCheck{expected:r.fingerprint().map_err(bad)?,checked:[false;n::MAX_HOUSEHOLDS]})
}
pub(crate) fn check_restore(world:&World,check:&mut RestoreCheck,index:usize,record:&JournalRecord)->Result<(),TxError>{
    let r=world.service_needs.as_deref().ok_or(TxError::InvalidOperation)?;
    if r.fingerprint().map_err(bad)?!=check.expected || index>=r.households().len() || check.checked[index] {
        return Err(TxError::InvalidOperation);
    }
    let row=r.households()[index];let entry=row.entry.ok_or(TxError::InvalidOperation)?;
    let housing=world.housing.as_deref().ok_or(TxError::InvalidOperation)?;
    let household=housing.opening.household(row.household_id).map_err(bad)?;
    let Some(Action::HousingLease{offer_id,offer_sha256})=record.actions[0] else{return Err(TxError::InvalidOperation);};
    let offer=housing.opening.lease_offer(offer_id).map_err(bad)?;
    if record.operation_id!=entry.operation_id || record.actor!=household.account
        || record.actions[1..].iter().any(Option::is_some) || record.outcome.is_err()
        || offer.household_id!=row.household_id || offer.starts_at.value()<0
        || offer.starts_at.value() as u64!=entry.at
        || crate::housing::genesis_lease_offer_sha256(offer)!=offer_sha256
        || !housing.state.consumed_genesis_offer_ids().any(|id|id==offer_id) {return Err(TxError::InvalidOperation);}
    check.checked[index]=true;Ok(())
}
pub(crate) fn finish_restore(world:&mut World,check:RestoreCheck)->Result<(),TxError>{
    let r=world.service_needs_mut().ok_or(TxError::InvalidOperation)?;
    if r.fingerprint().map_err(bad)?!=check.expected
        || r.households().iter().enumerate().any(|(i,h)|h.entry.is_some()&&!check.checked[i]) {return Err(TxError::InvalidOperation);}
    r.mark_authenticated().map_err(bad)?;Ok(())
}
impl crate::restore_into::Restore for Option<Box<n::Runtime>> {
    fn restore<'de,D:serde::Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error>{
        *self=Option::<Box<n::Runtime>>::deserialize(d)?;Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_staged_budget_rejects_before_due_page_and_keeps_original_state(){
        let mut households=crate::model_rows::Rows::new().unwrap();
        households.try_push(n::Household{household_id:1,source_id:10,generation:1,people:1}).unwrap();
        let input=n::Opening{version:1,profile:n::Profile::LocalDevelopmentNaturalNeedsV1,
            scope:crate::arena::Scope::new(1,1).unwrap(),seed:0,city_id:1,households};
        let mut runtime=Some(Box::new(n::Runtime::new(input,0).unwrap()));
        let before=runtime.as_deref().unwrap().fingerprint().unwrap();
        let candidate=n::Runtime::candidate_bytes();
        let limit=crate::transaction::ENGINE_BYTE_BUDGET;
        let isolated=limit-candidate-1;
        assert!(check_peak(isolated,limit).is_ok());
        // Two more bytes from another original staged domain cross the joint gate.
        assert_eq!(due_page(&mut runtime,1,1,isolated+2,limit),Err(TxError::WorkingLimit));
        assert_eq!(runtime.as_deref().unwrap().fingerprint().unwrap(),before);
        assert_eq!(runtime.as_deref().unwrap().covered_through(),0);
        assert!(runtime.as_deref().unwrap().pending().is_none());
        assert_eq!(check_peak(usize::MAX,limit),Err(TxError::WorkingLimit));
        assert_eq!(due_page(&mut runtime,1,1,limit-candidate,limit),Ok(true));
        assert_eq!(runtime.as_deref().unwrap().covered_through(),1);
    }
}
