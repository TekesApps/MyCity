//! Persistent bounded equity records and exact share-proportional allocation.
//!
//! Immutable opening charters bind legal principals to existing account identities. This
//! module does not establish that cash was transferred or that a dividend is legally
//! distributable; the transaction layer supplies a private proof only after staging a real
//! protected-cash transfer in the same candidate.

use crate::{model_rows::Record, town_runtime::FixedRows, units::Money};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_DIRECT_SHAREHOLDERS: usize = 8;
/// Per-world active ownership records. A world coordinator must enforce this across all
/// companies; it must not multiply this limit by a per-company allowance.
pub const WORLD_EQUITY_RELATION_BUDGET: usize = 8192;
/// One game currency unit is 100 cents. The initial policy issues one share per unit.
pub const INITIAL_CENTS_PER_SHARE: i64 = 100;
pub const MAX_EQUITY_COMPANIES: usize = 4;
/// Opening endowment provenance is deliberately bounded per company. Larger seed inventories
/// must be consolidated into fewer source lots before charter registration.
pub const MAX_EQUITY_OPENING_SOURCES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyKind {
    Person,
    Household,
    Treasury,
    Company,
}

/// A stable legal/economic principal identity. `account` is stored separately on the holder
/// so account changes cannot silently transfer ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityParty {
    pub kind: PartyKind,
    pub id: u32,
}

/// Versioned restrictive rules for one non-listed, one-class ordinary-share register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityPolicy {
    pub policy_id: u32,
    pub version: u16,
    /// Initial rule is exactly 100 cents per issued share (one game currency unit).
    pub cents_per_share: i64,
    /// Set by the company's valid charter; no issue may exceed this cap.
    pub max_issued_shares: u64,
    pub max_direct_shareholders: u8,
    pub ordinary_shares_only: bool,
    pub cross_company_holding_allowed: bool,
    pub secondary_market_enabled: bool,
}

impl EquityPolicy {
    pub fn initial(policy_id: u32, max_issued_shares: u64) -> Self {
        Self {
            policy_id,
            version: 1,
            cents_per_share: INITIAL_CENTS_PER_SHARE,
            max_issued_shares,
            max_direct_shareholders: MAX_DIRECT_SHAREHOLDERS as u8,
            ordinary_shares_only: true,
            cross_company_holding_allowed: false,
            secondary_market_enabled: false,
        }
    }

    pub(crate) fn validate(self) -> Result<(), EquityError> {
        if self.policy_id == 0
            || self.version != 1
            || self.cents_per_share != INITIAL_CENTS_PER_SHARE
            || self.max_issued_shares == 0
            || self.max_direct_shareholders as usize != MAX_DIRECT_SHAREHOLDERS
            || !self.ordinary_shares_only
            || self.cross_company_holding_allowed
            || self.secondary_market_enabled
        {
            return Err(EquityError::InvalidPolicy);
        }
        Ok(())
    }
}

/// One immutable opening identity-to-account binding. A Person identity is meaningful only
/// because this charter explicitly registered it; it does not imply a resident/home link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityInvestorBinding {
    pub party: EquityParty,
    pub account: u32,
    /// Explicit genesis ownership for capital already assigned to this company. The sum is
    /// checked against company cash plus its opening inventory basis before operations.
    pub opening_shares: u64,
}

/// Immutable assignment of one actual opening source to the person/household receiving its
/// endowment shares. This is provenance for genesis assets, not a recorded cash contribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityOpeningSourceBinding {
    pub source_id: u32,
    pub party: EquityParty,
}

impl Record for EquityOpeningSourceBinding {
    fn inactive() -> Self {
        Self {
            source_id: 0,
            party: EquityParty {
                kind: PartyKind::Person,
                id: 0,
            },
        }
    }
}

impl Record for EquityInvestorBinding {
    fn inactive() -> Self {
        Self {
            party: EquityParty {
                kind: PartyKind::Person,
                id: 0,
            },
            account: 0,
            opening_shares: 0,
        }
    }
}

/// Fixed opening authority for one company's initial share issue. Bindings are immutable
/// during this first version; there is no secondary market or account reassignment action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityCharter {
    pub company: u32,
    pub policy: EquityPolicy,
    /// Explicit opening-bound signatory for ordinary retained-earnings distributions. This
    /// is a declared authority, not a majority-vote or general company-governance model.
    pub distribution_authority: EquityParty,
    pub investors: FixedRows<EquityInvestorBinding, MAX_DIRECT_SHAREHOLDERS>,
    /// Exact source IDs behind opening company cash and owned stock basis. No aggregate-only
    /// endowment declaration is accepted.
    #[serde(default)]
    pub opening_sources: FixedRows<EquityOpeningSourceBinding, MAX_EQUITY_OPENING_SOURCES>,
}

impl Record for EquityCharter {
    fn inactive() -> Self {
        Self {
            company: 0,
            policy: EquityPolicy::initial(0, 1),
            distribution_authority: EquityParty {
                kind: PartyKind::Person,
                id: 0,
            },
            investors: FixedRows::new(),
            opening_sources: FixedRows::new(),
        }
    }
}

impl EquityCharter {
    pub(crate) fn validate(&self) -> Result<(), EquityError> {
        self.policy.validate()?;
        if self.company == 0
            || self.investors.is_empty()
            || self.distribution_authority.id == 0
            || self
                .binding_for_party(self.distribution_authority)
                .is_none()
        {
            return Err(EquityError::InvalidRegister);
        }
        let mut previous = None;
        let mut opening_shares = 0_u64;
        for binding in self.investors.as_slice() {
            if binding.party.id == 0
                || binding.account == 0
                || binding.party.kind == PartyKind::Company
            {
                return Err(EquityError::InvalidRegister);
            }
            opening_shares = opening_shares
                .checked_add(binding.opening_shares)
                .ok_or(EquityError::Overflow)?;
            if previous.is_some_and(|party| party >= binding.party) {
                return Err(EquityError::DuplicateOrUnsortedHolder);
            }
            if self
                .investors
                .as_slice()
                .iter()
                .any(|other| other.party != binding.party && other.account == binding.account)
            {
                return Err(EquityError::AccountBindingChanged);
            }
            previous = Some(binding.party);
        }
        if opening_shares > self.policy.max_issued_shares {
            return Err(EquityError::IssueCapExceeded);
        }
        let mut previous_source = 0;
        for source in self.opening_sources.as_slice() {
            if source.source_id == 0
                || source.source_id <= previous_source
                || source.party.id == 0
                || source.party.kind == PartyKind::Company
                || self.binding_for_party(source.party).is_none()
            {
                return Err(EquityError::InvalidRegister);
            }
            previous_source = source.source_id;
        }
        Ok(())
    }

    pub(crate) fn binding_for_account(&self, account: u32) -> Option<EquityParty> {
        self.investors
            .as_slice()
            .iter()
            .find(|binding| binding.account == account)
            .map(|binding| binding.party)
    }

    pub(crate) fn binding_for_party(&self, party: EquityParty) -> Option<u32> {
        self.investors
            .as_slice()
            .iter()
            .find(|binding| binding.party == party)
            .map(|binding| binding.account)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityHolder {
    pub party: EquityParty,
    /// Current receiving/funding account binding. Any change requires a separately authorized
    /// account-change operation and does not change `party` or its shares.
    pub account: u32,
    pub shares: u64,
    /// Cumulative actual paid-in capital, in cents, at the current issue price.
    pub paid_in: Money,
}

impl Record for EquityHolder {
    fn inactive() -> Self {
        Self {
            party: EquityParty {
                kind: PartyKind::Person,
                id: 0,
            },
            account: 0,
            shares: 0,
            paid_in: Money::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquityRegister {
    pub company: u32,
    pub policy: EquityPolicy,
    pub revision: u64,
    pub issued_shares: u64,
    pub opening_issued_shares: u64,
    pub opening_paid_in: Money,
    /// C liquidation capital returned; never an operating expense or loss.
    #[serde(default, skip_serializing_if = "is_zero_money")]
    pub cumulative_capital_returned: Money,
    /// Liquidation proceeds above paid-in capital, separate from ordinary retained earnings.
    #[serde(default, skip_serializing_if = "is_zero_money")]
    pub cumulative_liquidation_surplus_distributed: Money,
    /// Ordinary operating distributions; separate from both paid-in capital and C liquidation.
    #[serde(default, skip_serializing_if = "is_zero_money")]
    pub cumulative_dividends_distributed: Money,
    holders: FixedRows<EquityHolder, MAX_DIRECT_SHAREHOLDERS>,
}

fn is_zero_money(value: &Money) -> bool {
    *value == Money::ZERO
}

impl EquityRegister {
    pub fn empty(company: u32, policy: EquityPolicy) -> Result<Self, EquityError> {
        policy.validate()?;
        if company == 0 {
            return Err(EquityError::InvalidRegister);
        }
        Ok(Self {
            company,
            policy,
            revision: 0,
            issued_shares: 0,
            opening_issued_shares: 0,
            opening_paid_in: Money::ZERO,
            cumulative_capital_returned: Money::ZERO,
            cumulative_liquidation_surplus_distributed: Money::ZERO,
            cumulative_dividends_distributed: Money::ZERO,
            holders: FixedRows::new(),
        })
    }

    pub fn from_charter(
        company: u32,
        policy: EquityPolicy,
        bindings: &[EquityInvestorBinding],
    ) -> Result<Self, EquityError> {
        let mut register = Self::empty(company, policy)?;
        for binding in bindings {
            if binding.opening_shares == 0 {
                continue;
            }
            let amount = checked_cents_per_share(binding.opening_shares, policy.cents_per_share)?;
            let contribution = PaidContribution {
                party: binding.party,
                account: binding.account,
                shares: binding.opening_shares,
                amount,
                receipt_id: 1,
            };
            // Genesis ownership is seeded from the immutable charter only after assembly
            // validates each opening source ID against its exact company asset.
            register.seed_opening_contribution(contribution)?;
        }
        if register.issued_shares > 0 {
            register.revision = 1;
        }
        register.validate()?;
        Ok(register)
    }

    fn seed_opening_contribution(
        &mut self,
        contribution: PaidContribution,
    ) -> Result<(), EquityError> {
        if self.issued_shares != 0
            && self
                .holders
                .as_slice()
                .last()
                .is_some_and(|holder| holder.party >= contribution.party)
        {
            return Err(EquityError::DuplicateOrUnsortedHolder);
        }
        let total_shares = self
            .issued_shares
            .checked_add(contribution.shares)
            .ok_or(EquityError::Overflow)?;
        if total_shares > self.policy.max_issued_shares {
            return Err(EquityError::IssueCapExceeded);
        }
        self.holders
            .push(EquityHolder {
                party: contribution.party,
                account: contribution.account,
                shares: contribution.shares,
                paid_in: contribution.amount,
            })
            .map_err(|_| EquityError::HolderCapExceeded)?;
        self.issued_shares = total_shares;
        self.opening_issued_shares = total_shares;
        self.opening_paid_in = self
            .opening_paid_in
            .checked_add(contribution.amount)
            .map_err(|_| EquityError::Overflow)?;
        Ok(())
    }

    pub fn holders(&self) -> &[EquityHolder] {
        self.holders.as_slice()
    }

    pub(crate) fn record_liquidation_distribution(
        &mut self,
        amount: Money,
    ) -> Result<(Money, Money), EquityError> {
        if amount <= Money::ZERO {
            return Err(EquityError::InvalidRegister);
        }
        let paid_in = self
            .holders
            .as_slice()
            .iter()
            .try_fold(0_i128, |sum, holder| {
                sum.checked_add(i128::from(holder.paid_in.value()))
            })
            .ok_or(EquityError::Overflow)?;
        let unpaid_capital = paid_in
            .checked_sub(i128::from(self.cumulative_capital_returned.value()))
            .ok_or(EquityError::Overflow)?
            .max(0);
        let capital_value = i128::from(amount.value()).min(unpaid_capital);
        let surplus_value = i128::from(amount.value()) - capital_value;
        let capital = Money::new(i64::try_from(capital_value).map_err(|_| EquityError::Overflow)?)
            .map_err(|_| EquityError::Overflow)?;
        let surplus = Money::new(i64::try_from(surplus_value).map_err(|_| EquityError::Overflow)?)
            .map_err(|_| EquityError::Overflow)?;
        self.cumulative_capital_returned = self
            .cumulative_capital_returned
            .checked_add(capital)
            .map_err(|_| EquityError::Overflow)?;
        self.cumulative_liquidation_surplus_distributed = self
            .cumulative_liquidation_surplus_distributed
            .checked_add(surplus)
            .map_err(|_| EquityError::Overflow)?;
        Ok((capital, surplus))
    }

    pub fn validate(&self) -> Result<(), EquityError> {
        self.policy.validate()?;
        if self.company == 0 || self.holders.len() > MAX_DIRECT_SHAREHOLDERS {
            return Err(EquityError::InvalidRegister);
        }
        let mut shares = 0_u64;
        let mut previous = None;
        for holder in self.holders.as_slice() {
            if holder.party.id == 0 || holder.account == 0 || holder.shares == 0 {
                return Err(EquityError::InvalidRegister);
            }
            if holder.party.kind == PartyKind::Company {
                return Err(EquityError::InvalidRegister);
            }
            if previous.is_some_and(|party| party >= holder.party) {
                return Err(EquityError::DuplicateOrUnsortedHolder);
            }
            previous = Some(holder.party);
            let expected = checked_cents_per_share(holder.shares, self.policy.cents_per_share)?;
            if holder.paid_in != expected {
                return Err(EquityError::PaidInMismatch);
            }
            shares = shares
                .checked_add(holder.shares)
                .ok_or(EquityError::Overflow)?;
        }
        if shares != self.issued_shares || shares > self.policy.max_issued_shares {
            return Err(EquityError::ShareCountMismatch);
        }
        let expected_opening =
            checked_cents_per_share(self.opening_issued_shares, self.policy.cents_per_share)?;
        if self.opening_issued_shares > self.issued_shares
            || self.opening_paid_in != expected_opening
            || (self.revision == 0) != (self.issued_shares == 0)
            || self.cumulative_capital_returned < Money::ZERO
            || self.cumulative_liquidation_surplus_distributed < Money::ZERO
            || self.cumulative_dividends_distributed < Money::ZERO
            || i128::from(self.cumulative_capital_returned.value())
                > self
                    .holders
                    .as_slice()
                    .iter()
                    .map(|holder| i128::from(holder.paid_in.value()))
                    .sum::<i128>()
        {
            return Err(EquityError::ShareCountMismatch);
        }
        Ok(())
    }

    /// Engine-only transaction helper. Call only after its candidate has applied the real
    /// account-to-company transfer and verified the authorization. Store operation idempotency
    /// supplies exactly-once behavior; this receipt reference is retained as a link, not proof.
    pub(crate) fn commit_paid_contribution(
        &mut self,
        contribution: PaidContribution,
        transfer: CommittedCashTransfer,
    ) -> Result<(), EquityError> {
        self.validate()?;
        if contribution.receipt_id == 0
            || contribution.party.id == 0
            || contribution.account == 0
            || contribution.shares == 0
            || transfer.operation_id == 0
            || transfer.from != contribution.account
            || transfer.to != self.company
            || transfer.amount != contribution.amount
        {
            return Err(EquityError::InvalidContribution);
        }
        let expected = checked_cents_per_share(contribution.shares, self.policy.cents_per_share)?;
        if expected != contribution.amount {
            return Err(EquityError::ContributionAmountMismatch);
        }
        let new_total = self
            .issued_shares
            .checked_add(contribution.shares)
            .ok_or(EquityError::Overflow)?;
        if new_total > self.policy.max_issued_shares {
            return Err(EquityError::IssueCapExceeded);
        }
        let paid_in = self
            .holders
            .as_slice()
            .iter()
            .find(|holder| holder.party == contribution.party)
            .map(|holder| holder.paid_in)
            .unwrap_or(Money::ZERO)
            .checked_add(contribution.amount)
            .map_err(|_| EquityError::Overflow)?;
        let current_shares = self
            .holders
            .as_slice()
            .iter()
            .find(|holder| holder.party == contribution.party)
            .map_or(0, |holder| holder.shares);
        let updated_shares = current_shares
            .checked_add(contribution.shares)
            .ok_or(EquityError::Overflow)?;
        let existing = self
            .holders
            .as_slice()
            .iter()
            .position(|holder| holder.party == contribution.party);
        if existing.is_none() && self.holders.len() >= MAX_DIRECT_SHAREHOLDERS {
            return Err(EquityError::HolderCapExceeded);
        }
        if contribution.party.kind == PartyKind::Company {
            return Err(EquityError::InvalidContribution);
        }
        let next_revision = self.revision.checked_add(1).ok_or(EquityError::Overflow)?;
        let holder = EquityHolder {
            party: contribution.party,
            account: contribution.account,
            shares: updated_shares,
            paid_in,
        };
        if let Some(index) = existing {
            if self.holders.as_slice()[index].account != contribution.account {
                return Err(EquityError::AccountBindingChanged);
            }
        }
        let mut next_holders = FixedRows::new();
        let mut inserted = false;
        for current in self.holders.as_slice() {
            if !inserted && current.party > holder.party {
                next_holders
                    .push(holder)
                    .map_err(|_| EquityError::HolderCapExceeded)?;
                inserted = true;
            }
            if current.party == holder.party {
                next_holders
                    .push(holder)
                    .map_err(|_| EquityError::HolderCapExceeded)?;
                inserted = true;
            } else {
                next_holders
                    .push(*current)
                    .map_err(|_| EquityError::HolderCapExceeded)?;
            }
        }
        if !inserted {
            next_holders
                .push(holder)
                .map_err(|_| EquityError::HolderCapExceeded)?;
        }
        self.holders = next_holders;
        self.issued_shares = new_total;
        self.revision = next_revision;
        self.validate()
    }
}

impl Record for EquityRegister {
    fn inactive() -> Self {
        Self {
            company: 0,
            policy: EquityPolicy::initial(0, 1),
            revision: 0,
            issued_shares: 0,
            opening_issued_shares: 0,
            opening_paid_in: Money::ZERO,
            cumulative_capital_returned: Money::ZERO,
            cumulative_liquidation_surplus_distributed: Money::ZERO,
            cumulative_dividends_distributed: Money::ZERO,
            holders: FixedRows::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PaidContribution {
    pub party: EquityParty,
    pub account: u32,
    pub shares: u64,
    pub amount: Money,
    pub receipt_id: u64,
}

/// Non-serializable proof minted only by the transaction module after the actual staged cash
/// transfer succeeds. Private fields prevent callers from asserting the two old booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommittedCashTransfer {
    operation_id: u64,
    from: u32,
    to: u32,
    amount: Money,
}

impl CommittedCashTransfer {
    pub(crate) fn after_staged_transfer(
        operation_id: u64,
        from: u32,
        to: u32,
        amount: Money,
    ) -> Result<Self, EquityError> {
        if operation_id == 0 || from == 0 || to == 0 || from == to || amount <= Money::ZERO {
            return Err(EquityError::UncommittedContribution);
        }
        Ok(Self {
            operation_id,
            from,
            to,
            amount,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DistributionRow {
    pub party: EquityParty,
    pub account: u32,
    pub shares: u64,
    pub amount: Money,
}

/// Deterministically split an already-authorized distribution amount. The caller must obtain
/// `amount` from a trusted Engine dividend or debt-cleared liquidation commit path; this helper
/// does not decide whether the company may distribute it.
pub fn allocate_pro_rata(
    amount: Money,
    holders: &[EquityHolder],
) -> Result<Vec<DistributionRow>, EquityError> {
    if holders.is_empty() || holders.len() > MAX_DIRECT_SHAREHOLDERS || amount == Money::ZERO {
        return Err(EquityError::InvalidDistribution);
    }
    let mut total_shares = 0_u128;
    let mut previous = None;
    for holder in holders {
        if holder.party.id == 0
            || holder.account == 0
            || holder.shares == 0
            || holder.party.kind == PartyKind::Company
        {
            return Err(EquityError::InvalidRegister);
        }
        if previous.is_some_and(|party| party >= holder.party) {
            return Err(EquityError::DuplicateOrUnsortedHolder);
        }
        previous = Some(holder.party);
        total_shares = total_shares
            .checked_add(u128::from(holder.shares))
            .ok_or(EquityError::Overflow)?;
    }
    if total_shares == 0 {
        return Err(EquityError::InvalidRegister);
    }
    let amount_cents =
        u128::try_from(amount.value()).map_err(|_| EquityError::InvalidDistribution)?;
    let mut rows = Vec::with_capacity(holders.len());
    let mut allocated = 0_u64;
    for holder in holders {
        let numerator = amount_cents
            .checked_mul(u128::from(holder.shares))
            .ok_or(EquityError::Overflow)?;
        let floor = numerator / total_shares;
        let remainder = numerator % total_shares;
        let floor = u64::try_from(floor).map_err(|_| EquityError::Overflow)?;
        allocated = allocated.checked_add(floor).ok_or(EquityError::Overflow)?;
        rows.push((
            DistributionRow {
                party: holder.party,
                account: holder.account,
                shares: holder.shares,
                amount: Money::new(i64::try_from(floor).map_err(|_| EquityError::Overflow)?)
                    .map_err(|_| EquityError::Overflow)?,
            },
            remainder,
        ));
    }
    let remainder_cents = u64::try_from(amount.value())
        .map_err(|_| EquityError::InvalidDistribution)?
        .checked_sub(allocated)
        .ok_or(EquityError::Overflow)?;
    let bonus_count = usize::try_from(remainder_cents).map_err(|_| EquityError::Overflow)?;
    if bonus_count > rows.len() {
        return Err(EquityError::InvalidDistribution);
    }
    let mut rank: Vec<usize> = (0..rows.len()).collect();
    rank.sort_by(|left, right| {
        rows[*right]
            .1
            .cmp(&rows[*left].1)
            .then_with(|| rows[*left].0.party.cmp(&rows[*right].0.party))
    });
    for index in rank.into_iter().take(bonus_count) {
        let row = &mut rows[index].0;
        row.amount = row
            .amount
            .checked_add(Money::new(1).map_err(|_| EquityError::Overflow)?)
            .map_err(|_| EquityError::Overflow)?;
    }
    rows.sort_by_key(|(row, _)| row.party);
    let result: Vec<_> = rows.into_iter().map(|(row, _)| row).collect();
    let sum = result.iter().try_fold(0_i64, |sum, row| {
        sum.checked_add(row.amount.value())
            .ok_or(EquityError::Overflow)
    })?;
    if sum != amount.value() {
        return Err(EquityError::InvalidDistribution);
    }
    Ok(result)
}

fn checked_cents_per_share(shares: u64, cents_per_share: i64) -> Result<Money, EquityError> {
    let amount = i128::from(shares)
        .checked_mul(i128::from(cents_per_share))
        .ok_or(EquityError::Overflow)?;
    Money::new(i64::try_from(amount).map_err(|_| EquityError::Overflow)?)
        .map_err(|_| EquityError::Overflow)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquityError {
    InvalidPolicy,
    InvalidRegister,
    InvalidContribution,
    InvalidDistribution,
    DuplicateOrUnsortedHolder,
    ShareCountMismatch,
    PaidInMismatch,
    ContributionAmountMismatch,
    UncommittedContribution,
    IssueCapExceeded,
    HolderCapExceeded,
    AccountBindingChanged,
    Overflow,
}

impl fmt::Display for EquityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party(id: u32) -> EquityParty {
        EquityParty {
            kind: PartyKind::Person,
            id,
        }
    }

    fn holder(id: u32, shares: u64) -> EquityHolder {
        EquityHolder {
            party: party(id),
            account: id,
            shares,
            paid_in: Money::new(i64::try_from(shares).unwrap() * INITIAL_CENTS_PER_SHARE).unwrap(),
        }
    }

    #[test]
    fn persisted_register_roundtrips_and_rejects_oversized_holder_arrays() {
        let mut register = EquityRegister::empty(7, EquityPolicy::initial(1, 100)).unwrap();
        let proof = CommittedCashTransfer::after_staged_transfer(9, 2, 7, Money::new(300).unwrap())
            .unwrap();
        register
            .commit_paid_contribution(
                PaidContribution {
                    party: party(2),
                    account: 2,
                    shares: 3,
                    amount: Money::new(300).unwrap(),
                    receipt_id: 9,
                },
                proof,
            )
            .unwrap();
        let encoded = serde_json::to_vec(&register).unwrap();
        let restored: EquityRegister = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored, register);

        let oversized = serde_json::json!({
            "company": 7,
            "policy": EquityPolicy::initial(1, 100),
            "revision": 1,
            "issued_shares": 9,
            "holders": (1..=9).map(|id| holder(id, 1)).collect::<Vec<_>>(),
        });
        assert!(serde_json::from_value::<EquityRegister>(oversized).is_err());
    }

    #[test]
    fn issue_requires_authorized_committed_cash_and_exact_price() {
        let mut register = EquityRegister::empty(7, EquityPolicy::initial(1, 10)).unwrap();
        let contribution = PaidContribution {
            party: party(1),
            account: 1,
            shares: 2,
            amount: Money::new(200).unwrap(),
            receipt_id: 5,
        };
        assert_eq!(
            CommittedCashTransfer::after_staged_transfer(0, 1, 7, Money::new(200).unwrap()),
            Err(EquityError::UncommittedContribution)
        );
        assert_eq!(register.issued_shares, 0);
        let mut wrong_amount = contribution;
        wrong_amount.amount = Money::new(199).unwrap();
        assert_eq!(
            register.commit_paid_contribution(
                wrong_amount,
                CommittedCashTransfer::after_staged_transfer(5, 1, 7, Money::new(199).unwrap())
                    .unwrap()
            ),
            Err(EquityError::ContributionAmountMismatch)
        );
        assert_eq!(register.issued_shares, 0);
    }

    #[test]
    fn share_issue_caps_and_unique_party_account_binding_hold() {
        let mut register = EquityRegister::empty(7, EquityPolicy::initial(1, 4)).unwrap();
        let contribution = PaidContribution {
            party: party(1),
            account: 1,
            shares: 2,
            amount: Money::new(200).unwrap(),
            receipt_id: 1,
        };
        let proof = CommittedCashTransfer::after_staged_transfer(1, 1, 7, Money::new(200).unwrap())
            .unwrap();
        register
            .commit_paid_contribution(contribution, proof)
            .unwrap();
        let mut changed_account = contribution;
        changed_account.account = 2;
        changed_account.shares = 1;
        changed_account.amount = Money::new(100).unwrap();
        changed_account.receipt_id = 2;
        assert_eq!(
            register.commit_paid_contribution(
                changed_account,
                CommittedCashTransfer::after_staged_transfer(2, 2, 7, Money::new(100).unwrap())
                    .unwrap()
            ),
            Err(EquityError::AccountBindingChanged)
        );
        let mut over_cap = contribution;
        over_cap.party = party(2);
        over_cap.account = 2;
        over_cap.shares = 3;
        over_cap.amount = Money::new(300).unwrap();
        over_cap.receipt_id = 3;
        assert_eq!(
            register.commit_paid_contribution(
                over_cap,
                CommittedCashTransfer::after_staged_transfer(3, 2, 7, Money::new(300).unwrap())
                    .unwrap()
            ),
            Err(EquityError::IssueCapExceeded)
        );
    }

    #[test]
    fn distribution_uses_largest_remainder_then_stable_party_id_and_conserves_cents() {
        let holders = [holder(3, 1), holder(1, 1), holder(2, 1)];
        let mut sorted = holders;
        sorted.sort_by_key(|row| row.party);
        let shares = allocate_pro_rata(Money::new(2).unwrap(), &sorted).unwrap();
        assert_eq!(
            shares
                .iter()
                .map(|row| (row.party.id, row.amount.value()))
                .collect::<Vec<_>>(),
            [(1, 1), (2, 1), (3, 0)]
        );
        assert_eq!(
            allocate_pro_rata(Money::ZERO, &sorted),
            Err(EquityError::InvalidDistribution)
        );
    }

    #[test]
    fn maximum_integer_share_allocation_and_bad_holder_order_are_deterministic() {
        let huge = [EquityHolder {
            party: party(1),
            account: 1,
            shares: u64::MAX,
            paid_in: Money::ZERO,
        }];
        assert_eq!(
            allocate_pro_rata(Money::new(1).unwrap(), &huge).unwrap()[0].amount,
            Money::new(1).unwrap()
        );
        let duplicate = [holder(1, 1), holder(1, 1)];
        assert_eq!(
            allocate_pro_rata(Money::new(1).unwrap(), &duplicate),
            Err(EquityError::DuplicateOrUnsortedHolder)
        );
    }
}
