//! Finite startup interlock experiment, not the LC38 plant lifecycle profile.
use crate::{
    assembly::{Commodity, Role, Stock, World},
    transaction::TxError,
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
pub const PROFILE: &str = "pboot-cold-micro-v1";
pub const SECONDS: u64 = 60;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub schema: String,
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
        if o.schema != "mycity-m0-boot-v1"
            || o.profile != PROFILE
            || (w.region.is_some() && !joint)
            || w.economy.is_some()
            || w.account_ids.len() != 4
            || w.stock_ids.len() != if joint { 14 } else { 6 }
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
            profile: crate::FrozenLabel(PROFILE),
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
    fn finish(&mut self, i: usize, failed: bool, l: &mut impl Ledger) -> Result<(), TxError> {
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
        let end = plus(s.offset, quantity)?;
        let d = i128::from(s.origin_quantity.value());
        if d == 0 || end > s.origin_quantity {
            return Err(error(Error::Invariant));
        }
        let amount = i128::from(s.origin_basis.value()) * i128::from(end.value()) / d
            - i128::from(s.origin_basis.value()) * i128::from(s.offset.value()) / d;
        s.offset = end;
        amount
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
