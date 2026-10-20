//! W4 versioned finite business input and direct CPU reference; no endurance claim.
use mycity_core::{
    assembly::assemble,
    background::{self, Command as B},
    boot::{Command as Boot, Facility},
    exchange::Command as X,
    inbox::Command as I,
    schedule::Phase,
    sha256,
    timeline::{Command as T, PayrollSpec},
    transaction::{Action, Caller, Engine, Envelope, Receipt, Resource},
    units::Money,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-background-joint-opening.json");
pub const POLICY: &[u8] = include_bytes!("../../../fixtures/m0-background-joint-policy.json");
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tax {
    pub kind: String,
    pub obligation_id: u32,
    pub payer: u32,
    pub treasury: u32,
    pub amount_cents: i64,
    pub effective_at: u64,
    pub first_due: u64,
    pub period_seconds: u64,
    pub ends_at: u64,
    pub opening_arrears_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Debt {
    pub id: u64,
    pub payer: u32,
    pub payee: u32,
    pub principal_cents: i64,
    pub due: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loan {
    pub claim_id: u64,
    pub lender: u32,
    pub borrower: u32,
    pub principal_cents: i64,
    pub issued_at: u64,
    pub due: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inactive {
    pub city: u32,
    pub switch_to: u32,
    pub closed_at: u64,
    pub opened_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub game_seconds: u64,
    pub minimum_wall_seconds: u64,
    pub maximum_window_seconds: u64,
    pub tax: Tax,
    pub payroll: Vec<PayrollSpec>,
    pub contract: Debt,
    pub loan: Loan,
    pub inactive_city: Inactive,
    pub freight_quotes: Vec<u32>,
    pub inbox_delivery_sequences: Vec<u64>,
}
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn money(n: i64) -> Result<Money, String> {
    Money::new(n).map_err(err)
}
pub fn policy() -> Result<Policy, String> {
    let p: Policy = serde_json::from_slice(POLICY).map_err(err)?;
    if p.schema != "mycity-background-joint-policy-v1"
        || p.game_seconds != 7 * 86400
        || p.minimum_wall_seconds < 1800
        || p.maximum_window_seconds != 3600
        || p.tax.kind != "fixed_daily_business_tax"
        || p.tax.period_seconds != 86400
        || p.tax.first_due != 86400
        || p.tax.effective_at != 0
        || p.tax.ends_at != p.game_seconds
        || p.tax.opening_arrears_cents != 0
        || p.tax.amount_cents <= 0
        || p.payroll.len() != 2
        || p.freight_quotes.len() != 2
        || p.inbox_delivery_sequences != [1, 2]
    {
        return Err("joint_policy_scope".into());
    }
    let world = assemble(OPENING)?;
    let region = world.view().region.ok_or("joint_region")?;
    let tax = region
        .obligations
        .iter()
        .flatten()
        .find(|o| o.id == p.tax.obligation_id)
        .ok_or("joint_tax_mapping")?;
    if tax.payer != p.tax.payer
        || tax.payee != p.tax.treasury
        || tax.amount.value() != p.tax.amount_cents
        || u64::from(tax.period_seconds) != p.tax.period_seconds
        || tax.next_due != Some(p.tax.first_due)
        || tax.expires_at != p.tax.ends_at
        || tax.arrears.value() != p.tax.opening_arrears_cents
    {
        return Err("joint_tax_definition_differs".into());
    }
    Ok(p)
}
pub fn resources() -> Result<Vec<Resource>, String> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
    ]
    .into_iter()
    .map(|(id, n)| Resource::new(id, n).map_err(err))
    .collect()
}
pub fn engine() -> Result<Engine, String> {
    Engine::new(assemble(OPENING)?, resources()?).map_err(err)
}
pub fn setup(p: &Policy) -> Result<Vec<Action>, String> {
    let mut actions = vec![
        Action::Inbox { command: I::Enable },
        Action::Boot {
            command: Boot::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
    ];
    for quote in &p.freight_quotes {
        actions.push(Action::Exchange {
            command: X::Dispatch { quote: *quote },
        });
    }
    actions.push(Action::Timeline { command: T::Enable });
    for spec in &p.payroll {
        actions.push(Action::Timeline {
            command: T::AddPayroll { spec: *spec },
        });
    }
    actions.push(Action::Timeline {
        command: T::Claims {
            first_id: p.contract.id,
            count: 1,
            payer: p.contract.payer,
            payee: p.contract.payee,
            amount: money(p.contract.principal_cents)?,
            due: p.contract.due,
        },
    });
    actions.push(Action::Background { command: B::Enable });
    Ok(actions)
}
pub fn loan(p: &Policy) -> Result<Vec<Action>, String> {
    Ok(vec![Action::Timeline {
        command: T::Lend {
            id: p.loan.claim_id,
            lender: p.loan.lender,
            borrower: p.loan.borrower,
            principal: money(p.loan.principal_cents)?,
            due: p.loan.due,
        },
    }])
}
pub fn business_digest(e: &Engine) -> Result<String, String> {
    Ok(sha256(&e.business_comparison_checkpoint()?))
}
/// Independent finite outcome requirements, in addition to branch equality.
pub fn final_facts(e: &Engine, p: &Policy) -> Result<Value, String> {
    let view = e.world().view();
    let exchange = view.exchange.as_ref().ok_or("joint_exchange")?;
    let mut delivered = Vec::new();
    for id in &p.freight_quotes {
        let q = exchange
            .quotes
            .iter()
            .find(|q| q.spec.id == *id)
            .ok_or("joint_quote")?;
        if q.status != mycity_core::exchange::Status::Delivered
            || q.delivered.value() != 1000
            || q.invoice.value() != 2000
            || q.freight.value() != 20
        {
            return Err("joint_delivery_not_realized".into());
        }
        delivered.push(serde_json::to_value(q).map_err(err)?);
    }
    let boot = view.boot.as_ref().ok_or("joint_boot")?;
    if boot
        .jobs
        .iter()
        .any(|j| j.phase != mycity_core::boot::Phase::Idle || j.completed != 1 || j.failed != 0)
        || boot.fuel_used.value() != 1000
        || boot.fees.value() != 500
    {
        return Err("joint_boot_not_completed".into());
    }
    if e.resources()
        .iter()
        .any(|r| r.reserved != 0 || r.occupied != 0)
    {
        return Err("joint_capacity_not_released".into());
    }
    if exchange.inbox.accrued.value() != 10
        || exchange.inbox.paid.value() != 10
        || exchange.inbox.closed != 2
        || exchange
            .inbox
            .facts
            .iter()
            .chain(exchange.inbox.retained.iter())
            .any(Option::is_some)
    {
        return Err("joint_receiving_responsibility_not_settled".into());
    }
    let opening = assemble(OPENING)?.view();
    let initial_treasury = opening
        .accounts
        .iter()
        .find(|a| a.id == p.tax.treasury)
        .ok_or("joint_treasury")?
        .cash
        .value();
    let treasury = view
        .accounts
        .iter()
        .find(|a| a.id == p.tax.treasury)
        .ok_or("joint_treasury")?
        .cash
        .value();
    let due_count = 1 + (p.tax.ends_at - p.tax.first_due) / p.tax.period_seconds;
    if treasury - initial_treasury
        != i64::try_from(due_count)
            .map_err(err)?
            .checked_mul(p.tax.amount_cents)
            .ok_or("joint_tax_overflow")?
    {
        return Err("joint_treasury_did_not_receive_tax".into());
    }
    for (id, quantity) in [
        (1, 4000),
        (2, 400000),
        (3, 540000),
        (4, 5400000),
        (5, 60000),
        (6, 700000),
        (7, 3000),
        (8, 1000),
        (9, 1000),
    ] {
        if view
            .stocks
            .iter()
            .find(|s| s.id == id)
            .is_none_or(|s| s.quantity.value() != quantity)
        {
            return Err("joint_realized_stock_outputs".into());
        }
    }
    let t = e.timeline().ok_or("joint_timeline")?;
    let contract = t
        .claims
        .iter()
        .find(|c| c.id == p.contract.id)
        .ok_or("joint_contract")?;
    let loan = t
        .claims
        .iter()
        .find(|c| c.id == p.loan.claim_id)
        .ok_or("joint_loan")?;
    if contract.principal.value() != 1000
        || contract.paid != contract.principal
        || !contract.processed
        || loan.principal.value() != 850000
        || loan.paid.value() != 0
        || loan.processed
        || loan.due <= p.game_seconds
    {
        return Err("joint_debt_outcome".into());
    }
    Ok(
        json!({"delivered_quotes":delivered,"boot":boot,"resources":e.resources(),"inbox":exchange.inbox,"paid_contract":contract,"future_loan":loan}),
    )
}
pub struct Reference {
    pub engine: Engine,
    next_operation: u64,
    pub windows: u64,
}
impl Reference {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            engine: engine()?,
            next_operation: 1,
            windows: 0,
        })
    }
    /// Fresh diagnostic reference only; never restores or writes a Store.
    pub fn from_unstarted_engine(engine: Engine) -> Result<Self, String> {
        if engine.world().view().revision != 0
            || !engine.journal().is_empty()
            || engine.timeline().is_some()
            || engine.background().is_some()
        {
            return Err("reference_requires_unstarted_engine".into());
        }
        Ok(Self {
            engine,
            next_operation: 1,
            windows: 0,
        })
    }
    pub fn execute(&mut self, actions: Vec<Action>) -> Result<Receipt, String> {
        if self.engine.journal().len() >= 512 {
            self.engine.retire_checkpointed_journal();
        }
        let view = self.engine.world().view();
        let operation_id = self.next_operation;
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or("joint_operation")?;
        self.engine
            .execute(
                Caller::System,
                Envelope {
                    world: view.scope.world(),
                    branch: view.scope.branch(),
                    operation_id,
                    expected_revision: view.revision,
                    actions,
                },
            )
            .map_err(err)
    }
    pub fn complete_field(&mut self) -> Result<(), String> {
        let job = self
            .engine
            .background()
            .and_then(|b| b.pending())
            .ok_or("joint_no_task")?
            .id;
        self.execute(vec![Action::Background {
            command: B::RebindLease { job },
        }])?;
        let b = self.engine.background().ok_or("joint_no_background")?;
        let generation = b.pending().ok_or("joint_no_task")?.generation;
        let mut state = b.frozen_input()?;
        while !state.complete() {
            state.page(8192, 1024)?;
        }
        let text = sha256(&state.checkpoint()?);
        let mut hash = [0; 32];
        for (i, byte) in hash.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(err)?;
        }
        let view = self.engine.world().view();
        let env = Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: background::accept_operation_id(job)?,
            expected_revision: view.revision,
            actions: vec![Action::Background {
                command: B::Accept {
                    job,
                    generation,
                    candidate_sha256: hash,
                },
            }],
        };
        self.engine
            .execute_recorded_with_background(Caller::System, env, Some(&state), |_, _| {
                Ok::<(), String>(())
            })?
            .map_err(err)?;
        self.windows += 1;
        Ok(())
    }
    pub fn drive(&mut self, target: u64) -> Result<(), String> {
        for _ in 0..10000 {
            let t = self.engine.timeline().ok_or("joint_no_timeline")?;
            if t.progress.target.is_none() && t.progress.last_complete == target {
                return Ok(());
            }
            if t.progress.phase == Some(Phase::P4)
                && self.engine.background().and_then(|b| b.pending()).is_some()
            {
                self.complete_field()?;
            } else {
                self.execute(vec![Action::Timeline {
                    command: T::Advance {
                        target,
                        limit: 8192,
                    },
                }])?;
            }
        }
        Err("joint_reference_limit".into())
    }
    pub fn summary(&self, p: &Policy) -> Result<Value, String> {
        let view = self.engine.world().view();
        let region = view.region.as_ref().ok_or("joint_region")?;
        let tax = region
            .obligations
            .iter()
            .flatten()
            .find(|o| o.id == p.tax.obligation_id)
            .ok_or("joint_tax")?;
        let at = region.interaction_time.min(p.tax.ends_at);
        let occurrences = if at < p.tax.first_due {
            0
        } else {
            1 + (at - p.tax.first_due) / p.tax.period_seconds
        };
        let accrued = i64::try_from(occurrences)
            .map_err(err)?
            .checked_mul(p.tax.amount_cents)
            .ok_or("joint_tax_overflow")?;
        let t = self.engine.timeline().ok_or("joint_timeline")?;
        Ok(
            json!({"time":region.interaction_time,"tax":{"occurrences":occurrences,"accrued_cents":accrued,"arrears_cents":tax.arrears.value(),"paid_cents":accrued-tax.arrears.value()},"payroll":t.payroll,"claims":t.claims,"accounts":view.accounts,"cash_total":view.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),"background_windows":self.windows,"business_sha256":business_digest(&self.engine)?,"air_sha256":self.engine.background().and_then(|b|b.field()).map(|s|s.checkpoint().map(|b|sha256(&b))).transpose()?}),
        )
    }
}
