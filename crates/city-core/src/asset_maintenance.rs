//! Bounded maintenance candidates only. No inventory, money, scheduler or Store writes.
//! Callers must resolve provider authority and actual delivery in the existing writer.
use crate::units::{ArithmeticError, Carry, GameTime, Quantity};
use serde::{Deserialize, Serialize};

pub const REVISION: u32 = 1;
pub const MAX_COMPONENTS: usize = 8;
pub const MAX_PAGE: usize = 16;
const FULL: u16 = 10_000; // condition/damage basis points, not percent

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Unknown, Invalid, Overflow, Stale, Limit }
impl From<ArithmeticError> for Error {
    fn from(_: ArithmeticError) -> Self { Self::Overflow }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source { pub id: u32, pub revision: u32 }
impl Source {
    fn check(self) -> Result<(), Error> {
        if self.id == 0 || self.revision == 0 { Err(Error::Unknown) } else { Ok(()) }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind { Bridge, OrdinaryEquipment }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockKind { LocalDevelopment, InteractionObligation }

/// Projected #31 references. A nonzero reference is NOT certification or permission.
/// Derived daily_cents must be converted by the adapter to cents/second Carry.
/// Inherited maintenance must be resolved; do not fall back to a generic rate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub revision: u32,
    pub kind: Kind,
    pub facility: Source,
    pub maintenance: Source,
    pub clock: ClockKind,
    pub inspection_period_seconds: u64,
    pub service_period_seconds: u64,
    pub inspection_worker_seconds: u64,
    pub service_worker_seconds: u64,
    /// Milli-native units of exactly this material recipe; never cents of budget.
    pub material: Source,
    pub service_material: Quantity,
    pub service_improvement_bps: u16,
    pub replacement_worker_seconds: u64,
    pub replacement_material: Quantity,
}
impl Profile {
    fn check(&self) -> Result<(), Error> {
        self.facility.check()?;
        self.maintenance.check()?;
        self.material.check()?;
        if self.revision != REVISION || self.inspection_period_seconds == 0
            || self.service_period_seconds == 0 || self.inspection_worker_seconds == 0
            || self.service_worker_seconds == 0 || self.service_material == Quantity::ZERO
            || self.replacement_worker_seconds == 0 || self.replacement_material == Quantity::ZERO
            || self.service_improvement_bps > FULL {
            return Err(Error::Unknown);
        }
        if self.kind == Kind::OrdinaryEquipment && self.service_period_seconds != 30 * 86_400 {
            return Err(Error::Unknown);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Component {
    id: u64,
    age_seconds: u64,
    operating_seconds: u64,
    damage_bps: u16,
    wear_bps: u16,
    #[serde(deserialize_with = "restore_carry")]
    calendar_wear: Carry,
    #[serde(deserialize_with = "restore_carry")]
    operating_wear: Carry,
    generation: u64,
    service_done: u64,
    inspection_done: u64,
    service_work: u64,
    service_material: Quantity,
    inspection_work: u64,
    credited_bps: u16,
}
/// Persist this complete snapshot through the owning Store adapter. JSON decoding
/// is not exposed here; bound bytes/depth before serde and call validate afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    asset: u64,
    base_revision: u64,
    source_snapshot: u64,
    last_receipt: u64,
    origin: GameTime,
    at: GameTime,
    age_seconds: u64,
    obligation_elapsed: u64,
    #[serde(deserialize_with = "restore_carry")]
    annual_fee: Carry,
    components: Vec<Component>,
    profile: Profile,
}
fn add(a: u64, b: u64) -> Result<u64, Error> { a.checked_add(b).ok_or(Error::Overflow) }
fn bound_points(value: u64) -> u16 { value.min(u64::from(FULL)) as u16 }
// units::Carry deliberately has no Deserialize; use its checked restore API.
fn restore_carry<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Carry, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire { numerator: u64, denominator: u64, remainder: u64 }
    let w = Wire::deserialize(d)?;
    Carry::restore(w.numerator, w.denominator, w.remainder)
        .map_err(|_| serde::de::Error::custom("invalid_maintenance_carry"))
}
impl Snapshot {
    /// Initial authoritative condition/age comes from provider, not this constructor.
    /// Use restore for existing assets; constructor supports newly commissioned assets.
    pub fn commissioned(asset: u64, source_snapshot: u64, at: GameTime, profile: Profile,
        parts: &[(u64, Carry, Carry)], fee_cents_per_second: Carry) -> Result<Self, Error> {
        profile.check()?;
        if asset == 0 || source_snapshot == 0 || parts.is_empty() || parts.len() > MAX_COMPONENTS {
            return Err(Error::Limit);
        }
        let components = parts.iter().map(|&(id, calendar_wear, operating_wear)| Component {
            id, calendar_wear, operating_wear, age_seconds: 0, operating_seconds: 0,
            damage_bps: 0, wear_bps: 0, generation: 0, service_done: 0, inspection_done: 0,
            service_work: 0, service_material: Quantity::ZERO, inspection_work: 0, credited_bps: 0,
        }).collect();
        let result = Self { asset, source_snapshot, origin: at, at, profile, annual_fee: fee_cents_per_second,
            base_revision: 0, last_receipt: 0, age_seconds: 0, obligation_elapsed: 0, components };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.profile.check()?;
        if self.asset == 0 || self.source_snapshot == 0 || self.components.is_empty()
            || self.components.len() > MAX_COMPONENTS { return Err(Error::Limit); }
        let since_origin = u64::try_from(self.at.checked_sub(self.origin)?.value()).map_err(|_| Error::Invalid)?;
        if self.age_seconds != self.obligation_elapsed || self.obligation_elapsed > since_origin
            || self.profile.clock == ClockKind::InteractionObligation && self.obligation_elapsed != since_origin {
            return Err(Error::Invalid);
        }
        for (i, c) in self.components.iter().enumerate() {
            let earned = (u128::from(c.service_work) * u128::from(self.profile.service_improvement_bps)
                / u128::from(self.profile.service_worker_seconds))
                .min((c.service_material.value() as u128) * u128::from(self.profile.service_improvement_bps)
                    / (self.profile.service_material.value() as u128)) as u16;
            if c.id == 0 || self.components[..i].iter().any(|p| p.id == c.id)
                || c.damage_bps > FULL || c.wear_bps > FULL || c.age_seconds > self.age_seconds
                || c.operating_seconds > c.age_seconds || c.credited_bps > self.profile.service_improvement_bps
                || c.credited_bps != earned
                || c.service_work > self.profile.service_worker_seconds
                || c.service_material > self.profile.service_material
                || c.service_material >= self.profile.service_material
                    && c.service_work >= self.profile.service_worker_seconds
                || c.inspection_work >= self.profile.inspection_worker_seconds
                || c.service_done > self.obligation_elapsed / self.profile.service_period_seconds
                || c.inspection_done > self.obligation_elapsed / self.profile.inspection_period_seconds {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    pub fn age_seconds(&self) -> u64 { self.age_seconds }
    pub fn clock_origin(&self) -> GameTime { self.origin }
    /// Read-only identity fields for the existing maintenance authority adapter.
    pub(crate) fn asset_id(&self) -> u64 { self.asset }
    pub(crate) fn source_snapshot(&self) -> u64 { self.source_snapshot }
    pub(crate) fn at(&self) -> GameTime { self.at }
    pub(crate) fn last_receipt(&self) -> u64 { self.last_receipt }
    pub(crate) fn profile(&self) -> &Profile { &self.profile }
    pub(crate) fn component_generation(&self, id: u64) -> Option<u64> {
        self.components.iter().find(|c| c.id == id).map(|c| c.generation)
    }
    pub fn component_age(&self, id: u64) -> Option<u64> {
        self.components.iter().find(|c| c.id == id).map(|c| c.age_seconds)
    }
    pub fn condition_bps(&self, id: u64) -> Option<u16> {
        self.components.iter().find(|c| c.id == id)
            .map(|c| FULL.saturating_sub(c.damage_bps.saturating_add(c.wear_bps)))
    }
    /// Stateless due scan. A full/zero-capacity queue changes no completion or cursor.
    /// Pass the previous page's opaque cursor to resume the same snapshot.
    pub fn due_page(&self, cursor: Option<DueCursor>, capacity: usize) -> Result<DuePage, Error> {
        self.validate()?;
        let cursor = cursor.unwrap_or(DueCursor { asset: self.asset,
            revision: self.base_revision, source_snapshot: self.source_snapshot, position: 0 });
        if cursor.asset != self.asset || cursor.revision != self.base_revision
            || cursor.source_snapshot != self.source_snapshot { return Err(Error::Stale); }
        if capacity > MAX_PAGE || cursor.position > self.components.len() * 2 { return Err(Error::Limit); }
        let mut page = DuePage { orders: Vec::new(), cursor, complete: false };
        while page.cursor.position < self.components.len() * 2 {
            let c = &self.components[page.cursor.position / 2];
            let inspection = page.cursor.position % 2 == 0;
            let (period, done, kind) = if inspection {
                (self.profile.inspection_period_seconds, c.inspection_done, DueKind::Inspection)
            } else { (self.profile.service_period_seconds, c.service_done, DueKind::MinimumService) };
            let total = self.obligation_elapsed / period;
            if total > done {
                if page.orders.len() == capacity { return Ok(page); }
                page.orders.push(Due { asset: self.asset, component: c.id, kind,
                    first_ordinal: add(done, 1)?, count: total - done,
                    first_due_elapsed: add(done, 1)?.checked_mul(period).ok_or(Error::Overflow)?,
                    clock: self.profile.clock, base_revision: self.base_revision });
            }
            page.cursor.position += 1;
        }
        page.complete = true;
        Ok(page)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DueKind { Inspection, MinimumService }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DueCursor { asset: u64, revision: u64, source_snapshot: u64, position: usize }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Due {
    pub asset: u64, pub component: u64, pub kind: DueKind,
    pub first_ordinal: u64, pub count: u64, pub first_due_elapsed: u64,
    pub clock: ClockKind, pub base_revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuePage { pub orders: Vec<Due>, pub cursor: DueCursor, pub complete: bool }

/// Provider-verified actual interval, not predicted work or payment. All part rows
/// are mandatory and ordered exactly as the snapshot, so missing coverage is refused.
pub struct Exposure<'a> {
    pub to: GameTime,
    pub development_seconds: u64,
    pub obligation_seconds: u64,
    pub parts: &'a [(u64, u64, u16)], // component, actual operating seconds, actual damage bps
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Inspect, Service, Replace }
/// The original writer must verify receipt uniqueness, labor qualification, material
/// compatibility, physical delivery, authority and atomic consumption. IDs alone do not.
pub struct Delivery {
    pub receipt_serial: u64,
    pub source_snapshot: u64,
    pub component: u64,
    pub generation: u64,
    pub action: Action,
    pub due_ordinal: u64,
    pub worker_seconds: u64,
    pub material: Source,
    pub quantity: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate { base: Snapshot, next: Snapshot, fee_cents: u64 }
impl Candidate {
    pub fn fee_cents(&self) -> u64 { self.fee_cents }
    /// Read-only proposed snapshot. This does not commit anything. Owning writer must
    /// compare full current base and publish snapshot + receipt + physical changes atomically.
    pub fn against(&self, current: &Snapshot) -> Result<&Snapshot, Error> {
        if current != &self.base { Err(Error::Stale) } else { Ok(&self.next) }
    }
}
pub fn advance(base: &Snapshot, exposure: Exposure<'_>) -> Result<Candidate, Error> {
    base.validate()?;
    let elapsed = u64::try_from(exposure.to.checked_sub(base.at)?.value()).map_err(|_| Error::Invalid)?;
    if exposure.development_seconds > elapsed || exposure.obligation_seconds > elapsed
        || exposure.parts.len() != base.components.len() { return Err(Error::Invalid); }
    let required = match base.profile.clock {
        ClockKind::LocalDevelopment => exposure.development_seconds,
        ClockKind::InteractionObligation => elapsed,
    };
    if exposure.obligation_seconds != required { return Err(Error::Invalid); }
    let mut next = base.clone();
    next.at = exposure.to;
    next.age_seconds = add(next.age_seconds, required)?;
    next.obligation_elapsed = add(next.obligation_elapsed, exposure.obligation_seconds)?;
    for (c, &(id, operating, damage)) in next.components.iter_mut().zip(exposure.parts) {
        if c.id != id || operating > required || damage > FULL { return Err(Error::Invalid); }
        c.age_seconds = add(c.age_seconds, required)?;
        c.operating_seconds = add(c.operating_seconds, operating)?;
        let wear = add(c.calendar_wear.accrue(required)?, c.operating_wear.accrue(operating)?)?;
        c.wear_bps = bound_points(add(u64::from(c.wear_bps), wear)?);
        c.damage_bps = bound_points(u64::from(c.damage_bps) + u64::from(damage));
    }
    let fee_cents = next.annual_fee.accrue(exposure.obligation_seconds)?;
    next.base_revision = add(next.base_revision, 1)?;
    Ok(Candidate { base: base.clone(), next, fee_cents })
}

pub fn deliver(base: &Snapshot, d: &Delivery) -> Result<Candidate, Error> {
    base.validate()?;
    if d.receipt_serial != add(base.last_receipt, 1)? || d.source_snapshot != base.source_snapshot {
        return Err(Error::Stale);
    }
    if d.material != base.profile.material { return Err(Error::Unknown); }
    let mut next = base.clone();
    let p = &next.profile;
    let c = next.components.iter_mut().find(|c| c.id == d.component).ok_or(Error::Unknown)?;
    if d.generation != c.generation { return Err(Error::Stale); }
    match d.action {
        Action::Inspect => {
            if d.due_ordinal != add(c.inspection_done, 1)?
                || d.due_ordinal > base.obligation_elapsed / p.inspection_period_seconds
                || d.quantity != Quantity::ZERO { return Err(Error::Invalid); }
            c.inspection_work = add(c.inspection_work, d.worker_seconds)?;
            if c.inspection_work > p.inspection_worker_seconds { return Err(Error::Invalid); }
            if c.inspection_work == p.inspection_worker_seconds {
                c.inspection_done += 1;
                c.inspection_work = 0;
            }
        }
        Action::Service => {
            if d.due_ordinal != add(c.service_done, 1)?
                || d.due_ordinal > base.obligation_elapsed / p.service_period_seconds { return Err(Error::Invalid); }
            c.service_work = add(c.service_work, d.worker_seconds)?;
            c.service_material = c.service_material.checked_add(d.quantity)?;
            if c.service_work > p.service_worker_seconds || c.service_material > p.service_material {
                return Err(Error::Invalid);
            }
            let labor = u128::from(c.service_work) * u128::from(p.service_improvement_bps)
                / u128::from(p.service_worker_seconds);
            let material = (c.service_material.value() as u128) * u128::from(p.service_improvement_bps)
                / (p.service_material.value() as u128);
            let earned = labor.min(material) as u16;
            let improvement = earned.checked_sub(c.credited_bps).ok_or(Error::Invalid)?;
            // Credit consumed even on undamaged parts; cannot bank future repair.
            let repaired = improvement.min(c.damage_bps);
            c.damage_bps -= repaired;
            c.wear_bps = c.wear_bps.saturating_sub(improvement - repaired);
            c.credited_bps = earned;
            if c.service_work == p.service_worker_seconds && c.service_material == p.service_material {
                c.service_done += 1;
                c.service_work = 0;
                c.service_material = Quantity::ZERO;
                c.credited_bps = 0;
            }
        }
        Action::Replace => {
            if d.due_ordinal != 0 || d.worker_seconds != p.replacement_worker_seconds
                || d.quantity != p.replacement_material || c.service_work != 0
                || c.service_material != Quantity::ZERO || c.inspection_work != 0 { return Err(Error::Invalid); }
            c.generation = add(c.generation, 1)?;
            c.age_seconds = 0;
            c.operating_seconds = 0;
            c.damage_bps = 0;
            c.wear_bps = 0;
            // Retain sub-bps asset-stream carries conservatively. Replacement does
            // not erase already accrued fractional maintenance responsibility.
            // Established inspection/service obligations remain outstanding.
        }
    }
    next.last_receipt = d.receipt_serial;
    next.base_revision = add(next.base_revision, 1)?;
    Ok(Candidate { base: base.clone(), next, fee_cents: 0 })
}
