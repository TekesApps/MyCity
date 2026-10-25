//! Private original-writer treatment state. No alternate stock, wallet, clock or
//! ticket authority. A batch is bound to an actual original Start operation;
//! parsing this state alone never authenticates its funding or source history.
use crate::{assembly::{Stock,World},continuous_supply as source,transaction::TxError,water_treatment as w};
use serde::{Serialize,Deserialize};

pub(crate) const MAX_SEGMENTS:usize=8;
fn bad(_:impl std::fmt::Debug)->TxError{TxError::InvalidOperation}

/// One real physical/policy segment, independent of Timeline pagination. The
/// final constrained cell is represented explicitly instead of stretching its
/// fractional work over a whole preceding interval. All quantities are exact.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Segment {
    pub source_run:u64,
    pub source_start_revision:u64,
    pub started_at:u64,
    pub ended_at:Option<u64>,
    pub pumps:u8,
    pub generator_seconds_base:u64,
    pub raw_ml_base:u64,
    pub produced_each_base:u64,
    pub processed_base:w::Exact,
    pub processed_until:w::Exact,
    pub generator_seconds:u64,
    pub treatment_seconds:u64,
    pub electric_contract:u64,
    pub water_contract:u64,
    pub electric_hourly_cents:u64,
    pub electric_members:u64,
    pub electric_wage_carry:u16,
    pub water_hourly_cents:u64,
    pub water_members:u64,
    pub water_wage_carry:u16,
    pub water_start_cents:u64,
    pub other_start_cents:u64,
    pub constrained_tail:Option<TailCell>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TailCell {pub start:u64,pub input_ml:w::Exact}

fn integer(n:u64)->Result<w::Exact,TxError>{w::Exact::integer(n).map_err(bad)}
fn minimum(a:w::Exact,b:w::Exact)->Result<w::Exact,TxError>{Ok(if a.cmp_exact(b).map_err(bad)?==std::cmp::Ordering::Greater{b}else{a})}
fn multiply(a:w::Exact,b:w::Exact)->Result<w::Exact,TxError>{
    let(whole,rem,den)=b.parts();a.ratio(whole,1).and_then(|x|a.ratio(rem,den).and_then(|y|x.add(y))).map_err(bad)
}
pub(crate) fn divide(left:w::Exact,right:w::Exact)->Result<w::Exact,TxError>{
    fn pair(x:w::Exact)->(u128,u128){let(a,b,c)=x.parts();(u128::from(a)*u128::from(c)+u128::from(b),u128::from(c))}
    fn gcd(mut a:u128,mut b:u128)->u128{while b!=0{let r=a%b;a=b;b=r;}a.max(1)}
    let(mut a,mut b)=pair(left);let(mut c,mut d)=pair(right);
    if c==0{return Err(TxError::InvalidOperation);}
    let g=gcd(a,c);a/=g;c/=g;let g=gcd(d,b);d/=g;b/=g;
    w::Exact::fraction(a.checked_mul(d).ok_or(TxError::Overflow)?,b.checked_mul(c).ok_or(TxError::Overflow)?).map_err(bad)
}
impl Segment {
    pub(crate) fn input_rate(&self)->Result<w::Exact,TxError>{
        let power=match self.pumps{3=>12_000,0=>24_000,_=>return Err(TxError::InvalidOperation)};
        // W *1000mWh/Wh /3600s/h *5mL/mWh, never an integer energy credit.
        w::Exact::fraction(power*25,18).map_err(bad)
    }
    /// Time position only within this already observed physical segment. The
    /// result is independent of the last P5 page and bounded by actual cells.
    pub(crate) fn time_at_input(&self,prefix:w::Exact)->Result<Option<w::Exact>,TxError>{
        use std::cmp::Ordering;
        if prefix.cmp_exact(self.processed_base).map_err(bad)?!=Ordering::Greater{return Ok(None);}
        if prefix.cmp_exact(self.processed_until).map_err(bad)?==Ordering::Greater{return Err(TxError::InvalidOperation);}
        let delta=prefix.sub(self.processed_base).map_err(bad)?;
        let full=self.treatment_seconds.checked_sub(u64::from(self.constrained_tail.is_some())).ok_or(TxError::InvalidOperation)?;
        let rate=self.input_rate()?;let regular=rate.ratio(full,1).map_err(bad)?;
        let position=if delta.cmp_exact(regular).map_err(bad)?!=Ordering::Greater{
            divide(delta,rate)?
        }else{
            let tail=self.constrained_tail.ok_or(TxError::InvalidOperation)?;
            if tail.start!=self.started_at.checked_add(full).ok_or(TxError::Overflow)?||tail.input_ml.is_zero(){return Err(TxError::InvalidOperation);}
            integer(full)?.add(divide(delta.sub(regular).map_err(bad)?,tail.input_ml)?).map_err(bad)?
        };
        if position.ceil().map_err(bad)?>self.treatment_seconds{return Err(TxError::InvalidOperation);}
        Ok(Some(position))
    }
    pub(crate) fn completed_fee_cells_at_input(&self,prefix:w::Exact)->Result<u64,TxError>{
        self.time_at_input(prefix)?.map_or(Ok(0),|v|v.ceil().map_err(bad))
    }
    /// Original earned cost, never payment or an estimated staffing cost. Root
    /// still matches these endpoints against original same-Prepared work rows.
    pub(crate) fn earned_at_cells(&self,cells:u64,water:bool)->Result<u64,TxError>{
        if cells>self.treatment_seconds{return Err(TxError::InvalidOperation);}
        let(rate,members,carry)=if water{(self.water_hourly_cents,self.water_members,self.water_wage_carry)}
            else{(self.electric_hourly_cents,self.electric_members,self.electric_wage_carry)};
        if members==0||carry>=3600{return Err(TxError::InvalidOperation);}
        let n=u128::from(rate).checked_mul(u128::from(members)).and_then(|v|v.checked_mul(u128::from(cells)))
            .and_then(|v|v.checked_add(u128::from(carry))).ok_or(TxError::Overflow)?/3600;
        let n=u64::try_from(n).map_err(|_|TxError::Overflow)?;
        if n>i64::MAX as u64{return Err(TxError::Overflow);}Ok(n)
    }
}

/// The array stores canonical per-purpose cumulative basis already consumed by
/// this one original batch, not arbitrary claims named by a caller. Combined
/// with the immutable source/segment origin and receipt hash it bounds replay
/// checking without scanning an ever-growing journal or accepting renamed IDs.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Batch {
    pub config:w::FundingConfigV2,
    pub checkpoint:w::FundingCheckpointV2,
    pub start_revision:u64,
    pub consumed_cost:[w::Exact;w::FUNDING_COST_KINDS],
    pub segments:[Option<Segment>;MAX_SEGMENTS],
    pub product_credited_ml:u64,
    pub product_credited_basis_cents:u64,
    pub product_transferred_ml:u64,
    pub product_transferred_basis_cents:u64,
    pub product_cost_wip:w::Exact,
    pub water_earned_cents:u64,
    pub operation_paid_cents:u64,
    pub maintenance_paid_cents:u64,
    pub acceptance_paid_cents:u64,
    pub custody_paid_cents:u64,
    pub pump_electricity_mwh:w::Exact,
    pub treatment_electricity_mwh:w::Exact,
    pub dump_electricity_heat_mwh:w::Exact,
}

impl Batch {
    pub(crate) fn sludge_at(&self,input:w::Exact)->Result<w::Exact,TxError>{
        let w::Mass::Known(suspended)=self.config.material.raw.composition.suspended_mq else{return Err(TxError::InvalidOperation);};
        multiply(input.ratio(1,self.config.material.raw_origin.quantity_milli).map_err(bad)?,suspended)?
            .add(input.ratio(7,10_000_000).map_err(bad)?).map_err(bad)
    }
    pub(crate) fn due_fees_at(&self,o:&source::v2::Opening,input:w::Exact)->Result<[u64;4],TxError>{
        let mut cells=0u64;for s in self.segments.iter().flatten(){cells=cells.checked_add(s.completed_fee_cells_at_input(minimum(input,s.processed_until)?)?).ok_or(TxError::Overflow)?;}
        Ok([cells.checked_mul(o.fees.operation_cents_per_second).ok_or(TxError::Overflow)?,
            cells.checked_mul(o.fees.maintenance_cents_per_second).ok_or(TxError::Overflow)?,
            input.ratio(1,50_000).map_err(bad)?.ceil().map_err(bad)?.checked_mul(o.fees.concentrate_cents_per_started_litre).ok_or(TxError::Overflow)?,
            self.sludge_at(input)?.ceil().map_err(bad)?.checked_mul(o.fees.sludge_cents_per_started_mq).ok_or(TxError::Overflow)?])
    }
    pub(crate) fn water_earned_at(&self,input:w::Exact)->Result<u64,TxError>{
        self.segments.iter().flatten().try_fold(0u64,|n,s|n.checked_add(s.earned_at_cells(s.completed_fee_cells_at_input(minimum(input,s.processed_until)?)?,true)?).ok_or(TxError::Overflow))
    }
    pub(crate) fn reagent_basis_at(&self,input:w::Exact)->Result<w::Exact,TxError>{
        input.ratio(1,1_000_000).and_then(|v|v.ratio(self.config.material.reagent_origin.basis_cents,self.config.material.reagent_origin.quantity_milli)).map_err(bad)
    }
    /// Prefix cost of already completed production only. Fixed original cells
    /// and residual quantity thresholds are independent of P5 pagination.
    pub(crate) fn product_basis_at(r:&source::Runtime,input:w::Exact)->Result<w::Exact,TxError>{
        let b=r.treatment_batch().ok_or(TxError::InvalidOperation)?;
        let o=r.treatment_opening().ok_or(TxError::InvalidOperation)?;
        if input.cmp_exact(b.checkpoint.physical.processed_ml).map_err(bad)?==std::cmp::Ordering::Greater{return Err(TxError::InvalidOperation);}
        let(_,raw)=source::v2::consumed_water_basis(r,input)?;
        let electric=source::v2::electric_parts(r,Some(input))?;let fees=b.due_fees_at(o,input)?;
        let mut cost=raw.converted.add(b.reagent_basis_at(input)?).map_err(bad)?;
        for n in electric.into_iter().chain(fees).chain([b.water_earned_at(input)?]){cost=cost.add(integer(n)?).map_err(bad)?;}Ok(cost)
    }
    pub(crate) fn input_limit(&self,declared:&source::v2::Opening)->Result<w::Exact,TxError>{
        use std::cmp::Ordering;
        let c=&self.config.material;let full=c.raw_origin.quantity_milli;
        let reagent=w::Exact::fraction(u128::from(c.reagent_origin.quantity_milli)*1_000_000,1).map_err(bad)?;
        let product=w::Exact::fraction(u128::from(declared.outlet_capacities[0])*50,49).map_err(bad)?;
        let concentrate=w::Exact::fraction(u128::from(declared.outlet_capacities[1])*50,1).map_err(bad)?;
        let w::Mass::Known(suspended)=c.raw.composition.suspended_mq else{return Err(TxError::InvalidOperation);};
        let full_sludge=suspended.add(w::Exact::fraction(u128::from(full)*7,10_000_000).map_err(bad)?).map_err(bad)?;
        let sludge=divide(w::Exact::fraction(u128::from(declared.outlet_capacities[2])*u128::from(full),1).map_err(bad)?,full_sludge)?;
        let mut limit=integer(full)?;
        for bound in [reagent,product,concentrate,sludge]{if bound.cmp_exact(limit).map_err(bad)?==Ordering::Less{limit=bound;}}
        Ok(limit)
    }
    /// Called only while staging the original accepted source Start, after the
    /// root has checked its real actor, admitted workers, funds and resources.
    /// The operation is neither predicted during Opening nor synthesized later.
    pub(crate) fn from_original_start(world:&World,proposed_source:&source::Runtime,
        operation:u64,revision:u64,at:u64,reagent:Stock)->Result<Box<Self>,TxError>{
        let base=proposed_source.opening();
        let source::Profile::ContinuousGensetTwinPumpTreatmentV2(declared)=&base.profile else{return Err(TxError::InvalidOperation);};
        let observed=proposed_source.view()?;
        let first=observed.runs[0].ok_or(TxError::InvalidOperation)?;
        if observed.genset.run_count!=1||observed.genset.status!=crate::continuous_genset::Status::Running
            ||first.operation!=operation||first.started_at!=at||first.id!=observed.genset.run_id
            ||world.revision.checked_add(1)!=Some(revision)
            ||operation==0||at<declared.receiving_from||at>=declared.receiving_until
            ||reagent.id!=declared.reagent_stock||reagent.offset!=crate::units::Quantity::ZERO{return Err(TxError::InvalidOperation);}
        let raw=world.stocks.get(world.stock_handle(base.stocks[2]).map_err(bad)?).map_err(bad)?;
        let lot=|s:&Stock|->Result<w::InputLot,TxError>{Ok(w::InputLot{stock_id:s.id,lot_ref:u64::from(s.lot_id),
            quantity_milli:u64::try_from(s.origin_quantity.value()).map_err(bad)?,basis_cents:u64::try_from(s.origin_basis.value()).map_err(bad)?})};
        let config=w::FundingConfigV2{version:w::FUNDING_VERSION,profile:w::FundingProfileV2::ActualFundingV2,
            material:w::Config{version:w::VERSION,precision_version:w::PRECISION_VERSION,profile:w::Profile::RawPurificationV1,
                binding:w::Binding{world_ref:world.scope.world(),root_epoch:world.scope.branch(),source_version:u64::from(source::v2::VERSION),city_id:base.city_id,operation,origin_cursor:at},
                raw:declared.source_quality.clone(),raw_origin:lot(raw)?,reagent_origin:lot(&reagent)?,
                raw_basis:w::RawBasisMode::RootConfirmedStream{ports:[base.stocks[3],base.stocks[4]]},
                output_stock:declared.outlet_stocks[0],concentrate_stock:declared.outlet_stocks[1],sludge_stock:declared.outlet_stocks[2]},
            accrual_policy:declared.wage_settlement.accrual_policy()};
        let checkpoint=config.open().map_err(bad)?.checkpoint();
        Ok(Box::new(Self{config,checkpoint,start_revision:revision,consumed_cost:[w::Exact::ZERO;w::FUNDING_COST_KINDS],segments:std::array::from_fn(|_|None),
            product_credited_ml:0,product_credited_basis_cents:0,product_transferred_ml:0,product_transferred_basis_cents:0,product_cost_wip:w::Exact::ZERO,
            water_earned_cents:0,operation_paid_cents:0,maintenance_paid_cents:0,acceptance_paid_cents:0,custody_paid_cents:0,
            pump_electricity_mwh:w::Exact::ZERO,treatment_electricity_mwh:w::Exact::ZERO,dump_electricity_heat_mwh:w::Exact::ZERO}))
    }
    pub(crate) fn validate_structure(&self)->Result<(),TxError>{
        if self.start_revision==0||self.config.material.binding.source_version!=u64::from(source::v2::VERSION){return Err(TxError::InvalidOperation);}
        let state=self.config.restore(self.checkpoint.clone()).map_err(bad)?;
        let meters=self.config.meters(&state).map_err(bad)?;
        let sum=self.consumed_cost.iter().try_fold(w::Exact::ZERO,|a,b|a.add(*b)).map_err(bad)?;
        if sum!=meters.costs.total_basis{return Err(TxError::InvalidOperation);}
        let product=self.product_credited_ml.checked_add(self.product_transferred_ml).ok_or(TxError::Overflow)?;
        let basis=self.product_credited_basis_cents.checked_add(self.product_transferred_basis_cents).ok_or(TxError::Overflow)?;
        if product!=meters.materials.product.water_ml.floor()
            ||w::Exact::integer(basis).and_then(|v|v.add(self.product_cost_wip)).map_err(bad)?!=meters.costs.total_basis
            ||self.treatment_electricity_mwh!=meters.materials.electricity_mwh{return Err(TxError::InvalidOperation);}
        if self.consumed_cost[0]!=meters.materials.raw_converted_basis||self.consumed_cost[1]!=meters.materials.reagent_converted_basis{return Err(TxError::InvalidOperation);}
        let mut gap=false;let mut previous=None;let mut open=false;let mut processed=w::Exact::ZERO;
        for segment in &self.segments{match segment{
            None=>gap=true,
            Some(s)=>{
                if gap||open||s.source_run==0||s.source_start_revision==0||s.pumps!=0&&s.pumps!=3||s.started_at<self.config.material.binding.origin_cursor
                    ||s.electric_contract==0||s.water_contract==0||s.electric_contract==s.water_contract
                    ||s.electric_members==0||s.water_members==0||s.electric_hourly_cents==0||s.water_hourly_cents==0
                    ||s.electric_wage_carry>=3600||s.water_wage_carry>=3600||s.treatment_seconds>s.generator_seconds
                    ||s.ended_at.is_some_and(|v|v<s.started_at)
                    ||previous.is_some_and(|v|v>s.started_at){return Err(TxError::InvalidOperation);}
                let end=s.started_at.checked_add(s.generator_seconds).ok_or(TxError::Overflow)?;
                if end>self.checkpoint.physical.cursor||s.ended_at.is_some_and(|v|v!=end)||s.processed_base!=processed{return Err(TxError::InvalidOperation);}
                let full=s.treatment_seconds.checked_sub(u64::from(s.constrained_tail.is_some())).ok_or(TxError::InvalidOperation)?;
                let expected=s.processed_base.add(s.input_rate()?.ratio(full,1).map_err(bad)?).map_err(bad)?
                    .add(s.constrained_tail.map_or(w::Exact::ZERO,|t|t.input_ml)).map_err(bad)?;
                if s.processed_until!=expected{return Err(TxError::InvalidOperation);}
                if let Some(t)=s.constrained_tail{
                    if t.input_ml.is_zero()||t.start<s.started_at||s.ended_at!=t.start.checked_add(1){return Err(TxError::InvalidOperation);}
                }
                previous=s.ended_at;open=previous.is_none();processed=s.processed_until;
            }
        }}
        if processed!=self.checkpoint.physical.processed_ml{return Err(TxError::InvalidOperation);}
        // These are structural gates only; root additionally recomputes source
        // curves, actual worker coverage, payments, Stock projections and energy
        // from this same Prepared and authenticates original restore receipts.
        Ok(())
    }
}

pub(crate) struct CheckedTreatment {
    checkpoint:w::FundingCheckpointV2,
    delta:w::MaterialDeltaV2,
    costs:[w::Exact;8],
    water:u64,
    fees:[u64;4],
}
impl CheckedTreatment {
    pub(crate) fn checkpoint(&self)->&w::FundingCheckpointV2{&self.checkpoint}
    pub(crate) fn delta(&self)->&w::MaterialDeltaV2{&self.delta}
    pub(crate) fn install(self,b:&mut Batch){
        b.checkpoint=self.checkpoint;b.consumed_cost=self.costs;b.water_earned_cents=self.water;
        [b.operation_paid_cents,b.maintenance_paid_cents,b.acceptance_paid_cents,b.custody_paid_cents]=self.fees;
    }
}
struct Witness<'a>{config:&'a w::FundingConfigV2,base:&'a w::FundingCheckpointV2,request:w::Request,facts:&'a w::FundingFactsV2}
impl w::RootVerifier for Witness<'_>{
    fn verifies(&self,_:&w::Config,_:&w::Checkpoint,_:w::Request,_:&w::RootFacts)->bool{false}
    fn verifies_funding_v2(&self,c:&w::FundingConfigV2,b:&w::FundingCheckpointV2,q:w::Request,f:&w::FundingFactsV2)->bool{c==self.config&&b==self.base&&q==self.request&&f==self.facts}
    fn verifies_accrual_v2(&self,c:&w::FundingConfigV2,b:&w::FundingCheckpointV2,q:w::Request,f:&w::FundingFactsV2,i:usize)->bool{
        self.verifies_funding_v2(c,b,q,f)&&f.items.get(i).and_then(|v|*v).is_some_and(|x|
            matches!(x.coverage,w::CoverageV2::AccruedPayable{policy,..}if Some(policy)==c.accrual_policy)
                &&matches!(x.kind,w::CostKindV2::ElectricityInput|w::CostKindV2::Personnel)
                &&matches!(x.coverage,w::CoverageV2::AccruedPayable{evidence,..}if evidence.allocation_ref==5||evidence.allocation_ref==6))
    }
}
fn cover_idle(r:&mut source::Runtime,at:u64,op:u64,revision:u64)->Result<(),TxError>{
    if let Some(b)=r.treatment_batch(){
        let from=b.checkpoint.physical.cursor;if at==from{return Ok(());}
        if b.segments.iter().flatten().any(|s|s.ended_at.is_none_or(|end|end>from)){return Err(TxError::InvalidOperation);}
        struct Idle<'a>{c:&'a w::FundingConfigV2,b:&'a w::FundingCheckpointV2,f:&'a w::FundingIdleFactsV2}
        impl w::RootVerifier for Idle<'_>{
            fn verifies(&self,_:&w::Config,_:&w::Checkpoint,_:w::Request,_:&w::RootFacts)->bool{false}
            fn verifies_funding_idle_v2(&self,c:&w::FundingConfigV2,b:&w::FundingCheckpointV2,f:&w::FundingIdleFactsV2)->bool{c==self.c&&b==self.b&&f==self.f}
        }
        let facts=w::FundingIdleFactsV2{version:2,material:w::IdleFacts{policy_version:w::IDLE_POLICY_VERSION,binding:b.config.material.binding,
            from,until:at,read_version:revision,timeline_revision:revision,idle_receipt_ref:op},funding_revision:revision};
        let proof=Idle{c:&b.config,b:&b.checkpoint,f:&facts};let base=b.config.restore(b.checkpoint.clone()).map_err(bad)?;
        let candidate=b.config.cover_idle(&proof,&base,&facts,1).map_err(bad)?;
        let next=b.config.revalidate_idle(&proof,&base,&candidate,&facts).map_err(bad)?.checkpoint();
        r.treatment_batch_mut().unwrap().checkpoint=next;
    }
    source::v2::idle_cursor(r,at,op)
}
pub(crate) fn validate_material_stocks(r:&source::Runtime,mut stock:impl FnMut(u32)->Result<Stock,TxError>)->Result<(),TxError>{
    let Some(b)=r.treatment_batch()else{return Ok(());};let o=r.treatment_opening().ok_or(TxError::InvalidOperation)?;
    b.validate_structure()?;let m=b.config.meters(&b.config.restore(b.checkpoint.clone()).map_err(bad)?).map_err(bad)?.materials;
    let reagent=stock(o.reagent_stock)?;let origin=b.config.material.reagent_origin;
    if reagent.id!=origin.stock_id||u64::from(reagent.lot_id)!=origin.lot_ref||reagent.origin_quantity!=qty(origin.quantity_milli)?
        ||reagent.origin_basis!=money(origin.basis_cents)?||reagent.offset!=qty(m.reagent_taken_mq)?
        ||reagent.quantity!=qty(origin.quantity_milli-m.reagent_taken_mq)?||reagent.basis!=money(origin.basis_cents-m.reagent_held_basis_cents)?{return Err(TxError::Stock);}
    let(_,raw)=source::v2::consumed_water_basis(r,b.checkpoint.physical.processed_ml)?;
    if b.checkpoint.physical.raw_stream_basis!=Some(raw){return Err(TxError::Stock);}
    let input=b.checkpoint.physical.processed_ml;let electric=source::v2::electric_parts(r,None)?;let fees=b.due_fees_at(o,input)?;
    let expected=[raw.converted,b.reagent_basis_at(input)?,integer(total(electric)?)?,integer(b.water_earned_at(input)?)?,integer(fees[0])?,integer(fees[1])?,integer(fees[2])?,integer(fees[3])?];
    if expected!=b.consumed_cost{return Err(TxError::Stock);}
    let f=b.checkpoint.funding;let wages=integer(electric[2])?.add(expected[3]).map_err(bad)?;
    if f.original_stock!=expected[0].add(expected[1]).and_then(|v|v.add(integer(electric[0]).map_err(|_|w::Error::Invalid)?)).map_err(bad)?
        ||f.prior_receipt_paid!=integer(electric[1])?||f.cash_due_now!=fees.into_iter().try_fold(w::Exact::ZERO,|v,n|v.add(w::Exact::integer(n)?)).map_err(bad)?
        ||f.same_prepared_paid!=if b.config.accrual_policy.is_none(){wages}else{w::Exact::ZERO}
        ||f.accrued_payable!=if b.config.accrual_policy.is_some(){wages}else{w::Exact::ZERO}{return Err(TxError::Stock);}
    let amount=m.product.water_ml.floor();let cost=Batch::product_basis_at(r,integer(amount)?.ratio(50,49).map_err(bad)?)?.floor();
    let moved=Batch::product_basis_at(r,integer(b.product_transferred_ml)?.ratio(50,49).map_err(bad)?)?.floor();
    if moved!=b.product_transferred_basis_cents||b.product_transferred_ml>o.product_storage.capacity_ml{return Err(TxError::Stock);}
    if b.product_credited_ml!=amount.checked_sub(b.product_transferred_ml).ok_or(TxError::Stock)?
        ||b.product_credited_basis_cents!=cost.checked_sub(b.product_transferred_basis_cents).ok_or(TxError::Stock)?{return Err(TxError::Stock);}
    for(id,amount,cost)in[(o.outlet_stocks[0],b.product_credited_ml,b.product_credited_basis_cents),(o.outlet_stocks[1],m.concentrate.water_ml.floor(),0),(o.outlet_stocks[2],b.sludge_at(input)?.floor(),0),(o.product_storage.stock,b.product_transferred_ml,moved)]{
        let s=stock(id)?;if s.quantity!=qty(amount)?||s.basis!=money(cost)?||s.origin_quantity!=s.quantity||s.origin_basis!=s.basis||s.offset!=crate::units::Quantity::ZERO{return Err(TxError::Stock);}
    }Ok(())
}
fn money(n:u64)->Result<crate::units::Money,TxError>{crate::units::Money::new(i64::try_from(n).map_err(|_|TxError::Overflow)?).map_err(bad)}
fn qty(n:u64)->Result<crate::units::Quantity,TxError>{crate::units::Quantity::new(i64::try_from(n).map_err(|_|TxError::Overflow)?).map_err(bad)}
fn quantity(s:Stock)->Result<u64,TxError>{u64::try_from(s.quantity.value()).map_err(bad)}
fn total(a:[u64;3])->Result<u64,TxError>{a.into_iter().try_fold(0u64,|n,v|n.checked_add(v).ok_or(TxError::Overflow))}
fn item(items:&mut[Option<w::FundingItemV2>;16],kind:w::CostKindV2,coverage:fn(w::FundingEvidenceV2)->w::CoverageV2,e:w::FundingEvidenceV2)->Result<(),TxError>{
    let basis=e.cost_until.sub(e.cost_from).map_err(bad)?;if basis.is_zero(){return Ok(());}
    *items.iter_mut().find(|x|x.is_none()).ok_or(TxError::WorkingLimit)?=Some(w::FundingItemV2{kind,basis,coverage:coverage(e)});Ok(())
}
fn original(e:w::FundingEvidenceV2)->w::CoverageV2{w::CoverageV2::OriginalStockConversion{evidence:e}}
fn prior(e:w::FundingEvidenceV2)->w::CoverageV2{w::CoverageV2::PriorReceiptPaid{evidence:e}}
fn due(e:w::FundingEvidenceV2)->w::CoverageV2{w::CoverageV2::CashDueNow{evidence:e}}

/// Original P5's one atomic source/work/funding/material path. Only root calls
/// this with its transaction-local clones and original ledger; no candidate DTO
/// or caller-supplied verified flag can enter the witness constructor.
pub(crate) fn advance(world:&World,r:&mut source::Runtime,f:&mut crate::facility_authority::Runtime,
    members:&mut crate::member_activity::Ledger,l:&mut(impl crate::exchange::Ledger+crate::facility_authority::Funds),
    at:u64,op:u64,root:[u8;32],revision:u64)->Result<(),TxError>{
    if at==r.cursor(){
        if r.view()?.genset.status==crate::continuous_genset::Status::Running&&source::v2::boundary(r,l,at)?==Some(at){
            source::v2::stop(r,l,at,op,root,revision,source::StopCause::InputOrReceiverBoundary)?;
            crate::facility_authority::close_for_source(f,members,r,op,at,l)?;
        }
        return Ok(());
    }
    if at<r.cursor()||at-r.cursor()>3600{return Err(TxError::InvalidOperation);}
    if r.view()?.genset.status!=crate::continuous_genset::Status::Running{
        cover_idle(r,at,op,revision)?;
        crate::facility_authority::settle_device(world,f,members,r,op,at,l)?;return Ok(());
    }
    let from=r.cursor();
    if !crate::facility_authority::can_continue(world,f,from)?{
        r.stop_for_operator(l,from,op,root,revision)?;
        crate::facility_authority::close_for_source(f,members,r,op,from,l)?;
        return cover_idle(r,at,op,revision);
    }
    let old=r.treatment_batch().ok_or(TxError::InvalidOperation)?.clone();
    let old_electric=source::v2::electric_parts(r,None)?;
    let plan=source::v2::plan_interval(r,l,at)?;
    source::v2::stage_physical(r,l,&plan,op,root,revision)?;
    let work_before=*f.state.records();
    crate::facility_authority::settle_device(world,f,members,r,op,at,l)?;
    let o=r.treatment_opening().ok_or(TxError::InvalidOperation)?.clone();
    let b=r.treatment_batch().ok_or(TxError::InvalidOperation)?;
    for (before,after) in work_before.into_iter().zip(f.state.records()){
        let Some((before,after))=before.zip(*after)else{continue;};
        let earned=after.earned.checked_sub(before.earned).ok_or(TxError::InvalidOperation)?;
        let paid=after.paid.checked_sub(before.paid).ok_or(TxError::InvalidOperation)?;
        if b.config.accrual_policy.is_some(){if paid!=0{return Err(TxError::Protection);}}
        else if paid!=earned||paid>0&&after.last_payment.is_none_or(|p|p.source.operation_id!=op||p.source.occurred_at!=at||p.cash_operation_id!=op||p.cents!=paid){return Err(TxError::Protection);}
    }
    let input=plan.next_processed_ml;let electric=source::v2::electric_parts(r,None)?;
    let water=b.water_earned_at(input)?;let fees=b.due_fees_at(&o,input)?;
    let(_,raw)=source::v2::consumed_water_basis(r,input)?;
    let costs=[raw.converted,b.reagent_basis_at(input)?,integer(total(electric)?)?,integer(water)?,integer(fees[0])?,integer(fees[1])?,integer(fees[2])?,integer(fees[3])?];
    // Independent original records must agree with the provisional curve; no
    // estimated labor becomes a material cost or permission to keep running.
    let earned=f.state.opening().contracts.iter().zip(f.state.records()).filter_map(|(c,a)|c.zip(*a))
        .try_fold([0u64;2],|mut sum,(c,a)|{let i=usize::from(c.role==crate::facility_work::Role::Water);sum[i]=sum[i].checked_add(a.earned).ok_or(TxError::Overflow)?;Ok::<_,TxError>(sum)})?;
    if money(earned[0])?!=r.view()?.operator_labor_basis||earned[1]!=water
        ||l.protection(r.opening().owner)?<crate::facility_authority::required_protection(Some(f),r.opening().owner)?{return Err(TxError::Protection);}
    let delta:Vec<_>=costs.iter().zip(old.consumed_cost).map(|(a,b)|a.sub(b).map_err(bad)).collect::<Result<_,_>>()?;
    let state=old.config.restore(old.checkpoint.clone()).map_err(bad)?;
    let meters=old.config.meters(&state).map_err(bad)?.materials;
    let p=w::MaterialFactsV2{binding:old.config.material.binding,from,until:at,read_version:revision,
        raw_stock_available_ml:quantity(l.stock(r.opening().stocks[3])?)?.checked_add(quantity(l.stock(r.opening().stocks[4])?)?).ok_or(TxError::Overflow)?,
        reagent_stock_available_mq:quantity(l.stock(o.reagent_stock)?)?,electricity_available_mwh:plan.treatment_energy_mwh,raw_stream_basis_after:Some(raw),
        output_free_ml:integer(o.outlet_capacities[0])?.sub(meters.product.water_ml).map_err(bad)?,
        concentrate_free_ml:integer(o.outlet_capacities[1])?.sub(meters.concentrate.water_ml).map_err(bad)?,
        sludge_free_mq:integer(o.outlet_capacities[2])?.sub(old.sludge_at(old.checkpoint.physical.processed_ml)?).map_err(bad)?,supply_receipt_ref:op};
    let evidence=|reference,version,purpose,allocation,lo,hi|w::FundingEvidenceV2{source_ref:reference,source_revision:version,from,until:at,purpose_ref:purpose,allocation_ref:allocation,cost_from:lo,cost_until:hi};
    let mut items=[None;16];let source_id=r.opening().source_id;
    item(&mut items,w::CostKindV2::RawInput,original,evidence(source_id,2,source_id,1,old.consumed_cost[0],costs[0]))?;
    item(&mut items,w::CostKindV2::ReagentInput,original,evidence(old.config.material.reagent_origin.lot_ref,1,u64::from(o.reagent_stock),2,old.consumed_cost[1],costs[1]))?;
    item(&mut items,w::CostKindV2::ElectricityInput,original,evidence(source_id,2,u64::from(r.opening().resources[0]),3,integer(old_electric[0])?,integer(electric[0])?))?;
    for(i,s)in b.segments.iter().enumerate(){let Some(s)=s else{continue;};let before=old.segments[i].as_ref().map_or(0,|s|s.generator_seconds);
        if s.generator_seconds>0&&before==0&&s.other_start_cents>0{
            let run=r.view()?.runs.into_iter().flatten().find(|v|v.id==s.source_run).ok_or(TxError::InvalidOperation)?;
            item(&mut items,w::CostKindV2::ElectricityInput,prior,evidence(run.operation,s.source_start_revision,u64::from(r.opening().resources[0]),4,w::Exact::ZERO,integer(s.other_start_cents)?))?;
        }
    }
    let current=b.segments.iter().flatten().last().ok_or(TxError::InvalidOperation)?;
    for(kind,contract,allocation,lo,hi)in[(w::CostKindV2::ElectricityInput,current.electric_contract,5,integer(old_electric[2])?,integer(electric[2])?),
        (w::CostKindV2::Personnel,current.water_contract,6,old.consumed_cost[3],costs[3])]{
        let basis=hi.sub(lo).map_err(bad)?;if basis.is_zero(){continue;}
        let e=evidence(op,revision,contract,allocation,lo,hi);
        let coverage=if let Some(policy)=old.config.accrual_policy{w::CoverageV2::AccruedPayable{evidence:e,policy}}else{w::CoverageV2::SamePreparedPaid{evidence:e}};
        *items.iter_mut().find(|v|v.is_none()).ok_or(TxError::WorkingLimit)?=Some(w::FundingItemV2{kind,basis,coverage});
    }
    for(i,kind)in[w::CostKindV2::EquipmentOperation,w::CostKindV2::MaintenanceUse,w::CostKindV2::ResidualAcceptance,w::CostKindV2::ResidualCustody].into_iter().enumerate(){
        item(&mut items,kind,due,evidence(op,revision,o.fees.contract_ref,7+i as u64,old.consumed_cost[4+i],costs[4+i]))?;
    }
    let cash=l.cash(r.opening().owner)?.checked_sub(l.protection(r.opening().owner)?).map_err(bad)?;
    let facts=w::FundingFactsV2{version:w::FUNDING_VERSION,physical:p,basis:w::IntervalBasisV2{electricity_cents:delta[2].floor(),personnel_cents:delta[3].floor(),
        equipment_operation_cents:delta[4].floor(),maintenance_use_cents:delta[5].floor(),residual_acceptance_cents:delta[6].floor(),residual_custody_cents:delta[7].floor()},items,cash_available_cents:u64::try_from(cash.value()).map_err(bad)?,funding_revision:revision};
    let request=w::Request{until:at,input_ml:plan.input_ml};let proof=Witness{config:&old.config,base:&old.checkpoint,request,facts:&facts};
    let candidate=old.config.prepare(&proof,&state,request,&facts,1).map_err(bad)?;
    let next=old.config.revalidate(&proof,&state,&candidate,&facts).map_err(bad)?;
    for i in 0..4{let payee=if i<2{r.opening().start_supplier}else{o.receiver_account};l.pay(r.opening().owner,payee,money(delta[4+i].floor())?)?;}
    let checked=CheckedTreatment{checkpoint:next.checkpoint(),delta:candidate.delta.materials,costs,water,fees};
    source::v2::adopt_treatment(r,l,checked)?;
    let gross=r.treatment_batch().unwrap().config.meters(&r.treatment_batch().unwrap().config.restore(r.treatment_batch().unwrap().checkpoint.clone()).map_err(bad)?).map_err(bad)?.materials;
    let product=gross.product.water_ml.floor();let cost=Batch::product_basis_at(r,integer(product)?.ratio(50,49).map_err(bad)?)?.floor();
    let b=r.treatment_batch_mut().unwrap();b.product_credited_ml=product.checked_sub(b.product_transferred_ml).ok_or(TxError::Stock)?;
    b.product_credited_basis_cents=cost.checked_sub(b.product_transferred_basis_cents).ok_or(TxError::Stock)?;b.product_cost_wip=b.checkpoint.funding.total_basis.sub(integer(cost)?).map_err(bad)?;
    for(id,amount,cost)in[(o.outlet_stocks[0],b.product_credited_ml,b.product_credited_basis_cents),(o.outlet_stocks[1],gross.concentrate.water_ml.floor(),0),(o.outlet_stocks[2],b.sludge_at(input)?.floor(),0)]{
        let mut s=l.stock(id)?;s.quantity=qty(amount)?;s.basis=money(cost)?;s.origin_quantity=s.quantity;s.origin_basis=s.basis;s.offset=crate::units::Quantity::ZERO;l.put(id,s)?;
    }
    if plan.stop_all||!crate::facility_authority::can_continue(world,f,at)?{
        source::v2::stop(r,l,at,op,root,revision,source::StopCause::InputOrReceiverBoundary)?;
        crate::facility_authority::close_for_source(f,members,r,op,at,l)?;
    }else if plan.stop_pumps{source::v2::stop_pumps(r,l,f,at,revision)?;}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(n:u128,d:u128)->w::Exact{w::Exact::fraction(n,d).unwrap()}
    fn segment(seconds:u64)->Segment{Segment{source_run:1,source_start_revision:1,started_at:0,ended_at:None,pumps:3,generator_seconds_base:0,raw_ml_base:0,produced_each_base:0,
        processed_base:w::Exact::ZERO,processed_until:f(u128::from(seconds)*50_000,3),generator_seconds:seconds,treatment_seconds:seconds,
        electric_contract:1,water_contract:2,electric_hourly_cents:3601,electric_members:1,electric_wage_carry:0,
        water_hourly_cents:3601,water_members:1,water_wage_carry:0,water_start_cents:550,other_start_cents:150,constrained_tail:None}}
    #[test]
    fn exact_cell_boundary_does_not_charge_next_actual_second(){
        let s=segment(4);
        assert_eq!(s.completed_fee_cells_at_input(f(0,1)).unwrap(),0);
        // Independent12kW/0.2mWh/mL oracle: exactly50000mL takes3s.
        assert_eq!(s.time_at_input(f(50_000,1)).unwrap(),Some(f(3,1)));
        assert_eq!(s.completed_fee_cells_at_input(f(50_000,1)).unwrap(),3);
        assert_eq!(s.earned_at_cells(3,true).unwrap(),3);
        assert_eq!(s.completed_fee_cells_at_input(f(2_450_001,49)).unwrap(),4);
        assert_eq!(s.earned_at_cells(4,true).unwrap(),4);
        // Later observed work extends admission, never changes an old prefix.
        assert_eq!(segment(17).completed_fee_cells_at_input(f(50_000,1)),s.completed_fee_cells_at_input(f(50_000,1)));
        assert!(s.completed_fee_cells_at_input(f(70_000,1)).is_err());
    }
    #[test]
    fn sub_millilitre_final_product_keeps_tail_cell_cost_out_of_old_stock(){
        let mut s=segment(4);s.ended_at=Some(4);s.processed_until=f(100_001,2);
        s.constrained_tail=Some(TailCell{start:3,input_ml:f(1,2)});
        // Actual final cell treated0.5mL ->0.49mL product. Integer product
        // stays49000mL, so its input position remains exactly50000mL/3s.
        let product=s.processed_until.ratio(49,50).unwrap();assert_eq!(product.floor(),49_000);
        let credited_input=w::Exact::integer(product.floor()).unwrap().ratio(50,49).unwrap();
        assert_eq!(s.completed_fee_cells_at_input(credited_input).unwrap(),3);
        assert_eq!(s.completed_fee_cells_at_input(s.processed_until).unwrap(),4);
        assert_eq!(s.time_at_input(s.processed_until).unwrap(),Some(f(4,1)));
        let restored:Segment=serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(restored.completed_fee_cells_at_input(credited_input).unwrap(),3);
    }
    #[test]
    fn pump_off_segment_derives_remaining_work_and_keeps_original_wage_carry(){
        let mut s=segment(500);s.pumps=0;s.started_at=5000;s.ended_at=Some(5500);
        s.generator_seconds_base=5000;s.raw_ml_base=100_000_000;s.produced_each_base=50_000_000;
        s.processed_base=f(250_000_000,3);s.processed_until=f(100_000_000,1);
        s.electric_wage_carry=1400;s.water_wage_carry=1400;s.water_start_cents=0;s.other_start_cents=0;
        // Remaining50/3m3 at24kW requires500 seconds; no hardcoded5500
        // exists in the execution helper. Only this independent fixture has it.
        assert_eq!(s.time_at_input(s.processed_until).unwrap(),Some(f(500,1)));
        assert_eq!(s.earned_at_cells(500,true).unwrap(),500);
        assert_eq!(s.earned_at_cells(500,false).unwrap(),500);
        assert_eq!(5001+s.earned_at_cells(500,true).unwrap(),5501);
        assert!(s.earned_at_cells(501,true).is_err());
    }
}
