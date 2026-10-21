//! Transaction-local adapters: every page is prepared with the ledger and committed once.
use super::*;
use crate::{
    schedule::Phase as P,
    timeline::{self as t, Command as C, Error as E, Timeline},
};
macro_rules! ledger {
    ($f:expr) => {
        BootLedger {
            base: EconomicLedger {
                world: $f.world,
                accounts: $f.accounts,
                stocks: $f.stocks,
            },
            resources: $f.resources,
            leases: $f.leases,
            next_token: $f.next_token,
        }
    };
}
fn err(e: E) -> TxError {
    TxError::Timeline(e)
}
fn check_time(at: u64) -> Result<(), TxError> {
    crate::calendar::Date::from_time(
        crate::units::GameTime::new(i64::try_from(at).map_err(|_| TxError::Overflow)?)
            .map_err(|_| TxError::Overflow)?,
    )
    .map_err(|_| err(E::Calendar))?;
    Ok(())
}
pub(super) struct Frame<'a, 'd> {
    pub trace: &'a mut crate::diagnostics::Trace<'d>,
    pub world: &'a World,
    pub background: &'a mut crate::model_rows::Resident<crate::background::Background>,
    pub background_work: &'a mut crate::background::Work,
    pub region: &'a mut Option<crate::region::Region>,
    pub economy: &'a mut Option<crate::economy::Economy>,
    pub boot: &'a mut Option<crate::boot::Boot>,
    pub exchange: &'a mut Option<crate::exchange::Exchange>,
    pub accounts: &'a mut DirtyAccounts,
    pub stocks: &'a mut DirtyStocks,
    pub resources: &'a mut ResourceRows,
    pub leases: &'a mut LeaseRows,
    pub next_token: &'a mut u64,
}
impl Frame<'_, '_> {
    fn now(&self) -> u64 {
        self.region
            .as_ref()
            .map(|r| r.interaction_time)
            .or_else(|| self.economy.as_ref().map(|e| e.time))
            .or_else(|| self.boot.as_ref().map(|b| b.time))
            .unwrap_or(0)
    }
    fn parties(&self, payer: u32, payee: u32) -> Result<(), TxError> {
        if payer == payee {
            return Err(err(E::Input));
        }
        self.world
            .account_ref(payer)
            .map_err(|_| TxError::InvalidHandle)?;
        self.world
            .account_ref(payee)
            .map_err(|_| TxError::InvalidHandle)?;
        Ok(())
    }
    fn pay(&mut self, payer: u32, payee: u32, due: Money) -> Result<Money, TxError> {
        let mut l = EconomicLedger {
            world: self.world,
            accounts: self.accounts,
            stocks: self.stocks,
        };
        let paid = crate::economy::Ledger::available(&mut l, payer)?.min(due);
        if paid != Money::ZERO {
            crate::economy::Ledger::pay(&mut l, payer, payee, paid, false)?;
        }
        Ok(paid)
    }
    pub fn command(
        &mut self,
        slot: &mut crate::model_rows::Resident<Timeline>,
        command: C,
    ) -> Result<(), TxError> {
        if matches!(command, C::Enable) {
            if slot.is_some() {
                return Err(err(E::Duplicate));
            }
            slot.activate().reset_at(self.now());
            return Ok(());
        }
        let t = slot.as_mut().ok_or(err(E::Input))?;
        if !matches!(command, C::Advance { .. } | C::CancelAdvance) && t.progress.target.is_some() {
            return Err(err(E::Busy));
        }
        t.progress.page_paid = Money::ZERO;
        t.progress.page_sources = 0;
        match command {
            C::Enable => unreachable!(),
            C::CancelAdvance => {
                if t.progress.phase.is_some() {
                    return Err(err(E::Busy));
                }
                t.progress.target = None;
            }
            C::Lend {
                id,
                lender,
                borrower,
                principal,
                due,
            } => {
                self.parties(lender, borrower)?;
                check_time(due)?;
                if id == 0
                    || due <= t.progress.last_complete
                    || principal == Money::ZERO
                    || t.claims.len() == t::CLAIM_LIMIT
                    || t.claims.last().is_some_and(|c| c.id >= id)
                {
                    return Err(err(E::Input));
                }
                t.claims
                    .try_reserve_exact(1)
                    .map_err(|_| TxError::WorkingLimit)?;
                let mut ledger = EconomicLedger {
                    world: self.world,
                    accounts: self.accounts,
                    stocks: self.stocks,
                };
                crate::economy::Ledger::pay(&mut ledger, lender, borrower, principal, false)?;
                t.claims.push(t::Claim {
                    id,
                    payer: borrower,
                    payee: lender,
                    due,
                    principal,
                    paid: Money::ZERO,
                    processed: false,
                });
            }
            C::Claims {
                first_id,
                count,
                payer,
                payee,
                amount,
                due,
            } => {
                self.parties(payer, payee)?;
                check_time(due)?;
                let end = first_id
                    .checked_add(u64::from(count))
                    .ok_or(TxError::Overflow)?;
                if first_id == 0
                    || count == 0
                    || t.claims.len() + count as usize > t::CLAIM_LIMIT
                    || due <= t.progress.last_complete
                    || amount == Money::ZERO
                    || t.claims.last().is_some_and(|c| c.id >= first_id)
                {
                    return Err(err(E::Input));
                }
                let total = i128::from(amount.value()) * i128::from(count);
                if total > i128::from(i64::MAX) {
                    return Err(TxError::Overflow);
                }
                t.claims
                    .try_reserve_exact(count as usize)
                    .map_err(|_| TxError::WorkingLimit)?;
                for id in first_id..end {
                    t.claims.push(t::Claim {
                        id,
                        payer,
                        payee,
                        due,
                        principal: amount,
                        paid: Money::ZERO,
                        processed: false,
                    });
                }
            }
            C::AddPayroll { spec } => {
                self.parties(spec.payer, spec.payee)?;
                check_time(spec.ends_at)?;
                if self
                    .world
                    .accounts
                    .get(
                        self.world
                            .account_handle(spec.payee)
                            .map_err(|_| TxError::InvalidHandle)?,
                    )
                    .map_err(|_| TxError::InvalidHandle)?
                    .role
                    != crate::assembly::Role::Household
                {
                    return Err(err(E::Input));
                }
                if self.region.is_some() && spec.home_city == 0 {
                    return Err(err(E::Input));
                }
                // This roster is independent of pe0's own labor; it may not double-pay that profile.
                if self.economy.is_some()
                    || t.payroll.len() == t::PAYROLL_LIMIT
                    || t.payroll.last().is_some_and(|c| c.spec.id >= spec.id)
                {
                    return Err(err(E::Input));
                }
                if spec.home_city != 0
                    && !self
                        .region
                        .as_ref()
                        .is_some_and(|r| r.cities.iter().flatten().any(|c| c.id == spec.home_city))
                {
                    return Err(err(E::Input));
                }
                t.payroll
                    .try_reserve_exact(1)
                    .map_err(|_| TxError::WorkingLimit)?;
                t.payroll
                    .push(t::Payroll::new(spec, t.progress.last_complete)?);
            }
            C::StopWork { id }
            | C::ChangeCycle { id, .. }
            | C::Terminate { id }
            | C::SettlePayroll { id } => {
                let c = t
                    .payroll
                    .iter_mut()
                    .find(|c| c.spec.id == id)
                    .ok_or(err(E::Input))?;
                match command {
                    C::StopWork { .. } => c.working = false,
                    C::ChangeCycle { cycle, .. } => {
                        cycle.next(t.progress.last_complete)?;
                        if c.next_due.is_none() {
                            return Err(err(E::Input));
                        }
                        c.pending_cycle = Some(cycle);
                    }
                    C::Terminate { .. } => {
                        c.working = false;
                        c.spec.ends_at = t.progress.last_complete;
                        c.next_due = None;
                        c.billed = c.accrued;
                    }
                    _ => {}
                }
                if matches!(command, C::SettlePayroll { .. } | C::Terminate { .. }) {
                    let paid = self.pay(
                        c.spec.payer,
                        c.spec.payee,
                        c.billed
                            .checked_sub(c.paid)
                            .map_err(|_| TxError::Overflow)?,
                    )?;
                    c.paid = c.paid.checked_add(paid).map_err(|_| TxError::Overflow)?;
                    t.progress.page_paid = paid;
                }
            }
            C::SettleClaim { id } => {
                let c = t
                    .claims
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or(err(E::Input))?;
                if !c.processed {
                    return Err(err(E::Input));
                }
                let paid = self.pay(
                    c.payer,
                    c.payee,
                    c.principal
                        .checked_sub(c.paid)
                        .map_err(|_| TxError::Overflow)?,
                )?;
                c.paid = c.paid.checked_add(paid).map_err(|_| TxError::Overflow)?;
                t.progress.page_paid = paid;
            }
            C::Advance { target, limit } => {
                if limit == 0
                    || limit > t::PAGE_LIMIT
                    || target <= t.progress.last_complete
                    || t.progress.target.is_some_and(|old| old != target)
                {
                    return Err(err(E::Cursor));
                }
                crate::calendar::Date::from_time(
                    crate::units::GameTime::new(
                        i64::try_from(target).map_err(|_| TxError::Overflow)?,
                    )
                    .map_err(|_| TxError::Overflow)?,
                )
                .map_err(|_| err(E::Calendar))?;
                if t.progress.phase.is_none() {
                    t.progress.target = Some(target);
                    self.boundary(t)?;
                }
                for page_step in 0..limit {
                    if t.progress.phase == Some(P::P4)
                        && self.background.as_ref().is_some_and(|b| {
                            u64::from(b.time()) != t.progress.at || b.pending().is_some()
                        })
                    {
                        if page_step == 0 {
                            return Err(TxError::BackgroundRequired);
                        }
                        break;
                    }
                    self.one(t)?;
                    if t.progress.phase.is_none() {
                        break;
                    }
                }
                t.progress.serial = t.progress.serial.checked_add(1).ok_or(TxError::Overflow)?;
            }
        }
        t.validate()?;
        let expected = if t.progress.phase == Some(P::P0) {
            t.progress.last_complete
        } else {
            t.progress.at
        };
        if (self.region.is_some() || self.economy.is_some() || self.boot.is_some())
            && self.now() != expected
        {
            return Err(err(E::Clock));
        }
        Ok(())
    }
    fn boundary(&self, t: &mut Timeline) -> Result<(), TxError> {
        let now = t.progress.last_complete;
        let mut at = t
            .progress
            .target
            .ok_or(err(E::Cursor))?
            .min(now.checked_add(3600).ok_or(TxError::Overflow)?);
        let mut take = |due: u64| -> Result<(), TxError> {
            if due < now {
                return Err(err(E::CausalOrder));
            }
            at = at.min(due);
            Ok(())
        };
        for c in &t.claims {
            if !c.processed {
                take(c.due)?;
            }
        }
        for c in &t.payroll {
            if let Some(due) = c.next_due {
                take(due)?;
            }
        }
        if let Some(e) = self.economy.as_ref() {
            if !matches!(
                e.phase,
                crate::economy::Phase::Finished
                    | crate::economy::Phase::Closing
                    | crate::economy::Phase::Closed
            ) {
                take(e.next_due)?;
            }
        }
        if let Some(b) = self.boot.as_ref() {
            for j in &b.jobs {
                if j.phase == crate::boot::Phase::Running {
                    take(j.due)?;
                }
            }
        }
        if let Some(r) = self.region.as_ref() {
            for o in r.obligations.iter().flatten() {
                if let Some(due) = o.next_due {
                    take(due)?;
                }
            }
            for p in r.transits.iter().flatten() {
                if p.pause_remaining.is_none() {
                    take(p.arrives_at)?;
                }
            }
        }
        if let Some(e) = self.exchange.as_ref() {
            for s in e.shipments.iter().flatten() {
                if let Some(due) = s.arrives_at {
                    take(due)?;
                }
            }
            for q in &e.quotes {
                if q.status == crate::exchange::Status::Pending {
                    take(q.spec.expires_at)?;
                }
            }
        }
        t.progress.at = at;
        t.progress.phase = Some(P::P0);
        t.progress.cursor = 0;
        t.progress.lane = 0;
        Ok(())
    }
    fn economy_event(&mut self, t: &mut Timeline) -> Result<(), TxError> {
        self.economy
            .as_mut()
            .ok_or(err(E::Input))?
            .event(&mut EconomicLedger {
                world: self.world,
                accounts: self.accounts,
                stocks: self.stocks,
            })?;
        t.progress.page_sources += 1;
        Ok(())
    }
    fn economy_due(&self, t: &Timeline, phases: &[crate::economy::Phase]) -> bool {
        self.economy
            .as_ref()
            .is_some_and(|e| e.next_due == t.progress.at && phases.contains(&e.phase))
    }
    fn one(&mut self, t: &mut Timeline) -> Result<(), TxError> {
        use crate::diagnostics::Class;
        let Some(started) = self.trace.start() else {
            return self.one_inner(t);
        };
        let p = t.progress;
        let payroll_accrual = p.phase == Some(P::P0) && (p.cursor as usize) < t.payroll.len();
        let class = match (p.phase, p.lane) {
            (Some(P::P0), _) if payroll_accrual => Class::Payroll,
            (Some(P::P2), 0) => Class::Claims,
            (Some(P::P2), 1) => Class::Payroll,
            _ => Class::Region,
        };
        let accrual_changed = payroll_accrual && t.payroll[p.cursor as usize].last_accrual < p.at;
        let result = self.one_inner(t);
        let nanos = self.trace.nanos(Some(started));
        let scanned = if p.phase == Some(P::P2) && p.lane <= 1 {
            let len = if p.lane == 0 {
                t.claims.len()
            } else {
                t.payroll.len()
            };
            let end = if t.progress.lane != p.lane || t.progress.phase != p.phase {
                len as u32
            } else {
                t.progress.cursor
            };
            u64::from(end.saturating_sub(p.cursor))
        } else {
            u64::from(payroll_accrual)
        };
        let work = u64::from(t.progress.page_sources.saturating_sub(p.page_sources))
            + u64::from(accrual_changed && result.is_ok())
            + u64::from(
                p.phase == Some(P::P0)
                    && !payroll_accrual
                    && p.at > p.last_complete
                    && self.region.is_some()
                    && result.is_ok(),
            );
        let paid = t
            .progress
            .page_paid
            .value()
            .saturating_sub(p.page_paid.value()) as u64;
        let role = self.trace.role.index();
        if class == Class::Payroll && work > 0 {
            self.trace.summary.payroll_subtypes[role][usize::from(!payroll_accrual)] += work;
        }
        if class == Class::Region && work > 0 {
            let subtype = match (p.phase, p.lane) {
                (Some(P::P0), _) => 0,
                (Some(P::P1), _) => 1,
                (Some(P::P2), 2) => 2,
                (Some(P::P2), 4) => 3,
                (Some(P::P2), 5) => 4,
                (Some(P::P3), _) => 5,
                _ => 6,
            };
            self.trace.summary.region_subtypes[role][subtype] += work;
        }
        self.trace.add(class, nanos, scanned, work, paid);
        result
    }
    fn one_inner(&mut self, t: &mut Timeline) -> Result<(), TxError> {
        use crate::economy::Phase as EP;
        let p = t.progress;
        let at = p.at;
        match p.phase.ok_or(err(E::Cursor))? {
            P::P0 => {
                if (p.cursor as usize) < t.payroll.len() {
                    let c = &mut t.payroll[p.cursor as usize];
                    c.accrue(at, self.region.as_ref().map(|r| r.active_city))?;
                    t.progress.cursor += 1;
                } else {
                    let delta = at - p.last_complete;
                    if let Some(b) = self.background.as_mut().filter(|_| delta > 0) {
                        let started = self.trace.start();
                        let capture = b
                            .capture(
                                self.exchange.as_ref().ok_or(TxError::InvalidOperation)?,
                                p.last_complete,
                                at,
                                self.world
                                    .revision
                                    .checked_add(1)
                                    .ok_or(TxError::Overflow)?,
                                self.background_work,
                            )
                            .map_err(|_| TxError::InvalidOperation);
                        self.trace.end(
                            crate::diagnostics::Class::Capture,
                            started,
                            0,
                            u64::from(capture.is_ok()),
                            0,
                        );
                        capture?;
                    }
                    if let Some(r) = self.region.as_mut() {
                        if delta > 0 {
                            r.bump().map_err(TxError::Region)?;
                            r.interaction_time = at;
                            let c = r
                                .cities
                                .iter_mut()
                                .flatten()
                                .find(|c| c.id == r.active_city)
                                .ok_or(err(E::Input))?;
                            c.development_elapsed = c
                                .development_elapsed
                                .checked_add(delta)
                                .ok_or(TxError::Overflow)?;
                        }
                    }
                    if let Some(b) = self.boot.as_mut() {
                        b.time = at;
                    }
                    if let Some(e) = self.economy.as_mut() {
                        e.time = at;
                    }
                    // Work ending at T belongs to the old interval; it can create a same-T wage source.
                    if self.economy_due(t, &[EP::Work]) {
                        self.economy_event(t)?;
                    }
                    if self.economy_due(t, &[EP::DayEnd]) {
                        self.economy
                            .as_mut()
                            .ok_or(err(E::Input))?
                            .accrue_day_tax()?;
                    }
                    t.progress.phase = Some(P::P1);
                    t.progress.cursor = 0;
                }
            }
            P::P1 => {
                if let Some(e) = self.exchange.as_mut() {
                    for q in &mut e.quotes {
                        if q.status == crate::exchange::Status::Pending && q.spec.expires_at <= at {
                            q.status = crate::exchange::Status::Expired;
                            t.progress.page_sources += 1;
                        }
                    }
                }
                t.progress.phase = Some(P::P2);
            }
            P::P2 => self.rights(t)?,
            P::P3 => {
                if let Some(r) = self.region.as_mut() {
                    if let Some(i) = r.transits.iter().position(|v| {
                        v.is_some_and(|v| v.pause_remaining.is_none() && v.arrives_at <= at)
                    }) {
                        let transit = r.transits[i].ok_or(err(E::Cursor))?;
                        if self.exchange.is_some() {
                            crate::exchange::Ledger::release_path(
                                &mut ledger!(self),
                                transit.lease_tokens,
                            )?;
                        }
                        let c = r
                            .cities
                            .iter_mut()
                            .flatten()
                            .find(|c| c.id == transit.to)
                            .ok_or(err(E::Input))?;
                        c.population = c
                            .population
                            .checked_add(transit.actual)
                            .ok_or(TxError::Overflow)?;
                        r.transits[i] = None;
                        r.bump().map_err(TxError::Region)?;
                        t.progress.page_sources += 1;
                        return Ok(());
                    }
                }
                t.progress.phase = Some(P::P4);
            }
            P::P4 => {
                if self.economy_due(t, &[EP::DayEnd]) {
                    self.economy.as_mut().ok_or(err(E::Input))?.close_day();
                    t.progress.page_sources += 1;
                } else {
                    t.progress.phase = Some(P::P5);
                }
            }
            P::P5 => {
                if self.economy_due(t, &[EP::Offers])
                    || (at < t.progress.target.ok_or(err(E::Cursor))?
                        && self.economy_due(t, &[EP::Open]))
                {
                    self.economy_event(t)?;
                } else {
                    t.progress.last_complete = at;
                    t.progress.phase = None;
                    t.progress.cursor = 0;
                    t.progress.lane = 0;
                    if Some(at) == t.progress.target {
                        t.progress.target = None;
                    }
                }
            }
        }
        Ok(())
    }
    fn rights(&mut self, t: &mut Timeline) -> Result<(), TxError> {
        use crate::economy::Phase as EP;
        let p = t.progress;
        let at = p.at;
        match p.lane {
            0 => {
                while (t.progress.cursor as usize) < t.claims.len() {
                    let i = t.progress.cursor as usize;
                    t.progress.cursor += 1;
                    let c = &mut t.claims[i];
                    if !c.processed && c.due == at {
                        let paid = self.pay(c.payer, c.payee, c.principal)?;
                        c.paid = paid;
                        c.processed = true;
                        t.progress.page_paid = t
                            .progress
                            .page_paid
                            .checked_add(paid)
                            .map_err(|_| TxError::Overflow)?;
                        t.progress.page_sources += 1;
                        return Ok(());
                    }
                }
            }
            1 => {
                while (t.progress.cursor as usize) < t.payroll.len() {
                    let i = t.progress.cursor as usize;
                    t.progress.cursor += 1;
                    let c = &mut t.payroll[i];
                    if c.next_due == Some(at) {
                        c.bill(at)?;
                        let paid = self.pay(
                            c.spec.payer,
                            c.spec.payee,
                            c.billed
                                .checked_sub(c.paid)
                                .map_err(|_| TxError::Overflow)?,
                        )?;
                        c.paid = c.paid.checked_add(paid).map_err(|_| TxError::Overflow)?;
                        t.progress.page_paid = t
                            .progress
                            .page_paid
                            .checked_add(paid)
                            .map_err(|_| TxError::Overflow)?;
                        t.progress.page_sources += 1;
                        return Ok(());
                    }
                }
            }
            2 => {
                if let Some(r) = self.region.as_mut() {
                    if let Some(o) = r
                        .obligations
                        .iter_mut()
                        .flatten()
                        .find(|o| o.next_due == Some(at))
                    {
                        o.arrears = o
                            .arrears
                            .checked_add(o.amount)
                            .map_err(|_| TxError::Overflow)?;
                        let old = o.arrears;
                        settle_obligation(self.world, self.accounts, o)?;
                        let paid = old.checked_sub(o.arrears).map_err(|_| TxError::Overflow)?;
                        t.progress.page_paid = t
                            .progress
                            .page_paid
                            .checked_add(paid)
                            .map_err(|_| TxError::Overflow)?;
                        o.next_due = at
                            .checked_add(u64::from(o.period_seconds))
                            .filter(|n| *n <= o.expires_at);
                        t.progress.page_sources += 1;
                        return Ok(());
                    }
                }
            }
            3 => {
                if self.economy_due(t, &[EP::DayEnd]) {
                    let e = self.economy.as_mut().ok_or(err(E::Input))?;
                    if usize::from(e.day) == crate::economy::DAYS && e.claims[2].is_none() {
                        e.pay_day_tax(&mut EconomicLedger {
                            world: self.world,
                            accounts: self.accounts,
                            stocks: self.stocks,
                        })?;
                        t.progress.page_sources += 1;
                    }
                }
                if self.economy_due(t, &[EP::Wage]) {
                    self.economy_event(t)?;
                    return Ok(());
                }
            }
            4 => {
                if let Some(b) = self.boot.as_mut() {
                    if let Some(i) = b
                        .jobs
                        .iter()
                        .position(|j| j.phase == crate::boot::Phase::Running && j.due == at)
                    {
                        b.finish(i, false, &mut ledger!(self))?;
                        t.progress.page_sources += 1;
                        return Ok(());
                    }
                }
                if self.economy_due(t, &[EP::RawArrival, EP::Produce]) {
                    self.economy_event(t)?;
                    return Ok(());
                }
            }
            5 => {
                if let Some(e) = self.exchange.as_mut() {
                    if let Some(i) = e
                        .shipments
                        .iter()
                        .position(|s| s.is_some_and(|s| s.arrives_at == Some(at)))
                    {
                        e.arrive_one(
                            self.region.as_mut().ok_or(err(E::Input))?,
                            &mut ledger!(self),
                            i,
                        )?;
                        t.progress.page_sources += 1;
                        return Ok(());
                    }
                }
                if self.economy_due(t, &[EP::Delivery]) {
                    self.economy_event(t)?;
                    return Ok(());
                }
            }
            _ => return Err(err(E::Cursor)),
        }
        t.progress.cursor = 0;
        if p.lane == 5 {
            t.progress.phase = Some(P::P3);
            t.progress.lane = 0;
        } else {
            t.progress.lane += 1;
        }
        Ok(())
    }
}
pub(super) fn validate(engine: &Engine, t: &Timeline) -> Result<(), TxError> {
    t.validate()?;
    let p = t.progress;
    let expected = if p.phase == Some(P::P0) {
        p.last_complete
    } else {
        p.at
    };
    let actual = engine
        .world
        .region
        .as_ref()
        .map(|r| r.interaction_time)
        .or_else(|| engine.world.economy.as_ref().map(|e| e.time))
        .or_else(|| engine.world.boot.as_ref().map(|b| b.time));
    if actual.is_some_and(|time| time != expected) {
        return Err(err(E::Clock));
    }
    for c in &t.claims {
        engine
            .world
            .account_ref(c.payer)
            .map_err(|_| TxError::InvalidHandle)?;
        engine
            .world
            .account_ref(c.payee)
            .map_err(|_| TxError::InvalidHandle)?;
    }
    for (i, c) in t.payroll.iter().enumerate() {
        engine
            .world
            .account_ref(c.spec.payer)
            .map_err(|_| TxError::InvalidHandle)?;
        engine
            .world
            .account_ref(c.spec.payee)
            .map_err(|_| TxError::InvalidHandle)?;
        let last = if p.phase == Some(P::P0) && i >= p.cursor as usize {
            p.last_complete
        } else {
            p.at
        };
        if c.last_accrual != last {
            return Err(err(E::Cursor));
        }
    }
    Ok(())
}
