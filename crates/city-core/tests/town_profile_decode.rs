use mycity_core::town_utility;
// Frozen old derived schema is a test-only wire oracle, not a second runtime profile.
use mycity_core::{
    model_rows::Record,
    town_economy::{self, DeliveryQuote, HouseholdGroup, Recipe, ShopSnapshot},
    town_runtime::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProfile {
    pub schema_version: u16,
    pub id: u32,
    /// SHA-256 of the immutable Store-owned daily transport allocation profile; all zeroes
    /// means the legacy profile delivery quotes remain the active substitute.
    #[serde(default)]
    pub transport_allocation_sha256: [u8; 32],
    /// Optional genesis-bound physical electricity supply; absent preserves legacy bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utility_supply: Option<crate::town_utility::BootSupplyBinding>,
    /// Optional genesis-bound liquidation authority. Omitted for legacy markets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_policy: Option<ExitPolicy>,
    /// Up to four companies may have immutable, opening-bound share charters. Empty is
    /// omitted so existing worlds keep their canonical representation.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub equity_charters: FixedRows<
        crate::town_economy::equity::EquityCharter,
        { crate::town_economy::equity::MAX_EQUITY_COMPANIES },
    >,
    /// Opening-bound bounded first-version business tax sources. Missing means there is no
    /// authoritative tax-coverage proof, so positive residual distribution remains blocked.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub tax_rules: FixedRows<CompanyTaxRule, MAX_COMPANY_TAX_RULES>,
    pub households: FixedRows<HouseholdGroup, { crate::town_economy::MAX_HOUSEHOLD_GROUPS }>,
    pub employers: FixedRows<EmployerSpec, { crate::town_economy::MAX_EMPLOYERS }>,
    pub shops: FixedRows<ShopSnapshot, { crate::town_economy::MAX_SHOP_STOCK_LOTS }>,
    pub recipes: FixedRows<Recipe, { crate::town_economy::MAX_RECIPES }>,
    pub delivery: FixedRows<DeliveryQuote, MAX_MARKET_LANES>,
}

fn profile() -> Value {
    serde_json::from_slice::<Value>(include_bytes!(
        "../../../fixtures/core-town-utility-opening.json"
    ))
    .unwrap()["town"]
        .clone()
}
fn compare(bytes: &[u8]) -> bool {
    let legacy = serde_json::from_slice::<LegacyProfile>(bytes);
    let current = serde_json::from_slice::<MarketProfile>(bytes);
    assert_eq!(
        current.is_ok(),
        legacy.is_ok(),
        "{}",
        String::from_utf8_lossy(bytes)
    );
    match (legacy, current) {
        (Ok(old), Ok(new)) => {
            assert_eq!(
                serde_json::to_vec(&old).unwrap(),
                serde_json::to_vec(&new).unwrap()
            );
            true
        }
        (Err(old), Err(new)) => {
            // Row limits must keep their exact original error token.
            if old.to_string().contains("town_profile_rows_full") {
                assert!(new.to_string().contains("town_profile_rows_full"));
            }
            false
        }
        _ => unreachable!(),
    }
}
#[test]
fn current_and_legacy_wire_match_for_defaults_and_explicit_nulls() {
    let base = profile();
    assert!(compare(&serde_json::to_vec(&base).unwrap()));
    for key in [
        "transport_allocation_sha256",
        "utility_supply",
        "exit_policy",
        "equity_charters",
        "tax_rules",
    ] {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(compare(&serde_json::to_vec(&v).unwrap()), "missing {key}");
        v[key] = Value::Null;
        let accepted = compare(&serde_json::to_vec(&v).unwrap());
        assert_eq!(
            accepted,
            matches!(key, "utility_supply" | "exit_policy"),
            "null {key}"
        );
    }
}
#[test]
fn required_unknown_and_duplicate_fields_keep_original_rejections() {
    let base = profile();
    for key in [
        "schema_version",
        "id",
        "households",
        "employers",
        "shops",
        "recipes",
        "delivery",
    ] {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(!compare(&serde_json::to_vec(&v).unwrap()));
    }
    let mut unknown = base.clone();
    unknown["unknown"] = json!(1);
    assert!(!compare(&serde_json::to_vec(&unknown).unwrap()));
    for key in [
        "schema_version",
        "id",
        "transport_allocation_sha256",
        "utility_supply",
        "exit_policy",
        "equity_charters",
        "tax_rules",
        "households",
        "employers",
        "shops",
        "recipes",
        "delivery",
    ] {
        let mut v = base.clone();
        let value = match key {
            "utility_supply" | "exit_policy" => Value::Null,
            "equity_charters" | "tax_rules" => json!([]),
            "transport_allocation_sha256" => json!(vec![0; 32]),
            _ => v[key].clone(),
        };
        v[key] = value.clone();
        let mut bytes = serde_json::to_vec(&v).unwrap();
        assert_eq!(bytes.pop(), Some(b'}'));
        bytes.extend_from_slice(
            format!(
                ",{}:{} }}",
                serde_json::to_string(key).unwrap(),
                serde_json::to_string(&value).unwrap()
            )
            .as_bytes(),
        );
        assert!(!compare(&bytes), "duplicate {key}");
    }
    for bytes in [b"{".as_slice(), b"null", b"[]", b"{}{}", &[0xff]] {
        assert!(!compare(bytes));
    }
}
#[test]
fn every_fixed_table_preserves_capacity_order_and_overflow_error() {
    let cases = [
        (
            "households",
            town_economy::MAX_HOUSEHOLD_GROUPS,
            serde_json::to_value(HouseholdGroup::inactive()).unwrap(),
        ),
        (
            "employers",
            town_economy::MAX_EMPLOYERS,
            serde_json::to_value(EmployerSpec::inactive()).unwrap(),
        ),
        (
            "shops",
            town_economy::MAX_SHOP_STOCK_LOTS,
            serde_json::to_value(ShopSnapshot::inactive()).unwrap(),
        ),
        (
            "recipes",
            town_economy::MAX_RECIPES,
            serde_json::to_value(Recipe::inactive()).unwrap(),
        ),
        (
            "delivery",
            MAX_MARKET_LANES,
            serde_json::to_value(DeliveryQuote::inactive()).unwrap(),
        ),
        (
            "equity_charters",
            town_economy::equity::MAX_EQUITY_COMPANIES,
            serde_json::to_value(town_economy::equity::EquityCharter::inactive()).unwrap(),
        ),
        (
            "tax_rules",
            MAX_COMPANY_TAX_RULES,
            serde_json::to_value(CompanyTaxRule::inactive()).unwrap(),
        ),
    ];
    for (key, n, row) in cases {
        let mut v = profile();
        v[key] = json!(vec![row.clone(); n]);
        assert!(compare(&serde_json::to_vec(&v).unwrap()), "{key} at limit");
        v[key] = json!(vec![row; n + 1]);
        assert!(!compare(&serde_json::to_vec(&v).unwrap()), "{key} overflow");
    }
}

#[test]
fn distinct_input_rows_keep_their_original_order() {
    let mut v = profile();
    let first = v["households"][0].clone();
    let mut second = first.clone();
    second["id"] = json!(first["id"].as_u64().unwrap() + 123);
    v["households"] = json!([second.clone(), first.clone()]);
    let bytes = serde_json::to_vec(&v).unwrap();
    assert!(compare(&bytes));
    let decoded: MarketProfile = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        serde_json::to_value(decoded.households.as_slice()).unwrap(),
        json!([second, first])
    );
}
