//! Single-writer, bounded volatile transactions. No durable-success claim until storage.
use crate::diagnostics::{Class, Role, Trace};
use crate::{
    arena::{AccountDomain, Handle, InventoryDomain, RawHandle},
    assembly::{Account, Stock, World},
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
use std::mem::size_of;
mod bundle;
mod slots;
pub use bundle::{EngineWorkspace, EngineWorkspaceIdentity};
#[path = "timeline_driver.rs"]
mod timeline_driver;
mod workspace;
pub use slots::{EngineSlot, SlotIdentity};

pub const MAX_ACTIONS: usize = 16;
pub const MAX_RECEIPTS: usize = 1024;
// The integrated city needs independent site, operating, disposal and utility
// resources. This is a bounded schema capacity, not a larger byte-budget grant.
pub const MAX_RESOURCES: usize = 16;
pub const MAX_LEASES: usize = 64;
pub const ENGINE_BYTE_BUDGET: usize = 4 * 1024 * 1024;

type DirtyAccounts =
    crate::model_rows::Rows<(Handle<AccountDomain>, Account), { crate::assembly::MAX_ACCOUNTS }>;
type DirtyStocks =
    crate::model_rows::Rows<(Handle<InventoryDomain>, Stock), { crate::assembly::MAX_STOCKS }>;
type ResourceRows = crate::model_rows::Rows<Resource, MAX_RESOURCES>;
type LeaseRows = crate::model_rows::Rows<Option<Lease>, MAX_LEASES>;
type HotJournalRows = crate::model_rows::Rows<HotJournalRecord, MAX_RECEIPTS>;

fn valid_background_operation(caller: Caller, env: &Envelope) -> bool {
    if env.operation_id & 0xf000_0000_0000_0000 == 0xd000_0000_0000_0000 {
        return matches!(caller, Caller::System)
            && matches!(
                env.actions.as_slice(),
                [Action::Background {
                    command: crate::background::Command::Restart
                        | crate::background::Command::Lease { .. }
                        | crate::background::Command::RebindLease { .. }
                        | crate::background::Command::Fail { .. }
                }]
            );
    }
    let canonical = env.actions.iter().find_map(|a| match a {
        Action::Background {
            command: crate::background::Command::Accept { job, .. },
        } => Some(crate::background::accept_operation_id(*job)),
        _ => None,
    });
    if let Some(id) = canonical {
        return matches!(caller, Caller::System)
            && env.actions.len() == 1
            && id.ok() == Some(env.operation_id);
    }
    env.operation_id & 0xf000_0000_0000_0000 != crate::background::ACCEPT_PREFIX
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacityToken {
    pub world: u64,
    pub branch: u64,
    pub number: u64,
}

/// Only trusted host code constructs this context; never deserialized from tool input.
#[derive(Clone, Copy, Debug)]
pub enum Caller {
    System,
    Account(u32),
}
impl Caller {
    fn key(self) -> u32 {
        match self {
            Self::System => 0,
            Self::Account(id) => id,
        }
    }
    fn permits(self, id: u32) -> bool {
        matches!(self, Self::System) || self.key() == id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Background {
        command: crate::background::Command,
    },
    Timeline {
        command: crate::timeline::Command,
    },
    Inbox {
        command: crate::inbox::Command,
    },
    Exchange {
        command: crate::exchange::Command,
    },
    Boot {
        command: crate::boot::Command,
    },
    Economy {
        command: crate::economy::Command,
    },
    /// One active construction projection with scalar retry commands. Project configuration
    /// is fixed in genesis and lives in World, not in every journal action.
    Construction {
        command: crate::construction::EngineCommand,
    },
    /// Recompute and atomically settle one complete game-day market from authoritative
    /// Engine accounts, stock lots, Timeline payroll and the genesis-bound profile.
    TownSettle {
        profile_id: u32,
    },
    /// Resolve one immutable, Store-verified transport proof for this market-day settlement.
    TownTransportSettle {
        profile_id: u32,
        service_day: u64,
        generation: u64,
        proof_sha256: [u8; 32],
    },
    /// A trusted owner/authority spatial permission command. The Store publishes an Apply's
    /// immutable transport profile version in the same durable transaction.
    SpatialPermission {
        command: crate::spatial_transport_permissions::SpatialPermissionCommand,
    },
    /// Select one immutable genesis lease offer; all lease terms resolve from that trusted row.
    /// This compact variant stays within the hot-journal Action size limit.
    HousingLease {
        offer_id: u64,
        offer_sha256: [u8; 32],
    },
    /// Permanently stop new operations for one configured town-market business. This is
    /// an operational control only; it does not approve insolvency, liquidation or closure.
    TownSuspendOperations {
        profile_id: u32,
        company_account: u32,
    },
    /// Issue ordinary shares for a charter-bound actual account contribution. The caller
    /// chooses no party or account identity; both resolve through the immutable opening charter.
    EquityContribute {
        company: u32,
        amount: Money,
    },
    /// Pay a bounded, pro-rata cash distribution from actual retained operating earnings.
    /// The caller is the immutable charter's distribution authority, never CitySystem.
    EquityDistributeRetainedEarnings {
        company: u32,
        requested: Money,
    },
    /// Start the bounded, policy-authorized exit workflow for a configured company.
    BeginExit {
        company: u32,
        policy_id: u32,
    },
    /// Settle due liabilities from actual cash and inventory proceeds recorded by the Engine.
    SettleExit {
        company: u32,
    },
    /// Close only after all supported obligations and assets are resolved with no residual cash.
    CloseExit {
        company: u32,
    },
    RegionSettleDebt {
        obligation: u32,
    },
    RegionActivate {
        city: u32,
    },
    RegionAdvance {
        seconds: u32,
    },
    RegionMigrate {
        from: u32,
        to: u32,
        requested: u32,
        tolerance: u32,
    },
    Transfer {
        from: RawHandle,
        to: RawHandle,
        amount: Money,
    },
    Protect {
        account: RawHandle,
        amount: Money,
    },
    ReleaseProtection {
        account: RawHandle,
        amount: Money,
    },
    PayProtected {
        from: RawHandle,
        to: RawHandle,
        amount: Money,
    },
    MoveStock {
        from: RawHandle,
        to: RawHandle,
        quantity: Quantity,
    },
    ReserveCapacity {
        resource: u32,
        units: u32,
    },
    OccupyCapacity {
        token: CapacityToken,
    },
    ReleaseCapacity {
        token: CapacityToken,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub world: u64,
    pub branch: u64,
    pub operation_id: u64,
    pub expected_revision: u64,
    pub actions: Vec<Action>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxError {
    WrongScope,
    InvalidOperation,
    TooManyActions,
    Unauthorized,
    OperationConflict,
    JournalFull,
    InboxFull,
    UnsettledResponsibility,
    StaleRevision,
    BackgroundRequired,
    InvalidHandle,
    InsufficientFunds,
    Protection,
    Overflow,
    Stock,
    Resource,
    Capacity,
    Lease,
    WorkingLimit,
    Region(crate::region::Error),
    Economy(crate::economy::Error),
    Boot(crate::boot::Error),
    Exchange(crate::exchange::Error),
    Timeline(crate::timeline::Error),
    Construction(crate::construction::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Receipt {
    pub operation_id: u64,
    pub revision: u64,
    pub durability: crate::FrozenLabel,
    pub migrated: [Option<u32>; MAX_ACTIONS],
    pub created_facts: [Option<u64>; MAX_ACTIONS],
    pub settled: [Option<Money>; MAX_ACTIONS],
    pub created_tokens: [Option<CapacityToken>; MAX_ACTIONS],
    pub timeline: Option<crate::timeline::Progress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub town: Option<crate::town_runtime::MarketSummary>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct JournalRecord {
    pub actor: u32,
    pub emergency_eligible: bool,
    pub operation_id: u64,
    pub expected_revision: u64,
    pub payload_sha256: [u8; 32],
    pub outcome: Result<Receipt, TxError>,
    pub actions: [Option<Action>; MAX_ACTIONS],
}

/// In-memory retry window. Durable receipts continue to use the full `JournalRecord`; this
/// resident form keeps only the spatial request identity needed for bounded diagnostics. The
/// digest and request id are not an authorization witness.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HotJournalRecord {
    pub actor: u32,
    pub emergency_eligible: bool,
    pub operation_id: u64,
    pub expected_revision: u64,
    pub payload_sha256: [u8; 32],
    pub outcome: Result<Receipt, TxError>,
    pub actions: [Option<HotActionSummary>; MAX_ACTIONS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HotActionSummary {
    Other,
    SpatialPermission {
        command_kind: u8,
        request_id: u64,
        command_sha256: [u8; 32],
    },
}

impl HotJournalRecord {
    fn from_durable(record: &JournalRecord) -> Self {
        let mut actions = [None; MAX_ACTIONS];
        for (index, action) in record.actions.iter().enumerate() {
            let Some(action) = action else { continue };
            actions[index] = Some(match action {
                Action::SpatialPermission { command } => {
                    use sha2::{Digest, Sha256};
                    let bytes = serde_json::to_vec(command).expect("bounded spatial command");
                    let (command_kind, request_id) = match command {
                        crate::spatial_transport_permissions::SpatialPermissionCommand::Request {
                            request,
                            ..
                        } => (0, request.request_id),
                        crate::spatial_transport_permissions::SpatialPermissionCommand::Approve {
                            approval,
                            ..
                        } => (1, approval.request_id),
                        crate::spatial_transport_permissions::SpatialPermissionCommand::Withdraw {
                            request_id,
                            ..
                        } => (2, *request_id),
                        crate::spatial_transport_permissions::SpatialPermissionCommand::Apply {
                            request_id,
                            ..
                        } => (3, *request_id),
                    };
                    HotActionSummary::SpatialPermission {
                        command_kind,
                        request_id,
                        command_sha256: Sha256::digest(bytes).into(),
                    }
                }
                _ => HotActionSummary::Other,
            });
        }
        Self {
            actor: record.actor,
            emergency_eligible: record.emergency_eligible,
            operation_id: record.operation_id,
            expected_revision: record.expected_revision,
            payload_sha256: record.payload_sha256,
            outcome: record.outcome,
            actions,
        }
    }

    /// Confirm that a durable WAL/checkpoint receipt is the source of this hot retry summary.
    /// This is an integrity cross-check only; it does not authorize the summarized command.
    pub fn matches_durable(&self, record: &JournalRecord) -> bool {
        self.actor == record.actor
            && self.emergency_eligible == record.emergency_eligible
            && self.operation_id == record.operation_id
            && self.expected_revision == record.expected_revision
            && self.payload_sha256 == record.payload_sha256
            && self.outcome == record.outcome
            && self.actions == Self::from_durable(record).actions
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: u32,
    pub capacity: u32,
    pub reserved: u32,
    pub occupied: u32,
}
impl Resource {
    pub fn new(id: u32, capacity: u32) -> Result<Self, TxError> {
        if id == 0 {
            return Err(TxError::Resource);
        }
        Ok(Self {
            id,
            capacity,
            reserved: 0,
            occupied: 0,
        })
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Lease {
    pub token: u64,
    pub actor: u32,
    pub resource: u32,
    pub units: u32,
    pub occupied: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    pub(crate) world: Box<World>,
    resources: ResourceRows,
    leases: LeaseRows,
    next_token: u64,
    journal: HotJournalRows,
    timeline: crate::model_rows::Resident<crate::timeline::Timeline>,
    background: crate::model_rows::Resident<crate::background::Background>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spatial_permissions: Option<crate::spatial_transport_permissions::SpatialPermissionState>,
    #[serde(skip)]
    preparation_workspace: Option<workspace::PreparationWorkspace>,
}

struct Prepared {
    timeline: workspace::TimelineLease,
    background: workspace::BackgroundLease,
    exchange: Option<crate::exchange::Exchange>,
    boot: Option<crate::boot::Boot>,
    economy: Option<crate::economy::Economy>,
    region: Option<crate::region::Region>,
    construction: Option<crate::construction::Project>,
    housing: Option<crate::housing::HousingState>,
    town: Option<crate::town_runtime::MarketState>,
    spatial_permissions: Option<workspace::SpatialPermissionLease>,
    spatial_update_grant: Option<crate::spatial_transport_permissions::VerifiedSpatialUpdateGrant>,
    accounts: workspace::AccountLease,
    stocks: workspace::StockLease,
    resources: workspace::ResourceLease,
    leases: workspace::CapacityLease,
    next_token: u64,
    receipt: Receipt,
}

/// Ephemeral metadata from a fully prepared transaction. It is passed only to the unique
/// persistence hook and is deliberately absent from per-receipt storage.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommitContext<'a> {
    pub spatial_update_grant:
        Option<crate::spatial_transport_permissions::VerifiedSpatialUpdateGrant>,
    pub spatial_permissions:
        Option<&'a crate::spatial_transport_permissions::SpatialPermissionState>,
    /// Active Engine state that was used as the authority for preparing this candidate.
    /// Storage validates the transition against this root-pinned prior image before publishing.
    pub prior_spatial_permissions:
        Option<&'a crate::spatial_transport_permissions::SpatialPermissionState>,
}

pub(crate) fn preparation_workspace_capacity() -> (usize, usize, usize) {
    workspace::capacity_inventory()
}
pub(crate) const fn prepared_header_bytes() -> usize {
    size_of::<Prepared>()
}

impl Engine {
    pub fn new(world: World, mut resources: Vec<Resource>) -> Result<Self, TxError> {
        if world.economy.is_some() && !resources.is_empty() {
            return Err(TxError::Resource);
        }
        if resources.len() > MAX_RESOURCES {
            return Err(TxError::Resource);
        }
        resources.sort_by_key(|r| r.id);
        if let Some(project) = world.construction {
            let site = resources
                .iter()
                .find(|r| r.id == project.site_resource)
                .ok_or(TxError::Resource)?;
            if project.site_units > site.capacity {
                return Err(TxError::Capacity);
            }
            if project.waste_units != 0 {
                let receiver = resources
                    .iter()
                    .find(|r| r.id == project.waste_receiver_resource)
                    .ok_or(TxError::Resource)?;
                if project.waste_units > receiver.capacity {
                    return Err(TxError::Capacity);
                }
            }
            if project.operating_resource != 0 {
                let operating = resources
                    .iter()
                    .find(|r| r.id == project.operating_resource)
                    .ok_or(TxError::Resource)?;
                let expected = project.operating_base_capacity;
                if operating.capacity != expected {
                    return Err(TxError::Capacity);
                }
            }
        }
        if let Some(e) = world.exchange {
            if e.edges.iter().any(|e| {
                !resources
                    .iter()
                    .any(|r| r.id == e.resource && r.capacity > 0)
            }) {
                return Err(TxError::Resource);
            }
        }
        if let Some(b) = world.boot {
            if b.resources
                .iter()
                .any(|id| !resources.iter().any(|r| r.id == *id && r.capacity > 0))
            {
                return Err(TxError::Resource);
            }
        }
        if resources
            .iter()
            .any(|r| r.id == 0 || r.reserved != 0 || r.occupied != 0)
            || resources.windows(2).any(|w| w[0].id == w[1].id)
        {
            return Err(TxError::Resource);
        }
        let mut journal = HotJournalRows::new().map_err(|_| TxError::WorkingLimit)?;
        journal
            .try_reserve_exact(MAX_RECEIPTS)
            .map_err(|_| TxError::WorkingLimit)?;
        let mut leases = LeaseRows::new().map_err(|_| TxError::WorkingLimit)?;
        leases
            .try_reserve_exact(MAX_LEASES)
            .map_err(|_| TxError::WorkingLimit)?;
        leases.resize(MAX_LEASES, None);
        let mut resource_rows = ResourceRows::new().map_err(|_| TxError::WorkingLimit)?;
        for r in &resources {
            resource_rows.push(*r);
        }
        drop(resources);
        let engine = Self {
            world: Box::new(world),
            resources: resource_rows,
            leases,
            next_token: 1,
            journal,
            timeline: crate::model_rows::Resident::empty(),
            background: crate::model_rows::Resident::empty(),
            spatial_permissions: None,
            preparation_workspace: Some(workspace::reserve()?),
        };
        validate_construction_binding(
            engine.world.construction,
            &engine.resources,
            &engine.leases,
            engine.world.boot.as_ref(),
            engine.world.exchange.as_ref(),
            None,
            engine.world.town.map(|town| town.state.settled_day),
        )?;
        let owned_capacity = engine.owned_capacity_bytes();
        if owned_capacity > ENGINE_BYTE_BUDGET {
            #[cfg(test)]
            {
                let spatial_active = engine
                    .spatial_permissions
                    .as_ref()
                    .map_or(0, |state| state.owned_capacity_bytes());
                let spatial_spare = engine
                    .preparation_workspace
                    .as_ref()
                    .and_then(|workspace| {
                        workspace.spatial_permissions.lock().ok().and_then(|slot| {
                            slot.as_ref().map(|state| state.owned_capacity_bytes())
                        })
                    })
                    .unwrap_or(0);
                let timeline = engine.timeline.backing().owned_bytes();
                let background = engine.background.backing().owned_bytes()
                    - size_of::<crate::background::Background>();
                eprintln!(
                    "ENGINE_NEW_BUDGET_FAILURE {{\"owned\":{owned_capacity},\"budget\":{ENGINE_BYTE_BUDGET},\"engine_header\":{},\"world\":{},\"resources\":{},\"leases\":{},\"journal\":{},\"timeline\":{timeline},\"background_extra\":{background},\"spatial_active\":{spatial_active},\"spatial_spare\":{spatial_spare}}}",
                    size_of::<Self>(),
                    engine.world.owned_capacity_bytes(),
                    engine.resources.capacity() * size_of::<Resource>(),
                    engine.leases.capacity() * size_of::<Option<Lease>>(),
                    engine.journal.capacity() * size_of::<HotJournalRecord>(),
                );
            }
            return Err(TxError::WorkingLimit);
        }
        Ok(engine)
    }
    /// Complete authoritative state, excluding the hot retry window (retained by Store's immutable archive).
    pub fn export_checkpoint(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        self.write_checkpoint(&mut bytes)?;
        Ok(bytes)
    }
    /// Serialize every authoritative field without allocating a JSON DOM or hot journal copy.
    /// Storage still binds bytes to its rules/genesis and validates before publication.
    pub fn write_checkpoint(&self, writer: impl std::io::Write) -> Result<(), String> {
        #[derive(Serialize)]
        struct View<'a> {
            world: &'a World,
            resources: &'a [Resource],
            leases: &'a [Option<Lease>],
            next_token: u64,
            journal: &'a [HotJournalRecord],
            timeline: &'a crate::model_rows::Resident<crate::timeline::Timeline>,
            background: &'a crate::model_rows::Resident<crate::background::Background>,
            #[serde(skip_serializing_if = "Option::is_none")]
            spatial_permissions:
                &'a Option<crate::spatial_transport_permissions::SpatialPermissionState>,
        }
        struct Bounded<W> {
            inner: W,
            bytes: usize,
        }
        impl<W: std::io::Write> std::io::Write for Bounded<W> {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                if data.len() > 4 * 1024 * 1024 - self.bytes {
                    return Err(std::io::Error::other("checkpoint_byte_limit"));
                }
                let n = self.inner.write(data)?;
                self.bytes += n;
                Ok(n)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.inner.flush()
            }
        }
        serde_json::to_writer(
            Bounded {
                inner: writer,
                bytes: 0,
            },
            &View {
                world: &self.world,
                resources: &self.resources,
                leases: &self.leases,
                next_token: self.next_token,
                journal: &[],
                timeline: &self.timeline,
                background: &self.background,
                spatial_permissions: &self.spatial_permissions,
            },
        )
        .map_err(|e| e.to_string())
    }

    fn business_view(&self) -> impl Serialize + '_ {
        #[derive(Serialize)]
        struct View<'a> {
            schema: &'static str,
            world: crate::assembly::BusinessWorld<'a>,
            resources: &'a [Resource],
            leases: &'a [Option<Lease>],
            next_token: u64,
            timeline: &'a crate::model_rows::Resident<crate::timeline::Timeline>,
            #[serde(skip_serializing_if = "Option::is_none")]
            spatial_permissions:
                &'a Option<crate::spatial_transport_permissions::SpatialPermissionState>,
        }
        View {
            schema: "mycity-business-comparison-v2",
            world: self.world.business_world(),
            resources: &self.resources,
            leases: &self.leases,
            next_token: self.next_token,
            timeline: &self.timeline,
            spatial_permissions: &self.spatial_permissions,
        }
    }
    /// All business bytes, with only hot receipts/revision/background excluded.
    /// Caller owns the serial codec and output; no hidden Engine workspace.
    pub fn write_business_comparison(
        &self,
        codec: &mut crate::canonical::Codec,
        writer: impl std::io::Write,
    ) -> Result<(), String> {
        codec.write(&self.business_view(), writer)
    }
    pub fn business_digest_with(
        &self,
        codec: &mut crate::canonical::Codec,
    ) -> Result<String, String> {
        codec.hash(&self.business_view())
    }
    /// Diagnostic allocating wrapper, excluded from pooled hot paths.
    pub fn business_comparison_checkpoint(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        self.write_business_comparison(&mut crate::canonical::Codec::new()?, &mut bytes)?;
        Ok(bytes)
    }
    /// Only a hash-verified, rules-bound storage dependency may enter this boundary.
    pub fn restore_checkpoint(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("checkpoint_byte_limit".into());
        }
        crate::bounded_decode::guard(bytes)?;
        let candidate: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        Self::validate_restored(candidate)
    }
    /// Streaming counterpart of restore_checkpoint. The caller must additionally
    /// bind the complete input to its trusted length/hash before publishing it.
    /// Lexical and byte limits run before serde accumulates tokens or records.
    pub fn restore_checkpoint_reader(reader: impl std::io::Read) -> Result<Self, String> {
        struct Limited<R> {
            reader: R,
            bytes: usize,
        }
        impl<R: std::io::Read> std::io::Read for Limited<R> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                let size = out.len().min(4 * 1024 * 1024 + 1 - self.bytes);
                let n = self.reader.read(&mut out[..size])?;
                self.bytes += n;
                if self.bytes > 4 * 1024 * 1024 {
                    return Err(std::io::Error::other("checkpoint_byte_limit"));
                }
                Ok(n)
            }
        }
        let reader =
            crate::fixed_restore::JsonReader::with_string_limit(Limited { reader, bytes: 0 }, 384);
        let mut decoder = serde_json::Deserializer::from_reader(reader);
        let candidate = Self::deserialize(&mut decoder).map_err(|e| e.to_string())?;
        decoder.end().map_err(|e| e.to_string())?;
        Self::validate_restored(candidate)
    }
    fn validate_restored(mut candidate: Self) -> Result<Self, String> {
        // Explicit scratch construction, not a serde default which silently allocates.
        // Whole-graph seed/authorization is still being migrated in this phase.
        candidate.preparation_workspace =
            Some(workspace::reserve().map_err(|e| format!("workspace:{e:?}"))?);
        if let Some(state) = candidate.spatial_permissions.as_mut() {
            state
                .reserve_fixed_rows()
                .map_err(|error| format!("spatial_permission_capacity:{error:?}"))?;
            let spare =
                crate::spatial_transport_permissions::SpatialPermissionState::empty_backing()
                    .map_err(|error| format!("spatial_permission_spare:{error:?}"))?;
            let owner = &candidate
                .preparation_workspace
                .as_ref()
                .ok_or("missing_workspace")?
                .spatial_permissions;
            *owner.lock().map_err(|_| "spatial_permission_slot_poison")? = Some(spare);
        }
        candidate.validate_resident()?;
        Ok(candidate)
    }
    fn validate_resident(&mut self) -> Result<(), String> {
        let candidate = self;
        if !candidate.journal.is_empty()
            || candidate.resources.len() > MAX_RESOURCES
            || candidate.leases.len() != MAX_LEASES
            || candidate.next_token == 0
        {
            return Err("checkpoint_collection_limits".into());
        }
        candidate.world.restore_capacity()?;
        if let Some(state) = candidate.spatial_permissions.as_ref() {
            if state.validate().is_err()
                || state.world_id != candidate.world.scope.world()
                || state.branch_id != candidate.world.scope.branch()
                || state.assets.iter().any(|asset| {
                    candidate.world.account_handle(asset.owner_account).is_err()
                        || candidate
                            .world
                            .account_handle(asset.permit_authority_account)
                            .is_err()
                })
            {
                return Err("checkpoint_spatial_permission_binding".into());
            }
        }
        if let Some(b) = candidate.world.boot {
            b.validate_world_shape(&candidate.world, candidate.world.exchange.is_some())?;
        }
        if let Some(e) = candidate.world.exchange {
            e.validate_world_shape(&candidate.world)?;
        }
        if let Some(b) = candidate.background.as_ref() {
            let mut work = workspace::BackgroundWorkLease::take(
                &candidate
                    .preparation_workspace
                    .as_ref()
                    .ok_or("missing_workspace")?
                    .background_work,
            )
            .map_err(|e| format!("workspace:{e:?}"))?;
            b.validate_with(&mut work)?;
            let t = candidate.timeline.as_ref().ok_or("background_timeline")?;
            b.validate_timeline(t, candidate.world.revision)?;
            if candidate.world.exchange.is_none()
                || candidate.world.region.is_none()
                || b.pending()
                    .is_some_and(|j| j.base_revision > candidate.world.revision)
                || u64::from(b.time()) < t.progress.last_complete
                || u64::from(b.time()) > t.progress.at.max(t.progress.last_complete)
            {
                return Err("background_restore_clock".into());
            }
        }
        if let Some(t) = candidate.timeline.as_ref() {
            t.validate()
                .map_err(|e| format!("timeline_restore:{e:?}"))?;
        }
        validate_town_suspension_consistency(
            candidate.world.town.as_ref().map(|market| &market.state),
            candidate.timeline.as_ref(),
        )
        .map_err(|_| "checkpoint_town_suspension")?;
        validate_town_exit_state(
            candidate.world.town.as_ref().map(|market| &market.profile),
            candidate.world.town.as_ref().map(|market| &market.state),
            candidate.timeline.as_ref(),
        )
        .map_err(|_| "checkpoint_town_exit")?;
        validate_town_exit_world(
            &candidate.world,
            candidate.timeline.as_ref(),
            &candidate.leases,
        )
        .map_err(|_| "checkpoint_town_exit_assets")?;

        for (i, r) in candidate.resources.iter().enumerate() {
            if r.id == 0 || candidate.resources[..i].iter().any(|p| p.id == r.id) {
                return Err("checkpoint_resource_identity".into());
            }
            let mut reserved = 0u32;
            let mut occupied = 0u32;
            for l in candidate
                .leases
                .iter()
                .flatten()
                .filter(|l| l.resource == r.id)
            {
                let sum = if l.occupied {
                    &mut occupied
                } else {
                    &mut reserved
                };
                *sum = sum
                    .checked_add(l.units)
                    .ok_or("checkpoint_capacity_overflow")?;
            }
            if reserved != r.reserved
                || occupied != r.occupied
                || reserved
                    .checked_add(occupied)
                    .ok_or("checkpoint_capacity_overflow")?
                    > r.capacity
            {
                return Err("checkpoint_capacity_mismatch".into());
            }
        }
        for (i, l) in candidate
            .leases
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.as_ref().map(|l| (i, l)))
        {
            if l.token == 0
                || l.token >= candidate.next_token
                || l.units == 0
                || !candidate.resources.iter().any(|r| r.id == l.resource)
                || candidate.leases[..i]
                    .iter()
                    .flatten()
                    .any(|old| old.token == l.token)
            {
                return Err("checkpoint_lease_identity".into());
            }
        }
        if let Some(project) = candidate.world.construction {
            if !candidate.resources.iter().any(|resource| {
                resource.id == project.site_resource && project.site_units <= resource.capacity
            }) {
                return Err("checkpoint_construction_site_resource".into());
            }
            if validate_construction_binding(
                Some(project),
                &candidate.resources,
                &candidate.leases,
                candidate.world.boot.as_ref(),
                candidate.world.exchange.as_ref(),
                candidate.timeline.as_ref(),
                candidate.world.town.map(|town| town.state.settled_day),
            )
            .is_err()
            {
                return Err("checkpoint_construction_site_lease".into());
            }
        }
        if let Some(r) = candidate.world.region {
            if r.transits.iter().flatten().any(|t| {
                t.route_indices
                    .iter()
                    .flatten()
                    .any(|i| *i >= r.routes.len() || r.routes[*i].is_none())
            }) {
                return Err("checkpoint_route_index".into());
            }
        }
        if let Some(e) = candidate.world.exchange {
            if e.shipments
                .iter()
                .flatten()
                .any(|s| s.quote_index >= e.quotes.len())
            {
                return Err("checkpoint_quote_index".into());
            }
        }
        candidate
            .validate_checkpoint_runtime()
            .map_err(|e| format!("checkpoint_invariant:{e:?}"))?;
        candidate.journal.clear();
        if candidate.owned_capacity_bytes() > ENGINE_BYTE_BUDGET {
            return Err("checkpoint_engine_budget".into());
        }
        Ok(())
    }
    fn validate_checkpoint_runtime(&self) -> Result<(), TxError> {
        if let Some(t) = self.timeline.as_ref() {
            timeline_driver::validate(self, t)?;
        }
        let economy = self.world.economy;
        let boot = self.world.boot;
        let exchange = self.world.exchange;
        let region = self.world.region;
        let storage = self
            .preparation_workspace
            .as_ref()
            .ok_or(TxError::WorkingLimit)?;
        let mut accounts = workspace::AccountLease::take(&storage.accounts)?;
        let mut stocks = workspace::StockLease::take(&storage.stocks)?;
        let mut resources = workspace::ResourceLease::take(&storage.resources)?;
        let mut leases = workspace::CapacityLease::take(&storage.leases)?;
        resources.copy_active_from(&self.resources);
        leases.copy_active_from(&self.leases);
        let mut next_token = self.next_token;
        if let Some(e) = &economy {
            e.validate(&mut EconomicLedger {
                world: &self.world,
                accounts: &mut accounts,
                stocks: &mut stocks,
            })?;
            for (_, a) in &accounts {
                if a.protected < e.required_protection(a.id) {
                    return Err(TxError::Protection);
                }
            }
        }
        if let Some(b) = boot {
            let mut ledger = BootLedger {
                base: EconomicLedger {
                    world: &self.world,
                    accounts: &mut accounts,
                    stocks: &mut stocks,
                },
                resources: &mut resources,
                leases: &mut leases,
                next_token: &mut next_token,
            };
            let mut cash = Money::ZERO;
            for (id, _) in &self.world.account_ids {
                cash = cash
                    .checked_add(crate::boot::Ledger::cash(&mut ledger, *id)?)
                    .map_err(|_| TxError::Overflow)?;
            }
            b.validate(&mut ledger, cash)?;
        }
        if let Some(e) = exchange {
            e.validate(
                region.as_ref().ok_or(TxError::InvalidOperation)?,
                boot.as_ref().ok_or(TxError::InvalidOperation)?,
                self.world.construction.as_ref(),
                &mut BootLedger {
                    base: EconomicLedger {
                        world: &self.world,
                        accounts: &mut accounts,
                        stocks: &mut stocks,
                    },
                    resources: &mut resources,
                    leases: &mut leases,
                    next_token: &mut next_token,
                },
            )?;
        }
        Ok(())
    }
    /// Storage may retire this window only after the checkpoint and exact receipt archive are published.
    pub fn retire_checkpointed_journal(&mut self) {
        self.journal.clear();
    }
    pub fn leases(&self) -> &[Option<Lease>] {
        &self.leases
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }
    /// Compact in-memory retry window. Use the storage receipt archive for full action records.
    pub fn journal(&self) -> &[HotJournalRecord] {
        &self.journal
    }
    pub fn timeline_quote(
        &self,
        caller: Caller,
        env: &Envelope,
    ) -> Option<(crate::timeline::Progress, usize)> {
        self.timeline_quote_diagnosed(caller, env, &mut Trace::disabled())
    }
    pub fn timeline_quote_diagnosed(
        &self,
        caller: Caller,
        env: &Envelope,
        trace: &mut Trace<'_>,
    ) -> Option<(crate::timeline::Progress, usize)> {
        if !matches!(
            env.actions.as_slice(),
            [Action::Timeline {
                command: crate::timeline::Command::Advance { .. }
            }]
        ) || !matches!(caller, Caller::System)
            || env.world != self.world.scope.world()
            || env.branch != self.world.scope.branch()
            || env.expected_revision != self.world.revision
        {
            return None;
        }
        let old = trace.role;
        trace.role = Role::Quote;
        let started = trace.start();
        let prepared = self.prepare_diagnosed(caller, env, None, None, trace);
        trace.end(Class::Prepare, started, 0, u64::from(prepared.is_ok()), 0);
        trace.role = old;
        let prepared = prepared.ok()?;
        let progress = prepared.timeline.as_ref()?.progress;
        let t = self.timeline.as_ref()?;
        // Every examined due row, phase transition, and small domain table is included.
        let steps = t
            .claims
            .iter()
            .filter(|c| !c.processed && c.due == progress.at)
            .count()
            + self
                .world
                .housing
                .as_ref()
                .map_or(0, |housing| housing.state.units().len())
            + 2 * t.payroll.len()
            + 16
            + 16
            + 2
            + 4
            + 16
            + 32;
        Some((progress, steps))
    }
    /// Pure quote for funding a dedicated background control slot before persistence.
    pub fn background_control_quote(
        &self,
        caller: Caller,
        env: &Envelope,
        candidate: Option<&crate::air::State>,
    ) -> bool {
        self.background_control_quote_diagnosed(caller, env, candidate, &mut Trace::disabled())
    }
    pub fn background_control_quote_diagnosed(
        &self,
        caller: Caller,
        env: &Envelope,
        candidate: Option<&crate::air::State>,
        trace: &mut Trace<'_>,
    ) -> bool {
        valid_background_operation(caller, env)
            && matches!(caller, Caller::System)
            && env.world == self.world.scope.world()
            && env.branch == self.world.scope.branch()
            && env.expected_revision == self.world.revision
            && matches!(env.actions.as_slice(), [Action::Background { command }] if !matches!(command, crate::background::Command::Enable))
            && {
                let old = trace.role;
                trace.role = Role::Quote;
                let started = trace.start();
                let result = self.prepare_diagnosed(caller, env, candidate, None, trace);
                trace.end(Class::Prepare, started, 0, u64::from(result.is_ok()), 0);
                trace.role = old;
                result.is_ok()
            }
    }
    /// Recovery/staged validation borrows the same separately charged CPU image.
    pub fn background_candidate_matches(
        &self,
        candidate: &crate::air::State,
    ) -> Result<bool, String> {
        let storage = self
            .preparation_workspace
            .as_ref()
            .ok_or("missing_workspace")?;
        let mut work = workspace::BackgroundWorkLease::take(&storage.background_work)
            .map_err(|e| format!("workspace:{e:?}"))?;
        self.background
            .as_ref()
            .ok_or("background_missing")?
            .candidate_matches(candidate, &mut work, &mut Trace::disabled())
    }
    pub fn background(&self) -> Option<&crate::background::Background> {
        self.background.as_ref()
    }
    pub fn timeline(&self) -> Option<&crate::timeline::Timeline> {
        self.timeline.as_ref()
    }
    pub fn spatial_permissions(
        &self,
    ) -> Option<&crate::spatial_transport_permissions::SpatialPermissionState> {
        self.spatial_permissions.as_ref()
    }
    /// Bind an immutable, Store-validated spatial permission genesis state before the world is
    /// published. This cannot be used after any operation or revision has been committed.
    pub fn initialize_spatial_permissions(
        &mut self,
        state: crate::spatial_transport_permissions::SpatialPermissionState,
    ) -> Result<(), TxError> {
        if self.spatial_permissions.is_some()
            || self.world.revision != 0
            || !self.journal.is_empty()
            || state.world_id != self.world.scope.world()
            || state.branch_id != self.world.scope.branch()
            || state.validate().is_err()
            || state.assets.iter().any(|asset| {
                self.world.account_handle(asset.owner_account).is_err()
                    || self
                        .world
                        .account_handle(asset.permit_authority_account)
                        .is_err()
            })
        {
            return Err(TxError::InvalidOperation);
        }
        let workspace = self
            .preparation_workspace
            .as_ref()
            .ok_or(TxError::WorkingLimit)?;
        {
            let mut spare = workspace
                .spatial_permissions
                .lock()
                .map_err(|_| TxError::WorkingLimit)?;
            if spare.is_none() {
                *spare = Some(
                    crate::spatial_transport_permissions::SpatialPermissionState::empty_backing()
                        .map_err(|_| TxError::WorkingLimit)?,
                );
            }
        }
        self.spatial_permissions = Some(state);
        if self.owned_capacity_bytes() > ENGINE_BYTE_BUDGET {
            let mut active = self
                .spatial_permissions
                .take()
                .ok_or(TxError::InvalidOperation)?;
            active.clear_preserving_capacity();
            let mut spare = workspace
                .spatial_permissions
                .lock()
                .map_err(|_| TxError::WorkingLimit)?;
            *spare = Some(active);
            return Err(TxError::WorkingLimit);
        }
        Ok(())
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        let extra = self.timeline.backing().owned_bytes() + self.background.backing().owned_bytes()
            - size_of::<crate::background::Background>();
        let spatial_workspace = self
            .preparation_workspace
            .as_ref()
            .map(|workspace| {
                workspace
                    .spatial_permissions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .as_ref()
                    .map_or(0, |state| state.owned_capacity_bytes())
            })
            .unwrap_or(0);
        extra
            + size_of::<Self>()
            + self.world.owned_capacity_bytes()
            + self.resources.capacity() * size_of::<Resource>()
            + self.leases.capacity() * size_of::<Option<Lease>>()
            + self.journal.capacity() * size_of::<HotJournalRecord>()
            + self
                .spatial_permissions
                .as_ref()
                .map_or(0, |state| state.owned_capacity_bytes())
            + spatial_workspace
    }
    fn state_view(&self) -> impl Serialize + '_ {
        #[derive(Serialize)]
        struct View<'a> {
            world: crate::assembly::BorrowedWorldView<'a>,
            resources: &'a [Resource],
            leases: &'a [Option<Lease>],
            next_token: u64,
            timeline: &'a crate::model_rows::Resident<crate::timeline::Timeline>,
            background: &'a crate::model_rows::Resident<crate::background::Background>,
            #[serde(skip_serializing_if = "Option::is_none")]
            spatial_permissions:
                &'a Option<crate::spatial_transport_permissions::SpatialPermissionState>,
        }
        View {
            world: self.world.borrowed_view(),
            resources: &self.resources,
            leases: &self.leases,
            next_token: self.next_token,
            timeline: &self.timeline,
            background: &self.background,
            spatial_permissions: &self.spatial_permissions,
        }
    }
    pub fn write_state_comparison(
        &self,
        codec: &mut crate::canonical::Codec,
        writer: impl std::io::Write,
    ) -> Result<(), String> {
        codec.write(&self.state_view(), writer)
    }
    pub fn state_hash_with(&self, codec: &mut crate::canonical::Codec) -> Result<String, String> {
        codec.hash(&self.state_view())
    }
    /// Diagnostic convenience wrapper; production roles retain one caller codec.
    pub fn state_hash(&self) -> String {
        self.state_hash_with(&mut crate::canonical::Codec::new().expect("state codec"))
            .expect("exact state")
    }
    pub fn execute(&mut self, caller: Caller, env: Envelope) -> Result<Receipt, TxError> {
        self.execute_recorded(caller, env, |_, _| Ok::<(), std::convert::Infallible>(()))
            .unwrap_or_else(|never| match never {})
    }

    /// Trusted storage boundary. The hook gets a fully validated fixed outcome before
    /// any CPU world or receipt publication. A hook error leaves the model unchanged.
    /// This core method itself never claims disk durability.
    pub fn execute_recorded<E>(
        &mut self,
        caller: Caller,
        env: Envelope,
        persist: impl FnOnce(&Envelope, &JournalRecord) -> Result<(), E>,
    ) -> Result<Result<Receipt, TxError>, E> {
        self.execute_recorded_with_background(caller, env, None, persist)
    }
    /// Store supplies a hash-verified immutable candidate, including during WAL replay.
    pub fn execute_recorded_with_background<E>(
        &mut self,
        caller: Caller,
        env: Envelope,
        candidate: Option<&crate::air::State>,
        persist: impl FnOnce(&Envelope, &JournalRecord) -> Result<(), E>,
    ) -> Result<Result<Receipt, TxError>, E> {
        self.execute_recorded_diagnosed(caller, env, candidate, persist, &mut Trace::disabled())
    }
    pub fn execute_recorded_diagnosed<E>(
        &mut self,
        caller: Caller,
        env: Envelope,
        candidate: Option<&crate::air::State>,
        persist: impl FnOnce(&Envelope, &JournalRecord) -> Result<(), E>,
        trace: &mut Trace<'_>,
    ) -> Result<Result<Receipt, TxError>, E> {
        self.execute_recorded_transport_diagnosed(caller, env, candidate, None, persist, trace)
    }
    /// Storage supplies quotes resolved from an immutable proof whose digest is also present in
    /// the action. The transaction still performs exactly one planner and reconciliation pass.
    pub fn execute_recorded_transport_diagnosed<E>(
        &mut self,
        caller: Caller,
        env: Envelope,
        candidate: Option<&crate::air::State>,
        transport: Option<&crate::market_access::SettlementDeliveryInput<'_>>,
        persist: impl FnOnce(&Envelope, &JournalRecord) -> Result<(), E>,
        trace: &mut Trace<'_>,
    ) -> Result<Result<Receipt, TxError>, E> {
        self.execute_recorded_transport_with_context_diagnosed(
            caller,
            env,
            candidate,
            transport,
            |envelope, record, _| persist(envelope, record),
            trace,
        )
    }

    /// The unique durable writer may consume a small ephemeral proof emitted by the prepared
    /// transaction without copying it into each persistent JournalRecord/Receipt.
    pub fn execute_recorded_transport_with_context_diagnosed<E>(
        &mut self,
        caller: Caller,
        env: Envelope,
        candidate: Option<&crate::air::State>,
        transport: Option<&crate::market_access::SettlementDeliveryInput<'_>>,
        persist: impl FnOnce(&Envelope, &JournalRecord, CommitContext<'_>) -> Result<(), E>,
        trace: &mut Trace<'_>,
    ) -> Result<Result<Receipt, TxError>, E> {
        if env.world != self.world.scope.world() || env.branch != self.world.scope.branch() {
            return Ok(Err(TxError::WrongScope));
        }
        if env.actions.iter().any(|a|matches!(a,Action::Inbox{command:crate::inbox::Command::External{fact}} if env.operation_id!=fact.operation_id())) {return Ok(Err(TxError::InvalidOperation));}
        if env.operation_id >= 0xf000000000000000
            && !(matches!(caller, Caller::System)
                && env.actions.len() == 1
                && matches!(env.actions[0],Action::Inbox{command:crate::inbox::Command::External{fact}} if fact.operation_id()==env.operation_id))
        {
            return Ok(Err(TxError::InvalidOperation));
        }
        let background_accept_id =
            env.operation_id & 0xf000_0000_0000_0000 == crate::background::ACCEPT_PREFIX;
        if !valid_background_operation(caller, &env) {
            return Ok(Err(TxError::InvalidOperation));
        }
        if env.operation_id == 0 || matches!(caller, Caller::Account(0)) {
            return Ok(Err(TxError::InvalidOperation));
        }
        if env.actions.is_empty() || env.actions.len() > MAX_ACTIONS {
            return Ok(Err(TxError::TooManyActions));
        }
        if let Caller::Account(id) = caller {
            if self.world.account_handle(id).is_err() {
                return Ok(Err(TxError::Unauthorized));
            }
        }
        let encode_started = trace.start();
        let digest: [u8; 32] = {
            use sha2::{Digest, Sha256};
            Sha256::digest(serde_json::to_vec(&env).expect("bounded typed envelope")).into()
        };
        trace.end(Class::Encode, encode_started, 0, 1, 0);
        if let Some(old) = self
            .journal
            .iter()
            .find(|r| r.operation_id == env.operation_id)
        {
            if old.actor != caller.key() {
                return Ok(Err(TxError::Unauthorized));
            }
            trace.summary.replayed = true;
            return Ok(if old.payload_sha256 == digest {
                old.outcome
            } else {
                Err(TxError::OperationConflict)
            });
        }
        if self.journal.len() == MAX_RECEIPTS {
            return Ok(Err(TxError::JournalFull));
        }
        let normal_full = self
            .world
            .exchange
            .is_some_and(|e| e.inbox.admission_full());
        let prepare_started = trace.start();
        let prepared = if normal_full && env.actions.iter().any(Action::is_new_commitment) {
            Err(TxError::InboxFull)
        } else if env.expected_revision != self.world.revision {
            Err(TxError::StaleRevision)
        } else {
            self.prepare_diagnosed(caller, &env, candidate, transport, trace)
        };
        trace.end(
            Class::Prepare,
            prepare_started,
            0,
            u64::from(prepared.is_ok()),
            0,
        );
        // A stale lease or unavailable candidate must not burn the job's final receipt.
        // The hot/cold duplicate paths still return the original committed admission.
        if background_accept_id {
            if let Err(error) = prepared.as_ref() {
                return Ok(Err(*error));
            }
        }
        let outcome = prepared.as_ref().map(|p| p.receipt).map_err(|e| *e);
        let mut actions = [None; MAX_ACTIONS];
        for (i, a) in env.actions.iter().enumerate() {
            actions[i] = Some(*a)
        }
        let record = JournalRecord {
            actor: caller.key(),
            emergency_eligible: prepared
                .as_ref()
                .is_ok_and(|p| self.emergency_progress(caller, &env, p)),
            operation_id: env.operation_id,
            expected_revision: env.expected_revision,
            payload_sha256: digest,
            outcome,
            actions,
        };
        // Build the compact retry summary before the durable callback. A conversion failure or
        // allocation panic must happen before publication, never after a successful WAL commit.
        let hot_record = HotJournalRecord::from_durable(&record);
        let commit_context = prepared.as_ref().map_or_else(
            |_| CommitContext::default(),
            |prepared| CommitContext {
                spatial_update_grant: prepared.spatial_update_grant,
                spatial_permissions: prepared.spatial_permissions.as_deref(),
                prior_spatial_permissions: prepared
                    .spatial_permissions
                    .as_ref()
                    .and(self.spatial_permissions.as_ref()),
            },
        );
        persist(&env, &record, commit_context)?;
        trace.summary.hook_succeeded = true;
        if let Ok(change) = prepared {
            let started = trace.start();
            self.apply(change);
            trace.end(Class::Apply, started, 0, 1, 0);
            trace.summary.applied = true;
        }
        self.journal.push(hot_record);
        Ok(outcome)
    }

    pub fn can_advance_existing_responsibility(&self, caller: Caller, env: &Envelope) -> bool {
        self.can_advance_existing_responsibility_diagnosed(caller, env, &mut Trace::disabled())
    }
    pub fn can_advance_existing_responsibility_diagnosed(
        &self,
        caller: Caller,
        env: &Envelope,
        trace: &mut Trace<'_>,
    ) -> bool {
        if env.world != self.world.scope.world()
            || env.branch != self.world.scope.branch()
            || env.expected_revision != self.world.revision
        {
            return false;
        }
        let old = trace.role;
        trace.role = Role::Quote;
        let started = trace.start();
        let prepared = self.prepare_diagnosed(caller, env, None, None, trace);
        trace.end(Class::Prepare, started, 0, u64::from(prepared.is_ok()), 0);
        trace.role = old;
        prepared
            .as_ref()
            .is_ok_and(|p| self.emergency_progress(caller, env, p))
    }
    fn emergency_progress(&self, _caller: Caller, env: &Envelope, p: &Prepared) -> bool {
        if env.actions.len() != 1 {
            return false;
        }
        match env.actions[0] {
            Action::Timeline {
                command:
                    crate::timeline::Command::Advance { .. }
                    | crate::timeline::Command::SettleClaim { .. }
                    | crate::timeline::Command::SettlePayroll { .. }
                    | crate::timeline::Command::Terminate { .. },
            } => {
                self.timeline
                    .as_ref()
                    .zip(p.timeline.as_ref())
                    .is_some_and(|(old, new)| {
                        matches!(
                            env.actions[0],
                            Action::Timeline {
                                command: crate::timeline::Command::Advance { .. }
                            }
                        ) && old.progress.phase.is_some()
                            && old.progress.at == new.progress.at
                            && old.progress.target == new.progress.target.or(old.progress.target)
                            && new.progress.serial == old.progress.serial + 1
                    })
                    || p.timeline
                        .as_ref()
                        .is_some_and(|t| t.progress.page_paid.value() > 0)
                    || self
                        .world
                        .boot
                        .zip(p.boot)
                        .is_some_and(|(a, b)| a.jobs != b.jobs)
                    || self
                        .world
                        .exchange
                        .zip(p.exchange)
                        .is_some_and(|(a, b)| a.shipments != b.shipments)
                    || self.world.region.zip(p.region).is_some_and(|(a, b)| {
                        a.transits != b.transits || a.obligations != b.obligations
                    })
            }
            Action::Inbox { command }
                if !matches!(
                    command,
                    crate::inbox::Command::Enable | crate::inbox::Command::Receive { .. }
                ) =>
            {
                self.world
                    .exchange
                    .zip(p.exchange)
                    .is_some_and(|(a, b)| a.inbox != b.inbox)
            }
            Action::Exchange {
                command: crate::exchange::Command::Cancel { .. },
            } => self
                .world
                .exchange
                .zip(p.exchange)
                .is_some_and(|(a, b)| a.quotes != b.quotes),
            Action::Exchange {
                command: crate::exchange::Command::SetLink { .. },
            } => self.world.exchange.zip(p.exchange).is_some_and(|(a, b)| {
                a.open != b.open
                    && (a.link_faults != b.link_faults
                        || a.link_repairs != b.link_repairs
                        || a.population_link_controls != b.population_link_controls)
            }),
            Action::Boot {
                command:
                    crate::boot::Command::Fault { .. } | crate::boot::Command::ClearFailure { .. },
            } => self.world.boot != p.boot,
            Action::RegionAdvance { seconds } => {
                seconds > 0
                    && (self.world.boot.is_some_and(|b| {
                        b.jobs
                            .iter()
                            .any(|j| j.phase == crate::boot::Phase::Running)
                    }) || self.world.exchange.is_some_and(|e| {
                        e.shipments.iter().flatten().any(|s| s.arrives_at.is_some())
                    }) || self.world.region.is_some_and(|r| {
                        r.transits
                            .iter()
                            .flatten()
                            .any(|t| t.pause_remaining.is_none())
                            || r.obligations.iter().flatten().any(|o| o.next_due.is_some())
                    }))
            }
            Action::RegionSettleDebt { .. } => self
                .world
                .region
                .zip(p.region)
                .is_some_and(|(a, b)| a.obligations != b.obligations),
            Action::ReleaseCapacity { .. } => {
                self.leases.iter().flatten().count() > p.leases.iter().flatten().count()
            }
            _ => false,
        }
    }
    fn prepare_diagnosed(
        &self,
        caller: Caller,
        env: &Envelope,
        candidate: Option<&crate::air::State>,
        transport: Option<&crate::market_access::SettlementDeliveryInput<'_>>,
        trace: &mut Trace<'_>,
    ) -> Result<Prepared, TxError> {
        let next_revision = self
            .world
            .revision
            .checked_add(1)
            .ok_or(TxError::Overflow)?;
        let storage = self
            .preparation_workspace
            .as_ref()
            .ok_or(TxError::WorkingLimit)?;
        let mut timeline = workspace::TimelineLease::acquire(&storage.timeline, &self.timeline)?;
        let mut accounts = workspace::AccountLease::take(&storage.accounts)?;
        let mut stocks = workspace::StockLease::take(&storage.stocks)?;
        let mut resources = workspace::ResourceLease::take(&storage.resources)?;
        let mut leases = workspace::CapacityLease::take(&storage.leases)?;
        resources.copy_active_from(&self.resources);
        leases.copy_active_from(&self.leases);
        let mut next_token = self.next_token;
        let mut created_tokens = [None; MAX_ACTIONS];
        let mut migrated = [None; MAX_ACTIONS];
        let mut created_facts = [None; MAX_ACTIONS];
        let mut settled = [None; MAX_ACTIONS];
        let mut region = self.world.region;
        let mut economy = self.world.economy;
        let mut boot = self.world.boot;
        let mut exchange = self.world.exchange;
        let mut construction = self.world.construction;
        let mut town = self.world.town.map(|m| m.state);
        let mut housing: Option<crate::housing::HousingState> = None;
        let mut spatial_permissions: Option<workspace::SpatialPermissionLease> = None;
        let mut spatial_update_grant = None;
        let mut town_summary = None;
        let mut background =
            workspace::BackgroundLease::acquire(&storage.background, &self.background)?;
        let mut background_work = workspace::BackgroundWorkLease::take(&storage.background_work)?;
        for (action_index, action) in env.actions.iter().enumerate() {
            if matches!(
                action,
                Action::Timeline {
                    command: crate::timeline::Command::Advance { .. }
                        | crate::timeline::Command::SettleClaim { .. }
                }
            ) && housing.is_none()
            {
                housing = self
                    .world
                    .housing
                    .as_deref()
                    .map(|runtime| runtime.state.clone());
            }
            let domain_start = if matches!(
                action,
                Action::Boot { .. }
                    | Action::Exchange { .. }
                    | Action::RegionAdvance { .. }
                    | Action::RegionActivate { .. }
                    | Action::RegionMigrate { .. }
                    | Action::RegionSettleDebt { .. }
            ) {
                trace.start().map(|s| (s, region, boot, exchange))
            } else {
                None
            };
            if timeline
                .as_ref()
                .is_some_and(|t| t.progress.target.is_some())
                && !matches!(
                    action,
                    Action::Background { .. }
                        | Action::Timeline {
                            command: crate::timeline::Command::Advance { .. }
                                | crate::timeline::Command::CancelAdvance
                        }
                        | Action::Inbox {
                            command: crate::inbox::Command::External { .. }
                        }
                )
                && !safe_inbox_release(action, exchange.as_ref())
            {
                return Err(TxError::Timeline(crate::timeline::Error::Busy));
            }
            if timeline.is_some()
                && matches!(
                    action,
                    Action::RegionAdvance { .. }
                        | Action::Economy {
                            command: crate::economy::Command::Advance { .. }
                        }
                        | Action::Boot {
                            command: crate::boot::Command::Advance { .. }
                        }
                )
            {
                return Err(TxError::Timeline(crate::timeline::Error::LegacyClock));
            }
            match *action {
                Action::Background { command } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    if matches!(command, crate::background::Command::Enable) {
                        let t = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                        if background.is_some()
                            || region.is_none()
                            || exchange.as_ref().is_none_or(|x| {
                                // This finite laboratory field covers freight source maps,
                                // not every discovered city's unrelated metric frame.
                                x.maps.iter().any(|m| {
                                    x.quotes.iter().any(|q| {
                                        q.spec.kind == crate::exchange::Kind::Freight
                                            && q.spec.from == m.city
                                    }) && (m.origin_x_cm < 0
                                        || m.origin_x_cm
                                            > i64::from(crate::background::SIDE) * 3200 - 102400)
                                }) || x.quotes.iter().any(|q| {
                                    q.spec.kind == crate::exchange::Kind::Freight
                                        && q.spec.quantity.value() > 1_000_000
                                })
                            })
                            || t.progress.target.is_some()
                            || t.progress.last_complete != 0
                        {
                            return Err(TxError::InvalidOperation);
                        }
                        background.activate();
                    } else {
                        if !matches!(command, crate::background::Command::Restart)
                            && timeline.as_ref().is_none_or(|t| {
                                t.progress.phase != Some(crate::schedule::Phase::P4)
                            })
                        {
                            return Err(TxError::InvalidOperation);
                        }
                        let started = trace.start();
                        let result = background
                            .as_mut()
                            .ok_or(TxError::InvalidOperation)?
                            .command_diagnosed(command, candidate, trace, &mut background_work)
                            .map_err(|_| TxError::InvalidOperation);
                        if matches!(command, crate::background::Command::Accept { .. }) {
                            trace.end(Class::Admit, started, 0, u64::from(result.is_ok()), 0);
                        }
                        result?;
                    }
                }
                Action::Timeline { command } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    if env.actions.len() != 1 {
                        return Err(TxError::Timeline(crate::timeline::Error::Input));
                    }
                    if let crate::timeline::Command::Lend {
                        lender, borrower, ..
                    }
                    | crate::timeline::Command::Claims {
                        payer: lender,
                        payee: borrower,
                        ..
                    } = command
                    {
                        if [lender, borrower].iter().any(|id| {
                            market_is_suspended(town.as_ref(), *id)
                                || market_is_exiting(town.as_ref(), *id)
                        }) {
                            return Err(TxError::InvalidOperation);
                        }
                    }
                    let payroll_id = match command {
                        crate::timeline::Command::StopWork { id }
                        | crate::timeline::Command::ChangeCycle { id, .. }
                        | crate::timeline::Command::Terminate { id }
                        | crate::timeline::Command::EndForExit { id } => Some(id),
                        _ => None,
                    };
                    if payroll_id.is_some_and(|id| {
                        timeline
                            .as_ref()
                            .and_then(|t| t.payroll.iter().find(|row| row.spec.id == id))
                            .is_some_and(|row| market_is_exiting(town.as_ref(), row.spec.payer))
                    }) {
                        return Err(TxError::InvalidOperation);
                    }
                    timeline_driver::Frame {
                        trace,
                        world: &self.world,
                        town: &mut town,
                        background: &mut background,
                        background_work: &mut background_work,
                        region: &mut region,
                        economy: &mut economy,
                        boot: &mut boot,
                        exchange: &mut exchange,
                        accounts: &mut accounts,
                        stocks: &mut stocks,
                        resources: &mut resources,
                        leases: &mut leases,
                        housing: &mut housing,
                        next_token: &mut next_token,
                    }
                    .command(&mut timeline, command)?;
                }
                Action::Inbox { command } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    let previous_inbox = exchange.as_ref().ok_or(TxError::InvalidOperation)?.inbox;
                    exchange
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .inbox_command(
                            command,
                            region
                                .as_ref()
                                .ok_or(TxError::InvalidOperation)?
                                .interaction_time,
                            &mut BootLedger {
                                base: EconomicLedger {
                                    world: &self.world,
                                    accounts: &mut accounts,
                                    stocks: &mut stocks,
                                },
                                resources: &mut resources,
                                leases: &mut leases,
                                next_token: &mut next_token,
                            },
                        )?;
                    let next = exchange.as_ref().ok_or(TxError::InvalidOperation)?.inbox;
                    if next.next_sequence != previous_inbox.next_sequence {
                        created_facts[action_index] = Some(previous_inbox.next_sequence);
                    }
                    if let crate::inbox::Command::Settle { sequence } = command {
                        let before = previous_inbox
                            .all()
                            .find(|f| f.sequence == sequence)
                            .ok_or(TxError::InvalidOperation)?
                            .paid;
                        let after = next
                            .all()
                            .find(|f| f.sequence == sequence)
                            .ok_or(TxError::InvalidOperation)?
                            .paid;
                        settled[action_index] =
                            Some(after.checked_sub(before).map_err(|_| TxError::Overflow)?);
                    }
                }
                Action::Exchange { command } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    if let crate::exchange::Command::Dispatch { quote } = command {
                        let e = exchange.as_ref().ok_or(TxError::InvalidOperation)?;
                        let spec = e
                            .quotes
                            .iter()
                            .find(|row| row.spec.id == quote)
                            .ok_or(TxError::InvalidOperation)?
                            .spec;
                        if [spec.seller, spec.buyer, spec.carrier].iter().any(|id| {
                            market_is_suspended(town.as_ref(), *id)
                                || market_is_exiting(town.as_ref(), *id)
                        }) {
                            return Err(TxError::InvalidOperation);
                        }
                    }
                    let previous = exchange
                        .as_ref()
                        .ok_or(TxError::InvalidOperation)?
                        .inbox
                        .next_sequence;
                    exchange
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .command(
                            command,
                            region.as_mut().ok_or(TxError::InvalidOperation)?,
                            boot.as_mut().ok_or(TxError::InvalidOperation)?,
                            &mut BootLedger {
                                base: EconomicLedger {
                                    world: &self.world,
                                    accounts: &mut accounts,
                                    stocks: &mut stocks,
                                },
                                resources: &mut resources,
                                leases: &mut leases,
                                next_token: &mut next_token,
                            },
                        )?;
                    if exchange
                        .as_ref()
                        .ok_or(TxError::InvalidOperation)?
                        .inbox
                        .next_sequence
                        != previous
                    {
                        created_facts[action_index] = Some(previous);
                    }
                }
                Action::Boot { command } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    if matches!(command, crate::boot::Command::Start { .. }) {
                        let b = boot.as_ref().ok_or(TxError::InvalidOperation)?;
                        if [b.operator, b.contractor].iter().any(|id| {
                            market_is_suspended(town.as_ref(), *id)
                                || market_is_exiting(town.as_ref(), *id)
                        }) {
                            return Err(TxError::InvalidOperation);
                        }
                    }
                    if let Some(e) = exchange {
                        let r = region.as_ref().ok_or(TxError::InvalidOperation)?;
                        if matches!(command, crate::boot::Command::Advance { .. })
                            || matches!(command, crate::boot::Command::Start { .. })
                                && r.active_city != e.home_city
                        {
                            return Err(TxError::Exchange(crate::exchange::Error::Clock));
                        }
                    }
                    let b = boot.as_mut().ok_or(TxError::InvalidOperation)?;
                    b.command(
                        command,
                        &mut BootLedger {
                            base: EconomicLedger {
                                world: &self.world,
                                accounts: &mut accounts,
                                stocks: &mut stocks,
                            },
                            resources: &mut resources,
                            leases: &mut leases,
                            next_token: &mut next_token,
                        },
                    )?;
                }
                Action::Economy { command } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    let e = economy.as_mut().ok_or(TxError::InvalidOperation)?;
                    e.command(
                        command,
                        &mut EconomicLedger {
                            world: &self.world,
                            accounts: &mut accounts,
                            stocks: &mut stocks,
                        },
                    )?;
                }
                Action::Construction { command } => {
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    let p = construction.as_mut().ok_or(TxError::InvalidOperation)?;
                    if (construction_operation_requires_active_business(command)
                        && [p.owner, p.contractor]
                            .iter()
                            .any(|id| market_is_suspended(town.as_ref(), *id)))
                        || [p.owner, p.contractor]
                            .iter()
                            .any(|id| market_is_exiting(town.as_ref(), *id))
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    prepare_construction(
                        caller,
                        command,
                        env.operation_id,
                        &self.world,
                        &mut boot,
                        p,
                        &mut accounts,
                        &mut stocks,
                        &mut resources,
                        &mut leases,
                        &mut next_token,
                        &mut created_tokens[action_index],
                        timeline.as_ref(),
                        town.as_ref(),
                    )?;
                }
                Action::HousingLease {
                    offer_id,
                    offer_sha256,
                } => {
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    if housing.is_none() {
                        housing = self
                            .world
                            .housing
                            .as_deref()
                            .map(|runtime| runtime.state.clone());
                    }
                    let state = housing.as_mut().ok_or(TxError::InvalidOperation)?;
                    let timeline_state = timeline.as_mut().ok_or(TxError::InvalidOperation)?;
                    prepare_housing_lease(
                        caller,
                        offer_id,
                        offer_sha256,
                        &self.world,
                        state,
                        timeline_state,
                        &mut accounts,
                    )?;
                }
                Action::SpatialPermission { command } => {
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    let Caller::Account(actor) = caller else {
                        return Err(TxError::Unauthorized);
                    };
                    let current = self
                        .spatial_permissions
                        .as_ref()
                        .ok_or(TxError::InvalidOperation)?;
                    let boundary = if matches!(
                        command,
                        crate::spatial_transport_permissions::SpatialPermissionCommand::Apply { .. }
                    ) {
                        let t = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                        let at = t.progress.last_complete;
                        let complete = t.progress.phase.is_none()
                            && t.progress.target.is_none()
                            && t.progress.at == at;
                        let at = i64::try_from(at).map_err(|_| TxError::InvalidOperation)?;
                        Some(
                            crate::spatial_transport_permissions::CompletedDayBoundary::from_engine_timeline(
                                self.world.scope.world(),
                                self.world.scope.branch(),
                                crate::units::GameTime::new(at)
                                    .map_err(|_| TxError::InvalidOperation)?,
                                complete,
                            )
                            .map_err(|_| TxError::InvalidOperation)?,
                        )
                    } else {
                        None
                    };
                    let owner = &storage.spatial_permissions;
                    let mut next = workspace::SpatialPermissionLease::take(owner)?;
                    let grant = current
                        .copy_and_apply_into(&mut next, actor, command, boundary)
                        .map_err(|_| TxError::InvalidOperation)?;
                    spatial_permissions = Some(next);
                    spatial_update_grant = grant;
                }
                Action::TownSettle { profile_id } => {
                    if !matches!(caller, Caller::System) || env.actions.len() != 1 {
                        return Err(TxError::Unauthorized);
                    }
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    if market.profile.id != profile_id
                        || market
                            .profile
                            .transport_allocation_sha256
                            .iter()
                            .any(|byte| *byte != 0)
                        || town.is_none()
                        || market
                            .profile
                            .equity_charters
                            .as_slice()
                            .iter()
                            .any(|charter| {
                                town.as_ref().is_none_or(|state| {
                                    state.equity_registers.as_slice().iter().all(|register| {
                                        register.company != charter.company
                                            || register.issued_shares == 0
                                    })
                                })
                            })
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let t = timeline.as_mut().ok_or(TxError::InvalidOperation)?;
                    town_summary = Some(prepare_town_settlement(TownSettlementContext {
                        operation_id: env.operation_id,
                        world_id: self.world.scope.world(),
                        branch_id: self.world.scope.branch(),
                        revision: self.world.revision,
                        world: &self.world,
                        profile: &market.profile,
                        state: &mut town,
                        timeline: t,
                        accounts: &mut accounts,
                        stocks: &mut stocks,
                        boot: &mut boot,
                        resources: &mut resources,
                        leases: &mut leases,
                        next_token: &mut next_token,
                        transport: None,
                    })?);
                }
                Action::TownTransportSettle {
                    profile_id,
                    service_day,
                    generation,
                    proof_sha256,
                } => {
                    if !matches!(caller, Caller::System) || env.actions.len() != 1 {
                        return Err(TxError::Unauthorized);
                    }
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    let proof = transport.ok_or(TxError::InvalidOperation)?;
                    let expected_allocation_sha256 =
                        if let Some(permissions) = self.spatial_permissions.as_ref() {
                            let Some(first) = permissions.assets().first() else {
                                return Err(TxError::InvalidOperation);
                            };
                            if first.current_profile_sha256 == [0; 32]
                                || permissions.assets().iter().any(|asset| {
                                    asset.current_profile_sha256 != first.current_profile_sha256
                                })
                            {
                                return Err(TxError::InvalidOperation);
                            }
                            first.current_profile_sha256
                        } else {
                            market.profile.transport_allocation_sha256
                        };
                    if market.profile.id != profile_id
                        || market
                            .profile
                            .transport_allocation_sha256
                            .iter()
                            .all(|byte| *byte == 0)
                        || town.is_none()
                        || market
                            .profile
                            .equity_charters
                            .as_slice()
                            .iter()
                            .any(|charter| {
                                town.as_ref().is_none_or(|state| {
                                    state.equity_registers.as_slice().iter().all(|register| {
                                        register.company != charter.company
                                            || register.issued_shares == 0
                                    })
                                })
                            })
                        || proof.proof_sha256 != proof_sha256
                        || proof.proof.schema != 1
                        || proof.proof.profile_id != profile_id
                        || proof.proof.allocation_profile_sha256 != expected_allocation_sha256
                        || proof.proof.generation != generation
                        || proof.proof.period_seconds != crate::timeline::DAY as u32
                        || proof.proof.world_id != self.world.scope.world()
                        || proof.proof.branch_id != self.world.scope.branch()
                        || proof.proof.service_day != service_day
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let t = timeline.as_mut().ok_or(TxError::InvalidOperation)?;
                    let expected_generation = town
                        .as_ref()
                        .and_then(|state| state.last_transport)
                        .map_or(0, |receipt| receipt.consumed_generation)
                        .checked_add(1)
                        .ok_or(TxError::Overflow)?;
                    if generation != expected_generation {
                        return Err(TxError::InvalidOperation);
                    }
                    town_summary = Some(prepare_town_settlement(TownSettlementContext {
                        operation_id: env.operation_id,
                        world_id: self.world.scope.world(),
                        branch_id: self.world.scope.branch(),
                        revision: self.world.revision,
                        world: &self.world,
                        profile: &market.profile,
                        state: &mut town,
                        timeline: t,
                        accounts: &mut accounts,
                        stocks: &mut stocks,
                        boot: &mut boot,
                        resources: &mut resources,
                        leases: &mut leases,
                        next_token: &mut next_token,
                        transport: Some(proof),
                    })?);
                }
                Action::TownSuspendOperations {
                    profile_id,
                    company_account,
                } => {
                    if !matches!(caller, Caller::System) || env.actions.len() != 1 {
                        return Err(TxError::Unauthorized);
                    }
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    let profile = &market.profile;
                    let current_timeline = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                    let state = town.as_ref().ok_or(TxError::InvalidOperation)?;
                    let at = current_timeline.progress.last_complete;
                    if profile.id != profile_id
                        || company_account == 0
                        || market_is_exiting(town.as_ref(), company_account)
                        || !(profile
                            .employers
                            .as_slice()
                            .iter()
                            .any(|row| row.account == company_account)
                            || profile
                                .shops
                                .as_slice()
                                .iter()
                                .any(|row| row.account == company_account))
                        || current_timeline.progress.phase.is_some()
                        || current_timeline.progress.target.is_some()
                        || at == 0
                        || at % crate::timeline::DAY != 0
                        || state.settled_day != at / crate::timeline::DAY
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let already_suspended = state
                        .suspended_operators
                        .as_slice()
                        .iter()
                        .any(|row| row.account == company_account);
                    if !already_suspended {
                        if active_company_dependency(
                            company_account,
                            exchange.as_ref(),
                            boot.as_ref(),
                            construction.as_ref(),
                            &leases,
                        ) {
                            return Err(TxError::InvalidOperation);
                        }
                        let payroll_ids: Vec<u32> = current_timeline
                            .payroll
                            .iter()
                            .filter(|row| row.spec.payer == company_account)
                            .map(|row| row.spec.id)
                            .collect();
                        for id in payroll_ids {
                            timeline_driver::Frame {
                                trace,
                                world: &self.world,
                                town: &mut town,
                                background: &mut background,
                                background_work: &mut background_work,
                                region: &mut region,
                                economy: &mut economy,
                                boot: &mut boot,
                                exchange: &mut exchange,
                                accounts: &mut accounts,
                                stocks: &mut stocks,
                                resources: &mut resources,
                                leases: &mut leases,
                                housing: &mut housing,
                                next_token: &mut next_token,
                            }
                            .command(&mut timeline, crate::timeline::Command::EndForExit { id })?;
                        }
                        let state = town.as_mut().ok_or(TxError::InvalidOperation)?;
                        let row = crate::town_runtime::SuspendedOperator {
                            account: company_account,
                            suspended_at: at,
                        };
                        state
                            .suspended_operators
                            .push(row)
                            .map_err(|_| TxError::InvalidOperation)?;
                    }
                }
                Action::EquityContribute { company, amount } => {
                    if env.actions.len() != 1 || amount <= Money::ZERO {
                        return Err(TxError::InvalidOperation);
                    }
                    let Caller::Account(investor_account) = caller else {
                        return Err(TxError::Unauthorized);
                    };
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    if market.state.settled_day != 0
                        || timeline.as_ref().is_some_and(|timeline| {
                            timeline.progress.at != 0
                                || timeline.progress.last_complete != 0
                                || timeline.progress.phase.is_some()
                                || timeline.progress.target.is_some()
                        })
                    {
                        // Opening contributions and share issuance happen before operations;
                        // later fundraising/dilution requires a separately authorized issue.
                        return Err(TxError::InvalidOperation);
                    }
                    let charter = market
                        .profile
                        .equity_charters
                        .as_slice()
                        .iter()
                        .find(|charter| charter.company == company)
                        .ok_or(TxError::InvalidOperation)?;
                    let party = charter
                        .binding_for_account(investor_account)
                        .ok_or(TxError::Unauthorized)?;
                    let account_role = self
                        .world
                        .account_handle(investor_account)
                        .ok()
                        .and_then(|handle| self.world.accounts.get(handle).ok())
                        .map(|account| account.role)
                        .ok_or(TxError::InvalidHandle)?;
                    let role_matches = match party.kind {
                        crate::town_economy::equity::PartyKind::Person
                        | crate::town_economy::equity::PartyKind::Household => {
                            account_role == crate::assembly::Role::Household
                        }
                        crate::town_economy::equity::PartyKind::Treasury => {
                            account_role == crate::assembly::Role::Treasury
                        }
                        crate::town_economy::equity::PartyKind::Company => false,
                    };
                    if !role_matches || market_is_exiting(town.as_ref(), company) {
                        return Err(TxError::Unauthorized);
                    }
                    let shares = u64::try_from(amount.value())
                        .map_err(|_| TxError::InvalidOperation)?
                        / u64::try_from(crate::town_economy::equity::INITIAL_CENTS_PER_SHARE)
                            .map_err(|_| TxError::InvalidOperation)?;
                    if shares == 0
                        || i128::from(shares)
                            * i128::from(crate::town_economy::equity::INITIAL_CENTS_PER_SHARE)
                            != i128::from(amount.value())
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let register = town
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .equity_registers
                        .as_mut_slice()
                        .iter_mut()
                        .find(|register| register.company == company)
                        .ok_or(TxError::InvalidOperation)?;
                    register.validate().map_err(|_| TxError::InvalidOperation)?;
                    if register.policy != charter.policy
                        || charter.binding_for_party(party) != Some(investor_account)
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    // The non-serializable proof is minted only after the protected cash
                    // transfer has succeeded in this same Prepared candidate.
                    market_transfer(
                        &self.world,
                        &mut accounts,
                        investor_account,
                        company,
                        amount,
                    )?;
                    let transfer =
                        crate::town_economy::equity::CommittedCashTransfer::after_staged_transfer(
                            env.operation_id,
                            investor_account,
                            company,
                            amount,
                        )
                        .map_err(|_| TxError::InvalidOperation)?;
                    register
                        .commit_paid_contribution(
                            crate::town_economy::equity::PaidContribution {
                                party,
                                account: investor_account,
                                shares,
                                amount,
                                receipt_id: env.operation_id,
                            },
                            transfer,
                        )
                        .map_err(|_| TxError::InvalidOperation)?;
                    settled[action_index] = Some(amount);
                }
                Action::EquityDistributeRetainedEarnings { company, requested } => {
                    if env.actions.len() != 1 || requested <= Money::ZERO {
                        return Err(TxError::InvalidOperation);
                    }
                    let Caller::Account(caller_account) = caller else {
                        return Err(TxError::Unauthorized);
                    };
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    let charter = market
                        .profile
                        .equity_charters
                        .as_slice()
                        .iter()
                        .find(|charter| charter.company == company)
                        .copied()
                        .ok_or(TxError::InvalidOperation)?;
                    if charter.binding_for_party(charter.distribution_authority)
                        != Some(caller_account)
                    {
                        return Err(TxError::Unauthorized);
                    }
                    let authority_role = self
                        .world
                        .account_handle(caller_account)
                        .ok()
                        .and_then(|handle| self.world.accounts.get(handle).ok())
                        .map(|account| account.role)
                        .ok_or(TxError::InvalidHandle)?;
                    let authority_role_matches = match charter.distribution_authority.kind {
                        crate::town_economy::equity::PartyKind::Person
                        | crate::town_economy::equity::PartyKind::Household => {
                            authority_role == crate::assembly::Role::Household
                        }
                        crate::town_economy::equity::PartyKind::Treasury => {
                            authority_role == crate::assembly::Role::Treasury
                        }
                        crate::town_economy::equity::PartyKind::Company => false,
                    };
                    if !authority_role_matches {
                        return Err(TxError::Unauthorized);
                    }
                    let policy = market
                        .profile
                        .exit_policy
                        .filter(|policy| policy.company_account == company)
                        .ok_or(TxError::InvalidOperation)?;
                    let current = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                    let state = town.as_ref().ok_or(TxError::InvalidOperation)?;
                    let at = current.progress.last_complete;
                    if market_is_exiting(town.as_ref(), company)
                        || market_is_suspended(town.as_ref(), company)
                        || current.progress.phase.is_some()
                        || current.progress.target.is_some()
                        || at == 0
                        || at % crate::timeline::DAY != 0
                        || state.settled_day != at / crate::timeline::DAY
                        || !policy.claims_complete
                        || !policy.asset_register_complete
                        || !policy.employment_exit_terms_complete
                        || !exit_timeline_bindings_match(&policy, current, &market.profile, state)
                        || !tax_assessments_cover_exit(company, &market.profile, state, current)
                        || dividend_has_unresolved_or_unmodeled_liability(company, current)
                        || dividend_has_unmodeled_income_source(&self.world, company)
                    {
                        return Err(TxError::InvalidOperation);
                    }

                    let register = state
                        .equity_registers
                        .as_slice()
                        .iter()
                        .find(|register| register.company == company)
                        .copied()
                        .ok_or(TxError::InvalidOperation)?;
                    register.validate().map_err(|_| TxError::InvalidOperation)?;
                    if register.policy != charter.policy || register.issued_shares == 0 {
                        return Err(TxError::InvalidOperation);
                    }
                    let cursor = town
                        .as_ref()
                        .ok_or(TxError::InvalidOperation)?
                        .tax_cursors
                        .as_slice()
                        .iter()
                        .find(|cursor| cursor.company == company)
                        .copied()
                        .filter(|cursor| {
                            cursor.policy_id
                                == market
                                    .profile
                                    .tax_rules
                                    .as_slice()
                                    .iter()
                                    .find(|rule| rule.company == company)
                                    .map(|rule| rule.policy_id)
                                    .unwrap_or(0)
                        })
                        .ok_or(TxError::InvalidOperation)?;
                    if cursor.through_day != state.settled_day
                        || cursor.last_operation_id != state.last_summary.operation_id
                        || cursor.retained_earnings_cents <= 0
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let company_raw = self
                        .world
                        .account_ref(company)
                        .map_err(|_| TxError::InvalidHandle)?;
                    let company_index = stage_account(&self.world, &mut accounts, company_raw)?;
                    let spendable = accounts[company_index]
                        .1
                        .cash
                        .checked_sub(accounts[company_index].1.protected)
                        .map_err(|_| TxError::Protection)?;
                    let retained = Money::new(cursor.retained_earnings_cents)
                        .map_err(|_| TxError::InvalidOperation)?;
                    let amount = requested.min(retained).min(spendable);
                    if amount <= Money::ZERO {
                        return Err(TxError::InsufficientFunds);
                    }
                    let allocations =
                        crate::town_economy::equity::allocate_pro_rata(amount, register.holders())
                            .map_err(|_| TxError::InvalidOperation)?;
                    let mut paid = Money::ZERO;
                    for allocation in allocations {
                        if allocation.amount == Money::ZERO {
                            continue;
                        }
                        post_exit_payment(
                            &self.world,
                            &mut accounts,
                            company,
                            allocation.account,
                            allocation.amount,
                        )?;
                        paid = paid
                            .checked_add(allocation.amount)
                            .map_err(|_| TxError::Overflow)?;
                    }
                    if paid != amount {
                        return Err(TxError::InvalidOperation);
                    }
                    let cursor = town
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .tax_cursors
                        .as_mut_slice()
                        .iter_mut()
                        .find(|cursor| cursor.company == company)
                        .ok_or(TxError::InvalidOperation)?;
                    cursor.retained_earnings_cents = cursor
                        .retained_earnings_cents
                        .checked_sub(paid.value())
                        .ok_or(TxError::Overflow)?;
                    let register = town
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .equity_registers
                        .as_mut_slice()
                        .iter_mut()
                        .find(|register| register.company == company)
                        .ok_or(TxError::InvalidOperation)?;
                    register.cumulative_dividends_distributed = register
                        .cumulative_dividends_distributed
                        .checked_add(paid)
                        .map_err(|_| TxError::Overflow)?;
                    settled[action_index] = Some(paid);
                }
                Action::BeginExit { company, policy_id } => {
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    let policy = town_exit_policy(&self.world, company, policy_id)?;
                    if !matches!(caller, Caller::Account(id) if id == policy.requester) {
                        return Err(TxError::Unauthorized);
                    }
                    let current = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                    let state = town.as_ref().ok_or(TxError::InvalidOperation)?;
                    let at = current.progress.last_complete;
                    if current.progress.phase.is_some()
                        || current.progress.target.is_some()
                        || at == 0
                        || at % crate::timeline::DAY != 0
                        || state.settled_day != at / crate::timeline::DAY
                        || state.exit.is_some()
                        || state
                            .suspended_operators
                            .as_slice()
                            .iter()
                            .any(|row| row.account == company)
                        || town_exit_dependencies(
                            company,
                            exchange.as_ref(),
                            boot.as_ref(),
                            economy.as_ref(),
                            construction.as_ref(),
                            &leases,
                        )
                        || town_exit_profile_dependency(
                            &self.world.town.ok_or(TxError::InvalidOperation)?.profile,
                            company,
                        )
                        || region.as_ref().is_some_and(|r| {
                            r.obligations.iter().flatten().any(|obligation| {
                                obligation.payer == company || obligation.payee == company
                            })
                        })
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let market_profile = &self.world.town.ok_or(TxError::InvalidOperation)?.profile;
                    if !exit_timeline_bindings_match(
                        policy,
                        current,
                        market_profile,
                        town.as_ref().ok_or(TxError::InvalidOperation)?,
                    ) {
                        return Err(TxError::InvalidOperation);
                    }
                    if !tax_assessments_cover_exit(
                        company,
                        market_profile,
                        town.as_ref().ok_or(TxError::InvalidOperation)?,
                        current,
                    ) {
                        return Err(TxError::InvalidOperation);
                    }
                    if exit_has_unsupported_receivables(company, current) {
                        return Err(TxError::InvalidOperation);
                    }
                    // A queued payroll-cycle change is an unresolved future employment
                    // obligation. The bounded exit path can stop current payroll, but it
                    // does not adjudicate or silently discard a pending cycle amendment.
                    if exit_has_pending_payroll_cycle(company, current) {
                        return Err(TxError::InvalidOperation);
                    }
                    let payroll_ids: Vec<u32> = current
                        .payroll
                        .iter()
                        .filter(|row| row.spec.payer == company)
                        .map(|row| row.spec.id)
                        .collect();
                    for id in payroll_ids {
                        timeline_driver::Frame {
                            trace,
                            world: &self.world,
                            town: &mut town,
                            background: &mut background,
                            background_work: &mut background_work,
                            region: &mut region,
                            economy: &mut economy,
                            boot: &mut boot,
                            exchange: &mut exchange,
                            accounts: &mut accounts,
                            stocks: &mut stocks,
                            resources: &mut resources,
                            leases: &mut leases,
                            housing: &mut housing,
                            next_token: &mut next_token,
                        }
                        .command(&mut timeline, crate::timeline::Command::EndForExit { id })?;
                    }
                    let state = town.as_mut().ok_or(TxError::InvalidOperation)?;
                    state.exit = Some(crate::town_runtime::ExitRuntime {
                        policy_id,
                        company_account: company,
                        started_at: at,
                        phase: crate::town_runtime::ExitPhase::Liquidating,
                    });
                }
                Action::SettleExit { company } => {
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    let runtime = town
                        .as_ref()
                        .and_then(|state| state.exit)
                        .filter(|exit| exit.company_account == company)
                        .ok_or(TxError::InvalidOperation)?;
                    let policy = town_exit_policy(&self.world, company, runtime.policy_id)?;
                    if !matches!(caller, Caller::Account(id) if id == policy.executor) {
                        return Err(TxError::Unauthorized);
                    }
                    let current = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                    let at = current.progress.last_complete;
                    if runtime.phase != crate::town_runtime::ExitPhase::Liquidating
                        || current.progress.phase.is_some()
                        || current.progress.target.is_some()
                        || at == 0
                        || at % crate::timeline::DAY != 0
                        || town.as_ref().map(|state| state.settled_day)
                            != Some(at / crate::timeline::DAY)
                        || town_exit_dependencies(
                            company,
                            exchange.as_ref(),
                            boot.as_ref(),
                            economy.as_ref(),
                            construction.as_ref(),
                            &leases,
                        )
                        || town_exit_profile_dependency(&market.profile, company)
                        || region.as_ref().is_some_and(|r| {
                            r.obligations.iter().flatten().any(|obligation| {
                                obligation.payer == company || obligation.payee == company
                            })
                        })
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let current = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                    if !exit_timeline_bindings_match(
                        policy,
                        current,
                        &market.profile,
                        town.as_ref().ok_or(TxError::InvalidOperation)?,
                    ) {
                        return Err(TxError::InvalidOperation);
                    }
                    if exit_has_unsupported_receivables(company, current) {
                        return Err(TxError::InvalidOperation);
                    }
                    let snapshot = build_exit_snapshot(ExitSnapshotInputs {
                        world: &self.world,
                        policy,
                        profile: &market.profile,
                        market_state: town.as_ref().ok_or(TxError::InvalidOperation)?,
                        timeline: current,
                        accounts: &accounts,
                        stocks: &stocks,
                        operation_id: env.operation_id,
                    })?;
                    let mut plan = crate::town_economy::exit::plan_exit(&snapshot)
                        .map_err(|_| TxError::InvalidOperation)?;
                    if market
                        .profile
                        .equity_charters
                        .as_slice()
                        .iter()
                        .any(|charter| charter.company == company)
                    {
                        let register = town
                            .as_ref()
                            .and_then(|state| {
                                state
                                    .equity_registers
                                    .as_slice()
                                    .iter()
                                    .find(|register| register.company == company)
                            })
                            .ok_or(TxError::InvalidOperation)?;
                        apply_equity_exit_distribution(&mut plan, register)?;
                    }
                    let mut exit_settled = Money::ZERO;
                    for action in plan.actions {
                        match action {
                            crate::town_economy::exit::ExitAction::SettleLiability {
                                source,
                                payee,
                                amount,
                            } => match source {
                                crate::town_economy::exit::LiabilitySource::TimelineClaim {
                                    id,
                                } => {
                                    let timeline =
                                        timeline.as_mut().ok_or(TxError::InvalidOperation)?;
                                    let claim = timeline
                                        .claims
                                        .iter_mut()
                                        .find(|claim| {
                                            claim.id == id
                                                && claim.payer == company
                                                && claim.payee == payee
                                        })
                                        .ok_or(TxError::InvalidOperation)?;
                                    let due = claim
                                        .principal
                                        .checked_sub(claim.paid)
                                        .map_err(|_| TxError::InvalidOperation)?;
                                    if amount > due {
                                        return Err(TxError::InvalidOperation);
                                    }
                                    post_exit_payment(
                                        &self.world,
                                        &mut accounts,
                                        company,
                                        payee,
                                        amount,
                                    )?;
                                    exit_settled = exit_settled
                                        .checked_add(amount)
                                        .map_err(|_| TxError::Overflow)?;
                                    claim.paid = claim
                                        .paid
                                        .checked_add(amount)
                                        .map_err(|_| TxError::Overflow)?;
                                }
                                crate::town_economy::exit::LiabilitySource::PayrollArrears {
                                    id,
                                } => {
                                    let timeline =
                                        timeline.as_mut().ok_or(TxError::InvalidOperation)?;
                                    let payroll = timeline
                                        .payroll
                                        .iter_mut()
                                        .find(|row| {
                                            row.spec.id == id
                                                && row.spec.payer == company
                                                && row.spec.payee == payee
                                        })
                                        .ok_or(TxError::InvalidOperation)?;
                                    let due = payroll
                                        .accrued
                                        .checked_sub(payroll.paid)
                                        .map_err(|_| TxError::InvalidOperation)?;
                                    if payroll.working
                                        || payroll.next_due.is_some()
                                        || payroll.billed != payroll.accrued
                                        || amount > due
                                    {
                                        return Err(TxError::InvalidOperation);
                                    }
                                    post_exit_payment(
                                        &self.world,
                                        &mut accounts,
                                        company,
                                        payee,
                                        amount,
                                    )?;
                                    exit_settled = exit_settled
                                        .checked_add(amount)
                                        .map_err(|_| TxError::Overflow)?;
                                    payroll.paid = payroll
                                        .paid
                                        .checked_add(amount)
                                        .map_err(|_| TxError::Overflow)?;
                                }
                            },
                            crate::town_economy::exit::ExitAction::DistributeResidual {
                                shareholder,
                                amount,
                            } => {
                                if !snapshot.tax_assessment_complete
                                    || !snapshot.shareholders_complete
                                    || !snapshot.shareholders.iter().any(|row| {
                                        row.account == shareholder && amount > Money::ZERO
                                    })
                                {
                                    return Err(TxError::InvalidOperation);
                                }
                                post_exit_payment(
                                    &self.world,
                                    &mut accounts,
                                    company,
                                    shareholder,
                                    amount,
                                )?;
                                let register = town
                                    .as_mut()
                                    .and_then(|state| {
                                        state
                                            .equity_registers
                                            .as_mut_slice()
                                            .iter_mut()
                                            .find(|register| register.company == company)
                                    })
                                    .ok_or(TxError::InvalidOperation)?;
                                register
                                    .record_liquidation_distribution(amount)
                                    .map_err(|_| TxError::InvalidOperation)?;
                                exit_settled = exit_settled
                                    .checked_add(amount)
                                    .map_err(|_| TxError::Overflow)?;
                            }
                            _ => {}
                        }
                    }
                    if exit_settled > Money::ZERO {
                        settled[action_index] = Some(exit_settled);
                    }
                }
                Action::CloseExit { company } => {
                    if env.actions.len() != 1 {
                        return Err(TxError::InvalidOperation);
                    }
                    let market = self.world.town.ok_or(TxError::InvalidOperation)?;
                    let runtime = town
                        .as_ref()
                        .and_then(|state| state.exit)
                        .filter(|exit| exit.company_account == company)
                        .ok_or(TxError::InvalidOperation)?;
                    let policy = town_exit_policy(&self.world, company, runtime.policy_id)?;
                    if !matches!(caller, Caller::Account(id) if id == policy.executor) {
                        return Err(TxError::Unauthorized);
                    }
                    let current = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
                    let at = current.progress.last_complete;
                    if runtime.phase != crate::town_runtime::ExitPhase::Liquidating
                        || current.progress.phase.is_some()
                        || current.progress.target.is_some()
                        || at == 0
                        || at % crate::timeline::DAY != 0
                        || town.as_ref().map(|state| state.settled_day)
                            != Some(at / crate::timeline::DAY)
                        || town_exit_dependencies(
                            company,
                            exchange.as_ref(),
                            boot.as_ref(),
                            economy.as_ref(),
                            construction.as_ref(),
                            &leases,
                        )
                        || town_exit_profile_dependency(&market.profile, company)
                        || region.as_ref().is_some_and(|r| {
                            r.obligations.iter().flatten().any(|obligation| {
                                obligation.payer == company || obligation.payee == company
                            })
                        })
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let snapshot = build_exit_snapshot(ExitSnapshotInputs {
                        world: &self.world,
                        policy,
                        profile: &market.profile,
                        market_state: town.as_ref().ok_or(TxError::InvalidOperation)?,
                        timeline: timeline.as_ref().ok_or(TxError::InvalidOperation)?,
                        accounts: &accounts,
                        stocks: &stocks,
                        operation_id: env.operation_id,
                    })?;
                    if exit_has_unsupported_receivables(
                        company,
                        timeline.as_ref().ok_or(TxError::InvalidOperation)?,
                    ) {
                        return Err(TxError::InvalidOperation);
                    }
                    let plan = crate::town_economy::exit::plan_exit(&snapshot)
                        .map_err(|_| TxError::InvalidOperation)?;
                    if plan.status != crate::town_economy::exit::ExitStatus::ReadyToClose
                        || !plan.actions.iter().any(|action| {
                            matches!(action, crate::town_economy::exit::ExitAction::FinalizeCompanyExit { company: id } if *id == company)
                        })
                        || !plan.actions.iter().all(|action| {
                            matches!(
                                action,
                                crate::town_economy::exit::ExitAction::StopNewProduction { .. }
                                    | crate::town_economy::exit::ExitAction::StopNewInputs { .. }
                                    | crate::town_economy::exit::ExitAction::FinalizeCompanyExit { .. }
                            )
                        })
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let account = accounts
                        .iter()
                        .find(|(handle, _)| {
                            self.world.account_handle(company).ok() == Some(*handle)
                        })
                        .map(|(_, account)| *account)
                        .or_else(|| {
                            self.world
                                .account_handle(company)
                                .ok()
                                .and_then(|handle| self.world.accounts.get(handle).ok().copied())
                        })
                        .ok_or(TxError::InvalidHandle)?;
                    if account.cash != Money::ZERO || account.protected != Money::ZERO {
                        return Err(TxError::InvalidOperation);
                    }
                    town.as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .exit
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .phase = crate::town_runtime::ExitPhase::Closed;
                }
                Action::RegionSettleDebt { obligation } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    let r = region.as_mut().ok_or(TxError::InvalidOperation)?;
                    let o = r
                        .obligations
                        .iter_mut()
                        .flatten()
                        .find(|o| o.id == obligation)
                        .ok_or(TxError::InvalidOperation)?;
                    if market_is_exiting(town.as_ref(), o.payer) {
                        return Err(TxError::InvalidOperation);
                    }
                    let due = o.arrears;
                    settle_obligation(&self.world, &mut accounts, o)?;
                    settled[action_index] =
                        Some(due.checked_sub(o.arrears).map_err(|_| TxError::Overflow)?);
                }
                Action::RegionActivate { city } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    region
                        .as_mut()
                        .ok_or(TxError::InvalidOperation)?
                        .activate(city)
                        .map_err(TxError::Region)?;
                }
                Action::RegionAdvance { seconds } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    if seconds == 0 || seconds > crate::region::MAX_ADVANCE_SECONDS {
                        return Err(TxError::Region(crate::region::Error::Limit));
                    }
                    let r = region.as_mut().ok_or(TxError::InvalidOperation)?;
                    let mut remaining = seconds;
                    while remaining > 0 {
                        let mut step = r.next_step(remaining);
                        if let Some(e) = exchange {
                            if let Some(t) = e.next_boundary(r.interaction_time) {
                                step = step
                                    .min(u32::try_from(t - r.interaction_time).unwrap_or(u32::MAX));
                            }
                            if let Some(b) = boot {
                                for j in b
                                    .jobs
                                    .iter()
                                    .filter(|j| j.phase == crate::boot::Phase::Running)
                                {
                                    step = step.min(
                                        u32::try_from(j.due - r.interaction_time)
                                            .map_err(|_| TxError::Overflow)?,
                                    );
                                }
                            }
                            step = step.min(60);
                        }
                        if step == 0 {
                            return Err(TxError::Exchange(crate::exchange::Error::Clock));
                        }
                        let end = r
                            .interaction_time
                            .checked_add(u64::from(step))
                            .ok_or(TxError::Overflow)?;
                        let due: [bool; crate::region::MAX_OBLIGATIONS] =
                            std::array::from_fn(|i| {
                                r.obligations[i]
                                    .is_some_and(|o| o.next_due.is_some_and(|t| t <= end))
                            });
                        if exchange.is_some() {
                            for t in r
                                .transits
                                .iter()
                                .flatten()
                                .filter(|t| t.pause_remaining.is_none() && t.arrives_at <= end)
                            {
                                crate::exchange::Ledger::release_path(
                                    &mut BootLedger {
                                        base: EconomicLedger {
                                            world: &self.world,
                                            accounts: &mut accounts,
                                            stocks: &mut stocks,
                                        },
                                        resources: &mut resources,
                                        leases: &mut leases,
                                        next_token: &mut next_token,
                                    },
                                    t.lease_tokens,
                                )?;
                            }
                        }
                        r.advance(step).map_err(TxError::Region)?;
                        for (i, o) in r.obligations.iter_mut().enumerate() {
                            if due[i] {
                                settle_obligation(
                                    &self.world,
                                    &mut accounts,
                                    o.as_mut().expect("due obligation"),
                                )?;
                            }
                        }
                        if let Some(e) = exchange.as_mut() {
                            let mut ledger = BootLedger {
                                base: EconomicLedger {
                                    world: &self.world,
                                    accounts: &mut accounts,
                                    stocks: &mut stocks,
                                },
                                resources: &mut resources,
                                leases: &mut leases,
                                next_token: &mut next_token,
                            };
                            boot.as_mut().ok_or(TxError::InvalidOperation)?.command(
                                crate::boot::Command::Advance { seconds: step },
                                &mut ledger,
                            )?;
                            e.arrive(r, &mut ledger)?;
                        }
                        remaining -= step;
                    }
                }
                Action::RegionMigrate {
                    from,
                    to,
                    requested,
                    tolerance,
                } => {
                    if !matches!(caller, Caller::System) {
                        return Err(TxError::Unauthorized);
                    }
                    migrated[action_index] = Some(
                        region
                            .as_mut()
                            .ok_or(TxError::InvalidOperation)?
                            .migrate(from, to, requested, tolerance)
                            .map_err(TxError::Region)?,
                    );
                    if let Some(e) = exchange {
                        let r = region.as_mut().ok_or(TxError::InvalidOperation)?;
                        let i = r
                            .transits
                            .iter()
                            .enumerate()
                            .filter_map(|(i, t)| t.map(|t| (i, t.id)))
                            .max_by_key(|(_, id)| *id)
                            .ok_or(TxError::InvalidOperation)?
                            .0;
                        let t = r.transits[i].ok_or(TxError::InvalidOperation)?;
                        let requests = e.migration_requests(r, t.route_indices, t.actual)?;
                        let tokens = crate::exchange::Ledger::claim_path(
                            &mut BootLedger {
                                base: EconomicLedger {
                                    world: &self.world,
                                    accounts: &mut accounts,
                                    stocks: &mut stocks,
                                },
                                resources: &mut resources,
                                leases: &mut leases,
                                next_token: &mut next_token,
                            },
                            &requests,
                        )?;
                        r.transits[i].as_mut().expect("transit").lease_tokens = tokens;
                    }
                }
                Action::Transfer { from, to, amount }
                | Action::PayProtected { from, to, amount } => {
                    let f = stage_account(&self.world, &mut accounts, from)?;
                    let t = stage_account(&self.world, &mut accounts, to)?;
                    if f == t {
                        return Err(TxError::InvalidOperation);
                    }
                    if !caller.permits(accounts[f].1.id) {
                        return Err(TxError::Unauthorized);
                    }
                    let payer = accounts[f].1.id;
                    let payee = accounts[t].1.id;
                    if self.world.town.is_some_and(|market| {
                        market
                            .profile
                            .equity_charters
                            .as_slice()
                            .iter()
                            .any(|charter| charter.company == payer || charter.company == payee)
                    }) {
                        // Chartered businesses use explicit contribution and distribution
                        // actions; ordinary transfers cannot bypass share issuance or payout
                        // authorization, or create an unclassified source of distributable cash.
                        return Err(TxError::InvalidOperation);
                    }
                    if market_is_exiting(town.as_ref(), payer)
                        || town.as_ref().is_some_and(|state| {
                            state.exit.is_some_and(|exit| {
                                exit.phase == crate::town_runtime::ExitPhase::Closed
                                    && exit.company_account == payee
                            })
                        })
                        || matches!(action, Action::PayProtected { .. })
                            && (market_is_exiting(town.as_ref(), payee)
                                || market_is_exiting(town.as_ref(), payer))
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    if matches!(action, Action::PayProtected { .. }) {
                        // Source-bound release rules are not implemented yet. Host-only
                        // laboratory primitive; account ownership alone is not authority.
                        if !matches!(caller, Caller::System) {
                            return Err(TxError::Unauthorized);
                        }
                        accounts[f].1.protected = accounts[f]
                            .1
                            .protected
                            .checked_sub(amount)
                            .map_err(|_| TxError::Protection)?;
                    } else if accounts[f]
                        .1
                        .cash
                        .checked_sub(accounts[f].1.protected)
                        .map_err(|_| TxError::Protection)?
                        < amount
                    {
                        return Err(TxError::InsufficientFunds);
                    }
                    accounts[f].1.cash = accounts[f]
                        .1
                        .cash
                        .checked_sub(amount)
                        .map_err(|_| TxError::InsufficientFunds)?;
                    accounts[t].1.cash = accounts[t]
                        .1
                        .cash
                        .checked_add(amount)
                        .map_err(|_| TxError::Overflow)?;
                }
                Action::Protect { account, amount }
                | Action::ReleaseProtection { account, amount } => {
                    if matches!(action, Action::ReleaseProtection { .. })
                        && !matches!(caller, Caller::System)
                    {
                        return Err(TxError::Unauthorized);
                    }
                    let i = stage_account(&self.world, &mut accounts, account)?;
                    if !caller.permits(accounts[i].1.id) {
                        return Err(TxError::Unauthorized);
                    }
                    if market_is_exiting(town.as_ref(), accounts[i].1.id) {
                        return Err(TxError::InvalidOperation);
                    }
                    let a = &mut accounts[i].1;
                    a.protected = if matches!(action, Action::Protect { .. }) {
                        a.protected
                            .checked_add(amount)
                            .map_err(|_| TxError::Overflow)?
                    } else {
                        a.protected
                            .checked_sub(amount)
                            .map_err(|_| TxError::Protection)?
                    };
                    if a.protected > a.cash {
                        return Err(TxError::InsufficientFunds);
                    }
                }
                Action::MoveStock { from, to, quantity } => {
                    if self.world.town.is_some_and(|market| {
                        let chartered =
                            |stock_ref: crate::arena::Handle<crate::arena::InventoryDomain>| {
                                self.world.stocks.get(stock_ref).ok().is_some_and(|stock| {
                                    self.world
                                        .accounts
                                        .get(stock.owner)
                                        .ok()
                                        .is_some_and(|owner| {
                                            market
                                                .profile
                                                .equity_charters
                                                .as_slice()
                                                .iter()
                                                .any(|charter| charter.company == owner.id)
                                        })
                                })
                            };
                        self.world.stocks.resolve(from).ok().is_some_and(chartered)
                            || self.world.stocks.resolve(to).ok().is_some_and(chartered)
                    }) {
                        return Err(TxError::InvalidOperation);
                    }
                    if let Some(e) = exchange {
                        let mut ids = [0; 2];
                        for (i, raw) in [from, to].iter().enumerate() {
                            let h = self
                                .world
                                .stocks
                                .resolve(*raw)
                                .map_err(|_| TxError::InvalidHandle)?;
                            ids[i] = self
                                .world
                                .stocks
                                .get(h)
                                .map_err(|_| TxError::InvalidHandle)?
                                .id;
                        }
                        if !e.permits_extra_stock_move(ids) {
                            return Err(TxError::Unauthorized);
                        }
                    }
                    if let Some(b) = boot {
                        for raw in [from, to] {
                            let h = self
                                .world
                                .stocks
                                .resolve(raw)
                                .map_err(|_| TxError::InvalidHandle)?;
                            if b.governs(
                                self.world
                                    .stocks
                                    .get(h)
                                    .map_err(|_| TxError::InvalidHandle)?
                                    .id,
                            ) {
                                return Err(TxError::Unauthorized);
                            }
                        }
                    }
                    if let Some(e) = &economy {
                        for raw in [from, to] {
                            let h = self
                                .world
                                .stocks
                                .resolve(raw)
                                .map_err(|_| TxError::InvalidHandle)?;
                            if e.governs(
                                self.world
                                    .stocks
                                    .get(h)
                                    .map_err(|_| TxError::InvalidHandle)?
                                    .id,
                            ) {
                                return Err(TxError::Unauthorized);
                            }
                        }
                    }
                    let f = stage_stock(&self.world, &mut stocks, from)?;
                    let t = stage_stock(&self.world, &mut stocks, to)?;
                    for stock in [stocks[f].1, stocks[t].1] {
                        let owner = self
                            .world
                            .accounts
                            .get(stock.owner)
                            .map_err(|_| TxError::InvalidHandle)?;
                        if market_is_exiting(town.as_ref(), owner.id) {
                            return Err(TxError::InvalidOperation);
                        }
                    }
                    if f == t
                        || quantity == Quantity::ZERO
                        || stocks[f].1.commodity != stocks[t].1.commodity
                    {
                        return Err(TxError::Stock);
                    }
                    let owner = self
                        .world
                        .accounts
                        .get(stocks[f].1.owner)
                        .map_err(|_| TxError::InvalidHandle)?
                        .id;
                    if !caller.permits(owner) {
                        return Err(TxError::Unauthorized);
                    }
                    let old = stocks[f].1;
                    let left = old
                        .quantity
                        .checked_sub(quantity)
                        .map_err(|_| TxError::Stock)?;
                    let end = old
                        .offset
                        .checked_add(quantity)
                        .map_err(|_| TxError::Overflow)?;
                    let denominator = old.origin_quantity.value();
                    if denominator == 0 || end > old.origin_quantity {
                        return Err(TxError::Stock);
                    }
                    let cost = i128::from(old.origin_basis.value()) * i128::from(end.value())
                        / i128::from(denominator)
                        - i128::from(old.origin_basis.value()) * i128::from(old.offset.value())
                            / i128::from(denominator);
                    let cost = Money::new(i64::try_from(cost).map_err(|_| TxError::Overflow)?)
                        .map_err(|_| TxError::Overflow)?;
                    let destination = &mut stocks[t].1;
                    if destination.quantity == Quantity::ZERO {
                        if destination.basis != Money::ZERO {
                            return Err(TxError::Stock);
                        }
                        destination.lot_id = old.lot_id;
                        destination.origin_quantity = old.origin_quantity;
                        destination.origin_basis = old.origin_basis;
                        destination.offset = old.offset;
                    } else if destination.lot_id != old.lot_id
                        || destination.origin_quantity != old.origin_quantity
                        || destination.origin_basis != old.origin_basis
                        || destination
                            .offset
                            .checked_add(destination.quantity)
                            .map_err(|_| TxError::Overflow)?
                            != old.offset
                    {
                        // One contiguous acquisition lot per slot for this laboratory.
                        // Mixing requires separately budgeted lot rows, never a basis reset.
                        return Err(TxError::Stock);
                    }
                    stocks[f].1.quantity = left;
                    stocks[f].1.offset = end;
                    stocks[f].1.basis = old.basis.checked_sub(cost).map_err(|_| TxError::Stock)?;
                    stocks[t].1.quantity = stocks[t]
                        .1
                        .quantity
                        .checked_add(quantity)
                        .map_err(|_| TxError::Overflow)?;
                    stocks[t].1.basis = stocks[t]
                        .1
                        .basis
                        .checked_add(cost)
                        .map_err(|_| TxError::Overflow)?;
                }
                Action::ReserveCapacity { resource, units } => {
                    if let Caller::Account(account) = caller {
                        if market_is_suspended(town.as_ref(), account)
                            || market_is_exiting(town.as_ref(), account)
                        {
                            return Err(TxError::InvalidOperation);
                        }
                    }
                    if units == 0 {
                        return Err(TxError::Capacity);
                    }
                    if construction.is_some_and(|project| {
                        project.operating_edge_id != 0 && project.operating_resource == resource
                    }) {
                        return Err(TxError::InvalidOperation);
                    }
                    let r = resources
                        .iter_mut()
                        .find(|r| r.id == resource)
                        .ok_or(TxError::Resource)?;
                    let reserved = r.reserved.checked_add(units).ok_or(TxError::Overflow)?;
                    if reserved.checked_add(r.occupied).ok_or(TxError::Overflow)? > r.capacity {
                        return Err(TxError::Capacity);
                    }
                    let slot = leases
                        .iter_mut()
                        .find(|l| l.is_none())
                        .ok_or(TxError::Lease)?;
                    let token = next_token;
                    next_token = next_token.checked_add(1).ok_or(TxError::Overflow)?;
                    *slot = Some(Lease {
                        token,
                        actor: caller.key(),
                        resource,
                        units,
                        occupied: false,
                    });
                    r.reserved = reserved;
                    created_tokens[action_index] = Some(CapacityToken {
                        world: self.world.scope.world(),
                        branch: self.world.scope.branch(),
                        number: token,
                    });
                }
                Action::OccupyCapacity { token } | Action::ReleaseCapacity { token } => {
                    if token.world != self.world.scope.world()
                        || token.branch != self.world.scope.branch()
                    {
                        return Err(TxError::WrongScope);
                    }
                    let slot = leases
                        .iter_mut()
                        .find(|l| l.as_ref().is_some_and(|l| l.token == token.number))
                        .ok_or(TxError::Lease)?;
                    let l = slot.as_mut().ok_or(TxError::Lease)?;
                    if !matches!(caller, Caller::System) && l.actor != caller.key() {
                        return Err(TxError::Unauthorized);
                    }
                    let r = resources
                        .iter_mut()
                        .find(|r| r.id == l.resource)
                        .ok_or(TxError::Resource)?;
                    if matches!(action, Action::OccupyCapacity { .. }) {
                        if l.occupied {
                            return Err(TxError::Lease);
                        }
                        r.reserved = r.reserved.checked_sub(l.units).ok_or(TxError::Lease)?;
                        r.occupied = r.occupied.checked_add(l.units).ok_or(TxError::Overflow)?;
                        l.occupied = true;
                    } else {
                        if l.occupied {
                            r.occupied = r.occupied.checked_sub(l.units).ok_or(TxError::Lease)?
                        } else {
                            r.reserved = r.reserved.checked_sub(l.units).ok_or(TxError::Lease)?
                        }
                        *slot = None;
                    }
                }
            }
            if let Some((started, old_region, old_boot, old_exchange)) = domain_start {
                let work =
                    u64::from(old_region != region || old_boot != boot || old_exchange != exchange);
                trace.end(Class::Region, Some(started), 0, work, 0);
                trace.summary.region_subtypes[trace.role.index()][7] += work;
            }
        }
        if let Some(e) = &economy {
            e.validate(&mut EconomicLedger {
                world: &self.world,
                accounts: &mut accounts,
                stocks: &mut stocks,
            })?;
            for (_, a) in &accounts {
                if a.protected < e.required_protection(a.id) {
                    return Err(TxError::Protection);
                }
            }
        }
        if let Some(b) = boot {
            let mut ledger = BootLedger {
                base: EconomicLedger {
                    world: &self.world,
                    accounts: &mut accounts,
                    stocks: &mut stocks,
                },
                resources: &mut resources,
                leases: &mut leases,
                next_token: &mut next_token,
            };
            let mut cash = Money::ZERO;
            for (id, _) in &self.world.account_ids {
                cash = cash
                    .checked_add(crate::boot::Ledger::cash(&mut ledger, *id)?)
                    .map_err(|_| TxError::Overflow)?;
            }
            b.validate(&mut ledger, cash)?;
        }
        if let Some(e) = exchange {
            e.validate(
                region.as_ref().ok_or(TxError::InvalidOperation)?,
                boot.as_ref().ok_or(TxError::InvalidOperation)?,
                construction.as_ref(),
                &mut BootLedger {
                    base: EconomicLedger {
                        world: &self.world,
                        accounts: &mut accounts,
                        stocks: &mut stocks,
                    },
                    resources: &mut resources,
                    leases: &mut leases,
                    next_token: &mut next_token,
                },
            )?;
        }
        if let Some(t) = timeline.as_ref() {
            t.validate()?;
        }
        validate_town_suspension_consistency(town.as_ref(), timeline.as_ref())
            .map_err(|_| TxError::InvalidOperation)?;
        validate_town_exit_state(
            self.world.town.as_ref().map(|market| &market.profile),
            town.as_ref(),
            timeline.as_ref(),
        )
        .map_err(|_| TxError::InvalidOperation)?;
        let timeline_progress = if env
            .actions
            .iter()
            .any(|a| matches!(a, Action::Timeline { .. }))
        {
            timeline.as_ref().map(|t| t.progress)
        } else {
            None
        };
        if let Some(b) = background.as_ref() {
            b.validate_timeline(
                timeline.as_ref().ok_or(TxError::InvalidOperation)?,
                next_revision,
            )
            .map_err(|_| TxError::InvalidOperation)?;
        }
        let bg_heap = |b: &crate::model_rows::Resident<crate::background::Background>| {
            b.backing().owned_bytes() - size_of::<crate::background::Background>()
        };
        let projected = self
            .owned_capacity_bytes()
            .saturating_sub(bg_heap(&self.background))
            .saturating_sub(self.timeline.backing().owned_bytes())
            + bg_heap(&background)
            + timeline.backing().owned_bytes();
        // The authoritative rows remain live while the candidate is prepared. The candidate
        // uses the reserved spare backing, so both fixed row sets count toward the peak.
        let projected = projected
            .saturating_add(
                spatial_permissions
                    .as_ref()
                    .map_or(0, |state| state.owned_capacity_bytes()),
            )
            .saturating_add(
                housing
                    .as_ref()
                    .map_or(0, |state| state.owned_capacity_bytes()),
            );
        if projected > ENGINE_BYTE_BUDGET {
            return Err(TxError::WorkingLimit);
        }
        validate_construction_binding(
            construction,
            &resources,
            &leases,
            boot.as_ref(),
            exchange.as_ref(),
            timeline.as_ref(),
            town.as_ref().map(|market| market.settled_day),
        )?;
        Ok(Prepared {
            timeline,
            background,
            exchange,
            boot,
            economy,
            region,
            construction,
            housing,
            town,
            spatial_permissions,
            spatial_update_grant,
            accounts,
            stocks,
            resources,
            leases,
            next_token,
            receipt: Receipt {
                operation_id: env.operation_id,
                revision: next_revision,
                durability: crate::FrozenLabel("volatile_only"),
                migrated,
                created_facts,
                settled,
                created_tokens,
                timeline: timeline_progress,
                town: town_summary,
            },
        })
    }

    fn apply(&mut self, mut prepared: Prepared) {
        // No user hook, allocation or business failure after the durable hook succeeds.
        for &(h, a) in prepared.accounts.iter() {
            self.world
                .accounts
                .replace(h, a)
                .expect("validated commit handle");
        }
        for &(h, s) in prepared.stocks.iter() {
            self.world
                .stocks
                .replace(h, s)
                .expect("validated commit handle");
        }
        // The old authoritative backing returns through the lease, never deallocated.
        std::mem::swap(&mut self.timeline, &mut *prepared.timeline);
        std::mem::swap(&mut self.background, &mut *prepared.background);
        self.world.exchange = prepared.exchange;
        self.world.boot = prepared.boot;
        self.world.economy = prepared.economy;
        self.world.region = prepared.region;
        self.world.construction = prepared.construction;
        if let (Some(world_town), Some(state)) = (&mut self.world.town, prepared.town) {
            world_town.state = state;
        }
        if let (Some(world_housing), Some(state)) = (&mut self.world.housing, prepared.housing) {
            world_housing.state = state;
        }
        if let Some(mut state) = prepared.spatial_permissions {
            std::mem::swap(
                self.spatial_permissions
                    .as_mut()
                    .expect("spatial source validated"),
                &mut *state,
            );
        }
        self.world.revision = prepared.receipt.revision;
        std::mem::swap(&mut self.resources, &mut *prepared.resources);
        std::mem::swap(&mut self.leases, &mut *prepared.leases);
        self.next_token = prepared.next_token;
    }
}

/// Every transaction path that mutates generic capacity rows must preserve the exact
/// occupied site lease that anchors an active construction project.
fn validate_construction_binding(
    project: Option<crate::construction::Project>,
    resources: &[Resource],
    leases: &[Option<Lease>],
    boot: Option<&crate::boot::Boot>,
    exchange: Option<&crate::exchange::Exchange>,
    timeline: Option<&crate::timeline::Timeline>,
    settled_day: Option<u64>,
) -> Result<(), TxError> {
    let Some(project) = project else {
        return Ok(());
    };
    project
        .validate_persisted()
        .map_err(TxError::Construction)?;
    if project.utility_contract.is_some() {
        boot.ok_or(TxError::InvalidOperation)?
            .validate_construction_contract(project)
            .map_err(|_| TxError::InvalidOperation)?;
    }
    // A clearance start is a committed calendar fact, not a future work reservation.
    // Check both timestamps against the restored authoritative clock as well as
    // the relative ordering checked by Project::validate_persisted.
    if project.is_uncommissioned_clearance()
        && timeline.is_some_and(|clock| {
            [project.started_at, project.last_work_at]
                .into_iter()
                .flatten()
                .any(|at| at.value() < 0 || at.value() as u64 > clock.progress.last_complete)
        })
    {
        return Err(TxError::InvalidOperation);
    }
    if project.operating_resource != 0 {
        let resource = resources
            .iter()
            .find(|resource| resource.id == project.operating_resource)
            .ok_or(TxError::Resource)?;
        let expected = project
            .operating_base_capacity
            .checked_add(if project.operating_published {
                project.operating_units
            } else {
                0
            })
            .ok_or(TxError::Overflow)?;
        if resource.capacity != expected {
            return Err(TxError::Capacity);
        }
        let phase_keeps_operating_contribution = match project.phase {
            crate::construction::Phase::Operational => {
                project.kind == crate::construction::Kind::Build
                    || (project.recoverable_commands()
                        && project.kind == crate::construction::Kind::Retrofit)
            }
            crate::construction::Phase::Review => {
                (project.kind == crate::construction::Kind::Build
                    || (project.recoverable_commands()
                        && project.kind == crate::construction::Kind::Retrofit))
                    && project.waste_receiver.is_some()
            }
            crate::construction::Phase::Ready => project.waste_receiver.is_some(),
            _ => false,
        };
        if project.operating_published && !phase_keeps_operating_contribution {
            return Err(TxError::InvalidOperation);
        }
        if project.operating_edge_id != 0 {
            if project.kind
                != if project.operating_withdrawn_at.is_some()
                    || project.is_uncommissioned_clearance()
                {
                    crate::construction::Kind::Demolish
                } else {
                    crate::construction::Kind::Build
                }
                || project.site_resource == project.operating_resource
                || (project.waste_units != 0
                    && project.waste_receiver_resource == project.operating_resource)
                || resource.reserved != 0
                || resource.occupied != 0
                || boot.is_some_and(|boot| boot.resources.contains(&project.operating_resource))
                || exchange.is_some_and(|exchange| {
                    exchange
                        .edges
                        .iter()
                        .any(|edge| edge.resource == project.operating_resource)
                })
            {
                return Err(TxError::InvalidOperation);
            }
            if let Some(published_at) = project.operating_published_at {
                validate_road_effect_publication_clock(
                    published_at,
                    timeline.map(|value| value.progress.last_complete),
                    settled_day,
                )?;
            }
            if let Some(withdrawn_at) = project.operating_withdrawn_at {
                validate_road_effect_publication_clock(
                    withdrawn_at,
                    timeline.map(|value| value.progress.last_complete),
                    settled_day,
                )?;
            }
        }
    }
    if project.waste_units != 0
        && !resources.iter().any(|resource| {
            resource.id == project.waste_receiver_resource
                && project.waste_units <= resource.capacity
        })
    {
        return Err(TxError::Resource);
    }
    if !project.site_reserved {
        if project.site_token.is_some() {
            return Err(TxError::Lease);
        }
        return Ok(());
    }
    let token = project.site_token.ok_or(TxError::Lease)?;
    let resource = resources
        .iter()
        .find(|resource| resource.id == project.site_resource)
        .ok_or(TxError::Resource)?;
    if project.site_units > resource.capacity {
        return Err(TxError::Capacity);
    }
    let mut matching = leases.iter().flatten().filter(|lease| {
        lease.token == token
            && lease.actor == project.owner
            && lease.resource == project.site_resource
            && lease.units == project.site_units
            && lease.occupied
    });
    if matching.next().is_none() || matching.next().is_some() {
        return Err(TxError::Lease);
    }
    Ok(())
}

fn validate_road_effect_publication_clock(
    published_at: crate::units::GameTime,
    last_complete: Option<u64>,
    settled_day: Option<u64>,
) -> Result<(), TxError> {
    let published_seconds =
        u64::try_from(published_at.value()).map_err(|_| TxError::InvalidOperation)?;
    let day = crate::timeline::DAY;
    if published_seconds % day != 0
        || last_complete.is_none_or(|complete| published_seconds > complete)
        || settled_day.is_none_or(|settled| published_seconds / day > settled)
    {
        return Err(TxError::InvalidOperation);
    }
    Ok(())
}

fn validate_road_effect_withdrawal_boundary(
    timeline: Option<&crate::timeline::Timeline>,
    settled_day: Option<u64>,
) -> Result<(), TxError> {
    let timeline = timeline.ok_or(TxError::InvalidOperation)?;
    let progress = timeline.progress;
    let day = crate::timeline::DAY;
    if progress.phase.is_some()
        || progress.target.is_some()
        || progress.at != progress.last_complete
        || progress.last_complete % day != 0
        || settled_day != Some(progress.last_complete / day)
    {
        return Err(TxError::InvalidOperation);
    }
    Ok(())
}

fn stage_account(
    world: &World,
    dirty: &mut DirtyAccounts,
    raw: RawHandle,
) -> Result<usize, TxError> {
    let h = world
        .accounts
        .resolve(raw)
        .map_err(|_| TxError::InvalidHandle)?;
    if let Some(i) = dirty.iter().position(|(old, _)| *old == h) {
        return Ok(i);
    }
    if dirty.len() == crate::assembly::MAX_ACCOUNTS {
        return Err(TxError::WorkingLimit);
    }
    dirty.push((
        h,
        *world.accounts.get(h).map_err(|_| TxError::InvalidHandle)?,
    ));
    Ok(dirty.len() - 1)
}
fn stage_stock(world: &World, dirty: &mut DirtyStocks, raw: RawHandle) -> Result<usize, TxError> {
    let h = world
        .stocks
        .resolve(raw)
        .map_err(|_| TxError::InvalidHandle)?;
    if let Some(i) = dirty.iter().position(|(old, _)| *old == h) {
        return Ok(i);
    }
    if dirty.len() == crate::assembly::MAX_STOCKS {
        return Err(TxError::WorkingLimit);
    }
    dirty.push((h, *world.stocks.get(h).map_err(|_| TxError::InvalidHandle)?));
    Ok(dirty.len() - 1)
}

struct TownSettlementContext<'a> {
    operation_id: u64,
    world_id: u64,
    branch_id: u64,
    revision: u64,
    world: &'a World,
    profile: &'a crate::town_runtime::MarketProfile,
    state: &'a mut Option<crate::town_runtime::MarketState>,
    timeline: &'a mut crate::timeline::Timeline,
    accounts: &'a mut DirtyAccounts,
    stocks: &'a mut DirtyStocks,
    boot: &'a mut Option<crate::boot::Boot>,
    resources: &'a mut ResourceRows,
    leases: &'a mut LeaseRows,
    next_token: &'a mut u64,
    transport: Option<&'a crate::market_access::SettlementDeliveryInput<'a>>,
}

fn prepare_town_settlement(
    context: TownSettlementContext<'_>,
) -> Result<crate::town_runtime::MarketSummary, TxError> {
    let TownSettlementContext {
        operation_id,
        world_id,
        branch_id,
        revision,
        world,
        profile,
        state,
        timeline,
        accounts,
        stocks,
        boot,
        resources,
        leases,
        next_token,
        transport,
    } = context;
    use crate::{
        town_economy::{
            self, AccountSnapshot, EmployerSnapshot, InventorySnapshot, MarketInput, PlanLine,
            Recipe, RecipeExecution,
        },
        town_runtime::PayrollCursor,
    };

    profile.validate().map_err(|_| TxError::InvalidOperation)?;
    let market_state = state.as_mut().ok_or(TxError::InvalidOperation)?;
    let at = timeline.progress.last_complete;
    if timeline.progress.phase.is_some()
        || timeline.progress.target.is_some()
        || at == 0
        || at % crate::timeline::DAY != 0
    {
        return Err(TxError::InvalidOperation);
    }
    let day = at / crate::timeline::DAY;
    if market_state.settled_day.checked_add(1) != Some(day) {
        return Err(TxError::InvalidOperation);
    }
    if transport.is_some_and(|proof| proof.proof.service_day != day) {
        return Err(TxError::InvalidOperation);
    }

    // A bound market consumes only completed physical electricity, never the
    // generator's job/lease capacity. All recipes see one shared operator pool.
    let utility_available = if let Some(binding) = profile.utility_supply {
        let supplier = boot.as_ref().ok_or(TxError::InvalidOperation)?;
        if supplier.time != at
            || supplier.operator != binding.boot_operator
            || supplier.stocks[1] != binding.boot_stock_id
        {
            return Err(TxError::InvalidOperation);
        }
        let handle = world
            .stock_handle(binding.boot_stock_id)
            .map_err(|_| TxError::InvalidHandle)?;
        let stock = stocks
            .iter()
            .find(|(old, _)| *old == handle)
            .map(|(_, stock)| *stock)
            .or_else(|| world.stocks.get(handle).ok().copied())
            .ok_or(TxError::InvalidHandle)?;
        if stock.commodity != crate::assembly::Commodity::Electricity
            || world
                .accounts
                .get(stock.owner)
                .map_err(|_| TxError::InvalidHandle)?
                .id
                != binding.boot_operator
        {
            return Err(TxError::InvalidOperation);
        }
        Some(
            binding
                .available_units(stock.quantity)
                .map_err(|_| TxError::Overflow)?,
        )
    } else {
        None
    };

    let mut employer_counts = Vec::<(u32, u32, i64)>::with_capacity(profile.employers.len());
    for employer in profile.employers.as_slice() {
        employer_counts.push((employer.account, 0, 0));
    }
    let mut worker_deltas = Vec::<(u32, u32, u64, i64, i64)>::new();
    let mut wages_accrued_total = 0i64;
    let prior_at = at - crate::timeline::DAY;
    let cursors = market_state.payroll.as_slice();
    for payroll in timeline.payroll.iter() {
        let Some(row) = employer_counts
            .iter_mut()
            .find(|row| row.0 == payroll.spec.payer)
        else {
            continue;
        };
        let previous = cursors.iter().find(|cursor| cursor.id == payroll.spec.id);
        if previous.is_none() && payroll.started < prior_at {
            return Err(TxError::InvalidOperation);
        }
        let old_work = previous.map_or(0, |cursor| cursor.work_seconds);
        let old_accrued = previous.map_or(Money::ZERO, |cursor| cursor.accrued);
        let old_paid = previous.map_or(Money::ZERO, |cursor| cursor.paid);
        let worked = payroll
            .work_seconds
            .checked_sub(old_work)
            .ok_or(TxError::InvalidOperation)?;
        let accrued = payroll
            .accrued
            .checked_sub(old_accrued)
            .map_err(|_| TxError::InvalidOperation)?;
        let paid = payroll
            .paid
            .checked_sub(old_paid)
            .map_err(|_| TxError::InvalidOperation)?;
        if payroll.paid > payroll.accrued {
            return Err(TxError::InvalidOperation);
        }
        if worked > 0 {
            row.1 = row.1.checked_add(1).ok_or(TxError::Overflow)?;
        }
        row.2 = row.2.checked_add(paid.value()).ok_or(TxError::Overflow)?;
        wages_accrued_total = wages_accrued_total
            .checked_add(accrued.value())
            .ok_or(TxError::Overflow)?;
        worker_deltas.push((
            payroll.spec.id,
            payroll.spec.payer,
            worked,
            accrued.value(),
            paid.value(),
        ));
    }
    worker_deltas.sort_by_key(|row| row.0);

    let mut employers = Vec::with_capacity(profile.employers.len());
    for spec in profile.employers.as_slice() {
        let (_, workers, paid) = *employer_counts
            .iter()
            .find(|row| row.0 == spec.account)
            .ok_or(TxError::InvalidOperation)?;
        if workers > spec.worker_limit {
            return Err(TxError::InvalidOperation);
        }
        employers.push(EmployerSnapshot {
            account: spec.account,
            available_workers: workers,
            wage_cost_per_worker_day: Money::ZERO,
            payroll_paid_today: Money::new(paid).map_err(|_| TxError::Overflow)?,
        });
    }

    let mut account_views = Vec::with_capacity(world.account_ids.len());
    for (id, handle) in world.account_ids.iter() {
        let account = world
            .accounts
            .get(*handle)
            .map_err(|_| TxError::InvalidHandle)?;
        account_views.push(AccountSnapshot {
            account: *id,
            cash: account.cash,
            protected: account.protected,
        });
    }
    let mut inventories = Vec::with_capacity(world.stock_ids.len());
    for (id, handle) in world.stock_ids.iter() {
        let stock = world
            .stocks
            .get(*handle)
            .map_err(|_| TxError::InvalidHandle)?;
        let owner = world
            .accounts
            .get(stock.owner)
            .map_err(|_| TxError::InvalidHandle)?;
        inventories.push(InventorySnapshot {
            stock_id: *id,
            owner: owner.id,
            good: crate::town_runtime::commodity_good(stock.commodity),
            quantity: stock.quantity,
            spendable_basis: stock.basis,
            origin_quantity: stock.origin_quantity,
            origin_basis: stock.origin_basis,
            offset: stock.offset,
        });
    }

    let mut recipes = Vec::<Recipe>::with_capacity(profile.recipes.len());
    let mut executions = Vec::<RecipeExecution>::with_capacity(profile.recipes.len());
    let mut used_workers = Vec::<(u32, u32)>::with_capacity(profile.employers.len());
    for employer in employers.iter() {
        used_workers.push((employer.account, 0));
    }
    let mut configured_recipes: Vec<_> = profile.recipes.as_slice().iter().collect();
    configured_recipes.sort_by_key(|recipe| recipe.recipe_id);
    let mut assigned_payroll_ids = Vec::<u32>::new();
    for configured in configured_recipes {
        let output_handle = world
            .stock_handle(configured.output_stock_id)
            .map_err(|_| TxError::InvalidHandle)?;
        let output = world
            .stocks
            .get(output_handle)
            .map_err(|_| TxError::InvalidHandle)?;
        // A single physical output slot represents one unmerged lot. Existing goods remain
        // sellable; production resumes after the slot is fully consumed and cleared.
        if output.quantity != Quantity::ZERO {
            continue;
        }
        let assigned = used_workers
            .iter_mut()
            .find(|row| row.0 == configured.employer)
            .ok_or(TxError::InvalidOperation)?;
        let workers = worker_deltas
            .iter()
            .filter(|row| {
                row.1 == configured.employer && row.2 > 0 && !assigned_payroll_ids.contains(&row.0)
            })
            .take(configured.assigned_workers as usize)
            .count() as u32;
        if workers == 0 {
            continue;
        }
        let selected: Vec<_> = worker_deltas
            .iter()
            .filter(|row| {
                row.1 == configured.employer && row.2 > 0 && !assigned_payroll_ids.contains(&row.0)
            })
            .take(workers as usize)
            .copied()
            .collect();
        assigned_payroll_ids.extend(selected.iter().map(|row| row.0));
        assigned.1 = assigned.1.checked_add(workers).ok_or(TxError::Overflow)?;
        let mut recipe = *configured;
        recipe.assigned_workers = workers;
        if let Some(binding) = profile.utility_supply {
            recipe.available_utility_per_day =
                utility_available.ok_or(TxError::InvalidOperation)?;
            recipe.utility_cost_per_output_q = binding
                .cost_per_output(recipe.utility_per_output_q)
                .map_err(|_| TxError::Overflow)?;
        }
        let worked_seconds = selected.iter().try_fold(0u64, |total, row| {
            total.checked_add(row.2).ok_or(TxError::Overflow)
        })?;
        let labor_cost = selected.iter().try_fold(0i64, |total, row| {
            total.checked_add(row.3).ok_or(TxError::Overflow)
        })?;
        if worked_seconds == 0 {
            continue;
        }
        let output_remainder = market_state
            .recipe_output_remainders
            .as_slice()
            .iter()
            .find(|row| row.recipe_id == configured.recipe_id)
            .map_or(0, |row| row.numerator);
        executions.push(RecipeExecution {
            recipe_id: configured.recipe_id,
            workers,
            worked_seconds,
            output_remainder,
            labor_cost: Money::new(labor_cost).map_err(|_| TxError::Overflow)?,
        });
        recipes.push(recipe);
    }

    let mut input = MarketInput {
        schema: "mycity-town-market-v0.1".to_owned(),
        world: world_id,
        branch: branch_id,
        revision,
        day,
        operation_id,
        households: profile.households.as_slice().to_vec(),
        employers,
        accounts: account_views,
        inventories,
        shops: profile.shops.as_slice().to_vec(),
        recipes,
        delivery: profile.delivery.as_slice().to_vec(),
    };
    let resolved_digest = if let Some(proof) = transport {
        let resolved = crate::market_access::resolve_delivery_quotes_from_snapshot(
            profile.id,
            crate::market_access::TransportSnapshotRef {
                generation: proof.proof.generation,
            },
            &input,
            proof.proof.opening,
            proof.proof.assignment,
            proof.proof.partitions,
            proof.proof.fee_policy,
        )
        .map_err(|_| TxError::InvalidOperation)?;
        input.delivery = resolved.delivery_quotes;
        Some(resolved.source_digest)
    } else {
        None
    };
    let transport_receipt = match (transport, resolved_digest) {
        (Some(proof), Some(resolved_sha256)) => Some(crate::town_runtime::MarketTransportReceipt {
            issued_day: day,
            issued_generation: proof.proof.generation,
            issued_proof_sha256: proof.proof_sha256,
            consumed_generation: proof.proof.generation,
            resolved_sha256,
        }),
        (None, None) => None,
        _ => return Err(TxError::InvalidOperation),
    };
    // Resolve and verify transport using the unchanged genesis-bound opening first. Only
    // then remove suspended operators from the economic planner input, so a suspension
    // cannot bypass the proof checks or invalidate the declaration being consumed.
    let suspended: Vec<u32> = market_state
        .suspended_operators
        .as_slice()
        .iter()
        .map(|row| row.account)
        .collect();
    let exit = market_state.exit;
    let closed_company = exit
        .filter(|runtime| runtime.phase == crate::town_runtime::ExitPhase::Closed)
        .map(|runtime| runtime.company_account);
    if !suspended.is_empty() || exit.is_some() {
        input.shops.retain(|shop| {
            !suspended.contains(&shop.account) && closed_company != Some(shop.account)
        });
        input.recipes.retain(|recipe| {
            !suspended.contains(&recipe.employer)
                && exit.is_none_or(|runtime| runtime.company_account != recipe.employer)
        });
        let active_recipe_ids: Vec<u32> = input
            .recipes
            .iter()
            .map(|recipe| recipe.recipe_id)
            .collect();
        executions.retain(|execution| active_recipe_ids.contains(&execution.recipe_id));
        let active_shop_ids: Vec<u32> = input.shops.iter().map(|shop| shop.shop_id).collect();
        input
            .delivery
            .retain(|quote| active_shop_ids.contains(&quote.shop));
    }
    let batch = town_economy::plan_day_with_execution(&input, &executions)
        .map_err(|_| TxError::InvalidOperation)?;
    let reconciled = town_economy::reconcile_with_execution(&input, &executions, &batch)
        .map_err(|_| TxError::InvalidOperation)?;

    for execution in &executions {
        let recipe = profile
            .recipes
            .as_slice()
            .iter()
            .find(|recipe| recipe.recipe_id == execution.recipe_id)
            .ok_or(TxError::InvalidOperation)?;
        let (_, remainder) = town_economy::worker_output(
            recipe.output_per_worker_day,
            execution.worked_seconds,
            execution.output_remainder,
        )
        .map_err(|_| TxError::InvalidOperation)?;
        if let Some(row) = market_state
            .recipe_output_remainders
            .as_mut_slice()
            .iter_mut()
            .find(|row| row.recipe_id == execution.recipe_id)
        {
            row.numerator = remainder;
        } else {
            market_state
                .recipe_output_remainders
                .push(crate::town_runtime::RecipeOutputRemainder {
                    recipe_id: execution.recipe_id,
                    numerator: remainder,
                })
                .map_err(|_| TxError::WorkingLimit)?;
        }
    }
    market_state
        .recipe_output_remainders
        .as_mut_slice()
        .sort_by_key(|row| row.recipe_id);

    let mut produced = Quantity::ZERO;
    let mut waste = Quantity::ZERO;
    for line in &batch.lines {
        match *line {
            PlanLine::Produce {
                employer,
                input_stock_id,
                input_quantity,
                output_stock_id,
                output_good,
                output_quantity,
                waste_quantity,
                utility_quantity,
                input_basis,
                output_basis,
                utility_cost,
                freight_cost,
                disposal_cost,
                utility_account,
                freight_account,
                disposal_account,
                ..
            } => {
                let input_raw = world
                    .stock_ref(input_stock_id)
                    .map_err(|_| TxError::InvalidHandle)?;
                let input_index = stage_stock(world, stocks, input_raw)?;
                let input_stock = &mut stocks[input_index].1;
                let measured_basis = stock_slice_basis(input_stock, input_quantity)?;
                if measured_basis != input_basis
                    || input_stock.owner
                        != world
                            .account_handle(employer)
                            .map_err(|_| TxError::InvalidHandle)?
                {
                    return Err(TxError::Stock);
                }
                consume_stock(input_stock, input_quantity, input_basis)?;

                let output_raw = world
                    .stock_ref(output_stock_id)
                    .map_err(|_| TxError::InvalidHandle)?;
                let output_index = stage_stock(world, stocks, output_raw)?;
                let output_stock = &mut stocks[output_index].1;
                if output_stock.quantity != Quantity::ZERO
                    || crate::town_runtime::commodity_good(output_stock.commodity) != output_good
                    || output_stock.owner
                        != world
                            .account_handle(employer)
                            .map_err(|_| TxError::InvalidHandle)?
                {
                    return Err(TxError::Stock);
                }
                output_stock.quantity = output_quantity;
                output_stock.basis = output_basis;
                output_stock.origin_quantity = output_quantity;
                output_stock.origin_basis = output_basis;
                output_stock.offset = Quantity::ZERO;
                output_stock.lot_id = output_stock
                    .lot_id
                    .checked_add(1)
                    .ok_or(TxError::Overflow)?
                    .max(1);

                if let Some(binding) = profile.utility_supply {
                    if utility_account != binding.boot_operator
                        || binding
                            .cost_per_output(utility_quantity)
                            .map_err(|_| TxError::Overflow)?
                            != utility_cost
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let quantity = binding
                        .stock_quantity(utility_quantity)
                        .map_err(|_| TxError::Overflow)?;
                    if quantity != Quantity::ZERO {
                        // Reuse Boot's export accounting, including exported_basis. Payment
                        // below is the sale revenue, not a second generator operation fee.
                        boot.as_mut()
                            .ok_or(TxError::InvalidOperation)?
                            .export_local_electricity(
                                quantity,
                                &mut BootLedger {
                                    base: EconomicLedger {
                                        world,
                                        accounts,
                                        stocks,
                                    },
                                    resources,
                                    leases,
                                    next_token,
                                },
                            )?;
                    }
                }
                market_transfer(world, accounts, employer, utility_account, utility_cost)?;
                market_transfer(world, accounts, employer, freight_account, freight_cost)?;
                market_transfer(world, accounts, employer, disposal_account, disposal_cost)?;
                produced = produced
                    .checked_add(output_quantity)
                    .map_err(|_| TxError::Overflow)?;
                waste = waste
                    .checked_add(waste_quantity)
                    .map_err(|_| TxError::Overflow)?;
            }
            PlanLine::Sale {
                buyer_account,
                shop_account,
                stock_id,
                quantity,
                goods_basis,
                goods_payment,
                freight_payment,
                utility_payment,
                freight_account,
                utility_account,
                ..
            } => {
                let raw = world
                    .stock_ref(stock_id)
                    .map_err(|_| TxError::InvalidHandle)?;
                let index = stage_stock(world, stocks, raw)?;
                let stock = &mut stocks[index].1;
                if world
                    .accounts
                    .get(stock.owner)
                    .map_err(|_| TxError::InvalidHandle)?
                    .id
                    != shop_account
                    || stock_slice_basis(stock, quantity)? != goods_basis
                {
                    return Err(TxError::Stock);
                }
                consume_stock(stock, quantity, goods_basis)?;
                market_transfer(world, accounts, buyer_account, shop_account, goods_payment)?;
                market_transfer(
                    world,
                    accounts,
                    buyer_account,
                    freight_account,
                    freight_payment,
                )?;
                market_transfer(
                    world,
                    accounts,
                    buyer_account,
                    utility_account,
                    utility_payment,
                )?;
            }
            PlanLine::Unmet { .. } => {}
        }
    }

    for payroll in timeline.payroll.iter() {
        let cursor = PayrollCursor {
            id: payroll.spec.id,
            work_seconds: payroll.work_seconds,
            accrued: payroll.accrued,
            paid: payroll.paid,
        };
        if let Some(existing) = market_state
            .payroll
            .as_mut_slice()
            .iter_mut()
            .find(|x| x.id == cursor.id)
        {
            *existing = cursor;
        } else {
            market_state
                .payroll
                .push(cursor)
                .map_err(|_| TxError::WorkingLimit)?;
        }
    }
    if market_state
        .payroll
        .as_slice()
        .windows(2)
        .any(|w| w[0].id >= w[1].id)
    {
        return Err(TxError::InvalidOperation);
    }
    let wages_accrued = Money::new(wages_accrued_total).map_err(|_| TxError::Overflow)?;
    let idle_labor_expense = wages_accrued
        .checked_sub(reconciled.labor_basis)
        .map_err(|_| TxError::InvalidOperation)?;
    let summary = crate::town_runtime::MarketSummary {
        day,
        operation_id,
        reference_need: batch.reference_need,
        market_demand: batch.market_demand,
        actual_sales: batch.actual_sales,
        unmet_market_demand: batch.unmet_market_demand,
        produced,
        waste,
        goods_payments: batch.planned_goods_payments,
        freight: batch.planned_freight,
        utilities: batch.planned_utilities,
        disposal: batch.planned_disposal,
        labor_basis: reconciled.labor_basis,
        wages_accrued,
        idle_labor_expense,
    };
    apply_company_tax_assessments(
        profile,
        market_state,
        timeline,
        &batch,
        &worker_deltas,
        day,
        operation_id,
    )?;
    // Record both actual application and the authoritative planner reconciliation in one
    // persisted cursor. Timeline has already paid any due wages; no second wage transfer occurs.
    let _labor_paid_by_timeline = reconciled.worker_days;
    market_state.settled_day = day;
    market_state.last_transport = transport_receipt;
    market_state.last_summary = summary;
    Ok(summary)
}

fn apply_company_tax_assessments(
    profile: &crate::town_runtime::MarketProfile,
    market_state: &mut crate::town_runtime::MarketState,
    timeline: &mut crate::timeline::Timeline,
    batch: &crate::town_economy::SettlementBatch,
    worker_deltas: &[(u32, u32, u64, i64, i64)],
    day: u64,
    operation_id: u64,
) -> Result<(), TxError> {
    use crate::town_economy::PlanLine;
    for rule in profile.tax_rules.as_slice() {
        let mut sales_cents = 0_i128;
        let mut sold_basis_cents = 0_i128;
        let mut accrued_wages_cents = 0_i128;
        let mut capitalized_labor_cents = 0_i128;
        for (_, payer, _, accrued, _) in worker_deltas {
            if *payer == rule.company {
                accrued_wages_cents = accrued_wages_cents
                    .checked_add(i128::from(*accrued))
                    .ok_or(TxError::Overflow)?;
            }
        }
        for line in &batch.lines {
            match *line {
                PlanLine::Sale {
                    shop_account,
                    goods_basis,
                    goods_payment,
                    ..
                } if shop_account == rule.company => {
                    sales_cents = sales_cents
                        .checked_add(i128::from(goods_payment.value()))
                        .ok_or(TxError::Overflow)?;
                    sold_basis_cents = sold_basis_cents
                        .checked_add(i128::from(goods_basis.value()))
                        .ok_or(TxError::Overflow)?;
                }
                PlanLine::Produce {
                    employer,
                    labor_cost,
                    ..
                } if employer == rule.company => {
                    capitalized_labor_cents = capitalized_labor_cents
                        .checked_add(i128::from(labor_cost.value()))
                        .ok_or(TxError::Overflow)?;
                }
                _ => {}
            }
        }
        let idle_labor = accrued_wages_cents
            .checked_sub(capitalized_labor_cents)
            .ok_or(TxError::InvalidOperation)?;
        let margin_cents = sales_cents
            .checked_sub(sold_basis_cents)
            .and_then(|v| v.checked_sub(idle_labor))
            .ok_or(TxError::Overflow)?;
        let taxable_cents = margin_cents.max(0);
        let assessed_cents = taxable_cents
            .checked_mul(i128::from(rule.rate_basis_points))
            .ok_or(TxError::Overflow)?
            / 10_000;
        let assessed = Money::new(i64::try_from(assessed_cents).map_err(|_| TxError::Overflow)?)
            .map_err(|_| TxError::Overflow)?;
        let taxable_basis =
            Money::new(i64::try_from(taxable_cents).map_err(|_| TxError::Overflow)?)
                .map_err(|_| TxError::Overflow)?;

        let cursor = market_state
            .tax_cursors
            .as_mut_slice()
            .iter_mut()
            .find(|cursor| cursor.company == rule.company)
            .filter(|cursor| cursor.policy_id == rule.policy_id)
            .ok_or(TxError::InvalidOperation)?;
        if cursor.through_day.checked_add(1) != Some(day)
            || (day == 1 && cursor.last_operation_id != 0)
            || operation_id == 0
        {
            return Err(TxError::InvalidOperation);
        }

        let claim_id = if assessed > Money::ZERO {
            let id = timeline
                .claims
                .last()
                .map_or(1, |claim| claim.id.checked_add(1).unwrap_or(0));
            if id == 0 || timeline.claims.len() >= crate::timeline::CLAIM_LIMIT {
                return Err(TxError::WorkingLimit);
            }
            let due = day
                .checked_add(1)
                .and_then(|next_day| next_day.checked_mul(crate::timeline::DAY))
                .ok_or(TxError::Overflow)?;
            timeline
                .claims
                .try_reserve_exact(1)
                .map_err(|_| TxError::WorkingLimit)?;
            let source_ref = timeline.push_tax_claim_source(
                id,
                crate::timeline::TaxClaimSource {
                    company: rule.company,
                    policy_id: rule.policy_id,
                    kind: rule.kind,
                    period: day,
                    settlement_operation_id: operation_id,
                    taxable_basis,
                    assessed,
                },
            )?;
            timeline.claims.push(crate::timeline::Claim {
                id,
                payer: rule.company,
                payee: rule.authority_account,
                due,
                principal: assessed,
                paid: Money::ZERO,
                processed: false,
                source_ref,
            });
            cursor.last_claim_id = id;
            id
        } else {
            cursor.last_claim_id
        };
        let after_tax_margin = margin_cents
            .checked_sub(assessed_cents)
            .ok_or(TxError::Overflow)?;
        cursor.retained_earnings_cents = i64::try_from(
            i128::from(cursor.retained_earnings_cents)
                .checked_add(after_tax_margin)
                .ok_or(TxError::Overflow)?,
        )
        .map_err(|_| TxError::Overflow)?;
        cursor.cumulative_assessed = cursor
            .cumulative_assessed
            .checked_add(assessed)
            .map_err(|_| TxError::Overflow)?;
        cursor.through_day = day;
        cursor.last_operation_id = operation_id;
        cursor.last_claim_id = claim_id;
    }
    Ok(())
}

fn stock_slice_basis(stock: &Stock, quantity: Quantity) -> Result<Money, TxError> {
    let end = stock
        .offset
        .checked_add(quantity)
        .map_err(|_| TxError::Overflow)?;
    let denominator = stock.origin_quantity.value();
    if denominator <= 0 || end > stock.origin_quantity || quantity <= Quantity::ZERO {
        return Err(TxError::Stock);
    }
    let origin = i128::from(stock.origin_basis.value());
    let left = origin * i128::from(stock.offset.value()) / i128::from(denominator);
    let right = origin * i128::from(end.value()) / i128::from(denominator);
    Money::new(i64::try_from(right - left).map_err(|_| TxError::Overflow)?)
        .map_err(|_| TxError::Overflow)
}

fn consume_stock(stock: &mut Stock, quantity: Quantity, basis: Money) -> Result<(), TxError> {
    stock.quantity = stock
        .quantity
        .checked_sub(quantity)
        .map_err(|_| TxError::Stock)?;
    stock.basis = stock.basis.checked_sub(basis).map_err(|_| TxError::Stock)?;
    stock.offset = stock
        .offset
        .checked_add(quantity)
        .map_err(|_| TxError::Overflow)?;
    if stock.quantity == Quantity::ZERO {
        if stock.basis != Money::ZERO {
            return Err(TxError::Stock);
        }
        stock.origin_quantity = Quantity::ZERO;
        stock.origin_basis = Money::ZERO;
        stock.offset = Quantity::ZERO;
        stock.lot_id = 0;
    }
    Ok(())
}

fn market_transfer(
    world: &World,
    accounts: &mut DirtyAccounts,
    from: u32,
    to: u32,
    amount: Money,
) -> Result<(), TxError> {
    if amount == Money::ZERO || from == to {
        return Ok(());
    }
    if amount < Money::ZERO {
        return Err(TxError::InvalidOperation);
    }
    let from_raw = world
        .account_ref(from)
        .map_err(|_| TxError::InvalidHandle)?;
    let from_index = stage_account(world, accounts, from_raw)?;
    let spendable = accounts[from_index]
        .1
        .cash
        .checked_sub(accounts[from_index].1.protected)
        .map_err(|_| TxError::Protection)?;
    if spendable < amount {
        return Err(TxError::InsufficientFunds);
    }
    accounts[from_index].1.cash = accounts[from_index]
        .1
        .cash
        .checked_sub(amount)
        .map_err(|_| TxError::InsufficientFunds)?;
    let to_raw = world.account_ref(to).map_err(|_| TxError::InvalidHandle)?;
    let to_index = stage_account(world, accounts, to_raw)?;
    accounts[to_index].1.cash = accounts[to_index]
        .1
        .cash
        .checked_add(amount)
        .map_err(|_| TxError::Overflow)?;
    Ok(())
}

fn prepare_housing_lease(
    caller: Caller,
    offer_id: u64,
    expected_offer_sha256: [u8; 32],
    world: &World,
    state: &mut crate::housing::HousingState,
    timeline: &mut crate::timeline::Timeline,
    accounts: &mut DirtyAccounts,
) -> Result<(), TxError> {
    if !matches!(caller, Caller::Account(_))
        || timeline.progress.phase.is_some()
        || timeline.progress.target.is_some()
        || timeline.progress.at != timeline.progress.last_complete
    {
        return Err(TxError::InvalidOperation);
    }
    let Caller::Account(tenant_account) = caller else {
        unreachable!()
    };
    let runtime = world.housing.as_deref().ok_or(TxError::InvalidOperation)?;
    let offer = runtime
        .opening
        .lease_offer(offer_id)
        .map_err(|_| TxError::InvalidOperation)?;
    if crate::housing::genesis_lease_offer_sha256(offer) != expected_offer_sha256 {
        return Err(TxError::InvalidOperation);
    }
    let unit_id = offer.unit_id;
    let current = state
        .units()
        .iter()
        .find(|unit| unit.unit_id == unit_id)
        .copied()
        .ok_or(TxError::InvalidOperation)?;
    let dwelling = runtime
        .opening
        .dwellings
        .binary_search_by_key(&unit_id, |row| row.fact.unit_id)
        .ok()
        .map(|index| runtime.opening.dwellings[index].fact)
        .ok_or(TxError::InvalidOperation)?;
    let accepted_at = crate::units::GameTime::new(
        i64::try_from(timeline.progress.last_complete).map_err(|_| TxError::Overflow)?,
    )
    .map_err(|_| TxError::InvalidOperation)?;
    let terms = offer.executable_terms();
    if terms.deposit != Money::ZERO || terms.starts_at != accepted_at {
        return Err(TxError::InvalidOperation);
    }
    let authorization = crate::housing::validate_genesis_lease_offer(
        &runtime.opening,
        offer_id,
        dwelling,
        world.scope.world(),
        world.scope.branch(),
        tenant_account,
        accepted_at,
        terms,
    )
    .map_err(|_| TxError::InvalidOperation)?;
    if authorization.people() == 0 || authorization.tenant_account() != tenant_account {
        return Err(TxError::InvalidOperation);
    }
    let tenant_handle = world
        .account_handle(tenant_account)
        .map_err(|_| TxError::InvalidHandle)?;
    let tenant = world
        .accounts
        .get(tenant_handle)
        .map_err(|_| TxError::InvalidHandle)?;
    if tenant.role != crate::assembly::Role::Household {
        return Err(TxError::Unauthorized);
    }
    world
        .account_handle(terms.escrow_account)
        .map_err(|_| TxError::InvalidHandle)?;
    let tenant_index = stage_account(
        world,
        accounts,
        world
            .account_ref(tenant_account)
            .map_err(|_| TxError::InvalidHandle)?,
    )?;
    let budget = crate::housing::HouseholdBudget {
        household_id: authorization.household_id(),
        account: crate::town_economy::AccountSnapshot {
            account: tenant_account,
            cash: accounts[tenant_index].1.cash,
            protected: accounts[tenant_index].1.protected,
        },
        people: authorization.people(),
        net_income_per_month: Money::ZERO,
        essential_costs_per_month: Money::ZERO,
        income_observed_at: accepted_at,
        income_window_complete: false,
        income_source_version: 0,
    };
    let plan = crate::housing::plan_lease(current, dwelling, budget, terms)
        .map_err(|_| TxError::InvalidOperation)?;
    if plan.first_due_amount <= Money::ZERO {
        return Err(TxError::InvalidOperation);
    }
    let due =
        u64::try_from(plan.updated_unit.next_due_at.value()).map_err(|_| TxError::Overflow)?;
    timeline.insert_rent_claim(
        crate::timeline::Claim {
            id: u64::from(terms.lease_id),
            payer: tenant_account,
            payee: authorization.owner_account(),
            due,
            principal: plan.first_due_amount,
            paid: Money::ZERO,
            processed: false,
            source_ref: 0,
        },
        crate::timeline::RentClaimSource {
            landlord: authorization.owner_account(),
            tenant: tenant_account,
            lease_id: terms.lease_id,
            unit_id,
        },
    )?;
    state
        .apply_genesis_offer_lease(plan, authorization)
        .map_err(|_| TxError::InvalidOperation)?;
    Ok(())
}

/// Applies the bounded construction projection and its external effects to the same
/// candidate Engine state. The caller publishes this candidate only after the Store's
/// durable hook succeeds.
#[allow(clippy::too_many_arguments)]
fn prepare_construction(
    caller: Caller,
    command: crate::construction::EngineCommand,
    operation_id: u64,
    world: &World,
    boot: &mut Option<crate::boot::Boot>,
    project: &mut crate::construction::Project,
    accounts: &mut DirtyAccounts,
    stocks: &mut DirtyStocks,
    resources: &mut ResourceRows,
    leases: &mut LeaseRows,
    next_token: &mut u64,
    created_token: &mut Option<CapacityToken>,
    timeline: Option<&crate::timeline::Timeline>,
    town: Option<&crate::town_runtime::MarketState>,
) -> Result<(), TxError> {
    use crate::construction::{EffectKind, EngineCommand, GateDecision, Receipt as CReceipt};
    let mut candidate = *project;
    let at = if let Some(timeline) = timeline {
        crate::units::GameTime::new(
            i64::try_from(timeline.progress.last_complete).map_err(|_| TxError::Overflow)?,
        )
        .map_err(|_| TxError::InvalidOperation)?
    } else {
        candidate
            .last_work_at
            .unwrap_or(crate::units::GameTime::ZERO)
    };
    let actor = match caller {
        Caller::System => 0,
        Caller::Account(id) => id,
    };
    if command == EngineCommand::File
        && candidate.compensation.is_some()
        && matches!(caller, Caller::System)
    {
        // A fixed compensation contract is only activated by the owner's real filing.
        return Err(TxError::Unauthorized);
    }
    if !candidate.recoverable_commands()
        && candidate.operating_edge_id != 0
        && matches!(
            command,
            EngineCommand::BeginDemolition { .. }
                | EngineCommand::AcceptWaste
                | EngineCommand::Close
        )
    {
        // Preserve exact historical negative WAL outcomes for pre-policy projects.
        return Err(TxError::InvalidOperation);
    }
    if command == EngineCommand::Start
        && candidate.operating_edge_id != 0
        && candidate.waste_receiver.is_some()
        && candidate.operating_published
    {
        validate_road_effect_withdrawal_boundary(timeline, town.map(|state| state.settled_day))?;
    }
    match command {
        EngineCommand::DecideGate {
            gate_id, decision, ..
        } => {
            let gate = candidate.gates[..candidate.gate_count as usize]
                .iter()
                .find(|gate| gate.id == gate_id)
                .ok_or(TxError::Unauthorized)?;
            if actor != gate.party && !matches!(caller, Caller::System) {
                return Err(TxError::Unauthorized);
            }
            if decision == GateDecision::Accepted && matches!(caller, Caller::System) {
                // The system may administer permits, while consent remains party-owned.
                if matches!(
                    gate.category,
                    crate::construction::GateCategory::OwnerConsent
                        | crate::construction::GateCategory::OccupantConsent
                        | crate::construction::GateCategory::Compensation
                ) {
                    return Err(TxError::Unauthorized);
                }
            }
        }
        EngineCommand::AcceptInspection { inspector, .. } => {
            if inspector != candidate.inspector
                || (actor != inspector && !matches!(caller, Caller::System))
            {
                return Err(TxError::Unauthorized);
            }
        }
        EngineCommand::Work { .. }
        | EngineCommand::Pause
        | EngineCommand::Resume
        | EngineCommand::SubmitInspection => {
            if actor != candidate.contractor && !matches!(caller, Caller::System) {
                return Err(TxError::Unauthorized);
            }
        }
        EngineCommand::BeginDemolition { .. } => {
            if actor != candidate.owner && !matches!(caller, Caller::System) {
                return Err(TxError::Unauthorized);
            }
            if !candidate.can_request_uncommissioned_clearance()
                && (!candidate.operating_published || candidate.operating_resource == 0)
            {
                return Err(TxError::InvalidOperation);
            }
        }
        EngineCommand::AcceptWaste => {
            if actor != candidate.waste_receiver_account && !matches!(caller, Caller::System) {
                return Err(TxError::Unauthorized);
            }
        }
        EngineCommand::Close => {
            if actor != candidate.owner && !matches!(caller, Caller::System) {
                return Err(TxError::Unauthorized);
            }
        }
        _ if actor != candidate.owner && !matches!(caller, Caller::System) => {
            return Err(TxError::Unauthorized);
        }
        _ => {}
    }

    // Compensation acceptance is the payment event: debit/credit, persisted receipt
    // identity, and Gate acceptance all remain in this single Engine candidate.
    if let EngineCommand::DecideGate {
        gate_id,
        decision: GateDecision::Accepted,
        ..
    } = command
    {
        if let Some((payee, amount)) = candidate.compensation_payment_for(gate_id) {
            market_transfer(world, accounts, candidate.owner, payee, amount)?;
            candidate
                .record_compensation_settlement(gate_id, operation_id)
                .map_err(TxError::Construction)?;
        }
    }

    if candidate.operating_edge_id != 0 {
        match command {
            EngineCommand::Cancel if !candidate.recoverable_commands() => {
                return Err(TxError::InvalidOperation)
            }
            EngineCommand::AcceptInspection { accepted: true, .. } => {
                let timeline = timeline.ok_or(TxError::InvalidOperation)?;
                let at = timeline.progress.last_complete;
                if timeline.progress.phase.is_some()
                    || timeline.progress.target.is_some()
                    || at % crate::timeline::DAY != 0
                    || town.map(|market| market.settled_day) != Some(at / crate::timeline::DAY)
                {
                    return Err(TxError::InvalidOperation);
                }
            }
            _ => {}
        }
    }

    let effect = match command {
        EngineCommand::Work {
            at: requested,
            elapsed_seconds,
            worker_seconds,
        } => {
            if requested != at {
                return Err(TxError::Construction(crate::construction::Error::Time));
            }
            if candidate.utility_contract.is_some() {
                let timeline =
                    timeline.ok_or(TxError::Construction(crate::construction::Error::Time))?;
                let boot_clock = boot.as_ref().ok_or(TxError::InvalidOperation)?.time;
                if timeline.progress.phase.is_some()
                    || timeline.progress.target.is_some()
                    || boot_clock != timeline.progress.last_complete
                {
                    return Err(TxError::Construction(crate::construction::Error::Time));
                }
            }
            let mut available = [crate::units::Quantity::ZERO; crate::construction::MAX_MATERIALS];
            for (i, slot) in available
                .iter_mut()
                .enumerate()
                .take(candidate.active_material_count())
            {
                let h = world
                    .stock_ref(candidate.materials[i].stock_id)
                    .map_err(|_| TxError::InvalidHandle)?;
                let index = stage_stock(world, stocks, h)?;
                *slot = stocks[index].1.quantity;
            }
            let owner_raw = world
                .account_ref(candidate.owner)
                .map_err(|_| TxError::InvalidHandle)?;
            let owner_idx = stage_account(world, accounts, owner_raw)?;
            let spendable = accounts[owner_idx]
                .1
                .cash
                .checked_sub(accounts[owner_idx].1.protected)
                .map_err(|_| TxError::Protection)?;
            candidate
                .propose_work_authoritative(
                    requested,
                    elapsed_seconds,
                    spendable,
                    available,
                    worker_seconds,
                )
                .map_err(TxError::Construction)?
        }
        _ => candidate
            .apply(at, command.into_command(&candidate))
            .map_err(TxError::Construction)?,
    };

    if let Some(mut effect) = effect {
        if !candidate.recoverable_commands() && effect.operation_id != operation_id {
            // Legacy replay keeps the historical exception list. V2 uses independent
            // Envelope identities: after a durable rejection, a new request can retry
            // the unchanged internal effect. Old IDs still return their original result.
            if !matches!(
                effect.kind,
                EffectKind::WithdrawOperatingCapacity | EffectKind::ReceiveWaste
            ) {
                return Err(TxError::InvalidOperation);
            }
        }
        match effect.kind {
            EffectKind::ReserveSite => {
                let r = resources
                    .iter_mut()
                    .find(|r| r.id == effect.site_resource)
                    .ok_or(TxError::Resource)?;
                let occupied = r
                    .occupied
                    .checked_add(effect.site_units)
                    .ok_or(TxError::Overflow)?;
                if occupied.checked_add(r.reserved).ok_or(TxError::Overflow)? > r.capacity {
                    return Err(TxError::Capacity);
                }
                let slot = leases
                    .iter_mut()
                    .find(|lease| lease.is_none())
                    .ok_or(TxError::Lease)?;
                let token = *next_token;
                *next_token = next_token.checked_add(1).ok_or(TxError::Overflow)?;
                *slot = Some(Lease {
                    token,
                    actor: candidate.owner,
                    resource: effect.site_resource,
                    units: effect.site_units,
                    occupied: true,
                });
                r.occupied = occupied;
                candidate.site_token = Some(token);
                *created_token = Some(CapacityToken {
                    world: world.scope.world(),
                    branch: world.scope.branch(),
                    number: token,
                });
            }
            EffectKind::Work => {
                if candidate.utility_contract.is_some() {
                    let contract = candidate
                        .utility_contract
                        .ok_or(TxError::InvalidOperation)?;
                    let utility_boot = boot.as_mut().ok_or(TxError::InvalidOperation)?;
                    utility_boot
                        .validate_construction_contract(candidate)
                        .map_err(|_| TxError::InvalidOperation)?;
                    for (index, supply) in [contract.electricity, contract.water]
                        .into_iter()
                        .enumerate()
                    {
                        let quantity = effect.utility_quantities[index];
                        let fee = effect.utility_fees[index];
                        if quantity == crate::units::Quantity::ZERO {
                            if fee != crate::units::Money::ZERO {
                                return Err(TxError::InvalidOperation);
                            }
                            continue;
                        }
                        let supply = supply.ok_or(TxError::InvalidOperation)?;
                        {
                            let mut ledger = BootLedger {
                                base: EconomicLedger {
                                    world,
                                    accounts,
                                    stocks,
                                },
                                resources,
                                leases,
                                next_token,
                            };
                            utility_boot.export_local_construction_utility(
                                index,
                                quantity,
                                &mut ledger,
                            )?;
                        }
                        if fee != crate::units::Money::ZERO {
                            let payer = stage_account(
                                world,
                                accounts,
                                world
                                    .account_ref(effect.payer)
                                    .map_err(|_| TxError::InvalidHandle)?,
                            )?;
                            let payee = stage_account(
                                world,
                                accounts,
                                world
                                    .account_ref(supply.operator)
                                    .map_err(|_| TxError::InvalidHandle)?,
                            )?;
                            let spendable = accounts[payer]
                                .1
                                .cash
                                .checked_sub(accounts[payer].1.protected)
                                .map_err(|_| TxError::Protection)?;
                            if spendable < fee {
                                return Err(TxError::Construction(
                                    crate::construction::Error::Funds,
                                ));
                            }
                            accounts[payer].1.cash = accounts[payer]
                                .1
                                .cash
                                .checked_sub(fee)
                                .map_err(|_| TxError::Overflow)?;
                            accounts[payee].1.cash = accounts[payee]
                                .1
                                .cash
                                .checked_add(fee)
                                .map_err(|_| TxError::Overflow)?;
                        }
                    }
                }
                // Charge the contractor from the owner's account in the same candidate.
                let payer = stage_account(
                    world,
                    accounts,
                    world
                        .account_ref(effect.payer)
                        .map_err(|_| TxError::InvalidHandle)?,
                )?;
                let payee = stage_account(
                    world,
                    accounts,
                    world
                        .account_ref(effect.payee)
                        .map_err(|_| TxError::InvalidHandle)?,
                )?;
                let spendable = accounts[payer]
                    .1
                    .cash
                    .checked_sub(accounts[payer].1.protected)
                    .map_err(|_| TxError::Protection)?;
                if spendable < effect.cash {
                    return Err(TxError::Construction(crate::construction::Error::Funds));
                }
                accounts[payer].1.cash = accounts[payer]
                    .1
                    .cash
                    .checked_sub(effect.cash)
                    .map_err(|_| TxError::Overflow)?;
                accounts[payee].1.cash = accounts[payee]
                    .1
                    .cash
                    .checked_add(effect.cash)
                    .map_err(|_| TxError::Overflow)?;
                let mut total_basis = effect.cash;
                for utility_fee in effect.utility_fees {
                    total_basis = total_basis
                        .checked_add(utility_fee)
                        .map_err(|_| TxError::Overflow)?;
                }
                for i in 0..candidate.active_material_count() {
                    // Governed electricity/water/fuel must use their owning system's
                    // consumption accounting, never the ordinary material COGS path.
                    // The policy gate keeps historical WAL outcomes unchanged.
                    if candidate.recoverable_commands()
                        && world
                            .boot
                            .as_ref()
                            .is_some_and(|boot| boot.governs(effect.stock_ids[i]))
                    {
                        return Err(TxError::Unauthorized);
                    }
                    if candidate.material_accounting.is_some()
                        && world.exchange.is_some()
                        && (1..=14).contains(&effect.stock_ids[i])
                        && !(7..=12).contains(&effect.stock_ids[i])
                    {
                        return Err(TxError::Unauthorized);
                    }
                    let raw = world
                        .stock_ref(effect.stock_ids[i])
                        .map_err(|_| TxError::InvalidHandle)?;
                    let index = stage_stock(world, stocks, raw)?;
                    let old = stocks[index].1;
                    if world
                        .accounts
                        .get(old.owner)
                        .map_err(|_| TxError::InvalidHandle)?
                        .id
                        != candidate.owner
                    {
                        return Err(TxError::Unauthorized);
                    }
                    let end = old
                        .offset
                        .checked_add(effect.materials[i])
                        .map_err(|_| TxError::Overflow)?;
                    if end > old.origin_quantity {
                        return Err(TxError::Construction(crate::construction::Error::Materials));
                    }
                    let basis_at = |offset: i64| -> Result<i64, TxError> {
                        i64::try_from(
                            i128::from(old.origin_basis.value()) * i128::from(offset)
                                / i128::from(old.origin_quantity.value()),
                        )
                        .map_err(|_| TxError::Overflow)
                    };
                    let consumed_basis = crate::units::Money::new(
                        basis_at(end.value())? - basis_at(old.offset.value())?,
                    )
                    .map_err(|_| TxError::Overflow)?;
                    stocks[index].1.quantity =
                        old.quantity.checked_sub(effect.materials[i]).map_err(|_| {
                            TxError::Construction(crate::construction::Error::Materials)
                        })?;
                    stocks[index].1.offset = end;
                    stocks[index].1.basis = old
                        .basis
                        .checked_sub(consumed_basis)
                        .map_err(|_| TxError::Stock)?;
                    effect.material_basis[i] = consumed_basis;
                    total_basis = total_basis
                        .checked_add(consumed_basis)
                        .map_err(|_| TxError::Overflow)?;
                }
                effect.total_basis = total_basis;
                let p = candidate
                    .pending
                    .as_mut()
                    .ok_or(TxError::InvalidOperation)?;
                p.material_basis = effect.material_basis;
                p.total_basis = total_basis;
                if candidate
                    .spent
                    .checked_add(total_basis)
                    .map_err(|_| TxError::Overflow)?
                    > candidate.budget
                {
                    return Err(TxError::Construction(crate::construction::Error::Funds));
                }
            }
            EffectKind::Inspect => {}
            EffectKind::PublishOperatingCapacity => {
                if candidate.operating_resource == 0
                    || candidate.operating_units == 0
                    || candidate.operating_published
                {
                    return Err(TxError::InvalidOperation);
                }
                let resource = resources
                    .iter_mut()
                    .find(|resource| resource.id == candidate.operating_resource)
                    .ok_or(TxError::Resource)?;
                if resource.capacity != candidate.operating_base_capacity {
                    return Err(TxError::Capacity);
                }
                resource.capacity = resource
                    .capacity
                    .checked_add(candidate.operating_units)
                    .ok_or(TxError::Overflow)?;
            }
            EffectKind::WithdrawOperatingCapacity => {
                if !candidate.operating_published
                    || candidate.operating_resource == 0
                    || candidate.operating_withdrawn_at.is_some()
                {
                    return Err(TxError::InvalidOperation);
                }
                let resource = resources
                    .iter_mut()
                    .find(|resource| resource.id == candidate.operating_resource)
                    .ok_or(TxError::Resource)?;
                let restored = resource
                    .capacity
                    .checked_sub(candidate.operating_units)
                    .ok_or(TxError::Capacity)?;
                if resource
                    .reserved
                    .checked_add(resource.occupied)
                    .ok_or(TxError::Overflow)?
                    > restored
                {
                    return Err(TxError::Capacity);
                }
                resource.capacity = restored;
            }
            EffectKind::ReceiveWaste => {
                if effect.waste_units == 0
                    || effect.waste_resource == 0
                    || effect.waste_units != candidate.effective_waste_units()
                    || effect.waste_resource != candidate.waste_receiver_resource
                    || effect.cash != candidate.effective_waste_fee()
                    || effect.waste_receiver != Some(candidate.waste_receiver_account)
                    || effect.payee != candidate.waste_receiver_account
                {
                    return Err(TxError::InvalidOperation);
                }
                let payer = stage_account(
                    world,
                    accounts,
                    world
                        .account_ref(effect.payer)
                        .map_err(|_| TxError::InvalidHandle)?,
                )?;
                let payee = stage_account(
                    world,
                    accounts,
                    world
                        .account_ref(effect.payee)
                        .map_err(|_| TxError::InvalidHandle)?,
                )?;
                let spendable = accounts[payer]
                    .1
                    .cash
                    .checked_sub(accounts[payer].1.protected)
                    .map_err(|_| TxError::Protection)?;
                if spendable < effect.cash {
                    return Err(TxError::Construction(crate::construction::Error::Funds));
                }
                let r = resources
                    .iter_mut()
                    .find(|r| r.id == effect.waste_resource)
                    .ok_or(TxError::Resource)?;
                let occupied = r
                    .occupied
                    .checked_add(effect.waste_units)
                    .ok_or(TxError::Overflow)?;
                if occupied.checked_add(r.reserved).ok_or(TxError::Overflow)? > r.capacity {
                    return Err(TxError::Capacity);
                }
                let slot = leases
                    .iter_mut()
                    .find(|lease| lease.is_none())
                    .ok_or(TxError::Lease)?;
                let token = *next_token;
                *next_token = next_token.checked_add(1).ok_or(TxError::Overflow)?;
                *slot = Some(Lease {
                    token,
                    actor: effect.payee,
                    resource: effect.waste_resource,
                    units: effect.waste_units,
                    occupied: true,
                });
                accounts[payer].1.cash = accounts[payer]
                    .1
                    .cash
                    .checked_sub(effect.cash)
                    .map_err(|_| TxError::Overflow)?;
                accounts[payee].1.cash = accounts[payee]
                    .1
                    .cash
                    .checked_add(effect.cash)
                    .map_err(|_| TxError::Overflow)?;
                r.occupied = occupied;
            }
            EffectKind::ReleaseSite => {
                if effect.site_resource != candidate.site_resource
                    || effect.site_units != candidate.site_units
                {
                    return Err(TxError::Lease);
                }
                let token = candidate.site_token.ok_or(TxError::Lease)?;
                let mut found = None;
                for (index, lease) in leases.iter().enumerate() {
                    if lease.is_some_and(|lease| lease.token == token) {
                        if found.is_some() {
                            return Err(TxError::Lease);
                        }
                        found = Some(index);
                    }
                }
                let index = found.ok_or(TxError::Lease)?;
                let lease = leases[index].ok_or(TxError::Lease)?;
                if lease.actor != candidate.owner
                    || lease.resource != candidate.site_resource
                    || lease.units != candidate.site_units
                    || !lease.occupied
                {
                    return Err(TxError::Lease);
                }
                let r = resources
                    .iter_mut()
                    .find(|r| r.id == candidate.site_resource)
                    .ok_or(TxError::Resource)?;
                r.occupied = r
                    .occupied
                    .checked_sub(candidate.site_units)
                    .ok_or(TxError::Lease)?;
                leases[index] = None;
            }
        }
        candidate
            .confirm(CReceipt {
                operation_id: effect.operation_id,
                success: true,
                cash: effect.cash,
                materials: effect.materials,
                utility_quantities: effect.utility_quantities,
                utility_fees: effect.utility_fees,
                total_basis: effect.total_basis,
                site_units: effect.site_units,
                waste_accepted: effect.kind == EffectKind::ReceiveWaste,
            })
            .map_err(TxError::Construction)?;
    }
    *project = candidate;
    Ok(())
}

// Only the trusted regional workflow calls this, never a user-provided payer alias.
fn settle_obligation(
    world: &World,
    accounts: &mut DirtyAccounts,
    o: &mut crate::region::Obligation,
) -> Result<(), TxError> {
    if o.arrears == Money::ZERO {
        return Ok(());
    }
    let f = stage_account(
        world,
        accounts,
        world
            .account_ref(o.payer)
            .map_err(|_| TxError::InvalidHandle)?,
    )?;
    let t = stage_account(
        world,
        accounts,
        world
            .account_ref(o.payee)
            .map_err(|_| TxError::InvalidHandle)?,
    )?;
    let available = accounts[f]
        .1
        .cash
        .checked_sub(accounts[f].1.protected)
        .map_err(|_| TxError::Protection)?;
    let paid = o.arrears.min(available);
    accounts[f].1.cash = accounts[f]
        .1
        .cash
        .checked_sub(paid)
        .map_err(|_| TxError::Overflow)?;
    accounts[t].1.cash = accounts[t]
        .1
        .cash
        .checked_add(paid)
        .map_err(|_| TxError::Overflow)?;
    o.arrears = o.arrears.checked_sub(paid).map_err(|_| TxError::Overflow)?;
    Ok(())
}

struct EconomicLedger<'a> {
    world: &'a World,
    accounts: &'a mut DirtyAccounts,
    stocks: &'a mut DirtyStocks,
}
impl crate::economy::Ledger for EconomicLedger<'_> {
    fn stock(&mut self, id: u32) -> Result<Stock, TxError> {
        let h = self
            .world
            .stock_handle(id)
            .map_err(|_| TxError::InvalidHandle)?;
        if let Some((_, s)) = self.stocks.iter().find(|(old, _)| *old == h) {
            return Ok(*s);
        }
        self.world
            .stocks
            .get(h)
            .copied()
            .map_err(|_| TxError::InvalidHandle)
    }
    fn put_stock(&mut self, id: u32, stock: Stock) -> Result<(), TxError> {
        let raw = self
            .world
            .stock_ref(id)
            .map_err(|_| TxError::InvalidHandle)?;
        let i = stage_stock(self.world, self.stocks, raw)?;
        self.stocks[i].1 = stock;
        Ok(())
    }
    fn cash(&mut self, id: u32) -> Result<Money, TxError> {
        let raw = self
            .world
            .account_ref(id)
            .map_err(|_| TxError::InvalidHandle)?;
        let i = stage_account(self.world, self.accounts, raw)?;
        Ok(self.accounts[i].1.cash)
    }
    fn available(&mut self, id: u32) -> Result<Money, TxError> {
        let raw = self
            .world
            .account_ref(id)
            .map_err(|_| TxError::InvalidHandle)?;
        let i = stage_account(self.world, self.accounts, raw)?;
        self.accounts[i]
            .1
            .cash
            .checked_sub(self.accounts[i].1.protected)
            .map_err(|_| TxError::Protection)
    }
    fn protect(&mut self, id: u32, amount: Money) -> Result<(), TxError> {
        if self.available(id)? < amount {
            return Err(TxError::InsufficientFunds);
        }
        let raw = self
            .world
            .account_ref(id)
            .map_err(|_| TxError::InvalidHandle)?;
        let i = stage_account(self.world, self.accounts, raw)?;
        self.accounts[i].1.protected = self.accounts[i]
            .1
            .protected
            .checked_add(amount)
            .map_err(|_| TxError::Overflow)?;
        Ok(())
    }
    fn pay(&mut self, from: u32, to: u32, amount: Money, protected: bool) -> Result<(), TxError> {
        if from == to {
            return Err(TxError::InvalidOperation);
        }
        if !protected && self.available(from)? < amount {
            return Err(TxError::InsufficientFunds);
        }
        let f = stage_account(
            self.world,
            self.accounts,
            self.world
                .account_ref(from)
                .map_err(|_| TxError::InvalidHandle)?,
        )?;
        let t = stage_account(
            self.world,
            self.accounts,
            self.world
                .account_ref(to)
                .map_err(|_| TxError::InvalidHandle)?,
        )?;
        if protected {
            self.accounts[f].1.protected = self.accounts[f]
                .1
                .protected
                .checked_sub(amount)
                .map_err(|_| TxError::Protection)?;
        }
        self.accounts[f].1.cash = self.accounts[f]
            .1
            .cash
            .checked_sub(amount)
            .map_err(|_| TxError::InsufficientFunds)?;
        self.accounts[t].1.cash = self.accounts[t]
            .1
            .cash
            .checked_add(amount)
            .map_err(|_| TxError::Overflow)?;
        Ok(())
    }
}

struct BootLedger<'a> {
    base: EconomicLedger<'a>,
    resources: &'a mut ResourceRows,
    leases: &'a mut LeaseRows,
    next_token: &'a mut u64,
}
impl crate::boot::Ledger for BootLedger<'_> {
    fn stock(&mut self, id: u32) -> Result<Stock, TxError> {
        crate::economy::Ledger::stock(&mut self.base, id)
    }
    fn put(&mut self, id: u32, s: Stock) -> Result<(), TxError> {
        crate::economy::Ledger::put_stock(&mut self.base, id, s)
    }
    fn cash(&mut self, id: u32) -> Result<Money, TxError> {
        crate::economy::Ledger::cash(&mut self.base, id)
    }
    fn pay(&mut self, from: u32, to: u32, amount: Money) -> Result<(), TxError> {
        crate::economy::Ledger::pay(&mut self.base, from, to, amount, false)
    }
    fn claim(&mut self, mut ids: [u32; 2]) -> Result<[u64; 2], TxError> {
        ids.sort();
        let mut tokens = [0; 2];
        for (i, id) in ids.into_iter().enumerate() {
            let r = self
                .resources
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or(TxError::Resource)?;
            if r.reserved
                .checked_add(r.occupied)
                .and_then(|v| v.checked_add(1))
                .ok_or(TxError::Overflow)?
                > r.capacity
            {
                return Err(TxError::Capacity);
            }
            let slot = self
                .leases
                .iter_mut()
                .find(|l| l.is_none())
                .ok_or(TxError::Lease)?;
            let token = *self.next_token;
            *self.next_token = self.next_token.checked_add(1).ok_or(TxError::Overflow)?;
            *slot = Some(Lease {
                token,
                actor: 0,
                resource: id,
                units: 1,
                occupied: true,
            });
            r.occupied += 1;
            tokens[i] = token;
        }
        Ok(tokens)
    }
    fn release(&mut self, tokens: [u64; 2]) -> Result<(), TxError> {
        for token in tokens {
            let slot = self
                .leases
                .iter_mut()
                .find(|l| l.is_some_and(|l| l.token == token))
                .ok_or(TxError::Lease)?;
            let lease = slot.ok_or(TxError::Lease)?;
            let r = self
                .resources
                .iter_mut()
                .find(|r| r.id == lease.resource)
                .ok_or(TxError::Resource)?;
            if !lease.occupied || lease.units != 1 || lease.actor != 0 {
                return Err(TxError::Lease);
            }
            r.occupied = r.occupied.checked_sub(1).ok_or(TxError::Lease)?;
            *slot = None;
        }
        Ok(())
    }
    fn owns(&mut self, mut ids: [u32; 2], tokens: [u64; 2]) -> bool {
        ids.sort();
        ids.into_iter().zip(tokens).all(|(id, token)| {
            self.leases.iter().flatten().any(|l| {
                l.token == token && l.resource == id && l.occupied && l.units == 1 && l.actor == 0
            })
        })
    }
}

impl crate::exchange::Ledger for BootLedger<'_> {
    fn protect(&mut self, id: u32, amount: Money) -> Result<(), TxError> {
        crate::economy::Ledger::protect(&mut self.base, id, amount)
    }
    fn pay_protected(&mut self, from: u32, to: u32, amount: Money) -> Result<(), TxError> {
        crate::economy::Ledger::pay(&mut self.base, from, to, amount, true)
    }
    fn protection(&mut self, id: u32) -> Result<Money, TxError> {
        let raw = self
            .base
            .world
            .account_ref(id)
            .map_err(|_| TxError::InvalidHandle)?;
        let i = stage_account(self.base.world, self.base.accounts, raw)?;
        Ok(self.base.accounts[i].1.protected)
    }
    fn claim_path(
        &mut self,
        requests: &[(u32, u32)],
    ) -> Result<[Option<u64>; crate::exchange::PATH], TxError> {
        if requests.len() > crate::exchange::PATH {
            return Err(TxError::Lease);
        }
        let mut sorted = requests.to_vec();
        sorted.sort();
        if sorted.windows(2).any(|p| p[0].0 == p[1].0) {
            return Err(TxError::Resource);
        }
        let mut tokens = [None; crate::exchange::PATH];
        for (i, (id, units)) in sorted.into_iter().enumerate() {
            if units == 0 {
                return Err(TxError::Capacity);
            }
            let r = self
                .resources
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or(TxError::Resource)?;
            if r.reserved
                .checked_add(r.occupied)
                .and_then(|n| n.checked_add(units))
                .ok_or(TxError::Overflow)?
                > r.capacity
            {
                return Err(TxError::Capacity);
            }
            let slot = self
                .leases
                .iter_mut()
                .find(|l| l.is_none())
                .ok_or(TxError::Lease)?;
            let token = *self.next_token;
            *self.next_token = self.next_token.checked_add(1).ok_or(TxError::Overflow)?;
            *slot = Some(Lease {
                token,
                actor: 0,
                resource: id,
                units,
                occupied: true,
            });
            r.occupied += units;
            tokens[i] = Some(token);
        }
        Ok(tokens)
    }
    fn release_path(
        &mut self,
        tokens: [Option<u64>; crate::exchange::PATH],
    ) -> Result<(), TxError> {
        for token in tokens.into_iter().flatten() {
            let slot = self
                .leases
                .iter_mut()
                .find(|s| s.is_some_and(|l| l.token == token))
                .ok_or(TxError::Lease)?;
            let lease = slot.ok_or(TxError::Lease)?;
            if lease.actor != 0 || !lease.occupied {
                return Err(TxError::Lease);
            }
            let r = self
                .resources
                .iter_mut()
                .find(|r| r.id == lease.resource)
                .ok_or(TxError::Resource)?;
            r.occupied = r.occupied.checked_sub(lease.units).ok_or(TxError::Lease)?;
            *slot = None;
        }
        Ok(())
    }
    fn owns_path(
        &self,
        requests: &[(u32, u32)],
        tokens: [Option<u64>; crate::exchange::PATH],
    ) -> bool {
        let mut sorted = requests.to_vec();
        sorted.sort();
        tokens.iter().flatten().count() == sorted.len()
            && sorted
                .into_iter()
                .zip(tokens)
                .all(|((resource, units), token)| {
                    self.leases.iter().flatten().any(|l| {
                        Some(l.token) == token
                            && l.actor == 0
                            && l.occupied
                            && l.resource == resource
                            && l.units == units
                    })
                })
    }
}

impl Action {
    pub fn is_new_commitment(&self) -> bool {
        matches!(
            self,
            Self::Background {
                command: crate::background::Command::Enable
            } | Self::Timeline {
                command: crate::timeline::Command::Claims { .. }
                    | crate::timeline::Command::Lend { .. }
                    | crate::timeline::Command::AddPayroll { .. }
            } | Self::Exchange {
                command: crate::exchange::Command::Dispatch { .. }
            } | Self::Boot {
                command: crate::boot::Command::Start { .. }
            } | Self::Construction {
                command: crate::construction::EngineCommand::Start
                    | crate::construction::EngineCommand::Work { .. }
            } | Self::RegionMigrate { .. }
                | Self::EquityContribute { .. }
                | Self::SpatialPermission { .. }
                | Self::HousingLease { .. }
                | Self::ReserveCapacity { .. }
        )
    }
    /// Only management of existing obligations can draw on storage's emergency reserve.
    pub fn is_responsibility_control(&self) -> bool {
        matches!(
            self,
            Self::Timeline {
                command: crate::timeline::Command::Advance { .. }
                    | crate::timeline::Command::SettleClaim { .. }
                    | crate::timeline::Command::SettlePayroll { .. }
                    | crate::timeline::Command::Terminate { .. }
                    | crate::timeline::Command::StopWork { .. }
            } | Self::Inbox {
                command: crate::inbox::Command::Receive { .. }
                    | crate::inbox::Command::External { .. }
                    | crate::inbox::Command::Incident { .. }
                    | crate::inbox::Command::Settle { .. }
                    | crate::inbox::Command::Handoff { .. }
                    | crate::inbox::Command::Archive { .. }
            } | Self::Exchange {
                command: crate::exchange::Command::Cancel { .. }
                    | crate::exchange::Command::SetLink { .. }
            } | Self::RegionAdvance { .. }
                | Self::RegionSettleDebt { .. }
                | Self::BeginExit { .. }
                | Self::SettleExit { .. }
                | Self::CloseExit { .. }
                | Self::ReleaseCapacity { .. }
                | Self::Boot {
                    command: crate::boot::Command::Fault { .. }
                        | crate::boot::Command::ClearFailure { .. }
                }
        )
    }
}

pub(super) fn market_is_suspended(
    state: Option<&crate::town_runtime::MarketState>,
    account: u32,
) -> bool {
    account != 0
        && state.is_some_and(|state| {
            state
                .suspended_operators
                .as_slice()
                .iter()
                .any(|row| row.account == account)
        })
}

pub(super) fn market_is_exiting(
    state: Option<&crate::town_runtime::MarketState>,
    account: u32,
) -> bool {
    account != 0
        && state.is_some_and(|state| {
            state
                .exit
                .is_some_and(|exit| exit.company_account == account)
        })
}

fn validate_town_suspension_consistency(
    state: Option<&crate::town_runtime::MarketState>,
    timeline: Option<&crate::timeline::Timeline>,
) -> Result<(), &'static str> {
    let Some(state) = state else {
        return Ok(());
    };
    if state.suspended_operators.is_empty() {
        return Ok(());
    }
    let timeline = timeline.ok_or("town_suspension_without_timeline")?;
    for suspension in state.suspended_operators.as_slice() {
        if suspension.suspended_at > timeline.progress.last_complete {
            return Err("town_suspension_from_future");
        }
        if timeline.payroll.iter().any(|payroll| {
            payroll.spec.payer == suspension.account
                && (payroll.working
                    || payroll.next_due.is_some()
                    || payroll.billed != payroll.accrued)
        }) {
            return Err("town_suspended_payroll_active");
        }
    }
    Ok(())
}

fn validate_town_exit_state(
    profile: Option<&crate::town_runtime::MarketProfile>,
    state: Option<&crate::town_runtime::MarketState>,
    timeline: Option<&crate::timeline::Timeline>,
) -> Result<(), &'static str> {
    let Some(state) = state else {
        return Ok(());
    };
    let Some(runtime) = state.exit else {
        return Ok(());
    };
    let profile = profile.ok_or("town_exit_without_profile")?;
    let policy = profile
        .exit_policy
        .filter(|policy| {
            policy.policy_id == runtime.policy_id
                && policy.company_account == runtime.company_account
        })
        .ok_or("town_exit_policy_missing")?;
    let timeline = timeline.ok_or("town_exit_without_timeline")?;
    if runtime.started_at > timeline.progress.last_complete
        || !exit_timeline_bindings_match(&policy, timeline, profile, state)
        || !tax_assessments_cover_exit(runtime.company_account, profile, state, timeline)
        || exit_has_unsupported_receivables(runtime.company_account, timeline)
    {
        return Err("town_exit_clock_or_binding");
    }
    if timeline.payroll.iter().any(|payroll| {
        payroll.spec.payer == runtime.company_account
            && (payroll.working
                || payroll.next_due.is_some()
                || payroll.pending_cycle.is_some()
                || payroll.billed != payroll.accrued)
    }) {
        return Err("town_exit_payroll_active");
    }
    if runtime.phase == crate::town_runtime::ExitPhase::Closed
        && (timeline.claims.iter().any(|claim| {
            (claim.payer == runtime.company_account || claim.payee == runtime.company_account)
                && claim.paid != claim.principal
        }) || timeline.payroll.iter().any(|payroll| {
            (payroll.spec.payer == runtime.company_account && payroll.paid != payroll.accrued)
                || (payroll.spec.payee == runtime.company_account
                    && (payroll.paid != payroll.accrued
                        || payroll.working
                        || payroll.next_due.is_some()))
        }))
    {
        return Err("town_exit_closed_with_liability");
    }
    Ok(())
}

fn validate_town_exit_world(
    world: &World,
    timeline: Option<&crate::timeline::Timeline>,
    leases: &LeaseRows,
) -> Result<(), &'static str> {
    let Some(market) = world.town else {
        return Ok(());
    };
    let Some(runtime) = market.state.exit else {
        return Ok(());
    };
    let region_has_company_obligation = world.region.is_some_and(|region| {
        region.obligations.iter().flatten().any(|obligation| {
            obligation.payer == runtime.company_account
                || obligation.payee == runtime.company_account
        })
    });
    if region_has_company_obligation
        || town_exit_profile_dependency(&market.profile, runtime.company_account)
        || town_exit_dependencies(
            runtime.company_account,
            world.exchange.as_ref(),
            world.boot.as_ref(),
            world.economy.as_ref(),
            world.construction.as_ref(),
            leases,
        )
    {
        return Err("town_exit_dependency");
    }
    if runtime.phase == crate::town_runtime::ExitPhase::Closed {
        let account = world
            .account_handle(runtime.company_account)
            .ok()
            .and_then(|handle| world.accounts.get(handle).ok())
            .ok_or("town_exit_company_missing")?;
        if account.cash != Money::ZERO
            || account.protected != Money::ZERO
            || world.stock_ids.iter().any(|(_, handle)| {
                world.stocks.get(*handle).is_ok_and(|stock| {
                    world
                        .accounts
                        .get(stock.owner)
                        .is_ok_and(|owner| owner.id == runtime.company_account)
                        && stock.quantity != Quantity::ZERO
                })
            })
            || timeline.is_none()
        {
            return Err("town_exit_closed_with_assets");
        }
    }
    Ok(())
}

fn active_company_dependency(
    account: u32,
    exchange: Option<&crate::exchange::Exchange>,
    boot: Option<&crate::boot::Boot>,
    construction: Option<&crate::construction::Project>,
    leases: &LeaseRows,
) -> bool {
    let reserved_capacity = leases.iter().flatten().any(|lease| lease.actor == account);
    let exchange_pending = exchange.is_some_and(|exchange| {
        exchange.quotes.iter().any(|quote| {
            matches!(
                quote.status,
                crate::exchange::Status::Pending | crate::exchange::Status::Transit
            ) && [quote.spec.seller, quote.spec.buyer, quote.spec.carrier].contains(&account)
        })
    });
    let boot_active = boot.is_some_and(|boot| {
        [boot.operator, boot.contractor].contains(&account)
            && boot.jobs.iter().any(|job| {
                matches!(
                    job.phase,
                    crate::boot::Phase::Running | crate::boot::Phase::Failed
                )
            })
    });
    let construction_active = construction.is_some_and(|project| {
        [project.owner, project.contractor].contains(&account)
            && !matches!(
                project.phase,
                crate::construction::Phase::Operational
                    | crate::construction::Phase::Closed
                    | crate::construction::Phase::Rejected
                    | crate::construction::Phase::Cancelled
            )
    });
    reserved_capacity || exchange_pending || boot_active || construction_active
}

fn town_exit_policy(
    world: &World,
    company: u32,
    policy_id: u32,
) -> Result<&crate::town_runtime::ExitPolicy, TxError> {
    let market = world.town.as_ref().ok_or(TxError::InvalidOperation)?;
    let policy = market
        .profile
        .exit_policy
        .as_ref()
        .filter(|policy| policy.company_account == company && policy.policy_id == policy_id)
        .ok_or(TxError::InvalidOperation)?;
    Ok(policy)
}

fn exit_timeline_bindings_match(
    policy: &crate::town_runtime::ExitPolicy,
    timeline: &crate::timeline::Timeline,
    profile: &crate::town_runtime::MarketProfile,
    state: &crate::town_runtime::MarketState,
) -> bool {
    let claim_count = timeline
        .claims
        .iter()
        .filter(|claim| claim.payer == policy.company_account)
        .count();
    let payroll_count = timeline
        .payroll
        .iter()
        .filter(|row| row.spec.payer == policy.company_account)
        .count();
    if claim_count > crate::town_runtime::MAX_EXIT_CLAIM_BINDINGS
        || payroll_count > crate::town_runtime::MAX_EXIT_PAYROLL_BINDINGS
        || payroll_count != policy.payroll.len()
    {
        return false;
    }
    for claim in timeline
        .claims
        .iter()
        .filter(|claim| claim.payer == policy.company_account)
    {
        let dynamic_tax_required = profile
            .equity_charters
            .as_slice()
            .iter()
            .any(|charter| charter.company == policy.company_account);
        let static_binding = policy
            .claims
            .as_slice()
            .iter()
            .find(|binding| binding.id == claim.id && binding.payee == claim.payee);
        let dynamic_tax = timeline
            .tax_source_for_claim(claim)
            .is_some_and(|source| tax_claim_source_matches(profile, state, claim, source));
        if static_binding.is_none() && !dynamic_tax
            || static_binding.is_some_and(|binding| {
                dynamic_tax_required
                    && binding.class == crate::town_economy::exit::ClaimClass::Tax
                    && !dynamic_tax
            })
        {
            return false;
        }
    }
    let known_claim_count = timeline
        .claims
        .iter()
        .filter(|claim| {
            claim.payer == policy.company_account
                && (policy
                    .claims
                    .as_slice()
                    .iter()
                    .any(|binding| binding.id == claim.id && binding.payee == claim.payee)
                    || timeline.tax_source_for_claim(claim).is_some_and(|source| {
                        tax_claim_source_matches(profile, state, claim, source)
                    }))
        })
        .count();
    if known_claim_count != claim_count {
        return false;
    }
    for row in timeline
        .payroll
        .iter()
        .filter(|row| row.spec.payer == policy.company_account)
    {
        if !policy
            .payroll
            .as_slice()
            .iter()
            .any(|binding| binding.id == row.spec.id && binding.payee == row.spec.payee)
        {
            return false;
        }
    }
    // Genesis bindings may not point at a row which never existed in this timeline.
    for binding in policy.claims.as_slice() {
        if !timeline.claims.iter().any(|claim| {
            claim.payer == policy.company_account
                && claim.id == binding.id
                && claim.payee == binding.payee
        }) {
            return false;
        }
    }
    for binding in policy.payroll.as_slice() {
        if !timeline.payroll.iter().any(|row| {
            row.spec.payer == policy.company_account
                && row.spec.id == binding.id
                && row.spec.payee == binding.payee
        }) {
            return false;
        }
    }
    true
}

fn tax_claim_source_matches(
    profile: &crate::town_runtime::MarketProfile,
    state: &crate::town_runtime::MarketState,
    claim: &crate::timeline::Claim,
    source: crate::timeline::TaxClaimSource,
) -> bool {
    let Some(rule) = profile
        .tax_rules
        .as_slice()
        .iter()
        .find(|rule| rule.company == source.company && rule.policy_id == source.policy_id)
    else {
        return false;
    };
    let Some(cursor) = state
        .tax_cursors
        .as_slice()
        .iter()
        .find(|cursor| cursor.company == rule.company && cursor.policy_id == rule.policy_id)
    else {
        return false;
    };
    let expected_tax = i128::from(source.taxable_basis.value())
        .checked_mul(i128::from(rule.rate_basis_points))
        .map(|value| value / 10_000)
        .and_then(|value| i64::try_from(value).ok());
    let expected_due = source
        .period
        .checked_add(1)
        .and_then(|day| day.checked_mul(crate::timeline::DAY));
    claim.id > 0
        && claim.payer == source.company
        && claim.payee == rule.authority_account
        && claim.principal == source.assessed
        && claim.principal > Money::ZERO
        && source.assessed.value() > 0
        && source.taxable_basis.value() > 0
        && source.kind == rule.kind
        && source.period > 0
        && source.period <= cursor.through_day
        && source.settlement_operation_id > 0
        && source.settlement_operation_id <= cursor.last_operation_id
        && expected_tax == Some(source.assessed.value())
        && expected_due == Some(claim.due)
}

fn tax_assessments_cover_exit(
    company: u32,
    profile: &crate::town_runtime::MarketProfile,
    state: &crate::town_runtime::MarketState,
    timeline: &crate::timeline::Timeline,
) -> bool {
    let dynamic_tax_required = profile
        .equity_charters
        .as_slice()
        .iter()
        .any(|charter| charter.company == company);
    if !dynamic_tax_required {
        // Legacy C exits use the explicit prototype completeness input. This is not a
        // measured or generated tax ledger; D-chartered companies require the real cursor.
        return profile
            .exit_policy
            .filter(|policy| policy.company_account == company)
            .is_some_and(|policy| policy.tax_assessment_complete)
            && timeline
                .claims
                .iter()
                .filter(|claim| claim.payer == company)
                .all(|claim| claim.source_ref == 0);
    }
    let Some(rule) = profile
        .tax_rules
        .as_slice()
        .iter()
        .find(|rule| rule.company == company)
    else {
        return false;
    };
    let Some(cursor) = state
        .tax_cursors
        .as_slice()
        .iter()
        .find(|cursor| cursor.company == company && cursor.policy_id == rule.policy_id)
    else {
        return false;
    };
    state.settled_day > 0
        && cursor.through_day == state.settled_day
        && cursor.last_operation_id == state.last_summary.operation_id
        && timeline.claims.iter().all(|claim| {
            if claim.payer != company {
                return true;
            }
            match timeline.source_for_claim(claim) {
                None => claim.source_ref == 0,
                Some(crate::timeline::ClaimSource::Loan { .. }) => true,
                Some(crate::timeline::ClaimSource::Rent { .. }) => false,
                Some(crate::timeline::ClaimSource::Tax { details }) => {
                    tax_claim_source_matches(profile, state, claim, details)
                }
            }
        })
}

fn dividend_has_unresolved_or_unmodeled_liability(
    company: u32,
    timeline: &crate::timeline::Timeline,
) -> bool {
    timeline.claims.iter().any(|claim| {
        // Only a fully settled Timeline::Lend receivable is a known company asset. Ordinary
        // paid Claims can represent compensation, fees or other costs and remain unmodeled.
        let settled_known_loan = timeline.loan_source_for_claim(claim).is_some()
            && claim.paid == claim.principal
            && claim.processed;
        (claim.payee == company && !settled_known_loan)
            || (claim.payer == company
                && (claim.paid != claim.principal
                    || !claim.processed
                    || timeline.source_for_claim(claim).is_none()
                    || timeline.rent_source_for_claim(claim).is_some()))
    }) || timeline.payroll.iter().any(|row| {
        row.spec.payee == company
            || (row.spec.payer == company
                && (row.accrued != row.paid
                    || row.billed != row.accrued
                    || row.working
                    || row.next_due.is_some()
                    || row.pending_cycle.is_some()))
    })
}

fn dividend_has_unmodeled_income_source(world: &World, company: u32) -> bool {
    // The prototype margin cursor models TownSettle goods sales, sold-lot basis, idle wages
    // and its own tax claim. Revenue through unrelated ledgers has no trusted margin source,
    // so block distributions when this account participates in one of them.
    world.exchange.as_ref().is_some_and(|exchange| {
        exchange
            .accounts
            .iter()
            .any(|binding| binding.account == company)
            || exchange.quotes.iter().any(|quote| {
                [quote.spec.seller, quote.spec.buyer, quote.spec.carrier].contains(&company)
            })
    }) || world
        .boot
        .as_ref()
        .is_some_and(|boot| [boot.operator, boot.contractor].contains(&company))
        || world
            .construction
            .as_ref()
            .is_some_and(|project| [project.owner, project.contractor].contains(&company))
        || world
            .economy
            .as_ref()
            .is_some_and(|economy| economy.accounts.contains(&company))
        || world.region.as_ref().is_some_and(|region| {
            region
                .obligations
                .iter()
                .flatten()
                .any(|obligation| obligation.payer == company || obligation.payee == company)
        })
}

fn exit_has_pending_payroll_cycle(company: u32, timeline: &crate::timeline::Timeline) -> bool {
    timeline
        .payroll
        .iter()
        .any(|row| row.spec.payer == company && row.pending_cycle.is_some())
}

fn exit_has_unsupported_receivables(company: u32, timeline: &crate::timeline::Timeline) -> bool {
    timeline.claims.iter().any(|claim| {
        timeline.rent_source_for_claim(claim).is_some()
            && (claim.payer == company || claim.payee == company)
            || claim.payee == company && claim.principal > claim.paid
    }) || timeline.payroll.iter().any(|row| {
        row.spec.payee == company
            && (row.accrued > row.paid || row.working || row.next_due.is_some())
    })
}

fn town_exit_dependencies(
    company: u32,
    exchange: Option<&crate::exchange::Exchange>,
    boot: Option<&crate::boot::Boot>,
    economy: Option<&crate::economy::Economy>,
    construction: Option<&crate::construction::Project>,
    leases: &LeaseRows,
) -> bool {
    leases.iter().flatten().any(|lease| lease.actor == company)
        || exchange.is_some_and(|exchange| {
            exchange.quotes.iter().any(|quote| {
                [quote.spec.seller, quote.spec.buyer, quote.spec.carrier].contains(&company)
            })
        })
        || boot.is_some_and(|boot| [boot.operator, boot.contractor].contains(&company))
        || economy.is_some_and(|economy| economy.accounts.contains(&company))
        || construction
            .is_some_and(|project| [project.owner, project.contractor].contains(&company))
}

fn town_exit_profile_dependency(
    profile: &crate::town_runtime::MarketProfile,
    company: u32,
) -> bool {
    profile
        .delivery
        .as_slice()
        .iter()
        .any(|quote| quote.freight_account == company || quote.utility_account == company)
        || profile.recipes.as_slice().iter().any(|recipe| {
            [
                recipe.utility_account,
                recipe.freight_account,
                recipe.disposal_account,
            ]
            .contains(&company)
        })
}

struct ExitSnapshotInputs<'a> {
    world: &'a World,
    policy: &'a crate::town_runtime::ExitPolicy,
    profile: &'a crate::town_runtime::MarketProfile,
    market_state: &'a crate::town_runtime::MarketState,
    timeline: &'a crate::timeline::Timeline,
    accounts: &'a DirtyAccounts,
    stocks: &'a DirtyStocks,
    operation_id: u64,
}

fn build_exit_snapshot(
    input: ExitSnapshotInputs<'_>,
) -> Result<crate::town_economy::exit::ExitSnapshot, TxError> {
    let ExitSnapshotInputs {
        world,
        policy,
        profile,
        market_state,
        timeline,
        accounts,
        stocks,
        operation_id,
    } = input;
    use crate::town_economy::exit::{
        ExitInventoryLot, ExitLiability, ExitPayroll, ExitSnapshot, ExitTrigger, LiabilitySource,
    };
    use crate::town_economy::{AccountSnapshot, InventorySnapshot};
    let company_handle = world
        .account_handle(policy.company_account)
        .map_err(|_| TxError::InvalidHandle)?;
    let company_account = accounts
        .iter()
        .find(|(handle, _)| *handle == company_handle)
        .map(|(_, account)| *account)
        .or_else(|| world.accounts.get(company_handle).ok().copied())
        .ok_or(TxError::InvalidHandle)?;
    let day = timeline.progress.last_complete / crate::timeline::DAY;
    let mut liabilities = Vec::new();
    let mut payroll_rows = Vec::new();
    for claim in timeline
        .claims
        .iter()
        .filter(|claim| claim.payer == policy.company_account)
    {
        let dynamic_tax_required = profile
            .equity_charters
            .as_slice()
            .iter()
            .any(|charter| charter.company == policy.company_account);
        let static_binding = policy
            .claims
            .as_slice()
            .iter()
            .find(|binding| binding.id == claim.id && binding.payee == claim.payee);
        let dynamic_tax = timeline
            .tax_source_for_claim(claim)
            .is_some_and(|source| tax_claim_source_matches(profile, market_state, claim, source));
        let class = if let Some(binding) = static_binding {
            if dynamic_tax_required
                && binding.class == crate::town_economy::exit::ClaimClass::Tax
                && !dynamic_tax
            {
                return Err(TxError::InvalidOperation);
            }
            binding.class
        } else if dynamic_tax {
            crate::town_economy::exit::ClaimClass::Tax
        } else {
            return Err(TxError::InvalidOperation);
        };
        let outstanding = claim
            .principal
            .checked_sub(claim.paid)
            .map_err(|_| TxError::InvalidOperation)?;
        if outstanding > Money::ZERO {
            liabilities.push(ExitLiability {
                source: LiabilitySource::TimelineClaim { id: claim.id },
                class,
                payee: claim.payee,
                due_at: claim
                    .due
                    .checked_add(crate::timeline::DAY - 1)
                    .ok_or(TxError::Overflow)?
                    / crate::timeline::DAY,
                outstanding,
            });
        }
    }
    for payroll in timeline
        .payroll
        .iter()
        .filter(|row| row.spec.payer == policy.company_account)
    {
        if !policy
            .payroll
            .as_slice()
            .iter()
            .any(|binding| binding.id == payroll.spec.id && binding.payee == payroll.spec.payee)
        {
            return Err(TxError::InvalidOperation);
        }
        payroll_rows.push(ExitPayroll {
            id: payroll.spec.id,
            active: payroll.working || payroll.next_due.is_some(),
        });
        let outstanding = payroll
            .accrued
            .checked_sub(payroll.paid)
            .map_err(|_| TxError::InvalidOperation)?;
        if outstanding > Money::ZERO {
            liabilities.push(ExitLiability {
                source: LiabilitySource::PayrollArrears {
                    id: payroll.spec.id,
                },
                class: crate::town_economy::exit::ClaimClass::Wage,
                payee: payroll.spec.payee,
                due_at: day,
                outstanding,
            });
        }
    }
    liabilities.sort_by_key(|row| {
        let class_index = policy
            .priority_policy
            .order
            .iter()
            .position(|class| *class == row.class)
            .unwrap_or(usize::MAX);
        let (source, id) = match row.source {
            LiabilitySource::TimelineClaim { id } => (0_u8, id),
            LiabilitySource::PayrollArrears { id } => (1_u8, u64::from(id)),
        };
        (class_index, source, id)
    });
    payroll_rows.sort_by_key(|row| row.id);
    let mut inventory = Vec::new();
    for (stock_id, handle) in world.stock_ids.iter() {
        let original = world
            .stocks
            .get(*handle)
            .map_err(|_| TxError::InvalidHandle)?;
        let owner = world
            .accounts
            .get(original.owner)
            .map_err(|_| TxError::InvalidHandle)?;
        if owner.id != policy.company_account {
            continue;
        }
        let stock = stocks
            .iter()
            .find(|(staged, _)| *staged == *handle)
            .map_or(*original, |(_, staged)| *staged);
        if stock.quantity == Quantity::ZERO {
            continue;
        }
        let sale_ready = profile
            .shops
            .as_slice()
            .iter()
            .any(|shop| shop.account == policy.company_account && shop.stock_id == *stock_id);
        inventory.push(ExitInventoryLot {
            inventory: InventorySnapshot {
                stock_id: *stock_id,
                owner: policy.company_account,
                good: crate::town_runtime::commodity_good(stock.commodity),
                quantity: stock.quantity,
                spendable_basis: stock.basis,
                origin_quantity: stock.origin_quantity,
                origin_basis: stock.origin_basis,
                offset: stock.offset,
            },
            sale_ready,
        });
    }
    let register = market_state
        .equity_registers
        .as_slice()
        .iter()
        .find(|register| register.company == policy.company_account);
    let charter = profile
        .equity_charters
        .as_slice()
        .iter()
        .find(|charter| charter.company == policy.company_account);
    let shareholders_complete = match (charter, register) {
        (Some(charter), Some(register)) => {
            register.validate().is_ok()
                && register.policy == charter.policy
                && register
                    .holders()
                    .iter()
                    .all(|holder| charter.binding_for_party(holder.party) == Some(holder.account))
        }
        _ => false,
    };
    let mut shareholders: Vec<_> = register
        .filter(|_| shareholders_complete)
        .map(|register| {
            register
                .holders()
                .iter()
                .map(|holder| crate::town_economy::exit::Shareholder {
                    account: holder.account,
                    shares: holder.shares,
                })
                .collect()
        })
        .unwrap_or_default();
    shareholders.sort_by_key(|holder| holder.account);
    let snapshot = ExitSnapshot {
        schema: "mycity-business-exit-v0.1".to_owned(),
        world: world.scope.world(),
        branch: world.scope.branch(),
        revision: world.revision,
        day,
        operation_id,
        company: policy.company_account,
        company_account: AccountSnapshot {
            account: company_account.id,
            cash: company_account.cash,
            protected: company_account.protected,
        },
        trigger: ExitTrigger::Voluntary,
        priority_policy: Some(policy.priority_policy),
        legal_clearance: policy.legal_clearance,
        claims_complete: policy.claims_complete
            && exit_timeline_bindings_match(policy, timeline, profile, market_state),
        // This is derived from per-period actual-settlement cursors and source-tagged Timeline
        // claims. The old genesis boolean is retained only for wire compatibility.
        tax_assessment_complete: tax_assessments_cover_exit(
            policy.company_account,
            profile,
            market_state,
            timeline,
        ),
        asset_register_complete: policy.asset_register_complete,
        employment_exit_terms_complete: policy.employment_exit_terms_complete,
        liabilities,
        payroll: payroll_rows,
        inventory,
        shareholders_complete,
        shareholders,
    };
    Ok(snapshot)
}

fn apply_equity_exit_distribution(
    plan: &mut crate::town_economy::exit::ExitPlan,
    register: &crate::town_economy::equity::EquityRegister,
) -> Result<(), TxError> {
    use crate::town_economy::exit::ExitAction;

    let mut distribution_index = None;
    let mut residual = Money::ZERO;
    for (index, action) in plan.actions.iter().enumerate() {
        if let ExitAction::DistributeResidual { amount, .. } = action {
            distribution_index.get_or_insert(index);
            residual = residual
                .checked_add(*amount)
                .map_err(|_| TxError::Overflow)?;
        }
    }
    let Some(index) = distribution_index else {
        return Ok(());
    };

    let allocations = crate::town_economy::equity::allocate_pro_rata(residual, register.holders())
        .map_err(|_| TxError::InvalidOperation)?;
    plan.actions
        .retain(|action| !matches!(action, ExitAction::DistributeResidual { .. }));
    for (offset, allocation) in allocations
        .into_iter()
        .filter(|allocation| allocation.amount > Money::ZERO)
        .enumerate()
    {
        plan.actions.insert(
            index + offset,
            ExitAction::DistributeResidual {
                shareholder: allocation.account,
                amount: allocation.amount,
            },
        );
    }
    Ok(())
}

fn post_exit_payment(
    world: &World,
    accounts: &mut DirtyAccounts,
    payer: u32,
    payee: u32,
    amount: Money,
) -> Result<(), TxError> {
    if amount <= Money::ZERO || payer == payee {
        return Err(TxError::InvalidOperation);
    }
    let payer_index = stage_account(
        world,
        accounts,
        world
            .account_ref(payer)
            .map_err(|_| TxError::InvalidHandle)?,
    )?;
    let payee_index = stage_account(
        world,
        accounts,
        world
            .account_ref(payee)
            .map_err(|_| TxError::InvalidHandle)?,
    )?;
    let available = accounts[payer_index]
        .1
        .cash
        .checked_sub(accounts[payer_index].1.protected)
        .map_err(|_| TxError::Protection)?;
    if available < amount {
        return Err(TxError::InsufficientFunds);
    }
    accounts[payer_index].1.cash = accounts[payer_index]
        .1
        .cash
        .checked_sub(amount)
        .map_err(|_| TxError::InsufficientFunds)?;
    accounts[payee_index].1.cash = accounts[payee_index]
        .1
        .cash
        .checked_add(amount)
        .map_err(|_| TxError::Overflow)?;
    Ok(())
}

fn construction_operation_requires_active_business(
    command: crate::construction::EngineCommand,
) -> bool {
    matches!(
        command,
        crate::construction::EngineCommand::File
            | crate::construction::EngineCommand::DecideGate { .. }
            | crate::construction::EngineCommand::Prepare
            | crate::construction::EngineCommand::Start
            | crate::construction::EngineCommand::Work { .. }
            | crate::construction::EngineCommand::Resume
            | crate::construction::EngineCommand::SubmitInspection
            | crate::construction::EngineCommand::AcceptInspection { .. }
            | crate::construction::EngineCommand::BeginDemolition { .. }
    )
}

fn safe_inbox_release(action: &Action, exchange: Option<&crate::exchange::Exchange>) -> bool {
    let Some(e) = exchange else { return false };
    match *action {
        Action::Inbox {
            command: crate::inbox::Command::Handoff { sequence, .. },
        } => e
            .inbox
            .all()
            .any(|f| f.sequence == sequence && f.received_at.is_some()),
        Action::Inbox {
            command: crate::inbox::Command::Archive { sequence },
        } => e
            .inbox
            .all()
            .any(|f| f.sequence == sequence && f.received_at.is_some() && f.fee == f.paid),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::assemble;

    #[test]
    fn road_withdrawal_requires_the_current_fully_settled_day_boundary() {
        let day = crate::timeline::DAY;
        let mut timeline = crate::timeline::Timeline::new(3 * day);
        assert!(validate_road_effect_withdrawal_boundary(Some(&timeline), Some(3)).is_ok());
        assert!(validate_road_effect_withdrawal_boundary(Some(&timeline), Some(2)).is_err());
        assert!(validate_road_effect_withdrawal_boundary(Some(&timeline), None).is_err());

        timeline.progress.target = Some(4 * day);
        assert!(validate_road_effect_withdrawal_boundary(Some(&timeline), Some(3)).is_err());
        timeline.progress.target = None;
        timeline.progress.phase = Some(crate::schedule::Phase::P0);
        assert!(validate_road_effect_withdrawal_boundary(Some(&timeline), Some(3)).is_err());

        timeline.progress.phase = None;
        timeline.progress.at += 1;
        assert!(validate_road_effect_withdrawal_boundary(Some(&timeline), Some(3)).is_err());
    }

    #[test]
    fn road_effect_restore_rejects_future_or_unsettled_publication() {
        let published_at =
            crate::units::GameTime::new(i64::try_from(2 * crate::timeline::DAY).unwrap()).unwrap();
        assert!(validate_road_effect_publication_clock(
            published_at,
            Some(crate::timeline::DAY),
            Some(1),
        )
        .is_err());
        assert!(validate_road_effect_publication_clock(
            published_at,
            Some(2 * crate::timeline::DAY),
            Some(1),
        )
        .is_err());
        assert!(validate_road_effect_publication_clock(
            published_at,
            Some(2 * crate::timeline::DAY),
            Some(2),
        )
        .is_ok());
    }

    fn engine() -> Engine {
        Engine::new(
            assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),
            vec![Resource::new(1, 10).unwrap(), Resource::new(2, 10).unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn diagnose_spatial_permission_layout_without_claiming_capacity_acceptance() {
        use crate::spatial_transport_permissions::SpatialPermissionState;

        assert!(size_of::<HotJournalRecord>() < size_of::<JournalRecord>());

        let (workspace_tables, workspace_air, workspace_controls) = workspace::capacity_inventory();
        let inventory = crate::model_capacity::inventory();
        eprintln!(
            "SPATIAL_LAYOUT {{\"engine\":{},\"prepared\":{},\"spatial_state\":{},\"spatial_state_owned_max\":{},\"lease_state\":{},\"receipt\":{},\"durable_journal_record\":{},\"hot_journal_record\":{},\"hot_journal_rows_total\":{},\"max_receipts\":{},\"max_actions\":{},\"workspace_tables\":{workspace_tables},\"workspace_air\":{workspace_air},\"workspace_controls\":{workspace_controls},\"workspace_spatial_rows\":{},\"workspace_spatial_control\":{},\"model_engine_bytes\":{},\"model_prepared_bytes\":{},\"model_retained_workspace_bytes\":{},\"engine_budget\":{ENGINE_BYTE_BUDGET}}}",
            size_of::<Engine>(),
            size_of::<Prepared>(),
            size_of::<SpatialPermissionState>(),
            SpatialPermissionState::max_owned_capacity_bytes(),
            size_of::<workspace::SpatialPermissionLease>(),
            size_of::<Receipt>(),
            size_of::<JournalRecord>(),
            size_of::<HotJournalRecord>(),
            MAX_RECEIPTS * size_of::<HotJournalRecord>(),
            MAX_RECEIPTS,
            MAX_ACTIONS,
            SpatialPermissionState::max_owned_capacity_bytes(),
            size_of::<std::sync::Mutex<Option<SpatialPermissionState>>>() + 256,
            inventory.engine_bytes,
            inventory.prepared_bytes,
            inventory.retained_preparation_workspace_bytes,
        );

        let world = assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        let result = Engine::new(
            world,
            vec![Resource::new(1, 10).unwrap(), Resource::new(2, 10).unwrap()],
        );
        eprintln!(
            "SPATIAL_LAYOUT_LEGACY_ENGINE_NEW {{\"result\":\"{}\"}}",
            if result.is_ok() { "ok" } else { "rejected" }
        );
    }

    fn construction_utility_engine(electricity_quantity: i64, water_quantity: i64) -> Engine {
        construction_utility_engine_with_contractor(electricity_quantity, water_quantity, 2)
    }

    fn construction_utility_engine_with_contractor(
        electricity_quantity: i64,
        water_quantity: i64,
        contractor: u32,
    ) -> Engine {
        construction_utility_engine_with_accounting(
            electricity_quantity,
            water_quantity,
            contractor,
            true,
        )
    }

    fn construction_utility_engine_with_accounting(
        electricity_quantity: i64,
        water_quantity: i64,
        contractor: u32,
        material_accounting: bool,
    ) -> Engine {
        use crate::construction::{
            ConstructionUtilityContract, ConstructionUtilitySupply, EngineCommand as C, Gate,
            GateCategory, GateDecision, Kind, Material, Project,
        };
        use crate::units::GameTime;
        let mut opening: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/m0-background-joint-opening.json"
        ))
        .unwrap();
        let electricity_source = opening["sources"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["id"] == 6)
            .unwrap();
        electricity_source["amount"] =
            serde_json::json!(if electricity_quantity == 0 { 0 } else { 100 });
        let electricity = opening["stocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["id"] == 2)
            .unwrap();
        electricity["quantity"] = serde_json::json!(electricity_quantity);
        electricity["basis"] = serde_json::json!(if electricity_quantity == 0 { 0 } else { 100 });
        let source = opening["sources"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["id"] == 7)
            .unwrap();
        source["amount"] = serde_json::json!(if water_quantity == 0 { 0 } else { 100 });
        let water = opening["stocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["id"] == 3)
            .unwrap();
        water["quantity"] = serde_json::json!(water_quantity);
        water["basis"] = serde_json::json!(if water_quantity == 0 { 0 } else { 100 });

        let start = GameTime::new(9 * 3_600).unwrap();
        let mut project = Project::new(
            7,
            Kind::Build,
            4,
            contractor,
            1,
            80,
            1,
            1,
            Money::new(100_000).unwrap(),
            Money::new(3_600).unwrap(),
            28_800,
            &[
                Gate {
                    id: 1,
                    category: GateCategory::OwnerConsent,
                    evidence_id: 0,
                    party: 4,
                    decided: GateDecision::Pending,
                },
                Gate {
                    id: 2,
                    category: GateCategory::Permit,
                    evidence_id: 0,
                    party: 1,
                    decided: GateDecision::Pending,
                },
            ],
            &[Material {
                stock_id: 7,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(400).unwrap(),
                basis_remainder: 0,
            }],
            start,
        )
        .unwrap();
        project.utility_contract = Some(ConstructionUtilityContract {
            electricity: Some(ConstructionUtilitySupply {
                operator: 3,
                stock_id: 2,
                rate_per_3600_worker_seconds: Quantity::new(1_000).unwrap(),
                tariff_per_quantity_unit: Money::new(20).unwrap(),
                quantity_unit: Quantity::new(1_000).unwrap(),
            }),
            water: Some(ConstructionUtilitySupply {
                operator: 3,
                stock_id: 3,
                rate_per_3600_worker_seconds: Quantity::new(1_000).unwrap(),
                tariff_per_quantity_unit: Money::new(10).unwrap(),
                quantity_unit: Quantity::new(1_000).unwrap(),
            }),
        });
        if material_accounting {
            project.enable_material_accounting().unwrap();
        }
        opening["construction"] = serde_json::to_value(project).unwrap();
        let mut engine = Engine::new(
            assemble(&serde_json::to_vec(&opening).unwrap()).unwrap(),
            vec![
                Resource::new(80, 4).unwrap(),
                Resource::new(101, 1).unwrap(),
                Resource::new(102, 1).unwrap(),
                Resource::new(103, 1).unwrap(),
                Resource::new(104, 1).unwrap(),
                Resource::new(201, 2_000).unwrap(),
                Resource::new(202, 2_000).unwrap(),
            ],
        )
        .unwrap();
        engine.timeline.activate();
        advance_timeline_to(&mut engine, 9 * 3_600);
        for (actor, command, id) in [
            (4, C::File, 9_000_001),
            (
                4,
                C::DecideGate {
                    gate_id: 1,
                    decision: GateDecision::Accepted,
                    evidence_id: 1,
                },
                9_000_002,
            ),
            (
                1,
                C::DecideGate {
                    gate_id: 2,
                    decision: GateDecision::Accepted,
                    evidence_id: 2,
                },
                9_000_003,
            ),
            (4, C::Prepare, 9_000_004),
            (4, C::Start, 9_000_005),
        ] {
            engine
                .execute(
                    Caller::Account(actor),
                    env(&engine, id, vec![Action::Construction { command }]),
                )
                .unwrap();
        }
        advance_timeline_to(&mut engine, 10 * 3_600);
        engine
    }

    fn construction_utility_empty_engine() -> Engine {
        let mut opening: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/m0-background-joint-opening.json"
        ))
        .unwrap();
        opening.as_object_mut().unwrap().remove("construction");
        Engine::new(
            assemble(&serde_json::to_vec(&opening).unwrap()).unwrap(),
            vec![
                Resource::new(80, 4).unwrap(),
                Resource::new(101, 1).unwrap(),
                Resource::new(102, 1).unwrap(),
                Resource::new(103, 1).unwrap(),
                Resource::new(104, 1).unwrap(),
                Resource::new(201, 2_000).unwrap(),
                Resource::new(202, 2_000).unwrap(),
            ],
        )
        .unwrap()
    }

    fn construction_compensation_engine() -> Engine {
        use crate::construction::{
            ConstructionCompensation, Gate, GateCategory, GateDecision, Kind, Material, Project,
        };
        let mut opening: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        let mut project = Project::new(
            7,
            Kind::Build,
            4,
            3,
            1,
            80,
            1,
            1,
            cash(100_000),
            cash(3_600),
            28_800,
            &[Gate {
                id: 1,
                category: GateCategory::Compensation,
                evidence_id: 0,
                party: 2,
                decided: GateDecision::Pending,
            }],
            &[Material {
                stock_id: 1,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: cash(400),
                basis_remainder: 0,
            }],
            crate::units::GameTime::ZERO,
        )
        .unwrap();
        project.compensation = Some(ConstructionCompensation::new(1, 2, cash(10_000)).unwrap());
        opening["construction"] = serde_json::to_value(project).unwrap();
        Engine::new(
            assemble(&serde_json::to_vec(&opening).unwrap()).unwrap(),
            vec![Resource::new(80, 4).unwrap()],
        )
        .unwrap()
    }

    fn construction_utility_work(e: &Engine, operation_id: u64) -> Envelope {
        Envelope {
            world: e.world.scope.world(),
            branch: e.world.scope.branch(),
            operation_id,
            expected_revision: e.world.revision,
            actions: vec![Action::Construction {
                command: crate::construction::EngineCommand::Work {
                    at: crate::units::GameTime::new(10 * 3_600).unwrap(),
                    elapsed_seconds: 3_600,
                    worker_seconds: 3_600,
                },
            }],
        }
    }

    #[test]
    fn construction_work_exports_and_pays_both_utilities_once_with_the_shared_boot_pool() {
        let mut engine = construction_utility_engine(100_000, 100_000);
        let request = construction_utility_work(&engine, 9_000_006);
        engine.execute(Caller::Account(2), request.clone()).unwrap();
        let after = engine.world.view();
        let project = after.construction.unwrap();
        assert_eq!(project.utility_worker_seconds, 3_600);
        assert_eq!(project.utility_consumed, [Quantity::new(1_000).unwrap(); 2]);
        assert_eq!(
            project.utility_charged,
            [Money::new(20).unwrap(), Money::new(10).unwrap()]
        );
        assert_eq!(project.spent, Money::new(3_680).unwrap());
        let material_accounting = project.material_accounting.unwrap();
        assert_eq!(material_accounting.row_count, 1);
        assert_eq!(material_accounting.rows[0].stock_id, 7);
        assert_eq!(
            material_accounting.rows[0].consumed,
            Quantity::new(125).unwrap()
        );
        assert_eq!(material_accounting.rows[0].basis, Money::new(50).unwrap());
        let boot = after.boot.unwrap();
        assert_eq!(boot.exports, [Quantity::new(1_000).unwrap(); 2]);
        assert_eq!(
            boot.local_construction_exported,
            [Quantity::new(1_000).unwrap(); 2]
        );
        assert_eq!(boot.local_construction_basis, [Money::new(1).unwrap(); 2]);
        assert_eq!(
            after.accounts.iter().find(|a| a.id == 3).unwrap().cash,
            Money::new(600_030).unwrap()
        );
        let cash_after = after
            .accounts
            .iter()
            .map(|a| (a.id, a.cash))
            .collect::<Vec<_>>();
        let stocks_after = serde_json::to_vec(&after.stocks).unwrap();
        engine.execute(Caller::Account(2), request).unwrap();
        let replayed = engine.world.view();
        assert_eq!(
            replayed
                .accounts
                .iter()
                .map(|a| (a.id, a.cash))
                .collect::<Vec<_>>(),
            cash_after
        );
        assert_eq!(serde_json::to_vec(&replayed.stocks).unwrap(), stocks_after);
        assert_eq!(replayed.construction.unwrap().utility_worker_seconds, 3_600);
    }

    #[test]
    fn construction_legacy_unaccounted_exchange_material_sink_remains_rejected() {
        let mut engine = construction_utility_engine_with_accounting(100_000, 100_000, 2, false);
        let before = engine.export_checkpoint().unwrap();
        let request = construction_utility_work(&engine, 9_200_001);
        let first = engine.execute(Caller::Account(2), request.clone());
        assert_eq!(
            first,
            Err(TxError::Exchange(crate::exchange::Error::Invariant))
        );
        assert_eq!(engine.execute(Caller::Account(2), request), first);
        let old = Engine::restore_checkpoint(&before).unwrap();
        let after = engine.world.view();
        let prior = old.world.view();
        assert_eq!(after.accounts, prior.accounts);
        assert_eq!(after.stocks, prior.stocks);
        assert_eq!(after.boot, prior.boot);
        assert_eq!(after.construction, prior.construction);
    }

    #[test]
    fn construction_utility_shortage_and_protected_combined_cost_leave_no_partial_export() {
        let mut no_water = construction_utility_engine(100_000, 0);
        let before = no_water.world.view();
        assert!(no_water
            .execute(
                Caller::Account(2),
                construction_utility_work(&no_water, 9_100_001),
            )
            .is_err());
        let after = no_water.world.view();
        assert_eq!(after.stocks, before.stocks);
        assert_eq!(after.accounts, before.accounts);
        assert_eq!(after.boot, before.boot);
        assert_eq!(after.construction, before.construction);

        let mut no_electricity = construction_utility_engine(0, 100_000);
        let before = no_electricity.world.view();
        assert!(no_electricity
            .execute(
                Caller::Account(2),
                construction_utility_work(&no_electricity, 9_100_003),
            )
            .is_err());
        let after = no_electricity.world.view();
        assert_eq!(after.stocks, before.stocks);
        assert_eq!(after.accounts, before.accounts);
        assert_eq!(after.boot, before.boot);
        assert_eq!(after.construction, before.construction);

        let mut protected = construction_utility_engine(100_000, 100_000);
        let owner = protected.world.account_handle(4).unwrap();
        let owner_cash = protected.world.accounts.get(owner).unwrap().cash;
        let spendable = Money::new(3_629).unwrap(); // utilities 30 + labour 3600 is 3630.
        let mut owner_row = *protected.world.accounts.get(owner).unwrap();
        owner_row.protected = owner_cash.checked_sub(spendable).unwrap();
        protected.world.accounts.replace(owner, owner_row).unwrap();
        let before = protected.world.view();
        assert!(protected
            .execute(
                Caller::Account(2),
                construction_utility_work(&protected, 9_100_002),
            )
            .is_err());
        let after = protected.world.view();
        assert_eq!(after.stocks, before.stocks);
        assert_eq!(after.accounts, before.accounts);
        assert_eq!(after.boot, before.boot);
        assert_eq!(after.construction, before.construction);
    }

    #[test]
    fn construction_utility_supplier_can_also_be_contractor_and_receives_combined_payments() {
        let mut engine = construction_utility_engine_with_contractor(100_000, 100_000, 3);
        let owner_before = engine
            .world
            .view()
            .accounts
            .iter()
            .find(|row| row.id == 4)
            .unwrap()
            .cash;
        let supplier_before = engine
            .world
            .view()
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .cash;

        engine
            .execute(
                Caller::Account(3),
                construction_utility_work(&engine, 9_000_106),
            )
            .unwrap();

        let after = engine.world.view();
        let owner_after = after.accounts.iter().find(|row| row.id == 4).unwrap().cash;
        let supplier_after = after.accounts.iter().find(|row| row.id == 3).unwrap().cash;
        assert_eq!(
            owner_before.checked_sub(owner_after).unwrap(),
            Money::new(3_630).unwrap()
        );
        assert_eq!(
            supplier_after.checked_sub(supplier_before).unwrap(),
            Money::new(3_630).unwrap()
        );
        assert_eq!(
            after.construction.unwrap().spent,
            Money::new(3_680).unwrap()
        );
        assert_eq!(
            after.boot.unwrap().local_construction_exported,
            [Quantity::new(1_000).unwrap(); 2]
        );
    }

    #[test]
    fn construction_boot_checkpoint_rejects_export_mismatch_and_basis_without_quantity() {
        let empty = construction_utility_empty_engine();
        let empty_checkpoint = empty.export_checkpoint().unwrap();
        let mut export_without_project: serde_json::Value =
            serde_json::from_slice(&empty_checkpoint).unwrap();
        export_without_project["world"]["boot"]["local_construction_exported"] =
            serde_json::json!([1, 0]);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&export_without_project).unwrap())
                .is_err()
        );

        let untouched = construction_utility_engine(100_000, 100_000);
        let untouched_checkpoint = untouched.export_checkpoint().unwrap();
        let mut basis_without_quantity: serde_json::Value =
            serde_json::from_slice(&untouched_checkpoint).unwrap();
        basis_without_quantity["world"]["boot"]["local_construction_basis"] =
            serde_json::json!([1, 0]);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&basis_without_quantity).unwrap())
                .is_err()
        );

        let mut engine = construction_utility_engine(100_000, 100_000);
        let request = construction_utility_work(&engine, 9_000_107);
        engine.execute(Caller::Account(2), request).unwrap();
        let checkpoint = engine.export_checkpoint().unwrap();
        let baseline: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();

        let mut mismatched_export = baseline.clone();
        mismatched_export["world"]["boot"]["local_construction_exported"][0] =
            serde_json::json!(999);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&mismatched_export).unwrap()).is_err()
        );
    }

    #[test]
    fn compensation_recipient_overflow_rolls_back_staged_owner_debit_and_gate() {
        let mut engine = construction_compensation_engine();
        engine
            .execute(
                Caller::Account(4),
                env(
                    &engine,
                    9_300_001,
                    vec![Action::Construction {
                        command: crate::construction::EngineCommand::File,
                    }],
                ),
            )
            .unwrap();
        let recipient = engine.world.account_handle(2).unwrap();
        let mut row = *engine.world.accounts.get(recipient).unwrap();
        row.cash = Money::new(i64::MAX - 5).unwrap();
        engine.world.accounts.replace(recipient, row).unwrap();
        let before = engine.world.view();
        let request = env(
            &engine,
            9_300_002,
            vec![Action::Construction {
                command: crate::construction::EngineCommand::DecideGate {
                    gate_id: 1,
                    decision: crate::construction::GateDecision::Accepted,
                    evidence_id: 1,
                },
            }],
        );
        assert_eq!(
            engine.execute(Caller::Account(2), request),
            Err(TxError::Overflow)
        );
        let after = engine.world.view();
        assert_eq!(after.accounts, before.accounts);
        assert_eq!(after.construction, before.construction);
    }
    fn cash(v: i64) -> Money {
        Money::new(v).unwrap()
    }
    fn account(e: &Engine, id: u32) -> RawHandle {
        e.world.account_handle(id).unwrap().raw()
    }
    fn env(e: &Engine, id: u64, actions: Vec<Action>) -> Envelope {
        Envelope {
            world: e.world.scope.world(),
            branch: e.world.scope.branch(),
            operation_id: id,
            expected_revision: e.world.revision,
            actions,
        }
    }
    fn market_engine() -> Engine {
        market_engine_from(engine())
    }

    fn equity_market_engine() -> Engine {
        let mut opening: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        for account in opening["accounts"].as_array_mut().unwrap() {
            if account["id"] == 3 {
                account["cash"] = serde_json::json!(0);
            }
        }
        for source in opening["sources"].as_array_mut().unwrap() {
            if source["id"] == 3 {
                source["amount"] = serde_json::json!(0);
            }
        }
        let bytes = serde_json::to_vec(&opening).unwrap();
        let e = Engine::new(
            assemble(&bytes).unwrap(),
            vec![Resource::new(1, 10).unwrap(), Resource::new(2, 10).unwrap()],
        )
        .unwrap();
        let mut e = market_engine_from(e);
        let progress = &mut e.timeline.as_mut().unwrap().progress;
        progress.last_complete = 0;
        progress.at = 0;
        e
    }

    fn market_engine_from(mut e: Engine) -> Engine {
        use crate::{
            town_economy::{DeliveryQuote, HouseholdGroup, ShopSnapshot},
            town_runtime::{EmployerSpec, FixedRows, MarketProfile, TownMarket},
        };
        let mut households = FixedRows::<HouseholdGroup, 64>::new();
        households
            .push(HouseholdGroup {
                id: 1,
                account: 2,
                households: 1,
                need_good: 0,
                need_per_household: Quantity::new(100).unwrap(),
                reference_price_per_q: cash(10),
            })
            .unwrap();
        let mut employers = FixedRows::<EmployerSpec, 16>::new();
        employers
            .push(EmployerSpec {
                account: 3,
                worker_limit: 1,
            })
            .unwrap();
        let mut shops = FixedRows::<ShopSnapshot, 128>::new();
        shops
            .push(ShopSnapshot {
                shop_id: 1,
                account: 4,
                stock_id: 1,
                good: 0,
                shelf_price_per_q: cash(10),
            })
            .unwrap();
        let recipes = FixedRows::new();
        let mut delivery = FixedRows::<DeliveryQuote, 512>::new();
        delivery
            .push(DeliveryQuote {
                quote_id: 1,
                shop: 1,
                household_group: 1,
                good: 0,
                reachable: true,
                route_source: 1,
                freight_account: 1,
                utility_account: 1,
                available_capacity: Quantity::new(100).unwrap(),
                freight_per_q: Money::ZERO,
                utility_surcharge_per_q: Money::ZERO,
            })
            .unwrap();
        e.world.town = Some(
            TownMarket::new(MarketProfile {
                schema_version: 1,
                id: 1,
                transport_allocation_sha256: [0; 32],
                utility_supply: None,
                exit_policy: None,
                equity_charters: FixedRows::new(),
                tax_rules: FixedRows::new(),
                households,
                employers,
                shops,
                recipes,
                delivery,
            })
            .unwrap(),
        );
        e.world.town.unwrap().validate_world(&e.world).unwrap();
        e.timeline.activate();
        e.timeline.as_mut().unwrap().progress.last_complete = crate::timeline::DAY;
        e.timeline.as_mut().unwrap().progress.at = crate::timeline::DAY;
        assert!(e.owned_capacity_bytes() <= ENGINE_BYTE_BUDGET);
        e
    }

    fn productive_market_engine() -> Engine {
        productive_market_engine_with(ProductiveMarketConfig::default())
    }

    #[derive(Clone, Copy)]
    struct ProductiveMarketConfig {
        raw_quantity: i64,
        household_need: i64,
        household_cash: i64,
        company_cash: i64,
        route_reachable: bool,
        route_capacity: i64,
        delivery_utility_surcharge: i64,
        production_utility_cost: i64,
    }

    impl Default for ProductiveMarketConfig {
        fn default() -> Self {
            Self {
                raw_quantity: 100_000,
                household_need: 10,
                household_cash: 200_000,
                company_cash: 600_000,
                route_reachable: true,
                route_capacity: 100,
                delivery_utility_surcharge: 1,
                production_utility_cost: 1,
            }
        }
    }

    // These variants deliberately alter fixed genesis inputs, then execute the same
    // authoritative Engine/Timeline settlement path used by the baseline fixture.
    fn productive_market_engine_with(config: ProductiveMarketConfig) -> Engine {
        let ProductiveMarketConfig {
            raw_quantity,
            household_need,
            household_cash,
            company_cash,
            route_reachable,
            route_capacity,
            delivery_utility_surcharge,
            production_utility_cost,
        } = config;
        let mut opening: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        for source in opening["sources"].as_array_mut().unwrap() {
            match source["id"].as_u64() {
                Some(2) => source["amount"] = household_cash.into(),
                Some(3) => source["amount"] = company_cash.into(),
                _ => {}
            }
        }
        for account in opening["accounts"].as_array_mut().unwrap() {
            match account["id"].as_u64() {
                Some(2) => account["cash"] = household_cash.into(),
                Some(3) => account["cash"] = company_cash.into(),
                _ => {}
            }
        }
        opening["sources"].as_array_mut().unwrap().extend([
            serde_json::json!({"id":6,"kind":"inventory_capital","amount":raw_quantity}),
            serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
        ]);
        opening["stocks"].as_array_mut().unwrap().extend([
            serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":raw_quantity,"basis":raw_quantity,"source":6}),
            serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
        ]);
        opening["town"] = serde_json::json!({
            "schema_version":1,
            "id":1,
            "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":household_need,"reference_price_per_q":1000}],
            "employers":[{"account":3,"worker_limit":1}],
            "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
            "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":1,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":100,"waste_per_output_q":0,"available_utility_per_day":1000,"utility_per_output_q":1,"utility_cost_per_output_q":production_utility_cost,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
            "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":route_reachable,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":route_capacity,"freight_per_q":1,"utility_surcharge_per_q":delivery_utility_surcharge}]
        });
        let mut e = Engine::new(
            assemble(&serde_json::to_vec(&opening).unwrap()).unwrap(),
            Vec::new(),
        )
        .unwrap();
        e.timeline.activate();
        e
    }

    fn settle_productive_day(e: &mut Engine) -> crate::town_runtime::MarketSummary {
        advance_productive_day(e);
        issue(e, Action::TownSettle { profile_id: 1 })
            .town
            .expect("town settlement receipt")
    }

    fn advance_productive_day(e: &mut Engine) {
        issue(
            e,
            Action::Timeline {
                command: crate::timeline::Command::AddPayroll {
                    spec: crate::timeline::PayrollSpec {
                        id: 1,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: cash(100),
                        cycle: crate::timeline::Cycle::Daily,
                        ends_at: 7 * crate::timeline::DAY,
                    },
                },
            },
        );
        for _ in 0..100 {
            issue(
                e,
                Action::Timeline {
                    command: crate::timeline::Command::Advance {
                        target: crate::timeline::DAY,
                        limit: 8192,
                    },
                },
            );
            if e.timeline().unwrap().progress.target.is_none() {
                break;
            }
        }
    }

    fn issue(e: &mut Engine, action: Action) -> Receipt {
        issue_as(e, Caller::System, action)
    }

    fn issue_as(e: &mut Engine, caller: Caller, action: Action) -> Receipt {
        let view = e.world.view();
        let action_description = format!("{action:?}");
        e.execute(
            caller,
            Envelope {
                world: view.scope.world(),
                branch: view.scope.branch(),
                operation_id: view.revision + 100,
                expected_revision: view.revision,
                actions: vec![action],
            },
        )
        .unwrap_or_else(|error| panic!("action {action_description} failed: {error:?}"))
    }

    fn install_exit_policy(e: &mut Engine) {
        use crate::town_economy::exit::{
            ClaimClass, ExitPriorityPolicy, LegalClearance, WithinTierRule,
        };
        use crate::town_runtime::{ExitClaimBinding, ExitPayrollBinding, ExitPolicy, FixedRows};
        let mut classes = [
            ClaimClass::Tax,
            ClaimClass::Wage,
            ClaimClass::SecuredDebt,
            ClaimClass::UnsecuredDebt,
            ClaimClass::Other,
        ];
        classes.rotate_right(1);
        let mut claims = FixedRows::new();
        claims
            .push(ExitClaimBinding {
                id: 100,
                payee: 4,
                class: ClaimClass::UnsecuredDebt,
            })
            .unwrap();
        let mut payroll = FixedRows::new();
        payroll
            .push(ExitPayrollBinding { id: 1, payee: 2 })
            .unwrap();
        let policy = ExitPolicy {
            policy_id: 1,
            company_account: 3,
            requester: 4,
            executor: 1,
            legal_clearance: LegalClearance::Cleared,
            priority_policy: ExitPriorityPolicy {
                order: classes,
                within_tier: WithinTierRule::ProRataByOutstandingStableId,
            },
            claims_complete: true,
            tax_assessment_complete: true,
            asset_register_complete: true,
            employment_exit_terms_complete: true,
            claims,
            payroll,
        };
        e.world.town.as_mut().unwrap().profile.exit_policy = Some(policy);
        e.world.town.unwrap().validate_world(&e.world).unwrap();
    }

    #[test]
    fn town_day_is_one_atomic_engine_receipt_and_retry_does_not_repeat_sale() {
        let mut e = market_engine();
        let cmd = env(&e, 500, vec![Action::TownSettle { profile_id: 1 }]);
        let receipt = e.execute(Caller::System, cmd.clone()).unwrap();
        assert_eq!(
            receipt.town.unwrap().actual_sales,
            Quantity::new(100).unwrap()
        );
        let after = e.state_hash();
        let view = e.world.view();
        assert_eq!(view.stocks[0].quantity, Quantity::new(599_900).unwrap());
        assert_eq!(
            view.accounts.iter().find(|a| a.id == 2).unwrap().cash,
            cash(199_000)
        );
        assert_eq!(
            view.accounts.iter().find(|a| a.id == 4).unwrap().cash,
            cash(1_201_000)
        );
        assert_eq!(view.town.unwrap().state.settled_day, 1);
        assert_eq!(e.execute(Caller::System, cmd).unwrap(), receipt);
        assert_eq!(e.state_hash(), after);
        eprintln!("action_size_bytes={}", std::mem::size_of::<Action>());
        assert!(std::mem::size_of::<Action>() <= 128);
    }

    fn install_equity_charter(e: &mut Engine) {
        use crate::town_economy::equity::{
            EquityCharter, EquityInvestorBinding, EquityOpeningSourceBinding, EquityParty,
            EquityPolicy, EquityRegister, PartyKind,
        };
        let mut investors = crate::town_runtime::FixedRows::new();
        investors
            .push(EquityInvestorBinding {
                party: EquityParty {
                    kind: PartyKind::Person,
                    id: 7001,
                },
                account: 2,
                opening_shares: 0,
            })
            .unwrap();
        let policy = EquityPolicy::initial(77, 1000);
        let party = EquityParty {
            kind: PartyKind::Person,
            id: 7001,
        };
        let mut opening_sources = crate::town_runtime::FixedRows::new();
        opening_sources
            .push(EquityOpeningSourceBinding {
                source_id: 3,
                party,
            })
            .unwrap();
        let company_handle = e.world.account_handle(3).unwrap();
        let mut company_account = *e.world.accounts.get(company_handle).unwrap();
        company_account.opening_source_id = 3;
        e.world
            .accounts
            .replace(company_handle, company_account)
            .unwrap();
        let town = e.world.town.as_mut().unwrap();
        town.profile
            .equity_charters
            .push(EquityCharter {
                company: 3,
                policy,
                distribution_authority: party,
                investors,
                opening_sources,
            })
            .unwrap();
        town.state
            .equity_registers
            .push(EquityRegister::empty(3, policy).unwrap())
            .unwrap();
        e.world.town.unwrap().validate_world(&e.world).unwrap();
    }

    #[test]
    fn opening_endowment_tracks_exact_sources_and_is_not_recomputed_from_live_assets() {
        let mut e = equity_market_engine();
        install_equity_charter(&mut e);
        let town = e.world.town.unwrap();
        assert!(town.validate_initial_world(&e.world).is_ok());
        let register = town.state.equity_registers.as_slice()[0];
        assert_eq!(register.opening_issued_shares, 0);
        assert_eq!(register.opening_paid_in, Money::ZERO);

        let company = e.world.account_handle(3).unwrap();
        let mut company_account = *e.world.accounts.get(company).unwrap();
        company_account.cash = cash(250);
        e.world.accounts.replace(company, company_account).unwrap();
        assert!(town.validate_world(&e.world).is_ok());
        assert!(town.validate_initial_world(&e.world).is_err());

        e.world
            .town
            .as_mut()
            .unwrap()
            .profile
            .equity_charters
            .as_mut_slice()[0]
            .opening_sources
            .as_mut_slice()[0]
            .source_id = 4;
        assert!(e.world.town.unwrap().validate_world(&e.world).is_err());
    }

    fn install_test_tax_coverage(e: &mut Engine, retained_cents: i64) {
        use crate::town_runtime::{
            CompanyTaxRule, ExitClaimBinding, ExitPayrollBinding, ExitPolicy, FixedRows,
            TaxAssessmentCursor, TaxKind,
        };
        let town = e.world.town.as_mut().unwrap();
        town.profile
            .tax_rules
            .push(CompanyTaxRule {
                company: 3,
                policy_id: 77,
                kind: TaxKind::DailyRealizedMarketMargin,
                authority_account: 1,
                rate_basis_points: 0,
            })
            .unwrap();
        town.state
            .tax_cursors
            .push(TaxAssessmentCursor {
                company: 3,
                policy_id: 77,
                through_day: 1,
                last_operation_id: 500,
                last_claim_id: 0,
                cumulative_assessed: Money::ZERO,
                retained_earnings_cents: retained_cents,
            })
            .unwrap();
        let classes = [
            crate::town_economy::exit::ClaimClass::Tax,
            crate::town_economy::exit::ClaimClass::Wage,
            crate::town_economy::exit::ClaimClass::SecuredDebt,
            crate::town_economy::exit::ClaimClass::UnsecuredDebt,
            crate::town_economy::exit::ClaimClass::Other,
        ];
        town.profile.exit_policy = Some(ExitPolicy {
            policy_id: 78,
            company_account: 3,
            requester: 1,
            executor: 4,
            legal_clearance: crate::town_economy::exit::LegalClearance::Cleared,
            priority_policy: crate::town_economy::exit::ExitPriorityPolicy {
                order: classes,
                within_tier:
                    crate::town_economy::exit::WithinTierRule::ProRataByOutstandingStableId,
            },
            claims_complete: true,
            tax_assessment_complete: false,
            asset_register_complete: true,
            employment_exit_terms_complete: true,
            claims:
                FixedRows::<ExitClaimBinding, { crate::town_runtime::MAX_EXIT_CLAIM_BINDINGS }>::new(
                ),
            payroll: FixedRows::<
                ExitPayrollBinding,
                { crate::town_runtime::MAX_EXIT_PAYROLL_BINDINGS },
            >::new(),
        });
        town.state.settled_day = 1;
        town.state.last_summary.day = 1;
        town.state.last_summary.operation_id = 500;
        let timeline = e.timeline.as_mut().unwrap();
        timeline.progress.last_complete = crate::timeline::DAY;
        timeline.progress.at = crate::timeline::DAY;
        e.world.town.unwrap().validate_world(&e.world).unwrap();
    }

    #[test]
    fn retained_earnings_distribution_is_bound_pro_rata_and_cannot_exceed_net_profit() {
        let mut e = equity_market_engine();
        install_equity_charter(&mut e);
        e.execute(
            Caller::Account(2),
            env(
                &e,
                9_400_120,
                vec![Action::EquityContribute {
                    company: 3,
                    amount: cash(1_200),
                }],
            ),
        )
        .unwrap();
        install_test_tax_coverage(&mut e, 600);
        let before = e.state_hash();
        let action = Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: cash(500),
        };
        assert_eq!(
            e.execute(Caller::System, env(&e, 9_400_121, vec![action])),
            Err(TxError::Unauthorized)
        );
        assert_eq!(
            e.execute(Caller::Account(4), env(&e, 9_400_122, vec![action])),
            Err(TxError::Unauthorized)
        );
        assert_eq!(e.state_hash(), before);

        let receipt = e
            .execute(Caller::Account(2), env(&e, 9_400_123, vec![action]))
            .unwrap();
        assert_eq!(receipt.settled[0], Some(cash(500)));
        let view = e.world.view();
        assert_eq!(
            view.accounts
                .iter()
                .find(|account| account.id == 3)
                .unwrap()
                .cash,
            cash(700)
        );
        assert_eq!(
            view.accounts
                .iter()
                .find(|account| account.id == 2)
                .unwrap()
                .cash,
            cash(199_300)
        );
        let cursor = view.town.unwrap().state.tax_cursors.as_slice()[0];
        assert_eq!(cursor.retained_earnings_cents, 100);
        assert_eq!(
            view.town.unwrap().state.equity_registers.as_slice()[0]
                .cumulative_dividends_distributed,
            cash(500)
        );

        let next = Action::EquityDistributeRetainedEarnings {
            company: 3,
            requested: cash(500),
        };
        let second = e
            .execute(Caller::Account(2), env(&e, 9_400_124, vec![next]))
            .unwrap();
        assert_eq!(second.settled[0], Some(cash(100)));
        let cursor = e.world.view().town.unwrap().state.tax_cursors.as_slice()[0];
        assert_eq!(cursor.retained_earnings_cents, 0);
        assert_eq!(
            e.world
                .view()
                .town
                .unwrap()
                .state
                .equity_registers
                .as_slice()[0]
                .cumulative_dividends_distributed,
            cash(600)
        );
        assert!(e
            .execute(Caller::Account(2), env(&e, 9_400_125, vec![next]))
            .is_err());
    }

    #[test]
    fn equity_contribution_requires_charter_bound_caller_and_commits_cash_with_shares() {
        use crate::town_economy::equity::{EquityParty, PartyKind};
        let mut e = equity_market_engine();
        install_equity_charter(&mut e);
        let investor_cash_before = e
            .world
            .view()
            .accounts
            .iter()
            .find(|a| a.id == 2)
            .unwrap()
            .cash;
        let company_cash_before = e
            .world
            .view()
            .accounts
            .iter()
            .find(|a| a.id == 3)
            .unwrap()
            .cash;
        let request = env(
            &e,
            9_400_100,
            vec![Action::EquityContribute {
                company: 3,
                amount: cash(1_200),
            }],
        );
        let receipt = e.execute(Caller::Account(2), request.clone()).unwrap();
        let view = e.world.view();
        assert_eq!(
            view.accounts.iter().find(|a| a.id == 2).unwrap().cash,
            investor_cash_before.checked_sub(cash(1_200)).unwrap()
        );
        assert_eq!(
            view.accounts.iter().find(|a| a.id == 3).unwrap().cash,
            company_cash_before.checked_add(cash(1_200)).unwrap()
        );
        assert_eq!(receipt.settled[0], Some(cash(1_200)));
        let market = view.town.unwrap();
        let register = market.state.equity_registers.as_slice().first().unwrap();
        assert_eq!(register.issued_shares, 12);
        assert_eq!(
            register.holders(),
            &[crate::town_economy::equity::EquityHolder {
                party: EquityParty {
                    kind: PartyKind::Person,
                    id: 7001
                },
                account: 2,
                shares: 12,
                paid_in: cash(1_200),
            }]
        );
        assert_eq!(e.execute(Caller::Account(2), request).unwrap(), receipt);
        let restored = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
        assert_eq!(restored.state_hash(), e.state_hash());
    }

    #[test]
    fn equity_contribution_is_allowed_before_timeline_creation_but_not_after_time_advances() {
        let mut genesis = equity_market_engine();
        install_equity_charter(&mut genesis);
        genesis.timeline.deactivate();
        let investor_before = genesis.world.view().accounts[1].cash;
        let company_before = genesis.world.view().accounts[2].cash;
        let request = env(
            &genesis,
            9_400_130,
            vec![Action::EquityContribute {
                company: 3,
                amount: cash(1_200),
            }],
        );

        let receipt = genesis.execute(Caller::Account(2), request).unwrap();
        assert_eq!(receipt.settled[0], Some(cash(1_200)));
        assert!(!genesis.timeline.is_some());
        let accounts = genesis.world.view().accounts;
        assert_eq!(
            accounts[1].cash,
            investor_before.checked_sub(cash(1_200)).unwrap()
        );
        assert_eq!(
            accounts[2].cash,
            company_before.checked_add(cash(1_200)).unwrap()
        );
        assert_eq!(
            genesis
                .world
                .view()
                .town
                .unwrap()
                .state
                .equity_registers
                .as_slice()[0]
                .issued_shares,
            12
        );

        let mut started = equity_market_engine();
        install_equity_charter(&mut started);
        started.timeline.as_mut().unwrap().progress.at = 1;
        let before = started.state_hash();
        assert_eq!(
            started.execute(
                Caller::Account(2),
                env(
                    &started,
                    9_400_131,
                    vec![Action::EquityContribute {
                        company: 3,
                        amount: cash(1_200),
                    }],
                ),
            ),
            Err(TxError::InvalidOperation)
        );
        assert_eq!(started.state_hash(), before);
    }

    #[test]
    fn unchartered_lend_keeps_legacy_timeline_wire_without_sparse_provenance() {
        let mut e = engine();
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Enable,
            },
        );
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Lend {
                    id: 1,
                    lender: 2,
                    borrower: 3,
                    principal: cash(1_000),
                    due: crate::timeline::DAY,
                },
            },
        );

        let timeline = e.timeline().unwrap();
        assert_eq!(timeline.claims[0].source_ref, 0);
        assert!(timeline.claim_sources.is_empty());
        let encoded = serde_json::to_value(timeline).unwrap();
        assert!(encoded.get("claim_sources").is_none());
        assert!(encoded["claims"][0].get("source_ref").is_none());
    }

    #[test]
    fn chartered_company_lend_records_real_loan_provenance() {
        let mut e = equity_market_engine();
        install_equity_charter(&mut e);
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Lend {
                    id: 1,
                    lender: 2,
                    borrower: 3,
                    principal: cash(1_000),
                    due: 2 * crate::timeline::DAY,
                },
            },
        );

        let timeline = e.timeline().unwrap();
        let claim = timeline.claims.iter().find(|claim| claim.id == 1).unwrap();
        assert_eq!(
            timeline.loan_source_for_claim(claim),
            Some(crate::timeline::LoanClaimSource {
                lender: 2,
                borrower: 3,
                principal: cash(1_000),
            })
        );
        assert_eq!(timeline.claims[0].source_ref, 1);
        assert_eq!(timeline.claim_sources.len(), 1);
    }

    #[test]
    fn equity_contribution_rejects_system_unbound_fractional_and_exit_frozen_inputs() {
        let mut e = equity_market_engine();
        install_equity_charter(&mut e);
        let action = Action::EquityContribute {
            company: 3,
            amount: cash(100),
        };
        let before = e.state_hash();
        assert_eq!(
            e.execute(Caller::System, env(&e, 9_400_101, vec![action])),
            Err(TxError::Unauthorized)
        );
        assert_eq!(
            e.execute(Caller::Account(4), env(&e, 9_400_102, vec![action])),
            Err(TxError::Unauthorized)
        );
        assert_eq!(e.state_hash(), before);
        assert_eq!(
            e.execute(
                Caller::Account(2),
                env(
                    &e,
                    9_400_103,
                    vec![Action::EquityContribute {
                        company: 3,
                        amount: cash(99)
                    }]
                )
            ),
            Err(TxError::InvalidOperation)
        );
        assert_eq!(e.state_hash(), before);
        e.world.town.as_mut().unwrap().state.exit = Some(crate::town_runtime::ExitRuntime {
            policy_id: 1,
            company_account: 3,
            started_at: crate::timeline::DAY,
            phase: crate::town_runtime::ExitPhase::Liquidating,
        });
        let frozen_hash = e.state_hash();
        assert_eq!(
            e.execute(Caller::Account(2), env(&e, 9_400_104, vec![action])),
            Err(TxError::Unauthorized)
        );
        assert_eq!(e.state_hash(), frozen_hash);
    }

    #[test]
    fn town_durable_hook_failure_does_not_publish_cash_stock_or_day() {
        let mut e = market_engine();
        let cmd = env(&e, 501, vec![Action::TownSettle { profile_id: 1 }]);
        let before = e.state_hash();
        let failed = e.execute_recorded(Caller::System, cmd, |_, _| Err("disk"));
        assert_eq!(failed, Err("disk"));
        assert_eq!(e.state_hash(), before);
        let view = e.world.view();
        assert_eq!(view.stocks[0].quantity, Quantity::new(600_000).unwrap());
        assert_eq!(view.town.unwrap().state.settled_day, 0);
    }

    #[test]
    fn town_production_uses_real_timeline_wages_and_preserves_lot_basis() {
        let mut e = productive_market_engine();
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::AddPayroll {
                    spec: crate::timeline::PayrollSpec {
                        id: 1,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: cash(100),
                        cycle: crate::timeline::Cycle::Daily,
                        ends_at: 7 * crate::timeline::DAY,
                    },
                },
            },
        );
        for _ in 0..100 {
            issue(
                &mut e,
                Action::Timeline {
                    command: crate::timeline::Command::Advance {
                        target: crate::timeline::DAY,
                        limit: 8192,
                    },
                },
            );
            if e.timeline().unwrap().progress.target.is_none() {
                break;
            }
        }
        let before_company = e
            .world
            .view()
            .accounts
            .iter()
            .find(|a| a.id == 3)
            .unwrap()
            .cash;
        let receipt = issue(&mut e, Action::TownSettle { profile_id: 1 });
        let summary = receipt.town.unwrap();
        assert_eq!(summary.day, 1);
        assert_eq!(summary.produced, Quantity::new(100).unwrap());
        assert_eq!(summary.actual_sales, Quantity::new(10).unwrap());
        assert_eq!(summary.labor_basis, cash(800));
        let view = e.world.view();
        let raw = view.stocks.iter().find(|s| s.id == 2).unwrap();
        let product = view.stocks.iter().find(|s| s.id == 3).unwrap();
        assert_eq!(raw.quantity, Quantity::new(99_900).unwrap());
        assert_eq!(raw.basis, cash(99_900));
        assert_eq!(product.quantity, Quantity::new(90).unwrap());
        assert_eq!(product.basis, cash(900));
        assert_eq!(product.origin_quantity, Quantity::new(100).unwrap());
        assert_eq!(product.offset, Quantity::new(10).unwrap());
        assert_eq!(
            view.accounts.iter().find(|a| a.id == 3).unwrap().cash,
            before_company.checked_add(cash(900)).unwrap()
        );
        assert_eq!(
            view.town.unwrap().state.payroll.as_slice()[0].paid,
            cash(800)
        );
    }

    #[test]
    fn town_exit_sells_real_inventory_pays_bound_claim_once_and_closes_at_zero_cash() {
        let mut e = productive_market_engine_with(ProductiveMarketConfig {
            raw_quantity: 100,
            household_need: 100,
            ..ProductiveMarketConfig::default()
        });
        install_exit_policy(&mut e);
        advance_productive_day(&mut e);
        let day_one = issue(&mut e, Action::TownSettle { profile_id: 1 })
            .town
            .unwrap();
        assert_eq!(day_one.produced, Quantity::new(100).unwrap());
        assert_eq!(day_one.actual_sales, Quantity::new(100).unwrap());
        let company_cash = e
            .world
            .view()
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .cash;
        assert!(company_cash > Money::ZERO);
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Claims {
                    first_id: 100,
                    count: 1,
                    payer: 3,
                    payee: 4,
                    amount: company_cash,
                    due: 2 * crate::timeline::DAY,
                },
            },
        );
        issue_as(
            &mut e,
            Caller::Account(4),
            Action::BeginExit {
                company: 3,
                policy_id: 1,
            },
        );
        let payroll = &e.timeline().unwrap().payroll[0];
        assert!(!payroll.working);
        assert_eq!(payroll.next_due, None);
        assert_eq!(payroll.billed, payroll.accrued);
        let before_unauthorized = e.state_hash();
        let unauthorized = e.execute(
            Caller::System,
            env(&e, 900_100, vec![Action::SettleExit { company: 3 }]),
        );
        assert_eq!(unauthorized, Err(TxError::Unauthorized));
        assert_eq!(e.state_hash(), before_unauthorized);

        for _ in 0..100 {
            issue(
                &mut e,
                Action::Timeline {
                    command: crate::timeline::Command::Advance {
                        target: 2 * crate::timeline::DAY,
                        limit: 8192,
                    },
                },
            );
            if e.timeline().unwrap().progress.target.is_none() {
                break;
            }
        }
        let receipt = issue(&mut e, Action::TownSettle { profile_id: 1 });
        assert_eq!(receipt.town.unwrap().produced, Quantity::ZERO);
        assert_eq!(receipt.town.unwrap().actual_sales, Quantity::ZERO);
        let payroll = &e.timeline().unwrap().payroll[0];
        assert_eq!(payroll.work_seconds, 8 * 3600);
        let claim = &e.timeline().unwrap().claims[0];
        assert!(claim.processed);
        assert_eq!(claim.paid, Money::ZERO); // automatic payment cannot bypass exit ranking
        let settlement = env(&e, 900_101, vec![Action::SettleExit { company: 3 }]);
        let pre_settlement = e.state_hash();
        // The payee can receive proceeds, but a failed staged credit must roll back both sides.
        let payee_handle = e.world.account_handle(4).unwrap();
        let original_payee = *e.world.accounts.get(payee_handle).unwrap();
        let mut overflow_payee = original_payee;
        overflow_payee.cash = Money::new(i64::MAX).unwrap();
        e.world
            .accounts
            .replace(payee_handle, overflow_payee)
            .unwrap();
        let before_failed_settlement = e.state_hash();
        assert_eq!(
            e.execute(Caller::Account(1), settlement.clone()),
            Err(TxError::Overflow)
        );
        assert_eq!(e.state_hash(), before_failed_settlement);
        e.world
            .accounts
            .replace(payee_handle, original_payee)
            .unwrap();
        assert_eq!(e.state_hash(), pre_settlement);
        let replay_failed = e.execute(Caller::Account(1), settlement.clone());
        assert_eq!(replay_failed, Err(TxError::Overflow));
        assert_eq!(e.state_hash(), pre_settlement);
        assert_eq!(
            e.journal()
                .last()
                .map(|record| (record.operation_id, record.outcome)),
            Some((900_101, Err(TxError::Overflow))),
            "a rejected operation remains an immutable idempotent receipt"
        );
        let successful_settlement = env(&e, 900_111, settlement.actions);
        let paid = e
            .execute(Caller::Account(1), successful_settlement)
            .unwrap();
        assert_eq!(paid.revision, e.world.revision);
        assert_eq!(e.timeline().unwrap().claims[0].paid, company_cash);
        let after_payment = e.world.view();
        assert_eq!(
            after_payment
                .accounts
                .iter()
                .find(|row| row.id == 3)
                .unwrap()
                .cash,
            Money::ZERO
        );
        assert_eq!(
            after_payment
                .accounts
                .iter()
                .find(|row| row.id == 4)
                .unwrap()
                .cash,
            original_payee.cash.checked_add(company_cash).unwrap()
        );
        let checkpoint = e.export_checkpoint().unwrap();
        let restored = Engine::restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(restored.state_hash(), e.state_hash());
        let before_close = e.state_hash();
        let close_envelope = env(&e, 900_112, vec![Action::CloseExit { company: 3 }]);
        let close = e
            .execute(Caller::Account(1), close_envelope.clone())
            .unwrap();
        assert_eq!(close.revision, e.world.revision);
        assert_ne!(e.state_hash(), before_close);
        assert_eq!(
            e.world.town.unwrap().state.exit.unwrap().phase,
            crate::town_runtime::ExitPhase::Closed
        );
        let closed = e.state_hash();
        let replay = e.execute(Caller::Account(1), close_envelope);
        assert_eq!(replay, Ok(close));
        assert_eq!(e.state_hash(), closed);

        let inbound_stock = e.execute(
            Caller::Account(4),
            env(
                &e,
                900_109,
                vec![Action::MoveStock {
                    from: e.world.stock_ref(1).unwrap(),
                    to: e.world.stock_ref(2).unwrap(),
                    quantity: Quantity::new(1).unwrap(),
                }],
            ),
        );
        assert_eq!(inbound_stock, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), closed);

        let inbound_transfer = e.execute(
            Caller::Account(4),
            env(
                &e,
                900_103,
                vec![Action::Transfer {
                    from: e.world.account_ref(4).unwrap(),
                    to: e.world.account_ref(3).unwrap(),
                    amount: cash(1),
                }],
            ),
        );
        assert_eq!(inbound_transfer, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), closed);

        let inbound_claim = e.execute(
            Caller::System,
            env(
                &e,
                900_104,
                vec![Action::Timeline {
                    command: crate::timeline::Command::Claims {
                        first_id: 200,
                        count: 1,
                        payer: 4,
                        payee: 3,
                        amount: cash(1),
                        due: 3 * crate::timeline::DAY,
                    },
                }],
            ),
        );
        assert_eq!(inbound_claim, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), closed);

        let inbound_loan = e.execute(
            Caller::System,
            env(
                &e,
                900_106,
                vec![Action::Timeline {
                    command: crate::timeline::Command::Lend {
                        id: 300,
                        lender: 4,
                        borrower: 3,
                        principal: cash(1),
                        due: 3 * crate::timeline::DAY,
                    },
                }],
            ),
        );
        assert_eq!(inbound_loan, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), closed);

        let inbound_payroll = e.execute(
            Caller::System,
            env(
                &e,
                900_107,
                vec![Action::Timeline {
                    command: crate::timeline::Command::AddPayroll {
                        spec: crate::timeline::PayrollSpec {
                            id: 2,
                            payer: 4,
                            payee: 3,
                            home_city: 0,
                            hourly_cents: cash(100),
                            cycle: crate::timeline::Cycle::Daily,
                            ends_at: 4 * crate::timeline::DAY,
                        },
                    },
                }],
            ),
        );
        assert_eq!(
            inbound_payroll,
            Err(TxError::Timeline(crate::timeline::Error::Input))
        );
        assert_eq!(e.state_hash(), closed);
    }

    #[test]
    fn town_exit_blocks_unregistered_timeline_receivables_before_liquidation() {
        let mut e = productive_market_engine();
        install_exit_policy(&mut e);
        advance_productive_day(&mut e);
        issue(&mut e, Action::TownSettle { profile_id: 1 });
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Claims {
                    first_id: 100,
                    count: 1,
                    payer: 3,
                    payee: 4,
                    amount: cash(100),
                    due: 2 * crate::timeline::DAY,
                },
            },
        );
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Lend {
                    id: 500,
                    lender: 3,
                    borrower: 4,
                    principal: cash(1_000),
                    due: 2 * crate::timeline::DAY,
                },
            },
        );
        let before = e.state_hash();
        let result = e.execute(
            Caller::Account(4),
            env(
                &e,
                900_108,
                vec![Action::BeginExit {
                    company: 3,
                    policy_id: 1,
                }],
            ),
        );
        assert_eq!(result, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), before);
        assert!(e.world.town.unwrap().state.exit.is_none());
    }

    #[test]
    fn town_exit_rejects_company_claim_sets_larger_than_policy_capacity() {
        let mut e = productive_market_engine();
        install_exit_policy(&mut e);
        advance_productive_day(&mut e);
        issue(&mut e, Action::TownSettle { profile_id: 1 });
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Claims {
                    first_id: 200,
                    count: (crate::town_runtime::MAX_EXIT_CLAIM_BINDINGS + 1) as u32,
                    payer: 3,
                    payee: 4,
                    amount: cash(1),
                    due: 2 * crate::timeline::DAY,
                },
            },
        );
        let before = e.state_hash();
        let result = e.execute(
            Caller::Account(4),
            env(
                &e,
                900_109,
                vec![Action::BeginExit {
                    company: 3,
                    policy_id: 1,
                }],
            ),
        );
        assert_eq!(result, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), before);
        assert!(e.world.town.unwrap().state.exit.is_none());
    }

    #[test]
    fn town_exit_rejects_pending_payroll_cycle_amendment_before_stopping_work() {
        let mut e = productive_market_engine();
        install_exit_policy(&mut e);
        advance_productive_day(&mut e);
        issue(&mut e, Action::TownSettle { profile_id: 1 });
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::Claims {
                    first_id: 100,
                    count: 1,
                    payer: 3,
                    payee: 4,
                    amount: cash(100),
                    due: 2 * crate::timeline::DAY,
                },
            },
        );
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::ChangeCycle {
                    id: 1,
                    cycle: crate::timeline::Cycle::Weekly,
                },
            },
        );
        assert_eq!(
            e.timeline().unwrap().payroll[0].pending_cycle,
            Some(crate::timeline::Cycle::Weekly)
        );
        let before = e.state_hash();
        let result = e.execute(
            Caller::Account(4),
            env(
                &e,
                900_110,
                vec![Action::BeginExit {
                    company: 3,
                    policy_id: 1,
                }],
            ),
        );
        assert_eq!(result, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), before);
        assert!(e.world.town.unwrap().state.exit.is_none());
    }

    #[test]
    fn engine_market_scenarios_propagate_cash_material_route_price_and_need_changes() {
        // Low household cash: route and production exist, but the household cannot buy.
        // Payroll first reaches the household normally; a recorded transfer then models its
        // remaining cash being spent before the market-clearing action.
        let mut low_cash = productive_market_engine_with(ProductiveMarketConfig {
            household_cash: 0,
            ..ProductiveMarketConfig::default()
        });
        advance_productive_day(&mut low_cash);
        let household = account(&low_cash, 2);
        let supplier = account(&low_cash, 4);
        issue(
            &mut low_cash,
            Action::Transfer {
                from: household,
                to: supplier,
                amount: cash(800),
            },
        );
        assert_eq!(
            low_cash
                .world
                .view()
                .accounts
                .iter()
                .find(|row| row.id == 2)
                .unwrap()
                .cash,
            Money::ZERO
        );
        let low_cash_summary = issue(&mut low_cash, Action::TownSettle { profile_id: 1 })
            .town
            .expect("low-cash settlement receipt");
        assert_eq!(low_cash_summary.produced, Quantity::new(100).unwrap());
        assert_eq!(low_cash_summary.actual_sales, Quantity::ZERO);
        assert_eq!(low_cash_summary.reference_need, Quantity::new(10).unwrap());
        assert_eq!(
            low_cash
                .world
                .view()
                .stocks
                .iter()
                .find(|s| s.id == 3)
                .unwrap()
                .quantity,
            Quantity::new(100).unwrap()
        );

        // Missing raw material blocks production, which leaves no product to sell.
        let mut no_material = productive_market_engine_with(ProductiveMarketConfig {
            raw_quantity: 0,
            ..ProductiveMarketConfig::default()
        });
        let no_material_summary = settle_productive_day(&mut no_material);
        assert_eq!(no_material_summary.produced, Quantity::ZERO);
        assert_eq!(no_material_summary.actual_sales, Quantity::ZERO);
        assert_eq!(
            no_material_summary.reference_need,
            Quantity::new(10).unwrap()
        );
        assert_eq!(
            no_material
                .world
                .view()
                .stocks
                .iter()
                .find(|s| s.id == 2)
                .unwrap()
                .quantity,
            Quantity::ZERO
        );

        // Route closure is represented by a fixed test quote (until the real transport
        // adapter is joined); production occurs, but delivery and sale are blocked.
        let mut closed_route = productive_market_engine_with(ProductiveMarketConfig {
            route_reachable: false,
            route_capacity: 0,
            ..ProductiveMarketConfig::default()
        });
        let closed_route_summary = settle_productive_day(&mut closed_route);
        assert_eq!(closed_route_summary.produced, Quantity::new(100).unwrap());
        assert_eq!(closed_route_summary.actual_sales, Quantity::ZERO);
        assert_eq!(
            closed_route_summary.reference_need,
            Quantity::new(10).unwrap()
        );

        // A higher delivery utility surcharge reduces the endogenous price ceiling.
        // The quote is a test substitute, not yet the live utility/transport assignment.
        let mut expensive_delivery = productive_market_engine_with(ProductiveMarketConfig {
            delivery_utility_surcharge: 1_998,
            ..ProductiveMarketConfig::default()
        });
        let expensive_summary = settle_productive_day(&mut expensive_delivery);
        assert_eq!(expensive_summary.reference_need, Quantity::new(10).unwrap());
        assert_eq!(expensive_summary.market_demand, Quantity::new(4).unwrap());
        assert_eq!(expensive_summary.actual_sales, Quantity::new(4).unwrap());

        // A lower underlying need passes through the same Engine path and lowers sales.
        let mut lower_need = productive_market_engine_with(ProductiveMarketConfig {
            household_need: 4,
            ..ProductiveMarketConfig::default()
        });
        let lower_need_summary = settle_productive_day(&mut lower_need);
        assert_eq!(lower_need_summary.reference_need, Quantity::new(4).unwrap());
        assert_eq!(lower_need_summary.actual_sales, Quantity::new(4).unwrap());

        // Higher production utility price is paid by the producer in the same atomic
        // Engine receipt and reduces its cash result; this is a fixed test cost, not a
        // live utility-network assignment.
        let mut ordinary_energy = productive_market_engine_with(ProductiveMarketConfig::default());
        let ordinary_summary = settle_productive_day(&mut ordinary_energy);
        let ordinary_company_cash = ordinary_energy
            .world
            .view()
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .cash;
        let mut expensive_energy = productive_market_engine_with(ProductiveMarketConfig {
            production_utility_cost: 1_000,
            ..ProductiveMarketConfig::default()
        });
        let expensive_summary = settle_productive_day(&mut expensive_energy);
        let expensive_company_cash = expensive_energy
            .world
            .view()
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .cash;
        assert_eq!(expensive_summary.produced, ordinary_summary.produced);
        assert_eq!(
            expensive_summary.actual_sales,
            ordinary_summary.actual_sales
        );
        assert_eq!(
            ordinary_company_cash
                .checked_sub(expensive_company_cash)
                .unwrap(),
            cash(99_900)
        );
    }

    #[test]
    fn town_assigns_exact_partial_worker_seconds_and_reconciles_100_101_wages() {
        let mut opening: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        opening["sources"].as_array_mut().unwrap().extend([
            serde_json::json!({"id":6,"kind":"inventory_capital","amount":100000}),
            serde_json::json!({"id":7,"kind":"inventory_capital","amount":0}),
        ]);
        opening["stocks"].as_array_mut().unwrap().extend([
            serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":100000,"basis":100000,"source":6}),
            serde_json::json!({"id":3,"owner":3,"commodity":"product","quantity":0,"basis":0,"source":7}),
        ]);
        opening["town"] = serde_json::json!({
            "schema_version":1,
            "id":1,
            "households":[{"id":1,"account":2,"households":1,"need_good":1,"need_per_household":10,"reference_price_per_q":1000}],
            "employers":[{"account":3,"worker_limit":2}],
            "shops":[{"shop_id":1,"account":3,"stock_id":3,"good":1,"shelf_price_per_q":100}],
            "recipes":[{"recipe_id":1,"employer":3,"assigned_workers":2,"input_stock_id":2,"input_per_output":1,"output_good":1,"output_stock_id":3,"output_per_worker_day":480,"waste_per_output_q":0,"available_utility_per_day":1,"utility_per_output_q":1,"utility_cost_per_output_q":1,"freight_cost_per_input_q":0,"disposal_cost_per_input_q":0,"utility_account":1,"freight_account":1,"disposal_account":1}],
            "delivery":[{"quote_id":1,"shop":1,"household_group":1,"good":1,"reachable":true,"route_source":77,"freight_account":1,"utility_account":1,"available_capacity":100,"freight_per_q":1,"utility_surcharge_per_q":1}]
        });
        let mut e = Engine::new(
            assemble(&serde_json::to_vec(&opening).unwrap()).unwrap(),
            Vec::new(),
        )
        .unwrap();
        e.timeline.activate();
        for (id, hourly_cents) in [(1, 6_000), (2, 6_060)] {
            issue(
                &mut e,
                Action::Timeline {
                    command: crate::timeline::Command::AddPayroll {
                        spec: crate::timeline::PayrollSpec {
                            id,
                            payer: 3,
                            payee: 2,
                            home_city: 0,
                            hourly_cents: cash(hourly_cents),
                            cycle: crate::timeline::Cycle::Daily,
                            ends_at: 9 * 3_600 + 60,
                        },
                    },
                },
            );
        }
        for _ in 0..100 {
            issue(
                &mut e,
                Action::Timeline {
                    command: crate::timeline::Command::Advance {
                        target: crate::timeline::DAY,
                        limit: 8192,
                    },
                },
            );
            if e.timeline().unwrap().progress.target.is_none() {
                break;
            }
        }
        let summary = issue(&mut e, Action::TownSettle { profile_id: 1 })
            .town
            .unwrap();
        assert_eq!(summary.wages_accrued, cash(201));
        assert_eq!(summary.produced, Quantity::new(1).unwrap());
        assert_eq!(summary.labor_basis, cash(100));
        assert_eq!(summary.idle_labor_expense, cash(101));
        assert_eq!(
            summary
                .labor_basis
                .checked_add(summary.idle_labor_expense)
                .unwrap(),
            summary.wages_accrued
        );
        assert_eq!(
            e.world
                .view()
                .town
                .unwrap()
                .state
                .recipe_output_remainders
                .as_slice()[0]
                .numerator,
            0
        );
    }

    #[test]
    fn weekly_timeline_payroll_catchup_does_not_block_daily_town_settlement() {
        let mut e = productive_market_engine();
        issue(
            &mut e,
            Action::Timeline {
                command: crate::timeline::Command::AddPayroll {
                    spec: crate::timeline::PayrollSpec {
                        id: 1,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: cash(100),
                        cycle: crate::timeline::Cycle::Weekly,
                        ends_at: 8 * crate::timeline::DAY,
                    },
                },
            },
        );
        for day in 1..=7 {
            for _ in 0..100 {
                issue(
                    &mut e,
                    Action::Timeline {
                        command: crate::timeline::Command::Advance {
                            target: day * crate::timeline::DAY,
                            limit: 8192,
                        },
                    },
                );
                if e.timeline().unwrap().progress.target.is_none() {
                    break;
                }
            }
            let summary = issue(&mut e, Action::TownSettle { profile_id: 1 })
                .town
                .unwrap();
            assert_eq!(summary.day, day);
            assert_eq!(summary.wages_accrued, cash(800));
        }
        let town = e.world.view().town.unwrap();
        let payroll = &town.state.payroll.as_slice()[0];
        assert_eq!(payroll.accrued, cash(5_600));
        assert_eq!(payroll.paid, cash(5_600));
    }
    #[test]
    fn failed_late_action_leaves_no_partial_cash_or_capacity() {
        let mut e = engine();
        let hash = e.state_hash();
        let a = account(&e, 2);
        let b = account(&e, 3);
        let cmd = env(
            &e,
            1,
            vec![
                Action::Transfer {
                    from: a,
                    to: b,
                    amount: cash(100),
                },
                Action::ReserveCapacity {
                    resource: 1,
                    units: 11,
                },
            ],
        );
        assert_eq!(e.execute(Caller::System, cmd), Err(TxError::Capacity));
        assert_eq!(e.state_hash(), hash);
        assert_eq!(e.journal.len(), 1);
    }
    #[test]
    fn retry_is_exact_and_changed_payload_or_actor_cannot_reuse_receipt() {
        let mut e = engine();
        let cmd = env(
            &e,
            1,
            vec![Action::Transfer {
                from: account(&e, 2),
                to: account(&e, 3),
                amount: cash(100),
            }],
        );
        let r = e.execute(Caller::Account(2), cmd.clone()).unwrap();
        let hash = e.state_hash();
        assert_eq!(e.execute(Caller::Account(2), cmd.clone()), Ok(r));
        assert_eq!(e.state_hash(), hash);
        assert_eq!(
            e.execute(Caller::Account(3), cmd.clone()),
            Err(TxError::Unauthorized)
        );
        let mut different = cmd;
        different.expected_revision = 1;
        assert_eq!(
            e.execute(Caller::Account(2), different),
            Err(TxError::OperationConflict)
        );
    }
    #[test]
    fn protection_is_not_new_cash_and_payment_converts_once() {
        let mut e = engine();
        let a = account(&e, 2);
        let b = account(&e, 3);
        let cmd = env(
            &e,
            1,
            vec![Action::Protect {
                account: a,
                amount: cash(150000),
            }],
        );
        e.execute(Caller::Account(2), cmd).unwrap();
        let cmd = env(
            &e,
            2,
            vec![Action::Transfer {
                from: a,
                to: b,
                amount: cash(100000),
            }],
        );
        assert_eq!(
            e.execute(Caller::Account(2), cmd),
            Err(TxError::InsufficientFunds)
        );
        let cmd = env(
            &e,
            3,
            vec![Action::PayProtected {
                from: a,
                to: b,
                amount: cash(150000),
            }],
        );
        e.execute(Caller::System, cmd).unwrap();
        let a = e
            .world
            .accounts
            .get(e.world.account_handle(2).unwrap())
            .unwrap();
        assert_eq!(a.cash, cash(50000));
        assert_eq!(a.protected, Money::ZERO);
        assert_eq!(
            e.world
                .view()
                .accounts
                .iter()
                .map(|a| a.cash.value())
                .sum::<i64>(),
            3_000_000
        );
    }
    #[test]
    fn two_resource_request_does_not_hold_one_when_second_is_unavailable() {
        let mut e = engine();
        let cmd = env(
            &e,
            1,
            vec![Action::ReserveCapacity {
                resource: 1,
                units: 8,
            }],
        );
        let r = e.execute(Caller::Account(2), cmd).unwrap();
        let before = e.state_hash();
        let cmd = env(
            &e,
            2,
            vec![
                Action::ReserveCapacity {
                    resource: 2,
                    units: 8,
                },
                Action::ReserveCapacity {
                    resource: 1,
                    units: 8,
                },
            ],
        );
        assert_eq!(e.execute(Caller::Account(3), cmd), Err(TxError::Capacity));
        assert_eq!(e.state_hash(), before);
        let cmd = env(
            &e,
            3,
            vec![Action::OccupyCapacity {
                token: r.created_tokens[0].unwrap(),
            }],
        );
        e.execute(Caller::Account(2), cmd).unwrap();
        assert_eq!(e.resources[0].occupied, 8);
        assert_eq!(e.resources[0].reserved, 0);
        let cmd = env(
            &e,
            4,
            vec![Action::ReleaseCapacity {
                token: r.created_tokens[0].unwrap(),
            }],
        );
        assert_eq!(
            e.execute(Caller::Account(3), cmd),
            Err(TxError::Unauthorized)
        );
    }
    #[test]
    fn stale_revision_scope_and_foreign_handles_do_not_write() {
        let mut e = engine();
        let hash = e.state_hash();
        let a = account(&e, 2);
        let b = account(&e, 3);
        let mut cmd = env(
            &e,
            1,
            vec![Action::Transfer {
                from: a,
                to: b,
                amount: cash(1),
            }],
        );
        cmd.expected_revision = 1;
        assert_eq!(e.execute(Caller::System, cmd), Err(TxError::StaleRevision));
        let mut cmd = env(
            &e,
            2,
            vec![Action::Transfer {
                from: a,
                to: b,
                amount: cash(1),
            }],
        );
        cmd.branch = 2;
        assert_eq!(e.execute(Caller::System, cmd), Err(TxError::WrongScope));
        let cmd = env(
            &e,
            3,
            vec![Action::Protect {
                account: RawHandle { world: 2, ..a },
                amount: cash(1),
            }],
        );
        assert_eq!(e.execute(Caller::System, cmd), Err(TxError::InvalidHandle));
        assert_eq!(e.state_hash(), hash);
    }
    #[test]
    fn full_journal_backpressures_without_erasing_prior_receipts() {
        let mut e = engine();
        let first = env(
            &e,
            1,
            vec![Action::ReserveCapacity {
                resource: 99,
                units: 1,
            }],
        );
        assert_eq!(
            e.execute(Caller::System, first.clone()),
            Err(TxError::Resource)
        );
        for id in 2..=MAX_RECEIPTS as u64 {
            let cmd = env(
                &e,
                id,
                vec![Action::ReserveCapacity {
                    resource: 99,
                    units: 1,
                }],
            );
            assert_eq!(e.execute(Caller::System, cmd), Err(TxError::Resource));
        }
        let before = e.state_hash();
        let cmd = env(
            &e,
            2048,
            vec![Action::Protect {
                account: account(&e, 2),
                amount: cash(1),
            }],
        );
        assert_eq!(e.execute(Caller::System, cmd), Err(TxError::JournalFull));
        assert_eq!(e.state_hash(), before);
        assert_eq!(e.execute(Caller::System, first), Err(TxError::Resource));
    }

    #[test]
    fn stock_and_payment_commit_as_one_with_conserved_quantity_and_basis() {
        let mut v: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        v["sources"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":6,"kind":"inventory_capital","amount":0}));
        v["stocks"].as_array_mut().unwrap().push(serde_json::json!({"id":2,"owner":3,"commodity":"raw","quantity":0,"basis":0,"source":6}));
        let mut e =
            Engine::new(assemble(&serde_json::to_vec(&v).unwrap()).unwrap(), vec![]).unwrap();
        let from = e.world.stock_ref(1).unwrap();
        let to = e.world.stock_ref(2).unwrap();
        let transfer = Action::Transfer {
            from: account(&e, 3),
            to: account(&e, 4),
            amount: cash(4000),
        };
        let before = e.state_hash();
        let cmd = env(
            &e,
            1,
            vec![
                transfer,
                Action::MoveStock {
                    from,
                    to,
                    quantity: Quantity::new(600001).unwrap(),
                },
            ],
        );
        assert_eq!(e.execute(Caller::System, cmd), Err(TxError::Stock));
        assert_eq!(e.state_hash(), before);
        let cmd = env(
            &e,
            2,
            vec![
                transfer,
                Action::MoveStock {
                    from,
                    to,
                    quantity: Quantity::new(10000).unwrap(),
                },
            ],
        );
        e.execute(Caller::System, cmd).unwrap();
        let v = e.world.view();
        assert_eq!(
            v.stocks.iter().map(|s| s.quantity.value()).sum::<i64>(),
            600000
        );
        assert_eq!(
            v.stocks.iter().map(|s| s.basis.value()).sum::<i64>(),
            240000
        );
        assert_eq!(v.stocks[1].basis.value(), 4000);
        assert_eq!(
            v.accounts.iter().map(|a| a.cash.value()).sum::<i64>(),
            3_000_000
        );
    }

    #[test]
    fn capacity_lifecycle_releases_once_and_never_reuses_token() {
        let mut e = engine();
        let cmd = env(
            &e,
            1,
            vec![Action::ReserveCapacity {
                resource: 1,
                units: 10,
            }],
        );
        let first = e.execute(Caller::Account(2), cmd).unwrap().created_tokens[0].unwrap();
        let cmd = env(&e, 2, vec![Action::OccupyCapacity { token: first }]);
        e.execute(Caller::Account(2), cmd).unwrap();
        let cmd = env(&e, 3, vec![Action::ReleaseCapacity { token: first }]);
        e.execute(Caller::Account(2), cmd).unwrap();
        let hash = e.state_hash();
        let cmd = env(&e, 4, vec![Action::ReleaseCapacity { token: first }]);
        assert_eq!(e.execute(Caller::Account(2), cmd), Err(TxError::Lease));
        assert_eq!(e.state_hash(), hash);
        let cmd = env(
            &e,
            5,
            vec![Action::ReserveCapacity {
                resource: 1,
                units: 10,
            }],
        );
        let second = e.execute(Caller::Account(2), cmd).unwrap().created_tokens[0].unwrap();
        assert!(second > first);
    }

    #[test]
    fn account_cannot_unprotect_or_redirect_reserved_funds() {
        let mut e = engine();
        let a = account(&e, 2);
        let b = account(&e, 3);
        let cmd = env(
            &e,
            1,
            vec![Action::Protect {
                account: a,
                amount: cash(100),
            }],
        );
        e.execute(Caller::Account(2), cmd).unwrap();
        let hash = e.state_hash();
        for (id, action) in [
            (
                2,
                Action::ReleaseProtection {
                    account: a,
                    amount: cash(100),
                },
            ),
            (
                3,
                Action::PayProtected {
                    from: a,
                    to: b,
                    amount: cash(100),
                },
            ),
        ] {
            let cmd = env(&e, id, vec![action]);
            assert_eq!(
                e.execute(Caller::Account(2), cmd),
                Err(TxError::Unauthorized)
            );
            assert_eq!(e.state_hash(), hash);
        }
    }

    #[test]
    fn persistence_hook_failure_publishes_neither_model_changes_nor_receipt() {
        let mut e = engine();
        let before = e.state_hash();
        let cmd = env(
            &e,
            1,
            vec![Action::Transfer {
                from: account(&e, 2),
                to: account(&e, 3),
                amount: cash(100),
            }],
        );
        let result = e.execute_recorded(Caller::System, cmd, |_, record| {
            assert!(record.outcome.is_ok());
            Err("disk_error")
        });
        assert_eq!(result, Err("disk_error"));
        assert_eq!(e.state_hash(), before);
        assert!(e.journal().is_empty());
    }

    #[test]
    fn foreign_capacity_reference_cannot_resolve_same_number_in_another_world() {
        let mut a = engine();
        let cmd = env(
            &a,
            1,
            vec![Action::ReserveCapacity {
                resource: 1,
                units: 2,
            }],
        );
        let foreign = a.execute(Caller::Account(2), cmd).unwrap().created_tokens[0].unwrap();
        let mut v: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap();
        v["bootstrap"]["world"] = serde_json::json!(2);
        let mut b = Engine::new(
            assemble(&serde_json::to_vec(&v).unwrap()).unwrap(),
            vec![Resource::new(1, 10).unwrap()],
        )
        .unwrap();
        let cmd = env(
            &b,
            1,
            vec![Action::ReserveCapacity {
                resource: 1,
                units: 2,
            }],
        );
        let local = b.execute(Caller::Account(2), cmd).unwrap().created_tokens[0].unwrap();
        assert_eq!(foreign.number, local.number);
        let before = b.state_hash();
        let cmd = env(&b, 2, vec![Action::OccupyCapacity { token: foreign }]);
        assert_eq!(b.execute(Caller::Account(2), cmd), Err(TxError::WrongScope));
        assert_eq!(b.state_hash(), before);
    }

    #[test]
    fn lot_cost_is_invariant_under_splitting_and_internal_transfer() {
        let setup = || {
            let mut v: serde_json::Value =
                serde_json::from_slice(include_bytes!("../../../fixtures/m0-e0-opening.json"))
                    .unwrap();
            v["stocks"][0]["quantity"] = serde_json::json!(5);
            v["stocks"][0]["basis"] = serde_json::json!(2);
            v["sources"][4]["amount"] = serde_json::json!(2);
            for (source, id) in [(6, 2), (7, 3)] {
                v["sources"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"id":source,"kind":"inventory_capital","amount":0}));
                v["stocks"].as_array_mut().unwrap().push(serde_json::json!({"id":id,"owner":4,"commodity":"raw","quantity":0,"basis":0,"source":source}));
            }
            Engine::new(assemble(&serde_json::to_vec(&v).unwrap()).unwrap(), vec![]).unwrap()
        };
        let mut whole = setup();
        let from = whole.world.stock_ref(1).unwrap();
        let to = whole.world.stock_ref(2).unwrap();
        let cmd = env(
            &whole,
            1,
            vec![Action::MoveStock {
                from,
                to,
                quantity: Quantity::new(3).unwrap(),
            }],
        );
        whole.execute(Caller::System, cmd).unwrap();
        let mut split = setup();
        for id in 1..=3 {
            let cmd = env(
                &split,
                id,
                vec![Action::MoveStock {
                    from,
                    to,
                    quantity: Quantity::new(1).unwrap(),
                }],
            );
            split.execute(Caller::System, cmd).unwrap();
        }
        assert_eq!(whole.world.view().stocks, split.world.view().stocks);
        assert_eq!(whole.world.view().stocks[1].basis.value(), 1);
        let mut relayed = setup();
        let mid = relayed.world.stock_ref(3).unwrap();
        for id in 1..=3 {
            let cmd = env(
                &relayed,
                id * 2,
                vec![Action::MoveStock {
                    from,
                    to: mid,
                    quantity: Quantity::new(1).unwrap(),
                }],
            );
            relayed.execute(Caller::System, cmd).unwrap();
            let cmd = env(
                &relayed,
                id * 2 + 1,
                vec![Action::MoveStock {
                    from: mid,
                    to,
                    quantity: Quantity::new(1).unwrap(),
                }],
            );
            relayed.execute(Caller::System, cmd).unwrap();
        }
        assert_eq!(relayed.world.view().stocks[1].basis.value(), 1);
        assert_eq!(relayed.world.view().stocks[0].basis.value(), 1);
    }

    fn advance_timeline_to(e: &mut Engine, target: u64) {
        for _ in 0..100 {
            issue(
                e,
                Action::Timeline {
                    command: crate::timeline::Command::Advance {
                        target,
                        limit: 8192,
                    },
                },
            );
            if e.timeline().unwrap().progress.target.is_none() {
                break;
            }
        }
        assert_eq!(e.timeline().unwrap().progress.last_complete, target);
    }

    fn add_daily_payroll(e: &mut Engine, id: u32, payer: u32) {
        issue(
            e,
            Action::Timeline {
                command: crate::timeline::Command::AddPayroll {
                    spec: crate::timeline::PayrollSpec {
                        id,
                        payer,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: cash(100),
                        cycle: crate::timeline::Cycle::Daily,
                        ends_at: 7 * crate::timeline::DAY,
                    },
                },
            },
        );
    }

    #[test]
    fn town_suspend_stops_payroll_and_market_operations_but_preserves_liabilities() {
        let mut e = productive_market_engine();
        e.world
            .town
            .as_mut()
            .unwrap()
            .profile
            .employers
            .push(crate::town_runtime::EmployerSpec {
                account: 4,
                worker_limit: 1,
            })
            .unwrap();
        add_daily_payroll(&mut e, 1, 3);
        add_daily_payroll(&mut e, 2, 4);
        advance_timeline_to(&mut e, crate::timeline::DAY);
        let before_unsettled_stop = e.state_hash();
        let rejected_stop = e.execute(
            Caller::System,
            env(
                &e,
                e.world.revision + 450,
                vec![Action::TownSuspendOperations {
                    profile_id: 1,
                    company_account: 3,
                }],
            ),
        );
        assert_eq!(rejected_stop, Err(TxError::InvalidOperation));
        assert_eq!(e.state_hash(), before_unsettled_stop);
        let day1 = issue(&mut e, Action::TownSettle { profile_id: 1 })
            .town
            .unwrap();
        assert!(day1.actual_sales.value() > 0);
        assert!(day1.produced.value() > 0);

        let before_suspend = e.world.view();
        let business_cash = before_suspend
            .accounts
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .cash;
        let inventory = before_suspend
            .stocks
            .iter()
            .find(|row| row.id == 2)
            .unwrap()
            .quantity;
        let paid = e.timeline().unwrap().payroll[0].paid;
        issue(
            &mut e,
            Action::TownSuspendOperations {
                profile_id: 1,
                company_account: 3,
            },
        );
        let suspended = e.world.view();
        assert_eq!(
            suspended
                .accounts
                .iter()
                .find(|row| row.id == 3)
                .unwrap()
                .cash,
            business_cash
        );
        assert_eq!(
            suspended
                .stocks
                .iter()
                .find(|row| row.id == 2)
                .unwrap()
                .quantity,
            inventory
        );
        assert_eq!(e.timeline().unwrap().payroll[0].paid, paid);
        assert!(!e.timeline().unwrap().payroll[0].working);
        assert_eq!(
            suspended.town.unwrap().state.suspended_operators.as_slice()[0].account,
            3
        );

        let unchanged = e.state_hash();
        let rejected = e.execute(
            Caller::System,
            env(
                &e,
                e.world.revision + 500,
                vec![Action::Timeline {
                    command: crate::timeline::Command::AddPayroll {
                        spec: crate::timeline::PayrollSpec {
                            id: 3,
                            payer: 3,
                            payee: 2,
                            home_city: 0,
                            hourly_cents: cash(100),
                            cycle: crate::timeline::Cycle::Daily,
                            ends_at: 7 * crate::timeline::DAY,
                        },
                    },
                }],
            ),
        );
        assert!(matches!(
            rejected,
            Err(TxError::Timeline(crate::timeline::Error::Input))
        ));
        assert_eq!(e.state_hash(), unchanged);

        advance_timeline_to(&mut e, 2 * crate::timeline::DAY);
        let day2 = issue(&mut e, Action::TownSettle { profile_id: 1 })
            .town
            .unwrap();
        assert_eq!(day2.actual_sales, Quantity::ZERO);
        assert_eq!(day2.produced, Quantity::ZERO);
        assert!(
            day2.wages_accrued.value() > 0,
            "unrelated employer payroll continues"
        );
        assert_eq!(
            e.world
                .view()
                .accounts
                .iter()
                .find(|row| row.id == 3)
                .unwrap()
                .cash,
            business_cash
        );
    }

    #[test]
    fn town_suspend_persistence_hook_failure_leaves_all_state_unchanged() {
        let mut e = productive_market_engine();
        add_daily_payroll(&mut e, 1, 3);
        advance_timeline_to(&mut e, crate::timeline::DAY);
        issue(&mut e, Action::TownSettle { profile_id: 1 });
        let before = e.state_hash();
        let command = env(
            &e,
            e.world.revision + 900,
            vec![Action::TownSuspendOperations {
                profile_id: 1,
                company_account: 3,
            }],
        );
        assert_eq!(
            e.execute_recorded(Caller::System, command, |_, _| Err(
                "simulated write failure"
            )),
            Err("simulated write failure")
        );
        assert_eq!(e.state_hash(), before);
    }

    #[test]
    fn checkpoint_rejects_future_suspension_and_resumed_or_unbilled_payroll() {
        let mut e = productive_market_engine();
        add_daily_payroll(&mut e, 1, 3);
        advance_timeline_to(&mut e, crate::timeline::DAY);
        issue(&mut e, Action::TownSettle { profile_id: 1 });
        issue(
            &mut e,
            Action::TownSuspendOperations {
                profile_id: 1,
                company_account: 3,
            },
        );
        let checkpoint = e.export_checkpoint().unwrap();
        let baseline: serde_json::Value = serde_json::from_slice(&checkpoint).unwrap();

        let mut future_marker = baseline.clone();
        future_marker["world"]["town"]["state"]["settled_day"] =
            serde_json::json!(2 * crate::timeline::DAY);
        future_marker["world"]["town"]["state"]["last_summary"]["day"] =
            serde_json::json!(2 * crate::timeline::DAY);
        future_marker["world"]["town"]["state"]["suspended_operators"][0]["suspended_at"] =
            serde_json::json!(2 * crate::timeline::DAY);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&future_marker).unwrap()).is_err(),
            "a suspension cannot be later than Timeline.last_complete"
        );

        let mut resumed_payroll = baseline.clone();
        resumed_payroll["timeline"]["payroll"][0]["working"] = serde_json::json!(true);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&resumed_payroll).unwrap()).is_err()
        );

        let mut pending_payroll = baseline.clone();
        pending_payroll["timeline"]["payroll"][0]["next_due"] =
            serde_json::json!(2 * crate::timeline::DAY);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&pending_payroll).unwrap()).is_err()
        );

        let mut unbilled_payroll = baseline;
        unbilled_payroll["timeline"]["payroll"][0]["billed"] = serde_json::json!(0);
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&unbilled_payroll).unwrap()).is_err()
        );
    }

    #[test]
    fn suspended_construction_guard_blocks_new_build_and_demolition_entry_only() {
        use crate::construction::EngineCommand as C;
        for command in [
            C::File,
            C::DecideGate {
                gate_id: 1,
                decision: crate::construction::GateDecision::Accepted,
                evidence_id: 1,
            },
            C::Prepare,
            C::Start,
            C::Work {
                at: crate::units::GameTime::ZERO,
                elapsed_seconds: 1,
                worker_seconds: 1,
            },
            C::Resume,
            C::SubmitInspection,
            C::AcceptInspection {
                inspector: 1,
                accepted: true,
            },
            C::BeginDemolition { waste_receiver: 1 },
        ] {
            assert!(construction_operation_requires_active_business(command));
        }
        for cleanup in [C::Pause, C::Cancel, C::AcceptWaste, C::Close] {
            assert!(!construction_operation_requires_active_business(cleanup));
        }
    }
}
