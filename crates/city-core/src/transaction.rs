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
pub const MAX_RESOURCES: usize = 8;
pub const MAX_LEASES: usize = 64;
pub const ENGINE_BYTE_BUDGET: usize = 4 * 1024 * 1024;

type DirtyAccounts =
    crate::model_rows::Rows<(Handle<AccountDomain>, Account), { crate::assembly::MAX_ACCOUNTS }>;
type DirtyStocks =
    crate::model_rows::Rows<(Handle<InventoryDomain>, Stock), { crate::assembly::MAX_STOCKS }>;
type ResourceRows = crate::model_rows::Rows<Resource, MAX_RESOURCES>;
type LeaseRows = crate::model_rows::Rows<Option<Lease>, MAX_LEASES>;
type JournalRows = crate::model_rows::Rows<JournalRecord, MAX_RECEIPTS>;

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
    journal: JournalRows,
    timeline: crate::model_rows::Resident<crate::timeline::Timeline>,
    background: crate::model_rows::Resident<crate::background::Background>,
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
    town: Option<crate::town_runtime::MarketState>,
    accounts: workspace::AccountLease,
    stocks: workspace::StockLease,
    resources: workspace::ResourceLease,
    leases: workspace::CapacityLease,
    next_token: u64,
    receipt: Receipt,
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
        let mut journal = JournalRows::new().map_err(|_| TxError::WorkingLimit)?;
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
            preparation_workspace: Some(workspace::reserve()?),
        };
        if engine.owned_capacity_bytes() > ENGINE_BYTE_BUDGET {
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
            journal: &'a [JournalRecord],
            timeline: &'a crate::model_rows::Resident<crate::timeline::Timeline>,
            background: &'a crate::model_rows::Resident<crate::background::Background>,
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
        }
        View {
            schema: "mycity-business-comparison-v2",
            world: self.world.business_world(),
            resources: &self.resources,
            leases: &self.leases,
            next_token: self.next_token,
            timeline: &self.timeline,
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
            if validate_construction_binding(Some(project), &candidate.resources, &candidate.leases)
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
    pub fn journal(&self) -> &[JournalRecord] {
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
    pub fn owned_capacity_bytes(&self) -> usize {
        let extra = self.timeline.backing().owned_bytes() + self.background.backing().owned_bytes()
            - size_of::<crate::background::Background>();
        extra
            + size_of::<Self>()
            + self.world.owned_capacity_bytes()
            + self.resources.capacity() * size_of::<Resource>()
            + self.leases.capacity() * size_of::<Option<Lease>>()
            + self.journal.capacity() * size_of::<JournalRecord>()
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
        }
        View {
            world: self.world.borrowed_view(),
            resources: &self.resources,
            leases: &self.leases,
            next_token: self.next_token,
            timeline: &self.timeline,
            background: &self.background,
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
        persist(&env, &record)?;
        trace.summary.hook_succeeded = true;
        if let Ok(change) = prepared {
            let started = trace.start();
            self.apply(change);
            trace.end(Class::Apply, started, 0, 1, 0);
            trace.summary.applied = true;
        }
        self.journal.push(record);
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
        let mut town_summary = None;
        let mut background =
            workspace::BackgroundLease::acquire(&storage.background, &self.background)?;
        let mut background_work = workspace::BackgroundWorkLease::take(&storage.background_work)?;
        for (action_index, action) in env.actions.iter().enumerate() {
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
                    timeline_driver::Frame {
                        trace,
                        world: &self.world,
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
                    prepare_construction(
                        caller,
                        command,
                        env.operation_id,
                        &self.world,
                        p,
                        &mut accounts,
                        &mut stocks,
                        &mut resources,
                        &mut leases,
                        &mut next_token,
                        &mut created_tokens[action_index],
                        timeline.as_ref(),
                    )?;
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
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let t = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
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
                    if market.profile.id != profile_id
                        || market
                            .profile
                            .transport_allocation_sha256
                            .iter()
                            .all(|byte| *byte == 0)
                        || town.is_none()
                        || proof.proof_sha256 != proof_sha256
                        || proof.proof.schema != 1
                        || proof.proof.profile_id != profile_id
                        || proof.proof.allocation_profile_sha256
                            != market.profile.transport_allocation_sha256
                        || proof.proof.generation != generation
                        || proof.proof.period_seconds != crate::timeline::DAY as u32
                        || proof.proof.world_id != self.world.scope.world()
                        || proof.proof.branch_id != self.world.scope.branch()
                        || proof.proof.service_day != service_day
                    {
                        return Err(TxError::InvalidOperation);
                    }
                    let t = timeline.as_ref().ok_or(TxError::InvalidOperation)?;
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
                        transport: Some(proof),
                    })?);
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
                    if units == 0 {
                        return Err(TxError::Capacity);
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
        if projected > ENGINE_BYTE_BUDGET {
            return Err(TxError::WorkingLimit);
        }
        validate_construction_binding(construction, &resources, &leases)?;
        Ok(Prepared {
            timeline,
            background,
            exchange,
            boot,
            economy,
            region,
            construction,
            town,
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
) -> Result<(), TxError> {
    let Some(project) = project else {
        return Ok(());
    };
    project
        .validate_persisted()
        .map_err(TxError::Construction)?;
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
            }
            crate::construction::Phase::Review => {
                project.kind == crate::construction::Kind::Build && project.waste_receiver.is_some()
            }
            crate::construction::Phase::Ready => project.waste_receiver.is_some(),
            _ => false,
        };
        if project.operating_published && !phase_keeps_operating_contribution {
            return Err(TxError::InvalidOperation);
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
    timeline: &'a crate::timeline::Timeline,
    accounts: &'a mut DirtyAccounts,
    stocks: &'a mut DirtyStocks,
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
    // Record both actual application and the authoritative planner reconciliation in one
    // persisted cursor. Timeline has already paid any due wages; no second wage transfer occurs.
    let _labor_paid_by_timeline = reconciled.worker_days;
    market_state.settled_day = day;
    market_state.last_transport = transport_receipt;
    market_state.last_summary = summary;
    Ok(summary)
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

/// Applies the bounded construction projection and its external effects to the same
/// candidate Engine state. The caller publishes this candidate only after the Store's
/// durable hook succeeds.
#[allow(clippy::too_many_arguments)]
fn prepare_construction(
    caller: Caller,
    command: crate::construction::EngineCommand,
    operation_id: u64,
    world: &World,
    project: &mut crate::construction::Project,
    accounts: &mut DirtyAccounts,
    stocks: &mut DirtyStocks,
    resources: &mut ResourceRows,
    leases: &mut LeaseRows,
    next_token: &mut u64,
    created_token: &mut Option<CapacityToken>,
    timeline: Option<&crate::timeline::Timeline>,
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
            if !candidate.operating_published || candidate.operating_resource == 0 {
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

    let effect = match command {
        EngineCommand::Work {
            at: requested,
            elapsed_seconds,
            worker_seconds,
        } => {
            if requested != at {
                return Err(TxError::Construction(crate::construction::Error::Time));
            }
            let mut available = [crate::units::Quantity::ZERO; crate::construction::MAX_MATERIALS];
            for (i, slot) in available
                .iter_mut()
                .enumerate()
                .take(candidate.material_count as usize)
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
        if effect.operation_id != operation_id {
            // These demolition effects can be blocked by live consumer capacity or
            // owner funds and retried after those inputs change. Keep their internal
            // project sequence separate from the caller's transaction id so a fresh
            // operation id can safely retry without mutating the failed candidate.
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
                for i in 0..candidate.material_count as usize {
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
                if !candidate.operating_published || candidate.operating_resource == 0 {
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
                | Self::ReleaseCapacity { .. }
                | Self::Boot {
                    command: crate::boot::Command::Fault { .. }
                        | crate::boot::Command::ClearFailure { .. }
                }
        )
    }
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
    fn engine() -> Engine {
        Engine::new(
            assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),
            vec![Resource::new(1, 10).unwrap(), Resource::new(2, 10).unwrap()],
        )
        .unwrap()
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
        use crate::{
            town_economy::{DeliveryQuote, HouseholdGroup, ShopSnapshot},
            town_runtime::{EmployerSpec, FixedRows, MarketProfile, TownMarket},
        };
        let mut e = engine();
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
        let view = e.world.view();
        let action_description = format!("{action:?}");
        e.execute(
            Caller::System,
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
}
