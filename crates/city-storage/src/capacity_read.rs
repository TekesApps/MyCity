//! One-call, store-local observation of original Resource/Lease and receipt rows.
//!
//! This is diagnostic-only. It does not grant rights, predict admission, or provide a
//! durable identity across Store instances. A committed receipt is copied verbatim.
use crate::{Error as StoreError, Store};
use mycity_core::{
    arena::Scope,
    transaction::{JournalRecord, Lease, Resource, MAX_LEASES, MAX_RESOURCES},
    timeline::Progress,
};
use std::mem::size_of;

#[derive(Debug)]
pub enum ReadError {
    Store(StoreError),
    Length,
    Overflow,
    Integrity,
}
impl From<StoreError> for ReadError {
    fn from(value: StoreError) -> Self { Self::Store(value) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelinePresence { Absent, Partial, Populated }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaseCounters {
    pub checked_lease_count: usize,
    pub referenced_resources_valid: bool,
    pub reserved_sums_match: bool,
    pub occupied_sums_match: bool,
}
impl LeaseCounters {
    pub fn consistent(self) -> bool {
        self.referenced_resources_valid && self.reserved_sums_match && self.occupied_sums_match
    }
}

/// Fixed-size projection. Measure with `size_of::<Projection>()`; this excludes the separately
/// boxed original journal record, Store::receipt decoder/stack costs, and allocator metadata.
#[derive(Clone, Copy, Debug)]
pub struct Projection {
    pub resources: [Resource; MAX_RESOURCES],
    pub resource_rows: usize,
    pub leases: [Option<Lease>; MAX_LEASES],
    pub lease_slots: usize,
    pub progress: Option<Progress>,
    pub timeline_presence: TimelinePresence,
    pub lease_counters: LeaseCounters,
}
pub const PROJECTION_BYTES: usize = size_of::<Projection>();
pub const ORIGINAL_RECORD_BYTES: usize = size_of::<JournalRecord>();
pub const BOXED_RECORD_HANDLE_BYTES: usize = size_of::<Box<JournalRecord>>();
/// Two fixed per-resource sum arrays are live while constructing the projection.
pub const COUNTER_SCRATCH_BYTES: usize = 2 * size_of::<[u64; MAX_RESOURCES]>();

#[derive(Debug)]
pub struct LocalCapacityObservation {
    pub scope: Scope,
    pub revision: u64,
    pub committed_sequence: u64,
    pub projection: Projection,
    /// Full immutable source record; retained independently from the bounded projection.
    pub receipt: Option<Box<JournalRecord>>,
    /// Diagnostic classifications only. Never authorization or admission evidence.
    pub units_aliases_windows_rights_future_admission: UnknownFields,
}
pub const OBSERVATION_VALUE_BYTES: usize = size_of::<LocalCapacityObservation>();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownFields {
    pub units: Unknown,
    pub aliases: Unknown,
    pub windows: Unknown,
    pub rights: Unknown,
    pub future_admission: Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unknown { Unknown }

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    scope: Scope,
    revision: u64,
    sequence: u64,
    progress: Option<Progress>,
}
pub const OBSERVATION_STAMP_BYTES: usize = size_of::<Stamp>();
/// Selected component bookkeeping sum only; this is neither a stack peak nor an upper bound.
/// It omits make_projection return/array-move overlap, outer value fields and padding,
/// JournalRecord temporaries and decoder cost, and allocator metadata.
pub const SELECTED_COMPONENT_BYTES: usize = PROJECTION_BYTES
    + 2 * OBSERVATION_STAMP_BYTES
    + COUNTER_SCRATCH_BYTES
    + BOXED_RECORD_HANDLE_BYTES;

fn stamp(store: &Store) -> Result<Stamp, ReadError> {
    let engine = store.engine()?;
    let (scope, revision) = engine.world().scope_revision();
    let progress = engine.timeline().map(|t| t.progress);
    Ok(Stamp { scope, revision, sequence: store.committed_sequence(), progress })
}

fn make_projection(engine: &mycity_core::transaction::Engine) -> Result<Projection, ReadError> {
    let raw_resources = engine.resources();
    let raw_leases = engine.leases();
    if raw_resources.len() > MAX_RESOURCES || raw_leases.len() > MAX_LEASES { return Err(ReadError::Length); }
    let mut resources = [Resource { id: 0, capacity: 0, reserved: 0, occupied: 0 }; MAX_RESOURCES];
    let mut leases: [Option<Lease>; MAX_LEASES] = [None; MAX_LEASES];
    for (dst, src) in resources.iter_mut().zip(raw_resources.iter()) {
        *dst = *src;
    }
    for (dst, src) in leases.iter_mut().zip(raw_leases.iter()) {
        *dst = *src;
    }
    let mut reserved = [0u64; MAX_RESOURCES];
    let mut occupied = [0u64; MAX_RESOURCES];
    let mut referenced_resources_valid = true;
    let mut checked_lease_count = 0usize;
    for lease in raw_leases.iter().flatten() {
        checked_lease_count = checked_lease_count.checked_add(1).ok_or(ReadError::Overflow)?;
        let Some(index) = raw_resources.iter().position(|r| r.id == lease.resource && r.id != 0) else {
            referenced_resources_valid = false;
            continue;
        };
        let total = if lease.occupied { &mut occupied[index] } else { &mut reserved[index] };
        *total = total.checked_add(lease.units as u64).ok_or(ReadError::Overflow)?;
    }
    let reserved_sums_match = raw_resources.iter().enumerate().all(|(i, r)| reserved[i] == r.reserved as u64);
    let occupied_sums_match = raw_resources.iter().enumerate().all(|(i, r)| occupied[i] == r.occupied as u64);
    let progress = engine.timeline().map(|t| t.progress);
    let timeline_presence = match progress {
        None => TimelinePresence::Absent,
        Some(p) if p.target.is_some() || p.phase.is_some() || p.at != p.last_complete => TimelinePresence::Partial,
        Some(_) => TimelinePresence::Populated,
    };
    Ok(Projection {
        resources,
        resource_rows: raw_resources.len(),
        leases,
        lease_slots: raw_leases.len(),
        progress,
        timeline_presence,
        lease_counters: LeaseCounters { checked_lease_count, referenced_resources_valid, reserved_sums_match, occupied_sums_match },
    })
}

/// Observe original fixed rows and optionally one exact Store receipt during a single exclusive
/// Store call. `None` from receipt lookup is preserved as None and never means safe to retry.
pub fn observe(store: &mut Store, receipt_operation: Option<u64>) -> Result<LocalCapacityObservation, ReadError> {
    let (before, projection) = {
        let engine = store.engine()?;
        let (scope, revision) = engine.world().scope_revision();
        let progress = engine.timeline().map(|t| t.progress);
        let before = Stamp { scope, revision, sequence: store.committed_sequence(), progress };
        let projection = make_projection(engine)?;
        (before, projection)
    };
    let receipt = match receipt_operation {
        Some(operation) => store.receipt(operation)?.map(Box::new),
        None => None,
    };
    let after = stamp(store)?;
    if before != after { return Err(ReadError::Integrity); }
    Ok(LocalCapacityObservation {
        scope: before.scope,
        revision: before.revision,
        committed_sequence: before.sequence,
        projection,
        receipt,
        units_aliases_windows_rights_future_admission: UnknownFields {
            units: Unknown::Unknown,
            aliases: Unknown::Unknown,
            windows: Unknown::Unknown,
            rights: Unknown::Unknown,
            future_admission: Unknown::Unknown,
        },
    })
}
