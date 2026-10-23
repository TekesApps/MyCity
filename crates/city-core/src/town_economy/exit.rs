//! Bounded, read-only business exit planning. The Engine remains the only settlement writer.
use super::{AccountSnapshot, InventorySnapshot};
use crate::units::Money;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_EXIT_LIABILITIES: usize = 64;
pub const MAX_EXIT_PAYROLLS: usize = 64;
pub const MAX_EXIT_INVENTORY_LOTS: usize = 128;
pub const MAX_EXIT_SHAREHOLDERS: usize = 64;
pub const MAX_EXIT_ACTIONS: usize =
    MAX_EXIT_LIABILITIES + MAX_EXIT_PAYROLLS + MAX_EXIT_INVENTORY_LOTS + 4;
pub const EXIT_PLANNER_HEAP_BUDGET: usize = 64 * 1024;

struct BoundedRows<T, const LIMIT: usize>(Vec<T>);

impl<'de, T: Deserialize<'de>, const LIMIT: usize> Deserialize<'de> for BoundedRows<T, LIMIT> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RowsVisitor<T, const LIMIT: usize>(std::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de>, const LIMIT: usize> Visitor<'de> for RowsVisitor<T, LIMIT> {
            type Value = BoundedRows<T, LIMIT>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "at most {LIMIT} bounded rows")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let initial = sequence.size_hint().unwrap_or(0).min(LIMIT);
                let mut rows = Vec::with_capacity(initial);
                while let Some(row) = sequence.next_element()? {
                    if rows.len() == LIMIT {
                        return Err(serde::de::Error::custom("exit_row_limit"));
                    }
                    rows.push(row);
                }
                Ok(BoundedRows(rows))
            }
        }

        deserializer.deserialize_seq(RowsVisitor::<T, LIMIT>(std::marker::PhantomData))
    }
}

macro_rules! bounded_rows_deserializer {
    ($name:ident, $row:ty, $limit:expr) => {
        fn $name<'de, D>(deserializer: D) -> Result<Vec<$row>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            BoundedRows::<$row, $limit>::deserialize(deserializer).map(|rows| rows.0)
        }
    };
}

bounded_rows_deserializer!(deserialize_liabilities, ExitLiability, MAX_EXIT_LIABILITIES);
bounded_rows_deserializer!(deserialize_payroll, ExitPayroll, MAX_EXIT_PAYROLLS);
bounded_rows_deserializer!(
    deserialize_inventory,
    ExitInventoryLot,
    MAX_EXIT_INVENTORY_LOTS
);
bounded_rows_deserializer!(deserialize_shareholders, Shareholder, MAX_EXIT_SHAREHOLDERS);

fn deserialize_schema<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let schema = String::deserialize(deserializer)?;
    if schema.len() > 64 {
        return Err(serde::de::Error::custom("exit_schema_too_long"));
    }
    Ok(schema)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimClass {
    Tax,
    Wage,
    SecuredDebt,
    UnsecuredDebt,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithinTierRule {
    ProRataByOutstandingStableId,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitPriorityPolicy {
    pub order: [ClaimClass; 5],
    /// Must be explicitly reviewed; unsupported jurisdiction rules fail closed.
    pub within_tier: WithinTierRule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum LiabilitySource {
    TimelineClaim { id: u64 },
    PayrollArrears { id: u32 },
}

impl LiabilitySource {
    fn order_key(self) -> (u8, u64) {
        match self {
            Self::TimelineClaim { id } => (0, id),
            Self::PayrollArrears { id } => (1, u64::from(id)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitLiability {
    pub source: LiabilitySource,
    pub class: ClaimClass,
    pub payee: u32,
    pub due_at: u64,
    /// Outstanding cents after every previously posted payment.
    pub outstanding: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitPayroll {
    pub id: u32,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitInventoryLot {
    pub inventory: InventorySnapshot,
    /// True only when this existing lot can enter the ordinary market without new production,
    /// a new purchase, or an unmodeled transport/permit obligation.
    pub sale_ready: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shareholder {
    pub account: u32,
    pub shares: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalClearance {
    Unknown,
    Denied,
    Cleared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitTrigger {
    Voluntary,
    InsolvencyWorkflow,
}

/// One immutable, bounded snapshot assembled from Engine-owned state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitSnapshot {
    #[serde(deserialize_with = "deserialize_schema")]
    pub schema: String,
    pub world: u64,
    pub branch: u64,
    pub revision: u64,
    pub day: u64,
    pub operation_id: u64,
    pub company: u32,
    pub company_account: AccountSnapshot,
    pub trigger: ExitTrigger,
    /// Complete jurisdiction-reviewed priority order and same-tier allocation rule.
    pub priority_policy: Option<ExitPriorityPolicy>,
    pub legal_clearance: LegalClearance,
    pub claims_complete: bool,
    pub tax_assessment_complete: bool,
    pub asset_register_complete: bool,
    pub employment_exit_terms_complete: bool,
    #[serde(deserialize_with = "deserialize_liabilities")]
    pub liabilities: Vec<ExitLiability>,
    #[serde(deserialize_with = "deserialize_payroll")]
    pub payroll: Vec<ExitPayroll>,
    #[serde(deserialize_with = "deserialize_inventory")]
    pub inventory: Vec<ExitInventoryLot>,
    pub shareholders_complete: bool,
    #[serde(deserialize_with = "deserialize_shareholders")]
    pub shareholders: Vec<Shareholder>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExitAction {
    StopNewProduction {
        company: u32,
    },
    StopNewInputs {
        company: u32,
    },
    CancelFuturePayroll {
        payroll_id: u32,
    },
    SettleLiability {
        source: LiabilitySource,
        payee: u32,
        amount: Money,
    },
    OfferExistingInventory {
        stock_id: u32,
        quantity: crate::units::Quantity,
    },
    DistributeResidual {
        shareholder: u32,
        amount: Money,
    },
    FinalizeCompanyExit {
        company: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitStatus {
    Blocked,
    ClaimsUnpaid,
    WaitingForFutureClaims,
    WaitingForInventorySale,
    ReadyToClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitBlock {
    MissingPriorityPolicy,
    IncompleteClaims,
    IncompleteTaxAssessment,
    IncompleteAssetRegister,
    IncompleteEmploymentTerms,
    LegalClearanceUnknown,
    LegalClearanceDenied,
    MissingShareholderRegister,
    InvalidShareholderRegister,
    InventoryNotSaleReady,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitPlan {
    pub world: u64,
    pub branch: u64,
    pub expected_revision: u64,
    pub day: u64,
    pub operation_id: u64,
    pub trigger: ExitTrigger,
    pub status: ExitStatus,
    pub blocked: Option<ExitBlock>,
    /// Company cash above its protected floor at the start of this plan.
    pub cash_for_due_claims: Money,
    /// Spendable cash after proposed due-claim settlement and before any residual distribution.
    pub cash_after_claims: Money,
    pub unpaid_due_liabilities: Money,
    pub future_liabilities: Money,
    pub actions: Vec<ExitAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitError {
    InvalidSnapshot,
    Limit,
    DuplicateIdentity,
    Overflow,
}

fn add(a: Money, b: Money) -> Result<Money, ExitError> {
    a.checked_add(b).map_err(|_| ExitError::Overflow)
}

fn sub(a: Money, b: Money) -> Result<Money, ExitError> {
    a.checked_sub(b).map_err(|_| ExitError::Overflow)
}

fn class_index(class: ClaimClass) -> usize {
    match class {
        ClaimClass::Tax => 0,
        ClaimClass::Wage => 1,
        ClaimClass::SecuredDebt => 2,
        ClaimClass::UnsecuredDebt => 3,
        ClaimClass::Other => 4,
    }
}

fn valid_priority(policy: &ExitPriorityPolicy) -> bool {
    let all = [
        ClaimClass::Tax,
        ClaimClass::Wage,
        ClaimClass::SecuredDebt,
        ClaimClass::UnsecuredDebt,
        ClaimClass::Other,
    ];
    all.iter().all(|class| policy.order.contains(class))
        && policy.within_tier == WithinTierRule::ProRataByOutstandingStableId
}

fn validate(snapshot: &ExitSnapshot) -> Result<(), ExitError> {
    if snapshot.schema != "mycity-business-exit-v0.1"
        || snapshot.world == 0
        || snapshot.branch == 0
        || snapshot.revision == 0
        || snapshot.day == 0
        || snapshot.operation_id == 0
        || matches!(
            snapshot.operation_id & 0xf000_0000_0000_0000,
            0xd000_0000_0000_0000 | 0xe000_0000_0000_0000
        )
        || snapshot.company == 0
        || snapshot.company_account.account == 0
        || snapshot.company_account.cash < snapshot.company_account.protected
    {
        return Err(ExitError::InvalidSnapshot);
    }
    if snapshot.liabilities.len() > MAX_EXIT_LIABILITIES
        || snapshot.payroll.len() > MAX_EXIT_PAYROLLS
        || snapshot.inventory.len() > MAX_EXIT_INVENTORY_LOTS
        || snapshot.shareholders.len() > MAX_EXIT_SHAREHOLDERS
    {
        return Err(ExitError::Limit);
    }
    let mut previous_by_class: [Option<(u8, u64)>; 5] = [None; 5];
    for (index, liability) in snapshot.liabilities.iter().enumerate() {
        if liability.payee == 0
            || liability.payee == snapshot.company_account.account
            || liability.outstanding == Money::ZERO
            || liability.source.order_key().1 == 0
        {
            return Err(ExitError::InvalidSnapshot);
        }
        if snapshot.liabilities[..index]
            .iter()
            .any(|prior| prior.source == liability.source)
        {
            return Err(ExitError::DuplicateIdentity);
        }
        let slot = class_index(liability.class);
        let key = liability.source.order_key();
        if previous_by_class[slot].is_some_and(|prior| prior > key) {
            return Err(ExitError::InvalidSnapshot);
        }
        previous_by_class[slot] = Some(key);
    }
    if snapshot
        .payroll
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
        || snapshot
            .inventory
            .windows(2)
            .any(|pair| pair[0].inventory.stock_id >= pair[1].inventory.stock_id)
        || snapshot
            .shareholders
            .windows(2)
            .any(|pair| pair[0].account >= pair[1].account)
    {
        return Err(ExitError::InvalidSnapshot);
    }
    if snapshot.payroll.iter().any(|row| row.id == 0)
        || snapshot.inventory.iter().any(|row| {
            let inventory = row.inventory;
            inventory.owner != snapshot.company_account.account
                || inventory.stock_id == 0
                || if inventory.quantity == crate::units::Quantity::ZERO {
                    inventory.spendable_basis != Money::ZERO
                        || inventory.origin_quantity != crate::units::Quantity::ZERO
                        || inventory.origin_basis != Money::ZERO
                        || inventory.offset != crate::units::Quantity::ZERO
                } else {
                    inventory.origin_quantity == crate::units::Quantity::ZERO
                        || super::basis_slice(
                            inventory.origin_basis,
                            inventory.origin_quantity,
                            inventory.offset,
                            inventory.quantity,
                        )
                        .ok()
                            != Some(inventory.spendable_basis)
                }
        })
        || snapshot.shareholders.iter().any(|row| {
            row.account == 0 || row.account == snapshot.company_account.account || row.shares == 0
        })
    {
        return Err(ExitError::InvalidSnapshot);
    }
    Ok(())
}

fn blank_plan(snapshot: &ExitSnapshot) -> ExitPlan {
    let cash = snapshot
        .company_account
        .cash
        .checked_sub(snapshot.company_account.protected)
        .unwrap_or(Money::ZERO);
    ExitPlan {
        world: snapshot.world,
        branch: snapshot.branch,
        expected_revision: snapshot.revision,
        day: snapshot.day,
        operation_id: snapshot.operation_id,
        trigger: snapshot.trigger,
        status: ExitStatus::ClaimsUnpaid,
        blocked: None,
        cash_for_due_claims: cash,
        cash_after_claims: cash,
        unpaid_due_liabilities: Money::ZERO,
        future_liabilities: Money::ZERO,
        actions: Vec::with_capacity(MAX_EXIT_ACTIONS),
    }
}

/// Build a deterministic proposal from an immutable Engine snapshot. No cash or stock is changed.
pub fn plan_exit(snapshot: &ExitSnapshot) -> Result<ExitPlan, ExitError> {
    validate(snapshot)?;
    let mut plan = blank_plan(snapshot);
    plan.actions.push(ExitAction::StopNewProduction {
        company: snapshot.company,
    });
    plan.actions.push(ExitAction::StopNewInputs {
        company: snapshot.company,
    });
    if !snapshot.employment_exit_terms_complete {
        plan.blocked = Some(ExitBlock::IncompleteEmploymentTerms);
        plan.status = ExitStatus::Blocked;
        return Ok(plan);
    }
    for payroll in snapshot.payroll.iter().filter(|row| row.active) {
        plan.actions.push(ExitAction::CancelFuturePayroll {
            payroll_id: payroll.id,
        });
    }
    if !snapshot.claims_complete {
        plan.blocked = Some(ExitBlock::IncompleteClaims);
    } else if !snapshot.tax_assessment_complete {
        plan.blocked = Some(ExitBlock::IncompleteTaxAssessment);
    } else if !snapshot.asset_register_complete {
        plan.blocked = Some(ExitBlock::IncompleteAssetRegister);
    } else if !snapshot
        .priority_policy
        .as_ref()
        .is_some_and(valid_priority)
    {
        plan.blocked = Some(ExitBlock::MissingPriorityPolicy);
    } else if snapshot.legal_clearance == LegalClearance::Unknown {
        plan.blocked = Some(ExitBlock::LegalClearanceUnknown);
    } else if snapshot.legal_clearance == LegalClearance::Denied {
        plan.blocked = Some(ExitBlock::LegalClearanceDenied);
    }
    if plan.blocked.is_some() {
        plan.status = ExitStatus::Blocked;
        return Ok(plan);
    }

    let order = snapshot
        .priority_policy
        .as_ref()
        .ok_or(ExitError::InvalidSnapshot)?
        .order;
    let mut cash_left = plan.cash_for_due_claims;
    let mut class_index = 0;
    while class_index < order.len() {
        let class = order[class_index];
        let tier = snapshot
            .liabilities
            .iter()
            .filter(|claim| claim.class == class && claim.due_at <= snapshot.day)
            .try_fold(Money::ZERO, |sum, claim| add(sum, claim.outstanding))?;
        if tier == Money::ZERO {
            class_index += 1;
            continue;
        }
        let available = cash_left.value();
        let tier_total = tier.value();
        if available >= tier_total {
            for claim in snapshot
                .liabilities
                .iter()
                .filter(|claim| claim.class == class && claim.due_at <= snapshot.day)
            {
                plan.actions.push(ExitAction::SettleLiability {
                    source: claim.source,
                    payee: claim.payee,
                    amount: claim.outstanding,
                });
            }
            cash_left = sub(cash_left, tier)?;
        } else if available > 0 {
            let mut allocated = 0i64;
            for claim in snapshot
                .liabilities
                .iter()
                .filter(|claim| claim.class == class && claim.due_at <= snapshot.day)
            {
                let portion = i128::from(available)
                    .checked_mul(i128::from(claim.outstanding.value()))
                    .ok_or(ExitError::Overflow)?
                    / i128::from(tier_total);
                let amount = i64::try_from(portion).map_err(|_| ExitError::Overflow)?;
                allocated = allocated.checked_add(amount).ok_or(ExitError::Overflow)?;
            }
            let mut remainder = available
                .checked_sub(allocated)
                .ok_or(ExitError::Overflow)?;
            for claim in snapshot
                .liabilities
                .iter()
                .filter(|claim| claim.class == class && claim.due_at <= snapshot.day)
            {
                let already = i128::from(available)
                    .checked_mul(i128::from(claim.outstanding.value()))
                    .ok_or(ExitError::Overflow)?
                    / i128::from(tier_total);
                let already = i64::try_from(already).map_err(|_| ExitError::Overflow)?;
                let bonus = if remainder > 0 && already < claim.outstanding.value() {
                    1
                } else {
                    0
                };
                let amount = already.checked_add(bonus).ok_or(ExitError::Overflow)?;
                if amount > 0 {
                    plan.actions.push(ExitAction::SettleLiability {
                        source: claim.source,
                        payee: claim.payee,
                        amount: Money::new(amount).map_err(|_| ExitError::Overflow)?,
                    });
                }
                if bonus > 0 {
                    remainder -= 1;
                }
            }
            if remainder != 0 {
                return Err(ExitError::InvalidSnapshot);
            }
            cash_left = Money::ZERO;
        }
        class_index += 1;
    }

    let due_outstanding = snapshot
        .liabilities
        .iter()
        .filter(|claim| claim.due_at <= snapshot.day)
        .try_fold(Money::ZERO, |sum, claim| add(sum, claim.outstanding))?;
    let paid = sub(plan.cash_for_due_claims, cash_left)?;
    let total_due_unpaid = sub(due_outstanding, paid)?;
    let total_future = snapshot
        .liabilities
        .iter()
        .filter(|claim| claim.due_at > snapshot.day)
        .try_fold(Money::ZERO, |sum, claim| add(sum, claim.outstanding))?;
    plan.unpaid_due_liabilities = total_due_unpaid;
    plan.future_liabilities = total_future;
    plan.cash_after_claims = cash_left;

    let mut unsold_inventory = false;
    for lot in &snapshot.inventory {
        if lot.inventory.quantity == crate::units::Quantity::ZERO {
            continue;
        }
        unsold_inventory = true;
        if !lot.sale_ready {
            plan.blocked = Some(ExitBlock::InventoryNotSaleReady);
            plan.status = ExitStatus::Blocked;
            return Ok(plan);
        }
        plan.actions.push(ExitAction::OfferExistingInventory {
            stock_id: lot.inventory.stock_id,
            quantity: lot.inventory.quantity,
        });
    }

    if total_due_unpaid > Money::ZERO {
        plan.status = ExitStatus::ClaimsUnpaid;
        return Ok(plan);
    }
    if total_future > Money::ZERO {
        plan.status = ExitStatus::WaitingForFutureClaims;
        return Ok(plan);
    }
    if unsold_inventory {
        plan.status = ExitStatus::WaitingForInventorySale;
        return Ok(plan);
    }
    if cash_left > Money::ZERO && !snapshot.shareholders_complete {
        plan.blocked = Some(ExitBlock::MissingShareholderRegister);
        plan.status = ExitStatus::Blocked;
        return Ok(plan);
    }
    if cash_left > Money::ZERO && snapshot.shareholders.is_empty() {
        plan.blocked = Some(ExitBlock::InvalidShareholderRegister);
        plan.status = ExitStatus::Blocked;
        return Ok(plan);
    }
    if cash_left > Money::ZERO {
        distribute_residual(snapshot, &mut plan, cash_left)?;
    }
    plan.actions.push(ExitAction::FinalizeCompanyExit {
        company: snapshot.company,
    });
    plan.status = ExitStatus::ReadyToClose;
    Ok(plan)
}

fn distribute_residual(
    snapshot: &ExitSnapshot,
    plan: &mut ExitPlan,
    residual: Money,
) -> Result<(), ExitError> {
    let total_shares = snapshot
        .shareholders
        .iter()
        .try_fold(0u128, |sum, holder| {
            sum.checked_add(u128::from(holder.shares))
                .ok_or(ExitError::Overflow)
        })?;
    if total_shares == 0 {
        return Err(ExitError::InvalidSnapshot);
    }
    let mut allocated = 0i64;
    for holder in &snapshot.shareholders {
        let portion = u128::try_from(residual.value())
            .map_err(|_| ExitError::InvalidSnapshot)?
            .checked_mul(u128::from(holder.shares))
            .ok_or(ExitError::Overflow)?
            / total_shares;
        let amount = i64::try_from(portion).map_err(|_| ExitError::Overflow)?;
        allocated = allocated.checked_add(amount).ok_or(ExitError::Overflow)?;
    }
    let mut remainder = residual
        .value()
        .checked_sub(allocated)
        .ok_or(ExitError::Overflow)?;
    for holder in &snapshot.shareholders {
        let base = u128::try_from(residual.value())
            .map_err(|_| ExitError::InvalidSnapshot)?
            .checked_mul(u128::from(holder.shares))
            .ok_or(ExitError::Overflow)?
            / total_shares;
        let base = i64::try_from(base).map_err(|_| ExitError::Overflow)?;
        let bonus = if remainder > 0 && base < residual.value() {
            1
        } else {
            0
        };
        let amount = base.checked_add(bonus).ok_or(ExitError::Overflow)?;
        if amount > 0 {
            plan.actions.push(ExitAction::DistributeResidual {
                shareholder: holder.account,
                amount: Money::new(amount).map_err(|_| ExitError::Overflow)?,
            });
        }
        remainder = remainder.checked_sub(bonus).ok_or(ExitError::Overflow)?;
    }
    if remainder != 0 {
        return Err(ExitError::InvalidSnapshot);
    }
    Ok(())
}
