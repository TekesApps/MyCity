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
nonnegative_unit!(Quantity); // milli-native commodity unit: Q, Wh or L fixed by the profile
nonnegative_unit!(Energy); // Wh
nonnegative_unit!(Water); // L
nonnegative_unit!(GameTime); // seconds since world epoch

/// Original immutable-lot curve, shared by batch and continuous consumers.
/// This computes a cost delta only: it grants neither stock nor spending rights.
/// Using differences of cumulative floors preserves cents across arbitrary
/// partitions instead of repricing the remaining inventory after each take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LotBasisError { Origin, Overflow }

pub(crate) fn frozen_lot_basis_delta(
    origin_quantity: Quantity,
    origin_basis: Money,
    offset: Quantity,
    take: Quantity,
) -> Result<(Quantity, Money), LotBasisError> {
    let end = offset.checked_add(take).map_err(|_| LotBasisError::Overflow)?;
    let denominator = i128::from(origin_quantity.value());
    if denominator == 0 || end > origin_quantity { return Err(LotBasisError::Origin); }
    let price = i128::from(origin_basis.value());
    let delta = price * i128::from(end.value()) / denominator
        - price * i128::from(offset.value()) / denominator;
    let cost = Money::new(i64::try_from(delta).map_err(|_| LotBasisError::Overflow)?)
        .map_err(|_| LotBasisError::Overflow)?;
    Ok((end, cost))
}

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

impl<'de> Deserialize<'de> for Carry {
    fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error>{
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Saved {numerator:u64,denominator:u64,remainder:u64}
        let saved=Saved::deserialize(d)?;
        Self::restore(saved.numerator,saved.denominator,saved.remainder)
            .map_err(|_|serde::de::Error::custom("invalid_carry"))
    }
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
    fn restored_carry_uses_the_original_denominator_and_remainder_invariants(){
        let mut original=Carry::restore(7,3600,3599).unwrap();
        let bytes=serde_json::to_vec(&original).unwrap();
        let mut restored:Carry=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored,original);assert_eq!(restored.accrue(99),original.accrue(99));
        assert_eq!(restored,original);
        for invalid in [r#"{"numerator":7,"denominator":0,"remainder":0}"#,
            r#"{"numerator":7,"denominator":3,"remainder":3}"#,
            r#"{"numerator":7,"denominator":3,"remainder":-1}"#,
            r#"{"numerator":7,"denominator":3,"remainder":0,"extra":1}"#]{
            assert!(serde_json::from_str::<Carry>(invalid).is_err());
        }
    }
    #[test]
    fn frozen_lot_cents_do_not_change_when_consumption_is_partitioned() {
        let origin=Quantity::new(10).unwrap();let basis=Money::new(7).unwrap();
        let mut offset=Quantity::ZERO;let mut total=Money::ZERO;
        for (take,expected_total) in [(4,2),(3,4),(3,7)] {
            let (next,cost)=frozen_lot_basis_delta(origin,basis,offset,Quantity::new(take).unwrap()).unwrap();
            offset=next;total=total.checked_add(cost).unwrap();
            assert_eq!(total.value(),expected_total);
        }
        assert_eq!(offset,origin);
        assert_eq!(frozen_lot_basis_delta(origin,basis,Quantity::ZERO,origin).unwrap().1,total);
        let mut offset=Quantity::ZERO;let mut total=Money::ZERO;
        for expected in [0,1,2,2,3,4,4,5,6,7] {
            let (next,cost)=frozen_lot_basis_delta(origin,basis,offset,Quantity::new(1).unwrap()).unwrap();
            offset=next;total=total.checked_add(cost).unwrap();assert_eq!(total.value(),expected);
        }
    }
    #[test]
    fn frozen_lot_bounds_and_extreme_valid_product_are_checked() {
        let max=Quantity::new(i64::MAX).unwrap();
        assert_eq!(frozen_lot_basis_delta(max,Money::new(i64::MAX).unwrap(),
            Quantity::new(i64::MAX-1).unwrap(),Quantity::new(1).unwrap()).unwrap().1,Money::new(1).unwrap());
        assert_eq!(frozen_lot_basis_delta(Quantity::ZERO,Money::ZERO,Quantity::ZERO,Quantity::ZERO),Err(LotBasisError::Origin));
        assert_eq!(frozen_lot_basis_delta(Quantity::new(1).unwrap(),Money::ZERO,Quantity::new(1).unwrap(),Quantity::new(1).unwrap()),Err(LotBasisError::Origin));
        assert_eq!(frozen_lot_basis_delta(max,Money::ZERO,max,Quantity::new(1).unwrap()),Err(LotBasisError::Overflow));
    }
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
