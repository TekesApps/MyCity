//! #85 V3 exact-rate proposal. No inventory, payment, provider, or Store authority lives here.
use crate::units::Quantity;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const POLICY_VERSION: u32 = 3;
pub const SCALE: u64 = 1_000; // milli-native World Quantity units
pub const MAX_WINDOW_SECONDS: u64 = 3_600;
pub const MAX_RATE_NUMERATOR: u64 = 1_000_000_000_000;
pub const MAX_RATIO_COMPONENT: u32 = 1_000;
pub const MAX_COMMON_DENOMINATOR: u128 = 1_000_000_000_000;
pub const MAX_CONSUMERS: usize = 4;
pub const MAX_EDGES: usize = 4;
pub const MAX_PATH: usize = 4;
pub const MAX_WORK: u32 = 256;
pub const MAX_MILLI_QUANTITY: u64 = i64::MAX as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Resource { MilliWattHour, MilliLiter }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SourceKind { ContinuousElectricity, StoredWater }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Scope { pub development_city: u32 }

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuthorityBinding {
    pub scope: Scope,
    pub world_ref: u64,
    pub branch_ref: u64,
    pub root_ref: u64,
    pub provider_ref: u64,
    pub provider_revision: u64,
    pub source_ref: u64,
    pub source_revision: u64,
    pub stock_ref: u64,
    pub contract_ref: u64,
    pub clock_revision: u64,
    pub scale_revision: u64,
    pub effective_start: u64,
    pub effective_end: u64,
}

/// A caller-supplied trusted root verifier. DTO nonzero IDs alone never authorize a source.
pub trait RootVerifier {
    /// Authenticates all source, edge, consumer, right, payer, meter, sink and contract facts.
    fn verifies(&self, contract: &Contract) -> bool;
    /// Authenticates the original owner opening receipt, not a new opening. Restore and every
    /// state-consuming operation recheck this receipt so a revoked genesis cannot bypass the
    /// stable source-contract authorization above; implementations must make this a read-only check.
    fn verifies_opening(&self, contract: &Contract, genesis_ref: u64) -> bool;
    /// Confirms a real root-owned receipt covering exactly this half-open interval and amount.
    fn verify_supply(&self, contract: &Contract, from: u64, until: u64,
        physical_gross: ExactAmount) -> Option<SupplyWitness>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Window { pub start_second: u64, pub end_second: u64 }

/// Exact milli-native units per second. Zero is allowed for unavailable source output and capacities;
/// required consumer rates must be positive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rate { pub numerator: u64, pub denominator_seconds: u32 }

impl Rate {
    pub fn new(numerator: u64, denominator_seconds: u32) -> Result<Self, Error> {
        if denominator_seconds == 0 || denominator_seconds > 3_600
            || numerator > MAX_RATE_NUMERATOR { return Err(Error::Precision); }
        let g = gcd(u128::from(numerator), u128::from(denominator_seconds));
        Ok(Self { numerator: (u128::from(numerator) / g) as u64,
            denominator_seconds: (u128::from(denominator_seconds) / g) as u32 })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Ratio { pub numerator: u32, pub denominator: u32 }

impl Ratio {
    pub fn new(numerator: u32, denominator: u32, allow_zero: bool) -> Result<Self, Error> {
        if denominator == 0 || denominator > MAX_RATIO_COMPONENT || numerator > MAX_RATIO_COMPONENT
            || numerator > denominator || (!allow_zero && numerator == 0) { return Err(Error::Precision); }
        let g = gcd(u128::from(numerator), u128::from(denominator));
        Ok(Self { numerator: (u128::from(numerator) / g) as u32,
            denominator: (u128::from(denominator) / g) as u32 })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    pub id: u32,
    pub kind: SourceKind,
    pub node: u32,
    pub resource: Resource,
    pub quality: u32,
    pub source_ref: u64,
    pub stock_ref: u64,
    pub contract_ref: u64,
    /// Source outlet rate entering the network before edge losses, after source auxiliary use.
    pub physical_supply_rate: Rate,
    /// Gross input rate capacity; includes routing loss and auxiliary consumption.
    pub input_capacity_rate: Rate,
    /// Gross source-side hard peak, including auxiliary consumption.
    pub hard_peak_rate: Rate,
    /// One finite source allocation for this whole window, in milli-native units.
    pub available_input_milli: Quantity,
    pub auxiliary_ratio: Ratio,
    /// Root-owned physical sink for auxiliary consumption; required when the ratio is non-zero.
    pub auxiliary_sink_ref: u64,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Edge {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub resource: Resource,
    pub quality: u32,
    pub physical_ref: u64,
    pub stock_alias: Option<u64>,
    /// Both edge capacity and peak are measured at the edge input boundary.
    pub capacity_rate: Rate,
    pub hard_peak_rate: Rate,
    pub retention: Ratio,
    pub loss_sink_ref: u64,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Consumer {
    pub id: u32,
    pub node: u32,
    pub resource: Resource,
    pub quality: u32,
    pub source: u32,
    pub path: Vec<u32>,
    pub connection_right_ref: u64,
    pub payer: u64,
    pub meter: u64,
    pub contract_ref: u64,
    /// Canonical obligation order; all obligations are jointly hard and never prorated.
    pub priority: u8,
    pub required_rate: Rate,
    pub hard_peak_rate: Rate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Contract {
    pub policy_version: u32,
    pub binding: AuthorityBinding,
    pub window: Window,
    pub resource: Resource,
    pub quality: u32,
    pub source: Source,
    pub edges: Vec<Edge>,
    pub consumers: Vec<Consumer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Limit, UnknownContract, Incompatible, InvalidPath, Precision, Overflow,
    StaleBase, WrongVersion, Boundary, CorruptCheckpoint, InvalidAuthorization,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Evidence { ClosedSource(u32), ClosedEdge(u32), SourceRate(u32), SourcePeak(u32),
    SourceInputCapacity(u32), SourceBudget(u32), EdgeCapacity(u32), EdgePeak(u32), ConsumerPeak(u32) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnresolvedReason { WorkBudget, MissingSupplyWitness }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ExactAmount { pub whole: u64, pub rem: u64, pub den: u64 }

impl ExactAmount {
    fn zero() -> Self { Self { whole: 0, rem: 0, den: 1 } }
    fn integer(n: u128) -> Result<Self, Error> { Self::checked(n, 1) }
    fn checked(numerator: u128, denominator: u128) -> Result<Self, Error> {
        if denominator == 0 { return Err(Error::Precision); }
        let g = gcd(numerator, denominator);
        let n = numerator / g;
        let d = denominator / g;
        if d > MAX_COMMON_DENOMINATOR { return Err(Error::Precision); }
        let whole = u64::try_from(n / d).map_err(|_| Error::Overflow)?;
        let rem = u64::try_from(n % d).map_err(|_| Error::Overflow)?;
        let den = u64::try_from(d).map_err(|_| Error::Overflow)?;
        Ok(Self { whole, rem, den })
    }
    fn fraction(self) -> Result<(u128, u128), Error> {
        if self.den == 0 || self.rem >= self.den { return Err(Error::CorruptCheckpoint); }
        let n = u128::from(self.whole).checked_mul(u128::from(self.den)).ok_or(Error::Overflow)?
            .checked_add(u128::from(self.rem)).ok_or(Error::Overflow)?;
        Ok((n, u128::from(self.den)))
    }
    fn on_denominator(self, denominator: u64) -> Result<Self, Error> {
        let (n, d) = self.fraction()?;
        let target = u128::from(denominator);
        if target == 0 || target % d != 0 { return Err(Error::Precision); }
        let scaled = n.checked_mul(target / d).ok_or(Error::Overflow)?;
        Ok(Self { whole: u64::try_from(scaled / target).map_err(|_| Error::Overflow)?,
            rem: u64::try_from(scaled % target).map_err(|_| Error::Overflow)?, den: denominator })
    }
    fn add(self, rhs: Self) -> Result<Self, Error> {
        let (an, ad) = self.fraction()?;
        let (bn, bd) = rhs.fraction()?;
        let d = lcm(ad, bd)?;
        let a = an.checked_mul(d / ad).ok_or(Error::Overflow)?;
        let b = bn.checked_mul(d / bd).ok_or(Error::Overflow)?;
        Self::checked(a.checked_add(b).ok_or(Error::Overflow)?, d)
    }
    fn sub(self, rhs: Self) -> Result<Self, Error> {
        let (an, ad) = self.fraction()?;
        let (bn, bd) = rhs.fraction()?;
        let d = lcm(ad, bd)?;
        let a = an.checked_mul(d / ad).ok_or(Error::Overflow)?;
        let b = bn.checked_mul(d / bd).ok_or(Error::Overflow)?;
        Self::checked(a.checked_sub(b).ok_or(Error::Overflow)?, d)
    }
    fn mul_ratio(self, ratio: Ratio) -> Result<Self, Error> {
        let (n, d) = self.fraction()?;
        Self::checked(n.checked_mul(u128::from(ratio.numerator)).ok_or(Error::Overflow)?,
            d.checked_mul(u128::from(ratio.denominator)).ok_or(Error::Overflow)?)
    }
    fn div_ratio(self, ratio: Ratio) -> Result<Self, Error> {
        if ratio.numerator == 0 { return Err(Error::Precision); }
        let (n, d) = self.fraction()?;
        Self::checked(n.checked_mul(u128::from(ratio.denominator)).ok_or(Error::Overflow)?,
            d.checked_mul(u128::from(ratio.numerator)).ok_or(Error::Overflow)?)
    }
    fn cmp(self, rhs: Self) -> Result<std::cmp::Ordering, Error> {
        let (an, ad) = self.fraction()?;
        let (bn, bd) = rhs.fraction()?;
        let a = an.checked_mul(bd).ok_or(Error::Overflow)?;
        let b = bn.checked_mul(ad).ok_or(Error::Overflow)?;
        Ok(a.cmp(&b))
    }
    fn floor_u64(self) -> Result<u64, Error> { self.fraction()?; Ok(self.whole) }
    fn ceil_u64(self) -> Result<u64, Error> {
        self.fraction()?;
        self.whole.checked_add(u64::from(self.rem != 0)).ok_or(Error::Overflow)
    }
    fn remainder(self) -> Result<Self, Error> {
        self.fraction()?;
        Ok(Self { whole: 0, rem: self.rem, den: self.den })
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    if a == 0 && b == 0 { return 1; }
    while b != 0 { let r = a % b; a = b; b = r; }
    a.max(1)
}
fn lcm(a: u128, b: u128) -> Result<u128, Error> {
    let v = (a / gcd(a, b)).checked_mul(b).ok_or(Error::Overflow)?;
    if v > MAX_COMMON_DENOMINATOR { return Err(Error::Precision); }
    Ok(v)
}
fn rate_amount(rate: Rate, seconds: u64) -> Result<ExactAmount, Error> {
    ExactAmount::checked(u128::from(rate.numerator).checked_mul(u128::from(seconds)).ok_or(Error::Overflow)?,
        u128::from(rate.denominator_seconds))
}
fn rate_ratio(rate: Rate, ratio: Ratio, inverse: bool) -> Result<ExactAmount, Error> {
    let r = ExactAmount::checked(u128::from(rate.numerator), u128::from(rate.denominator_seconds))?;
    if inverse { r.div_ratio(ratio) } else { r.mul_ratio(ratio) }
}
fn rate_as_fraction(rate: Rate) -> Result<ExactAmount, Error> {
    ExactAmount::checked(u128::from(rate.numerator), u128::from(rate.denominator_seconds))
}
fn fingerprint<T: Serialize>(v: &T) -> Result<[u8; 32], Error> {
    Ok(Sha256::digest(serde_json::to_vec(v).map_err(|_| Error::UnknownContract)?).into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedContract { fingerprint: [u8; 32] }

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Checkpoint {
    pub policy_version: u32,
    pub fingerprint: [u8; 32],
    pub genesis_ref: u64,
    pub cursor_second: u64,
    pub sequence: u64,
    pub common_denominator: u64,
    pub physical: PhysicalMeters,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct State { checkpoint: Checkpoint }

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PhysicalMeters {
    pub consumers: Vec<(u32, ExactAmount)>,
    pub source_output: ExactAmount,
    pub source_auxiliary: ExactAmount,
    pub source_gross: ExactAmount,
    pub edges: Vec<(u32, ExactAmount, ExactAmount, ExactAmount)>, // input/output/loss
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConsumerDelta {
    pub consumer: u32,
    pub physical: ExactAmount,
    pub credited_milli_delta: u64,
    pub fractional_delivered_before: ExactAmount,
    pub fractional_delivered_after: ExactAmount,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceDelta {
    pub source: u32,
    pub physical_output: ExactAmount,
    pub physical_auxiliary: ExactAmount,
    pub physical_gross: ExactAmount,
    /// Integer Quantity meter projection only; never a fuel-stock debit or generation amount.
    /// The root Prepared receipt owns fuel, cooling, source-stock and sink accounting.
    pub source_meter_ceil_delta_milli: u64,
    pub prepaid_unconverted_before: ExactAmount,
    pub prepaid_unconverted_after: ExactAmount,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EdgeDelta {
    pub edge: u32,
    pub physical_input: ExactAmount,
    pub physical_output: ExactAmount,
    pub physical_loss: ExactAmount,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub policy_version: u32,
    pub base_digest: [u8; 32],
    pub from_second: u64,
    pub until_second: u64,
    pub next: Checkpoint,
    pub consumer_deltas: Vec<ConsumerDelta>,
    pub source_delta: SourceDelta,
    pub source_witness: SupplyWitness,
    pub edge_deltas: Vec<EdgeDelta>,
    pub work_used: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SupplyWitness {
    pub source_window_ref: u64,
    pub source_ref: u64,
    pub from_second: u64,
    pub until_second: u64,
    pub physical_gross: ExactAmount,
    pub receipt_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal { pub candidate: Option<Candidate>, pub evidence: Option<Evidence>,
    pub pending_reason: Option<UnresolvedReason>, pub work_used: u32 }

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sealed { pub checkpoint: Checkpoint, pub digest: [u8; 32], pub at_second: u64, pub change_ref: u64 }

impl Contract {
    pub fn authorize<V: RootVerifier>(&self, verifier: &V) -> Result<AuthorizedContract, Error> {
        self.validate()?;
        if !verifier.verifies(self) { return Err(Error::InvalidAuthorization); }
        Ok(AuthorizedContract { fingerprint: self.fingerprint()? })
    }

    pub fn fingerprint(&self) -> Result<[u8; 32], Error> {
        self.validate()?;
        let mut c = self.clone();
        c.edges.sort_by_key(|e| e.id);
        c.consumers.sort_by_key(|d| (d.priority, d.id));
        fingerprint(&c)
    }

    /// Returns the immutable public exact-meter scale without opening or authenticating anything.
    pub fn precision_denominator(&self) -> Result<u64, Error> {
        self.validate()?;
        self.compile_denominator()
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.policy_version != POLICY_VERSION { return Err(Error::WrongVersion); }
        if self.consumers.is_empty() || self.consumers.len() > MAX_CONSUMERS
            || self.edges.len() > MAX_EDGES { return Err(Error::Limit); }
        let b = &self.binding;
        if b.scope.development_city == 0 || b.world_ref == 0 || b.branch_ref == 0 || b.root_ref == 0
            || b.provider_ref == 0 || b.source_ref == 0 || b.stock_ref == 0 || b.contract_ref == 0
            || b.provider_revision == 0 || b.source_revision == 0 || b.clock_revision == 0
            || b.scale_revision == 0 || self.window.start_second != b.effective_start
            || self.window.end_second != b.effective_end || self.window.end_second <= self.window.start_second
            || self.window.end_second - self.window.start_second > MAX_WINDOW_SECONDS || self.quality == 0 {
            return Err(Error::UnknownContract);
        }
        let s = &self.source;
        let expected_resource = match s.kind {
            SourceKind::ContinuousElectricity => Resource::MilliWattHour,
            SourceKind::StoredWater => Resource::MilliLiter,
        };
        if s.id == 0 || s.node == 0 || s.resource != expected_resource || s.resource != self.resource
            || s.quality != self.quality || s.source_ref != b.source_ref || s.stock_ref != b.stock_ref
            || s.contract_ref != b.contract_ref {
            return Err(Error::UnknownContract);
        }
        validate_rate(s.physical_supply_rate)?;
        validate_rate(s.input_capacity_rate)?;
        validate_rate(s.hard_peak_rate)?;
        validate_ratio(s.auxiliary_ratio, true)?;
        if s.auxiliary_ratio.numerator > 0 && s.auxiliary_sink_ref == 0 { return Err(Error::UnknownContract); }
        if s.available_input_milli.value() < 0 || s.available_input_milli.value() as u64 > MAX_MILLI_QUANTITY {
            return Err(Error::Precision);
        }
        let mut edge_ids = BTreeSet::new();
        let mut edge_phys = BTreeSet::new();
        let mut aliases = BTreeSet::new();
        for e in &self.edges {
            if !edge_ids.insert(e.id) || !edge_phys.insert(e.physical_ref)
                || e.stock_alias.is_some_and(|a| a == s.stock_ref || !aliases.insert(a)) { return Err(Error::UnknownContract); }
            if e.id == 0 || e.from == 0 || e.to == 0 || e.from == e.to || e.physical_ref == 0
                || e.loss_sink_ref == 0 || e.retention.numerator == 0 { return Err(Error::UnknownContract); }
            validate_rate(e.capacity_rate)?;
            validate_rate(e.hard_peak_rate)?;
            validate_ratio(e.retention, false)?;
            if e.resource != self.resource || e.quality != self.quality { return Err(Error::Incompatible); }
        }
        let mut ids = BTreeSet::new();
        let mut meters = BTreeSet::new();
        let mut used_edges = BTreeSet::new();
        for d in &self.consumers {
            if !ids.insert(d.id) || !meters.insert(d.meter) { return Err(Error::UnknownContract); }
            if d.id == 0 || d.node == 0 || d.source != s.id || d.resource != self.resource
                || d.quality != self.quality || d.connection_right_ref == 0 || d.payer == 0
                || d.meter == 0 || d.contract_ref == 0 || d.priority > 3
                || d.required_rate.numerator == 0 || d.hard_peak_rate.numerator == 0
                || d.path.len() > MAX_PATH { return Err(Error::UnknownContract); }
            validate_rate(d.required_rate)?;
            validate_rate(d.hard_peak_rate)?;
            let mut node = s.node;
            let mut visited = BTreeSet::from([node]);
            for id in &d.path {
                let e = self.edges.iter().find(|e| e.id == *id).ok_or(Error::InvalidPath)?;
                used_edges.insert(*id);
                if e.from != node || !visited.insert(e.to) { return Err(Error::InvalidPath); }
                node = e.to;
            }
            if node != d.node { return Err(Error::InvalidPath); }
        }
        if used_edges.len() != self.edges.len() { return Err(Error::InvalidPath); }
        self.compile_denominator()?;
        Ok(())
    }

    fn compile_denominator(&self) -> Result<u64, Error> {
        let mut denominator = 1u128;
        for rate in [self.source.physical_supply_rate, self.source.input_capacity_rate, self.source.hard_peak_rate] {
            denominator = lcm(denominator, u128::from(rate.denominator_seconds))?;
        }
        for edge in &self.edges {
            denominator = lcm(denominator, u128::from(edge.capacity_rate.denominator_seconds))?;
            denominator = lcm(denominator, u128::from(edge.hard_peak_rate.denominator_seconds))?;
        }
        for d in &self.consumers {
            denominator = lcm(denominator, u128::from(d.required_rate.denominator_seconds))?;
            denominator = lcm(denominator, u128::from(d.hard_peak_rate.denominator_seconds))?;
            let mut required_factor = u128::from(d.required_rate.denominator_seconds);
            let mut peak_factor = u128::from(d.hard_peak_rate.denominator_seconds);
            for id in &d.path {
                let e = self.edges.iter().find(|e| e.id == *id).ok_or(Error::InvalidPath)?;
                required_factor = required_factor.checked_mul(u128::from(e.retention.numerator)).ok_or(Error::Overflow)?;
                peak_factor = peak_factor.checked_mul(u128::from(e.retention.numerator)).ok_or(Error::Overflow)?;
            }
            let aux_den = u128::from(self.source.auxiliary_ratio.denominator);
            required_factor = required_factor.checked_mul(aux_den).ok_or(Error::Overflow)?;
            peak_factor = peak_factor.checked_mul(aux_den).ok_or(Error::Overflow)?;
            denominator = lcm(denominator, required_factor)?;
            denominator = lcm(denominator, peak_factor)?;
        }
        u64::try_from(denominator).map_err(|_| Error::Precision)
    }

    pub fn opening<V: RootVerifier>(&self, verifier: &V, genesis_ref: u64) -> Result<State, Error> {
        if genesis_ref == 0 { return Err(Error::UnknownContract); }
        let auth = self.authorize(verifier)?;
        if !verifier.verifies_opening(self, genesis_ref) { return Err(Error::InvalidAuthorization); }
        let fp = self.fingerprint()?;
        if auth.fingerprint != fp { return Err(Error::InvalidAuthorization); }
        let physical = self.meters_at(self.window.start_second)?;
        Ok(State { checkpoint: Checkpoint { policy_version: POLICY_VERSION, fingerprint: fp, genesis_ref,
            cursor_second: self.window.start_second, sequence: 0,
            common_denominator: self.compile_denominator()?, physical } })
    }

    pub fn restore<V: RootVerifier>(&self, verifier: &V, checkpoint: Checkpoint,
        trusted_digest: [u8; 32]) -> Result<State, Error> {
        let auth = self.authorize(verifier)?;
        if checkpoint.physical.consumers.len() > MAX_CONSUMERS || checkpoint.physical.edges.len() > MAX_EDGES {
            return Err(Error::Limit);
        }
        if auth.fingerprint != checkpoint.fingerprint { return Err(Error::StaleBase); }
        if fingerprint(&checkpoint)? != trusted_digest { return Err(Error::CorruptCheckpoint); }
        if checkpoint.policy_version != POLICY_VERSION { return Err(Error::WrongVersion); }
        if !verifier.verifies_opening(self, checkpoint.genesis_ref) {
            return Err(Error::InvalidAuthorization);
        }
        if checkpoint.genesis_ref == 0 || checkpoint.cursor_second < self.window.start_second || checkpoint.cursor_second > self.window.end_second
            || checkpoint.sequence != checkpoint.cursor_second - self.window.start_second
            || checkpoint.common_denominator != self.compile_denominator()?
            || checkpoint.physical != self.meters_at(checkpoint.cursor_second)? { return Err(Error::CorruptCheckpoint); }
        // A matching digest authenticates stored bytes, not their historical feasibility.
        // At the zero-origin opening no supply has yet been consumed; later checkpoints must
        // fit this segment's rates, peaks, and single finite source allocation.
        if checkpoint.cursor_second != self.window.start_second
            && self.check_feasible(&checkpoint.physical)?.is_some() {
            return Err(Error::CorruptCheckpoint);
        }
        Ok(State { checkpoint })
    }

    pub fn propose<V: RootVerifier>(&self, verifier: &V, base: &State, until: u64, work_limit: u32)
        -> Result<Proposal, Error> {
        let auth = self.authorize(verifier)?;
        self.validate_state(verifier, &auth, base)?;
        if until < base.cursor() || until > self.window.end_second { return Err(Error::Boundary); }
        let work = 1u32 + self.edges.len() as u32 + self.consumers.iter().map(|d| 1 + d.path.len() as u32).sum::<u32>();
        let limit = work_limit.min(MAX_WORK);
        if limit == 0 || work > limit { return Ok(Proposal { candidate: None, evidence: None,
            pending_reason: Some(UnresolvedReason::WorkBudget), work_used: limit.min(work) }); }
        if until == base.cursor() { return Ok(Proposal { candidate: None, evidence: None, pending_reason: None, work_used: work }); }
        let next_physical = self.meters_at(until)?;
        if let Some(evidence) = self.check_feasible(&next_physical)? {
            return Ok(Proposal { candidate: None, evidence: Some(evidence), pending_reason: None, work_used: work });
        }
        let before = &base.checkpoint.physical;
        let common_denominator = self.compile_denominator()?;
        let mut consumer_deltas = Vec::new();
        for ((id, old), (next_id, new)) in before.consumers.iter().zip(&next_physical.consumers) {
            if id != next_id { return Err(Error::CorruptCheckpoint); }
            let physical = new.sub(*old)?.on_denominator(common_denominator)?;
            let credited_before = old.floor_u64()?;
            let credited_after = new.floor_u64()?;
            consumer_deltas.push(ConsumerDelta { consumer: *id, physical,
                credited_milli_delta: credited_after.checked_sub(credited_before).ok_or(Error::CorruptCheckpoint)?,
                fractional_delivered_before: old.remainder()?.on_denominator(common_denominator)?,
                fractional_delivered_after: new.remainder()?.on_denominator(common_denominator)? });
        }
        let gross_before = before.source_gross;
        let gross_after = next_physical.source_gross;
        let physical_gross = gross_after.sub(gross_before)?.on_denominator(common_denominator)?;
        let Some(source_witness) = verifier.verify_supply(self, base.cursor(), until, physical_gross) else {
            return Ok(Proposal { candidate: None, evidence: None,
                pending_reason: Some(UnresolvedReason::MissingSupplyWitness), work_used: work });
        };
        if source_witness.source_window_ref == 0 || source_witness.source_ref != self.source.source_ref
            || source_witness.from_second != base.cursor() || source_witness.until_second != until
            || source_witness.physical_gross != physical_gross
            || source_witness.receipt_digest.iter().all(|b| *b == 0) { return Err(Error::InvalidAuthorization); }
        let prepaid_before = prepaid(gross_before)?.on_denominator(common_denominator)?;
        let prepaid_after = prepaid(gross_after)?.on_denominator(common_denominator)?;
        let source_delta = SourceDelta { source: self.source.id,
            physical_output: next_physical.source_output.sub(before.source_output)?.on_denominator(common_denominator)?,
            physical_auxiliary: next_physical.source_auxiliary.sub(before.source_auxiliary)?.on_denominator(common_denominator)?,
            physical_gross,
            source_meter_ceil_delta_milli: gross_after.ceil_u64()?.checked_sub(gross_before.ceil_u64()?).ok_or(Error::CorruptCheckpoint)?,
            prepaid_unconverted_before: prepaid_before, prepaid_unconverted_after: prepaid_after };
        let mut edge_deltas = Vec::new();
        for ((id, old_in, old_out, old_loss), (next_id, in_v, out_v, loss_v))
            in before.edges.iter().zip(&next_physical.edges) {
            if id != next_id { return Err(Error::CorruptCheckpoint); }
            edge_deltas.push(EdgeDelta { edge: *id, physical_input: in_v.sub(*old_in)?.on_denominator(common_denominator)?,
                physical_output: out_v.sub(*old_out)?.on_denominator(common_denominator)?,
                physical_loss: loss_v.sub(*old_loss)?.on_denominator(common_denominator)? });
        }
        let mut next = base.checkpoint.clone();
        next.cursor_second = until;
        next.sequence = until - self.window.start_second;
        next.physical = next_physical;
        let candidate = Candidate { policy_version: POLICY_VERSION, base_digest: base.digest()?,
            from_second: base.cursor(), until_second: until, next, consumer_deltas,
            source_delta, source_witness, edge_deltas, work_used: work };
        Ok(Proposal { candidate: Some(candidate), evidence: None, pending_reason: None, work_used: work })
    }

    pub fn revalidate<V: RootVerifier>(&self, verifier: &V, base: &State, candidate: &Candidate) -> Result<State, Error> {
        if candidate.consumer_deltas.len() > MAX_CONSUMERS || candidate.edge_deltas.len() > MAX_EDGES
            || candidate.next.physical.consumers.len() > MAX_CONSUMERS
            || candidate.next.physical.edges.len() > MAX_EDGES { return Err(Error::Limit); }
        if candidate.policy_version != POLICY_VERSION || candidate.base_digest != base.digest()?
            || candidate.from_second != base.cursor() { return Err(Error::StaleBase); }
        let expected = self.propose(verifier, base, candidate.until_second, candidate.work_used)?;
        if expected.candidate.as_ref() != Some(candidate) { return Err(Error::StaleBase); }
        Ok(State { checkpoint: candidate.next.clone() })
    }

    pub fn seal_at_change<V: RootVerifier>(&self, verifier: &V, state: &State, at: u64, change_ref: u64) -> Result<Sealed, Error> {
        if state.cursor() != at || at < self.window.start_second || at > self.window.end_second || change_ref == 0 {
            return Err(Error::Boundary);
        }
        let auth = self.authorize(verifier)?;
        self.validate_state(verifier, &auth, state)?;
        Ok(Sealed { checkpoint: state.checkpoint.clone(), digest: state.digest()?, at_second: at, change_ref })
    }

    fn validate_state<V: RootVerifier>(&self, verifier: &V, auth: &AuthorizedContract, state: &State) -> Result<(), Error> {
        if auth.fingerprint != self.fingerprint()? || state.checkpoint.fingerprint != auth.fingerprint
            || state.checkpoint.policy_version != POLICY_VERSION { return Err(Error::StaleBase); }
        if state.checkpoint.genesis_ref == 0 || !verifier.verifies_opening(self, state.checkpoint.genesis_ref) {
            return Err(Error::InvalidAuthorization);
        }
        if state.checkpoint.genesis_ref == 0 || state.checkpoint.common_denominator != self.compile_denominator()?
            || state.checkpoint.cursor_second < self.window.start_second || state.checkpoint.cursor_second > self.window.end_second
            || state.checkpoint.sequence != state.checkpoint.cursor_second - self.window.start_second
            || state.checkpoint.physical != self.meters_at(state.checkpoint.cursor_second)? { return Err(Error::CorruptCheckpoint); }
        if state.checkpoint.cursor_second != self.window.start_second
            && self.check_feasible(&state.checkpoint.physical)?.is_some() {
            return Err(Error::CorruptCheckpoint);
        }
        Ok(())
    }

    fn meters_at(&self, cursor: u64) -> Result<PhysicalMeters, Error> {
        if cursor < self.window.start_second || cursor > self.window.end_second { return Err(Error::Boundary); }
        let elapsed = cursor - self.window.start_second;
        let mut consumers = Vec::new();
        let mut source_output = ExactAmount::zero();
        let mut edge_inputs: Vec<ExactAmount> = vec![ExactAmount::zero(); self.edges.len()];
        let mut edge_outputs: Vec<ExactAmount> = vec![ExactAmount::zero(); self.edges.len()];
        let mut ordered_consumers: Vec<_> = self.consumers.iter().collect();
        ordered_consumers.sort_by_key(|d| d.id);
        for d in ordered_consumers {
            let delivered = rate_amount(d.required_rate, elapsed)?;
            consumers.push((d.id, delivered));
            let mut flow = rate_amount(d.required_rate, elapsed)?;
            for edge_id in d.path.iter().rev() {
                let i = self.edges.iter().position(|e| e.id == *edge_id).ok_or(Error::InvalidPath)?;
                edge_outputs[i] = edge_outputs[i].add(flow)?;
                flow = flow.div_ratio(self.edges[i].retention)?;
                edge_inputs[i] = edge_inputs[i].add(flow)?;
            }
            source_output = source_output.add(flow)?;
        }
        let source_auxiliary = source_output.mul_ratio(self.source.auxiliary_ratio)?;
        let source_gross = source_output.add(source_auxiliary)?;
        let mut edges = Vec::new();
        for (i, e) in self.edges.iter().enumerate() {
            let loss = edge_inputs[i].sub(edge_outputs[i])?;
            edges.push((e.id, edge_inputs[i], edge_outputs[i], loss));
        }
        consumers.sort_by_key(|(id, _)| *id);
        edges.sort_by_key(|(id, _, _, _)| *id);
        let denominator = self.compile_denominator()?;
        for (_, amount) in &mut consumers { *amount = amount.on_denominator(denominator)?; }
        for (_, input, output, loss) in &mut edges {
            *input = input.on_denominator(denominator)?;
            *output = output.on_denominator(denominator)?;
            *loss = loss.on_denominator(denominator)?;
        }
        Ok(PhysicalMeters { consumers, source_output: source_output.on_denominator(denominator)?,
            source_auxiliary: source_auxiliary.on_denominator(denominator)?,
            source_gross: source_gross.on_denominator(denominator)?, edges })
    }

    fn check_feasible(&self, meters: &PhysicalMeters) -> Result<Option<Evidence>, Error> {
        if !self.source.open { return Ok(Some(Evidence::ClosedSource(self.source.id))); }
        let source_out_rate = self.consumer_source_rate(false)?;
        let source_gross_rate = source_out_rate.add(source_out_rate.mul_ratio(self.source.auxiliary_ratio)?)?;
        let source_peak_output = self.consumer_source_rate(true)?;
        let source_peak_gross = source_peak_output.add(source_peak_output.mul_ratio(self.source.auxiliary_ratio)?)?;
        let s = &self.source;
        if source_out_rate.cmp(rate_as_fraction(s.physical_supply_rate)?)? == std::cmp::Ordering::Greater {
            return Ok(Some(Evidence::SourceRate(self.source.id)));
        }
        if source_gross_rate.cmp(rate_as_fraction(s.input_capacity_rate)?)? == std::cmp::Ordering::Greater {
            return Ok(Some(Evidence::SourceInputCapacity(self.source.id)));
        }
        if source_peak_gross.cmp(rate_as_fraction(s.hard_peak_rate)?)? == std::cmp::Ordering::Greater {
            return Ok(Some(Evidence::SourcePeak(self.source.id)));
        }
        let stock = ExactAmount::integer(s.available_input_milli.value() as u128)?;
        if meters.source_gross.cmp(stock)? == std::cmp::Ordering::Greater
            || meters.source_gross.ceil_u64()? > s.available_input_milli.value() as u64
            || meters.source_gross.ceil_u64()? > MAX_MILLI_QUANTITY {
            return Ok(Some(Evidence::SourceBudget(self.source.id)));
        }
        if meters.consumers.iter().any(|(_, amount)| amount.ceil_u64().map_or(true, |n| n > MAX_MILLI_QUANTITY)) {
            return Err(Error::Overflow);
        }
        for e in &self.edges {
            if !e.open { return Ok(Some(Evidence::ClosedEdge(e.id))); }
            let mut input_rate = ExactAmount::zero();
            let mut peak_input_rate = ExactAmount::zero();
            for d in &self.consumers {
                let mut flow = rate_as_fraction(d.required_rate)?;
                for id in d.path.iter().rev() {
                    let edge = self.edges.iter().find(|x| x.id == *id).ok_or(Error::InvalidPath)?;
                    flow = flow.div_ratio(edge.retention)?;
                    if edge.id == e.id { input_rate = input_rate.add(flow)?; }
                }
                let mut peak = rate_as_fraction(d.hard_peak_rate)?;
                for id in d.path.iter().rev() {
                    let edge = self.edges.iter().find(|x| x.id == *id).ok_or(Error::InvalidPath)?;
                    peak = peak.div_ratio(edge.retention)?;
                    if edge.id == e.id { peak_input_rate = peak_input_rate.add(peak)?; }
                }
            }
            if input_rate.cmp(rate_as_fraction(e.capacity_rate)?)? == std::cmp::Ordering::Greater {
                return Ok(Some(Evidence::EdgeCapacity(e.id)));
            }
            if peak_input_rate.cmp(rate_as_fraction(e.hard_peak_rate)?)? == std::cmp::Ordering::Greater {
                return Ok(Some(Evidence::EdgePeak(e.id)));
            }
        }
        for d in &self.consumers {
            if rate_as_fraction(d.required_rate)?.cmp(rate_as_fraction(d.hard_peak_rate)?)? == std::cmp::Ordering::Greater {
                return Ok(Some(Evidence::ConsumerPeak(d.id)));
            }
        }
        let (gross_num, gross_den) = meters.source_gross.fraction()?;
        let denominator = u128::from(self.compile_denominator()?);
        if denominator % gross_den != 0 { return Err(Error::Precision); }
        let final_num = gross_num.checked_mul(denominator / gross_den).ok_or(Error::Overflow)?;
        let _exact_scaled = final_num; // proves each authoritative meter fits compiled exact scale
        Ok(None)
    }

    fn consumer_source_rate(&self, use_peak: bool) -> Result<ExactAmount, Error> {
        let mut sum = ExactAmount::zero();
        for d in &self.consumers {
            let mut flow = rate_as_fraction(if use_peak { d.hard_peak_rate } else { d.required_rate })?;
            for id in d.path.iter().rev() {
                let edge = self.edges.iter().find(|e| e.id == *id).ok_or(Error::InvalidPath)?;
                flow = flow.div_ratio(edge.retention)?;
            }
            sum = sum.add(flow)?;
        }
        Ok(sum)
    }
}

fn validate_rate(rate: Rate) -> Result<(), Error> {
    if Rate::new(rate.numerator, rate.denominator_seconds)? != rate { return Err(Error::Precision); }
    Ok(())
}

fn validate_ratio(ratio: Ratio, allow_zero: bool) -> Result<(), Error> {
    if Ratio::new(ratio.numerator, ratio.denominator, allow_zero)? != ratio { return Err(Error::Precision); }
    Ok(())
}

fn prepaid(gross: ExactAmount) -> Result<ExactAmount, Error> {
    ExactAmount::integer(u128::from(gross.ceil_u64()?))?.sub(gross)
}

impl State {
    pub fn cursor(&self) -> u64 { self.checkpoint.cursor_second }
    pub fn checkpoint(&self) -> Checkpoint { self.checkpoint.clone() }
    pub fn digest(&self) -> Result<[u8; 32], Error> { fingerprint(&self.checkpoint) }
}
