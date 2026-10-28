//! One original-ledger continuous production owner. No inventory, wallet or clock pool.
//!
//! This explicit laboratory profile jointly runs a genset and one/two pumps. Water
//! is PROCESS WATER, without a sanitary certificate. All received electric energy
//! becomes pump heat; unreceived energy becomes genset heat. Neither is a battery.
use crate::{assembly::{Commodity, Stock, World},
    continuous_genset as g, exchange::Ledger, transaction::{Resource, Lease, TxError},
    units::{Money, Quantity}};
use serde::{Deserialize, Deserializer, Serialize};
use std::mem::size_of;
use sha2::{Digest, Sha256};
mod cost_curve;
pub mod v2;

pub const VERSION:u32=1;
pub const MAX_OWNED_BYTES:usize=16*1024;
pub const MAX_WORKSPACE_BYTES:usize=96*1024;
pub const MAX_RUNS:usize=16;
const PUMP_START_CENTS:u64=200;
const RATE_RAW:u64=10_000; // mL per Development second, per actually running pump
const HEAT_NUM:u64=50_000; // total mWh per second, denominator three

pub fn budget_layout()->serde_json::Value{serde_json::json!({
    "runtime_owned_bytes":size_of::<Runtime>(),"owned_cap_bytes":MAX_OWNED_BYTES,
    "genset_candidate_header_bytes":size_of::<g::Candidate>(),
    "seven_stock_read_bytes":7*size_of::<Stock>(),"joint_extra_reserved_bytes":MAX_WORKSPACE_BYTES,
    "max_runs":MAX_RUNS,"native_stack_gpu_and_16gb":"not_measured"
    ,"v2_opening_backing_bytes":size_of::<v2::Opening>(),"v2_state_backing_bytes":size_of::<v2::State>(),
    "runtime_owned_bytes_excludes_optional_backing":true
})}
pub(crate) fn check_joint_peak(existing_and_other_staged:usize,limit:usize,staged_owned:usize)->Result<(),TxError>{
    if existing_and_other_staged.checked_add(staged_owned).and_then(|n|n.checked_add(MAX_WORKSPACE_BYTES)).is_none_or(|n|n>limit){Err(TxError::WorkingLimit)}else{Ok(())}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { ContinuousGensetTwinPumpV1, ContinuousGensetTwinPumpTreatmentV2(Box<v2::Opening>) }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version:u32, pub profile:Profile, pub city_id:u32, pub source_id:u64,
    pub owner:u32, pub start_supplier:u32,
    /// Original Stock IDs: fuel, cooling water, raw water, output A, output B, heat, waste water.
    pub stocks:[u32;7],
    /// Original Resource IDs: genset, pump A, pump B. Each is one exclusive machine slot.
    pub resources:[u32;3],
    pub water_capacities_ml:[u64;2], pub heat_capacity_mwh:u64,
    pub wastewater_capacity_ml:u64,
    pub heat_peak_w:u64, pub wastewater_peak_ml_per_second:u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Command { Start{run_id:u64,pumps:u8}, Stop, Fault{fault_ref:u64} }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopCause { Explicit, Fault(u64), InputOrReceiverBoundary, OperatorBoundary }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id:u64, pub operation:u64, pub pumps:u8, pub started_at:u64,
    pub ended_at:Option<u64>, pub stop_cause:Option<StopCause>,
    pub production_seconds:u64,
    #[serde(default,skip_serializing_if="money_is_zero")]
    pub operator_labor_basis:Money,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    opening:Opening, config:g::Config, genset:g::State,
    raw_lot:g::InputLot,
    raw_used_ml:u64, produced_ml:[u64;2], output_basis:[Money;2],
    #[serde(default,skip_serializing_if="money_is_zero")]
    operator_labor_basis:Money,
    /// Start money remains WIP until real production, or is expensed on a zero-work stop.
    startup_wip:Money, zero_work_expense:Money, pump_start_fees:Money,
    run_converted_base:Money, run_output_base:[Money;2],
    runs:[Option<Run>;MAX_RUNS],
    tokens:[Option<u64>;crate::exchange::PATH],
    last_operation:u64,
    #[serde(default,skip_serializing_if="Option::is_none")]
    v2:Option<Box<v2::State>>,
    #[serde(skip)] ready:bool,
}

/// Small authenticated observation; never an execution grant or a replacement ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DueView {
    pub version:u32, pub scope:crate::arena::Scope,
    pub city_id:u32, pub source_id:u64, pub fingerprint:[u8;32],
    pub registration:[u8;32], pub cursor:u64, pub status:g::Status,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub version:u32, pub city_id:u32, pub cursor:u64, pub genset:g::State,
    pub machine_meters:g::Meters,
    pub produced_ml:[u64;2], pub output_basis:[Money;2], pub raw_used_ml:u64,
    #[serde(skip_serializing_if="money_is_zero")]
    pub operator_labor_basis:Money,
    pub heat_mwh:u64, pub heat_remainder_numerator:u64, pub denominator:u64,
    pub startup_wip:Money, pub zero_work_expense:Money,
    pub runs:[Option<Run>;MAX_RUNS],
    pub sanitary_quality_known:bool,
    /// V2 authority includes exact WIP, fee/strategy/cursor and lease state that
    /// does not appear as spendable Stock. It must still affect state_hash.
    #[serde(skip_serializing_if="Option::is_none")]
    pub v2_fingerprint:Option<[u8;32]>,
}

fn money_is_zero(v:&Money)->bool{*v==Money::ZERO}
fn bad(_:impl std::fmt::Debug)->TxError {TxError::InvalidOperation}
fn q(v:u64)->Result<Quantity,TxError>{Quantity::new(i64::try_from(v).map_err(|_|TxError::Overflow)?).map_err(bad)}
fn m(v:u64)->Result<Money,TxError>{Money::new(i64::try_from(v).map_err(|_|TxError::Overflow)?).map_err(bad)}
fn add(a:u64,b:u64)->Result<u64,TxError>{a.checked_add(b).ok_or(TxError::Overflow)}
fn mul(a:u64,b:u64)->Result<u64,TxError>{a.checked_mul(b).ok_or(TxError::Overflow)}
fn digest(v:&impl Serialize)->Result<[u8;32],TxError>{Ok(Sha256::digest(serde_json::to_vec(v).map_err(bad)?).into())}
fn basis(lot:&g::InputLot,n:u64)->Result<Money,TxError>{
    if n==0{return Ok(Money::ZERO);}
    crate::units::frozen_lot_basis_delta(lot.quantity,lot.basis,Quantity::ZERO,q(n)?)
        .map(|(_,v)|v).map_err(bad)
}
fn lot(s:Stock)->g::InputLot{g::InputLot{stock_id:s.id,lot_ref:u64::from(s.lot_id),quantity:s.origin_quantity,basis:s.origin_basis}}
fn instant(r:&Runtime,at:u64)->g::Instant{g::Instant{scope:r.config.scope,clock:r.config.clock,second:at}}

impl Runtime {
    pub(crate) fn owned_bytes(&self)->usize{
        size_of::<Self>()+Self::profile_backing_bytes(&self.opening)
            +self.v2.as_deref().map_or(0,v2::State::owned_bytes)
    }
    fn profile_backing_bytes(opening:&Opening)->usize{
        match opening.profile{Profile::ContinuousGensetTwinPumpV1=>0,Profile::ContinuousGensetTwinPumpTreatmentV2(_)=>size_of::<v2::Opening>()}
    }
    pub(crate) fn opening_owned_bytes(opening:&Opening)->usize{
        size_of::<Self>()+Self::profile_backing_bytes(opening)
            +if matches!(opening.profile,Profile::ContinuousGensetTwinPumpTreatmentV2(_)){size_of::<v2::State>()}else{0}
    }
    fn validate_profile(&self)->Result<(),TxError>{
        match (&self.opening.profile,self.opening.version,self.v2.as_deref()){
            (Profile::ContinuousGensetTwinPumpV1,VERSION,None)=>Ok(()),
            (Profile::ContinuousGensetTwinPumpTreatmentV2(o),v2::VERSION,Some(v))if o.version==v2::VERSION=>{
                v.validate()?;
                match (self.genset.run_count,v.batch.as_deref()){
                    (0,None)=>Ok(()),
                    (n,Some(b))if n>0=>{
                        let first=self.runs[0].ok_or(TxError::InvalidOperation)?;
                        let c=&b.config.material;let binding=c.binding;
                        if b.config.accrual_policy!=o.wage_settlement.accrual_policy()
                            ||binding.operation!=first.operation||binding.origin_cursor!=first.started_at
                            ||binding.world_ref!=self.config.scope.world()||binding.root_epoch!=self.config.scope.branch()
                            ||binding.city_id!=self.opening.city_id||binding.source_version!=u64::from(v2::VERSION)
                            ||c.raw!=o.source_quality||c.raw_origin.stock_id!=self.raw_lot.stock_id
                            ||c.raw_origin.lot_ref!=self.raw_lot.lot_ref||c.raw_origin.quantity_milli!=self.raw_lot.quantity.value()as u64
                            ||c.raw_origin.basis_cents!=self.raw_lot.basis.value()as u64
                            ||c.reagent_origin.stock_id!=o.reagent_stock
                            ||c.raw_basis!=(crate::water_treatment::RawBasisMode::RootConfirmedStream{ports:[self.opening.stocks[3],self.opening.stocks[4]]})
                            ||c.output_stock!=o.outlet_stocks[0]||c.concentrate_stock!=o.outlet_stocks[1]||c.sludge_stock!=o.outlet_stocks[2]
                            ||b.checkpoint.physical.cursor>self.cursor(){return Err(TxError::InvalidOperation);}
                        Ok(())
                    },
                    _=>Err(TxError::InvalidOperation),
                }
            },
            _=>Err(TxError::InvalidOperation),
        }
    }
    fn legacy_execution(&self)->Result<(),TxError>{
        self.validate_profile()?;
        if self.v2.is_some(){return Err(TxError::InvalidOperation);}
        Ok(())
    }
    pub(crate) fn registration(&self)->Result<[u8;32],TxError>{digest(&(&self.opening,&self.config,&self.raw_lot))}
    pub(crate) fn due_view(&self)->Result<DueView,TxError>{
        if !self.ready {return Err(TxError::InvalidOperation);}
        self.validate_profile()?;
        Ok(DueView{version:self.opening.version,scope:self.config.scope,city_id:self.opening.city_id,
            source_id:self.opening.source_id,fingerprint:digest(self)?,
            registration:self.registration()?,cursor:self.cursor(),status:self.genset.status})
    }
    pub(crate) fn governs(&self,id:u32)->bool{self.opening.stocks.contains(&id)||matches!(&self.opening.profile,Profile::ContinuousGensetTwinPumpTreatmentV2(o)if o.governs(id))}
    pub(crate) fn is_ready(&self)->bool{self.ready}
    pub(crate) fn cursor(&self)->u64{self.genset.cursor}
    pub(crate) fn changed_by(&self,operation:u64)->bool{self.last_operation==operation}
    pub(crate) fn opening(&self)->&Opening{&self.opening}
    pub(crate) fn owns_resource(&self,id:u32)->bool{self.opening.resources.contains(&id)||self.treatment_opening().is_some_and(|o|v2::roles(o).contains(&id))}
    pub(crate) fn treatment_device(&self)->Option<u64>{
        match &self.opening.profile{Profile::ContinuousGensetTwinPumpTreatmentV2(o)=>Some(u64::from(o.purifier_resource)),_=>None}
    }
    pub(crate) fn treatment_opening(&self)->Option<&v2::Opening>{match &self.opening.profile{Profile::ContinuousGensetTwinPumpTreatmentV2(o)=>Some(o),_=>None}}
    pub(crate) fn treatment_batch(&self)->Option<&crate::water_treatment_authority::Batch>{self.v2.as_deref().and_then(|s|s.batch.as_deref())}
    pub(crate) fn treatment_batch_mut(&mut self)->Option<&mut crate::water_treatment_authority::Batch>{self.v2.as_deref_mut().and_then(|s|s.batch.as_deref_mut())}
    pub(crate) fn work_covered(&self,device:u64,from:u64,until:u64)->Result<u64,TxError>{
        if until<from{return Err(TxError::InvalidOperation);}
        if device==u64::from(self.opening.resources[0]){
            self.runs.iter().flatten().try_fold(0u64,|n,r|{
                let end=add(r.started_at,r.production_seconds)?;
                add(n,until.min(end).saturating_sub(from.max(r.started_at)))
            })
        }else if self.treatment_device()==Some(device){
            let segments=self.v2.as_deref().and_then(|s|s.batch.as_deref()).map(|b|&b.segments);
            let mut n=0;if let Some(segments)=segments{for s in segments.iter().flatten(){
                let end=add(s.started_at,s.treatment_seconds)?;
                n=add(n,until.min(end).saturating_sub(from.max(s.started_at)))?;
            }}Ok(n)
        }else{Err(TxError::InvalidOperation)}
    }
    pub(crate) fn labor_basis_for_run(&self,run:u64,water:bool)->Result<Money,TxError>{
        if !water{return self.runs.iter().flatten().find(|r|r.id==run).map(|r|r.operator_labor_basis).ok_or(TxError::InvalidOperation);}
        let batch=self.v2.as_deref().and_then(|s|s.batch.as_deref()).ok_or(TxError::InvalidOperation)?;
        let mut cents=0;for s in batch.segments.iter().flatten().filter(|s|s.source_run==run){cents=add(cents,s.earned_at_cells(s.treatment_seconds,true)?)?;}m(cents)
    }
    pub(crate) fn all_labor_basis(&self)->Result<Money,TxError>{
        self.operator_labor_basis.checked_add(m(self.v2.as_deref().and_then(|s|s.batch.as_deref()).map_or(0,|b|b.water_earned_cents))?).map_err(bad)
    }
    fn run(&self)->Option<&Run>{self.runs.iter().flatten().last()}
    fn run_mut(&mut self)->Option<&mut Run>{self.runs.iter_mut().flatten().last()}
    fn mask(&self)->u8{if self.genset.status==g::Status::Running{
        if let Some(b)=self.v2.as_deref().and_then(|s|s.batch.as_deref()){b.segments.iter().flatten().last().map_or(0,|s|s.pumps)}
        else{self.run().map_or(0,|r|r.pumps)}
    }else{0}}
    fn heat(&self)->Result<(u64,u64),TxError>{
        let total=mul(self.genset.active_seconds,HEAT_NUM)?;
        Ok((total/3,total%3))
    }
    pub(crate) fn view(&self)->Result<View,TxError>{let (heat,rem)=self.heat()?;Ok(View{
        version:self.opening.version,city_id:self.opening.city_id,cursor:self.cursor(),genset:self.genset.clone(),
        machine_meters:g::ContinuousGensetV1::meters(&self.config,&self.genset).map_err(bad)?,
        produced_ml:self.produced_ml,output_basis:self.output_basis,raw_used_ml:self.raw_used_ml,operator_labor_basis:self.operator_labor_basis,
        heat_mwh:heat,heat_remainder_numerator:rem,denominator:3,
        startup_wip:self.startup_wip,zero_work_expense:self.zero_work_expense,runs:self.runs,
        sanitary_quality_known:false,v2_fingerprint:self.v2.as_ref().map(|_|digest(self)).transpose()?})}
    fn converted_basis(&self)->Result<Money,TxError>{
        let meters=g::ContinuousGensetV1::meters(&self.config,&self.genset).map_err(bad)?;
        meters.fuel_converted_basis.checked_add(meters.water_converted_basis).map_err(bad)?
            .checked_add(basis(&self.raw_lot,self.raw_used_ml)?).map_err(bad)
    }
    fn stocks(&self,l:&mut impl Ledger)->Result<[Stock;7],TxError>{
        let ids=self.opening.stocks;
        Ok([l.stock(ids[0])?,l.stock(ids[1])?,l.stock(ids[2])?,l.stock(ids[3])?,
            l.stock(ids[4])?,l.stock(ids[5])?,l.stock(ids[6])?])
    }
    fn available_seconds(&self,s:&[Stock;7],mask:u8)->Result<u64,TxError>{
        if mask==0||mask>3{return Err(TxError::InvalidOperation);}
        let active=self.genset.active_seconds;
        let fuel_total=(self.config.fuel.quantity.value() as u64).checked_mul(3).ok_or(TxError::Overflow)?/50;
        let water_total=self.config.water.quantity.value() as u64/1000;
        let mut n=3600.min(fuel_total.saturating_sub(active)).min(water_total.saturating_sub(active));
        n=n.min(s[2].quantity.value() as u64/mul(RATE_RAW,u64::from(mask.count_ones()))?);
        for i in 0..2 {if mask&(1<<i)!=0 {
            let free=self.opening.water_capacities_ml[i].checked_sub(s[3+i].quantity.value() as u64).ok_or(TxError::Capacity)?;
            n=n.min(free/RATE_RAW);
        }}
        let heat_limit=u128::from(self.opening.heat_capacity_mwh)*3;
        let used=u128::from(active)*u128::from(HEAT_NUM);
        n=n.min(u64::try_from(heat_limit.checked_sub(used).ok_or(TxError::Capacity)?/u128::from(HEAT_NUM)).map_err(|_|TxError::Overflow)?);
        let waste_free=self.opening.wastewater_capacity_ml.checked_sub(s[6].quantity.value() as u64).ok_or(TxError::Capacity)?;
        Ok(n.min(waste_free/1000))
    }
    pub(crate) fn boundary(&self,l:&mut impl Ledger,now:u64)->Result<Option<u64>,TxError>{
        if self.v2.is_some(){return v2::boundary(self,l,now);}
        if self.genset.status!=g::Status::Running{return Ok(None);}
        if self.cursor()!=now{return Err(TxError::InvalidOperation);}
        Ok(Some(add(now,self.available_seconds(&self.stocks(l)?,self.mask())?)?))
    }
    fn authority(&self,l:&mut impl Ledger,at:u64,root:[u8;32],revision:u64)->Result<g::AuthorityView,TxError>{
        let s=self.stocks(l)?;let(heat,rem)=self.heat()?;
        let capacity_scaled=u128::from(self.opening.heat_capacity_mwh)*3;
        let free=capacity_scaled.checked_sub(u128::from(heat)*3+u128::from(rem)).ok_or(TxError::Capacity)?;
        let cash=l.cash(self.opening.owner)?;
        let protected=Ledger::protection(l,self.opening.owner)?;
        Ok(g::AuthorityView{at:instant(self,at),root,read_version:revision,source_version:1,
            observed_at:self.cursor(),covers_from:self.cursor(),covers_until:at,max_age:3600,
            fuel_materialization:g::Materialization::ExistingStock,water_materialization:g::Materialization::ExistingStock,
            fuel_arrived_at:0,water_arrived_at:0,fuel_available_mq:s[0].quantity,water_available_ml:s[1].quantity,
            heat_free_mwh:u64::try_from(free/3).map_err(|_|TxError::Overflow)?,heat_free_remainder_numerator:(free%3) as u64,
            wastewater_free_ml:self.opening.wastewater_capacity_ml.checked_sub(s[6].quantity.value() as u64).ok_or(TxError::Capacity)?,
            payer_available:cash.checked_sub(protected).map_err(bad)?})
    }
    fn materialize(&mut self,l:&mut impl Ledger,candidate:g::Candidate)->Result<(),TxError>{
        let d=&candidate.delta;
        let f=adjust_input(l,&self.config.fuel,d.fuel_take_mq,d.fuel_return_mq)?;
        let w=adjust_input(l,&self.config.water,d.water_take_ml,d.water_return_ml)?;
        l.pay(self.opening.owner,self.opening.start_supplier,d.start_fee)?;
        g::ContinuousGensetV1::confirm_actual_basis(&candidate,&g::ActualBasis{
            fuel_take:f.0,fuel_return:f.1,water_take:w.0,water_return:w.1,fee_paid:d.start_fee}).map_err(bad)?;
        self.genset=candidate.next;Ok(())
    }
    pub(crate) fn command(&mut self,world:&World,facility:Option<&crate::facility_authority::Runtime>,l:&mut impl Ledger,command:Command,at:u64,operation:u64,root:[u8;32],revision:u64)->Result<(),TxError>{
        if self.v2.is_some(){return v2::command(self,world,facility,l,command,at,operation,root,revision);}
        self.legacy_execution()?;
        if !self.ready || operation==0 || at!=self.cursor(){return Err(TxError::InvalidOperation);}
        self.validate_stocks(&self.stocks(l)?)?;
        let view=self.authority(l,at,root,revision)?;
        match command {
            Command::Start{run_id,pumps}=>{
                if self.runs[MAX_RUNS-1].is_some(){return Err(TxError::WorkingLimit);}
                if self.available_seconds(&self.stocks(l)?,pumps)?==0{return Err(TxError::Capacity);}
                let c=g::ContinuousGensetV1::propose(&self.config,&self.genset,&view,g::Command::Start{at:instant(self,at),run_id}).map_err(bad)?;
                g::ContinuousGensetV1::revalidate(&self.config,&self.genset,&view,&c).map_err(bad)?;
                let mut requests=[(self.opening.resources[0],1);3];let mut count=1;
                for i in 0..2 {if pumps&(1<<i)!=0{requests[count]=(self.opening.resources[i+1],1);count+=1;}}
                self.tokens=Ledger::claim_path(l,&requests[..count])?;
                let pump_fee=m(mul(PUMP_START_CENTS,u64::from(pumps.count_ones()))?)?;
                l.pay(self.opening.owner,self.opening.start_supplier,pump_fee)?;
                self.pump_start_fees=self.pump_start_fees.checked_add(pump_fee).map_err(bad)?;
                self.startup_wip=c.delta.start_fee.checked_add(pump_fee).map_err(bad)?;
                self.run_converted_base=self.converted_basis()?;self.run_output_base=self.output_basis;
                self.materialize(l,c)?;
                *self.runs.iter_mut().find(|r|r.is_none()).ok_or(TxError::WorkingLimit)?=Some(Run{
                    id:run_id,operation,pumps,started_at:at,ended_at:None,stop_cause:None,production_seconds:0,operator_labor_basis:Money::ZERO});
            },
            Command::Stop=>self.stop(l,&view,StopCause::Explicit)?,
            Command::Fault{fault_ref}=>{
                if fault_ref==0{return Err(TxError::InvalidOperation);}
                self.stop(l,&view,StopCause::Fault(fault_ref))?;
            }
        }
        self.last_operation=operation;self.validate_stocks(&self.stocks(l)?)
    }
    fn stop(&mut self,l:&mut impl Ledger,view:&g::AuthorityView,cause:StopCause)->Result<(),TxError>{
        let command=match cause{StopCause::Fault(fault_ref)=>g::Command::Fault{at:view.at,fault_ref},_=>g::Command::Stop{at:view.at}};
        let c=g::ContinuousGensetV1::propose(&self.config,&self.genset,view,command).map_err(bad)?;
        g::ContinuousGensetV1::revalidate(&self.config,&self.genset,view,&c).map_err(bad)?;
        self.materialize(l,c)?;
        Ledger::release_path(l,self.tokens)?;self.tokens=[None;crate::exchange::PATH];
        self.zero_work_expense=self.zero_work_expense.checked_add(self.startup_wip).map_err(bad)?;
        self.startup_wip=Money::ZERO;
        let run=self.run_mut().ok_or(TxError::InvalidOperation)?;run.ended_at=Some(view.at.second);run.stop_cause=Some(cause);
        Ok(())
    }
    pub(crate) fn stop_for_operator(&mut self,l:&mut impl Ledger,at:u64,operation:u64,root:[u8;32],revision:u64)->Result<(),TxError>{
        if !self.ready||at!=self.cursor()||operation==0{return Err(TxError::InvalidOperation);}
        if self.v2.is_some(){
            if self.genset.status==g::Status::Running{return v2::stop(self,l,at,operation,root,revision,StopCause::OperatorBoundary);}
            return Ok(());
        }
        if self.genset.status==g::Status::Running{let view=self.authority(l,at,root,revision)?;self.stop(l,&view,StopCause::OperatorBoundary)?;self.last_operation=operation;}
        self.validate_stocks(&self.stocks(l)?)
    }
    /// P5 joint original-ledger prefix. It does not advance or own a second clock.
    pub(crate) fn advance(&mut self,l:&mut impl Ledger,at:u64,operation:u64,root:[u8;32],revision:u64)->Result<(),TxError>{
        self.legacy_execution()?;
        if !self.ready || at<self.cursor() || at-self.cursor()>3600 || operation==0{return Err(TxError::InvalidOperation);}
        self.validate_stocks(&self.stocks(l)?)?;
        if self.genset.status!=g::Status::Running{
            if at!=self.cursor()&&self.genset.run_count!=0{self.last_operation=operation;}
            self.genset.cursor=at;return Ok(());
        }
        let dt=at-self.cursor();let mask=self.mask();let s=self.stocks(l)?;
        let possible=self.available_seconds(&s,mask)?;
        if dt>possible{return Err(TxError::Capacity);}
        if dt>0 {
            let count=u64::from(mask.count_ones());
            let received_scaled=add(mul(self.genset.received_mwh,3)?,self.genset.received_remainder_numerator)?;
            let received_scaled=add(received_scaled,mul(mul(dt,5_000)?,count)?)?;
            let view=self.authority(l,at,root,revision)?;
            let reception=g::Reception{scope:self.config.scope,clock:self.config.clock,genset_id:self.config.genset_id,
                source_version:1,run_id:self.genset.run_id,consumer_ref:self.config.consumer_ref,consumer_version:1,
                receipt_ref:operation,root,from:self.cursor(),until:at,
                cumulative_received_mwh:received_scaled/3,received_remainder_numerator:received_scaled%3,actual_peak_w:6_000*count};
            let c=g::ContinuousGensetV1::propose(&self.config,&self.genset,&view,g::Command::Advance{at:instant(self,at),reception}).map_err(bad)?;
            g::ContinuousGensetV1::revalidate(&self.config,&self.genset,&view,&c).map_err(bad)?;
            let source_candidate=c.clone();
            self.materialize(l,c)?;
            let water_each=mul(dt,RATE_RAW)?;
            let raw=mul(water_each,count)?;
            adjust_input(l,&self.raw_lot,raw,0)?;
            self.raw_used_ml=add(self.raw_used_ml,raw)?;
            for i in 0..2{if mask&(1<<i)!=0{self.produced_ml[i]=add(self.produced_ml[i],water_each)?;}}
            // Keep the whole start fee in the run allocation after the first productive prefix.
            let prior_assigned=self.output_basis[0].checked_sub(self.run_output_base[0]).map_err(bad)?
                .checked_add(self.output_basis[1].checked_sub(self.run_output_base[1]).map_err(bad)?).map_err(bad)?;
            let old_converted=self.converted_basis()?.checked_sub(self.run_converted_base).map_err(bad)?;
            let fee=if self.startup_wip!=Money::ZERO{self.startup_wip}else{
                let prev_seconds=self.run().ok_or(TxError::InvalidOperation)?.production_seconds;
                if prev_seconds==0{Money::ZERO}else{m(g::START_FEE_CENTS+PUMP_START_CENTS*count)?}
            };
            let total=old_converted.checked_add(fee).map_err(bad)?.checked_add(self.run().ok_or(TxError::InvalidOperation)?.operator_labor_basis).map_err(bad)?;
            if total<prior_assigned{return Err(TxError::InvalidOperation);}
            let assigned=match mask{1=>[total,Money::ZERO],2=>[Money::ZERO,total],3=>{
                let a=m(total.value() as u64/2)?;[a,total.checked_sub(a).map_err(bad)?]},_=>return Err(TxError::InvalidOperation)};
            for i in 0..2{self.output_basis[i]=self.run_output_base[i].checked_add(assigned[i]).map_err(bad)?;}
            self.startup_wip=Money::ZERO;
            let run=self.run_mut().ok_or(TxError::InvalidOperation)?;run.production_seconds=add(run.production_seconds,dt)?;
            let(heat,_)=self.heat()?;let meters=g::ContinuousGensetV1::meters(&self.config,&self.genset).map_err(bad)?;
            for(i,quantity,cost)in[(3,self.produced_ml[0],self.output_basis[0]),(4,self.produced_ml[1],self.output_basis[1]),(5,heat,Money::ZERO),(6,meters.wastewater_ml,Money::ZERO)]{
                set_output(l,self.opening.stocks[i],quantity,cost)?;
            }
            check_actual_uniform_prefix(self,&source_candidate,mask,revision,operation)?;
            self.last_operation=operation;
        }
        let remaining=self.available_seconds(&self.stocks(l)?,mask)?;
        if remaining==0{let view=self.authority(l,at,root,revision)?;self.stop(l,&view,StopCause::InputOrReceiverBoundary)?;self.last_operation=operation;}
        self.validate_stocks(&self.stocks(l)?)
    }
    /// Root supplies ONLY a same-Prepared actual operator wage delta after original
    /// member settlement and protected cash debit/credit. No public cost ingress.
    pub(crate) fn book_operator_wage(&mut self,l:&mut impl Ledger,amount:Money,operation:u64)->Result<(),TxError>{
        if amount==Money::ZERO{return Ok(());}
        if self.last_operation!=operation||self.run().is_none_or(|r|r.production_seconds==0){return Err(TxError::InvalidOperation);}
        self.validate_stocks(&self.stocks(l)?)?;
        self.operator_labor_basis=self.operator_labor_basis.checked_add(amount).map_err(bad)?;
        let r=self.run_mut().ok_or(TxError::InvalidOperation)?;
        r.operator_labor_basis=r.operator_labor_basis.checked_add(amount).map_err(bad)?;
        let mask=r.pumps;let count=u64::from(mask.count_ones());let labor=r.operator_labor_basis;
        let total=self.converted_basis()?.checked_sub(self.run_converted_base).map_err(bad)?
            .checked_add(m(g::START_FEE_CENTS+PUMP_START_CENTS*count)?).map_err(bad)?.checked_add(labor).map_err(bad)?;
        let assigned=match mask{1=>[total,Money::ZERO],2=>[Money::ZERO,total],3=>{let a=m(total.value() as u64/2)?;[a,total.checked_sub(a).map_err(bad)?]},_=>return Err(TxError::InvalidOperation)};
        for i in 0..2{self.output_basis[i]=self.run_output_base[i].checked_add(assigned[i]).map_err(bad)?;set_output(l,self.opening.stocks[i+3],self.produced_ml[i],self.output_basis[i])?;}
        self.validate_stocks(&self.stocks(l)?)
    }
    fn validate_stocks(&self,s:&[Stock;7])->Result<(),TxError>{
        self.validate_profile()?;
        if self.v2.is_some(){return v2::validate_stocks(self,s);}
        let meters=g::ContinuousGensetV1::meters(&self.config,&self.genset).map_err(bad)?;
        for (i,lot,net,held)in[(0,&self.config.fuel,self.genset.fuel_taken_mq.checked_sub(self.genset.fuel_returned_mq).ok_or(TxError::Stock)?,meters.fuel_held_basis),
            (1,&self.config.water,self.genset.water_taken_ml.checked_sub(self.genset.water_returned_ml).ok_or(TxError::Stock)?,meters.water_held_basis),
            (2,&self.raw_lot,self.raw_used_ml,basis(&self.raw_lot,self.raw_used_ml)?)]{
            if s[i].id!=lot.stock_id || u64::from(s[i].lot_id)!=lot.lot_ref || s[i].origin_quantity!=lot.quantity
                || s[i].origin_basis!=lot.basis || s[i].offset!=q(net)?
                || s[i].quantity!=lot.quantity.checked_sub(q(net)?).map_err(bad)?
                || s[i].basis!=lot.basis.checked_sub(held).map_err(bad)?{return Err(TxError::Stock);}
        }
        let(heat,rem)=self.heat()?;
        if u128::from(heat)*3+u128::from(rem)>u128::from(self.opening.heat_capacity_mwh)*3{return Err(TxError::Capacity);}
        for(i,n,b)in[(3,self.produced_ml[0],self.output_basis[0]),(4,self.produced_ml[1],self.output_basis[1]),(5,heat,Money::ZERO),(6,meters.wastewater_ml,Money::ZERO)]{
            if s[i].quantity!=q(n)?||s[i].basis!=b||s[i].origin_quantity!=q(n)?||s[i].origin_basis!=b||s[i].offset!=Quantity::ZERO{return Err(TxError::Stock);}
        }
        if self.raw_used_ml!=add(self.produced_ml[0],self.produced_ml[1])?
            || self.produced_ml.iter().zip(self.opening.water_capacities_ml).any(|(n,cap)|*n>cap)
            || meters.wastewater_ml>self.opening.wastewater_capacity_ml{return Err(TxError::Stock);}
        let cost=self.converted_basis()?.checked_add(meters.start_fees).map_err(bad)?
            .checked_add(self.pump_start_fees).map_err(bad)?.checked_add(self.operator_labor_basis).map_err(bad)?;
        let placed=self.output_basis[0].checked_add(self.output_basis[1]).map_err(bad)?
            .checked_add(self.startup_wip).map_err(bad)?.checked_add(self.zero_work_expense).map_err(bad)?;
        if cost!=placed{return Err(TxError::Stock);}
        let runs=self.runs.iter().flatten().collect::<Vec<_>>();
        if self.runs.iter().take(runs.len()).any(Option::is_none){return Err(TxError::InvalidOperation);}
        if runs.len() as u64!=self.genset.run_count || runs.iter().enumerate().any(|(i,r)|
            r.id==0||r.operation==0||r.pumps==0||r.pumps>3||r.started_at>self.cursor()
            || r.ended_at.is_some()!=r.stop_cause.is_some()
            || r.ended_at.is_some_and(|end|end<r.started_at||end>self.cursor()||r.production_seconds!=end-r.started_at)
            || (i+1<runs.len() && (r.ended_at.is_none()||r.id>=runs[i+1].id||r.ended_at>Some(runs[i+1].started_at)))){return Err(TxError::InvalidOperation);}
        if self.genset.status==g::Status::Running {
            let r=self.run().ok_or(TxError::InvalidOperation)?;
            if r.ended_at.is_some()||r.production_seconds!=self.cursor()-r.started_at{return Err(TxError::InvalidOperation);}
        } else if self.run().is_some_and(|r|r.ended_at.is_none())||self.tokens.iter().any(Option::is_some){return Err(TxError::Lease);}
        let mut active=0;let mut raw=0;let mut outputs=[0;2];let mut output_cost=[Money::ZERO;2];
        let mut expected_received=0;let mut pump_fees=0;let mut expense=Money::ZERO;let mut startup=Money::ZERO;let mut labor=Money::ZERO;
        for run in &runs {
            labor=labor.checked_add(run.operator_labor_basis).map_err(bad)?;
            let before=converted_at(&self.config,&self.raw_lot,active,raw)?;
            if Some(run.id)==self.run().map(|r|r.id)
                && (self.run_converted_base!=before||self.run_output_base!=output_cost){return Err(TxError::Stock);}
            active=add(active,run.production_seconds)?;
            let count=u64::from(run.pumps.count_ones());
            let produced=mul(run.production_seconds,RATE_RAW)?;
            for i in 0..2{if run.pumps&(1<<i)!=0{outputs[i]=add(outputs[i],produced)?;}}
            raw=add(raw,mul(produced,count)?)?;
            expected_received=add(expected_received,mul(mul(run.production_seconds,5_000)?,count)?)?;
            pump_fees=add(pump_fees,mul(PUMP_START_CENTS,count)?)?;
            let fee=m(add(g::START_FEE_CENTS,mul(PUMP_START_CENTS,count)?)?)?;
            if run.production_seconds==0 {
                if run.operator_labor_basis!=Money::ZERO{return Err(TxError::Stock);}
                if run.ended_at.is_some(){expense=expense.checked_add(fee).map_err(bad)?;}else{startup=fee;}
            }else{
                let total=converted_at(&self.config,&self.raw_lot,active,raw)?.checked_sub(before).map_err(bad)?.checked_add(fee).map_err(bad)?.checked_add(run.operator_labor_basis).map_err(bad)?;
                let allocated=match run.pumps{1=>[total,Money::ZERO],2=>[Money::ZERO,total],3=>{
                    let a=m(total.value() as u64/2)?;[a,total.checked_sub(a).map_err(bad)?]},_=>return Err(TxError::InvalidOperation)};
                for i in 0..2{output_cost[i]=output_cost[i].checked_add(allocated[i]).map_err(bad)?;}
            }
        }
        if active!=self.genset.active_seconds||raw!=self.raw_used_ml||outputs!=self.produced_ml
            ||labor!=self.operator_labor_basis||output_cost!=self.output_basis||m(pump_fees)?!=self.pump_start_fees
            ||expense!=self.zero_work_expense||startup!=self.startup_wip
            ||expected_received!=add(mul(self.genset.received_mwh,3)?,self.genset.received_remainder_numerator)?
            ||self.run().map_or(0,|r|r.id)!=self.genset.run_id{return Err(TxError::InvalidOperation);}
        Ok(())
    }
    pub(crate) fn validate_prepared(&self,l:&mut impl Ledger,t:Option<&crate::timeline::Timeline>)->Result<(),TxError>{
        self.validate_stocks(&self.stocks(l)?)?;
        if self.v2.is_some(){v2::validate_prepared(self,l)?;}
        if self.genset.status==g::Status::Running {
            let mut requests=[(self.opening.resources[0],1);3];let mut n=1;
            for i in 0..2{if self.mask()&(1<<i)!=0{requests[n]=(self.opening.resources[i+1],1);n+=1;}}
            if !Ledger::owns_path(l,&requests[..n],self.tokens){return Err(TxError::Lease);}
        }
        if t.is_none_or(|t|self.cursor()==t.progress.last_complete
            ||(t.progress.phase==Some(crate::schedule::Phase::P5)&&self.cursor()==t.progress.at)){Ok(())}else{Err(TxError::InvalidOperation)}
    }
}

/// V3 is a read-only interval verifier here, not an extra prepaid energy ledger.
/// The original owner persists lifetime exact machine/receiver meters above. This
/// fixed zero-loss prototype never spends V3's ceil meter or treats it as a battery.
fn check_actual_uniform_prefix(r:&Runtime,generated:&g::Candidate,mask:u8,revision:u64,operation:u64)->Result<(),TxError>{
    use crate::utility_rational_v3 as v;
    let g::Command::Advance{reception,..}=&generated.command else{return Err(TxError::InvalidOperation);};
    let rate=|n,d|v::Rate::new(n,d).map_err(bad);
    let source_rate=rate(20_000,3)?;let pump_rate=rate(5_000,3)?;
    let resource=v::Resource::MilliWattHour;
    // Quality 1 here is the explicit fixed electric bus class, never a water certificate.
    let binding=v::AuthorityBinding{scope:v::Scope{development_city:r.opening.city_id},world_ref:r.config.scope.world(),branch_ref:r.config.scope.branch(),
        root_ref:revision,provider_ref:r.opening.source_id,provider_revision:1,source_ref:r.opening.source_id,source_revision:1,
        stock_ref:u64::from(r.opening.stocks[0]),contract_ref:r.opening.source_id,clock_revision:1,scale_revision:1,
        effective_start:reception.from,effective_end:reception.until};
    let generated_scaled=add(mul(generated.delta.electricity_generated_mwh,3)?,generated.delta.electricity_generated_remainder_numerator)?;
    let available=q(generated_scaled/3+u64::from(generated_scaled%3!=0))?;
    let source=v::Source{id:1,kind:v::SourceKind::ContinuousElectricity,node:1,resource,quality:1,source_ref:r.opening.source_id,
        stock_ref:binding.stock_ref,contract_ref:binding.contract_ref,physical_supply_rate:source_rate,input_capacity_rate:source_rate,hard_peak_rate:source_rate,
        available_input_milli:available,auxiliary_ratio:v::Ratio::new(0,1,true).map_err(bad)?,auxiliary_sink_ref:0,open:true};
    let mut edges=Vec::new();let mut consumers=Vec::new();
    for i in 0..2{if mask&(1<<i)!=0{
        let id=i as u32+1;let physical=r.opening.resources[i+1];
        edges.push(v::Edge{id,from:1,to:id+1,resource,quality:1,physical_ref:u64::from(physical),stock_alias:None,
            capacity_rate:pump_rate,hard_peak_rate:pump_rate,retention:v::Ratio::new(1,1,false).map_err(bad)?,
            loss_sink_ref:u64::from(r.opening.stocks[5]),open:true});
        consumers.push(v::Consumer{id,node:id+1,resource,quality:1,source:1,path:vec![id],connection_right_ref:u64::from(physical),
            payer:u64::from(r.opening.owner),meter:u64::from(r.opening.stocks[i+3]),contract_ref:r.opening.source_id,
            priority:id as u8,required_rate:pump_rate,hard_peak_rate:pump_rate});
    }}
    let contract=v::Contract{policy_version:3,binding,window:v::Window{start_second:reception.from,end_second:reception.until},resource,quality:1,source,edges,consumers};
    struct PreparedWitness<'a>{contract:&'a v::Contract,source:&'a g::Candidate,operation:u64,digest:[u8;32]}
    impl v::RootVerifier for PreparedWitness<'_>{
        fn verifies(&self,c:&v::Contract)->bool{c==self.contract}
        fn verifies_opening(&self,c:&v::Contract,operation:u64)->bool{c==self.contract&&operation==self.operation}
        fn verify_supply(&self,c:&v::Contract,from:u64,until:u64,gross:v::ExactAmount)->Option<v::SupplyWitness>{
            let d=&self.source.delta;
            if c!=self.contract||from!=c.window.start_second||until!=c.window.end_second||gross.den!=3
                ||gross.whole!=d.received_mwh||gross.rem!=d.received_remainder_numerator{return None;}
            Some(v::SupplyWitness{source_window_ref:self.operation,source_ref:c.source.source_ref,from_second:from,until_second:until,physical_gross:gross,receipt_digest:self.digest})
        }
    }
    if contract.precision_denominator().map_err(bad)?!=3{return Err(TxError::InvalidOperation);}
    let witness=PreparedWitness{contract:&contract,source:generated,operation,digest:digest(generated)?};
    let state=contract.opening(&witness,operation).map_err(bad)?;
    let candidate=contract.propose(&witness,&state,reception.until,v::MAX_WORK).map_err(bad)?.candidate.ok_or(TxError::InvalidOperation)?;
    contract.revalidate(&witness,&state,&candidate).map_err(bad)?;
    for d in candidate.consumer_deltas{
        if d.physical.den!=3||add(mul(d.physical.whole,3)?,d.physical.rem)?!=mul(reception.until-reception.from,5_000)?{return Err(TxError::InvalidOperation);}
    }
    Ok(())
}

fn converted_at(config:&g::Config,raw_lot:&g::InputLot,seconds:u64,raw:u64)->Result<Money,TxError>{
    let fuel=cost_curve::lot_prefix(&config.fuel,u128::from(mul(seconds,50)?),3)?;
    m(fuel)?.checked_add(m(cost_curve::lot_prefix(&config.water,u128::from(mul(seconds,1000)?),1)?)?).map_err(bad)?
        .checked_add(m(cost_curve::lot_prefix(raw_lot,u128::from(raw),1)?)?).map_err(bad)
}

pub(crate) fn source_root(world:&World,r:&Runtime)->Result<[u8;32],TxError>{
    digest(&(world.scope,world.seed,world.revision,r.registration()?))
}

/// Original receipts are supplied by Store's authenticated archive, never by a model DTO.
pub struct RestoreCheck { fingerprint:[u8;32], checked:[bool;MAX_RUNS+1] }
pub(crate) fn restore_operations(world:&World)->[Option<u64>;MAX_RUNS+1]{
    let mut ops=[None;MAX_RUNS+1];
    if let Some(r)=world.continuous_supply.as_deref(){
        for(i,run)in r.runs.iter().enumerate(){ops[i]=run.map(|r|r.operation);}
        ops[MAX_RUNS]=(r.last_operation!=0).then_some(r.last_operation);
    }
    ops
}
pub(crate) fn begin_restore(world:&World)->Result<RestoreCheck,TxError>{
    let r=world.continuous_supply.as_deref().ok_or(TxError::InvalidOperation)?;
    Ok(RestoreCheck{fingerprint:digest(r)?,checked:[false;MAX_RUNS+1]})
}
pub(crate) fn check_restore(world:&World,c:&mut RestoreCheck,index:usize,record:&crate::transaction::JournalRecord)->Result<(),TxError>{
    use crate::transaction::Action;
    let r=world.continuous_supply.as_deref().ok_or(TxError::InvalidOperation)?;
    let ops=restore_operations(world);
    if index>MAX_RUNS||c.checked[index]||digest(r)?!=c.fingerprint||ops[index]!=Some(record.operation_id)
        ||record.actions.iter().skip(1).any(Option::is_some){return Err(TxError::InvalidOperation);}
    let receipt=record.outcome.map_err(|_|TxError::InvalidOperation)?;
    let at=receipt.timeline.map(|p|p.at);
    if index<MAX_RUNS {
        let run=r.runs[index].ok_or(TxError::InvalidOperation)?;
        if record.actor!=r.opening.owner||at!=Some(run.started_at)
            ||!matches!(record.actions[0],Some(Action::ContinuousSupply{command:Command::Start{run_id,pumps}})if run_id==run.id&&pumps==run.pumps){return Err(TxError::InvalidOperation);}
    }else{
        match record.actions[0]{
            Some(Action::ContinuousSupply{..}) if record.actor==r.opening.owner&&at==Some(r.cursor())=>{},
            Some(Action::Timeline{command:crate::timeline::Command::Advance{..}})if record.actor==0&&at==Some(r.cursor())=>{},
            Some(Action::MoveStock{from,to,quantity})if (record.actor==0||record.actor==r.opening.owner)&&at==Some(r.cursor())&&quantity!=Quantity::ZERO
                &&world.stocks.resolve(from).ok().and_then(|h|world.stocks.get(h).ok()).zip(world.stocks.resolve(to).ok().and_then(|h|world.stocks.get(h).ok()))
                    .is_some_and(|(a,b)|v2::product_move(r,a.id,b.id))=>{},
            Some(Action::FacilityWork{command:crate::facility_authority::Command::Stop{contract_id}})if at==Some(r.cursor())&&world.facility_work.as_deref().is_some_and(|f|f.state.opening().contracts.iter().flatten().any(|c|c.id==contract_id&&(c.device_id==u64::from(r.opening.resources[0])||r.treatment_device()==Some(c.device_id))&&c.employer_account==r.opening.owner&&(record.actor==0||record.actor==c.employer_account||record.actor==c.employee_account)))=>{},
            Some(Action::Venue{use_id,command:crate::venue_runtime::Command::SafeExit{..}})if record.actor==0&&at==Some(r.cursor())&&world.facility_work.as_deref().is_some_and(|f|f.state.opening().contracts.iter().flatten().any(|c|c.venue_use_id==use_id&&(c.device_id==u64::from(r.opening.resources[0])||r.treatment_device()==Some(c.device_id))&&c.employer_account==r.opening.owner))=>{},
            _=>return Err(TxError::InvalidOperation),
        }
    }
    c.checked[index]=true;Ok(())
}
pub(crate) fn finish_restore(world:&mut World,c:RestoreCheck)->Result<(),TxError>{
    let ops=restore_operations(world);
    let r=world.continuous_supply_mut().ok_or(TxError::InvalidOperation)?;
    if digest(r)?!=c.fingerprint||ops.iter().enumerate().any(|(i,op)|op.is_some()&&!c.checked[i]){return Err(TxError::InvalidOperation);}
    r.ready=true;Ok(())
}
pub(crate) fn authenticate_empty(world:&mut World)->Result<(),TxError>{
    if let Some(r)=world.continuous_supply.as_deref(){
        if r.genset.run_count!=0{return Ok(());}
        if r.last_operation!=0||r.genset.active_seconds!=0{return Err(TxError::InvalidOperation);}
        let c=begin_restore(world)?;finish_restore(world,c)?;
    }Ok(())
}

fn adjust_input(l:&mut impl Ledger,lot:&g::InputLot,take:u64,give:u64)->Result<(Money,Money),TxError>{
    if take!=0&&give!=0{return Err(TxError::Stock);}
    let mut s=l.stock(lot.stock_id)?;let old=s.offset.value() as u64;
    let next=add(old,take)?.checked_sub(give).ok_or(TxError::Stock)?;
    let before=basis(lot,old)?;let after=basis(lot,next)?;
    let debited=if after>=before{after.checked_sub(before).map_err(bad)?}else{Money::ZERO};
    let refunded=if before>=after{before.checked_sub(after).map_err(bad)?}else{Money::ZERO};
    s.offset=q(next)?;s.quantity=lot.quantity.checked_sub(s.offset).map_err(bad)?;
    s.basis=lot.basis.checked_sub(after).map_err(bad)?;l.put(lot.stock_id,s)?;Ok((debited,refunded))
}
fn set_output(l:&mut impl Ledger,id:u32,amount:u64,cost:Money)->Result<(),TxError>{
    let mut s=l.stock(id)?;s.quantity=q(amount)?;s.basis=cost;s.origin_quantity=s.quantity;s.origin_basis=cost;s.offset=Quantity::ZERO;l.put(id,s)
}

pub(crate) fn initialize(opening:Opening,world:&World)->Result<Box<Runtime>,String>{
    let fail=||"continuous_supply_opening".to_owned();
    let v2=match (&opening.profile,opening.version){
        (Profile::ContinuousGensetTwinPumpV1,VERSION)=>None,
        (Profile::ContinuousGensetTwinPumpTreatmentV2(o),v2::VERSION)=>{
            o.validate_registration(&opening,world,true).map_err(|_|fail())?;
            Some(Box::new(v2::State::pristine()))
        },
        _=>return Err(fail()),
    };
    if Runtime::opening_owned_bytes(&opening)>MAX_OWNED_BYTES{return Err(fail());}
    if opening.city_id==0||opening.source_id==0||opening.owner==opening.start_supplier
        ||opening.stocks.contains(&0)||opening.resources.contains(&0)
        || opening.stocks.iter().enumerate().any(|(i,id)|opening.stocks[..i].contains(id))
        || opening.resources.iter().enumerate().any(|(i,id)|opening.resources[..i].contains(id))
        || opening.heat_peak_w<60_000||opening.wastewater_peak_ml_per_second<1000
        || opening.water_capacities_ml.iter().any(|n|*n>i64::MAX as u64)
        || opening.heat_capacity_mwh>i64::MAX as u64||opening.wastewater_capacity_ml>i64::MAX as u64{return Err(fail());}
    // This first profile excludes other production owners rather than silently sharing their lots.
    if world.region.is_some()||world.boot.is_some()||world.economy.is_some()||world.exchange.is_some()
        ||world.construction.is_some()||world.town.is_some(){return Err("continuous_supply_conflicting_production_profile".into());}
    let owner=world.account_handle(opening.owner).map_err(|_|fail())?;
    world.account_handle(opening.start_supplier).map_err(|_|fail())?;
    let stock=|id|world.stock_handle(id).and_then(|h|world.stocks.get(h).map_err(|e|format!("{e:?}"))).copied();
    let s=[stock(opening.stocks[0])?,stock(opening.stocks[1])?,stock(opening.stocks[2])?,stock(opening.stocks[3])?,stock(opening.stocks[4])?,stock(opening.stocks[5])?,stock(opening.stocks[6])?];
    let kinds=[Commodity::Fuel,Commodity::Water,Commodity::RawWater,Commodity::Water,Commodity::Water,Commodity::Heat,Commodity::WasteWater];
    if s.iter().zip(kinds).any(|(s,c)|s.owner!=owner||s.commodity!=c||s.lot_id==0||s.offset!=Quantity::ZERO||s.quantity!=s.origin_quantity||s.basis!=s.origin_basis)
        ||s[..3].iter().any(|s|s.quantity==Quantity::ZERO)
        ||s[3..].iter().any(|s|s.quantity!=Quantity::ZERO||s.basis!=Money::ZERO){return Err(fail());}
    let config=g::Config{version:1,profile:g::Profile::ContinuousGensetV1,scope:world.scope,city_id:opening.city_id,
        clock:g::ClockBinding::Development{city_id:opening.city_id},genset_id:opening.source_id,source_version:1,reception_denominator:3,
        fuel:lot(s[0]),water:lot(s[1]),heat_sink_id:opening.stocks[5],wastewater_sink_id:opening.stocks[6],
        heat_capacity_mwh:opening.heat_capacity_mwh,heat_capacity_remainder_numerator:0,wastewater_capacity_ml:opening.wastewater_capacity_ml,
        heat_peak_w:opening.heat_peak_w,wastewater_peak_ml_per_second:opening.wastewater_peak_ml_per_second,
        consumer_ref:Some(opening.source_id),consumer_version:1};
    let genset=g::ContinuousGensetV1::opening(&config,g::Instant{scope:world.scope,clock:config.clock,second:0}).map_err(|_|fail())?;
    let r=Box::new(Runtime{opening,config,genset,raw_lot:lot(s[2]),raw_used_ml:0,produced_ml:[0;2],output_basis:[Money::ZERO;2],operator_labor_basis:Money::ZERO,
        startup_wip:Money::ZERO,zero_work_expense:Money::ZERO,pump_start_fees:Money::ZERO,
        run_converted_base:Money::ZERO,run_output_base:[Money::ZERO;2],runs:[None;MAX_RUNS],tokens:[None;crate::exchange::PATH],last_operation:0,v2,ready:true});
    r.validate_stocks(&s).map_err(|_|fail())?;Ok(r)
}

pub(crate) fn validate(world:&World,r:Option<&Runtime>,timeline:Option<&crate::timeline::Timeline>,resources:&[Resource],leases:&[Option<Lease>])->Result<(),TxError>{
    let Some(r)=r else{return Ok(());};
    if r.config.scope!=world.scope||r.config.city_id!=r.opening.city_id||r.config.genset_id!=r.opening.source_id
        ||r.config.reception_denominator!=3||r.owned_bytes()>MAX_OWNED_BYTES{return Err(TxError::InvalidOperation);}
    if let Profile::ContinuousGensetTwinPumpTreatmentV2(o)=&r.opening.profile{
        o.validate_registration(&r.opening,world,r.genset.run_count==0)?;
        o.validate_resources(resources)?;
        v2::validate_leases(r,leases)?;
        crate::water_treatment_authority::validate_material_stocks(r,|id|world.stock_handle(id).and_then(|h|world.stocks.get(h).copied().map_err(|e|format!("{e:?}"))).map_err(bad))?;
    }
    let stocks=r.opening.stocks.map(|id|world.stock_handle(id).ok().and_then(|h|world.stocks.get(h).ok()).copied());
    if stocks.iter().any(Option::is_none){return Err(TxError::Stock);}
    let s=stocks.map(Option::unwrap);r.validate_stocks(&s)?;
    if r.opening.resources.iter().any(|id|!resources.iter().any(|resource|resource.id==*id&&resource.capacity==1)){return Err(TxError::Resource);}
    if r.genset.status==g::Status::Running{
        let mut required=[r.opening.resources[0];3];let mut n=1;
        for i in 0..2{if r.mask()&(1<<i)!=0{required[n]=r.opening.resources[i+1];n+=1;}}
        required[..n].sort();
        if r.tokens[n..].iter().any(Option::is_some)||required[..n].iter().zip(&r.tokens[..n]).any(|(id,token)|
            !leases.iter().flatten().any(|l|Some(l.token)==*token&&l.resource==*id&&l.actor==0&&l.units==1&&l.occupied)){return Err(TxError::Lease);}
    }
    if let Some(t)=timeline {
        if r.cursor()!=t.progress.last_complete && !(t.progress.phase==Some(crate::schedule::Phase::P5)&&r.cursor()==t.progress.at){return Err(TxError::InvalidOperation);}
    }else if r.cursor()!=0{return Err(TxError::InvalidOperation);}
    Ok(())
}

impl crate::restore_into::Restore for Option<Box<Runtime>> {
    fn restore<'de,D:Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error>{*self=Option::<Box<Runtime>>::deserialize(d)?;Ok(())}
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn shared_peak_counts_other_original_candidates_before_allocating_supply(){
        let limit=crate::transaction::ENGINE_BYTE_BUDGET;
        assert!(size_of::<Runtime>()<=MAX_OWNED_BYTES);
        let base=limit-size_of::<Runtime>()-MAX_WORKSPACE_BYTES;
        assert_eq!(check_joint_peak(base,limit,size_of::<Runtime>()),Ok(()));
        assert_eq!(check_joint_peak(base+1,limit,size_of::<Runtime>()),Err(TxError::WorkingLimit));
        assert_eq!(check_joint_peak(usize::MAX,limit,size_of::<Runtime>()),Err(TxError::WorkingLimit));
        let fixed=2*size_of::<g::Candidate>()+7*size_of::<Stock>()+size_of::<Runtime>();
        assert!(fixed<MAX_WORKSPACE_BYTES);
    }
}
