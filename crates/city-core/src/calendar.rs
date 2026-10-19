//! Laboratory Gregorian calendar: epoch 2001-01-01, independent of host timezone.
use crate::units::GameTime;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}
pub fn leap(year: u16) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn month_days(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, &'static str> {
        if !(2001..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > month_days(year, month)
        {
            return Err("invalid_game_date");
        }
        Ok(Self { year, month, day })
    }
    pub fn year(self) -> u16 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }
    pub fn midnight(self) -> GameTime {
        let y = i64::from(self.year) - 1;
        let leap_days = y / 4 - y / 100 + y / 400 - (2000 / 4 - 2000 / 100 + 2000 / 400);
        let days = 365 * (i64::from(self.year) - 2001)
            + leap_days
            + (1..self.month)
                .map(|m| i64::from(month_days(self.year, m)))
                .sum::<i64>()
            + i64::from(self.day)
            - 1;
        GameTime::new(days * 86400).expect("bounded calendar date")
    }
    pub fn from_time(time: GameTime) -> Result<Self, &'static str> {
        if time.value() >= Self::new(9999, 12, 31)?.midnight().value() + 86400 {
            return Err("calendar_range");
        }
        let mut lo = 2001u16;
        let mut hi = 9999u16;
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if Self::new(mid, 1, 1)?.midnight() <= time {
                lo = mid
            } else {
                hi = mid - 1
            }
        }
        let mut day = (time.value() - Self::new(lo, 1, 1)?.midnight().value()) / 86400;
        let mut month = 1;
        while day >= i64::from(month_days(lo, month)) {
            day -= i64::from(month_days(lo, month));
            month += 1;
        }
        Self::new(lo, month, day as u8 + 1)
    }
    /// Preserve the contract's original anchor: Jan31 -> Feb28 -> Mar31.
    pub fn next_monthly(self, anchor_day: u8) -> Result<Self, &'static str> {
        if !(1..=31).contains(&anchor_day) {
            return Err("invalid_month_anchor");
        }
        let (year, month) = if self.month == 12 {
            (self.year.checked_add(1).ok_or("calendar_range")?, 1)
        } else {
            (self.year, self.month + 1)
        };
        Self::new(year, month, anchor_day.min(month_days(year, month)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn month_anchor_does_not_drift_after_february() {
        let jan = Date::new(2001, 1, 31).unwrap();
        let feb = jan.next_monthly(31).unwrap();
        assert_eq!(feb, Date::new(2001, 2, 28).unwrap());
        assert_eq!(
            feb.next_monthly(31).unwrap(),
            Date::new(2001, 3, 31).unwrap()
        );
        assert_eq!(
            Date::new(2004, 1, 31).unwrap().next_monthly(31).unwrap(),
            Date::new(2004, 2, 29).unwrap()
        );
        assert!(!leap(2100));
        assert!(leap(2400));
    }
    #[test]
    fn rent_30_days_is_not_calendar_month_or_four_weeks() {
        let jan = Date::new(2001, 1, 1).unwrap();
        let rent = jan
            .midnight()
            .checked_add(GameTime::new(30 * 86400).unwrap())
            .unwrap();
        assert_ne!(rent, jan.next_monthly(1).unwrap().midnight());
        assert_eq!(
            Date::from_time(rent).unwrap(),
            Date::new(2001, 1, 31).unwrap()
        );
    }
    #[test]
    fn dates_round_trip_and_reject_invalid_bounds() {
        for y in [2001, 2004, 2100, 2400, 9999] {
            for m in 1..=12 {
                for d in [1, month_days(y, m)] {
                    let date = Date::new(y, m, d).unwrap();
                    assert_eq!(Date::from_time(date.midnight()).unwrap(), date);
                }
            }
        }
        assert!(Date::new(2001, 2, 29).is_err());
        assert!(Date::new(2000, 1, 1).is_err());
        assert!(Date::new(9999, 12, 31).unwrap().next_monthly(1).is_err());
    }
}
