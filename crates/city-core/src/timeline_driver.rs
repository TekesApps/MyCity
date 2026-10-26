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
    pub service_needs: &'a mut Option<Box<crate::service_needs::Runtime>>,
    pub continuous_supply: &'a mut Option<Box<crate::continuous_supply::Runtime>>,
    pub facility_work: &'a mut Option<Box<crate::facility_authority::Runtime>>,
    pub facility_member: &'a mut Option<Box<crate::member_activity::Ledger>>,
    pub operation_id: u64,
    pub service_working_base: usize,
    pub town: &'a mut Option<crate::town_runtime::MarketState>,
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
    pub housing: &'a mut Option<crate::housing::HousingState>,
    pub housing_move_history: Option<&'a crate::housing_runtime_move::MoveRuntimeState>,
    pub housing_wage_window: &'a mut Option<Box<crate::housing_income::PaidWageWindow>>,
    pub next_token: &'a mut u64,
}
impl Frame<'_, '_> {
    fn sync_rent_claim(
        &mut self,
        source: crate::timeline::RentClaimSource,
        claim: crate::timeline::Claim,
    ) -> Result<(), TxError> {
        // A moved-out lease keeps its original Claim and source so the arrears remain
        // explicitly settleable. Its former unit may now be vacant or leased to another
        // household; do not try to synchronize that historical Claim into the current unit.
        if self
            .housing_move_history
            .and_then(|history| history.historical_source(&claim))
            == Some(source)
        {
            return Ok(());
        }
        if let Some(housing) = self.housing.as_mut() {
            if let Some(unit) = housing
                .units()
                .iter()
                .find(|unit| unit.unit_id == source.unit_id && unit.lease_id == source.lease_id)
                .copied()
            {
                let patch =
                    crate::housing::plan_rent_claim_reference_sync(unit, source.tenant, claim)
                        .map_err(|_| TxError::InvalidOperation)?;
                housing
                    .apply_transaction(&[patch])
                    .map_err(|_| TxError::InvalidOperation)?;
                return Ok(());
            }
        }
        Err(TxError::InvalidOperation)
    }
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
    fn ensure_housing_wage_coverage(&mut self, at: u64) -> Result<(), TxError> {
        if self.world.housing.is_none() {
            return Ok(());
        }
        self.housing_wage_window
            .get_or_insert_with(|| Box::new(crate::housing_income::PaidWageWindow::new()))
            .begin_coverage(at)
            .map_err(|_| TxError::InvalidOperation)
    }
    fn record_household_payroll_payment(
        &mut self,
        payee_account: u32,
        amount: Money,
        at: u64,
    ) -> Result<(), TxError> {
        if amount == Money::ZERO {
            return Ok(());
        }
        let Some(runtime) = self.world.housing.as_deref() else {
            return Ok(());
        };
        if runtime
            .opening
            .households
            .iter()
            .all(|household| household.account != payee_account)
        {
            return Ok(());
        }
        self.housing_wage_window
            .as_mut()
            .ok_or(TxError::InvalidOperation)?
            .record_paid_delta(&runtime.opening, payee_account, amount, at)
            .map_err(|_| TxError::InvalidOperation)
    }
    fn pay(&mut self, payer: u32, payee: u32, due: Money) -> Result<Money, TxError> {
        // Accrual and processing continue, but automatic lanes cannot bypass exit ranking.
        if super::market_is_exiting(self.town.as_ref(), payer) {
            return Ok(Money::ZERO);
        }
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
            let now = self.now();
            slot.activate().reset_at(now);
            self.ensure_housing_wage_coverage(now)?;
            return Ok(());
        }
        let t = slot.as_mut().ok_or(err(E::Input))?;
        if !matches!(command, C::Advance { .. } | C::CancelAdvance) && t.progress.target.is_some() {
            return Err(err(E::Busy));
        }
        self.ensure_housing_wage_coverage(t.progress.last_complete)?;
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
                let equity_company_loan = self.world.town.is_some_and(|market| {
                    market
                        .profile
                        .equity_charters
                        .as_slice()
                        .iter()
                        .any(|charter| charter.company == lender || charter.company == borrower)
                });
                let mut ledger = EconomicLedger {
                    world: self.world,
                    accounts: self.accounts,
                    stocks: self.stocks,
                };
                crate::economy::Ledger::pay(&mut ledger, lender, borrower, principal, false)?;
                let source_ref = if equity_company_loan {
                    t.push_loan_claim_source(
                        id,
                        t::LoanClaimSource {
                            lender,
                            borrower,
                            principal,
                        },
                    )?
                } else {
                    0
                };
                t.claims.push(t::Claim {
                    id,
                    payer: borrower,
                    payee: lender,
                    due,
                    principal,
                    paid: Money::ZERO,
                    processed: false,
                    source_ref,
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
                        source_ref: 0,
                    });
                }
            }
            C::AddPayroll { spec } => {
                crate::facility_authority::check_payroll(self.facility_work.as_deref(),spec.payee,t.progress.last_complete,spec.ends_at,true)?;
                if self.town.as_ref().is_some_and(|state| {
                    crate::transaction::market_is_suspended(Some(state), spec.payer)
                        || crate::transaction::market_is_exiting(Some(state), spec.payer)
                        || crate::transaction::market_is_exiting(Some(state), spec.payee)
                }) {
                    return Err(err(E::Input));
                }
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
            | C::EndForExit { id }
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
                    C::EndForExit { .. } => {
                        let at = t.progress.last_complete;
                        if t.progress.phase.is_some() || t.progress.target.is_some() {
                            return Err(err(E::Busy));
                        }
                        if c.last_accrual != at {
                            return Err(err(E::Cursor));
                        }
                        // A completed, previously stopped payroll is a stable no-op. This
                        // also makes retries after checkpoint recovery safe.
                        if !c.working && c.next_due.is_none() {
                            if c.billed != c.accrued {
                                return Err(err(E::Input));
                            }
                        } else {
                            // Do not terminate or rewrite the contractual end date here. That
                            // remains an unresolved obligation for the exit/legal workflow.
                            if c.next_due.is_none()
                                || c.next_due.is_some_and(|due| due <= at)
                                || c.spec.ends_at <= at
                            {
                                return Err(err(E::Input));
                            }
                            c.working = false;
                            c.next_due = None;
                            c.billed = c.accrued;
                        }
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
                    let payee = c.spec.payee;
                    self.record_household_payroll_payment(payee, paid, t.progress.last_complete)?;
                }
            }
            C::SettleClaim { id } => {
                let index = t
                    .claims
                    .iter()
                    .position(|claim| claim.id == id)
                    .ok_or(err(E::Input))?;
                let before = t.claims[index];
                if !before.processed {
                    return Err(err(E::Input));
                }
                let paid = self.pay(
                    before.payer,
                    before.payee,
                    before
                        .principal
                        .checked_sub(before.paid)
                        .map_err(|_| TxError::Overflow)?,
                )?;
                t.claims[index].paid = before
                    .paid
                    .checked_add(paid)
                    .map_err(|_| TxError::Overflow)?;
                t.progress.page_paid = paid;
                if let Some(crate::timeline::ClaimSource::Rent { details }) =
                    t.source_for_claim(&t.claims[index])
                {
                    self.sync_rent_claim(details, t.claims[index])?;
                }
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
    fn boundary(&mut self, t: &mut Timeline) -> Result<(), TxError> {
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
        if let Some(supply)=self.continuous_supply.as_deref(){
            let mut ledger=ledger!(self);
            if let Some(due)=supply.boundary(&mut ledger,now)?{take(due)?;}
        }
        if let Some(facility)=self.facility_work.as_deref(){
            if let Some(due)=crate::facility_authority::next_boundary(self.world,Some(facility),t,self.continuous_supply.as_deref())?{take(due)?;}
        }
        if self.service_needs.as_deref().is_some_and(|n|n.delivery.is_some()){
            let ledger=ledger!(self);
            if let Some(due)=service_delivery_boundary(self.world,self.service_needs,self.facility_member,ledger,now,self.operation_id)?{take(due)?;}
            if let Some(due)=crate::qualification_authority::next_source_change(self.world,now)?{take(due)?;}
        }
        if let Some(service)=self.service_needs.as_deref() {
            if let Some(due)=service.next_due().map_err(|_|TxError::InvalidOperation)? {take(due)?;}
        }
        for c in &t.claims {
            if !c.processed {
                take(c.due)?;
            }
        }
        if let Some(housing) = self.housing.as_ref() {
            for unit in housing.units().iter().filter(|unit| unit.lease_id != 0) {
                take(u64::try_from(unit.next_due_at.value()).map_err(|_| TxError::Overflow)?)?;
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
            (Some(P::P1), _)
                if self.housing.as_ref().is_some_and(|housing| {
                    housing.units().iter().any(|unit| {
                        unit.lease_id != 0
                            && u64::try_from(unit.next_due_at.value()).ok() == Some(p.at)
                    })
                }) =>
            {
                Class::Claims
            }
            (Some(P::P2), 0) => Class::Claims,
            (Some(P::P2), 1) => Class::Payroll,
            _ => Class::Region,
        };
        let accrual_changed = payroll_accrual && t.payroll[p.cursor as usize].last_accrual < p.at;
        let result = self.one_inner(t);
        let nanos = self.trace.nanos(Some(started));
        let scanned = if p.phase == Some(P::P1) {
            self.housing
                .as_ref()
                .map_or(0, |housing| housing.units().len() as u64)
        } else if p.phase == Some(P::P2) && p.lane <= 1 {
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
                self.accrue_housing_rent_occurrences(t, at)?;
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
                    if let Some(supply)=self.continuous_supply.as_deref_mut(){
                        let root=crate::continuous_supply::source_root(self.world,supply)?;
                        let revision=self.world.revision.checked_add(1).ok_or(TxError::Overflow)?;
                        let mut ledger=ledger!(self);
                        if supply.treatment_device().is_some(){
                            let(f,m)=(self.facility_work.as_deref_mut().ok_or(TxError::InvalidOperation)?,self.facility_member.as_deref_mut().ok_or(TxError::InvalidOperation)?);
                            let before=crate::facility_authority::paid_snapshot(f);
                            crate::water_treatment_authority::advance(self.world,supply,f,m,&mut ledger,at,self.operation_id,root,revision)?;
                            drop(ledger);
                            crate::facility_authority::record_actual_wages(self.world,before,f,self.operation_id,at,self.housing_wage_window)?;
                        }else{
                        if let Some(f)=self.facility_work.as_deref(){
                            if supply.cursor()==t.progress.last_complete&&!crate::facility_authority::can_continue(self.world,f,t.progress.last_complete)?{
                                supply.stop_for_operator(&mut ledger,t.progress.last_complete,self.operation_id,root,revision)?;
                            }
                        }
                        supply.advance(&mut ledger,at,self.operation_id,root,revision)?;
                        if let (Some(f),Some(m))=(self.facility_work.as_deref_mut(),self.facility_member.as_deref_mut()){
                            if !crate::facility_authority::can_continue(self.world,f,at)?{supply.stop_for_operator(&mut ledger,at,self.operation_id,root,revision)?;}
                            let before=crate::facility_authority::paid_snapshot(f);
                            let wage=crate::facility_authority::settle_device(self.world,f,m,supply,self.operation_id,at,&mut ledger)?;
                            supply.book_operator_wage(&mut ledger,wage,self.operation_id)?;
                            drop(ledger);
                            crate::facility_authority::record_actual_wages(self.world,before,f,self.operation_id,at,self.housing_wage_window)?;
                        }
                        }
                    }
                    if !crate::service_needs_authority::due_page(self.service_needs,at,self.operation_id,self.service_working_base,crate::transaction::ENGINE_BYTE_BUDGET)? {return Ok(());}
                    if self.service_needs.as_deref().is_some_and(|n|n.delivery.is_some()){
                        let ledger=ledger!(self);
                        apply_service_delivery(self.world,self.service_needs,self.facility_member,ledger,at,self.operation_id,Caller::System,None,self.housing_wage_window)?;
                    }
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
                    let original = t.claims[i];
                    let rent_source = t.rent_source_for_claim(&original);
                    let rent_due_now = match rent_source {
                        Some(source)
                            if is_historical_rent_claim(
                                self.housing_move_history,
                                &original,
                                source,
                            ) =>
                        {
                            // A moved-out lease retains its original Timeline Claim and source
                            // as historical debt. Its old unit is intentionally vacant, so it
                            // must not pass the active-unit rent-occurrence check. If the old
                            // Claim still has an unprocessed contractual due date, the ordinary
                            // `original.due == at` path below continues to settle it.
                            false
                        }
                        Some(_) => self.rent_occurrence_is_current(t, &original, at)?,
                        None => false,
                    };
                    if !original.processed && (original.due == at || rent_due_now) {
                        if rent_source.is_some() && !rent_due_now {
                            return Err(err(E::Input));
                        }
                        let due = if rent_source.is_some() {
                            original
                                .principal
                                .checked_sub(original.paid)
                                .map_err(|_| TxError::Overflow)?
                        } else {
                            original.principal
                        };
                        let paid = self.pay(original.payer, original.payee, due)?;
                        let c = &mut t.claims[i];
                        c.paid = if rent_source.is_some() {
                            original
                                .paid
                                .checked_add(paid)
                                .map_err(|_| TxError::Overflow)?
                        } else {
                            paid
                        };
                        c.processed = true;
                        t.progress.page_paid = t
                            .progress
                            .page_paid
                            .checked_add(paid)
                            .map_err(|_| TxError::Overflow)?;
                        t.progress.page_sources += 1;
                        if let Some(crate::timeline::ClaimSource::Rent { details }) =
                            t.source_for_claim(&t.claims[i])
                        {
                            self.sync_rent_claim(details, t.claims[i])?;
                        }
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
                        let payee = c.spec.payee;
                        self.record_household_payroll_payment(payee, paid, at)?;
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

    fn rent_occurrence_is_current(
        &self,
        t: &Timeline,
        claim: &crate::timeline::Claim,
        at: u64,
    ) -> Result<bool, TxError> {
        let Some(details) = t.rent_source_for_claim(claim) else {
            return Ok(false);
        };
        let housing = self.housing.as_ref().ok_or(err(E::Input))?;
        let unit = housing
            .units()
            .iter()
            .find(|unit| unit.unit_id == details.unit_id)
            .ok_or(err(E::Input))?;
        if unit.lease_id != details.lease_id
            || u64::from(unit.lease_id) != claim.id
            || unit.owner_account != details.landlord
            || claim.payer != details.tenant
            || claim.payee != details.landlord
        {
            return Err(err(E::Input));
        }
        Ok(
            u64::try_from(unit.last_accounted_at.value()).map_err(|_| TxError::Overflow)? == at
                && u64::try_from(unit.next_due_at.value()).map_err(|_| TxError::Overflow)? > at,
        )
    }

    fn accrue_housing_rent_occurrences(
        &mut self,
        t: &mut Timeline,
        at: u64,
    ) -> Result<(), TxError> {
        let Some(housing) = self.housing.as_mut() else {
            return Ok(());
        };
        let due_at = crate::units::GameTime::new(i64::try_from(at).map_err(|_| TxError::Overflow)?)
            .map_err(|_| err(E::Calendar))?;
        let unit_count = housing.units().len();
        for unit_index in 0..unit_count {
            let unit = housing.units()[unit_index];
            if unit.lease_id == 0
                || u64::try_from(unit.next_due_at.value()).map_err(|_| TxError::Overflow)? != at
            {
                continue;
            }
            let household = self
                .world
                .housing
                .as_ref()
                .ok_or(err(E::Input))?
                .opening
                .households
                .iter()
                .find(|row| row.household_id == unit.household_id)
                .ok_or(err(E::Input))?;
            let claim_id = u64::from(unit.lease_id);
            let claim_index = t.claims.binary_search_by_key(&claim_id, |claim| claim.id);
            let existing = match claim_index {
                Ok(index) => {
                    let claim = t.claims[index];
                    let source = t.rent_source_for_claim(&claim).ok_or(err(E::Input))?;
                    if source.unit_id != unit.unit_id
                        || source.lease_id != unit.lease_id
                        || source.tenant != household.account
                        || source.landlord != unit.owner_account
                    {
                        return Err(err(E::Input));
                    }
                    Some(claim)
                }
                Err(_) => None,
            };
            let plan = crate::housing::plan_rent_occurrence(
                unit,
                household.account,
                unit.owner_account,
                due_at,
                existing,
            )
            .map_err(|_| err(E::Input))?;
            if plan.expected_claim != existing {
                return Err(err(E::Input));
            }
            if let Ok(index) = claim_index {
                t.claims[index] = plan.updated_claim;
            } else {
                t.insert_rent_claim(
                    plan.updated_claim,
                    crate::timeline::RentClaimSource {
                        landlord: unit.owner_account,
                        tenant: household.account,
                        lease_id: unit.lease_id,
                        unit_id: unit.unit_id,
                    },
                )?;
            }
            housing
                .apply_rent_occurrence(plan)
                .map_err(|_| err(E::Input))?;
            t.progress.page_sources += 1;
        }
        Ok(())
    }
}

fn is_historical_rent_claim(
    history: Option<&crate::housing_runtime_move::MoveRuntimeState>,
    claim: &crate::timeline::Claim,
    source: crate::timeline::RentClaimSource,
) -> bool {
    history.and_then(|moves| moves.historical_source(claim)) == Some(source)
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
    validate_housing_rent(engine, t)?;
    Ok(())
}

fn validate_housing_rent(engine: &Engine, timeline: &Timeline) -> Result<(), TxError> {
    let Some(runtime) = engine.world.housing.as_ref() else {
        return if timeline
            .claims
            .iter()
            .any(|claim| timeline.rent_source_for_claim(claim).is_some())
        {
            Err(err(E::Input))
        } else {
            Ok(())
        };
    };
    let progress = timeline.progress;
    for claim in &timeline.claims {
        let Some(source) = timeline.rent_source_for_claim(claim) else {
            continue;
        };
        // Historical rent provenance wins over the current unit index. A moved-out
        // claim keeps its original unit and lease IDs, but that unit may now hold a
        // different active lease. The move-state validator below separately enforces
        // that each retained source is owned by exactly one active or historical row.
        if let Some(history) = runtime
            .moves
            .as_deref()
            .and_then(|moves| moves.historical_source(claim))
        {
            if history != source
                || claim.id != u64::from(source.lease_id)
                || claim.payer != source.tenant
                || claim.payee != source.landlord
                || claim.principal <= Money::ZERO
                || claim.paid > claim.principal
            {
                return Err(err(E::Input));
            }
            continue;
        }
        let Some(unit) = runtime
            .state
            .units()
            .iter()
            .find(|unit| unit.unit_id == source.unit_id && unit.lease_id == source.lease_id)
        else {
            return Err(err(E::Input));
        };
        let household = runtime
            .opening
            .households
            .iter()
            .find(|row| row.household_id == unit.household_id)
            .ok_or(err(E::Input))?;
        let last_accounted =
            u64::try_from(unit.last_accounted_at.value()).map_err(|_| TxError::Overflow)?;
        let next_due = u64::try_from(unit.next_due_at.value()).map_err(|_| TxError::Overflow)?;
        if unit.lease_id == 0
            || unit.lease_id != source.lease_id
            || claim.id != u64::from(unit.lease_id)
            || source.landlord != unit.owner_account
            || source.tenant != household.account
            || claim.payer != source.tenant
            || claim.payee != source.landlord
            || claim.principal <= Money::ZERO
            || claim.paid > claim.principal
            || last_accounted >= next_due
        {
            return Err(err(E::Input));
        }
        if claim.processed {
            continue;
        }
        let claim_due = claim.due;
        if claim_due > progress.at {
            if claim_due != next_due {
                return Err(err(E::Input));
            }
            continue;
        }
        match progress.phase {
            Some(P::P0) if next_due == progress.at && claim_due == progress.at => {}
            Some(P::P1) if next_due == progress.at && claim_due <= progress.at => {}
            Some(P::P2)
                if last_accounted == progress.at
                    && next_due > progress.at
                    && progress.lane == 0
                    && timeline
                        .claims
                        .binary_search_by_key(&claim.id, |row| row.id)
                        .is_ok_and(|index| index >= progress.cursor as usize) => {}
            _ => return Err(err(E::Input)),
        }
    }
    for unit in runtime
        .state
        .units()
        .iter()
        .filter(|unit| unit.lease_id != 0)
    {
        let claim_index = timeline
            .claims
            .binary_search_by_key(&u64::from(unit.lease_id), |claim| claim.id)
            .map_err(|_| err(E::Input))?;
        let source = timeline
            .rent_source_for_claim(&timeline.claims[claim_index])
            .ok_or(err(E::Input))?;
        if source.unit_id != unit.unit_id
            || source.lease_id != unit.lease_id
            || source.landlord != unit.owner_account
        {
            return Err(err(E::Input));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_rent_claim_skips_active_lease_occurrence_validation() {
        use crate::{
            housing_runtime_move::{HistoricalLease, MoveRuntimeState},
            timeline::{Claim, RentClaimSource},
            units::Money,
        };

        let source = RentClaimSource {
            landlord: 4,
            tenant: 2,
            lease_id: 9_001,
            unit_id: 101,
        };
        let claim = Claim {
            id: 9_001,
            payer: source.tenant,
            payee: source.landlord,
            due: crate::timeline::DAY,
            principal: Money::new(10).unwrap(),
            paid: Money::new(4).unwrap(),
            processed: true,
            source_ref: 1,
        };
        let mut history = MoveRuntimeState::default();
        history
            .append_history(HistoricalLease {
                city_id: 1,
                household_id: 501,
                unit_id: source.unit_id,
                lease_id: source.lease_id,
                closed_at: crate::units::GameTime::new(2 * crate::timeline::DAY as i64).unwrap(),
                claim_id: claim.id,
                source,
            })
            .unwrap();

        assert!(is_historical_rent_claim(Some(&history), &claim, source));
        assert!(!is_historical_rent_claim(None, &claim, source));
        let mismatched = RentClaimSource {
            unit_id: source.unit_id + 1,
            ..source
        };
        assert!(!is_historical_rent_claim(
            Some(&history),
            &claim,
            mismatched
        ));
    }
}
