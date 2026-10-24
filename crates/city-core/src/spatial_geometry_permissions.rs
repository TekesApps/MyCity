//! Independent, descriptor-bound authorization for geometry and transport-profile revisions.
//!
//! Existing road/door Close and Restore permission records keep their v1 wire format and
//! digest. This state only authorizes one immutable geometry descriptor hash; Store must still
//! load and validate the descriptor and its exact Scene/profile bytes before publication.
use crate::spatial_transport_permissions::SpatialPermissionState;
use serde::{de, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const SPATIAL_GEOMETRY_SCHEMA: u16 = 1;
pub const MAX_GEOMETRY_REQUESTS: usize = 8;
pub const MAX_GEOMETRY_APPROVALS: usize = 16;

fn rows<'de, D, T, const LIMIT: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T, const LIMIT: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const LIMIT: usize> de::Visitor<'de> for Bounded<T, LIMIT> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "at most {LIMIT} geometry authorization rows")
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut result = Vec::new();
            for _ in 0..LIMIT {
                match seq.next_element()? {
                    Some(row) => {
                        result
                            .try_reserve_exact(1)
                            .map_err(|_| de::Error::custom("spatial_geometry_allocation"))?;
                        result.push(row);
                    }
                    None => return Ok(result),
                }
            }
            struct Reject;
            impl<'de> de::DeserializeSeed<'de> for Reject {
                type Value = ();
                fn deserialize<D2: Deserializer<'de>>(self, _: D2) -> Result<(), D2::Error> {
                    Err(de::Error::custom("spatial_geometry_limit"))
                }
            }
            seq.next_element_seed(Reject)?;
            Ok(result)
        }
    }
    deserializer.deserialize_seq(Bounded::<T, LIMIT>(std::marker::PhantomData))
}

fn request_rows<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<GeometryRequestRecord>, D::Error> {
    rows::<D, GeometryRequestRecord, MAX_GEOMETRY_REQUESTS>(d)
}
fn approval_rows<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<GeometryApproval>, D::Error> {
    rows::<D, GeometryApproval, MAX_GEOMETRY_APPROVALS>(d)
}

/// Immutable descriptor body stored under its exact SHA-256. The bytes named by the scene and
/// profile hashes are independently validated by Store before Apply can publish them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryRevisionDescriptor {
    pub schema: u16,
    pub world_id: u64,
    pub branch_id: u64,
    pub asset_id: u32,
    pub parent_geometry_revision: u64,
    pub parent_scene_input_sha256: [u8; 32],
    pub parent_scene_state_sha256: [u8; 32],
    pub parent_transport_revision: u64,
    pub parent_profile_sha256: [u8; 32],
    /// Hash of the immutable traffic-adapter basis used to derive both baseline and candidates.
    /// Geometry versions are invalid when the Store cannot load this exact basis.
    pub adapter_basis_sha256: [u8; 32],
    pub candidate_geometry_revision: u64,
    pub candidate_scene_input_sha256: [u8; 32],
    pub candidate_scene_state_sha256: [u8; 32],
    pub candidate_transport_revision: u64,
    pub candidate_profile_sha256: [u8; 32],
    pub effective_service_day: u64,
}

impl GeometryRevisionDescriptor {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != SPATIAL_GEOMETRY_SCHEMA
            || self.world_id == 0
            || self.branch_id == 0
            || self.asset_id == 0
            || self.parent_geometry_revision == 0
            || self.parent_transport_revision == 0
            || self.candidate_geometry_revision
                != self
                    .parent_geometry_revision
                    .checked_add(1)
                    .ok_or(Error::Overflow)?
            || self.candidate_transport_revision
                != self
                    .parent_transport_revision
                    .checked_add(1)
                    .ok_or(Error::Overflow)?
            || self.parent_scene_input_sha256 == [0; 32]
            || self.parent_scene_state_sha256 == [0; 32]
            || self.parent_profile_sha256 == [0; 32]
            || self.adapter_basis_sha256 == [0; 32]
            || self.candidate_scene_input_sha256 == [0; 32]
            || self.candidate_scene_state_sha256 == [0; 32]
            || self.candidate_profile_sha256 == [0; 32]
            || self.parent_scene_input_sha256 == self.candidate_scene_input_sha256
            || self.parent_scene_state_sha256 == self.candidate_scene_state_sha256
            || self.parent_profile_sha256 == self.candidate_profile_sha256
            || self.effective_service_day == 0
        {
            return Err(Error::InvalidDescriptor);
        }
        Ok(())
    }

    pub fn sha256(&self) -> Result<[u8; 32], Error> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Encoding)?;
        Ok(Sha256::digest(bytes).into())
    }
}

/// Verify the preserved geometry request and both approvals against the immutable genesis
/// ownership registry. This is used when reopening a catalog after the active request itself
/// has already been consumed by Apply.
pub fn validate_authorization_witness(
    descriptor: &GeometryRevisionDescriptor,
    witness: SpatialGeometryAuthorizationWitness,
    permissions: &SpatialPermissionState,
) -> Result<VerifiedGeometryGrant, Error> {
    descriptor.validate()?;
    permissions.validate().map_err(|_| Error::InvalidIdentity)?;
    let request = witness.request;
    let descriptor_sha256 = descriptor.sha256()?;
    let asset = permissions
        .assets()
        .iter()
        .find(|asset| asset.asset_id == descriptor.asset_id)
        .ok_or(Error::InvalidRequest)?;
    let genesis_scene_sha256 = permissions
        .assets()
        .first()
        .ok_or(Error::InvalidIdentity)?
        .source_scene_sha256;
    let state = SpatialGeometryState::new(descriptor.world_id, descriptor.branch_id)?;
    let request_sha256 = state.request_sha256(&request)?;
    if descriptor.world_id != permissions.world_id
        || descriptor.branch_id != permissions.branch_id
        || asset.kind != crate::spatial_transport_permissions::SpatialAssetKind::Road
        || asset.source_scene_sha256 != genesis_scene_sha256
        || permissions
            .assets()
            .iter()
            .any(|entry| entry.current_profile_sha256 != descriptor.parent_profile_sha256)
        || request.request_id == 0
        || request.asset_id != descriptor.asset_id
        || request.expected_geometry_revision != descriptor.parent_geometry_revision
        || request.expected_asset_revision != asset.revision
        || request.descriptor_sha256 != descriptor_sha256
        || request.applicant_account == 0
        || request.executor_account == 0
        || request.applicant_account == request.executor_account
        || request.applicant_account == asset.owner_account
        || request.applicant_account == asset.permit_authority_account
        || request.executor_account == asset.owner_account
        || request.executor_account == asset.permit_authority_account
        || witness.owner_approval.request_id != request.request_id
        || witness.owner_approval.request_sha256 != request_sha256
        || witness.owner_approval.approver_account != asset.owner_account
        || witness.owner_approval.role != GeometryApprovalRole::OwnerConsent
        || !witness.owner_approval.approved
        || witness.authority_approval.request_id != request.request_id
        || witness.authority_approval.request_sha256 != request_sha256
        || witness.authority_approval.approver_account != asset.permit_authority_account
        || witness.authority_approval.role != GeometryApprovalRole::AuthorityPermit
        || !witness.authority_approval.approved
        || witness.owner_approval.approver_account == witness.authority_approval.approver_account
    {
        return Err(Error::InvalidRequest);
    }
    Ok(VerifiedGeometryGrant {
        request,
        owner_approval: witness.owner_approval,
        authority_approval: witness.authority_approval,
        request_sha256,
        world_id: descriptor.world_id,
        branch_id: descriptor.branch_id,
        parent_profile_sha256: descriptor.parent_profile_sha256,
        candidate_profile_sha256: descriptor.candidate_profile_sha256,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryRequest {
    pub request_id: u64,
    pub applicant_account: u32,
    pub executor_account: u32,
    pub asset_id: u32,
    pub expected_geometry_revision: u64,
    pub expected_asset_revision: u64,
    pub descriptor_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum GeometryApprovalRole {
    OwnerConsent,
    AuthorityPermit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryApproval {
    pub request_id: u64,
    pub request_sha256: [u8; 32],
    pub approver_account: u32,
    pub role: GeometryApprovalRole,
    pub approved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SpatialGeometryCommand {
    Request {
        expected_state_revision: u64,
        request_id: u64,
        executor_account: u32,
        asset_id: u32,
        expected_geometry_revision: u64,
        expected_asset_revision: u64,
        descriptor_sha256: [u8; 32],
    },
    Approve {
        expected_state_revision: u64,
        request_id: u64,
        descriptor_sha256: [u8; 32],
        approved: bool,
    },
    Withdraw {
        expected_state_revision: u64,
        request_id: u64,
        descriptor_sha256: [u8; 32],
    },
    Apply {
        expected_state_revision: u64,
        request_id: u64,
        descriptor_sha256: [u8; 32],
        candidate_profile_sha256: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryRequestRecord {
    pub request: GeometryRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedGeometryGrant {
    request: GeometryRequest,
    owner_approval: GeometryApproval,
    authority_approval: GeometryApproval,
    request_sha256: [u8; 32],
    world_id: u64,
    branch_id: u64,
    parent_profile_sha256: [u8; 32],
    candidate_profile_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialGeometryAuthorizationWitness {
    pub request: GeometryRequest,
    pub owner_approval: GeometryApproval,
    pub authority_approval: GeometryApproval,
}

impl VerifiedGeometryGrant {
    pub fn request(self) -> GeometryRequest {
        self.request
    }
    pub fn owner_approval(self) -> GeometryApproval {
        self.owner_approval
    }
    pub fn authority_approval(self) -> GeometryApproval {
        self.authority_approval
    }
    pub fn request_sha256(self) -> [u8; 32] {
        self.request_sha256
    }
    pub fn world_id(self) -> u64 {
        self.world_id
    }
    pub fn branch_id(self) -> u64 {
        self.branch_id
    }
    pub fn asset_id(self) -> u32 {
        self.request.asset_id
    }
    pub fn expected_asset_revision(self) -> u64 {
        self.request.expected_asset_revision
    }
    pub fn parent_profile_sha256(self) -> [u8; 32] {
        self.parent_profile_sha256
    }
    pub fn descriptor_sha256(self) -> [u8; 32] {
        self.request.descriptor_sha256
    }
    pub fn candidate_profile_sha256(self) -> [u8; 32] {
        self.candidate_profile_sha256
    }
    pub fn witness(self) -> SpatialGeometryAuthorizationWitness {
        SpatialGeometryAuthorizationWitness {
            request: self.request,
            owner_approval: self.owner_approval,
            authority_approval: self.authority_approval,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentity,
    InvalidDescriptor,
    InvalidRequest,
    Unauthorized,
    StaleRevision,
    Capacity,
    Overflow,
    Encoding,
    MissingRequest,
    MissingApproval,
    InvalidBoundary,
}

/// Sparse, lazily activated authorization state. No row storage exists in a legacy world until
/// its first geometry request; bounds are enforced on every decode and transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialGeometryState {
    pub schema: u16,
    pub world_id: u64,
    pub branch_id: u64,
    pub revision: u64,
    pub request_id_highwater: u64,
    #[serde(deserialize_with = "request_rows")]
    requests: Vec<GeometryRequestRecord>,
    #[serde(deserialize_with = "approval_rows")]
    approvals: Vec<GeometryApproval>,
}

impl SpatialGeometryState {
    pub fn new(world_id: u64, branch_id: u64) -> Result<Self, Error> {
        if world_id == 0 || branch_id == 0 {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self {
            schema: SPATIAL_GEOMETRY_SCHEMA,
            world_id,
            branch_id,
            revision: 0,
            request_id_highwater: 0,
            requests: Vec::new(),
            approvals: Vec::new(),
        })
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != SPATIAL_GEOMETRY_SCHEMA
            || self.world_id == 0
            || self.branch_id == 0
            || self.requests.len() > MAX_GEOMETRY_REQUESTS
            || self.approvals.len() > MAX_GEOMETRY_APPROVALS
        {
            return Err(Error::InvalidIdentity);
        }
        for (index, entry) in self.requests.iter().enumerate() {
            let req = entry.request;
            if req.request_id == 0
                || req.request_id > self.request_id_highwater
                || req.applicant_account == 0
                || req.executor_account == 0
                || req.asset_id == 0
                || req.expected_geometry_revision == 0
                || req.expected_asset_revision == 0
                || req.descriptor_sha256 == [0; 32]
                || req.applicant_account == req.executor_account
                || self.requests[..index]
                    .iter()
                    .any(|prior| prior.request.request_id == req.request_id)
            {
                return Err(Error::InvalidRequest);
            }
        }
        for (index, approval) in self.approvals.iter().enumerate() {
            if approval.request_id == 0
                || approval.request_sha256 == [0; 32]
                || approval.approver_account == 0
                || self.approvals[..index].iter().any(|prior| {
                    prior.request_id == approval.request_id && prior.role == approval.role
                })
                || self
                    .requests
                    .iter()
                    .all(|request| request.request.request_id != approval.request_id)
            {
                return Err(Error::InvalidRequest);
            }
        }
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn active_request_count(&self) -> usize {
        self.requests.len()
    }

    pub fn requests(&self) -> &[GeometryRequestRecord] {
        &self.requests
    }

    pub fn validate_against(&self, permissions: &SpatialPermissionState) -> Result<(), Error> {
        self.validate()?;
        if self.world_id != permissions.world_id || self.branch_id != permissions.branch_id {
            return Err(Error::InvalidIdentity);
        }
        for entry in &self.requests {
            let request = entry.request;
            let asset = permissions
                .assets
                .iter()
                .find(|asset| asset.asset_id == request.asset_id)
                .ok_or(Error::InvalidRequest)?;
            if request.applicant_account == asset.permit_authority_account
                || request.executor_account == asset.owner_account
                || request.executor_account == asset.permit_authority_account
            {
                return Err(Error::InvalidRequest);
            }
        }
        for approval in &self.approvals {
            let request = self
                .requests
                .iter()
                .find(|entry| entry.request.request_id == approval.request_id)
                .ok_or(Error::InvalidRequest)?
                .request;
            let asset = permissions
                .assets
                .iter()
                .find(|asset| asset.asset_id == request.asset_id)
                .ok_or(Error::InvalidRequest)?;
            let expected_account = match approval.role {
                GeometryApprovalRole::OwnerConsent => asset.owner_account,
                GeometryApprovalRole::AuthorityPermit => asset.permit_authority_account,
            };
            if approval.approver_account != expected_account
                || approval.approver_account == request.applicant_account
                || approval.approver_account == request.executor_account
                || approval.request_sha256 != self.request_sha256(&request)?
            {
                return Err(Error::InvalidRequest);
            }
        }
        Ok(())
    }

    pub fn request_sha256(&self, request: &GeometryRequest) -> Result<[u8; 32], Error> {
        if request.request_id == 0 || request.descriptor_sha256 == [0; 32] {
            return Err(Error::InvalidRequest);
        }
        let body = serde_json::to_vec(request).map_err(|_| Error::Encoding)?;
        let mut digest = Sha256::new();
        digest.update(b"mycity-spatial-geometry-request-v1\0");
        digest.update(self.world_id.to_le_bytes());
        digest.update(self.branch_id.to_le_bytes());
        digest.update(body);
        Ok(digest.finalize().into())
    }

    pub fn apply(
        &mut self,
        actor_account: u32,
        command: SpatialGeometryCommand,
        permissions: &SpatialPermissionState,
    ) -> Result<Option<VerifiedGeometryGrant>, Error> {
        if actor_account == 0 || self.validate().is_err() || permissions.validate().is_err() {
            return Err(Error::InvalidIdentity);
        }
        if self.world_id != permissions.world_id || self.branch_id != permissions.branch_id {
            return Err(Error::InvalidIdentity);
        }
        let expected = match command {
            SpatialGeometryCommand::Request {
                expected_state_revision,
                ..
            }
            | SpatialGeometryCommand::Approve {
                expected_state_revision,
                ..
            }
            | SpatialGeometryCommand::Withdraw {
                expected_state_revision,
                ..
            }
            | SpatialGeometryCommand::Apply {
                expected_state_revision,
                ..
            } => expected_state_revision,
        };
        if expected != self.revision {
            return Err(Error::StaleRevision);
        }
        let next_revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        match command {
            SpatialGeometryCommand::Request {
                request_id,
                executor_account,
                asset_id,
                expected_geometry_revision,
                expected_asset_revision,
                descriptor_sha256,
                ..
            } => {
                let asset = permissions
                    .assets
                    .iter()
                    .find(|asset| asset.asset_id == asset_id)
                    .ok_or(Error::InvalidRequest)?;
                if request_id == 0
                    || request_id <= self.request_id_highwater
                    || executor_account == 0
                    || actor_account == executor_account
                    || actor_account == asset.permit_authority_account
                    || executor_account == asset.permit_authority_account
                    || asset.kind != crate::spatial_transport_permissions::SpatialAssetKind::Road
                    || asset.revision != expected_asset_revision
                    || expected_geometry_revision == 0
                    || descriptor_sha256 == [0; 32]
                    || self.requests.len() >= MAX_GEOMETRY_REQUESTS
                    || self.approvals.len() + 2 > MAX_GEOMETRY_APPROVALS
                {
                    return Err(Error::InvalidRequest);
                }
                let request = GeometryRequest {
                    request_id,
                    applicant_account: actor_account,
                    executor_account,
                    asset_id,
                    expected_geometry_revision,
                    expected_asset_revision,
                    descriptor_sha256,
                };
                self.requests.push(GeometryRequestRecord { request });
                self.request_id_highwater = request_id;
                self.revision = next_revision;
                Ok(None)
            }
            SpatialGeometryCommand::Approve {
                request_id,
                descriptor_sha256,
                approved,
                ..
            } => {
                let request = self.find_request(request_id, descriptor_sha256)?;
                let asset = permissions
                    .assets
                    .iter()
                    .find(|asset| asset.asset_id == request.asset_id)
                    .ok_or(Error::InvalidRequest)?;
                let role = if actor_account == asset.owner_account {
                    GeometryApprovalRole::OwnerConsent
                } else if actor_account == asset.permit_authority_account {
                    GeometryApprovalRole::AuthorityPermit
                } else {
                    return Err(Error::Unauthorized);
                };
                if actor_account == request.applicant_account
                    || actor_account == request.executor_account
                    || self.approvals.len() >= MAX_GEOMETRY_APPROVALS
                    || self
                        .approvals
                        .iter()
                        .any(|entry| entry.request_id == request_id && entry.role == role)
                {
                    return Err(Error::Unauthorized);
                }
                self.approvals.push(GeometryApproval {
                    request_id,
                    request_sha256: self.request_sha256(&request)?,
                    approver_account: actor_account,
                    role,
                    approved,
                });
                self.revision = next_revision;
                Ok(None)
            }
            SpatialGeometryCommand::Withdraw {
                request_id,
                descriptor_sha256,
                ..
            } => {
                let request = self.find_request(request_id, descriptor_sha256)?;
                let asset = permissions
                    .assets
                    .iter()
                    .find(|asset| asset.asset_id == request.asset_id)
                    .ok_or(Error::InvalidRequest)?;
                if actor_account != request.applicant_account
                    && actor_account != asset.owner_account
                    && actor_account != asset.permit_authority_account
                {
                    return Err(Error::Unauthorized);
                }
                self.remove_request(request_id);
                self.revision = next_revision;
                Ok(None)
            }
            SpatialGeometryCommand::Apply {
                request_id,
                descriptor_sha256,
                candidate_profile_sha256,
                ..
            } => {
                let request = self.find_request(request_id, descriptor_sha256)?;
                let asset = permissions
                    .assets
                    .iter()
                    .find(|asset| asset.asset_id == request.asset_id)
                    .ok_or(Error::InvalidRequest)?;
                if actor_account != request.executor_account
                    || asset.revision != request.expected_asset_revision
                {
                    return Err(Error::Unauthorized);
                }
                let digest = self.request_sha256(&request)?;
                let owner = self
                    .approvals
                    .iter()
                    .find(|entry| {
                        entry.request_id == request_id
                            && entry.request_sha256 == digest
                            && entry.approver_account == asset.owner_account
                            && entry.role == GeometryApprovalRole::OwnerConsent
                    })
                    .copied()
                    .ok_or(Error::MissingApproval)?;
                let authority = self
                    .approvals
                    .iter()
                    .find(|entry| {
                        entry.request_id == request_id
                            && entry.request_sha256 == digest
                            && entry.approver_account == asset.permit_authority_account
                            && entry.role == GeometryApprovalRole::AuthorityPermit
                    })
                    .copied()
                    .ok_or(Error::MissingApproval)?;
                if !owner.approved
                    || !authority.approved
                    || owner.approver_account == authority.approver_account
                    || authority.approver_account == request.applicant_account
                    || authority.approver_account == request.executor_account
                    || candidate_profile_sha256 == [0; 32]
                {
                    return Err(Error::Unauthorized);
                }
                self.remove_request(request_id);
                self.revision = next_revision;
                Ok(Some(VerifiedGeometryGrant {
                    request,
                    owner_approval: owner,
                    authority_approval: authority,
                    request_sha256: digest,
                    world_id: self.world_id,
                    branch_id: self.branch_id,
                    parent_profile_sha256: asset.current_profile_sha256,
                    candidate_profile_sha256,
                }))
            }
        }
    }

    fn find_request(&self, id: u64, descriptor: [u8; 32]) -> Result<GeometryRequest, Error> {
        self.requests
            .iter()
            .find(|entry| {
                entry.request.request_id == id && entry.request.descriptor_sha256 == descriptor
            })
            .map(|entry| entry.request)
            .ok_or(Error::MissingRequest)
    }

    fn remove_request(&mut self, id: u64) {
        self.requests.retain(|entry| entry.request.request_id != id);
        self.approvals.retain(|entry| entry.request_id != id);
    }

    pub const fn max_owned_rows_bytes() -> usize {
        MAX_GEOMETRY_REQUESTS * size_of::<GeometryRequestRecord>()
            + MAX_GEOMETRY_APPROVALS * size_of::<GeometryApproval>()
    }

    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.owned_rows_bytes()
    }

    /// Heap rows owned by this state. The inline `Option<SpatialGeometryState>` header is
    /// already included in Engine/Prepared `size_of` accounting.
    pub fn owned_rows_bytes(&self) -> usize {
        self.requests.capacity() * size_of::<GeometryRequestRecord>()
            + self.approvals.capacity() * size_of::<GeometryApproval>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial_transport_permissions::{
        SpatialAccessEdge, SpatialAccessSurface, SpatialAssetKind, SpatialAssetState,
    };

    fn fixture() -> (SpatialGeometryState, SpatialPermissionState) {
        let permissions = SpatialPermissionState::from_genesis(
            10,
            20,
            vec![SpatialAssetState {
                asset_id: 4,
                kind: SpatialAssetKind::Road,
                owner_account: 2,
                permit_authority_account: 3,
                source_scene_sha256: [7; 32],
                revision: 1,
                current_profile_sha256: [8; 32],
                edges: vec![SpatialAccessEdge {
                    edge_id: 5,
                    surface: SpatialAccessSurface::Road,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: 1,
                }],
            }],
        )
        .unwrap();
        (SpatialGeometryState::new(10, 20).unwrap(), permissions)
    }

    #[test]
    fn geometry_transport_and_asset_revisions_are_independent() {
        let permissions = SpatialPermissionState::from_genesis(
            10,
            20,
            vec![SpatialAssetState {
                asset_id: 4,
                kind: SpatialAssetKind::Road,
                owner_account: 2,
                permit_authority_account: 3,
                source_scene_sha256: [7; 32],
                revision: 3,
                current_profile_sha256: [8; 32],
                edges: vec![SpatialAccessEdge {
                    edge_id: 5,
                    surface: SpatialAccessSurface::Road,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: 1,
                }],
            }],
        )
        .unwrap();
        let mut state = SpatialGeometryState::new(10, 20).unwrap();
        let descriptor = GeometryRevisionDescriptor {
            schema: SPATIAL_GEOMETRY_SCHEMA,
            world_id: 10,
            branch_id: 20,
            asset_id: 4,
            parent_geometry_revision: 5,
            parent_scene_input_sha256: [11; 32],
            parent_scene_state_sha256: [12; 32],
            parent_transport_revision: 17,
            parent_profile_sha256: [8; 32],
            adapter_basis_sha256: [13; 32],
            candidate_geometry_revision: 6,
            candidate_scene_input_sha256: [14; 32],
            candidate_scene_state_sha256: [15; 32],
            candidate_transport_revision: 18,
            candidate_profile_sha256: [10; 32],
            effective_service_day: 1,
        };
        let descriptor_sha256 = descriptor.sha256().unwrap();
        state
            .apply(
                1,
                SpatialGeometryCommand::Request {
                    expected_state_revision: 0,
                    request_id: 1,
                    executor_account: 4,
                    asset_id: 4,
                    expected_geometry_revision: 5,
                    expected_asset_revision: 3,
                    descriptor_sha256,
                },
                &permissions,
            )
            .unwrap();
        state
            .apply(
                2,
                SpatialGeometryCommand::Approve {
                    expected_state_revision: 1,
                    request_id: 1,
                    descriptor_sha256,
                    approved: true,
                },
                &permissions,
            )
            .unwrap();
        state
            .apply(
                3,
                SpatialGeometryCommand::Approve {
                    expected_state_revision: 2,
                    request_id: 1,
                    descriptor_sha256,
                    approved: true,
                },
                &permissions,
            )
            .unwrap();
        let grant = state
            .apply(
                4,
                SpatialGeometryCommand::Apply {
                    expected_state_revision: 3,
                    request_id: 1,
                    descriptor_sha256,
                    candidate_profile_sha256: [10; 32],
                },
                &permissions,
            )
            .unwrap()
            .unwrap();

        let mut mismatched = descriptor;
        mismatched.adapter_basis_sha256 = [99; 32];
        assert!(validate_authorization_witness(&mismatched, grant.witness(), &permissions).is_err());
        let verified =
            validate_authorization_witness(&descriptor, grant.witness(), &permissions).unwrap();
        assert_eq!(verified.expected_asset_revision(), 3);
        assert_eq!(verified.world_id(), 10);
        assert_eq!(verified.branch_id(), 20);
        let mut updated = permissions;
        updated.apply_geometry_profile_revision(verified).unwrap();
        assert_eq!(updated.assets()[0].revision, 4);
        assert_eq!(updated.assets()[0].current_profile_sha256, [10; 32]);
    }

    #[test]
    fn selector_binds_exact_descriptor_and_requires_independent_roles() {
        let (mut state, permissions) = fixture();
        let request = SpatialGeometryCommand::Request {
            expected_state_revision: 0,
            request_id: 1,
            executor_account: 4,
            asset_id: 4,
            expected_geometry_revision: 1,
            expected_asset_revision: 1,
            descriptor_sha256: [9; 32],
        };
        assert!(state.apply(1, request, &permissions).unwrap().is_none());
        assert!(state
            .apply(
                2,
                SpatialGeometryCommand::Approve {
                    expected_state_revision: 1,
                    request_id: 1,
                    descriptor_sha256: [10; 32],
                    approved: true,
                },
                &permissions
            )
            .is_err());
        state
            .apply(
                2,
                SpatialGeometryCommand::Approve {
                    expected_state_revision: 1,
                    request_id: 1,
                    descriptor_sha256: [9; 32],
                    approved: true,
                },
                &permissions,
            )
            .unwrap();
        state
            .apply(
                3,
                SpatialGeometryCommand::Approve {
                    expected_state_revision: 2,
                    request_id: 1,
                    descriptor_sha256: [9; 32],
                    approved: true,
                },
                &permissions,
            )
            .unwrap();
        let grant = state
            .apply(
                4,
                SpatialGeometryCommand::Apply {
                    expected_state_revision: 3,
                    request_id: 1,
                    descriptor_sha256: [9; 32],
                    candidate_profile_sha256: [11; 32],
                },
                &permissions,
            )
            .unwrap()
            .unwrap();
        assert_eq!(grant.descriptor_sha256(), [9; 32]);
        assert!(state.requests().is_empty());
    }

    #[test]
    fn descriptor_requires_both_parent_chains_and_monotone_next_versions() {
        let descriptor = GeometryRevisionDescriptor {
            schema: 1,
            world_id: 10,
            branch_id: 20,
            asset_id: 4,
            parent_geometry_revision: 1,
            parent_scene_input_sha256: [1; 32],
            parent_scene_state_sha256: [2; 32],
            parent_transport_revision: 2,
            parent_profile_sha256: [3; 32],
            adapter_basis_sha256: [11; 32],
            candidate_geometry_revision: 2,
            candidate_scene_input_sha256: [4; 32],
            candidate_scene_state_sha256: [5; 32],
            candidate_transport_revision: 3,
            candidate_profile_sha256: [6; 32],
            effective_service_day: 3,
        };
        assert!(descriptor.sha256().is_ok());
        let mut stale = descriptor;
        stale.parent_profile_sha256 = [7; 32];
        stale.candidate_geometry_revision = 1;
        assert_eq!(stale.validate(), Err(Error::InvalidDescriptor));
    }
}
