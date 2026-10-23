//! Store-owned fixed daily transport allocation and immutable settlement proofs.
use super::*;
use mycity_core::market_access::{
    SettlementDeliveryInput, TransportAllocationProfile, TransportProofAsset,
};

pub(super) const ALLOCATION_PREFIX: &str = "town-allocation-";
pub(super) const PROOF_PREFIX: &str = "town-proof-";
pub(super) const PROOF_LIMIT: usize = mycity_core::market_access::TRANSPORT_PROOF_READ_LIMIT;

fn name(prefix: &str, hash: &[u8; 32]) -> String {
    format!("{prefix}{}.json", super::hex(hash))
}

#[derive(Clone, Copy)]
pub(super) struct ArchivedProofRef {
    profile_id: u32,
    service_day: u64,
    generation: u64,
    proof_sha256: [u8; 32],
}

pub(super) struct ArchivedProofRefs(Vec<ArchivedProofRef>);

impl ArchivedProofRefs {
    pub(super) fn new() -> Result<Self, Error> {
        Ok(Self(Vec::new()))
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    pub(super) fn push_record(&mut self, record: &JournalRecord) -> Result<(), Error> {
        for action in record.actions.iter().flatten() {
            if let mycity_core::transaction::Action::TownTransportSettle {
                profile_id,
                service_day,
                generation,
                proof_sha256,
            } = action
            {
                if self.0.capacity() == 0 {
                    self.0
                        .try_reserve_exact(MAX_RECEIPTS)
                        .map_err(|_| Error::Invalid("town_transport_archive_allocation"))?;
                    if self.0.capacity() != MAX_RECEIPTS {
                        return Err(Error::Invalid("town_transport_archive_capacity"));
                    }
                }
                if self.0.len() == self.0.capacity() {
                    return Err(Error::Invalid("town_transport_archive_proof_count"));
                }
                self.0.push(ArchivedProofRef {
                    profile_id: *profile_id,
                    service_day: *service_day,
                    generation: *generation,
                    proof_sha256: *proof_sha256,
                });
            }
        }
        Ok(())
    }

    pub(super) fn verify(
        &self,
        dir: &Path,
        metadata: &mut MetadataWorkspace,
        engine: &Engine,
        profile: Option<&TransportAllocationProfile>,
        allocation_hash: Option<[u8; 32]>,
        catalog: Option<&super::spatial_transport_updates::SpatialTransportCatalog>,
    ) -> Result<(), Error> {
        for reference in &self.0 {
            let (profile, allocation_hash) = if let Some(catalog) = catalog {
                let scope = engine.world().view().scope;
                if catalog.profile_id != reference.profile_id
                    || catalog.world_id != scope.world()
                    || catalog.branch_id != scope.branch()
                {
                    return Err(Error::Invalid("town_transport_archive_profile"));
                }
                let hash = catalog
                    .profile_sha256_for_service_day(reference.service_day)
                    .map_err(|_| Error::Invalid("town_transport_catalog_service_day"))?;
                let market = engine
                    .world()
                    .view()
                    .town
                    .ok_or(Error::Invalid("town_transport_market_missing"))?;
                let profile = read_allocation_for_settlement(
                    dir,
                    metadata,
                    &hash,
                    catalog.world_id,
                    catalog.branch_id,
                    market.profile.id,
                )?;
                (profile, hash)
            } else {
                (
                    profile
                        .cloned()
                        .ok_or(Error::Invalid("town_transport_profile_missing"))?,
                    allocation_hash.ok_or(Error::Invalid("town_transport_profile_missing"))?,
                )
            };
            let proof = read_proof(dir, metadata, reference.proof_sha256)?;
            if reference.profile_id != profile.market_profile_id {
                return Err(Error::Invalid("town_transport_archive_profile"));
            }
            verify_proof_matches_allocation(
                engine,
                &proof,
                &profile,
                allocation_hash,
                reference.proof_sha256,
                reference.service_day,
                reference.generation,
            )?;
        }
        Ok(())
    }
}

pub(super) fn verify_latest_market_cursor(
    engine: &Engine,
    dir: &Path,
    metadata: &mut MetadataWorkspace,
    profile: Option<&TransportAllocationProfile>,
    allocation_hash: Option<[u8; 32]>,
    catalog: Option<&super::spatial_transport_updates::SpatialTransportCatalog>,
) -> Result<(), Error> {
    let Some(market) = engine.world().view().town else {
        if profile.is_some() || allocation_hash.is_some() {
            return Err(Error::Invalid("town_transport_market_missing"));
        }
        return Ok(());
    };
    let Some(receipt) = market.state.last_transport else {
        let bound = market
            .profile
            .transport_allocation_sha256
            .iter()
            .any(|byte| *byte != 0);
        if bound && market.state.settled_day != 0 {
            return Err(Error::Invalid("town_transport_checkpoint_cursor_missing"));
        }
        return Ok(());
    };
    if market
        .profile
        .transport_allocation_sha256
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(Error::Invalid("town_transport_checkpoint_unbound_cursor"));
    }
    if receipt.issued_day != market.state.settled_day
        || receipt.issued_generation != market.state.settled_day
        || receipt.consumed_generation != market.state.settled_day
        || receipt.issued_proof_sha256.iter().all(|byte| *byte == 0)
        || receipt.resolved_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(Error::Invalid("town_transport_checkpoint_cursor"));
    }
    let (profile, allocation_hash) = if let Some(catalog) = catalog {
        if catalog.profile_id != market.profile.id
            || catalog.world_id != engine.world().view().scope.world()
            || catalog.branch_id != engine.world().view().scope.branch()
        {
            return Err(Error::Invalid("town_transport_catalog_binding"));
        }
        let hash = catalog
            .profile_sha256_for_service_day(receipt.issued_day)
            .map_err(|_| Error::Invalid("town_transport_catalog_service_day"))?;
        let profile = read_allocation_for_settlement(
            dir,
            metadata,
            &hash,
            catalog.world_id,
            catalog.branch_id,
            catalog.profile_id,
        )?;
        (profile, hash)
    } else {
        (
            profile
                .cloned()
                .ok_or(Error::Invalid("town_transport_profile_missing"))?,
            allocation_hash.ok_or(Error::Invalid("town_transport_profile_missing"))?,
        )
    };
    let proof = read_proof(dir, metadata, receipt.issued_proof_sha256)?;
    verify_proof_matches_allocation(
        engine,
        &proof,
        &profile,
        allocation_hash,
        receipt.issued_proof_sha256,
        receipt.issued_day,
        receipt.issued_generation,
    )?;
    Ok(())
}

pub(super) fn validate_allocation(
    bytes: &[u8],
    world_id: u64,
    branch_id: u64,
    profile_id: u32,
    expected_hash: [u8; 32],
) -> Result<TransportAllocationProfile, Error> {
    if bytes.len() > PROOF_LIMIT
        || bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES
        || super::digest(bytes) != expected_hash
    {
        return Err(Error::Invalid("town_transport_allocation_hash_or_limit"));
    }
    metadata::guard(bytes, 4096)?;
    let profile = decode_allocation(bytes)?;
    profile
        .validate()
        .map_err(|_| Error::Invalid("town_transport_allocation_invalid"))?;
    validate_allocation_scope(profile, world_id, branch_id, profile_id)
}

/// The immutable profile and fixed Project must agree at genesis/open. The road resource is
/// exclusive to this project and cannot also serve water, power, waste, or generic leases.
pub(super) fn validate_allocation_project(
    engine: &Engine,
    profile: &TransportAllocationProfile,
) -> Result<(), Error> {
    let view = engine.world().view();
    let Some(binding) = profile.road_capacity_effect else {
        if view
            .construction
            .is_some_and(|project| project.operating_edge_id != 0)
        {
            return Err(Error::Invalid("town_transport_road_binding_missing"));
        }
        return Ok(());
    };
    let project = view
        .construction
        .ok_or(Error::Invalid("town_transport_road_project_missing"))?;
    let resource = engine
        .resources()
        .iter()
        .find(|resource| resource.id == binding.resource_id)
        .ok_or(Error::Invalid("town_transport_road_resource_missing"))?;
    let expected_capacity = project
        .operating_base_capacity
        .checked_add(
            if project.operating_published && project.operating_withdrawn_at.is_none() {
                project.operating_units
            } else {
                0
            },
        )
        .ok_or(Error::Invalid("town_transport_road_capacity_overflow"))?;
    if project.id != binding.project_id
        || project.kind
            != if project.operating_withdrawn_at.is_some() || project.is_uncommissioned_clearance()
            {
                mycity_core::construction::Kind::Demolish
            } else {
                mycity_core::construction::Kind::Build
            }
        || project.operating_edge_id != binding.edge_id
        || project.operating_resource != binding.resource_id
        || project.operating_base_capacity != binding.base_capacity_people
        || project.operating_units != binding.added_capacity_people
        || resource.capacity != expected_capacity
        || resource.reserved != 0
        || resource.occupied != 0
        || project.site_resource == binding.resource_id
        || (project.waste_units != 0 && project.waste_receiver_resource == binding.resource_id)
        || view
            .boot
            .is_some_and(|boot| boot.resources.contains(&binding.resource_id))
        || view.exchange.is_some_and(|exchange| {
            exchange
                .edges
                .iter()
                .any(|edge| edge.resource == binding.resource_id)
        })
    {
        return Err(Error::Invalid("town_transport_road_project_mismatch"));
    }
    // A pristine bound road is valid before the timeline has been enabled.
    // Require the clock only when there is an actual publication/withdrawal epoch.
    if project.operating_published_at.is_none() && project.operating_withdrawn_at.is_none() {
        return Ok(());
    }
    let last_complete = engine
        .timeline()
        .map(|timeline| timeline.progress.last_complete)
        .ok_or(Error::Invalid("town_transport_road_project_mismatch"))?;
    let settled_day = engine
        .world()
        .view()
        .town
        .map(|town| town.state.settled_day)
        .ok_or(Error::Invalid("town_transport_road_project_mismatch"))?;
    for at in [
        project.operating_published_at,
        project.operating_withdrawn_at,
    ]
    .into_iter()
    .flatten()
    {
        let seconds = u64::try_from(at.value())
            .map_err(|_| Error::Invalid("town_transport_road_project_mismatch"))?;
        let day = u64::from(mycity_core::timeline::DAY as u32);
        if seconds % day != 0 || seconds > last_complete || seconds / day > settled_day {
            return Err(Error::Invalid("town_transport_road_project_mismatch"));
        }
    }
    Ok(())
}

pub(super) fn validate_no_orphan_road_project(engine: &Engine) -> Result<(), Error> {
    if engine
        .world()
        .view()
        .construction
        .is_some_and(|project| project.operating_edge_id != 0)
    {
        return Err(Error::Invalid("town_transport_road_binding_missing"));
    }
    Ok(())
}

fn road_effect_times(
    engine: &Engine,
    profile: &TransportAllocationProfile,
) -> Result<
    (
        Option<mycity_core::units::GameTime>,
        Option<mycity_core::units::GameTime>,
    ),
    Error,
> {
    validate_allocation_project(engine, profile)?;
    Ok(profile.road_capacity_effect.map_or((None, None), |_| {
        engine
            .world()
            .view()
            .construction
            .map_or((None, None), |project| {
                (
                    project.operating_published_at,
                    project.operating_withdrawn_at,
                )
            })
    }))
}

fn decode_allocation(bytes: &[u8]) -> Result<TransportAllocationProfile, Error> {
    metadata::guard(bytes, 4096)?;
    Ok(serde_json::from_slice(bytes)?)
}

fn validate_allocation_scope(
    profile: TransportAllocationProfile,
    world_id: u64,
    branch_id: u64,
    profile_id: u32,
) -> Result<TransportAllocationProfile, Error> {
    if profile.world_id != world_id
        || profile.branch_id != branch_id
        || profile.market_profile_id != profile_id
    {
        return Err(Error::Invalid("town_transport_allocation_scope"));
    }
    Ok(profile)
}

pub(super) fn allocation_ref(engine: &Engine) -> Option<[u8; 32]> {
    engine.world().view().town.and_then(|town| {
        let hash = town.profile.transport_allocation_sha256;
        hash.iter().any(|byte| *byte != 0).then_some(hash)
    })
}

pub(super) fn read_allocation(
    dir: &Path,
    metadata: &mut MetadataWorkspace,
    hash: &[u8; 32],
    world_id: u64,
    branch_id: u64,
    profile_id: u32,
) -> Result<TransportAllocationProfile, Error> {
    metadata
        .file
        .read(&dir.join(name(ALLOCATION_PREFIX, hash)), PROOF_LIMIT)?;
    validate_allocation(
        metadata.file.as_slice(),
        world_id,
        branch_id,
        profile_id,
        *hash,
    )
}

/// Runtime settlement reads the immutable profile version selected by the committed catalog's
/// service-day boundary. The Engine resolver performs one Assignment verification for that day.
pub(super) fn read_allocation_for_settlement(
    dir: &Path,
    metadata: &mut MetadataWorkspace,
    hash: &[u8; 32],
    world_id: u64,
    branch_id: u64,
    profile_id: u32,
) -> Result<TransportAllocationProfile, Error> {
    metadata
        .file
        .read(&dir.join(name(ALLOCATION_PREFIX, hash)), PROOF_LIMIT)?;
    let bytes = metadata.file.as_slice();
    if bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES
        || super::digest(bytes) != *hash
    {
        return Err(Error::Invalid("town_transport_allocation_hash_or_limit"));
    }
    validate_allocation_scope(decode_allocation(bytes)?, world_id, branch_id, profile_id)
}

/// Bounded raw-byte read for immutable spatial profile revision lookup. Preserving these bytes
/// matters because historical proofs name the hash of the originally admitted JSON encoding.
pub(super) fn read_verified_allocation_bytes(
    dir: &Path,
    hash: &[u8; 32],
) -> Result<Vec<u8>, Error> {
    let bytes = read_file(&dir.join(name(ALLOCATION_PREFIX, hash)), PROOF_LIMIT as u64)?;
    if super::digest(&bytes) != *hash
        || bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES
    {
        return Err(Error::Invalid("town_transport_allocation_hash_or_limit"));
    }
    // The caller decodes once through `validate_allocation`; keep this helper focused on the
    // bounded file/hash check so raw bytes and original digest are not normalized.
    Ok(bytes)
}

pub(super) fn write_immutable(
    dir: &Path,
    prefix: &str,
    bytes: &[u8],
    hash: &[u8; 32],
) -> Result<(), Error> {
    let path = dir.join(name(prefix, hash));
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(Error::Invalid("town_transport_asset_type"));
        }
        Ok(_) => {
            let existing = read_file(&path, PROOF_LIMIT as u64)?;
            if super::digest(&existing) != *hash || existing != bytes {
                return Err(Error::Invalid("town_transport_asset_conflict"));
            }
            return Ok(());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    atomic_file(dir, &name(prefix, hash), bytes, || Ok(()))
}

pub(super) fn admit_immutable_write(
    dir: &Path,
    policy: CapacityPolicy,
    prefix: &str,
    bytes: &[u8],
    hash: &[u8; 32],
    include_settlement_frame: bool,
) -> Result<(), Error> {
    let path = dir.join(name(prefix, hash));
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(Error::Invalid("town_transport_asset_type"));
        }
        Ok(_) => {
            let existing = read_file(&path, PROOF_LIMIT as u64)?;
            if super::digest(&existing) != *hash || existing != bytes {
                return Err(Error::Invalid("town_transport_asset_conflict"));
            }
            return Ok(());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let proof_publication = (bytes.len() as u64)
        .checked_mul(2)
        .ok_or(Error::Invalid("town_transport_capacity_overflow"))?;
    let settlement_frame = if include_settlement_frame {
        FRAME_LIMIT as u64 + 2 * META_LIMIT
    } else {
        0
    };
    let required = proof_publication
        .checked_add(settlement_frame)
        .ok_or(Error::Invalid("town_transport_capacity_overflow"))?;
    super::capacity::admit(dir, policy, required, false)
}

pub(super) fn decode_proof(
    bytes: &[u8],
    expected_hash: [u8; 32],
) -> Result<TransportProofAsset, Error> {
    if bytes.len() > PROOF_LIMIT
        || bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES
        || super::digest(bytes) != expected_hash
    {
        return Err(Error::Invalid("town_transport_proof_hash_or_limit"));
    }
    metadata::guard(bytes, 4096)?;
    Ok(serde_json::from_slice(bytes)?)
}

pub(super) fn read_proof(
    dir: &Path,
    metadata: &mut MetadataWorkspace,
    hash: [u8; 32],
) -> Result<TransportProofAsset, Error> {
    metadata
        .file
        .read(&dir.join(name(PROOF_PREFIX, &hash)), PROOF_LIMIT)?;
    decode_proof(metadata.file.as_slice(), hash)
}

pub(super) fn verify_proof_matches_allocation<'a>(
    engine: &Engine,
    proof: &'a TransportProofAsset,
    profile: &TransportAllocationProfile,
    allocation_hash: [u8; 32],
    proof_hash: [u8; 32],
    service_day: u64,
    generation: u64,
) -> Result<SettlementDeliveryInput<'a>, Error> {
    let (published_at, withdrawn_at) = road_effect_times(engine, profile)?;
    let (opening, assignment) = profile
        .derive_for_day(service_day, published_at, withdrawn_at)
        .map_err(|_| Error::Invalid("town_transport_road_epoch"))?;
    if proof.schema != 1
        || proof.world_id != profile.world_id
        || proof.branch_id != profile.branch_id
        || proof.profile_id != profile.market_profile_id
        || proof.allocation_profile_sha256 != allocation_hash
        || proof.service_day != service_day
        || proof.period_seconds != profile.period_seconds
        || proof.generation != generation
        || proof.opening != opening
        || proof.assignment != assignment
        || proof.partitions != profile.partitions
        || proof.fee_policy != profile.fee_policy
    {
        return Err(Error::Invalid("town_transport_proof_allocation_mismatch"));
    }
    Ok(SettlementDeliveryInput {
        proof_sha256: proof_hash,
        proof: proof.view(),
    })
}

impl Store {
    pub fn settle_town_market(&mut self, operation_id: u64) -> Result<Outcome, Error> {
        self.settle_town_market_with_observer(operation_id, |_| Ok(()))
    }

    /// Issue the next transport proof using the immutable profile version selected for the
    /// authoritative complete Timeline service day, then persist and settle it through the
    /// existing Engine writer.
    pub fn settle_town_market_with_observer(
        &mut self,
        operation_id: u64,
        mut observer: impl FnMut(WritePoint) -> io::Result<()>,
    ) -> Result<Outcome, Error> {
        if operation_id == 0 {
            return Err(Error::Invalid("town_transport_operation_id"));
        }
        // Journal-first retry: recover the exact recorded compact command without attempting
        // to issue tomorrow's proof or requiring an already-committed asset to be regenerated.
        if let Some(record) = self.receipt(operation_id)? {
            let view = self.engine()?.world().view();
            let envelope = Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id,
                expected_revision: record.expected_revision,
                actions: record.actions.into_iter().flatten().collect(),
            };
            return self.execute_observed_transport_diagnosed(
                Caller::System,
                envelope,
                None,
                &mut observer,
                &mut mycity_core::diagnostics::Trace::disabled(),
            );
        }
        self.engine()?;
        let active_profile_hash = self.current_transport_profile_hash()?;
        let view = self.engine.world().view();
        let market = view
            .town
            .ok_or(Error::Invalid("town_transport_market_missing"))?;
        let settled_day = market.state.settled_day;
        let last_transport = market.state.last_transport;
        let profile_id = market.profile.id;
        let world_id = view.scope.world();
        let branch_id = view.scope.branch();
        let revision = view.revision;
        let timeline = self
            .engine
            .timeline()
            .ok_or(Error::Invalid("town_transport_timeline_missing"))?;
        let at = timeline.progress.last_complete;
        if timeline.progress.phase.is_some()
            || timeline.progress.target.is_some()
            || at == 0
            || at % mycity_core::timeline::DAY != 0
        {
            return Err(Error::Invalid("town_transport_day_incomplete"));
        }
        let service_day = at / mycity_core::timeline::DAY;
        let catalog = self.spatial_transport_catalog()?;
        let allocation_hash = if let Some(catalog) = catalog.as_ref() {
            catalog
                .profile_sha256_for_service_day(service_day)
                .map_err(|_| Error::Invalid("town_transport_catalog_service_day"))?
        } else {
            active_profile_hash
        };
        if allocation_hash == [0; 32] {
            return Err(Error::Invalid("town_transport_profile_unbound"));
        }
        if settled_day.checked_add(1) != Some(service_day) {
            return Err(Error::Invalid("town_transport_day_not_next"));
        }
        let generation = last_transport
            .map_or(0, |receipt| receipt.consumed_generation)
            .checked_add(1)
            .ok_or(Error::Invalid("town_transport_generation_overflow"))?;
        let profile = read_allocation_for_settlement(
            &self.io.dir,
            &mut self.io.metadata,
            &allocation_hash,
            world_id,
            branch_id,
            profile_id,
        )?;
        let (published_at, withdrawn_at) = road_effect_times(&self.engine, &profile)?;
        let (opening, assignment) = profile
            .derive_for_day(service_day, published_at, withdrawn_at)
            .map_err(|_| Error::Invalid("town_transport_road_epoch"))?;
        let proof_view = mycity_core::market_access::TransportProofView {
            schema: 1,
            world_id,
            branch_id,
            profile_id,
            allocation_profile_sha256: allocation_hash,
            service_day,
            period_seconds: mycity_core::timeline::DAY as u32,
            generation,
            opening: &opening,
            assignment: &assignment,
            partitions: &profile.partitions,
            fee_policy: profile.fee_policy,
        };
        self.io.metadata.file.encode(&proof_view, PROOF_LIMIT)?;
        let proof_bytes = self.io.metadata.file.as_slice();
        if proof_bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES {
            return Err(Error::Invalid("town_transport_proof_schema_limit"));
        }
        let proof_sha256 = super::digest(proof_bytes);
        observer(WritePoint::TransportProofEncoded)?;
        admit_immutable_write(
            &self.io.dir,
            self.io.root.capacity,
            PROOF_PREFIX,
            proof_bytes,
            &proof_sha256,
            true,
        )?;
        write_immutable(&self.io.dir, PROOF_PREFIX, proof_bytes, &proof_sha256)?;
        observer(WritePoint::TransportProofPublished)?;
        let input = SettlementDeliveryInput {
            proof_sha256,
            proof: proof_view,
        };
        let envelope = Envelope {
            world: world_id,
            branch: branch_id,
            operation_id,
            expected_revision: revision,
            actions: vec![mycity_core::transaction::Action::TownTransportSettle {
                profile_id,
                service_day,
                generation,
                proof_sha256,
            }],
        };
        self.execute_observed_transport_diagnosed(
            Caller::System,
            envelope,
            Some(&input),
            &mut observer,
            &mut mycity_core::diagnostics::Trace::disabled(),
        )
    }
}

#[cfg(test)]
mod road_epoch_clock_tests {
    use super::*;
    use mycity_core::{
        assembly::assemble,
        construction::{Gate, GateCategory, GateDecision, Kind, Material, Project},
        market_access::{
            AllocationScope, DemandRepeatPolicy, FeePolicyInput, RoadCapacityEffectBinding,
            TransportAllocationProfile, TripPartition,
        },
        timeline::{Command as TimelineCommand, DAY},
        transaction::{Action, Caller, Engine, Envelope, Resource},
        transport::{Entrance, Node, OdDemand, Opening, RoadEdge},
        units::{GameTime, Money, Quantity},
    };
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    const FIXTURE: &[u8] = include_bytes!("../../../fixtures/core-town-utility-opening.json");
    const PROJECT_ID: u64 = 7;
    const ROAD_RESOURCE: u32 = 81;
    const SITE_RESOURCE: u32 = 80;
    const BASE_CAPACITY: u32 = 2;
    const ADDED_CAPACITY: u32 = 8;
    static TEMP_ID: AtomicU64 = AtomicU64::new(1);

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::var_os("MYCITY_TEST_ARTIFACT_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!("mycity-road-epoch-{}-{id}", std::process::id())))
        }
    }

    fn project() -> Project {
        let mut project = Project::new(
            PROJECT_ID,
            Kind::Build,
            7,
            4,
            1,
            SITE_RESOURCE,
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
                stock_id: 15,
                required: Quantity::new(500).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(1_000).unwrap(),
                basis_remainder: 0,
            }],
            GameTime::ZERO,
        )
        .unwrap();
        project.operating_resource = ROAD_RESOURCE;
        project.operating_base_capacity = BASE_CAPACITY;
        project.operating_units = ADDED_CAPACITY;
        project.operating_edge_id = 10;
        project
    }

    fn opening() -> Opening {
        Opening {
            revision: 1,
            nodes: vec![Node { id: 1, city_id: 1 }, Node { id: 2, city_id: 1 }],
            edges: vec![
                RoadEdge {
                    id: 10,
                    from: 1,
                    to: 2,
                    city_from: 1,
                    city_to: 1,
                    free_flow_seconds: 10,
                    capacity_people: BASE_CAPACITY,
                    mode_mask: 1,
                    open: true,
                    boundary_source: None,
                },
                RoadEdge {
                    id: 11,
                    from: 2,
                    to: 1,
                    city_from: 1,
                    city_to: 1,
                    free_flow_seconds: 10,
                    capacity_people: BASE_CAPACITY,
                    mode_mask: 1,
                    open: true,
                    boundary_source: None,
                },
            ],
            entrances: vec![
                Entrance {
                    id: 1,
                    owner: 5,
                    node: 1,
                    mode_mask: 1,
                    authorized: true,
                },
                Entrance {
                    id: 2,
                    owner: 7,
                    node: 2,
                    mode_mask: 1,
                    authorized: true,
                },
            ],
            demands: vec![
                OdDemand {
                    source_id: 101,
                    origin_entrance: 1,
                    destination_entrance: 2,
                    mode: 1,
                    people: 1,
                },
                OdDemand {
                    source_id: 102,
                    origin_entrance: 2,
                    destination_entrance: 1,
                    mode: 1,
                    people: 1,
                },
            ],
        }
    }

    fn allocation() -> TransportAllocationProfile {
        let opening = opening();
        TransportAllocationProfile {
            schema: 1,
            world_id: 1,
            branch_id: 1,
            market_profile_id: 1,
            period_seconds: DAY as u32,
            allocation_scope: AllocationScope::SingleTownExclusive,
            demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
            road_capacity_effect: Some(RoadCapacityEffectBinding {
                project_id: PROJECT_ID,
                resource_id: ROAD_RESOURCE,
                edge_id: 10,
                base_capacity_people: BASE_CAPACITY,
                added_capacity_people: ADDED_CAPACITY,
            }),
            assignment: opening.assign().unwrap(),
            opening,
            partitions: vec![TripPartition {
                quote_id: 1,
                outbound_source: 101,
                return_source: 102,
                trip_count: 1,
                goods_per_trip: Quantity::new(1).unwrap(),
            }],
            fee_policy: FeePolicyInput {
                provider_account: 1,
                cents_per_travel_second: 0,
            },
        }
    }

    fn resources(road_capacity: u32) -> Vec<Resource> {
        [
            (SITE_RESOURCE, 1),
            (ROAD_RESOURCE, road_capacity),
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2_000),
            (202, 2_000),
        ]
        .into_iter()
        .map(|(id, capacity)| Resource::new(id, capacity).unwrap())
        .collect()
    }

    fn opening_bytes(profile_hash: [u8; 32], project: &Project) -> Vec<u8> {
        let mut value: Value = serde_json::from_slice(FIXTURE).unwrap();
        value["town"]["transport_allocation_sha256"] = json!(profile_hash);
        value["construction"] = serde_json::to_value(project).unwrap();
        serde_json::to_vec(&value).unwrap()
    }

    fn engine(project: &Project, road_capacity: u32) -> Engine {
        let mut value: Value = serde_json::from_slice(FIXTURE).unwrap();
        value["construction"] = serde_json::to_value(project).unwrap();
        Engine::new(
            assemble(&serde_json::to_vec(&value).unwrap()).unwrap(),
            resources(road_capacity),
        )
        .unwrap()
    }

    fn enable_timeline(engine: &mut Engine) {
        engine
            .execute(
                Caller::System,
                Envelope {
                    world: 1,
                    branch: 1,
                    operation_id: 1,
                    expected_revision: 0,
                    actions: vec![Action::Timeline {
                        command: TimelineCommand::Enable,
                    }],
                },
            )
            .unwrap();
    }

    fn set_epoch_engine(
        at: i64,
        withdrawn: bool,
        last_complete: u64,
        settled_day: u64,
        enable: bool,
    ) -> Engine {
        let mut engine = engine(&project(), BASE_CAPACITY);
        if enable {
            enable_timeline(&mut engine);
        }
        let mut encoded = serde_json::to_value(engine).unwrap();
        let construction = &mut encoded["world"]["construction"];
        construction["kind"] = json!(if withdrawn { "demolish" } else { "build" });
        construction["phase"] = json!(if withdrawn {
            "demolition"
        } else {
            "operational"
        });
        construction["operating_published"] = json!(!withdrawn);
        construction["operating_published_at"] = json!(if withdrawn { 0 } else { at });
        construction["operating_withdrawn_at"] = if withdrawn { json!(at) } else { Value::Null };
        for resource in encoded["resources"].as_array_mut().unwrap() {
            if resource["id"] == json!(ROAD_RESOURCE) {
                resource["capacity"] = json!(if withdrawn {
                    BASE_CAPACITY
                } else {
                    BASE_CAPACITY + ADDED_CAPACITY
                });
            }
        }
        if enable {
            encoded["timeline"]["progress"]["at"] = json!(last_complete);
            encoded["timeline"]["progress"]["last_complete"] = json!(last_complete);
        }
        encoded["world"]["town"]["state"]["settled_day"] = json!(settled_day);
        serde_json::from_value(encoded).unwrap()
    }

    fn assert_epoch_rejected(engine: &Engine) {
        assert!(matches!(
            validate_allocation_project(engine, &allocation()),
            Err(Error::Invalid("town_transport_road_project_mismatch"))
        ));
    }

    #[test]
    fn store_accepts_unpublished_road_binding_before_timeline_enable() {
        let profile = allocation();
        let profile_bytes = serde_json::to_vec(&profile).unwrap();
        let profile_hash: [u8; 32] = Sha256::digest(&profile_bytes).into();
        let project = project();
        let opening = opening_bytes(profile_hash, &project);
        let temp = Temp::new();
        let store = crate::Store::create_spatial_with_transport_allocation_profile(
            &temp.0,
            &opening,
            resources(BASE_CAPACITY),
            "road-epoch-clock-test-v1",
            (
                include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
                include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
            ),
            &profile_bytes,
        )
        .unwrap();
        assert!(store.engine().unwrap().timeline().is_none());
        validate_allocation_project(store.engine().unwrap(), &profile).unwrap();
    }

    #[test]
    fn publication_and_withdrawal_epochs_require_clock_and_settlement() {
        let published_no_clock = set_epoch_engine(0, false, 0, 0, false);
        assert_epoch_rejected(&published_no_clock);
        let published_with_clock = set_epoch_engine(0, false, 0, 0, true);
        validate_allocation_project(&published_with_clock, &allocation()).unwrap();

        let withdrawn_no_clock = set_epoch_engine(0, true, 0, 0, false);
        assert_epoch_rejected(&withdrawn_no_clock);
        let withdrawn_with_clock = set_epoch_engine(0, true, 0, 0, true);
        validate_allocation_project(&withdrawn_with_clock, &allocation()).unwrap();

        let unsettled_epoch = set_epoch_engine(DAY as i64, false, DAY, 0, true);
        assert_epoch_rejected(&unsettled_epoch);
    }

    #[test]
    fn future_and_non_day_road_epochs_are_rejected() {
        let future = set_epoch_engine((2 * DAY) as i64, false, DAY, 1, true);
        assert_epoch_rejected(&future);
        let non_day = set_epoch_engine((DAY + 1) as i64, false, 2 * DAY, 2, true);
        assert_epoch_rejected(&non_day);
    }
}
