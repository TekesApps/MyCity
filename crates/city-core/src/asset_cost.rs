//! Asset cost projections. No cash, rights, ledger, clock or Store writes.
//! Every Source below is an adapter assertion, NOT evidence authentication. The
//! existing writer must resolve original receipts, enforce exclusive cost use and
//! atomically publish cursor + cost deltas. This module never certifies net profit.
use crate::units::{ArithmeticError, GameTime, Money, Quantity};
use serde::{Deserialize, Serialize};

pub const REVISION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Unknown, Invalid, Stale, Duplicate, Overflow, Protected, Unsupported, Limit }
impl From<ArithmeticError> for Error {
    fn from(_: ArithmeticError) -> Self { Self::Overflow }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source { pub id: u64, pub revision: u64 }
impl Source {
    fn validate(self) -> Result<(), Error> {
        if self.id == 0 || self.revision == 0 { Err(Error::Unknown) } else { Ok(()) }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockKind { LocalDevelopment, InteractionObligation }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method { StraightLine, Unknown }
/// Only fixed-second, sourced straight-line profiles are supported. These are
/// game contract inputs, not tax/accounting policy defaults or invented lifetimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub revision: u32,
    pub source: Source,
    pub method: Method,
    pub clock: ClockKind,
    pub life_seconds: u64,
    /// Sourced homogeneous cost-allocation quantum, in milli-native quantity.
    /// Each quantum has identical integer-cent basis/residual. No inferred split
    /// of heterogeneous components. Use the whole scope if it is indivisible.
    pub cost_quantum: Quantity,
    /// Residual for the WHOLE original delivery, in cents.
    pub residual: Money,
}
impl Profile {
    fn validate(self, basis: Money) -> Result<(), Error> {
        self.source.validate()?;
        if self.revision != REVISION { return Err(Error::Stale); }
        if self.method != Method::StraightLine || self.life_seconds == 0 {
            return Err(Error::Unknown);
        }
        if self.residual > basis { return Err(Error::Invalid); }
        Ok(())
    }
}
/// Ordinary rental only. Actual established obligation, NOT a quote or payment.
/// Finite immutable interval; price changes/renewals require an original-writer
/// boundary adapter. Rent always uses the existing interaction obligation clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub source: Source,
    pub operator: u32,
    pub start: GameTime,
    pub end: GameTime,
    pub total: Money,
}
impl Lease {
    fn validate(self, owner: u32) -> Result<(), Error> {
        self.source.validate()?;
        if self.operator == 0 || self.operator == owner || self.end <= self.start {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn cumulative(self, at: GameTime) -> Result<Money, Error> {
        let elapsed = at.value().clamp(self.start.value(), self.end.value()) - self.start.value();
        fraction(self.total, elapsed as u64, (self.end.value() - self.start.value()) as u64)
    }
}
/// Projection of an ACTUAL commissioned delivery and original capital source.
/// Existing assets must restore their full original snapshot, never call new_delivery
/// to replace an old cursor. Quotes/#31 K estimates are deliberately not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActualDelivery {
    pub lot: u64,
    pub asset: u64,
    pub owner: u32,
    pub capital: Source,
    pub delivery: Source,
    pub source_snapshot: u64,
    pub quantity: Quantity,
    pub basis: Money,
    pub at: GameTime,
    pub profile: Profile,
    pub lease: Option<Lease>,
}
/// Derived cost cursor to embed in the SAME owning Store root, not a second ledger.
/// Whole-origin basis/quantity/profile/time stay immutable across partial exits
/// and related transfers. Fields are private; deserialized snapshots need validate
/// AND comparison to the owning authenticated root before use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    revision: u64,
    source_snapshot: u64,
    lot: u64,
    asset: u64,
    owner: u32,
    capital: Source,
    delivery: Source,
    origin_quantity: Quantity,
    origin_basis: Money,
    offset: Quantity,
    quantity: Quantity,
    commissioned: GameTime,
    at: GameTime,
    age_seconds: u64,
    profile: Profile,
    lease: Option<Lease>,
    last_receipt: Option<Source>,
    /// Remains attached even after later shareholder changes.
    related_origin: Option<Source>,
}
impl Snapshot {
    pub fn new_delivery(d: ActualDelivery) -> Result<Self, Error> {
        let s = Self { revision: 0, source_snapshot: d.source_snapshot, lot: d.lot,
            asset: d.asset, owner: d.owner, capital: d.capital, delivery: d.delivery,
            origin_quantity: d.quantity, origin_basis: d.basis, offset: Quantity::ZERO,
            quantity: d.quantity, commissioned: d.at, at: d.at, age_seconds: 0,
            profile: d.profile, lease: d.lease, last_receipt: None, related_origin: None };
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.capital.validate()?;
        self.delivery.validate()?;
        self.profile.validate(self.origin_basis)?;
        if let Some(l) = self.lease {
            l.validate(self.owner)?;
            if l.start < self.commissioned || l.source.id == self.capital.id {
                return Err(Error::Invalid);
            }
        }
        if let Some(r) = self.last_receipt { r.validate()?; }
        if let Some(r) = self.related_origin { r.validate()?; }
        if self.source_snapshot == 0 || self.lot == 0 || self.asset == 0 || self.owner == 0 {
            return Err(Error::Unknown);
        }
        if self.origin_quantity == Quantity::ZERO || self.at < self.commissioned
            || self.offset.checked_add(self.quantity)? > self.origin_quantity {
            return Err(Error::Invalid);
        }
        let quantum = self.profile.cost_quantum.value();
        if quantum == 0 || self.origin_quantity.value() % quantum != 0
            || self.offset.value() % quantum != 0 || self.quantity.value() % quantum != 0 {
            return Err(Error::Unsupported);
        }
        let count = self.origin_quantity.value() / quantum;
        if self.origin_basis.value() % count != 0 || self.profile.residual.value() % count != 0 {
            return Err(Error::Unsupported);
        }
        let elapsed = (self.at.value() - self.commissioned.value()) as u64;
        if self.age_seconds > elapsed
            || self.profile.clock == ClockKind::InteractionObligation && self.age_seconds != elapsed {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn source_snapshot(&self) -> u64 { self.source_snapshot }
    pub fn lot(&self) -> u64 { self.lot }
    pub fn owner(&self) -> u32 { self.owner }
    pub fn age_seconds(&self) -> u64 { self.age_seconds }
    pub fn quantity(&self) -> Quantity { self.quantity }
    pub fn origin_basis(&self) -> Money { self.origin_basis }
    pub fn related_origin(&self) -> Option<Source> { self.related_origin }
    pub fn carrying(&self) -> Result<Money, Error> {
        self.validate()?;
        carrying_slice(self, self.offset, self.quantity, self.age_seconds)
    }
    /// Per-quantum time numerator remainder, not spendable money. The original
    /// slice coordinates carry quantity rounding; never reset them after a sale.
    pub fn time_remainder(&self) -> Result<u64, Error> {
        self.validate()?;
        let count = self.origin_quantity.value() / self.profile.cost_quantum.value();
        let depreciable = (self.origin_basis.checked_sub(self.profile.residual)?.value() / count) as u128;
        Ok((depreciable * u128::from(self.age_seconds.min(self.profile.life_seconds))
            % u128::from(self.profile.life_seconds)) as u64)
    }
}

/// Fixed-shape local candidate wire only, not a replacement Store receipt decoder.
/// Bounded bytes before parsing; serde's recursion bound remains enabled, fields
/// contain no collections/strings. Original-writer root authentication is separate.
pub fn parse_snapshot(bytes: &[u8]) -> Result<Snapshot, Error> {
    if bytes.len() > MAX_SNAPSHOT_BYTES { return Err(Error::Limit); }
    let s: Snapshot = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    s.validate()?;
    Ok(s)
}

fn money(n: i128) -> Result<Money, Error> {
    Ok(Money::new(i64::try_from(n).map_err(|_| Error::Overflow)?)?)
}
fn fraction(total: Money, numerator: u64, denominator: u64) -> Result<Money, Error> {
    if denominator == 0 || numerator > denominator { return Err(Error::Invalid); }
    money(i128::from(total.value()) * i128::from(numerator) / i128::from(denominator))
}
/// Reuse the original immutable lot curve; no additional inventory or money authority.
fn slice(total: Money, origin: Quantity, offset: Quantity, count: Quantity) -> Result<Money, Error> {
    crate::units::frozen_lot_basis_delta(origin,total,offset,count)
        .map(|(_,cost)|cost).map_err(|e|match e {
            crate::units::LotBasisError::Origin=>Error::Invalid,
            crate::units::LotBasisError::Overflow=>Error::Overflow,
        })
}

fn depreciation(s: &Snapshot, age: u64) -> Result<Money, Error> {
    let count = s.origin_quantity.value() / s.profile.cost_quantum.value();
    let per_quantum = Money::new(s.origin_basis.checked_sub(s.profile.residual)?.value() / count)?;
    Ok(fraction(per_quantum, age.min(s.profile.life_seconds), s.profile.life_seconds)?
        .checked_mul(count as u64)?)
}
fn carrying_slice(s: &Snapshot, offset: Quantity, quantity: Quantity, age: u64) -> Result<Money, Error> {
    Ok(slice(s.origin_basis, s.origin_quantity, offset, quantity)?
        .checked_sub(slice(depreciation(s, age)?, s.origin_quantity, offset, quantity)?)?)
}

/// Original-writer optimistic guard; source_snapshot is fixed for this lineage.
/// Source refresh/migration is unsupported, never silently reinitializes age/basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expected { pub revision: u64, pub source_snapshot: u64, pub receipt: Source }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpenseUse {
    /// Already incurred ordinary repair; does not improve physical condition here.
    Repair,
    /// Already incorporated in another cost pool; cannot expense again.
    AlreadyCapitalized,
    /// Price estimate or payment without classified incurred cost is insufficient.
    QuoteOrPayment,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActualExpense {
    pub source: Source,
    pub maintenance_delivery: Source,
    pub amount: Money,
    /// Contract-resolved liable party, not inferred from the payment account.
    pub liable: u32,
    pub use_kind: ExpenseUse,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKind { Sale, Recovery, Retirement }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection { Clear, ProtectedOrLiquidating, Unknown }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Existing clock delivers elapsed exposure, never wall-clock sampling.
    Accrue { to: GameTime, development_seconds: u64 },
    Expense(ActualExpense),
    /// No proceeds or profit inferred; money/rights and public sale protection
    /// remain in the existing writer. Replacement uses Retirement atomically.
    Exit { quantity: Quantity, kind: ExitKind, protection: Protection },
    RelatedTransfer { quantity: Quantity, destination_lot: u64, buyer: u32,
        protection: Protection },
    /// Actual replacement delivery + new cost source, NOT a repair payment.
    /// Only a whole homogeneous remaining scope can be replaced in this version.
    Replace { actual: ActualDelivery, maintenance_delivery: Source, protection: Protection },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostKind { Depreciation, Rent, Repair }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CostDelta { pub party: u32, pub kind: CostKind, pub amount: Money, pub source: Source }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Derecognition {
    pub quantity: Quantity,
    pub remaining_cost: Money,
    pub kind: ExitKind,
}
/// Fixed-size candidate: at most owner depreciation + operator rent, one exit,
/// one transferred/replacement lot. Never a cash transfer, profit certificate or
/// instruction to mutate maintenance condition. All deltas are incremental.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    base: Snapshot,
    next: Snapshot,
    pub receipt: Source,
    pub costs: [Option<CostDelta>; 2],
    pub derecognition: Option<Derecognition>,
    pub additional_lot: Option<Snapshot>,
    /// Distinguishes true new replacement capital from transferred old basis.
    pub new_capital: Option<Money>,
    /// Original writer must exclusively consume this cost source, also across
    /// rent/expense/capitalization paths and across lots/operations.
    pub exclusive_cost_source: Option<Source>,
}
impl Candidate {
    pub fn against(&self, current: &Snapshot) -> Result<&Snapshot, Error> {
        current.validate()?;
        if current != &self.base { return Err(Error::Stale); }
        Ok(&self.next)
    }
    /// Deliberately never opens the existing fiscal/distribution completeness gate.
    pub fn establishes_distributable_profit(&self) -> bool { false }
}

pub fn plan(base: &Snapshot, expected: Expected, action: Action) -> Result<Candidate, Error> {
    base.validate()?;
    expected.receipt.validate()?;
    if expected.revision != base.revision || expected.source_snapshot != base.source_snapshot {
        return Err(Error::Stale);
    }
    if base.last_receipt.is_some_and(|r| r.id == expected.receipt.id) { return Err(Error::Duplicate); }
    if base.quantity == Quantity::ZERO { return Err(Error::Invalid); }
    let mut c = Candidate { base: base.clone(), next: base.clone(), receipt: expected.receipt,
        costs: [None, None], derecognition: None, additional_lot: None, new_capital: None,
        exclusive_cost_source: None };
    match action {
        Action::Accrue { to, development_seconds } => {
            if to <= base.at { return Err(Error::Invalid); }
            let interaction = (to.value() - base.at.value()) as u64;
            if development_seconds > interaction { return Err(Error::Invalid); }
            let elapsed = match base.profile.clock {
                ClockKind::LocalDevelopment => development_seconds,
                ClockKind::InteractionObligation => interaction,
            };
            c.next.age_seconds = base.age_seconds.checked_add(elapsed).ok_or(Error::Overflow)?;
            c.next.at = to;
            let amount = slice(depreciation(base, c.next.age_seconds)?, base.origin_quantity,
                base.offset, base.quantity)?.checked_sub(slice(depreciation(base, base.age_seconds)?,
                    base.origin_quantity, base.offset, base.quantity)?)?;
            c.costs[0] = Some(CostDelta { party: base.owner, kind: CostKind::Depreciation,
                amount, source: base.capital });
            if let Some(l) = base.lease {
                // Lease belongs to a whole scope. Transfers/exits under this lease
                // require a sourced boundary adapter, rather than guessed proration.
                c.costs[1] = Some(CostDelta { party: l.operator, kind: CostKind::Rent,
                    amount: l.cumulative(to)?.checked_sub(l.cumulative(base.at)?)?, source: l.source });
            }
        }
        Action::Expense(e) => {
            e.source.validate()?;
            e.maintenance_delivery.validate()?;
            match e.use_kind {
                ExpenseUse::Unknown | ExpenseUse::QuoteOrPayment => return Err(Error::Unknown),
                ExpenseUse::AlreadyCapitalized => return Err(Error::Duplicate),
                ExpenseUse::Repair => (),
            }
            if e.source.id == base.capital.id || base.lease.is_some_and(|l| l.source.id == e.source.id) {
                return Err(Error::Duplicate);
            }
            if e.liable != base.owner && !base.lease.is_some_and(|l| l.operator == e.liable) {
                return Err(Error::Invalid);
            }
            c.costs[0] = Some(CostDelta { party: e.liable, kind: CostKind::Repair,
                amount: e.amount, source: e.source });
            c.exclusive_cost_source = Some(e.source);
        }
        Action::Exit { quantity, kind, protection } => {
            // Clear is an asserted original-writer protection resolution, not proof.
            require_clear(protection)?;
            remove_prefix(&mut c, quantity, kind)?;
        }
        Action::RelatedTransfer { quantity, destination_lot, buyer, protection } => {
            require_clear(protection)?;
            if buyer == 0 || buyer == base.owner || destination_lot == 0 || destination_lot == base.lot {
                return Err(Error::Invalid);
            }
            remove_prefix(&mut c, quantity, ExitKind::Sale)?;
            let mut transferred = base.clone();
            transferred.quantity = quantity;
            transferred.owner = buyer;
            transferred.lot = destination_lot;
            transferred.related_origin = base.related_origin.or(Some(expected.receipt));
            transferred.last_receipt = Some(expected.receipt);
            c.additional_lot = Some(transferred);
        }
        Action::Replace { actual, maintenance_delivery, protection } => {
            require_clear(protection)?;
            maintenance_delivery.validate()?;
            if actual.asset != base.asset || actual.owner != base.owner || actual.at != base.at
                || actual.quantity != base.quantity || actual.lot == base.lot
                || actual.capital.id == base.capital.id || actual.delivery.id == base.delivery.id
                || actual.source_snapshot != base.source_snapshot || actual.lease.is_some() {
                return Err(Error::Invalid);
            }
            // A whole remaining component is retired at its remaining old cost;
            // only actual new delivery initializes a new component life/basis.
            remove_prefix(&mut c, base.quantity, ExitKind::Retirement)?;
            c.additional_lot = Some(Snapshot::new_delivery(actual)?);
            c.new_capital = Some(actual.basis);
            c.exclusive_cost_source = Some(actual.capital);
        }
    }
    c.next.revision = base.revision.checked_add(1).ok_or(Error::Overflow)?;
    c.next.last_receipt = Some(expected.receipt);
    if matches!(action, Action::RelatedTransfer { .. }) {
        if let Some(s) = c.additional_lot.as_mut() { s.revision = c.next.revision; }
    }
    c.next.validate()?;
    if let Some(s) = c.additional_lot.as_ref() { s.validate()?; }
    Ok(c)
}
fn require_clear(protection: Protection) -> Result<(), Error> {
    match protection {
        Protection::Clear => Ok(()),
        Protection::Unknown => Err(Error::Unknown),
        Protection::ProtectedOrLiquidating => Err(Error::Protected),
    }
}
fn remove_prefix(c: &mut Candidate, quantity: Quantity, kind: ExitKind) -> Result<(), Error> {
    if c.base.lease.is_some() { return Err(Error::Unsupported); }
    if quantity == Quantity::ZERO || quantity > c.base.quantity { return Err(Error::Invalid); }
    if quantity.value() % c.base.profile.cost_quantum.value() != 0 {
        return Err(Error::Unsupported);
    }
    c.derecognition = Some(Derecognition { quantity, kind,
        remaining_cost: carrying_slice(&c.base, c.base.offset, quantity, c.base.age_seconds)? });
    c.next.offset = c.base.offset.checked_add(quantity)?;
    c.next.quantity = c.base.quantity.checked_sub(quantity)?;
    Ok(())
}
