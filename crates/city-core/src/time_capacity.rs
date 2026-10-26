//! Explicit timed projection of the one original Resource/Lease pool.
//! Rows here add identity/window context; quantities and Actual state remain in Lease.
use crate::{model_rows::{Record, Rows}, transaction::{Lease, Resource, MAX_LEASES, MAX_RESOURCES}};
use serde::{Deserialize, Serialize};

pub const PROFILE_VERSION: u32 = 1;
pub const MAX_ALIASES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit { Count, ItemsPerSecond, Kilowatts }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    #[serde(with = "canonical_decimal")]
    pub canonical: u128,
    pub resource: u32,
    pub aliases: [Option<u64>; MAX_ALIASES],
    pub unit: Unit,
    pub owner: u32,
    pub operator: u32,
    pub nominal_capacity: u32,
    pub effective_capacity: u32,
    pub identity_version: u64,
    pub policy_version: u64,
    pub open: bool,
}
impl Record for Registration {
    fn inactive() -> Self { Self { canonical: 0, resource: 0, aliases: [None; MAX_ALIASES], unit: Unit::Count, owner: 0, operator: 0, nominal_capacity: 0, effective_capacity: 0, identity_version: 0, policy_version: 0, open: false } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub token: u64,
    pub consumer: u64,
    pub consumer_version: u64,
    pub identity_version: u64,
    pub start: u64,
    pub end: u64,
    pub accepted_operation: u64,
}
impl Record for Window {
    fn inactive() -> Self { Self { token: 0, consumer: 0, consumer_version: 0, identity_version: 0, start: 0, end: 0, accepted_operation: 0 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Register { registration: Registration },
    SetPolicy { alias: u64, expected_version: u64, effective_capacity: u32, open: bool },
    Reserve { alias: u64, account: u32, consumer: u64, consumer_version: u64, start: u64, end: u64, units: u32 },
    Enter { token: u64, occurred_at: u64 },
    Cancel { token: u64 },
    Exit { token: u64, occurred_at: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    pub version: u32,
    pub registrations: Rows<Registration, MAX_RESOURCES>,
    pub windows: Rows<Window, MAX_LEASES>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projection {
    pub reserved_peak: u32,
    pub occupied: u32,
    /// A forecast/commitment conflict; it does not assert physical over-occupancy.
    pub pending_conflict: bool,
    pub effective_overcommit: bool,
}

impl Runtime {
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self { version: PROFILE_VERSION, registrations: Rows::new()?, windows: Rows::new()? })
    }
    pub const fn max_owned_bytes() -> usize {
        std::mem::size_of::<Self>() + MAX_RESOURCES * std::mem::size_of::<Registration>() + MAX_LEASES * std::mem::size_of::<Window>()
    }
    pub fn owned_bytes(&self) -> usize { Self::max_owned_bytes() }
    pub fn manages(&self, resource: u32) -> bool { self.registrations.iter().any(|r| r.resource == resource) }
    pub fn resolve(&self, alias: u64) -> Option<&Registration> {
        self.registrations.iter().find(|r| r.aliases.contains(&Some(alias)))
    }
    /// No allocated event list or sampled averages: maxima occur at original starts.
    pub fn project(&self, registration: &Registration, leases: &[Option<Lease>]) -> Result<Projection, &'static str> {
        let mut occupied = 0u32;
        let mut peak = 0u32;
        for lease in leases.iter().flatten().filter(|l| l.resource == registration.resource) {
            if lease.occupied { occupied = occupied.checked_add(lease.units).ok_or("timed_capacity_overflow")?; }
            else {
                let window = self.windows.iter().find(|w| w.token == lease.token).ok_or("timed_capacity_window_missing")?;
                let mut at_start = 0u32;
                for other in leases.iter().flatten().filter(|l| l.resource == registration.resource && !l.occupied) {
                    let w = self.windows.iter().find(|w| w.token == other.token).ok_or("timed_capacity_window_missing")?;
                    if w.start <= window.start && window.start < w.end { at_start = at_start.checked_add(other.units).ok_or("timed_capacity_overflow")?; }
                }
                peak = peak.max(at_start);
            }
        }
        Ok(Projection { reserved_peak: peak, occupied,
            pending_conflict: u64::from(occupied) + u64::from(peak) > u64::from(registration.effective_capacity),
            effective_overcommit: occupied > registration.effective_capacity })
    }
    /// Validate the original rows and supplementary index in both directions.
    /// A future conflict or a recorded effective-capacity reduction is recoverable.
    pub fn validate(&self, resources: &[Resource], leases: &[Option<Lease>]) -> Result<(), &'static str> {
        if self.version != PROFILE_VERSION { return Err("timed_capacity_profile_version"); }
        for (i, r) in self.registrations.iter().enumerate() {
            if r.canonical == 0 || r.resource == 0 || r.owner == 0 || r.operator == 0 || r.nominal_capacity == 0
                || r.effective_capacity > r.nominal_capacity || r.identity_version == 0 || r.policy_version == 0
                || !r.aliases.iter().any(Option::is_some) || r.aliases.contains(&Some(0)) {
                return Err("timed_capacity_registration");
            }
            if self.registrations[..i].iter().any(|old| old.canonical == r.canonical || old.resource == r.resource) { return Err("timed_capacity_duplicate_resource"); }
            if resources.iter().any(|old| old.id != r.resource && (r.canonical == u128::from(old.id) || r.aliases.contains(&Some(u64::from(old.id))))) { return Err("timed_capacity_foreign_resource_alias"); }
            if self.registrations[..i].iter().any(|old| old.aliases.iter().flatten().any(|a| u128::from(*a) == r.canonical) || r.aliases.iter().flatten().any(|a| u128::from(*a) == old.canonical)) { return Err("timed_capacity_canonical_alias_conflict"); }

            for (j, alias) in r.aliases.iter().enumerate() {
                if let Some(alias) = alias {
                    if r.aliases[..j].contains(&Some(*alias)) || self.registrations[..i].iter().any(|old| old.aliases.contains(&Some(*alias))) { return Err("timed_capacity_duplicate_alias"); }
                }
            }
            let original = resources.iter().find(|row| row.id == r.resource).ok_or("timed_capacity_original_missing")?;
            if original.capacity != r.nominal_capacity { return Err("timed_capacity_original_changed"); }
            let projection = self.project(r, leases)?;
            if original.reserved != projection.reserved_peak || original.occupied != projection.occupied || original.occupied > original.capacity { return Err("timed_capacity_projection_mismatch"); }
        }
        for (i, w) in self.windows.iter().enumerate() {
            if w.token == 0 || w.consumer == 0 || w.consumer_version == 0 || w.identity_version == 0 || w.start >= w.end || w.accepted_operation == 0
                || self.windows[..i].iter().any(|old| old.token == w.token) { return Err("timed_capacity_window"); }
            let lease = leases.iter().flatten().find(|lease| lease.token == w.token).ok_or("timed_capacity_orphan_window")?;
            let r = self.registrations.iter().find(|r| r.resource == lease.resource).ok_or("timed_capacity_unmanaged_window")?;
            if w.identity_version != r.identity_version { return Err("timed_capacity_identity_changed"); }
        }
        for lease in leases.iter().flatten().filter(|l| self.manages(l.resource)) {
            if self.windows.iter().filter(|w| w.token == lease.token).count() != 1 { return Err("timed_capacity_missing_window"); }
        }
        Ok(())
    }
}

// Internally tagged game commands buffer through serde Content, whose integer
// visitor does not support u128. Decimal identifiers keep the full width intact.
mod canonical_decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u128, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse::<u128>().map_err(serde::de::Error::custom)
    }
}
