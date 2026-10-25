//! #92 bounded exact quality side records and RawPurificationV1 proposals.
//! These records project original Stock/lot quantities; they are never spendable inventory.
//! No writer, scheduler, provider, wallet, Store authentication or sanitary-use authority.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;

pub const VERSION: u32 = 1;
/// Cursor-only idle coverage policy; does not change production or chemistry policy.
pub const IDLE_POLICY_VERSION: u32 = 1;
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
/// Historical whole-interval idle evidence. An ID/digest or current stopped status
/// is insufficient: root must authenticate original Timeline coverage and ensure no
/// already accepted treatment work is skipped. No electricity/cost ticket is invented.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdleFacts {
    pub policy_version:u32, pub binding:Binding, pub from:u64, pub until:u64,
    pub read_version:u64, pub timeline_revision:u64, pub idle_receipt_ref:u64,
}
pub trait RootVerifier {
    fn verifies(&self, config:&Config, base:&Checkpoint, request:Request, facts:&RootFacts)->bool;
    /// Default-deny preserves the production-only verifier's authority boundary.
    /// Implementations check complete [from,until) history, original source receipts,
    /// accepted-work revision and root epoch/CAS. No use/right renewal or liability release.
    fn verifies_idle(&self, _config:&Config, _base:&Checkpoint, _facts:&IdleFacts)->bool { false }
    /// Explicit funding-V2 authority. Old production/idle verifiers cannot authorize it.
    /// Authenticate complete cost basis, real original allocations/payments, exact
    /// interval/purpose/amount/CAS and globally single-use coverage in original journals.
    fn verifies_funding_v2(&self, _config:&FundingConfigV2, _base:&FundingCheckpointV2,
        _request:Request, _facts:&FundingFactsV2)->bool { false }
    /// Additional default-deny gate for an explicitly permitted original accrual policy.
    /// This creates no new lending permission and never authorizes a late payment cost.
    fn verifies_accrual_v2(&self, _config:&FundingConfigV2, _base:&FundingCheckpointV2,
        _request:Request, _facts:&FundingFactsV2, _item:usize)->bool { false }
    /// Funding-V2 idle authority is independent of V1 idle and V2 production authority.
    /// Root authenticates whole original history and all unchanged funding/material facts.
    fn verifies_funding_idle_v2(&self, _config:&FundingConfigV2, _base:&FundingCheckpointV2,
        _facts:&FundingIdleFactsV2)->bool { false }
}
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
/// Separate typed candidate: never a zero-input production candidate or provider grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdleCandidate {
    pub policy_version:u32, pub base_digest:[u8;32], pub facts:IdleFacts,
    pub next:Checkpoint, pub delta:Delta,
}
impl Delta {
    fn zero()->Self {
        Self { input_ml:Exact::ZERO,reagent_mq:Exact::ZERO,electricity_mwh:Exact::ZERO,
            product:Composition::known_zero(Exact::ZERO),concentrate:Composition::known_zero(Exact::ZERO),
            sludge:Composition::known_zero(Exact::ZERO),raw_take_ml:0,reagent_take_mq:0,
            raw_take_basis_cents:0,reagent_take_basis_cents:0,
            raw_converted_basis:Exact::ZERO,reagent_converted_basis:Exact::ZERO,root_interval_cost_cents:0 }
    }
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
            || (checkpoint.sequence!=0 && checkpoint.cursor==self.binding.origin_cursor)
            || (checkpoint.processed_ml.is_zero() && checkpoint.raw_stream_basis!=self.initial_stream_basis()) {
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
    // Shared physical arithmetic only, called after the respective version's root
    // authorization. No cash augmentation, synthetic ticket or V1 verifier adapter.
    fn compute_materials(&self, before:&Meters, cp:&Checkpoint, request:Request,
        facts:&MaterialFactsV2, v1_costs:Option<[u64;3]>)->Result<(Checkpoint,Delta),Error> {
        let input=cp.processed_ml.add(request.input_ml)?;
        let after=self.meters_at(input,facts.raw_stream_basis_after)?;
        let product=composition_delta(after.product,before.product)?;
        let concentrate=composition_delta(after.concentrate,before.concentrate)?;
        let sludge=composition_delta(after.sludge,before.sludge)?;
        let interval_cost_cents=match v1_costs {
            Some([electricity,personnel,residual])=>electricity.checked_add(personnel)
                .and_then(|x|x.checked_add(residual)).ok_or(Error::Overflow)?,
            None=>0, // V2 returns separate fully reconciled costs, not this legacy field.
        };
        let delta=Delta { input_ml:request.input_ml,reagent_mq:after.reagent_mq.sub(before.reagent_mq)?,
            electricity_mwh:after.electricity_mwh.sub(before.electricity_mwh)?,product,concentrate,sludge,
            raw_take_ml:after.raw_taken_ml-before.raw_taken_ml,reagent_take_mq:after.reagent_taken_mq-before.reagent_taken_mq,
            raw_take_basis_cents:after.raw_held_basis_cents.checked_sub(before.raw_held_basis_cents).ok_or(Error::Invalid)?,
            reagent_take_basis_cents:after.reagent_held_basis_cents-before.reagent_held_basis_cents,
            raw_converted_basis:after.raw_converted_basis.sub(before.raw_converted_basis)?,
            reagent_converted_basis:after.reagent_converted_basis.sub(before.reagent_converted_basis)?,
            root_interval_cost_cents:interval_cost_cents };
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
        let next=Checkpoint { version:VERSION,config_digest:cp.config_digest,cursor:request.until,
            sequence:cp.sequence.checked_add(1).ok_or(Error::Overflow)?,processed_ml:input,raw_stream_basis:facts.raw_stream_basis_after };
        Ok((next,delta))
    }
    pub fn prepare<V:RootVerifier>(&self, verifier:&V, state:&State, request:Request, facts:&RootFacts, work:u32)->Result<Candidate,Error> {
        if work==0 { return Err(Error::WorkBudget); }
        let before=self.meters(state)?; let cp=&state.checkpoint;
        if request.until<=cp.cursor || request.input_ml.is_zero() { return Err(Error::Invalid); }
        if facts.binding!=self.binding || facts.from!=cp.cursor || facts.until!=request.until || facts.read_version==0 {
            return Err(Error::Stale);
        }
        if facts.supply_receipt_ref==0 || facts.cost_receipt_ref==0 || !verifier.verifies(self,cp,request,facts) { return Err(Error::Unauthorized); }
        let (next,delta)=self.compute_materials(&before,cp,request,&MaterialFactsV2::from_v1(facts),
            Some([facts.electricity_basis_cents,facts.personnel_cents,facts.residual_cents]))?;
        if delta.root_interval_cost_cents>facts.cash_available_cents || delta.root_interval_cost_cents>MAX_WHOLE { return Err(Error::UnresolvedCost); }
        Ok(Candidate { version:VERSION,base_digest:digest(cp)?,facts:facts.clone(),request,next,delta })
    }
    /// Advance only the verified historical idle cursor. All persisted material,
    /// prefetched WIP, original StreamBasis and every derived meter/cost stay unchanged.
    /// This is a proposal: only root's original Envelope/receipt writer can commit it.
    pub fn cover_idle<V:RootVerifier>(&self, verifier:&V, base:&State, facts:&IdleFacts, work:u32)->Result<IdleCandidate,Error> {
        if work==0 { return Err(Error::WorkBudget); }
        self.restore(base.checkpoint.clone())?;
        let cp=&base.checkpoint;
        if facts.policy_version!=IDLE_POLICY_VERSION { return Err(Error::Invalid); }
        if facts.binding!=self.binding || facts.from!=cp.cursor { return Err(Error::Stale); }
        if facts.until<=facts.from { return Err(Error::Invalid); }
        if facts.read_version==0 || facts.timeline_revision==0 || facts.idle_receipt_ref==0
            || !verifier.verifies_idle(self,cp,facts) { return Err(Error::Unauthorized); }
        let next=self.idle_next(cp,facts.until)?;
        Ok(IdleCandidate { policy_version:IDLE_POLICY_VERSION,base_digest:digest(cp)?,
            facts:facts.clone(),next,delta:Delta::zero() })
    }
    fn idle_next(&self,cp:&Checkpoint,until:u64)->Result<Checkpoint,Error> {
        let mut next=cp.clone(); next.cursor=until;
        next.sequence=cp.sequence.checked_add(1).ok_or(Error::Overflow)?;
        self.restore(next.clone())?; Ok(next)
    }
    pub fn revalidate_idle<V:RootVerifier>(&self, verifier:&V, base:&State, candidate:&IdleCandidate, fresh:&IdleFacts)->Result<State,Error> {
        if candidate.policy_version!=IDLE_POLICY_VERSION || candidate.base_digest!=digest(&base.checkpoint)?
            || &candidate.facts!=fresh { return Err(Error::Stale); }
        let expected=self.cover_idle(verifier,base,fresh,1)?;
        if &expected!=candidate { return Err(Error::Stale); }
        self.restore(candidate.next.clone())
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

/// Independent funding envelope. VERSION/Config/Checkpoint/RootFacts V1 wire stays V1.
pub const FUNDING_VERSION: u32 = 2;
pub const MAX_FUNDING_ITEMS: usize = 16;
pub const FUNDING_COST_KINDS: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingProfileV2 { ActualFundingV2 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccrualPolicyV2 { pub policy_ref:u64, pub version:u32 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingConfigV2 {
    pub version:u32, pub profile:FundingProfileV2, pub material:Config,
    /// None by default at the caller: no implicit credit or inherited V1 debt authority.
    pub accrual_policy:Option<AccrualPolicyV2>,
}
/// Only physical facts, not a V1 cash/ticket DTO with fabricated funding values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialFactsV2 {
    pub binding:Binding, pub from:u64, pub until:u64, pub read_version:u64,
    pub raw_stock_available_ml:u64, pub reagent_stock_available_mq:u64,
    pub electricity_available_mwh:Exact, pub raw_stream_basis_after:Option<StreamBasis>,
    pub output_free_ml:Exact, pub concentrate_free_ml:Exact, pub sludge_free_mq:Exact,
    pub supply_receipt_ref:u64,
}
impl MaterialFactsV2 {
    fn from_v1(f:&RootFacts)->Self {
        Self { binding:f.binding,from:f.from,until:f.until,read_version:f.read_version,
            raw_stock_available_ml:f.raw_stock_available_ml,reagent_stock_available_mq:f.reagent_stock_available_mq,
            electricity_available_mwh:f.electricity_available_mwh,raw_stream_basis_after:f.raw_stream_basis_after,
            output_free_ml:f.output_free_ml,concentrate_free_ml:f.concentrate_free_ml,
            sludge_free_mq:f.sludge_free_mq,supply_receipt_ref:f.supply_receipt_ref }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostKindV2 {
    RawInput, ReagentInput, ElectricityInput, Personnel,
    EquipmentOperation, MaintenanceUse, ResidualAcceptance, ResidualCustody,
}
impl CostKindV2 {
    fn index(self)->usize { match self {
        Self::RawInput=>0,Self::ReagentInput=>1,Self::ElectricityInput=>2,Self::Personnel=>3,
        Self::EquipmentOperation=>4,Self::MaintenanceUse=>5,Self::ResidualAcceptance=>6,Self::ResidualCustody=>7,
    } }
}
/// Original envelope/source plus its immutable specific cost allocation and exact
/// half-open cumulative basis span (cents). One envelope may have many legitimate
/// purposes; remaining ceil-prefetched WIP may later consume another disjoint span.
/// Root authenticates these identities against actual inputs/payments, original allocation
/// budgets/purpose/remaining WIP and all previous journal claims. New IDs are not proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingEvidenceV2 {
    pub source_ref:u64, pub source_revision:u64, pub from:u64, pub until:u64, pub purpose_ref:u64,
    pub allocation_ref:u64, pub cost_from:Exact, pub cost_until:Exact,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CoverageV2 {
    OriginalStockConversion { evidence:FundingEvidenceV2 },
    SamePreparedPaid { evidence:FundingEvidenceV2 },
    PriorReceiptPaid { evidence:FundingEvidenceV2 },
    CashDueNow { evidence:FundingEvidenceV2 },
    AccruedPayable { evidence:FundingEvidenceV2, policy:AccrualPolicyV2 },
}
impl CoverageV2 {
    fn evidence(self)->FundingEvidenceV2 { match self {
        Self::OriginalStockConversion { evidence } | Self::SamePreparedPaid { evidence }
        | Self::PriorReceiptPaid { evidence } | Self::CashDueNow { evidence }
        | Self::AccruedPayable { evidence,.. }=>evidence,
    } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingItemV2 { pub kind:CostKindV2, pub basis:Exact, pub coverage:CoverageV2 }
/// These are original earned/converted interval basis, not a quote or a second payment.
/// Maintenance/wear-use is distinct from wages/operation; custody is not lawful disposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntervalBasisV2 {
    pub electricity_cents:u64, pub personnel_cents:u64, pub equipment_operation_cents:u64,
    pub maintenance_use_cents:u64, pub residual_acceptance_cents:u64, pub residual_custody_cents:u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingFactsV2 {
    pub version:u32, pub physical:MaterialFactsV2, pub basis:IntervalBasisV2,
    pub items:[Option<FundingItemV2>;MAX_FUNDING_ITEMS],
    /// Real uncommitted cash for CashDueNow ONLY; never add inventory/paid basis here.
    pub cash_available_cents:u64, pub funding_revision:u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingTotalsV2 {
    pub total_basis:Exact, pub original_stock:Exact, pub same_prepared_paid:Exact,
    pub prior_receipt_paid:Exact, pub cash_due_now:Exact, pub accrued_payable:Exact,
}
impl FundingTotalsV2 {
    pub const ZERO:Self=Self { total_basis:Exact::ZERO,original_stock:Exact::ZERO,
        same_prepared_paid:Exact::ZERO,prior_receipt_paid:Exact::ZERO,cash_due_now:Exact::ZERO,accrued_payable:Exact::ZERO };
    fn validate(self)->Result<(),Error> {
        let sum=self.original_stock.add(self.same_prepared_paid)?.add(self.prior_receipt_paid)?
            .add(self.cash_due_now)?.add(self.accrued_payable)?;
        if sum!=self.total_basis || self.same_prepared_paid.parts().1!=0 || self.prior_receipt_paid.parts().1!=0
            || self.cash_due_now.parts().1!=0 || self.accrued_payable.parts().1!=0 { return Err(Error::Invalid); }
        Ok(())
    }
    fn add(self,b:Self)->Result<Self,Error> {
        let n=Self { total_basis:self.total_basis.add(b.total_basis)?,original_stock:self.original_stock.add(b.original_stock)?,
            same_prepared_paid:self.same_prepared_paid.add(b.same_prepared_paid)?,prior_receipt_paid:self.prior_receipt_paid.add(b.prior_receipt_paid)?,
            cash_due_now:self.cash_due_now.add(b.cash_due_now)?,accrued_payable:self.accrued_payable.add(b.accrued_payable)? };
        n.validate()?; Ok(n)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingCheckpointV2 {
    pub version:u32, pub profile:FundingProfileV2, pub config_digest:[u8;32],
    pub physical:Checkpoint, pub funding:FundingTotalsV2,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FundingStateV2 { checkpoint:FundingCheckpointV2 }
impl FundingStateV2 { pub fn checkpoint(&self)->FundingCheckpointV2 { self.checkpoint.clone() } }
/// Physical delta with no ambiguous legacy root_interval_cost_cents cash field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialDeltaV2 {
    pub input_ml:Exact, pub reagent_mq:Exact, pub electricity_mwh:Exact,
    pub product:Composition, pub concentrate:Composition, pub sludge:Composition,
    pub raw_take_ml:u64, pub reagent_take_mq:u64,
    pub raw_take_basis_cents:u64, pub reagent_take_basis_cents:u64,
    pub raw_converted_basis:Exact, pub reagent_converted_basis:Exact,
}
impl From<Delta> for MaterialDeltaV2 {
    fn from(d:Delta)->Self {
        Self { input_ml:d.input_ml,reagent_mq:d.reagent_mq,electricity_mwh:d.electricity_mwh,
            product:d.product,concentrate:d.concentrate,sludge:d.sludge,
            raw_take_ml:d.raw_take_ml,reagent_take_mq:d.reagent_take_mq,
            raw_take_basis_cents:d.raw_take_basis_cents,reagent_take_basis_cents:d.reagent_take_basis_cents,
            raw_converted_basis:d.raw_converted_basis,reagent_converted_basis:d.reagent_converted_basis }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingDeltaV2 { pub materials:MaterialDeltaV2, pub costs:FundingTotalsV2 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingCandidateV2 {
    pub version:u32, pub profile:FundingProfileV2, pub base_digest:[u8;32],
    pub facts:FundingFactsV2, pub request:Request, pub next:FundingCheckpointV2, pub delta:FundingDeltaV2,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FundingMetersV2 { pub materials:Meters, pub costs:FundingTotalsV2 }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingIdleFactsV2 {
    pub version:u32, pub material:IdleFacts, pub funding_revision:u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingIdleCandidateV2 {
    pub version:u32, pub profile:FundingProfileV2, pub base_digest:[u8;32],
    pub facts:FundingIdleFactsV2, pub next:FundingCheckpointV2, pub delta:FundingDeltaV2,
}
impl FundingConfigV2 {
    pub fn validate(&self)->Result<(),Error> {
        if self.version!=FUNDING_VERSION { return Err(Error::Invalid); }
        self.material.validate()?;
        if let Some(p)=self.accrual_policy { if p.policy_ref==0 || p.version==0 { return Err(Error::Invalid); } }
        Ok(())
    }
    pub fn open(&self)->Result<FundingStateV2,Error> {
        self.validate()?;
        Ok(FundingStateV2 { checkpoint:FundingCheckpointV2 { version:FUNDING_VERSION,profile:self.profile,
            config_digest:digest(self)?,physical:self.material.open()?.checkpoint(),funding:FundingTotalsV2::ZERO } })
    }
    /// Structural funding envelope restore only. No V1 auto-upgrade, credit grant,
    /// late payment replay or root opening/coverage receipt authentication is implied.
    pub fn restore(&self,cp:FundingCheckpointV2)->Result<FundingStateV2,Error> {
        self.validate()?;
        if cp.version!=FUNDING_VERSION || cp.profile!=self.profile || cp.config_digest!=digest(self)? { return Err(Error::Stale); }
        let physical=self.material.restore(cp.physical.clone())?;
        cp.funding.validate()?;
        if (cp.physical.processed_ml.is_zero() && cp.funding!=FundingTotalsV2::ZERO)
            || (self.accrual_policy.is_none() && !cp.funding.accrued_payable.is_zero()) { return Err(Error::Invalid); }
        let m=self.material.meters(&physical)?;
        let inputs=m.raw_converted_basis.add(m.reagent_converted_basis)?;
        if cp.funding.original_stock.cmp_exact(inputs)?==Ordering::Less { return Err(Error::Invalid); }
        Ok(FundingStateV2 { checkpoint:cp })
    }
    pub fn meters(&self,state:&FundingStateV2)->Result<FundingMetersV2,Error> {
        let state=self.restore(state.checkpoint())?;
        let physical=self.material.restore(state.checkpoint.physical)?;
        Ok(FundingMetersV2 { materials:self.material.meters(&physical)?,costs:state.checkpoint.funding })
    }
    pub fn prepare<V:RootVerifier>(&self,verifier:&V,state:&FundingStateV2,request:Request,
        facts:&FundingFactsV2,work:u32)->Result<FundingCandidateV2,Error> {
        if work==0 { return Err(Error::WorkBudget); }
        self.restore(state.checkpoint())?;
        let cp=&state.checkpoint; let p=&facts.physical;
        if request.until<=cp.physical.cursor || request.input_ml.is_zero() { return Err(Error::Invalid); }
        if facts.version!=FUNDING_VERSION { return Err(Error::Invalid); }
        if p.binding!=self.material.binding || p.from!=cp.physical.cursor || p.until!=request.until || p.read_version==0 { return Err(Error::Stale); }
        if p.supply_receipt_ref==0 || facts.funding_revision==0
            || !verifier.verifies_funding_v2(self,cp,request,facts) { return Err(Error::Unauthorized); }
        let before=self.material.meters(&self.material.restore(cp.physical.clone())?)?;
        let (physical,legacy_delta)=self.material.compute_materials(&before,&cp.physical,request,p,None)?;
        let materials=MaterialDeltaV2::from(legacy_delta);
        let costs=self.check_funding(verifier,cp,request,facts,&materials)?;
        let next=FundingCheckpointV2 { version:FUNDING_VERSION,profile:self.profile,config_digest:cp.config_digest,
            physical,funding:cp.funding.add(costs)? };
        self.restore(next.clone())?;
        Ok(FundingCandidateV2 { version:FUNDING_VERSION,profile:self.profile,base_digest:digest(cp)?,
            facts:facts.clone(),request,next,delta:FundingDeltaV2 { materials,costs } })
    }
    pub fn revalidate<V:RootVerifier>(&self,verifier:&V,base:&FundingStateV2,candidate:&FundingCandidateV2,
        fresh:&FundingFactsV2)->Result<FundingStateV2,Error> {
        if candidate.version!=FUNDING_VERSION || candidate.profile!=self.profile
            || candidate.base_digest!=digest(&base.checkpoint)? || &candidate.facts!=fresh { return Err(Error::Stale); }
        let expected=self.prepare(verifier,base,candidate.request,fresh,1)?;
        if expected!=*candidate { return Err(Error::Stale); }
        self.restore(candidate.next.clone())
    }
    pub fn cover_idle<V:RootVerifier>(&self,verifier:&V,base:&FundingStateV2,facts:&FundingIdleFactsV2,
        work:u32)->Result<FundingIdleCandidateV2,Error> {
        if work==0 { return Err(Error::WorkBudget); }
        self.restore(base.checkpoint())?;
        let cp=&base.checkpoint; let f=&facts.material;
        if facts.version!=FUNDING_VERSION || f.policy_version!=IDLE_POLICY_VERSION { return Err(Error::Invalid); }
        if f.binding!=self.material.binding || f.from!=cp.physical.cursor { return Err(Error::Stale); }
        if f.until<=f.from { return Err(Error::Invalid); }
        if facts.funding_revision==0 || f.read_version==0 || f.timeline_revision==0 || f.idle_receipt_ref==0
            || !verifier.verifies_funding_idle_v2(self,cp,facts) { return Err(Error::Unauthorized); }
        let mut next=cp.clone(); next.physical=self.material.idle_next(&cp.physical,f.until)?;
        self.restore(next.clone())?;
        Ok(FundingIdleCandidateV2 { version:FUNDING_VERSION,profile:self.profile,base_digest:digest(cp)?,
            facts:facts.clone(),next,delta:FundingDeltaV2 { materials:MaterialDeltaV2::from(Delta::zero()),costs:FundingTotalsV2::ZERO } })
    }
    pub fn revalidate_idle<V:RootVerifier>(&self,verifier:&V,base:&FundingStateV2,candidate:&FundingIdleCandidateV2,
        fresh:&FundingIdleFactsV2)->Result<FundingStateV2,Error> {
        if candidate.version!=FUNDING_VERSION || candidate.profile!=self.profile
            || candidate.base_digest!=digest(&base.checkpoint)? || &candidate.facts!=fresh { return Err(Error::Stale); }
        let expected=self.cover_idle(verifier,base,fresh,1)?;
        if expected!=*candidate { return Err(Error::Stale); }
        self.restore(candidate.next.clone())
    }
    fn check_funding<V:RootVerifier>(&self,verifier:&V,base:&FundingCheckpointV2,request:Request,
        facts:&FundingFactsV2,materials:&MaterialDeltaV2)->Result<FundingTotalsV2,Error> {
        let b=facts.basis;
        let expected=[materials.raw_converted_basis,materials.reagent_converted_basis,
            Exact::integer(b.electricity_cents)?,Exact::integer(b.personnel_cents)?,Exact::integer(b.equipment_operation_cents)?,
            Exact::integer(b.maintenance_use_cents)?,Exact::integer(b.residual_acceptance_cents)?,Exact::integer(b.residual_custody_cents)?];
        let mut by_kind=[Exact::ZERO;FUNDING_COST_KINDS];
        let mut totals=FundingTotalsV2::ZERO; let mut gap=false;
        for (i,item) in facts.items.iter().enumerate() {
            let item=match item { None=> { gap=true;continue; },Some(item)=>item };
            if gap || item.basis.is_zero() { return Err(Error::Invalid); }
            let e=item.coverage.evidence(); let p=&facts.physical;
            if e.source_ref==0 || e.source_revision==0 || e.purpose_ref==0 || e.allocation_ref==0
                || e.from!=p.from || e.until!=p.until || e.cost_until.sub(e.cost_from)?!=item.basis { return Err(Error::Invalid); }
            for previous in facts.items[..i].iter().flatten() {
                let old=previous.coverage.evidence();
                if old.source_ref==e.source_ref && old.allocation_ref==e.allocation_ref
                    && e.cost_from.cmp_exact(old.cost_until)?==Ordering::Less
                    && old.cost_from.cmp_exact(e.cost_until)?==Ordering::Less { return Err(Error::Invalid); }
            }
            let raw_or_reagent=matches!(item.kind,CostKindV2::RawInput|CostKindV2::ReagentInput);
            if raw_or_reagent && !matches!(item.coverage,CoverageV2::OriginalStockConversion { .. }) { return Err(Error::Invalid); }
            if !matches!(item.coverage,CoverageV2::OriginalStockConversion { .. }) && item.basis.parts().1!=0 { return Err(Error::Precision); }
            by_kind[item.kind.index()]=by_kind[item.kind.index()].add(item.basis)?;
            totals.total_basis=totals.total_basis.add(item.basis)?;
            match item.coverage {
                CoverageV2::OriginalStockConversion { .. }=> {
                    if !matches!(item.kind,CostKindV2::RawInput|CostKindV2::ReagentInput|CostKindV2::ElectricityInput) { return Err(Error::Invalid); }
                    totals.original_stock=totals.original_stock.add(item.basis)?;
                },
                CoverageV2::SamePreparedPaid { .. }=>totals.same_prepared_paid=totals.same_prepared_paid.add(item.basis)?,
                CoverageV2::PriorReceiptPaid { .. }=>totals.prior_receipt_paid=totals.prior_receipt_paid.add(item.basis)?,
                CoverageV2::CashDueNow { .. }=>totals.cash_due_now=totals.cash_due_now.add(item.basis)?,
                CoverageV2::AccruedPayable { policy,.. }=> {
                    if self.accrual_policy!=Some(policy) || !verifier.verifies_accrual_v2(self,base,request,facts,i) { return Err(Error::Unauthorized); }
                    totals.accrued_payable=totals.accrued_payable.add(item.basis)?;
                },
            }
        }
        if by_kind!=expected { return Err(Error::Invalid); }
        totals.validate()?;
        if facts.cash_available_cents>MAX_WHOLE { return Err(Error::Overflow); }
        if totals.cash_due_now.cmp_exact(Exact::integer(facts.cash_available_cents)?)?==Ordering::Greater { return Err(Error::UnresolvedCost); }
        Ok(totals)
    }
}
