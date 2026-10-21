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
        }
    }
    pub fn owned_bytes(&self) -> usize {
        self.claims.capacity() * std::mem::size_of::<Claim>()
            + self.payroll.capacity() * std::mem::size_of::<Payroll>()
    }
    pub(crate) fn validate(&self) -> Result<(), TxError> {
        let p = self.progress;
        if self.claims.len() > CLAIM_LIMIT
            || self.payroll.len() > PAYROLL_LIMIT
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
            if c.id == 0
                || c.payer == c.payee
                || c.paid > c.principal
                || (!c.processed && c.paid != Money::ZERO)
                || (c.processed && c.due > p.at)
                || (i > 0 && self.claims[i - 1].id >= c.id)
                || (!c.processed && c.due <= p.last_complete)
                || (!c.processed && c.due < p.at)
                || (!c.processed
                    && c.due == p.at
                    && (p.phase.is_none()
                        || p.phase.is_some_and(|phase| phase > Phase::P2)
                        || (p.phase == Some(Phase::P2) && (p.lane > 0 || i < (p.cursor as usize)))))
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
        crate::restore_into::object!(d,self,Timeline,7u64,{
        progress,0=>Copy,
        claims,1=>Into,
        payroll,2=>Into,
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
