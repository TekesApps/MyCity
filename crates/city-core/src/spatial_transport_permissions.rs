//! Bounded owner/permit approvals for a narrow spatial road/door access update.
//!
//! This is state and validation logic only. The authoritative Engine writer must run it on the
//! same prepared state as the profile-version publication and commit both together. This module
//! has no clock, journal, filesystem, or legal-system integration of its own.
use crate::units::GameTime;
use serde::{de, de::DeserializeSeed, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const MAX_SPATIAL_ASSETS: usize = 8;
pub const MAX_SPATIAL_REQUESTS: usize = 8;
pub const MAX_SPATIAL_APPROVALS: usize = 16;
pub const MAX_SPATIAL_EDGES_PER_ASSET: usize = 8;
pub const SPATIAL_PERMISSION_SCHEMA: u16 = 1;
pub const SPATIAL_ALLOW_CLOSE: u8 = 1;
pub const SPATIAL_ALLOW_RESTORE: u8 = 2;
const SPATIAL_ALLOWED_OPERATION_BITS: u8 = SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE;

fn bounded_vec<'de, D, T, const N: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> de::Visitor<'de> for Bounded<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "at most {N} spatial permission records")
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut rows = Vec::new();
            rows.try_reserve_exact(N)
                .map_err(|_| de::Error::custom("spatial_permission_allocation"))?;
            for _ in 0..N {
                match seq.next_element()? {
                    Some(value) => rows.push(value),
                    None => return Ok(rows),
                }
            }
            struct Reject;
            impl<'de> de::DeserializeSeed<'de> for Reject {
                type Value = ();
                fn deserialize<D2: Deserializer<'de>>(self, _: D2) -> Result<(), D2::Error> {
                    Err(de::Error::custom("spatial_permission_limit"))
                }
            }
            seq.next_element_seed(Reject)?;
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(Bounded::<T, N>(std::marker::PhantomData))
}

fn asset_rows<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<SpatialAssetState>, D::Error> {
    bounded_vec::<D, SpatialAssetState, MAX_SPATIAL_ASSETS>(d)
}
fn request_rows<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<SpatialUpdateRequestRecord>, D::Error> {
    bounded_vec::<D, SpatialUpdateRequestRecord, MAX_SPATIAL_REQUESTS>(d)
}
fn approval_rows<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<SpatialUpdateApproval>, D::Error> {
    bounded_vec::<D, SpatialUpdateApproval, MAX_SPATIAL_APPROVALS>(d)
}
fn edge_rows<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<SpatialAccessEdge>, D::Error> {
    bounded_vec::<D, SpatialAccessEdge, MAX_SPATIAL_EDGES_PER_ASSET>(d)
}

struct FixedRowsSeed<'a, T, const N: usize>(&'a mut Vec<T>);
impl<'de, T: Deserialize<'de>, const N: usize> de::DeserializeSeed<'de>
    for FixedRowsSeed<'_, T, N>
{
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        struct Rows<'a, T, const N: usize>(&'a mut Vec<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> de::Visitor<'de> for Rows<'_, T, N> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "at most {N} bounded rows")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
                self.0.clear();
                for _ in 0..N {
                    match seq.next_element()? {
                        Some(row) => self.0.push(row),
                        None => return Ok(()),
                    }
                }
                struct Reject;
                impl<'de> de::DeserializeSeed<'de> for Reject {
                    type Value = ();
                    fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
                        Err(de::Error::custom("spatial_permission_limit"))
                    }
                }
                seq.next_element_seed(Reject)?;
                Ok(())
            }
        }
        deserializer.deserialize_seq(Rows::<T, N>(self.0))
    }
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum AssetField {
    AssetId,
    Kind,
    OwnerAccount,
    PermitAuthorityAccount,
    SourceSceneSha256,
    Revision,
    CurrentProfileSha256,
    Edges,
}

struct AssetFieldsSeed<'a> {
    edge_rows: &'a mut Vec<SpatialAccessEdge>,
}
impl<'de> de::DeserializeSeed<'de> for AssetFieldsSeed<'_> {
    type Value = SpatialAssetState;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        struct AssetVisitor<'a> {
            edge_rows: &'a mut Vec<SpatialAccessEdge>,
        }
        impl<'de> de::Visitor<'de> for AssetVisitor<'_> {
            type Value = SpatialAssetState;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded spatial asset row")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut asset_id = None;
                let mut kind = None;
                let mut owner_account = None;
                let mut permit_authority_account = None;
                let mut source_scene_sha256 = None;
                let mut revision = None;
                let mut current_profile_sha256 = None;
                let mut saw_edges = false;
                while let Some(field) = map.next_key::<AssetField>()? {
                    match field {
                        AssetField::AssetId => {
                            if asset_id.is_some() {
                                return Err(de::Error::duplicate_field("asset_id"));
                            }
                            asset_id = Some(map.next_value()?);
                        }
                        AssetField::Kind => {
                            if kind.is_some() {
                                return Err(de::Error::duplicate_field("kind"));
                            }
                            kind = Some(map.next_value()?);
                        }
                        AssetField::OwnerAccount => {
                            if owner_account.is_some() {
                                return Err(de::Error::duplicate_field("owner_account"));
                            }
                            owner_account = Some(map.next_value()?);
                        }
                        AssetField::PermitAuthorityAccount => {
                            if permit_authority_account.is_some() {
                                return Err(de::Error::duplicate_field("permit_authority_account"));
                            }
                            permit_authority_account = Some(map.next_value()?)
                        }
                        AssetField::SourceSceneSha256 => {
                            if source_scene_sha256.is_some() {
                                return Err(de::Error::duplicate_field("source_scene_sha256"));
                            }
                            source_scene_sha256 = Some(map.next_value()?)
                        }
                        AssetField::Revision => {
                            if revision.is_some() {
                                return Err(de::Error::duplicate_field("revision"));
                            }
                            revision = Some(map.next_value()?);
                        }
                        AssetField::CurrentProfileSha256 => {
                            if current_profile_sha256.is_some() {
                                return Err(de::Error::duplicate_field("current_profile_sha256"));
                            }
                            current_profile_sha256 = Some(map.next_value()?)
                        }
                        AssetField::Edges => {
                            if saw_edges {
                                return Err(de::Error::duplicate_field("edges"));
                            }
                            saw_edges = true;
                            map.next_value_seed(FixedRowsSeed::<_, MAX_SPATIAL_EDGES_PER_ASSET>(
                                self.edge_rows,
                            ))?;
                        }
                    }
                }
                if !saw_edges {
                    return Err(de::Error::missing_field("edges"));
                }
                Ok(SpatialAssetState {
                    asset_id: asset_id.ok_or_else(|| de::Error::missing_field("asset_id"))?,
                    kind: kind.ok_or_else(|| de::Error::missing_field("kind"))?,
                    owner_account: owner_account
                        .ok_or_else(|| de::Error::missing_field("owner_account"))?,
                    permit_authority_account: permit_authority_account
                        .ok_or_else(|| de::Error::missing_field("permit_authority_account"))?,
                    source_scene_sha256: source_scene_sha256
                        .ok_or_else(|| de::Error::missing_field("source_scene_sha256"))?,
                    revision: revision.ok_or_else(|| de::Error::missing_field("revision"))?,
                    current_profile_sha256: current_profile_sha256
                        .ok_or_else(|| de::Error::missing_field("current_profile_sha256"))?,
                    edges: Vec::new(),
                })
            }
        }
        deserializer.deserialize_map(AssetVisitor {
            edge_rows: self.edge_rows,
        })
    }
}

struct AssetRowsSeed<'a> {
    assets: &'a mut Vec<SpatialAssetState>,
    backing: &'a mut Vec<Vec<SpatialAccessEdge>>,
}
impl<'de> de::DeserializeSeed<'de> for AssetRowsSeed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        struct Assets<'a> {
            rows: &'a mut Vec<SpatialAssetState>,
            backing: &'a mut Vec<Vec<SpatialAccessEdge>>,
        }
        impl<'de> de::Visitor<'de> for Assets<'_> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "at most {MAX_SPATIAL_ASSETS} spatial assets")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
                self.rows.clear();
                for _ in 0..MAX_SPATIAL_ASSETS {
                    let Some(mut edges) = self.backing.pop() else {
                        return Err(de::Error::custom("spatial_permission_backing"));
                    };
                    edges.clear();
                    match seq.next_element_seed(AssetFieldsSeed {
                        edge_rows: &mut edges,
                    }) {
                        Ok(Some(mut asset)) => {
                            asset.edges = edges;
                            self.rows.push(asset);
                        }
                        Ok(None) => {
                            self.backing.push(edges);
                            return Ok(());
                        }
                        Err(error) => {
                            edges.clear();
                            self.backing.push(edges);
                            return Err(error);
                        }
                    }
                }
                struct Reject;
                impl<'de> de::DeserializeSeed<'de> for Reject {
                    type Value = ();
                    fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
                        Err(de::Error::custom("spatial_permission_limit"))
                    }
                }
                seq.next_element_seed(Reject)?;
                Ok(())
            }
        }
        deserializer.deserialize_seq(Assets {
            rows: self.assets,
            backing: self.backing,
        })
    }
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum StateField {
    Schema,
    WorldId,
    BranchId,
    Revision,
    RequestIdHighwater,
    Assets,
    Requests,
    Approvals,
}

struct RestoreStateSeed<'a>(&'a mut SpatialPermissionState);
impl<'de> de::DeserializeSeed<'de> for RestoreStateSeed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        struct StateVisitor<'a>(&'a mut SpatialPermissionState);
        impl<'de> de::Visitor<'de> for StateVisitor<'_> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded spatial permission state")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
                let mut schema = None;
                let mut world_id = None;
                let mut branch_id = None;
                let mut revision = None;
                let mut request_id_highwater = None;
                let mut saw_assets = false;
                let mut saw_requests = false;
                let mut saw_approvals = false;
                while let Some(field) = map.next_key::<StateField>()? {
                    match field {
                        StateField::Schema => {
                            if schema.is_some() {
                                return Err(de::Error::duplicate_field("schema"));
                            }
                            schema = Some(map.next_value()?);
                        }
                        StateField::WorldId => {
                            if world_id.is_some() {
                                return Err(de::Error::duplicate_field("world_id"));
                            }
                            world_id = Some(map.next_value()?);
                        }
                        StateField::BranchId => {
                            if branch_id.is_some() {
                                return Err(de::Error::duplicate_field("branch_id"));
                            }
                            branch_id = Some(map.next_value()?);
                        }
                        StateField::Revision => {
                            if revision.is_some() {
                                return Err(de::Error::duplicate_field("revision"));
                            }
                            revision = Some(map.next_value()?);
                        }
                        StateField::RequestIdHighwater => {
                            if request_id_highwater.is_some() {
                                return Err(de::Error::duplicate_field("request_id_highwater"));
                            }
                            request_id_highwater = Some(map.next_value()?)
                        }
                        StateField::Assets => {
                            if saw_assets {
                                return Err(de::Error::duplicate_field("assets"));
                            }
                            saw_assets = true;
                            map.next_value_seed(AssetRowsSeed {
                                assets: &mut self.0.assets,
                                backing: &mut self.0.asset_edge_backing,
                            })?;
                        }
                        StateField::Requests => {
                            if saw_requests {
                                return Err(de::Error::duplicate_field("requests"));
                            }
                            saw_requests = true;
                            map.next_value_seed(FixedRowsSeed::<_, MAX_SPATIAL_REQUESTS>(
                                &mut self.0.requests,
                            ))?;
                        }
                        StateField::Approvals => {
                            if saw_approvals {
                                return Err(de::Error::duplicate_field("approvals"));
                            }
                            saw_approvals = true;
                            map.next_value_seed(FixedRowsSeed::<_, MAX_SPATIAL_APPROVALS>(
                                &mut self.0.approvals,
                            ))?;
                        }
                    }
                }
                self.0.schema = schema.ok_or_else(|| de::Error::missing_field("schema"))?;
                self.0.world_id = world_id.ok_or_else(|| de::Error::missing_field("world_id"))?;
                self.0.branch_id =
                    branch_id.ok_or_else(|| de::Error::missing_field("branch_id"))?;
                self.0.revision = revision.ok_or_else(|| de::Error::missing_field("revision"))?;
                self.0.request_id_highwater = request_id_highwater
                    .ok_or_else(|| de::Error::missing_field("request_id_highwater"))?;
                if !saw_assets {
                    return Err(de::Error::missing_field("assets"));
                }
                if !saw_requests {
                    return Err(de::Error::missing_field("requests"));
                }
                if !saw_approvals {
                    return Err(de::Error::missing_field("approvals"));
                }
                self.0
                    .validate()
                    .map_err(|error| de::Error::custom(format!("spatial_permission:{error:?}")))
            }
        }
        deserializer.deserialize_map(StateVisitor(self.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SpatialAccessSurface {
    Road,
    Door,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SpatialAccessOperation {
    Close,
    Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SpatialAssetKind {
    Road,
    Building,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialAccessEdge {
    pub edge_id: u32,
    pub surface: SpatialAccessSurface,
    /// Must be extracted from the exact validated initial transport profile.
    pub initially_open: bool,
    /// Current immutable profile version state, advanced only by an accepted update.
    pub currently_open: bool,
    /// Genesis-configured operation bits: `SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE`.
    pub allowed_operations: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialAssetState {
    pub asset_id: u32,
    pub kind: SpatialAssetKind,
    pub owner_account: u32,
    pub permit_authority_account: u32,
    pub source_scene_sha256: [u8; 32],
    pub revision: u64,
    pub current_profile_sha256: [u8; 32],
    #[serde(deserialize_with = "edge_rows")]
    pub edges: Vec<SpatialAccessEdge>,
}

/// Authority context for permission digests, derived only from immutable genesis-bound state.
/// It is deliberately omitted from requests so callers cannot choose the trust context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialAuthorizationContext {
    pub world_id: u64,
    pub branch_id: u64,
    pub source_scene_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialUpdateRequest {
    pub request_id: u64,
    pub applicant_account: u32,
    pub executor_account: u32,
    pub asset_id: u32,
    pub operation: SpatialAccessOperation,
    pub surface: SpatialAccessSurface,
    pub edge_id: u32,
    pub expected_revision: u64,
    /// SHA-256 of the exact encoded candidate allocation asset bytes.
    pub candidate_profile_sha256: [u8; 32],
    /// Exact requested calendar boundary, checked against the Engine's completed Timeline.
    pub effective_time: GameTime,
    /// Raw parent allocation bytes' hash, not a hash of re-serialized decoded data.
    pub parent_profile_sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialUpdateRequestRecord {
    pub request: SpatialUpdateRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SpatialApprovalRole {
    OwnerConsent,
    AuthorityPermit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialUpdateApproval {
    pub request_id: u64,
    pub request_sha256: [u8; 32],
    pub approver_account: u32,
    pub role: SpatialApprovalRole,
    pub approved: bool,
}

/// Exact bounded authorization material retained with a catalog version so historical updates
/// can be independently replayed from immutable genesis rather than trusting a digest alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialAuthorizationWitness {
    pub request: SpatialUpdateRequest,
    pub owner_approval: SpatialUpdateApproval,
    pub authority_approval: SpatialUpdateApproval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpatialPermissionCommand {
    Request {
        expected_state_revision: u64,
        request: SpatialUpdateRequest,
    },
    Approve {
        expected_state_revision: u64,
        approval: SpatialUpdateApproval,
    },
    Withdraw {
        expected_state_revision: u64,
        request_id: u64,
        request_sha256: [u8; 32],
    },
    Apply {
        expected_state_revision: u64,
        request_id: u64,
        request_sha256: [u8; 32],
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPermissionState {
    pub schema: u16,
    pub world_id: u64,
    pub branch_id: u64,
    pub revision: u64,
    /// Monotone high-water mark; terminal requests leave the active table but IDs never recur.
    pub request_id_highwater: u64,
    #[serde(deserialize_with = "asset_rows")]
    pub assets: Vec<SpatialAssetState>,
    #[serde(deserialize_with = "request_rows")]
    pub requests: Vec<SpatialUpdateRequestRecord>,
    #[serde(deserialize_with = "approval_rows")]
    pub approvals: Vec<SpatialUpdateApproval>,
    /// Spare nested edge rows. Runtime copies move these buffers into active assets and back,
    /// keeping the bounded state at a fixed allocation without cloning nested vectors.
    #[serde(skip)]
    asset_edge_backing: Vec<Vec<SpatialAccessEdge>>,
}

impl PartialEq for SpatialPermissionState {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.world_id == other.world_id
            && self.branch_id == other.branch_id
            && self.revision == other.revision
            && self.request_id_highwater == other.request_id_highwater
            && self.assets == other.assets
            && self.requests == other.requests
            && self.approvals == other.approvals
    }
}
impl Eq for SpatialPermissionState {}

fn reserve_rows<T>(rows: &mut Vec<T>, cap: usize) -> Result<(), Error> {
    rows.try_reserve_exact(cap.saturating_sub(rows.len()))
        .map_err(|_| Error::Capacity)?;
    if rows.capacity() != cap {
        return Err(Error::Capacity);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentity,
    Capacity,
    DuplicateAsset,
    MissingAsset,
    InvalidAsset,
    StaleRevision,
    InvalidRequest,
    DuplicateRequest,
    RequestMismatch,
    Unauthorized,
    DuplicateApproval,
    MissingApproval,
    Denied,
    InvalidBoundary,
    CandidateMismatch,
    AlreadyApplied,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletedDayBoundary {
    world_id: u64,
    branch_id: u64,
    time: GameTime,
}

impl CompletedDayBoundary {
    /// Call only from the unique Engine writer after checking Timeline is complete at `time`.
    pub(crate) fn from_engine_timeline(
        world_id: u64,
        branch_id: u64,
        time: GameTime,
        timeline_complete: bool,
    ) -> Result<Self, Error> {
        let seconds = u64::try_from(time.value()).map_err(|_| Error::InvalidBoundary)?;
        if world_id == 0
            || branch_id == 0
            || !timeline_complete
            || seconds == 0
            || seconds % crate::timeline::DAY != 0
        {
            return Err(Error::InvalidBoundary);
        }
        Ok(Self {
            world_id,
            branch_id,
            time,
        })
    }
}

/// Only the permission-state apply path can construct this token. Store must consume it in the
/// same prepared commit that publishes the profile asset/version-chain root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedSpatialUpdateGrant {
    world_id: u64,
    branch_id: u64,
    request_id: u64,
    asset_id: u32,
    asset_kind: SpatialAssetKind,
    operation: SpatialAccessOperation,
    surface: SpatialAccessSurface,
    edge_id: u32,
    new_revision: u64,
    effective_service_day: u64,
    parent_profile_sha256: [u8; 32],
    candidate_profile_sha256: [u8; 32],
    source_scene_sha256: [u8; 32],
    evidence_sha256: [u8; 32],
    witness: SpatialAuthorizationWitness,
}

impl VerifiedSpatialUpdateGrant {
    pub fn world_id(self) -> u64 {
        self.world_id
    }
    pub fn branch_id(self) -> u64 {
        self.branch_id
    }
    pub fn request_id(self) -> u64 {
        self.request_id
    }
    pub fn asset_id(self) -> u32 {
        self.asset_id
    }
    pub fn asset_kind(self) -> SpatialAssetKind {
        self.asset_kind
    }
    pub fn operation(self) -> SpatialAccessOperation {
        self.operation
    }
    pub fn surface(self) -> SpatialAccessSurface {
        self.surface
    }
    pub fn edge_id(self) -> u32 {
        self.edge_id
    }
    pub fn new_revision(self) -> u64 {
        self.new_revision
    }
    pub fn effective_service_day(self) -> u64 {
        self.effective_service_day
    }
    pub fn parent_profile_sha256(self) -> [u8; 32] {
        self.parent_profile_sha256
    }
    pub fn candidate_profile_sha256(self) -> [u8; 32] {
        self.candidate_profile_sha256
    }
    pub fn source_scene_sha256(self) -> [u8; 32] {
        self.source_scene_sha256
    }
    pub fn evidence_sha256(self) -> [u8; 32] {
        self.evidence_sha256
    }
    pub fn witness(self) -> SpatialAuthorizationWitness {
        self.witness
    }
}

impl SpatialPermissionState {
    /// Worst-case retained heap for this state's enforced fixed row capacities.
    pub const fn max_owned_capacity_bytes() -> usize {
        size_of::<Self>()
            + MAX_SPATIAL_ASSETS * size_of::<SpatialAssetState>()
            + MAX_SPATIAL_REQUESTS * size_of::<SpatialUpdateRequestRecord>()
            + MAX_SPATIAL_APPROVALS * size_of::<SpatialUpdateApproval>()
            + MAX_SPATIAL_ASSETS * size_of::<Vec<SpatialAccessEdge>>()
            + MAX_SPATIAL_ASSETS * MAX_SPATIAL_EDGES_PER_ASSET * size_of::<SpatialAccessEdge>()
    }

    /// Construct an inactive, fully preallocated destination for a prepared-state copy or
    /// bounded restore. It is intentionally not a valid publishable permission state until
    /// populated with `copy_into` or `restore_into_existing`.
    pub fn empty_backing() -> Result<Self, Error> {
        let mut state = Self {
            schema: SPATIAL_PERMISSION_SCHEMA,
            world_id: 0,
            branch_id: 0,
            revision: 0,
            request_id_highwater: 0,
            assets: Vec::new(),
            requests: Vec::new(),
            approvals: Vec::new(),
            asset_edge_backing: Vec::new(),
        };
        reserve_rows(&mut state.assets, MAX_SPATIAL_ASSETS)?;
        reserve_rows(&mut state.requests, MAX_SPATIAL_REQUESTS)?;
        reserve_rows(&mut state.approvals, MAX_SPATIAL_APPROVALS)?;
        reserve_rows(&mut state.asset_edge_backing, MAX_SPATIAL_ASSETS)?;
        for _ in 0..MAX_SPATIAL_ASSETS {
            let mut edges = Vec::new();
            reserve_rows(&mut edges, MAX_SPATIAL_EDGES_PER_ASSET)?;
            state.asset_edge_backing.push(edges);
        }
        Ok(state)
    }

    /// Copy logical state into an existing fixed-capacity backing without allocating.
    pub fn copy_into(&self, target: &mut Self) -> Result<(), Error> {
        self.validate()?;
        target.check_backing_capacity()?;
        target.clear_preserving_capacity();
        target.schema = self.schema;
        target.world_id = self.world_id;
        target.branch_id = self.branch_id;
        target.revision = self.revision;
        target.request_id_highwater = self.request_id_highwater;
        for source in &self.assets {
            let mut edges = target.asset_edge_backing.pop().ok_or(Error::Capacity)?;
            edges.clear();
            if edges.capacity() != MAX_SPATIAL_EDGES_PER_ASSET {
                return Err(Error::Capacity);
            }
            edges.extend_from_slice(&source.edges);
            target.assets.push(SpatialAssetState {
                asset_id: source.asset_id,
                kind: source.kind,
                owner_account: source.owner_account,
                permit_authority_account: source.permit_authority_account,
                source_scene_sha256: source.source_scene_sha256,
                revision: source.revision,
                current_profile_sha256: source.current_profile_sha256,
                edges,
            });
        }
        target.requests.extend(self.requests.iter().cloned());
        target.approvals.extend_from_slice(&self.approvals);
        Ok(())
    }

    /// Clear logical rows while returning every active nested edge buffer to the retained pool.
    pub fn clear_preserving_capacity(&mut self) {
        for asset in self.assets.drain(..) {
            let mut edges = asset.edges;
            edges.clear();
            if self.asset_edge_backing.len() < MAX_SPATIAL_ASSETS {
                self.asset_edge_backing.push(edges);
            }
        }
        self.requests.clear();
        self.approvals.clear();
        self.schema = SPATIAL_PERMISSION_SCHEMA;
        self.world_id = 0;
        self.branch_id = 0;
        self.revision = 0;
        self.request_id_highwater = 0;
    }

    fn check_backing_capacity(&self) -> Result<(), Error> {
        if self.assets.capacity() != MAX_SPATIAL_ASSETS
            || self.requests.capacity() != MAX_SPATIAL_REQUESTS
            || self.approvals.capacity() != MAX_SPATIAL_APPROVALS
            || self.asset_edge_backing.capacity() != MAX_SPATIAL_ASSETS
            || self
                .assets
                .iter()
                .any(|a| a.edges.capacity() != MAX_SPATIAL_EDGES_PER_ASSET)
            || self
                .asset_edge_backing
                .iter()
                .any(|e| e.capacity() != MAX_SPATIAL_EDGES_PER_ASSET)
            || self.assets.len() + self.asset_edge_backing.len() != MAX_SPATIAL_ASSETS
        {
            return Err(Error::Capacity);
        }
        Ok(())
    }

    /// Replace state from a bounded serialized value using retained destination backing. The
    /// wire decoder itself is capped at the fixed row limits; on malformed input this target is
    /// left inactive and its allocated rows remain reusable.
    pub fn restore_into_existing<'de, D>(&mut self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        self.check_backing_capacity()
            .map_err(|error| de::Error::custom(format!("spatial_permission_backing:{error:?}")))?;
        self.clear_preserving_capacity();
        if let Err(error) = RestoreStateSeed(self).deserialize(deserializer) {
            self.clear_preserving_capacity();
            return Err(error);
        }
        Ok(())
    }

    pub fn from_genesis(
        world_id: u64,
        branch_id: u64,
        mut assets: Vec<SpatialAssetState>,
    ) -> Result<Self, Error> {
        if world_id == 0 || branch_id == 0 || assets.is_empty() || assets.len() > MAX_SPATIAL_ASSETS
        {
            return Err(Error::InvalidIdentity);
        }
        assets.sort_by_key(|asset| asset.asset_id);
        let mut state = Self {
            schema: SPATIAL_PERMISSION_SCHEMA,
            world_id,
            branch_id,
            revision: 0,
            request_id_highwater: 0,
            assets,
            requests: Vec::new(),
            approvals: Vec::new(),
            asset_edge_backing: Vec::new(),
        };
        state.reserve_fixed_rows()?;
        state.validate()?;
        Ok(state)
    }

    pub(crate) fn reserve_fixed_rows(&mut self) -> Result<(), Error> {
        reserve_rows(&mut self.assets, MAX_SPATIAL_ASSETS)?;
        reserve_rows(&mut self.requests, MAX_SPATIAL_REQUESTS)?;
        reserve_rows(&mut self.approvals, MAX_SPATIAL_APPROVALS)?;
        for asset in &mut self.assets {
            reserve_rows(&mut asset.edges, MAX_SPATIAL_EDGES_PER_ASSET)?;
        }
        reserve_rows(&mut self.asset_edge_backing, MAX_SPATIAL_ASSETS)?;
        while self.assets.len() + self.asset_edge_backing.len() < MAX_SPATIAL_ASSETS {
            let mut edges = Vec::new();
            reserve_rows(&mut edges, MAX_SPATIAL_EDGES_PER_ASSET)?;
            self.asset_edge_backing.push(edges);
        }
        self.check_backing_capacity()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != SPATIAL_PERMISSION_SCHEMA
            || self.world_id == 0
            || self.branch_id == 0
            || self.assets.is_empty()
            || self.assets.len() > MAX_SPATIAL_ASSETS
            || self.requests.len() > MAX_SPATIAL_REQUESTS
            || self.approvals.len() > MAX_SPATIAL_APPROVALS
            || self.assets.capacity() != MAX_SPATIAL_ASSETS
            || self.requests.capacity() != MAX_SPATIAL_REQUESTS
            || self.approvals.capacity() != MAX_SPATIAL_APPROVALS
        {
            return Err(Error::InvalidIdentity);
        }
        let shared_profile_sha256 = self.assets[0].current_profile_sha256;
        let shared_scene_sha256 = self.assets[0].source_scene_sha256;
        for (index, asset) in self.assets.iter().enumerate() {
            if asset.asset_id == 0
                || asset.owner_account == 0
                || asset.permit_authority_account == 0
                || asset.source_scene_sha256 == [0; 32]
                || asset.revision == 0
                || asset.current_profile_sha256 == [0; 32]
                || asset.current_profile_sha256 != shared_profile_sha256
                || asset.source_scene_sha256 != shared_scene_sha256
                || asset.edges.is_empty()
                || asset.edges.len() > MAX_SPATIAL_EDGES_PER_ASSET
                || asset.edges.capacity() != MAX_SPATIAL_EDGES_PER_ASSET
                || self.assets[..index]
                    .iter()
                    .any(|prior| prior.asset_id == asset.asset_id)
            {
                return Err(Error::InvalidAsset);
            }
            for edge in &asset.edges {
                if matches!(
                    (asset.kind, edge.surface),
                    (SpatialAssetKind::Road, SpatialAccessSurface::Door)
                        | (SpatialAssetKind::Building, SpatialAccessSurface::Road)
                ) {
                    return Err(Error::InvalidAsset);
                }
            }
            for (edge_index, edge) in asset.edges.iter().enumerate() {
                if edge.edge_id == 0
                    || (edge.currently_open && !edge.initially_open)
                    || edge.allowed_operations == 0
                    || edge.allowed_operations & !SPATIAL_ALLOWED_OPERATION_BITS != 0
                    || asset.edges[..edge_index]
                        .iter()
                        .any(|prior| prior.edge_id == edge.edge_id)
                {
                    return Err(Error::InvalidAsset);
                }
            }
        }
        for (index, record) in self.requests.iter().enumerate() {
            let request = &record.request;
            if request.request_id == 0
                || request.request_id > self.request_id_highwater
                || request.applicant_account == 0
                || request.executor_account == 0
                || request.expected_revision == 0
                || request.candidate_profile_sha256 == [0; 32]
                || request.parent_profile_sha256 == [0; 32]
                || self.asset(request.asset_id).is_none()
                || self.requests[..index]
                    .iter()
                    .any(|prior| prior.request.request_id == request.request_id)
            {
                return Err(Error::InvalidRequest);
            }
        }
        for (index, approval) in self.approvals.iter().enumerate() {
            let request = self
                .request(approval.request_id)
                .ok_or(Error::InvalidRequest)?;
            let context = self.authorization_context(request.asset_id)?;
            if approval.approver_account == 0
                || approval.request_sha256 != request_sha256(context, request)?
                || self.approvals[..index].iter().any(|prior| {
                    prior.request_id == approval.request_id && prior.role == approval.role
                })
            {
                return Err(Error::InvalidRequest);
            }
            let asset = self.asset(request.asset_id).ok_or(Error::MissingAsset)?;
            let valid_approver = match approval.role {
                SpatialApprovalRole::OwnerConsent => {
                    approval.approver_account == asset.owner_account
                }
                SpatialApprovalRole::AuthorityPermit => {
                    approval.approver_account == asset.permit_authority_account
                        && approval.approver_account != request.applicant_account
                        && approval.approver_account != request.executor_account
                }
            };
            if !valid_approver {
                return Err(Error::Unauthorized);
            }
        }
        Ok(())
    }

    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>()
            + self.assets.capacity() * size_of::<SpatialAssetState>()
            + self.requests.capacity() * size_of::<SpatialUpdateRequestRecord>()
            + self.approvals.capacity() * size_of::<SpatialUpdateApproval>()
            + self
                .assets
                .iter()
                .map(|asset| asset.edges.capacity() * size_of::<SpatialAccessEdge>())
                .sum::<usize>()
            + self.asset_edge_backing.capacity() * size_of::<Vec<SpatialAccessEdge>>()
            + self
                .asset_edge_backing
                .iter()
                .map(|edges| edges.capacity() * size_of::<SpatialAccessEdge>())
                .sum::<usize>()
    }

    pub fn assets(&self) -> &[SpatialAssetState] {
        &self.assets
    }

    /// Derive the only acceptable request-hash context from this registered immutable asset.
    pub fn authorization_context(
        &self,
        asset_id: u32,
    ) -> Result<SpatialAuthorizationContext, Error> {
        if self.world_id == 0 || self.branch_id == 0 {
            return Err(Error::InvalidIdentity);
        }
        let asset = self.asset(asset_id).ok_or(Error::MissingAsset)?;
        if asset.source_scene_sha256 == [0; 32] {
            return Err(Error::InvalidAsset);
        }
        Ok(SpatialAuthorizationContext {
            world_id: self.world_id,
            branch_id: self.branch_id,
            source_scene_sha256: asset.source_scene_sha256,
        })
    }

    /// Replay a previously committed, fully authorized update while validating the bounded
    /// witness against the immutable genesis identity and current replay cursor.
    pub fn replay_authorized_update(
        &mut self,
        request: SpatialUpdateRequest,
        owner_approval: SpatialUpdateApproval,
        authority_approval: SpatialUpdateApproval,
        evidence_sha256: [u8; 32],
    ) -> Result<(), Error> {
        self.validate()?;
        let context = self.authorization_context(request.asset_id)?;
        let digest = request_sha256(context, &request)?;
        let asset_index = self
            .assets
            .iter()
            .position(|asset| asset.asset_id == request.asset_id)
            .ok_or(Error::MissingAsset)?;
        let asset = &self.assets[asset_index];
        if request.request_id == 0
            || request.request_id <= self.request_id_highwater
            || request.applicant_account == 0
            || request.executor_account == 0
            || request.applicant_account == request.executor_account
            || request.expected_revision != asset.revision
            || request.parent_profile_sha256 != asset.current_profile_sha256
            || request.candidate_profile_sha256 == [0; 32]
            || request.candidate_profile_sha256 == request.parent_profile_sha256
            || request.effective_time.value() <= 0
            || u64::try_from(request.effective_time.value())
                .map_or(true, |seconds| seconds % crate::timeline::DAY != 0)
            || owner_approval.request_id != request.request_id
            || owner_approval.request_sha256 != digest
            || owner_approval.role != SpatialApprovalRole::OwnerConsent
            || owner_approval.approver_account != asset.owner_account
            || !owner_approval.approved
            || authority_approval.request_id != request.request_id
            || authority_approval.request_sha256 != digest
            || authority_approval.role != SpatialApprovalRole::AuthorityPermit
            || authority_approval.approver_account != asset.permit_authority_account
            || authority_approval.approver_account == request.applicant_account
            || authority_approval.approver_account == request.executor_account
            || !authority_approval.approved
            || approval_evidence_sha256(digest, &owner_approval, &authority_approval)?
                != evidence_sha256
        {
            return Err(Error::Unauthorized);
        }
        let next_asset_revision = asset.revision.checked_add(1).ok_or(Error::Overflow)?;
        let edge = self.assets[asset_index]
            .edges
            .iter_mut()
            .find(|edge| edge.edge_id == request.edge_id)
            .ok_or(Error::InvalidAsset)?;
        if edge.surface != request.surface
            || !edge.initially_open
            || !edge.allows(request.operation)
        {
            return Err(Error::InvalidRequest);
        }
        match request.operation {
            SpatialAccessOperation::Close if edge.currently_open => edge.currently_open = false,
            SpatialAccessOperation::Restore if !edge.currently_open => edge.currently_open = true,
            _ => return Err(Error::InvalidRequest),
        }
        self.assets[asset_index].revision = next_asset_revision;
        for asset in &mut self.assets {
            asset.current_profile_sha256 = request.candidate_profile_sha256;
        }
        self.request_id_highwater = request.request_id;
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.validate()
    }

    /// Apply to a transaction draft only. Caller must commit this state with the profile asset
    /// publication atomically; discarding the draft leaves approvals and revision untouched.
    pub fn apply(
        &mut self,
        actor_account: u32,
        command: SpatialPermissionCommand,
        boundary: Option<CompletedDayBoundary>,
    ) -> Result<Option<VerifiedSpatialUpdateGrant>, Error> {
        let mut next = Self::empty_backing()?;
        self.copy_into(&mut next)?;
        let grant = next.apply_in_place(actor_account, command, boundary)?;
        std::mem::swap(self, &mut next);
        Ok(grant)
    }

    pub fn copy_and_apply(
        &self,
        actor_account: u32,
        command: SpatialPermissionCommand,
        boundary: Option<CompletedDayBoundary>,
    ) -> Result<(Self, Option<VerifiedSpatialUpdateGrant>), Error> {
        let mut next = Self::empty_backing()?;
        self.copy_into(&mut next)?;
        let grant = next.apply_in_place(actor_account, command, boundary)?;
        Ok((next, grant))
    }

    /// Copy into caller-owned prepared backing and apply there. Failed commands leave the source
    /// unchanged and return the destination to an inactive reusable state.
    pub fn copy_and_apply_into(
        &self,
        target: &mut Self,
        actor_account: u32,
        command: SpatialPermissionCommand,
        boundary: Option<CompletedDayBoundary>,
    ) -> Result<Option<VerifiedSpatialUpdateGrant>, Error> {
        self.copy_into(target)?;
        match target.apply_in_place(actor_account, command, boundary) {
            Ok(grant) => Ok(grant),
            Err(error) => {
                target.clear_preserving_capacity();
                Err(error)
            }
        }
    }

    /// Store-authorized geometry Apply advances the shared active profile reference and the
    /// targeted road asset revision. Store validates the immutable descriptor and its parent
    /// scene/profile on the same prepare path before this grant is committed; catalog replay
    /// performs that validation again from the retained descriptor and authorization witness.
    /// This narrow mutation accepts only the private verified token, never caller-supplied IDs.
    pub fn apply_geometry_profile_revision(
        &mut self,
        grant: crate::spatial_geometry_permissions::VerifiedGeometryGrant,
    ) -> Result<(), Error> {
        self.validate()?;
        let asset_id = grant.asset_id();
        let expected_revision = grant.expected_asset_revision();
        let candidate_profile_sha256 = grant.candidate_profile_sha256();
        if grant.world_id() != self.world_id
            || grant.branch_id() != self.branch_id
            || grant.descriptor_sha256() == [0; 32]
            || grant.parent_profile_sha256() == [0; 32]
            || candidate_profile_sha256 == [0; 32]
            || self
                .assets
                .iter()
                .any(|asset| asset.current_profile_sha256 != grant.parent_profile_sha256())
        {
            return Err(Error::InvalidRequest);
        }
        let index = self
            .assets
            .iter()
            .position(|asset| asset.asset_id == asset_id)
            .ok_or(Error::InvalidRequest)?;
        let asset = &self.assets[index];
        if asset.kind != SpatialAssetKind::Road || asset.revision != expected_revision {
            return Err(Error::StaleRevision);
        }
        let next_revision = asset.revision.checked_add(1).ok_or(Error::Overflow)?;
        for candidate in &mut self.assets {
            candidate.current_profile_sha256 = candidate_profile_sha256;
        }
        self.assets[index].revision = next_revision;
        self.validate()
    }

    pub fn validate_geometry_profile_transition(
        prior: &Self,
        candidate: &Self,
        grant: crate::spatial_geometry_permissions::VerifiedGeometryGrant,
    ) -> Result<(), Error> {
        prior.validate()?;
        candidate.validate()?;
        let asset_id = grant.asset_id();
        let expected_revision = grant.expected_asset_revision();
        let candidate_profile_sha256 = grant.candidate_profile_sha256();
        if prior.schema != candidate.schema
            || prior.world_id != candidate.world_id
            || prior.branch_id != candidate.branch_id
            || prior.world_id != grant.world_id()
            || prior.branch_id != grant.branch_id()
            || prior.revision != candidate.revision
            || prior.request_id_highwater != candidate.request_id_highwater
            || prior.requests != candidate.requests
            || prior.approvals != candidate.approvals
            || prior.assets.len() != candidate.assets.len()
            || candidate_profile_sha256 == [0; 32]
            || grant.descriptor_sha256() == [0; 32]
            || grant.parent_profile_sha256() == [0; 32]
        {
            return Err(Error::InvalidRequest);
        }
        if prior
            .assets
            .iter()
            .any(|asset| asset.current_profile_sha256 != grant.parent_profile_sha256())
        {
            return Err(Error::InvalidRequest);
        }
        let mut found = false;
        for (before, after) in prior.assets.iter().zip(&candidate.assets) {
            let expected_asset_revision = if before.asset_id == asset_id {
                if found
                    || before.kind != SpatialAssetKind::Road
                    || before.revision != expected_revision
                {
                    return Err(Error::InvalidRequest);
                }
                found = true;
                before.revision.checked_add(1).ok_or(Error::Overflow)?
            } else {
                before.revision
            };
            if before.asset_id != after.asset_id
                || before.kind != after.kind
                || before.owner_account != after.owner_account
                || before.permit_authority_account != after.permit_authority_account
                || before.source_scene_sha256 != after.source_scene_sha256
                || before.edges != after.edges
                || after.revision != expected_asset_revision
                || after.current_profile_sha256 != candidate_profile_sha256
            {
                return Err(Error::InvalidRequest);
            }
        }
        if !found {
            return Err(Error::InvalidRequest);
        }
        Ok(())
    }

    fn apply_in_place(
        &mut self,
        actor_account: u32,
        command: SpatialPermissionCommand,
        boundary: Option<CompletedDayBoundary>,
    ) -> Result<Option<VerifiedSpatialUpdateGrant>, Error> {
        self.validate()?;
        match command {
            SpatialPermissionCommand::Request {
                expected_state_revision,
                request,
            } => {
                self.expect_revision(expected_state_revision)?;
                self.revision.checked_add(1).ok_or(Error::Overflow)?;
                self.add_request(actor_account, request)?;
                self.bump_revision()?;
                Ok(None)
            }
            SpatialPermissionCommand::Approve {
                expected_state_revision,
                approval,
            } => {
                self.expect_revision(expected_state_revision)?;
                self.revision.checked_add(1).ok_or(Error::Overflow)?;
                self.add_approval(actor_account, approval)?;
                self.bump_revision()?;
                Ok(None)
            }
            SpatialPermissionCommand::Withdraw {
                expected_state_revision,
                request_id,
                request_sha256,
            } => {
                self.expect_revision(expected_state_revision)?;
                self.revision.checked_add(1).ok_or(Error::Overflow)?;
                self.withdraw(actor_account, request_id, request_sha256)?;
                self.bump_revision()?;
                Ok(None)
            }
            SpatialPermissionCommand::Apply {
                expected_state_revision,
                request_id,
                request_sha256,
            } => {
                self.expect_revision(expected_state_revision)?;
                let boundary = boundary.ok_or(Error::InvalidBoundary)?;
                self.apply_approved(actor_account, request_id, request_sha256, boundary)
            }
        }
    }

    fn expect_revision(&self, expected: u64) -> Result<(), Error> {
        if self.revision == expected {
            Ok(())
        } else {
            Err(Error::StaleRevision)
        }
    }

    fn bump_revision(&mut self) -> Result<(), Error> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        Ok(())
    }

    fn add_request(&mut self, actor: u32, request: SpatialUpdateRequest) -> Result<(), Error> {
        let asset = self.asset(request.asset_id).ok_or(Error::MissingAsset)?;
        let edge = asset
            .edges
            .iter()
            .find(|edge| edge.edge_id == request.edge_id)
            .ok_or(Error::InvalidRequest)?;
        if actor == 0
            || request.request_id == 0
            || request.applicant_account != actor
            || request.executor_account == 0
            || request.executor_account == request.applicant_account
            || request.executor_account == asset.permit_authority_account
            || request.expected_revision != asset.revision
            || request.parent_profile_sha256 != asset.current_profile_sha256
            || request.candidate_profile_sha256 == [0; 32]
            || request.candidate_profile_sha256 == request.parent_profile_sha256
            || request.effective_time.value() <= 0
            || u64::try_from(request.effective_time.value())
                .map_or(true, |seconds| seconds % crate::timeline::DAY != 0)
            || edge.surface != request.surface
            || !edge.initially_open
            || !edge.allows(request.operation)
            || (request.operation == SpatialAccessOperation::Close && !edge.currently_open)
            || (request.operation == SpatialAccessOperation::Restore && edge.currently_open)
        {
            return Err(Error::InvalidRequest);
        }
        if self
            .requests
            .iter()
            .any(|prior| prior.request.request_id == request.request_id)
        {
            return Err(Error::DuplicateRequest);
        }
        if request.request_id <= self.request_id_highwater {
            return Err(Error::InvalidRequest);
        }
        if self
            .requests
            .iter()
            .any(|prior| prior.request.asset_id == request.asset_id)
        {
            return Err(Error::InvalidRequest);
        }
        if self.requests.len() == MAX_SPATIAL_REQUESTS {
            return Err(Error::Capacity);
        }
        let request_id = request.request_id;
        self.requests.push(SpatialUpdateRequestRecord { request });
        self.request_id_highwater = request_id;
        Ok(())
    }

    fn add_approval(&mut self, actor: u32, approval: SpatialUpdateApproval) -> Result<(), Error> {
        let request = self
            .request(approval.request_id)
            .ok_or(Error::InvalidRequest)?;
        let context = self.authorization_context(request.asset_id)?;
        let request_sha256 = request_sha256(context, request)?;
        let asset = self.asset(request.asset_id).ok_or(Error::MissingAsset)?;
        if actor == 0
            || actor != approval.approver_account
            || approval.request_sha256 != request_sha256
        {
            return Err(Error::RequestMismatch);
        }
        let allowed = match approval.role {
            SpatialApprovalRole::OwnerConsent => actor == asset.owner_account,
            SpatialApprovalRole::AuthorityPermit => {
                actor == asset.permit_authority_account
                    && actor != request.applicant_account
                    && actor != request.executor_account
            }
        };
        if !allowed {
            return Err(Error::Unauthorized);
        }
        if self
            .approvals
            .iter()
            .any(|prior| prior.request_id == approval.request_id && prior.role == approval.role)
        {
            return Err(Error::DuplicateApproval);
        }
        if self.approvals.len() == MAX_SPATIAL_APPROVALS {
            return Err(Error::Capacity);
        }
        self.approvals.push(approval);
        if !approval.approved {
            self.close_active_request(approval.request_id);
        }
        Ok(())
    }

    fn withdraw(
        &mut self,
        actor: u32,
        request_id: u64,
        supplied_request_sha256: [u8; 32],
    ) -> Result<(), Error> {
        let request = self.request(request_id).ok_or(Error::InvalidRequest)?;
        let context = self.authorization_context(request.asset_id)?;
        let digest = request_sha256(context, request)?;
        if actor == 0 || actor != request.applicant_account || digest != supplied_request_sha256 {
            return Err(Error::Unauthorized);
        }
        self.close_active_request(request_id);
        Ok(())
    }

    fn close_active_request(&mut self, request_id: u64) {
        self.requests
            .retain(|record| record.request.request_id != request_id);
        self.approvals
            .retain(|approval| approval.request_id != request_id);
    }

    fn apply_approved(
        &mut self,
        actor: u32,
        request_id: u64,
        supplied_request_sha256: [u8; 32],
        boundary: CompletedDayBoundary,
    ) -> Result<Option<VerifiedSpatialUpdateGrant>, Error> {
        if boundary.world_id != self.world_id || boundary.branch_id != self.branch_id {
            return Err(Error::InvalidBoundary);
        }
        let index = self
            .requests
            .iter()
            .position(|record| record.request.request_id == request_id)
            .ok_or(Error::InvalidRequest)?;
        let request = self.requests[index].request;
        let context = self.authorization_context(request.asset_id)?;
        let digest = request_sha256(context, &request)?;
        if supplied_request_sha256 != digest || actor != request.executor_account {
            return Err(Error::RequestMismatch);
        }
        if boundary.time.value() != request.effective_time.value()
            || u64::try_from(request.effective_time.value()).map_err(|_| Error::InvalidBoundary)?
                % crate::timeline::DAY
                != 0
        {
            return Err(Error::InvalidBoundary);
        }
        let asset_index = self
            .assets
            .iter()
            .position(|asset| asset.asset_id == request.asset_id)
            .ok_or(Error::MissingAsset)?;
        let asset = &self.assets[asset_index];
        if asset.revision != request.expected_revision
            || asset.current_profile_sha256 != request.parent_profile_sha256
        {
            return Err(Error::StaleRevision);
        }
        let source_scene_sha256 = asset.source_scene_sha256;
        let asset_kind = asset.kind;
        let new_revision = asset.revision.checked_add(1).ok_or(Error::Overflow)?;
        let owner_approval = self.approval_for(request_id, SpatialApprovalRole::OwnerConsent);
        let authority_approval =
            self.approval_for(request_id, SpatialApprovalRole::AuthorityPermit);
        let owner = owner_approval.copied().ok_or(Error::MissingApproval)?;
        let authority = authority_approval.copied().ok_or(Error::MissingApproval)?;
        if !owner.approved || !authority.approved {
            return Err(Error::Denied);
        }
        if owner.request_sha256 != digest || authority.request_sha256 != digest {
            return Err(Error::RequestMismatch);
        }
        let edge = self.assets[asset_index]
            .edges
            .iter_mut()
            .find(|edge| edge.edge_id == request.edge_id)
            .ok_or(Error::InvalidAsset)?;
        match request.operation {
            SpatialAccessOperation::Close
                if edge.currently_open && edge.allows(SpatialAccessOperation::Close) =>
            {
                edge.currently_open = false
            }
            SpatialAccessOperation::Restore
                if !edge.currently_open
                    && edge.initially_open
                    && edge.allows(SpatialAccessOperation::Restore) =>
            {
                edge.currently_open = true
            }
            _ => return Err(Error::InvalidRequest),
        }
        let next_state_revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.assets[asset_index].revision = new_revision;
        // Spatial allocation profiles are shared by all registered assets in this Store.
        // Applying one asset's authorized edge change advances the common profile hash while
        // only that asset's own revision increases.
        for asset in &mut self.assets {
            asset.current_profile_sha256 = request.candidate_profile_sha256;
        }
        self.revision = next_state_revision;
        let effective_service_day = u64::try_from(request.effective_time.value())
            .map_err(|_| Error::InvalidBoundary)?
            / crate::timeline::DAY;
        let evidence_sha256 = approval_evidence_sha256(digest, &owner, &authority)?;
        let grant = VerifiedSpatialUpdateGrant {
            world_id: self.world_id,
            branch_id: self.branch_id,
            request_id,
            asset_id: request.asset_id,
            asset_kind,
            operation: request.operation,
            surface: request.surface,
            edge_id: request.edge_id,
            new_revision,
            effective_service_day,
            parent_profile_sha256: request.parent_profile_sha256,
            candidate_profile_sha256: request.candidate_profile_sha256,
            source_scene_sha256,
            evidence_sha256,
            witness: SpatialAuthorizationWitness {
                request,
                owner_approval: owner,
                authority_approval: authority,
            },
        };
        self.close_active_request(request_id);
        Ok(Some(grant))
    }

    fn asset(&self, asset_id: u32) -> Option<&SpatialAssetState> {
        self.assets.iter().find(|asset| asset.asset_id == asset_id)
    }
    fn request(&self, request_id: u64) -> Option<&SpatialUpdateRequest> {
        self.requests
            .iter()
            .find(|record| record.request.request_id == request_id)
            .map(|record| &record.request)
    }
    fn approval_for(
        &self,
        request_id: u64,
        role: SpatialApprovalRole,
    ) -> Option<&SpatialUpdateApproval> {
        self.approvals
            .iter()
            .find(|approval| approval.request_id == request_id && approval.role == role)
    }
}

impl SpatialAccessEdge {
    fn allows(&self, operation: SpatialAccessOperation) -> bool {
        let bit = match operation {
            SpatialAccessOperation::Close => SPATIAL_ALLOW_CLOSE,
            SpatialAccessOperation::Restore => SPATIAL_ALLOW_RESTORE,
        };
        self.allowed_operations & bit != 0
    }
}

pub fn request_sha256(
    context: SpatialAuthorizationContext,
    request: &SpatialUpdateRequest,
) -> Result<[u8; 32], Error> {
    #[derive(Serialize)]
    struct ContextualRequest<'a> {
        domain: &'static str,
        context: SpatialAuthorizationContext,
        request: &'a SpatialUpdateRequest,
    }
    if context.world_id == 0 || context.branch_id == 0 || context.source_scene_sha256 == [0; 32] {
        return Err(Error::InvalidIdentity);
    }
    let bytes = serde_json::to_vec(&ContextualRequest {
        domain: "mycity-spatial-authorization-request-v2",
        context,
        request,
    })
    .map_err(|_| Error::InvalidRequest)?;
    Ok(Sha256::digest(bytes).into())
}

pub fn approval_evidence_sha256(
    request_sha256: [u8; 32],
    owner: &SpatialUpdateApproval,
    authority: &SpatialUpdateApproval,
) -> Result<[u8; 32], Error> {
    #[derive(Serialize)]
    struct Evidence<'a> {
        schema: u16,
        request_sha256: [u8; 32],
        owner: &'a SpatialUpdateApproval,
        authority: &'a SpatialUpdateApproval,
    }
    Ok(Sha256::digest(
        serde_json::to_vec(&Evidence {
            schema: SPATIAL_PERMISSION_SCHEMA,
            request_sha256,
            owner,
            authority,
        })
        .map_err(|_| Error::InvalidRequest)?,
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> SpatialPermissionState {
        SpatialPermissionState::from_genesis(
            1,
            1,
            vec![SpatialAssetState {
                asset_id: 7,
                kind: SpatialAssetKind::Road,
                owner_account: 10,
                permit_authority_account: 20,
                source_scene_sha256: [1; 32],
                revision: 1,
                current_profile_sha256: [2; 32],
                edges: vec![SpatialAccessEdge {
                    edge_id: 9,
                    surface: SpatialAccessSurface::Road,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
                }],
            }],
        )
        .unwrap()
    }

    fn request() -> SpatialUpdateRequest {
        SpatialUpdateRequest {
            request_id: 100,
            applicant_account: 30,
            executor_account: 40,
            asset_id: 7,
            operation: SpatialAccessOperation::Close,
            surface: SpatialAccessSurface::Road,
            edge_id: 9,
            expected_revision: 1,
            candidate_profile_sha256: [3; 32],
            effective_time: GameTime::new((2 * crate::timeline::DAY) as i64).unwrap(),
            parent_profile_sha256: [2; 32],
        }
    }

    fn maximum_state() -> SpatialPermissionState {
        let mut assets = Vec::new();
        for asset_id in 1..=MAX_SPATIAL_ASSETS as u32 {
            let mut edges = Vec::new();
            for edge_id in 1..=MAX_SPATIAL_EDGES_PER_ASSET as u32 {
                edges.push(SpatialAccessEdge {
                    edge_id,
                    surface: SpatialAccessSurface::Road,
                    initially_open: true,
                    currently_open: true,
                    allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
                });
            }
            assets.push(SpatialAssetState {
                asset_id,
                kind: SpatialAssetKind::Road,
                owner_account: 10,
                permit_authority_account: 20,
                source_scene_sha256: [1; 32],
                revision: 1,
                current_profile_sha256: [2; 32],
                edges,
            });
        }
        let mut state = SpatialPermissionState::from_genesis(1, 1, assets).unwrap();
        for asset_id in 1..=MAX_SPATIAL_REQUESTS as u32 {
            let request = SpatialUpdateRequest {
                request_id: asset_id as u64,
                applicant_account: 30,
                executor_account: 40,
                asset_id,
                operation: SpatialAccessOperation::Close,
                surface: SpatialAccessSurface::Road,
                edge_id: 1,
                expected_revision: 1,
                candidate_profile_sha256: [asset_id as u8 + 2; 32],
                effective_time: GameTime::new((2 * crate::timeline::DAY) as i64).unwrap(),
                parent_profile_sha256: [2; 32],
            };
            let digest = request_sha256(
                state.authorization_context(request.asset_id).unwrap(),
                &request,
            )
            .unwrap();
            state.requests.push(SpatialUpdateRequestRecord { request });
            state.approvals.push(SpatialUpdateApproval {
                request_id: asset_id as u64,
                request_sha256: digest,
                approver_account: 10,
                role: SpatialApprovalRole::OwnerConsent,
                approved: true,
            });
            state.approvals.push(SpatialUpdateApproval {
                request_id: asset_id as u64,
                request_sha256: digest,
                approver_account: 20,
                role: SpatialApprovalRole::AuthorityPermit,
                approved: true,
            });
        }
        state.request_id_highwater = MAX_SPATIAL_REQUESTS as u64;
        state.validate().unwrap();
        state
    }

    fn edge_backing_ptrs(state: &SpatialPermissionState) -> Vec<usize> {
        let mut pointers = state
            .assets
            .iter()
            .map(|asset| asset.edges.as_ptr() as usize)
            .chain(
                state
                    .asset_edge_backing
                    .iter()
                    .map(|edges| edges.as_ptr() as usize),
            )
            .collect::<Vec<_>>();
        pointers.sort_unstable();
        pointers
    }

    #[test]
    fn preallocated_backing_reuses_maximum_rows_without_allocation() {
        let source = maximum_state();
        let mut target = SpatialPermissionState::empty_backing().unwrap();
        let fixed_owned = target.owned_capacity_bytes();
        let source_owned = source.owned_capacity_bytes();
        assert_eq!(
            fixed_owned,
            SpatialPermissionState::max_owned_capacity_bytes()
        );
        assert_eq!(source_owned, fixed_owned);
        let two_resident_state_peak = source_owned + fixed_owned;
        let edge_ptrs = edge_backing_ptrs(&target);
        let assets_ptr = target.assets.as_ptr();
        let requests_ptr = target.requests.as_ptr();
        let approvals_ptr = target.approvals.as_ptr();
        for _ in 0..4 {
            source.copy_into(&mut target).unwrap();
            assert_eq!(target, source);
            assert_eq!(target.owned_capacity_bytes(), fixed_owned);
            assert_eq!(
                source.owned_capacity_bytes() + target.owned_capacity_bytes(),
                two_resident_state_peak
            );
            assert_eq!(edge_backing_ptrs(&target), edge_ptrs);
            assert_eq!(target.assets.as_ptr(), assets_ptr);
            assert_eq!(target.requests.as_ptr(), requests_ptr);
            assert_eq!(target.approvals.as_ptr(), approvals_ptr);
            target.clear_preserving_capacity();
            assert_eq!(target.owned_capacity_bytes(), fixed_owned);
            assert_eq!(edge_backing_ptrs(&target), edge_ptrs);
        }
    }

    #[test]
    fn in_place_restore_reuses_backing_and_truncated_input_is_fail_closed() {
        let source = maximum_state();
        let bytes = serde_json::to_vec(&source).unwrap();
        let mut target = SpatialPermissionState::empty_backing().unwrap();
        let owned = target.owned_capacity_bytes();
        let pointers = edge_backing_ptrs(&target);
        target
            .restore_into_existing(&mut serde_json::Deserializer::from_slice(&bytes))
            .unwrap();
        assert_eq!(target, source);
        assert_eq!(target.owned_capacity_bytes(), owned);
        assert_eq!(edge_backing_ptrs(&target), pointers);

        let truncated = &bytes[..bytes.len() - 1];
        assert!(target
            .restore_into_existing(&mut serde_json::Deserializer::from_slice(truncated))
            .is_err());
        assert!(target.assets.is_empty());
        assert!(target.requests.is_empty());
        assert!(target.approvals.is_empty());
        assert_eq!(target.owned_capacity_bytes(), owned);
        assert_eq!(edge_backing_ptrs(&target), pointers);
        target
            .restore_into_existing(&mut serde_json::Deserializer::from_slice(&bytes))
            .unwrap();
        assert_eq!(target, source);
    }

    fn submit_request(
        state: &mut SpatialPermissionState,
        request: SpatialUpdateRequest,
    ) -> [u8; 32] {
        let digest = request_sha256(
            state.authorization_context(request.asset_id).unwrap(),
            &request,
        )
        .unwrap();
        state
            .apply(
                30,
                SpatialPermissionCommand::Request {
                    expected_state_revision: state.revision,
                    request,
                },
                None,
            )
            .unwrap();
        digest
    }

    fn approve(
        state: &mut SpatialPermissionState,
        digest: [u8; 32],
        actor: u32,
        role: SpatialApprovalRole,
    ) {
        state
            .apply(
                actor,
                SpatialPermissionCommand::Approve {
                    expected_state_revision: state.revision,
                    approval: SpatialUpdateApproval {
                        request_id: 100,
                        request_sha256: digest,
                        approver_account: actor,
                        role,
                        approved: true,
                    },
                },
                None,
            )
            .unwrap();
    }

    fn boundary(world: u64, branch: u64) -> CompletedDayBoundary {
        CompletedDayBoundary::from_engine_timeline(
            world,
            branch,
            GameTime::new((2 * crate::timeline::DAY) as i64).unwrap(),
            true,
        )
        .unwrap()
    }

    #[test]
    fn owner_and_independent_authority_approve_then_executor_applies_at_exact_boundary() {
        let mut replay = state();
        let mut state = state();
        let fixed_owned_bytes = state.owned_capacity_bytes();
        let digest = submit_request(&mut state, request());
        approve(&mut state, digest, 10, SpatialApprovalRole::OwnerConsent);
        approve(&mut state, digest, 20, SpatialApprovalRole::AuthorityPermit);
        let grant = state
            .apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: state.revision,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(1, 1)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(grant.world_id(), 1);
        assert_eq!(grant.effective_service_day(), 2);
        assert_eq!(grant.parent_profile_sha256(), [2; 32]);
        assert_eq!(grant.candidate_profile_sha256(), [3; 32]);
        assert_eq!(grant.new_revision(), 2);
        assert!(!state.assets[0].edges[0].currently_open);
        assert_eq!(state.assets[0].current_profile_sha256, [3; 32]);
        assert_eq!(state.owned_capacity_bytes(), fixed_owned_bytes);
        assert!(state.requests.is_empty());
        assert_eq!(state.request_id_highwater, 100);
        assert!(state.validate().is_ok());
        let witness = grant.witness();
        replay
            .replay_authorized_update(
                witness.request,
                witness.owner_approval,
                witness.authority_approval,
                grant.evidence_sha256(),
            )
            .unwrap();
        assert_eq!(replay.assets, state.assets);
        assert_eq!(replay.request_id_highwater, state.request_id_highwater);
        assert!(replay
            .replay_authorized_update(
                witness.request,
                witness.owner_approval,
                witness.authority_approval,
                grant.evidence_sha256(),
            )
            .is_err());
        let cold: SpatialPermissionState =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(cold, state);
        assert!(cold.validate().is_ok());
    }

    #[test]
    fn request_authorization_is_bound_to_immutable_scene_context() {
        let mut original = state();
        let mut other_scene = state();
        other_scene.assets[0].source_scene_sha256 = [9; 32];
        let digest = submit_request(&mut original, request());
        approve(&mut original, digest, 10, SpatialApprovalRole::OwnerConsent);
        approve(
            &mut original,
            digest,
            20,
            SpatialApprovalRole::AuthorityPermit,
        );
        let grant = original
            .apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: original.revision,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(1, 1)),
            )
            .unwrap()
            .unwrap();
        let witness = grant.witness();
        assert_ne!(
            state().authorization_context(7).unwrap(),
            other_scene.authorization_context(7).unwrap()
        );
        assert_eq!(
            other_scene.replay_authorized_update(
                witness.request,
                witness.owner_approval,
                witness.authority_approval,
                grant.evidence_sha256(),
            ),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn shared_profile_hash_advances_while_only_target_asset_revision_changes() {
        let first = state().assets[0].clone();
        let second = SpatialAssetState {
            asset_id: 8,
            kind: SpatialAssetKind::Building,
            owner_account: 11,
            permit_authority_account: 20,
            source_scene_sha256: [1; 32],
            revision: 4,
            current_profile_sha256: [2; 32],
            edges: vec![SpatialAccessEdge {
                edge_id: 12,
                surface: SpatialAccessSurface::Door,
                initially_open: true,
                currently_open: true,
                allowed_operations: SPATIAL_ALLOW_CLOSE | SPATIAL_ALLOW_RESTORE,
            }],
        };
        let mut state = SpatialPermissionState::from_genesis(1, 1, vec![first, second]).unwrap();
        let digest = submit_request(&mut state, request());
        approve(&mut state, digest, 10, SpatialApprovalRole::OwnerConsent);
        approve(&mut state, digest, 20, SpatialApprovalRole::AuthorityPermit);
        let grant = state
            .apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: state.revision,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(1, 1)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(grant.asset_id(), 7);
        assert_eq!(grant.asset_kind(), SpatialAssetKind::Road);
        assert_eq!(grant.new_revision(), 2);
        assert_eq!(state.assets[0].revision, 2);
        assert_eq!(state.assets[1].revision, 4);
        assert_eq!(state.assets[0].current_profile_sha256, [3; 32]);
        assert_eq!(state.assets[1].current_profile_sha256, [3; 32]);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn unauthorized_or_missing_approvals_do_not_mutate_state() {
        let mut state = state();
        let digest = submit_request(&mut state, request());
        let before = state.clone();
        let unauthorized = state.apply(
            30,
            SpatialPermissionCommand::Approve {
                expected_state_revision: state.revision,
                approval: SpatialUpdateApproval {
                    request_id: 100,
                    request_sha256: digest,
                    approver_account: 30,
                    role: SpatialApprovalRole::AuthorityPermit,
                    approved: true,
                },
            },
            None,
        );
        assert_eq!(unauthorized, Err(Error::Unauthorized));
        assert_eq!(state, before);
        let apply = state.apply(
            40,
            SpatialPermissionCommand::Apply {
                expected_state_revision: state.revision,
                request_id: 100,
                request_sha256: digest,
            },
            Some(boundary(1, 1)),
        );
        assert_eq!(apply, Err(Error::MissingApproval));
        assert_eq!(state, before);
    }

    #[test]
    fn applicant_executor_and_wrong_authority_cannot_supply_authority_approval() {
        let mut state = state();
        let digest = submit_request(&mut state, request());
        for actor in [30, 40] {
            let result = state.apply(
                actor,
                SpatialPermissionCommand::Approve {
                    expected_state_revision: state.revision,
                    approval: SpatialUpdateApproval {
                        request_id: 100,
                        request_sha256: digest,
                        approver_account: actor,
                        role: SpatialApprovalRole::AuthorityPermit,
                        approved: true,
                    },
                },
                None,
            );
            assert_eq!(result, Err(Error::Unauthorized));
        }
    }

    #[test]
    fn tampered_candidate_stale_revision_and_cross_world_boundary_reject() {
        let mut state = state();
        let digest = submit_request(&mut state, request());
        approve(&mut state, digest, 10, SpatialApprovalRole::OwnerConsent);
        approve(&mut state, digest, 20, SpatialApprovalRole::AuthorityPermit);
        let before = state.clone();
        let mut tampered = SpatialPermissionState::empty_backing().unwrap();
        state.copy_into(&mut tampered).unwrap();
        tampered.requests[0].request.candidate_profile_sha256 = [4; 32];
        assert_eq!(tampered.validate(), Err(Error::InvalidRequest));
        assert_eq!(state, before);
        assert_eq!(
            state.apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: state.revision - 1,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(1, 1)),
            ),
            Err(Error::StaleRevision)
        );
        assert_eq!(
            state.apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: state.revision,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(2, 1)),
            ),
            Err(Error::InvalidBoundary)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn completed_boundary_is_exact_day_and_repeat_apply_is_rejected() {
        assert_eq!(
            CompletedDayBoundary::from_engine_timeline(
                1,
                1,
                GameTime::new((2 * crate::timeline::DAY + 1) as i64).unwrap(),
                true,
            ),
            Err(Error::InvalidBoundary)
        );
        let mut state = state();
        let digest = submit_request(&mut state, request());
        approve(&mut state, digest, 10, SpatialApprovalRole::OwnerConsent);
        approve(&mut state, digest, 20, SpatialApprovalRole::AuthorityPermit);
        state
            .apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: state.revision,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(1, 1)),
            )
            .unwrap();
        assert_eq!(
            state.apply(
                40,
                SpatialPermissionCommand::Apply {
                    expected_state_revision: state.revision,
                    request_id: 100,
                    request_sha256: digest,
                },
                Some(boundary(1, 1)),
            ),
            Err(Error::InvalidRequest)
        );
    }

    #[test]
    fn withdrawal_and_veto_release_active_slot_without_reusing_request_ids() {
        let mut state = state();
        let first_digest = submit_request(&mut state, request());
        state
            .apply(
                30,
                SpatialPermissionCommand::Withdraw {
                    expected_state_revision: state.revision,
                    request_id: 100,
                    request_sha256: first_digest,
                },
                None,
            )
            .unwrap();
        assert!(state.requests.is_empty());
        assert!(state.approvals.is_empty());
        assert_eq!(state.request_id_highwater, 100);
        let mut reused = request();
        reused.request_id = 100;
        assert_eq!(
            state.apply(
                30,
                SpatialPermissionCommand::Request {
                    expected_state_revision: state.revision,
                    request: reused,
                },
                None,
            ),
            Err(Error::InvalidRequest)
        );

        let mut later = request();
        later.request_id = 101;
        let later_digest = submit_request(&mut state, later);
        state
            .apply(
                10,
                SpatialPermissionCommand::Approve {
                    expected_state_revision: state.revision,
                    approval: SpatialUpdateApproval {
                        request_id: 101,
                        request_sha256: later_digest,
                        approver_account: 10,
                        role: SpatialApprovalRole::OwnerConsent,
                        approved: false,
                    },
                },
                None,
            )
            .unwrap();
        assert!(state.requests.is_empty());
        assert!(state.approvals.is_empty());
        assert_eq!(state.request_id_highwater, 101);
        assert!(state.validate().is_ok());
    }
}
