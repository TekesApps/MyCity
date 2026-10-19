//! Finite P-E0/P-E1 laboratory. This profile cannot be used as a real building recipe.
use crate::{
    assembly::{Commodity, Role, Stock, World},
    transaction::TxError,
    units::{Money, Quantity},
};
use serde::{Deserialize, Serialize};
pub const DAYS: usize = 30;
pub const HOUR: u64 = 3600;
pub const DAY: u64 = 86400;
pub const MAX_SALE_LINES: usize = 32;
pub const MAX_CLAIMS: usize = 3;
pub const BATCH_Q: i64 = 10000;
pub const RAW_PRICE: i64 = 4000;
pub const GROSS_WAGE: i64 = 8000;
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebtInput {
    pub source: u64,
    pub payer: u32,
    pub payee: u32,
    pub amount: Money,
    pub due: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub schema: String,
    pub profile: String,
    pub accounts: [u32; 4],
    pub supplier_stock: u32,
    pub raw_stock: u32,
    pub product_stocks: [u32; DAYS],
    pub transport_source: String,
    pub distance_m: u32,
    pub travel_seconds: u32,
    pub capacity: Quantity,
    pub daily_capacity: Quantity,
    pub electric_wh: u32,
    pub water_l: u32,
    pub waste_q: Quantity,
    pub debt: Option<DebtInput>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Advance { seconds: u32 },
    SuspendShift,
    Prepay { quantity: Quantity },
    CancelFuture,
    SettleDebt { claim: u8 },
    HandoffDebt { claim: u8 },
    BeginExit,
    Close,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    Input,
    Unsupported,
    Stage,
    Limit,
    Overflow,
    Claim,
    NotDue,
    Responsibility,
    Stock,
    Protection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Phase {
    Open,
    RawArrival,
    Work,
    Wage,
    Produce,
    Offers,
    Delivery,
    DayEnd,
    Finished,
    Closing,
    Closed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Reason {
    None,
    Accepted,
    NoFunds,
    NoStock,
    NoInput,
    Suspended,
    AlreadyCanceled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DayRecord {
    pub accepted: bool,
    pub reason: Reason,
    pub worked_hours: u8,
    pub gross_paid: Money,
    pub raw_acquired: Quantity,
    pub produced: Quantity,
    pub f_result: Reason,
    pub g_result: Reason,
    pub f_consumed: Quantity,
    pub g_consumed: Quantity,
    pub sales: Money,
    pub cogs: Money,
    pub expense: Money,
    pub tax_delta_cents: i64,
    pub freight_q: Quantity,
}
impl Default for DayRecord {
    fn default() -> Self {
        Self {
            accepted: false,
            reason: Reason::None,
            worked_hours: 0,
            gross_paid: Money::ZERO,
            raw_acquired: Quantity::ZERO,
            produced: Quantity::ZERO,
            f_result: Reason::None,
            g_result: Reason::None,
            f_consumed: Quantity::ZERO,
            g_consumed: Quantity::ZERO,
            sales: Money::ZERO,
            cogs: Money::ZERO,
            expense: Money::ZERO,
            tax_delta_cents: 0,
            freight_q: Quantity::ZERO,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Fragment {
    pub lot_id: u32,
    pub origin_quantity: Quantity,
    pub origin_basis: Money,
    pub offset: Quantity,
    pub quantity: Quantity,
    pub basis: Money,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Sale {
    pub buyer: u32,
    pub fragment: Fragment,
    pub price: Money,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ClaimKind {
    Existing,
    Refund,
    ProfitTax,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Claim {
    pub source: u64,
    pub kind: ClaimKind,
    pub payer: u32,
    pub payee: u32,
    pub principal: Money,
    pub paid: Money,
    pub due: u64,
    pub registry: bool,
}
impl Claim {
    pub fn outstanding(&self) -> Money {
        self.principal
            .checked_sub(self.paid)
            .expect("claim invariant")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Prepayment {
    pub ordered: Quantity,
    pub dispatched: Quantity,
    pub paid: Money,
    pub canceled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Economy {
    pub profile: &'static str,
    pub accounts: [u32; 4],
    pub supplier_stock: u32,
    pub raw_stock: u32,
    pub product_stocks: [u32; DAYS],
    pub time: u64,
    pub next_due: u64,
    pub day: u8,
    pub closed_days: u8,
    pub phase: Phase,
    pub active: bool,
    pub records: [DayRecord; DAYS],
    pub raw_transit: Option<Fragment>,
    pub sales_transit: [Option<Sale>; MAX_SALE_LINES],
    pub claims: [Option<Claim>; MAX_CLAIMS],
    pub prepayment: Option<Prepayment>,
    pub protection: [Money; 4],
    pub raw_used: Quantity,
    pub produced: Quantity,
    pub consumed_f: Quantity,
    pub consumed_g: Quantity,
    pub direct_wages: Money,
    pub sales: Money,
    pub cogs: Money,
    pub expenses: Money,
    pub tax_accrued: Money,
    pub supplier_sales: Money,
    pub supplier_cogs: Money,
    pub initial_raw: Quantity,
    pub initial_cash: Money,
    pub suspended: bool,
    pub last_result: Reason,
}
fn m(v: i64) -> Money {
    Money::new(v).expect("nonnegative profile amount")
}
fn q(v: i64) -> Quantity {
    Quantity::new(v).expect("nonnegative profile quantity")
}
fn add(a: Money, b: Money) -> Result<Money, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
fn sub(a: Money, b: Money) -> Result<Money, TxError> {
    a.checked_sub(b).map_err(|_| TxError::Protection)
}
fn qa(a: Quantity, b: Quantity) -> Result<Quantity, TxError> {
    a.checked_add(b).map_err(|_| TxError::Overflow)
}
fn qs(a: Quantity, b: Quantity) -> Result<Quantity, TxError> {
    a.checked_sub(b).map_err(|_| TxError::Stock)
}
fn err(e: Error) -> TxError {
    TxError::Economy(e)
}
pub(crate) trait Ledger {
    fn stock(&mut self, id: u32) -> Result<Stock, TxError>;
    fn put_stock(&mut self, id: u32, stock: Stock) -> Result<(), TxError>;
    fn available(&mut self, account: u32) -> Result<Money, TxError>;
    fn cash(&mut self, account: u32) -> Result<Money, TxError>;
    fn protect(&mut self, account: u32, amount: Money) -> Result<(), TxError>;
    fn pay(&mut self, from: u32, to: u32, amount: Money, protected: bool) -> Result<(), TxError>;
}
impl Economy {
    pub(crate) fn new(o: Opening, w: &World) -> Result<Self, String> {
        if o.schema != "mycity-m0-economy-v1"
            || o.profile != "pe0-30d-v1"
            || o.transport_source != "laboratory-corridor-v1"
            || o.distance_m != 1000
            || o.travel_seconds != 3600
            || o.capacity != q(BATCH_Q)
            || o.daily_capacity != q(20000)
            || o.electric_wh != 0
            || o.water_l != 0
            || o.waste_q != Quantity::ZERO
            || w.region.is_some()
        {
            return Err("unsupported_economy_profile".into());
        }
        if w.view().opening_sources.iter().any(|s| s.id >= 0x80000000) {
            return Err("economy_lot_namespace".into());
        }
        if w.account_ids.len() != 4 || w.stock_ids.len() != DAYS + 2 {
            return Err("economy_fixed_actor_or_slot_count".into());
        }
        let mut ids = o.accounts;
        ids.sort();
        if ids[0] == 0 || ids.windows(2).any(|x| x[0] == x[1]) {
            return Err("economy_accounts".into());
        }
        for (id, role) in o.accounts.into_iter().zip([
            Role::Treasury,
            Role::Household,
            Role::Company,
            Role::Supplier,
        ]) {
            if w.accounts
                .get(w.account_handle(id)?)
                .map_err(|_| "account")?
                .role
                != role
            {
                return Err("economy_roles".into());
            }
        }
        let mut stocks = [0u32; DAYS + 2];
        stocks[0] = o.supplier_stock;
        stocks[1] = o.raw_stock;
        stocks[2..].copy_from_slice(&o.product_stocks);
        stocks.sort();
        if stocks[0] == 0 || stocks.windows(2).any(|x| x[0] == x[1]) {
            return Err("economy_stock_ids".into());
        }
        let mut initial_raw = Quantity::ZERO;
        for (i, id) in std::iter::once(o.supplier_stock)
            .chain(std::iter::once(o.raw_stock))
            .chain(o.product_stocks)
            .enumerate()
        {
            let s = w.stocks.get(w.stock_handle(id)?).map_err(|_| "stock")?;
            let owner = w.accounts.get(s.owner).map_err(|_| "owner")?.id;
            if s.lot_id >= 0x80000000
                || owner != o.accounts[if i == 0 { 3 } else { 2 }]
                || s.commodity
                    != if i < 2 {
                        Commodity::Raw
                    } else {
                        Commodity::Product
                    }
            {
                return Err("economy_stock_type".into());
            }
            if i == 0 {
                if s.quantity.value() % 1000 != 0
                    || i128::from(s.basis.value()) * 1000 != i128::from(s.quantity.value()) * 400
                {
                    return Err("economy_initial_basis".into());
                }
                initial_raw = s.quantity;
            } else if s.quantity != Quantity::ZERO || s.basis != Money::ZERO {
                return Err("economy_nonempty_output".into());
            }
        }
        let initial_cash = w
            .view()
            .accounts
            .iter()
            .try_fold(Money::ZERO, |sum, a| add(sum, a.cash))
            .map_err(|_| "economy_cash")?;
        let mut claims = [None; MAX_CLAIMS];
        if let Some(d) = o.debt {
            if matches!(d.source, 0 | 2 | 3)
                || d.amount == Money::ZERO
                || d.payer == d.payee
                || !o.accounts.contains(&d.payer)
                || !o.accounts.contains(&d.payee)
            {
                return Err("economy_debt".into());
            }
            claims[0] = Some(Claim {
                source: d.source,
                kind: ClaimKind::Existing,
                payer: d.payer,
                payee: d.payee,
                principal: d.amount,
                paid: Money::ZERO,
                due: d.due,
                registry: false,
            });
        }
        Ok(Self {
            profile: "pe0-30d-v1",
            accounts: o.accounts,
            supplier_stock: o.supplier_stock,
            raw_stock: o.raw_stock,
            product_stocks: o.product_stocks,
            time: 0,
            next_due: 0,
            day: 1,
            closed_days: 0,
            phase: Phase::Open,
            active: true,
            records: [DayRecord::default(); DAYS],
            raw_transit: None,
            sales_transit: [None; MAX_SALE_LINES],
            claims,
            prepayment: None,
            protection: [Money::ZERO; 4],
            raw_used: Quantity::ZERO,
            produced: Quantity::ZERO,
            consumed_f: Quantity::ZERO,
            consumed_g: Quantity::ZERO,
            direct_wages: Money::ZERO,
            sales: Money::ZERO,
            cogs: Money::ZERO,
            expenses: Money::ZERO,
            tax_accrued: Money::ZERO,
            supplier_sales: Money::ZERO,
            supplier_cogs: Money::ZERO,
            initial_raw,
            initial_cash,
            suspended: false,
            last_result: Reason::None,
        })
    }
    pub fn governs(&self, id: u32) -> bool {
        id == self.supplier_stock || id == self.raw_stock || self.product_stocks.contains(&id)
    }
    pub fn required_protection(&self, id: u32) -> Money {
        self.accounts
            .iter()
            .position(|a| *a == id)
            .map_or(Money::ZERO, |i| self.protection[i])
    }
    fn reserve(&mut self, l: &mut impl Ledger, actor: usize, amount: Money) -> Result<(), TxError> {
        l.protect(self.accounts[actor], amount)?;
        self.protection[actor] = add(self.protection[actor], amount)?;
        Ok(())
    }
    fn paid(
        &mut self,
        l: &mut impl Ledger,
        from: usize,
        to: usize,
        amount: Money,
    ) -> Result<(), TxError> {
        l.pay(self.accounts[from], self.accounts[to], amount, true)?;
        self.protection[from] = sub(self.protection[from], amount)?;
        Ok(())
    }
    fn row(&mut self) -> &mut DayRecord {
        &mut self.records[usize::from(self.day) - 1]
    }
    pub(crate) fn command(&mut self, c: Command, l: &mut impl Ledger) -> Result<(), TxError> {
        self.last_result = Reason::None;
        match c {
            Command::Advance { seconds } => {
                if seconds == 0 || seconds > 3600 {
                    return Err(err(Error::Limit));
                }
                let target = self
                    .time
                    .checked_add(u64::from(seconds))
                    .ok_or(TxError::Overflow)?;
                let mut count = 0;
                while !matches!(self.phase, Phase::Finished | Phase::Closing | Phase::Closed)
                    && (self.next_due < target
                        || (self.next_due == target && self.phase != Phase::Open))
                {
                    if self.next_due < self.time || count == 64 {
                        return Err(err(Error::Stage));
                    }
                    self.time = self.next_due;
                    self.event(l)?;
                    count += 1;
                }
                self.time = target;
            }
            Command::SuspendShift => {
                if !self.active
                    || !self.row().accepted
                    || !matches!(self.phase, Phase::RawArrival | Phase::Work)
                {
                    return Err(err(Error::Stage));
                }
                self.suspended = true;
                self.row().reason = Reason::Suspended;
            }
            Command::Prepay { quantity } => {
                if !self.active
                    || self.phase != Phase::Open
                    || self.prepayment.is_some()
                    || quantity.value() <= 0
                    || quantity.value() % BATCH_Q != 0
                    || quantity.value() > i64::from(DAYS as u8 - self.closed_days) * BATCH_Q
                    || quantity > l.stock(self.supplier_stock)?.quantity
                {
                    return Err(err(Error::Stage));
                }
                let cost =
                    Money::new(quantity.value() / 1000 * 400).map_err(|_| TxError::Overflow)?;
                if l.available(self.accounts[2])? < cost {
                    self.last_result = Reason::NoFunds;
                    return Ok(());
                }
                l.pay(self.accounts[2], self.accounts[3], cost, false)?;
                self.prepayment = Some(Prepayment {
                    ordered: quantity,
                    dispatched: Quantity::ZERO,
                    paid: cost,
                    canceled: false,
                });
            }
            Command::CancelFuture => {
                let mut order = self.prepayment.ok_or(err(Error::Stage))?;
                if order.canceled {
                    self.last_result = Reason::AlreadyCanceled;
                    return Ok(());
                }
                let remaining = qs(order.ordered, order.dispatched)?;
                order.canceled = true;
                self.prepayment = Some(order);
                let refund = m(remaining.value() / 1000 * 400);
                if refund != Money::ZERO {
                    if self.claims[1].is_some() {
                        return Err(err(Error::Claim));
                    }
                    self.claims[1] = Some(Claim {
                        source: 2,
                        kind: ClaimKind::Refund,
                        payer: self.accounts[3],
                        payee: self.accounts[2],
                        principal: refund,
                        paid: Money::ZERO,
                        due: self.time,
                        registry: false,
                    });
                    self.settle(1, l)?;
                }
            }
            Command::SettleDebt { claim } => {
                self.settle(
                    usize::from(claim).checked_sub(1).ok_or(err(Error::Claim))?,
                    l,
                )?;
            }
            Command::HandoffDebt { claim } => {
                let i = usize::from(claim).checked_sub(1).ok_or(err(Error::Claim))?;
                let c = self
                    .claims
                    .get_mut(i)
                    .and_then(Option::as_mut)
                    .ok_or(err(Error::Claim))?;
                c.registry = true;
            }
            Command::BeginExit => {
                if !matches!(self.phase, Phase::Open | Phase::Finished)
                    || (self.phase == Phase::Open && self.time != self.next_due)
                    || self.raw_transit.is_some()
                    || self.sales_transit.iter().any(Option::is_some)
                    || self.protection.iter().any(|a| *a != Money::ZERO)
                {
                    return Err(err(Error::Stage));
                }
                self.make_tax_claim()?;
                self.phase = Phase::Closing;
            }
            Command::Close => {
                if self.phase != Phase::Closing
                    || self
                        .claims
                        .iter()
                        .flatten()
                        .any(|c| c.outstanding() != Money::ZERO && !c.registry)
                    || self
                        .prepayment
                        .is_some_and(|o| !o.canceled && o.dispatched < o.ordered)
                {
                    return Err(err(Error::Responsibility));
                }
                self.active = false;
                self.phase = Phase::Closed;
            }
        }
        Ok(())
    }
    fn settle(&mut self, i: usize, l: &mut impl Ledger) -> Result<(), TxError> {
        let mut c = self
            .claims
            .get(i)
            .copied()
            .flatten()
            .ok_or(err(Error::Claim))?;
        if self.time < c.due {
            return Err(err(Error::NotDue));
        }
        let amount = c.outstanding().min(l.available(c.payer)?);
        l.pay(c.payer, c.payee, amount, false)?;
        c.paid = add(c.paid, amount)?;
        self.claims[i] = Some(c);
        if amount == Money::ZERO && c.outstanding() != Money::ZERO {
            self.last_result = Reason::NoFunds;
        }
        Ok(())
    }
    fn make_tax_claim(&mut self) -> Result<(), TxError> {
        if self.claims[2].is_none() && self.tax_accrued != Money::ZERO {
            self.claims[2] = Some(Claim {
                source: 3,
                kind: ClaimKind::ProfitTax,
                payer: self.accounts[2],
                payee: self.accounts[0],
                principal: self.tax_accrued,
                paid: Money::ZERO,
                due: self.time,
                registry: false,
            });
        }
        Ok(())
    }
    fn event(&mut self, l: &mut impl Ledger) -> Result<(), TxError> {
        let base = (u64::from(self.day) - 1) * DAY;
        match self.phase {
            Phase::Open => {
                self.suspended = false;
                let output = l.stock(self.product_stocks[usize::from(self.day) - 1])?;
                if output.quantity != Quantity::ZERO || output.basis != Money::ZERO {
                    return Err(err(Error::Stock));
                }
                let raw = l.stock(self.raw_stock)?;
                let source = l.stock(self.supplier_stock)?;
                let must_buy = raw.quantity < q(BATCH_Q);
                let prepaid = self.prepayment.is_some_and(|o| {
                    !o.canceled && o.ordered.value() - o.dispatched.value() >= BATCH_Q
                });
                let needed = m(GROSS_WAGE + if must_buy && !prepaid { RAW_PRICE } else { 0 });
                if must_buy && source.quantity < q(BATCH_Q) {
                    self.row().reason = Reason::NoInput;
                } else if l.available(self.accounts[2])? < needed {
                    self.row().reason = Reason::NoFunds;
                } else {
                    self.reserve(l, 2, needed)?;
                    self.row().accepted = true;
                    self.row().reason = Reason::Accepted;
                    if must_buy {
                        let fragment = take(l, self.supplier_stock, q(BATCH_Q))?;
                        if fragment.basis != m(RAW_PRICE) {
                            return Err(err(Error::Stock));
                        }
                        if prepaid {
                            let o = self.prepayment.as_mut().ok_or(err(Error::Stage))?;
                            o.dispatched = qa(o.dispatched, q(BATCH_Q))?;
                        } else {
                            self.paid(l, 2, 3, m(RAW_PRICE))?;
                        }
                        self.supplier_sales = add(self.supplier_sales, m(RAW_PRICE))?;
                        self.supplier_cogs = add(self.supplier_cogs, fragment.basis)?;
                        self.raw_transit = Some(fragment);
                        self.row().raw_acquired = q(BATCH_Q);
                        self.row().freight_q = q(BATCH_Q);
                    }
                }
                self.phase = Phase::RawArrival;
                self.next_due = base + HOUR;
            }
            Phase::RawArrival => {
                if let Some(f) = self.raw_transit.take() {
                    deposit(l, self.raw_stock, f)?;
                }
                self.phase = Phase::Work;
                self.next_due = base + 2 * HOUR;
            }
            Phase::Work => {
                if self.row().accepted && !self.suspended {
                    self.row().worked_hours += 1;
                }
                if self.time < base + 9 * HOUR {
                    self.next_due += HOUR;
                } else {
                    self.phase = Phase::Wage;
                }
            }
            Phase::Wage => {
                if self.row().accepted {
                    self.paid(l, 2, 1, m(7200))?;
                    self.paid(l, 2, 0, m(800))?;
                    self.row().gross_paid = m(GROSS_WAGE);
                    if self.suspended {
                        self.expenses = add(self.expenses, m(GROSS_WAGE))?;
                        self.row().expense = m(GROSS_WAGE);
                    }
                }
                self.phase = Phase::Produce;
            }
            Phase::Produce => {
                if self.row().accepted && !self.suspended {
                    if self.row().worked_hours != 8 {
                        return Err(err(Error::Stage));
                    }
                    let raw = take(l, self.raw_stock, q(BATCH_Q))?;
                    let id = self.product_stocks[usize::from(self.day) - 1];
                    let mut product = l.stock(id)?;
                    product.quantity = q(BATCH_Q);
                    product.basis = add(raw.basis, m(GROSS_WAGE))?;
                    product.offset = Quantity::ZERO;
                    product.lot_id = 0x80000000 + u32::from(self.day);
                    product.origin_quantity = product.quantity;
                    product.origin_basis = product.basis;
                    l.put_stock(id, product)?;
                    self.raw_used = qa(self.raw_used, q(BATCH_Q))?;
                    self.produced = qa(self.produced, q(BATCH_Q))?;
                    self.direct_wages = add(self.direct_wages, m(GROSS_WAGE))?;
                    self.row().produced = q(BATCH_Q);
                }
                self.phase = Phase::Offers;
            }
            Phase::Offers => {
                self.offer(l, 1, q(6000))?;
                self.offer(l, 0, q(4000))?;
                if self.row().freight_q > q(20000) {
                    return Err(err(Error::Limit));
                }
                let in_transit = self
                    .sales_transit
                    .iter()
                    .flatten()
                    .try_fold(Quantity::ZERO, |n, s| qa(n, s.fragment.quantity))?;
                if in_transit > q(BATCH_Q) {
                    return Err(err(Error::Limit));
                }
                self.phase = Phase::Delivery;
                self.next_due = base + 10 * HOUR;
            }
            Phase::Delivery => {
                for i in 0..MAX_SALE_LINES {
                    if let Some(s) = self.sales_transit[i].take() {
                        let buyer = self
                            .accounts
                            .iter()
                            .position(|a| *a == s.buyer)
                            .ok_or(err(Error::Input))?;
                        self.paid(l, buyer, 2, s.price)?;
                        self.sales = add(self.sales, s.price)?;
                        self.cogs = add(self.cogs, s.fragment.basis)?;
                        let row = self.row();
                        row.sales = add(row.sales, s.price)?;
                        row.cogs = add(row.cogs, s.fragment.basis)?;
                        if buyer == 1 {
                            self.consumed_f = qa(self.consumed_f, s.fragment.quantity)?;
                            let row = self.row();
                            row.f_consumed = qa(row.f_consumed, s.fragment.quantity)?;
                        } else {
                            self.consumed_g = qa(self.consumed_g, s.fragment.quantity)?;
                            let row = self.row();
                            row.g_consumed = qa(row.g_consumed, s.fragment.quantity)?;
                        }
                    }
                }
                self.phase = Phase::DayEnd;
                self.next_due = base + DAY;
            }
            Phase::DayEnd => {
                let profit = i128::from(self.sales.value())
                    - i128::from(self.cogs.value())
                    - i128::from(self.expenses.value());
                let tax =
                    Money::new(i64::try_from(profit.max(0) / 10).map_err(|_| TxError::Overflow)?)
                        .map_err(|_| TxError::Overflow)?;
                self.row().tax_delta_cents = tax.value() - self.tax_accrued.value();
                self.tax_accrued = tax;
                self.closed_days = self.day;
                if usize::from(self.day) == DAYS {
                    self.make_tax_claim()?;
                    if self.claims[2].is_some() {
                        self.settle(2, l)?;
                    }
                    self.phase = Phase::Finished;
                } else {
                    self.day += 1;
                    self.phase = Phase::Open;
                }
            }
            _ => return Err(err(Error::Stage)),
        }
        Ok(())
    }
    pub(crate) fn validate(&self, l: &mut impl Ledger) -> Result<(), TxError> {
        let mut cash = Money::ZERO;
        for id in self.accounts {
            cash = add(cash, l.cash(id)?)?;
        }
        if cash != self.initial_cash {
            return Err(err(Error::Input));
        }
        let source = l.stock(self.supplier_stock)?;
        let raw = l.stock(self.raw_stock)?;
        let rqt = self.raw_transit.map_or(Quantity::ZERO, |f| f.quantity);
        let rbt = self.raw_transit.map_or(Money::ZERO, |f| f.basis);
        if qa(qa(qa(source.quantity, raw.quantity)?, rqt)?, self.raw_used)? != self.initial_raw
            || rqt > q(BATCH_Q)
        {
            return Err(err(Error::Stock));
        }
        let initial_basis = i128::from(self.initial_raw.value()) * 400 / 1000;
        if i128::from(source.basis.value())
            + i128::from(raw.basis.value())
            + i128::from(rbt.value())
            + i128::from(self.raw_used.value()) * 400 / 1000
            != initial_basis
        {
            return Err(err(Error::Stock));
        }
        let mut held = Quantity::ZERO;
        let mut basis = Money::ZERO;
        for id in self.product_stocks {
            let s = l.stock(id)?;
            held = qa(held, s.quantity)?;
            basis = add(basis, s.basis)?;
        }
        let mut transit = Quantity::ZERO;
        for s in self.sales_transit.iter().flatten() {
            held = qa(held, s.fragment.quantity)?;
            transit = qa(transit, s.fragment.quantity)?;
            basis = add(basis, s.fragment.basis)?;
        }
        if transit > q(BATCH_Q) || qa(qa(held, self.consumed_f)?, self.consumed_g)? != self.produced
        {
            return Err(err(Error::Stock));
        }
        if i128::from(basis.value()) + i128::from(self.cogs.value())
            != i128::from(self.raw_used.value()) * 400 / 1000
                + i128::from(self.direct_wages.value())
        {
            return Err(err(Error::Stock));
        }
        let gross = self
            .records
            .iter()
            .try_fold(Money::ZERO, |sum, r| add(sum, r.gross_paid))?;
        if gross != add(self.direct_wages, self.expenses)?
            || self
                .records
                .iter()
                .any(|r| r.worked_hours > 8 || r.freight_q > q(20000))
        {
            return Err(err(Error::Stage));
        }
        Ok(())
    }
    fn offer(
        &mut self,
        l: &mut impl Ledger,
        buyer: usize,
        quantity: Quantity,
    ) -> Result<(), TxError> {
        let amount = m(quantity.value() / 1000 * 2000);
        let mut total = Quantity::ZERO;
        for id in self.product_stocks {
            total = qa(total, l.stock(id)?.quantity)?;
        }
        let reason = if total < quantity {
            Reason::NoStock
        } else if l.available(self.accounts[buyer])? < amount {
            Reason::NoFunds
        } else {
            Reason::Accepted
        };
        if buyer == 1 {
            self.row().f_result = reason;
        } else {
            self.row().g_result = reason;
        }
        if reason != Reason::Accepted {
            return Ok(());
        }
        self.reserve(l, buyer, amount)?;
        let mut left = quantity;
        for id in self.product_stocks {
            if left == Quantity::ZERO {
                break;
            }
            let available = l.stock(id)?.quantity;
            let n = available.min(left);
            if n == Quantity::ZERO {
                continue;
            }
            let f = take(l, id, n)?;
            let slot = self
                .sales_transit
                .iter()
                .position(Option::is_none)
                .ok_or(err(Error::Limit))?;
            self.sales_transit[slot] = Some(Sale {
                buyer: self.accounts[buyer],
                fragment: f,
                price: m(n.value() / 1000 * 2000),
            });
            left = qs(left, n)?;
        }
        let row = self.row();
        row.freight_q = qa(row.freight_q, quantity)?;
        Ok(())
    }
}
fn take(l: &mut impl Ledger, id: u32, quantity: Quantity) -> Result<Fragment, TxError> {
    let mut s = l.stock(id)?;
    let end = qa(s.offset, quantity)?;
    if quantity == Quantity::ZERO
        || quantity > s.quantity
        || s.origin_quantity == Quantity::ZERO
        || end > s.origin_quantity
    {
        return Err(err(Error::Stock));
    }
    let cost = i128::from(s.origin_basis.value()) * i128::from(end.value())
        / i128::from(s.origin_quantity.value())
        - i128::from(s.origin_basis.value()) * i128::from(s.offset.value())
            / i128::from(s.origin_quantity.value());
    let basis = Money::new(i64::try_from(cost).map_err(|_| TxError::Overflow)?)
        .map_err(|_| TxError::Overflow)?;
    let fragment = Fragment {
        lot_id: s.lot_id,
        origin_quantity: s.origin_quantity,
        origin_basis: s.origin_basis,
        offset: s.offset,
        quantity,
        basis,
    };
    s.quantity = qs(s.quantity, quantity)?;
    s.basis = sub(s.basis, basis)?;
    s.offset = end;
    l.put_stock(id, s)?;
    Ok(fragment)
}
fn deposit(l: &mut impl Ledger, id: u32, f: Fragment) -> Result<(), TxError> {
    let mut s = l.stock(id)?;
    if s.quantity != Quantity::ZERO || s.basis != Money::ZERO {
        return Err(err(Error::Stock));
    }
    s.lot_id = f.lot_id;
    s.origin_quantity = f.origin_quantity;
    s.origin_basis = f.origin_basis;
    s.offset = f.offset;
    s.quantity = f.quantity;
    s.basis = f.basis;
    l.put_stock(id, s)
}
