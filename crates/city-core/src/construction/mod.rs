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

const CONSTRUCTION_UTILITY_COUNT: usize = 2; // electricity, water

fn zero_u64(value: &u64) -> bool {
    *value == 0
}
fn zero_quantities2(value: &[Quantity; CONSTRUCTION_UTILITY_COUNT]) -> bool {
    value.iter().all(|quantity| *quantity == Quantity::ZERO)
}
fn zero_money2(value: &[Money; CONSTRUCTION_UTILITY_COUNT]) -> bool {
    value.iter().all(|money| *money == Money::ZERO)
}
fn no_material_accounting(value: &Option<ConstructionMaterialAccounting>) -> bool {
    value.is_none()
}
fn ceil_nonnegative(numerator: i128, denominator: i128) -> Result<i64, Error> {
    if numerator < 0 || denominator <= 0 {
        return Err(Error::InvalidInput);
    }
    let quotient = numerator
        .checked_add(denominator - 1)
        .ok_or(Error::Overflow)?
        / denominator;
    i64::try_from(quotient).map_err(|_| Error::Overflow)
}

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

/// Cumulative real material settlement, frozen to the opening stock limits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionMaterialRow {
    pub stock_id: u32,
    pub required: Quantity,
    pub consumed: Quantity,
    pub basis: Money,
}

/// Optional bounded material accounting retained after project materials leave the site.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionMaterialAccounting {
    pub row_count: u8,
    pub rows: [ConstructionMaterialRow; MAX_MATERIALS],
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

/// One immutable Boot-backed utility purchase line. Rates are milli-native units per
/// 3600 construction worker-seconds; tariff is cents per `quantity_unit` milli-units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionUtilitySupply {
    pub operator: u32,
    pub stock_id: u32,
    pub rate_per_3600_worker_seconds: Quantity,
    pub tariff_per_quantity_unit: Money,
    pub quantity_unit: Quantity,
}

/// Optional fixed electricity/water supply contract attached at genesis. The two slots
/// have stable identities: 0 electricity, 1 water. Every Work phase uses the same rates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionUtilityContract {
    pub electricity: Option<ConstructionUtilitySupply>,
    pub water: Option<ConstructionUtilitySupply>,
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
    #[serde(default, skip_serializing_if = "zero_quantities2")]
    pub utility_quantities: [Quantity; CONSTRUCTION_UTILITY_COUNT],
    #[serde(default, skip_serializing_if = "zero_money2")]
    pub utility_fees: [Money; CONSTRUCTION_UTILITY_COUNT],
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
    #[serde(default, skip_serializing_if = "zero_quantities2")]
    pub utility_quantities: [Quantity; CONSTRUCTION_UTILITY_COUNT],
    #[serde(default, skip_serializing_if = "zero_money2")]
    pub utility_fees: [Money; CONSTRUCTION_UTILITY_COUNT],
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
    /// Optional genesis-enabled stock usage history; omitted for legacy semantics.
    #[serde(default, skip_serializing_if = "no_material_accounting")]
    pub material_accounting: Option<ConstructionMaterialAccounting>,
    /// Fixed, opening-bound work rules for clearing a never-commissioned project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_clearance_spec: Option<AbortClearanceSpec>,
    /// Immutable record created when approved clearance work starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_clearance: Option<AbortClearanceHistory>,
    /// Optional immutable Boot supply contract. Missing preserves pre-integration projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utility_contract: Option<ConstructionUtilityContract>,
    /// Cumulative cursor intentionally survives pause, clearance, and demolition phases.
    #[serde(default, skip_serializing_if = "zero_u64")]
    pub utility_worker_seconds: u64,
    #[serde(default, skip_serializing_if = "zero_quantities2")]
    pub utility_consumed: [Quantity; CONSTRUCTION_UTILITY_COUNT],
    #[serde(default, skip_serializing_if = "zero_money2")]
    pub utility_charged: [Money; CONSTRUCTION_UTILITY_COUNT],
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

    /// Enable immutable opening-time material usage accounting for this pristine project.
    pub fn enable_material_accounting(&mut self) -> Result<(), Error> {
        if self.material_accounting.is_some() || self.kind == Kind::Demolish {
            return Err(Error::InvalidInput);
        }
        self.validate_profile()?;
        let count = usize::from(self.material_count);
        if count == 0 || count > MAX_MATERIALS {
            return Err(Error::InvalidInput);
        }
        let mut accounting = ConstructionMaterialAccounting {
            row_count: self.material_count,
            ..ConstructionMaterialAccounting::default()
        };
        for i in 0..count {
            let material = self.materials[i];
            accounting.rows[i] = ConstructionMaterialRow {
                stock_id: material.stock_id,
                required: material.required,
                consumed: Quantity::ZERO,
                basis: Money::ZERO,
            };
        }
        self.material_accounting = Some(accounting);
        self.validate_profile()
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

    fn utility_contract_valid(&self, contract: ConstructionUtilityContract) -> bool {
        let supplies = [contract.electricity, contract.water];
        if supplies.iter().all(Option::is_none) {
            return false;
        }
        supplies.into_iter().flatten().all(|supply| {
            supply.operator != 0
                && supply.operator != self.owner
                && supply.stock_id != 0
                && supply.rate_per_3600_worker_seconds != Quantity::ZERO
                && supply.tariff_per_quantity_unit != Money::ZERO
                && supply.quantity_unit != Quantity::ZERO
        })
    }

    fn validate_material_accounting(&self) -> Result<(), Error> {
        let Some(accounting) = self.material_accounting else {
            return Ok(());
        };
        let count = usize::from(accounting.row_count);
        let material_count = usize::from(self.material_count);
        if count == 0
            || count > MAX_MATERIALS
            || accounting.rows[count..]
                .iter()
                .any(|row| *row != ConstructionMaterialRow::default())
        {
            return Err(Error::InvalidInput);
        }
        let mut total_basis = Money::ZERO;
        for (i, row) in accounting.rows[..count].iter().enumerate() {
            if row.stock_id == 0
                || row.required.value() <= 0
                || row.consumed.value() < 0
                || row.consumed > row.required
                || row.basis.value() < 0
                || (row.consumed == Quantity::ZERO && row.basis != Money::ZERO)
                || accounting.rows[..i]
                    .iter()
                    .any(|previous| previous.stock_id == row.stock_id)
            {
                return Err(Error::InvalidInput);
            }
            total_basis = total_basis
                .checked_add(row.basis)
                .map_err(|_| Error::Overflow)?;
        }
        if total_basis > self.spent {
            return Err(Error::InvalidInput);
        }

        if let Some(history) = self.abort_clearance {
            if count != material_count {
                return Err(Error::InvalidInput);
            }
            for i in 0..count {
                let row = accounting.rows[i];
                let material = self.materials[i];
                if row.stock_id != material.stock_id
                    || row.required != material.required
                    || row.consumed != history.original_consumed[i]
                    || row.consumed != material.consumed
                {
                    return Err(Error::InvalidInput);
                }
            }
        } else if self.kind == Kind::Demolish && material_count == 0 {
            if accounting.rows[..count]
                .iter()
                .any(|row| row.consumed != row.required)
            {
                return Err(Error::InvalidInput);
            }
        } else {
            if count != material_count {
                return Err(Error::InvalidInput);
            }
            for i in 0..count {
                let row = accounting.rows[i];
                let material = self.materials[i];
                if row.stock_id != material.stock_id
                    || row.required != material.required
                    || row.consumed != material.consumed
                {
                    return Err(Error::InvalidInput);
                }
            }
        }
        Ok(())
    }

    fn utility_totals(
        &self,
        worker_seconds: u64,
    ) -> Result<
        (
            [Quantity; CONSTRUCTION_UTILITY_COUNT],
            [Money; CONSTRUCTION_UTILITY_COUNT],
        ),
        Error,
    > {
        let mut quantities = [Quantity::ZERO; CONSTRUCTION_UTILITY_COUNT];
        let mut fees = [Money::ZERO; CONSTRUCTION_UTILITY_COUNT];
        let Some(contract) = self.utility_contract else {
            return Ok((quantities, fees));
        };
        for (index, supply) in [contract.electricity, contract.water]
            .into_iter()
            .enumerate()
        {
            let Some(supply) = supply else { continue };
            let quantity_numerator = i128::from(supply.rate_per_3600_worker_seconds.value())
                .checked_mul(i128::from(worker_seconds))
                .ok_or(Error::Overflow)?;
            let quantity_value = ceil_nonnegative(quantity_numerator, 3600)?;
            let quantity = Quantity::new(quantity_value).map_err(|_| Error::Overflow)?;
            let fee_numerator = i128::from(supply.tariff_per_quantity_unit.value())
                .checked_mul(i128::from(quantity.value()))
                .ok_or(Error::Overflow)?;
            let fee_value =
                ceil_nonnegative(fee_numerator, i128::from(supply.quantity_unit.value()))?;
            quantities[index] = quantity;
            fees[index] = Money::new(fee_value).map_err(|_| Error::Overflow)?;
        }
        Ok((quantities, fees))
    }

    fn utility_delta(
        &self,
        worker_seconds: u64,
    ) -> Result<
        (
            [Quantity; CONSTRUCTION_UTILITY_COUNT],
            [Money; CONSTRUCTION_UTILITY_COUNT],
        ),
        Error,
    > {
        let next_seconds = self
            .utility_worker_seconds
            .checked_add(worker_seconds)
            .ok_or(Error::Overflow)?;
        let (next_quantity, next_fees) = self.utility_totals(next_seconds)?;
        let mut quantity_delta = [Quantity::ZERO; CONSTRUCTION_UTILITY_COUNT];
        let mut fee_delta = [Money::ZERO; CONSTRUCTION_UTILITY_COUNT];
        for i in 0..CONSTRUCTION_UTILITY_COUNT {
            quantity_delta[i] = next_quantity[i]
                .checked_sub(self.utility_consumed[i])
                .map_err(|_| Error::InvalidInput)?;
            fee_delta[i] = next_fees[i]
                .checked_sub(self.utility_charged[i])
                .map_err(|_| Error::InvalidInput)?;
        }
        Ok((quantity_delta, fee_delta))
    }

    fn validate_utility_state(&self) -> Result<(), Error> {
        match self.utility_contract {
            None => {
                if self.utility_worker_seconds != 0
                    || !zero_quantities2(&self.utility_consumed)
                    || !zero_money2(&self.utility_charged)
                {
                    return Err(Error::InvalidInput);
                }
            }
            Some(contract) => {
                if !self.utility_contract_valid(contract) {
                    return Err(Error::InvalidInput);
                }
                let (quantities, fees) = self.utility_totals(self.utility_worker_seconds)?;
                if quantities != self.utility_consumed || fees != self.utility_charged {
                    return Err(Error::InvalidInput);
                }
            }
        }
        if let Some(effect) = self.pending {
            if effect.kind == EffectKind::Work {
                let (quantities, fees) = self.utility_delta(effect.worker_seconds)?;
                if quantities != effect.utility_quantities || fees != effect.utility_fees {
                    return Err(Error::InvalidInput);
                }
            } else if !zero_quantities2(&effect.utility_quantities)
                || !zero_money2(&effect.utility_fees)
            {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
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
        self.validate_material_accounting()?;
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
        self.validate_utility_state()?;
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
            || self.material_accounting.is_some_and(|accounting| {
                accounting.rows[..usize::from(accounting.row_count)]
                    .iter()
                    .any(|row| row.consumed != Quantity::ZERO || row.basis != Money::ZERO)
            })
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
            material_accounting: None,
            abort_clearance_spec: None,
            abort_clearance: None,
            utility_contract: None,
            utility_worker_seconds: 0,
            utility_consumed: [Quantity::ZERO; CONSTRUCTION_UTILITY_COUNT],
            utility_charged: [Money::ZERO; CONSTRUCTION_UTILITY_COUNT],
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
            utility_quantities: [Quantity::ZERO; CONSTRUCTION_UTILITY_COUNT],
            utility_fees: [Money::ZERO; CONSTRUCTION_UTILITY_COUNT],
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
        let (utility_quantities, utility_fees) = self.utility_delta(ws)?;
        e.utility_quantities = utility_quantities;
        e.utility_fees = utility_fees;
        e.material_basis = material_basis;
        e.basis_remainders = remainders;
        for utility_fee in utility_fees {
            total_basis = total_basis
                .checked_add(utility_fee)
                .map_err(|_| Error::Overflow)?;
        }
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
            || receipt.utility_quantities != e.utility_quantities
            || receipt.utility_fees != e.utility_fees
            || receipt.total_basis != e.total_basis
            || receipt.site_units != e.site_units
            || (e.kind == EffectKind::ReceiveWaste && receipt.success && !receipt.waste_accepted)
        {
            return Err(Error::ReceiptMismatch);
        }
        let mut material_accounting = self.material_accounting;
        if receipt.success && e.kind == EffectKind::Work {
            if let Some(accounting) = material_accounting.as_mut() {
                let row_count = usize::from(accounting.row_count);
                if row_count == 0 || row_count > MAX_MATERIALS {
                    return Err(Error::ReceiptMismatch);
                }
                let mut total_basis = Money::ZERO;
                for row in &accounting.rows[..row_count] {
                    total_basis = total_basis
                        .checked_add(row.basis)
                        .map_err(|_| Error::Overflow)?;
                }
                for i in 0..MAX_MATERIALS {
                    let quantity = e.materials[i];
                    let basis = e.material_basis[i];
                    if quantity.value() < 0 || basis.value() < 0 {
                        return Err(Error::ReceiptMismatch);
                    }
                    let stock_id = e.stock_ids[i];
                    if stock_id == 0 {
                        if quantity != Quantity::ZERO || basis != Money::ZERO {
                            return Err(Error::ReceiptMismatch);
                        }
                        continue;
                    }
                    let row = accounting.rows[..row_count]
                        .iter_mut()
                        .find(|row| row.stock_id == stock_id)
                        .ok_or(Error::ReceiptMismatch)?;
                    if quantity == Quantity::ZERO {
                        if basis != Money::ZERO {
                            return Err(Error::ReceiptMismatch);
                        }
                        continue;
                    }
                    row.consumed = row
                        .consumed
                        .checked_add(quantity)
                        .map_err(|_| Error::Overflow)?;
                    if row.consumed > row.required {
                        return Err(Error::ReceiptMismatch);
                    }
                    row.basis = row.basis.checked_add(basis).map_err(|_| Error::Overflow)?;
                    total_basis = total_basis
                        .checked_add(basis)
                        .map_err(|_| Error::Overflow)?;
                }
                if total_basis
                    > self
                        .spent
                        .checked_add(e.total_basis)
                        .map_err(|_| Error::Overflow)?
                {
                    return Err(Error::ReceiptMismatch);
                }
            }
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
                if self.utility_contract.is_some() {
                    self.utility_worker_seconds = self
                        .utility_worker_seconds
                        .checked_add(e.worker_seconds)
                        .ok_or(Error::Overflow)?;
                    for i in 0..CONSTRUCTION_UTILITY_COUNT {
                        self.utility_consumed[i] = self.utility_consumed[i]
                            .checked_add(e.utility_quantities[i])
                            .map_err(|_| Error::Overflow)?;
                        self.utility_charged[i] = self.utility_charged[i]
                            .checked_add(e.utility_fees[i])
                            .map_err(|_| Error::Overflow)?;
                    }
                }
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
                self.material_accounting = material_accounting;
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
        assert!(!object.contains_key("utility_contract"));
        assert!(!object.contains_key("utility_worker_seconds"));
        assert!(!object.contains_key("utility_consumed"));
        assert!(!object.contains_key("utility_charged"));
        let restored: Project = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(restored.execution_policy, ExecutionPolicy::LegacyV1);
        assert_eq!(restored.operating_edge_id, 0);
        assert_eq!(restored.operating_published_at, None);
        assert_eq!(restored.operating_withdrawn_at, None);
        assert_eq!(restored.abort_clearance_spec, None);
        assert_eq!(restored.abort_clearance, None);
        assert_eq!(restored.utility_contract, None);
        assert_eq!(restored.utility_worker_seconds, 0);
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
                utility_quantities: effect.utility_quantities,
                utility_fees: effect.utility_fees,
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

#[cfg(test)]
mod construction_utility_rounding_tests {
    use super::*;

    fn contracted_project() -> Project {
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
            GameTime::ZERO,
        )
        .unwrap();
        project.utility_contract = Some(ConstructionUtilityContract {
            electricity: Some(ConstructionUtilitySupply {
                operator: 5,
                stock_id: 2,
                rate_per_3600_worker_seconds: Quantity::new(1_000).unwrap(),
                tariff_per_quantity_unit: Money::new(23).unwrap(),
                quantity_unit: Quantity::new(1_000).unwrap(),
            }),
            water: Some(ConstructionUtilitySupply {
                operator: 5,
                stock_id: 3,
                rate_per_3600_worker_seconds: Quantity::new(500).unwrap(),
                tariff_per_quantity_unit: Money::new(11).unwrap(),
                quantity_unit: Quantity::new(1_000).unwrap(),
            }),
        });
        project
    }

    #[test]
    fn cumulative_ceil_charges_are_partition_invariant_and_tiny_work_is_not_free() {
        let whole = contracted_project();
        let expected = whole.utility_delta(3_600).unwrap();
        assert_eq!(
            expected.0,
            [Quantity::new(1_000).unwrap(), Quantity::new(500).unwrap()]
        );
        assert_eq!(
            expected.1,
            [Money::new(23).unwrap(), Money::new(6).unwrap()]
        );

        let mut split = contracted_project();
        let first = split.utility_delta(1_800).unwrap();
        assert_eq!(first.1, [Money::new(12).unwrap(), Money::new(3).unwrap()]);
        split.utility_worker_seconds = 1_800;
        split.utility_consumed = first.0;
        split.utility_charged = first.1;
        // The construction phase may change, but the utility cursor is independent of
        // completed_worker_seconds and keeps applying the same fixed rates.
        split.kind = Kind::Demolish;
        split.phase = Phase::Demolition;
        split.completed_worker_seconds = 0;
        let second = split.utility_delta(1_800).unwrap();
        assert_eq!(first.0[0].checked_add(second.0[0]).unwrap(), expected.0[0]);
        assert_eq!(first.0[1].checked_add(second.0[1]).unwrap(), expected.0[1]);
        assert_eq!(first.1[0].checked_add(second.1[0]).unwrap(), expected.1[0]);
        assert_eq!(first.1[1].checked_add(second.1[1]).unwrap(), expected.1[1]);

        let tiny = contracted_project().utility_delta(1).unwrap();
        assert!(tiny.0.iter().all(|quantity| *quantity > Quantity::ZERO));
        assert!(tiny.1.iter().all(|fee| *fee > Money::ZERO));
    }

    #[test]
    fn utility_contract_rejects_self_supply_and_invalid_rounding_denominators() {
        let mut project = contracted_project();
        assert!(project.validate_persisted().is_ok());
        let mut contract = project.utility_contract.unwrap();
        contract.electricity.as_mut().unwrap().operator = project.owner;
        project.utility_contract = Some(contract);
        assert!(project.validate_persisted().is_err());

        let mut project = contracted_project();
        let mut contract = project.utility_contract.unwrap();
        contract.water.as_mut().unwrap().quantity_unit = Quantity::ZERO;
        project.utility_contract = Some(contract);
        assert!(project.validate_persisted().is_err());
    }
}

#[cfg(test)]
mod material_accounting_tests {
    use super::*;

    fn pending_receipt(effect: Effect, success: bool) -> Receipt {
        Receipt {
            operation_id: effect.operation_id,
            success,
            cash: effect.cash,
            materials: effect.materials,
            utility_quantities: effect.utility_quantities,
            utility_fees: effect.utility_fees,
            total_basis: effect.total_basis,
            site_units: effect.site_units,
            waste_accepted: false,
        }
    }

    fn accounted_draft() -> Project {
        let at = GameTime::new(9 * 3_600).unwrap();
        let mut project = Project::new(
            17,
            Kind::Build,
            4,
            3,
            1,
            80,
            1,
            1,
            Money::new(1_000_000).unwrap(),
            Money::new(3_600).unwrap(),
            3_600,
            &[Gate {
                id: 1,
                category: GateCategory::Permit,
                evidence_id: 0,
                party: 1,
                decided: GateDecision::Pending,
            }],
            &[Material {
                stock_id: 7,
                required: Quantity::new(1_000).unwrap(),
                consumed: Quantity::ZERO,
                basis_per_unit: Money::new(400).unwrap(),
                basis_remainder: 0,
            }],
            at,
        )
        .unwrap();
        project.enable_material_accounting().unwrap();
        project
    }

    fn accounted_project() -> Project {
        let at = GameTime::new(9 * 3_600).unwrap();
        let mut project = accounted_draft();
        project.apply(at, Command::File).unwrap();
        project
            .apply(
                at,
                Command::DecideGate {
                    gate_id: 1,
                    decision: GateDecision::Accepted,
                    evidence_id: 1,
                },
            )
            .unwrap();
        project.apply(at, Command::Prepare).unwrap();
        let reserve = project.apply(at, Command::Start).unwrap().unwrap();
        project.confirm(pending_receipt(reserve, true)).unwrap();
        // The Engine publishes the generated resource lease with the receipt.
        project.site_token = Some(1);
        project
    }

    fn complete_material_work(project: &mut Project) {
        project
            .propose_work(
                GameTime::new(10 * 3_600).unwrap(),
                3_600,
                Money::new(1_000_000).unwrap(),
                [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
                3_600,
            )
            .unwrap()
            .unwrap();
        // The Engine adapter replaces this profile estimate with the source lot's actual basis.
        let pending = project.pending.as_mut().unwrap();
        pending.material_basis[0] = Money::new(275).unwrap();
        pending.total_basis = Money::new(3_875).unwrap();
        let effect = project.pending.unwrap();
        project.confirm(pending_receipt(effect, true)).unwrap();
    }

    #[test]
    fn opening_material_accounting_records_success_and_ignores_failed_receipts() {
        eprintln!(
            "construction_project_size_bytes={}",
            std::mem::size_of::<Project>()
        );
        let mut project = accounted_project();
        let accounting = project.material_accounting.unwrap();
        assert_eq!(accounting.row_count, 1);
        assert_eq!(accounting.rows[0].stock_id, 7);
        assert_eq!(accounting.rows[0].required, Quantity::new(1_000).unwrap());
        assert_eq!(accounting.rows[0].consumed, Quantity::ZERO);
        assert_eq!(accounting.rows[0].basis, Money::ZERO);

        let failed_work = project
            .propose_work(
                GameTime::new(10 * 3_600).unwrap(),
                3_600,
                Money::new(1_000_000).unwrap(),
                [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
                3_600,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            project.confirm(pending_receipt(failed_work, false)),
            Err(Error::FailedEffect)
        );
        let accounting_after_failure = project.material_accounting.unwrap();
        assert_eq!(accounting_after_failure.rows[0].consumed, Quantity::ZERO);
        assert_eq!(accounting_after_failure.rows[0].basis, Money::ZERO);

        // Retry with the next effect and observe actual confirmed material basis.
        complete_material_work(&mut project);
        let accounting = project.material_accounting.unwrap();
        assert_eq!(accounting.rows[0].consumed, Quantity::new(1_000).unwrap());
        assert_eq!(accounting.rows[0].basis, Money::new(275).unwrap());
        project.validate_persisted().unwrap();
    }

    #[test]
    fn optional_material_accounting_is_omitted_for_legacy_and_enablement_is_one_time() {
        let mut legacy = accounted_project();
        legacy.material_accounting = None;
        let value = serde_json::to_value(legacy).unwrap();
        assert!(!value
            .as_object()
            .unwrap()
            .contains_key("material_accounting"));
        let restored: Project = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(restored.material_accounting, None);
        assert_eq!(serde_json::to_value(restored).unwrap(), value);

        let mut project = accounted_project();
        assert_eq!(
            project.enable_material_accounting(),
            Err(Error::InvalidInput)
        );
        assert_eq!(project.material_accounting.unwrap().row_count, 1);
    }

    #[test]
    fn material_accounting_survives_cancellation_and_normal_demolition_sink() {
        let mut cancelled = accounted_draft();
        cancelled
            .apply(GameTime::new(9 * 3_600).unwrap(), Command::Cancel)
            .unwrap();
        assert_eq!(cancelled.material_accounting.unwrap().row_count, 1);

        let mut demolished = accounted_project();
        complete_material_work(&mut demolished);
        demolished.kind = Kind::Demolish;
        demolished.phase = Phase::Demolition;
        demolished.material_count = 0;
        demolished.materials = [Material::default(); MAX_MATERIALS];
        assert_eq!(
            demolished.material_accounting.unwrap().rows[0].consumed,
            Quantity::new(1_000).unwrap()
        );
        demolished.validate_persisted().unwrap();
    }

    #[test]
    fn material_accounting_rejects_orphaned_and_unbacked_rows() {
        let mut project = accounted_project();
        let accounting = project.material_accounting.as_mut().unwrap();
        accounting.rows[0].required = Quantity::new(2_000).unwrap();
        assert_eq!(project.validate_persisted(), Err(Error::InvalidInput));

        let mut project = accounted_project();
        let accounting = project.material_accounting.as_mut().unwrap();
        accounting.rows[0].basis = Money::new(1).unwrap();
        assert_eq!(project.validate_persisted(), Err(Error::InvalidInput));
    }

    #[test]
    fn material_accounting_rejects_unknown_effect_stock_and_basis_without_quantity() {
        let mut unknown_stock = accounted_project();
        unknown_stock
            .propose_work(
                GameTime::new(10 * 3_600).unwrap(),
                3_600,
                Money::new(1_000_000).unwrap(),
                [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
                3_600,
            )
            .unwrap();
        unknown_stock.pending.as_mut().unwrap().stock_ids[0] = 99;
        let effect = unknown_stock.pending.unwrap();
        assert_eq!(
            unknown_stock.confirm(pending_receipt(effect, true)),
            Err(Error::ReceiptMismatch)
        );
        assert_eq!(
            unknown_stock.material_accounting.unwrap().rows[0].consumed,
            Quantity::ZERO
        );

        let mut basis_without_quantity = accounted_project();
        basis_without_quantity
            .propose_work(
                GameTime::new(10 * 3_600).unwrap(),
                3_600,
                Money::new(1_000_000).unwrap(),
                [Quantity::new(1_000).unwrap(); MAX_MATERIALS],
                3_600,
            )
            .unwrap();
        let pending = basis_without_quantity.pending.as_mut().unwrap();
        pending.materials[0] = Quantity::ZERO;
        pending.material_basis[0] = Money::new(1).unwrap();
        let effect = basis_without_quantity.pending.unwrap();
        assert_eq!(
            basis_without_quantity.confirm(pending_receipt(effect, true)),
            Err(Error::ReceiptMismatch)
        );
        assert_eq!(
            basis_without_quantity.material_accounting.unwrap().rows[0].basis,
            Money::ZERO
        );
    }
}
