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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: u64,
    pub kind: Kind,
    pub phase: Phase,
    pub previous_phase: Phase,
    pub owner: u32,
    pub contractor: u32,
    pub inspector: u32,
    pub site_resource: u32,
    pub site_units: u32,
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
    pub pending: Option<Effect>,
    pub pending_at: Option<GameTime>,
    pub next_effect: u32,
    pub waste_receiver: Option<u32>,
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
impl Project {
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
            || self.crew == 0
            || self.budget == Money::ZERO
            || self.labor_cents_per_hour == Money::ZERO
            || self.planned_worker_seconds == 0
            || self.next_effect == 0
            || gc == 0
            || gc > MAX_GATES
            || mc == 0
            || mc > MAX_MATERIALS
            || self.kind == Kind::Demolish
            || self.spent > self.budget
            || self.completed_worker_seconds > self.planned_worker_seconds
            || self.pending.is_some() != self.pending_at.is_some()
            || self.site_reserved != self.site_token.is_some()
            || self.gates[gc..].iter().any(|g| *g != Gate::default())
            || self.materials[mc..]
                .iter()
                .any(|m| *m != Material::default())
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
                || m.required == Quantity::ZERO
                || m.consumed > m.required
                || m.basis_per_unit == Money::ZERO
                || self.materials[..i].iter().any(|x| x.stock_id == m.stock_id)
            {
                return Err(Error::InvalidInput);
            }
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
            || self.spent != Money::ZERO
            || self.completed_worker_seconds != 0
            || self.phase != Phase::Draft
            || self.site_reserved
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
            kind,
            phase: Phase::Draft,
            previous_phase: Phase::Draft,
            owner,
            contractor,
            inspector,
            site_resource,
            site_units,
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
            pending: None,
            pending_at: None,
            next_effect: 1,
            waste_receiver: None,
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
            Command::Start if self.phase == Phase::Ready && self.waste_receiver.is_some() => {
                self.gates_clear()?;
                self.kind = Kind::Demolish;
                self.completed_worker_seconds = 0;
                self.material_count = 0;
                self.last_work_at = Some(at);
                self.phase = Phase::Demolition;
                Ok(None)
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
                    self.phase = Phase::Operational;
                    Ok(None)
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
            } if matches!(self.phase, Phase::Operational | Phase::Paused)
                && waste_receiver != 0
                && gate_count > 0
                && gate_count as usize <= MAX_GATES =>
            {
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
                    && self.completed_worker_seconds == self.planned_worker_seconds =>
            {
                let e = self.effect(
                    EffectKind::ReceiveWaste,
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
        for i in 0..self.material_count as usize {
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
        for i in 0..self.material_count as usize {
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
        let mut e = self.effect(EffectKind::Work, cost, qty, ws, at, 0)?;
        e.material_basis = material_basis;
        e.basis_remainders = remainders;
        e.total_basis = total_basis;
        if self
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
                for i in 0..self.material_count as usize {
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
            EffectKind::ReleaseSite => {
                self.site_reserved = false;
                self.site_token = None;
                self.phase = Phase::Closed
            }
            EffectKind::ReceiveWaste => {
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
