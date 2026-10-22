//! Bounded construction workflow projection. Financial/material/capacity effects
//! must be committed with this state by the shared Engine/Store adapter.
use crate::units::{GameTime, Money, Quantity};
use serde::{Deserialize, Serialize};

pub const MAX_GATES: usize = 8;
pub const MAX_MATERIALS: usize = 8;
pub const DAY: i64 = 86_400;
pub const WORKDAY_SECONDS: i64 = 28_800;
pub const STATE_BUDGET_BYTES: usize = 2048;
/// C namespace avoids the existing D-prefixed system/background operation IDs.
pub const OPERATION_PREFIX: u64 = 0xc000_0000_0000_0000;

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub id: u32,
    pub category: GateCategory,
    pub evidence_id: u64,
    pub party: u32,
    pub decided: GateDecision,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateCategory {
    #[default]
    Permit,
    OwnerConsent,
    OccupantConsent,
    Compensation,
    Planning,
    PublicHearing,
    Heritage,
    Utility,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    #[default]
    Pending,
    Accepted,
    Rejected,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub stock_id: u32,
    pub required: Quantity,
    pub consumed: Quantity,
    /// Cost basis in cents per 1000 milli-units, copied from the source stock lot by Engine.
    pub basis_per_unit: Money,
    pub basis_remainder: u16,
}

/// Opening-bound disposal-work rule for one construction material stock.
/// Quantities use the existing milli-unit representation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbortClearanceMaterialRule {
    pub stock_id: u32,
    pub quantity_per_waste_capacity_unit: u32,
    pub worker_seconds_per_thousand_quantity: u32,
}

/// Fixed, genesis-bound work factors for clearing an uncommissioned project.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbortClearanceSpec {
    pub base_worker_seconds: u64,
    pub material_rules: [AbortClearanceMaterialRule; MAX_MATERIALS],
}

/// Immutable record of the construction state replaced by clearance work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbortClearanceHistory {
    pub original_kind: Kind,
    pub original_planned_worker_seconds: u64,
    pub original_completed_worker_seconds: u64,
    pub original_spent: Money,
    pub original_consumed: [Quantity; MAX_MATERIALS],
    pub original_basis_remainders: [u16; MAX_MATERIALS],
    pub started_at: Option<GameTime>,
    pub required_worker_seconds: u64,
    pub waste_units: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Build,
    Retrofit,
    Demolish,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Draft,
    Review,
    Ready,
    Mobilizing,
    Building,
    Paused,
    Inspection,
    Operational,
    Demolition,
    Closed,
    Rejected,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    ReserveSite,
    Work,
    Inspect,
    PublishOperatingCapacity,
    WithdrawOperatingCapacity,
    ReleaseSite,
    ReceiveWaste,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub operation_id: u64,
    pub kind: EffectKind,
    pub project_id: u64,
    pub cash: Money,
    pub materials: [Quantity; MAX_MATERIALS],
    pub material_basis: [Money; MAX_MATERIALS],
    pub basis_remainders: [u16; MAX_MATERIALS],
    pub total_basis: Money,
    pub site_resource: u32,
    pub site_units: u32,
    pub payer: u32,
    pub payee: u32,
    pub stock_ids: [u32; MAX_MATERIALS],
    pub waste_receiver: Option<u32>,
    /// Waste quantity accepted by the configured receiver, in receiver capacity units.
    #[serde(default)]
    pub waste_units: u32,
    #[serde(default)]
    pub waste_resource: u32,
    pub worker_seconds: u64,
    pub game_time: GameTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub operation_id: u64,
    pub success: bool,
    pub cash: Money,
    pub materials: [Quantity; MAX_MATERIALS],
    pub total_basis: Money,
    pub site_units: u32,
    pub waste_accepted: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidInput,
    WrongPhase,
    GatePending,
    GateRejected,
    Capacity,
    Funds,
    Materials,
    Time,
    WorkLimit,
    EffectPending,
    ReceiptMismatch,
    FailedEffect,
    Inspection,
    Waste,
    Overflow,
    AlreadyClosed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    File,
    DecideGate {
        gate_id: u32,
        decision: GateDecision,
        evidence_id: u64,
    },
    Prepare,
    Start,
    Work {
        at: GameTime,
        elapsed_seconds: u32,
        worker_seconds: u32,
    },
    Pause,
    Resume,
    SubmitInspection,
    AcceptInspection {
        inspector: u32,
        accepted: bool,
    },
    Cancel,
    BeginDemolition {
        waste_receiver: u32,
        gates: [Gate; MAX_GATES],
        gate_count: u8,
    },
    AcceptWaste,
    Close,
}

/// Compact command retained in the Engine retry journal. Large project configuration
/// stays in the genesis-bound Project slot and can never multiply by 16*1024 receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EngineCommand {
    File,
    DecideGate {
        gate_id: u32,
        decision: GateDecision,
        evidence_id: u64,
    },
    Prepare,
    Start,
    Work {
        at: GameTime,
        elapsed_seconds: u32,
        worker_seconds: u32,
    },
    Pause,
    Resume,
    SubmitInspection,
    AcceptInspection {
        inspector: u32,
        accepted: bool,
    },
    Cancel,
    BeginDemolition {
        waste_receiver: u32,
    },
    AcceptWaste,
    Close,
}

impl EngineCommand {
    pub(crate) fn into_command(self, project: &Project) -> Command {
        match self {
            Self::File => Command::File,
            Self::DecideGate {
                gate_id,
                decision,
                evidence_id,
            } => Command::DecideGate {
                gate_id,
                decision,
                evidence_id,
            },
            Self::Prepare => Command::Prepare,
            Self::Start => Command::Start,
            Self::Work {
                at,
                elapsed_seconds,
                worker_seconds,
            } => Command::Work {
                at,
                elapsed_seconds,
                worker_seconds,
            },
            Self::Pause => Command::Pause,
            Self::Resume => Command::Resume,
            Self::SubmitInspection => Command::SubmitInspection,
            Self::AcceptInspection {
                inspector,
                accepted,
            } => Command::AcceptInspection {
                inspector,
                accepted,
            },
            Self::Cancel => Command::Cancel,
            Self::BeginDemolition { waste_receiver } => {
                // The initial work profile fixes the involved parties and gate kinds.
                // Reopening those same gates requires fresh decisions and evidence.
                let mut gates = project.gates;
                for gate in gates.iter_mut().take(project.gate_count as usize) {
                    gate.decided = GateDecision::Pending;
                    gate.evidence_id = 0;
                }
                Command::BeginDemolition {
                    waste_receiver,
                    gates,
                    gate_count: project.gate_count,
                }
            }
            Self::AcceptWaste => Command::AcceptWaste,
            Self::Close => Command::Close,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicy {
    #[default]
    LegacyV1,
    RecoverableV2,
}
fn legacy_execution(value: &ExecutionPolicy) -> bool {
    *value == ExecutionPolicy::LegacyV1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: u64,
    /// Genesis-bound execution semantics. Missing preserves historical WAL outcomes.
    #[serde(default, skip_serializing_if = "legacy_execution")]
    pub execution_policy: ExecutionPolicy,
    pub kind: Kind,
    pub phase: Phase,
    pub previous_phase: Phase,
    pub owner: u32,
    pub contractor: u32,
    pub inspector: u32,
    pub site_resource: u32,
    pub site_units: u32,
    /// Immutable source-bound operating-capacity contribution.
    #[serde(default)]
    pub operating_resource: u32,
    #[serde(default)]
    pub operating_base_capacity: u32,
    #[serde(default)]
    pub operating_units: u32,
    /// Exact contribution currently included in Resource.capacity.
    #[serde(default)]
    pub operating_published: bool,
    /// Non-zero only for a genesis-bound, one-way road capacity effect project.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub operating_edge_id: u32,
    /// Authoritative time at which inspection published the contribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_published_at: Option<GameTime>,
    /// Permanent end-of-day withdrawal boundary. The publication timestamp remains so
    /// historical transport proofs can derive the correct capacity epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_withdrawn_at: Option<GameTime>,
    pub crew: u16,
    pub budget: Money,
    pub spent: Money,
    pub labor_cents_per_hour: Money,
    pub planned_worker_seconds: u64,
    pub completed_worker_seconds: u64,
    pub labor_remainder: u16,
    pub started_at: Option<GameTime>,
    pub last_work_at: Option<GameTime>,
    pub gate_count: u8,
    pub gates: [Gate; MAX_GATES],
    pub material_count: u8,
    pub materials: [Material; MAX_MATERIALS],
    /// Fixed, opening-bound work rules for clearing a never-commissioned project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_clearance_spec: Option<AbortClearanceSpec>,
    /// Immutable record created when approved clearance work starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_clearance: Option<AbortClearanceHistory>,
    pub pending: Option<Effect>,
    pub pending_at: Option<GameTime>,
    pub next_effect: u32,
    pub waste_receiver: Option<u32>,
    /// Immutable disposal contract copied from the opening project profile. Zero values
    /// identify legacy profiles that cannot safely accept waste.
    #[serde(default)]
    pub waste_receiver_account: u32,
    #[serde(default)]
    pub waste_receiver_resource: u32,
    #[serde(default)]
    pub waste_units: u32,
    #[serde(default)]
    pub waste_fee: Money,
    pub site_reserved: bool,
    /// Exact Engine lease token anchoring the occupied project site.
    #[serde(default)]
    pub site_token: Option<u64>,
    pub inspected: bool,
    pub waste_accepted: bool,
    pub failure_count: u32,
}

fn work_window_capacity(from: GameTime, to: GameTime, crew: u16) -> Result<u64, Error> {
    let mut t = from.value();
    let end = to.value();
    let mut seconds = 0i64;
    while t < end {
        let day = t.div_euclid(DAY);
        let midnight = day * DAY;
        if day.rem_euclid(7) < 5 {
            let begin = t.max(midnight + 9 * 3600);
            let finish = end.min(midnight + 17 * 3600);
            if finish > begin {
                seconds = seconds.checked_add(finish - begin).ok_or(Error::Overflow)?;
            }
        }
        t = end.min(midnight.checked_add(DAY).ok_or(Error::Overflow)?);
    }
    u64::try_from(
        seconds
            .checked_mul(i64::from(crew))
            .ok_or(Error::Overflow)?,
    )
    .map_err(|_| Error::Overflow)
}

fn clearance_spec_valid(project: &Project, spec: AbortClearanceSpec) -> bool {
    if spec.base_worker_seconds == 0 {
        return false;
    }
    let count = usize::from(project.material_count);
    if count == 0 || count > MAX_MATERIALS {
        return false;
    }
    let mut full_capacity = 0u128;
    for i in 0..count {
        let material = project.materials[i];
        let rule = spec.material_rules[i];
        if rule.stock_id != material.stock_id
            || rule.quantity_per_waste_capacity_unit == 0
            || rule.worker_seconds_per_thousand_quantity == 0
            || material.required.value() <= 0
        {
            return false;
        }
        let quantity = material.required.value() as u128;
        let denominator = u128::from(rule.quantity_per_waste_capacity_unit);
        let Some(units) = quantity
            .checked_add(denominator - 1)
            .map(|n| n / denominator)
        else {
            return false;
        };
        let Some(total) = full_capacity.checked_add(units) else {
            return false;
        };
        full_capacity = total;
    }
    spec.material_rules[count..]
        .iter()
        .all(|rule| *rule == AbortClearanceMaterialRule::default())
        && full_capacity <= u128::from(project.waste_units)
        && {
            let mut full_materials = project.materials;
            for material in &mut full_materials[..count] {
                material.consumed = material.required;
            }
            clearance_totals(full_materials, count, spec).is_ok()
        }
}

fn clearance_totals(
    materials: [Material; MAX_MATERIALS],
    count: usize,
    spec: AbortClearanceSpec,
) -> Result<(u64, u32), Error> {
    if count == 0 || count > MAX_MATERIALS {
        return Err(Error::InvalidInput);
    }
    let mut worker_seconds = u128::from(spec.base_worker_seconds);
    let mut waste_units = 0u128;
    for (material, rule) in materials[..count].iter().zip(&spec.material_rules[..count]) {
        if material.stock_id == 0
            || material.required.value() <= 0
            || material.consumed.value() < 0
            || material.consumed > material.required
            || rule.stock_id != material.stock_id
            || rule.quantity_per_waste_capacity_unit == 0
            || rule.worker_seconds_per_thousand_quantity == 0
        {
            return Err(Error::InvalidInput);
        }
        let quantity = u128::try_from(material.consumed.value()).map_err(|_| Error::Overflow)?;
        let worker_numerator = quantity
            .checked_mul(u128::from(rule.worker_seconds_per_thousand_quantity))
            .ok_or(Error::Overflow)?;
        let worker = worker_numerator.checked_add(999).ok_or(Error::Overflow)? / 1000;
        worker_seconds = worker_seconds.checked_add(worker).ok_or(Error::Overflow)?;
        let capacity_unit = u128::from(rule.quantity_per_waste_capacity_unit);
        let units = quantity
            .checked_add(capacity_unit - 1)
            .ok_or(Error::Overflow)?
            / capacity_unit;
        waste_units = waste_units.checked_add(units).ok_or(Error::Overflow)?;
    }
    Ok((
        u64::try_from(worker_seconds).map_err(|_| Error::Overflow)?,
        u32::try_from(waste_units).map_err(|_| Error::Overflow)?,
    ))
}

impl Project {
    pub fn recoverable_commands(&self) -> bool {
        self.execution_policy == ExecutionPolicy::RecoverableV2
    }

    /// Whether this occupied, never-published project can enter the clearance review path.
    pub fn can_request_uncommissioned_clearance(&self) -> bool {
        self.recoverable_commands()
            && self.kind != Kind::Demolish
            && matches!(
                self.phase,
                Phase::Building | Phase::Paused | Phase::Inspection
            )
            && self.site_reserved
            && self.site_token.is_some()
            && self.started_at.is_some()
            && self.last_work_at.is_some()
            && self.pending.is_none()
            && self.pending_at.is_none()
            && self.abort_clearance.is_none()
            && self.waste_receiver.is_none()
            && !self.waste_accepted
            && !self.operating_published
            && self.operating_published_at.is_none()
            && self.operating_withdrawn_at.is_none()
            && self
                .abort_clearance_spec
                .is_some_and(|spec| clearance_spec_valid(self, spec))
    }

    /// Number of material slots that can still be charged by a proposed work effect.
    pub fn active_material_count(&self) -> usize {
        if self.kind == Kind::Demolish {
            0
        } else {
            usize::from(self.material_count)
        }
    }

    /// Waste amount fixed when clearance started, or the ordinary demolition contract amount.
    pub fn effective_waste_units(&self) -> u32 {
        self.abort_clearance
            .map_or(self.waste_units, |history| history.waste_units)
    }

    /// Disposal is a flat service charge: positive waste pays the full contract fee.
    pub fn effective_waste_fee(&self) -> Money {
        if self.abort_clearance.is_some() && self.effective_waste_units() == 0 {
            Money::ZERO
        } else {
            self.waste_fee
        }
    }

    pub fn is_uncommissioned_clearance(&self) -> bool {
        self.abort_clearance.is_some()
    }

    /// Validate static configuration and runtime invariants for checkpoints.
    pub fn validate_persisted(&self) -> Result<(), Error> {
        let gc = usize::from(self.gate_count);
        let mc = usize::from(self.material_count);
        if self.id == 0
            || self.id > 0x0fff_ffff
            || self.owner == 0
            || self.contractor == 0
            || self.owner == self.contractor
            || self.inspector == 0
            || self.site_resource == 0
            || self.site_units == 0
            || (self.operating_resource == 0
                && (self.operating_base_capacity != 0
                    || self.operating_units != 0
                    || self.operating_published
                    || self.operating_edge_id != 0
                    || self.operating_published_at.is_some()
                    || self.operating_withdrawn_at.is_some()))
            || (self.operating_resource != 0 && self.operating_units == 0)
            || (self.operating_published && self.operating_resource == 0)
            || (self.operating_edge_id != 0
                && (self.operating_resource == 0
                    || self.operating_base_capacity == 0
                    || (self.kind == Kind::Build && self.operating_withdrawn_at.is_some())
                    || (self.kind == Kind::Demolish
                        && self.operating_withdrawn_at.is_none()
                        && self.abort_clearance.is_none())))
            || (self.operating_published_at.is_some()
                && !self.operating_published
                && self.operating_withdrawn_at.is_none())
            || (self.operating_published_at.is_some() && self.operating_edge_id == 0)
            || (self.operating_withdrawn_at.is_some()
                && (self.operating_edge_id == 0
                    || self.operating_published
                    || self.operating_published_at.is_none()))
            || (self.operating_edge_id != 0
                && self.operating_published
                    != (self.operating_published_at.is_some()
                        && self.operating_withdrawn_at.is_none()))
            || self.operating_published_at.is_some_and(|at| {
                at.value() < 0
                    || at.value() % DAY != 0
                    || self.last_work_at.is_none_or(|work_at| {
                        self.kind == Kind::Build && at.value() < work_at.value()
                    })
            })
            || self.operating_withdrawn_at.is_some_and(|at| {
                at.value() < 0
                    || at.value() % DAY != 0
                    || self
                        .operating_published_at
                        .is_none_or(|published| at.value() < published.value())
                    || self
                        .last_work_at
                        .is_none_or(|work_at| work_at.value() < at.value())
            })
            || self.crew == 0
            || self.budget == Money::ZERO
            || self.labor_cents_per_hour == Money::ZERO
            || self.planned_worker_seconds == 0
            || self.next_effect == 0
            || gc == 0
            || gc > MAX_GATES
            || (mc == 0 && self.kind != Kind::Demolish)
            || mc > MAX_MATERIALS
            || (self.kind == Kind::Demolish
                && !matches!(self.phase, Phase::Demolition | Phase::Closed)
                && !(self.phase == Phase::Paused
                    && self.waste_receiver.is_some()
                    && self.site_reserved
                    && !self.operating_published
                    && self.pending.is_none()))
            || self.spent > self.budget
            || self.completed_worker_seconds > self.planned_worker_seconds
            || self.pending.is_some() != self.pending_at.is_some()
            || self.site_reserved != self.site_token.is_some()
            || (self.waste_receiver_resource == 0) != (self.waste_units == 0)
            || (self.waste_units != 0 && self.waste_receiver_account == 0)
            || (self.waste_units == 0 && self.waste_fee != Money::ZERO)
            || self.waste_fee.value() < 0
            || self.gates[gc..].iter().any(|g| *g != Gate::default())
            || self.materials[mc..]
                .iter()
                .any(|m| *m != Material::default())
            || self.abort_clearance_spec.is_some_and(|spec| {
                self.kind != Kind::Demolish && !clearance_spec_valid(self, spec)
            })
        {
            return Err(Error::InvalidInput);
        }
        for (i, g) in self.gates[..gc].iter().enumerate() {
            if g.id == 0
                || g.party == 0
                || (g.decided == GateDecision::Pending && g.evidence_id != 0)
                || (g.decided == GateDecision::Accepted && g.evidence_id == 0)
                || (g.decided == GateDecision::Rejected && g.evidence_id != 0)
                || self.gates[..i].iter().any(|x| x.id == g.id)
            {
                return Err(Error::InvalidInput);
            }
        }
        for (i, m) in self.materials[..mc].iter().enumerate() {
            if m.stock_id == 0
                || m.required.value() <= 0
                || m.consumed > m.required
                || m.consumed.value() < 0
                || m.basis_per_unit == Money::ZERO
                || self.materials[..i].iter().any(|x| x.stock_id == m.stock_id)
            {
                return Err(Error::InvalidInput);
            }
        }
        if let Some(history) = self.abort_clearance {
            if !self.recoverable_commands() {
                return Err(Error::InvalidInput);
            }
            let spec = self.abort_clearance_spec.ok_or(Error::InvalidInput)?;
            if !clearance_spec_valid(self, spec) {
                return Err(Error::InvalidInput);
            }
            let (required, waste_units) = clearance_totals(
                history.original_consumed.iter().enumerate().fold(
                    [Material::default(); MAX_MATERIALS],
                    |mut saved, (i, q)| {
                        if i < mc {
                            saved[i] = self.materials[i];
                            saved[i].consumed = *q;
                            saved[i].basis_remainder = history.original_basis_remainders[i];
                        }
                        saved
                    },
                ),
                mc,
                spec,
            )?;
            if self.kind != Kind::Demolish
                || !matches!(
                    self.phase,
                    Phase::Demolition | Phase::Paused | Phase::Closed
                )
                || history.original_kind == Kind::Demolish
                || self.operating_published
                || self.operating_published_at.is_some()
                || self.operating_withdrawn_at.is_some()
                || !self.site_reserved && self.phase != Phase::Closed
                || !self.site_reserved && !self.waste_accepted
                || self.material_count as usize != mc
                || history.original_planned_worker_seconds == 0
                || history.original_completed_worker_seconds
                    > history.original_planned_worker_seconds
                || history.started_at.is_none()
                || history
                    .started_at
                    .is_some_and(|started| started.value() < 0)
                || history.started_at.is_none_or(|started| {
                    self.started_at
                        .is_none_or(|now| now.value() < started.value())
                })
                || self.started_at.is_none_or(|started| {
                    self.last_work_at
                        .is_none_or(|last| started.value() > last.value())
                })
                || history.required_worker_seconds != required
                || self.planned_worker_seconds != required
                || self.completed_worker_seconds > required
                || history.waste_units != waste_units
                || waste_units > self.waste_units
                || history.original_spent < Money::ZERO
                || history.original_spent > self.spent
                || history.original_consumed[mc..]
                    .iter()
                    .any(|q| *q != Quantity::ZERO)
                || history.original_basis_remainders[mc..]
                    .iter()
                    .any(|r| *r != 0)
                || self.materials[..mc]
                    .iter()
                    .zip(history.original_consumed)
                    .any(|(m, consumed)| m.consumed != consumed)
                || self.materials[..mc]
                    .iter()
                    .zip(history.original_basis_remainders)
                    .any(|(m, remainder)| m.basis_remainder != remainder)
                || (self.phase == Phase::Closed && (self.site_reserved || !self.waste_accepted))
                || (self.operating_edge_id != 0 && self.operating_withdrawn_at.is_some())
            {
                return Err(Error::InvalidInput);
            }
            for i in 0..mc {
                let original_completed = i128::from(history.original_completed_worker_seconds);
                let original_planned = i128::from(history.original_planned_worker_seconds);
                let expected_consumed = i128::from(self.materials[i].required.value())
                    .checked_mul(original_completed)
                    .ok_or(Error::Overflow)?
                    / original_planned;
                if i128::from(history.original_consumed[i].value()) != expected_consumed
                    || history.original_basis_remainders[i] >= 1000
                {
                    return Err(Error::InvalidInput);
                }
            }
        } else if self.kind == Kind::Demolish
            && self.operating_edge_id != 0
            && self.operating_withdrawn_at.is_none()
        {
            return Err(Error::InvalidInput);
        }
        if let Some(effect) = self.pending {
            if effect.project_id != self.id
                || effect.operation_id
                    != (OPERATION_PREFIX
                        | (self.id << 32)
                        | u64::from(self.next_effect.saturating_sub(1)))
                || effect.site_resource != self.site_resource
            {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
    }
    /// Validate a pristine opening-bound project profile before it becomes authoritative.
    pub fn validate_profile(&self) -> Result<(), Error> {
        self.validate_persisted()?;
        if self.pending.is_some()
            || self.pending_at.is_some()
            || self.abort_clearance.is_some()
            || self.spent != Money::ZERO
            || self.completed_worker_seconds != 0
            || self.phase != Phase::Draft
            || self.site_reserved
            || self.operating_published
            || self.inspected
            || self.waste_receiver.is_some()
            || self.waste_accepted
            || self.failure_count != 0
            || self.materials[..self.material_count as usize]
                .iter()
                .any(|m| m.consumed != Quantity::ZERO || m.basis_remainder != 0)
            || self.gates[..self.gate_count as usize]
                .iter()
                .any(|g| g.decided != GateDecision::Pending)
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        kind: Kind,
        owner: u32,
        contractor: u32,
        inspector: u32,
        site_resource: u32,
        site_units: u32,
        crew: u16,
        budget: Money,
        labor_cents_per_hour: Money,
        planned_worker_seconds: u64,
        gates: &[Gate],
        materials: &[Material],
        at: GameTime,
    ) -> Result<Self, Error> {
        if id == 0
            || id > 0x0fff_ffff
            || owner == 0
            || contractor == 0
            || contractor == owner
            || inspector == 0
            || site_resource == 0
            || site_units == 0
            || crew == 0
            || budget == Money::ZERO
            || labor_cents_per_hour == Money::ZERO
            || planned_worker_seconds == 0
            || gates.is_empty()
            || gates.len() > MAX_GATES
            || materials.is_empty()
            || materials.len() > MAX_MATERIALS
            || kind == Kind::Demolish
            || materials.iter().any(|m| {
                m.stock_id == 0
                    || m.required == Quantity::ZERO
                    || m.consumed != Quantity::ZERO
                    || m.basis_per_unit == Money::ZERO
                    || m.basis_remainder != 0
            })
            || materials.iter().enumerate().any(|(i, m)| {
                m.required != Quantity::ZERO
                    && materials[..i].iter().any(|p| p.stock_id == m.stock_id)
            })
            || gates.iter().any(|g| {
                g.id == 0
                    || g.party == 0
                    || g.decided != GateDecision::Pending
                    || g.evidence_id != 0
            })
            || gates
                .iter()
                .map(|g| g.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != gates.len()
        {
            return Err(Error::InvalidInput);
        }
        let mut gs = [Gate::default(); MAX_GATES];
        gs[..gates.len()].copy_from_slice(gates);
        let mut ms = [Material::default(); MAX_MATERIALS];
        ms[..materials.len()].copy_from_slice(materials);
        Ok(Self {
            id,
            execution_policy: ExecutionPolicy::RecoverableV2,
            kind,
            phase: Phase::Draft,
            previous_phase: Phase::Draft,
            owner,
            contractor,
            inspector,
            site_resource,
            site_units,
            operating_resource: 0,
            operating_base_capacity: 0,
            operating_units: 0,
            operating_published: false,
            operating_edge_id: 0,
            operating_published_at: None,
            operating_withdrawn_at: None,
            crew,
            budget,
            spent: Money::ZERO,
            labor_cents_per_hour,
            planned_worker_seconds,
            completed_worker_seconds: 0,
            labor_remainder: 0,
            started_at: None,
            last_work_at: Some(at),
            gate_count: gates.len() as u8,
            gates: gs,
            material_count: materials.len() as u8,
            materials: ms,
            abort_clearance_spec: None,
            abort_clearance: None,
            pending: None,
            pending_at: None,
            next_effect: 1,
            waste_receiver: None,
            waste_receiver_account: 0,
            waste_receiver_resource: 0,
            waste_units: 0,
            waste_fee: Money::ZERO,
            site_reserved: false,
            site_token: None,
            inspected: false,
            waste_accepted: false,
            failure_count: 0,
        })
    }
    pub fn apply(&mut self, at: GameTime, command: Command) -> Result<Option<Effect>, Error> {
        if self.pending.is_some() {
            return Err(Error::EffectPending);
        }
        match command {
            Command::File if self.phase == Phase::Draft => {
                self.phase = Phase::Review;
                Ok(None)
            }
            Command::DecideGate {
                gate_id,
                decision,
                evidence_id,
            } if self.phase == Phase::Review => {
                let g = self.gates[..self.gate_count as usize]
                    .iter_mut()
                    .find(|g| g.id == gate_id)
                    .ok_or(Error::InvalidInput)?;
                if g.decided != GateDecision::Pending
                    || (decision == GateDecision::Accepted && evidence_id == 0)
                    || (decision == GateDecision::Rejected && evidence_id != 0)
                {
                    return Err(Error::InvalidInput);
                }
                g.decided = decision;
                g.evidence_id = evidence_id;
                if decision == GateDecision::Rejected {
                    if self.waste_receiver.is_some() {
                        self.phase = self.previous_phase;
                        self.waste_receiver = None
                    } else {
                        self.phase = Phase::Rejected
                    }
                }
                Ok(None)
            }
            Command::Prepare if self.phase == Phase::Review => {
                self.gates_clear()?;
                self.phase = Phase::Ready;
                Ok(None)
            }
            Command::Start
                if self.phase == Phase::Ready
                    && self.waste_receiver.is_some()
                    && !self.operating_published
                    && self.operating_published_at.is_none()
                    && self.operating_withdrawn_at.is_none()
                    && self.abort_clearance.is_none()
                    && self.site_reserved
                    && self.site_token.is_some()
                    && at.value() >= 0
                    && self
                        .last_work_at
                        .is_some_and(|last| at.value() >= last.value())
                    && self
                        .abort_clearance_spec
                        .is_some_and(|spec| clearance_spec_valid(self, spec)) =>
            {
                self.gates_clear()?;
                let spec = self.abort_clearance_spec.ok_or(Error::InvalidInput)?;
                let (required_worker_seconds, waste_units) =
                    clearance_totals(self.materials, usize::from(self.material_count), spec)?;
                let history = AbortClearanceHistory {
                    original_kind: self.kind,
                    original_planned_worker_seconds: self.planned_worker_seconds,
                    original_completed_worker_seconds: self.completed_worker_seconds,
                    original_spent: self.spent,
                    original_consumed: self.materials.map(|material| material.consumed),
                    original_basis_remainders: self
                        .materials
                        .map(|material| material.basis_remainder),
                    started_at: self.started_at,
                    required_worker_seconds,
                    waste_units,
                };
                self.abort_clearance = Some(history);
                self.kind = Kind::Demolish;
                self.planned_worker_seconds = required_worker_seconds;
                self.completed_worker_seconds = 0;
                self.started_at = Some(at);
                self.last_work_at = Some(at);
                self.phase = Phase::Demolition;
                // No capacity or site effect is due: the existing occupied site stays leased.
                Ok(None)
            }
            Command::Start
                if self.phase == Phase::Ready
                    && self.waste_receiver.is_some()
                    && self.operating_published
                    && self.operating_withdrawn_at.is_none() =>
            {
                self.gates_clear()?;
                self.kind = Kind::Demolish;
                self.completed_worker_seconds = 0;
                self.material_count = 0;
                self.materials = [Material::default(); MAX_MATERIALS];
                self.last_work_at = Some(at);
                let e = self.effect(
                    EffectKind::WithdrawOperatingCapacity,
                    Money::ZERO,
                    [Quantity::ZERO; MAX_MATERIALS],
                    0,
                    at,
                    0,
                )?;
                self.pending = Some(e);
                self.pending_at = Some(at);
                Ok(Some(e))
            }
            Command::Start if self.phase == Phase::Ready && self.kind != Kind::Demolish => {
                let e = self.effect(
                    EffectKind::ReserveSite,
                    Money::ZERO,
                    [Quantity::ZERO; MAX_MATERIALS],
                    0,
                    at,
                    self.site_units,
                )?;
                self.pending = Some(e);
                self.pending_at = Some(at);
                self.phase = Phase::Mobilizing;
                Ok(Some(e))
            }
            Command::Work { .. } => Err(Error::InvalidInput),
            Command::Pause if self.phase == Phase::Building || self.phase == Phase::Demolition => {
                self.phase = Phase::Paused;
                self.last_work_at = Some(at);
                Ok(None)
            }
            Command::Resume if self.phase == Phase::Paused => {
                self.phase = if self.kind == Kind::Demolish {
                    Phase::Demolition
                } else {
                    Phase::Building
                };
                self.last_work_at = Some(at);
                Ok(None)
            }
            Command::SubmitInspection
                if self.phase == Phase::Building
                    && self.completed_worker_seconds == self.planned_worker_seconds =>
            {
                let e = self.effect(
                    EffectKind::Inspect,
                    Money::ZERO,
                    [Quantity::ZERO; MAX_MATERIALS],
                    0,
                    at,
                    0,
                )?;
                self.pending = Some(e);
                self.pending_at = Some(at);
                Ok(Some(e))
            }
            Command::AcceptInspection {
                inspector,
                accepted,
            } if self.phase == Phase::Inspection && inspector == self.inspector => {
                if accepted {
                    self.inspected = true;
                    let e = self.effect(
                        EffectKind::PublishOperatingCapacity,
                        Money::ZERO,
                        [Quantity::ZERO; MAX_MATERIALS],
                        0,
                        at,
                        0,
                    )?;
                    self.pending = Some(e);
                    self.pending_at = Some(at);
                    Ok(Some(e))
                } else {
                    self.phase = Phase::Building;
                    Ok(None)
                }
            }
            Command::Cancel
                if self.waste_receiver.is_none()
                    && matches!(self.phase, Phase::Draft | Phase::Review | Phase::Ready) =>
            {
                self.phase = Phase::Cancelled;
                Ok(None)
            }
            Command::BeginDemolition {
                waste_receiver,
                gates,
                gate_count,
            } if matches!(
                self.phase,
                Phase::Operational | Phase::Building | Phase::Paused | Phase::Inspection
            ) && waste_receiver != 0
                && waste_receiver == self.waste_receiver_account
                && self.waste_units != 0
                && self.waste_receiver_resource != 0
                && gate_count > 0
                && gate_count as usize <= MAX_GATES =>
            {
                let operating_demolition = self.phase == Phase::Operational
                    && self.operating_published
                    && self.operating_withdrawn_at.is_none()
                    && self.abort_clearance.is_none();
                let clearance = self.can_request_uncommissioned_clearance();
                if !operating_demolition && !clearance {
                    return Err(Error::InvalidInput);
                }
                if gates[..gate_count as usize].iter().any(|g| {
                    g.id == 0
                        || g.party == 0
                        || g.decided != GateDecision::Pending
                        || g.evidence_id != 0
                }) {
                    return Err(Error::InvalidInput);
                }
                let mut ids = std::collections::BTreeSet::new();
                if gates[..gate_count as usize]
                    .iter()
                    .any(|g| !ids.insert(g.id))
                {
                    return Err(Error::InvalidInput);
                }
                self.previous_phase = self.phase;
                self.phase = Phase::Review;
                self.gates = gates;
                self.gate_count = gate_count;
                self.waste_receiver = Some(waste_receiver);
                self.waste_accepted = false;
                Ok(None)
            }
            Command::AcceptWaste
                if self.phase == Phase::Demolition
                    && self.waste_receiver.is_some()
                    && (!self.waste_accepted || !self.recoverable_commands())
                    && self.completed_worker_seconds == self.planned_worker_seconds =>
            {
                let waste_units = self.effective_waste_units();
                if waste_units == 0 {
                    // The receiver explicitly signs off the completed no-material clearance;
                    // do not invent waste, capacity use, or a fee effect.
                    self.waste_accepted = true;
                    return Ok(None);
                }
                let e = self.effect(
                    EffectKind::ReceiveWaste,
                    self.effective_waste_fee(),
                    [Quantity::ZERO; MAX_MATERIALS],
                    0,
                    at,
                    0,
                )?;
                let mut e = e;
                e.waste_units = waste_units;
                e.waste_resource = self.waste_receiver_resource;
                e.payee = self.waste_receiver_account;
                if self
                    .spent
                    .checked_add(e.cash)
                    .map_err(|_| Error::Overflow)?
                    > self.budget
                {
                    return Err(Error::Funds);
                }
                self.pending = Some(e);
                self.pending_at = Some(at);
                Ok(Some(e))
            }
            Command::Close if self.phase == Phase::Demolition && self.waste_accepted => {
                let e = self.effect(
                    EffectKind::ReleaseSite,
                    Money::ZERO,
                    [Quantity::ZERO; MAX_MATERIALS],
                    0,
                    at,
                    self.site_units,
                )?;
                self.pending = Some(e);
                self.pending_at = Some(at);
                Ok(Some(e))
            }
            Command::Close if self.phase == Phase::Demolition && self.waste_receiver.is_none() => {
                Err(Error::Waste)
            }
            _ => Err(Error::WrongPhase),
        }
    }
    fn gates_clear(&self) -> Result<(), Error> {
        for g in &self.gates[..self.gate_count as usize] {
            match g.decided {
                GateDecision::Pending => return Err(Error::GatePending),
                GateDecision::Rejected => return Err(Error::GateRejected),
                GateDecision::Accepted => {}
            }
        }
        Ok(())
    }
    fn effect(
        &mut self,
        kind: EffectKind,
        cash: Money,
        materials: [Quantity; MAX_MATERIALS],
        worker_seconds: u64,
        game_time: GameTime,
        site_units: u32,
    ) -> Result<Effect, Error> {
        let id = OPERATION_PREFIX | (self.id << 32) | u64::from(self.next_effect);
        self.next_effect = self.next_effect.checked_add(1).ok_or(Error::Overflow)?;
        Ok(Effect {
            operation_id: id,
            kind,
            project_id: self.id,
            cash,
            materials,
            material_basis: [Money::ZERO; MAX_MATERIALS],
            basis_remainders: self.materials.map(|m| m.basis_remainder),
            total_basis: cash,
            site_resource: self.site_resource,
            site_units,
            payer: self.owner,
            payee: self.contractor,
            stock_ids: self.materials.map(|m| m.stock_id),
            waste_receiver: self.waste_receiver,
            waste_units: 0,
            waste_resource: 0,
            worker_seconds,
            game_time,
        })
    }
    /// Called by the trusted Engine adapter with a fresh view of authoritative balances/stocks.
    /// Resource counts never arrive in serialized player commands.
    pub fn propose_work(
        &mut self,
        at: GameTime,
        elapsed: u32,
        funds: Money,
        available: [Quantity; MAX_MATERIALS],
        worker_seconds: u32,
    ) -> Result<Option<Effect>, Error> {
        self.propose_work_inner(at, elapsed, funds, available, worker_seconds, true)
    }

    /// Engine entry point: profile basis is only a planning estimate. The authoritative
    /// adapter reprices the pending effect from immutable inventory-lot offsets and checks
    /// the actual total against the project budget before publication.
    pub(crate) fn propose_work_authoritative(
        &mut self,
        at: GameTime,
        elapsed: u32,
        funds: Money,
        available: [Quantity; MAX_MATERIALS],
        worker_seconds: u32,
    ) -> Result<Option<Effect>, Error> {
        self.propose_work_inner(at, elapsed, funds, available, worker_seconds, false)
    }

    fn propose_work_inner(
        &mut self,
        at: GameTime,
        elapsed: u32,
        funds: Money,
        available: [Quantity; MAX_MATERIALS],
        worker_seconds: u32,
        check_estimated_material_basis: bool,
    ) -> Result<Option<Effect>, Error> {
        if !matches!(self.phase, Phase::Building | Phase::Demolition) {
            return Err(Error::WrongPhase);
        }
        let last = self.last_work_at.ok_or(Error::Time)?;
        let delta = at.value().checked_sub(last.value()).ok_or(Error::Time)?;
        if delta < 0 || u64::try_from(delta).map_err(|_| Error::Time)? != u64::from(elapsed) {
            return Err(Error::Time);
        }
        let max = work_window_capacity(last, at, self.crew)?;
        let remaining = self
            .planned_worker_seconds
            .saturating_sub(self.completed_worker_seconds);
        let ws = u64::from(worker_seconds);
        if ws == 0 || ws > max || ws > remaining {
            return Err(Error::WorkLimit);
        }
        let numerator = i128::from(self.labor_cents_per_hour.value()) * i128::from(ws)
            + i128::from(self.labor_remainder);
        let cents = i64::try_from(numerator / 3600).map_err(|_| Error::Overflow)?;
        let cost = Money::new(cents).map_err(|_| Error::Overflow)?;
        if cost > funds || self.spent.checked_add(cost).map_err(|_| Error::Overflow)? > self.budget
        {
            return Err(Error::Funds);
        }
        let mut qty = [Quantity::ZERO; MAX_MATERIALS];
        for i in 0..self.active_material_count() {
            let m = self.materials[i];
            let target = i128::from(m.required.value())
                * i128::from(self.completed_worker_seconds + ws)
                / i128::from(self.planned_worker_seconds);
            let prev = i128::from(m.required.value()) * i128::from(self.completed_worker_seconds)
                / i128::from(self.planned_worker_seconds);
            let useq = Quantity::new(i64::try_from(target - prev).map_err(|_| Error::Overflow)?)
                .map_err(|_| Error::Overflow)?;
            if useq > available[i]
                || m.consumed.checked_add(useq).map_err(|_| Error::Overflow)? > m.required
            {
                return Err(Error::Materials);
            }
            qty[i] = useq;
        }
        let mut material_basis = [Money::ZERO; MAX_MATERIALS];
        let mut remainders = self.materials.map(|m| m.basis_remainder);
        let mut total_basis = cost;
        if check_estimated_material_basis {
            for i in 0..self.active_material_count() {
                let numerator = i128::from(qty[i].value())
                    * i128::from(self.materials[i].basis_per_unit.value())
                    + i128::from(remainders[i]);
                let cents = i64::try_from(numerator / 1000).map_err(|_| Error::Overflow)?;
                material_basis[i] = Money::new(cents).map_err(|_| Error::Overflow)?;
                remainders[i] = (numerator % 1000) as u16;
                total_basis = total_basis
                    .checked_add(material_basis[i])
                    .map_err(|_| Error::Overflow)?;
            }
        }
        let mut e = self.effect(EffectKind::Work, cost, qty, ws, at, 0)?;
        e.material_basis = material_basis;
        e.basis_remainders = remainders;
        e.total_basis = total_basis;
        if check_estimated_material_basis
            && self
                .spent
                .checked_add(total_basis)
                .map_err(|_| Error::Overflow)?
                > self.budget
        {
            return Err(Error::Funds);
        }
        self.pending = Some(e);
        self.pending_at = Some(at);
        Ok(Some(e))
    }
    pub fn confirm(&mut self, receipt: Receipt) -> Result<(), Error> {
        let e = self.pending.ok_or(Error::ReceiptMismatch)?;
        if receipt.operation_id != e.operation_id
            || receipt.cash != e.cash
            || receipt.materials != e.materials
            || receipt.total_basis != e.total_basis
            || receipt.site_units != e.site_units
            || (e.kind == EffectKind::ReceiveWaste && receipt.success && !receipt.waste_accepted)
        {
            return Err(Error::ReceiptMismatch);
        }
        if !receipt.success {
            self.failure_count = self.failure_count.checked_add(1).ok_or(Error::Overflow)?;
            if e.kind == EffectKind::ReserveSite {
                self.phase = Phase::Ready;
            }
            self.pending = None;
            self.pending_at = None;
            self.next_effect = self.next_effect.checked_add(1).ok_or(Error::Overflow)?;
            return Err(Error::FailedEffect);
        }
        match e.kind {
            EffectKind::ReserveSite => {
                self.site_reserved = true;
                self.phase = Phase::Building;
                self.started_at = self.pending_at;
                self.last_work_at = self.pending_at
            }
            EffectKind::Work => {
                self.spent = self
                    .spent
                    .checked_add(e.total_basis)
                    .map_err(|_| Error::Overflow)?;
                self.completed_worker_seconds = self
                    .completed_worker_seconds
                    .checked_add(e.worker_seconds)
                    .ok_or(Error::Overflow)?;
                self.labor_remainder = ((i128::from(self.labor_cents_per_hour.value())
                    * i128::from(e.worker_seconds)
                    + i128::from(self.labor_remainder))
                    % 3600) as u16;
                for i in 0..self.active_material_count() {
                    self.materials[i].consumed = self.materials[i]
                        .consumed
                        .checked_add(e.materials[i])
                        .map_err(|_| Error::Overflow)?;
                    self.materials[i].basis_remainder = e.basis_remainders[i];
                }
                self.last_work_at = self.pending_at;
                self.phase = if self.kind == Kind::Demolish {
                    Phase::Demolition
                } else {
                    Phase::Building
                }
            }
            EffectKind::Inspect => self.phase = Phase::Inspection,
            EffectKind::PublishOperatingCapacity => {
                self.operating_published = true;
                if self.operating_edge_id != 0 {
                    self.operating_published_at = self.pending_at;
                }
                self.phase = Phase::Operational;
            }
            EffectKind::WithdrawOperatingCapacity => {
                self.operating_published = false;
                if self.operating_edge_id != 0 {
                    self.operating_withdrawn_at = self.pending_at;
                }
                self.phase = Phase::Demolition;
            }
            EffectKind::ReleaseSite => {
                self.site_reserved = false;
                self.site_token = None;
                self.phase = Phase::Closed
            }
            EffectKind::ReceiveWaste => {
                self.spent = self
                    .spent
                    .checked_add(e.cash)
                    .map_err(|_| Error::Overflow)?;
                self.waste_accepted = receipt.waste_accepted;
                if !self.waste_accepted {
                    return Err(Error::Waste);
                }
            }
        }
        self.pending = None;
        self.pending_at = None;
        Ok(())
    }
}

#[cfg(test)]
mod road_effect_legacy_serde_tests {
    use super::*;

    #[test]
    fn legacy_project_defaults_new_road_fields_without_changing_json() {
        let project = Project::new(
            7,
            Kind::Build,
            4,
            3,
            1,
            80,
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
                stock_id: 1,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(400).unwrap(),
                basis_remainder: 0,
            }],
            GameTime::new(0).unwrap(),
        )
        .unwrap();
        assert_eq!(project.execution_policy, ExecutionPolicy::RecoverableV2);
        let mut legacy = serde_json::to_value(project).unwrap();
        legacy.as_object_mut().unwrap().remove("execution_policy");
        let object = legacy.as_object().unwrap();
        assert!(!object.contains_key("operating_edge_id"));
        assert!(!object.contains_key("operating_published_at"));
        assert!(!object.contains_key("operating_withdrawn_at"));
        assert!(!object.contains_key("abort_clearance_spec"));
        assert!(!object.contains_key("abort_clearance"));
        let restored: Project = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(restored.execution_policy, ExecutionPolicy::LegacyV1);
        assert_eq!(restored.operating_edge_id, 0);
        assert_eq!(restored.operating_published_at, None);
        assert_eq!(restored.operating_withdrawn_at, None);
        assert_eq!(restored.abort_clearance_spec, None);
        assert_eq!(restored.abort_clearance, None);
        assert_eq!(serde_json::to_value(restored).unwrap(), legacy);
    }

    #[test]
    fn road_publication_cannot_precede_final_construction_work() {
        let mut project = Project::new(
            7,
            Kind::Build,
            4,
            3,
            1,
            80,
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
                stock_id: 1,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(400).unwrap(),
                basis_remainder: 0,
            }],
            GameTime::new(0).unwrap(),
        )
        .unwrap();
        project.operating_resource = 2;
        project.operating_base_capacity = 40;
        project.operating_units = 160;
        project.operating_published = true;
        project.operating_edge_id = 9;
        project.operating_published_at = Some(GameTime::new(0).unwrap());
        project.last_work_at = Some(GameTime::new(DAY).unwrap());
        project.phase = Phase::Operational;

        assert!(project.validate_persisted().is_err());
    }

    #[test]
    fn approved_road_demolition_records_one_permanent_withdrawal_boundary() {
        let mut project = Project::new(
            7,
            Kind::Build,
            4,
            3,
            1,
            80,
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
                stock_id: 1,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(400).unwrap(),
                basis_remainder: 0,
            }],
            GameTime::new(0).unwrap(),
        )
        .unwrap();
        project.operating_resource = 2;
        project.operating_base_capacity = 40;
        project.operating_units = 160;
        project.operating_edge_id = 9;
        project.operating_published = true;
        project.operating_published_at = Some(GameTime::new(DAY).unwrap());
        project.gates[0].decided = GateDecision::Accepted;
        project.gates[0].evidence_id = 1;
        project.phase = Phase::Ready;
        project.waste_receiver = Some(3);
        project.waste_receiver_account = 3;
        project.waste_receiver_resource = 4;
        project.waste_units = 2;
        project.waste_fee = Money::new(250).unwrap();

        let at = GameTime::new(3 * DAY).unwrap();
        let effect = project.apply(at, Command::Start).unwrap().unwrap();
        assert_eq!(effect.kind, EffectKind::WithdrawOperatingCapacity);
        assert!(project.operating_published);
        assert_eq!(project.operating_withdrawn_at, None);
        project
            .confirm(Receipt {
                operation_id: effect.operation_id,
                success: true,
                cash: effect.cash,
                materials: effect.materials,
                total_basis: effect.total_basis,
                site_units: effect.site_units,
                waste_accepted: false,
            })
            .unwrap();
        assert_eq!(project.kind, Kind::Demolish);
        assert_eq!(project.phase, Phase::Demolition);
        assert!(!project.operating_published);
        assert_eq!(
            project.operating_published_at,
            Some(GameTime::new(DAY).unwrap())
        );
        assert_eq!(project.operating_withdrawn_at, Some(at));
        assert_eq!(project.abort_clearance, None);
        project.validate_persisted().unwrap();

        let mut premature = project;
        premature.operating_withdrawn_at = Some(GameTime::new(0).unwrap());
        assert!(premature.validate_persisted().is_err());

        let mut withdrawal_before_work = project;
        withdrawal_before_work.last_work_at = Some(GameTime::new(2 * DAY).unwrap());
        assert!(withdrawal_before_work.validate_persisted().is_err());

        let mut missing_withdrawal = project;
        missing_withdrawal.operating_withdrawn_at = None;
        assert!(missing_withdrawal.validate_persisted().is_err());
    }

    #[test]
    fn clearance_profile_rejects_unrepresentable_full_material_work() {
        let mut project = Project::new(
            7,
            Kind::Build,
            4,
            3,
            1,
            80,
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
                stock_id: 1,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(400).unwrap(),
                basis_remainder: 0,
            }],
            GameTime::new(0).unwrap(),
        )
        .unwrap();
        project.waste_units = 1;
        let spec = AbortClearanceSpec {
            base_worker_seconds: u64::MAX,
            material_rules: [
                AbortClearanceMaterialRule {
                    stock_id: 1,
                    quantity_per_waste_capacity_unit: 2_000,
                    worker_seconds_per_thousand_quantity: 1,
                },
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
                AbortClearanceMaterialRule::default(),
            ],
        };
        assert!(!clearance_spec_valid(&project, spec));
    }
}
