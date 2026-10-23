//! Persistent dynamic source protocol for implemented laboratory domains.
//! The sources are business state, not an acknowledged queue of synthetic counters.
use crate::{
    calendar::Date,
    schedule::Phase,
    transaction::TxError,
    units::{GameTime, Money},
};
use serde::{Deserialize, Serialize};
pub const CLAIM_LIMIT: usize = 20000;
/// Bounded provenance only; the actual liability remains in ordinary Claim rows.
pub const MAX_CLAIM_SOURCES: usize = 256;
pub const PAYROLL_LIMIT: usize = 64;
pub const PAGE_LIMIT: u16 = 8192;
pub const HEAP_LIMIT: usize = 2 * 1024 * 1024;
pub const DAY: u64 = 86400;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Cycle {
    Daily,
    Weekly,
    Monthly { anchor: u8 },
}
// Direct fixed fields avoid internally-tagged Content buffering in diagnostic
// payroll restores. Serialization and accepted valid calendar rules stay unchanged.
impl<'de> Deserialize<'de> for Cycle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Kind {
            Daily,
            Weekly,
            Monthly,
        }
        #[derive(Default)]
        enum Anchor {
            #[default]
            Absent,
            Value(u8),
        }
        impl<'de> Deserialize<'de> for Anchor {
            fn deserialize<S: serde::Deserializer<'de>>(d: S) -> Result<Self, S::Error> {
                u8::deserialize(d).map(Self::Value)
            }
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            kind: Kind,
            #[serde(default)]
            anchor: Anchor,
        }
        let raw = Raw::deserialize(d)?;
        match (raw.kind, raw.anchor) {
            (Kind::Daily, Anchor::Absent) => Ok(Self::Daily),
            (Kind::Weekly, Anchor::Absent) => Ok(Self::Weekly),
            (Kind::Monthly, Anchor::Value(anchor)) => Ok(Self::Monthly { anchor }),
            (Kind::Monthly, Anchor::Absent) => Err(serde::de::Error::missing_field("anchor")),
            _ => Err(serde::de::Error::custom("anchor_only_valid_for_monthly")),
        }
    }
}
impl Cycle {
    pub fn next(self, at: u64) -> Result<u64, TxError> {
        let date = Date::from_time(
            GameTime::new(i64::try_from(at).map_err(|_| TxError::Overflow)?)
                .map_err(|_| TxError::Overflow)?,
        )
        .map_err(|_| TxError::Timeline(Error::Calendar))?;
        match self {
            Self::Daily => at.checked_add(DAY).ok_or(TxError::Overflow),
            Self::Weekly => at.checked_add(7 * DAY).ok_or(TxError::Overflow),
            Self::Monthly { anchor } => {
                if !(1..=31).contains(&anchor) {
                    return Err(TxError::Timeline(Error::Calendar));
                }
                let current = (1..=anchor)
                    .rev()
                    .find_map(|d| Date::new(date.year(), date.month(), d).ok())
                    .ok_or(TxError::Timeline(Error::Calendar))?;
                Ok(if current.midnight().value() as u64 > at {
                    current.midnight()
                } else {
                    date.next_monthly(anchor)
                        .map_err(|_| TxError::Timeline(Error::Calendar))?
                        .midnight()
                }
                .value() as u64)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayrollSpec {
    pub id: u32,
    pub payer: u32,
    pub payee: u32,
    pub home_city: u32,
    pub hourly_cents: Money,
    pub cycle: Cycle,
    pub ends_at: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payroll {
    pub spec: PayrollSpec,
    pub started: u64,
    pub last_accrual: u64,
    pub next_due: Option<u64>,
    pub working: bool,
    pub pending_cycle: Option<Cycle>,
    pub accrued: Money,
    pub billed: Money,
    pub paid: Money,
    pub remainder: u16,
    pub occurrences: u64,
    pub work_seconds: u64,
}
impl Payroll {
    pub(crate) fn new(spec: PayrollSpec, now: u64) -> Result<Self, TxError> {
        if spec.id == 0
            || spec.payer == spec.payee
            || spec.hourly_cents == Money::ZERO
            || spec.ends_at <= now
            || now % DAY != 0
        {
            return Err(TxError::Timeline(Error::Input));
        }
        let due = spec.cycle.next(now)?.min(spec.ends_at);
        Ok(Self {
            spec,
            started: now,
            last_accrual: now,
            next_due: Some(due),
            working: true,
            pending_cycle: None,
            accrued: Money::ZERO,
            billed: Money::ZERO,
            paid: Money::ZERO,
            remainder: 0,
            occurrences: 0,
            work_seconds: 0,
        })
    }
    pub(crate) fn accrue(&mut self, to: u64, active_city: Option<u32>) -> Result<(), TxError> {
        if to < self.last_accrual || to - self.last_accrual > 3600 {
            return Err(TxError::Timeline(Error::Cursor));
        }
        let end = to.min(self.spec.ends_at);
        let mut t = self.last_accrual;
        let mut seconds = 0u64;
        if self.working && (self.spec.home_city == 0 || active_city == Some(self.spec.home_city)) {
            while t < end {
                let midnight = t / DAY * DAY;
                let until = end.min(midnight + DAY);
                seconds += until
                    .min(midnight + 17 * 3600)
                    .saturating_sub(t.max(midnight + 9 * 3600));
                t = until;
            }
        }
        self.work_seconds = self
            .work_seconds
            .checked_add(seconds)
            .ok_or(TxError::Overflow)?;
        let numerator = i128::from(self.spec.hourly_cents.value()) * i128::from(seconds)
            + i128::from(self.remainder);
        let earned = Money::new(i64::try_from(numerator / 3600).map_err(|_| TxError::Overflow)?)
            .map_err(|_| TxError::Overflow)?;
        self.accrued = self
            .accrued
            .checked_add(earned)
            .map_err(|_| TxError::Overflow)?;
        self.remainder = (numerator % 3600) as u16;
        self.last_accrual = to;
        Ok(())
    }
    pub(crate) fn bill(&mut self, at: u64) -> Result<(), TxError> {
        if self.next_due != Some(at) {
            return Err(TxError::Timeline(Error::Cursor));
        }
        self.billed = self.accrued;
        self.occurrences = self.occurrences.checked_add(1).ok_or(TxError::Overflow)?;
        if at == self.spec.ends_at {
            self.next_due = None;
            self.working = false;
        } else {
            if let Some(c) = self.pending_cycle.take() {
                self.spec.cycle = c;
            }
            self.next_due = Some(self.spec.cycle.next(at)?.min(self.spec.ends_at));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: u64,
    pub payer: u32,
    pub payee: u32,
    pub due: u64,
    pub principal: Money,
    pub paid: Money,
    pub processed: bool,
    /// One-based index into Timeline.claim_sources; zero means no trusted provenance.
    /// Kept compact because this record is reserved CLAIM_LIMIT times.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub source_ref: u32,
}
// A u32 reference occupies existing alignment slack after the bool on supported 64-bit targets;
// never regress to a per-Claim heap object or large inline Option.
const _: () = assert!(std::mem::size_of::<Claim>() <= 48);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaxClaimSource {
    pub company: u32,
    pub policy_id: u32,
    pub kind: crate::town_runtime::TaxKind,
    pub period: u64,
    pub settlement_operation_id: u64,
    pub taxable_basis: Money,
    pub assessed: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoanClaimSource {
    pub lender: u32,
    pub borrower: u32,
    pub principal: Money,
}

/// Provenance for a rent Claim generated from an Engine-authorized housing occurrence.
/// Claim principal/paid remain the sole debt balance; this row only identifies the lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RentClaimSource {
    pub landlord: u32,
    pub tenant: u32,
    pub lease_id: u32,
    pub unit_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimSource {
    Tax { details: TaxClaimSource },
    Loan { details: LoanClaimSource },
    Rent { details: RentClaimSource },
}

/// Sparse provenance row keyed back to the ordinary Timeline Claim. This table is not a
/// second debt ledger: principal and paid remain authoritative on Claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimSourceRecord {
    pub claim_id: u64,
    pub source: ClaimSource,
}
const _: () = assert!(std::mem::size_of::<ClaimSourceRecord>() <= 64);
const _: () = assert!(MAX_CLAIM_SOURCES * std::mem::size_of::<ClaimSourceRecord>() <= 16 * 1024);
impl crate::model_rows::Record for ClaimSourceRecord {
    fn inactive() -> Self {
        Self {
            claim_id: 0,
            source: ClaimSource::Loan {
                details: LoanClaimSource {
                    lender: 0,
                    borrower: 0,
                    principal: Money::ZERO,
                },
            },
        }
    }
}
fn empty_claim_sources() -> crate::model_rows::Rows<ClaimSourceRecord, MAX_CLAIM_SOURCES> {
    crate::model_rows::Rows::new().expect("bounded tax source rows")
}
fn claim_sources_is_empty(
    rows: &crate::model_rows::Rows<ClaimSourceRecord, MAX_CLAIM_SOURCES>,
) -> bool {
    rows.is_empty()
}
fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub target: Option<u64>,
    pub at: u64,
    pub last_complete: u64,
    pub phase: Option<Phase>,
    pub lane: u8,
    pub cursor: u32,
    pub serial: u64,
    pub page_sources: u32,
    pub page_paid: Money,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    pub progress: Progress,
    pub claims: crate::model_rows::Rows<Claim, CLAIM_LIMIT>,
    pub payroll: crate::model_rows::Rows<Payroll, PAYROLL_LIMIT>,
    /// Sparse trusted provenance for generated tax/loan claims; omitted when empty for legacy wire.
    #[serde(
        default = "empty_claim_sources",
        skip_serializing_if = "claim_sources_is_empty"
    )]
    pub claim_sources: crate::model_rows::Rows<ClaimSourceRecord, MAX_CLAIM_SOURCES>,
}

impl crate::model_rows::Record for Claim {
    fn inactive() -> Self {
        Self {
            id: 0,
            payer: 0,
            payee: 0,
            due: 0,
            principal: Money::ZERO,
            paid: Money::ZERO,
            processed: false,
            source_ref: 0,
        }
    }
}
impl crate::model_rows::Record for Payroll {
    fn inactive() -> Self {
        Self {
            spec: PayrollSpec {
                id: 0,
                payer: 0,
                payee: 0,
                home_city: 0,
                hourly_cents: Money::ZERO,
                cycle: Cycle::Daily,
                ends_at: 0,
            },
            started: 0,
            last_accrual: 0,
            next_due: None,
            working: false,
            pending_cycle: None,
            accrued: Money::ZERO,
            billed: Money::ZERO,
            paid: Money::ZERO,
            remainder: 0,
            occurrences: 0,
            work_seconds: 0,
        }
    }
}
impl Clone for Timeline {
    fn clone(&self) -> Self {
        let mut out = Self::new(0);
        out.clone_from(self);
        out
    }
    fn clone_from(&mut self, other: &Self) {
        self.progress = other.progress;
        self.claims.copy_active_from(&other.claims);
        self.payroll.copy_active_from(&other.payroll);
        self.claim_sources.copy_active_from(&other.claim_sources);
    }
}
impl crate::model_rows::ResidentValue for Timeline {
    fn empty() -> Self {
        Self::new(0)
    }
    fn reset(&mut self) {
        self.reset_at(0);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Enable,
    CancelAdvance,
    Claims {
        first_id: u64,
        count: u32,
        payer: u32,
        payee: u32,
        amount: Money,
        due: u64,
    },
    /// Finite zero-interest principal contract: cash disbursement and debt are atomic.
    Lend {
        id: u64,
        lender: u32,
        borrower: u32,
        principal: Money,
        due: u64,
    },
    AddPayroll {
        spec: PayrollSpec,
    },
    StopWork {
        id: u32,
    },
    ChangeCycle {
        id: u32,
        cycle: Cycle,
    },
    Terminate {
        id: u32,
    },
    /// Stop future payroll accrual without settling cash or rewriting the contract end.
    /// This is a system-only Timeline primitive, not legal approval or company closure.
    EndForExit {
        id: u32,
    },
    SettleClaim {
        id: u64,
    },
    SettlePayroll {
        id: u32,
    },
    Advance {
        target: u64,
        limit: u16,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Input,
    Limit,
    Cursor,
    Busy,
    Clock,
    Calendar,
    Duplicate,
    LegacyClock,
    CausalOrder,
}
impl Timeline {
    pub(crate) fn reset_at(&mut self, now: u64) {
        self.claims.clear();
        self.payroll.clear();
        self.claim_sources.clear();
        self.progress = Progress {
            target: None,
            at: now,
            last_complete: now,
            phase: None,
            lane: 0,
            cursor: 0,
            serial: 0,
            page_sources: 0,
            page_paid: Money::ZERO,
        };
    }
    pub(crate) fn new(now: u64) -> Self {
        Self {
            progress: Progress {
                target: None,
                at: now,
                last_complete: now,
                phase: None,
                lane: 0,
                cursor: 0,
                serial: 0,
                page_sources: 0,
                page_paid: Money::ZERO,
            },
            claims: crate::model_rows::Rows::new().expect("fixed claims backing"),
            payroll: crate::model_rows::Rows::new().expect("fixed payroll backing"),
            claim_sources: empty_claim_sources(),
        }
    }
    pub fn owned_bytes(&self) -> usize {
        self.claims.capacity() * std::mem::size_of::<Claim>()
            + self.payroll.capacity() * std::mem::size_of::<Payroll>()
            + self.claim_sources.capacity() * std::mem::size_of::<ClaimSourceRecord>()
    }
    /// Resolve provenance through the bounded sparse table. Claim principal and paid remain
    /// the authoritative tax amount/outstanding balance.
    pub fn source_for_claim_id(&self, claim_id: u64) -> Option<ClaimSource> {
        let claim = self.claims.iter().find(|claim| claim.id == claim_id)?;
        self.source_for_claim(claim)
    }
    /// O(1) provenance lookup when the Timeline Claim record is already in hand.
    pub fn source_for_claim(&self, claim: &Claim) -> Option<ClaimSource> {
        let index = usize::try_from(claim.source_ref.checked_sub(1)?).ok()?;
        let record = self.claim_sources.get(index)?;
        (record.claim_id == claim.id).then_some(record.source)
    }
    pub fn tax_source_for_claim_id(&self, claim_id: u64) -> Option<TaxClaimSource> {
        match self.source_for_claim_id(claim_id)? {
            ClaimSource::Tax { details } => Some(details),
            ClaimSource::Loan { .. } | ClaimSource::Rent { .. } => None,
        }
    }
    pub fn tax_source_for_claim(&self, claim: &Claim) -> Option<TaxClaimSource> {
        match self.source_for_claim(claim)? {
            ClaimSource::Tax { details } => Some(details),
            ClaimSource::Loan { .. } | ClaimSource::Rent { .. } => None,
        }
    }
    pub fn loan_source_for_claim(&self, claim: &Claim) -> Option<LoanClaimSource> {
        match self.source_for_claim(claim)? {
            ClaimSource::Loan { details } => Some(details),
            ClaimSource::Tax { .. } | ClaimSource::Rent { .. } => None,
        }
    }
    pub fn rent_source_for_claim(&self, claim: &Claim) -> Option<RentClaimSource> {
        match self.source_for_claim(claim)? {
            ClaimSource::Rent { details } => Some(details),
            ClaimSource::Tax { .. } | ClaimSource::Loan { .. } => None,
        }
    }
    pub fn rent_source_for_claim_id(&self, claim_id: u64) -> Option<RentClaimSource> {
        let claim = self.claims.iter().find(|claim| claim.id == claim_id)?;
        self.rent_source_for_claim(claim)
    }
    /// Append source metadata atomically to the candidate Timeline. A full table refuses the
    /// settlement; it never drops provenance or the ordinary debt row.
    pub(crate) fn push_tax_claim_source(
        &mut self,
        claim_id: u64,
        source: TaxClaimSource,
    ) -> Result<u32, TxError> {
        self.push_claim_source(claim_id, ClaimSource::Tax { details: source })
    }
    pub(crate) fn push_loan_claim_source(
        &mut self,
        claim_id: u64,
        source: LoanClaimSource,
    ) -> Result<u32, TxError> {
        self.push_claim_source(claim_id, ClaimSource::Loan { details: source })
    }
    pub(crate) fn push_rent_claim_source(
        &mut self,
        claim_id: u64,
        source: RentClaimSource,
    ) -> Result<u32, TxError> {
        self.push_claim_source(claim_id, ClaimSource::Rent { details: source })
    }
    fn push_claim_source(&mut self, claim_id: u64, source: ClaimSource) -> Result<u32, TxError> {
        if claim_id == 0 || self.claim_sources.len() >= MAX_CLAIM_SOURCES {
            return Err(TxError::WorkingLimit);
        }
        let index = match self
            .claim_sources
            .binary_search_by_key(&claim_id, |row| row.claim_id)
        {
            Ok(_) => return Err(TxError::InvalidOperation),
            Err(index) => index,
        };
        let old_len = self.claim_sources.len();
        let reference = u32::try_from(index + 1).map_err(|_| TxError::Overflow)?;
        self.claim_sources
            .try_reserve_exact(1)
            .map_err(|_| TxError::WorkingLimit)?;
        self.claim_sources.resize(
            old_len + 1,
            ClaimSourceRecord {
                claim_id: 0,
                source: ClaimSource::Loan {
                    details: LoanClaimSource {
                        lender: 0,
                        borrower: 0,
                        principal: Money::ZERO,
                    },
                },
            },
        );
        for row_index in (index..old_len).rev() {
            self.claim_sources[row_index + 1] = self.claim_sources[row_index];
        }
        for claim in self.claims.iter_mut() {
            if claim.source_ref >= reference {
                claim.source_ref = claim.source_ref.checked_add(1).ok_or(TxError::Overflow)?;
            }
        }
        self.claim_sources[index] = ClaimSourceRecord { claim_id, source };
        Ok(reference)
    }

    /// Insert an Engine-created housing Claim and its source link into the ordinary Timeline.
    /// Cash remains unpaid until the regular P2 boundary; this method creates no second ledger.
    pub(crate) fn insert_rent_claim(
        &mut self,
        mut claim: Claim,
        source: RentClaimSource,
    ) -> Result<(), TxError> {
        if claim.id == 0
            || claim.payer != source.tenant
            || claim.payee != source.landlord
            || claim.id != u64::from(source.lease_id)
            || claim.principal <= Money::ZERO
            || claim.paid != Money::ZERO
            || claim.processed
            || claim.due <= self.progress.last_complete
            || self.claims.len() >= CLAIM_LIMIT
        {
            return Err(TxError::InvalidOperation);
        }
        let index = match self.claims.binary_search_by_key(&claim.id, |row| row.id) {
            Ok(_) => return Err(TxError::InvalidOperation),
            Err(index) => index,
        };
        let old_len = self.claims.len();
        self.claims
            .try_reserve_exact(1)
            .map_err(|_| TxError::WorkingLimit)?;
        self.claims.resize(
            old_len + 1,
            <Claim as crate::model_rows::Record>::inactive(),
        );
        for row_index in (index..old_len).rev() {
            self.claims[row_index + 1] = self.claims[row_index];
        }
        let reference = self.push_rent_claim_source(claim.id, source)?;
        claim.source_ref = reference;
        self.claims[index] = claim;
        Ok(())
    }
    pub(crate) fn validate(&self) -> Result<(), TxError> {
        let p = self.progress;
        if self.claims.len() > CLAIM_LIMIT
            || self.payroll.len() > PAYROLL_LIMIT
            || self.claim_sources.len() > MAX_CLAIM_SOURCES
            || self.owned_bytes() > HEAP_LIMIT
            || p.last_complete > p.at
            || p.at - p.last_complete > 3600
            || p.lane > 5
            || p.cursor as usize > CLAIM_LIMIT
            || (p.phase.is_some() && p.target.is_none())
            || p.target.is_some_and(|t| t < p.at)
        {
            return Err(TxError::Timeline(Error::Cursor));
        }
        if p.phase.is_none() && (p.at != p.last_complete || p.lane != 0 || p.cursor != 0) {
            return Err(TxError::Timeline(Error::Cursor));
        }
        let cap = match (p.phase, p.lane) {
            (Some(Phase::P0), _) => self.payroll.len(),
            (Some(Phase::P2), 0) => self.claims.len(),
            (Some(Phase::P2), 1) => self.payroll.len(),
            _ => 0,
        };
        if p.cursor as usize > cap || (p.phase != Some(Phase::P2) && p.lane != 0) {
            return Err(TxError::Timeline(Error::Cursor));
        }
        for (i, c) in self.claims.iter().enumerate() {
            let rent_occurrence_in_progress = self.rent_source_for_claim(c).is_some()
                && match p.phase {
                    Some(Phase::P1) => true,
                    Some(Phase::P2) => p.lane == 0 && i >= p.cursor as usize,
                    _ => false,
                };
            if c.id == 0
                || c.payer == c.payee
                || c.paid > c.principal
                || (!c.processed && c.paid != Money::ZERO && !rent_occurrence_in_progress)
                || (c.processed && c.due > p.at)
                || (i > 0 && self.claims[i - 1].id >= c.id)
                || (!c.processed && c.due <= p.last_complete && !rent_occurrence_in_progress)
                || (!c.processed && c.due < p.at && !rent_occurrence_in_progress)
                || (!c.processed
                    && c.due == p.at
                    && (p.phase.is_none()
                        || p.phase.is_some_and(|phase| phase > Phase::P2)
                        || (p.phase == Some(Phase::P2) && (p.lane > 0 || i < (p.cursor as usize)))))
            {
                return Err(TxError::Timeline(Error::Input));
            }
        }
        for (index, row) in self.claim_sources.iter().enumerate() {
            let reference = u32::try_from(index + 1).map_err(|_| TxError::Overflow)?;
            let claim_position = self
                .claims
                .binary_search_by_key(&row.claim_id, |claim| claim.id);
            let source_matches_claim = claim_position.is_ok_and(|position| {
                let claim = &self.claims[position];
                if claim.source_ref != reference {
                    return false;
                }
                match row.source {
                    ClaimSource::Tax { details } => {
                        claim.payer == details.company
                            && claim.principal == details.assessed
                            && details.assessed > Money::ZERO
                            && details.taxable_basis > Money::ZERO
                            && details.period > 0
                            && details.settlement_operation_id > 0
                            && details
                                .period
                                .checked_add(1)
                                .and_then(|day| day.checked_mul(DAY))
                                == Some(claim.due)
                    }
                    ClaimSource::Loan { details } => {
                        details.lender != 0
                            && details.borrower != 0
                            && details.lender != details.borrower
                            && details.principal > Money::ZERO
                            && claim.payer == details.borrower
                            && claim.payee == details.lender
                            && claim.principal == details.principal
                    }
                    ClaimSource::Rent { details } => {
                        details.landlord != 0
                            && details.tenant != 0
                            && details.landlord != details.tenant
                            && details.lease_id != 0
                            && details.unit_id != 0
                            && claim.id == u64::from(details.lease_id)
                            && claim.payer == details.tenant
                            && claim.payee == details.landlord
                    }
                }
            });
            if row.claim_id == 0
                || (index > 0 && self.claim_sources[index - 1].claim_id >= row.claim_id)
                || claim_position.is_err()
                || !source_matches_claim
            {
                return Err(TxError::Timeline(Error::Input));
            }
        }
        for claim in &self.claims {
            if claim.source_ref != 0
                && self
                    .claim_sources
                    .get((claim.source_ref - 1) as usize)
                    .is_none_or(|row| row.claim_id != claim.id)
            {
                return Err(TxError::Timeline(Error::Input));
            }
        }
        for (i, c) in self.payroll.iter().enumerate() {
            if c.spec.id == 0
                || (i > 0 && self.payroll[i - 1].spec.id >= c.spec.id)
                || c.paid > c.billed
                || c.billed > c.accrued
                || c.remainder >= 3600
                || (c.last_accrual < p.last_complete || c.last_accrual > p.at)
                || c.started > c.last_accrual
                || c.next_due
                    .is_some_and(|t| t <= p.last_complete || t < p.at || t > c.spec.ends_at)
                || c.next_due == Some(p.at)
                    && (p.phase.is_none()
                        || p.phase.is_some_and(|phase| phase > Phase::P2)
                        || (p.phase == Some(Phase::P2)
                            && (p.lane > 1 || (p.lane == 1 && i < (p.cursor as usize)))))
                || c.spec.payer == c.spec.payee
                || c.spec.hourly_cents == Money::ZERO
                || matches!(c.spec.cycle,Cycle::Monthly{anchor} if !(1..=31).contains(&anchor))
                || matches!(c.pending_cycle,Some(Cycle::Monthly{anchor}) if !(1..=31).contains(&anchor))
            {
                return Err(TxError::Timeline(Error::Input));
            }
        }
        for c in &self.payroll {
            if i128::from(c.accrued.value()) * 3600 + i128::from(c.remainder)
                != i128::from(c.spec.hourly_cents.value()) * i128::from(c.work_seconds)
            {
                return Err(TxError::Timeline(Error::Input));
            }
        }
        Ok(())
    }
}

impl crate::restore_into::Restore for Timeline {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        // A missing sparse table is the legacy wire representation. Clear before decoding so
        // restoring an older checkpoint into a reused resident cannot retain stale provenance.
        self.claim_sources.clear();
        crate::restore_into::object!(d,self,Timeline,7u64,{
        progress,0=>Copy,
        claims,1=>Into,
        payroll,2=>Into,
        claim_sources,3=>Into,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod fixed_cycle_decode_tests {
    use super::*;
    #[test]
    fn cycle_field_order_and_canonical_bytes_stay_compatible_without_content() {
        for (text, cycle) in [
            (r#"{"kind":"daily"}"#, Cycle::Daily),
            (r#"{"kind":"weekly"}"#, Cycle::Weekly),
            (
                r#"{"kind":"monthly","anchor":31}"#,
                Cycle::Monthly { anchor: 31 },
            ),
        ] {
            assert_eq!(serde_json::from_str::<Cycle>(text).unwrap(), cycle);
            assert_eq!(serde_json::to_string(&cycle).unwrap(), text);
        }
        assert_eq!(
            serde_json::from_str::<Cycle>(r#"{"anchor":31,"kind":"monthly"}"#).unwrap(),
            Cycle::Monthly { anchor: 31 }
        );
        for bad in [
            r#"{"kind":"daily","anchor":null}"#,
            r#"{"kind":"daily","anchor":1}"#,
            r#"{"kind":"monthly"}"#,
            r#"{"kind":"monthly","anchor":null}"#,
            r#"{"kind":"monthly","anchor":1,"anchor":2}"#,
            r#"{"kind":"daily","kind":"weekly"}"#,
            r#"{"extra":[[[0]]],"kind":"daily"}"#,
        ] {
            assert!(serde_json::from_str::<Cycle>(bad).is_err(), "{bad}");
        }
    }
}

#[cfg(test)]
mod sparse_tax_source_tests {
    use super::*;

    fn source() -> TaxClaimSource {
        TaxClaimSource {
            company: 7,
            policy_id: 3,
            kind: crate::town_runtime::TaxKind::DailyRealizedMarketMargin,
            period: 1,
            settlement_operation_id: 41,
            taxable_basis: Money::new(1_000).unwrap(),
            assessed: Money::new(50).unwrap(),
        }
    }

    #[test]
    fn legacy_empty_timeline_wire_stays_without_sparse_field() {
        let timeline = Timeline::new(0);
        let encoded = serde_json::to_string(&timeline).unwrap();
        assert!(!encoded.contains("claim_sources"));
        assert!(!encoded.contains("source_ref"));
        let decoded: Timeline = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, timeline);
        assert!(decoded.claim_sources.is_empty());
    }

    #[test]
    fn sparse_tax_source_roundtrips_with_ordinary_claim_as_debt_authority() {
        let mut timeline = Timeline::new(0);
        let reference = timeline.push_tax_claim_source(1, source()).unwrap();
        timeline
            .claims
            .try_push(Claim {
                id: 1,
                payer: 7,
                payee: 9,
                due: 2 * DAY,
                principal: Money::new(50).unwrap(),
                paid: Money::ZERO,
                processed: false,
                source_ref: reference,
            })
            .unwrap();
        timeline.validate().unwrap();
        assert_eq!(timeline.tax_source_for_claim_id(1), Some(source()));

        let clone = timeline.clone();
        assert_eq!(clone, timeline);
        let encoded = serde_json::to_string(&timeline).unwrap();
        let decoded: Timeline = serde_json::from_str(&encoded).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, timeline);
        assert_eq!(decoded.claims[0].principal, Money::new(50).unwrap());
        assert_eq!(decoded.claims[0].paid, Money::ZERO);
    }

    #[test]
    fn loan_source_is_distinct_from_tax_and_matches_original_claim_terms() {
        let mut timeline = Timeline::new(0);
        let loan = LoanClaimSource {
            lender: 5,
            borrower: 7,
            principal: Money::new(1_000).unwrap(),
        };
        let reference = timeline.push_loan_claim_source(1, loan).unwrap();
        timeline.progress.at = DAY;
        timeline.progress.last_complete = DAY;
        timeline
            .claims
            .try_push(Claim {
                id: 1,
                payer: loan.borrower,
                payee: loan.lender,
                due: DAY,
                principal: loan.principal,
                paid: loan.principal,
                processed: true,
                source_ref: reference,
            })
            .unwrap();
        timeline.validate().unwrap();
        assert_eq!(
            timeline.loan_source_for_claim(&timeline.claims[0]),
            Some(loan)
        );
        assert_eq!(timeline.tax_source_for_claim(&timeline.claims[0]), None);
        let encoded = serde_json::to_string(&timeline).unwrap();
        let decoded: Timeline = serde_json::from_str(&encoded).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, timeline);
    }

    #[test]
    fn full_sparse_table_backpressures_without_dropping_existing_sources() {
        let mut timeline = Timeline::new(0);
        for claim_id in 1..=MAX_CLAIM_SOURCES as u64 {
            timeline.push_tax_claim_source(claim_id, source()).unwrap();
        }
        assert_eq!(timeline.claim_sources.len(), MAX_CLAIM_SOURCES);
        assert_eq!(
            timeline.push_tax_claim_source(MAX_CLAIM_SOURCES as u64 + 1, source()),
            Err(TxError::WorkingLimit)
        );
        assert_eq!(timeline.claim_sources.len(), MAX_CLAIM_SOURCES);
    }

    #[test]
    fn rent_claim_insert_reorders_sparse_sources_and_keeps_tax_reference_exact() {
        let mut timeline = Timeline::new(0);
        let tax_source = source();
        let tax_ref = timeline.push_tax_claim_source(20, tax_source).unwrap();
        timeline
            .claims
            .try_push(Claim {
                id: 20,
                payer: tax_source.company,
                payee: 9,
                due: 2 * DAY,
                principal: tax_source.assessed,
                paid: Money::ZERO,
                processed: false,
                source_ref: tax_ref,
            })
            .unwrap();
        timeline
            .insert_rent_claim(
                Claim {
                    id: 10,
                    payer: 8,
                    payee: 11,
                    due: DAY,
                    principal: Money::new(400).unwrap(),
                    paid: Money::ZERO,
                    processed: false,
                    source_ref: 0,
                },
                RentClaimSource {
                    landlord: 11,
                    tenant: 8,
                    lease_id: 10,
                    unit_id: 2,
                },
            )
            .unwrap();
        assert_eq!(timeline.claims[0].id, 10);
        assert_eq!(timeline.claims[0].source_ref, 1);
        assert_eq!(timeline.claims[1].id, 20);
        assert_eq!(timeline.claims[1].source_ref, 2);
        assert_eq!(timeline.tax_source_for_claim_id(20), Some(tax_source));
        assert_eq!(
            timeline.rent_source_for_claim_id(10),
            Some(RentClaimSource {
                landlord: 11,
                tenant: 8,
                lease_id: 10,
                unit_id: 2,
            })
        );
        timeline.validate().unwrap();
    }
}
