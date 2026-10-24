//! Fixed ContinuousGensetV1 physical proposals. No Stock/World/Timeline writer.
//! V1: 1Q fuel + 60L water / 60 Development seconds -> 400Wh electric + 600Wh heat.
//! Source ticks are mQ/mL, energy ticks mWh. Fractional fuel/energy use thirds.
//! Unreceived electricity becomes Heat; auxiliary is explicitly zero only for this profile.
use crate::{arena::Scope, units::{Money, Quantity}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const VERSION: u32 = 1;
pub const MAX_ADVANCE_SECONDS: u64 = 3600;
pub const MAX_RESTORE_BYTES: usize = 8192;
pub const MAX_RECEPTION_DENOMINATOR: u64 = 3_600_000;
pub const START_FEE_CENTS: u64 = 300;
pub const ELECTRIC_PEAK_W: u64 = 24_000;
pub const HEAT_PEAK_W: u64 = 60_000;
pub const WASTE_PEAK_ML_PER_SECOND: u64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { ContinuousGensetV1 }
/// An explicit domain binding only, not a second clock or scheduler. Non-development rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ClockBinding { Development { city_id: u32 }, World, Interaction }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instant { pub scope: Scope, pub clock: ClockBinding, pub second: u64 }

/// One immutable, normalized original Stock lot, exclusively bound to this genset.
/// Origin is the original writer's quantity/basis curve, not a price/profile replacement.
/// The bridge must confirm actual take/refund basis against Candidate::delta before commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputLot {
    pub stock_id: u32, pub lot_ref: u64, pub quantity: Quantity, pub basis: Money,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32, pub profile: Profile, pub scope: Scope, pub city_id: u32,
    pub clock: ClockBinding, pub genset_id: u64, pub source_version: u64,
    /// Frozen common precision compiled by root; must include source thirds exactly.
    pub reception_denominator: u64,
    pub fuel: InputLot, pub water: InputLot,
    pub heat_sink_id: u32, pub wastewater_sink_id: u32,
    /// Finite lifetime allowance, not capacity magically renewed on each query.
    pub heat_capacity_mwh: u64, pub heat_capacity_remainder_numerator: u64,
    pub wastewater_capacity_ml: u64,
    pub heat_peak_w: u64, pub wastewater_peak_ml_per_second: u64,
    /// None means no actual electric consumer: all electricity must go to Heat.
    pub consumer_ref: Option<u64>, pub consumer_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Materialization { ExistingStock, FutureProduction }
/// Original Authority read: nonzero identifiers/root do not authenticate these values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityView {
    pub at: Instant, pub root: [u8;32], pub read_version: u64, pub source_version: u64,
    pub observed_at: u64, pub covers_from: u64, pub covers_until: u64, pub max_age: u64,
    pub fuel_materialization: Materialization, pub water_materialization: Materialization,
    pub fuel_arrived_at: u64, pub water_arrived_at: u64,
    pub fuel_available_mq: Quantity, pub water_available_ml: Quantity,
    pub heat_free_mwh: u64, pub heat_free_remainder_numerator: u64,
    pub wastewater_free_ml: u64, pub payer_available: Money,
}
/// Root-certified actual reception, not predicted demand or an electric inventory credit.
/// Root validates connection, subject, meter, payment, event shape and actual peak.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reception {
    pub scope: Scope, pub clock: ClockBinding,
    pub genset_id: u64, pub source_version: u64, pub run_id: u64,
    pub consumer_ref: Option<u64>, pub consumer_version: u64,
    pub receipt_ref: u64, pub root: [u8;32], pub from: u64, pub until: u64,
    /// Lifetime cumulative actual accepted mWh, retained across stop/restart.
    pub cumulative_received_mwh: u64,
    pub received_remainder_numerator: u64,
    /// Actual certified interval maximum; aggregate energy alone cannot prove this peak.
    pub actual_peak_w: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Status { Stopped, Running, Faulted { fault_ref: u64 } }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u32, pub config_digest: [u8;32], pub cursor: u64,
    pub status: Status, pub run_id: u64, pub run_count: u64,
    /// Lifetime running seconds, never reset by a new run or a query window.
    pub active_seconds: u64,
    pub fuel_taken_mq: u64, pub fuel_returned_mq: u64,
    pub water_taken_ml: u64, pub water_returned_ml: u64,
    pub received_mwh: u64,
    pub received_remainder_numerator: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Command {
    Start { at: Instant, run_id: u64 },
    Advance { at: Instant, reception: Reception },
    Stop { at: Instant },
    Fault { at: Instant, fault_ref: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint { Input, SinkCapacity, SinkPeak, Funding }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Invalid, Version, Clock, Stale, Unknown, Boundary, Overflow, Restore, Basis, Precision,
    /// Infeasible candidate only, not authenticated real shortage/damage evidence.
    Unresolved(Constraint) }

/// Exact lifetime physical and cost meters. Third-mQ remainder is NOT burnt fuel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meters {
    pub fuel_burned_thirds_mq: u64, pub fuel_wip_thirds_mq: u64,
    pub water_consumed_ml: u64, pub water_wip_ml: u64,
    /// All energy fractions below share denominator; projections are not extra energy WIP.
    pub denominator: u64,
    pub electricity_generated_mwh: u64, pub electricity_remainder_numerator: u64,
    pub intrinsic_heat_mwh: u64, pub rejected_electricity_heat_mwh: u64,
    pub rejected_electricity_heat_remainder_numerator: u64,
    pub heat_mwh: u64, pub heat_remainder_numerator: u64, pub wastewater_ml: u64,
    pub fuel_held_basis: Money, pub fuel_converted_basis: Money, pub fuel_wip_basis: Money,
    pub water_held_basis: Money, pub water_converted_basis: Money, pub water_wip_basis: Money,
    /// Fractional converted cost not yet recognized as whole cents, bound to original lot.
    pub fuel_basis_remainder: u64, pub fuel_basis_denominator: u64,
    pub water_basis_remainder: u64, pub water_basis_denominator: u64,
    pub start_fees: Money,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    pub fuel_take_mq: u64, pub water_take_ml: u64,
    pub fuel_return_mq: u64, pub water_return_ml: u64,
    pub fuel_take_basis: Money, pub water_take_basis: Money,
    pub fuel_return_basis: Money, pub water_return_basis: Money,
    pub fuel_converted_basis: Money, pub water_converted_basis: Money,
    pub start_fee: Money,
    pub fuel_burned_thirds_mq: u64, pub water_consumed_ml: u64,
    pub electricity_generated_mwh: u64, pub received_mwh: u64,
    pub denominator: u64, pub electricity_generated_remainder_numerator: u64,
    pub received_remainder_numerator: u64,
    pub intrinsic_heat_mwh: u64, pub rejected_electricity_heat_mwh: u64,
    pub rejected_electricity_heat_remainder_numerator: u64,
    pub heat_mwh: u64, pub heat_remainder_numerator: u64, pub wastewater_ml: u64,
    /// Differences of lifetime whole-meter projections; not the physical exact deltas above.
    pub whole_meters: WholeMeterDelta,
    pub after: Meters,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WholeMeterDelta {
    pub electricity_generated_mwh: u64, pub received_mwh: u64,
    pub heat_mwh: u64, pub rejected_electricity_heat_mwh: u64,
}
/// The actual original take/refund/fee results, supplied by root inside the Prepared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActualBasis {
    pub fuel_take: Money, pub water_take: Money,
    pub fuel_return: Money, pub water_return: Money, pub fee_paid: Money,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub base_digest: [u8;32], pub command: Command, pub view: AuthorityView,
    pub next: State, pub delta: Delta,
}

fn add(a:u64,b:u64)->Result<u64,Error> { a.checked_add(b).ok_or(Error::Overflow) }
fn sub(a:u64,b:u64)->Result<u64,Error> { a.checked_sub(b).ok_or(Error::Invalid) }
fn mul(a:u64,b:u64)->Result<u64,Error> { a.checked_mul(b).ok_or(Error::Overflow) }
fn mul128(a:u128,b:u128)->Result<u128,Error> { a.checked_mul(b).ok_or(Error::Overflow) }
fn add128(a:u128,b:u128)->Result<u128,Error> { a.checked_add(b).ok_or(Error::Overflow) }
fn sub128(a:u128,b:u128)->Result<u128,Error> { a.checked_sub(b).ok_or(Error::Invalid) }
fn exact(whole:u64,remainder:u64,denominator:u64)->Result<u128,Error> {
    if denominator==0 || remainder>=denominator { return Err(Error::Precision); }
    add128(mul128(u128::from(whole),u128::from(denominator))?,u128::from(remainder))
}
fn parts(scaled:u128,denominator:u64)->Result<(u64,u64),Error> {
    if denominator==0 { return Err(Error::Precision); }
    Ok((u64::try_from(scaled/u128::from(denominator)).map_err(|_|Error::Overflow)?,
        (scaled%u128::from(denominator)) as u64))
}
fn generation(t:u64,denominator:u64)->Result<u128,Error> {
    mul128(mul128(u128::from(t),20_000)?,u128::from(denominator/3))
}
fn money(n:u64)->Result<Money,Error> {
    Money::new(i64::try_from(n).map_err(|_|Error::Overflow)?).map_err(|_|Error::Overflow)
}
fn q(q:Quantity)->u64 { q.value() as u64 }
fn m(m:Money)->u64 { m.value() as u64 }
fn fraction(n:u128,d:u128)->Result<u64,Error> {
    if d==0 { return Err(Error::Invalid); }
    u64::try_from(n/d).map_err(|_|Error::Overflow)
}
fn ceil_thirds(n:u64)->Result<u64,Error> { add(n/3,u64::from(n%3!=0)) }
fn hash<T:Serialize>(value:&T)->Result<[u8;32],Error> {
    let mut h=Sha256::new();h.update(b"MyCity/ContinuousGensetV1/1\0");
    h.update(serde_json::to_vec(value).map_err(|_|Error::Invalid)?);
    Ok(h.finalize().into())
}
impl InputLot {
    fn basis_at(&self, milli_thirds:u64)->Result<Money,Error> {
        if q(self.quantity)==0 { return Err(Error::Unknown); }
        money(fraction(mul128(u128::from(m(self.basis)),u128::from(milli_thirds))?,
            mul128(u128::from(q(self.quantity)),3)?)?)
    }
    fn whole_basis(&self, quantity:u64)->Result<Money,Error> { self.basis_at(mul(quantity,3)?) }
}
impl Config {
    pub fn validate(&self)->Result<(),Error> {
        if self.version!=VERSION { return Err(Error::Version); }
        if self.reception_denominator==0 || self.reception_denominator>MAX_RECEPTION_DENOMINATOR
            || self.reception_denominator%3!=0
            || self.heat_capacity_remainder_numerator>=self.reception_denominator {
            return Err(Error::Precision);
        }
        if self.city_id==0 || self.clock!=(ClockBinding::Development {city_id:self.city_id}) {
            return Err(Error::Clock);
        }
        if self.genset_id==0 || self.source_version==0 || self.fuel.lot_ref==0 || self.water.lot_ref==0
            || q(self.fuel.quantity)==0 || q(self.water.quantity)==0
            || self.consumer_ref==Some(0) || (self.consumer_ref.is_some() && self.consumer_version==0) {
            return Err(Error::Unknown);
        }
        let ids=[self.fuel.stock_id,self.water.stock_id,self.heat_sink_id,self.wastewater_sink_id];
        if ids.contains(&0) || ids.iter().enumerate().any(|(i,id)|ids[..i].contains(id)) {
            return Err(Error::Invalid);
        }
        // Fixed original-lot basis denominators must be representable before admission.
        mul(q(self.fuel.quantity),3)?;mul(q(self.water.quantity),3)?;
        Ok(())
    }
    pub fn fingerprint(&self)->Result<[u8;32],Error> { self.validate()?;hash(self) }
    fn instant(&self,at:Instant)->Result<(),Error> {
        if at.scope!=self.scope || at.clock!=self.clock { return Err(Error::Clock); }
        Ok(())
    }
    fn view(&self,v:&AuthorityView,from:u64,until:u64)->Result<(),Error> {
        self.instant(v.at)?;
        if v.source_version!=self.source_version { return Err(Error::Stale); }
        if v.root==[0;32] || v.read_version==0 { return Err(Error::Unknown); }
        if v.at.second!=until || from>until || v.covers_from>from || v.covers_until<until
            || v.observed_at>from || until-v.observed_at>v.max_age { return Err(Error::Stale); }
        if v.fuel_materialization!=Materialization::ExistingStock
            || v.water_materialization!=Materialization::ExistingStock
            || v.fuel_arrived_at>from || v.water_arrived_at>from { return Err(Error::Unknown); }
        Ok(())
    }
}

/// Stateless candidate facade. All state/commit ownership remains with the original writer.
pub struct ContinuousGensetV1;
impl ContinuousGensetV1 {
    pub fn opening(c:&Config,at:Instant)->Result<State,Error> {
        c.validate()?;c.instant(at)?;
        Ok(State {version:VERSION,config_digest:c.fingerprint()?,cursor:at.second,status:Status::Stopped,
            run_id:0,run_count:0,active_seconds:0,fuel_taken_mq:0,fuel_returned_mq:0,
            water_taken_ml:0,water_returned_ml:0,received_mwh:0,received_remainder_numerator:0})
    }
    pub fn meters(c:&Config,s:&State)->Result<Meters,Error> {
        c.validate()?;
        if s.version!=VERSION || s.config_digest!=c.fingerprint()? { return Err(Error::Version); }
        if s.active_seconds>s.cursor || (s.run_count==0 && (s.run_id!=0 || s.active_seconds!=0 || s.status!=Status::Stopped))
            || (s.run_count>0 && s.run_id<s.run_count) { return Err(Error::Invalid); }
        if matches!(s.status,Status::Faulted {fault_ref:0}) { return Err(Error::Unknown); }
        let fuel_net=sub(s.fuel_taken_mq,s.fuel_returned_mq)?;
        let water_net=sub(s.water_taken_ml,s.water_returned_ml)?;
        if fuel_net>q(c.fuel.quantity) || water_net>q(c.water.quantity) { return Err(Error::Invalid); }
        let burned=mul(s.active_seconds,50)?;
        let water=mul(s.active_seconds,1000)?;
        let fuel_wip=sub(mul(fuel_net,3)?,burned)?;
        let water_wip=sub(water_net,water)?;
        // Only the next second may be preloaded; stop/fault returns whole unused ticks.
        if fuel_wip>52 || water_wip>1000
            || (s.status!=Status::Running && (fuel_wip>=3 || water_wip!=0)) {
            return Err(Error::Invalid);
        }
        let den=c.reception_denominator;
        let electric_exact=generation(s.active_seconds,den)?;
        let received_exact=exact(s.received_mwh,s.received_remainder_numerator,den)?;
        if received_exact>electric_exact || (c.consumer_ref.is_none() && received_exact>0) { return Err(Error::Invalid); }
        let intrinsic=mul(s.active_seconds,10_000)?;
        let rejected_exact=sub128(electric_exact,received_exact)?;
        let heat_exact=add128(exact(intrinsic,0,den)?,rejected_exact)?;
        if heat_exact>exact(c.heat_capacity_mwh,c.heat_capacity_remainder_numerator,den)?
            || water>c.wastewater_capacity_ml { return Err(Error::Invalid); }
        let (electric,electricity_remainder_numerator)=parts(electric_exact,den)?;
        let (rejected,rejected_electricity_heat_remainder_numerator)=parts(rejected_exact,den)?;
        let (heat,heat_remainder_numerator)=parts(heat_exact,den)?;
        let fuel_held=c.fuel.whole_basis(fuel_net)?;
        let fuel_converted=c.fuel.basis_at(burned)?;
        let water_held=c.water.whole_basis(water_net)?;
        let water_converted=c.water.whole_basis(water)?;
        let fuel_basis_denominator=mul(q(c.fuel.quantity),3)?;
        let water_basis_denominator=q(c.water.quantity);
        Ok(Meters {fuel_burned_thirds_mq:burned,fuel_wip_thirds_mq:fuel_wip,
            water_consumed_ml:water,water_wip_ml:water_wip,denominator:den,electricity_generated_mwh:electric,
            electricity_remainder_numerator,intrinsic_heat_mwh:intrinsic,
            rejected_electricity_heat_mwh:rejected,rejected_electricity_heat_remainder_numerator,
            heat_mwh:heat,heat_remainder_numerator,wastewater_ml:water,
            fuel_held_basis:fuel_held,fuel_converted_basis:fuel_converted,
            fuel_wip_basis:money(sub(m(fuel_held),m(fuel_converted))?)?,
            water_held_basis:water_held,water_converted_basis:water_converted,
            water_wip_basis:money(sub(m(water_held),m(water_converted))?)?,
            fuel_basis_remainder:(mul128(u128::from(m(c.fuel.basis)),u128::from(burned))?
                %u128::from(fuel_basis_denominator)) as u64,fuel_basis_denominator,
            water_basis_remainder:(mul128(u128::from(m(c.water.basis)),u128::from(water))?
                %u128::from(water_basis_denominator)) as u64,water_basis_denominator,
            start_fees:money(mul(s.run_count,START_FEE_CENTS)?)?})
    }
    pub fn state_digest(s:&State)->Result<[u8;32],Error> { hash(s) }
    /// Bounded JSON decoding plus an independently authenticated owning checkpoint digest.
    /// This function does not read a Store, verify permissions or authenticate its digest.
    pub fn restore(c:&Config,bytes:&[u8],trusted_digest:[u8;32])->Result<State,Error> {
        if bytes.len()>MAX_RESTORE_BYTES { return Err(Error::Restore); }
        let s:State=serde_json::from_slice(bytes).map_err(|_|Error::Restore)?;
        if hash(&s)?!=trusted_digest { return Err(Error::Restore); }
        Self::meters(c,&s)?;Ok(s)
    }
    pub fn propose(c:&Config,base:&State,v:&AuthorityView,command:Command)->Result<Candidate,Error> {
        let before=Self::meters(c,base)?;
        let mut next=base.clone();
        let at=match &command {Command::Start {at,..}|Command::Advance {at,..}
            |Command::Stop {at}|Command::Fault {at,..}=>*at};
        c.instant(at)?;
        match &command {
            Command::Start {run_id,..}=>{
                if base.status==Status::Running || *run_id<=base.run_id || at.second<base.cursor {
                    return Err(Error::Boundary);
                }
                c.view(v,at.second,at.second)?;
                if m(v.payer_available)<START_FEE_CENTS { return Err(Error::Unresolved(Constraint::Funding)); }
                // Reserve actual existing inputs for first second. No conversion or energy here.
                let future=add(base.active_seconds,1)?;
                load(c,&mut next,ceil_thirds(mul(future,50)?)?,mul(future,1000)?,v)?;
                let den=c.reception_denominator;
                let first_electric=sub128(generation(future,den)?,generation(base.active_seconds,den)?)?;
                check_sinks(c,v,&before,add128(exact(10_000,0,den)?,first_electric)?,1000)?;
                next.status=Status::Running;next.run_id=*run_id;
                next.run_count=add(next.run_count,1)?;next.cursor=at.second;
            },
            Command::Advance {reception,..}=>{
                if base.status!=Status::Running || at.second<=base.cursor
                    || at.second-base.cursor>MAX_ADVANCE_SECONDS { return Err(Error::Boundary); }
                c.view(v,base.cursor,at.second)?;
                let dt=at.second-base.cursor;
                next.active_seconds=add(base.active_seconds,dt)?;
                let future=next.active_seconds;
                load(c,&mut next,ceil_thirds(mul(future,50)?)?,mul(future,1000)?,v)?;
                let den=c.reception_denominator;
                let generated_delta=sub128(generation(next.active_seconds,den)?,generation(base.active_seconds,den)?)?;
                if reception.scope!=c.scope || reception.clock!=c.clock { return Err(Error::Clock); }
                if reception.root!=v.root || reception.receipt_ref==0 || reception.from!=base.cursor
                    || reception.until!=at.second || reception.consumer_ref!=c.consumer_ref
                    || reception.consumer_version!=c.consumer_version || reception.genset_id!=c.genset_id
                    || reception.source_version!=c.source_version || reception.run_id!=base.run_id {
                    return Err(Error::Stale);
                }
                let received=sub128(exact(reception.cumulative_received_mwh,reception.received_remainder_numerator,den)?,
                    exact(base.received_mwh,base.received_remainder_numerator,den)?)?;
                if received>generated_delta || reception.actual_peak_w>ELECTRIC_PEAK_W
                    || (received>0 && reception.consumer_ref.is_none()) { return Err(Error::Invalid); }
                // Peak is separately certified, never inferred from a rounded whole-meter jump.
                let peak_n=mul128(mul128(mul128(u128::from(reception.actual_peak_w),1000)?,u128::from(dt))?,u128::from(den))?;
                if mul128(received,3600)?>peak_n { return Err(Error::Invalid); }
                next.received_mwh=reception.cumulative_received_mwh;
                next.received_remainder_numerator=reception.received_remainder_numerator;next.cursor=at.second;
                let intrinsic_delta=mul(dt,10_000)?;
                let rejected_delta=sub128(generated_delta,received)?;
                let heat_delta=add128(exact(intrinsic_delta,0,den)?,rejected_delta)?;
                let water_delta=mul(dt,1000)?;
                // Worst case unreceived electric is Heat: never average 60kW into a lower peak.
                check_sinks(c,v,&before,heat_delta,water_delta)?;
            },
            Command::Stop {..}|Command::Fault {..}=>{
                if base.status!=Status::Running || at.second!=base.cursor { return Err(Error::Boundary); }
                c.view(v,at.second,at.second)?;
                let fuel_return=before.fuel_wip_thirds_mq/3;
                next.fuel_returned_mq=add(next.fuel_returned_mq,fuel_return)?;
                next.water_returned_ml=add(next.water_returned_ml,before.water_wip_ml)?;
                next.status=match &command {
                    Command::Fault {fault_ref,..}=>{
                        if *fault_ref==0 { return Err(Error::Unknown); }
                        Status::Faulted {fault_ref:*fault_ref}
                    },
                    _=>Status::Stopped,
                };
            },
        }
        let after=Self::meters(c,&next)?;
        let delta=deltas(c,base,&next,&before,after)?;
        Ok(Candidate {base_digest:hash(base)?,command,view:v.clone(),next,delta})
    }
    /// Recomputes with latest root view. Stale state, view/root or modified amounts are refused.
    pub fn revalidate(c:&Config,base:&State,current:&AuthorityView,p:&Candidate)->Result<State,Error> {
        if p.base_digest!=hash(base)? || current!=&p.view
            || Self::propose(c,base,current,p.command.clone())?!=*p { return Err(Error::Stale); }
        Ok(p.next.clone())
    }
    pub fn confirm_actual_basis(p:&Candidate,a:&ActualBasis)->Result<(),Error> {
        if a.fuel_take!=p.delta.fuel_take_basis || a.water_take!=p.delta.water_take_basis
            || a.fuel_return!=p.delta.fuel_return_basis || a.water_return!=p.delta.water_return_basis
            || a.fee_paid!=p.delta.start_fee { return Err(Error::Basis); }
        Ok(())
    }
}
fn check_sinks(c:&Config,v:&AuthorityView,b:&Meters,heat:u128,water:u64)->Result<(),Error> {
    if c.heat_peak_w<HEAT_PEAK_W || c.wastewater_peak_ml_per_second<WASTE_PEAK_ML_PER_SECOND {
        return Err(Error::Unresolved(Constraint::SinkPeak));
    }
    let den=c.reception_denominator;
    if heat>exact(v.heat_free_mwh,v.heat_free_remainder_numerator,den)? || water>v.wastewater_free_ml
        || add128(exact(b.heat_mwh,b.heat_remainder_numerator,den)?,heat)?
            >exact(c.heat_capacity_mwh,c.heat_capacity_remainder_numerator,den)?
        || add(b.wastewater_ml,water)?>c.wastewater_capacity_ml {
        return Err(Error::Unresolved(Constraint::SinkCapacity));
    }
    Ok(())
}
fn load(c:&Config,s:&mut State,fuel_required:u64,water_required:u64,v:&AuthorityView)->Result<(),Error> {
    let net_fuel=sub(s.fuel_taken_mq,s.fuel_returned_mq)?;
    let net_water=sub(s.water_taken_ml,s.water_returned_ml)?;
    let fuel_take=fuel_required.saturating_sub(net_fuel);
    let water_take=water_required.saturating_sub(net_water);
    if fuel_take>q(v.fuel_available_mq) || water_take>q(v.water_available_ml)
        || fuel_required>q(c.fuel.quantity) || water_required>q(c.water.quantity) {
        return Err(Error::Unresolved(Constraint::Input));
    }
    s.fuel_taken_mq=add(s.fuel_taken_mq,fuel_take)?;
    s.water_taken_ml=add(s.water_taken_ml,water_take)?;Ok(())
}
fn basis_delta(l:&InputLot,before:u64,after:u64)->Result<(Money,Money),Error> {
    let old=m(l.whole_basis(before)?);let new=m(l.whole_basis(after)?);
    Ok((money(new.saturating_sub(old))?,money(old.saturating_sub(new))?))
}
fn deltas(c:&Config,b:&State,n:&State,bm:&Meters,after:Meters)->Result<Delta,Error> {
    let den=c.reception_denominator;
    let (electricity_generated_mwh,electricity_generated_remainder_numerator)=parts(
        sub128(generation(n.active_seconds,den)?,generation(b.active_seconds,den)?)?,den)?;
    let (received_mwh,received_remainder_numerator)=parts(
        sub128(exact(n.received_mwh,n.received_remainder_numerator,den)?,
            exact(b.received_mwh,b.received_remainder_numerator,den)?)?,den)?;
    let (rejected_electricity_heat_mwh,rejected_electricity_heat_remainder_numerator)=parts(
        sub128(exact(after.rejected_electricity_heat_mwh,after.rejected_electricity_heat_remainder_numerator,den)?,
            exact(bm.rejected_electricity_heat_mwh,bm.rejected_electricity_heat_remainder_numerator,den)?)?,den)?;
    let (heat_mwh,heat_remainder_numerator)=parts(
        sub128(exact(after.heat_mwh,after.heat_remainder_numerator,den)?,
            exact(bm.heat_mwh,bm.heat_remainder_numerator,den)?)?,den)?;
    let (fuel_take_basis,fuel_return_basis)=basis_delta(&c.fuel,
        sub(b.fuel_taken_mq,b.fuel_returned_mq)?,sub(n.fuel_taken_mq,n.fuel_returned_mq)?)?;
    let (water_take_basis,water_return_basis)=basis_delta(&c.water,
        sub(b.water_taken_ml,b.water_returned_ml)?,sub(n.water_taken_ml,n.water_returned_ml)?)?;
    Ok(Delta {fuel_take_mq:sub(n.fuel_taken_mq,b.fuel_taken_mq)?,
        water_take_ml:sub(n.water_taken_ml,b.water_taken_ml)?,fuel_return_mq:sub(n.fuel_returned_mq,b.fuel_returned_mq)?,
        water_return_ml:sub(n.water_returned_ml,b.water_returned_ml)?,
        fuel_take_basis,water_take_basis,fuel_return_basis,water_return_basis,
        fuel_converted_basis:money(sub(m(after.fuel_converted_basis),m(bm.fuel_converted_basis))?)?,
        water_converted_basis:money(sub(m(after.water_converted_basis),m(bm.water_converted_basis))?)?,
        start_fee:money(sub(m(after.start_fees),m(bm.start_fees))?)?,
        fuel_burned_thirds_mq:sub(after.fuel_burned_thirds_mq,bm.fuel_burned_thirds_mq)?,
        water_consumed_ml:sub(after.water_consumed_ml,bm.water_consumed_ml)?,
        denominator:den,electricity_generated_mwh,electricity_generated_remainder_numerator,
        received_mwh,received_remainder_numerator,
        intrinsic_heat_mwh:sub(after.intrinsic_heat_mwh,bm.intrinsic_heat_mwh)?,
        rejected_electricity_heat_mwh,rejected_electricity_heat_remainder_numerator,
        heat_mwh,heat_remainder_numerator,wastewater_ml:sub(after.wastewater_ml,bm.wastewater_ml)?,
        whole_meters:WholeMeterDelta {
            electricity_generated_mwh:sub(after.electricity_generated_mwh,bm.electricity_generated_mwh)?,
            received_mwh:sub(n.received_mwh,b.received_mwh)?,heat_mwh:sub(after.heat_mwh,bm.heat_mwh)?,
            rejected_electricity_heat_mwh:sub(after.rejected_electricity_heat_mwh,bm.rejected_electricity_heat_mwh)?,
        },after})
}
