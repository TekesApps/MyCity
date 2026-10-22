//! Immutable mapping from one Boot electricity stock to one utility operator.
//!
//! This is a value object and checked unit conversion only. It grants no authority
//! and does not create or promise supply. For electricity, the stock quantity and
//! recipe utility requirement are both represented by `Quantity` integer ticks; the
//! binding states how many Boot stock ticks fund one recipe utility tick. Thus a
//! conversion never rounds a stock requirement down or invents fractional stock.
use crate::units::{ArithmeticError, Money, Quantity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootSupplyBinding {
    /// Account operating the Boot-backed electricity generator.
    pub boot_operator: u32,
    /// Existing Boot stock ID. Caller must verify its owner and electricity commodity.
    pub boot_stock_id: u32,
    /// Boot-stock integer ticks consumed per recipe utility integer tick.
    /// Both sides use `Quantity`; the fixed electricity profile determines their
    /// native scale (the stock is electricity in Wh-derived Quantity ticks).
    pub stock_quantity_per_utility_unit: u32,
    /// Fixed cents charged per recipe utility integer tick.
    pub tariff_per_utility_unit: Money,
}

impl BootSupplyBinding {
    /// Validates the immutable electricity binding fields.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.boot_operator == 0 || self.boot_stock_id == 0 {
            return Err("zero_identity");
        }
        if self.stock_quantity_per_utility_unit == 0 {
            return Err("zero_stock_conversion");
        }
        if self.tariff_per_utility_unit == Money::ZERO {
            return Err("zero_utility_tariff");
        }
        Ok(())
    }

    /// Whole recipe-utility ticks supportable by available Boot stock (floor).
    pub fn available_units(&self, stock_quantity: Quantity) -> Result<Quantity, ArithmeticError> {
        self.validate().map_err(|_| ArithmeticError::InvalidRate)?;
        Quantity::new(stock_quantity.value() / i64::from(self.stock_quantity_per_utility_unit))
    }

    /// Exact Boot stock ticks required for a recipe utility quantity.
    pub fn stock_quantity(&self, units_quantity: Quantity) -> Result<Quantity, ArithmeticError> {
        self.validate().map_err(|_| ArithmeticError::InvalidRate)?;
        let value = i128::from(units_quantity.value())
            .checked_mul(i128::from(self.stock_quantity_per_utility_unit))
            .ok_or(ArithmeticError::Overflow)?;
        Quantity::new(i64::try_from(value).map_err(|_| ArithmeticError::Overflow)?)
    }

    /// Total cents for this recipe's electricity input quantity.
    pub fn cost_per_output(&self, utility_per_output: Quantity) -> Result<Money, ArithmeticError> {
        self.validate().map_err(|_| ArithmeticError::InvalidRate)?;
        let value = i128::from(utility_per_output.value())
            .checked_mul(i128::from(self.tariff_per_utility_unit.value()))
            .ok_or(ArithmeticError::Overflow)?;
        Money::new(i64::try_from(value).map_err(|_| ArithmeticError::Overflow)?)
    }
}
