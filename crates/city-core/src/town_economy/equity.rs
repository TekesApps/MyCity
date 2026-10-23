//! Persistent bounded equity records and exact share-proportional allocation.
//!
//! This module does not establish that cash was transferred or that a dividend is legally
//! distributable. The Engine's private commit path must verify the actual account transfer,
//! accounting close, creditor protections and operation receipt before calling its mutation
//! function. A receipt identifier is a link to that authority, never proof by itself.

use crate::units::Money;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_DIRECT_SHAREHOLDERS: usize = 8;
/// Per-world active ownership records. A world coordinator must enforce this across all
/// companies; it must not multiply this limit by a per-company allowance.
pub const WORLD_EQUITY_RELATION_BUDGET: usize = 8192;
/// One game currency unit is 100 cents. The initial policy issues one share per unit.
pub const INITIAL_CENTS_PER_SHARE: i64 = 100;

struct BoundedHolders(Vec<EquityHolder>);

impl<'de> Deserialize<'de> for BoundedHolders {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HolderVisitor;

        impl<'de> Visitor<'de> for HolderVisitor {
            type Value = BoundedHolders;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    "at most {MAX_DIRECT_SHAREHOLDERS} equity holders"
                )
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let capacity = sequence
                    .size_hint()
                    .unwrap_or(0)
                    .min(MAX_DIRECT_SHAREHOLDERS);
                let mut holders = Vec::with_capacity(capacity);
                while let Some(holder) = sequence.next_element()? {
                    if holders.len() == MAX_DIRECT_SHAREHOLDERS {
                        return Err(serde::de::Error::custom("equity_holder_limit"));
                    }
                    holders.push(holder);
                }
                Ok(BoundedHolders(holders))
            }
        }

        deserializer.deserialize_seq(HolderVisitor)
    }
}

fn deserialize_holders<'de, D>(deserializer: D) -> Result<Vec<EquityHolder>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    BoundedHolders::deserialize(deserializer).map(|rows| rows.0)
}

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

    fn validate(self) -> Result<(), EquityError> {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EquityRegister {
    pub company: u32,
    pub policy: EquityPolicy,
    pub revision: u64,
    pub issued_shares: u64,
    holders: Vec<EquityHolder>,
}

impl<'de> Deserialize<'de> for EquityRegister {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EncodedRegister {
            company: u32,
            policy: EquityPolicy,
            revision: u64,
            issued_shares: u64,
            #[serde(deserialize_with = "deserialize_holders")]
            holders: Vec<EquityHolder>,
        }

        let encoded = EncodedRegister::deserialize(deserializer)?;
        let register = Self {
            company: encoded.company,
            policy: encoded.policy,
            revision: encoded.revision,
            issued_shares: encoded.issued_shares,
            holders: encoded.holders,
        };
        register.validate().map_err(serde::de::Error::custom)?;
        Ok(register)
    }
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
            holders: Vec::with_capacity(MAX_DIRECT_SHAREHOLDERS),
        })
    }

    pub fn holders(&self) -> &[EquityHolder] {
        &self.holders
    }

    pub fn validate(&self) -> Result<(), EquityError> {
        self.policy.validate()?;
        if self.company == 0 || self.holders.len() > MAX_DIRECT_SHAREHOLDERS {
            return Err(EquityError::InvalidRegister);
        }
        let mut shares = 0_u64;
        let mut previous = None;
        for holder in &self.holders {
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
        Ok(())
    }

    /// Engine-only transaction helper. Call only after its candidate has applied the real
    /// account-to-company transfer and verified the authorization. Store operation idempotency
    /// supplies exactly-once behavior; this receipt reference is retained as a link, not proof.
    pub(crate) fn commit_paid_contribution(
        &mut self,
        contribution: PaidContribution,
        authorized_by_valid_charter: bool,
        payment_committed_in_candidate: bool,
    ) -> Result<(), EquityError> {
        if !authorized_by_valid_charter || !payment_committed_in_candidate {
            return Err(EquityError::UncommittedContribution);
        }
        self.validate()?;
        if contribution.receipt_id == 0
            || contribution.party.id == 0
            || contribution.account == 0
            || contribution.shares == 0
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
            .iter()
            .find(|holder| holder.party == contribution.party)
            .map(|holder| holder.paid_in)
            .unwrap_or(Money::ZERO)
            .checked_add(contribution.amount)
            .map_err(|_| EquityError::Overflow)?;
        let current_shares = self
            .holders
            .iter()
            .find(|holder| holder.party == contribution.party)
            .map_or(0, |holder| holder.shares);
        let updated_shares = current_shares
            .checked_add(contribution.shares)
            .ok_or(EquityError::Overflow)?;
        let position = match self
            .holders
            .binary_search_by_key(&contribution.party, |holder| holder.party)
        {
            Ok(index) => index,
            Err(index) if self.holders.len() < MAX_DIRECT_SHAREHOLDERS => index,
            Err(_) => return Err(EquityError::HolderCapExceeded),
        };
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
        if position < self.holders.len() && self.holders[position].party == contribution.party {
            if self.holders[position].account != contribution.account {
                return Err(EquityError::AccountBindingChanged);
            }
            self.holders[position] = holder;
        } else {
            self.holders.insert(position, holder);
        }
        self.issued_shares = new_total;
        self.revision = next_revision;
        self.validate()
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
        register
            .commit_paid_contribution(
                PaidContribution {
                    party: party(2),
                    account: 2,
                    shares: 3,
                    amount: Money::new(300).unwrap(),
                    receipt_id: 9,
                },
                true,
                true,
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
            register.commit_paid_contribution(contribution, true, false),
            Err(EquityError::UncommittedContribution)
        );
        assert_eq!(register.issued_shares, 0);
        let mut wrong_amount = contribution;
        wrong_amount.amount = Money::new(199).unwrap();
        assert_eq!(
            register.commit_paid_contribution(wrong_amount, true, true),
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
        register
            .commit_paid_contribution(contribution, true, true)
            .unwrap();
        let mut changed_account = contribution;
        changed_account.account = 2;
        changed_account.shares = 1;
        changed_account.amount = Money::new(100).unwrap();
        changed_account.receipt_id = 2;
        assert_eq!(
            register.commit_paid_contribution(changed_account, true, true),
            Err(EquityError::AccountBindingChanged)
        );
        let mut over_cap = contribution;
        over_cap.party = party(2);
        over_cap.account = 2;
        over_cap.shares = 3;
        over_cap.amount = Money::new(300).unwrap();
        over_cap.receipt_id = 3;
        assert_eq!(
            register.commit_paid_contribution(over_cap, true, true),
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
