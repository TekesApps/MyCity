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
    ) -> Result<(), Error> {
        for reference in &self.0 {
            let profile = profile.ok_or(Error::Invalid("town_transport_profile_missing"))?;
            let allocation_hash =
                allocation_hash.ok_or(Error::Invalid("town_transport_profile_missing"))?;
            let proof = read_proof(dir, metadata, reference.proof_sha256)?;
            if reference.profile_id != profile.market_profile_id {
                return Err(Error::Invalid("town_transport_archive_profile"));
            }
            verify_proof_matches_allocation(
                engine,
                &proof,
                profile,
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
    let profile = profile.ok_or(Error::Invalid("town_transport_profile_missing"))?;
    let allocation_hash =
        allocation_hash.ok_or(Error::Invalid("town_transport_profile_missing"))?;
    if receipt.issued_day != market.state.settled_day
        || receipt.issued_generation != market.state.settled_day
        || receipt.consumed_generation != market.state.settled_day
        || receipt.issued_proof_sha256.iter().all(|byte| *byte == 0)
        || receipt.resolved_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(Error::Invalid("town_transport_checkpoint_cursor"));
    }
    let proof = read_proof(dir, metadata, receipt.issued_proof_sha256)?;
    verify_proof_matches_allocation(
        engine,
        &proof,
        profile,
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
            != if project.operating_withdrawn_at.is_some() {
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
    let last_complete = engine
        .timeline()
        .map(|timeline| timeline.progress.last_complete)
        .ok_or(Error::Invalid("town_transport_road_project_mismatch"))?;
    let settled_day = engine
        .world()
        .view()
        .town
        .map(|town| town.settled_day)
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

/// Runtime settlement reads the same genesis-hash-bound profile validated at Store open. The
/// Engine resolver performs the one required Assignment verification for the day's settlement.
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

    /// Issue the next fixed-profile transport proof for the authoritative complete timeline
    /// day, persist that exact immutable proof, then settle through the existing Engine writer.
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
        let view = self.engine.world().view();
        let market = view
            .town
            .ok_or(Error::Invalid("town_transport_market_missing"))?;
        let allocation_hash = market.profile.transport_allocation_sha256;
        if allocation_hash.iter().all(|byte| *byte == 0) {
            return Err(Error::Invalid("town_transport_profile_unbound"));
        }
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
        if market.state.settled_day.checked_add(1) != Some(service_day) {
            return Err(Error::Invalid("town_transport_day_not_next"));
        }
        let generation = market
            .state
            .last_transport
            .map_or(0, |receipt| receipt.consumed_generation)
            .checked_add(1)
            .ok_or(Error::Invalid("town_transport_generation_overflow"))?;
        let profile_id = market.profile.id;
        let world_id = view.scope.world();
        let branch_id = view.scope.branch();
        let revision = view.revision;
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
