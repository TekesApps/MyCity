//! Root-owned persistence wrapper for managed venue rules. Original pool remains authoritative.
//! The first raw activity ingress records trusted game-host facts, not transport/education proof.
use crate::{arena::Scope, model_rows::{Record, Rows}, venue_capacity as v};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable original membership namespace; not an authority minted by a use request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopulationSource { HousingGenesisV1 }

/// Explicit optional profile, not an increase of legacy World's base allocation gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile { ManagedPeopleV1 }


/// Immutable registration provenance, checked against the one original Resource capacity.
/// This is not another mutable available/reserved/occupied balance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredLimit { pub resource_id: u32, pub maximum_people: u32 }
impl Record for RegisteredLimit { fn inactive() -> Self { Self { resource_id: 0, maximum_people: 0 } } }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub profile: Profile,
    pub population_source: PopulationSource,
    pub venues: Rows<v::VenueDefinition, {v::MAX_VENUES}>,
    pub limits: Rows<RegisteredLimit, {v::MAX_VENUES}>,
    pub offers: Rows<v::UseOffer, {v::MAX_USE_OFFERS}>,
}

/// Trusted game-host raw activity ingress. Enter/exit do not authenticate upstream
/// travel or teaching; only the original System host may submit these explicit facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Accept,
    CancelReserved,
    Enter { event_id: u64, token_number: u64, occurred_at: u64 },
    SafeExit { event_id: u64, token_number: u64, occurred_at: u64 },
}

/// A committed host input's identity survives checkpointing. Event identity is independent
/// of Envelope operation identity so a new operation cannot reapply the same original fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityFact {
    pub event_id: u64,
    pub operation_id: u64,
    pub occurred_at: u64,
    pub transaction_source: v::SourceRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityFacts {
    pub use_id: u64,
    pub entered: Option<ActivityFact>,
    pub exited: Option<ActivityFact>,
}
impl Record for ActivityFacts {
    fn inactive() -> Self { Self { use_id: 0, entered: None, exited: None } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    pub opening: Opening,
    pub population_origin_sha256: [u8; 32],
    pub policies: Rows<v::VenuePolicy, {v::MAX_VENUES}>,
    pub states: Rows<v::UseState, {v::MAX_USE_STATES}>,
    pub facts: Rows<ActivityFacts, {v::MAX_USE_STATES}>,
}
impl Runtime {
    /// Bind ranges to already registered original game people, never inferred from account IDs.
    pub fn validate_population(&self, scope: Scope, housing: Option<&crate::housing::OpeningHousing>) -> Result<(), &'static str> {
        let housing = housing.ok_or("venue_population_source_missing")?;
        if self.population_origin_sha256 != population_origin(scope, housing) {
            return Err("venue_population_origin_changed");
        }
        let total = housing.households.iter().try_fold(0u64, |n, row| n.checked_add(u64::from(row.people))).ok_or("venue_population_overflow")?;
        for offer in self.opening.offers.iter() {
            let partition = offer.partition;
            if partition.partition_source_id != 1 || partition.partition_version != 1
                || partition.member_range_end > total || partition.member_range_start >= partition.member_range_end {
                return Err("venue_population_partition_source");
            }
            let people = u64::from(offer.simultaneous_people().map_err(|_| "venue_population_count")?);
            if partition.member_range_end - partition.member_range_start != people {
                return Err("venue_population_count");
            }
            let mut start = 0u64;
            let mut expected_sources = 0usize;
            for household in housing.households.iter() {
                let end = start.checked_add(u64::from(household.people)).ok_or("venue_population_overflow")?;
                if start < partition.member_range_end && partition.member_range_start < end {
                    expected_sources += 1;
                    let matching = offer.source_refs.iter().flatten().filter(|source| {
                        source.kind == v::SourceKind::Genesis && source.scope == scope
                            && source.source_id == household.source_id && source.revision == 0
                            && source.operation_id == 0 && source.event_id == 0
                    }).count();
                    if matching != 1 { return Err("venue_population_source_binding"); }
                }
                start = end;
            }
            if expected_sources == 0 || expected_sources != offer.source_refs.iter().flatten().count() {
                return Err("venue_population_source_binding");
            }
        }
        Ok(())
    }
    pub fn new(opening: Opening, scope: Scope, housing: &crate::housing::OpeningHousing) -> Result<Self, &'static str> {
        let mut policies = Rows::new()?;
        for venue in opening.venues.iter() {
            policies.try_push(v::VenuePolicy {
                venue_id: venue.venue_id, owner: venue.owner, operator: venue.operator,
                permit_version: venue.permit_version, open: true, revoked: false,
            })?;
        }
        Ok(Self { opening, population_origin_sha256: population_origin(scope, housing), policies, states: Rows::new()?, facts: Rows::new()? })
    }
    pub fn owned_bytes(&self) -> usize { Self::max_owned_bytes() }
    pub const fn max_owned_bytes() -> usize {
        std::mem::size_of::<Self>()
            + v::MAX_VENUES * (std::mem::size_of::<v::VenueDefinition>() + std::mem::size_of::<RegisteredLimit>())
            + v::MAX_USE_OFFERS * std::mem::size_of::<v::UseOffer>()
            + v::MAX_VENUES * std::mem::size_of::<v::VenuePolicy>()
            + v::MAX_USE_STATES * (std::mem::size_of::<v::UseState>() + std::mem::size_of::<ActivityFacts>())
    }
    pub fn is_managed(&self, resource: u32) -> bool {
        self.opening.venues.iter().any(|row| row.resource_id == resource)
    }
    /// Due is derived from persistent offers and the caller's authoritative completed tick.
    /// Neither this read nor an unactivated venue task can release an Actual obligation.
    pub fn exit_due(&self, use_id: u64, completed_tick: u64) -> bool {
        self.states.iter().any(|state| state.use_id == use_id && state.status == v::UseStatus::Actual)
            && self.opening.offers.iter().any(|offer| offer.use_id == use_id && completed_tick >= offer.window.end)
    }
    pub fn validate_facts(&self) -> Result<(), &'static str> {
        self.validate_fact_tables(&self.states, &self.facts)
    }
    pub fn validate_fact_tables(&self, states: &[v::UseState], facts_rows: &[ActivityFacts]) -> Result<(), &'static str> {
        for (i, facts) in facts_rows.iter().enumerate() {
            if facts.use_id == 0 || facts_rows[..i].iter().any(|old| old.use_id == facts.use_id) {
                return Err("venue_duplicate_fact_use");
            }
            let state = states.iter().find(|row| row.use_id == facts.use_id).ok_or("venue_orphan_activity")?;
            let offer = self.opening.offers.iter().find(|row| row.use_id == facts.use_id).ok_or("venue_fact_offer")?;
            if facts.exited.is_some() && facts.entered.is_none() { return Err("venue_exit_without_entry"); }
            if facts.entered.is_some() != matches!(state.status, v::UseStatus::Actual | v::UseStatus::Released)
                || facts.exited.is_some() != (state.status == v::UseStatus::Released) {
                return Err("venue_activity_state_mismatch");
            }
            for event in [facts.entered, facts.exited].into_iter().flatten() {
                if event.event_id == 0 || event.operation_id == 0
                    || event.operation_id == state.accepted_operation_id
                    || event.transaction_source.kind != v::SourceKind::GameEvent
                    || event.transaction_source.operation_id != event.operation_id
                    || event.transaction_source.event_id != event.operation_id
                    || event.transaction_source.scope != state.accepted_scope
                    || event.transaction_source.source_id != 0
                    || event.transaction_source.revision < state.accepted_revision
                    || event.transaction_source.revision > state.last_event.revision {
                    return Err("venue_invalid_activity_identity");
                }
                if facts_rows.iter().flat_map(|row| [row.entered, row.exited]).flatten()
                    .filter(|other| other.operation_id == event.operation_id).count() != 1 {
                    return Err("venue_duplicate_activity_operation");
                }
                let count = facts_rows.iter().flat_map(|row| [row.entered, row.exited]).flatten()
                    .filter(|other| other.event_id == event.event_id).count();
                if count != 1 { return Err("venue_duplicate_activity_identity"); }
            }
            if state.status == v::UseStatus::Actual
                && facts.entered.is_none_or(|event| event.transaction_source != state.last_event)
                || state.status == v::UseStatus::Released
                    && facts.exited.is_none_or(|event| event.transaction_source != state.last_event) {
                return Err("venue_activity_operation_source_mismatch");
            }
            if state.entered_at != facts.entered.map(|event| event.occurred_at)
                || state.released_at != facts.exited.map(|event| event.occurred_at) {
                return Err("venue_activity_time_mismatch");
            }
            if let Some(entered) = facts.entered {
                if entered.occurred_at < offer.window.start || entered.occurred_at >= offer.window.end {
                    return Err("venue_entry_outside_window");
                }
                if facts.exited.is_some_and(|exited| exited.occurred_at < entered.occurred_at) {
                    return Err("venue_exit_before_entry");
                }
            }
        }
        for state in states.iter() {
            if matches!(state.status, v::UseStatus::Actual | v::UseStatus::Released)
                && !facts_rows.iter().any(|row| row.use_id == state.use_id) {
                return Err("venue_missing_actual_source");
            }
        }
        Ok(())
    }
}

impl Record for v::VenueDefinition {
    fn inactive() -> Self { Self { venue_id: 0, resource_id: 0, aliases: [None; v::MAX_ALIASES_PER_VENUE], owner: 0, operator: 0, purpose: v::PurposeId(0), permit_version: 0 } }
}
impl Record for v::VenuePolicy {
    fn inactive() -> Self { Self { venue_id: 0, owner: 0, operator: 0, permit_version: 0, open: false, revoked: true } }
}
impl Record for v::UseOffer {
    fn inactive() -> Self { Self { use_id: 0, venue_id: 0, group_id: 0,
        population: v::Population { learners: 0, teachers: 0, staff: 0, visitors: 0, enrollment: 0 },
        partition: v::GroupPartition { group_id: 0, partition_source_id: 0, partition_version: 0, member_range_start: 0, member_range_end: 0 },
        purpose: v::PurposeId(0), operator: 0, window: v::UseWindow { start: 0, end: 0 },
        source_refs: [None; v::MAX_SOURCE_REFS_PER_OFFER], accepted_version: 0 } }
}
impl Record for v::UseState {
    fn inactive() -> Self { Self { use_id: 0, token: crate::transaction::CapacityToken { world: 0, branch: 0, number: 0 },
        actor: 0, resource_id: 0, units: 0, accepted_scope: Scope::new(1, 1).expect("inactive scope"),
        accepted_revision: 0, accepted_operation_id: 0, accepted_owner: 0, accepted_operator: 0,
        accepted_version: 0,
        accepted_event: v::SourceRef { kind: v::SourceKind::Genesis, scope: Scope::new(1, 1).expect("inactive scope"), revision: 0, operation_id: 0, event_id: 0, source_id: 0 },
        last_event: v::SourceRef { kind: v::SourceKind::Genesis, scope: Scope::new(1, 1).expect("inactive scope"), revision: 0, operation_id: 0, event_id: 0, source_id: 0 },
        accepted_at: 0, entered_at: None, released_at: None,
        status: v::UseStatus::Cancelled } }
}

/// Exact relevant frozen population-content identity, itself bound by Store's original
/// genesis/Opening hash. No per-member allocation or account-alias inference occurs.
fn population_origin(scope: Scope, housing: &crate::housing::OpeningHousing) -> [u8; 32] {
    crate::member_qualification::housing_population_origin(scope,housing)
}

/// Only mutable per-use tables are copied during prepare. The immutable directory and
/// original membership catalogue stay borrowed; no per-action registry clone occurs.
pub(crate) struct UseCandidate {
    pub states: Rows<v::UseState, {v::MAX_USE_STATES}>,
    pub facts: Rows<ActivityFacts, {v::MAX_USE_STATES}>,
}
impl UseCandidate {
    pub fn from_runtime(runtime: &Runtime) -> Self { Self { states: runtime.states.clone(), facts: runtime.facts.clone() } }
    pub const fn payload_bytes() -> usize { v::MAX_USE_STATES * (std::mem::size_of::<v::UseState>() + std::mem::size_of::<ActivityFacts>()) }
}

// Sealed optional restore, matching the original Housing bounded visitor pattern. World
// clears the old feature slot before decoding, so an old checkpoint cannot retain it.
impl crate::restore_into::Restore for Option<Box<Runtime>> {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, deserializer: D) -> Result<(), D::Error> {
        struct Read<'a>(&'a mut Option<Box<Runtime>>);
        impl<'de> serde::de::Visitor<'de> for Read<'_> {
            type Value = ();
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("optional bounded venue runtime") }
            fn visit_none<E: serde::de::Error>(self) -> Result<(), E> { *self.0 = None; Ok(()) }
            fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> { self.visit_none() }
            fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
                *self.0 = None;
                let runtime = Runtime::deserialize(d)?;
                *self.0 = Some(Box::new(runtime));
                Ok(())
            }
        }
        deserializer.deserialize_option(Read(self))
    }
}
