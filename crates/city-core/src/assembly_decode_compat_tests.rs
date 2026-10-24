//! Direct-derive mirrors of the CURRENT schema, compared with the small-carrier decoder.
//! Run in release: large derived frames intentionally remain the independent strategy oracle.
//! The frozen pre-venue World shape remains below as a separate legacy wire oracle.
mod old_opening {
    use super::*;
    #[allow(dead_code)] // Fields are compared through derived Debug in this legacy mirror.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields, rename = "Opening")]
    pub(super) struct Opening {
        #[serde(deserialize_with = "crate::bounded_decode::text64")]
        pub schema: String,
        pub bootstrap: Bootstrap,
        pub sources: Rows<OpeningSource, MAX_SOURCES>,
        pub accounts: Rows<OpeningAccount, MAX_ACCOUNTS>,
        pub stocks: Rows<OpeningStock, MAX_STOCKS>,
        pub region: Option<crate::region::Opening>,
        pub economy: Option<crate::economy::Opening>,
        pub boot: Option<crate::boot::Opening>,
        pub exchange: Option<crate::exchange::Opening>,
        /// Optional fixed one-project prototype profile. Runtime changes only through Engine actions.
        #[serde(default)]
        pub construction: Option<crate::construction::Project>,
        /// Optional fixed market profile. It is genesis configuration; runtime changes use
        /// compact Engine actions and persist in the same World/checkpoint.
        #[serde(default)]
        pub town: Option<crate::town_runtime::MarketProfile>,
        /// Optional bounded housing stock and immutable one-shot lease-offer catalogue.
        #[serde(default)]
        pub housing: Option<crate::housing::OpeningHousing>,
        pub venue: Option<crate::venue_runtime::Opening>,
        pub education: Option<crate::education_runtime::Opening>,
        pub qualification: Option<crate::qualification_authority::Opening>,
        pub member_activity: Option<crate::member_authority::Registration>,
        pub service_needs: Option<crate::service_needs::Opening>,
    }
}
use old_opening::Opening as MirrorOpening;

mod old_world {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename = "World")]
    pub(super) struct World {
        pub(crate) scope: Scope,
        pub(crate) seed: u64,
        pub(crate) revision: u64,
        pub(crate) accounts: Arena<AccountDomain, Account>,
        pub(crate) stocks: Arena<InventoryDomain, Stock>,
        pub(crate) account_ids: Rows<(u32, Handle<AccountDomain>), MAX_ACCOUNTS>,
        pub(crate) stock_ids: Rows<(u32, Handle<InventoryDomain>), MAX_STOCKS>,
        sources: Rows<OpeningSource, MAX_SOURCES>,
        pub(crate) region: Option<crate::region::Region>,
        pub(crate) economy: Option<crate::economy::Economy>,
        pub(crate) boot: Option<crate::boot::Boot>,
        pub(crate) exchange: Option<crate::exchange::Exchange>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) construction: Option<crate::construction::Project>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) town: Option<crate::town_runtime::TownMarket>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) housing: Option<Box<HousingRuntime>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) venue: Option<Box<crate::venue_runtime::Runtime>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) education: Option<Box<crate::education_runtime::Runtime>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) member_activity: Option<Box<crate::member_activity::Ledger>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) qualification: Option<Box<crate::qualification_authority::Runtime>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) service_needs: Option<Box<crate::service_needs::Runtime>>,
    }
}
use old_world::World as MirrorWorld;

// Exact pre-venue field shape retained, not updated with the current schema.
mod legacy_world {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename = "World")]
    pub(super) struct World {
        pub(crate) scope: Scope,
        pub(crate) seed: u64,
        pub(crate) revision: u64,
        pub(crate) accounts: Arena<AccountDomain, Account>,
        pub(crate) stocks: Arena<InventoryDomain, Stock>,
        pub(crate) account_ids: Rows<(u32, Handle<AccountDomain>), MAX_ACCOUNTS>,
        pub(crate) stock_ids: Rows<(u32, Handle<InventoryDomain>), MAX_STOCKS>,
        sources: Rows<OpeningSource, MAX_SOURCES>,
        pub(crate) region: Option<crate::region::Region>,
        pub(crate) economy: Option<crate::economy::Economy>,
        pub(crate) boot: Option<crate::boot::Boot>,
        pub(crate) exchange: Option<crate::exchange::Exchange>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) construction: Option<crate::construction::Project>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) town: Option<crate::town_runtime::TownMarket>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) housing: Option<Box<HousingRuntime>>,
    }
}
use legacy_world::World as LegacyWorld;

mod old_townmarket {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename = "TownMarket")]
    pub(super) struct TownMarket {
        profile: MarketProfile,
        state: MarketState,
    }
}
use old_townmarket::TownMarket as MirrorTownMarket;

fn project_fixture() -> crate::construction::Project {
    use crate::{
        construction::{Gate, GateCategory, GateDecision, Kind, Material},
        units::GameTime,
    };
    crate::construction::Project::new(
        7,
        Kind::Build,
        4,
        3,
        1,
        80,
        1,
        1,
        Money::new(1_000_000).unwrap(),
        Money::new(3_600).unwrap(),
        28_800,
        &[Gate {
            id: 1,
            category: GateCategory::Permit,
            evidence_id: 0,
            party: 1,
            decided: GateDecision::Pending,
        }],
        &[Material {
            stock_id: 1,
            required: Quantity::new(1_000).unwrap(),
            consumed: Quantity::ZERO,
            basis_per_unit: Money::new(400).unwrap(),
            basis_remainder: 0,
        }],
        GameTime::ZERO,
    )
    .unwrap()
}

use super::*;
use crate::town_runtime::{MarketProfile, MarketState, TownMarket};
use serde_json::{json, Value};

fn cases(value: &Value) -> Vec<Vec<u8>> {
    let original = serde_json::to_string(value).unwrap();
    let mut cases = vec![
        original.as_bytes().to_vec(),
        b"[]".to_vec(),
        b"null".to_vec(),
        format!("{original} true").into_bytes(),
    ];
    let mut unknown = value.clone();
    unknown["unknown_decode_field"] = json!(1);
    cases.push(serde_json::to_vec(&unknown).unwrap());
    for (key, field) in value.as_object().unwrap() {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        cases.push(serde_json::to_vec(&missing).unwrap());
        let mut null = value.clone();
        null[key] = Value::Null;
        cases.push(serde_json::to_vec(&null).unwrap());
        for first in [Value::Null, field.clone()] {
            cases.push(
                format!(
                    "{{{}:{},{}",
                    serde_json::to_string(key).unwrap(),
                    serde_json::to_string(&first).unwrap(),
                    &original[1..]
                )
                .into_bytes(),
            );
        }
    }
    cases
}
fn compare_world(bytes: &[u8]) {
    match (
        serde_json::from_slice::<MirrorWorld>(bytes),
        serde_json::from_slice::<World>(bytes),
    ) {
        (Ok(a), Ok(b)) => assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        ),
        (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
        _ => panic!("World compatibility diverged"),
    }
}
fn compare_legacy_world(bytes: &[u8]) {
    let legacy: LegacyWorld = serde_json::from_slice(bytes).unwrap();
    let original_wire = serde_json::to_vec(&legacy).unwrap();
    let current: World = serde_json::from_slice(&original_wire).unwrap();
    assert!(current.venue.is_none());
    assert!(!serde_json::from_slice::<Value>(&original_wire).unwrap().as_object().unwrap().contains_key("venue"));
    assert_eq!(serde_json::to_vec(&current).unwrap(), original_wire);
}
fn compare_town(bytes: &[u8]) {
    match (
        serde_json::from_slice::<MirrorTownMarket>(bytes),
        serde_json::from_slice::<TownMarket>(bytes),
    ) {
        (Ok(a), Ok(b)) => assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        ),
        (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
        _ => panic!("TownMarket compatibility diverged"),
    }
}
#[test]
fn opening_legacy_derive_wire_acceptance_and_errors_match() {
    let fixtures: &[&[u8]] = &[
        include_bytes!("../../../fixtures/m0-e0-opening.json"),
        include_bytes!("../../../fixtures/m0-region-opening.json"),
        include_bytes!("../../../fixtures/m0-economy-opening.json"),
        include_bytes!("../../../fixtures/m0-boot-opening.json"),
        include_bytes!("../../../fixtures/m0-exchange-opening.json"),
        include_bytes!("../../../fixtures/core-town-utility-opening.json"),
    ];
    for bytes in fixtures {
        assert!(serde_json::from_slice::<Value>(bytes).unwrap().get("venue").is_none());
        assert!(serde_json::from_slice::<Opening>(bytes).unwrap().venue.is_none());
    }
    let mut values: Vec<Value> = fixtures
        .iter()
        .map(|b| serde_json::from_slice(b).unwrap())
        .collect();
    let mut extras = values[0].clone();
    extras["construction"] = serde_json::to_value(project_fixture()).unwrap();
    extras["housing"] = json!({"unit_limit":8,"dwellings":[],"lease_offers":[]});
    values.push(extras);
    values.push(serde_json::from_slice(include_bytes!("../../city-storage/tests/fixtures/opening-venue-store/managed-people-v1-opening.json")).unwrap());
    let mut registered=values.last().unwrap().clone();
    registered["member_activity"]=serde_json::to_value(crate::member_authority::Registration{version:1,
        scope:crate::arena::Scope::new(1,1).unwrap(),population_origin:[7;32],total_people:10,epoch:1}).unwrap();
    values.push(registered); // Parser equivalence only, not a valid population provider.
    let mut needs=values.last().unwrap().clone();
    needs["service_needs"]=json!({"version":1,"profile":"LocalDevelopmentNaturalNeedsV1","scope":{"world":1,"branch":1},"seed":7,"city_id":1,
        "households":[{"household_id":7,"source_id":7001,"generation":1,"people":10}]});
    values.push(needs); // Current parser coverage, not a claim of authentic entry.
    let mut count = 0;
    for value in values {
        for bytes in cases(&value) {
            match (
                serde_json::from_slice::<MirrorOpening>(&bytes),
                serde_json::from_slice::<Opening>(&bytes),
            ) {
                (Ok(a), Ok(b)) => assert_eq!(
                    format!("{a:?}").replacen("MirrorOpening", "Opening", 1),
                    format!("{b:?}")
                ),
                (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
                _ => panic!("Opening compatibility diverged"),
            }
            count += 1;
        }
    }
    eprintln!("opening mirror cases={count}");
}
#[test]
fn non_initial_town_and_world_legacy_wire_match() {
    let raw = include_bytes!("../../../fixtures/core-town-utility-opening.json");
    let mut world = assemble(raw).unwrap();
    let town = world.town.as_mut().unwrap();
    town.state.settled_day = 37;
    town.state.last_summary.day = 37;
    town.state.last_summary.operation_id = 70037;
    town.state
        .payroll
        .push(crate::town_runtime::PayrollCursor {
            id: 7,
            work_seconds: 12345,
            accrued: Money::new(1234).unwrap(),
            paid: Money::new(731).unwrap(),
        })
        .unwrap();
    town.state
        .recipe_output_remainders
        .push(crate::town_runtime::RecipeOutputRemainder {
            recipe_id: 1,
            numerator: 123,
        })
        .unwrap();
    town.state
        .tax_cursors
        .push(crate::town_runtime::TaxAssessmentCursor {
            company: 3,
            policy_id: 77,
            through_day: 37,
            last_operation_id: 70037,
            last_claim_id: 991,
            cumulative_assessed: Money::new(321).unwrap(),
            retained_earnings_cents: -17,
        })
        .unwrap();
    use crate::town_economy::equity::{
        EquityInvestorBinding, EquityParty, EquityPolicy, EquityRegister, PartyKind,
    };
    let mut register = EquityRegister::from_charter(
        3,
        EquityPolicy::initial(77, 1000),
        &[EquityInvestorBinding {
            party: EquityParty {
                kind: PartyKind::Person,
                id: 7001,
            },
            account: 2,
            opening_shares: 5,
        }],
    )
    .unwrap();
    register.cumulative_dividends_distributed = Money::new(23).unwrap();
    town.state.equity_registers.push(register).unwrap();
    let town_value = serde_json::to_value(&*town).unwrap();
    for bytes in cases(&town_value) {
        compare_town(&bytes);
    }
    let restored: TownMarket = serde_json::from_value(town_value).unwrap();
    assert_eq!(restored.state.settled_day, 37);
    assert_eq!(
        restored.state.payroll.as_slice()[0].paid,
        Money::new(731).unwrap()
    );
    assert_eq!(
        restored.state.recipe_output_remainders.as_slice()[0].numerator,
        123
    );
    assert_eq!(
        restored.state.tax_cursors.as_slice()[0].retained_earnings_cents,
        -17
    );
    assert_eq!(
        restored.state.equity_registers.as_slice()[0].cumulative_dividends_distributed,
        Money::new(23).unwrap()
    );
    compare_legacy_world(&serde_json::to_vec(&world).unwrap());
    for bytes in cases(&serde_json::to_value(&world).unwrap()) {
        compare_world(&bytes);
    }
    let legacy = assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
    compare_legacy_world(&serde_json::to_vec(&legacy).unwrap());
    for bytes in cases(&serde_json::to_value(legacy).unwrap()) {
        compare_world(&bytes);
    }
    let venue = assemble(include_bytes!("../../city-storage/tests/fixtures/opening-venue-store/managed-people-v1-opening.json")).unwrap();
    assert!(venue.venue.is_some());
    for bytes in cases(&serde_json::to_value(&venue).unwrap()) { compare_world(&bytes); }
    eprintln!("decode layout opening={} opening_scratch={} world={} world_scratch={} town={} town_scratch={}",size_of::<Opening>(),size_of::<OpeningDecode>(),size_of::<World>(),size_of::<WorldDecode>(),size_of::<TownMarket>(),size_of::<(Box<MarketProfile>,Box<MarketState>)>());
}
