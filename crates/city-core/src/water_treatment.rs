//! #92 bounded exact quality side records and RawPurificationV1 proposals.
//! These records project original Stock/lot quantities; they are never spendable inventory.
//! No writer, scheduler, provider, wallet, Store authentication or sanitary-use authority.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;

pub const VERSION: u32 = 1;
pub const PRECISION_VERSION: u32 = 1;
/// Independent water/mass precision policy; NOT the electric network's denominator grant.
pub const MAX_DENOMINATOR: u64 = 1_000_000_000_000_000_000;
pub const MAX_WHOLE: u64 = i64::MAX as u64;
pub const MAX_REFERENCES: usize = 4;
pub const MAX_SIDE_RECORDS: usize = 4;
pub const MAX_CANDIDATES: usize = 1;
pub const MAX_STREAM_PORTS: usize = 2;
/// Conservative JSON envelope estimates, not measured allocations or resource grants.
pub const SAVED_ENVELOPE_ESTIMATE: usize = 12 * 1024;
pub const CANDIDATE_ENVELOPE_ESTIMATE: usize = 32 * 1024;
pub const PERSISTENT_PLANNING_CAP: usize = 16 * 1024;
pub const WORKSPACE_PLANNING_CAP: usize = 96 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Precision, Overflow, Negative, Invalid, Stale, UnknownComposition,
    UnsupportedComponents, UnsupportedProfile, ReferenceLimit, Unauthorized,
    UnresolvedInput, UnresolvedEnergy, UnresolvedSink, UnresolvedCost, WorkBudget,
}

/// Exact nonnegative amount: fields specify mL, mWh, mQ or cents, never interchangeable.
/// Canonical reduced whole/remainder/denominator. No rounding or tolerance is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Exact { whole: u64, rem: u64, den: u64 }
impl<'de> Deserialize<'de> for Exact {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Saved { whole: u64, rem: u64, den: u64 }
        let s = Saved::deserialize(d)?;
        Self::restore(s.whole, s.rem, s.den).map_err(|_| serde::de::Error::custom("water_exact"))
    }
}
impl Exact {
    pub const ZERO: Self = Self { whole: 0, rem: 0, den: 1 };
    pub fn integer(n: u64) -> Result<Self, Error> { Self::fraction(u128::from(n), 1) }
    pub fn fraction(n: u128, d: u128) -> Result<Self, Error> {
        if d == 0 { return Err(Error::Precision); }
        let g = gcd(n, d); let n = n / g; let d = d / g;
        if d > u128::from(MAX_DENOMINATOR) { return Err(Error::Precision); }
        let whole = u64::try_from(n / d).map_err(|_| Error::Overflow)?;
        if whole > MAX_WHOLE { return Err(Error::Overflow); }
        Ok(Self { whole, rem: (n % d) as u64, den: d as u64 })
    }
    pub fn restore(whole: u64, rem: u64, den: u64) -> Result<Self, Error> {
        if den == 0 || rem >= den || den > MAX_DENOMINATOR || whole > MAX_WHOLE {
            return Err(Error::Precision);
        }
        let x = Self::fraction(u128::from(whole) * u128::from(den) + u128::from(rem), u128::from(den))?;
        if x != (Self { whole, rem, den }) { return Err(Error::Precision); }
        Ok(x)
    }
    pub fn parts(self) -> (u64, u64, u64) { (self.whole, self.rem, self.den) }
    pub fn floor(self) -> u64 { self.whole }
    pub fn ceil(self) -> Result<u64, Error> {
        let n = self.whole.checked_add(u64::from(self.rem != 0)).ok_or(Error::Overflow)?;
        if n > MAX_WHOLE { return Err(Error::Overflow); } Ok(n)
    }
    pub fn is_zero(self) -> bool { self == Self::ZERO }
    fn pair(self) -> (u128, u128) {
        (u128::from(self.whole) * u128::from(self.den) + u128::from(self.rem), u128::from(self.den))
    }
    pub fn add(self, rhs: Self) -> Result<Self, Error> { self.combine(rhs, false) }
    pub fn sub(self, rhs: Self) -> Result<Self, Error> { self.combine(rhs, true) }
    fn combine(self, rhs: Self, subtract: bool) -> Result<Self, Error> {
        let (a,b) = self.pair(); let (c,d) = rhs.pair(); let g = gcd(b,d);
        let left = a.checked_mul(d/g).ok_or(Error::Overflow)?;
        let right = c.checked_mul(b/g).ok_or(Error::Overflow)?;
        let n = if subtract { left.checked_sub(right).ok_or(Error::Negative)? }
            else { left.checked_add(right).ok_or(Error::Overflow)? };
        Self::fraction(n, b.checked_mul(d/g).ok_or(Error::Overflow)?)
    }
    pub fn ratio(self, num: u64, den: u64) -> Result<Self, Error> {
        self.mul_fraction(u128::from(num), u128::from(den))
    }
    fn mul_fraction(self, mut num: u128, mut den: u128) -> Result<Self, Error> {
        if den == 0 { return Err(Error::Precision); }
        let (mut a,mut b) = self.pair();
        let g = gcd(a,den); a /= g; den /= g;
        let g = gcd(num,b); num /= g; b /= g;
        Self::fraction(a.checked_mul(num).ok_or(Error::Overflow)?, b.checked_mul(den).ok_or(Error::Overflow)?)
    }
    pub fn cmp_exact(self, rhs: Self) -> Result<Ordering, Error> {
        let (a,b) = self.pair(); let (c,d) = rhs.pair(); let g = gcd(b,d);
        Ok(a.checked_mul(d/g).ok_or(Error::Overflow)?.cmp(&c.checked_mul(b/g).ok_or(Error::Overflow)?))
    }
    fn portion(self, take: Self, total: Self) -> Result<Self, Error> {
        if total.is_zero() || take.cmp_exact(total)? == Ordering::Greater { return Err(Error::Invalid); }
        let (a,b) = take.pair(); let (c,d) = total.pair();
        // Cancel before forming a/t; subsequent multiplication is checked again.
        let g = gcd(a,c); let h = gcd(d,b);
        self.mul_fraction((a/g).checked_mul(d/h).ok_or(Error::Overflow)?,
            (b/h).checked_mul(c/g).ok_or(Error::Overflow)?)
    }
}
fn gcd(mut a: u128, mut b: u128) -> u128 { while b != 0 { let r = a%b; a=b; b=r; } a.max(1) }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mass { Unknown, Known(Exact) }
impl Mass {
    fn add(self, rhs: Self) -> Result<Self, Error> {
        match (self,rhs) { (Self::Known(a),Self::Known(b)) => Ok(Self::Known(a.add(b)?)), _ => Ok(Self::Unknown) }
    }
    fn portion(self, take: Exact, total: Exact) -> Result<Self, Error> {
        match self { Self::Unknown => Ok(Self::Unknown), Self::Known(a) => Ok(Self::Known(a.portion(take,total)?)) }
    }
    fn known(self) -> Result<Exact, Error> { match self { Self::Known(a)=>Ok(a), Self::Unknown=>Err(Error::UnknownComposition) } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub water_ml: Exact,
    pub harmful_mq: Mass,
    pub suspended_mq: Mass,
    pub allowed_dissolved_mq: Mass,
    /// Catch-all coverage: Unknown if the provider cannot rule out untracked components.
    pub other_mq: Mass,
}
impl Composition {
    pub fn known_zero(water_ml: Exact) -> Self {
        Self { water_ml, harmful_mq: Mass::Known(Exact::ZERO), suspended_mq: Mass::Known(Exact::ZERO),
            allowed_dissolved_mq: Mass::Known(Exact::ZERO), other_mq: Mass::Known(Exact::ZERO) }
    }
    pub fn mix(self, rhs: Self) -> Result<Self, Error> {
        Ok(Self { water_ml:self.water_ml.add(rhs.water_ml)?, harmful_mq:self.harmful_mq.add(rhs.harmful_mq)?,
            suspended_mq:self.suspended_mq.add(rhs.suspended_mq)?, allowed_dissolved_mq:self.allowed_dissolved_mq.add(rhs.allowed_dissolved_mq)?,
            other_mq:self.other_mq.add(rhs.other_mq)? })
    }
    /// Homogeneous transfer/leak: both receiver and remainder must be committed by root.
    /// Unknown stays unknown on both sides; exact last outlet uses subtraction.
    pub fn split(self, water_ml: Exact) -> Result<(Self,Self),Error> {
        let remainder = self.water_ml.sub(water_ml)?;
        if self.water_ml.is_zero() { return Err(Error::Invalid); }
        let take = Self { water_ml, harmful_mq:self.harmful_mq.portion(water_ml,self.water_ml)?,
            suspended_mq:self.suspended_mq.portion(water_ml,self.water_ml)?,
            allowed_dissolved_mq:self.allowed_dissolved_mq.portion(water_ml,self.water_ml)?,
            other_mq:self.other_mq.portion(water_ml,self.water_ml)? };
        let residual = |a:Mass,b:Mass| -> Result<Mass,Error> { match (a,b) {
            (Mass::Known(a),Mass::Known(b))=>Ok(Mass::Known(a.sub(b)?)), _=>Ok(Mass::Unknown) } };
        Ok((take,Self { water_ml:remainder, harmful_mq:residual(self.harmful_mq,take.harmful_mq)?,
            suspended_mq:residual(self.suspended_mq,take.suspended_mq)?,
            allowed_dissolved_mq:residual(self.allowed_dissolved_mq,take.allowed_dissolved_mq)?, other_mq:residual(self.other_mq,take.other_mq)? }))
    }
    /// Only this profile's water evaporates. All four nonvolatile mass fields stay behind.
    /// The returned water-only composition requires a real natural receiving endpoint.
    pub fn evaporate(self, water_ml: Exact) -> Result<(Self,Self),Error> {
        let mut retained=self; retained.water_ml=self.water_ml.sub(water_ml)?;
        Ok((retained,Self::known_zero(water_ml)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub source: u64, pub version: u64, pub coverage_from: u64, pub coverage_until: u64,
    pub observed_at: u64, pub purpose_ref: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LotQuality {
    pub stock_id: u32, pub lot_ref: u64, pub composition: Composition,
    pub references: [Option<SourceRef>; MAX_REFERENCES],
}
impl LotQuality {
    pub fn validate(&self) -> Result<(), Error> {
        if self.stock_id==0 || self.lot_ref==0 || self.references[0].is_none() { return Err(Error::Invalid); }
        let mut empty=false;
        for (i,r) in self.references.iter().enumerate() {
            match r { None=>empty=true, Some(r)=> {
                if empty || r.source==0 || r.version==0 || r.purpose_ref==0 || r.coverage_from>r.coverage_until
                    || r.observed_at>r.coverage_until || self.references[..i].contains(&Some(*r)) { return Err(Error::Invalid); }
            } }
        } Ok(())
    }
    /// Preserve all provenance on mixing. No source is silently truncated at the bound.
    pub fn mixed(&self, rhs:&Self, stock_id:u32, lot_ref:u64) -> Result<Self,Error> {
        self.validate()?; rhs.validate()?;
        let mut refs=self.references;
        for r in rhs.references.iter().flatten() {
            if !refs.contains(&Some(*r)) {
                let slot=refs.iter_mut().find(|x|x.is_none()).ok_or(Error::ReferenceLimit)?; *slot=Some(*r);
            }
        }
        let out=Self { stock_id,lot_ref,composition:self.composition.mix(rhs.composition)?,references:refs };
        out.validate()?; Ok(out)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standard { OrdinaryGameWaterV1, ReclaimedIndustrialGreenV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityJudgment { CompositionUnknown, KnownOutOfSpec, MeetsGameChemicalStandard(Standard) }
/// Concentration classification only. It grants no drinking/medical/police/fire-use permission.
pub fn judge(c:Composition, standard:Standard) -> Result<QualityJudgment,Error> {
    let (h,s,a,o)=match (c.harmful_mq,c.suspended_mq,c.allowed_dissolved_mq,c.other_mq) {
        (Mass::Known(h),Mass::Known(s),Mass::Known(a),Mass::Known(o))=>(h,s,a,o),
        _=>return Ok(QualityJudgment::CompositionUnknown),
    };
    let _allowed_is_separate=a;
    if c.water_ml.is_zero() || !s.is_zero() || !o.is_zero() { return Ok(QualityJudgment::KnownOutOfSpec); }
    // 0.00002 Q/m3 = 1 mQ / 50,000,000 mL; reclaimed = 1 / 10,000,000.
    let den=match standard { Standard::OrdinaryGameWaterV1=>50_000_000, Standard::ReclaimedIndustrialGreenV1=>10_000_000 };
    if h.cmp_exact(c.water_ml.ratio(1,den)?)?==Ordering::Greater { Ok(QualityJudgment::KnownOutOfSpec) }
    else { Ok(QualityJudgment::MeetsGameChemicalStandard(standard)) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub world_ref:u64, pub root_epoch:u64, pub source_version:u64, pub city_id:u32,
    pub operation:u64, pub origin_cursor:u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { RawPurificationV1, SewageUnsupportedV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputLot { pub stock_id:u32, pub lot_ref:u64, pub quantity_milli:u64, pub basis_cents:u64 }
/// FrozenLot prices one original batch. RootConfirmedStream names original producer
/// output ports, never an invented aggregate Stock. Their actual production-cost curve
/// and deterministic take order remain root-owned and are rechecked by RootVerifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawBasisMode { FrozenLot, RootConfirmedStream { ports:[u32;MAX_STREAM_PORTS] } }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamBasis {
    pub taken_cents:u64, pub converted:Exact, pub wip:Exact,
}
impl StreamBasis {
    pub const ZERO:Self=Self { taken_cents:0,converted:Exact::ZERO,wip:Exact::ZERO };
    fn validate(self)->Result<(),Error> {
        if self.taken_cents>MAX_WHOLE || self.converted.add(self.wip)?!=Exact::integer(self.taken_cents)? { return Err(Error::Invalid); }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version:u32, pub precision_version:u32, pub profile:Profile, pub binding:Binding,
    /// In stream mode raw/raw_origin describe the common original quality source
    /// (e.g. raw103), not spendable output quantity/basis. Actual stock is only ports.
    pub raw:LotQuality, pub raw_origin:InputLot, pub reagent_origin:InputLot,
    pub raw_basis:RawBasisMode,
    pub output_stock:u32, pub concentrate_stock:u32, pub sludge_stock:u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub version:u32, pub config_digest:[u8;32], pub cursor:u64, pub sequence:u64,
    /// Cumulative actual converted mL in this immutable original lot, not a water pool.
    pub processed_ml:Exact,
    pub raw_stream_basis:Option<StreamBasis>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State { checkpoint:Checkpoint }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request { pub until:u64, pub input_ml:Exact }
/// Fresh root-owned facts for this proposed half-open Development interval. The verifier
/// must bind actual same-Prepared electricity, remaining original Stock/WIP, source/coverage,
/// finite lawful sinks, labor/payment and source journal authentication; no future supplies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootFacts {
    pub binding:Binding, pub from:u64, pub until:u64, pub read_version:u64,
    pub raw_stock_available_ml:u64, pub reagent_stock_available_mq:u64, pub electricity_available_mwh:Exact,
    /// Root's cumulative NEXT original-port take/converted/WIP cost, not a quoted
    /// average or predicted final cost. Required only for RootConfirmedStream.
    pub raw_stream_basis_after:Option<StreamBasis>,
    pub output_free_ml:Exact, pub concentrate_free_ml:Exact, pub sludge_free_mq:Exact,
    /// Actual interval costs supplied by root. These are NOT prices invented by the recipe.
    pub electricity_basis_cents:u64, pub personnel_cents:u64, pub residual_cents:u64,
    pub cash_available_cents:u64, pub cost_receipt_ref:u64, pub supply_receipt_ref:u64,
}
pub trait RootVerifier { fn verifies(&self, config:&Config, base:&Checkpoint, request:Request, facts:&RootFacts)->bool; }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meters {
    pub input_ml:Exact, pub reagent_mq:Exact, pub electricity_mwh:Exact,
    pub product:Composition, pub concentrate:Composition, pub sludge:Composition,
    /// Ceil-prefetched original Stock mL/mQ and unused exact fractions. Not actual conversion.
    pub raw_taken_ml:u64, pub raw_wip:Composition, pub reagent_taken_mq:u64, pub reagent_wip_mq:Exact,
    pub raw_converted_basis:Exact, pub reagent_converted_basis:Exact,
    pub raw_held_basis_cents:u64, pub reagent_held_basis_cents:u64,
    pub raw_wip_basis:Exact, pub reagent_wip_basis:Exact,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    pub input_ml:Exact, pub reagent_mq:Exact, pub electricity_mwh:Exact,
    pub product:Composition, pub concentrate:Composition, pub sludge:Composition,
    pub raw_take_ml:u64, pub reagent_take_mq:u64,
    pub raw_take_basis_cents:u64, pub reagent_take_basis_cents:u64,
    pub raw_converted_basis:Exact, pub reagent_converted_basis:Exact,
    pub root_interval_cost_cents:u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version:u32, pub base_digest:[u8;32], pub facts:RootFacts, pub request:Request,
    pub next:Checkpoint, pub delta:Delta,
}
fn digest<T:Serialize>(x:&T)->Result<[u8;32],Error> {
    // Fixed structs/arrays only: no unbounded strings, maps, vectors or histories.
    let bytes=serde_json::to_vec(x).map_err(|_|Error::Invalid)?;
    Ok(Sha256::digest(bytes).into())
}
impl Config {
    pub fn validate(&self)->Result<(),Error> {
        if self.version!=VERSION || self.precision_version!=PRECISION_VERSION { return Err(Error::Precision); }
        if self.profile!=Profile::RawPurificationV1 { return Err(Error::UnsupportedProfile); }
        self.raw.validate()?;
        let b=self.binding;
        if b.world_ref==0 || b.root_epoch==0 || b.source_version==0 || b.city_id==0 || b.operation==0 { return Err(Error::Invalid); }
        let ids=[self.raw_origin.stock_id,self.reagent_origin.stock_id,self.output_stock,self.concentrate_stock,self.sludge_stock];
        for (i,id) in ids.iter().enumerate() { if *id==0 || ids[..i].contains(id) { return Err(Error::Invalid); } }
        if let RawBasisMode::RootConfirmedStream { ports }=self.raw_basis {
            for (i,id) in ports.iter().enumerate() {
                if *id==0 || ids.contains(id) || ports[..i].contains(id) { return Err(Error::Invalid); }
            }
        }
        for lot in [self.raw_origin,self.reagent_origin] {
            if lot.lot_ref==0 || lot.quantity_milli==0 || lot.quantity_milli>MAX_WHOLE || lot.basis_cents>MAX_WHOLE { return Err(Error::Invalid); }
        }
        if self.raw.stock_id!=self.raw_origin.stock_id || self.raw.lot_ref!=self.raw_origin.lot_ref
            || self.raw.composition.water_ml!=Exact::integer(self.raw_origin.quantity_milli)? { return Err(Error::Invalid); }
        let c=self.raw.composition;
        let _h=c.harmful_mq.known()?;
        let _s=c.suspended_mq.known()?;
        let _a=c.allowed_dissolved_mq.known()?;
        if !c.other_mq.known()?.is_zero() { return Err(Error::UnsupportedComponents); }
        // Preflight the immutable full-lot endpoint under this precision policy.
        // Oversized/unsupported exact arithmetic rejects admission; it is not a shortage.
        let reagent_bound=Exact::integer(self.reagent_origin.quantity_milli)?;
        let maximum=if c.water_ml.ratio(1,1_000_000)?.cmp_exact(reagent_bound)?==Ordering::Greater {
            reagent_bound.ratio(1_000_000,1)?
        } else { c.water_ml };
        self.meters_at(maximum,self.initial_stream_basis())?;
        Ok(())
    }
    pub fn open(&self)->Result<State,Error> {
        self.validate()?;
        Ok(State { checkpoint:Checkpoint { version:VERSION, config_digest:digest(self)?,
            cursor:self.binding.origin_cursor, sequence:0, processed_ml:Exact::ZERO,raw_stream_basis:self.initial_stream_basis() } })
    }
    /// Structure/precision/cumulative-bound validation only. Caller must authenticate original
    /// opening and last-commit journal receipts before accepting any restored provider facts.
    pub fn restore(&self, checkpoint:Checkpoint)->Result<State,Error> {
        self.validate()?;
        if checkpoint.version!=VERSION || checkpoint.config_digest!=digest(self)?
            || checkpoint.cursor<self.binding.origin_cursor || checkpoint.sequence>checkpoint.cursor-self.binding.origin_cursor
            || (checkpoint.sequence==0 && (checkpoint.cursor!=self.binding.origin_cursor || !checkpoint.processed_ml.is_zero() || checkpoint.raw_stream_basis!=self.initial_stream_basis()))
            || (checkpoint.sequence!=0 && (checkpoint.processed_ml.is_zero() || checkpoint.cursor==self.binding.origin_cursor)) {
            return Err(Error::Stale);
        }
        self.meters_at(checkpoint.processed_ml,checkpoint.raw_stream_basis)?;
        Ok(State { checkpoint })
    }
    pub fn meters(&self, state:&State)->Result<Meters,Error> {
        self.restore(state.checkpoint.clone())?; self.meters_at(state.checkpoint.processed_ml,state.checkpoint.raw_stream_basis)
    }
    fn initial_stream_basis(&self)->Option<StreamBasis> {
        match self.raw_basis { RawBasisMode::FrozenLot=>None, RawBasisMode::RootConfirmedStream { .. }=>Some(StreamBasis::ZERO) }
    }
    fn meters_at(&self, input:Exact, stream_basis:Option<StreamBasis>)->Result<Meters,Error> {
        match (self.raw_basis,stream_basis) {
            (RawBasisMode::FrozenLot,None)=>{},
            (RawBasisMode::RootConfirmedStream { .. },Some(b))=>b.validate()?,
            _=>return Err(Error::Invalid),
        }
        let raw_total=Exact::integer(self.raw_origin.quantity_milli)?;
        if input.cmp_exact(raw_total)?==Ordering::Greater { return Err(Error::UnresolvedInput); }
        let reagent=input.ratio(1,1_000_000)?;
        if reagent.ceil()?>self.reagent_origin.quantity_milli { return Err(Error::UnresolvedInput); }
        let harmful=self.raw.composition.harmful_mq.known()?.portion(input,raw_total)?;
        let suspended=self.raw.composition.suspended_mq.known()?.portion(input,raw_total)?;
        let dissolved=self.raw.composition.allowed_dissolved_mq.known()?.portion(input,raw_total)?;
        let out_water=input.ratio(98,100)?;
        let mut product=Composition::known_zero(out_water);
        product.harmful_mq=Mass::Known(harmful.ratio(1,100)?);
        product.allowed_dissolved_mq=Mass::Known(dissolved.ratio(98,100)?.add(reagent.ratio(1,10)?)?);
        let mut concentrate=Composition::known_zero(input.sub(out_water)?);
        concentrate.harmful_mq=Mass::Known(harmful.sub(product.harmful_mq.known()?)?);
        concentrate.allowed_dissolved_mq=Mass::Known(dissolved.sub(dissolved.ratio(98,100)?)?.add(reagent.ratio(1,5)?)?);
        let mut sludge=Composition::known_zero(Exact::ZERO);
        sludge.suspended_mq=Mass::Known(suspended);
        sludge.allowed_dissolved_mq=Mass::Known(reagent.sub(reagent.ratio(1,10)?)?.sub(reagent.ratio(1,5)?)?);
        let raw_taken=input.ceil()?; let reagent_taken=reagent.ceil()?;
        let raw_wip_water=Exact::integer(raw_taken)?.sub(input)?;
        if let Some(b)=stream_basis { if raw_wip_water.is_zero() && !b.wip.is_zero() { return Err(Error::Invalid); } }
        let mut raw_wip=Composition::known_zero(raw_wip_water);
        raw_wip.harmful_mq=Mass::Known(self.raw.composition.harmful_mq.known()?.portion(raw_wip_water,raw_total)?);
        raw_wip.suspended_mq=Mass::Known(self.raw.composition.suspended_mq.known()?.portion(raw_wip_water,raw_total)?);
        raw_wip.allowed_dissolved_mq=Mass::Known(self.raw.composition.allowed_dissolved_mq.known()?.portion(raw_wip_water,raw_total)?);
        let raw_converted_basis=match stream_basis { Some(b)=>b.converted,None=>lot_basis(self.raw_origin,input)? };
        let reagent_converted_basis=lot_basis(self.reagent_origin,reagent)?;
        let raw_held_basis_cents=match stream_basis { Some(b)=>b.taken_cents,None=>lot_basis(self.raw_origin,Exact::integer(raw_taken)?)?.floor() };
        let reagent_held_basis_cents=lot_basis(self.reagent_origin,Exact::integer(reagent_taken)?)?.floor();
        // Exact cost curve is retained; whole-cent Stock take uses cumulative floor. Its
        // fractional entitlement stays on the original source lot, not an unfunded WIP cent.
        let raw_wip_basis=match stream_basis { Some(b)=>b.wip,None=>lot_basis(self.raw_origin,raw_wip_water)? };
        let reagent_wip_mq=Exact::integer(reagent_taken)?.sub(reagent)?;
        let reagent_wip_basis=lot_basis(self.reagent_origin,reagent_wip_mq)?;
        Ok(Meters { input_ml:input,reagent_mq:reagent,electricity_mwh:input.ratio(1,5)?,
            product,concentrate,sludge,raw_taken_ml:raw_taken,raw_wip,reagent_taken_mq:reagent_taken,reagent_wip_mq,
            raw_converted_basis,reagent_converted_basis,raw_held_basis_cents,reagent_held_basis_cents,raw_wip_basis,reagent_wip_basis })
    }
    pub fn prepare<V:RootVerifier>(&self, verifier:&V, state:&State, request:Request, facts:&RootFacts, work:u32)->Result<Candidate,Error> {
        if work==0 { return Err(Error::WorkBudget); }
        let before=self.meters(state)?; let cp=&state.checkpoint;
        if request.until<=cp.cursor || request.input_ml.is_zero() { return Err(Error::Invalid); }
        if facts.binding!=self.binding || facts.from!=cp.cursor || facts.until!=request.until || facts.read_version==0 {
            return Err(Error::Stale);
        }
        if facts.supply_receipt_ref==0 || facts.cost_receipt_ref==0 || !verifier.verifies(self,cp,request,facts) { return Err(Error::Unauthorized); }
        let input=cp.processed_ml.add(request.input_ml)?;
        let after=self.meters_at(input,facts.raw_stream_basis_after)?;
        let product=composition_delta(after.product,before.product)?;
        let concentrate=composition_delta(after.concentrate,before.concentrate)?;
        let sludge=composition_delta(after.sludge,before.sludge)?;
        let delta=Delta { input_ml:request.input_ml,reagent_mq:after.reagent_mq.sub(before.reagent_mq)?,
            electricity_mwh:after.electricity_mwh.sub(before.electricity_mwh)?,product,concentrate,sludge,
            raw_take_ml:after.raw_taken_ml-before.raw_taken_ml,reagent_take_mq:after.reagent_taken_mq-before.reagent_taken_mq,
            raw_take_basis_cents:after.raw_held_basis_cents.checked_sub(before.raw_held_basis_cents).ok_or(Error::Invalid)?,
            reagent_take_basis_cents:after.reagent_held_basis_cents-before.reagent_held_basis_cents,
            raw_converted_basis:after.raw_converted_basis.sub(before.raw_converted_basis)?,
            reagent_converted_basis:after.reagent_converted_basis.sub(before.reagent_converted_basis)?,
            root_interval_cost_cents:facts.electricity_basis_cents.checked_add(facts.personnel_cents).and_then(|x|x.checked_add(facts.residual_cents)).ok_or(Error::Overflow)? };
        if delta.raw_take_ml==0 && delta.raw_take_basis_cents!=0 { return Err(Error::Invalid); }
        // Available original Stock excludes prior ceil-prefetched WIP. Check both
        // fresh integer takes and exact consumption; WIP is counted precisely once.
        if facts.raw_stock_available_ml>MAX_WHOLE || facts.reagent_stock_available_mq>MAX_WHOLE { return Err(Error::Overflow); }
        if delta.raw_take_ml>facts.raw_stock_available_ml || delta.reagent_take_mq>facts.reagent_stock_available_mq
            || delta.input_ml.cmp_exact(Exact::integer(facts.raw_stock_available_ml)?.add(before.raw_wip.water_ml)?)?==Ordering::Greater
            || delta.reagent_mq.cmp_exact(Exact::integer(facts.reagent_stock_available_mq)?.add(before.reagent_wip_mq)?)?==Ordering::Greater { return Err(Error::UnresolvedInput); }
        if delta.electricity_mwh.cmp_exact(facts.electricity_available_mwh)?==Ordering::Greater { return Err(Error::UnresolvedEnergy); }
        let sludge_mass=sludge.suspended_mq.known()?.add(sludge.allowed_dissolved_mq.known()?)?;
        if product.water_ml.cmp_exact(facts.output_free_ml)?==Ordering::Greater
            || concentrate.water_ml.cmp_exact(facts.concentrate_free_ml)?==Ordering::Greater
            || sludge_mass.cmp_exact(facts.sludge_free_mq)?==Ordering::Greater { return Err(Error::UnresolvedSink); }
        if delta.root_interval_cost_cents>facts.cash_available_cents || delta.root_interval_cost_cents>MAX_WHOLE { return Err(Error::UnresolvedCost); }
        let next=Checkpoint { version:VERSION,config_digest:cp.config_digest,cursor:request.until,
            sequence:cp.sequence.checked_add(1).ok_or(Error::Overflow)?,processed_ml:input,raw_stream_basis:facts.raw_stream_basis_after };
        Ok(Candidate { version:VERSION,base_digest:digest(cp)?,facts:facts.clone(),request,next,delta })
    }
    /// Recompute against fresh root facts. Digests and nonzero references are never authority.
    pub fn revalidate<V:RootVerifier>(&self, verifier:&V, base:&State, candidate:&Candidate, fresh:&RootFacts)->Result<State,Error> {
        if candidate.version!=VERSION || candidate.base_digest!=digest(&base.checkpoint)? || &candidate.facts!=fresh { return Err(Error::Stale); }
        let expected=self.prepare(verifier,base,candidate.request,fresh,1)?;
        if &expected!=candidate { return Err(Error::Stale); }
        self.restore(candidate.next.clone())
    }
}
impl State { pub fn checkpoint(&self)->Checkpoint { self.checkpoint.clone() } }
fn lot_basis(lot:InputLot, amount:Exact)->Result<Exact,Error> {
    if amount.cmp_exact(Exact::integer(lot.quantity_milli)?)?==Ordering::Greater { return Err(Error::UnresolvedInput); }
    amount.ratio(lot.basis_cents,lot.quantity_milli)
}
fn composition_delta(a:Composition,b:Composition)->Result<Composition,Error> {
    Ok(Composition { water_ml:a.water_ml.sub(b.water_ml)?,harmful_mq:Mass::Known(a.harmful_mq.known()?.sub(b.harmful_mq.known()?)?),
        suspended_mq:Mass::Known(a.suspended_mq.known()?.sub(b.suspended_mq.known()?)?),
        allowed_dissolved_mq:Mass::Known(a.allowed_dissolved_mq.known()?.sub(b.allowed_dissolved_mq.known()?)?),
        other_mq:Mass::Known(a.other_mq.known()?.sub(b.other_mq.known()?)?) })
}
