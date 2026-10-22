use mycity_core::{
    town_utility::BootSupplyBinding,
    units::{ArithmeticError, Money, Quantity},
};

fn binding() -> BootSupplyBinding {
    BootSupplyBinding {
        boot_operator: 7,
        boot_stock_id: 19,
        stock_quantity_per_utility_unit: 3,
        tariff_per_utility_unit: Money::new(25).unwrap(),
    }
}

#[test]
fn binding_converts_with_floor_and_exact_integer_cost() {
    let value = binding();
    value.validate().unwrap();
    assert_eq!(
        value.available_units(Quantity::new(10).unwrap()).unwrap(),
        Quantity::new(3).unwrap()
    );
    assert_eq!(
        value.stock_quantity(Quantity::new(4).unwrap()).unwrap(),
        Quantity::new(12).unwrap()
    );
    assert_eq!(
        value.cost_per_output(Quantity::new(4).unwrap()).unwrap(),
        Money::new(100).unwrap()
    );
}

#[test]
fn zero_stock_and_zero_output_cost_remain_zero() {
    let value = binding();
    assert_eq!(
        value.available_units(Quantity::ZERO).unwrap(),
        Quantity::ZERO
    );
    assert_eq!(
        value.stock_quantity(Quantity::ZERO).unwrap(),
        Quantity::ZERO
    );
    assert_eq!(value.cost_per_output(Quantity::ZERO).unwrap(), Money::ZERO);
}

#[test]
fn zero_identity_conversion_and_tariff_are_invalid() {
    let mut invalid = binding();
    invalid.boot_operator = 0;
    assert_eq!(invalid.validate(), Err("zero_identity"));
    invalid = binding();
    invalid.boot_stock_id = 0;
    assert_eq!(invalid.validate(), Err("zero_identity"));
    invalid = binding();
    invalid.stock_quantity_per_utility_unit = 0;
    assert_eq!(invalid.validate(), Err("zero_stock_conversion"));
    invalid = binding();
    invalid.tariff_per_utility_unit = Money::ZERO;
    assert_eq!(invalid.validate(), Err("zero_utility_tariff"));
    assert_eq!(
        invalid.available_units(Quantity::new(10).unwrap()),
        Err(ArithmeticError::InvalidRate)
    );
}

#[test]
fn conversion_and_cost_overflow_are_rejected() {
    let mut value = binding();
    value.stock_quantity_per_utility_unit = 2;
    assert_eq!(
        value.stock_quantity(Quantity::new(i64::MAX).unwrap()),
        Err(ArithmeticError::Overflow)
    );

    value = binding();
    value.tariff_per_utility_unit = Money::new(i64::MAX).unwrap();
    assert_eq!(
        value.cost_per_output(Quantity::new(2).unwrap()),
        Err(ArithmeticError::Overflow)
    );
}

#[test]
fn serde_rejects_unknown_fields_and_preserves_binding() {
    let value = binding();
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: BootSupplyBinding = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
    let with_unknown = encoded.trim_end_matches('}').to_owned() + ",\"other\":1}";
    assert!(serde_json::from_str::<BootSupplyBinding>(&with_unknown).is_err());
}

#[test]
fn optional_supply_binding_stays_within_the_documented_fixed_slot() {
    assert!(std::mem::size_of::<Option<BootSupplyBinding>>() <= 32);
    assert!(!std::mem::needs_drop::<BootSupplyBinding>());
}
