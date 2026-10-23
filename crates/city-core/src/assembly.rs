//! Bounded initial capital and stock assembly. Restores must not call this initializer.
use crate::model_rows::{Record, Rows};
use crate::{
    arena::{AccountDomain, Arena, Handle, InventoryDomain, Scope},
    sha256,
    units::{Money, Quantity},
    Bootstrap, INPUT_LIMIT,
};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use std::mem::size_of;

pub const ASSEMBLY_SCHEMA: &str = "mycity-m0-opening-v1";
pub const MAX_ACCOUNTS: usize = 64;
pub const MAX_STOCKS: usize = 128;
pub const MAX_SOURCES: usize = MAX_ACCOUNTS + MAX_STOCKS;
pub const WORLD_BYTE_BUDGET: usize = 128 * 1024;

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Treasury,
    Household,
    Company,
    Supplier,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Commodity {
    Raw,
    Product,
    Fuel,
    Electricity,
    Water,
    RawWater,
    WasteWater,
    Heat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    CashCapital,
    InventoryCapital,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningSource {
    pub id: u32,
    pub kind: SourceKind,
    pub amount: Money,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningAccount {
    pub id: u32,
    pub role: Role,
    pub cash: Money,
    pub source: u32,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningStock {
    pub id: u32,
    pub owner: u32,
    pub commodity: Commodity,
    pub quantity: Quantity,
    pub basis: Money,
    pub source: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
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
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Account {
    pub id: u32,
    pub role: Role,
    pub cash: Money,
    pub protected: Money,
    /// Genesis source for chartered company cash. Zero on legacy/non-chartered accounts.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub opening_source_id: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Stock {
    pub id: u32,
    pub owner: Handle<AccountDomain>,
    pub commodity: Commodity,
    pub quantity: Quantity,
    pub basis: Money,
    pub lot_id: u32,
    pub origin_quantity: Quantity,
    pub origin_basis: Money,
    pub offset: Quantity,
    /// Immutable genesis source for chartered company inventory; zero otherwise.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub opening_source_id: u32,
    /// Immutable source amount, distinct from the mutable basis of later production lots.
    #[serde(default, skip_serializing_if = "is_zero_money")]
    pub opening_basis: Money,
}

/// Optional bounded housing runtime. The Box keeps the common no-housing World header stable;
/// its owned bytes are charged explicitly to the existing World budget when present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HousingRuntime {
    pub(crate) opening: crate::housing::OpeningHousing,
    pub(crate) state: crate::housing::HousingState,
}
impl HousingRuntime {
    fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.opening.owned_capacity_bytes() + self.state.owned_capacity_bytes()
    }
}

impl crate::restore_into::Restore for Option<Box<HousingRuntime>> {
    fn restore<'de, D: Deserializer<'de>>(&mut self, deserializer: D) -> Result<(), D::Error> {
        struct HousingOptionInto<'a>(&'a mut Option<Box<HousingRuntime>>);
        impl<'de> Visitor<'de> for HousingOptionInto<'_> {
            type Value = ();

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("optional bounded housing runtime")
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<(), E> {
                *self.0 = None;
                Ok(())
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
                self.visit_none()
            }

            fn visit_some<D2: Deserializer<'de>>(self, deserializer: D2) -> Result<(), D2::Error> {
                // OpeningHousing rows have a fixed 32-row ceiling and HousingState checks its
                // encoded limit before allocating. Decode exactly one such bounded runtime,
                // then box it; this is never a generic/unbounded Box restore.
                *self.0 = None;
                let runtime = HousingRuntime::deserialize(deserializer)?;
                *self.0 = Some(Box::new(runtime));
                Ok(())
            }
        }
        deserializer.deserialize_option(HousingOptionInto(self))
    }
}

fn is_zero_money(value: &Money) -> bool {
    *value == Money::ZERO
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct World {
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

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AccountView {
    pub id: u32,
    pub role: Role,
    pub cash: Money,
    pub protected: Money,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub opening_source_id: u32,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct StockView {
    pub id: u32,
    pub owner: u32,
    pub commodity: Commodity,
    pub quantity: Quantity,
    pub basis: Money,
    pub lot_id: u32,
    pub origin_quantity: Quantity,
    pub origin_basis: Money,
    pub offset: Quantity,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub opening_source_id: u32,
    #[serde(skip_serializing_if = "is_zero_money")]
    pub opening_basis: Money,
}
/// Publicly observable housing summary. Immutable offer terms and genesis source registrations
/// remain in the checkpoint/runtime, while a view copies only the bounded mutable unit state.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct HousingView {
    pub unit_limit: u8,
    pub dwelling_count: u8,
    pub household_registration_count: u8,
    pub lease_offer_count: u8,
    pub units: Vec<crate::housing::HousingUnit>,
    pub consumed_offer_ids: Vec<u64>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct WorldView {
    pub schema: &'static str,
    pub scope: Scope,
    pub seed: u64,
    pub revision: u64,
    pub accounts: Vec<AccountView>,
    pub stocks: Vec<StockView>,
    pub opening_sources: Vec<OpeningSource>,
    pub region: Option<crate::region::Region>,
    pub economy: Option<crate::economy::Economy>,
    pub boot: Option<crate::boot::Boot>,
    pub exchange: Option<crate::exchange::Exchange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub construction: Option<crate::construction::Project>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub town: Option<crate::town_runtime::TownMarket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub housing: Option<HousingView>,
}

impl Record for OpeningSource {
    fn inactive() -> Self {
        Self {
            id: 0,
            kind: SourceKind::CashCapital,
            amount: Money::ZERO,
        }
    }
}
impl Record for OpeningAccount {
    fn inactive() -> Self {
        Self {
            id: 0,
            role: Role::Treasury,
            cash: Money::ZERO,
            source: 0,
        }
    }
}
impl Record for OpeningStock {
    fn inactive() -> Self {
        Self {
            id: 0,
            owner: 0,
            commodity: Commodity::Raw,
            quantity: Quantity::ZERO,
            basis: Money::ZERO,
            source: 0,
        }
    }
}
impl<D: crate::arena::Domain> Record for (u32, Handle<D>) {
    fn inactive() -> Self {
        (0, Handle::inactive())
    }
}
/// Full checkpoint world with a deliberately zeroed revision, borrowing all tables.
/// Keep distinct from the observable WorldView (which translates inventory owners).
#[derive(Serialize)]
pub(crate) struct BusinessWorld<'a> {
    scope: Scope,
    seed: u64,
    revision: u64,
    accounts: &'a Arena<AccountDomain, Account>,
    stocks: &'a Arena<InventoryDomain, Stock>,
    account_ids: &'a Rows<(u32, Handle<AccountDomain>), MAX_ACCOUNTS>,
    stock_ids: &'a Rows<(u32, Handle<InventoryDomain>), MAX_STOCKS>,
    sources: &'a Rows<OpeningSource, MAX_SOURCES>,
    region: &'a Option<crate::region::Region>,
    economy: &'a Option<crate::economy::Economy>,
    boot: &'a Option<crate::boot::Boot>,
    exchange: &'a Option<crate::exchange::Exchange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    construction: &'a Option<crate::construction::Project>,
    #[serde(skip_serializing_if = "Option::is_none")]
    town: &'a Option<crate::town_runtime::TownMarket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    housing: &'a Option<Box<HousingRuntime>>,
}
#[derive(Serialize)]
pub(crate) struct BorrowedWorldView<'a> {
    schema: &'static str,
    scope: Scope,
    seed: u64,
    revision: u64,
    accounts: AccountViews<'a>,
    stocks: StockViews<'a>,
    opening_sources: &'a Rows<OpeningSource, MAX_SOURCES>,
    region: &'a Option<crate::region::Region>,
    economy: &'a Option<crate::economy::Economy>,
    boot: &'a Option<crate::boot::Boot>,
    exchange: &'a Option<crate::exchange::Exchange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    construction: &'a Option<crate::construction::Project>,
    #[serde(skip_serializing_if = "Option::is_none")]
    town: &'a Option<crate::town_runtime::TownMarket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    housing: &'a Option<Box<HousingRuntime>>,
}
struct AccountViews<'a>(&'a World);
impl Serialize for AccountViews<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(self.0.account_ids.len()))?;
        for (_, h) in self.0.account_ids.iter() {
            let a = self.0.accounts.get(*h).expect("validated world");
            seq.serialize_element(&AccountView {
                id: a.id,
                role: a.role,
                cash: a.cash,
                protected: a.protected,
                opening_source_id: a.opening_source_id,
            })?;
        }
        seq.end()
    }
}
struct StockViews<'a>(&'a World);
impl Serialize for StockViews<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(self.0.stock_ids.len()))?;
        for (_, h) in self.0.stock_ids.iter() {
            let v = self.0.stocks.get(*h).expect("validated world");
            seq.serialize_element(&StockView {
                id: v.id,
                owner: self.0.accounts.get(v.owner).expect("validated owner").id,
                commodity: v.commodity,
                quantity: v.quantity,
                basis: v.basis,
                lot_id: v.lot_id,
                origin_quantity: v.origin_quantity,
                origin_basis: v.origin_basis,
                offset: v.offset,
                opening_source_id: v.opening_source_id,
                opening_basis: v.opening_basis,
            })?;
        }
        seq.end()
    }
}
impl World {
    pub(crate) fn restore_empty() -> Result<Self, String> {
        let scope = Scope::new(1, 1).expect("internal scope");
        Ok(Self {
            scope,
            seed: 0,
            revision: 0,
            accounts: Arena::new(scope, MAX_ACCOUNTS as u32, WORLD_BYTE_BUDGET / 2)
                .map_err(|e| format!("{e:?}"))?,
            stocks: Arena::new(scope, MAX_STOCKS as u32, WORLD_BYTE_BUDGET / 2)
                .map_err(|e| format!("{e:?}"))?,
            account_ids: Rows::new().map_err(str::to_owned)?,
            stock_ids: Rows::new().map_err(str::to_owned)?,
            sources: Rows::new().map_err(str::to_owned)?,
            region: None,
            economy: None,
            boot: None,
            exchange: None,
            construction: None,
            town: None,
            housing: None,
        })
    }
    pub(crate) fn reset_backing(&mut self) {
        self.accounts.reset_backing();
        self.stocks.reset_backing();
        self.account_ids.clear();
        self.stock_ids.clear();
        self.sources.clear();
        self.scope = Scope::new(1, 1).expect("internal scope");
        self.seed = 0;
        self.revision = 0;
        self.region = None;
        self.economy = None;
        self.boot = None;
        self.exchange = None;
        self.construction = None;
        self.town = None;
        self.housing = None;
    }
    pub(crate) fn copy_into(&self, target: &mut Self) -> Result<(), String> {
        self.accounts.copy_into(&mut target.accounts)?;
        self.stocks.copy_into(&mut target.stocks)?;
        target.account_ids.copy_active_from(&self.account_ids);
        target.stock_ids.copy_active_from(&self.stock_ids);
        target.sources.copy_active_from(&self.sources);
        target.scope = self.scope;
        target.seed = self.seed;
        target.revision = self.revision;
        target.region = self.region;
        target.economy = self.economy;
        target.boot = self.boot;
        target.exchange = self.exchange;
        target.construction = self.construction;
        target.town = self.town;
        target.housing = self.housing.clone();
        Ok(())
    }
    pub(crate) fn backing_ids(&self) -> [usize; 7] {
        let a = self.accounts.backing_ids();
        let b = self.stocks.backing_ids();
        [
            a[0],
            a[1],
            b[0],
            b[1],
            self.account_ids.allocation(),
            self.stock_ids.allocation(),
            self.sources.allocation(),
        ]
    }
    pub(crate) fn business_world(&self) -> BusinessWorld<'_> {
        BusinessWorld {
            scope: self.scope,
            seed: self.seed,
            revision: 0,
            accounts: &self.accounts,
            stocks: &self.stocks,
            account_ids: &self.account_ids,
            stock_ids: &self.stock_ids,
            sources: &self.sources,
            region: &self.region,
            economy: &self.economy,
            boot: &self.boot,
            exchange: &self.exchange,
            construction: &self.construction,
            town: &self.town,
            housing: &self.housing,
        }
    }
    pub(crate) fn borrowed_view(&self) -> BorrowedWorldView<'_> {
        BorrowedWorldView {
            schema: if self.housing.is_some() {
                "mycity-m0-state-v8"
            } else if self.construction.is_some() || self.town.is_some() {
                "mycity-m0-state-v7"
            } else {
                "mycity-m0-state-v6"
            },
            scope: self.scope,
            seed: self.seed,
            revision: self.revision,
            accounts: AccountViews(self),
            stocks: StockViews(self),
            opening_sources: &self.sources,
            region: &self.region,
            economy: &self.economy,
            boot: &self.boot,
            exchange: &self.exchange,
            construction: &self.construction,
            town: &self.town,
            housing: &self.housing,
        }
    }
    pub(crate) fn restore_capacity(&mut self) -> Result<(), String> {
        self.accounts
            .restore_capacity(self.scope, MAX_ACCOUNTS as u32, WORLD_BYTE_BUDGET / 2)
            .map_err(|_| "checkpoint_account_arena")?;
        self.stocks
            .restore_capacity(self.scope, MAX_STOCKS as u32, WORLD_BYTE_BUDGET / 2)
            .map_err(|_| "checkpoint_stock_arena")?;
        if self.account_ids.first().is_some_and(|x| x.0 == 0)
            || self.stock_ids.first().is_some_and(|x| x.0 == 0)
            || self.account_ids.windows(2).any(|w| w[0].0 >= w[1].0)
            || self.stock_ids.windows(2).any(|w| w[0].0 >= w[1].0)
            || self.sources.windows(2).any(|w| w[0].id >= w[1].id)
            || self.sources.first().is_some_and(|s| s.id == 0)
            || self.account_ids.len() != self.accounts.len()
            || self.stock_ids.len() != self.stocks.len()
            || self.sources.len() > MAX_SOURCES
        {
            return Err("checkpoint_index_length".into());
        }
        for (i, (id, h)) in self.account_ids.iter().enumerate() {
            let a = self
                .accounts
                .get(*h)
                .map_err(|_| "checkpoint_account_handle")?;
            if a.id != *id
                || a.protected > a.cash
                || (a.opening_source_id != 0
                    && self
                        .opening_source(a.opening_source_id)
                        .is_none_or(|source| source.kind != SourceKind::CashCapital))
                || self.account_ids[..i].iter().any(|(old, _)| old == id)
            {
                return Err("checkpoint_account_index".into());
            }
        }
        for (i, (id, h)) in self.stock_ids.iter().enumerate() {
            let stock = self.stocks.get(*h).map_err(|_| "checkpoint_stock_handle")?;
            self.accounts
                .get(stock.owner)
                .map_err(|_| "checkpoint_stock_owner")?;
            let opening_source = if stock.opening_source_id == 0 {
                None
            } else {
                self.opening_source(stock.opening_source_id)
            };
            if stock.id != *id
                || self.stock_ids[..i].iter().any(|(old, _)| old == id)
                || (stock.opening_source_id != 0
                    && opening_source.is_none_or(|source| {
                        source.kind != SourceKind::InventoryCapital
                            || source.amount != stock.opening_basis
                    }))
            {
                return Err("checkpoint_stock_index".into());
            }
        }
        if self.exchange.is_some()
            && (self.boot.is_none() || self.region.is_none() || self.economy.is_some())
        {
            return Err("checkpoint_runtime_combination".into());
        }
        if let Some(project) = self.construction {
            project
                .validate_persisted()
                .map_err(|_| "checkpoint_construction_profile")?;
            self.account_handle(project.owner)
                .map_err(|_| "checkpoint_construction_owner")?;
            self.account_handle(project.contractor)
                .map_err(|_| "checkpoint_construction_contractor")?;
            self.account_handle(project.inspector)
                .map_err(|_| "checkpoint_construction_inspector")?;
            if project.waste_receiver_account != 0 {
                self.account_handle(project.waste_receiver_account)
                    .map_err(|_| "checkpoint_construction_waste_receiver")?;
            }
            if let Some(contract) = project.compensation {
                self.account_handle(contract.payee)
                    .map_err(|_| "checkpoint_construction_compensation_payee")?;
            }
            for gate in project.gates.iter().take(project.gate_count as usize) {
                self.account_handle(gate.party)
                    .map_err(|_| "checkpoint_construction_gate_party")?;
            }
            for material in project
                .materials
                .iter()
                .take(project.material_count as usize)
            {
                if project.recoverable_commands()
                    && self
                        .boot
                        .as_ref()
                        .is_some_and(|boot| boot.governs(material.stock_id))
                {
                    return Err("checkpoint_construction_governed_stock".into());
                }
                let stock_handle = self
                    .stock_handle(material.stock_id)
                    .map_err(|_| "checkpoint_construction_stock")?;
                let stock = self
                    .stocks
                    .get(stock_handle)
                    .map_err(|_| "checkpoint_construction_stock")?;
                if self
                    .accounts
                    .get(stock.owner)
                    .map_err(|_| "checkpoint_construction_stock_owner")?
                    .id
                    != project.owner
                {
                    return Err("checkpoint_construction_stock_owner".into());
                }
            }
            if let Some(accounting) = project.material_accounting {
                for row in accounting
                    .rows
                    .iter()
                    .take(usize::from(accounting.row_count))
                {
                    let stock = self
                        .stocks
                        .get(
                            self.stock_handle(row.stock_id)
                                .map_err(|_| "checkpoint_construction_material_accounting_stock")?,
                        )
                        .map_err(|_| "checkpoint_construction_material_accounting_stock")?;
                    if self
                        .accounts
                        .get(stock.owner)
                        .map_err(|_| "checkpoint_construction_material_accounting_owner")?
                        .id
                        != project.owner
                    {
                        return Err("checkpoint_construction_material_accounting_owner".into());
                    }
                    if self
                        .boot
                        .as_ref()
                        .is_some_and(|boot| boot.governs(row.stock_id))
                        || (self.exchange.is_some()
                            && (1..=14).contains(&row.stock_id)
                            && !(7..=12).contains(&row.stock_id))
                    {
                        return Err(
                            "checkpoint_construction_material_accounting_governed_stock".into()
                        );
                    }
                }
            }
            if project.utility_contract.is_some() {
                let boot = self
                    .boot
                    .as_ref()
                    .ok_or("checkpoint_construction_utility_boot_missing")?;
                boot.validate_construction_contract(project)
                    .map_err(|_| "checkpoint_construction_utility_boot_binding")?;
                for supply in project
                    .utility_contract
                    .into_iter()
                    .flat_map(|contract| [contract.electricity, contract.water])
                    .flatten()
                {
                    self.account_handle(supply.operator)
                        .map_err(|_| "checkpoint_construction_utility_operator")?;
                }
            }
        }
        if let Some(town) = self.town {
            town.validate_persisted()
                .map_err(|_| "checkpoint_town_profile")?;
            town.validate_world(self)
                .map_err(|_| "checkpoint_town_binding")?;
        }
        if let Some(housing) = self.housing.as_ref() {
            housing
                .opening
                .validate()
                .map_err(|_| "checkpoint_housing_opening")?;
            if housing.state.unit_limit() != usize::from(housing.opening.unit_limit) {
                return Err("checkpoint_housing_limit".into());
            }
            let facts: Vec<_> = housing
                .opening
                .dwellings
                .iter()
                .map(|row| row.fact)
                .collect();
            housing
                .state
                .validate_with_snapshot(&facts)
                .map_err(|_| "checkpoint_housing_snapshot")?;
            for dwelling in housing.opening.dwellings.iter() {
                self.account_handle(dwelling.fact.owner_account)
                    .map_err(|_| "checkpoint_housing_owner")?;
            }
            for household in housing.opening.households.iter() {
                let handle = self
                    .account_handle(household.account)
                    .map_err(|_| "checkpoint_housing_household")?;
                if self
                    .accounts
                    .get(handle)
                    .map_err(|_| "checkpoint_housing_household")?
                    .role
                    != Role::Household
                {
                    return Err("checkpoint_housing_household_role".into());
                }
            }
            validate_housing_town_household_bindings(
                self.town.as_ref().map(|market| &market.profile),
                &housing.opening,
            )
            .map_err(|_| "checkpoint_housing_town_household")?;
            for offer in housing.opening.lease_offers.iter() {
                self.account_handle(offer.owner_account)
                    .map_err(|_| "checkpoint_housing_offer_owner")?;
                self.account_handle(offer.tenant_account)
                    .map_err(|_| "checkpoint_housing_offer_tenant")?;
            }
        }
        let housing_scratch = if self.housing.is_some() {
            crate::housing::MAX_VALIDATION_SCRATCH_BYTES
        } else {
            0
        };
        if self.owned_capacity_bytes().saturating_add(housing_scratch) > WORLD_BYTE_BUDGET {
            return Err("checkpoint_world_budget".into());
        }
        Ok(())
    }
    pub fn account_ref(&self, id: u32) -> Result<crate::arena::RawHandle, String> {
        Ok(self.account_handle(id)?.raw())
    }
    pub fn stock_ref(&self, id: u32) -> Result<crate::arena::RawHandle, String> {
        Ok(self.stock_handle(id)?.raw())
    }
    pub(crate) fn account_handle(&self, id: u32) -> Result<Handle<AccountDomain>, String> {
        self.account_ids
            .binary_search_by_key(&id, |x| x.0)
            .map(|i| self.account_ids[i].1)
            .map_err(|_| "missing_account".into())
    }
    pub(crate) fn stock_handle(&self, id: u32) -> Result<Handle<InventoryDomain>, String> {
        self.stock_ids
            .binary_search_by_key(&id, |x| x.0)
            .map(|i| self.stock_ids[i].1)
            .map_err(|_| "missing_stock".into())
    }
    pub(crate) fn opening_source(&self, id: u32) -> Option<OpeningSource> {
        self.sources.iter().find(|source| source.id == id).copied()
    }
    pub fn view(&self) -> WorldView {
        WorldView {
            schema: if self.housing.is_some() {
                "mycity-m0-state-v8"
            } else if self.construction.is_some() || self.town.is_some() {
                "mycity-m0-state-v7"
            } else {
                "mycity-m0-state-v6"
            },
            scope: self.scope,
            seed: self.seed,
            revision: self.revision,
            accounts: self
                .account_ids
                .iter()
                .map(|(_, h)| {
                    let a = self.accounts.get(*h).expect("validated world");
                    AccountView {
                        id: a.id,
                        role: a.role,
                        cash: a.cash,
                        protected: a.protected,
                        opening_source_id: a.opening_source_id,
                    }
                })
                .collect(),
            stocks: self
                .stock_ids
                .iter()
                .map(|(_, h)| {
                    let s = self.stocks.get(*h).expect("validated world");
                    StockView {
                        id: s.id,
                        owner: self.accounts.get(s.owner).expect("validated owner").id,
                        commodity: s.commodity,
                        quantity: s.quantity,
                        basis: s.basis,
                        lot_id: s.lot_id,
                        origin_quantity: s.origin_quantity,
                        origin_basis: s.origin_basis,
                        offset: s.offset,
                        opening_source_id: s.opening_source_id,
                        opening_basis: s.opening_basis,
                    }
                })
                .collect(),
            opening_sources: self.sources.to_vec(),
            region: self.region,
            economy: self.economy,
            boot: self.boot,
            exchange: self.exchange,
            construction: self.construction,
            town: self.town,
            housing: self.housing.as_deref().map(|housing| HousingView {
                unit_limit: housing.opening.unit_limit,
                dwelling_count: housing.opening.dwellings.len() as u8,
                household_registration_count: housing.opening.households.len() as u8,
                lease_offer_count: housing.opening.lease_offers.len() as u8,
                units: housing.state.units().to_vec(),
                consumed_offer_ids: housing.state.consumed_genesis_offer_ids().collect(),
            }),
        }
    }
    pub fn state_hash(&self) -> String {
        sha256(&serde_json::to_vec(&self.view()).expect("finite exact world"))
    }
    pub(crate) fn owned_capacity_bytes(&self) -> usize {
        self.accounts.owned_bytes()
            + self.stocks.owned_bytes()
            + self.account_ids.capacity() * size_of::<(u32, Handle<AccountDomain>)>()
            + self.stock_ids.capacity() * size_of::<(u32, Handle<InventoryDomain>)>()
            + self.sources.capacity() * size_of::<OpeningSource>()
            + self
                .housing
                .as_ref()
                .map_or(0, |housing| housing.owned_capacity_bytes())
            + size_of::<Self>()
            - size_of::<Arena<AccountDomain, Account>>()
            - size_of::<Arena<InventoryDomain, Stock>>()
    }
    pub fn layout(&self) -> serde_json::Value {
        let index_bytes = self.account_ids.capacity() * size_of::<(u32, Handle<AccountDomain>)>()
            + self.stock_ids.capacity() * size_of::<(u32, Handle<InventoryDomain>)>();
        let source_bytes = self.sources.capacity() * size_of::<OpeningSource>();
        let headers = size_of::<Self>()
            - size_of::<Arena<AccountDomain, Account>>()
            - size_of::<Arena<InventoryDomain, Stock>>();
        let mut layout = serde_json::json!({"accounts":self.accounts.layout(),"stocks":self.stocks.layout(),"index_capacity_bytes":index_bytes,"construction_slot_bytes":size_of::<Option<crate::construction::Project>>(),"town_market_slot_bytes":size_of::<Option<crate::town_runtime::TownMarket>>(),"town_market_slot_included_in_world_remaining_header_bytes":true,"town_profile_owned_bytes":self.town.map_or(0,|town|town.owned_bytes()),"construction_slot_included_in_world_remaining_header_bytes":true,"opening_sources_capacity_bytes":source_bytes,"world_remaining_header_bytes":headers,"owned_capacity_bytes":self.accounts.owned_bytes()+self.stocks.owned_bytes()+index_bytes+source_bytes+headers,"allocator_metadata_included":false,"temporary_parse_and_view_pages_included":false});
        if let Some(housing) = self.housing.as_ref() {
            let object = layout.as_object_mut().expect("layout object");
            object.insert(
                "housing_opening_bytes".into(),
                housing.opening.owned_capacity_bytes().into(),
            );
            object.insert(
                "housing_dwelling_bytes".into(),
                (housing.opening.dwellings.capacity()
                    * size_of::<crate::housing::OpeningDwelling>())
                .into(),
            );
            object.insert(
                "housing_household_registration_bytes".into(),
                (housing.opening.households.capacity()
                    * size_of::<crate::housing::OpeningHousehold>())
                .into(),
            );
            object.insert(
                "housing_lease_offer_bytes".into(),
                (housing.opening.lease_offers.capacity()
                    * size_of::<crate::housing::GenesisLeaseOffer>())
                .into(),
            );
            object.insert(
                "housing_state_bytes".into(),
                housing.state.owned_capacity_bytes().into(),
            );
            object.insert(
                "housing_unit_limit".into(),
                housing.opening.unit_limit.into(),
            );
            object.insert(
                "owned_capacity_bytes".into(),
                self.owned_capacity_bytes().into(),
            );
        }
        layout
    }
}

pub fn assemble(bytes: &[u8]) -> Result<World, String> {
    let mut world = World::restore_empty()?;
    assemble_into(bytes, &mut world)?;
    Ok(world)
}
/// Initial capital only, into an unpublished caller-owned root. Never for restore.
pub(crate) fn assemble_into(bytes: &[u8], world: &mut World) -> Result<(), String> {
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("input_too_large".into());
    }
    crate::bounded_decode::guard(bytes)?;
    let mut input: Opening = serde_json::from_slice(bytes).map_err(|e| {
        if e.to_string().contains("model_rows_extra_record") {
            "opening_count_limit".to_owned()
        } else {
            e.to_string()
        }
    })?;
    if input.schema != ASSEMBLY_SCHEMA {
        return Err("unsupported_opening_schema".into());
    }
    // Share all bootstrap semantics, including the laboratory-only guard.
    input.bootstrap.validate().map_err(str::to_owned)?;
    if input.accounts.is_empty()
        || input.accounts.len() > MAX_ACCOUNTS
        || input.stocks.len() > MAX_STOCKS
        || input.sources.len() > MAX_SOURCES
    {
        return Err("opening_count_limit".into());
    }
    input.accounts.sort_by_key(|a| a.id);
    input.stocks.sort_by_key(|s| s.id);
    input.sources.sort_by_key(|s| s.id);
    fn valid_ids(ids: impl Iterator<Item = u32>) -> bool {
        let mut previous = 0;
        for id in ids {
            if id <= previous {
                return false;
            }
            previous = id;
        }
        true
    }
    if !valid_ids(input.accounts.iter().map(|a| a.id))
        || !valid_ids(input.stocks.iter().map(|a| a.id))
        || !valid_ids(input.sources.iter().map(|a| a.id))
    {
        return Err("duplicate_or_zero_id".into());
    }
    let mut used = [false; MAX_SOURCES];
    let mut consume_source = |id: u32, kind: SourceKind, amount: Money| -> Result<(), String> {
        let index = input
            .sources
            .binary_search_by_key(&id, |s| s.id)
            .map_err(|_| "missing_capital_source")?;
        let src = input.sources[index];
        if used[index] || src.kind != kind || src.amount != amount {
            return Err("capital_source_reused_or_mismatch".into());
        }
        used[index] = true;
        Ok(())
    };
    let mut total = Money::ZERO;
    for a in &input.accounts {
        consume_source(a.source, SourceKind::CashCapital, a.cash)?;
        total = total
            .checked_add(a.cash)
            .map_err(|_| "opening_cash_overflow")?;
    }
    for s in &input.stocks {
        if input
            .accounts
            .binary_search_by_key(&s.owner, |a| a.id)
            .is_err()
        {
            return Err("missing_stock_owner".into());
        }
        if s.quantity == Quantity::ZERO && s.basis != Money::ZERO {
            return Err("cost_without_stock".into());
        }
        consume_source(s.source, SourceKind::InventoryCapital, s.basis)?;
    }
    if used[..input.sources.len()].iter().any(|x| !*x) {
        return Err("unused_capital_source".into());
    }
    let scope =
        Scope::new(input.bootstrap.world, input.bootstrap.branch).map_err(|e| format!("{e:?}"))?;
    world.reset_backing();
    world.scope = scope;
    world.seed = input.bootstrap.seed;
    world.accounts.initialize_scope(scope);
    world.stocks.initialize_scope(scope);
    world.sources.copy_active_from(&input.sources);
    world.region = input
        .region
        .map(crate::region::Region::new)
        .transpose()
        .map_err(|e| format!("region_{e:?}"))?;
    for a in &input.accounts {
        let h = world
            .accounts
            .insert(Account {
                id: a.id,
                role: a.role,
                cash: a.cash,
                protected: Money::ZERO,
                opening_source_id: input
                    .town
                    .as_ref()
                    .filter(|profile| {
                        profile
                            .equity_charters
                            .as_slice()
                            .iter()
                            .any(|charter| charter.company == a.id)
                    })
                    .map_or(0, |_| a.source),
            })
            .map_err(|e| format!("{e:?}"))?;
        world.account_ids.push((a.id, h));
    }
    for s in &input.stocks {
        let owner = world.account_handle(s.owner)?;
        let h = world
            .stocks
            .insert(Stock {
                id: s.id,
                owner,
                commodity: s.commodity,
                quantity: s.quantity,
                basis: s.basis,
                lot_id: s.source,
                origin_quantity: s.quantity,
                origin_basis: s.basis,
                offset: Quantity::ZERO,
                opening_source_id: input
                    .town
                    .as_ref()
                    .filter(|profile| {
                        profile
                            .equity_charters
                            .as_slice()
                            .iter()
                            .any(|charter| charter.company == s.owner)
                    })
                    .map_or(0, |_| s.source),
                opening_basis: input
                    .town
                    .as_ref()
                    .filter(|profile| {
                        profile
                            .equity_charters
                            .as_slice()
                            .iter()
                            .any(|charter| charter.company == s.owner)
                    })
                    .map_or(Money::ZERO, |_| s.basis),
            })
            .map_err(|e| format!("{e:?}"))?;
        world.stock_ids.push((s.id, h));
    }
    if let Some(region) = &world.region {
        for obligation in region.obligations.iter().flatten() {
            world.account_handle(obligation.payer)?;
            world.account_handle(obligation.payee)?;
        }
    }
    if let Some(economy) = input.economy {
        world.economy = Some(crate::economy::Economy::new(economy, world)?);
    }
    if let Some(boot) = input.boot {
        world.boot = Some(crate::boot::Boot::new(
            boot,
            world,
            input.exchange.is_some(),
        )?);
    }
    if let Some(exchange) = input.exchange {
        world.exchange = Some(crate::exchange::Exchange::new(exchange, world)?);
    }
    world.construction = input.construction;
    if let Some(project) = world.construction {
        project
            .validate_profile()
            .map_err(|_| "construction_profile")?;
        world.account_handle(project.owner)?;
        world.account_handle(project.contractor)?;
        world.account_handle(project.inspector)?;
        if project.waste_receiver_account != 0 {
            world.account_handle(project.waste_receiver_account)?;
        }
        if let Some(contract) = project.compensation {
            world.account_handle(contract.payee)?;
        }
        for gate in project.gates.iter().take(project.gate_count as usize) {
            world.account_handle(gate.party)?;
        }
        for material in project
            .materials
            .iter()
            .take(project.material_count as usize)
        {
            if project.recoverable_commands()
                && world
                    .boot
                    .as_ref()
                    .is_some_and(|boot| boot.governs(material.stock_id))
            {
                return Err("construction_governed_stock".into());
            }
            let stock = world.stock_handle(material.stock_id)?;
            let stock = world.stocks.get(stock).map_err(|_| "construction_stock")?;
            if world
                .accounts
                .get(stock.owner)
                .map_err(|_| "construction_stock_owner")?
                .id
                != project.owner
            {
                return Err("construction_stock_owner".into());
            }
            // The genesis rate is only a planning estimate. Runtime COGS is derived from
            // the immutable source-lot offsets by the Engine adapter.
            if stock.origin_quantity.value() == 0 || material.basis_per_unit.value() == 0 {
                return Err("construction_material_basis".into());
            }
        }
        if project.utility_contract.is_some() {
            let boot = world
                .boot
                .as_ref()
                .ok_or("construction_utility_boot_missing")?;
            boot.validate_construction_contract(project)?;
            for supply in project
                .utility_contract
                .into_iter()
                .flat_map(|contract| [contract.electricity, contract.water])
                .flatten()
            {
                world.account_handle(supply.operator)?;
            }
        }
    }
    world.town = input
        .town
        .map(crate::town_runtime::TownMarket::new)
        .transpose()
        .map_err(str::to_owned)?;
    if let Some(town) = world.town {
        town.validate_initial_world(world).map_err(str::to_owned)?;
    }
    if let Some(opening) = input.housing {
        let state = crate::housing::HousingState::from_opening(&opening)
            .map_err(|error| format!("housing_opening:{error:?}"))?;
        for dwelling in opening.dwellings.iter() {
            world.account_handle(dwelling.fact.owner_account)?;
        }
        for offer in opening.lease_offers.iter() {
            world.account_handle(offer.owner_account)?;
            world.account_handle(offer.tenant_account)?;
        }
        for household in opening.households.iter() {
            let handle = world.account_handle(household.account)?;
            if world
                .accounts
                .get(handle)
                .map_err(|_| "housing_household_account")?
                .role
                != Role::Household
            {
                return Err("housing_household_role".into());
            }
        }
        validate_housing_town_household_bindings(
            world.town.as_ref().map(|market| &market.profile),
            &opening,
        )?;
        world.housing = Some(Box::new(HousingRuntime { opening, state }));
    }
    let housing_scratch = if world.housing.is_some() {
        crate::housing::MAX_VALIDATION_SCRATCH_BYTES
    } else {
        0
    };
    if world.owned_capacity_bytes().saturating_add(housing_scratch) > WORLD_BYTE_BUDGET {
        return Err("world_byte_limit".into());
    }
    // Exercise the same checked ID index used by subsequent model commands.
    for (id, _) in &world.stock_ids {
        world.stock_handle(*id)?;
    }
    Ok(())
}

fn validate_housing_town_household_bindings(
    profile: Option<&crate::town_runtime::MarketProfile>,
    opening: &crate::housing::OpeningHousing,
) -> Result<(), String> {
    let Some(profile) = profile else {
        return Ok(());
    };
    for household in opening.households.iter() {
        let groups = profile.households.as_slice();
        let id_match = groups
            .iter()
            .find(|group| group.id == household.household_id);
        let account_match = groups
            .iter()
            .find(|group| group.account == household.account);
        if id_match.is_some_and(|group| group.account != household.account)
            || account_match.is_some_and(|group| group.id != household.household_id)
        {
            return Err("housing_town_household_account".into());
        }
    }
    Ok(())
}

impl crate::restore_into::Restore for World {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        // Pre-construction checkpoints have no field 12. Clear the reused slot first so
        // restoring such a checkpoint over a world with a project cannot retain stale state.
        self.construction = None;
        self.town = None;
        self.housing = None;
        crate::restore_into::object!(d,self,World,255u64,{
        scope,0=>Copy,
        seed,1=>Copy,
        revision,2=>Copy,
        accounts,3=>Into,
        stocks,4=>Into,
        account_ids,5=>Into,
        stock_ids,6=>Into,
        sources,7=>Into,
        region,8=>Copy,
        economy,9=>Copy,
        boot,10=>Copy,
        exchange,11=>Copy,
        construction,12=>Copy,
        town,13=>Copy,
        housing,14=>Into,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const INPUT: &[u8] = include_bytes!("../../../fixtures/m0-e0-opening.json");

    fn project() -> crate::construction::Project {
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

    fn town_profile() -> crate::town_runtime::MarketProfile {
        use crate::town_economy::{DeliveryQuote, HouseholdGroup, ShopSnapshot};
        use crate::town_runtime::{EmployerSpec, FixedRows};
        let mut households = FixedRows::new();
        households
            .push(HouseholdGroup {
                id: 1,
                account: 2,
                households: 1,
                need_good: 0,
                need_per_household: Quantity::new(1).unwrap(),
                reference_price_per_q: Money::new(1).unwrap(),
            })
            .unwrap();
        let mut employers = FixedRows::new();
        employers
            .push(EmployerSpec {
                account: 3,
                worker_limit: 1,
            })
            .unwrap();
        let mut shops = FixedRows::new();
        shops
            .push(ShopSnapshot {
                shop_id: 1,
                account: 3,
                stock_id: 1,
                good: 0,
                shelf_price_per_q: Money::new(1).unwrap(),
            })
            .unwrap();
        let mut delivery = FixedRows::new();
        delivery
            .push(DeliveryQuote {
                quote_id: 1,
                shop: 1,
                household_group: 1,
                good: 0,
                reachable: true,
                route_source: 1,
                freight_account: 3,
                utility_account: 3,
                available_capacity: Quantity::new(1).unwrap(),
                freight_per_q: Money::ZERO,
                utility_surcharge_per_q: Money::ZERO,
            })
            .unwrap();
        crate::town_runtime::MarketProfile {
            schema_version: 1,
            id: 1,
            transport_allocation_sha256: [0; 32],
            utility_supply: None,
            exit_policy: None,
            equity_charters: FixedRows::new(),
            tax_rules: FixedRows::new(),
            households,
            employers,
            shops,
            recipes: FixedRows::new(),
            delivery,
        }
    }

    #[test]
    fn restore_of_preconstruction_object_clears_reused_optional_slot() {
        let mut reused = assemble(INPUT).unwrap();
        reused.construction = Some(project());

        // This synthetic object tests only the reused-slot defaulting invariant. The
        // authentic v106 Store/WAL compatibility test uses its unmodified archived bytes.
        let fresh = assemble(INPUT).unwrap();
        assert_eq!(fresh.view().schema, "mycity-m0-state-v6");
        assert!(!serde_json::to_value(fresh.view())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("construction"));
        assert!(!serde_json::to_value(fresh.business_world())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("construction"));
        assert!(!serde_json::to_value(fresh.borrowed_view())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("construction"));
        let mut legacy = serde_json::to_value(fresh).unwrap();
        assert!(legacy
            .as_object_mut()
            .unwrap()
            .remove("construction")
            .is_none());
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
        crate::restore_into::Restore::restore(&mut reused, &mut deserializer).unwrap();
        assert!(reused.construction.is_none());
    }

    #[test]
    fn town_configuration_is_omitted_when_absent_and_legacy_restore_clears_reused_slot() {
        let mut world = assemble(INPUT).unwrap();
        assert_eq!(world.view().schema, "mycity-m0-state-v6");
        assert!(!serde_json::to_value(world.view())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("town"));
        assert!(!serde_json::to_value(world.business_world())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("town"));

        world.town = Some(crate::town_runtime::TownMarket::new(town_profile()).unwrap());
        let mut legacy = serde_json::to_value(assemble(INPUT).unwrap()).unwrap();
        assert!(legacy.as_object_mut().unwrap().remove("town").is_none());
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
        crate::restore_into::Restore::restore(&mut world, &mut deserializer).unwrap();
        assert!(world.town.is_none());
    }

    #[test]
    fn sources_and_independent_initial_worlds_are_exact() {
        let w = assemble(INPUT).unwrap();
        assert_eq!(
            w.view()
                .accounts
                .iter()
                .map(|a| a.cash.value())
                .sum::<i64>(),
            3_000_000
        );
        let v = w.view();
        assert_eq!(v.stocks[0].quantity.value(), 600_000);
        assert_eq!(v.stocks[0].basis.value(), 240_000);
        assert_eq!(w.state_hash(), assemble(INPUT).unwrap().state_hash());
        assert!(w.layout()["owned_capacity_bytes"].as_u64().unwrap() <= WORLD_BYTE_BUDGET as u64);
    }
    #[test]
    fn duplicate_sources_missing_owners_and_cost_mismatches_reject() {
        for f in [0, 1, 2, 3, 4] {
            let mut v: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
            match f {
                0 => v["accounts"][1]["source"] = serde_json::json!(1),
                1 => v["stocks"][0]["owner"] = serde_json::json!(999),
                2 => v["stocks"][0]["basis"] = serde_json::json!(0),
                3 => v["accounts"][1]["id"] = serde_json::json!(1),
                _ => v["bootstrap"]["laboratory_only"] = serde_json::json!(false),
            }
            assert!(
                assemble(&serde_json::to_vec(&v).unwrap()).is_err(),
                "case {f}"
            );
        }
    }
    #[test]
    fn input_order_does_not_change_world_identity_or_hash() {
        let mut v: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
        v["accounts"].as_array_mut().unwrap().reverse();
        v["sources"].as_array_mut().unwrap().reverse();
        assert_eq!(
            assemble(INPUT).unwrap().state_hash(),
            assemble(&serde_json::to_vec(&v).unwrap())
                .unwrap()
                .state_hash()
        );
    }

    #[test]
    fn oversized_counts_and_unspent_initial_sources_do_not_publish_a_world() {
        let mut v: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
        let row = v["accounts"][0].clone();
        v["accounts"] = serde_json::json!(vec![row; MAX_ACCOUNTS + 1]);
        assert!(
            matches!(assemble(&serde_json::to_vec(&v).unwrap()), Err(e) if e=="opening_count_limit")
        );
        let mut v: serde_json::Value = serde_json::from_slice(INPUT).unwrap();
        v["sources"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":6,"kind":"cash_capital","amount":1}));
        assert!(
            matches!(assemble(&serde_json::to_vec(&v).unwrap()), Err(e) if e=="unused_capital_source")
        );
    }
}
