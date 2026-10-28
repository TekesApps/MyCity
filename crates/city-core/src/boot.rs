//! Finite startup interlock experiment, not the LC38 plant lifecycle profile.
use crate::{
    assembly::{Commodity, Role, Stock, World},
    transaction::TxError,
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
pub const PROFILE: &str = "pboot-cold-micro-v1";
pub const SCALE_PROFILE: &str = "pboot-cold-scale-v1";
/// Same finite recipe, scoped to a local city without Region/Exchange.
pub const LOCAL_PROFILE: &str = "pboot-local-development-v1";
pub const SECONDS: u64 = 60;
fn zero_quantity(value: &Quantity) -> bool {
    *value == Quantity::ZERO
}
fn zero_money(value: &Money) -> bool {
    *value == Money::ZERO
}
fn zero_quantities2(value: &[Quantity; 2]) -> bool {
    value.iter().all(|quantity| *quantity == Quantity::ZERO)
}
fn zero_money2(value: &[Money; 2]) -> bool {
    value.iter().all(|money| *money == Money::ZERO)
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    pub schema: String,
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
    pub profile: String,
    pub operator: u32,
    pub contractor: u32,
    pub stocks: [u32; 6],
    pub resources: [u32; 2],
    pub limits: [Quantity; 6],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facility {
    Pump,
    Generator,
}
impl Facility {
    fn index(self) -> usize {
        match self {
            Self::Pump => 0,
            Self::Generator => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Start {
        facility: Facility,
        reverse_request: bool,
    },
    Advance {
        seconds: u32,
    },
    Fault {
        facility: Facility,
    },
    ClearFailure {
        facility: Facility,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    Profile,
    Stage,
    Input,
    OutputFull,
    Invariant,
    Limit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Idle,
    Running,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub phase: Phase,
    pub identity: u64,
    pub started: u64,
    pub due: u64,
    pub tokens: [u64; 2],
    pub inputs: [Quantity; 6],
    pub cost: Money,
    pub completed: u64,
    pub failed: u64,
}
impl Default for Job {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            identity: 0,
            started: 0,
            due: 0,
            tokens: [0; 2],
            inputs: [Quantity::ZERO; 6],
            cost: Money::ZERO,
            completed: 0,
            failed: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Boot {
    pub profile: crate::FrozenLabel,
    pub operator: u32,
    pub contractor: u32,
    pub stocks: [u32; 6],
    pub resources: [u32; 2],
    pub limits: [Quantity; 6],
    pub initial: [Quantity; 6],
    pub initial_basis: Money,
    pub initial_cash: Money,
    pub time: u64,
    pub next_job: u64,
    pub jobs: [Job; 2],
    pub fees: Money,
    pub failed_expense: Money,
    pub fuel_used: Quantity,
    pub exports: [Quantity; 2],
    pub exported_basis: Money,
    /// Subset of total exports consumed locally by the bound Town, not Exchange.
    #[serde(default, skip_serializing_if = "zero_quantity")]
    pub local_electricity_exported: Quantity,
    #[serde(default, skip_serializing_if = "zero_money")]
    pub local_electricity_basis: Money,
    /// Construction consumption is separate from Town local use and Exchange delivery.
    #[serde(default, skip_serializing_if = "zero_quantities2")]
    pub local_construction_exported: [Quantity; 2],
    #[serde(default, skip_serializing_if = "zero_money2")]
    pub local_construction_basis: [Money; 2],
}
fn q(v: i64) -> Quantity {
    Quantity::new(v).expect("profile quantity")
}
fn m(v: i64) -> Money {
    Money::new(v).expect("profile cents")
}
fn error(e: Error) -> TxError {
    TxError::Boot(e)
}
fn plus(a: Quantity, b: Quantity) -> Result<Quantity, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
fn add(a: Money, b: Money) -> Result<Money, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
pub(crate) trait Ledger {
    fn stock(&mut self, id: u32) -> Result<Stock, TxError>;
    fn put(&mut self, id: u32, s: Stock) -> Result<(), TxError>;
    fn cash(&mut self, id: u32) -> Result<Money, TxError>;
    fn pay(&mut self, from: u32, to: u32, amount: Money) -> Result<(), TxError>;
    fn claim(&mut self, resources: [u32; 2]) -> Result<[u64; 2], TxError>;
    fn release(&mut self, tokens: [u64; 2]) -> Result<(), TxError>;
    fn owns(&mut self, resources: [u32; 2], tokens: [u64; 2]) -> bool;
}
impl Boot {
    pub(crate) fn new(o: Opening, w: &World, joint: bool) -> Result<Self, String> {
        let scaled = o.profile == SCALE_PROFILE;
        let local = o.profile == LOCAL_PROFILE;
        let profile = if local { LOCAL_PROFILE } else if scaled { SCALE_PROFILE } else { PROFILE };
        if o.schema != "mycity-m0-boot-v1"
            || o.profile != profile
            || (w.region.is_some() && !joint)
            || w.economy.is_some()
            || (!local && !scaled && w.stock_ids.len() != if joint { 14 } else { 6 })
            || (scaled
                && (!joint || !(14..=crate::assembly::MAX_STOCKS).contains(&w.stock_ids.len())))
            || (local && (joint || w.region.is_some() || !(6..=crate::assembly::MAX_STOCKS).contains(&w.stock_ids.len())))
            || o.operator == o.contractor
            || o.resources[0] == 0
            || o.resources[1] == 0
            || o.resources[0] == o.resources[1]
        {
            return Err("boot_profile".into());
        }
        let mut ids = o.stocks;
        ids.sort();
        if ids[0] == 0 || ids.windows(2).any(|x| x[0] == x[1]) {
            return Err("boot_stocks".into());
        }
        for id in [o.operator, o.contractor] {
            if w.accounts
                .get(w.account_handle(id)?)
                .map_err(|_| "account")?
                .role
                != Role::Company
                && id == o.operator
            {
                return Err("boot_operator".into());
            }
        }
        let mut initial = [Quantity::ZERO; 6];
        let mut basis = Money::ZERO;
        for (i, kind) in [
            Commodity::Fuel,
            Commodity::Electricity,
            Commodity::Water,
            Commodity::RawWater,
            Commodity::WasteWater,
            Commodity::Heat,
        ]
        .into_iter()
        .enumerate()
        {
            let s = w
                .stocks
                .get(w.stock_handle(o.stocks[i])?)
                .map_err(|_| "stock")?;
            if s.owner != w.account_handle(o.operator)?
                || s.commodity != kind
                || s.quantity > o.limits[i]
                || o.limits[i] == Quantity::ZERO
                || s.quantity.value() % 1000 != 0
            {
                return Err("boot_stock_profile".into());
            }
            initial[i] = s.quantity;
            basis = basis.checked_add(s.basis).map_err(|_| "basis")?;
            if i >= 4 && s.quantity != Quantity::ZERO {
                return Err("boot_nonempty_waste".into());
            }
        }
        let cash = w
            .view()
            .accounts
            .iter()
            .try_fold(Money::ZERO, |sum, a| sum.checked_add(a.cash))
            .map_err(|_| "cash")?;
        Ok(Self {
            profile: crate::FrozenLabel(profile),
            operator: o.operator,
            contractor: o.contractor,
            stocks: o.stocks,
            resources: o.resources,
            limits: o.limits,
            initial,
            initial_basis: basis,
            initial_cash: cash,
            time: 0,
            next_job: 1,
            jobs: [Job::default(); 2],
            fees: Money::ZERO,
            failed_expense: Money::ZERO,
            fuel_used: Quantity::ZERO,
            exports: [Quantity::ZERO; 2],
            exported_basis: Money::ZERO,
            local_electricity_exported: Quantity::ZERO,
            local_electricity_basis: Money::ZERO,
            local_construction_exported: [Quantity::ZERO; 2],
            local_construction_basis: [Money::ZERO; 2],
        })
    }
    pub(crate) fn export(
        &mut self,
        index: usize,
        quantity: Quantity,
        l: &mut impl Ledger,
    ) -> Result<Money, TxError> {
        if index > 1 {
            return Err(error(Error::Input));
        }
        let cost = take(
            l,
            self.stocks[if index == 0 { 1 } else { 2 }],
            quantity,
            false,
        )?;
        self.exports[index] = plus(self.exports[index], quantity)?;
        self.exported_basis = add(self.exported_basis, cost)?;
        Ok(cost)
    }
    /// The local profile has no Exchange or other export class.
    pub(crate) fn validate_local_exports(&self,service_quantity:[Quantity;2],service_basis:[Money;2])->Result<(),TxError>{
        if self.profile.as_str()!=LOCAL_PROFILE{return Ok(());}
        let electricity=plus(plus(self.local_electricity_exported,self.local_construction_exported[0])?,service_quantity[0])?;
        let water=plus(self.local_construction_exported[1],service_quantity[1])?;
        let basis=add(add(add(self.local_electricity_basis,self.local_construction_basis[0])?,self.local_construction_basis[1])?,add(service_basis[0],service_basis[1])?)?;
        if self.exports!=[electricity,water]||self.exported_basis!=basis{return Err(error(Error::Invariant));}Ok(())
    }
    pub(crate) fn export_local_electricity(
        &mut self,
        quantity: Quantity,
        ledger: &mut impl Ledger,
    ) -> Result<Money, TxError> {
        let basis = self.export(0, quantity, ledger)?;
        self.local_electricity_exported = plus(self.local_electricity_exported, quantity)?;
        self.local_electricity_basis = add(self.local_electricity_basis, basis)?;
        Ok(basis)
    }
    pub(crate) fn export_local_construction_utility(
        &mut self,
        index: usize,
        quantity: Quantity,
        ledger: &mut impl Ledger,
    ) -> Result<Money, TxError> {
        if index > 1 {
            return Err(error(Error::Input));
        }
        let basis = self.export(index, quantity, ledger)?;
        self.local_construction_exported[index] =
            plus(self.local_construction_exported[index], quantity)?;
        self.local_construction_basis[index] = add(self.local_construction_basis[index], basis)?;
        Ok(basis)
    }
    pub(crate) fn validate_construction_contract(
        &self,
        project: crate::construction::Project,
    ) -> Result<(), String> {
        if let Some(contract) = project.utility_contract {
            for (index, supply) in [contract.electricity, contract.water]
                .into_iter()
                .enumerate()
            {
                if let Some(supply) = supply {
                    let stock_index = if index == 0 { 1 } else { 2 };
                    if supply.operator != self.operator
                        || supply.stock_id != self.stocks[stock_index]
                        || supply.operator == project.owner
                    {
                        return Err("construction_utility_boot_binding".into());
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn validate_world_shape(&self, w: &World, joint: bool) -> Result<(), String> {
        if let Some(project) = w.construction {
            self.validate_construction_contract(project)?;
        }
        if (self.local_electricity_exported != Quantity::ZERO
            || self.local_electricity_basis != Money::ZERO)
            && !w.town.as_ref().is_some_and(|town| {
                town.profile.utility_supply.is_some_and(|binding| {
                    binding.boot_operator == self.operator
                        && binding.boot_stock_id == self.stocks[1]
                })
            })
        {
            return Err("boot_unbound_local_electricity".into());
        }
        let project_contract = w.construction.and_then(|project| project.utility_contract);
        for index in 0..2 {
            let expected_export = w
                .construction
                .map_or(Quantity::ZERO, |project| project.utility_consumed[index]);
            if self.local_construction_exported[index] != expected_export {
                return Err("boot_construction_export_mismatch".into());
            }
            if self.local_construction_exported[index] == Quantity::ZERO
                && self.local_construction_basis[index] != Money::ZERO
            {
                return Err("boot_construction_basis_without_export".into());
            }
            if self.local_construction_exported[index] != Quantity::ZERO
                || self.local_construction_basis[index] != Money::ZERO
            {
                let bound = project_contract.is_some_and(|contract| {
                    let supply = if index == 0 {
                        contract.electricity
                    } else {
                        contract.water
                    };
                    supply.is_some_and(|supply| {
                        supply.operator == self.operator
                            && supply.stock_id == self.stocks[if index == 0 { 1 } else { 2 }]
                    })
                });
                if !bound {
                    return Err("boot_unbound_local_construction_utility".into());
                }
            }
        }
        let scaled = self.profile.as_str() == SCALE_PROFILE;
        let local = self.profile.as_str() == LOCAL_PROFILE;
        if ![PROFILE, SCALE_PROFILE, LOCAL_PROFILE].contains(&self.profile.as_str())
            || w.economy.is_some()
            || (w.region.is_some() && !joint)
            || (!local && !scaled && w.stock_ids.len() != if joint { 14 } else { 6 })
            || (scaled
                && (!joint || !(14..=crate::assembly::MAX_STOCKS).contains(&w.stock_ids.len())))
            || (local && (joint || w.region.is_some() || !(6..=crate::assembly::MAX_STOCKS).contains(&w.stock_ids.len())))
            || self.operator == self.contractor
            || self.resources.contains(&0)
            || self.resources[0] == self.resources[1]
        {
            return Err("boot_profile_shape".into());
        }
        let owner = w.account_handle(self.operator)?;
        if w.accounts.get(owner).map_err(|_| "boot_owner")?.role != Role::Company {
            return Err("boot_operator".into());
        }
        w.account_handle(self.contractor)?;
        for (i, kind) in [
            Commodity::Fuel,
            Commodity::Electricity,
            Commodity::Water,
            Commodity::RawWater,
            Commodity::WasteWater,
            Commodity::Heat,
        ]
        .iter()
        .enumerate()
        {
            let stock = w
                .stocks
                .get(w.stock_handle(self.stocks[i])?)
                .map_err(|_| "boot_stock")?;
            if self.stocks[..i].contains(&self.stocks[i])
                || stock.owner != owner
                || stock.commodity != *kind
                || self.limits[i] == Quantity::ZERO
                || stock.quantity > self.limits[i]
            {
                return Err("boot_stock_binding".into());
            }
        }
        Ok(())
    }
    pub fn governs(&self, id: u32) -> bool {
        self.stocks.contains(&id)
    }
    // Every input is paid and physically moved into WIP at admission; no output before due.
    fn recipe(f: Facility) -> ([Quantity; 6], Money) {
        match f {
            Facility::Pump => ([q(0), q(100000), q(0), q(600000), q(0), q(0)], m(200)),
            Facility::Generator => ([q(1000), q(0), q(60000), q(0), q(0), q(0)], m(300)),
        }
    }
    fn output(f: Facility, failed: bool) -> [Quantity; 6] {
        match (f, failed) {
            (Facility::Pump, false) => [q(0), q(0), q(600000), q(0), q(0), q(100000)],
            (Facility::Pump, true) => [q(0), q(0), q(0), q(0), q(600000), q(100000)],
            (Facility::Generator, false) => [q(0), q(400000), q(0), q(0), q(60000), q(600000)],
            (Facility::Generator, true) => [q(0), q(0), q(0), q(0), q(60000), q(1000000)],
        }
    }
    fn worst(f: Facility) -> [Quantity; 6] {
        let normal = Self::output(f, false);
        let failure = Self::output(f, true);
        std::array::from_fn(|i| normal[i].max(failure[i]))
    }
    pub(crate) fn command(&mut self, c: Command, l: &mut impl Ledger) -> Result<(), TxError> {
        match c {
            Command::Start {
                facility,
                reverse_request,
            } => {
                let i = facility.index();
                if self.jobs[i].phase != Phase::Idle {
                    return Err(error(Error::Stage));
                }
                let (inputs, fee) = Self::recipe(facility);
                for (n, amount) in inputs.iter().enumerate() {
                    if l.stock(self.stocks[n])?.quantity < *amount {
                        return Err(error(Error::Input));
                    }
                }
                // Reserve both success and fault outlets, including other jobs' uncommitted outputs.
                for (n, input) in inputs.iter().enumerate() {
                    let mut future = l.stock(self.stocks[n])?.quantity;
                    future = future
                        .checked_sub(*input)
                        .map_err(|_| error(Error::Input))?;
                    future = plus(future, Self::worst(facility)[n])?;
                    for (j, job) in self.jobs.iter().enumerate() {
                        if job.phase == Phase::Running {
                            future = plus(
                                future,
                                Self::worst(if j == 0 {
                                    Facility::Pump
                                } else {
                                    Facility::Generator
                                })[n],
                            )?
                        }
                    }
                    if future > self.limits[n] {
                        return Err(error(Error::OutputFull));
                    }
                }
                // Request order is deliberately variable; the adapter always sorts resource keys.
                let resources = if reverse_request {
                    [self.resources[1], self.resources[0]]
                } else {
                    self.resources
                };
                let tokens = l.claim(resources)?;
                l.pay(self.operator, self.contractor, fee)?;
                let mut cost = fee;
                for (n, amount) in inputs.iter().enumerate() {
                    if *amount != Quantity::ZERO {
                        cost = add(cost, take(l, self.stocks[n], *amount, n == 0)?)?
                    }
                }
                self.fees = add(self.fees, fee)?;
                let identity = self.next_job;
                self.next_job = self.next_job.checked_add(1).ok_or(TxError::Overflow)?;
                self.jobs[i] = Job {
                    phase: Phase::Running,
                    identity,
                    started: self.time,
                    due: self.time.checked_add(SECONDS).ok_or(TxError::Overflow)?,
                    tokens,
                    inputs,
                    cost,
                    completed: self.jobs[i].completed,
                    failed: self.jobs[i].failed,
                };
            }
            Command::Advance { seconds } => {
                if seconds == 0 || u64::from(seconds) > SECONDS {
                    return Err(error(Error::Limit));
                }
                let target = self
                    .time
                    .checked_add(u64::from(seconds))
                    .ok_or(TxError::Overflow)?;
                // Stable facility key order at equal time; all earlier completions precede later ones.
                loop {
                    let next = self
                        .jobs
                        .iter()
                        .enumerate()
                        .filter(|(_, j)| j.phase == Phase::Running && j.due <= target)
                        .min_by_key(|(i, j)| (j.due, *i))
                        .map(|(i, _)| i);
                    let Some(i) = next else { break };
                    self.time = self.jobs[i].due;
                    self.finish(i, false, l)?;
                }
                self.time = target;
            }
            Command::Fault { facility } => {
                let i = facility.index();
                if self.jobs[i].phase != Phase::Running {
                    return Err(error(Error::Stage));
                }
                self.finish(i, true, l)?;
            }
            Command::ClearFailure { facility } => {
                let i = facility.index();
                if self.jobs[i].phase != Phase::Failed {
                    return Err(error(Error::Stage));
                }
                l.release(self.jobs[i].tokens)?;
                self.jobs[i].tokens = [0; 2];
                self.jobs[i].phase = Phase::Idle;
            }
        }
        Ok(())
    }
    pub(crate) fn finish(
        &mut self,
        i: usize,
        failed: bool,
        l: &mut impl Ledger,
    ) -> Result<(), TxError> {
        let f = if i == 0 {
            Facility::Pump
        } else {
            Facility::Generator
        };
        let job = self.jobs[i];
        let outputs = Self::output(f, failed);
        for (n, amount) in outputs.iter().enumerate() {
            if *amount != Quantity::ZERO {
                let s = l.stock(self.stocks[n])?;
                let quantity = plus(s.quantity, *amount)?;
                if quantity > self.limits[n] {
                    return Err(error(Error::OutputFull));
                }
                put_pool(
                    l,
                    self.stocks[n],
                    quantity,
                    add(
                        s.basis,
                        if !failed && ((i == 0 && n == 2) || (i == 1 && n == 1)) {
                            job.cost
                        } else {
                            Money::ZERO
                        },
                    )?,
                )?;
            }
        }
        if i == 1 {
            self.fuel_used = plus(self.fuel_used, job.inputs[0])?;
        }
        if failed {
            self.failed_expense = add(self.failed_expense, job.cost)?;
            self.jobs[i].phase = Phase::Failed;
            self.jobs[i].failed = self.jobs[i]
                .failed
                .checked_add(1)
                .ok_or(TxError::Overflow)?;
        } else {
            l.release(job.tokens)?;
            self.jobs[i].phase = Phase::Idle;
            self.jobs[i].tokens = [0; 2];
            self.jobs[i].completed = self.jobs[i]
                .completed
                .checked_add(1)
                .ok_or(TxError::Overflow)?;
        }
        self.jobs[i].inputs = [Quantity::ZERO; 6];
        self.jobs[i].cost = Money::ZERO;
        Ok(())
    }
    pub(crate) fn validate(&self, l: &mut impl Ledger, world_cash: Money) -> Result<(), TxError> {
        let local_construction_electricity = self.local_construction_exported[0];
        let local_construction_water = self.local_construction_exported[1];
        let local_construction_basis = add(
            self.local_construction_basis[0],
            self.local_construction_basis[1],
        )?;
        let local_electricity_total = plus(
            self.local_electricity_exported,
            local_construction_electricity,
        )?;
        let all_local_basis = add(self.local_electricity_basis, local_construction_basis)?;
        if local_electricity_total > self.exports[0]
            || local_construction_water > self.exports[1]
            || all_local_basis > self.exported_basis
            || (self.local_electricity_exported == Quantity::ZERO
                && self.local_electricity_basis != Money::ZERO)
            || (zero_quantities2(&self.local_construction_exported)
                && !zero_money2(&self.local_construction_basis))
        {
            return Err(error(Error::Invariant));
        }
        if world_cash != self.initial_cash {
            return Err(error(Error::Invariant));
        }
        let mut quantities = [Quantity::ZERO; 6];
        let mut basis = add(self.failed_expense, self.exported_basis)?;
        for (n, id) in self.stocks.iter().enumerate() {
            let s = l.stock(*id)?;
            if s.quantity > self.limits[n]
                || (s.quantity == Quantity::ZERO && s.basis != Money::ZERO)
            {
                return Err(error(Error::Invariant));
            }
            quantities[n] = s.quantity;
            basis = add(basis, s.basis)?;
        }
        for job in self.jobs {
            if job.phase != Phase::Idle && !l.owns(self.resources, job.tokens) {
                return Err(error(Error::Invariant));
            }
            basis = add(basis, job.cost)?;
            for (n, q) in job.inputs.iter().enumerate() {
                quantities[n] = plus(quantities[n], *q)?;
            }
        }
        if basis != add(self.initial_basis, self.fees)? {
            return Err(error(Error::Invariant));
        }
        // Fuel Q has 1000 Wh/Q; all utility quantities are milli-native-unit.
        let energy = i128::from(quantities[0].value()) * 1000
            + i128::from(quantities[1].value())
            + i128::from(self.exports[0].value())
            + i128::from(quantities[5].value());
        let initial_energy =
            i128::from(self.initial[0].value()) * 1000 + i128::from(self.initial[1].value());
        let water = i128::from(self.exports[1].value())
            + i128::from(quantities[2].value())
            + i128::from(quantities[3].value())
            + i128::from(quantities[4].value());
        if energy != initial_energy
            || water != i128::from(self.initial[2].value()) + i128::from(self.initial[3].value())
            || plus(quantities[0], self.fuel_used)? != self.initial[0]
        {
            return Err(error(Error::Invariant));
        }
        Ok(())
    }
}
fn take(l: &mut impl Ledger, id: u32, quantity: Quantity, frozen: bool) -> Result<Money, TxError> {
    let mut s = l.stock(id)?;
    let left = s
        .quantity
        .checked_sub(quantity)
        .map_err(|_| error(Error::Input))?;
    let cost = if frozen {
        let (end,cost)=crate::units::frozen_lot_basis_delta(
            s.origin_quantity,s.origin_basis,s.offset,quantity).map_err(|e|match e {
                crate::units::LotBasisError::Origin=>error(Error::Invariant),
                crate::units::LotBasisError::Overflow=>TxError::Overflow,
            })?;
        s.offset = end;
        i128::from(cost.value())
    } else {
        if s.quantity == Quantity::ZERO {
            return Err(error(Error::Input));
        }
        i128::from(s.basis.value()) * i128::from(quantity.value()) / i128::from(s.quantity.value())
    };
    let cost = Money::new(i64::try_from(cost).map_err(|_| TxError::Overflow)?)
        .map_err(|_| TxError::Overflow)?;
    s.quantity = left;
    s.basis = s
        .basis
        .checked_sub(cost)
        .map_err(|_| error(Error::Invariant))?;
    if !frozen {
        s.origin_quantity = s.quantity;
        s.origin_basis = s.basis;
        s.offset = Quantity::ZERO;
    }
    l.put(id, s)?;
    Ok(cost)
}
fn put_pool(l: &mut impl Ledger, id: u32, quantity: Quantity, basis: Money) -> Result<(), TxError> {
    let mut s = l.stock(id)?;
    s.quantity = quantity;
    s.basis = basis;
    s.origin_quantity = quantity;
    s.origin_basis = basis;
    s.offset = Quantity::ZERO;
    l.put(id, s)
}
