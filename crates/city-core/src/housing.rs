//! Bounded residential rights and lease planning.
//!
//! The housing table stores one record per *constructed dwelling unit*, not per person.
//! Geometry, room count, legal access, and person capacity remain owned by the building and
//! spatial models and enter through `DwellingFact`. Account balances and rent debt remain owned
//! by the shared ledger; the plans below describe effects but never settle them.
//! `MAX_DWELLING_UNITS` is a design ceiling, not a per-world allocation or a measured capacity
//! claim. Each world must open this table with an explicit stock-derived limit.

use crate::{
    calendar::Date,
    model_rows::{Record, Rows},
    timeline::Cycle,
    town_economy::AccountSnapshot,
    units::{GameTime, Money},
};
use serde::{de, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::mem::size_of;

pub const MAX_DWELLING_UNITS: usize = 32_768;
/// Small first residential transaction prototype. This is not the frozen full M0 load limit.
pub const MAX_M0_HOUSING_UNITS: usize = 32;
/// A genesis offer can be accepted at most once and is retained in the bounded housing state.
/// This is a prototype cap tied to actual configured dwelling stock, not an M0 load target.
pub const MAX_GENESIS_LEASE_OFFERS: usize = MAX_M0_HOUSING_UNITS;
pub const MAX_MONTHLY_INTERVAL_SECONDS: u64 = 31 * 86_400;
/// Conservative peak heap scratch for opening/restore validation: one dwelling fact, one source
/// ID and three identity/capacity keys per configured unit. It is transient, not persistent state.
pub const MAX_VALIDATION_SCRATCH_BYTES: usize =
    MAX_M0_HOUSING_UNITS * (size_of::<DwellingFact>() + 4 * size_of::<u64>());
pub const MAX_MOVE_CANDIDATES: usize = 16;
pub const LOW_SATISFACTION_DAYS: u64 = 14;
pub const MOVE_COOLDOWN_DAYS: u64 = 30;
pub const SOFT_RENT_SHARE_PERCENT: u64 = 35;
pub const LOW_SATISFACTION_THRESHOLD: u8 = 35;
pub const MIN_MOVE_IMPROVEMENT_POINTS: u8 = 5;

/// One occupied or vacant physical unit. This is the entire persistent housing row: the
/// 64-byte target does not include geometry or capacity, which belong to the building model.
/// `arrears_ref` is the lease key used to resolve rent debt in the shared liability ledger; it
/// is never a duplicate debt balance. A zero ref means no current rent debt. Rent is due at a
/// calendar-month anchor and must be advanced by the existing Timeline/Cycle scheduler.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HousingUnit {
    pub unit_id: u32,
    pub building_id: u32,
    pub owner_account: u32,
    /// Zero means vacant. A household is atomic here; its people are not separate records.
    pub household_id: u32,
    /// Stable active lease identity; zero for a vacancy or owner-occupied unit.
    pub lease_id: u32,
    /// Shared-ledger identity for unpaid rent; zero when no debt exists.
    pub arrears_ref: u32,
    pub clause_version: u32,
    /// Calendar monthly rent anchor (1..=31); zero for a vacancy.
    pub billing_anchor: u32,
    pub rent_per_cycle: Money,
    /// Escrow liability summary; the actual restricted balance remains in the shared ledger.
    pub deposit_held: Money,
    /// Rent has been accounted through this instant; next due uses calendar-month boundaries.
    pub last_accounted_at: GameTime,
    pub next_due_at: GameTime,
}

impl Record for HousingUnit {
    fn inactive() -> Self {
        Self::default()
    }
}

/// The only accepted source of a dwelling's physical and legal suitability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DwellingFact {
    pub unit_id: u32,
    pub building_id: u32,
    pub owner_account: u32,
    pub building_unit_capacity: u16,
    pub person_capacity: u16,
    pub delivered: bool,
    pub legal_to_occupy: bool,
    pub has_legal_access: bool,
    /// Version of the building/spatial facts used to evaluate this offer.
    pub source_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DwellingOriginKind {
    /// A trusted immutable genesis registration, not a construction completion receipt.
    ImmutableGenesisEndowment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningDwelling {
    pub fact: DwellingFact,
    pub origin: DwellingOriginKind,
    /// Identifies the host-supplied genesis asset entry that registered the dwelling.
    pub asset_source_id: u64,
}

impl Record for OpeningDwelling {
    fn inactive() -> Self {
        Self {
            fact: DwellingFact {
                unit_id: 0,
                building_id: 0,
                owner_account: 0,
                building_unit_capacity: 0,
                person_capacity: 0,
                delivered: false,
                legal_to_occupy: false,
                has_legal_access: false,
                source_version: 0,
            },
            origin: DwellingOriginKind::ImmutableGenesisEndowment,
            asset_source_id: 0,
        }
    }
}

/// Bounded genesis registration. The host inputs are trusted registration facts, not evidence
/// that construction ran or that an external permit process was automatically approved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningHousing {
    pub unit_limit: u8,
    pub dwellings: Rows<OpeningDwelling, MAX_M0_HOUSING_UNITS>,
    /// Host-supplied bounded identity registrations used by offers. These are explicit opening
    /// facts; they do not claim to discover households from account IDs.
    #[serde(default = "empty_opening_households")]
    pub households: Rows<OpeningHousehold, MAX_M0_HOUSING_UNITS>,
    /// Immutable host-supplied one-shot opening offers; runtime lease actions select by ID and
    /// cannot provide or replace these records.
    pub lease_offers: Rows<GenesisLeaseOffer, MAX_GENESIS_LEASE_OFFERS>,
}

fn empty_opening_households() -> Rows<OpeningHousehold, MAX_M0_HOUSING_UNITS> {
    Rows::new().expect("fixed opening household rows")
}

/// A bounded host registration linking a household identity to its account and member count.
/// `source_id` identifies the explicit genesis registration and is not a guessed account alias.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningHousehold {
    pub household_id: u32,
    pub account: u32,
    pub people: u16,
    pub source_id: u64,
}

impl Record for OpeningHousehold {
    fn inactive() -> Self {
        Self {
            household_id: 0,
            account: 0,
            people: 0,
            source_id: 0,
        }
    }
}

/// One immutable host-supplied lease offer issued from an opening owner/endowment registration.
/// It is not a runtime signature, a dynamic landlord workflow, or an authorization derived from
/// caller-provided IDs. It carries executable terms so Engine actions need only select an offer.
/// At most 32 rows may be supplied.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisLeaseOffer {
    pub offer_id: u64,
    pub world_id: u64,
    pub branch_id: u64,
    pub unit_id: u32,
    pub building_id: u32,
    pub owner_account: u32,
    pub tenant_account: u32,
    pub household_id: u32,
    pub valid_from: GameTime,
    pub valid_until: GameTime,
    pub asset_source_id: u64,
    pub authority_source_id: u64,
    pub contract_source_id: u64,
    pub landlord_authorization_id: u64,
    pub tenant_consent_id: u64,
    pub lease_id: u32,
    pub clause_version: u32,
    pub rent_per_cycle: Money,
    pub cycle: Cycle,
    pub deposit: Money,
    pub escrow_account: u32,
    pub starts_at: GameTime,
    pub terms_sha256: [u8; 32],
}

impl Record for GenesisLeaseOffer {
    fn inactive() -> Self {
        Self {
            offer_id: 0,
            world_id: 0,
            branch_id: 0,
            unit_id: 0,
            building_id: 0,
            owner_account: 0,
            tenant_account: 0,
            household_id: 0,
            valid_from: GameTime::ZERO,
            valid_until: GameTime::ZERO,
            asset_source_id: 0,
            authority_source_id: 0,
            contract_source_id: 0,
            landlord_authorization_id: 0,
            tenant_consent_id: 0,
            lease_id: 0,
            clause_version: 0,
            rent_per_cycle: Money::ZERO,
            cycle: Cycle::Monthly { anchor: 1 },
            deposit: Money::ZERO,
            escrow_account: 0,
            starts_at: GameTime::ZERO,
            terms_sha256: [0; 32],
        }
    }
}

impl GenesisLeaseOffer {
    pub fn executable_terms(self) -> LeaseTerms {
        LeaseTerms {
            lease_id: self.lease_id,
            clause_version: self.clause_version,
            contract_source_id: self.contract_source_id,
            landlord_authorization_id: self.landlord_authorization_id,
            tenant_consent_id: self.tenant_consent_id,
            rent_per_cycle: self.rent_per_cycle,
            cycle: self.cycle,
            deposit: self.deposit,
            escrow_account: self.escrow_account,
            starts_at: self.starts_at,
        }
    }
}

/// Opaque proof produced only by `validate_genesis_lease_offer`. Engine must additionally
/// verify the real transaction caller and resolve all current snapshots before using it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenesisLeaseAuthorization {
    offer_id: u64,
    world_id: u64,
    branch_id: u64,
    unit_id: u32,
    building_id: u32,
    owner_account: u32,
    tenant_account: u32,
    household_id: u32,
    people: u16,
    offer_sha256: [u8; 32],
    terms_sha256: [u8; 32],
}

impl GenesisLeaseAuthorization {
    pub fn offer_id(self) -> u64 {
        self.offer_id
    }
    pub fn offer_sha256(self) -> [u8; 32] {
        self.offer_sha256
    }
    pub fn world_id(self) -> u64 {
        self.world_id
    }
    pub fn branch_id(self) -> u64 {
        self.branch_id
    }
    pub fn tenant_account(self) -> u32 {
        self.tenant_account
    }
    pub fn household_id(self) -> u32 {
        self.household_id
    }
    pub fn people(self) -> u16 {
        self.people
    }
    pub fn owner_account(self) -> u32 {
        self.owner_account
    }
    pub fn unit_id(self) -> u32 {
        self.unit_id
    }
    pub fn building_id(self) -> u32 {
        self.building_id
    }
}

impl OpeningHousing {
    /// Bytes owned by the bounded immutable genesis snapshot, excluding allocator metadata.
    pub fn owned_capacity_bytes(&self) -> usize {
        self.dwellings.capacity() * size_of::<OpeningDwelling>()
            + self.households.capacity() * size_of::<OpeningHousehold>()
            + self.lease_offers.capacity() * size_of::<GenesisLeaseOffer>()
    }

    pub fn household(&self, household_id: u32) -> Result<OpeningHousehold, HousingError> {
        self.households
            .binary_search_by_key(&household_id, |household| household.household_id)
            .map(|index| self.households[index])
            .map_err(|_| HousingError::InvalidIdentity)
    }

    pub fn validate(&self) -> Result<(), HousingError> {
        self.validate_lease_offer_catalog()
    }

    /// Returns a copy of an immutable opening offer. Runtime commands carry only this ID; the
    /// Engine must resolve the record from its already-validated opening snapshot.
    pub fn lease_offer(&self, offer_id: u64) -> Result<GenesisLeaseOffer, HousingError> {
        self.validate_lease_offer_catalog()?;
        self.lease_offers
            .binary_search_by_key(&offer_id, |offer| offer.offer_id)
            .map(|index| self.lease_offers[index])
            .map_err(|_| HousingError::UnknownOffer)
    }

    fn validate_lease_offer_catalog(&self) -> Result<(), HousingError> {
        if self.unit_limit == 0
            || usize::from(self.unit_limit) > MAX_M0_HOUSING_UNITS
            || self.dwellings.len() != usize::from(self.unit_limit)
            || self
                .dwellings
                .windows(2)
                .any(|pair| pair[0].fact.unit_id >= pair[1].fact.unit_id)
            || self.lease_offers.len() > usize::from(self.unit_limit)
            || self.households.len() > usize::from(self.unit_limit)
            || self
                .lease_offers
                .windows(2)
                .any(|pair| pair[0].offer_id >= pair[1].offer_id)
        {
            return Err(HousingError::DuplicateIdentity);
        }
        for (index, dwelling) in self.dwellings.iter().enumerate() {
            validate_dwelling(dwelling.fact)?;
            if dwelling.origin != DwellingOriginKind::ImmutableGenesisEndowment
                || dwelling.asset_source_id == 0
                || self.dwellings[..index]
                    .iter()
                    .any(|prior| prior.asset_source_id == dwelling.asset_source_id)
                || self
                    .households
                    .iter()
                    .any(|household| household.source_id == dwelling.asset_source_id)
            {
                return Err(HousingError::InvalidDwelling);
            }
        }
        if self
            .households
            .windows(2)
            .any(|pair| pair[0].household_id >= pair[1].household_id)
        {
            return Err(HousingError::DuplicateIdentity);
        }
        for (index, household) in self.households.iter().enumerate() {
            if household.household_id == 0
                || household.account == 0
                || household.people == 0
                || household.source_id == 0
                || self.households[..index].iter().any(|prior| {
                    prior.account == household.account || prior.source_id == household.source_id
                })
                || self
                    .dwellings
                    .iter()
                    .any(|dwelling| dwelling.asset_source_id == household.source_id)
            {
                return Err(HousingError::InvalidIdentity);
            }
        }
        for offer in &self.lease_offers {
            if offer.offer_id == 0
                || offer.world_id == 0
                || offer.branch_id == 0
                || offer.unit_id == 0
                || offer.building_id == 0
                || offer.owner_account == 0
                || offer.tenant_account == 0
                || offer.owner_account == offer.tenant_account
                || offer.household_id == 0
                || offer.asset_source_id == 0
                || offer.authority_source_id == 0
                || offer.contract_source_id == 0
                || offer.landlord_authorization_id == 0
                || offer.tenant_consent_id == 0
                || offer.lease_id == 0
                || offer.clause_version == 0
                || offer.escrow_account == 0
                || offer.starts_at < offer.valid_from
                || offer.starts_at > offer.valid_until
                || offer.terms_sha256 == [0; 32]
                || offer.valid_from > offer.valid_until
                || Date::from_time(offer.valid_from).is_err()
                || Date::from_time(offer.valid_until).is_err()
                || Date::from_time(offer.starts_at).is_err()
                || lease_terms_sha256(offer.executable_terms()).ok() != Some(offer.terms_sha256)
            {
                return Err(HousingError::InvalidTerms);
            }
            let registered = self
                .dwellings
                .binary_search_by_key(&offer.unit_id, |dwelling| dwelling.fact.unit_id)
                .ok()
                .map(|index| self.dwellings[index])
                .ok_or(HousingError::InvalidDwelling)?;
            if registered.origin != DwellingOriginKind::ImmutableGenesisEndowment
                || registered.asset_source_id != offer.asset_source_id
                || registered.fact.building_id != offer.building_id
                || registered.fact.owner_account != offer.owner_account
            {
                return Err(HousingError::OfferMismatch);
            }
            let household = self.household(offer.household_id)?;
            if household.account != offer.tenant_account
                || household.people > registered.fact.person_capacity
            {
                return Err(HousingError::OfferMismatch);
            }
        }
        Ok(())
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct ConsumedOfferId(u64);

impl Record for ConsumedOfferId {
    fn inactive() -> Self {
        Self(0)
    }
}

/// Bounded housing-domain request types. The Engine lease path accepts the compact
/// `Action::HousingLease` selector and derives executable terms from an immutable opening offer;
/// these wider values remain planner inputs, never caller-authorized transaction fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Lease {
        /// Exact one-shot record from immutable `OpeningHousing`; caller cannot synthesize it.
        offer_id: u64,
        unit_id: u32,
        household_id: u32,
        lease_id: u32,
        clause_version: u32,
        rent_per_cycle: Money,
        cycle: Cycle,
        deposit: Money,
        escrow_account: u32,
        contract_source_id: u64,
        landlord_authorization_id: u64,
        tenant_consent_id: u64,
    },
    /// Timeline-produced occurrence; the Engine resolves the household, owner, account and
    /// Claim facts from the standing lease rather than accepting them from the command.
    RentDue {
        unit_id: u32,
        lease_id: u32,
        due_at: GameTime,
    },
    Move {
        household_id: u32,
        old_unit_id: u32,
        new_unit_id: u32,
        expected_old_lease_id: u32,
        new_lease_id: u32,
        cost_quote_source_id: u64,
        contract_source_id: u64,
        landlord_authorization_id: u64,
        tenant_consent_id: u64,
        effective_at: GameTime,
    },
    EndLease {
        unit_id: u32,
        expected_lease_id: u32,
        effective_at: GameTime,
        contract_exit_source_id: u64,
    },
}

/// Income facts are supplied by the household/economy model, never inferred by this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HouseholdBudget {
    pub household_id: u32,
    pub account: AccountSnapshot,
    pub people: u16,
    /// Actual, normalized monthly wages/other household income from the economy model.
    pub net_income_per_month: Money,
    pub essential_costs_per_month: Money,
    pub income_observed_at: GameTime,
    pub income_window_complete: bool,
    pub income_source_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaseTerms {
    pub lease_id: u32,
    pub clause_version: u32,
    pub contract_source_id: u64,
    /// Owner/landlord authorization reference; caller verifies actual authority and signature.
    pub landlord_authorization_id: u64,
    /// Tenant acceptance reference; caller verifies the consenting household/account.
    pub tenant_consent_id: u64,
    pub rent_per_cycle: Money,
    pub cycle: Cycle,
    pub deposit: Money,
    /// Actual restricted-funds account supplied by the contract/ledger authority.
    pub escrow_account: u32,
    pub starts_at: GameTime,
}

/// Canonical digest for every contractual field, including the evidence IDs whose authority is
/// checked separately against the immutable opening offer. This fixed-field encoding avoids
/// JSON/serde ordering and allocates no temporary buffer.
pub fn lease_terms_sha256(terms: LeaseTerms) -> Result<[u8; 32], HousingError> {
    let anchor = monthly_anchor(terms.cycle)?;
    if terms.lease_id == 0
        || terms.clause_version == 0
        || terms.contract_source_id == 0
        || terms.landlord_authorization_id == 0
        || terms.tenant_consent_id == 0
        || terms.escrow_account == 0
        || terms.starts_at.value() < 0
    {
        return Err(HousingError::InvalidTerms);
    }
    Date::from_time(terms.starts_at).map_err(|_| HousingError::Arithmetic)?;
    let mut hash = Sha256::new();
    hash.update(b"MyCity/genesis-lease-terms/v1\0");
    hash.update(terms.lease_id.to_le_bytes());
    hash.update(terms.clause_version.to_le_bytes());
    hash.update(terms.contract_source_id.to_le_bytes());
    hash.update(terms.landlord_authorization_id.to_le_bytes());
    hash.update(terms.tenant_consent_id.to_le_bytes());
    hash.update(terms.rent_per_cycle.value().to_le_bytes());
    hash.update([anchor]);
    hash.update(terms.deposit.value().to_le_bytes());
    hash.update(terms.escrow_account.to_le_bytes());
    hash.update(terms.starts_at.value().to_le_bytes());
    Ok(hash.finalize().into())
}

/// Validate a host-supplied immutable genesis offer against the opening asset, exact current
/// dwelling, trusted transaction context and complete terms. `tenant_account` must come from the
/// Engine's resolved account and Engine must separately require the actual caller to be
/// `Caller::Account(tenant_account)` before invoking this function.
// Keep the independently authenticated context facts explicit at this boundary.
#[allow(clippy::too_many_arguments)]
pub fn validate_genesis_lease_offer(
    opening: &OpeningHousing,
    offer_id: u64,
    current_dwelling: DwellingFact,
    world_id: u64,
    branch_id: u64,
    tenant_account: u32,
    accepted_at: GameTime,
    terms: LeaseTerms,
) -> Result<GenesisLeaseAuthorization, HousingError> {
    let offer = opening.lease_offer(offer_id)?;
    let registered = opening
        .dwellings
        .binary_search_by_key(&offer.unit_id, |dwelling| dwelling.fact.unit_id)
        .ok()
        .map(|index| opening.dwellings[index])
        .ok_or(HousingError::InvalidDwelling)?;
    if offer.offer_id == 0
        || offer.world_id == 0
        || offer.branch_id == 0
        || world_id == 0
        || branch_id == 0
        || offer.unit_id == 0
        || offer.building_id == 0
        || offer.owner_account == 0
        || offer.tenant_account == 0
        || offer.owner_account == offer.tenant_account
        || offer.household_id == 0
        || offer.asset_source_id == 0
        || offer.authority_source_id == 0
        || offer.contract_source_id == 0
        || offer.landlord_authorization_id == 0
        || offer.tenant_consent_id == 0
        || offer.lease_id == 0
        || offer.clause_version == 0
        || offer.escrow_account == 0
        || offer.starts_at < offer.valid_from
        || offer.starts_at > offer.valid_until
        || tenant_account == 0
        || accepted_at.value() < 0
    {
        return Err(HousingError::InvalidIdentity);
    }
    Date::from_time(offer.valid_from).map_err(|_| HousingError::OfferMismatch)?;
    Date::from_time(offer.valid_until).map_err(|_| HousingError::OfferMismatch)?;
    Date::from_time(offer.starts_at).map_err(|_| HousingError::OfferMismatch)?;
    Date::from_time(accepted_at).map_err(|_| HousingError::OfferMismatch)?;
    if offer.valid_from > offer.valid_until {
        return Err(HousingError::OfferMismatch);
    }
    if accepted_at < offer.valid_from || accepted_at > offer.valid_until {
        return Err(HousingError::OfferExpired);
    }
    if registered.origin != DwellingOriginKind::ImmutableGenesisEndowment
        || registered.asset_source_id != offer.asset_source_id
        || registered.fact.unit_id != offer.unit_id
        || registered.fact.building_id != offer.building_id
        || registered.fact.owner_account != offer.owner_account
        || current_dwelling.unit_id != offer.unit_id
        || current_dwelling.building_id != offer.building_id
        || current_dwelling.owner_account != offer.owner_account
        || !current_dwelling.delivered
        || !current_dwelling.legal_to_occupy
        || !current_dwelling.has_legal_access
        || offer.world_id != world_id
        || offer.branch_id != branch_id
        || offer.tenant_account != tenant_account
        || terms.contract_source_id != offer.contract_source_id
        || terms.landlord_authorization_id != offer.landlord_authorization_id
        || terms.tenant_consent_id != offer.tenant_consent_id
        || terms.starts_at < accepted_at
    {
        return Err(HousingError::OfferMismatch);
    }
    let household = opening.household(offer.household_id)?;
    if household.account != offer.tenant_account
        || household.people > current_dwelling.person_capacity
    {
        return Err(HousingError::OfferMismatch);
    }
    let terms_sha256 = lease_terms_sha256(terms)?;
    if offer.executable_terms() != terms || offer.terms_sha256 != terms_sha256 {
        return Err(HousingError::OfferMismatch);
    }
    Ok(GenesisLeaseAuthorization {
        offer_id: offer.offer_id,
        world_id,
        branch_id,
        unit_id: offer.unit_id,
        building_id: offer.building_id,
        owner_account: offer.owner_account,
        tenant_account,
        household_id: household.household_id,
        people: household.people,
        offer_sha256: genesis_lease_offer_sha256(offer),
        terms_sha256,
    })
}

/// Stable content digest suitable for the existing transaction receipt. It identifies the
/// immutable genesis offer itself, not a tenant's transaction signature.
pub fn genesis_lease_offer_sha256(offer: GenesisLeaseOffer) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"MyCity/genesis-lease-offer/v2\0");
    hash.update(offer.offer_id.to_le_bytes());
    hash.update(offer.world_id.to_le_bytes());
    hash.update(offer.branch_id.to_le_bytes());
    hash.update(offer.unit_id.to_le_bytes());
    hash.update(offer.building_id.to_le_bytes());
    hash.update(offer.owner_account.to_le_bytes());
    hash.update(offer.tenant_account.to_le_bytes());
    hash.update(offer.household_id.to_le_bytes());
    hash.update(offer.valid_from.value().to_le_bytes());
    hash.update(offer.valid_until.value().to_le_bytes());
    hash.update(offer.asset_source_id.to_le_bytes());
    hash.update(offer.authority_source_id.to_le_bytes());
    hash.update(offer.contract_source_id.to_le_bytes());
    hash.update(offer.landlord_authorization_id.to_le_bytes());
    hash.update(offer.tenant_consent_id.to_le_bytes());
    hash.update(offer.terms_sha256);
    hash.finalize().into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affordability {
    Unknown,
    WithinSoftTarget,
    AboveSoftTarget,
    IncomeDoesNotCoverRentAndEssentials,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffordabilityReport {
    pub classification: Affordability,
    pub rent_share_percent_floor: Option<u8>,
    pub projected_residual_per_month: Option<Money>,
    /// The 35% target is preference information, not a legal or lease veto.
    pub soft_target_percent: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CashTransfer {
    pub from_account: u32,
    pub to_account: u32,
    pub amount: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeasePlan {
    pub unit_id: u32,
    pub expected_lease_id: u32,
    pub expected_household_id: u32,
    pub updated_unit: HousingUnit,
    /// First invoice for an off-anchor move-in, rounded up to the nearest cent once.
    pub first_due_amount: Money,
    pub deposit_transfer: CashTransfer,
    pub affordability: AffordabilityReport,
    pub contract_source_id: u64,
    pub dwelling_source_version: u64,
    pub income_source_version: u64,
    pub new_contract_source_id: u64,
    pub landlord_authorization_id: u64,
    pub tenant_consent_id: u64,
}

/// Fixed-size-by-contract row delta for Engine's prepared shadow. Financial effects are staged
/// separately in the shared ledger and both shadows publish only after the durable hook succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HousingPatch {
    pub expected: HousingUnit,
    pub updated: HousingUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RentCollectionPlan {
    pub unit_id: u32,
    pub lease_id: u32,
    /// Exact existing Timeline/Cycle boundary being consumed; use it as the recurring event key.
    pub due_at: GameTime,
    pub expected_last_accounted_at: GameTime,
    pub updated_unit: HousingUnit,
    pub landlord_transfer: CashTransfer,
    /// Whether this one invoice rounded a fractional cent upward (adds less than one cent).
    pub rounded_up_fractional_cent: bool,
    /// Difference between rent due and cash actually available; caller records this in A.
    pub remaining_arrears: Money,
    pub debt_source_lease_id: u32,
}

/// Trusted-calendar rent accrual intent. This plan carries no transfer or duplicate arrears
/// amount: Engine applies `updated_claim` to its Timeline shadow, lets Timeline perform the only
/// cash collection, then reconciles `arrears_ref` from that resulting Claim before publishing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RentOccurrencePlan {
    pub occurrence_lease_id: u32,
    pub occurrence_due_at: GameTime,
    pub expected_unit: HousingUnit,
    pub updated_unit: HousingUnit,
    /// Exact prior native Timeline row. `None` means the first claim is created at this due.
    pub expected_claim: Option<crate::timeline::Claim>,
    /// Native Timeline row to stage before its current-boundary Claim payment phase.
    pub updated_claim: crate::timeline::Claim,
    /// New rent added to principal. Zero only when Lease already registered this exact first
    /// invoice ahead of the due event; it is never the amount paid or the arrears balance.
    pub accrual_increment: Money,
    pub invoice: Money,
    pub earliest_due_at: GameTime,
    pub next_due_at: GameTime,
    pub rounded_up_fractional_cent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveScore {
    pub lower: u8,
    pub upper: u8,
    pub fully_covered: bool,
    /// Both sources are mandatory even when the score was composed by another subsystem.
    pub environment_source: u64,
    pub public_service_source: u64,
    pub source_version: u64,
    pub assessed_at: GameTime,
    pub valid_through: GameTime,
    pub hard_failure_mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveTrigger {
    /// Persisted by the satisfaction/family model after 14 full, continuously observed days.
    pub low_since: Option<GameTime>,
    pub covered_through: GameTime,
    pub continuous_low_and_covered: bool,
    pub cooldown_until: Option<GameTime>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveCostQuote {
    pub source_id: u64,
    pub valid_through: GameTime,
    pub reachable: bool,
    pub travel_seconds: u64,
    pub moving_charge: Money,
    pub moving_provider_account: u32,
    /// Old lease rent/exit charges, excluding old arrears already in the shared debt ledger.
    pub old_lease_close_due: Money,
    pub old_close_payee_account: u32,
    pub old_clause_version: u32,
    pub old_contract_exit_source_id: u64,
    pub old_escrow_account: u32,
    pub deposit_resolution_source_id: u64,
    pub old_deposit_refund_authorized: Money,
    pub old_deposit_deduction_authorized: Money,
    pub old_deposit_deduction_account: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveCandidate {
    pub dwelling: DwellingFact,
    pub terms: LeaseTerms,
    pub score: MoveScore,
    pub move_cost: MoveCostQuote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveWait {
    SatisfactionWindow,
    Cooldown,
    EvidenceIncomplete,
    CandidateNotBetter,
    NoReachableCandidate,
    CandidateLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovePlan {
    pub household_id: u32,
    pub old_unit_id: u32,
    pub new_unit_id: u32,
    pub expected_old_lease_id: u32,
    pub expected_new_lease_id: u32,
    pub updated_old_unit: HousingUnit,
    pub updated_new_unit: HousingUnit,
    pub new_first_due_amount: Money,
    pub effects: [CashTransfer; 5],
    pub effect_count: u8,
    /// Prior rent/deposit disputes stay linked to the old lease in the shared ledger.
    pub old_arrears_ref_retained_by_ledger: u32,
    pub old_escrow_account: u32,
    pub old_deposit_liability_retained_by_ledger: Money,
    /// Terminal pro-rata invoice rounds a fractional cent upward at most once.
    pub final_rent_rounded_up_fractional_cent: bool,
    pub score_delta_lower_bound: u8,
    pub effective_at: GameTime,
    pub cooldown_until: GameTime,
    pub cost_source_id: u64,
    pub contract_exit_source_id: u64,
    pub new_contract_source_id: u64,
    pub new_landlord_authorization_id: u64,
    pub tenant_consent_id: u64,
    pub deposit_resolution_source_id: u64,
    pub environment_source: u64,
    pub public_service_source: u64,
    pub dwelling_source_version: u64,
    pub income_source_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HousingError {
    InvalidIdentity,
    InvalidDwelling,
    UnitLimit,
    DuplicateUnit,
    DuplicateIdentity,
    SnapshotMismatch,
    ValidationScratch,
    UnknownUnit,
    Occupied,
    HouseholdAlreadyHoused,
    CapacityExceeded,
    InvalidTerms,
    DuplicateLease,
    InvalidAccountSnapshot,
    InsufficientCash,
    Arithmetic,
    NotDue,
    TimeRegression,
    CandidateLimit,
    StalePlan,
    NotTriggered,
    EvidenceIncomplete,
    CandidateNotBetter,
    Unreachable,
    ExpiredQuote,
    ExistingLeaseMismatch,
    OfferExpired,
    OfferMismatch,
    OfferConsumed,
    UnknownOffer,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct HousingState {
    /// The startup snapshot chooses a smaller limit based on real delivered housing stock.
    unit_limit: usize,
    units: Vec<HousingUnit>,
    /// One-shot genesis offer receipts are persistent logical state, not a duplicate lease/debt
    /// ledger. The bounded IDs prevent reuse after vacancy and cold restore.
    consumed_offer_ids: Vec<ConsumedOfferId>,
}

impl Clone for HousingState {
    fn clone(&self) -> Self {
        let mut clone = Self::new(self.unit_limit).expect("bounded housing backing allocation");
        clone.units.extend_from_slice(&self.units);
        clone
            .consumed_offer_ids
            .extend_from_slice(&self.consumed_offer_ids);
        clone
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HousingWire {
    unit_limit: u8,
    units: Rows<HousingUnit, MAX_M0_HOUSING_UNITS>,
    consumed_offer_ids: Rows<ConsumedOfferId, MAX_GENESIS_LEASE_OFFERS>,
}

impl<'de> Deserialize<'de> for HousingState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = HousingWire::deserialize(deserializer)?;
        let limit = usize::from(wire.unit_limit);
        if limit > MAX_M0_HOUSING_UNITS
            || wire.units.len() > limit
            || wire.consumed_offer_ids.len() > limit
        {
            return Err(de::Error::custom("housing_unit_limit"));
        }
        let mut state = Self::new(limit).map_err(|_| de::Error::custom("housing_limit"))?;
        state.units.extend_from_slice(&wire.units);
        state
            .consumed_offer_ids
            .extend_from_slice(&wire.consumed_offer_ids);
        state
            .validate_structure()
            .map_err(|_| de::Error::custom("housing_structure"))?;
        Ok(state)
    }
}

impl crate::restore_into::Restore for HousingState {
    fn restore<'de, D: Deserializer<'de>>(&mut self, deserializer: D) -> Result<(), D::Error> {
        struct RowsInto<'a> {
            rows: &'a mut Vec<HousingUnit>,
            limit: usize,
        }
        struct OfferRowsInto<'a> {
            rows: &'a mut Vec<ConsumedOfferId>,
            limit: usize,
        }
        struct RejectExtra;
        impl<'de> de::DeserializeSeed<'de> for RejectExtra {
            type Value = ();
            fn deserialize<D2: Deserializer<'de>>(self, _: D2) -> Result<(), D2::Error> {
                Err(de::Error::custom("housing_rows_extra"))
            }
        }
        impl<'de> de::DeserializeSeed<'de> for RowsInto<'_> {
            type Value = ();
            fn deserialize<D2: Deserializer<'de>>(self, d: D2) -> Result<(), D2::Error> {
                struct Read<'a> {
                    rows: &'a mut Vec<HousingUnit>,
                    limit: usize,
                }
                impl<'de> de::Visitor<'de> for Read<'_> {
                    type Value = ();
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "at most {} housing rows", self.limit)
                    }
                    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
                        self.rows.clear();
                        for _ in 0..self.limit {
                            match seq.next_element()? {
                                Some(row) => self.rows.push(row),
                                None => return Ok(()),
                            }
                        }
                        seq.next_element_seed(RejectExtra)?;
                        Ok(())
                    }
                }
                d.deserialize_seq(Read {
                    rows: self.rows,
                    limit: self.limit,
                })
            }
        }
        impl<'de> de::DeserializeSeed<'de> for OfferRowsInto<'_> {
            type Value = ();
            fn deserialize<D2: Deserializer<'de>>(self, d: D2) -> Result<(), D2::Error> {
                struct Read<'a> {
                    rows: &'a mut Vec<ConsumedOfferId>,
                    limit: usize,
                }
                impl<'de> de::Visitor<'de> for Read<'_> {
                    type Value = ();
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "at most {} consumed genesis offers", self.limit)
                    }
                    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
                        self.rows.clear();
                        for _ in 0..self.limit {
                            match seq.next_element()? {
                                Some(id) => self.rows.push(id),
                                None => return Ok(()),
                            }
                        }
                        seq.next_element_seed(RejectExtra)?;
                        Ok(())
                    }
                }
                d.deserialize_seq(Read {
                    rows: self.rows,
                    limit: self.limit,
                })
            }
        }
        struct Read<'a>(&'a mut HousingState);
        impl<'de> de::Visitor<'de> for Read<'_> {
            type Value = ();
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bounded housing state")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
                let target = self.0;
                target.units.clear();
                target.consumed_offer_ids.clear();
                let mut seen = 0_u8;
                while let Some(key) = map.next_key::<crate::restore_into::Key>()? {
                    match key.as_str() {
                        "unit_limit" => {
                            if seen & 1 != 0 {
                                target.units.clear();
                                target.consumed_offer_ids.clear();
                                return Err(de::Error::custom("housing_duplicate_field"));
                            }
                            seen |= 1;
                            let encoded = map.next_value::<u8>()?;
                            if usize::from(encoded) != target.unit_limit {
                                target.units.clear();
                                target.consumed_offer_ids.clear();
                                return Err(de::Error::custom("housing_restore_capacity"));
                            }
                        }
                        "units" => {
                            if seen & 2 != 0 {
                                target.units.clear();
                                target.consumed_offer_ids.clear();
                                return Err(de::Error::custom("housing_duplicate_field"));
                            }
                            seen |= 2;
                            let limit = target.unit_limit;
                            if let Err(error) = map.next_value_seed(RowsInto {
                                rows: &mut target.units,
                                limit,
                            }) {
                                target.units.clear();
                                target.consumed_offer_ids.clear();
                                return Err(error);
                            }
                        }
                        "consumed_offer_ids" => {
                            if seen & 4 != 0 {
                                target.units.clear();
                                target.consumed_offer_ids.clear();
                                return Err(de::Error::custom("housing_duplicate_field"));
                            }
                            seen |= 4;
                            let limit = target.unit_limit;
                            if let Err(error) = map.next_value_seed(OfferRowsInto {
                                rows: &mut target.consumed_offer_ids,
                                limit,
                            }) {
                                target.units.clear();
                                target.consumed_offer_ids.clear();
                                return Err(error);
                            }
                        }
                        _ => {
                            target.units.clear();
                            target.consumed_offer_ids.clear();
                            return Err(de::Error::custom("housing_unknown_field"));
                        }
                    }
                }
                if seen != 7 {
                    target.units.clear();
                    target.consumed_offer_ids.clear();
                    return Err(de::Error::custom("housing_missing_field"));
                }
                if target.validate_structure().is_err() {
                    target.units.clear();
                    target.consumed_offer_ids.clear();
                    return Err(de::Error::custom("housing_structure"));
                }
                Ok(())
            }
        }
        deserializer.deserialize_map(Read(self))
    }
}

impl HousingState {
    pub fn new(unit_limit: usize) -> Result<Self, HousingError> {
        if unit_limit > MAX_M0_HOUSING_UNITS {
            return Err(HousingError::UnitLimit);
        }
        let mut units = Vec::new();
        units
            .try_reserve_exact(unit_limit)
            .map_err(|_| HousingError::UnitLimit)?;
        if units.capacity() < unit_limit {
            return Err(HousingError::UnitLimit);
        }
        let mut consumed_offer_ids = Vec::new();
        consumed_offer_ids
            .try_reserve_exact(unit_limit)
            .map_err(|_| HousingError::UnitLimit)?;
        if consumed_offer_ids.capacity() < unit_limit {
            return Err(HousingError::UnitLimit);
        }
        Ok(Self {
            unit_limit,
            units,
            consumed_offer_ids,
        })
    }

    pub fn units(&self) -> &[HousingUnit] {
        &self.units
    }

    pub fn consumed_genesis_offer_ids(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        self.consumed_offer_ids.iter().map(|id| id.0)
    }

    pub fn unit_limit(&self) -> usize {
        self.unit_limit
    }

    pub fn owned_capacity_bytes(&self) -> usize {
        self.units.capacity() * size_of::<HousingUnit>()
            + self.consumed_offer_ids.capacity() * size_of::<ConsumedOfferId>()
    }

    /// Clears live lease/household data while retaining the configured stock-sized backing.
    pub fn reset(&mut self) {
        self.units.clear();
        self.consumed_offer_ids.clear();
    }

    /// Decodes into the existing configured backing and validates against authoritative spatial
    /// facts before the caller can publish the restored state. On any error, the target is empty
    /// and can be reused for an explicit retry.
    pub fn restore_with_snapshot<'de, D: Deserializer<'de>>(
        &mut self,
        deserializer: D,
        facts: &[DwellingFact],
    ) -> Result<usize, D::Error> {
        <Self as crate::restore_into::Restore>::restore(self, deserializer)?;
        match self.validate_with_snapshot(facts) {
            Ok(scratch) => Ok(scratch),
            Err(_) => {
                self.reset();
                Err(de::Error::custom("housing_snapshot"))
            }
        }
    }

    /// Copies into an already configured target without changing its allocation.
    pub fn copy_from(&mut self, source: &Self) -> Result<(), HousingError> {
        if std::ptr::eq(self, source) {
            return Ok(());
        }
        if self.unit_limit != source.unit_limit
            || source.units.len() > self.units.capacity()
            || source.consumed_offer_ids.len() > self.consumed_offer_ids.capacity()
        {
            return Err(HousingError::UnitLimit);
        }
        self.units.clear();
        self.units.extend_from_slice(&source.units);
        self.consumed_offer_ids.clear();
        self.consumed_offer_ids
            .extend_from_slice(&source.consumed_offer_ids);
        Ok(())
    }

    /// Builds the first housing inventory from explicit immutable genesis asset registrations.
    /// It does not claim those assets were produced by the construction simulator.
    pub fn from_opening(opening: &OpeningHousing) -> Result<Self, HousingError> {
        let limit = usize::from(opening.unit_limit);
        if limit == 0 || limit != opening.dwellings.len() {
            return Err(HousingError::UnitLimit);
        }
        if opening
            .dwellings
            .windows(2)
            .any(|pair| pair[0].fact.unit_id >= pair[1].fact.unit_id)
        {
            return Err(HousingError::DuplicateUnit);
        }
        let mut state = Self::new(limit)?;
        let mut facts = Vec::new();
        facts
            .try_reserve_exact(limit)
            .map_err(|_| HousingError::ValidationScratch)?;
        let mut source_ids = Vec::new();
        source_ids
            .try_reserve_exact(limit)
            .map_err(|_| HousingError::ValidationScratch)?;
        for registered in &opening.dwellings {
            if registered.origin != DwellingOriginKind::ImmutableGenesisEndowment
                || registered.asset_source_id == 0
            {
                return Err(HousingError::InvalidDwelling);
            }
            state.register(registered.fact)?;
            facts.push(registered.fact);
            source_ids.push(registered.asset_source_id);
        }
        source_ids.sort_unstable();
        if source_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(HousingError::DuplicateIdentity);
        }
        opening.validate_lease_offer_catalog()?;
        state.validate_with_snapshot(&facts)?;
        Ok(state)
    }

    /// Prototype capacity is deliberately capped at 32 actual dwelling rows. Later capacity
    /// growth must be introduced with a measured role-budget change, not by bypassing this cap.
    pub fn extend_capacity(&mut self, new_limit: usize) -> Result<(), HousingError> {
        if new_limit < self.unit_limit || new_limit > MAX_M0_HOUSING_UNITS {
            return Err(HousingError::UnitLimit);
        }
        if new_limit == self.unit_limit {
            return Ok(());
        }
        self.units
            .try_reserve_exact(new_limit.saturating_sub(self.units.len()))
            .map_err(|_| HousingError::UnitLimit)?;
        if self.units.capacity() < new_limit {
            return Err(HousingError::UnitLimit);
        }
        self.unit_limit = new_limit;
        Ok(())
    }

    /// Validates the self-contained portion of a decoded housing table. Unit IDs must be sorted;
    /// household and lease uniqueness is checked with a bounded temporary key array. This does
    /// not prove that a row still corresponds to a delivered/legal building unit or that a
    /// building's aggregate capacity is respected; restore must additionally call
    /// `validate_with_snapshot` before publishing this state.
    pub(crate) fn validate_structure(&self) -> Result<(), HousingError> {
        self.validate_rows_basic()?;
        let mut keys = validation_keys(self.units.len().saturating_mul(2))?;
        collect_identity_keys(&self.units, &mut keys);
        keys.sort_unstable();
        if keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(HousingError::DuplicateIdentity);
        }
        Ok(())
    }

    fn validate_rows_basic(&self) -> Result<(), HousingError> {
        if self.unit_limit > MAX_M0_HOUSING_UNITS
            || self.units.len() > self.unit_limit
            || self.units.capacity() > MAX_M0_HOUSING_UNITS
        {
            return Err(HousingError::UnitLimit);
        }
        if self
            .units
            .windows(2)
            .any(|pair| pair[0].unit_id >= pair[1].unit_id)
        {
            return Err(HousingError::DuplicateUnit);
        }
        for unit in &self.units {
            validate_unit(unit)?;
        }
        if self.consumed_offer_ids.len() > self.unit_limit
            || self.consumed_offer_ids.iter().any(|id| id.0 == 0)
            || self
                .consumed_offer_ids
                .windows(2)
                .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(HousingError::OfferConsumed);
        }
        Ok(())
    }

    /// Validates a persisted table against the authoritative building/spatial snapshot. The
    /// snapshot must contain exactly one fact for each unit, sorted by `unit_id`. Requiring this
    /// second call at the World/Store restore boundary prevents valid-looking rows from pointing
    /// at demolished, inaccessible, illegal or over-capacity dwellings.
    pub fn validate_with_snapshot(&self, facts: &[DwellingFact]) -> Result<usize, HousingError> {
        self.validate_rows_basic()?;
        if facts.len() != self.units.len() {
            return Err(HousingError::SnapshotMismatch);
        }
        let mut keys = validation_keys(self.units.len().saturating_mul(3))?;
        collect_identity_keys(&self.units, &mut keys);
        for (unit, fact) in self.units.iter().zip(facts) {
            validate_dwelling(*fact)?;
            if unit.unit_id != fact.unit_id
                || unit.building_id != fact.building_id
                || unit.owner_account != fact.owner_account
            {
                return Err(HousingError::SnapshotMismatch);
            }
            keys.push(building_capacity_key(
                fact.building_id,
                fact.building_unit_capacity,
            ));
        }
        keys.sort_unstable();
        if keys
            .windows(2)
            .any(|pair| key_kind(pair[0]) != BUILDING_KEY_KIND && pair[0] == pair[1])
        {
            return Err(HousingError::DuplicateIdentity);
        }
        let mut previous_building = 0;
        let mut previous_capacity = 0_u16;
        let mut building_count = 0_usize;
        for key in keys
            .iter()
            .copied()
            .filter(|key| key_kind(*key) == BUILDING_KEY_KIND)
        {
            let (building, capacity) = decode_building_capacity_key(key);
            if building == previous_building {
                if capacity != previous_capacity {
                    return Err(HousingError::SnapshotMismatch);
                }
                building_count += 1;
            } else {
                previous_building = building;
                previous_capacity = capacity;
                building_count = 1;
            }
            if building_count > usize::from(capacity) {
                return Err(HousingError::CapacityExceeded);
            }
        }
        Ok(keys.capacity() * size_of::<u64>())
    }

    /// Registers one delivered unit whose authoritative facts have been supplied by the caller.
    pub fn register(&mut self, fact: DwellingFact) -> Result<(), HousingError> {
        validate_dwelling(fact)?;
        if self.units.len() >= self.unit_limit {
            return Err(HousingError::UnitLimit);
        }
        if self.units.iter().any(|row| row.unit_id == fact.unit_id) {
            return Err(HousingError::DuplicateUnit);
        }
        let building_units = self
            .units
            .iter()
            .filter(|row| row.building_id == fact.building_id)
            .count();
        if building_units >= usize::from(fact.building_unit_capacity) {
            return Err(HousingError::CapacityExceeded);
        }
        let row = HousingUnit {
            unit_id: fact.unit_id,
            building_id: fact.building_id,
            owner_account: fact.owner_account,
            ..HousingUnit::default()
        };
        let index = self
            .units
            .binary_search_by_key(&fact.unit_id, |unit| unit.unit_id)
            .unwrap_or_else(|index| index);
        self.units.insert(index, row);
        Ok(())
    }

    /// Low-level row mutation retained for existing component tests and trusted planner callers.
    /// The first real Engine lease path must use `apply_genesis_offer_lease`; this method alone
    /// provides no landlord/tenant authority proof and must not be called from a user Action.
    pub fn apply_lease(&mut self, plan: LeasePlan) -> Result<(), HousingError> {
        let index = self.find_index(plan.unit_id)?;
        let current = self.units[index];
        if current.lease_id != plan.expected_lease_id
            || current.household_id != plan.expected_household_id
            || current.household_id != 0
            || plan.updated_unit.household_id == 0
            || plan.updated_unit.lease_id == 0
        {
            return Err(HousingError::StalePlan);
        }
        let (expected_first_due, _) = prorated_rent(
            plan.updated_unit.rent_per_cycle,
            plan.updated_unit.last_accounted_at,
            plan.updated_unit.next_due_at,
            plan.updated_unit.billing_anchor as u8,
        )?;
        if plan.first_due_amount != expected_first_due {
            return Err(HousingError::StalePlan);
        }
        self.apply_transaction(&[HousingPatch {
            expected: current,
            updated: plan.updated_unit,
        }])
    }

    /// Atomically applies an offer-backed lease row and persists the one-shot offer marker.
    /// The Engine must first resolve `plan` from authoritative balances/facts, verify the actual
    /// `Caller::Account(authorization.tenant_account)`, and stage the corresponding cash/Claim/
    /// timeline effects in the same durable transaction. This method never handles money.
    pub fn apply_genesis_offer_lease(
        &mut self,
        plan: LeasePlan,
        authorization: GenesisLeaseAuthorization,
    ) -> Result<(), HousingError> {
        let index = self.find_index(plan.unit_id)?;
        let current = self.units[index];
        if authorization.offer_id == 0
            || authorization.world_id == 0
            || authorization.branch_id == 0
            || authorization.unit_id != plan.unit_id
            || authorization.building_id != current.building_id
            || authorization.owner_account != current.owner_account
            || authorization.tenant_account != plan.deposit_transfer.from_account
            || authorization.household_id != plan.updated_unit.household_id
            || plan.new_contract_source_id != plan.contract_source_id
            || plan.updated_unit.building_id != authorization.building_id
            || plan.updated_unit.owner_account != authorization.owner_account
            || plan.updated_unit.lease_id == 0
            || plan.updated_unit.household_id == 0
            || authorization.terms_sha256 != lease_plan_terms_sha256(plan)?
        {
            return Err(HousingError::OfferMismatch);
        }
        if self
            .consumed_offer_ids
            .binary_search_by_key(&authorization.offer_id, |id| id.0)
            .is_ok()
        {
            return Err(HousingError::OfferConsumed);
        }
        if self.consumed_offer_ids.len() >= self.unit_limit {
            return Err(HousingError::UnitLimit);
        }
        self.apply_lease(plan)?;
        let index = self
            .consumed_offer_ids
            .binary_search_by_key(&authorization.offer_id, |id| id.0)
            .unwrap_err();
        self.consumed_offer_ids
            .insert(index, ConsumedOfferId(authorization.offer_id));
        Ok(())
    }

    pub fn apply_rent_collection(&mut self, plan: RentCollectionPlan) -> Result<(), HousingError> {
        let index = self.find_index(plan.unit_id)?;
        let current = self.units[index];
        if current.lease_id != plan.lease_id
            || current.last_accounted_at != plan.expected_last_accounted_at
        {
            return Err(HousingError::StalePlan);
        }
        self.apply_transaction(&[HousingPatch {
            expected: current,
            updated: plan.updated_unit,
        }])
    }

    /// Applies only the prepared lease/calendar cursor patch. The caller must stage the native
    /// Timeline Claim update and Claim payment in the same transaction; this method never touches
    /// cash, Claim principal, or Claim paid.
    pub fn apply_rent_occurrence(&mut self, plan: RentOccurrencePlan) -> Result<(), HousingError> {
        if plan.occurrence_lease_id != plan.expected_unit.lease_id
            || plan.occurrence_due_at != plan.expected_unit.next_due_at
            || plan.next_due_at != plan.updated_unit.next_due_at
        {
            return Err(HousingError::StalePlan);
        }
        let index = self.find_index(plan.expected_unit.unit_id)?;
        if self.units[index] != plan.expected_unit {
            return Err(HousingError::StalePlan);
        }
        self.apply_transaction(&[HousingPatch {
            expected: plan.expected_unit,
            updated: plan.updated_unit,
        }])
    }

    /// Applies both occupancy changes together after the caller stages every ledger effect.
    pub fn apply_move(&mut self, plan: MovePlan, applied_at: GameTime) -> Result<(), HousingError> {
        if applied_at != plan.effective_at {
            return Err(HousingError::StalePlan);
        }
        let old_index = self.find_index(plan.old_unit_id)?;
        let new_index = self.find_index(plan.new_unit_id)?;
        if old_index == new_index {
            return Err(HousingError::StalePlan);
        }
        let old = self.units[old_index];
        let new = self.units[new_index];
        if old.household_id != plan.household_id
            || old.lease_id != plan.expected_old_lease_id
            || old.arrears_ref != plan.old_arrears_ref_retained_by_ledger
            || new.household_id != 0
            || new.lease_id != plan.expected_new_lease_id
            || plan.updated_new_unit.household_id != plan.household_id
        {
            return Err(HousingError::StalePlan);
        }
        let (expected_first_due, _) = prorated_rent(
            plan.updated_new_unit.rent_per_cycle,
            plan.updated_new_unit.last_accounted_at,
            plan.updated_new_unit.next_due_at,
            plan.updated_new_unit.billing_anchor as u8,
        )?;
        if plan.new_first_due_amount != expected_first_due {
            return Err(HousingError::StalePlan);
        }
        self.apply_transaction(&[
            HousingPatch {
                expected: old,
                updated: plan.updated_old_unit,
            },
            HousingPatch {
                expected: new,
                updated: plan.updated_new_unit,
            },
        ])
    }

    /// Validates an at-most-two-row prepared update with no allocation. The Engine must stage
    /// its ledger/claim effects alongside this patch and apply to its shadow before durable
    /// persistence. Publication must then swap both shadows atomically.
    pub fn apply_transaction(&mut self, patches: &[HousingPatch]) -> Result<(), HousingError> {
        if patches.is_empty() || patches.len() > 2 {
            return Err(HousingError::StalePlan);
        }
        let mut indices = [usize::MAX; 2];
        for (i, patch) in patches.iter().enumerate() {
            validate_unit(&patch.updated)?;
            if !unit_transition_valid(patch.expected, patch.updated) {
                return Err(HousingError::StalePlan);
            }
            indices[i] = self.find_index(patch.expected.unit_id)?;
            if self.units[indices[i]] != patch.expected {
                return Err(HousingError::StalePlan);
            }
            if patches[..i]
                .iter()
                .any(|prior| prior.expected.unit_id == patch.expected.unit_id)
            {
                return Err(HousingError::StalePlan);
            }
        }
        for (i, patch) in patches.iter().enumerate() {
            for (j, other) in patches.iter().enumerate() {
                if i == j {
                    continue;
                }
                if nonzero_equal(patch.updated.household_id, other.updated.household_id) {
                    return Err(HousingError::HouseholdAlreadyHoused);
                }
                if nonzero_equal(patch.updated.lease_id, other.updated.lease_id) {
                    return Err(HousingError::DuplicateLease);
                }
            }
            for (index, other) in self.units.iter().enumerate() {
                if indices[..patches.len()].contains(&index) {
                    continue;
                }
                if nonzero_equal(patch.updated.household_id, other.household_id) {
                    return Err(HousingError::HouseholdAlreadyHoused);
                }
                if nonzero_equal(patch.updated.lease_id, other.lease_id) {
                    return Err(HousingError::DuplicateLease);
                }
            }
        }
        for (patch, index) in patches.iter().zip(indices) {
            if index != usize::MAX {
                self.units[index] = patch.updated;
            }
        }
        Ok(())
    }

    fn find_index(&self, unit_id: u32) -> Result<usize, HousingError> {
        self.units
            .binary_search_by_key(&unit_id, |unit| unit.unit_id)
            .ok()
            .ok_or(HousingError::UnknownUnit)
    }
}

fn nonzero_equal(left: u32, right: u32) -> bool {
    left != 0 && left == right
}

fn lease_plan_terms_sha256(plan: LeasePlan) -> Result<[u8; 32], HousingError> {
    lease_terms_sha256(LeaseTerms {
        lease_id: plan.updated_unit.lease_id,
        clause_version: plan.updated_unit.clause_version,
        contract_source_id: plan.contract_source_id,
        landlord_authorization_id: plan.landlord_authorization_id,
        tenant_consent_id: plan.tenant_consent_id,
        rent_per_cycle: plan.updated_unit.rent_per_cycle,
        cycle: Cycle::Monthly {
            anchor: plan.updated_unit.billing_anchor as u8,
        },
        deposit: plan.deposit_transfer.amount,
        escrow_account: plan.deposit_transfer.to_account,
        starts_at: plan.updated_unit.last_accounted_at,
    })
}

const HOUSEHOLD_KEY_KIND: u64 = 1;
const LEASE_KEY_KIND: u64 = 2;
const BUILDING_KEY_KIND: u64 = 3;
const KEY_KIND_SHIFT: u32 = 62;
const KEY_PAYLOAD_MASK: u64 = (1_u64 << KEY_KIND_SHIFT) - 1;

fn key_kind(key: u64) -> u64 {
    key >> KEY_KIND_SHIFT
}

fn identity_key(kind: u64, id: u32) -> u64 {
    (kind << KEY_KIND_SHIFT) | u64::from(id)
}

fn building_capacity_key(building: u32, capacity: u16) -> u64 {
    (BUILDING_KEY_KIND << KEY_KIND_SHIFT) | (u64::from(building) << 16) | u64::from(capacity)
}

fn decode_building_capacity_key(key: u64) -> (u32, u16) {
    let payload = key & KEY_PAYLOAD_MASK;
    ((payload >> 16) as u32, payload as u16)
}

fn validation_keys(capacity: usize) -> Result<Vec<u64>, HousingError> {
    if capacity > MAX_DWELLING_UNITS.saturating_mul(3) {
        return Err(HousingError::UnitLimit);
    }
    let mut keys = Vec::new();
    keys.try_reserve_exact(capacity)
        .map_err(|_| HousingError::ValidationScratch)?;
    Ok(keys)
}

fn collect_identity_keys(units: &[HousingUnit], keys: &mut Vec<u64>) {
    for unit in units {
        if unit.household_id != 0 {
            keys.push(identity_key(HOUSEHOLD_KEY_KIND, unit.household_id));
        }
        if unit.lease_id != 0 {
            keys.push(identity_key(LEASE_KEY_KIND, unit.lease_id));
        }
    }
}

fn validate_unit(unit: &HousingUnit) -> Result<(), HousingError> {
    if unit.unit_id == 0 || unit.building_id == 0 || unit.owner_account == 0 {
        return Err(HousingError::InvalidIdentity);
    }
    if unit.household_id == 0 {
        if unit.lease_id != 0
            || unit.rent_per_cycle != Money::ZERO
            || unit.billing_anchor != 0
            || unit.deposit_held != Money::ZERO
        {
            return Err(HousingError::InvalidTerms);
        }
    } else if unit.lease_id == 0
        && (unit.rent_per_cycle != Money::ZERO || unit.deposit_held != Money::ZERO)
    {
        // Owner-occupied homes may not silently carry a residential rent contract.
        return Err(HousingError::InvalidTerms);
    }
    if unit.lease_id == 0 && unit.billing_anchor != 0 {
        return Err(HousingError::InvalidTerms);
    }
    if unit.lease_id != 0 && !(1..=31).contains(&unit.billing_anchor) {
        return Err(HousingError::InvalidTerms);
    }
    if unit.arrears_ref != 0 && unit.arrears_ref != unit.lease_id {
        return Err(HousingError::InvalidTerms);
    }
    if unit.lease_id != 0 && unit.next_due_at < unit.last_accounted_at {
        return Err(HousingError::InvalidTerms);
    }
    Ok(())
}

fn validate_dwelling(fact: DwellingFact) -> Result<(), HousingError> {
    if fact.unit_id == 0 || fact.building_id == 0 || fact.owner_account == 0 {
        return Err(HousingError::InvalidIdentity);
    }
    if fact.building_unit_capacity == 0
        || fact.person_capacity == 0
        || fact.source_version == 0
        || !fact.delivered
        || !fact.legal_to_occupy
        || !fact.has_legal_access
    {
        return Err(HousingError::InvalidDwelling);
    }
    Ok(())
}

fn monthly_anchor(cycle: Cycle) -> Result<u8, HousingError> {
    match cycle {
        Cycle::Monthly { anchor } if (1..=31).contains(&anchor) => Ok(anchor),
        _ => Err(HousingError::InvalidTerms),
    }
}

fn next_monthly_due(anchor: u8, at: GameTime) -> Result<GameTime, HousingError> {
    let seconds = u64::try_from(at.value()).map_err(|_| HousingError::Arithmetic)?;
    let due = Cycle::Monthly { anchor }
        .next(seconds)
        .map_err(|_| HousingError::Arithmetic)?;
    GameTime::new(i64::try_from(due).map_err(|_| HousingError::Arithmetic)?)
        .map_err(|_| HousingError::Arithmetic)
}

/// Returns the immediately preceding clamped calendar-month anchor for a due boundary.
/// The accepted epoch/calendar is the same fixed Gregorian calendar as Timeline::Cycle.
fn previous_monthly_due(anchor: u8, due_at: GameTime) -> Result<GameTime, HousingError> {
    let due_date = Date::from_time(due_at).map_err(|_| HousingError::Arithmetic)?;
    if due_date.midnight() != due_at {
        return Err(HousingError::NotDue);
    }
    let previous_month = if due_date.month() == 1 {
        (
            due_date
                .year()
                .checked_sub(1)
                .ok_or(HousingError::Arithmetic)?,
            12,
        )
    } else {
        (due_date.year(), due_date.month() - 1)
    };
    if previous_month.0 < 2001 {
        return Err(HousingError::Arithmetic);
    }
    let mut month_last_day = 31;
    while month_last_day > 0
        && Date::new(previous_month.0, previous_month.1, month_last_day).is_err()
    {
        month_last_day -= 1;
    }
    if month_last_day == 0 {
        return Err(HousingError::Arithmetic);
    }
    let previous_day = anchor.min(month_last_day);
    let previous = Date::new(previous_month.0, previous_month.1, previous_day)
        .map_err(|_| HousingError::Arithmetic)?
        .midnight();
    if previous >= due_at {
        return Err(HousingError::NotDue);
    }
    Ok(previous)
}

/// Charges a single occupied portion of one calendar-month interval. Fractional cents are
/// rounded upward once per invoice, so every invoice differs from exact arithmetic by less than
/// one cent. Callers must not split/reopen this invoice per simulation tick.
fn prorated_rent(
    monthly_rent: Money,
    occupied_from: GameTime,
    due_at: GameTime,
    anchor: u8,
) -> Result<(Money, bool), HousingError> {
    let interval_start = previous_monthly_due(anchor, due_at)?;
    prorated_interval_rent(monthly_rent, occupied_from, due_at, interval_start, due_at)
}

/// Charges only the occupied portion of the active contractual interval. `interval_end` is the
/// lease's already-persisted next calendar due boundary; using `occupied_until` to infer the
/// interval would incorrectly ask for a previous month before the supported epoch when a lease
/// starts on 2001-01-01 and ends before its first due date.
fn prorated_interval_rent(
    monthly_rent: Money,
    occupied_from: GameTime,
    occupied_until: GameTime,
    interval_start: GameTime,
    interval_end: GameTime,
) -> Result<(Money, bool), HousingError> {
    if interval_start >= interval_end
        || occupied_from < interval_start
        || occupied_from > occupied_until
        || occupied_until > interval_end
    {
        return Err(HousingError::TimeRegression);
    }
    let period_seconds = i128::from(interval_end.value()) - i128::from(interval_start.value());
    if period_seconds <= 0 {
        return Err(HousingError::Arithmetic);
    }
    let occupied_seconds = i128::from(occupied_until.value()) - i128::from(occupied_from.value());
    let numerator = i128::from(monthly_rent.value())
        .checked_mul(occupied_seconds)
        .ok_or(HousingError::Arithmetic)?;
    let quotient = numerator / period_seconds;
    let remainder = numerator % period_seconds;
    let cents = quotient + i128::from(remainder != 0);
    Ok((
        Money::new(i64::try_from(cents).map_err(|_| HousingError::Arithmetic)?)
            .map_err(|_| HousingError::Arithmetic)?,
        remainder != 0,
    ))
}

fn unit_transition_valid(old: HousingUnit, new: HousingUnit) -> bool {
    old.unit_id == new.unit_id
        && old.building_id == new.building_id
        && old.owner_account == new.owner_account
}

fn account_spendable(snapshot: AccountSnapshot) -> Result<Money, HousingError> {
    if snapshot.account == 0 || snapshot.protected > snapshot.cash {
        return Err(HousingError::InvalidAccountSnapshot);
    }
    snapshot
        .cash
        .checked_sub(snapshot.protected)
        .map_err(|_| HousingError::InvalidAccountSnapshot)
}

pub fn affordability(
    rent: Money,
    budget: HouseholdBudget,
) -> Result<AffordabilityReport, HousingError> {
    if budget.household_id == 0 || budget.account.account == 0 || budget.people == 0 {
        return Err(HousingError::InvalidIdentity);
    }
    let residual = budget
        .net_income_per_month
        .checked_sub(budget.essential_costs_per_month);
    let residual = residual.ok();
    if !budget.income_window_complete {
        return Ok(AffordabilityReport {
            classification: Affordability::Unknown,
            rent_share_percent_floor: None,
            projected_residual_per_month: None,
            soft_target_percent: SOFT_RENT_SHARE_PERCENT as u8,
        });
    }
    let income = budget.net_income_per_month.value();
    let share = if income == 0 {
        if rent == Money::ZERO {
            Some(0)
        } else {
            Some(100)
        }
    } else {
        Some(((i128::from(rent.value()) * 100) / i128::from(income)).min(100) as u8)
    };
    let class = match residual {
        Some(r) if r >= rent => {
            if rent.value() as i128 * 100 <= income as i128 * i128::from(SOFT_RENT_SHARE_PERCENT) {
                Affordability::WithinSoftTarget
            } else {
                Affordability::AboveSoftTarget
            }
        }
        _ => Affordability::IncomeDoesNotCoverRentAndEssentials,
    };
    Ok(AffordabilityReport {
        classification: class,
        rent_share_percent_floor: share,
        projected_residual_per_month: residual,
        soft_target_percent: SOFT_RENT_SHARE_PERCENT as u8,
    })
}

pub fn plan_lease(
    current: HousingUnit,
    dwelling: DwellingFact,
    household: HouseholdBudget,
    terms: LeaseTerms,
) -> Result<LeasePlan, HousingError> {
    validate_unit(&current)?;
    validate_dwelling(dwelling)?;
    account_spendable(household.account)?;
    if dwelling.unit_id != current.unit_id
        || dwelling.building_id != current.building_id
        || dwelling.owner_account != current.owner_account
        || household.household_id == 0
        || household.account.account == current.owner_account
        || household.people == 0
        || household.people > dwelling.person_capacity
    {
        return Err(HousingError::InvalidDwelling);
    }
    if current.household_id != 0 || current.lease_id != 0 {
        return Err(HousingError::Occupied);
    }
    if terms.lease_id == 0
        || terms.clause_version == 0
        || terms.contract_source_id == 0
        || terms.landlord_authorization_id == 0
        || terms.tenant_consent_id == 0
        || terms.escrow_account == 0
        || terms.escrow_account == household.account.account
        || terms.escrow_account == current.owner_account
        || terms.starts_at.value() < 0
    {
        return Err(HousingError::InvalidTerms);
    }
    let anchor = monthly_anchor(terms.cycle)?;
    let next_due_at = next_monthly_due(anchor, terms.starts_at)?;
    let (first_due_amount, _) =
        prorated_rent(terms.rent_per_cycle, terms.starts_at, next_due_at, anchor)?;
    let spendable = account_spendable(household.account)?;
    if terms.deposit > spendable {
        return Err(HousingError::InsufficientCash);
    }
    let mut report = affordability(terms.rent_per_cycle, household)?;
    if household.income_observed_at > terms.starts_at
        || terms.starts_at.value() - household.income_observed_at.value()
            > i64::try_from(MAX_MONTHLY_INTERVAL_SECONDS).map_err(|_| HousingError::Arithmetic)?
    {
        report = AffordabilityReport {
            classification: Affordability::Unknown,
            rent_share_percent_floor: None,
            projected_residual_per_month: None,
            soft_target_percent: SOFT_RENT_SHARE_PERCENT as u8,
        };
    }
    let updated = HousingUnit {
        household_id: household.household_id,
        lease_id: terms.lease_id,
        clause_version: terms.clause_version,
        billing_anchor: u32::from(anchor),
        rent_per_cycle: terms.rent_per_cycle,
        deposit_held: terms.deposit,
        last_accounted_at: terms.starts_at,
        next_due_at,
        ..current
    };
    Ok(LeasePlan {
        unit_id: current.unit_id,
        expected_lease_id: current.lease_id,
        expected_household_id: current.household_id,
        updated_unit: updated,
        first_due_amount,
        deposit_transfer: CashTransfer {
            from_account: household.account.account,
            to_account: terms.escrow_account,
            amount: terms.deposit,
        },
        affordability: report,
        contract_source_id: terms.contract_source_id,
        dwelling_source_version: dwelling.source_version,
        income_source_version: household.income_source_version,
        new_contract_source_id: terms.contract_source_id,
        landlord_authorization_id: terms.landlord_authorization_id,
        tenant_consent_id: terms.tenant_consent_id,
    })
}

/// DEPRECATED for Engine/Store use: this legacy proposal calculates its own transfer and
/// outstanding-rent amount, which duplicates the native Timeline Claim path. Use
/// [`plan_rent_occurrence`] to update the original Claim and let Timeline own collection.
/// Kept temporarily for compatibility with isolated planner tests and callers during handoff.
#[deprecated(
    note = "Engine must use plan_rent_occurrence and Timeline Claim as sole debt/cash source"
)]
pub fn plan_rent_collection(
    unit: HousingUnit,
    household_account: u32,
    landlord_account: u32,
    now: GameTime,
    tenant_cash: AccountSnapshot,
    existing_arrears: Money,
) -> Result<RentCollectionPlan, HousingError> {
    validate_unit(&unit)?;
    let spendable = account_spendable(tenant_cash)?;
    if household_account == 0
        || tenant_cash.account != household_account
        || landlord_account != unit.owner_account
    {
        return Err(HousingError::InvalidIdentity);
    }
    if unit.household_id == 0 || unit.lease_id == 0 {
        return Err(HousingError::ExistingLeaseMismatch);
    }
    if (existing_arrears != Money::ZERO) != (unit.arrears_ref == unit.lease_id) {
        return Err(HousingError::ExistingLeaseMismatch);
    }
    if now < unit.next_due_at {
        return Err(HousingError::NotDue);
    }
    if now != unit.next_due_at {
        // Timeline owns repeated due boundaries. A late call must not collapse missed months.
        return Err(HousingError::NotDue);
    }
    if now < unit.last_accounted_at {
        return Err(HousingError::TimeRegression);
    }
    let (rent_due, rounded_up_fractional_cent) = prorated_rent(
        unit.rent_per_cycle,
        unit.last_accounted_at,
        now,
        unit.billing_anchor as u8,
    )?;
    let total_due = rent_due
        .checked_add(existing_arrears)
        .map_err(|_| HousingError::Arithmetic)?;
    let paid = Money::new(total_due.value().min(spendable.value()))
        .map_err(|_| HousingError::Arithmetic)?;
    let remaining_arrears = total_due
        .checked_sub(paid)
        .map_err(|_| HousingError::Arithmetic)?;
    let next_due_at = next_monthly_due(unit.billing_anchor as u8, now)?;
    let updated = HousingUnit {
        last_accounted_at: now,
        next_due_at,
        arrears_ref: if remaining_arrears == Money::ZERO {
            0
        } else {
            unit.lease_id
        },
        ..unit
    };
    Ok(RentCollectionPlan {
        unit_id: unit.unit_id,
        lease_id: unit.lease_id,
        due_at: now,
        expected_last_accounted_at: unit.last_accounted_at,
        updated_unit: updated,
        landlord_transfer: CashTransfer {
            from_account: household_account,
            to_account: landlord_account,
            amount: paid,
        },
        rounded_up_fractional_cent,
        remaining_arrears,
        debt_source_lease_id: unit.lease_id,
    })
}

/// Prepares one real-calendar rent occurrence against the native Timeline Claim.
///
/// The accepted lease may have already registered the exact first invoice as a future Claim. In
/// that sole case (due equals this boundary, unpaid/untouched Claim, no prior arrears, and Claim
/// principal equals the calculated first invoice), `accrual_increment` is zero to prevent a
/// duplicate first charge. Otherwise the invoice is added to the same lease Claim. Unpaid debt
/// keeps its earliest `Claim.due`; a fully paid old Claim starts the new occurrence at `now`.
///
/// This is a trusted Engine/calendar integration API, not a public user command. The Engine must
/// compare `expected_claim` against its Timeline shadow, apply `updated_claim`, run the existing
/// Claim payment phase once, reconcile arrears from the resulting Claim, and commit all state in
/// one Store/WAL transaction. There is deliberately no cash transfer or arrears amount here.
pub fn plan_rent_occurrence(
    unit: HousingUnit,
    household_account: u32,
    landlord_account: u32,
    due_at: GameTime,
    existing_claim: Option<crate::timeline::Claim>,
) -> Result<RentOccurrencePlan, HousingError> {
    validate_unit(&unit)?;
    if unit.household_id == 0 || unit.lease_id == 0 || household_account == 0 {
        return Err(HousingError::ExistingLeaseMismatch);
    }
    if landlord_account != unit.owner_account {
        return Err(HousingError::InvalidIdentity);
    }
    if due_at != unit.next_due_at {
        return Err(HousingError::NotDue);
    }
    if due_at < unit.last_accounted_at {
        return Err(HousingError::TimeRegression);
    }
    let anchor = unit.billing_anchor as u8;
    let interval_start = previous_monthly_due(anchor, unit.next_due_at)?;
    let (invoice, rounded_up_fractional_cent) = prorated_interval_rent(
        unit.rent_per_cycle,
        unit.last_accounted_at,
        due_at,
        interval_start,
        unit.next_due_at,
    )?;
    let lease_claim_id = u64::from(unit.lease_id);
    let (expected_claim, updated_claim, accrual_increment, earliest_due_at) = match existing_claim {
        None => {
            if unit.arrears_ref != 0 {
                return Err(HousingError::ExistingLeaseMismatch);
            }
            let claim = crate::timeline::Claim {
                id: lease_claim_id,
                payer: household_account,
                payee: landlord_account,
                due: u64::try_from(due_at.value()).map_err(|_| HousingError::Arithmetic)?,
                principal: invoice,
                paid: Money::ZERO,
                processed: false,
                source_ref: 0,
            };
            (None, claim, invoice, due_at)
        }
        Some(claim) => {
            if claim.id != lease_claim_id
                || claim.payer != household_account
                || claim.payee != landlord_account
                || claim.principal < claim.paid
            {
                return Err(HousingError::ExistingLeaseMismatch);
            }
            let claim_due =
                GameTime::new(i64::try_from(claim.due).map_err(|_| HousingError::Arithmetic)?)
                    .map_err(|_| HousingError::ExistingLeaseMismatch)?;
            if claim_due > due_at {
                return Err(HousingError::NotDue);
            }
            let outstanding = claim
                .principal
                .checked_sub(claim.paid)
                .map_err(|_| HousingError::Arithmetic)?;
            let preloaded_first_invoice = claim_due == due_at
                && !claim.processed
                && claim.paid == Money::ZERO
                && claim.principal == invoice
                && unit.arrears_ref == 0;
            if preloaded_first_invoice {
                (Some(claim), claim, Money::ZERO, due_at)
            } else {
                if claim_due == due_at
                    || (outstanding != Money::ZERO && unit.arrears_ref != unit.lease_id)
                    || (outstanding == Money::ZERO && unit.arrears_ref != 0)
                    || (claim_due < due_at && !claim.processed)
                {
                    return Err(HousingError::ExistingLeaseMismatch);
                }
                let principal = claim
                    .principal
                    .checked_add(invoice)
                    .map_err(|_| HousingError::Arithmetic)?;
                let due = if outstanding == Money::ZERO {
                    u64::try_from(due_at.value()).map_err(|_| HousingError::Arithmetic)?
                } else {
                    claim.due
                };
                let updated = crate::timeline::Claim {
                    principal,
                    due,
                    processed: false,
                    ..claim
                };
                let earliest =
                    GameTime::new(i64::try_from(due).map_err(|_| HousingError::Arithmetic)?)
                        .map_err(|_| HousingError::Arithmetic)?;
                (Some(claim), updated, invoice, earliest)
            }
        }
    };
    let next_due_at = next_monthly_due(anchor, due_at)?;
    let updated_unit = HousingUnit {
        last_accounted_at: due_at,
        next_due_at,
        // Provisional until the native Claim P2 payment completes in this same transaction.
        arrears_ref: unit.lease_id,
        ..unit
    };
    Ok(RentOccurrencePlan {
        occurrence_lease_id: unit.lease_id,
        occurrence_due_at: due_at,
        expected_unit: unit,
        updated_unit,
        expected_claim,
        updated_claim,
        accrual_increment,
        invoice,
        earliest_due_at,
        next_due_at,
        rounded_up_fractional_cent,
    })
}

/// Reconciles the housing-side Claim reference after Timeline has actually collected the due
/// occurrence or a later wage settlement. This stores no amount: only the lease-keyed reference
/// exists while the authoritative native Claim still has `principal > paid`.
pub fn plan_rent_claim_reference_sync(
    unit: HousingUnit,
    household_account: u32,
    claim: crate::timeline::Claim,
) -> Result<HousingPatch, HousingError> {
    validate_unit(&unit)?;
    if unit.household_id == 0
        || unit.lease_id == 0
        || household_account == 0
        || claim.id != u64::from(unit.lease_id)
        || claim.payer != household_account
        || claim.payee != unit.owner_account
        || claim.principal < claim.paid
        || !claim.processed
    {
        return Err(HousingError::ExistingLeaseMismatch);
    }
    let claim_due = GameTime::new(i64::try_from(claim.due).map_err(|_| HousingError::Arithmetic)?)
        .map_err(|_| HousingError::ExistingLeaseMismatch)?;
    if claim_due > unit.last_accounted_at {
        return Err(HousingError::ExistingLeaseMismatch);
    }
    let outstanding = claim
        .principal
        .checked_sub(claim.paid)
        .map_err(|_| HousingError::Arithmetic)?;
    let updated = HousingUnit {
        arrears_ref: if outstanding == Money::ZERO {
            0
        } else {
            unit.lease_id
        },
        ..unit
    };
    Ok(HousingPatch {
        expected: unit,
        updated,
    })
}

pub fn plan_move(
    state: &HousingState,
    household: HouseholdBudget,
    old_score: MoveScore,
    trigger: MoveTrigger,
    now: GameTime,
    candidates: &[MoveCandidate],
    existing_old_arrears_ref: u32,
) -> Result<MovePlan, MoveWait> {
    if candidates.len() > MAX_MOVE_CANDIDATES {
        return Err(MoveWait::CandidateLimit);
    }
    if household.household_id == 0 || household.account.account == 0 || household.people == 0 {
        return Err(MoveWait::EvidenceIncomplete);
    }
    if !household.income_window_complete {
        return Err(MoveWait::EvidenceIncomplete);
    }
    if household.income_observed_at > now
        || now.value() - household.income_observed_at.value()
            > i64::try_from(MAX_MONTHLY_INTERVAL_SECONDS).unwrap_or(i64::MAX)
    {
        return Err(MoveWait::EvidenceIncomplete);
    }
    if existing_old_arrears_ref
        != state
            .units
            .iter()
            .find(|unit| unit.household_id == household.household_id)
            .map(|unit| unit.arrears_ref)
            .unwrap_or(0)
    {
        return Err(MoveWait::EvidenceIncomplete);
    }
    if !old_score.fully_covered
        || old_score.lower > old_score.upper
        || old_score.upper > 100
        || old_score.upper >= LOW_SATISFACTION_THRESHOLD
        || old_score.environment_source == 0
        || old_score.public_service_source == 0
        || old_score.valid_through < now
        || old_score.assessed_at > now
    {
        return Err(MoveWait::EvidenceIncomplete);
    }
    if let Some(until) = trigger.cooldown_until {
        if now < until {
            return Err(MoveWait::Cooldown);
        }
    }
    let low_since = trigger.low_since.ok_or(MoveWait::SatisfactionWindow)?;
    let trigger_seconds =
        i128::from(low_since.value()) + i128::from(LOW_SATISFACTION_DAYS * 86_400);
    if !trigger.continuous_low_and_covered
        || trigger.covered_through < now
        || i128::from(now.value()) < trigger_seconds
    {
        return Err(MoveWait::SatisfactionWindow);
    }

    let old_index = state
        .units
        .iter()
        .position(|unit| unit.household_id == household.household_id)
        .ok_or(MoveWait::EvidenceIncomplete)?;
    let old = state.units[old_index];
    let spendable =
        account_spendable(household.account).map_err(|_| MoveWait::EvidenceIncomplete)?;
    let mut best: Option<(MoveCandidate, HousingUnit, HousingUnit, MovePlan)> = None;
    for candidate in candidates {
        if !candidate.score.fully_covered
            || candidate.score.lower > candidate.score.upper
            || candidate.score.upper > 100
            || candidate.score.environment_source == 0
            || candidate.score.public_service_source == 0
            || candidate.score.assessed_at > now
            || candidate.score.valid_through < now
            || candidate.score.hard_failure_mask & !old_score.hard_failure_mask != 0
        {
            continue;
        }
        let improvement = candidate.score.lower.saturating_sub(old_score.upper);
        if improvement < MIN_MOVE_IMPROVEMENT_POINTS {
            continue;
        }
        if !candidate.move_cost.reachable
            || candidate.move_cost.source_id == 0
            || candidate.move_cost.moving_provider_account == 0
            || candidate.move_cost.old_escrow_account == 0
            || (old.lease_id != 0 && candidate.move_cost.old_close_payee_account == 0)
            || (old.lease_id != 0
                && (candidate.move_cost.old_clause_version != old.clause_version
                    || candidate.move_cost.old_contract_exit_source_id == 0))
            || (old.deposit_held != Money::ZERO
                && candidate.move_cost.deposit_resolution_source_id == 0)
            || (candidate.move_cost.old_deposit_deduction_authorized != Money::ZERO
                && candidate.move_cost.old_deposit_deduction_account == 0)
        {
            continue;
        }
        if candidate.move_cost.valid_through < now {
            continue;
        }
        if validate_dwelling(candidate.dwelling).is_err()
            || candidate.dwelling.person_capacity < household.people
            || candidate.dwelling.owner_account == household.account.account
            || candidate.terms.lease_id == 0
            || candidate.terms.clause_version == 0
            || candidate.terms.escrow_account == 0
            || candidate.terms.escrow_account == household.account.account
            || candidate.terms.escrow_account == candidate.dwelling.owner_account
            || candidate.terms.contract_source_id == 0
            || candidate.terms.landlord_authorization_id == 0
            || candidate.terms.tenant_consent_id == 0
        {
            continue;
        }
        let new_index = match state
            .units
            .iter()
            .position(|unit| unit.unit_id == candidate.dwelling.unit_id)
        {
            Some(index) => index,
            None => continue,
        };
        let new_unit = state.units[new_index];
        if new_index == old_index
            || new_unit.household_id != 0
            || new_unit.lease_id != 0
            || candidate.dwelling.building_id != new_unit.building_id
            || candidate.dwelling.owner_account != new_unit.owner_account
        {
            continue;
        }
        if state
            .units
            .iter()
            .any(|row| row.unit_id != old.unit_id && row.lease_id == candidate.terms.lease_id)
        {
            continue;
        }
        let resolved_old_deposit = candidate
            .move_cost
            .old_deposit_refund_authorized
            .checked_add(candidate.move_cost.old_deposit_deduction_authorized)
            .map_err(|_| MoveWait::EvidenceIncomplete)?;
        if resolved_old_deposit > old.deposit_held {
            continue;
        }
        let effective_seconds =
            i128::from(now.value()) + i128::from(candidate.move_cost.travel_seconds);
        let effective_at = GameTime::new(
            i64::try_from(effective_seconds).map_err(|_| MoveWait::EvidenceIncomplete)?,
        )
        .map_err(|_| MoveWait::EvidenceIncomplete)?;
        if candidate.move_cost.valid_through < effective_at
            || candidate.score.valid_through < effective_at
        {
            continue;
        }
        if candidate.terms.starts_at != effective_at {
            continue;
        }
        let anchor = match monthly_anchor(candidate.terms.cycle) {
            Ok(anchor) => anchor,
            Err(_) => continue,
        };
        let effective_at = GameTime::new(
            i64::try_from(effective_seconds).map_err(|_| MoveWait::EvidenceIncomplete)?,
        )
        .map_err(|_| MoveWait::EvidenceIncomplete)?;
        let next_due_at = match next_monthly_due(anchor, effective_at) {
            Ok(due) => due,
            Err(_) => continue,
        };
        let new_first_due_amount = match prorated_rent(
            candidate.terms.rent_per_cycle,
            effective_at,
            next_due_at,
            anchor,
        ) {
            Ok((amount, _)) => amount,
            Err(_) => continue,
        };
        let updated_new = HousingUnit {
            household_id: household.household_id,
            lease_id: candidate.terms.lease_id,
            clause_version: candidate.terms.clause_version,
            billing_anchor: u32::from(anchor),
            rent_per_cycle: candidate.terms.rent_per_cycle,
            deposit_held: candidate.terms.deposit,
            last_accounted_at: effective_at,
            next_due_at,
            ..new_unit
        };
        let (old_final_due, final_rent_rounded_up_fractional_cent) = if old.lease_id == 0 {
            (Money::ZERO, false)
        } else {
            accrue_rent(old, effective_at).map_err(|_| MoveWait::EvidenceIncomplete)?
        };
        let old_close_due = old_final_due
            .checked_add(candidate.move_cost.old_lease_close_due)
            .map_err(|_| MoveWait::EvidenceIncomplete)?;
        let gross_cost = candidate
            .terms
            .deposit
            .checked_add(candidate.move_cost.moving_charge)
            .and_then(|cost| cost.checked_add(old_close_due))
            .map_err(|_| MoveWait::EvidenceIncomplete)?;
        let net_cost = gross_cost
            .checked_sub(candidate.move_cost.old_deposit_refund_authorized)
            .map_err(|_| MoveWait::EvidenceIncomplete)?;
        if net_cost > spendable {
            continue;
        }
        let affordability = affordability(candidate.terms.rent_per_cycle, household)
            .map_err(|_| MoveWait::EvidenceIncomplete)?;
        if affordability.classification == Affordability::IncomeDoesNotCoverRentAndEssentials {
            continue;
        }
        let effects = [
            CashTransfer {
                from_account: household.account.account,
                to_account: candidate.terms.escrow_account,
                amount: candidate.terms.deposit,
            },
            CashTransfer {
                from_account: household.account.account,
                to_account: if candidate.move_cost.old_close_payee_account == 0 {
                    household.account.account
                } else {
                    candidate.move_cost.old_close_payee_account
                },
                amount: old_close_due,
            },
            CashTransfer {
                from_account: household.account.account,
                to_account: candidate.move_cost.moving_provider_account,
                amount: candidate.move_cost.moving_charge,
            },
            CashTransfer {
                from_account: candidate.move_cost.old_escrow_account,
                to_account: household.account.account,
                amount: candidate.move_cost.old_deposit_refund_authorized,
            },
            CashTransfer {
                from_account: candidate.move_cost.old_escrow_account,
                to_account: if candidate.move_cost.old_deposit_deduction_account == 0 {
                    candidate.move_cost.old_escrow_account
                } else {
                    candidate.move_cost.old_deposit_deduction_account
                },
                amount: candidate.move_cost.old_deposit_deduction_authorized,
            },
        ];
        let updated_old = HousingUnit {
            household_id: 0,
            lease_id: 0,
            clause_version: 0,
            rent_per_cycle: Money::ZERO,
            deposit_held: Money::ZERO,
            last_accounted_at: GameTime::ZERO,
            next_due_at: GameTime::ZERO,
            billing_anchor: 0,
            arrears_ref: 0,
            ..old
        };
        let cooldown_seconds = effective_seconds + i128::from(MOVE_COOLDOWN_DAYS * 86_400);
        let plan = MovePlan {
            household_id: household.household_id,
            old_unit_id: old.unit_id,
            new_unit_id: new_unit.unit_id,
            expected_old_lease_id: old.lease_id,
            expected_new_lease_id: new_unit.lease_id,
            updated_old_unit: updated_old,
            updated_new_unit: updated_new,
            new_first_due_amount,
            effects,
            effect_count: 5,
            old_arrears_ref_retained_by_ledger: existing_old_arrears_ref,
            old_escrow_account: candidate.move_cost.old_escrow_account,
            old_deposit_liability_retained_by_ledger: old
                .deposit_held
                .checked_sub(resolved_old_deposit)
                .map_err(|_| MoveWait::EvidenceIncomplete)?,
            final_rent_rounded_up_fractional_cent,
            score_delta_lower_bound: improvement,
            effective_at,
            cooldown_until: GameTime::new(
                i64::try_from(cooldown_seconds).map_err(|_| MoveWait::EvidenceIncomplete)?,
            )
            .map_err(|_| MoveWait::EvidenceIncomplete)?,
            cost_source_id: candidate.move_cost.source_id,
            contract_exit_source_id: candidate.move_cost.old_contract_exit_source_id,
            new_contract_source_id: candidate.terms.contract_source_id,
            new_landlord_authorization_id: candidate.terms.landlord_authorization_id,
            tenant_consent_id: candidate.terms.tenant_consent_id,
            deposit_resolution_source_id: candidate.move_cost.deposit_resolution_source_id,
            environment_source: candidate.score.environment_source,
            public_service_source: candidate.score.public_service_source,
            dwelling_source_version: candidate.dwelling.source_version,
            income_source_version: household.income_source_version,
        };
        if best.as_ref().is_none_or(|(prior, _, _, _)| {
            candidate.score.lower > prior.score.lower
                || (candidate.score.lower == prior.score.lower
                    && candidate.dwelling.unit_id < prior.dwelling.unit_id)
        }) {
            best = Some((*candidate, updated_old, updated_new, plan));
        }
    }
    let Some((_candidate, _updated_old, _updated_new, plan)) = best else {
        return Err(if candidates.iter().all(|c| !c.move_cost.reachable) {
            MoveWait::NoReachableCandidate
        } else {
            MoveWait::CandidateNotBetter
        });
    };
    Ok(plan)
}

fn accrue_rent(unit: HousingUnit, now: GameTime) -> Result<(Money, bool), HousingError> {
    if now < unit.last_accounted_at {
        return Err(HousingError::TimeRegression);
    }
    if now > unit.next_due_at || unit.next_due_at <= unit.last_accounted_at {
        return Err(HousingError::NotDue);
    }
    let interval_start = previous_monthly_due(unit.billing_anchor as u8, unit.next_due_at)?;
    prorated_interval_rent(
        unit.rent_per_cycle,
        unit.last_accounted_at,
        now,
        interval_start,
        unit.next_due_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(seconds: i64) -> GameTime {
        GameTime::new(seconds).unwrap()
    }

    fn money(cents: i64) -> Money {
        Money::new(cents).unwrap()
    }

    fn dwelling(unit_id: u32, building_id: u32, owner: u32) -> DwellingFact {
        DwellingFact {
            unit_id,
            building_id,
            owner_account: owner,
            building_unit_capacity: 2,
            person_capacity: 4,
            delivered: true,
            legal_to_occupy: true,
            has_legal_access: true,
            source_version: 3,
        }
    }

    fn household(cash: i64, protected: i64) -> HouseholdBudget {
        HouseholdBudget {
            household_id: 101,
            account: AccountSnapshot {
                account: 201,
                cash: money(cash),
                protected: money(protected),
            },
            people: 3,
            net_income_per_month: money(100_000),
            essential_costs_per_month: money(10_000),
            income_observed_at: GameTime::ZERO,
            income_window_complete: true,
            income_source_version: 8,
        }
    }

    fn terms(lease_id: u32, starts_at: GameTime) -> LeaseTerms {
        LeaseTerms {
            lease_id,
            clause_version: 2,
            contract_source_id: 600 + u64::from(lease_id),
            landlord_authorization_id: 700 + u64::from(lease_id),
            tenant_consent_id: 800 + u64::from(lease_id),
            rent_per_cycle: money(50_000),
            cycle: Cycle::Monthly { anchor: 1 },
            deposit: money(10_000),
            escrow_account: 301,
            starts_at,
        }
    }

    fn opening_dwelling(fact: DwellingFact) -> OpeningDwelling {
        OpeningDwelling {
            fact,
            origin: DwellingOriginKind::ImmutableGenesisEndowment,
            asset_source_id: 7001,
        }
    }

    fn opening_with_records(
        dwellings: &[OpeningDwelling],
        offers: &[GenesisLeaseOffer],
    ) -> OpeningHousing {
        let mut dwelling_rows = Rows::new().unwrap();
        for dwelling in dwellings {
            dwelling_rows.try_push(*dwelling).unwrap();
        }
        let mut offer_rows = Rows::new().unwrap();
        for offer in offers {
            offer_rows.try_push(*offer).unwrap();
        }
        OpeningHousing {
            unit_limit: dwellings.len() as u8,
            dwellings: dwelling_rows,
            households: {
                let mut rows = Rows::new().unwrap();
                for offer in offers {
                    if !rows
                        .iter()
                        .any(|row: &OpeningHousehold| row.household_id == offer.household_id)
                    {
                        rows.try_push(OpeningHousehold {
                            household_id: offer.household_id,
                            account: offer.tenant_account,
                            people: 3,
                            source_id: 8000 + u64::from(offer.household_id),
                        })
                        .unwrap();
                    }
                }
                rows
            },
            lease_offers: offer_rows,
        }
    }

    fn opening_with_offer(fact: DwellingFact, offer: GenesisLeaseOffer) -> OpeningHousing {
        opening_with_records(&[opening_dwelling(fact)], &[offer])
    }

    fn genesis_offer(
        fact: DwellingFact,
        tenant_account: u32,
        terms: LeaseTerms,
    ) -> GenesisLeaseOffer {
        GenesisLeaseOffer {
            offer_id: 9001,
            world_id: 11,
            branch_id: 12,
            unit_id: fact.unit_id,
            building_id: fact.building_id,
            owner_account: fact.owner_account,
            tenant_account,
            household_id: 101,
            valid_from: GameTime::ZERO,
            valid_until: Date::new(2001, 2, 1).unwrap().midnight(),
            asset_source_id: 7001,
            authority_source_id: 7002,
            contract_source_id: terms.contract_source_id,
            landlord_authorization_id: terms.landlord_authorization_id,
            tenant_consent_id: terms.tenant_consent_id,
            lease_id: terms.lease_id,
            clause_version: terms.clause_version,
            rent_per_cycle: terms.rent_per_cycle,
            cycle: terms.cycle,
            deposit: terms.deposit,
            escrow_account: terms.escrow_account,
            starts_at: terms.starts_at,
            terms_sha256: lease_terms_sha256(terms).unwrap(),
        }
    }

    #[test]
    fn lease_offer_resolves_explicit_registered_household_identity_and_people() {
        let fact = dwelling(1, 10, 401);
        let terms = terms(201, GameTime::ZERO);
        let offer = genesis_offer(fact, 202, terms);
        let opening = opening_with_offer(fact, offer);
        let authorization = validate_genesis_lease_offer(
            &opening,
            offer.offer_id,
            fact,
            offer.world_id,
            offer.branch_id,
            offer.tenant_account,
            GameTime::ZERO,
            terms,
        )
        .unwrap();
        assert_eq!(authorization.household_id(), 101);
        assert_eq!(authorization.people(), 3);

        let mut wrong_account = opening.clone();
        wrong_account.households[0].account = 203;
        assert_eq!(wrong_account.validate(), Err(HousingError::OfferMismatch));
        let mut over_capacity = opening;
        over_capacity.households[0].people = fact.person_capacity + 1;
        assert_eq!(over_capacity.validate(), Err(HousingError::OfferMismatch));
    }

    #[test]
    fn housing_row_is_exactly_one_64_byte_unit_not_per_person_state() {
        assert_eq!(std::mem::size_of::<HousingUnit>(), 64);
        let small_world = HousingState::new(4).unwrap();
        assert_eq!(small_world.unit_limit(), 4);
        assert_eq!(small_world.units().len(), 0);
        assert_eq!(small_world.units.capacity(), 4);
        assert_ne!(small_world.units.capacity(), MAX_M0_HOUSING_UNITS);
        assert_eq!(
            MAX_VALIDATION_SCRATCH_BYTES,
            MAX_M0_HOUSING_UNITS
                * (std::mem::size_of::<DwellingFact>() + 4 * std::mem::size_of::<u64>())
        );
    }

    #[test]
    fn bounded_state_serde_restore_copy_and_reset_reuse_configured_backing() {
        let mut source = HousingState::new(2).unwrap();
        source.register(dwelling(2, 10, 401)).unwrap();
        source.register(dwelling(1, 10, 401)).unwrap();
        let allocation = source.units.as_ptr();
        let encoded = serde_json::to_string(&source).unwrap();
        let decoded: HousingState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, source);

        let mut restored = HousingState::new(2).unwrap();
        let restore_allocation = restored.units.as_ptr();
        let reordered = encoded.replacen("{\"unit_limit\":2,\"units\":", "{\"units\":", 1);
        let reordered = reordered.replacen("]}", "],\"unit_limit\":2}", 1);
        let mut deserializer = serde_json::Deserializer::from_str(&reordered);
        crate::restore_into::Restore::restore(&mut restored, &mut deserializer).unwrap();
        deserializer.end().unwrap();
        assert_eq!(restored, source);
        assert_eq!(restored.units.as_ptr(), restore_allocation);

        let mut copy = HousingState::new(2).unwrap();
        let copy_allocation = copy.units.as_ptr();
        copy.copy_from(&source).unwrap();
        assert_eq!(copy, source);
        assert_eq!(copy.units.as_ptr(), copy_allocation);
        copy.reset();
        assert!(copy.units.is_empty());
        assert!(copy.consumed_offer_ids.is_empty());
        assert_eq!(copy.units.capacity(), 2);
        assert_eq!(source.units.as_ptr(), allocation);
    }

    #[test]
    fn genesis_offer_binds_immutable_owner_exact_terms_and_calendar_validity() {
        assert!(size_of::<GenesisLeaseOffer>() <= 192);
        let fact = dwelling(1, 10, 401);
        let terms = terms(31, GameTime::ZERO);
        let offer = genesis_offer(fact, 201, terms);
        let opening = opening_with_offer(fact, offer);
        assert!(validate_genesis_lease_offer(
            &opening,
            offer.offer_id,
            fact,
            11,
            12,
            201,
            GameTime::ZERO,
            terms
        )
        .is_ok());

        let expired_at = Date::new(2001, 2, 2).unwrap().midnight();
        assert_eq!(
            validate_genesis_lease_offer(
                &opening,
                offer.offer_id,
                fact,
                11,
                12,
                201,
                expired_at,
                terms
            ),
            Err(HousingError::OfferExpired)
        );

        let mut changed_terms = terms;
        changed_terms.rent_per_cycle = money(50_001);
        assert_eq!(
            validate_genesis_lease_offer(
                &opening,
                offer.offer_id,
                fact,
                11,
                12,
                201,
                GameTime::ZERO,
                changed_terms
            ),
            Err(HousingError::OfferMismatch)
        );

        let wrong_owner = dwelling(1, 10, 402);
        assert_eq!(
            validate_genesis_lease_offer(
                &opening,
                offer.offer_id,
                wrong_owner,
                11,
                12,
                201,
                GameTime::ZERO,
                terms
            ),
            Err(HousingError::OfferMismatch)
        );
        let wrong_unit = dwelling(2, 10, 401);
        assert_eq!(
            validate_genesis_lease_offer(
                &opening,
                offer.offer_id,
                wrong_unit,
                11,
                12,
                201,
                GameTime::ZERO,
                terms
            ),
            Err(HousingError::OfferMismatch)
        );
        assert_eq!(
            validate_genesis_lease_offer(
                &opening,
                offer.offer_id,
                fact,
                11,
                12,
                999,
                GameTime::ZERO,
                terms
            ),
            Err(HousingError::OfferMismatch)
        );
        let mut wrong_payee_terms = terms;
        wrong_payee_terms.escrow_account = 302;
        assert_eq!(
            validate_genesis_lease_offer(
                &opening,
                offer.offer_id,
                fact,
                11,
                12,
                201,
                GameTime::ZERO,
                wrong_payee_terms
            ),
            Err(HousingError::OfferMismatch)
        );
    }

    #[test]
    fn genesis_offer_consumption_survives_vacancy_copy_and_cold_restore() {
        let fact = dwelling(1, 10, 401);
        let terms = terms(31, GameTime::ZERO);
        let offer = genesis_offer(fact, 201, terms);
        let opening = opening_with_offer(fact, offer);
        let auth = validate_genesis_lease_offer(
            &opening,
            offer.offer_id,
            fact,
            11,
            12,
            201,
            GameTime::ZERO,
            terms,
        )
        .unwrap();
        let mut state = HousingState::new(1).unwrap();
        state.register(fact).unwrap();
        let plan = plan_lease(state.units()[0], fact, household(100_000, 0), terms).unwrap();
        state.apply_genesis_offer_lease(plan, auth).unwrap();
        assert_eq!(
            state.consumed_genesis_offer_ids().collect::<Vec<_>>(),
            [offer.offer_id]
        );

        let occupied = state.units()[0];
        let mut vacant = occupied;
        vacant.household_id = 0;
        vacant.lease_id = 0;
        vacant.arrears_ref = 0;
        vacant.clause_version = 0;
        vacant.billing_anchor = 0;
        vacant.rent_per_cycle = Money::ZERO;
        vacant.deposit_held = Money::ZERO;
        vacant.last_accounted_at = GameTime::ZERO;
        vacant.next_due_at = GameTime::ZERO;
        state
            .apply_transaction(&[HousingPatch {
                expected: occupied,
                updated: vacant,
            }])
            .unwrap();

        let encoded = serde_json::to_string(&state).unwrap();
        let reordered =
            serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&encoded).unwrap())
                .unwrap();
        let mut restored = HousingState::new(1).unwrap();
        let mut decoder = serde_json::Deserializer::from_str(&reordered);
        restored
            .restore_with_snapshot(&mut decoder, &[fact])
            .unwrap();
        decoder.end().unwrap();
        assert_eq!(restored, state);
        let replay = plan_lease(restored.units()[0], fact, household(100_000, 0), terms).unwrap();
        assert_eq!(
            restored.apply_genesis_offer_lease(replay, auth),
            Err(HousingError::OfferConsumed)
        );

        let mut copied = HousingState::new(1).unwrap();
        copied.copy_from(&restored).unwrap();
        assert_eq!(
            copied.consumed_genesis_offer_ids().collect::<Vec<_>>(),
            [offer.offer_id]
        );
        assert_eq!(
            copied.owned_capacity_bytes(),
            size_of::<HousingUnit>() + size_of::<ConsumedOfferId>()
        );
    }

    #[test]
    fn genesis_offer_acceptance_cannot_double_occupy_household_and_failed_offer_stays_unused() {
        let first_fact = dwelling(1, 10, 401);
        let second_fact = dwelling(2, 10, 401);
        let first_terms = terms(31, GameTime::ZERO);
        let mut second_terms = terms(32, GameTime::ZERO);
        second_terms.contract_source_id = 632;
        second_terms.landlord_authorization_id = 732;
        second_terms.tenant_consent_id = 832;
        let mut first_offer = genesis_offer(first_fact, 201, first_terms);
        let mut second_offer = genesis_offer(second_fact, 201, second_terms);
        first_offer.offer_id = 9001;
        second_offer.offer_id = 9002;
        second_offer.asset_source_id = 7003;
        second_offer.authority_source_id = 7004;
        let opening = opening_with_records(
            &[
                opening_dwelling(first_fact),
                OpeningDwelling {
                    asset_source_id: 7003,
                    ..opening_dwelling(second_fact)
                },
            ],
            &[first_offer, second_offer],
        );
        let first_auth = validate_genesis_lease_offer(
            &opening,
            first_offer.offer_id,
            first_fact,
            11,
            12,
            201,
            GameTime::ZERO,
            first_terms,
        )
        .unwrap();
        let second_auth = validate_genesis_lease_offer(
            &opening,
            second_offer.offer_id,
            second_fact,
            11,
            12,
            201,
            GameTime::ZERO,
            second_terms,
        )
        .unwrap();
        let mut state = HousingState::new(2).unwrap();
        state.register(first_fact).unwrap();
        state.register(second_fact).unwrap();
        let first_plan = plan_lease(
            state.units()[0],
            first_fact,
            household(100_000, 0),
            first_terms,
        )
        .unwrap();
        state
            .apply_genesis_offer_lease(first_plan, first_auth)
            .unwrap();
        let second_plan = plan_lease(
            state.units()[1],
            second_fact,
            household(100_000, 0),
            second_terms,
        )
        .unwrap();
        assert_eq!(
            state.apply_genesis_offer_lease(second_plan, second_auth),
            Err(HousingError::HouseholdAlreadyHoused)
        );
        assert_eq!(
            state.consumed_genesis_offer_ids().collect::<Vec<_>>(),
            vec![first_offer.offer_id]
        );
    }

    #[test]
    fn restore_failure_clears_reused_target_before_retry() {
        let mut source = HousingState::new(2).unwrap();
        source.register(dwelling(1, 10, 401)).unwrap();
        let encoded = serde_json::to_string(&source).unwrap();
        let mut target = HousingState::new(2).unwrap();
        let malformed = encoded.replace("\"unit_limit\":2", "\"unit_limit\":3");
        let mut failed = serde_json::Deserializer::from_str(&malformed);
        assert!(crate::restore_into::Restore::restore(&mut target, &mut failed).is_err());
        assert!(target.units.is_empty());
        let mut retry = serde_json::Deserializer::from_str(&encoded);
        crate::restore_into::Restore::restore(&mut target, &mut retry).unwrap();
        retry.end().unwrap();
        assert_eq!(target, source);
    }

    #[test]
    fn snapshot_checked_restore_clears_target_before_retry() {
        let mut source = HousingState::new(1).unwrap();
        source.register(dwelling(1, 10, 401)).unwrap();
        let encoded = serde_json::to_string(&source).unwrap();

        let mut target = HousingState::new(1).unwrap();
        target.register(dwelling(99, 90, 901)).unwrap();
        let wrong = [dwelling(1, 10, 402)];
        let mut failed = serde_json::Deserializer::from_str(&encoded);
        assert!(target.restore_with_snapshot(&mut failed, &wrong).is_err());
        assert!(target.units().is_empty());

        let valid = [dwelling(1, 10, 401)];
        let mut retry = serde_json::Deserializer::from_str(&encoded);
        assert!(target.restore_with_snapshot(&mut retry, &valid).is_ok());
        retry.end().unwrap();
        assert_eq!(target, source);
    }

    #[test]
    fn genesis_endowment_registration_is_bounded_and_source_identified() {
        let mut dwellings = Rows::new().unwrap();
        dwellings
            .try_push(OpeningDwelling {
                fact: dwelling(1, 10, 401),
                origin: DwellingOriginKind::ImmutableGenesisEndowment,
                asset_source_id: 7001,
            })
            .unwrap();
        dwellings
            .try_push(OpeningDwelling {
                fact: dwelling(2, 10, 401),
                origin: DwellingOriginKind::ImmutableGenesisEndowment,
                asset_source_id: 7002,
            })
            .unwrap();
        let opening = OpeningHousing {
            unit_limit: 2,
            dwellings,
            households: Rows::new().unwrap(),
            lease_offers: Rows::new().unwrap(),
        };
        let state = HousingState::from_opening(&opening).unwrap();
        assert_eq!(state.unit_limit(), 2);
        assert_eq!(state.units().len(), 2);
        assert!(HousingState::new(MAX_M0_HOUSING_UNITS + 1).is_err());
    }

    #[test]
    fn restore_structure_rejects_duplicate_household_lease_and_unsorted_units() {
        let mut state = HousingState::new(3).unwrap();
        state.register(dwelling(2, 10, 401)).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        assert_eq!(state.units()[0].unit_id, 1);
        let first_due = Date::new(2001, 2, 1).unwrap().midnight();
        let occupied = HousingUnit {
            household_id: 77,
            lease_id: 88,
            rent_per_cycle: money(1),
            billing_anchor: 1,
            last_accounted_at: time(0),
            next_due_at: first_due,
            ..state.units()[0]
        };
        state.units[0] = occupied;
        state.units[1] = HousingUnit {
            unit_id: 2,
            household_id: 77,
            lease_id: 89,
            rent_per_cycle: money(1),
            billing_anchor: 1,
            last_accounted_at: time(0),
            next_due_at: first_due,
            ..state.units[1]
        };
        assert_eq!(
            state.validate_structure(),
            Err(HousingError::DuplicateIdentity)
        );

        state.units[1].household_id = 78;
        state.units[1].lease_id = 88;
        assert_eq!(
            state.validate_structure(),
            Err(HousingError::DuplicateIdentity)
        );

        state.units[1].lease_id = 89;
        state.units.swap(0, 1);
        assert_eq!(state.validate_structure(), Err(HousingError::DuplicateUnit));
    }

    #[test]
    fn restore_snapshot_requires_exact_legal_units_and_building_capacity() {
        let mut state = HousingState::new(2).unwrap();
        state.register(dwelling(2, 10, 401)).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        let snapshot = [dwelling(1, 10, 401), dwelling(2, 10, 401)];
        assert!(state.validate_with_snapshot(&snapshot).is_ok());
        assert_eq!(
            state.validate_with_snapshot(&snapshot[..1]),
            Err(HousingError::SnapshotMismatch)
        );
        let over_capacity = [
            DwellingFact {
                building_unit_capacity: 1,
                ..snapshot[0]
            },
            DwellingFact {
                building_unit_capacity: 1,
                ..snapshot[1]
            },
        ];
        assert_eq!(
            state.validate_with_snapshot(&over_capacity),
            Err(HousingError::CapacityExceeded)
        );
        let conflicting_capacity = [
            snapshot[0],
            DwellingFact {
                building_unit_capacity: 3,
                ..snapshot[1]
            },
        ];
        assert_eq!(
            state.validate_with_snapshot(&conflicting_capacity),
            Err(HousingError::SnapshotMismatch)
        );
        let demolished = [
            snapshot[0],
            DwellingFact {
                delivered: false,
                ..snapshot[1]
            },
        ];
        assert_eq!(
            state.validate_with_snapshot(&demolished),
            Err(HousingError::InvalidDwelling)
        );
    }

    #[test]
    fn one_household_cannot_occupy_two_units_and_physical_capacity_is_enforced() {
        let mut state = HousingState::new(2).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        state.register(dwelling(2, 10, 401)).unwrap();
        let first = plan_lease(
            state.units()[0],
            dwelling(1, 10, 401),
            household(100_000, 0),
            terms(900, time(0)),
        )
        .unwrap();
        state.apply_lease(first).unwrap();
        let second = plan_lease(
            state.units()[1],
            dwelling(2, 10, 401),
            household(100_000, 0),
            terms(901, time(0)),
        )
        .unwrap();
        assert_eq!(
            state.apply_lease(second),
            Err(HousingError::HouseholdAlreadyHoused)
        );
        let mut other = household(100_000, 0);
        other.household_id = 102;
        other.people = 5;
        assert_eq!(
            plan_lease(
                state.units()[1],
                dwelling(2, 10, 401),
                other,
                terms(902, time(0))
            ),
            Err(HousingError::InvalidDwelling)
        );
    }

    #[test]
    fn prepared_housing_transaction_is_all_or_nothing_on_stale_second_row() {
        let mut state = HousingState::new(2).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        state.register(dwelling(2, 10, 401)).unwrap();
        let before = state.units.to_vec();
        let first = HousingUnit {
            household_id: 101,
            lease_id: 901,
            clause_version: 2,
            billing_anchor: 1,
            rent_per_cycle: money(10_000),
            last_accounted_at: time(0),
            next_due_at: time(31 * 86_400),
            ..state.units[0]
        };
        let mut stale = state.units[1];
        stale.owner_account = 999;
        assert_eq!(
            state.apply_transaction(&[
                HousingPatch {
                    expected: state.units[0],
                    updated: first,
                },
                HousingPatch {
                    expected: stale,
                    updated: state.units[1],
                },
            ]),
            Err(HousingError::StalePlan)
        );
        assert_eq!(state.units, before);
    }

    #[test]
    fn new_dwellings_require_an_explicit_capacity_expansion() {
        let mut state = HousingState::new(1).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        assert_eq!(
            state.register(dwelling(2, 10, 401)),
            Err(HousingError::UnitLimit)
        );
        state.extend_capacity(2).unwrap();
        state.register(dwelling(2, 10, 401)).unwrap();
        assert_eq!(state.units().len(), 2);
        assert_eq!(state.unit_limit(), 2);
        state.extend_capacity(3).unwrap();
        assert_eq!(
            state.register(dwelling(3, 10, 401)),
            Err(HousingError::CapacityExceeded)
        );
    }

    #[test]
    fn protected_cash_cannot_fund_deposit_but_35_percent_is_only_a_soft_preference() {
        let fact = dwelling(1, 10, 401);
        let vacant = HousingUnit {
            unit_id: 1,
            building_id: 10,
            owner_account: 401,
            ..HousingUnit::default()
        };
        let mut poor_liquidity = household(50_000, 45_000);
        assert_eq!(
            plan_lease(vacant, fact, poor_liquidity, terms(900, time(0))),
            Err(HousingError::InsufficientCash)
        );

        poor_liquidity.account.cash = money(100_000);
        poor_liquidity.account.protected = money(0);
        poor_liquidity.net_income_per_month = money(100_000);
        poor_liquidity.essential_costs_per_month = money(0);
        let mut expensive = terms(901, time(0));
        expensive.rent_per_cycle = money(50_000);
        let plan = plan_lease(vacant, fact, poor_liquidity, expensive).unwrap();
        assert_eq!(
            plan.affordability.classification,
            Affordability::AboveSoftTarget
        );
        assert_eq!(plan.affordability.rent_share_percent_floor, Some(50));
    }

    #[test]
    #[allow(deprecated)]
    fn rent_is_billed_on_calendar_month_anchor_and_unpaid_balance_is_a_ledger_effect() {
        let fact = dwelling(1, 10, 401);
        let vacant = HousingUnit {
            unit_id: 1,
            building_id: 10,
            owner_account: 401,
            ..HousingUnit::default()
        };
        let plan = plan_lease(vacant, fact, household(100_000, 0), terms(900, time(0))).unwrap();
        assert_eq!(
            plan_rent_collection(
                plan.updated_unit,
                201,
                401,
                time(30 * 86_400),
                household(100_000, 0).account,
                Money::ZERO
            ),
            Err(HousingError::NotDue)
        );
        let due = plan_rent_collection(
            plan.updated_unit,
            201,
            401,
            time(31 * 86_400),
            AccountSnapshot {
                account: 201,
                cash: money(5_000),
                protected: money(1_000),
            },
            money(0),
        )
        .unwrap();
        assert_eq!(due.landlord_transfer.amount, money(4_000));
        assert_eq!(due.remaining_arrears, money(46_000));
        assert_eq!(due.updated_unit.last_accounted_at, time(31 * 86_400));
    }

    #[test]
    fn rent_occurrences_accumulate_on_one_timeline_claim_and_keep_oldest_due() {
        let mut state = HousingState::new(1).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        let lease = plan_lease(
            state.units()[0],
            dwelling(1, 10, 401),
            household(200_000, 0),
            terms(900, time(0)),
        )
        .unwrap();
        state.apply_lease(lease).unwrap();
        let first_due = Date::new(2001, 2, 1).unwrap().midnight();
        let second_due = Date::new(2001, 3, 1).unwrap().midnight();
        let third_due = Date::new(2001, 4, 1).unwrap().midnight();
        let fourth_due = Date::new(2001, 5, 1).unwrap().midnight();
        let first_claim = crate::timeline::Claim {
            id: 900,
            payer: 201,
            payee: 401,
            due: first_due.value() as u64,
            principal: lease.first_due_amount,
            paid: Money::ZERO,
            processed: false,
            source_ref: 0,
        };

        // Lease acceptance has already placed the first invoice on the native Claim. The due
        // event recognizes that exact row instead of adding the first invoice a second time.
        let first =
            plan_rent_occurrence(state.units()[0], 201, 401, first_due, Some(first_claim)).unwrap();
        assert_eq!(first.accrual_increment, Money::ZERO);
        assert_eq!(first.invoice, money(50_000));
        assert_eq!(first.updated_claim.principal, money(50_000));
        assert_eq!(first.updated_claim.paid, Money::ZERO);
        assert_eq!(first.earliest_due_at, first_due);
        assert_eq!(first.next_due_at, second_due);
        state.apply_rent_occurrence(first).unwrap();

        // Timeline alone records this partial payment. Housing stores only its Claim reference.
        let mut after_first_payment = first.updated_claim;
        after_first_payment.paid = money(20_000);
        after_first_payment.processed = true;
        let sync =
            plan_rent_claim_reference_sync(state.units()[0], 201, after_first_payment).unwrap();
        assert_eq!(sync.updated.arrears_ref, 900);
        state.apply_transaction(&[sync]).unwrap();

        // Month two adds one invoice, preserves cumulative paid, and leaves the original overdue
        // date untouched. The D Claim payment phase may then pay another 10,000 cents.
        let second = plan_rent_occurrence(
            state.units()[0],
            201,
            401,
            second_due,
            Some(after_first_payment),
        )
        .unwrap();
        assert_eq!(second.accrual_increment, money(50_000));
        assert_eq!(second.updated_claim.principal, money(100_000));
        assert_eq!(second.updated_claim.paid, money(20_000));
        assert_eq!(
            second
                .updated_claim
                .principal
                .checked_sub(second.updated_claim.paid)
                .unwrap(),
            money(80_000)
        );
        assert_eq!(second.updated_claim.due, first_due.value() as u64);
        assert_eq!(second.earliest_due_at, first_due);
        assert_eq!(second.next_due_at, third_due);
        state.apply_rent_occurrence(second).unwrap();
        let mut after_second_payment = second.updated_claim;
        after_second_payment.paid = money(30_000);
        after_second_payment.processed = true;
        let sync =
            plan_rent_claim_reference_sync(state.units()[0], 201, after_second_payment).unwrap();
        state.apply_transaction(&[sync]).unwrap();

        // The third unpaid occurrence continues the same bounded Claim identity. Old age and
        // cumulative paid are unchanged while principal grows by exactly one invoice.
        let third = plan_rent_occurrence(
            state.units()[0],
            201,
            401,
            third_due,
            Some(after_second_payment),
        )
        .unwrap();
        assert_eq!(third.updated_claim.id, 900);
        assert_eq!(third.accrual_increment, money(50_000));
        assert_eq!(third.updated_claim.principal, money(150_000));
        assert_eq!(third.updated_claim.paid, money(30_000));
        assert_eq!(third.updated_claim.due, first_due.value() as u64);
        assert_eq!(third.earliest_due_at, first_due);
        assert_eq!(third.next_due_at, fourth_due);
        state.apply_rent_occurrence(third).unwrap();

        // Later wages fully clear the original debt. The pointer disappears; a later invoice
        // reuses the lease Claim identity but starts its new due-age at the new occurrence.
        let mut fully_settled = third.updated_claim;
        fully_settled.paid = fully_settled.principal;
        fully_settled.processed = true;
        let sync = plan_rent_claim_reference_sync(state.units()[0], 201, fully_settled).unwrap();
        assert_eq!(sync.updated.arrears_ref, 0);
        state.apply_transaction(&[sync]).unwrap();
        let fourth =
            plan_rent_occurrence(state.units()[0], 201, 401, fourth_due, Some(fully_settled))
                .unwrap();
        assert_eq!(fourth.accrual_increment, money(50_000));
        assert_eq!(fourth.updated_claim.principal, money(200_000));
        assert_eq!(fourth.updated_claim.paid, money(150_000));
        assert_eq!(fourth.updated_claim.due, fourth_due.value() as u64);
        assert_eq!(fourth.earliest_due_at, fourth_due);
        state.apply_rent_occurrence(fourth).unwrap();
        assert_eq!(
            plan_rent_occurrence(
                state.units()[0],
                201,
                401,
                fourth_due,
                Some(fourth.updated_claim),
            ),
            Err(HousingError::NotDue)
        );
        assert_eq!(
            state.apply_rent_occurrence(fourth),
            Err(HousingError::StalePlan)
        );
    }

    #[test]
    fn rent_occurrence_claim_identity_and_principal_overflow_fail_closed() {
        let mut state = HousingState::new(1).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        let lease = plan_lease(
            state.units()[0],
            dwelling(1, 10, 401),
            household(200_000, 0),
            terms(900, time(0)),
        )
        .unwrap();
        state.apply_lease(lease).unwrap();
        let first_due = Date::new(2001, 2, 1).unwrap().midnight();
        let first = plan_rent_occurrence(state.units()[0], 201, 401, first_due, None).unwrap();
        assert_eq!(first.updated_claim.id, 900);
        assert_eq!(first.updated_claim.principal, money(50_000));
        assert_eq!(first.accrual_increment, money(50_000));

        let mut after_first = first.updated_claim;
        after_first.processed = true;
        state.apply_rent_occurrence(first).unwrap();
        let sync = plan_rent_claim_reference_sync(state.units()[0], 201, after_first).unwrap();
        state.apply_transaction(&[sync]).unwrap();

        let second_due = Date::new(2001, 3, 1).unwrap().midnight();
        let overflowing = crate::timeline::Claim {
            id: 900,
            payer: 201,
            payee: 401,
            due: first_due.value() as u64,
            principal: Money::new(i64::MAX).unwrap(),
            paid: Money::ZERO,
            processed: true,
            source_ref: 0,
        };
        assert_eq!(
            plan_rent_occurrence(state.units()[0], 201, 401, second_due, Some(overflowing)),
            Err(HousingError::Arithmetic)
        );
        let wrong_owner = crate::timeline::Claim {
            payee: 402,
            ..overflowing
        };
        assert_eq!(
            plan_rent_occurrence(state.units()[0], 201, 401, second_due, Some(wrong_owner)),
            Err(HousingError::ExistingLeaseMismatch)
        );
    }

    #[test]
    #[allow(deprecated)]
    fn monthly_rent_requires_each_calendar_boundary_and_never_collapses_missed_months() {
        let vacant = HousingUnit {
            unit_id: 1,
            building_id: 10,
            owner_account: 401,
            ..HousingUnit::default()
        };
        let lease = plan_lease(
            vacant,
            dwelling(1, 10, 401),
            household(1_000_000, 0),
            terms(900, time(0)),
        )
        .unwrap();
        let first_at = time(31 * 86_400);
        let due = plan_rent_collection(
            lease.updated_unit,
            201,
            401,
            first_at,
            AccountSnapshot {
                account: 201,
                cash: money(1_000_000),
                protected: Money::ZERO,
            },
            Money::ZERO,
        )
        .unwrap();
        assert_eq!(due.landlord_transfer.amount, money(50_000));
        assert_eq!(due.updated_unit.last_accounted_at, first_at);
        let second_at = due.updated_unit.next_due_at;
        assert_eq!(second_at, time(59 * 86_400));
        assert_eq!(
            plan_rent_collection(
                due.updated_unit,
                201,
                401,
                time(75 * 86_400),
                AccountSnapshot {
                    account: 201,
                    cash: money(1_000_000),
                    protected: Money::ZERO,
                },
                Money::ZERO,
            ),
            Err(HousingError::NotDue)
        );
        let second = plan_rent_collection(
            due.updated_unit,
            201,
            401,
            second_at,
            AccountSnapshot {
                account: 201,
                cash: money(1_000_000),
                protected: Money::ZERO,
            },
            Money::ZERO,
        )
        .unwrap();
        assert_eq!(second.landlord_transfer.amount, money(50_000));
        assert_eq!(second.updated_unit.last_accounted_at, second_at);
    }

    #[test]
    fn monthly_proration_uses_real_calendar_intervals_and_clamped_anchors() {
        let midnight = |year, month, day| Date::new(year, month, day).unwrap().midnight();
        let seconds = |start: GameTime, end: GameTime| (end.value() - start.value()) as u64;

        assert_eq!(
            seconds(midnight(2001, 1, 1), midnight(2001, 2, 1)),
            31 * 86_400
        );
        assert_eq!(
            seconds(midnight(2001, 2, 1), midnight(2001, 3, 1)),
            28 * 86_400
        );
        assert_eq!(
            seconds(midnight(2004, 2, 1), midnight(2004, 3, 1)),
            29 * 86_400
        );
        assert_eq!(
            seconds(midnight(2001, 4, 1), midnight(2001, 5, 1)),
            30 * 86_400
        );
        assert_eq!(
            seconds(midnight(2001, 3, 1), midnight(2001, 4, 1)),
            31 * 86_400
        );

        assert_eq!(
            previous_monthly_due(29, midnight(2001, 2, 28)).unwrap(),
            midnight(2001, 1, 29)
        );
        assert_eq!(
            previous_monthly_due(29, midnight(2004, 2, 29)).unwrap(),
            midnight(2004, 1, 29)
        );
        assert_eq!(
            previous_monthly_due(31, midnight(2001, 2, 28)).unwrap(),
            midnight(2001, 1, 31)
        );
        assert_eq!(
            previous_monthly_due(31, midnight(2001, 3, 31)).unwrap(),
            midnight(2001, 2, 28)
        );

        let (fractional_month, rounded) =
            prorated_rent(money(100), midnight(2001, 1, 15), midnight(2001, 2, 1), 1).unwrap();
        assert_eq!(fractional_month, money(55)); // ceil(100 * 17 / 31)
        assert!(rounded);
        let (short_term, rounded) = prorated_rent(
            money(1),
            time(midnight(2001, 2, 1).value() - 1),
            midnight(2001, 2, 1),
            1,
        )
        .unwrap();
        assert_eq!(short_term, money(1));
        assert!(rounded);
        let (zero_span, rounded) =
            prorated_rent(money(10_000), midnight(2001, 2, 1), midnight(2001, 2, 1), 1).unwrap();
        assert_eq!(zero_span, Money::ZERO);
        assert!(!rounded);
    }

    #[test]
    #[allow(deprecated)]
    fn midmonth_move_in_charges_one_prorated_invoice_then_full_calendar_months() {
        let starts = Date::new(2001, 1, 15).unwrap().midnight();
        let due = Date::new(2001, 2, 1).unwrap().midnight();
        let vacant = HousingUnit {
            unit_id: 1,
            building_id: 10,
            owner_account: 401,
            ..HousingUnit::default()
        };
        let mut lease_terms = terms(900, starts);
        lease_terms.rent_per_cycle = money(100);
        let lease = plan_lease(
            vacant,
            dwelling(1, 10, 401),
            household(100_000, 0),
            lease_terms,
        )
        .unwrap();
        assert_eq!(lease.updated_unit.next_due_at, due);
        assert_eq!(lease.first_due_amount, money(55));

        let first = plan_rent_collection(
            lease.updated_unit,
            201,
            401,
            due,
            AccountSnapshot {
                account: 201,
                cash: money(100_000),
                protected: Money::ZERO,
            },
            Money::ZERO,
        )
        .unwrap();
        assert_eq!(first.landlord_transfer.amount, money(55));
        assert_eq!(
            first.updated_unit.next_due_at,
            Date::new(2001, 3, 1).unwrap().midnight()
        );

        let second = plan_rent_collection(
            first.updated_unit,
            201,
            401,
            first.updated_unit.next_due_at,
            AccountSnapshot {
                account: 201,
                cash: money(100_000),
                protected: Money::ZERO,
            },
            Money::ZERO,
        )
        .unwrap();
        assert_eq!(second.landlord_transfer.amount, money(100));
    }

    #[test]
    fn move_waits_for_14_observed_days_then_uses_actual_costs_and_preserves_old_debt() {
        let mut state = HousingState::new(2).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        state.register(dwelling(2, 20, 402)).unwrap();
        let old_lease = plan_lease(
            state.units()[0],
            dwelling(1, 10, 401),
            household(1_000_000, 0),
            terms(900, time(0)),
        )
        .unwrap();
        state.apply_lease(old_lease).unwrap();
        let old = HousingUnit {
            arrears_ref: 900,
            ..state.units()[0]
        };
        state.units[0] = old;
        let now = time((LOW_SATISFACTION_DAYS * 86_400) as i64);
        let effective = time(now.value() + 3_600);
        let old_score = MoveScore {
            lower: 25,
            upper: 30,
            fully_covered: true,
            environment_source: 10,
            public_service_source: 11,
            source_version: 2,
            assessed_at: now,
            valid_through: effective,
            hard_failure_mask: 0,
        };
        let candidate = MoveCandidate {
            dwelling: dwelling(2, 20, 402),
            terms: terms(901, effective),
            score: MoveScore {
                lower: 35,
                upper: 45,
                fully_covered: true,
                environment_source: 12,
                public_service_source: 13,
                source_version: 4,
                assessed_at: now,
                valid_through: effective,
                hard_failure_mask: 0,
            },
            move_cost: MoveCostQuote {
                source_id: 55,
                valid_through: effective,
                reachable: true,
                travel_seconds: 3_600,
                moving_charge: money(2_000),
                moving_provider_account: 501,
                old_lease_close_due: Money::ZERO,
                old_close_payee_account: 401,
                old_clause_version: 2,
                old_contract_exit_source_id: 70,
                old_escrow_account: 301,
                deposit_resolution_source_id: 71,
                old_deposit_refund_authorized: money(10_000),
                old_deposit_deduction_authorized: Money::ZERO,
                old_deposit_deduction_account: 0,
            },
        };
        let mut trigger = MoveTrigger {
            low_since: Some(time(0)),
            covered_through: now,
            continuous_low_and_covered: true,
            cooldown_until: None,
        };
        let mut below_gate = candidate;
        below_gate.score.lower = 34;
        assert_eq!(
            plan_move(
                &state,
                household(1_000_000, 0),
                old_score,
                trigger,
                now,
                &[below_gate],
                900
            ),
            Err(MoveWait::CandidateNotBetter)
        );
        trigger.covered_through = time(now.value() - 1);
        assert_eq!(
            plan_move(
                &state,
                household(1_000_000, 0),
                old_score,
                trigger,
                now,
                &[candidate],
                900
            ),
            Err(MoveWait::SatisfactionWindow)
        );
        trigger.covered_through = now;
        let plan = plan_move(
            &state,
            household(1_000_000, 0),
            old_score,
            trigger,
            now,
            &[candidate],
            900,
        )
        .unwrap();
        assert_eq!(plan.old_arrears_ref_retained_by_ledger, 900);
        assert_eq!(plan.effective_at, effective);
        // This lease starts at the supported epoch. The final partial invoice must use its
        // persisted February due boundary, not infer a December 2000 interval from `now`.
        assert_eq!(plan.effects[1].amount, money(22_648));
        assert_eq!(plan.new_first_due_amount, money(27_353));
        assert_eq!(plan.updated_old_unit.household_id, 0);
        assert_eq!(plan.updated_new_unit.household_id, 101);
        assert_eq!(plan.old_deposit_liability_retained_by_ledger, Money::ZERO);
        assert!(plan.final_rent_rounded_up_fractional_cent);
        state.apply_move(plan, effective).unwrap();
        assert_eq!(state.units()[0].household_id, 0);
        assert_eq!(state.units()[1].household_id, 101);
        assert_eq!(state.units()[1].last_accounted_at, effective);
    }

    #[test]
    fn move_refuses_unknown_environment_or_service_and_stale_plans() {
        let mut state = HousingState::new(2).unwrap();
        state.register(dwelling(1, 10, 401)).unwrap();
        state.register(dwelling(2, 20, 402)).unwrap();
        let lease = plan_lease(
            state.units()[0],
            dwelling(1, 10, 401),
            household(1_000_000, 0),
            terms(900, time(0)),
        )
        .unwrap();
        state.apply_lease(lease).unwrap();
        let now = time((LOW_SATISFACTION_DAYS * 86_400) as i64);
        let score = MoveScore {
            lower: 20,
            upper: 25,
            fully_covered: true,
            environment_source: 1,
            public_service_source: 2,
            source_version: 1,
            assessed_at: now,
            valid_through: time(now.value() + 10_000),
            hard_failure_mask: 0,
        };
        let mut candidate = MoveCandidate {
            dwelling: dwelling(2, 20, 402),
            terms: terms(901, time(now.value() + 1)),
            score: MoveScore {
                lower: 40,
                upper: 50,
                environment_source: 0,
                ..score
            },
            move_cost: MoveCostQuote {
                source_id: 50,
                valid_through: time(now.value() + 10_000),
                reachable: true,
                travel_seconds: 1,
                moving_charge: Money::ZERO,
                moving_provider_account: 501,
                old_lease_close_due: Money::ZERO,
                old_close_payee_account: 401,
                old_clause_version: 2,
                old_contract_exit_source_id: 70,
                old_escrow_account: 301,
                deposit_resolution_source_id: 71,
                old_deposit_refund_authorized: money(10_000),
                old_deposit_deduction_authorized: Money::ZERO,
                old_deposit_deduction_account: 0,
            },
        };
        let trigger = MoveTrigger {
            low_since: Some(time(0)),
            covered_through: now,
            continuous_low_and_covered: true,
            cooldown_until: None,
        };
        assert_eq!(
            plan_move(
                &state,
                household(1_000_000, 0),
                score,
                trigger,
                now,
                &[candidate],
                0
            ),
            Err(MoveWait::CandidateNotBetter)
        );
        candidate.score.environment_source = 3;
        candidate.score.public_service_source = 0;
        assert_eq!(
            plan_move(
                &state,
                household(1_000_000, 0),
                score,
                trigger,
                now,
                &[candidate],
                0
            ),
            Err(MoveWait::CandidateNotBetter)
        );
    }
}
