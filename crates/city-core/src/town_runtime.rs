//! Persisted, bounded town-market configuration and compact operational state.
//!
//! The planner in `town_economy` is a pure candidate builder. This module holds the
//! genesis-bound inputs and the small authoritative cursor that an Engine action can
//! update atomically alongside accounts, inventory lots and the timeline.
use crate::{
    model_rows::Record,
    town_economy::{DeliveryQuote, HouseholdGroup, Recipe, ShopSnapshot},
    units::{Money, Quantity},
};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{fmt, mem::size_of};

pub const MAX_MARKET_LANES: usize = 512;
pub const MAX_PAYROLL_CURSORS: usize = crate::timeline::PAYROLL_LIMIT;
pub const MAX_SUSPENDED_OPERATORS: usize =
    crate::town_economy::MAX_EMPLOYERS + crate::town_economy::MAX_SHOP_STOCK_LOTS;
pub const MAX_EXIT_CLAIM_BINDINGS: usize = crate::town_economy::exit::MAX_EXIT_LIABILITIES;
pub const MAX_EXIT_PAYROLL_BINDINGS: usize = crate::timeline::PAYROLL_LIMIT;
pub const MAX_COMPANY_TAX_RULES: usize = crate::town_economy::equity::MAX_EQUITY_COMPANIES;

/// A fixed-capacity table that serializes only active values. Its complete backing is
/// inline in World and therefore charged by `size_of::<World>()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedRows<T: Record, const N: usize> {
    values: [T; N],
    active: u16,
}

impl<T: Record, const N: usize> FixedRows<T, N> {
    pub fn new() -> Self {
        assert!(N <= u16::MAX as usize);
        Self {
            values: [T::inactive(); N],
            active: 0,
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values[..usize::from(self.active)]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.values[..usize::from(self.active)]
    }

    pub fn len(&self) -> usize {
        usize::from(self.active)
    }

    pub fn is_empty(&self) -> bool {
        self.active == 0
    }

    pub fn push(&mut self, value: T) -> Result<(), &'static str> {
        let index = usize::from(self.active);
        if index >= N {
            return Err("town_profile_rows_full");
        }
        self.values[index] = value;
        self.active += 1;
        Ok(())
    }

    pub fn capacity_bytes(&self) -> usize {
        size_of::<T>() * N
    }
}

impl<T: Record, const N: usize> Default for FixedRows<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Record + Serialize, const N: usize> Serialize for FixedRows<T, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.as_slice())
    }
}

impl<'de, T: Record + Deserialize<'de>, const N: usize> Deserialize<'de> for FixedRows<T, N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RowsVisitor<T: Record, const N: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Record + Deserialize<'de>, const N: usize> Visitor<'de> for RowsVisitor<T, N> {
            type Value = FixedRows<T, N>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "at most {N} fixed town rows")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut rows = FixedRows::new();
                while let Some(value) = seq.next_element::<T>()? {
                    if usize::from(rows.active) == N {
                        return Err(de::Error::custom("town_profile_rows_full"));
                    }
                    rows.values[usize::from(rows.active)] = value;
                    rows.active += 1;
                }
                Ok(rows)
            }
        }
        deserializer.deserialize_seq(RowsVisitor::<T, N>(std::marker::PhantomData))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmployerSpec {
    pub account: u32,
    pub worker_limit: u32,
}

impl Record for EmployerSpec {
    fn inactive() -> Self {
        Self {
            account: 0,
            worker_limit: 0,
        }
    }
}

/// Genesis authority mapping for an actual Timeline claim. Unknown/unlisted claims stop exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitClaimBinding {
    pub id: u64,
    pub payee: u32,
    pub class: crate::town_economy::exit::ClaimClass,
}

impl Record for ExitClaimBinding {
    fn inactive() -> Self {
        Self {
            id: 0,
            payee: 0,
            class: crate::town_economy::exit::ClaimClass::Other,
        }
    }
}

/// Genesis binding for an actual Timeline payroll row owed by the exiting company.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitPayrollBinding {
    pub id: u32,
    pub payee: u32,
}

impl Record for ExitPayrollBinding {
    fn inactive() -> Self {
        Self { id: 0, payee: 0 }
    }
}

/// Trusted, finite authority and completeness declaration for the bounded first exit path.
/// These values are immutable genesis inputs; actions can only name this policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitPolicy {
    pub policy_id: u32,
    pub company_account: u32,
    pub requester: u32,
    pub executor: u32,
    pub legal_clearance: crate::town_economy::exit::LegalClearance,
    pub priority_policy: crate::town_economy::exit::ExitPriorityPolicy,
    pub claims_complete: bool,
    pub tax_assessment_complete: bool,
    pub asset_register_complete: bool,
    pub employment_exit_terms_complete: bool,
    pub claims: FixedRows<ExitClaimBinding, MAX_EXIT_CLAIM_BINDINGS>,
    pub payroll: FixedRows<ExitPayrollBinding, MAX_EXIT_PAYROLL_BINDINGS>,
}

/// Explicit first-version tax basis. It is a fixture policy, not a complete tax code.
/// The margin is measured from actual TownSettle sale proceeds/basis and idle payroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxKind {
    DailyRealizedMarketMargin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanyTaxRule {
    pub company: u32,
    pub policy_id: u32,
    pub kind: TaxKind,
    pub authority_account: u32,
    pub rate_basis_points: u16,
}

impl Record for CompanyTaxRule {
    fn inactive() -> Self {
        Self {
            company: 0,
            policy_id: 0,
            kind: TaxKind::DailyRealizedMarketMargin,
            authority_account: 0,
            rate_basis_points: 0,
        }
    }
}

/// Persistent assessment coverage and accumulated after-tax realized operating result.
/// It carries no cash or duplicate debt balance; the actual tax liability is a Timeline Claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaxAssessmentCursor {
    pub company: u32,
    pub policy_id: u32,
    pub through_day: u64,
    pub last_operation_id: u64,
    pub last_claim_id: u64,
    pub cumulative_assessed: Money,
    /// Signed cents: sales margin minus goods basis, uncapitalized idle wages and assessed tax.
    pub retained_earnings_cents: i64,
}

impl Record for TaxAssessmentCursor {
    fn inactive() -> Self {
        Self {
            company: 0,
            policy_id: 0,
            through_day: 0,
            last_operation_id: 0,
            last_claim_id: 0,
            cumulative_assessed: Money::ZERO,
            retained_earnings_cents: 0,
        }
    }
}

impl ExitPolicy {
    fn validate(&self) -> Result<(), &'static str> {
        let p = self.priority_policy;
        let classes = [
            crate::town_economy::exit::ClaimClass::Tax,
            crate::town_economy::exit::ClaimClass::Wage,
            crate::town_economy::exit::ClaimClass::SecuredDebt,
            crate::town_economy::exit::ClaimClass::UnsecuredDebt,
            crate::town_economy::exit::ClaimClass::Other,
        ];
        if self.policy_id == 0
            || self.company_account == 0
            || self.requester == 0
            || self.executor == 0
            || self.company_account == self.requester
            || self.company_account == self.executor
            || self.requester == self.executor
            || p.within_tier
                != crate::town_economy::exit::WithinTierRule::ProRataByOutstandingStableId
            || classes.iter().any(|class| !p.order.contains(class))
            || self.claims.as_slice().iter().enumerate().any(|(i, row)| {
                self.claims.as_slice()[..i]
                    .iter()
                    .any(|prior| prior.id == row.id)
            })
            || self
                .claims
                .as_slice()
                .iter()
                .any(|row| row.id == 0 || row.payee == 0)
            || duplicate_by(self.payroll.as_slice(), |row| row.id)
            || self
                .payroll
                .as_slice()
                .iter()
                .any(|row| row.id == 0 || row.payee == 0)
        {
            return Err("town_exit_policy");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketProfile {
    pub schema_version: u16,
    pub id: u32,
    /// SHA-256 of the immutable Store-owned daily transport allocation profile; all zeroes
    /// means the legacy profile delivery quotes remain the active substitute.
    #[serde(default)]
    pub transport_allocation_sha256: [u8; 32],
    /// Optional genesis-bound physical electricity supply; absent preserves legacy bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utility_supply: Option<crate::town_utility::BootSupplyBinding>,
    /// Optional genesis-bound liquidation authority. Omitted for legacy markets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_policy: Option<ExitPolicy>,
    /// Up to four companies may have immutable, opening-bound share charters. Empty is
    /// omitted so existing worlds keep their canonical representation.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub equity_charters: FixedRows<
        crate::town_economy::equity::EquityCharter,
        { crate::town_economy::equity::MAX_EQUITY_COMPANIES },
    >,
    /// Opening-bound bounded first-version business tax sources. Missing means there is no
    /// authoritative tax-coverage proof, so positive residual distribution remains blocked.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub tax_rules: FixedRows<CompanyTaxRule, MAX_COMPANY_TAX_RULES>,
    pub households: FixedRows<HouseholdGroup, { crate::town_economy::MAX_HOUSEHOLD_GROUPS }>,
    pub employers: FixedRows<EmployerSpec, { crate::town_economy::MAX_EMPLOYERS }>,
    pub shops: FixedRows<ShopSnapshot, { crate::town_economy::MAX_SHOP_STOCK_LOTS }>,
    pub recipes: FixedRows<Recipe, { crate::town_economy::MAX_RECIPES }>,
    pub delivery: FixedRows<DeliveryQuote, MAX_MARKET_LANES>,
}

impl MarketProfile {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.profile_equity_charters_invalid() {
            return Err("town_equity_charter");
        }
        if self
            .tax_rules
            .as_slice()
            .iter()
            .enumerate()
            .any(|(index, rule)| {
                rule.company == 0
                || rule.policy_id == 0
                || rule.authority_account == 0
                || rule.authority_account == rule.company
                || rule.rate_basis_points > 10_000
                || (index > 0 && self.tax_rules.as_slice()[index - 1].company >= rule.company)
                || !self.equity_charters.as_slice().iter().any(|row| row.company == rule.company)
                || !(self.employers.as_slice().iter().any(|row| row.account == rule.company)
                    || self.shops.as_slice().iter().any(|row| row.account == rule.company))
                // First-version tax coverage does not include independently operated freight,
                // utility or disposal businesses; reject those mixed roles rather than treating
                // their gross receipts as profit.
                || self.delivery.as_slice().iter().any(|quote| {
                    quote.freight_account == rule.company || quote.utility_account == rule.company
                })
                || self.recipes.as_slice().iter().any(|recipe| {
                    recipe.utility_account == rule.company
                        || recipe.freight_account == rule.company
                        || recipe.disposal_account == rule.company
                })
            })
        {
            return Err("town_tax_rule");
        }
        if self
            .exit_policy
            .is_some_and(|policy| policy.validate().is_err())
        {
            return Err("town_exit_policy");
        }
        if self.utility_supply.is_some_and(|binding| {
            binding.validate().is_err()
                || self.recipes.as_slice().iter().any(|recipe| {
                    recipe.utility_account != binding.boot_operator
                        || recipe.employer == binding.boot_operator
                        || recipe.available_utility_per_day != Quantity::ZERO
                        || recipe.utility_cost_per_output_q != Money::ZERO
                        || binding
                            .cost_per_output(recipe.utility_per_output_q)
                            .is_err()
                })
                || self
                    .delivery
                    .as_slice()
                    .iter()
                    .any(|quote| quote.utility_surcharge_per_q != Money::ZERO)
        }) {
            return Err("town_utility_profile");
        }
        if self.schema_version != 1
            || self.id == 0
            || self.households.is_empty()
            || self.employers.is_empty()
            || self.shops.is_empty()
            || self.delivery.is_empty()
        {
            return Err("town_profile_header");
        }
        if duplicate_by(self.households.as_slice(), |x| x.id)
            || duplicate_by(self.households.as_slice(), |x| x.account)
            || duplicate_by(self.employers.as_slice(), |x| x.account)
            || duplicate_by(self.shops.as_slice(), |x| x.shop_id)
            || duplicate_by(self.shops.as_slice(), |x| x.stock_id)
            || duplicate_by(self.recipes.as_slice(), |x| x.recipe_id)
            || self.delivery.as_slice().iter().enumerate().any(|(i, row)| {
                self.delivery.as_slice()[..i]
                    .iter()
                    .any(|prior| prior.quote_id == row.quote_id)
            })
            || self
                .households
                .as_slice()
                .iter()
                .any(|x| x.id == 0 || x.account == 0)
            || self
                .shops
                .as_slice()
                .iter()
                .any(|x| x.shop_id == 0 || x.account == 0 || x.stock_id == 0)
            || self
                .delivery
                .as_slice()
                .iter()
                .any(|x| x.quote_id == 0 || x.route_source == 0)
            || self
                .employers
                .as_slice()
                .iter()
                .any(|x| x.account == 0 || x.worker_limit == 0)
            || self.households.as_slice().iter().any(|household| {
                self.employers
                    .as_slice()
                    .iter()
                    .any(|employer| employer.account == household.account)
                    || self
                        .shops
                        .as_slice()
                        .iter()
                        .any(|shop| shop.account == household.account)
            })
            || self.recipes.as_slice().iter().any(|recipe| {
                recipe.assigned_workers == 0
                    || recipe.input_stock_id == recipe.output_stock_id
                    || recipe.input_per_output.value() <= 0
                    || recipe.input_per_output.value()
                        != 1_i64.saturating_add(recipe.waste_per_output_q.value())
                    || !self
                        .employers
                        .as_slice()
                        .iter()
                        .any(|employer| employer.account == recipe.employer)
            })
            || self.delivery.as_slice().iter().any(|quote| {
                !self
                    .shops
                    .as_slice()
                    .iter()
                    .any(|shop| shop.shop_id == quote.shop && shop.good == quote.good)
                    || !self.households.as_slice().iter().any(|household| {
                        household.id == quote.household_group && household.need_good == quote.good
                    })
            })
        {
            return Err("town_profile_identity");
        }
        for employer in self.employers.as_slice() {
            let assigned = self
                .recipes
                .as_slice()
                .iter()
                .filter(|recipe| recipe.employer == employer.account)
                .try_fold(0u32, |sum, recipe| sum.checked_add(recipe.assigned_workers))
                .ok_or("town_profile_worker_overflow")?;
            if assigned > employer.worker_limit {
                return Err("town_profile_worker_limit");
            }
        }
        Ok(())
    }

    fn profile_equity_charters_invalid(&self) -> bool {
        let charters = self.equity_charters.as_slice();
        charters.iter().enumerate().any(|(index, charter)| {
            charter.validate().is_err()
                || (index > 0 && charters[index - 1].company >= charter.company)
                || !(self
                    .employers
                    .as_slice()
                    .iter()
                    .any(|row| row.account == charter.company)
                    || self
                        .shops
                        .as_slice()
                        .iter()
                        .any(|row| row.account == charter.company))
        })
    }

    pub fn owned_bytes(&self) -> usize {
        self.households.capacity_bytes()
            + self.employers.capacity_bytes()
            + self.shops.capacity_bytes()
            + self.recipes.capacity_bytes()
            + self.delivery.capacity_bytes()
            + self.equity_charters.capacity_bytes()
            + self.tax_rules.capacity_bytes()
            + self.exit_policy.map_or(0, |policy| {
                policy.claims.capacity_bytes() + policy.payroll.capacity_bytes()
            })
    }
}

impl Record for HouseholdGroup {
    fn inactive() -> Self {
        Self {
            id: 0,
            account: 0,
            households: 0,
            need_good: 0,
            need_per_household: Quantity::ZERO,
            reference_price_per_q: Money::ZERO,
        }
    }
}

impl Record for ShopSnapshot {
    fn inactive() -> Self {
        Self {
            shop_id: 0,
            account: 0,
            stock_id: 0,
            good: 0,
            shelf_price_per_q: Money::ZERO,
        }
    }
}

impl Record for Recipe {
    fn inactive() -> Self {
        Self {
            recipe_id: 0,
            employer: 0,
            assigned_workers: 0,
            input_stock_id: 0,
            input_per_output: Quantity::ZERO,
            output_good: 0,
            output_stock_id: 0,
            output_per_worker_day: Quantity::ZERO,
            waste_per_output_q: Quantity::ZERO,
            available_utility_per_day: Quantity::ZERO,
            utility_per_output_q: Quantity::ZERO,
            utility_cost_per_output_q: Money::ZERO,
            freight_cost_per_input_q: Money::ZERO,
            disposal_cost_per_input_q: Money::ZERO,
            utility_account: 0,
            freight_account: 0,
            disposal_account: 0,
        }
    }
}

impl Record for DeliveryQuote {
    fn inactive() -> Self {
        Self {
            quote_id: 0,
            shop: 0,
            household_group: 0,
            good: 0,
            reachable: false,
            route_source: 0,
            freight_account: 0,
            utility_account: 0,
            available_capacity: Quantity::ZERO,
            freight_per_q: Money::ZERO,
            utility_surcharge_per_q: Money::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayrollCursor {
    pub id: u32,
    pub work_seconds: u64,
    pub accrued: Money,
    pub paid: Money,
}

impl Record for PayrollCursor {
    fn inactive() -> Self {
        Self {
            id: 0,
            work_seconds: 0,
            accrued: Money::ZERO,
            paid: Money::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketSummary {
    pub day: u64,
    pub operation_id: u64,
    pub reference_need: Quantity,
    pub market_demand: Quantity,
    pub actual_sales: Quantity,
    pub unmet_market_demand: Quantity,
    pub produced: Quantity,
    pub waste: Quantity,
    pub goods_payments: Money,
    pub freight: Money,
    pub utilities: Money,
    pub disposal: Money,
    pub labor_basis: Money,
    pub wages_accrued: Money,
    pub idle_labor_expense: Money,
}

impl Record for MarketSummary {
    fn inactive() -> Self {
        Self {
            day: 0,
            operation_id: 0,
            reference_need: Quantity::ZERO,
            market_demand: Quantity::ZERO,
            actual_sales: Quantity::ZERO,
            unmet_market_demand: Quantity::ZERO,
            produced: Quantity::ZERO,
            waste: Quantity::ZERO,
            goods_payments: Money::ZERO,
            freight: Money::ZERO,
            utilities: Money::ZERO,
            disposal: Money::ZERO,
            labor_basis: Money::ZERO,
            wages_accrued: Money::ZERO,
            idle_labor_expense: Money::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketState {
    pub settled_day: u64,
    pub last_summary: MarketSummary,
    #[serde(default)]
    pub last_transport: Option<MarketTransportReceipt>,
    pub payroll: FixedRows<PayrollCursor, MAX_PAYROLL_CURSORS>,
    pub recipe_output_remainders:
        FixedRows<RecipeOutputRemainder, { crate::town_economy::MAX_RECIPES }>,
    /// Durable stop-operation markers. Empty rows are omitted to preserve the legacy JSON
    /// representation and canonical hash for worlds that have never suspended a business.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub suspended_operators: FixedRows<SuspendedOperator, MAX_SUSPENDED_OPERATORS>,
    /// Active first-version business exit. Missing preserves existing world bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<ExitRuntime>,
    /// Durable issued-share registers. Empty is omitted to preserve old world bytes.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub equity_registers: FixedRows<
        crate::town_economy::equity::EquityRegister,
        { crate::town_economy::equity::MAX_EQUITY_COMPANIES },
    >,
    /// Compact per-company assessment coverage; tax debts remain Timeline claims.
    #[serde(default, skip_serializing_if = "FixedRows::is_empty")]
    pub tax_cursors: FixedRows<TaxAssessmentCursor, MAX_COMPANY_TAX_RULES>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitPhase {
    Liquidating,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitRuntime {
    pub policy_id: u32,
    pub company_account: u32,
    pub started_at: u64,
    pub phase: ExitPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuspendedOperator {
    pub account: u32,
    pub suspended_at: u64,
}

impl Record for SuspendedOperator {
    fn inactive() -> Self {
        Self {
            account: 0,
            suspended_at: 0,
        }
    }
}

/// Small receipt for the exact transport proof consumed by the last market settlement.
/// Full immutable source records stay in Store-owned proof assets, never in Copy World.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketTransportReceipt {
    pub issued_day: u64,
    pub issued_generation: u64,
    pub issued_proof_sha256: [u8; 32],
    pub consumed_generation: u64,
    pub resolved_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeOutputRemainder {
    pub recipe_id: u32,
    /// Numerator remainder in units of 1/28,800 worker-day output.
    pub numerator: u32,
}

impl Record for RecipeOutputRemainder {
    fn inactive() -> Self {
        Self {
            recipe_id: 0,
            numerator: 0,
        }
    }
}

impl MarketState {
    pub fn new() -> Self {
        Self {
            settled_day: 0,
            last_summary: MarketSummary::inactive(),
            last_transport: None,
            payroll: FixedRows::new(),
            recipe_output_remainders: FixedRows::new(),
            suspended_operators: FixedRows::new(),
            exit: None,
            equity_registers: FixedRows::new(),
            tax_cursors: FixedRows::new(),
        }
    }
}

impl Default for MarketState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TownMarket {
    pub profile: MarketProfile,
    pub state: MarketState,
}

impl TownMarket {
    pub fn new(profile: MarketProfile) -> Result<Self, &'static str> {
        profile.validate()?;
        let mut state = MarketState::new();
        for charter in profile.equity_charters.as_slice() {
            state
                .equity_registers
                .push(
                    crate::town_economy::equity::EquityRegister::from_charter(
                        charter.company,
                        charter.policy,
                        charter.investors.as_slice(),
                    )
                    .map_err(|_| "town_equity_charter")?,
                )
                .map_err(|_| "town_equity_capacity")?;
        }
        for rule in profile.tax_rules.as_slice() {
            state
                .tax_cursors
                .push(TaxAssessmentCursor {
                    company: rule.company,
                    policy_id: rule.policy_id,
                    through_day: 0,
                    last_operation_id: 0,
                    last_claim_id: 0,
                    cumulative_assessed: Money::ZERO,
                    retained_earnings_cents: 0,
                })
                .map_err(|_| "town_tax_capacity")?;
        }
        Ok(Self { profile, state })
    }

    pub fn owned_bytes(&self) -> usize {
        self.profile.owned_bytes()
            + self.state.payroll.capacity_bytes()
            + self.state.recipe_output_remainders.capacity_bytes()
            + self.state.suspended_operators.capacity_bytes()
            + self.state.equity_registers.capacity_bytes()
            + self.state.tax_cursors.capacity_bytes()
    }

    pub fn validate_persisted(&self) -> Result<(), &'static str> {
        self.profile.validate()?;
        let transport_bound = self
            .profile
            .transport_allocation_sha256
            .iter()
            .any(|byte| *byte != 0);
        let transport_cursor_valid = match (
            transport_bound,
            self.state.settled_day,
            self.state.last_transport,
        ) {
            (false, _, None) | (true, 0, None) => true,
            (true, day, Some(receipt)) if day > 0 => {
                receipt.issued_day == day
                    && receipt.issued_generation == day
                    && receipt.consumed_generation == day
                    && receipt.issued_proof_sha256.iter().any(|byte| *byte != 0)
                    && receipt.resolved_sha256.iter().any(|byte| *byte != 0)
            }
            _ => false,
        };
        if self.state.payroll.len() > MAX_PAYROLL_CURSORS
            || self.state.suspended_operators.len() > MAX_SUSPENDED_OPERATORS
            || self.state.equity_registers.len() != self.profile.equity_charters.len()
            || self
                .state
                .equity_registers
                .as_slice()
                .iter()
                .enumerate()
                .any(|(index, register)| {
                    let Some(charter) = self.profile.equity_charters.as_slice().get(index) else {
                        return true;
                    };
                    register.validate().is_err()
                        || register.company != charter.company
                        || register.policy != charter.policy
                        || register.opening_issued_shares
                            != charter
                                .investors
                                .as_slice()
                                .iter()
                                .map(|binding| binding.opening_shares)
                                .try_fold(0_u64, u64::checked_add)
                                .unwrap_or(u64::MAX)
                        || charter.investors.as_slice().iter().any(|binding| {
                            register
                                .holders()
                                .iter()
                                .find(|holder| holder.party == binding.party)
                                .map_or(binding.opening_shares != 0, |holder| {
                                    holder.account != binding.account
                                        || holder.shares < binding.opening_shares
                                })
                        })
                        || register.holders().iter().any(|holder| {
                            charter.binding_for_party(holder.party) != Some(holder.account)
                        })
                })
            || self.state.tax_cursors.len() != self.profile.tax_rules.len()
            || self
                .state
                .tax_cursors
                .as_slice()
                .iter()
                .enumerate()
                .any(|(index, cursor)| {
                    let Some(rule) = self.profile.tax_rules.as_slice().get(index) else {
                        return true;
                    };
                    cursor.company != rule.company
                        || cursor.policy_id != rule.policy_id
                        || cursor.through_day > self.state.settled_day
                        || (cursor.through_day == 0 && cursor.last_operation_id != 0)
                        || (cursor.through_day > 0 && cursor.last_operation_id == 0)
                        || cursor.cumulative_assessed < Money::ZERO
                })
            || self.state.exit.is_some_and(|exit| {
                exit.policy_id == 0
                    || exit.company_account == 0
                    || exit.started_at == 0
                    || exit.started_at % crate::timeline::DAY != 0
                    || exit.started_at / crate::timeline::DAY > self.state.settled_day
            })
            || self
                .state
                .suspended_operators
                .as_slice()
                .iter()
                .enumerate()
                .any(|(i, row)| {
                    row.account == 0
                        || row.suspended_at == 0
                        || row.suspended_at % crate::timeline::DAY != 0
                        || row.suspended_at / crate::timeline::DAY > self.state.settled_day
                        || self.state.suspended_operators.as_slice()[..i]
                            .iter()
                            .any(|prior| prior.account == row.account)
                })
            || self.state.payroll.as_slice().iter().any(|x| x.id == 0)
            || self
                .state
                .payroll
                .as_slice()
                .windows(2)
                .any(|w| w[0].id >= w[1].id)
            || self
                .state
                .recipe_output_remainders
                .as_slice()
                .iter()
                .any(|row| row.recipe_id == 0 || row.numerator >= 28_800)
            || self
                .state
                .recipe_output_remainders
                .as_slice()
                .windows(2)
                .any(|w| w[0].recipe_id >= w[1].recipe_id)
            || self.state.last_summary.day > self.state.settled_day
            || (self.state.settled_day == 0 && self.state.last_summary.day != 0)
            || (self.state.settled_day > 0 && self.state.last_summary.day != self.state.settled_day)
            || !transport_cursor_valid
        {
            return Err("town_market_state");
        }
        Ok(())
    }

    pub fn validate_world(&self, world: &crate::assembly::World) -> Result<(), &'static str> {
        self.profile.validate()?;
        for charter in self.profile.equity_charters.as_slice() {
            let company_handle = world.account_handle(charter.company).ok();
            let company = company_handle.and_then(|handle| world.accounts.get(handle).ok());
            if company.is_none_or(|account| account.role != crate::assembly::Role::Company) {
                return Err("town_equity_company");
            }
            self.state
                .equity_registers
                .as_slice()
                .iter()
                .find(|register| register.company == charter.company)
                .ok_or("town_equity_register")?;
            for binding in charter.investors.as_slice() {
                let account = world
                    .account_handle(binding.account)
                    .ok()
                    .and_then(|handle| world.accounts.get(handle).ok())
                    .ok_or("town_equity_investor")?;
                let role_matches = match binding.party.kind {
                    crate::town_economy::equity::PartyKind::Person => {
                        // Personhood is the explicit immutable charter mapping. Existing
                        // Household accounts may fund it, but do not become residence data.
                        account.role == crate::assembly::Role::Household
                    }
                    crate::town_economy::equity::PartyKind::Household => {
                        account.role == crate::assembly::Role::Household
                    }
                    crate::town_economy::equity::PartyKind::Treasury => {
                        account.role == crate::assembly::Role::Treasury
                    }
                    crate::town_economy::equity::PartyKind::Company => false,
                };
                if !role_matches {
                    return Err("town_equity_investor_role");
                }
            }
        }
        self.validate_equity_opening_sources(world, false)?;
        for rule in self.profile.tax_rules.as_slice() {
            let company = world
                .account_handle(rule.company)
                .ok()
                .and_then(|handle| world.accounts.get(handle).ok());
            let authority = world
                .account_handle(rule.authority_account)
                .ok()
                .and_then(|handle| world.accounts.get(handle).ok());
            if company.is_none_or(|account| account.role != crate::assembly::Role::Company)
                || authority.is_none_or(|account| account.role != crate::assembly::Role::Treasury)
            {
                return Err("town_tax_authority");
            }
        }
        if let Some(binding) = self.profile.utility_supply {
            let boot = world.boot.as_ref().ok_or("town_utility_boot")?;
            let stock = world
                .stocks
                .get(
                    world
                        .stock_handle(binding.boot_stock_id)
                        .map_err(|_| "town_utility_stock")?,
                )
                .map_err(|_| "town_utility_stock")?;
            if boot.operator != binding.boot_operator
                || boot.stocks[1] != binding.boot_stock_id
                || stock.commodity != crate::assembly::Commodity::Electricity
                || world
                    .accounts
                    .get(stock.owner)
                    .map_err(|_| "town_utility_owner")?
                    .id
                    != binding.boot_operator
            {
                return Err("town_utility_binding");
            }
        }
        let has_account = |id| world.account_handle(id).is_ok();
        let has_stock = |id| world.stock_handle(id).is_ok();
        if let Some(policy) = self.profile.exit_policy {
            let company = world
                .account_handle(policy.company_account)
                .ok()
                .and_then(|handle| world.accounts.get(handle).ok());
            let requester = world
                .account_handle(policy.requester)
                .ok()
                .and_then(|handle| world.accounts.get(handle).ok());
            let executor = world
                .account_handle(policy.executor)
                .ok()
                .and_then(|handle| world.accounts.get(handle).ok());
            if company.is_none_or(|account| account.role != crate::assembly::Role::Company)
                || requester.is_none()
                || executor.is_none()
                || policy.claims.as_slice().iter().any(|binding| {
                    binding.payee == policy.company_account || !has_account(binding.payee)
                })
                || policy.payroll.as_slice().iter().any(|binding| {
                    binding.payee == policy.company_account || !has_account(binding.payee)
                })
                || !(self
                    .profile
                    .employers
                    .as_slice()
                    .iter()
                    .any(|row| row.account == policy.company_account)
                    || self
                        .profile
                        .shops
                        .as_slice()
                        .iter()
                        .any(|row| row.account == policy.company_account))
            {
                return Err("town_exit_policy_authority");
            }
        }
        if self
            .profile
            .households
            .as_slice()
            .iter()
            .any(|x| !has_account(x.account))
            || self
                .profile
                .employers
                .as_slice()
                .iter()
                .any(|x| !has_account(x.account))
            || self.state.suspended_operators.as_slice().iter().any(|row| {
                !has_account(row.account)
                    || !(self
                        .profile
                        .employers
                        .as_slice()
                        .iter()
                        .any(|employer| employer.account == row.account)
                        || self
                            .profile
                            .shops
                            .as_slice()
                            .iter()
                            .any(|shop| shop.account == row.account))
            })
            || self.profile.shops.as_slice().iter().any(|x| {
                !has_account(x.account)
                    || world
                        .stock_handle(x.stock_id)
                        .ok()
                        .and_then(|h| world.stocks.get(h).ok())
                        .is_none_or(|stock| {
                            world.accounts.get(stock.owner).is_err()
                                || world
                                    .accounts
                                    .get(stock.owner)
                                    .is_ok_and(|a| a.id != x.account)
                                || commodity_good(stock.commodity) != x.good
                        })
            })
            || self.profile.recipes.as_slice().iter().any(|x| {
                !has_account(x.employer)
                    || !has_account(x.utility_account)
                    || !has_account(x.freight_account)
                    || !has_account(x.disposal_account)
                    || !has_stock(x.input_stock_id)
                    || !has_stock(x.output_stock_id)
                    || world
                        .stock_handle(x.input_stock_id)
                        .ok()
                        .and_then(|h| world.stocks.get(h).ok())
                        .is_none_or(|stock| {
                            world.accounts.get(stock.owner).is_err()
                                || world
                                    .accounts
                                    .get(stock.owner)
                                    .is_ok_and(|account| account.id != x.employer)
                                || commodity_good(stock.commodity) != 0
                        })
                    || world
                        .stock_handle(x.output_stock_id)
                        .ok()
                        .and_then(|h| world.stocks.get(h).ok())
                        .is_none_or(|stock| {
                            world.accounts.get(stock.owner).is_err()
                                || world
                                    .accounts
                                    .get(stock.owner)
                                    .is_ok_and(|account| account.id != x.employer)
                                || commodity_good(stock.commodity) != x.output_good
                        })
            })
            || self
                .profile
                .delivery
                .as_slice()
                .iter()
                .any(|x| !has_account(x.freight_account) || !has_account(x.utility_account))
        {
            return Err("town_market_reference");
        }
        Ok(())
    }

    /// Validate immutable endowment provenance without comparing it to current balances.
    /// `initial` is true only during assembly and additionally checks current cash/basis still
    /// equals the source amount before any operation has run.
    fn validate_equity_opening_sources(
        &self,
        world: &crate::assembly::World,
        initial: bool,
    ) -> Result<(), &'static str> {
        let mut seen = [0_u32;
            crate::town_economy::equity::MAX_EQUITY_COMPANIES
                * crate::town_economy::equity::MAX_EQUITY_OPENING_SOURCES];
        let mut seen_len = 0_usize;
        for charter in self.profile.equity_charters.as_slice() {
            let company_handle = world
                .account_handle(charter.company)
                .map_err(|_| "town_equity_company")?;
            let company = world
                .accounts
                .get(company_handle)
                .map_err(|_| "town_equity_company")?;
            if company.opening_source_id == 0
                || !charter
                    .opening_sources
                    .as_slice()
                    .iter()
                    .any(|binding| binding.source_id == company.opening_source_id)
            {
                return Err("town_equity_opening_cash_source_unassigned");
            }
            let mut capital_by_holder =
                [0_i128; crate::town_economy::equity::MAX_DIRECT_SHAREHOLDERS];
            for source_binding in charter.opening_sources.as_slice() {
                if seen[..seen_len].contains(&source_binding.source_id) {
                    return Err("town_equity_opening_source_reused");
                }
                if seen_len == seen.len() {
                    return Err("town_equity_opening_source_limit");
                }
                seen[seen_len] = source_binding.source_id;
                seen_len += 1;
                let source = world
                    .opening_source(source_binding.source_id)
                    .ok_or("town_equity_opening_source_missing")?;
                let holder_index = charter
                    .investors
                    .as_slice()
                    .iter()
                    .position(|binding| binding.party == source_binding.party)
                    .ok_or("town_equity_opening_source_party")?;
                match source.kind {
                    crate::assembly::SourceKind::CashCapital => {
                        if company.opening_source_id != source.id
                            || (initial && company.cash != source.amount)
                        {
                            return Err("town_equity_opening_cash_source_mismatch");
                        }
                    }
                    crate::assembly::SourceKind::InventoryCapital => {
                        let mut matched_stock = None;
                        for (_, handle) in world.stock_ids.iter() {
                            let stock = world
                                .stocks
                                .get(*handle)
                                .map_err(|_| "town_equity_opening_inventory_source_mismatch")?;
                            if stock.owner == company_handle
                                && stock.opening_source_id == source.id
                                && matched_stock.replace(stock).is_some()
                            {
                                return Err("town_equity_opening_inventory_source_mismatch");
                            }
                        }
                        let stock =
                            matched_stock.ok_or("town_equity_opening_inventory_source_mismatch")?;
                        if stock.opening_basis != source.amount
                            || (initial && stock.basis != stock.opening_basis)
                        {
                            return Err("town_equity_opening_inventory_source_mismatch");
                        }
                    }
                }
                capital_by_holder[holder_index] = capital_by_holder[holder_index]
                    .checked_add(i128::from(source.amount.value()))
                    .ok_or("town_equity_capital_overflow")?;
            }
            for (_, handle) in world.stock_ids.iter() {
                let stock = world
                    .stocks
                    .get(*handle)
                    .map_err(|_| "town_equity_opening_inventory_source_unassigned")?;
                if stock.owner != company_handle {
                    continue;
                }
                if !charter
                    .opening_sources
                    .as_slice()
                    .iter()
                    .any(|binding| binding.source_id == stock.opening_source_id)
                {
                    return Err("town_equity_opening_inventory_source_unassigned");
                }
            }
            for (index, investor) in charter.investors.as_slice().iter().enumerate() {
                let represented = i128::from(investor.opening_shares)
                    .checked_mul(i128::from(charter.policy.cents_per_share))
                    .ok_or("town_equity_capital_overflow")?;
                if represented != capital_by_holder[index] {
                    return Err("town_equity_opening_holder_source_mismatch");
                }
            }
        }
        Ok(())
    }

    /// Genesis may require dedicated output lots to be empty before the first
    /// production run. Restored runtime worlds must allow unsold inventory.
    pub fn validate_initial_world(
        &self,
        world: &crate::assembly::World,
    ) -> Result<(), &'static str> {
        self.validate_world(world)?;
        self.validate_equity_opening_sources(world, true)?;
        for charter in self.profile.equity_charters.as_slice() {
            charter.validate().map_err(|_| "town_equity_charter")?;
            let company_handle = world
                .account_handle(charter.company)
                .map_err(|_| "town_equity_company")?;
            let company = world
                .accounts
                .get(company_handle)
                .map_err(|_| "town_equity_company")?;
            let register = self
                .state
                .equity_registers
                .as_slice()
                .iter()
                .find(|register| register.company == charter.company)
                .ok_or("town_equity_register")?;
            if company.role != crate::assembly::Role::Company || company.opening_source_id == 0 {
                return Err("town_equity_company_opening_source");
            }
            let registered_shares = charter
                .investors
                .as_slice()
                .iter()
                .map(|investor| investor.opening_shares)
                .try_fold(0_u64, u64::checked_add)
                .ok_or("town_equity_capital_overflow")?;
            let registered_capital = i128::from(register.opening_issued_shares)
                .checked_mul(i128::from(register.policy.cents_per_share))
                .ok_or("town_equity_capital_overflow")?;
            if registered_shares != register.opening_issued_shares
                || registered_capital != i128::from(register.opening_paid_in.value())
                || register.issued_shares != register.opening_issued_shares
            {
                return Err("town_equity_opening_share_mismatch");
            }
        }
        if self.profile.recipes.as_slice().iter().any(|recipe| {
            world
                .stock_handle(recipe.output_stock_id)
                .ok()
                .and_then(|handle| world.stocks.get(handle).ok())
                .is_none_or(|stock| stock.quantity != Quantity::ZERO)
        }) {
            return Err("town_output_stock_not_empty_at_genesis");
        }
        Ok(())
    }
}

pub(crate) fn commodity_good(commodity: crate::assembly::Commodity) -> u8 {
    match commodity {
        crate::assembly::Commodity::Raw => 0,
        crate::assembly::Commodity::Product => 1,
        crate::assembly::Commodity::Fuel => 2,
        crate::assembly::Commodity::Electricity => 3,
        crate::assembly::Commodity::Water => 4,
        crate::assembly::Commodity::RawWater => 5,
        crate::assembly::Commodity::WasteWater => 6,
        crate::assembly::Commodity::Heat => 7,
    }
}

fn duplicate_by<T>(rows: &[T], key: impl Fn(&T) -> u32) -> bool {
    rows.iter()
        .enumerate()
        .any(|(i, row)| rows[..i].iter().any(|prior| key(prior) == key(row)))
}

#[cfg(test)]
mod transport_cursor_tests {
    use super::*;

    fn profile() -> MarketProfile {
        use crate::town_economy::{DeliveryQuote, HouseholdGroup, ShopSnapshot};
        let mut households = FixedRows::new();
        households
            .push(HouseholdGroup {
                id: 1,
                account: 2,
                households: 1,
                need_good: 0,
                need_per_household: Quantity::new(1).unwrap(),
                reference_price_per_q: Money::new(1).unwrap(),
            })
            .unwrap();
        let mut employers = FixedRows::new();
        employers
            .push(EmployerSpec {
                account: 3,
                worker_limit: 1,
            })
            .unwrap();
        let mut shops = FixedRows::new();
        shops
            .push(ShopSnapshot {
                shop_id: 1,
                account: 3,
                stock_id: 1,
                good: 0,
                shelf_price_per_q: Money::new(1).unwrap(),
            })
            .unwrap();
        let mut delivery = FixedRows::new();
        delivery
            .push(DeliveryQuote {
                quote_id: 1,
                shop: 1,
                household_group: 1,
                good: 0,
                reachable: true,
                route_source: 1,
                freight_account: 3,
                utility_account: 3,
                available_capacity: Quantity::new(1).unwrap(),
                freight_per_q: Money::ZERO,
                utility_surcharge_per_q: Money::ZERO,
            })
            .unwrap();
        MarketProfile {
            schema_version: 1,
            id: 1,
            transport_allocation_sha256: [0; 32],
            utility_supply: None,
            exit_policy: None,
            equity_charters: FixedRows::new(),
            tax_rules: FixedRows::new(),
            households,
            employers,
            shops,
            recipes: FixedRows::new(),
            delivery,
        }
    }

    #[test]
    fn persisted_transport_cursor_must_match_bound_profile_and_settled_day() {
        let mut town = TownMarket::new(profile()).unwrap();
        assert!(town.validate_persisted().is_ok());

        town.state.last_transport = Some(MarketTransportReceipt {
            issued_day: 0,
            issued_generation: 0,
            issued_proof_sha256: [1; 32],
            consumed_generation: 0,
            resolved_sha256: [2; 32],
        });
        assert!(town.validate_persisted().is_err());

        town.state.last_transport = None;
        town.profile.transport_allocation_sha256 = [7; 32];
        assert!(town.validate_persisted().is_ok());
        town.state.settled_day = 1;
        town.state.last_summary.day = 1;
        assert!(town.validate_persisted().is_err());
        town.state.last_transport = Some(MarketTransportReceipt {
            issued_day: 1,
            issued_generation: 1,
            issued_proof_sha256: [3; 32],
            consumed_generation: 1,
            resolved_sha256: [4; 32],
        });
        assert!(town.validate_persisted().is_ok());

        town.state.last_transport.as_mut().unwrap().issued_day = 0;
        assert!(town.validate_persisted().is_err());
    }

    #[test]
    fn empty_suspension_state_keeps_legacy_json_shape_and_defaults_on_restore() {
        let state = MarketState::new();
        let mut encoded = serde_json::to_value(state).unwrap();
        assert!(encoded.get("suspended_operators").is_none());
        encoded
            .as_object_mut()
            .unwrap()
            .remove("suspended_operators");
        let restored: MarketState = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored, state);
        assert!(restored.suspended_operators.is_empty());
    }
}
