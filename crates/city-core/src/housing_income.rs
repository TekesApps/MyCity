//! Bounded, derived observation of cash actually paid through Timeline payrolls.
//!
//! This is not a balance sheet or a second wage ledger. `Timeline::Payroll::paid` and the
//! account transfer remain authoritative; these day buckets only preserve the time placement
//! of actual paid deltas so housing affordability can ask about a recent complete window.

use crate::{
    housing::OpeningHousing,
    timeline::{Timeline, DAY},
    units::Money,
};
use serde::{Deserialize, Serialize};
use std::mem::size_of;

pub(crate) const PAID_WAGE_WINDOW_DAYS: u64 = 31;
pub(crate) const PAID_WAGE_RING_DAYS: usize = 32;
const MAX_WAGE_HOUSEHOLDS: usize = 32;
const _: () = assert!(size_of::<DailyPaidWages>() == 264);
const _: () = assert!(size_of::<PaidWageWindow>() == 8 + 32 * 264);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DailyPaidWages {
    /// `game_day + 1`; zero means the ring slot has never been populated.
    day_plus_one: u64,
    /// Indexed by immutable `OpeningHousing.households` order.
    amounts: [Money; MAX_WAGE_HOUSEHOLDS],
}

impl DailyPaidWages {
    fn empty() -> Self {
        Self {
            day_plus_one: 0,
            amounts: [Money::ZERO; MAX_WAGE_HOUSEHOLDS],
        }
    }

    fn is_empty(&self) -> bool {
        self.day_plus_one == 0 && self.amounts.iter().all(|amount| *amount == Money::ZERO)
    }
}

/// A fixed 32-day ring: 31 complete days plus the current partial day. It stores only actual
/// positive Payroll.paid deltas, never accruals or a duplicate cash balance.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PaidWageWindow {
    /// First full game-day boundary observed by this tracker, encoded as `day + 1`.
    /// Zero means no coverage has been established yet (e.g. a legacy checkpoint).
    coverage_start_day_plus_one: u64,
    days: [DailyPaidWages; PAID_WAGE_RING_DAYS],
}

impl PaidWageWindow {
    pub(crate) fn new() -> Self {
        Self {
            coverage_start_day_plus_one: 0,
            days: std::array::from_fn(|_| DailyPaidWages::empty()),
        }
    }

    pub(crate) fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>()
    }

    /// Start coverage on the first complete game-day at or after `at`. A legacy state with no
    /// window therefore never claims that earlier `Payroll.paid` values were observed here.
    pub(crate) fn begin_coverage(&mut self, at: u64) -> Result<(), PaidWageError> {
        if self.coverage_start_day_plus_one != 0 {
            return Ok(());
        }
        let day = at / DAY;
        let first_full_day = if at % DAY == 0 {
            day
        } else {
            day.checked_add(1).ok_or(PaidWageError::Overflow)?
        };
        self.coverage_start_day_plus_one = first_full_day
            .checked_add(1)
            .ok_or(PaidWageError::Overflow)?;
        Ok(())
    }

    /// Record one actual, positive Timeline payroll payment at its event time. A payment at
    /// exactly 00:00 belongs to that new day. Any part-day before coverage starts is ignored.
    pub(crate) fn record_paid_delta(
        &mut self,
        opening: &OpeningHousing,
        payee_account: u32,
        paid_delta: Money,
        at: u64,
    ) -> Result<(), PaidWageError> {
        if paid_delta < Money::ZERO {
            return Err(PaidWageError::Invalid);
        }
        if paid_delta == Money::ZERO {
            return Ok(());
        }
        if self.coverage_start_day_plus_one == 0 {
            return Err(PaidWageError::Uninitialized);
        }
        let household_index = opening
            .households
            .iter()
            .position(|household| household.account == payee_account)
            .ok_or(PaidWageError::UnknownHousehold)?;
        if household_index >= MAX_WAGE_HOUSEHOLDS {
            return Err(PaidWageError::Limit);
        }

        let day = at / DAY;
        let coverage_start = self.coverage_start_day_plus_one - 1;
        if day < coverage_start {
            return Ok(());
        }
        let day_plus_one = day.checked_add(1).ok_or(PaidWageError::Overflow)?;
        let slot = (day % PAID_WAGE_RING_DAYS as u64) as usize;
        if self.days[slot].day_plus_one != day_plus_one {
            self.days[slot] = DailyPaidWages {
                day_plus_one,
                amounts: [Money::ZERO; MAX_WAGE_HOUSEHOLDS],
            };
        }
        self.days[slot].amounts[household_index] = self.days[slot].amounts[household_index]
            .checked_add(paid_delta)
            .map_err(|_| PaidWageError::Overflow)?;
        Ok(())
    }

    /// Summarize exactly the 31 complete days ending at a completed Timeline boundary. The
    /// current partial day is excluded. A missing day bucket is a valid zero only when coverage
    /// proves that the day was observed.
    pub(crate) fn summarize(
        &self,
        opening: &OpeningHousing,
        timeline: &Timeline,
        household_id: u32,
    ) -> Result<PaidWageSummary, PaidWageError> {
        if timeline.progress.phase.is_some()
            || timeline.progress.target.is_some()
            || timeline.progress.at != timeline.progress.last_complete
            || timeline.progress.last_complete % DAY != 0
        {
            return Err(PaidWageError::IncompleteBoundary);
        }
        let household_index = opening
            .households
            .iter()
            .position(|household| household.household_id == household_id)
            .ok_or(PaidWageError::UnknownHousehold)?;
        if household_index >= MAX_WAGE_HOUSEHOLDS {
            return Err(PaidWageError::Limit);
        }
        let end_day = timeline.progress.last_complete / DAY;
        let Some(start_day) = end_day.checked_sub(PAID_WAGE_WINDOW_DAYS) else {
            return Err(PaidWageError::CoverageIncomplete);
        };
        let start_at = start_day.checked_mul(DAY).ok_or(PaidWageError::Overflow)?;
        let end_at = end_day.checked_mul(DAY).ok_or(PaidWageError::Overflow)?;
        if self.coverage_start_day_plus_one == 0 || start_day < self.coverage_start_day_plus_one - 1
        {
            return Err(PaidWageError::CoverageIncomplete);
        }
        let mut paid = Money::ZERO;
        for day in start_day..end_day {
            let slot = (day % PAID_WAGE_RING_DAYS as u64) as usize;
            if self.days[slot].day_plus_one == day + 1 {
                paid = paid
                    .checked_add(self.days[slot].amounts[household_index])
                    .map_err(|_| PaidWageError::Overflow)?;
            }
        }
        let account = opening.households[household_index].account;
        let qualifying_contracts = timeline
            .payroll
            .iter()
            .filter(|payroll| {
                payroll.spec.payee == account
                    && payroll.started < end_at
                    && payroll.spec.ends_at > start_at
            })
            .count();
        let active_contracts = timeline
            .payroll
            .iter()
            .filter(|payroll| {
                payroll.spec.payee == account
                    && payroll.working
                    && payroll.next_due.is_some()
                    && payroll.spec.ends_at > timeline.progress.last_complete
            })
            .count();
        let billed_arrears = timeline
            .payroll
            .iter()
            .any(|payroll| payroll.spec.payee == account && payroll.billed > payroll.paid);
        Ok(PaidWageSummary {
            household_id,
            account,
            start_day,
            end_day,
            paid_wages: paid,
            qualifying_contracts: u16::try_from(qualifying_contracts)
                .map_err(|_| PaidWageError::Limit)?,
            active_contracts: u16::try_from(active_contracts).map_err(|_| PaidWageError::Limit)?,
            billed_arrears,
        })
    }

    pub(crate) fn validate(
        &self,
        opening: &OpeningHousing,
        timeline: &Timeline,
    ) -> Result<(), PaidWageError> {
        if self.coverage_start_day_plus_one == 0 {
            if self.days.iter().any(|day| !day.is_empty()) {
                return Err(PaidWageError::Invalid);
            }
            return Ok(());
        }
        let coverage_start = self.coverage_start_day_plus_one - 1;
        let max_observed_day = timeline
            .progress
            .target
            .unwrap_or(timeline.progress.last_complete)
            / DAY;
        if coverage_start > max_observed_day.saturating_add(1) {
            return Err(PaidWageError::Invalid);
        }
        for (slot, day) in self.days.iter().enumerate() {
            if day.day_plus_one == 0 {
                if !day.is_empty() {
                    return Err(PaidWageError::Invalid);
                }
                continue;
            }
            let day_index = day.day_plus_one - 1;
            if day_index < coverage_start
                || day_index > max_observed_day
                || (day_index % PAID_WAGE_RING_DAYS as u64) as usize != slot
                || day.amounts.iter().any(|amount| *amount < Money::ZERO)
            {
                return Err(PaidWageError::Invalid);
            }
        }
        for (household_index, household) in opening.households.iter().enumerate() {
            let mut observed = Money::ZERO;
            for day in &self.days {
                if day.day_plus_one != 0 {
                    observed = observed
                        .checked_add(day.amounts[household_index])
                        .map_err(|_| PaidWageError::Overflow)?;
                }
            }
            let available = timeline
                .payroll
                .iter()
                .filter(|payroll| payroll.spec.payee == household.account)
                .try_fold(Money::ZERO, |total, payroll| {
                    total.checked_add(payroll.paid)
                })
                .map_err(|_| PaidWageError::Overflow)?;
            if observed > available {
                return Err(PaidWageError::Invalid);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaidWageSummary {
    pub household_id: u32,
    pub account: u32,
    pub start_day: u64,
    pub end_day: u64,
    /// Tax withholding and non-wage income are not modeled here; this is actual cash wages paid.
    pub paid_wages: Money,
    pub qualifying_contracts: u16,
    pub active_contracts: u16,
    /// A prior billed wage amount still unpaid; callers may conservatively wait, but must not
    /// replace the observed paid sum with accruals.
    pub billed_arrears: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaidWageError {
    Limit,
    Overflow,
    Invalid,
    Uninitialized,
    UnknownHousehold,
    IncompleteBoundary,
    CoverageIncomplete,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opening() -> OpeningHousing {
        let mut opening = OpeningHousing {
            unit_limit: 1,
            dwellings: crate::model_rows::Rows::new().unwrap(),
            households: crate::model_rows::Rows::new().unwrap(),
            lease_offers: crate::model_rows::Rows::new().unwrap(),
        };
        opening
            .households
            .try_push(crate::housing::OpeningHousehold {
                household_id: 1,
                account: 10,
                people: 1,
                source_id: 1,
            })
            .unwrap();
        opening
    }

    fn timeline(at: u64) -> Timeline {
        Timeline::new(at)
    }

    #[test]
    fn records_actual_payments_in_half_open_complete_day_window() {
        let opening = opening();
        let mut window = PaidWageWindow::new();
        window.begin_coverage(0).unwrap();
        window
            .record_paid_delta(&opening, 10, Money::new(100).unwrap(), 0)
            .unwrap();
        window
            .record_paid_delta(&opening, 10, Money::new(20).unwrap(), 30 * DAY)
            .unwrap();
        // At day 31's boundary, days 0..30 are complete and included; day 31 is not.
        let summary = window.summarize(&opening, &timeline(31 * DAY), 1).unwrap();
        assert_eq!((summary.start_day, summary.end_day), (0, 31));
        assert_eq!(summary.paid_wages, Money::new(120).unwrap());

        window
            .record_paid_delta(&opening, 10, Money::new(7).unwrap(), 31 * DAY)
            .unwrap();
        let next = window.summarize(&opening, &timeline(32 * DAY), 1).unwrap();
        assert_eq!((next.start_day, next.end_day), (1, 32));
        assert_eq!(next.paid_wages, Money::new(27).unwrap());
    }

    #[test]
    fn incomplete_coverage_and_zero_income_are_distinct() {
        let opening = opening();
        let mut window = PaidWageWindow::new();
        assert_eq!(
            window.summarize(&opening, &timeline(31 * DAY), 1),
            Err(PaidWageError::CoverageIncomplete)
        );
        window.begin_coverage(0).unwrap();
        let empty_timeline = timeline(31 * DAY);
        let summary = window.summarize(&opening, &empty_timeline, 1).unwrap();
        assert_eq!(summary.paid_wages, Money::ZERO);
        assert_eq!(summary.qualifying_contracts, 0);
    }

    #[test]
    fn partial_day_genesis_does_not_backfill_earlier_payment() {
        let opening = opening();
        let mut window = PaidWageWindow::new();
        window.begin_coverage(12 * 3600).unwrap();
        window
            .record_paid_delta(&opening, 10, Money::new(999).unwrap(), 12 * 3600)
            .unwrap();
        assert_eq!(
            window.summarize(&opening, &timeline(31 * DAY), 1),
            Err(PaidWageError::CoverageIncomplete)
        );
        let summary = window.summarize(&opening, &timeline(32 * DAY), 1).unwrap();
        assert_eq!(summary.start_day, 1);
        assert_eq!(summary.paid_wages, Money::ZERO);
    }

    #[test]
    fn qualified_payroll_must_overlap_the_observed_window() {
        let opening = opening();
        let mut window = PaidWageWindow::new();
        window.begin_coverage(0).unwrap();

        let mut observed_timeline = timeline(32 * DAY);
        let ended_before_window = crate::timeline::Payroll::new(
            crate::timeline::PayrollSpec {
                id: 1,
                payer: 20,
                payee: 10,
                home_city: 0,
                hourly_cents: Money::new(100).unwrap(),
                cycle: crate::timeline::Cycle::Daily,
                ends_at: DAY,
            },
            0,
        )
        .unwrap();
        let current = crate::timeline::Payroll::new(
            crate::timeline::PayrollSpec {
                id: 2,
                payer: 21,
                payee: 10,
                home_city: 0,
                hourly_cents: Money::new(100).unwrap(),
                cycle: crate::timeline::Cycle::Daily,
                ends_at: 40 * DAY,
            },
            2 * DAY,
        )
        .unwrap();
        observed_timeline
            .payroll
            .try_push(ended_before_window)
            .unwrap();
        observed_timeline.payroll.try_push(current).unwrap();

        let summary = window.summarize(&opening, &observed_timeline, 1).unwrap();
        assert_eq!((summary.start_day, summary.end_day), (1, 32));
        assert_eq!(summary.qualifying_contracts, 1);
        assert_eq!(summary.paid_wages, Money::ZERO);

        let mut ended_only = timeline(32 * DAY);
        ended_only.payroll.try_push(ended_before_window).unwrap();
        window
            .record_paid_delta(&opening, 10, Money::new(50).unwrap(), 2 * DAY)
            .unwrap();
        let former_contract_payment = window.summarize(&opening, &ended_only, 1).unwrap();
        assert_eq!(former_contract_payment.qualifying_contracts, 0);
        assert_eq!(former_contract_payment.paid_wages, Money::new(50).unwrap());
    }
}
