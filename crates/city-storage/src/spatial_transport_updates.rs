//! Prepared immutable spatial/transport allocation revisions.
//!
//! This module prepares immutable profile/catalog assets for the Store writer. It can stage
//! bounded content-addressed files, but does not publish Root pointers, consume legal grants,
//! advance the calendar, or mutate an open Store. The unique writer must publish those changes
//! together at the existing transaction boundary before they can affect a settlement.
use mycity_core::market_access::TransportAllocationProfile;
use mycity_core::spatial_geometry_permissions::{
    GeometryRevisionDescriptor, SpatialGeometryAuthorizationWitness, VerifiedGeometryGrant,
};
use mycity_core::spatial_transport_permissions::{
    approval_evidence_sha256, request_sha256, SpatialAccessOperation, SpatialAccessSurface,
    SpatialAssetKind, SpatialAuthorizationContext, SpatialAuthorizationWitness,
    VerifiedSpatialUpdateGrant,
};
use mycity_core::transport::Opening;
use serde::{de, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path};

/// Schema 3 adds geometry-derived Scene/profile revisions alongside the schema-2 transport
/// authorization witness. Schema-2 records remain readable as the previous catalog chain.
pub const SPATIAL_TRANSPORT_VERSION_SCHEMA: u16 = 3;
const LEGACY_SPATIAL_TRANSPORT_VERSION_SCHEMA: u16 = 2;
pub const MAX_SPATIAL_TRANSPORT_VERSIONS: usize = 128;
pub const MAX_SPATIAL_TRANSPORT_ASSETS: usize = 8;
/// The complete catalog is an immutable sidecar, never Engine input or metadata payload.
/// At 128 compact rows this remains comfortably below the 2 MiB proof-file bound while the
/// independent row cap prevents a small forged file from causing unbounded Vec growth.
pub const SPATIAL_TRANSPORT_CATALOG_MAX_BYTES: usize = 128 * 1024;
pub const SPATIAL_TRANSPORT_CATALOG_PREFIX: &str = "spatial-transport-catalog-";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialTransportAssetBaseline {
    pub asset_id: u32,
    pub kind: SpatialAssetKind,
    pub revision: u64,
}

fn bounded_asset_baselines<'de, D>(
    deserializer: D,
) -> Result<Vec<SpatialTransportAssetBaseline>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Bounded;
    impl<'de> de::Visitor<'de> for Bounded {
        type Value = Vec<SpatialTransportAssetBaseline>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "at most {MAX_SPATIAL_TRANSPORT_ASSETS} spatial transport assets"
            )
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut rows = Vec::new();
            rows.try_reserve_exact(MAX_SPATIAL_TRANSPORT_ASSETS)
                .map_err(|_| de::Error::custom("spatial_transport_assets_allocation"))?;
            for _ in 0..MAX_SPATIAL_TRANSPORT_ASSETS {
                match seq.next_element()? {
                    Some(row) => rows.push(row),
                    None => return Ok(rows),
                }
            }
            struct Reject;
            impl<'de> de::DeserializeSeed<'de> for Reject {
                type Value = ();
                fn deserialize<D2: Deserializer<'de>>(self, _: D2) -> Result<(), D2::Error> {
                    Err(de::Error::custom("spatial_transport_assets_limit"))
                }
            }
            seq.next_element_seed(Reject)?;
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(Bounded)
}

fn bounded_version_rows<'de, D>(
    deserializer: D,
) -> Result<Vec<SpatialTransportVersionRecord>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Bounded;
    impl<'de> de::Visitor<'de> for Bounded {
        type Value = Vec<SpatialTransportVersionRecord>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "at most {MAX_SPATIAL_TRANSPORT_VERSIONS} spatial transport versions"
            )
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut rows = Vec::new();
            rows.try_reserve_exact(MAX_SPATIAL_TRANSPORT_VERSIONS)
                .map_err(|_| de::Error::custom("spatial_transport_catalog_allocation"))?;
            for _ in 0..MAX_SPATIAL_TRANSPORT_VERSIONS {
                match seq.next_element()? {
                    Some(row) => rows.push(row),
                    None => return Ok(rows),
                }
            }
            struct Reject;
            impl<'de> de::DeserializeSeed<'de> for Reject {
                type Value = ();
                fn deserialize<D2: Deserializer<'de>>(self, _: D2) -> Result<(), D2::Error> {
                    Err(de::Error::custom("spatial_transport_catalog_limit"))
                }
            }
            seq.next_element_seed(Reject)?;
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(Bounded)
}

pub struct VerifiedAllocationAsset {
    profile: TransportAllocationProfile,
    asset_sha256: [u8; 32],
}

impl VerifiedAllocationAsset {
    /// Validate exact raw asset bytes with the Store's existing bounded allocation decoder.
    pub fn from_raw_bytes(
        bytes: &[u8],
        expected_sha256: [u8; 32],
        world_id: u64,
        branch_id: u64,
        profile_id: u32,
    ) -> Result<Self, UpdateError> {
        let profile = super::town_transport::validate_allocation(
            bytes,
            world_id,
            branch_id,
            profile_id,
            expected_sha256,
        )
        .map_err(|_| UpdateError::InvalidSourceAsset)?;
        Ok(Self {
            profile,
            asset_sha256: expected_sha256,
        })
    }

    /// Load a content-addressed historical allocation through the existing bounded Store path.
    pub fn read_immutable(
        dir: &Path,
        expected_sha256: [u8; 32],
        world_id: u64,
        branch_id: u64,
        profile_id: u32,
    ) -> Result<Self, UpdateError> {
        let bytes = super::town_transport::read_verified_allocation_bytes(dir, &expected_sha256)
            .map_err(|_| UpdateError::AssetIo)?;
        Self::from_raw_bytes(&bytes, expected_sha256, world_id, branch_id, profile_id)
    }

    pub fn profile(&self) -> &TransportAllocationProfile {
        &self.profile
    }

    pub fn asset_sha256(&self) -> [u8; 32] {
        self.asset_sha256
    }

    pub fn as_ref(&self) -> VerifiedAllocationRef<'_> {
        VerifiedAllocationRef::from_verified_store(&self.profile, self.asset_sha256)
    }
}

/// A decoded allocation whose exact immutable asset bytes were checked by Store.
/// The hash is opaque here because the original valid JSON asset need not equal this crate's
/// later serialization of the decoded profile.
pub struct VerifiedAllocationRef<'a> {
    profile: &'a TransportAllocationProfile,
    asset_sha256: [u8; 32],
}

impl<'a> VerifiedAllocationRef<'a> {
    /// Construct only after Store has bounded-read the asset and verified its raw-byte digest.
    pub(crate) fn from_verified_store(
        profile: &'a TransportAllocationProfile,
        asset_sha256: [u8; 32],
    ) -> Self {
        Self {
            profile,
            asset_sha256,
        }
    }

    pub fn profile(&self) -> &'a TransportAllocationProfile {
        self.profile
    }

    pub fn asset_sha256(&self) -> [u8; 32] {
        self.asset_sha256
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialTransportVersion {
    pub schema: u16,
    pub request_id: u64,
    pub asset_id: u32,
    pub asset_kind: SpatialAssetKind,
    pub asset_revision: u64,
    pub world_id: u64,
    pub branch_id: u64,
    pub surface: SpatialAccessSurface,
    pub edge_id: u32,
    pub version: u64,
    /// First affected service day; update acceptance is only valid after the prior day settled.
    pub effective_service_day: u64,
    pub source_scene_sha256: [u8; 32],
    /// Digest of the Store-verified legal/administrative authorization record.
    pub authorization_evidence_sha256: [u8; 32],
    pub previous_profile_sha256: [u8; 32],
    pub profile_sha256: [u8; 32],
    /// Immutable allocation snapshot to be published as a content-addressed Store asset.
    pub profile: TransportAllocationProfile,
}

/// Compact append-only root record. Profile bytes remain in the existing content-addressed
/// allocation files rather than being duplicated in every root generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialTransportVersionRecord {
    pub schema: u16,
    pub request_id: u64,
    pub version: u64,
    pub effective_service_day: u64,
    pub engine_operation_id: u64,
    pub receipt_operation_id: u64,
    pub receipt_revision: u64,
    pub store_sequence: u64,
    pub changed_asset_id: u32,
    pub asset_kind: SpatialAssetKind,
    pub asset_revision: u64,
    pub source_scene_sha256: [u8; 32],
    pub authorization_evidence_sha256: [u8; 32],
    /// Original request and both accepted approvals; the digest is not a substitute for evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_witness: Option<SpatialAuthorizationWitness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_descriptor: Option<GeometryRevisionDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_witness: Option<SpatialGeometryAuthorizationWitness>,
    pub previous_profile_sha256: [u8; 32],
    pub profile_sha256: [u8; 32],
    pub surface: SpatialAccessSurface,
    pub edge_id: u32,
}

/// Bound to the successful prepared Engine receipt and the Store root publication sequence.
/// The unique writer supplies these values from its actual commit context, never from a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialTransportCommitBinding {
    pub engine_operation_id: u64,
    pub receipt_operation_id: u64,
    pub receipt_revision: u64,
    pub store_sequence: u64,
}

impl SpatialTransportCommitBinding {
    pub fn validate(self) -> Result<(), UpdateError> {
        if self.engine_operation_id == 0
            || self.receipt_operation_id == 0
            || self.receipt_revision == 0
            || self.store_sequence == 0
            || self.engine_operation_id != self.receipt_operation_id
        {
            return Err(UpdateError::InvalidCommitBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialTransportCatalog {
    pub schema: u16,
    pub world_id: u64,
    pub branch_id: u64,
    pub profile_id: u32,
    pub source_scene_sha256: [u8; 32],
    /// Exact originally admitted raw JSON hash; never regenerate it from decoded fields.
    pub baseline_profile_sha256: [u8; 32],
    /// Exact content-addressed parent sidecar hash; zero only for the genesis catalog.
    pub parent_catalog_sha256: [u8; 32],
    #[serde(deserialize_with = "bounded_asset_baselines")]
    assets: Vec<SpatialTransportAssetBaseline>,
    #[serde(deserialize_with = "bounded_version_rows")]
    versions: Vec<SpatialTransportVersionRecord>,
}

/// Exact bytes to stage as an immutable catalog sidecar. The digest is over these bytes, not a
/// reconstructed root record; the Store root only needs to retain this small digest pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedSpatialTransportCatalog {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
}

/// Validate and encode one bounded catalog generation. The old root's hash remains a complete
/// recovery reference to the parent generation; callers must never prune it while retained roots
/// or proofs can refer to it.
pub fn encode_spatial_transport_catalog(
    catalog: &SpatialTransportCatalog,
) -> Result<EncodedSpatialTransportCatalog, UpdateError> {
    catalog.validate()?;
    let bytes = serde_json::to_vec(catalog).map_err(|_| UpdateError::InvalidVersionChain)?;
    if bytes.is_empty() || bytes.len() > SPATIAL_TRANSPORT_CATALOG_MAX_BYTES {
        return Err(UpdateError::CatalogLimit);
    }
    let sha256 = Sha256::digest(&bytes).into();
    Ok(EncodedSpatialTransportCatalog { bytes, sha256 })
}

/// Decode an exact content-addressed catalog generation with byte, lexical and row bounds.
/// Deserialization is followed by fixed-capacity normalization before validation so owned
/// runtime capacity remains deterministic after cold reopen.
pub fn decode_spatial_transport_catalog(
    bytes: &[u8],
    expected_sha256: [u8; 32],
) -> Result<SpatialTransportCatalog, UpdateError> {
    let actual_sha256: [u8; 32] = Sha256::digest(bytes).into();
    if bytes.is_empty()
        || bytes.len() > SPATIAL_TRANSPORT_CATALOG_MAX_BYTES
        || actual_sha256 != expected_sha256
    {
        return Err(UpdateError::InvalidCatalogAsset);
    }
    super::metadata::guard(bytes, 4096).map_err(|_| UpdateError::InvalidCatalogAsset)?;
    let mut catalog: SpatialTransportCatalog =
        serde_json::from_slice(bytes).map_err(|_| UpdateError::InvalidCatalogAsset)?;
    catalog.reserve_rows()?;
    catalog.validate()?;
    Ok(catalog)
}

/// Read the exact immutable catalog named by its digest. The filename is derived only from the
/// checked digest and existing Store `read_file` rejects symlinks/non-files and limits bytes.
pub fn read_spatial_transport_catalog(
    dir: &Path,
    expected_sha256: [u8; 32],
) -> Result<SpatialTransportCatalog, UpdateError> {
    let path = dir.join(format!(
        "{}{}.json",
        SPATIAL_TRANSPORT_CATALOG_PREFIX,
        super::hex(&expected_sha256)
    ));
    let bytes = super::read_file(&path, SPATIAL_TRANSPORT_CATALOG_MAX_BYTES as u64)
        .map_err(|_| UpdateError::AssetIo)?;
    decode_spatial_transport_catalog(&bytes, expected_sha256)
}

/// Stage the exact encoded catalog as an immutable, content-addressed sidecar. The existing
/// Store admission check reserves the immutable publication's disk budget. A failed later root
/// commit leaves an unreferenced blob only; publishing the digest pointer and Engine candidate
/// together is the responsibility of the single Store writer.
pub fn stage_spatial_transport_catalog(
    dir: &Path,
    capacity: super::CapacityPolicy,
    encoded: &EncodedSpatialTransportCatalog,
) -> Result<(), UpdateError> {
    let catalog = decode_spatial_transport_catalog(&encoded.bytes, encoded.sha256)?;
    if encode_spatial_transport_catalog(&catalog)? != *encoded {
        return Err(UpdateError::InvalidCatalogAsset);
    }
    super::town_transport::admit_immutable_write(
        dir,
        capacity,
        SPATIAL_TRANSPORT_CATALOG_PREFIX,
        &encoded.bytes,
        &encoded.sha256,
        false,
    )
    .map_err(|_| UpdateError::AssetIo)?;
    super::town_transport::write_immutable(
        dir,
        SPATIAL_TRANSPORT_CATALOG_PREFIX,
        &encoded.bytes,
        &encoded.sha256,
    )
    .map_err(|_| UpdateError::AssetIo)
}

impl SpatialTransportCatalog {
    pub fn new(
        world_id: u64,
        branch_id: u64,
        profile_id: u32,
        source_scene_sha256: [u8; 32],
        baseline_profile_sha256: [u8; 32],
        assets: &[SpatialTransportAssetBaseline],
    ) -> Result<Self, UpdateError> {
        if world_id == 0
            || branch_id == 0
            || profile_id == 0
            || source_scene_sha256 == [0; 32]
            || baseline_profile_sha256 == [0; 32]
            || assets.is_empty()
            || assets.len() > MAX_SPATIAL_TRANSPORT_ASSETS
        {
            return Err(UpdateError::InvalidIdentity);
        }
        let mut assets = assets.to_vec();
        assets.sort_by_key(|asset| asset.asset_id);
        if assets.iter().enumerate().any(|(index, asset)| {
            asset.asset_id == 0
                || asset.revision == 0
                || assets[..index]
                    .iter()
                    .any(|previous| previous.asset_id == asset.asset_id)
        }) {
            return Err(UpdateError::InvalidIdentity);
        }
        assets
            .try_reserve_exact(MAX_SPATIAL_TRANSPORT_ASSETS.saturating_sub(assets.len()))
            .map_err(|_| UpdateError::Capacity)?;
        if assets.capacity() != MAX_SPATIAL_TRANSPORT_ASSETS {
            return Err(UpdateError::Capacity);
        }
        let mut versions = Vec::new();
        versions
            .try_reserve_exact(MAX_SPATIAL_TRANSPORT_VERSIONS)
            .map_err(|_| UpdateError::Capacity)?;
        if versions.capacity() != MAX_SPATIAL_TRANSPORT_VERSIONS {
            return Err(UpdateError::Capacity);
        }
        Ok(Self {
            schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
            world_id,
            branch_id,
            profile_id,
            source_scene_sha256,
            baseline_profile_sha256,
            parent_catalog_sha256: [0; 32],
            assets,
            versions,
        })
    }

    pub fn versions(&self) -> &[SpatialTransportVersionRecord] {
        &self.versions
    }

    pub fn assets(&self) -> &[SpatialTransportAssetBaseline] {
        &self.assets
    }

    fn reserve_rows(&mut self) -> Result<(), UpdateError> {
        self.versions
            .try_reserve_exact(MAX_SPATIAL_TRANSPORT_VERSIONS.saturating_sub(self.versions.len()))
            .map_err(|_| UpdateError::Capacity)?;
        if self.versions.capacity() != MAX_SPATIAL_TRANSPORT_VERSIONS {
            return Err(UpdateError::Capacity);
        }
        Ok(())
    }

    fn reserve_asset_rows(&mut self) -> Result<(), UpdateError> {
        self.assets
            .try_reserve_exact(MAX_SPATIAL_TRANSPORT_ASSETS.saturating_sub(self.assets.len()))
            .map_err(|_| UpdateError::Capacity)?;
        if self.assets.capacity() != MAX_SPATIAL_TRANSPORT_ASSETS {
            return Err(UpdateError::Capacity);
        }
        Ok(())
    }

    fn asset_revision(&self, asset_id: u32) -> Option<(SpatialAssetKind, u64)> {
        self.assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .map(|asset| (asset.kind, asset.revision))
    }

    fn current_asset_revision(&self, asset_id: u32) -> Option<(SpatialAssetKind, u64)> {
        let (kind, baseline) = self.asset_revision(asset_id)?;
        let updates = self
            .versions
            .iter()
            .filter(|record| record.changed_asset_id == asset_id)
            .count();
        Some((kind, baseline.checked_add(updates as u64)?))
    }

    pub fn owned_capacity_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.assets.capacity() * std::mem::size_of::<SpatialTransportAssetBaseline>()
            + self.versions.capacity() * std::mem::size_of::<SpatialTransportVersionRecord>()
    }

    pub fn validate(&self) -> Result<(), UpdateError> {
        self.validate_with_context(SpatialAuthorizationContext {
            world_id: self.world_id,
            branch_id: self.branch_id,
            source_scene_sha256: self.source_scene_sha256,
        })
    }

    /// Validate authorization witnesses against a caller-supplied trusted context derived from
    /// immutable genesis state. The catalog's own identity fields are not an authority source.
    pub fn validate_with_context(
        &self,
        context: SpatialAuthorizationContext,
    ) -> Result<(), UpdateError> {
        if context.world_id != self.world_id
            || context.branch_id != self.branch_id
            || context.source_scene_sha256 != self.source_scene_sha256
        {
            return Err(UpdateError::InvalidVersionChain);
        }
        if !matches!(
            self.schema,
            LEGACY_SPATIAL_TRANSPORT_VERSION_SCHEMA | SPATIAL_TRANSPORT_VERSION_SCHEMA
        ) || self.world_id == 0
            || self.branch_id == 0
            || self.profile_id == 0
            || self.source_scene_sha256 == [0; 32]
            || self.baseline_profile_sha256 == [0; 32]
            || (self.versions.is_empty() && self.parent_catalog_sha256 != [0; 32])
            || (!self.versions.is_empty() && self.parent_catalog_sha256 == [0; 32])
            || self.assets.is_empty()
            || self.assets.len() > MAX_SPATIAL_TRANSPORT_ASSETS
            || self.assets.capacity() != MAX_SPATIAL_TRANSPORT_ASSETS
            || self.versions.len() > MAX_SPATIAL_TRANSPORT_VERSIONS
            || self.versions.capacity() != MAX_SPATIAL_TRANSPORT_VERSIONS
        {
            return Err(UpdateError::InvalidVersionChain);
        }
        for (index, asset) in self.assets.iter().enumerate() {
            if asset.asset_id == 0
                || asset.revision == 0
                || self.assets[..index]
                    .iter()
                    .any(|prior| prior.asset_id >= asset.asset_id)
            {
                return Err(UpdateError::InvalidVersionChain);
            }
        }
        let mut parent = self.baseline_profile_sha256;
        let mut day = 0;
        let mut receipt_revision = 0;
        let mut store_sequence = 0;
        for (index, record) in self.versions.iter().enumerate() {
            let is_geometry =
                record.geometry_descriptor.is_some() || record.geometry_witness.is_some();
            if !matches!(
                record.schema,
                LEGACY_SPATIAL_TRANSPORT_VERSION_SCHEMA | SPATIAL_TRANSPORT_VERSION_SCHEMA
            ) || record.request_id == 0
                || record.engine_operation_id == 0
                || record.changed_asset_id == 0
                || record.asset_revision == 0
                || self.asset_revision(record.changed_asset_id).is_none()
                || self.asset_revision(record.changed_asset_id).unwrap().0 != record.asset_kind
                || record.asset_revision
                    != self
                        .asset_revision(record.changed_asset_id)
                        .and_then(|(_, baseline)| {
                            baseline
                                .checked_add(
                                    self.versions[..index]
                                        .iter()
                                        .filter(|prior| {
                                            prior.changed_asset_id == record.changed_asset_id
                                        })
                                        .count() as u64,
                                )
                                .and_then(|revision| revision.checked_add(1))
                        })
                        .unwrap_or(0)
                || record.receipt_operation_id != record.engine_operation_id
                || record.receipt_revision == 0
                || record.store_sequence == 0
                || record.receipt_revision <= receipt_revision
                || record.store_sequence <= store_sequence
                || record.version != (index as u64).saturating_add(1)
                || record.effective_service_day <= day
                || record.source_scene_sha256 != self.source_scene_sha256
                || record.authorization_evidence_sha256 == [0; 32]
                || if is_geometry {
                    record.authorization_witness.is_some()
                        || record.geometry_descriptor.is_none()
                        || record.geometry_witness.is_none()
                        || !record_geometry_witness_matches(record)
                } else {
                    record.authorization_witness.is_none()
                        || record.geometry_descriptor.is_some()
                        || record.geometry_witness.is_some()
                        || !record_authorization_witness_matches(record, context)
                }
                || record.previous_profile_sha256 != parent
                || record.profile_sha256 == [0; 32]
                || record.profile_sha256 == parent
                || (!is_geometry && record.edge_id == 0)
                || self.versions[..index].iter().any(|prior| {
                    prior.engine_operation_id == record.engine_operation_id
                        || prior.request_id == record.request_id
                            && (prior.geometry_descriptor.is_some() == is_geometry)
                })
            {
                return Err(UpdateError::InvalidVersionChain);
            }
            parent = record.profile_sha256;
            day = record.effective_service_day;
            receipt_revision = record.receipt_revision;
            store_sequence = record.store_sequence;
        }
        Ok(())
    }

    /// Append exactly one grant-bound candidate. A Store writer must stage the returned bytes
    /// and publish this new catalog with consumed permission state in the same commit.
    pub fn append_authorized(
        &mut self,
        baseline_ref: &VerifiedAllocationRef<'_>,
        current_ref: &VerifiedAllocationRef<'_>,
        version: &SpatialTransportVersion,
        grant: &VerifiedSpatialUpdateGrant,
        commit: SpatialTransportCommitBinding,
        parent_catalog_sha256: [u8; 32],
    ) -> Result<(), UpdateError> {
        self.append_authorized_with_structure(
            baseline_ref, baseline_ref, current_ref, version, grant, commit,
            parent_catalog_sha256,
        )
    }

    /// Select only a witnessed geometry revision in this already validated prefix. This
    /// selects an immutable asset; it does not authenticate a Scene or confer authority.
    pub(crate) fn structural_anchor_sha256(&self) -> Result<[u8; 32], UpdateError> {
        self.validate()?;
        Ok(self.versions.iter().rev().find(|row| {
            row.geometry_descriptor.is_some() && row.geometry_witness.is_some()
        }).map_or(self.baseline_profile_sha256, |row| row.profile_sha256))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_access_version(
        &self,
        baseline_ref: &VerifiedAllocationRef<'_>,
        structural_ref: &VerifiedAllocationRef<'_>,
        current_ref: &VerifiedAllocationRef<'_>,
        grant: &VerifiedSpatialUpdateGrant,
        version: u64,
        proposed_opening: Opening,
    ) -> Result<SpatialTransportVersion, UpdateError> {
        let parent = self.versions.last()
            .map_or(self.baseline_profile_sha256, |row| row.profile_sha256);
        if baseline_ref.asset_sha256 != self.baseline_profile_sha256
            || structural_ref.asset_sha256 != self.structural_anchor_sha256()?
            || current_ref.asset_sha256 != parent
            || version != self.versions.len() as u64 + 1
            || grant.source_scene_sha256() != self.source_scene_sha256
        {
            return Err(UpdateError::InvalidVersionChain);
        }
        prepare_version_with_structure(
            baseline_ref, structural_ref, current_ref, grant, version, proposed_opening,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn append_authorized_with_structure(
        &mut self,
        baseline_ref: &VerifiedAllocationRef<'_>,
        structural_ref: &VerifiedAllocationRef<'_>,
        current_ref: &VerifiedAllocationRef<'_>,
        version: &SpatialTransportVersion,
        grant: &VerifiedSpatialUpdateGrant,
        commit: SpatialTransportCommitBinding,
        parent_catalog_sha256: [u8; 32],
    ) -> Result<(), UpdateError> {
        self.validate()?;
        let mut next = self.clone();
        next.reserve_asset_rows()?;
        next.reserve_rows()?;
        if parent_catalog_sha256 == [0; 32] {
            return Err(UpdateError::InvalidVersionChain);
        }
        next.append_in_place(baseline_ref, structural_ref, current_ref, version, grant, commit)?;
        next.parent_catalog_sha256 = parent_catalog_sha256;
        next.schema = SPATIAL_TRANSPORT_VERSION_SCHEMA;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Append a Store-validated geometry-derived profile revision. Its authorization remains in
    /// the independent geometry domain; it is never represented as a road Close/Restore grant.
    pub fn append_geometry_authorized(
        &mut self,
        current_ref: &VerifiedAllocationRef<'_>,
        candidate_ref: &VerifiedAllocationRef<'_>,
        descriptor: GeometryRevisionDescriptor,
        grant: VerifiedGeometryGrant,
        commit: SpatialTransportCommitBinding,
        parent_catalog_sha256: [u8; 32],
    ) -> Result<(), UpdateError> {
        self.validate()?;
        commit.validate()?;
        descriptor
            .validate()
            .map_err(|_| UpdateError::InvalidAuthorizedCandidate)?;
        let mut next = self.clone();
        next.reserve_asset_rows()?;
        next.reserve_rows()?;
        next.schema = SPATIAL_TRANSPORT_VERSION_SCHEMA;
        if parent_catalog_sha256 == [0; 32] {
            return Err(UpdateError::InvalidVersionChain);
        }
        next.parent_catalog_sha256 = parent_catalog_sha256;
        let request = grant.request();
        let previous_profile = self
            .versions
            .last()
            .map_or(self.baseline_profile_sha256, |record| record.profile_sha256);
        let previous_day = self
            .versions
            .last()
            .map_or(0, |record| record.effective_service_day);
        let previous_geometry_revision = self
            .current_asset_revision(request.asset_id)
            .ok_or(UpdateError::InvalidAuthorizedCandidate)?;
        let descriptor_sha256 = descriptor
            .sha256()
            .map_err(|_| UpdateError::InvalidAuthorizedCandidate)?;
        if self.versions.len() == MAX_SPATIAL_TRANSPORT_VERSIONS
            || current_ref.asset_sha256 != previous_profile
            || current_ref.profile.world_id != self.world_id
            || current_ref.profile.branch_id != self.branch_id
            || current_ref.profile.market_profile_id != self.profile_id
            || candidate_ref.profile.world_id != self.world_id
            || candidate_ref.profile.branch_id != self.branch_id
            || candidate_ref.profile.market_profile_id != self.profile_id
            || candidate_ref.asset_sha256 != descriptor.candidate_profile_sha256
            || request.descriptor_sha256 != descriptor_sha256
            || request.asset_id != descriptor.asset_id
            || request.expected_geometry_revision != descriptor.parent_geometry_revision
            || request.expected_asset_revision != grant.expected_asset_revision()
            || request.expected_asset_revision != previous_geometry_revision.1
            || previous_geometry_revision.0 != SpatialAssetKind::Road
            || descriptor.world_id != self.world_id
            || descriptor.branch_id != self.branch_id
            || descriptor.parent_transport_revision != current_ref.profile.opening.revision
            || descriptor.candidate_transport_revision != candidate_ref.profile.opening.revision
            || descriptor.candidate_transport_revision
                != descriptor.parent_transport_revision.saturating_add(1)
            || descriptor.parent_profile_sha256 != current_ref.asset_sha256
            || descriptor.candidate_profile_sha256 != candidate_ref.asset_sha256
            || descriptor.effective_service_day <= previous_day
            || descriptor.effective_service_day == 0
            || descriptor.adapter_basis_sha256 == [0; 32]
            || grant.world_id() != self.world_id
            || grant.branch_id() != self.branch_id
            || grant.asset_id() != request.asset_id
            || grant.parent_profile_sha256() != current_ref.asset_sha256
            || grant.candidate_profile_sha256() != candidate_ref.asset_sha256
            || grant.descriptor_sha256() != descriptor_sha256
            || commit.receipt_operation_id != commit.engine_operation_id
        {
            return Err(UpdateError::InvalidAuthorizedCandidate);
        }
        let source_scene = self.source_scene_sha256;
        let record = SpatialTransportVersionRecord {
            schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
            request_id: request.request_id,
            version: self.versions.len() as u64 + 1,
            effective_service_day: descriptor.effective_service_day,
            engine_operation_id: commit.engine_operation_id,
            receipt_operation_id: commit.receipt_operation_id,
            receipt_revision: commit.receipt_revision,
            store_sequence: commit.store_sequence,
            changed_asset_id: request.asset_id,
            asset_kind: SpatialAssetKind::Road,
            asset_revision: request
                .expected_asset_revision
                .checked_add(1)
                .ok_or(UpdateError::InvalidAuthorizedCandidate)?,
            source_scene_sha256: source_scene,
            authorization_evidence_sha256: descriptor_sha256,
            authorization_witness: None,
            geometry_descriptor: Some(descriptor),
            geometry_witness: Some(grant.witness()),
            previous_profile_sha256: current_ref.asset_sha256,
            profile_sha256: candidate_ref.asset_sha256,
            surface: SpatialAccessSurface::Road,
            edge_id: 0,
        };
        next.versions.push(record);
        next.validate()?;
        *self = next;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn append_in_place(
        &mut self,
        baseline_ref: &VerifiedAllocationRef<'_>,
        structural_ref: &VerifiedAllocationRef<'_>,
        current_ref: &VerifiedAllocationRef<'_>,
        version: &SpatialTransportVersion,
        grant: &VerifiedSpatialUpdateGrant,
        commit: SpatialTransportCommitBinding,
    ) -> Result<(), UpdateError> {
        commit.validate()?;
        let expected_parent = self
            .versions
            .last()
            .map_or(self.baseline_profile_sha256, |record| record.profile_sha256);
        if baseline_ref.asset_sha256 != self.baseline_profile_sha256
            || current_ref.asset_sha256 != expected_parent
            || baseline_ref.profile.world_id != self.world_id
            || baseline_ref.profile.branch_id != self.branch_id
            || baseline_ref.profile.market_profile_id != self.profile_id
            || current_ref.profile.world_id != self.world_id
            || current_ref.profile.branch_id != self.branch_id
            || current_ref.profile.market_profile_id != self.profile_id
        {
            return Err(UpdateError::InvalidVersionChain);
        }
        let recomputed = self.prepare_access_version(
            baseline_ref,
            structural_ref,
            current_ref,
            grant,
            version.version,
            version.profile.opening.clone(),
        )?;
        if recomputed != *version {
            return Err(UpdateError::InvalidAuthorizedCandidate);
        }
        if self.versions.len() == MAX_SPATIAL_TRANSPORT_VERSIONS {
            return Err(UpdateError::Capacity);
        }
        if version.schema != SPATIAL_TRANSPORT_VERSION_SCHEMA
            || self.asset_revision(version.asset_id).is_none()
            || version.asset_kind != grant.asset_kind()
            || version.asset_revision != grant.new_revision()
            || version.world_id != self.world_id
            || version.branch_id != self.branch_id
            || version.profile.world_id != self.world_id
            || version.profile.branch_id != self.branch_id
            || version.profile.market_profile_id != self.profile_id
            || version.source_scene_sha256 != self.source_scene_sha256
            || version.version != self.versions.len() as u64 + 1
            || version.request_id != grant.request_id()
            || version.effective_service_day != grant.effective_service_day()
            || version.previous_profile_sha256 != grant.parent_profile_sha256()
            || version.profile_sha256 != grant.candidate_profile_sha256()
            || version.authorization_evidence_sha256 != grant.evidence_sha256()
            || version.surface != grant.surface()
            || version.edge_id != grant.edge_id()
            || grant.asset_id() != version.asset_id
            || grant.world_id() != self.world_id
            || grant.branch_id() != self.branch_id
            || grant.source_scene_sha256() != self.source_scene_sha256
        {
            return Err(UpdateError::InvalidAuthorizedCandidate);
        }
        let expected_parent = self
            .versions
            .last()
            .map_or(self.baseline_profile_sha256, |record| record.profile_sha256);
        let previous_day = self
            .versions
            .last()
            .map_or(0, |record| record.effective_service_day);
        let (asset_kind, previous_asset_revision) =
            self.current_asset_revision(version.asset_id)
                .ok_or(UpdateError::InvalidAuthorizedCandidate)?;
        if version.previous_profile_sha256 != expected_parent
            || version.effective_service_day <= previous_day
            || version.asset_kind != asset_kind
            || previous_asset_revision.checked_add(1) != Some(version.asset_revision)
        {
            return Err(UpdateError::InvalidVersionChain);
        }
        let record = SpatialTransportVersionRecord {
            schema: version.schema,
            request_id: version.request_id,
            version: version.version,
            effective_service_day: version.effective_service_day,
            engine_operation_id: commit.engine_operation_id,
            receipt_operation_id: commit.receipt_operation_id,
            receipt_revision: commit.receipt_revision,
            store_sequence: commit.store_sequence,
            changed_asset_id: version.asset_id,
            asset_kind: version.asset_kind,
            asset_revision: version.asset_revision,
            source_scene_sha256: version.source_scene_sha256,
            authorization_evidence_sha256: version.authorization_evidence_sha256,
            authorization_witness: Some(grant.witness()),
            geometry_descriptor: None,
            geometry_witness: None,
            previous_profile_sha256: version.previous_profile_sha256,
            profile_sha256: version.profile_sha256,
            surface: version.surface,
            edge_id: version.edge_id,
        };
        self.versions.push(record);
        Ok(())
    }

    pub fn profile_sha256_for_service_day(
        &self,
        service_day: u64,
    ) -> Result<[u8; 32], UpdateError> {
        self.validate()?;
        Ok(self
            .versions
            .iter()
            .take_while(|record| record.effective_service_day <= service_day)
            .last()
            .map_or(self.baseline_profile_sha256, |record| record.profile_sha256))
    }
}

fn record_authorization_witness_matches(
    record: &SpatialTransportVersionRecord,
    context: SpatialAuthorizationContext,
) -> bool {
    let Some(witness) = record.authorization_witness else {
        return false;
    };
    let request = witness.request;
    let Ok(request_digest) = request_sha256(context, &request) else {
        return false;
    };
    let effective_day = u64::try_from(request.effective_time.value())
        .ok()
        .filter(|seconds| *seconds > 0 && *seconds % mycity_core::timeline::DAY == 0)
        .map(|seconds| seconds / mycity_core::timeline::DAY);
    request.request_id == record.request_id
        && request.asset_id == record.changed_asset_id
        && request.expected_revision.checked_add(1) == Some(record.asset_revision)
        && context.source_scene_sha256 == record.source_scene_sha256
        && request.parent_profile_sha256 == record.previous_profile_sha256
        && request.candidate_profile_sha256 == record.profile_sha256
        && request.surface == record.surface
        && request.edge_id == record.edge_id
        && effective_day == Some(record.effective_service_day)
        && request.applicant_account != 0
        && request.executor_account != 0
        && request.applicant_account != request.executor_account
        && witness.owner_approval.request_id == request.request_id
        && witness.owner_approval.request_sha256 == request_digest
        && witness.owner_approval.role
            == mycity_core::spatial_transport_permissions::SpatialApprovalRole::OwnerConsent
        && witness.owner_approval.approved
        && witness.authority_approval.request_id == request.request_id
        && witness.authority_approval.request_sha256 == request_digest
        && witness.authority_approval.role
            == mycity_core::spatial_transport_permissions::SpatialApprovalRole::AuthorityPermit
        && witness.authority_approval.approved
        && witness.authority_approval.approver_account != request.applicant_account
        && witness.authority_approval.approver_account != request.executor_account
        && approval_evidence_sha256(
            request_digest,
            &witness.owner_approval,
            &witness.authority_approval,
        ) == Ok(record.authorization_evidence_sha256)
}

fn record_geometry_witness_matches(record: &SpatialTransportVersionRecord) -> bool {
    let (Some(descriptor), Some(witness)) = (record.geometry_descriptor, record.geometry_witness)
    else {
        return false;
    };
    let Ok(descriptor_sha256) = descriptor.sha256() else {
        return false;
    };
    let request = witness.request;
    record.authorization_evidence_sha256 == descriptor_sha256
        && record.request_id == request.request_id
        && record.changed_asset_id == descriptor.asset_id
        && record.asset_kind == SpatialAssetKind::Road
        && record.asset_revision == request.expected_asset_revision.saturating_add(1)
        && record.effective_service_day == descriptor.effective_service_day
        && record.previous_profile_sha256 == descriptor.parent_profile_sha256
        && record.profile_sha256 == descriptor.candidate_profile_sha256
        && request.expected_geometry_revision == descriptor.parent_geometry_revision
        && request.descriptor_sha256 == descriptor_sha256
        && request.asset_id == descriptor.asset_id
        && request.expected_asset_revision > 0
        && request.expected_asset_revision.checked_add(1) == Some(record.asset_revision)
        && request.expected_geometry_revision.checked_add(1)
            == Some(descriptor.candidate_geometry_revision)
        && descriptor.parent_transport_revision.checked_add(1)
            == Some(descriptor.candidate_transport_revision)
        && descriptor.parent_profile_sha256 == record.previous_profile_sha256
        && descriptor.candidate_profile_sha256 == record.profile_sha256
        && witness.owner_approval.request_id == request.request_id
        && witness.authority_approval.request_id == request.request_id
        && witness.owner_approval.request_sha256 != [0; 32]
        && witness.owner_approval.request_sha256 == witness.authority_approval.request_sha256
        && witness.owner_approval.role
            == mycity_core::spatial_geometry_permissions::GeometryApprovalRole::OwnerConsent
        && witness.authority_approval.role
            == mycity_core::spatial_geometry_permissions::GeometryApprovalRole::AuthorityPermit
        && witness.owner_approval.approved
        && witness.authority_approval.approved
        && witness.owner_approval.approver_account != witness.authority_approval.approver_account
}

/// Small writer-facing payload. It does not itself grant legal authority or commit anything;
/// the unique Store/Engine writer must validate its authorization digest and durable cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialTransportUpdateCommandPayload {
    pub schema: u16,
    pub profile_id: u32,
    pub asset_id: u32,
    pub asset_kind: SpatialAssetKind,
    pub asset_revision: u64,
    pub world_id: u64,
    pub branch_id: u64,
    pub request_id: u64,
    pub engine_operation_id: u64,
    pub receipt_operation_id: u64,
    pub receipt_revision: u64,
    pub store_sequence: u64,
    pub version: u64,
    pub effective_service_day: u64,
    pub source_scene_sha256: [u8; 32],
    pub authorization_evidence_sha256: [u8; 32],
    pub previous_profile_sha256: [u8; 32],
    pub profile_sha256: [u8; 32],
    pub surface: SpatialAccessSurface,
    pub edge_id: u32,
}

pub struct PreparedSpatialTransportUpdate {
    pub command: SpatialTransportUpdateCommandPayload,
    pub asset: EncodedProfileAsset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateError {
    InvalidIdentity,
    InvalidSourceAsset,
    StaleCurrentProfile,
    InvalidEffectiveDay,
    InvalidRevision,
    InvalidProfile,
    TopologyChanged,
    PermissionExpanded,
    CapacityChanged,
    InvalidVersionChain,
    DuplicateOperation,
    InvalidAuthorizedCandidate,
    AssetIo,
    Capacity,
    InvalidCommitBinding,
    InvalidCatalogAsset,
    CatalogLimit,
}

/// Stage an exact content-addressed child allocation using the existing Store capacity policy.
/// Call only while the Store writer lock/Prepared transaction is held. A failed later root
/// commit may leave an unreferenced immutable blob; this function never consumes permissions,
/// mutates the version-chain root, or deletes prior assets. Only a committed chain reference
/// makes the file authoritative; orphan collection must consult that authoritative root.
pub fn stage_profile_asset(
    dir: &Path,
    capacity: super::CapacityPolicy,
    world_id: u64,
    branch_id: u64,
    profile_id: u32,
    asset: &EncodedProfileAsset,
) -> Result<(), UpdateError> {
    let actual_sha256: [u8; 32] = Sha256::digest(&asset.bytes).into();
    if asset.bytes.is_empty()
        || asset.bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES
        || actual_sha256 != asset.sha256
    {
        return Err(UpdateError::InvalidSourceAsset);
    }
    super::town_transport::validate_allocation(
        &asset.bytes,
        world_id,
        branch_id,
        profile_id,
        asset.sha256,
    )
    .map_err(|_| UpdateError::InvalidProfile)?;
    super::town_transport::admit_immutable_write(
        dir,
        capacity,
        super::town_transport::ALLOCATION_PREFIX,
        &asset.bytes,
        &asset.sha256,
        false,
    )
    .map_err(|_| UpdateError::AssetIo)?;
    super::town_transport::write_immutable(
        dir,
        super::town_transport::ALLOCATION_PREFIX,
        &asset.bytes,
        &asset.sha256,
    )
    .map_err(|_| UpdateError::AssetIo)
}

/// Prepare an exact child asset for a request before dual approval. The returned digest can be
/// embedded in the immutable permission request; this does not grant authority or write a Store.
pub fn preview_profile_asset(
    current_ref: &VerifiedAllocationRef<'_>,
    proposed_opening: Opening,
) -> Result<EncodedProfileAsset, UpdateError> {
    let current = current_ref.profile;
    if current.validate().is_err() {
        return Err(UpdateError::InvalidProfile);
    }
    let expected_revision = current
        .opening
        .revision
        .checked_add(1)
        .ok_or(UpdateError::InvalidRevision)?;
    if proposed_opening.revision != expected_revision {
        return Err(UpdateError::InvalidRevision);
    }
    check_bounded_spatial_change(&current.opening, &proposed_opening)?;
    let mut profile = current.clone();
    profile.opening = proposed_opening;
    profile.assignment = profile
        .opening
        .assign()
        .map_err(|_| UpdateError::InvalidProfile)?;
    profile
        .validate()
        .map_err(|_| UpdateError::InvalidProfile)?;
    let asset = encoded_profile_asset(&profile)?;
    if asset.sha256 == current_ref.asset_sha256 {
        return Err(UpdateError::InvalidProfile);
    }
    Ok(asset)
}

/// Prepare an authorized profile version. The grant can only be created by the bounded
/// permission-state apply path. The unique writer must still commit the consumed permission
/// state, immutable profile asset, and version-chain root in one transaction.
pub fn prepare_version(
    baseline_ref: &VerifiedAllocationRef<'_>,
    current_ref: &VerifiedAllocationRef<'_>,
    grant: &VerifiedSpatialUpdateGrant,
    version: u64,
    proposed_opening: Opening,
) -> Result<SpatialTransportVersion, UpdateError> {
    prepare_version_with_structure(
        baseline_ref, baseline_ref, current_ref, grant, version, proposed_opening,
    )
}

fn prepare_version_with_structure(
    baseline_ref: &VerifiedAllocationRef<'_>,
    structural_ref: &VerifiedAllocationRef<'_>,
    current_ref: &VerifiedAllocationRef<'_>,
    grant: &VerifiedSpatialUpdateGrant,
    version: u64,
    proposed_opening: Opening,
) -> Result<SpatialTransportVersion, UpdateError> {
    let current = current_ref.profile;
    if grant.world_id() != current.world_id
        || grant.branch_id() != current.branch_id
        || grant.world_id() != baseline_ref.profile.world_id
        || grant.branch_id() != baseline_ref.profile.branch_id
        || grant.parent_profile_sha256() != current_ref.asset_sha256
        || grant.source_scene_sha256() == [0; 32]
        || grant.request_id() == 0
        || grant.asset_id() == 0
        || grant.effective_service_day() == 0
    {
        return Err(UpdateError::InvalidIdentity);
    }
    let encoded = preview_profile_asset(current_ref, proposed_opening.clone())?;
    if encoded.sha256 != grant.candidate_profile_sha256() {
        return Err(UpdateError::InvalidAuthorizedCandidate);
    }
    let (edge_id, operation) = changed_access_edge(&current.opening, &proposed_opening)?;
    if edge_id != grant.edge_id() || operation != grant.operation() {
        return Err(UpdateError::InvalidAuthorizedCandidate);
    }
    let settled_day = grant
        .effective_service_day()
        .checked_sub(1)
        .ok_or(UpdateError::InvalidEffectiveDay)?;
    let mut prepared = build_version_with_structure(
        baseline_ref,
        structural_ref,
        current_ref,
        grant.source_scene_sha256(),
        grant.evidence_sha256(),
        settled_day,
        version,
        grant.request_id(),
        proposed_opening,
    )?;
    if prepared.effective_service_day != grant.effective_service_day()
        || prepared.profile_sha256 != grant.candidate_profile_sha256()
    {
        return Err(UpdateError::InvalidAuthorizedCandidate);
    }
    prepared.asset_id = grant.asset_id();
    prepared.asset_kind = grant.asset_kind();
    prepared.asset_revision = grant.new_revision();
    prepared.world_id = grant.world_id();
    prepared.branch_id = grant.branch_id();
    prepared.surface = grant.surface();
    prepared.edge_id = grant.edge_id();
    Ok(prepared)
}

/// Build a candidate without authorization. Private to keep unverified digests from acting as grants.
/// Only the operating `open` state on existing baseline edges can change. Nodes, directed
/// topology, modes, capacities, entrances/permissions, OD demand, tariffs, partitions, and
/// project road-capacity effect remain fixed. Physical capacity growth remains governed by the
/// existing construction `road_capacity_effect` derivation.
// These eight inputs are intentionally explicit: the two verified raw assets, immutable Scene
// identity, authorization evidence, settled boundary, chain version, unique request ID, and
// proposed opening each have separate validation and must not be collapsed into caller input.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn build_version(
    baseline_ref: &VerifiedAllocationRef<'_>,
    current_ref: &VerifiedAllocationRef<'_>,
    source_scene_sha256: [u8; 32],
    authorization_evidence_sha256: [u8; 32],
    settled_day: u64,
    version: u64,
    request_id: u64,
    proposed_opening: Opening,
) -> Result<SpatialTransportVersion, UpdateError> {
    build_version_with_structure(
        baseline_ref, baseline_ref, current_ref, source_scene_sha256,
        authorization_evidence_sha256, settled_day, version, request_id, proposed_opening,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_version_with_structure(
    baseline_ref: &VerifiedAllocationRef<'_>,
    structural_ref: &VerifiedAllocationRef<'_>,
    current_ref: &VerifiedAllocationRef<'_>,
    source_scene_sha256: [u8; 32],
    authorization_evidence_sha256: [u8; 32],
    settled_day: u64,
    version: u64,
    request_id: u64,
    proposed_opening: Opening,
) -> Result<SpatialTransportVersion, UpdateError> {
    let baseline = baseline_ref.profile;
    let current = current_ref.profile;
    let current_profile_sha256 = current_ref.asset_sha256;
    if request_id == 0
        || version == 0
        || source_scene_sha256 == [0; 32]
        || authorization_evidence_sha256 == [0; 32]
        || baseline.world_id != current.world_id
        || baseline.branch_id != current.branch_id
        || baseline.market_profile_id != current.market_profile_id
        || baseline.period_seconds != current.period_seconds
        || !same_fixed_assumptions(baseline, current)
        || baseline.validate().is_err()
        || current.validate().is_err()
    {
        return Err(UpdateError::InvalidIdentity);
    }
    if current_profile_sha256 == [0; 32] || baseline_ref.asset_sha256 == [0; 32] {
        return Err(UpdateError::StaleCurrentProfile);
    }
    validate_access_profile_transition(baseline, structural_ref.profile, current, &proposed_opening)?;
    let effective_service_day = settled_day
        .checked_add(1)
        .ok_or(UpdateError::InvalidEffectiveDay)?;
    let expected_revision = current
        .opening
        .revision
        .checked_add(1)
        .ok_or(UpdateError::InvalidRevision)?;
    if proposed_opening.revision != expected_revision {
        return Err(UpdateError::InvalidRevision);
    }

    let mut profile = current.clone();
    profile.opening = proposed_opening;
    profile.assignment = profile
        .opening
        .assign()
        .map_err(|_| UpdateError::InvalidProfile)?;
    profile
        .validate()
        .map_err(|_| UpdateError::InvalidProfile)?;
    let profile_sha256 = encoded_profile_asset(&profile)?.sha256;
    if profile_sha256 == current_profile_sha256 {
        return Err(UpdateError::InvalidProfile);
    }

    let (edge_id, _) = changed_access_edge(&current.opening, &profile.opening)?;
    Ok(SpatialTransportVersion {
        schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
        request_id,
        asset_id: 1,
        asset_kind: SpatialAssetKind::Road,
        asset_revision: profile.opening.revision,
        world_id: profile.world_id,
        branch_id: profile.branch_id,
        surface: SpatialAccessSurface::Road,
        edge_id,
        version,
        effective_service_day,
        source_scene_sha256,
        authorization_evidence_sha256,
        previous_profile_sha256: current_profile_sha256,
        profile_sha256,
        profile,
    })
}

/// Validate and select the immutable allocation version active on one already scheduled day.
/// The base profile is active until the first version's effective day. Input order is part of
/// the signed/indexed chain: versions must be strictly increasing and each predecessor hash
/// must equal the immediately preceding profile hash.
pub fn profile_for_service_day<'a>(
    baseline_ref: &VerifiedAllocationRef<'a>,
    versions: &'a [SpatialTransportVersion],
    service_day: u64,
) -> Result<&'a TransportAllocationProfile, UpdateError> {
    if versions.len() > MAX_SPATIAL_TRANSPORT_VERSIONS {
        return Err(UpdateError::InvalidVersionChain);
    }
    let baseline = baseline_ref.profile;
    let baseline_sha256 = baseline_ref.asset_sha256;
    if baseline.validate().is_err() || baseline_sha256 == [0; 32] {
        return Err(UpdateError::InvalidVersionChain);
    }
    let mut active = baseline;
    let mut active_hash = baseline_sha256;
    let mut previous_day = 0;
    let mut previous_version: u64 = 0;
    for (index, version) in versions.iter().enumerate() {
        if version.schema != SPATIAL_TRANSPORT_VERSION_SCHEMA
            || version.request_id == 0
            || version.asset_id == 0
            || previous_version.checked_add(1) != Some(version.version)
            || version.effective_service_day <= previous_day
            || version.previous_profile_sha256 != active_hash
            || version.profile_sha256 != encoded_profile_asset(&version.profile)?.sha256
            || version.profile.validate().is_err()
            || version.profile.world_id != baseline.world_id
            || version.profile.branch_id != baseline.branch_id
            || version.world_id != baseline.world_id
            || version.branch_id != baseline.branch_id
            || version.profile.market_profile_id != baseline.market_profile_id
            || !same_fixed_assumptions(baseline, &version.profile)
            || version.source_scene_sha256 == [0; 32]
            || version.authorization_evidence_sha256 == [0; 32]
            || versions[..index]
                .iter()
                .any(|prior| prior.request_id == version.request_id)
        {
            return Err(
                if versions[..index]
                    .iter()
                    .any(|prior| prior.request_id == version.request_id)
                {
                    UpdateError::DuplicateOperation
                } else {
                    UpdateError::InvalidVersionChain
                },
            );
        }
        // Every selected version must remain within the same immutable baseline topology and
        // original permissions, even if an untrusted catalog was supplied by a caller.
        check_bounded_spatial_change(&baseline.opening, &version.profile.opening)?;
        check_initial_open_permissions(&baseline.opening, &version.profile.opening)?;
        if index > 0 && version.source_scene_sha256 != versions[0].source_scene_sha256 {
            return Err(UpdateError::InvalidVersionChain);
        }
        if version.effective_service_day > service_day {
            break;
        }
        active = &version.profile;
        active_hash = version.profile_sha256;
        previous_day = version.effective_service_day;
        previous_version = version.version;
    }
    Ok(active)
}

/// Resolve a persisted proof to the exact historical immutable allocation asset it names.
/// The returned profile can be passed to the existing Store proof verifier; this helper does
/// not replace verification of the proof's opening, assignment, tariff, or bytes.
pub fn profile_for_proof<'a>(
    baseline_ref: &VerifiedAllocationRef<'a>,
    versions: &'a [SpatialTransportVersion],
    proof: &mycity_core::market_access::TransportProofAsset,
) -> Result<&'a TransportAllocationProfile, UpdateError> {
    let profile = profile_for_service_day(baseline_ref, versions, proof.service_day)?;
    let expected_hash = versions
        .iter()
        .take_while(|version| version.effective_service_day <= proof.service_day)
        .last()
        .map_or(baseline_ref.asset_sha256, |version| version.profile_sha256);
    if proof.allocation_profile_sha256 != expected_hash {
        return Err(UpdateError::InvalidVersionChain);
    }
    Ok(profile)
}

/// Resolve the exact content-addressed allocation hash named by a persisted proof, without
/// loading or rewriting any historical profile bytes.
pub fn profile_sha256_for_proof(
    catalog: &SpatialTransportCatalog,
    proof: &mycity_core::market_access::TransportProofAsset,
) -> Result<[u8; 32], UpdateError> {
    if proof.world_id != catalog.world_id
        || proof.branch_id != catalog.branch_id
        || proof.profile_id != catalog.profile_id
    {
        return Err(UpdateError::InvalidVersionChain);
    }
    let expected = catalog.profile_sha256_for_service_day(proof.service_day)?;
    if proof.allocation_profile_sha256 != expected {
        return Err(UpdateError::InvalidVersionChain);
    }
    Ok(expected)
}

pub fn prepare_command_payload(
    version: &SpatialTransportVersion,
    grant: &VerifiedSpatialUpdateGrant,
    commit: SpatialTransportCommitBinding,
) -> Result<PreparedSpatialTransportUpdate, UpdateError> {
    commit.validate()?;
    if version.schema != SPATIAL_TRANSPORT_VERSION_SCHEMA
        || version.request_id == 0
        || version.version == 0
        || version.asset_id == 0
        || version.asset_revision == 0
        || version.world_id == 0
        || version.branch_id == 0
        || version.edge_id == 0
        || version.source_scene_sha256 == [0; 32]
        || version.authorization_evidence_sha256 == [0; 32]
        || version.previous_profile_sha256 == [0; 32]
        || version.request_id != grant.request_id()
        || version.asset_id != grant.asset_id()
        || version.asset_kind != grant.asset_kind()
        || version.asset_revision != grant.new_revision()
        || version.world_id != grant.world_id()
        || version.branch_id != grant.branch_id()
        || version.surface != grant.surface()
        || version.edge_id != grant.edge_id()
        || version.effective_service_day != grant.effective_service_day()
        || version.source_scene_sha256 != grant.source_scene_sha256()
        || version.authorization_evidence_sha256 != grant.evidence_sha256()
        || version.previous_profile_sha256 != grant.parent_profile_sha256()
        || version.profile_sha256 != grant.candidate_profile_sha256()
    {
        return Err(UpdateError::InvalidIdentity);
    }
    let asset = encoded_profile_asset(&version.profile)?;
    if asset.sha256 != version.profile_sha256 {
        return Err(UpdateError::InvalidVersionChain);
    }
    Ok(PreparedSpatialTransportUpdate {
        command: SpatialTransportUpdateCommandPayload {
            schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
            profile_id: version.profile.market_profile_id,
            asset_id: version.asset_id,
            asset_kind: version.asset_kind,
            asset_revision: version.asset_revision,
            world_id: version.world_id,
            branch_id: version.branch_id,
            request_id: version.request_id,
            engine_operation_id: commit.engine_operation_id,
            receipt_operation_id: commit.receipt_operation_id,
            receipt_revision: commit.receipt_revision,
            store_sequence: commit.store_sequence,
            version: version.version,
            effective_service_day: version.effective_service_day,
            source_scene_sha256: version.source_scene_sha256,
            authorization_evidence_sha256: version.authorization_evidence_sha256,
            previous_profile_sha256: version.previous_profile_sha256,
            profile_sha256: version.profile_sha256,
            surface: version.surface,
            edge_id: version.edge_id,
        },
        asset,
    })
}

pub struct EncodedProfileAsset {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
}

/// Serialize a newly accepted child profile once; Store must write these exact bytes under this
/// digest. Existing raw assets must never be rewritten using this canonical encoding.
pub fn encoded_profile_asset(
    profile: &TransportAllocationProfile,
) -> Result<EncodedProfileAsset, UpdateError> {
    let bytes = serde_json::to_vec(profile).map_err(|_| UpdateError::InvalidProfile)?;
    if bytes.len() > mycity_core::market_access::TRANSPORT_PROOF_SCHEMA_MAX_BYTES {
        return Err(UpdateError::InvalidProfile);
    }
    let sha256 = Sha256::digest(&bytes).into();
    Ok(EncodedProfileAsset { bytes, sha256 })
}

/// Shared hot-commit/cold-prefix check. The Store must authenticate the structural anchor
/// against the last rebuilt geometry record; genesis remains the access-rights ceiling.
pub(crate) fn validate_access_profile_transition(
    genesis: &TransportAllocationProfile,
    structural: &TransportAllocationProfile,
    current: &TransportAllocationProfile,
    proposed: &Opening,
) -> Result<(u32, SpatialAccessOperation), UpdateError> {
    if !same_fixed_assumptions(genesis, structural)
        || !same_fixed_assumptions(genesis, current)
        || genesis.validate().is_err()
        || structural.validate().is_err()
        || current.validate().is_err()
    {
        return Err(UpdateError::InvalidIdentity);
    }
    if current.opening.revision.checked_add(1) != Some(proposed.revision) {
        return Err(UpdateError::InvalidRevision);
    }
    check_bounded_spatial_change(&structural.opening, &current.opening)?;
    check_bounded_spatial_change(&structural.opening, proposed)?;
    check_initial_open_permissions(&genesis.opening, &structural.opening)?;
    check_initial_open_permissions(&genesis.opening, &current.opening)?;
    check_initial_open_permissions(&genesis.opening, proposed)?;
    changed_access_edge(&current.opening, proposed)
}

/// Compare immutable geometry/topology/capacity while allowing the operating open bit to toggle.
/// This helper is also used for candidate preview against the *current* profile, where restoring
/// an edge closed by an earlier authorized update must remain representable. Genesis permission
/// checks are deliberately separate and must always compare against the initial profile.
fn check_bounded_spatial_change(baseline: &Opening, proposed: &Opening) -> Result<(), UpdateError> {
    if proposed.nodes != baseline.nodes
        || proposed.entrances != baseline.entrances
        || proposed.demands != baseline.demands
        || proposed.edges.len() != baseline.edges.len()
    {
        return Err(UpdateError::TopologyChanged);
    }
    for base_edge in &baseline.edges {
        let edge = proposed
            .edges
            .iter()
            .find(|candidate| candidate.id == base_edge.id)
            .ok_or(UpdateError::TopologyChanged)?;
        if edge.id != base_edge.id
            || edge.from != base_edge.from
            || edge.to != base_edge.to
            || edge.city_from != base_edge.city_from
            || edge.city_to != base_edge.city_to
            || edge.free_flow_seconds != base_edge.free_flow_seconds
            || edge.mode_mask != base_edge.mode_mask
            || edge.boundary_source != base_edge.boundary_source
        {
            return Err(UpdateError::TopologyChanged);
        }
        if edge.capacity_people != base_edge.capacity_people {
            return Err(UpdateError::CapacityChanged);
        }
    }
    proposed.validate().map_err(|_| UpdateError::InvalidProfile)
}

/// The immutable genesis profile is the ceiling for access rights: a later version can restore
/// only an edge that was initially open. `check_bounded_spatial_change` cannot enforce this when
/// it is comparing a candidate with the currently-closed profile, so callers at version-build
/// and catalog-validation boundaries must call this with the original baseline.
fn check_initial_open_permissions(
    genesis: &Opening,
    proposed: &Opening,
) -> Result<(), UpdateError> {
    for initial in &genesis.edges {
        let edge = proposed
            .edges
            .iter()
            .find(|candidate| candidate.id == initial.id)
            .ok_or(UpdateError::TopologyChanged)?;
        if edge.open && !initial.open {
            return Err(UpdateError::PermissionExpanded);
        }
    }
    Ok(())
}

fn changed_access_edge(
    current: &Opening,
    proposed: &Opening,
) -> Result<(u32, SpatialAccessOperation), UpdateError> {
    if current.edges.len() != proposed.edges.len() {
        return Err(UpdateError::TopologyChanged);
    }
    let mut changed = None;
    for edge in &current.edges {
        let next = proposed
            .edges
            .iter()
            .find(|candidate| candidate.id == edge.id)
            .ok_or(UpdateError::TopologyChanged)?;
        if edge.open != next.open {
            if changed.is_some() {
                return Err(UpdateError::InvalidAuthorizedCandidate);
            }
            changed = Some((
                edge.id,
                if edge.open {
                    SpatialAccessOperation::Close
                } else {
                    SpatialAccessOperation::Restore
                },
            ));
        }
    }
    changed.ok_or(UpdateError::InvalidAuthorizedCandidate)
}

fn same_fixed_assumptions(
    left: &TransportAllocationProfile,
    right: &TransportAllocationProfile,
) -> bool {
    left.schema == right.schema
        && left.world_id == right.world_id
        && left.branch_id == right.branch_id
        && left.market_profile_id == right.market_profile_id
        && left.period_seconds == right.period_seconds
        && left.allocation_scope == right.allocation_scope
        && left.demand_repeat_policy == right.demand_repeat_policy
        && left.road_capacity_effect == right.road_capacity_effect
        && left.partitions == right.partitions
        && left.fee_policy == right.fee_policy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_anchor_allows_access_change_without_allowing_another_structural_edit() {
        let genesis = profile();
        let mut geometry = genesis.clone();
        geometry.opening.revision = 2;
        geometry.opening.edges[0].free_flow_seconds = 11;
        geometry.assignment = geometry.opening.assign().unwrap();
        let mut close = geometry.opening.clone();
        close.revision = 3;
        close.edges[0].open = false;
        assert_eq!(
            validate_access_profile_transition(&genesis, &geometry, &geometry, &close),
            Ok((10, SpatialAccessOperation::Close))
        );
        assert!(validate_access_profile_transition(&genesis, &genesis, &geometry, &close).is_err());
        let mut changed_cost = close.clone();
        changed_cost.edges[0].free_flow_seconds += 1;
        assert!(validate_access_profile_transition(&genesis, &geometry, &geometry, &changed_cost).is_err());
        let mut changed_capacity = close;
        changed_capacity.edges[0].capacity_people += 1;
        assert!(validate_access_profile_transition(&genesis, &geometry, &geometry, &changed_capacity).is_err());
    }

    #[test]
    fn geometry_anchor_never_expands_genesis_initial_access_rights() {
        let mut genesis = profile();
        genesis.opening.edges[1].open = false;
        genesis.assignment = genesis.opening.assign().unwrap();
        let mut geometry = genesis.clone();
        geometry.opening.revision = 2;
        geometry.opening.edges[0].free_flow_seconds = 11;
        geometry.assignment = geometry.opening.assign().unwrap();
        let mut unauthorized_restore = geometry.opening.clone();
        unauthorized_restore.revision = 3;
        unauthorized_restore.edges[1].open = true;
        assert_eq!(
            validate_access_profile_transition(&genesis, &geometry, &geometry, &unauthorized_restore),
            Err(UpdateError::PermissionExpanded)
        );
    }
    use mycity_core::{
        market_access::{AllocationScope, DemandRepeatPolicy, FeePolicyInput, TripPartition},
        timeline::DAY,
        transport::{Entrance, Node, OdDemand, RoadEdge},
        units::Quantity,
    };

    fn profile() -> TransportAllocationProfile {
        let opening = Opening {
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
                    capacity_people: 8,
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
                    capacity_people: 8,
                    mode_mask: 1,
                    open: true,
                    boundary_source: None,
                },
            ],
            entrances: vec![
                Entrance {
                    id: 1,
                    owner: 1,
                    node: 1,
                    mode_mask: 1,
                    authorized: true,
                },
                Entrance {
                    id: 2,
                    owner: 2,
                    node: 2,
                    mode_mask: 1,
                    authorized: true,
                },
            ],
            demands: vec![
                OdDemand {
                    source_id: 1,
                    origin_entrance: 1,
                    destination_entrance: 2,
                    mode: 1,
                    people: 4,
                },
                OdDemand {
                    source_id: 2,
                    origin_entrance: 2,
                    destination_entrance: 1,
                    mode: 1,
                    people: 4,
                },
            ],
        };
        TransportAllocationProfile {
            schema: 1,
            world_id: 1,
            branch_id: 1,
            market_profile_id: 1,
            period_seconds: DAY as u32,
            allocation_scope: AllocationScope::SingleTownExclusive,
            demand_repeat_policy: DemandRepeatPolicy::RepeatFixedDailyDemand,
            road_capacity_effect: None,
            assignment: opening.assign().unwrap(),
            opening,
            partitions: vec![TripPartition {
                quote_id: 1,
                outbound_source: 1,
                return_source: 2,
                trip_count: 1,
                goods_per_trip: Quantity::new(1).unwrap(),
            }],
            fee_policy: FeePolicyInput {
                provider_account: 1,
                cents_per_travel_second: 0,
            },
        }
    }

    fn verified(profile: &TransportAllocationProfile) -> VerifiedAllocationRef<'_> {
        let hash = encoded_profile_asset(profile).unwrap().sha256;
        VerifiedAllocationRef::from_verified_store(profile, hash)
    }

    fn close_forward_edge(source: &TransportAllocationProfile) -> Opening {
        let mut opening = source.opening.clone();
        opening.revision += 1;
        opening
            .edges
            .iter_mut()
            .find(|edge| edge.id == 10)
            .unwrap()
            .open = false;
        opening
    }

    #[test]
    fn closure_is_next_day_and_old_and_new_profile_hashes_select_independently() {
        let baseline = profile();
        let baseline_ref = verified(&baseline);
        let baseline_hash = baseline_ref.asset_sha256();
        let version = build_version(
            &baseline_ref,
            &baseline_ref,
            [7; 32],
            [8; 32],
            3,
            1,
            41,
            close_forward_edge(&baseline),
        )
        .unwrap();
        assert_eq!(version.effective_service_day, 4);
        assert_eq!(baseline.assignment.demand_outcomes[0].served_people, 4);
        assert_eq!(
            version.profile.assignment.demand_outcomes[0].served_people,
            0
        );
        assert_eq!(
            profile_for_service_day(&baseline_ref, &[version.clone()], 3).unwrap(),
            &baseline
        );
        assert_eq!(
            profile_for_service_day(&baseline_ref, &[version.clone()], 4).unwrap(),
            &version.profile
        );
        assert_eq!(version.previous_profile_sha256, baseline_hash);
        assert_eq!(
            version.profile_sha256,
            encoded_profile_asset(&version.profile).unwrap().sha256
        );
        assert!(profile_for_service_day(&baseline_ref, &[version], 4).is_ok());
    }

    #[test]
    fn accepted_raw_json_hash_remains_the_parent_and_old_proof_reference() {
        let original = profile();
        let mut raw = b" \n".to_vec();
        raw.extend(serde_json::to_vec(&original).unwrap());
        raw.extend(b"\n \t");
        let raw_hash: [u8; 32] = Sha256::digest(&raw).into();
        let decoded = crate::town_transport::validate_allocation(&raw, 1, 1, 1, raw_hash).unwrap();
        assert_eq!(decoded, original);
        let baseline_ref = VerifiedAllocationRef::from_verified_store(&decoded, raw_hash);
        let old_proof = mycity_core::market_access::TransportProofAsset {
            schema: 1,
            world_id: decoded.world_id,
            branch_id: decoded.branch_id,
            profile_id: decoded.market_profile_id,
            allocation_profile_sha256: raw_hash,
            service_day: 3,
            period_seconds: decoded.period_seconds,
            generation: 3,
            opening: decoded.opening.clone(),
            assignment: decoded.assignment.clone(),
            partitions: decoded.partitions.clone(),
            fee_policy: decoded.fee_policy,
        };

        let version = build_version(
            &baseline_ref,
            &baseline_ref,
            [7; 32],
            [8; 32],
            3,
            1,
            4,
            close_forward_edge(&decoded),
        )
        .unwrap();
        assert_eq!(version.previous_profile_sha256, raw_hash);
        assert_eq!(old_proof.allocation_profile_sha256, raw_hash);
        assert_eq!(
            profile_for_proof(&baseline_ref, &[version.clone()], &old_proof).unwrap(),
            &decoded
        );
        let mut mismatched_proof = old_proof.clone();
        mismatched_proof.allocation_profile_sha256 = version.profile_sha256;
        assert_eq!(
            profile_for_proof(&baseline_ref, &[version.clone()], &mismatched_proof),
            Err(UpdateError::InvalidVersionChain)
        );
        let child_asset = encoded_profile_asset(&version.profile).unwrap();
        assert_eq!(child_asset.sha256, version.profile_sha256);
        assert_ne!(child_asset.sha256, raw_hash);
        assert_eq!(
            serde_json::from_slice::<TransportAllocationProfile>(&child_asset.bytes).unwrap(),
            version.profile
        );
    }

    #[test]
    fn a_later_reopen_uses_a_new_profile_without_rewriting_the_closed_day() {
        let baseline = profile();
        let baseline_ref = verified(&baseline);
        let close = build_version(
            &baseline_ref,
            &baseline_ref,
            [7; 32],
            [8; 32],
            3,
            1,
            41,
            close_forward_edge(&baseline),
        )
        .unwrap();
        let mut reopened = close.profile.opening.clone();
        reopened.revision += 1;
        reopened
            .edges
            .iter_mut()
            .find(|edge| edge.id == 10)
            .unwrap()
            .open = true;
        let closed_ref =
            VerifiedAllocationRef::from_verified_store(&close.profile, close.profile_sha256);
        let reopen = build_version(
            &baseline_ref,
            &closed_ref,
            [7; 32],
            [9; 32],
            4,
            2,
            42,
            reopened,
        )
        .unwrap();
        assert_eq!(reopen.previous_profile_sha256, close.profile_sha256);
        assert_eq!(reopen.effective_service_day, 5);
        assert_eq!(
            profile_for_service_day(&baseline_ref, &[close.clone(), reopen.clone()], 4).unwrap(),
            &close.profile
        );
        assert_eq!(
            profile_for_service_day(&baseline_ref, &[close, reopen.clone()], 5).unwrap(),
            &reopen.profile
        );
    }

    #[test]
    fn update_cannot_add_edges_reopen_ungranted_edges_or_widen_entrance_authority() {
        let baseline = profile();
        let baseline_ref = verified(&baseline);

        let mut added = close_forward_edge(&baseline);
        added.edges.push(added.edges[0]);
        assert_eq!(
            build_version(
                &baseline_ref,
                &baseline_ref,
                [7; 32],
                [8; 32],
                0,
                1,
                1,
                added
            ),
            Err(UpdateError::TopologyChanged)
        );

        let mut widened = close_forward_edge(&baseline);
        widened.entrances[0].authorized = false;
        assert_eq!(
            build_version(
                &baseline_ref,
                &baseline_ref,
                [7; 32],
                [8; 32],
                0,
                1,
                2,
                widened
            ),
            Err(UpdateError::TopologyChanged)
        );

        let mut capacity_changed = close_forward_edge(&baseline);
        capacity_changed.edges[0].capacity_people += 1;
        assert_eq!(
            build_version(
                &baseline_ref,
                &baseline_ref,
                [7; 32],
                [8; 32],
                0,
                1,
                4,
                capacity_changed
            ),
            Err(UpdateError::CapacityChanged)
        );

        // A closed genesis edge is never restorable merely because preview can encode the
        // candidate. The immutable baseline permission check rejects it at version creation.
        let mut initially_closed = baseline.clone();
        initially_closed.opening.edges[0].open = false;
        initially_closed.assignment = initially_closed.opening.assign().unwrap();
        let closed_ref = verified(&initially_closed);
        let mut unauthorized = initially_closed.opening.clone();
        unauthorized.revision += 1;
        unauthorized.edges[0].open = true;
        assert!(preview_profile_asset(&closed_ref, unauthorized.clone()).is_ok());
        assert_eq!(
            build_version(
                &closed_ref,
                &closed_ref,
                [7; 32],
                [8; 32],
                1,
                2,
                3,
                unauthorized
            ),
            Err(UpdateError::PermissionExpanded)
        );
    }

    #[test]
    fn profile_chain_rejects_hash_reuse_and_duplicate_operations() {
        let baseline = profile();
        let baseline_ref = verified(&baseline);
        let first = build_version(
            &baseline_ref,
            &baseline_ref,
            [7; 32],
            [8; 32],
            0,
            1,
            9,
            close_forward_edge(&baseline),
        )
        .unwrap();
        let repeated = build_version(
            &baseline_ref,
            &baseline_ref,
            [7; 32],
            [8; 32],
            0,
            1,
            9,
            close_forward_edge(&baseline),
        )
        .unwrap();
        assert_eq!(first, repeated);

        let mut second = first.clone();
        second.version = 2;
        second.effective_service_day = 2;
        second.previous_profile_sha256 = first.profile_sha256;
        second.profile.opening.revision += 1;
        second.profile.opening.edges[1].open = false;
        second.profile.assignment = second.profile.opening.assign().unwrap();
        second.profile_sha256 = encoded_profile_asset(&second.profile).unwrap().sha256;
        let chain = [first, second];
        assert_eq!(
            profile_for_service_day(&baseline_ref, &chain, 2),
            Err(UpdateError::DuplicateOperation)
        );
    }

    #[test]
    fn compact_catalog_keeps_raw_baseline_and_bounds_serialized_history() {
        let baseline = profile();
        let baseline_ref = verified(&baseline);
        let catalog = SpatialTransportCatalog::new(
            baseline.world_id,
            baseline.branch_id,
            baseline.market_profile_id,
            [5; 32],
            baseline_ref.asset_sha256(),
            &[SpatialTransportAssetBaseline {
                asset_id: 7,
                kind: SpatialAssetKind::Road,
                revision: 1,
            }],
        )
        .unwrap();
        assert_eq!(
            catalog.profile_sha256_for_service_day(1).unwrap(),
            baseline_ref.asset_sha256()
        );
        let proof = mycity_core::market_access::TransportProofAsset {
            schema: 1,
            world_id: baseline.world_id,
            branch_id: baseline.branch_id,
            profile_id: baseline.market_profile_id,
            allocation_profile_sha256: baseline_ref.asset_sha256(),
            service_day: 1,
            period_seconds: baseline.period_seconds,
            generation: 1,
            opening: baseline.opening.clone(),
            assignment: baseline.assignment.clone(),
            partitions: baseline.partitions.clone(),
            fee_policy: baseline.fee_policy,
        };
        assert_eq!(
            profile_sha256_for_proof(&catalog, &proof).unwrap(),
            baseline_ref.asset_sha256()
        );
        let mut wrong_proof = proof;
        wrong_proof.allocation_profile_sha256 = [99; 32];
        assert_eq!(
            profile_sha256_for_proof(&catalog, &wrong_proof),
            Err(UpdateError::InvalidVersionChain)
        );
        let encoded = encode_spatial_transport_catalog(&catalog).unwrap();
        let decoded = decode_spatial_transport_catalog(&encoded.bytes, encoded.sha256).unwrap();
        assert_eq!(decoded, catalog);
        assert_eq!(decoded.assets.capacity(), MAX_SPATIAL_TRANSPORT_ASSETS);
        assert_eq!(decoded.versions.capacity(), MAX_SPATIAL_TRANSPORT_VERSIONS);
        assert!(decoded.owned_capacity_bytes() >= std::mem::size_of::<SpatialTransportCatalog>());
        let mut wrong_hash = encoded.sha256;
        wrong_hash[0] ^= 1;
        assert_eq!(
            decode_spatial_transport_catalog(&encoded.bytes, wrong_hash),
            Err(UpdateError::InvalidCatalogAsset)
        );
        let too_large = vec![b' '; SPATIAL_TRANSPORT_CATALOG_MAX_BYTES + 1];
        assert_eq!(
            decode_spatial_transport_catalog(&too_large, Sha256::digest(&too_large).into()),
            Err(UpdateError::InvalidCatalogAsset)
        );

        #[derive(Serialize)]
        struct OversizedCatalog {
            schema: u16,
            world_id: u64,
            branch_id: u64,
            profile_id: u32,
            source_scene_sha256: [u8; 32],
            baseline_profile_sha256: [u8; 32],
            parent_catalog_sha256: [u8; 32],
            assets: Vec<SpatialTransportAssetBaseline>,
            versions: Vec<SpatialTransportVersionRecord>,
        }
        let witness_request = mycity_core::spatial_transport_permissions::SpatialUpdateRequest {
            request_id: 1,
            applicant_account: 30,
            executor_account: 40,
            asset_id: 7,
            operation: SpatialAccessOperation::Close,
            surface: SpatialAccessSurface::Road,
            edge_id: 9,
            expected_revision: 1,
            candidate_profile_sha256: [3; 32],
            effective_time: mycity_core::units::GameTime::new(mycity_core::timeline::DAY as i64)
                .unwrap(),
            parent_profile_sha256: [2; 32],
        };
        let authorization_context = SpatialAuthorizationContext {
            world_id: 1,
            branch_id: 1,
            source_scene_sha256: [5; 32],
        };
        let request_digest = request_sha256(authorization_context, &witness_request).unwrap();
        let owner_approval = mycity_core::spatial_transport_permissions::SpatialUpdateApproval {
            request_id: 1,
            request_sha256: request_digest,
            approver_account: 10,
            role: mycity_core::spatial_transport_permissions::SpatialApprovalRole::OwnerConsent,
            approved: true,
        };
        let authority_approval =
            mycity_core::spatial_transport_permissions::SpatialUpdateApproval {
                request_id: 1,
                request_sha256: request_digest,
                approver_account: 20,
                role:
                    mycity_core::spatial_transport_permissions::SpatialApprovalRole::AuthorityPermit,
                approved: true,
            };
        let evidence_digest =
            approval_evidence_sha256(request_digest, &owner_approval, &authority_approval).unwrap();
        let row = SpatialTransportVersionRecord {
            schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
            request_id: 1,
            version: 1,
            effective_service_day: 1,
            engine_operation_id: 50,
            receipt_operation_id: 50,
            receipt_revision: 1,
            store_sequence: 1,
            changed_asset_id: 7,
            asset_kind: SpatialAssetKind::Road,
            asset_revision: 2,
            source_scene_sha256: [5; 32],
            authorization_evidence_sha256: evidence_digest,
            authorization_witness: Some(SpatialAuthorizationWitness {
                request: witness_request,
                owner_approval,
                authority_approval,
            }),
            geometry_descriptor: None,
            geometry_witness: None,
            previous_profile_sha256: [2; 32],
            profile_sha256: [3; 32],
            surface: SpatialAccessSurface::Road,
            edge_id: 9,
        };
        assert!(record_authorization_witness_matches(
            &row,
            authorization_context
        ));
        assert!(!record_authorization_witness_matches(
            &row,
            SpatialAuthorizationContext {
                source_scene_sha256: [6; 32],
                ..authorization_context
            }
        ));
        let mut forged_witness = row;
        forged_witness
            .authorization_witness
            .as_mut()
            .unwrap()
            .authority_approval
            .approver_account = 21;
        assert!(!record_authorization_witness_matches(
            &forged_witness,
            authorization_context
        ));
        let excessive = serde_json::to_vec(&OversizedCatalog {
            schema: SPATIAL_TRANSPORT_VERSION_SCHEMA,
            world_id: 1,
            branch_id: 1,
            profile_id: 1,
            source_scene_sha256: [5; 32],
            baseline_profile_sha256: [2; 32],
            parent_catalog_sha256: [0; 32],
            assets: vec![SpatialTransportAssetBaseline {
                asset_id: 7,
                kind: SpatialAssetKind::Road,
                revision: 1,
            }],
            versions: vec![row; MAX_SPATIAL_TRANSPORT_VERSIONS + 1],
        })
        .unwrap();
        assert!(serde_json::from_slice::<SpatialTransportCatalog>(&excessive).is_err());
    }
}
