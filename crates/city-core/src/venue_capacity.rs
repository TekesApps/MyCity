//! Bounded, pure validation and candidate deltas for managed venue people counts.
//!
//! This module borrows the original Resource/Lease pool. It owns no clock, balance,
//! scheduler, transaction, durable receipt or alternate capacity ledger. Root adapters
//! must construct the crate-private evidence only after validating the corresponding
//! game event and permission. A candidate is not a committed transaction.

use crate::{
    arena::Scope,
    transaction::{CapacityToken, Lease, Resource, MAX_LEASES, MAX_RESOURCES},
};
use serde::{Deserialize, Serialize};
use std::mem::size_of;

pub const MAX_VENUES: usize = 16;
pub const MAX_USE_OFFERS: usize = 64;
pub const MAX_USE_STATES: usize = 64;
pub const MAX_ALIASES_PER_VENUE: usize = 4;
pub const MAX_SOURCE_REFS_PER_OFFER: usize = 4;
pub const MAX_GROUP_PARTITIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PurposeId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Genesis,
    GameEvent,
}

/// Genesis sources have a stable source_id and zero event fields; GameEvent sources
/// bind an original event at its source base revision (which may be zero). Root's
/// original journal remains responsible for global historical event uniqueness; this
/// candidate checks duplicates among event references retained by current use rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub kind: SourceKind,
    pub scope: Scope,
    pub revision: u64,
    pub operation_id: u64,
    pub event_id: u64,
    pub source_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VenueDefinition {
    pub venue_id: u64,
    pub resource_id: u32,
    pub aliases: [Option<u64>; MAX_ALIASES_PER_VENUE],
    pub owner: u32,
    pub operator: u32,
    pub purpose: PurposeId,
    pub permit_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Population {
    pub learners: u32,
    pub teachers: u32,
    pub staff: u32,
    pub visitors: u32,
    /// Entitlement is an independent business limit, not simultaneous occupancy.
    pub enrollment: u32,
}

impl Population {
    pub fn simultaneous_people(self) -> Result<u32, ValidationError> {
        self.learners
            .checked_add(self.teachers)
            .and_then(|n| n.checked_add(self.staff))
            .and_then(|n| n.checked_add(self.visitors))
            .ok_or(ValidationError::Overflow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseWindow {
    pub start: u64,
    pub end: u64,
}

/// Frozen, authoritative membership coverage for a group. Ranges use half-open
/// intervals and all groups in one Opening share one source and partition version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupPartition {
    pub group_id: u64,
    pub partition_source_id: u64,
    pub partition_version: u64,
    pub member_range_start: u64,
    pub member_range_end: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseOffer {
    pub use_id: u64,
    pub venue_id: u64,
    pub group_id: u64,
    pub partition: GroupPartition,
    pub population: Population,
    pub purpose: PurposeId,
    pub operator: u32,
    pub window: UseWindow,
    pub source_refs: [Option<SourceRef>; MAX_SOURCE_REFS_PER_OFFER],
    pub accepted_version: u64,
}

impl UseOffer {
    pub fn simultaneous_people(self) -> Result<u32, ValidationError> {
        let total = self.population.simultaneous_people()?;
        if total == 0 {
            return Err(ValidationError::InvalidPopulation);
        }
        if self.population.learners > self.population.enrollment {
            return Err(ValidationError::EnrollmentExceeded);
        }
        Ok(total)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VenuePolicy {
    pub venue_id: u64,
    pub owner: u32,
    pub operator: u32,
    pub permit_version: u64,
    pub open: bool,
    pub revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseStatus {
    Reserved,
    Actual,
    Cancelled,
    Released,
}

/// Runtime metadata references one immutable opening offer and one original pool token.
/// Released/cancelled rows remain as tombstones; they never free a metadata slot here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseState {
    pub use_id: u64,
    pub token: CapacityToken,
    pub actor: u32,
    pub resource_id: u32,
    pub units: u32,
    pub accepted_scope: Scope,
    pub accepted_revision: u64,
    pub accepted_operation_id: u64,
    pub accepted_owner: u32,
    pub accepted_operator: u32,
    pub accepted_version: u64,
    pub accepted_event: SourceRef,
    pub last_event: SourceRef,
    pub accepted_at: u64,
    pub entered_at: Option<u64>,
    pub released_at: Option<u64>,
    pub status: UseStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VenueRegistry<'a> {
    pub venues: &'a [VenueDefinition],
}

#[derive(Clone, Copy, Debug)]
pub struct OpeningView<'a> {
    pub scope: Scope,
    pub registry: VenueRegistry<'a>,
    pub offers: &'a [UseOffer],
    pub resources: &'a [Resource],
    /// Root-authenticated directory entries for every resource owned by another domain.
    pub non_venue_resource_ids: &'a [u32],
}

#[derive(Clone, Copy, Debug)]
pub struct Snapshot<'a> {
    pub scope: Scope,
    pub revision: u64,
    pub completed_clock: Option<VerifiedCompletedClock>,
    pub registry: VenueRegistry<'a>,
    pub offers: &'a [UseOffer],
    pub policies: &'a [VenuePolicy],
    pub use_states: &'a [UseState],
    pub resources: &'a [Resource],
    pub leases: &'a [Option<Lease>],
    pub non_venue_resource_ids: &'a [u32],
}

/// Marker passed only at the crate-private root/Tick boundary after Timeline proves a
/// completed, non-partial boundary. It contains the exact source identity and tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedCompletedClock {
    scope: Scope,
    revision: u64,
    tick: u64,
}

impl VerifiedCompletedClock {
    /// Root may call this only after checking `target == None` and `phase == None` on the
    /// authoritative Timeline progress. This constructor is not public or deserializable.
    pub(crate) fn after_root_completed_timeline(scope: Scope, revision: u64, tick: u64) -> Self {
        Self { scope, revision, tick }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Accept,
    Enter,
    SafeExit,
    CancelReserved,
}

/// Compact command selector. Population, venue and rights resolve from the immutable
/// Opening/root evidence; occurred_at is accepted only as an exact System event time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub use_id: u64,
    pub operation_id: u64,
    pub kind: RequestKind,
    /// Only trusted System game transitions may provide this; it must equal the
    /// completed Timeline tick and authenticated event occurred_at.
    pub occurred_at: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RootActor {
    System,
    Account(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectivePermission {
    ReserveOffer,
    EnterVenue,
    SafeExitVenue,
    CancelReservation,
}

/// Opaque, non-deserializable result of a future root authorization check. The boundary
/// binds the entire resolved Opening offer and game context, not a bool/hash/DTO claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedRootEvent {
    request: Request,
    scope: Scope,
    revision: u64,
    clock: Option<VerifiedCompletedClock>,
    actor: RootActor,
    venue: VenueDefinition,
    offer: UseOffer,
    permission: EffectivePermission,
    event_source: SourceRef,
    occurred_at: u64,
    token: Option<CapacityToken>,
}

impl VerifiedRootEvent {
    /// Future root adapter boundary. Its caller must already have validated the original
    /// game event, actor, subject/group, offer provenance and effective permission.
    pub(crate) fn after_root_event_validation(
        request: Request,
        snapshot: Snapshot<'_>,
        actor: RootActor,
        permission: EffectivePermission,
        venue: VenueDefinition,
        offer: UseOffer,
        event_source: SourceRef,
        occurred_at: u64,
        token: Option<CapacityToken>,
    ) -> Self {
        Self {
            request,
            scope: snapshot.scope,
            revision: snapshot.revision,
            clock: snapshot.completed_clock,
            actor,
            venue,
            offer,
            permission,
            event_source,
            occurred_at,
            token,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    Limit,
    InvalidScope,
    InvalidIdentity,
    InvalidAlias,
    DuplicateEntity,
    DuplicateAlias,
    DuplicateResource,
    ResourceDomainConflict,
    ResourceDomainIncomplete,
    InvalidOffer,
    InvalidPopulation,
    EnrollmentExceeded,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    Unknown,
    Quota,
    CapacityGateRejected,
    Invalid,
    Stale,
    Unauthorized,
    Conflict,
    Integrity,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryReport {
    pub checked_resources: usize,
    pub checked_leases: usize,
    pub checked_active_uses: usize,
    pub overcommitted_resources: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolDelta {
    None,
    Reserve { resource_id: u32, actor: u32, units: u32 },
    Occupy { token: CapacityToken },
    Release { token: CapacityToken },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseStateDelta {
    None,
    Insert(UseState),
    Transition {
        use_id: u64,
        from: UseStatus,
        to: UseStatus,
        event: SourceRef,
        occurred_at: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub pool: PoolDelta,
    pub metadata: UseStateDelta,
}

pub const VENUE_DEFINITION_BYTES: usize = size_of::<VenueDefinition>();
pub const VENUE_POLICY_BYTES: usize = size_of::<VenuePolicy>();
pub const USE_OFFER_BYTES: usize = size_of::<UseOffer>();
pub const USE_STATE_BYTES: usize = size_of::<UseState>();
pub const REQUEST_BYTES: usize = size_of::<Request>();
pub const VERIFIED_ROOT_EVENT_BYTES: usize = size_of::<VerifiedRootEvent>();
pub const CANDIDATE_BYTES: usize = size_of::<Candidate>();
pub const GROUP_PARTITION_BYTES: usize = size_of::<GroupPartition>();
pub const MAX_METADATA_ROW_BYTES: usize = MAX_VENUES * VENUE_DEFINITION_BYTES
    + MAX_USE_OFFERS * USE_OFFER_BYTES
    + MAX_USE_STATES * USE_STATE_BYTES;
/// Planning ceilings requested for source-layout regression tests, not an allocation
/// budget or a measured peak; root must add policies, containers and transaction scratch.
pub const VENUE_DEFINITION_LAYOUT_CEILING: usize = 128;
pub const USE_OFFER_LAYOUT_CEILING: usize = 512;
pub const USE_STATE_LAYOUT_CEILING: usize = 384;
pub const METADATA_LAYOUT_CEILING_BYTES: usize = MAX_VENUES * VENUE_DEFINITION_LAYOUT_CEILING
    + MAX_USE_OFFERS * USE_OFFER_LAYOUT_CEILING
    + MAX_USE_STATES * USE_STATE_LAYOUT_CEILING;

const fn field_len(name: &str, value_max: usize) -> usize {
    name.len() + 3 + value_max
}

const fn object_len(fields: &[usize]) -> usize {
    let mut sum = 2usize;
    let mut index = 0usize;
    while index < fields.len() {
        sum += fields[index];
        if index > 0 {
            sum += 1;
        }
        index += 1;
    }
    sum
}

const fn array_len(rows: usize, row_bytes: usize) -> usize {
    if rows == 0 {
        2
    } else {
        2 + rows * row_bytes + rows - 1
    }
}

/// Exact max-value serde JSON bounds for candidate DTO rows. u64/u32/u16 decimal
/// widths are 20/10/5; enum and bool strings include JSON quotes. The test constructs
/// max-valued rows and checks actual encodings against each row and bounded-array formula.
pub const SCOPE_JSON_MAX: usize = object_len(&[
    field_len("world", 20),
    field_len("branch", 20),
]);
pub const SOURCE_REF_JSON_MAX: usize = object_len(&[
    field_len("kind", 12),
    field_len("scope", SCOPE_JSON_MAX),
    field_len("revision", 20),
    field_len("operation_id", 20),
    field_len("event_id", 20),
    field_len("source_id", 20),
]);
pub const GROUP_PARTITION_JSON_MAX: usize = object_len(&[
    field_len("group_id", 20),
    field_len("partition_source_id", 20),
    field_len("partition_version", 20),
    field_len("member_range_start", 20),
    field_len("member_range_end", 20),
]);
pub const VENUE_DEFINITION_JSON_MAX: usize = object_len(&[
    field_len("venue_id", 20),
    field_len("resource_id", 10),
    field_len("aliases", array_len(MAX_ALIASES_PER_VENUE, 20)),
    field_len("owner", 10),
    field_len("operator", 10),
    field_len("purpose", 5),
    field_len("permit_version", 20),
]);
pub const POPULATION_JSON_MAX: usize = object_len(&[
    field_len("learners", 10),
    field_len("teachers", 10),
    field_len("staff", 10),
    field_len("visitors", 10),
    field_len("enrollment", 10),
]);
pub const USE_WINDOW_JSON_MAX: usize = object_len(&[
    field_len("start", 20),
    field_len("end", 20),
]);
pub const USE_OFFER_JSON_MAX: usize = object_len(&[
    field_len("use_id", 20),
    field_len("venue_id", 20),
    field_len("group_id", 20),
    field_len("partition", GROUP_PARTITION_JSON_MAX),
    field_len("population", POPULATION_JSON_MAX),
    field_len("purpose", 5),
    field_len("operator", 10),
    field_len("window", USE_WINDOW_JSON_MAX),
    field_len("source_refs", array_len(MAX_SOURCE_REFS_PER_OFFER, SOURCE_REF_JSON_MAX)),
    field_len("accepted_version", 20),
]);
pub const CAPACITY_TOKEN_JSON_MAX: usize = object_len(&[
    field_len("world", 20),
    field_len("branch", 20),
    field_len("number", 20),
]);
pub const USE_STATE_JSON_MAX: usize = object_len(&[
    field_len("use_id", 20),
    field_len("token", CAPACITY_TOKEN_JSON_MAX),
    field_len("actor", 10),
    field_len("resource_id", 10),
    field_len("units", 10),
    field_len("accepted_scope", SCOPE_JSON_MAX),
    field_len("accepted_revision", 20),
    field_len("accepted_operation_id", 20),
    field_len("accepted_owner", 10),
    field_len("accepted_operator", 10),
    field_len("accepted_version", 20),
    field_len("accepted_event", SOURCE_REF_JSON_MAX),
    field_len("last_event", SOURCE_REF_JSON_MAX),
    field_len("accepted_at", 20),
    field_len("entered_at", 20),
    field_len("released_at", 20),
    field_len("status", 11),
]);
pub const REQUEST_JSON_MAX: usize = object_len(&[
    field_len("use_id", 20),
    field_len("operation_id", 20),
    field_len("kind", 17),
    field_len("occurred_at", 20),
]);
pub const RESOURCE_JSON_MAX: usize = object_len(&[
    field_len("id", 10),
    field_len("capacity", 10),
    field_len("reserved", 10),
    field_len("occupied", 10),
]);
pub const LEASE_JSON_MAX: usize = object_len(&[
    field_len("token", 20),
    field_len("actor", 10),
    field_len("resource", 10),
    field_len("units", 10),
    field_len("occupied", 5),
]);
/// Hand-checkable values from the formulas above are SourceRef=223, venue=249,
/// partition=199, offer=1488, state=1059; the 16/64/64 arrays are 4001/95297/67841.
/// Opening/runtime row totals are 99298/67841 bytes (167139 combined).
/// Maximum serialized JSON bytes for owned module data row arrays only. Root wrapper
/// fields, Vec/Box headers, allocator overhead and decoded-source temporaries are external.
pub const MAX_VENUES_JSON_BYTES: usize = array_len(MAX_VENUES, VENUE_DEFINITION_JSON_MAX);
pub const MAX_USE_OFFERS_JSON_BYTES: usize = array_len(MAX_USE_OFFERS, USE_OFFER_JSON_MAX);
pub const MAX_USE_STATES_JSON_BYTES: usize = array_len(MAX_USE_STATES, USE_STATE_JSON_MAX);
pub const MAX_OPENING_JSON_BYTES: usize =
    MAX_VENUES_JSON_BYTES + MAX_USE_OFFERS_JSON_BYTES;
pub const MAX_RUNTIME_JSON_BYTES: usize = MAX_USE_STATES_JSON_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceUseSummary {
    pub reserved: u64,
    pub occupied: u64,
    pub lease_count: usize,
}

pub fn validate_opening(view: OpeningView<'_>) -> Result<(), ValidationError> {
    if view.scope.world() == 0 || view.scope.branch() == 0 {
        return Err(ValidationError::InvalidScope);
    }
    if view.registry.venues.len() > MAX_VENUES
        || view.offers.len() > MAX_USE_OFFERS
        || view.offers.len() > MAX_GROUP_PARTITIONS
        || view.resources.len() > MAX_RESOURCES
        || view.non_venue_resource_ids.len() > MAX_RESOURCES
    {
        return Err(ValidationError::Limit);
    }

    for (index, venue) in view.registry.venues.iter().enumerate() {
        if venue.venue_id == 0
            || venue.resource_id == 0
            || venue.owner == 0
            || venue.operator == 0
            || venue.purpose.0 == 0
            || venue.permit_version == 0
        {
            return Err(ValidationError::InvalidIdentity);
        }
        if venue.venue_id == u64::from(venue.resource_id) {
            return Err(ValidationError::DuplicateEntity);
        }
        if !view.resources.iter().any(|r| r.id == venue.resource_id) {
            return Err(ValidationError::ResourceDomainIncomplete);
        }
        if view
            .non_venue_resource_ids
            .iter()
            .any(|resource_id| *resource_id == venue.resource_id)
        {
            return Err(ValidationError::ResourceDomainConflict);
        }
        if view.registry.venues[..index]
            .iter()
            .any(|prior| prior.venue_id == venue.venue_id || prior.resource_id == venue.resource_id)
        {
            return Err(ValidationError::DuplicateEntity);
        }
        for alias in venue.aliases.iter().flatten() {
            if *alias == 0 || *alias == venue.venue_id {
                return Err(ValidationError::InvalidAlias);
            }
            if *alias == u64::from(venue.resource_id)
                || view.resources.iter().any(|resource| u64::from(resource.id) == *alias)
            {
                return Err(ValidationError::DuplicateAlias);
            }
            if view.registry.venues[..index].iter().any(|prior| {
                prior.venue_id == *alias || prior.aliases.iter().flatten().any(|v| v == alias)
            }) || view.registry.venues[..=index].iter().any(|other| {
                other.venue_id != venue.venue_id
                    && (other.aliases.iter().flatten().any(|v| v == alias)
                        || (other.venue_id == *alias))
            }) {
                return Err(ValidationError::DuplicateAlias);
            }
        }
        let aliases = venue.aliases.iter().flatten().count();
        for (alias_index, alias) in venue.aliases.iter().enumerate() {
            if alias.is_some() && venue.aliases[..alias_index].contains(alias) {
                return Err(ValidationError::DuplicateAlias);
            }
        }
        if aliases > MAX_ALIASES_PER_VENUE {
            return Err(ValidationError::Limit);
        }
    }

    for (left_index, left) in view.registry.venues.iter().enumerate() {
        for right in view.registry.venues.iter().skip(left_index + 1) {
            if left.aliases.iter().flatten().any(|alias| {
                *alias == right.venue_id
                    || right.aliases.iter().flatten().any(|other| other == alias)
            }) || right.aliases.iter().flatten().any(|alias| *alias == left.venue_id)
            {
                return Err(ValidationError::DuplicateAlias);
            }
        }
    }

    for (index, resource) in view.resources.iter().enumerate() {
        if resource.id == 0 || view.resources[..index].iter().any(|r| r.id == resource.id) {
            return Err(ValidationError::DuplicateResource);
        }
        let is_venue = view.registry.venues.iter().any(|v| v.resource_id == resource.id);
        let in_non_venue = view.non_venue_resource_ids.contains(&resource.id);
        if is_venue == in_non_venue {
            return Err(if is_venue {
                ValidationError::ResourceDomainConflict
            } else {
                ValidationError::ResourceDomainIncomplete
            });
        }
        if view.registry.venues.iter().any(|venue| {
            venue.venue_id == u64::from(resource.id)
                || venue.aliases.iter().flatten().any(|alias| *alias == u64::from(resource.id))
        }) {
            return Err(ValidationError::DuplicateEntity);
        }
    }
    for (index, id) in view.non_venue_resource_ids.iter().enumerate() {
        if *id == 0
            || view.non_venue_resource_ids[..index].contains(id)
            || !view.resources.iter().any(|resource| resource.id == *id)
        {
            return Err(ValidationError::ResourceDomainConflict);
        }
    }

    let mut partition_identity: Option<(u64, u64)> = None;
    for (index, offer) in view.offers.iter().enumerate() {
        if offer.use_id == 0
            || offer.venue_id == 0
            || offer.group_id == 0
            || offer.operator == 0
            || offer.accepted_version == 0
            || offer.window.start >= offer.window.end
            || !view.offers[..index].iter().all(|prior| prior.use_id != offer.use_id)
            || offer.partition.group_id != offer.group_id
            || offer.partition.partition_source_id == 0
            || offer.partition.partition_version == 0
            || offer.partition.member_range_start >= offer.partition.member_range_end
        {
            return Err(ValidationError::InvalidOffer);
        }
        let identity = (
            offer.partition.partition_source_id,
            offer.partition.partition_version,
        );
        if partition_identity.is_some_and(|known| known != identity) {
            return Err(ValidationError::InvalidOffer);
        }
        partition_identity = Some(identity);
        if view.offers[..index].iter().any(|prior| {
            if prior.group_id == offer.group_id {
                prior.partition != offer.partition
            } else {
                prior.partition.member_range_start < offer.partition.member_range_end
                    && offer.partition.member_range_start < prior.partition.member_range_end
            }
        }) {
            return Err(ValidationError::InvalidOffer);
        }
        let venue = view
            .registry
            .venues
            .iter()
            .find(|venue| venue.venue_id == offer.venue_id)
            .ok_or(ValidationError::InvalidOffer)?;
        if venue.purpose != offer.purpose
            || venue.operator != offer.operator
            || venue.permit_version != offer.accepted_version
        {
            return Err(ValidationError::InvalidOffer);
        }
        let people = offer.simultaneous_people()?;
        if offer
            .partition
            .member_range_end
            .checked_sub(offer.partition.member_range_start)
            != Some(u64::from(people))
        {
            return Err(ValidationError::InvalidOffer);
        }
        let mut source_count = 0usize;
        for (source_index, source) in offer.source_refs.iter().enumerate() {
            let Some(source) = source else {
                continue;
            };
            source_count = source_count.checked_add(1).ok_or(ValidationError::Overflow)?;
            if !valid_opening_source(*source, view.scope) {
                return Err(ValidationError::InvalidOffer);
            }
            if offer.source_refs[..source_index].contains(&Some(*source)) {
                return Err(ValidationError::InvalidOffer);
            }
        }
        if source_count == 0 {
            return Err(ValidationError::InvalidOffer);
        }
    }
    Ok(())
}

fn opening_view(snapshot: Snapshot<'_>) -> OpeningView<'_> {
    OpeningView {
        scope: snapshot.scope,
        registry: snapshot.registry,
        offers: snapshot.offers,
        resources: snapshot.resources,
        non_venue_resource_ids: snapshot.non_venue_resource_ids,
    }
}

fn resource_index(resources: &[Resource], id: u32) -> Option<usize> {
    resources.iter().position(|resource| resource.id == id)
}

fn same_lease(left: Lease, right: Lease) -> bool {
    left.token == right.token
        && left.actor == right.actor
        && left.resource == right.resource
        && left.units == right.units
        && left.occupied == right.occupied
}

fn lease_totals(snapshot: Snapshot<'_>) -> Result<([ResourceUseSummary; MAX_RESOURCES], usize), PlanError> {
    if snapshot.resources.len() > MAX_RESOURCES || snapshot.leases.len() > MAX_LEASES {
        return Err(PlanError::Quota);
    }
    let mut totals = [ResourceUseSummary { reserved: 0, occupied: 0, lease_count: 0 }; MAX_RESOURCES];
    let mut lease_count = 0usize;
    for (index, resource) in snapshot.resources.iter().enumerate() {
        if resource.id == 0 || snapshot.resources[..index].iter().any(|r| r.id == resource.id) {
            return Err(PlanError::Integrity);
        }
    }
    for (lease_index, lease) in snapshot.leases.iter().flatten().enumerate() {
        if lease.token == 0 || lease.units == 0 {
            return Err(PlanError::Integrity);
        }
        if snapshot.leases.iter().flatten().take(lease_index).any(|old| old.token == lease.token) {
            return Err(PlanError::Integrity);
        }
        let index = resource_index(snapshot.resources, lease.resource).ok_or(PlanError::Integrity)?;
        let total = &mut totals[index];
        total.lease_count = total.lease_count.checked_add(1).ok_or(PlanError::Overflow)?;
        if lease.occupied {
            total.occupied = total.occupied.checked_add(u64::from(lease.units)).ok_or(PlanError::Overflow)?;
        } else {
            total.reserved = total.reserved.checked_add(u64::from(lease.units)).ok_or(PlanError::Overflow)?;
        }
        lease_count = lease_count.checked_add(1).ok_or(PlanError::Overflow)?;
    }
    for (index, resource) in snapshot.resources.iter().enumerate() {
        if totals[index].reserved != u64::from(resource.reserved)
            || totals[index].occupied != u64::from(resource.occupied)
        {
            return Err(PlanError::Integrity);
        }
    }
    Ok((totals, lease_count))
}

pub fn validate_recovery(snapshot: Snapshot<'_>) -> Result<RecoveryReport, PlanError> {
    validate_opening(opening_view(snapshot)).map_err(|_| PlanError::Integrity)?;
    if snapshot.policies.len() != snapshot.registry.venues.len()
        || snapshot.use_states.len() > MAX_USE_STATES
    {
        return Err(PlanError::Integrity);
    }
    let mut policy_count = 0usize;
    for (index, policy) in snapshot.policies.iter().enumerate() {
        policy_count = policy_count.checked_add(1).ok_or(PlanError::Overflow)?;
        if snapshot.policies[..index].iter().any(|old| old.venue_id == policy.venue_id)
            || !snapshot.registry.venues.iter().any(|venue| venue.venue_id == policy.venue_id)
        {
            return Err(PlanError::Integrity);
        }
    }
    if policy_count != snapshot.registry.venues.len() {
        return Err(PlanError::Integrity);
    }
    for offer in snapshot.offers {
        for source in offer.source_refs.iter().flatten() {
            if source.kind == SourceKind::GameEvent && source.revision > snapshot.revision {
                return Err(PlanError::Integrity);
            }
        }
    }
    let (totals, lease_count) = lease_totals(snapshot)?;
    let mut active_count = 0usize;
    for (state_index, state) in snapshot.use_states.iter().enumerate() {
        if snapshot.use_states[..state_index].iter().any(|old| old.use_id == state.use_id) {
            return Err(PlanError::Integrity);
        }
        if snapshot.use_states[..state_index]
            .iter()
            .any(|old| old.token == state.token)
        {
            return Err(PlanError::Integrity);
        }
        let offer = snapshot.offers.iter().find(|offer| offer.use_id == state.use_id)
            .ok_or(PlanError::Integrity)?;
        let venue = snapshot.registry.venues.iter().find(|venue| venue.venue_id == offer.venue_id)
            .ok_or(PlanError::Integrity)?;
        let people = offer.simultaneous_people().map_err(|_| PlanError::Integrity)?;
        if state.token.world != snapshot.scope.world()
            || state.token.branch != snapshot.scope.branch()
            || state.token.number == 0
            || state.accepted_scope != snapshot.scope
            || state.accepted_revision > snapshot.revision
            || state.accepted_operation_id == 0
            || state.actor != state.accepted_operator
            || state.actor != offer.operator
            || state.accepted_owner != venue.owner
            || state.accepted_operator != offer.operator
            || state.accepted_version != offer.accepted_version
            || state.resource_id != venue.resource_id
            || state.units != people
            || !valid_runtime_source(state.accepted_event, snapshot.scope, snapshot.revision)
            || !valid_runtime_source(state.last_event, snapshot.scope, snapshot.revision)
            || state.accepted_event.operation_id != state.accepted_operation_id
            || state.accepted_event.revision != state.accepted_revision
            || state.last_event.revision < state.accepted_event.revision
            || state.accepted_at >= offer.window.end
            || state.accepted_at > offer.window.start
        {
            return Err(PlanError::Integrity);
        }
        let mut matching: Option<&Lease> = None;
        let mut matching_count = 0usize;
        for lease in snapshot.leases.iter().flatten() {
            if lease.token == state.token.number {
                matching = Some(lease);
                matching_count = matching_count.checked_add(1).ok_or(PlanError::Overflow)?;
            }
        }
        let active = matches!(state.status, UseStatus::Reserved | UseStatus::Actual);
        if active {
            active_count = active_count.checked_add(1).ok_or(PlanError::Overflow)?;
            if matching_count != 1 {
                return Err(PlanError::Integrity);
            }
            let lease = matching.ok_or(PlanError::Integrity)?;
            let expected_occupied = state.status == UseStatus::Actual;
            if lease.actor != state.actor
                || lease.resource != state.resource_id
                || lease.units != state.units
                || lease.occupied != expected_occupied
            {
                return Err(PlanError::Integrity);
            }
        } else if matching_count != 0 {
            return Err(PlanError::Integrity);
        }
        match state.status {
            UseStatus::Reserved => {
                if state.entered_at.is_some() || state.released_at.is_some()
                    || state.last_event != state.accepted_event
                {
                    return Err(PlanError::Integrity);
                }
            }
            UseStatus::Actual => {
                let entered_at = state.entered_at.ok_or(PlanError::Integrity)?;
                if entered_at < state.accepted_at
                    || entered_at < offer.window.start
                    || entered_at >= offer.window.end
                    || state.released_at.is_some()
                    || state.last_event == state.accepted_event
                    || state.last_event.event_id == state.accepted_event.event_id
                {
                    return Err(PlanError::Integrity);
                }
            }
            UseStatus::Cancelled => {
                if state.entered_at.is_some() || state.released_at.is_some()
                    || state.last_event == state.accepted_event
                    || state.last_event.event_id == state.accepted_event.event_id
                {
                    return Err(PlanError::Integrity);
                }
            }
            UseStatus::Released => {
                let entered_at = state.entered_at.ok_or(PlanError::Integrity)?;
                let released_at = state.released_at.ok_or(PlanError::Integrity)?;
                if entered_at < state.accepted_at
                    || entered_at < offer.window.start
                    || entered_at >= offer.window.end
                    || released_at < entered_at
                    || state.last_event == state.accepted_event
                    || state.last_event.event_id == state.accepted_event.event_id
                {
                    return Err(PlanError::Integrity);
                }
            }
        }
        if snapshot.use_states[..state_index].iter().any(|prior| {
            prior.status == UseStatus::Actual
                && state.status == UseStatus::Actual
                && snapshot.offers.iter().any(|left| {
                    left.use_id == prior.use_id
                        && snapshot.offers.iter().any(|right| {
                            right.use_id == state.use_id && left.group_id == right.group_id
                        })
                })
        }) {
            return Err(PlanError::Integrity);
        }
        if snapshot.use_states[..state_index].iter().any(|prior| {
            prior.accepted_event.event_id == state.accepted_event.event_id
                || prior.last_event.event_id == state.last_event.event_id
                || prior.accepted_event.event_id == state.last_event.event_id
                || prior.last_event.event_id == state.accepted_event.event_id
        }) {
            return Err(PlanError::Integrity);
        }
    }
    for lease in snapshot.leases.iter().flatten() {
        let is_managed = snapshot.registry.venues.iter().any(|v| v.resource_id == lease.resource);
        if is_managed {
            let count = snapshot.use_states.iter().filter(|state| {
                matches!(state.status, UseStatus::Reserved | UseStatus::Actual)
                    && state.token.number == lease.token
            }).count();
            if count != 1 {
                return Err(PlanError::Integrity);
            }
        }
    }
    let mut overcommitted_resources = 0usize;
    for (index, resource) in snapshot.resources.iter().enumerate() {
        let capacity = u64::from(resource.capacity);
        if totals[index].reserved > capacity
            || totals[index].occupied > capacity.saturating_sub(totals[index].reserved)
        {
            overcommitted_resources = overcommitted_resources
                .checked_add(1)
                .ok_or(PlanError::Overflow)?;
        }
    }
    Ok(RecoveryReport {
        checked_resources: snapshot.resources.len(),
        checked_leases: lease_count,
        checked_active_uses: active_count,
        overcommitted_resources,
    })
}

fn valid_opening_source(source: SourceRef, scope: Scope) -> bool {
    if source.scope != scope {
        return false;
    }
    match source.kind {
        SourceKind::Genesis => {
            source.revision == 0
                && source.operation_id == 0
                && source.event_id == 0
                && source.source_id > 0
        }
        SourceKind::GameEvent => {
            source.operation_id > 0 && source.event_id > 0 && source.source_id == 0
        }
    }
}

fn valid_runtime_source(source: SourceRef, scope: Scope, current_revision: u64) -> bool {
    source.kind == SourceKind::GameEvent
        && source.scope == scope
        && source.revision <= current_revision
        && source.operation_id > 0
        && source.event_id > 0
        && source.source_id == 0
}

fn verify_event(
    snapshot: Snapshot<'_>,
    request: Request,
    evidence: &VerifiedRootEvent,
) -> Result<(VenueDefinition, UseOffer, VenuePolicy), PlanError> {
    if request.use_id == 0 || request.operation_id == 0 {
        return Err(PlanError::Invalid);
    }
    if evidence.request != request
        || evidence.scope != snapshot.scope
        || evidence.revision != snapshot.revision
        || evidence.clock != snapshot.completed_clock
    {
        return Err(PlanError::Stale);
    }
    let offer = snapshot.offers.iter().find(|offer| offer.use_id == request.use_id)
        .copied().ok_or(PlanError::Unknown)?;
    let venue = snapshot.registry.venues.iter().find(|venue| venue.venue_id == offer.venue_id)
        .copied().ok_or(PlanError::Unknown)?;
    let policy = snapshot.policies.iter().find(|policy| policy.venue_id == venue.venue_id)
        .copied().ok_or(PlanError::Unknown)?;
    if evidence.offer != offer || evidence.venue != venue {
        return Err(PlanError::Stale);
    }
    let clock = require_clock(snapshot)?;
    let request_time_valid = match request.kind {
        RequestKind::Enter | RequestKind::SafeExit => request.occurred_at == Some(clock),
        RequestKind::Accept | RequestKind::CancelReserved => request.occurred_at.is_none(),
    };
    if evidence.scope != snapshot.scope
        || evidence.revision != snapshot.revision
        || evidence.event_source.kind != SourceKind::GameEvent
        || evidence.event_source.scope != snapshot.scope
        || evidence.event_source.revision != snapshot.revision
        || evidence.event_source.operation_id != request.operation_id
        || evidence.event_source.event_id == 0
        || evidence.event_source.source_id != 0
        || evidence.occurred_at != clock
        || !request_time_valid
    {
        return Err(PlanError::Stale);
    }
    if snapshot.use_states.iter().any(|state| {
        state.accepted_event.event_id == evidence.event_source.event_id
            || state.last_event.event_id == evidence.event_source.event_id
    }) {
        return Err(PlanError::Conflict);
    }
    Ok((venue, offer, policy))
}

fn require_clock(snapshot: Snapshot<'_>) -> Result<u64, PlanError> {
    let clock = snapshot.completed_clock.ok_or(PlanError::Unknown)?;
    if clock.scope != snapshot.scope || clock.revision != snapshot.revision {
        return Err(PlanError::Stale);
    }
    Ok(clock.tick)
}

fn expected_permission(kind: RequestKind) -> EffectivePermission {
    match kind {
        RequestKind::Accept => EffectivePermission::ReserveOffer,
        RequestKind::Enter => EffectivePermission::EnterVenue,
        RequestKind::SafeExit => EffectivePermission::SafeExitVenue,
        RequestKind::CancelReserved => EffectivePermission::CancelReservation,
    }
}

fn current_state(snapshot: Snapshot<'_>, use_id: u64) -> Result<&UseState, PlanError> {
    snapshot.use_states.iter().find(|state| state.use_id == use_id).ok_or(PlanError::Unknown)
}

fn event_actor(evidence: &VerifiedRootEvent) -> u32 {
    match evidence.actor {
        RootActor::System => 0,
        RootActor::Account(id) => id,
    }
}

fn candidate(pool: PoolDelta, metadata: UseStateDelta) -> Candidate {
    Candidate { pool, metadata }
}

pub fn plan(
    snapshot: Snapshot<'_>,
    request: Request,
    evidence: &VerifiedRootEvent,
) -> Result<Candidate, PlanError> {
    validate_recovery(snapshot)?;
    let (venue, offer, policy) = verify_event(snapshot, request, evidence)?;
    if evidence.permission != expected_permission(request.kind) {
        return Err(PlanError::Unauthorized);
    }
    let actor = event_actor(evidence);
    match request.kind {
        RequestKind::Accept => {
            let now = require_clock(snapshot)?;
            if evidence.token.is_some() {
                return Err(PlanError::Stale);
            }
            if evidence.actor == RootActor::System
                || evidence.actor != RootActor::Account(offer.operator)
            {
                return Err(PlanError::Unauthorized);
            }
            if !policy.open
                || policy.revoked
                || policy.owner != venue.owner
                || policy.operator != offer.operator
                || policy.permit_version != offer.accepted_version
            {
                return Err(PlanError::Unauthorized);
            }
            if now > offer.window.start || now >= offer.window.end || offer.window.start >= offer.window.end {
                return Err(PlanError::Invalid);
            }
            if snapshot.use_states.len() >= MAX_USE_STATES {
                return Err(PlanError::Quota);
            }
            if snapshot.use_states.iter().any(|state| state.use_id == offer.use_id) {
                return Err(PlanError::Conflict);
            }
            let units = offer.simultaneous_people().map_err(|_| PlanError::Invalid)?;
            let (totals, _) = lease_totals(snapshot)?;
            let resource_index = resource_index(snapshot.resources, venue.resource_id)
                .ok_or(PlanError::Integrity)?;
            let resource = snapshot.resources[resource_index];
            let held = totals[resource_index]
                .reserved
                .checked_add(totals[resource_index].occupied)
                .ok_or(PlanError::Overflow)?;
            let requested = held.checked_add(u64::from(units)).ok_or(PlanError::Overflow)?;
            if requested > u64::from(resource.capacity) {
                return Err(PlanError::CapacityGateRejected);
            }
            Ok(candidate(
                PoolDelta::Reserve { resource_id: venue.resource_id, actor, units },
                UseStateDelta::None,
            ))
        }
        RequestKind::Enter => {
            let now = require_clock(snapshot)?;
            if evidence.token.is_none()
                || request.occurred_at != Some(now)
                || now < offer.window.start
                || now >= offer.window.end
            {
                return Err(PlanError::Invalid);
            }
            if evidence.actor != RootActor::System {
                return Err(PlanError::Unauthorized);
            }
            if !policy.open
                || policy.revoked
                || policy.owner != venue.owner
                || policy.operator != offer.operator
                || policy.permit_version != offer.accepted_version
            {
                return Err(PlanError::Unauthorized);
            }
            let state = *current_state(snapshot, offer.use_id)?;
            if state.status != UseStatus::Reserved {
                return Err(PlanError::Conflict);
            }
            if evidence.token != Some(state.token)
                || snapshot.use_states.iter().any(|other| {
                    other.use_id != state.use_id
                        && other.status == UseStatus::Actual
                        && snapshot.offers.iter().any(|left| {
                            left.use_id == state.use_id
                                && snapshot.offers.iter().any(|right| {
                                    right.use_id == other.use_id && left.group_id == right.group_id
                                })
                        })
                })
            {
                return Err(PlanError::Conflict);
            }
            let lease = snapshot.leases.iter().flatten()
                .find(|lease| lease.token == state.token.number)
                .ok_or(PlanError::Integrity)?;
            if lease.occupied || lease.actor != state.actor || lease.resource != state.resource_id
                || lease.units != state.units
            {
                return Err(PlanError::Integrity);
            }
            Ok(candidate(
                PoolDelta::Occupy { token: state.token },
                UseStateDelta::Transition {
                    use_id: state.use_id,
                    from: UseStatus::Reserved,
                    to: UseStatus::Actual,
                    event: evidence.event_source,
                    occurred_at: now,
                },
            ))
        }
        RequestKind::SafeExit => {
            let now = require_clock(snapshot)?;
            if evidence.token.is_none() || request.occurred_at != Some(now) {
                return Err(PlanError::Stale);
            }
            if evidence.actor != RootActor::System {
                return Err(PlanError::Unauthorized);
            }
            let state = *current_state(snapshot, offer.use_id)?;
            if state.status != UseStatus::Actual {
                return Err(PlanError::Conflict);
            }
            if evidence.token != Some(state.token)
                || now < state.entered_at.ok_or(PlanError::Integrity)?
            {
                return Err(PlanError::Conflict);
            }
            let lease = snapshot.leases.iter().flatten()
                .find(|lease| lease.token == state.token.number)
                .ok_or(PlanError::Integrity)?;
            if !lease.occupied || lease.actor != state.actor || lease.resource != state.resource_id
                || lease.units != state.units
            {
                return Err(PlanError::Integrity);
            }
            Ok(candidate(
                PoolDelta::Release { token: state.token },
                UseStateDelta::Transition {
                    use_id: state.use_id,
                    from: UseStatus::Actual,
                    to: UseStatus::Released,
                    event: evidence.event_source,
                    occurred_at: now,
                },
            ))
        }
        RequestKind::CancelReserved => {
            let now = require_clock(snapshot)?;
            if evidence.token.is_none() {
                return Err(PlanError::Stale);
            }
            let state = *current_state(snapshot, offer.use_id)?;
            if state.status != UseStatus::Reserved {
                return Err(PlanError::Conflict);
            }
            if evidence.token != Some(state.token) {
                return Err(PlanError::Stale);
            }
            let lease = snapshot.leases.iter().flatten()
                .find(|lease| lease.token == state.token.number)
                .ok_or(PlanError::Integrity)?;
            if lease.occupied || lease.actor != state.actor || lease.resource != state.resource_id
                || lease.units != state.units
            {
                return Err(PlanError::Integrity);
            }
            Ok(candidate(
                PoolDelta::Release { token: state.token },
                UseStateDelta::Transition {
                    use_id: state.use_id,
                    from: UseStatus::Reserved,
                    to: UseStatus::Cancelled,
                    event: evidence.event_source,
                    occurred_at: now,
                },
            ))
        }
    }
}

fn same_resource(a: Resource, b: Resource) -> bool {
    a.id == b.id && a.capacity == b.capacity && a.reserved == b.reserved && a.occupied == b.occupied
}

fn same_snapshot_base(left: Snapshot<'_>, right: Snapshot<'_>) -> bool {
    left.scope == right.scope
        && left.revision == right.revision
        && left.completed_clock == right.completed_clock
        && left.registry.venues == right.registry.venues
        && left.offers == right.offers
        && left.policies == right.policies
        && left.use_states == right.use_states
        && left.non_venue_resource_ids == right.non_venue_resource_ids
}

/// Bind an original-pool reservation candidate to immutable offer metadata. `before` and
/// `after` must share one source snapshot; the only accepted difference is one exact new
/// reserved lease and its matching Resource counter increment. Root commits both candidates
/// atomically. This function does not allocate a token or claim a receipt was written.
pub fn bind_reserved_offer(
    before: Snapshot<'_>,
    after: Snapshot<'_>,
    request: Request,
    evidence: &VerifiedRootEvent,
) -> Result<Candidate, PlanError> {
    if request.kind != RequestKind::Accept || !same_snapshot_base(before, after) {
        return Err(PlanError::Stale);
    }
    let expected = plan(before, request, evidence)?;
    let PoolDelta::Reserve { resource_id, actor, units } = expected.pool else {
        return Err(PlanError::Integrity);
    };
    if after.use_states.len() != before.use_states.len()
        || after.resources.len() != before.resources.len()
        || after.leases.len() != before.leases.len()
    {
        return Err(PlanError::Integrity);
    }
    let mut new_lease: Option<Lease> = None;
    for after_lease in after.leases.iter().flatten() {
        if before.leases.iter().flatten().any(|old| old.token == after_lease.token) {
            let old = before.leases.iter().flatten().find(|old| old.token == after_lease.token).unwrap();
            if !same_lease(*old, *after_lease) {
                return Err(PlanError::Integrity);
            }
        } else if new_lease.replace(*after_lease).is_some() {
            return Err(PlanError::Integrity);
        }
    }
    let lease = new_lease.ok_or(PlanError::Integrity)?;
    if lease.resource != resource_id || lease.actor != actor || lease.units != units || lease.occupied {
        return Err(PlanError::Integrity);
    }
    let before_count = before.leases.iter().flatten().count();
    let after_count = after.leases.iter().flatten().count();
    if after_count != before_count.checked_add(1).ok_or(PlanError::Overflow)? {
        return Err(PlanError::Integrity);
    }
    for (old, new) in before.resources.iter().zip(after.resources.iter()) {
        if old.id != new.id || old.capacity != new.capacity {
            return Err(PlanError::Integrity);
        }
        if old.id == resource_id {
            if new.reserved.checked_sub(old.reserved) != Some(units) || new.occupied != old.occupied {
                return Err(PlanError::Integrity);
            }
        } else if !same_resource(*old, *new) {
            return Err(PlanError::Integrity);
        }
    }
    if before.resources.iter().filter(|r| r.id == resource_id).count() != 1 {
        return Err(PlanError::Integrity);
    }
    if before.use_states.iter().any(|state| state.token.number == lease.token) {
        return Err(PlanError::Conflict);
    }
    let _ = lease_totals(after)?;
    let (venue, offer, _) = verify_event(before, request, evidence)?;
    let state = UseState {
        use_id: offer.use_id,
        token: CapacityToken { world: before.scope.world(), branch: before.scope.branch(), number: lease.token },
        actor: lease.actor,
        resource_id: lease.resource,
        units: lease.units,
        accepted_scope: before.scope,
        accepted_revision: before.revision,
        accepted_operation_id: request.operation_id,
        accepted_owner: venue.owner,
        accepted_operator: offer.operator,
        accepted_version: offer.accepted_version,
        accepted_event: evidence.event_source,
        last_event: evidence.event_source,
        accepted_at: evidence.occurred_at,
        entered_at: None,
        released_at: None,
        status: UseStatus::Reserved,
    };
    Ok(candidate(PoolDelta::None, UseStateDelta::Insert(state)))
}

/// Exact in-memory field layout for callers; it is not a stack-peak or shared-memory grant.
pub const OPENING_ROW_LAYOUT_BYTES: usize = MAX_VENUES * VENUE_DEFINITION_BYTES
    + MAX_USE_OFFERS * USE_OFFER_BYTES;
pub const POLICY_ROW_LAYOUT_BYTES: usize = MAX_VENUES * VENUE_POLICY_BYTES;
pub const RUNTIME_ROW_LAYOUT_BYTES: usize = MAX_USE_STATES * USE_STATE_BYTES;
pub const BORROWED_ORIGINAL_POOL_LAYOUT_BYTES: usize = MAX_RESOURCES * size_of::<Resource>()
    + MAX_LEASES * size_of::<Option<Lease>>();
