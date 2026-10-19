//! Exact CPU units. No implicit conversion between money, time or physical quantities.
//!
//! ```compile_fail
//! use mycity_core::units::{Money, GameTime};
//! Money::new(10).unwrap().checked_add(GameTime::new(10).unwrap());
//! ```
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticError {
    Negative,
    Overflow,
    InvalidRate,
}

macro_rules! nonnegative_unit {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(i64);

        impl $name {
            pub const ZERO: Self = Self(0);
            pub fn new(value: i64) -> Result<Self, ArithmeticError> {
                if value < 0 {
                    Err(ArithmeticError::Negative)
                } else {
                    Ok(Self(value))
                }
            }
            pub const fn value(self) -> i64 {
                self.0
            }
            pub fn checked_add(self, other: Self) -> Result<Self, ArithmeticError> {
                Self::new(
                    self.0
                        .checked_add(other.0)
                        .ok_or(ArithmeticError::Overflow)?,
                )
            }
            pub fn checked_sub(self, other: Self) -> Result<Self, ArithmeticError> {
                Self::new(
                    self.0
                        .checked_sub(other.0)
                        .ok_or(ArithmeticError::Overflow)?,
                )
            }
            pub fn checked_mul(self, factor: u64) -> Result<Self, ArithmeticError> {
                let value = i128::from(self.0)
                    .checked_mul(i128::from(factor))
                    .ok_or(ArithmeticError::Overflow)?;
                Self::new(i64::try_from(value).map_err(|_| ArithmeticError::Overflow)?)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::new(i64::deserialize(d)?)
                    .map_err(|_| serde::de::Error::custom("negative_unit"))
            }
        }
    };
}

nonnegative_unit!(Money); // cents, 100 per laboratory currency unit
nonnegative_unit!(Quantity); // milli-Q
nonnegative_unit!(Energy); // Wh
nonnegative_unit!(Water); // L
nonnegative_unit!(GameTime); // seconds since world epoch

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Centimeters(pub i64);

/// Per-source carry. The denominator is immutable for its lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Carry {
    numerator: u64,
    denominator: u64,
    remainder: u64,
}

impl Carry {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, ArithmeticError> {
        Self::restore(numerator, denominator, 0)
    }
    pub fn restore(
        numerator: u64,
        denominator: u64,
        remainder: u64,
    ) -> Result<Self, ArithmeticError> {
        if denominator == 0 || remainder >= denominator {
            return Err(ArithmeticError::InvalidRate);
        }
        Ok(Self {
            numerator,
            denominator,
            remainder,
        })
    }
    pub fn accrue(&mut self, units: u64) -> Result<u64, ArithmeticError> {
        let total = u128::from(units) * u128::from(self.numerator) + u128::from(self.remainder);
        let output = u64::try_from(total / u128::from(self.denominator))
            .map_err(|_| ArithmeticError::Overflow)?;
        self.remainder = (total % u128::from(self.denominator)) as u64;
        Ok(output)
    }
    pub const fn remainder(self) -> u64 {
        self.remainder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_values_and_overflow_cannot_enter_exact_units() {
        assert_eq!(Money::new(-1), Err(ArithmeticError::Negative));
        assert!(serde_json::from_str::<Money>("-1").is_err());
        assert_eq!(
            Money::new(i64::MAX)
                .unwrap()
                .checked_add(Money::new(1).unwrap()),
            Err(ArithmeticError::Overflow)
        );
        assert_eq!(
            Money::ZERO.checked_sub(Money::new(1).unwrap()),
            Err(ArithmeticError::Negative)
        );
        assert_eq!(
            Quantity::new(i64::MAX).unwrap().checked_mul(2),
            Err(ArithmeticError::Overflow)
        );
    }
    #[test]
    fn splitting_and_restoring_time_does_not_drop_fractional_cost() {
        let mut split = Carry::new(7, 3600).unwrap();
        let mut sum = 0;
        for _ in 0..1800 {
            sum += split.accrue(1).unwrap();
        }
        let mut restored = Carry::restore(7, 3600, split.remainder()).unwrap();
        for _ in 0..1800 {
            sum += restored.accrue(1).unwrap();
        }
        let mut whole = Carry::new(7, 3600).unwrap();
        assert_eq!(sum, whole.accrue(3600).unwrap());
        assert_eq!(restored.remainder(), whole.remainder());
        assert_eq!(sum, 7);
    }
    #[test]
    fn failed_accrual_preserves_carry() {
        let mut carry = Carry::restore(u64::MAX, 1, 0).unwrap();
        let old = carry;
        assert!(carry.accrue(2).is_err());
        assert_eq!(carry, old);
        assert!(Carry::restore(1, 3, 3).is_err());
        assert!(Carry::new(1, 0).is_err());
    }
}
