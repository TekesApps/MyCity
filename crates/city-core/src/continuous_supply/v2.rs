//! Explicit original-source V2 state. It is not a second inventory or writer.
//! The physical/P5 bridge is introduced separately; a pristine V2 registration
//! cannot run through the legacy fixed-mask V1 implementation.
use super::*;
use crate::water_treatment as w;

pub const VERSION:u32=2;
pub const WAGE_SETTLEMENT_VERSION:u32=1;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum ConsumptionCostPolicy { FrozenIntegerUnitLinearV1 }

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub enum WageSettlement {
    PayEachP5,
    RetainProtectedEarned{policy_ref:u64,version:u32},
}
impl WageSettlement {
    pub(crate) fn accrual_policy(self)->Option<w::AccrualPolicyV2>{
        match self{Self::PayEachP5=>None,Self::RetainProtectedEarned{policy_ref,version}=>Some(w::AccrualPolicyV2{policy_ref,version})}
    }
    fn validate(self)->Result<(),TxError>{
        if matches!(self,Self::RetainProtectedEarned{policy_ref:0,..})
            ||matches!(self,Self::RetainProtectedEarned{version,..}if version!=WAGE_SETTLEMENT_VERSION){return Err(TxError::InvalidOperation);}Ok(())
    }
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fees {
    pub contract_ref:u64,
    pub operation_cents_per_second:u64,
    pub maintenance_cents_per_second:u64,
    pub concentrate_cents_per_started_litre:u64,
    pub sludge_cents_per_started_mq:u64,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum ProductStoragePolicy { GameOwnedSingleSourceStorageV1 }
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductStorage {
    pub version:u32,
    pub policy:ProductStoragePolicy,
    pub stock:u32,
    pub resource:u32,
    pub owner:u32,
    pub capacity_ml:u64,
    pub from:u64,
    pub until:u64,
    pub standard:w::Standard,
}

/// Declared genesis facts and finite contracts, not a provider assertion.
/// Reagent/sludge use original Commodity::Raw in mQ under this explicit profile.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version:u32,
    pub precision_version:u32,
    pub source_quality:w::LotQuality,
    pub reagent_stock:u32,
    /// Product mL, concentrate mL, dry sludge mQ, in that order.
    pub outlet_stocks:[u32;3],
    pub outlet_capacities:[u64;3],
    pub purifier_resource:u32,
    pub receiver_resources:[u32;3],
    pub receiver_account:u32,
    pub receiving_permission_ref:u64,
    pub receiving_from:u64,
    pub receiving_until:u64,
    pub nominal_power_w:u64,
    pub consumption_cost_policy:ConsumptionCostPolicy,
    pub wage_settlement:WageSettlement,
    pub fees:Fees,
    pub product_storage:ProductStorage,
}

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortOutflow { pub ml:u64,pub cents:u64 }

/// Only original integer debits are recorded here. Fractional material belongs
/// to the authenticated treatment checkpoint/WIP, never an extra Stock.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub version:u32,
    pub outflow:[PortOutflow;2],
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub batch:Option<Box<crate::water_treatment_authority::Batch>>,
    pub other_purpose_basis_cents:u64,
    pub treatment_tokens:[Option<u64>;crate::exchange::PATH],
}
impl State {
    pub(super) fn pristine()->Self{Self{version:VERSION,outflow:[PortOutflow::default();2],batch:None,other_purpose_basis_cents:0,treatment_tokens:[None;crate::exchange::PATH]}}
    pub(super) fn owned_bytes(&self)->usize{
        size_of::<Self>()+self.batch.as_ref().map_or(0,|_|size_of::<crate::water_treatment_authority::Batch>())
    }
    pub(super) fn validate(&self)->Result<(),TxError>{
        if self.version!=VERSION{return Err(TxError::InvalidOperation);}
        match self.batch.as_deref(){
            None if self.outflow==[PortOutflow::default();2]&&self.other_purpose_basis_cents==0&&self.treatment_tokens.iter().all(Option::is_none)=>{},
            Some(b)=>{
                b.validate_structure()?;
                let amount=b.checkpoint.physical.processed_ml.ceil().map_err(bad)?;
                let cost=b.checkpoint.physical.raw_stream_basis.ok_or(TxError::InvalidOperation)?.taken_cents;
                if self.outflow[0].ml!=amount/2+amount%2||self.outflow[1].ml!=amount/2
                    ||add(self.outflow[0].cents,self.outflow[1].cents)?!=cost{return Err(TxError::Stock);}
            },
            _=>return Err(TxError::InvalidOperation),
        }
        Ok(())
    }
}

impl Opening {
    pub(super) fn validate_registration(&self,base:&super::Opening,world:&World,genesis:bool)->Result<(),TxError>{
        self.wage_settlement.validate()?;
        if self.version!=VERSION||self.precision_version!=w::PRECISION_VERSION
            ||self.nominal_power_w!=200_000||self.receiving_permission_ref==0
            ||self.receiving_from!=0||self.receiving_until==0||self.fees.contract_ref==0
            ||self.outlet_capacities.iter().any(|n|*n==0||*n>i64::MAX as u64)
            ||[self.fees.operation_cents_per_second,self.fees.maintenance_cents_per_second,
                self.fees.concentrate_cents_per_started_litre,self.fees.sludge_cents_per_started_mq].iter().any(|n|*n>i64::MAX as u64){return Err(TxError::InvalidOperation);}
        self.source_quality.validate().map_err(bad)?;
        let raw=*world.stocks.get(world.stock_handle(base.stocks[2]).map_err(bad)?).map_err(bad)?;
        if self.source_quality.stock_id!=raw.id||self.source_quality.lot_ref!=u64::from(raw.lot_id)
            ||self.source_quality.composition.water_ml!=w::Exact::integer(u64::try_from(raw.origin_quantity.value()).map_err(bad)?).map_err(bad)?{
            return Err(TxError::Stock);
        }
        let chemistry=self.source_quality.composition;
        if !matches!(chemistry.harmful_mq,w::Mass::Known(_))
            ||!matches!(chemistry.suspended_mq,w::Mass::Known(_))
            ||!matches!(chemistry.allowed_dissolved_mq,w::Mass::Known(_))
            ||chemistry.other_mq!=w::Mass::Known(w::Exact::ZERO){return Err(TxError::InvalidOperation);}
        // The explicit immutable genesis source must cover every admitted
        // receiving instant. A quality label alone supplies no permission.
        if self.source_quality.references.iter().flatten().any(|r|r.coverage_from!=0||r.observed_at!=0||r.coverage_until<self.receiving_until){return Err(TxError::InvalidOperation);}
        let p=&self.product_storage;
        if p.version!=1||p.owner!=base.owner||p.capacity_ml==0||p.capacity_ml>i64::MAX as u64||p.from>=p.until||p.until>self.receiving_until{return Err(TxError::InvalidOperation);}
        let stock_ids=[self.reagent_stock,self.outlet_stocks[0],self.outlet_stocks[1],self.outlet_stocks[2],p.stock];
        if stock_ids.iter().enumerate().any(|(i,id)|*id==0||base.stocks.contains(id)||stock_ids[..i].contains(id)){return Err(TxError::Stock);}
        let resource_ids=roles(self);
        if resource_ids.iter().enumerate().any(|(i,id)|*id==0||base.resources.contains(id)||resource_ids[..i].contains(id)){return Err(TxError::Resource);}
        let owner=world.account_handle(base.owner).map_err(bad)?;
        let receiver=world.account_handle(self.receiver_account).map_err(bad)?;
        if owner==receiver{return Err(TxError::InvalidOperation);}
        for (i,id) in stock_ids.iter().enumerate(){
            let s=world.stocks.get(world.stock_handle(*id).map_err(bad)?).map_err(bad)?;
            let expected_owner=if i<=1||i==4{owner}else{receiver};
            let commodity=if i==0||i==3{Commodity::Raw}else if i==1||i==4{Commodity::Water}else{Commodity::WasteWater};
            if s.owner!=expected_owner||s.commodity!=commodity||s.lot_id==0
                ||(genesis&&(s.offset!=Quantity::ZERO||s.quantity!=s.origin_quantity||s.basis!=s.origin_basis
                    ||(i==0&&s.quantity==Quantity::ZERO)||(i!=0&&(s.quantity!=Quantity::ZERO||s.basis!=Money::ZERO)))){return Err(TxError::Stock);}
        }
        Ok(())
    }
    pub(super) fn governs(&self,id:u32)->bool{self.reagent_stock==id||self.outlet_stocks.contains(&id)||self.product_storage.stock==id}
    pub(super) fn validate_resources(&self,resources:&[Resource])->Result<(),TxError>{
        for id in roles(self){
            if !resources.iter().any(|r|r.id==id&&r.capacity==1){return Err(TxError::Resource);}
        }Ok(())
    }
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
pub struct ProductStockView {
    pub version:u32,
    pub stock_id:u32,
    pub composition:w::Composition,
    pub judgment:w::QualityJudgment,
    pub original_start_operation:u64,
    pub source_registration:[u8;32],
    pub source_fingerprint:[u8;32],
}
pub(crate) fn product_quality(r:&Runtime,id:u32)->Result<Option<ProductStockView>,TxError>{
    let Some(o)=r.treatment_opening()else{return Ok(None);};
    if id!=o.outlet_stocks[0]&&id!=o.product_storage.stock{return Ok(None);}
    if !r.ready{return Err(TxError::InvalidOperation);}
    let Some(b)=r.treatment_batch()else{return Ok(None);};
    let gross=b.config.meters(&b.config.restore(b.checkpoint.clone()).map_err(bad)?).map_err(bad)?.materials.product;
    let amount=if id==o.product_storage.stock{b.product_transferred_ml}else{b.product_credited_ml};
    let composition=if amount==0{w::Composition::known_zero(exact(0)?)}else{gross.split(exact(amount)?).map_err(bad)?.0};
    Ok(Some(ProductStockView{version:1,stock_id:id,composition,judgment:w::judge(composition,o.product_storage.standard).map_err(bad)?,
        original_start_operation:b.config.material.binding.operation,source_registration:r.registration()?,source_fingerprint:digest(r)?}))
}
pub(crate) fn product_move(r:&Runtime,from:u32,to:u32)->bool{
    r.treatment_opening().is_some_and(|o|from==o.outlet_stocks[0]&&to==o.product_storage.stock)
}
pub(crate) fn transfer_product(r:&mut Runtime,l:&mut impl Ledger,from:u32,to:u32,n:Quantity,at:u64,operation:u64)->Result<(),TxError>{
    if !r.ready||!product_move(r,from,to)||n==Quantity::ZERO||at!=r.cursor()||operation==0{return Err(TxError::InvalidOperation);}
    validate_prepared(r,l)?;r.validate_stocks(&r.stocks(l)?)?;
    let o=r.treatment_opening().ok_or(TxError::InvalidOperation)?.clone();let p=&o.product_storage;
    if at<p.from||at>=p.until{return Err(TxError::InvalidOperation);}
    let amount=u64::try_from(n.value()).map_err(bad)?;
    let view=product_quality(r,from)?.ok_or(TxError::InvalidOperation)?;
    if view.judgment!=w::QualityJudgment::MeetsGameChemicalStandard(p.standard){return Err(TxError::Unauthorized);}
    // This same quantity moves all exact components; the cumulative receiver
    // and retained product composition are independently re-derived on restore.
    let _exact_split=view.composition.split(exact(amount)?).map_err(bad)?;
    let b=r.treatment_batch().ok_or(TxError::InvalidOperation)?;
    let next=add(b.product_transferred_ml,amount)?;
    if amount>b.product_credited_ml||next>p.capacity_ml{return Err(TxError::Capacity);}
    let cost=crate::water_treatment_authority::Batch::product_basis_at(r,exact(next)?.ratio(50,49).map_err(bad)?)?.floor();
    let delta=cost.checked_sub(b.product_transferred_basis_cents).ok_or(TxError::Stock)?;
    let mut source=l.stock(from)?;let mut receiver=l.stock(to)?;
    if source.owner!=receiver.owner||source.commodity!=Commodity::Water||receiver.commodity!=Commodity::Water{return Err(TxError::Unauthorized);}
    source.quantity=source.quantity.checked_sub(n).map_err(bad)?;source.basis=source.basis.checked_sub(m(delta)?).map_err(bad)?;
    source.origin_quantity=source.quantity;source.origin_basis=source.basis;source.offset=Quantity::ZERO;
    receiver.quantity=receiver.quantity.checked_add(n).map_err(bad)?;receiver.basis=receiver.basis.checked_add(m(delta)?).map_err(bad)?;
    receiver.origin_quantity=receiver.quantity;receiver.origin_basis=receiver.basis;receiver.offset=Quantity::ZERO;
    let index=extra_ids(&o).iter().position(|id|*id==p.resource).ok_or(TxError::Resource)?;
    if r.v2.as_deref().unwrap().treatment_tokens[index].is_none(){let token=l.claim_path(&[(p.resource,1)])?;r.v2.as_deref_mut().unwrap().treatment_tokens[index]=token[0];}
    l.put(from,source)?;l.put(to,receiver)?;
    let b=r.treatment_batch_mut().unwrap();b.product_transferred_ml=next;b.product_transferred_basis_cents=cost;
    b.product_credited_ml=b.product_credited_ml.checked_sub(amount).ok_or(TxError::Stock)?;
    b.product_credited_basis_cents=b.product_credited_basis_cents.checked_sub(delta).ok_or(TxError::Stock)?;
    let ids=extra_ids(&o);let roles=roles(&o);let holds=required_holds(r)?;let mut release=[None;crate::exchange::PATH];
    for(i,id)in ids.iter().enumerate(){let role=roles.iter().position(|v|v==id).ok_or(TxError::Resource)?;if !holds[role]{release[i]=r.v2.as_deref_mut().unwrap().treatment_tokens[i].take();}}
    l.release_path(release)?;r.last_operation=operation;validate_prepared(r,l)
}

pub(crate) fn adopt_treatment(r:&mut Runtime,l:&mut impl Ledger,checked:crate::water_treatment_authority::CheckedTreatment)->Result<(),TxError>{
    let old=r.v2.as_deref().ok_or(TxError::InvalidOperation)?.outflow;
    let next=water_prefix(r,checked.checkpoint().physical.processed_ml.ceil().map_err(bad)?)?;
    let mut take=0;let mut cents=0;
    for i in 0..2{
        let n=next.ports_ml[i].checked_sub(old[i].ml).ok_or(TxError::Stock)?;
        let c=next.ports_cents[i].checked_sub(old[i].cents).ok_or(TxError::Stock)?;take=add(take,n)?;cents=add(cents,c)?;
        let mut s=l.stock(r.opening.stocks[i+3])?;
        if s.quantity!=q(r.produced_ml[i]-old[i].ml)?||s.basis!=r.output_basis[i].checked_sub(m(old[i].cents)?).map_err(bad)?{return Err(TxError::Stock);}
        s.quantity=s.quantity.checked_sub(q(n)?).map_err(bad)?;s.basis=s.basis.checked_sub(m(c)?).map_err(bad)?;
        s.origin_quantity=s.quantity;s.origin_basis=s.basis;s.offset=Quantity::ZERO;l.put(s.id,s)?;
    }
    if take!=checked.delta().raw_take_ml||cents!=checked.delta().raw_take_basis_cents{return Err(TxError::Stock);}
    let reagent=r.treatment_batch().ok_or(TxError::InvalidOperation)?.config.material.reagent_origin;
    let lot=g::InputLot{stock_id:reagent.stock_id,lot_ref:reagent.lot_ref,quantity:q(reagent.quantity_milli)?,basis:m(reagent.basis_cents)?};
    let(actual,_)=adjust_input(l,&lot,checked.delta().reagent_take_mq,0)?;
    if actual!=m(checked.delta().reagent_take_basis_cents)?{return Err(TxError::Stock);}
    let state=r.v2.as_deref_mut().unwrap();state.outflow=std::array::from_fn(|i|PortOutflow{ml:next.ports_ml[i],cents:next.ports_cents[i]});
    checked.install(state.batch.as_deref_mut().ok_or(TxError::InvalidOperation)?);Ok(())
}

pub(crate) fn idle_cursor(r:&mut Runtime,at:u64,op:u64)->Result<(),TxError>{
    if !r.ready||r.genset.status==g::Status::Running||at<r.cursor(){return Err(TxError::InvalidOperation);}
    r.genset.cursor=at;if r.genset.run_count>0{r.last_operation=op;}Ok(())
}

pub(crate) fn stop_pumps(r:&mut Runtime,l:&mut impl Ledger,f:&crate::facility_authority::Runtime,at:u64,_revision:u64)->Result<(),TxError>{
    let(_,b,s)=current(r)?;if s.pumps!=3||at!=r.cursor(){return Err(TxError::InvalidOperation);}
    let previous=s.clone();let processed=b.checkpoint.physical.processed_ml;
    let e=f.state.records().iter().flatten().find(|c|c.contract_id==previous.electric_contract).ok_or(TxError::InvalidOperation)?;
    let w=f.state.records().iter().flatten().find(|c|c.contract_id==previous.water_contract).ok_or(TxError::InvalidOperation)?;
    let mut ids=r.opening.resources;ids.sort();let mut release=[None;crate::exchange::PATH];let mut keep=None;
    for(i,id)in ids.iter().enumerate(){if *id==r.opening.resources[0]{keep=r.tokens[i];}else{release[i]=r.tokens[i];}}
    l.release_path(release)?;r.tokens=[None;crate::exchange::PATH];r.tokens[0]=keep;
    let b=r.treatment_batch_mut().unwrap();b.segments.iter_mut().flatten().last().unwrap().ended_at=Some(at);
    let mut next=previous;next.started_at=at;next.ended_at=None;next.pumps=0;next.generator_seconds_base=r.genset.active_seconds;
    next.raw_ml_base=r.raw_used_ml;next.produced_each_base=r.produced_ml[0];next.processed_base=processed;next.processed_until=processed;
    next.generator_seconds=0;next.treatment_seconds=0;next.electric_wage_carry=e.remainder;next.water_wage_carry=w.remainder;
    next.water_start_cents=0;next.other_start_cents=0;next.constrained_tail=None;
    *r.treatment_batch_mut().unwrap().segments.iter_mut().find(|s|s.is_none()).ok_or(TxError::WorkingLimit)?=Some(next);Ok(())
}

fn roles(o:&Opening)->[u32;5]{[o.purifier_resource,o.receiver_resources[0],o.receiver_resources[1],o.receiver_resources[2],o.product_storage.resource]}
fn extra_ids(o:&Opening)->[u32;5]{let mut ids=roles(o);ids.sort();ids}
fn required_holds(r:&Runtime)->Result<[bool;5],TxError>{
    let running=r.genset.status==g::Status::Running;
    let Some(b)=r.v2.as_deref().and_then(|s|s.batch.as_deref())else{return Ok([false;5]);};
    let m=b.config.meters(&b.config.restore(b.checkpoint.clone()).map_err(bad)?).map_err(bad)?.materials;
    let product=m.product.water_ml.sub(exact(b.product_transferred_ml)?).map_err(bad)?;
    let mass=|m|if let w::Mass::Known(v)=m{Ok(v)}else{Err(TxError::InvalidOperation)};
    Ok([running||!m.raw_wip.water_ml.is_zero()||!m.reagent_wip_mq.is_zero(),running||!product.is_zero(),
        running||!m.concentrate.water_ml.is_zero(),running||!mass(m.sludge.suspended_mq)?.add(mass(m.sludge.allowed_dissolved_mq)?).map_err(bad)?.is_zero(),running||b.product_transferred_ml>0])
}
pub(super) fn validate_prepared(r:&Runtime,l:&mut impl Ledger)->Result<(),TxError>{
    let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile else{return Err(TxError::InvalidOperation);};
    let state=r.v2.as_deref().ok_or(TxError::InvalidOperation)?;
    let ids=extra_ids(o);let roles=roles(o);
    let holds=required_holds(r)?;
    if state.treatment_tokens[5..].iter().any(Option::is_some){return Err(TxError::Lease);}
    for(i,id)in ids.iter().enumerate(){
        let role=roles.iter().position(|v|v==id).ok_or(TxError::Resource)?;
        if holds[role]{
            let token=state.treatment_tokens[i].ok_or(TxError::Lease)?;let mut tokens=[None;crate::exchange::PATH];tokens[0]=Some(token);
            if !l.owns_path(&[(*id,1)],tokens){return Err(TxError::Lease);}
        }else if state.treatment_tokens[i].is_some(){return Err(TxError::Lease);}
    }
    let reserve=if r.genset.status==g::Status::Running{256}else{0};
    if r.owned_bytes()>MAX_OWNED_BYTES||serde_json::to_vec(r).map_err(bad)?.len()>MAX_OWNED_BYTES-reserve{return Err(TxError::WorkingLimit);}
    crate::water_treatment_authority::validate_material_stocks(r,|id|l.stock(id))?;
    Ok(())
}
pub(super) fn validate_leases(r:&Runtime,leases:&[Option<Lease>])->Result<(),TxError>{
    let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile else{return Err(TxError::InvalidOperation);};
    let state=r.v2.as_deref().ok_or(TxError::InvalidOperation)?;let ids=extra_ids(o);
    let roles=roles(o);let holds=required_holds(r)?;
    if state.treatment_tokens[5..].iter().any(Option::is_some){return Err(TxError::Lease);}
    for(i,id)in ids.iter().enumerate(){let role=roles.iter().position(|v|v==id).ok_or(TxError::Resource)?;
        if holds[role]{let token=state.treatment_tokens[i].ok_or(TxError::Lease)?;
            if !leases.iter().flatten().any(|l|l.token==token&&l.resource==*id&&l.actor==0&&l.units==1&&l.occupied){return Err(TxError::Lease);}
        }else if state.treatment_tokens[i].is_some(){return Err(TxError::Lease);}
    }Ok(())
}

pub(super) fn command(r:&mut Runtime,world:&World,facility:Option<&crate::facility_authority::Runtime>,l:&mut impl Ledger,command:Command,at:u64,operation:u64,root:[u8;32],revision:u64)->Result<(),TxError>{
    if !r.ready||at!=r.cursor()||operation==0||world.revision.checked_add(1)!=Some(revision){return Err(TxError::InvalidOperation);}
    r.validate_stocks(&r.stocks(l)?)?;
    match command{
        Command::Stop=>return stop(r,l,at,operation,root,revision,StopCause::Explicit),
        Command::Fault{fault_ref}=>{if fault_ref==0{return Err(TxError::InvalidOperation);}return stop(r,l,at,operation,root,revision,StopCause::Fault(fault_ref));},
        Command::Start{run_id,pumps}=>{
            if pumps!=0&&pumps!=3{return Err(TxError::InvalidOperation);}
            let terms=crate::facility_authority::joint_terms(world,facility,at)?;
            let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile else{return Err(TxError::InvalidOperation);};let o=o.clone();
            if at<o.receiving_from||at>=o.receiving_until||generator_room(r)?==0{return Err(TxError::Capacity);}
            if r.runs[MAX_RUNS-1].is_some(){return Err(TxError::WorkingLimit);}
            if let Some(b)=r.v2.as_deref().and_then(|s|s.batch.as_deref()){
                if b.checkpoint.physical.cursor!=at||b.checkpoint.physical.processed_ml==b.input_limit(&o)?{return Err(TxError::InvalidOperation);}
                // One spare entry is reserved for a mandatory pump-off split.
                if b.segments.iter().flatten().count()+2>crate::water_treatment_authority::MAX_SEGMENTS{return Err(TxError::WorkingLimit);}
            }else if pumps==0{return Err(TxError::InvalidOperation);}
            if pumps==3&&r.available_seconds(&r.stocks(l)?,3)?==0{return Err(TxError::Capacity);}
            if pumps==0&&add(l.stock(r.opening.stocks[3])?.quantity.value()as u64,l.stock(r.opening.stocks[4])?.quantity.value()as u64)?==0{return Err(TxError::Capacity);}
            if r.v2.as_deref().unwrap().batch.is_none()&&r.owned_bytes().checked_add(size_of::<crate::water_treatment_authority::Batch>()).is_none_or(|v|v>MAX_OWNED_BYTES){return Err(TxError::WorkingLimit);}
            let view=r.authority(l,at,root,revision)?;
            let candidate=g::ContinuousGensetV1::propose(&r.config,&r.genset,&view,g::Command::Start{at:instant(r,at),run_id}).map_err(bad)?;
            g::ContinuousGensetV1::revalidate(&r.config,&r.genset,&view,&candidate).map_err(bad)?;
            let mut requests=[(r.opening.resources[0],1);3];let n=if pumps==3{requests[1]=(r.opening.resources[1],1);requests[2]=(r.opening.resources[2],1);3}else{1};
            r.tokens=l.claim_path(&requests[..n])?;
            let ids=extra_ids(&o);for(i,id)in ids.iter().enumerate(){
                if r.v2.as_deref().unwrap().treatment_tokens[i].is_none(){
                    let tokens=l.claim_path(&[(*id,1)])?;r.v2.as_deref_mut().unwrap().treatment_tokens[i]=tokens[0];
                }else{
                    let mut tokens=[None;crate::exchange::PATH];tokens[0]=r.v2.as_deref().unwrap().treatment_tokens[i];
                    if !l.owns_path(&[(*id,1)],tokens){return Err(TxError::Lease);}
                }
            }
            let pump_fee=m(if pumps==3{400}else{0})?;l.pay(r.opening.owner,r.opening.start_supplier,pump_fee)?;
            r.pump_start_fees=r.pump_start_fees.checked_add(pump_fee).map_err(bad)?;
            r.startup_wip=candidate.delta.start_fee.checked_add(pump_fee).map_err(bad)?;
            r.run_converted_base=r.converted_basis()?;r.run_output_base=r.output_basis;
            r.materialize(l,candidate)?;
            *r.runs.iter_mut().find(|v|v.is_none()).ok_or(TxError::WorkingLimit)?=Some(Run{id:run_id,operation,pumps,started_at:at,ended_at:None,stop_cause:None,production_seconds:0,operator_labor_basis:Money::ZERO});
            if r.v2.as_deref().unwrap().batch.is_none(){
                let reagent=l.stock(o.reagent_stock)?;
                let b=crate::water_treatment_authority::Batch::from_original_start(world,r,operation,revision,at,reagent)?;
                r.v2.as_deref_mut().unwrap().batch=Some(b);
            }
            let b=r.v2.as_deref_mut().unwrap().batch.as_deref_mut().unwrap();
            let processed=b.checkpoint.physical.processed_ml;
            *b.segments.iter_mut().find(|v|v.is_none()).ok_or(TxError::WorkingLimit)?=Some(crate::water_treatment_authority::Segment{
                source_run:run_id,source_start_revision:revision,started_at:at,ended_at:None,pumps,generator_seconds_base:r.genset.active_seconds,
                raw_ml_base:r.raw_used_ml,produced_each_base:r.produced_ml[0],processed_base:processed,processed_until:processed,generator_seconds:0,treatment_seconds:0,
                electric_contract:terms.electric.id,water_contract:terms.water.id,electric_hourly_cents:terms.electric.hourly_cents_per_member,
                electric_members:terms.electric.members.end-terms.electric.members.start,electric_wage_carry:terms.electric_carry,
                water_hourly_cents:terms.water.hourly_cents_per_member,water_members:terms.water.members.end-terms.water.members.start,water_wage_carry:terms.water_carry,
                water_start_cents:if pumps==3{550}else{0},other_start_cents:if pumps==3{150}else{300},constrained_tail:None});
            r.last_operation=operation;r.validate_stocks(&r.stocks(l)?)?;validate_prepared(r,l)
        }
    }
}
pub(crate) fn stop(r:&mut Runtime,l:&mut impl Ledger,at:u64,operation:u64,root:[u8;32],revision:u64,cause:StopCause)->Result<(),TxError>{
    if !r.ready||at!=r.cursor()||operation==0{return Err(TxError::InvalidOperation);}
    let view=r.authority(l,at,root,revision)?;r.stop(l,&view,cause)?;
    if let Some(b)=r.v2.as_deref_mut().and_then(|s|s.batch.as_deref_mut()){
        if let Some(s)=b.segments.iter_mut().flatten().last(){s.ended_at=Some(at);}
    }
    let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile else{return Err(TxError::InvalidOperation);};
    let ids=extra_ids(o);let roles=roles(o);
    let holds=required_holds(r)?;
    let mut release=[None;crate::exchange::PATH];
    for(i,id)in ids.iter().enumerate(){let role=roles.iter().position(|v|v==id).ok_or(TxError::Resource)?;if !holds[role]{release[i]=r.v2.as_deref_mut().unwrap().treatment_tokens[i].take();}}
    l.release_path(release)?;r.last_operation=operation;r.validate_stocks(&r.stocks(l)?)?;validate_prepared(r,l)
}

/// Price only an already produced prefix of the same original paired ports.
/// Ordinary P5 cuts do not appear here: the original physical segment origin,
/// source lots, start allocation and earned-wage remainder determine the price.
pub(crate) fn water_prefix(r:&Runtime,amount:u64)->Result<cost_curve::Prefix,TxError>{
    if amount>r.raw_used_ml{return Err(TxError::Stock);}
    let state=r.v2.as_deref().ok_or(TxError::InvalidOperation)?;
    let mut left=amount;let mut result=cost_curve::Prefix{ports_ml:[0;2],ports_cents:[0;2]};
    if let Some(batch)=state.batch.as_deref(){
        for s in batch.segments.iter().flatten().filter(|s|s.pumps==3){
            let each=mul(s.generator_seconds,RATE_RAW)?;
            let take=left.min(mul(each,2)?);
            if take>0{
                let cycle=cost_curve::TwinPortAllocationCycle{fuel:&r.config.fuel,cooling:&r.config.water,raw:&r.raw_lot,
                    origin_active_seconds:s.generator_seconds_base,origin_raw_ml:s.raw_ml_base,observed_each_ml:each,water_start_cents:s.water_start_cents};
                let labor=cost_curve::LaborPrefix{hourly_cents_per_member:s.electric_hourly_cents,members:s.electric_members,
                    carried_numerator:u64::from(s.electric_wage_carry),actual_seconds:s.generator_seconds};
                let price=cycle.prefix_with_labor(&labor,take)?;
                for i in 0..2{result.ports_ml[i]=add(result.ports_ml[i],price.ports_ml[i])?;result.ports_cents[i]=add(result.ports_cents[i],price.ports_cents[i])?;}
                left-=take;
            }
            if left==0{break;}
        }
    }
    if left!=0{return Err(TxError::Stock);}Ok(result)
}

/// Value partial consumption of one specific original integer Stock unit. This
/// does not extend the physical startup step to a fraction of a production mL.
/// Both neighbouring integer prefixes must already have been produced; the
/// whole ceil-prefetched original unit's fixed cost is divided exactly between
/// conversion and its remaining material WIP. Later consumers use endpoint
/// differences of this same frozen curve, never a current-pool average.
pub(crate) fn consumed_water_basis(r:&Runtime,input:w::Exact)->Result<(cost_curve::Prefix,w::StreamBasis),TxError>{
    let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile else{return Err(TxError::InvalidOperation);};
    if o.consumption_cost_policy!=ConsumptionCostPolicy::FrozenIntegerUnitLinearV1{return Err(TxError::InvalidOperation);}
    let whole=input.floor();let ceil=input.ceil().map_err(bad)?;
    let before=water_prefix(r,whole)?;let taken=water_prefix(r,ceil)?;
    let a=add(before.ports_cents[0],before.ports_cents[1])?;let b=add(taken.ports_cents[0],taken.ports_cents[1])?;
    let fraction=input.sub(exact(whole)?).map_err(bad)?;
    let converted=exact(a)?.add(fraction.ratio(b.checked_sub(a).ok_or(TxError::Stock)?,1).map_err(bad)?).map_err(bad)?;
    let wip=exact(b)?.sub(converted).map_err(bad)?;
    Ok((taken,w::StreamBasis{taken_cents:b,converted,wip}))
}

/// Transaction-local physical plan, not a reception or work grant. The caller
/// must stage the original generator candidate, actual member/funds effects,
/// FundingV2 revalidation and original Stock projections before one commit.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct JointPlan {
    pub from:u64,pub until:u64,pub pumps:u8,
    pub raw_take_ml:u64,pub each_produced_ml:u64,
    pub input_ml:w::Exact,pub next_processed_ml:w::Exact,
    pub pump_energy_mwh:w::Exact,pub treatment_energy_mwh:w::Exact,pub dump_heat_mwh:w::Exact,
    pub tail:Option<crate::water_treatment_authority::TailCell>,
    pub stop_pumps:bool,pub stop_all:bool,
}
fn exact(n:u64)->Result<w::Exact,TxError>{w::Exact::integer(n).map_err(bad)}
fn minimum(a:w::Exact,b:w::Exact)->Result<w::Exact,TxError>{
    Ok(if a.cmp_exact(b).map_err(bad)?==std::cmp::Ordering::Greater{b}else{a})
}
fn current<'a>(r:&'a Runtime)->Result<(&'a Opening,&'a crate::water_treatment_authority::Batch,&'a crate::water_treatment_authority::Segment),TxError>{
    let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile else{return Err(TxError::InvalidOperation);};
    let batch=r.v2.as_deref().and_then(|s|s.batch.as_deref()).ok_or(TxError::InvalidOperation)?;
    let segment=batch.segments.iter().flatten().last().ok_or(TxError::InvalidOperation)?;
    if segment.ended_at.is_some(){return Err(TxError::InvalidOperation);}
    Ok((o,batch,segment))
}
fn generator_room(r:&Runtime)->Result<u64,TxError>{
    let fuel=(r.config.fuel.quantity.value()as u64).checked_mul(3).ok_or(TxError::Overflow)?/50;
    let cooling=r.config.water.quantity.value()as u64/1000;
    let thermal=u64::try_from(u128::from(r.opening.heat_capacity_mwh)*3/u128::from(HEAT_NUM)).map_err(|_|TxError::Overflow)?;
    let waste=r.opening.wastewater_capacity_ml/1000;
    Ok(fuel.min(cooling).min(thermal).min(waste).saturating_sub(r.genset.active_seconds))
}
/// Both net process ports grow monotonically while the two pumps run: each
/// produces10000mL/s, and this profile consumes at most8334 integer mL per
/// port/cell. Thus their final exact integer projection bounds every prefix,
/// including a constrained final treatment cell. No current-pool averaging.
fn process_ports_fit(r:&Runtime,dt:u64)->Result<bool,TxError>{
    let(o,b,s)=current(r)?;if s.pumps==0{return Ok(true);}
    let produced=add(r.raw_used_ml,mul(dt,20_000)?)?;
    let next=minimum(minimum(b.checkpoint.physical.processed_ml.add(s.input_rate()?.ratio(dt,1).map_err(bad)?).map_err(bad)?,b.input_limit(o)?)?,exact(produced)?)?;
    let taken=next.ceil().map_err(bad)?;
    for(i,take)in[taken/2+taken%2,taken/2].into_iter().enumerate(){
        let gross=add(r.produced_ml[i],mul(dt,RATE_RAW)?)?;
        if gross.checked_sub(take).ok_or(TxError::Stock)?>r.opening.water_capacities_ml[i]{return Ok(false);}
    }Ok(true)
}
/// Predict the earliest actual hard endpoint without executing or inventing a
/// clock. All arithmetic is exact; precision/work exhaustion is a refusal.
pub(crate) fn boundary(r:&Runtime,l:&mut impl Ledger,now:u64)->Result<Option<u64>,TxError>{
    if r.genset.status!=g::Status::Running{return Ok(None);}
    if r.cursor()!=now{return Err(TxError::InvalidOperation);}
    let(o,b,s)=current(r)?;
    let mut dt=3600.min(generator_room(r)?).min(o.receiving_until.saturating_sub(now));
    if s.pumps==3{
        dt=dt.min((r.raw_lot.quantity.value()as u64).checked_sub(r.raw_used_ml).ok_or(TxError::Stock)?/20_000);
    }
    let room=b.input_limit(o)?.sub(b.checkpoint.physical.processed_ml).map_err(bad)?;
    let mut needed=room;
    if s.pumps==0{
        let available=add(l.stock(r.opening.stocks[3])?.quantity.value()as u64,l.stock(r.opening.stocks[4])?.quantity.value()as u64)?;
        let held=b.config.meters(&b.config.restore(b.checkpoint.clone()).map_err(bad)?).map_err(bad)?.materials.raw_wip.water_ml;
        needed=minimum(needed,exact(available)?.add(held).map_err(bad)?)?;
    }
    let until_limit=crate::water_treatment_authority::divide(needed,s.input_rate()?)?.ceil().map_err(bad)?;
    dt=dt.min(until_limit);
    if s.pumps==3{
        // At most12 iterations for the existing3600-second P5 bound. A zero
        // safe whole-cell allowance becomes a real zero-width stop, not a
        // repeated failing page or permission to overfill the original ports.
        let(mut lo,mut hi)=(0,dt);
        while lo<hi{let mid=lo+(hi-lo+1)/2;if process_ports_fit(r,mid)?{lo=mid;}else{hi=mid-1;}}
        dt=lo;
    }
    Ok(Some(add(now,dt)?))
}

pub(crate) fn plan_interval(r:&Runtime,l:&mut impl Ledger,until:u64)->Result<JointPlan,TxError>{
    if r.genset.status!=g::Status::Running||until<=r.cursor(){return Err(TxError::InvalidOperation);}
    let(o,b,s)=current(r)?;let from=r.cursor();let dt=until-from;
    if dt>3600||boundary(r,l,from)?.is_none_or(|end|until>end)||b.checkpoint.physical.cursor!=from{return Err(TxError::InvalidOperation);}
    let each=if s.pumps==3{mul(dt,RATE_RAW)?}else{0};let raw=mul(each,2)?;
    let produced=add(r.raw_used_ml,raw)?;
    let possible=s.input_rate()?.ratio(dt,1).map_err(bad)?;
    let before=b.checkpoint.physical.processed_ml;
    // The generated paired stream is faster than the12kW purifier in this
    // segment; after pump-off only the actual existing backlog can be consumed.
    // Hence both endpoint and every prefix preserve supply-before-use.
    let next=minimum(minimum(before.add(possible).map_err(bad)?,b.input_limit(o)?)?,exact(produced)?)?;
    let input=next.sub(before).map_err(bad)?;
    if input.is_zero(){return Err(TxError::InvalidOperation);}
    let tail=if input==possible{None}else{
        let full=dt.checked_sub(1).ok_or(TxError::InvalidOperation)?;
        let amount=input.sub(s.input_rate()?.ratio(full,1).map_err(bad)?).map_err(bad)?;
        if amount.is_zero()||amount.cmp_exact(s.input_rate()?).map_err(bad)?!=std::cmp::Ordering::Less{return Err(TxError::InvalidOperation);}
        Some(crate::water_treatment_authority::TailCell{start:add(from,full)?,input_ml:amount})
    };
    let generated=w::Exact::fraction(u128::from(dt)*20_000,3).map_err(bad)?;
    let pump=w::Exact::fraction(u128::from(dt)*u128::from(s.pumps.count_ones())*5_000,3).map_err(bad)?;
    let treatment=input.ratio(1,5).map_err(bad)?;
    let dump=generated.sub(pump).and_then(|v|v.sub(treatment)).map_err(bad)?;
    let no_raw_cell=(r.raw_lot.quantity.value()as u64).checked_sub(produced).ok_or(TxError::Stock)?<20_000;
    let port_stop=s.pumps==3&&!no_raw_cell&&!process_ports_fit(r,dt.checked_add(1).ok_or(TxError::Overflow)?)?;
    Ok(JointPlan{from,until,pumps:s.pumps,raw_take_ml:raw,each_produced_ml:each,input_ml:input,next_processed_ml:next,
        pump_energy_mwh:pump,treatment_energy_mwh:treatment,dump_heat_mwh:dump,tail,
        stop_pumps:s.pumps==3&&no_raw_cell,
        stop_all:next==b.input_limit(o)?||dt==generator_room(r)?||until==o.receiving_until||port_stop||(next==exact(produced)?&&(s.pumps==0||no_raw_cell))})
}

fn generator_cost_at(r:&Runtime,seconds:u64)->Result<u64,TxError>{
    add(cost_curve::lot_prefix(&r.config.fuel,u128::from(seconds)*50,3)?,
        cost_curve::lot_prefix(&r.config.water,u128::from(seconds)*1000,1)?)
}
fn generator_cost_at_exact(r:&Runtime,seconds:w::Exact)->Result<u64,TxError>{
    let(a,b,d)=seconds.parts();let n=u128::from(a)*u128::from(d)+u128::from(b);
    add(cost_curve::lot_prefix(&r.config.fuel,n.checked_mul(50).ok_or(TxError::Overflow)?,u128::from(d)*3)?,
        cost_curve::lot_prefix(&r.config.water,n.checked_mul(1000).ok_or(TxError::Overflow)?,u128::from(d))?)
}
/// Primitive original conversion, original paid startup, original earned
/// electricity-side labor, in distinct canonical purposes. No electric Stock.
pub(crate) fn electric_parts(r:&Runtime,input:Option<w::Exact>)->Result<[u64;3],TxError>{
    let b=r.v2.as_deref().and_then(|s|s.batch.as_deref()).ok_or(TxError::InvalidOperation)?;
    let mut out=[0;3];
    for s in b.segments.iter().flatten(){
        let time=if let Some(x)=input{
            let x=minimum(x,s.processed_until)?;let Some(t)=s.time_at_input(x)?else{continue;};t
        }else{if s.generator_seconds==0{continue;}exact(s.generator_seconds)?};
        let end=time.add(exact(s.generator_seconds_base)?).map_err(bad)?;
        let g=generator_cost_at_exact(r,end)?.checked_sub(generator_cost_at(r,s.generator_seconds_base)?).ok_or(TxError::Stock)?;
        let e=s.earned_at_cells(time.ceil().map_err(bad)?,false)?;
        out[0]=add(out[0],if s.pumps==3{g-g/2}else{g})?;
        out[1]=add(out[1],s.other_start_cents)?;
        out[2]=add(out[2],if s.pumps==3{e-e/2}else{e})?;
    }Ok(out)
}

/// Mutates ONLY the original transaction-local clone and staged ledger. The
/// returned source state is deliberately not publishable before original work,
/// FundingV2 and all receiving projections finish in that same Prepared.
pub(crate) fn stage_physical(r:&mut Runtime,l:&mut impl Ledger,plan:&JointPlan,operation:u64,root:[u8;32],revision:u64)->Result<(),TxError>{
    if !r.ready||operation==0||revision==0||plan.from!=r.cursor()||plan.until<=plan.from{return Err(TxError::InvalidOperation);}
    let fresh=plan_interval(r,l,plan.until)?;
    if &fresh!=plan{return Err(TxError::InvalidOperation);}
    r.validate_stocks(&r.stocks(l)?)?;
    let dt=plan.until-plan.from;
    let received=add(add(mul(r.genset.received_mwh,3)?,r.genset.received_remainder_numerator)?,mul(dt,20_000)?)?;
    let view=r.authority(l,plan.until,root,revision)?;
    let reception=g::Reception{scope:r.config.scope,clock:r.config.clock,genset_id:r.config.genset_id,source_version:r.config.source_version,
        run_id:r.genset.run_id,consumer_ref:r.config.consumer_ref,consumer_version:r.config.consumer_version,receipt_ref:operation,root,
        from:plan.from,until:plan.until,cumulative_received_mwh:received/3,received_remainder_numerator:received%3,actual_peak_w:24_000};
    let c=g::ContinuousGensetV1::propose(&r.config,&r.genset,&view,g::Command::Advance{at:instant(r,plan.until),reception}).map_err(bad)?;
    g::ContinuousGensetV1::revalidate(&r.config,&r.genset,&view,&c).map_err(bad)?;
    // Equal constant source/bus rates and this exact partition prove every
    // prefix; a constrained tail is a separate final cell, never a whole-window
    // average or an integer electric inventory credit.
    let energy=plan.pump_energy_mwh.add(plan.treatment_energy_mwh).and_then(|v|v.add(plan.dump_heat_mwh)).map_err(bad)?;
    if energy!=w::Exact::fraction(u128::from(dt)*20_000,3).map_err(bad)?{return Err(TxError::InvalidOperation);}
    r.materialize(l,c)?;
    adjust_input(l,&r.raw_lot,plan.raw_take_ml,0)?;
    r.raw_used_ml=add(r.raw_used_ml,plan.raw_take_ml)?;
    for n in &mut r.produced_ml{*n=add(*n,plan.each_produced_ml)?;}
    let run=r.run_mut().ok_or(TxError::InvalidOperation)?;run.production_seconds=add(run.production_seconds,dt)?;
    let batch=r.v2.as_deref_mut().and_then(|s|s.batch.as_deref_mut()).ok_or(TxError::InvalidOperation)?;
    let segment=batch.segments.iter_mut().flatten().last().ok_or(TxError::InvalidOperation)?;
    segment.generator_seconds=add(segment.generator_seconds,dt)?;segment.treatment_seconds=add(segment.treatment_seconds,dt)?;
    segment.processed_until=plan.next_processed_ml;segment.constrained_tail=plan.tail;
    if plan.tail.is_some(){segment.ended_at=Some(plan.until);}
    batch.pump_electricity_mwh=batch.pump_electricity_mwh.add(plan.pump_energy_mwh).map_err(bad)?;
    batch.treatment_electricity_mwh=batch.treatment_electricity_mwh.add(plan.treatment_energy_mwh).map_err(bad)?;
    batch.dump_electricity_heat_mwh=batch.dump_electricity_heat_mwh.add(plan.dump_heat_mwh).map_err(bad)?;
    let mut electric=0;let mut other=0;
    let mut run_labor=[0u64;MAX_RUNS];
    for s in &r.v2.as_deref().unwrap().batch.as_deref().unwrap().segments{
        let Some(s)=s else{continue;};
        if s.generator_seconds==0{continue;}
        let earned=s.earned_at_cells(s.generator_seconds,false)?;
        let cost=generator_cost_at(r,add(s.generator_seconds_base,s.generator_seconds)?)?.checked_sub(generator_cost_at(r,s.generator_seconds_base)?).ok_or(TxError::Stock)?;
        electric=add(electric,earned)?;
        other=add(other,add(add(if s.pumps==3{cost-cost/2}else{cost},if s.pumps==3{earned-earned/2}else{earned})?,s.other_start_cents)?)?;
        let i=r.runs.iter().position(|r|r.is_some_and(|r|r.id==s.source_run)).ok_or(TxError::InvalidOperation)?;
        run_labor[i]=add(run_labor[i],earned)?;
    }
    for(i,run)in r.runs.iter_mut().enumerate(){if let Some(run)=run{run.operator_labor_basis=m(run_labor[i])?;}}
    r.operator_labor_basis=m(electric)?;
    r.v2.as_deref_mut().unwrap().other_purpose_basis_cents=other;
    let gross=water_prefix(r,r.raw_used_ml)?;r.output_basis=[m(gross.ports_cents[0])?,m(gross.ports_cents[1])?];
    r.startup_wip=Money::ZERO;r.last_operation=operation;
    let outflow=r.v2.as_deref().unwrap().outflow;
    for i in 0..2{
        set_output(l,r.opening.stocks[i+3],r.produced_ml[i].checked_sub(outflow[i].ml).ok_or(TxError::Stock)?,
            r.output_basis[i].checked_sub(m(outflow[i].cents)?).map_err(bad)?)?;
    }
    let(heat,_)=r.heat()?;let meters=g::ContinuousGensetV1::meters(&r.config,&r.genset).map_err(bad)?;
    set_output(l,r.opening.stocks[5],heat,Money::ZERO)?;set_output(l,r.opening.stocks[6],meters.wastewater_ml,Money::ZERO)?;
    // The caller must not validate/commit this intermediate state: checkpoint,
    // product/cost WIP, actual member rows and money have not been adopted yet.
    Ok(())
}

pub(super) fn validate_stocks(r:&Runtime,s:&[Stock;7])->Result<(),TxError>{
    let state=r.v2.as_deref().ok_or(TxError::InvalidOperation)?;state.validate()?;
    let meters=g::ContinuousGensetV1::meters(&r.config,&r.genset).map_err(bad)?;
    for(i,lot,net,held)in[(0,&r.config.fuel,r.genset.fuel_taken_mq.checked_sub(r.genset.fuel_returned_mq).ok_or(TxError::Stock)?,meters.fuel_held_basis),
        (1,&r.config.water,r.genset.water_taken_ml.checked_sub(r.genset.water_returned_ml).ok_or(TxError::Stock)?,meters.water_held_basis),
        (2,&r.raw_lot,r.raw_used_ml,basis(&r.raw_lot,r.raw_used_ml)?)]{
        if s[i].id!=lot.stock_id||u64::from(s[i].lot_id)!=lot.lot_ref||s[i].origin_quantity!=lot.quantity
            ||s[i].origin_basis!=lot.basis||s[i].offset!=q(net)?||s[i].quantity!=lot.quantity.checked_sub(q(net)?).map_err(bad)?
            ||s[i].basis!=lot.basis.checked_sub(held).map_err(bad)?{return Err(TxError::Stock);}
    }
    let gross=water_prefix(r,r.raw_used_ml)?;
    if gross.ports_ml!=r.produced_ml||gross.ports_cents!=r.output_basis.map(|v|v.value()as u64){return Err(TxError::Stock);}
    let consumed=water_prefix(r,add(state.outflow[0].ml,state.outflow[1].ml)?)?;
    for i in 0..2{
        if consumed.ports_ml[i]!=state.outflow[i].ml||consumed.ports_cents[i]!=state.outflow[i].cents{return Err(TxError::Stock);}
        let amount=r.produced_ml[i].checked_sub(state.outflow[i].ml).ok_or(TxError::Stock)?;
        let cost=r.output_basis[i].checked_sub(m(state.outflow[i].cents)?).map_err(bad)?;
        let actual=s[i+3];
        if actual.quantity!=q(amount)?||actual.basis!=cost||actual.origin_quantity!=actual.quantity||actual.origin_basis!=cost||actual.offset!=Quantity::ZERO
            ||amount>r.opening.water_capacities_ml[i]{return Err(TxError::Stock);}
    }
    let(heat,rem)=r.heat()?;
    if u128::from(heat)*3+u128::from(rem)>u128::from(r.opening.heat_capacity_mwh)*3||meters.wastewater_ml>r.opening.wastewater_capacity_ml{return Err(TxError::Capacity);}
    for(i,amount)in[(5,heat),(6,meters.wastewater_ml)]{
        if s[i].quantity!=q(amount)?||s[i].basis!=Money::ZERO||s[i].origin_quantity!=s[i].quantity||s[i].origin_basis!=Money::ZERO||s[i].offset!=Quantity::ZERO{return Err(TxError::Stock);}
    }
    let actual_source_cost=r.converted_basis()?.checked_add(meters.start_fees).map_err(bad)?.checked_add(r.pump_start_fees).map_err(bad)?.checked_add(r.operator_labor_basis).map_err(bad)?;
    let placed=r.output_basis[0].checked_add(r.output_basis[1]).map_err(bad)?.checked_add(m(state.other_purpose_basis_cents)?).map_err(bad)?
        .checked_add(r.startup_wip).map_err(bad)?.checked_add(r.zero_work_expense).map_err(bad)?;
    if actual_source_cost!=placed{return Err(TxError::Stock);}
    // Segment/run/actual labor/energy recomputation is the additional gate in
    // the joint adapter, not replaced by this net Stock projection identity.
    Ok(())
}
