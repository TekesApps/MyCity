//! Finite typed genesis input. No persistent directory, second World, or converted JSON.
//! Only the original EngineSlot resource context may accept this schema.
use super::*;
use crate::transaction::{Resource, MAX_RESOURCES};

pub(super) const SCHEMA: &str = "mycity-typed-opening-v1";

#[derive(Deserialize)]
struct SchemaProbe {
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    schema: String,
}
pub(super) fn selected(bytes: &[u8]) -> bool {
    serde_json::from_slice::<SchemaProbe>(bytes).is_ok_and(|p| p.schema == SCHEMA)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityScope {
    world: u64,
    branch: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Domain {
    Account,
    Stock,
    CashCapital,
    InventoryCapital,
    Resource,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    scope: IdentityScope,
    domain: Domain,
    id: u32,
    version: u32,
}
impl Reference {
    fn resolve(
        self,
        scope: IdentityScope,
        domain: Domain,
        version: Option<u32>,
    ) -> Result<u32, String> {
        if self.scope != scope || self.domain != domain || self.id == 0 {
            return Err("typed_opening_reference_scope_or_domain".into());
        }
        if self.version != 1 || version != Some(self.version) {
            return Err("typed_opening_reference_missing_or_version".into());
        }
        Ok(self.id)
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MoneyUnit {
    Cent,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cash {
    value: Money,
    unit: MoneyUnit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QuantityUnit {
    MilliQ,
    MilliWh,
    Millilitre,
}
fn quantity_unit(c: Commodity) -> QuantityUnit {
    match c {
        Commodity::Raw | Commodity::Product | Commodity::Fuel => QuantityUnit::MilliQ,
        Commodity::Electricity | Commodity::Heat => QuantityUnit::MilliWh,
        Commodity::Water | Commodity::RawWater | Commodity::WasteWater => QuantityUnit::Millilitre,
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Physical {
    value: Quantity,
    unit: QuantityUnit,
}
impl Physical {
    fn resolve(self, commodity: Commodity) -> Result<Quantity, String> {
        if self.unit != quantity_unit(commodity) {
            return Err("typed_opening_commodity_unit".into());
        }
        Ok(self.value)
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: u32,
    version: u32,
    kind: SourceKind,
    amount: Cash,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    id: u32,
    version: u32,
    role: Role,
    cash: Cash,
    source: Reference,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stock {
    id: u32,
    version: u32,
    owner: Reference,
    commodity: Commodity,
    quantity: Physical,
    basis: Cash,
    source: Reference,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CapacityUnit {
    Count,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceInput {
    id: u32,
    version: u32,
    capacity: u32,
    unit: CapacityUnit,
}

// Rows allocate their full fixed backing before accepting values; these private fills
// are never active records or defaults for omitted input fields.
const INVALID_REF: Reference = Reference {
    scope: IdentityScope {
        world: 0,
        branch: 0,
    },
    domain: Domain::Account,
    id: 0,
    version: 0,
};
const ZERO_CASH: Cash = Cash {
    value: Money::ZERO,
    unit: MoneyUnit::Cent,
};
impl Record for Source {
    fn inactive() -> Self {
        Self {
            id: 0,
            version: 0,
            kind: SourceKind::CashCapital,
            amount: ZERO_CASH,
        }
    }
}
impl Record for Account {
    fn inactive() -> Self {
        Self {
            id: 0,
            version: 0,
            role: Role::Treasury,
            cash: ZERO_CASH,
            source: INVALID_REF,
        }
    }
}
impl Record for Stock {
    fn inactive() -> Self {
        Self {
            id: 0,
            version: 0,
            owner: INVALID_REF,
            commodity: Commodity::Raw,
            quantity: Physical {
                value: Quantity::ZERO,
                unit: QuantityUnit::MilliQ,
            },
            basis: ZERO_CASH,
            source: INVALID_REF,
        }
    }
}
impl Record for ResourceInput {
    fn inactive() -> Self {
        Self {
            id: 0,
            version: 0,
            capacity: 0,
            unit: CapacityUnit::Count,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provider {
    kind: ProviderKind,
    version: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProviderKind {
    EmbeddedLaboratory,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Clock {
    None,
    BootDiscrete {
        scope: IdentityScope,
        version: u32,
        unit: TimeUnit,
        initial: u64,
    },
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TimeUnit {
    Second,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Profile {
    Base {
        version: u32,
    },
    BootMicro {
        version: u32,
        operator: Reference,
        contractor: Reference,
        stocks: [Reference; 6],
        resources: [Reference; 2],
        limits: [Physical; 6],
    },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypedOpening {
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    schema: String,
    provider: Provider,
    bootstrap: Bootstrap,
    sources: Rows<Source, MAX_SOURCES>,
    accounts: Rows<Account, MAX_ACCOUNTS>,
    stocks: Rows<Stock, MAX_STOCKS>,
    resources: Rows<ResourceInput, MAX_RESOURCES>,
    clock: Clock,
    profile: Profile,
}

// Conservative simultaneous logical allocation bound: fixed typed and original
// Rows backings, both conversion headers, Boot, bounded strings and parser
// scratch. Borrowed original bytes remain loader/caller or Store metadata-owned,
// with actual capacity counted there, never inferred from the slice length. The parsed typed owner is dropped before the original large builder.
// This is not a native stack or physical-process peak claim.
pub(super) const WORKSPACE_BYTES: usize = 2 * size_of::<TypedOpening>()
    + size_of::<Opening>()
    + MAX_SOURCES * (size_of::<Source>() + size_of::<OpeningSource>())
    + MAX_ACCOUNTS * (size_of::<Account>() + size_of::<OpeningAccount>())
    + MAX_STOCKS * (size_of::<Stock>() + size_of::<OpeningStock>())
    + MAX_RESOURCES * size_of::<ResourceInput>()
    + size_of::<crate::boot::Opening>()
    + 512
    + INPUT_LIMIT as usize;

#[inline(never)]
pub(super) fn decode(bytes: &[u8], actual: &[Resource]) -> Result<Opening, String> {
    let input: Box<TypedOpening> = serde_json::from_slice(bytes).map_err(|e| {
        if e.to_string().contains("model_rows_extra_record") {
            "opening_count_limit".into()
        } else {
            e.to_string()
        }
    })?;
    if input.schema != SCHEMA || input.provider.version != 1 {
        return Err("typed_opening_schema_or_provider_version".into());
    }
    let _transport_contract = &input.provider.kind;
    input.bootstrap.validate().map_err(str::to_owned)?;
    let scope = IdentityScope {
        world: input.bootstrap.world,
        branch: input.bootstrap.branch,
    };
    if input.sources.iter().any(|r| r.version != 1)
        || input.accounts.iter().any(|r| r.version != 1)
        || input.stocks.iter().any(|r| r.version != 1)
        || input.resources.iter().any(|r| r.version != 1)
    {
        return Err("typed_opening_catalog_version".into());
    }
    // Full external resource equality; no merge, new defaults, or occupancy import.
    if actual.len() != input.resources.len()
        || actual.len() > MAX_RESOURCES
        || actual
            .iter()
            .any(|r| r.id == 0 || r.reserved != 0 || r.occupied != 0)
        || actual
            .iter()
            .enumerate()
            .any(|(i, r)| actual[..i].iter().any(|s| s.id == r.id))
        || input.resources.iter().enumerate().any(|(i, r)| {
            r.id == 0
                || input.resources[..i].iter().any(|s| s.id == r.id)
                || !actual
                    .iter()
                    .any(|s| s.id == r.id && s.capacity == r.capacity)
        })
    {
        return Err("typed_opening_resource_context".into());
    }
    for resource in &input.resources {
        let _integer_count_contract = resource.unit;
    }
    let source_ref = |reference: Reference, kind: SourceKind| {
        let domain = match kind {
            SourceKind::CashCapital => Domain::CashCapital,
            SourceKind::InventoryCapital => Domain::InventoryCapital,
        };
        let version = input
            .sources
            .iter()
            .find(|s| s.id == reference.id && s.kind == kind)
            .map(|s| s.version);
        reference.resolve(scope, domain, version)
    };
    let account_ref = |reference: Reference| {
        reference.resolve(
            scope,
            Domain::Account,
            input
                .accounts
                .iter()
                .find(|s| s.id == reference.id)
                .map(|s| s.version),
        )
    };
    let stock_ref = |reference: Reference| {
        reference.resolve(
            scope,
            Domain::Stock,
            input
                .stocks
                .iter()
                .find(|s| s.id == reference.id)
                .map(|s| s.version),
        )
    };
    let resource_ref = |reference: Reference| {
        reference.resolve(
            scope,
            Domain::Resource,
            input
                .resources
                .iter()
                .find(|s| s.id == reference.id)
                .map(|s| s.version),
        )
    };
    let mut sources = Rows::new().map_err(str::to_owned)?;
    for s in &input.sources {
        let _unit = s.amount.unit;
        sources.push(OpeningSource {
            id: s.id,
            kind: s.kind,
            amount: s.amount.value,
        });
    }
    let mut accounts = Rows::new().map_err(str::to_owned)?;
    for a in &input.accounts {
        let _unit = a.cash.unit;
        accounts.push(OpeningAccount {
            id: a.id,
            role: a.role,
            cash: a.cash.value,
            source: source_ref(a.source, SourceKind::CashCapital)?,
        });
    }
    let mut stocks = Rows::new().map_err(str::to_owned)?;
    for s in &input.stocks {
        let _unit = s.basis.unit;
        stocks.push(OpeningStock {
            id: s.id,
            owner: account_ref(s.owner)?,
            commodity: s.commodity,
            quantity: s.quantity.resolve(s.commodity)?,
            basis: s.basis.value,
            source: source_ref(s.source, SourceKind::InventoryCapital)?,
        });
    }
    let boot = match &input.profile {
        Profile::Base { version: 1 } if matches!(input.clock, Clock::None) => None,
        Profile::BootMicro {
            version: 1,
            operator,
            contractor,
            stocks: ports,
            resources,
            limits,
        } => {
            if !matches!(input.clock,Clock::BootDiscrete{scope:s,version:1,unit:TimeUnit::Second,initial:0} if s==scope)
            {
                return Err("typed_opening_boot_clock".into());
            }
            let mut stock_ids = [0; 6];
            let mut resource_ids = [0; 2];
            let mut amounts = [Quantity::ZERO; 6];
            for i in 0..6 {
                stock_ids[i] = stock_ref(ports[i])?;
                let commodity = input
                    .stocks
                    .iter()
                    .find(|s| s.id == stock_ids[i])
                    .ok_or("typed_opening_boot_stock")?
                    .commodity;
                amounts[i] = limits[i].resolve(commodity)?;
            }
            for i in 0..2 {
                resource_ids[i] = resource_ref(resources[i])?;
            }
            Some(crate::boot::Opening {
                schema: "mycity-m0-boot-v1".into(),
                profile: crate::boot::PROFILE.into(),
                operator: account_ref(*operator)?,
                contractor: account_ref(*contractor)?,
                stocks: stock_ids,
                resources: resource_ids,
                limits: amounts,
            })
        }
        _ => return Err("typed_opening_profile_or_clock".into()),
    };
    // No converted byte buffer. The original builder still owns all capital, amount,
    // Boot semantics, World layout and population of runtime typed handles.
    let TypedOpening { bootstrap, .. } = *input;
    Ok(Opening {
        schema: ASSEMBLY_SCHEMA.into(),
        bootstrap,
        sources,
        accounts,
        stocks,
        boot,
        region: None,
        economy: None,
        exchange: None,
        construction: None,
        town: None,
        housing: None,
        venue: None,
        education: None,
        qualification: None,
        member_activity: None,
        service_needs: None,
        continuous_supply: None,
        facility_work: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_typed_conversion_backing_is_explicitly_inside_original_engine_budget() {
        let slot = crate::transaction::EngineSlot::new().unwrap();
        let retained = slot.identity().unwrap().owned_model_bytes;
        assert!(retained + WORKSPACE_BYTES <= crate::transaction::ENGINE_BYTE_BUDGET);
        println!("typed_workspace_bytes={WORKSPACE_BYTES}; original_slot_owned={retained}; input_capacity_owned_by_loader_or_metadata=true");
    }
}
