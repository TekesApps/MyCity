//! Deterministic bounded market planning. The Engine remains authoritative for all settlement.
use crate::units::{Money, Quantity};
use serde::{Deserialize, Serialize};

pub const MAX_HOUSEHOLD_GROUPS: usize = 64;
pub const MAX_EMPLOYERS: usize = 16;
pub const MAX_SHOPS: usize = 16;
pub const MAX_SHOP_STOCK_LOTS: usize = 128;
pub const MAX_RECIPES: usize = 16;
pub const MAX_GOODS: usize = 8;
pub const MAX_LINES: usize = 1024;
/// Additional bounded planning heap target; must be measured after Engine integration.
pub const PLANNER_HEAP_BUDGET_TARGET: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseholdGroup {
    pub id: u32,
    pub account: u32,
    pub households: u32,
    pub need_good: u8,
    pub need_per_household: Quantity,
    pub reference_price_per_q: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmployerSnapshot {
    pub account: u32,
    pub available_workers: u32,
    pub wage_cost_per_worker_day: Money,
    /// Informational input; the planner never accrues or pays these wages.
    pub payroll_paid_today: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventorySnapshot {
    pub stock_id: u32,
    pub owner: u32,
    pub good: u8,
    pub quantity: Quantity,
    pub spendable_basis: Money,
    pub origin_quantity: Quantity,
    pub origin_basis: Money,
    pub offset: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryQuote {
    pub quote_id: u64,
    pub shop: u32,
    pub household_group: u32,
    pub good: u8,
    pub reachable: bool,
    /// Stable transport shipment/edge identity supplied by transport.
    pub route_source: u64,
    pub freight_account: u32,
    pub utility_account: u32,
    /// Remaining daily capacity shared by quotes with the same route and good.
    pub available_capacity: Quantity,
    pub freight_per_q: Money,
    pub utility_surcharge_per_q: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShopSnapshot {
    pub shop_id: u32,
    pub account: u32,
    pub stock_id: u32,
    pub good: u8,
    pub shelf_price_per_q: Money,
}

/// Recipe quantities are integer profile units. This version only supports material recipes;
/// service recipes with zero physical input require a separate explicit contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Stable profile identity for compact Engine commands and immutable recipe lookup.
    pub recipe_id: u32,
    pub employer: u32,
    pub assigned_workers: u32,
    pub input_stock_id: u32,
    pub input_per_output: Quantity,
    pub output_good: u8,
    pub output_stock_id: u32,
    pub output_per_worker_day: Quantity,
    /// Normalized physical units entering the declared disposal stream per output unit.
    pub waste_per_output_q: Quantity,
    pub available_utility_per_day: Quantity,
    pub utility_per_output_q: Quantity,
    pub utility_cost_per_output_q: Money,
    pub freight_cost_per_input_q: Money,
    pub disposal_cost_per_input_q: Money,
    pub utility_account: u32,
    pub freight_account: u32,
    pub disposal_account: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountSnapshot {
    pub account: u32,
    pub cash: Money,
    pub protected: Money,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketInput {
    pub schema: String,
    pub world: u64,
    pub branch: u64,
    pub revision: u64,
    pub day: u64,
    /// Allocated by the command coordinator; must not use Engine-reserved operation prefixes.
    pub operation_id: u64,
    pub households: Vec<HouseholdGroup>,
    pub employers: Vec<EmployerSnapshot>,
    /// One authoritative balance per household/employer/shop/provider, after any existing
    /// Timeline payroll/tax actions have settled for this date.
    pub accounts: Vec<AccountSnapshot>,
    pub inventories: Vec<InventorySnapshot>,
    pub shops: Vec<ShopSnapshot>,
    pub recipes: Vec<Recipe>,
    pub delivery: Vec<DeliveryQuote>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    MissingRoute,
    NoDemand,
    NoCash,
    NoStock,
    RouteCapacity,
    NoInput,
    NoWorkers,
    UnsupportedGood,
    Limit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanLine {
    Produce {
        recipe_id: u32,
        employer: u32,
        workers: u32,
        input_stock_id: u32,
        input_quantity: Quantity,
        output_stock_id: u32,
        output_good: u8,
        output_quantity: Quantity,
        waste_quantity: Quantity,
        utility_quantity: Quantity,
        input_basis: Money,
        output_basis: Money,
        labor_cost: Money,
        utility_cost: Money,
        freight_cost: Money,
        disposal_cost: Money,
        utility_account: u32,
        freight_account: u32,
        disposal_account: u32,
    },
    Sale {
        buyer_account: u32,
        shop_account: u32,
        stock_id: u32,
        good: u8,
        quantity: Quantity,
        goods_basis: Money,
        goods_payment: Money,
        freight_payment: Money,
        utility_payment: Money,
        delivery_quote_id: u64,
        route_source: u64,
        freight_account: u32,
        utility_account: u32,
    },
    Unmet {
        household_group: u32,
        good: u8,
        /// Unserved natural/reference need; price does not erase this quantity.
        quantity: Quantity,
        /// Elasticity-adjusted quantity at the cheapest currently executable offer.
        market_demand: Quantity,
        /// Market demand less actual group purchases.
        unmet_market_demand: Quantity,
        reason: RejectReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementBatch {
    pub world: u64,
    pub branch: u64,
    pub expected_revision: u64,
    pub day: u64,
    pub operation_id: u64,
    pub lines: Vec<PlanLine>,
    pub planned_goods_payments: Money,
    pub planned_freight: Money,
    pub planned_utilities: Money,
    pub planned_disposal: Money,
    pub planned_labor_cost: Money,
    /// Baseline need before price response.
    pub reference_need: Quantity,
    /// Price-adjusted demand at the cheapest offer with stock and route capacity at clearing start.
    pub market_demand: Quantity,
    /// Sum of quantities actually sold.
    pub actual_sales: Quantity,
    /// Price-adjusted market demand not purchased due to price, cash, stock or route constraints.
    pub unmet_market_demand: Quantity,
    /// Reference need less actual sales; a high price never marks natural need satisfied.
    pub unmet_need: Quantity,
    pub reason: Option<RejectReason>,
}

/// Read-only reconciliation derived against the exact authoritative snapshot used for planning.
/// It is an audit summary and never applies cash or inventory changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reconciliation {
    pub cash_debits: Money,
    pub cash_credits: Money,
    pub input_quantity: Quantity,
    pub output_quantity: Quantity,
    pub sold_quantity: Quantity,
    pub reference_need: Quantity,
    pub market_demand: Quantity,
    pub unmet_market_demand: Quantity,
    pub waste_quantity: Quantity,
    pub consumed_basis: Money,
    pub output_basis: Money,
    pub route_quantity: Quantity,
    pub utility_quantity: Quantity,
    /// Timeline-accrued payroll embedded in production basis, not a cash transfer here.
    pub labor_basis: Money,
    pub worker_days: u32,
    pub unmet_quantity: Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    InvalidInput,
    DuplicateId,
    MissingReference,
    Overflow,
    TooManyRows,
    InsufficientProducerCash,
    Unbalanced,
    SnapshotMismatch,
}

fn checked_mul_money(price: Money, quantity: Quantity) -> Result<Money, PlanError> {
    let n = i128::from(price.value()) * i128::from(quantity.value());
    let cents = i64::try_from(n).map_err(|_| PlanError::Overflow)?;
    Money::new(cents).map_err(|_| PlanError::Overflow)
}

fn checked_mul_qty(quantity: Quantity, n: u32) -> Result<Quantity, PlanError> {
    quantity
        .checked_mul(u64::from(n))
        .map_err(|_| PlanError::Overflow)
}

fn demand_at_price(
    need: Quantity,
    reference_price: Money,
    delivered_price: i64,
) -> Result<Quantity, PlanError> {
    if delivered_price <= 0 || reference_price.value() <= 0 {
        return Err(PlanError::InvalidInput);
    }
    let demand = need
        .value()
        .checked_mul(reference_price.value())
        .ok_or(PlanError::Overflow)?
        .checked_div(delivered_price)
        .ok_or(PlanError::Overflow)?
        .min(need.value());
    Quantity::new(demand).map_err(|_| PlanError::Overflow)
}

fn add_money(a: Money, b: Money) -> Result<Money, PlanError> {
    a.checked_add(b).map_err(|_| PlanError::Overflow)
}

fn add_qty(a: Quantity, b: Quantity) -> Result<Quantity, PlanError> {
    a.checked_add(b).map_err(|_| PlanError::Overflow)
}

fn basis_slice(
    origin_basis: Money,
    origin_quantity: Quantity,
    offset: Quantity,
    quantity: Quantity,
) -> Result<Money, PlanError> {
    if origin_quantity == Quantity::ZERO {
        return Err(PlanError::InvalidInput);
    }
    let end = offset
        .checked_add(quantity)
        .map_err(|_| PlanError::Overflow)?;
    if end > origin_quantity {
        return Err(PlanError::InvalidInput);
    }
    let total = i128::from(origin_basis.value());
    let den = i128::from(origin_quantity.value());
    let right = total
        .checked_mul(i128::from(end.value()))
        .ok_or(PlanError::Overflow)?
        .checked_div(den)
        .ok_or(PlanError::InvalidInput)?;
    let left = total
        .checked_mul(i128::from(offset.value()))
        .ok_or(PlanError::Overflow)?
        .checked_div(den)
        .ok_or(PlanError::InvalidInput)?;
    Money::new(i64::try_from(right - left).map_err(|_| PlanError::Overflow)?)
        .map_err(|_| PlanError::Overflow)
}

fn account(input: &MarketInput, id: u32) -> Option<&AccountSnapshot> {
    input.accounts.iter().find(|a| a.account == id)
}

fn stock(input: &MarketInput, id: u32) -> Option<&InventorySnapshot> {
    input.inventories.iter().find(|s| s.stock_id == id)
}

fn input_valid(input: &MarketInput) -> bool {
    let ids_unique = |mut ids: Vec<u32>| {
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        ids.len() == n
    };
    let ids_unique_u64 = |mut ids: Vec<u64>| {
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        ids.len() == n
    };
    input.schema == "mycity-town-market-v0.1"
        && input.world != 0
        && input.branch != 0
        && input.day > 0
        && input.households.len() <= MAX_HOUSEHOLD_GROUPS
        && input.employers.len() <= MAX_EMPLOYERS
        && input.accounts.len() <= 64
        && input.inventories.len() <= 128
        && input.shops.len() <= MAX_SHOP_STOCK_LOTS
        && input.recipes.len() <= MAX_RECIPES
        && input.delivery.len() <= MAX_LINES
        && input.operation_id != 0
        && !matches!(
            input.operation_id & 0xf000_0000_0000_0000,
            0xd000_0000_0000_0000 | 0xe000_0000_0000_0000
        )
        && input.accounts.iter().all(|a| a.cash >= a.protected)
        && input.households.iter().all(|h| {
            h.id > 0
                && h.households > 0
                && h.need_good < MAX_GOODS as u8
                && h.reference_price_per_q.value() > 0
        })
        && input
            .shops
            .iter()
            .all(|s| s.good < MAX_GOODS as u8 && s.shelf_price_per_q.value() > 0)
        && input
            .inventories
            .iter()
            .all(|s| s.good < MAX_GOODS as u8 && account(input, s.owner).is_some())
        && input.employers.iter().all(|e| {
            e.available_workers > 0 || !input.recipes.iter().any(|r| r.employer == e.account)
        })
        && input.recipes.iter().all(|r| {
            r.output_good < MAX_GOODS as u8
                && r.input_per_output.value() > 0
                && r.assigned_workers > 0
                && r.output_stock_id != r.input_stock_id
                && r.input_per_output.value() == 1_i64.saturating_add(r.waste_per_output_q.value())
        })
        && ids_unique(input.households.iter().map(|x| x.id).collect())
        && ids_unique(input.employers.iter().map(|x| x.account).collect())
        && ids_unique(input.accounts.iter().map(|x| x.account).collect())
        && ids_unique(input.inventories.iter().map(|x| x.stock_id).collect())
        && ids_unique(input.shops.iter().map(|x| x.stock_id).collect())
        // Household consumption and employer operating budgets are separate legal accounts;
        // otherwise the two independently projected spendable balances could double-spend.
        && input.households.iter().all(|h| {
            !input.employers.iter().any(|e| e.account == h.account)
        })
        && input
            .households
            .iter()
            .all(|x| account(input, x.account).is_some_and(|a| a.cash >= a.protected))
        && input.shops.iter().all(|x| {
            x.shop_id != 0
                && account(input, x.account).is_some()
                && stock(input, x.stock_id)
                    .is_some_and(|s| s.owner == x.account && s.good == x.good)
        })
        && input.recipes.iter().all(|x| {
            account(input, x.employer).is_some()
                && stock(input, x.input_stock_id).is_some_and(|s| s.owner == x.employer)
                && stock(input, x.output_stock_id).is_some_and(|s| {
                    s.owner == x.employer && s.good == x.output_good && s.quantity == Quantity::ZERO
                })
                && account(input, x.utility_account).is_some()
                && account(input, x.freight_account).is_some()
                && account(input, x.disposal_account).is_some()
        })
        && input.recipes.iter().all(|r| {
            input
                .employers
                .iter()
                .find(|e| e.account == r.employer)
                .is_some_and(|e| {
                    input
                        .recipes
                        .iter()
                        .filter(|other| other.employer == r.employer)
                        .map(|other| u64::from(other.assigned_workers))
                        .sum::<u64>()
                        <= u64::from(e.available_workers)
                })
        })
        && {
            let mut utilities: Vec<(u32, i64)> = input
                .recipes
                .iter()
                .map(|r| (r.utility_account, r.available_utility_per_day.value()))
                .collect();
            utilities.sort_unstable();
            utilities
                .windows(2)
                .all(|w| w[0].0 != w[1].0 || w[0].1 == w[1].1)
        }
        && input.inventories.iter().all(|s| {
            if s.quantity == Quantity::ZERO {
                s.spendable_basis == Money::ZERO
                    && s.origin_quantity == Quantity::ZERO
                    && s.origin_basis == Money::ZERO
                    && s.offset == Quantity::ZERO
            } else {
                s.origin_quantity.value() > 0
                    && basis_slice(s.origin_basis, s.origin_quantity, s.offset, s.quantity).ok()
                        == Some(s.spendable_basis)
            }
        })
        && input.delivery.iter().all(|x| {
            x.quote_id > 0
                && x.route_source > 0
                && x.good < MAX_GOODS as u8
                && input
                    .shops
                    .iter()
                    .any(|s| s.shop_id == x.shop && s.good == x.good)
                && input.households.iter().any(|h| h.id == x.household_group)
                && account(input, x.freight_account).is_some()
                && account(input, x.utility_account).is_some()
        })
        && {
            let mut pairs: Vec<(u32, u32, u8)> = input
                .delivery
                .iter()
                .map(|q| (q.shop, q.household_group, q.good))
                .collect();
            let n = pairs.len();
            pairs.sort_unstable();
            pairs.dedup();
            pairs.len() == n
        }
        && ids_unique_u64(input.delivery.iter().map(|q| q.quote_id).collect())
        && {
            let mut caps: Vec<(u64, u8, i64, u32, u32)> = input
                .delivery
                .iter()
                .map(|q| {
                    (
                        q.route_source,
                        q.good,
                        q.available_capacity.value(),
                        q.freight_account,
                        q.utility_account,
                    )
                })
                .collect();
            caps.sort_unstable();
            caps.windows(2).all(|w| {
                w[0].0 != w[1].0
                    || w[0].1 != w[1].1
                    || (w[0].2 == w[1].2 && w[0].3 == w[1].3 && w[0].4 == w[1].4)
            })
        }
        && {
            let mut shops: Vec<(u32, u32)> =
                input.shops.iter().map(|s| (s.shop_id, s.account)).collect();
            shops.sort_unstable();
            shops
                .windows(2)
                .all(|w| w[0].0 != w[1].0 || w[0].1 == w[1].1)
        }
        && input
            .shops
            .iter()
            .map(|s| s.shop_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            <= MAX_SHOPS
        && {
            let mut outputs: Vec<u32> = input.recipes.iter().map(|r| r.output_stock_id).collect();
            let n = outputs.len();
            outputs.sort_unstable();
            outputs.dedup();
            outputs.len() == n
        }
        && ids_unique(input.recipes.iter().map(|r| r.recipe_id).collect())
        && input.recipes.iter().all(|r| r.recipe_id > 0)
}

#[derive(Clone, Copy)]
struct ProjectedLot {
    stock_id: u32,
    quantity: Quantity,
    basis: Money,
    origin_quantity: Quantity,
    origin_basis: Money,
    offset: Quantity,
}

fn push_line(lines: &mut Vec<PlanLine>, line: PlanLine) -> Result<(), PlanError> {
    if lines.len() >= MAX_LINES {
        return Err(PlanError::TooManyRows);
    }
    lines.push(line);
    Ok(())
}

/// Build one pure plan from a single authoritative revision. It neither changes balances nor
/// inventory; a caller must atomically commit every non-Unmet line through the Engine.
pub fn plan_day(input: &MarketInput) -> Result<SettlementBatch, PlanError> {
    if !input_valid(input) {
        return Err(PlanError::InvalidInput);
    }
    let mut lines = Vec::with_capacity(
        input
            .households
            .len()
            .saturating_mul(input.shops.len())
            .min(MAX_LINES),
    );
    let mut goods_total = Money::ZERO;
    let mut freight_total = Money::ZERO;
    let mut utility_total = Money::ZERO;
    let mut unmet_total = Quantity::ZERO;
    let mut disposal_total = Money::ZERO;
    let mut labor_total = Money::ZERO;
    let mut projected_stocks: Vec<ProjectedLot> = input
        .inventories
        .iter()
        .map(|s| ProjectedLot {
            stock_id: s.stock_id,
            quantity: s.quantity,
            basis: s.spendable_basis,
            origin_quantity: s.origin_quantity,
            origin_basis: s.origin_basis,
            offset: s.offset,
        })
        .collect();
    let mut route_capacity: Vec<(u64, u8, Quantity)> = Vec::new();
    for quote in &input.delivery {
        if !route_capacity
            .iter()
            .any(|x| x.0 == quote.route_source && x.1 == quote.good)
        {
            route_capacity.push((quote.route_source, quote.good, quote.available_capacity));
        }
    }
    let mut utility_capacity: Vec<(u32, Quantity)> = Vec::new();
    for recipe in &input.recipes {
        if !utility_capacity
            .iter()
            .any(|x| x.0 == recipe.utility_account)
        {
            utility_capacity.push((recipe.utility_account, recipe.available_utility_per_day));
        }
    }
    let mut employer_cash: Vec<(u32, i64)> = input
        .employers
        .iter()
        .map(|e| {
            account(input, e.account)
                .map(|a| (e.account, a.cash.value() - a.protected.value()))
                .ok_or(PlanError::MissingReference)
        })
        .collect::<Result<_, _>>()?;

    // Production proposals are constrained by actual worker availability and owned input stock.
    // Their generated output is projected for this clearing only; settlement must atomically
    // consume the input and create that output before the sale batch can be accepted.
    let mut recipes: Vec<&Recipe> = input.recipes.iter().collect();
    recipes.sort_by_key(|r| (r.employer, r.output_stock_id));
    for recipe in recipes {
        let employer = input
            .employers
            .iter()
            .find(|e| e.account == recipe.employer)
            .ok_or(PlanError::MissingReference)?;
        let _producer = account(input, recipe.employer).ok_or(PlanError::MissingReference)?;
        let inv = stock(input, recipe.input_stock_id).ok_or(PlanError::MissingReference)?;
        if inv.owner != recipe.employer {
            return Err(PlanError::MissingReference);
        }
        let actual = projected_stocks
            .iter_mut()
            .find(|x| x.stock_id == recipe.input_stock_id)
            .ok_or(PlanError::MissingReference)?;
        let workers = recipe.assigned_workers;
        if workers == 0 {
            continue;
        }
        let mut output = checked_mul_qty(recipe.output_per_worker_day, workers)?;
        let unit_in = recipe.input_per_output.value();
        let utility_per_output = recipe.utility_per_output_q.value();
        if unit_in <= 0 || utility_per_output <= 0 {
            return Err(PlanError::InvalidInput);
        }
        let input_capacity = actual
            .quantity
            .value()
            .checked_div(unit_in)
            .ok_or(PlanError::Overflow)?;
        let utility_left = utility_capacity
            .iter_mut()
            .find(|x| x.0 == recipe.utility_account)
            .ok_or(PlanError::MissingReference)?;
        let utility_capacity_for_recipe = utility_left
            .1
            .value()
            .checked_div(utility_per_output)
            .ok_or(PlanError::Overflow)?;
        output = Quantity::new(
            output
                .value()
                .min(input_capacity)
                .min(utility_capacity_for_recipe),
        )
        .map_err(|_| PlanError::Overflow)?;
        if output == Quantity::ZERO {
            continue;
        }
        let input_needed = output
            .value()
            .checked_mul(unit_in)
            .ok_or(PlanError::Overflow)?;
        let qty = Quantity::new(input_needed).map_err(|_| PlanError::Overflow)?;
        let input_basis = basis_slice(
            actual.origin_basis,
            actual.origin_quantity,
            actual.offset,
            qty,
        )?;
        let utility = checked_mul_money(recipe.utility_cost_per_output_q, output)?;
        let utility_quantity = checked_mul_qty(
            recipe.utility_per_output_q,
            u32::try_from(output.value()).map_err(|_| PlanError::Overflow)?,
        )?;
        // This is accrued labor cost from Timeline, not a second cash payment.
        let labor_cost = checked_mul_money(
            employer.wage_cost_per_worker_day,
            Quantity::new(i64::from(workers)).map_err(|_| PlanError::Overflow)?,
        )?;
        let freight = checked_mul_money(recipe.freight_cost_per_input_q, qty)?;
        let disposal = checked_mul_money(recipe.disposal_cost_per_input_q, qty)?;
        let waste_quantity = checked_mul_qty(
            recipe.waste_per_output_q,
            u32::try_from(output.value()).map_err(|_| PlanError::Overflow)?,
        )?;
        let total = add_money(add_money(utility, freight)?, disposal)?;
        let spendable = employer_cash
            .iter_mut()
            .find(|x| x.0 == recipe.employer)
            .ok_or(PlanError::MissingReference)?;
        if spendable.1 < total.value() {
            continue;
        }
        spendable.1 -= total.value();
        utility_left.1 = utility_left
            .1
            .checked_sub(utility_quantity)
            .map_err(|_| PlanError::Overflow)?;
        actual.quantity = actual
            .quantity
            .checked_sub(qty)
            .map_err(|_| PlanError::Overflow)?;
        actual.basis = actual
            .basis
            .checked_sub(input_basis)
            .map_err(|_| PlanError::Overflow)?;
        actual.offset = actual
            .offset
            .checked_add(qty)
            .map_err(|_| PlanError::Overflow)?;
        let out = projected_stocks
            .iter_mut()
            .find(|x| x.stock_id == recipe.output_stock_id)
            .ok_or(PlanError::MissingReference)?;
        let output_basis = add_money(
            add_money(add_money(input_basis, utility)?, labor_cost)?,
            add_money(freight, disposal)?,
        )?;
        out.quantity = add_qty(out.quantity, output)?;
        out.basis = output_basis;
        out.origin_quantity = output;
        out.origin_basis = output_basis;
        out.offset = Quantity::ZERO;
        disposal_total = add_money(disposal_total, disposal)?;
        labor_total = add_money(labor_total, labor_cost)?;
        utility_total = add_money(utility_total, utility)?;
        freight_total = add_money(freight_total, freight)?;
        push_line(
            &mut lines,
            PlanLine::Produce {
                recipe_id: recipe.recipe_id,
                employer: recipe.employer,
                workers,
                input_stock_id: recipe.input_stock_id,
                input_quantity: qty,
                output_stock_id: recipe.output_stock_id,
                output_good: recipe.output_good,
                output_quantity: output,
                waste_quantity,
                utility_quantity,
                input_basis,
                output_basis,
                labor_cost,
                utility_cost: utility,
                freight_cost: freight,
                disposal_cost: disposal,
                utility_account: recipe.utility_account,
                freight_account: recipe.freight_account,
                disposal_account: recipe.disposal_account,
            },
        )?;
    }

    let mut group_cash: Vec<(u32, i64)> = input
        .accounts
        .iter()
        .filter(|a| input.households.iter().any(|h| h.account == a.account))
        .map(|a| (a.account, a.cash.value() - a.protected.value()))
        .collect();
    let mut household_needs: Vec<&HouseholdGroup> = input.households.iter().collect();
    household_needs.sort_by_key(|h| h.id);
    let mut reference_need_total = Quantity::ZERO;
    let mut market_demand_total = Quantity::ZERO;
    let mut actual_sales_total = Quantity::ZERO;
    let mut unmet_market_demand_total = Quantity::ZERO;
    for household in household_needs {
        let need = checked_mul_qty(household.need_per_household, household.households)?;
        reference_need_total = add_qty(reference_need_total, need)?;
        if need.value() == 0 {
            continue;
        }
        let mut candidates: Vec<(&ShopSnapshot, &DeliveryQuote, i64)> = Vec::new();
        for shop in &input.shops {
            if shop.good != household.need_good {
                continue;
            }
            if let Some(q) = input.delivery.iter().find(|q| {
                q.shop == shop.shop_id
                    && q.household_group == household.id
                    && q.good == household.need_good
                    && q.reachable
            }) {
                let effective = shop
                    .shelf_price_per_q
                    .value()
                    .checked_add(q.freight_per_q.value())
                    .and_then(|v| v.checked_add(q.utility_surcharge_per_q.value()))
                    .ok_or(PlanError::Overflow)?;
                if effective > 0 {
                    candidates.push((shop, q, effective));
                }
            }
        }
        candidates.sort_by_key(|(s, _, p)| (*p, s.account, s.stock_id));
        let executable_price = candidates
            .iter()
            .filter(|(shop, quote, _)| {
                projected_stocks
                    .iter()
                    .find(|lot| lot.stock_id == shop.stock_id)
                    .is_some_and(|lot| lot.quantity.value() > 0)
                    && route_capacity
                        .iter()
                        .find(|pool| pool.0 == quote.route_source && pool.1 == shop.good)
                        .is_some_and(|pool| pool.2.value() > 0)
            })
            .map(|(_, _, price)| *price)
            .min();
        let market_demand = match executable_price {
            Some(price) => demand_at_price(need, household.reference_price_per_q, price)?,
            None => Quantity::ZERO,
        };
        market_demand_total = add_qty(market_demand_total, market_demand)?;
        let mut group_sales = Quantity::ZERO;
        let mut unmet_reason = if candidates.is_empty() {
            RejectReason::MissingRoute
        } else {
            RejectReason::NoStock
        };
        let mut route_blocked = false;
        let mut cash_blocked = false;
        let mut price_blocked = false;
        for (shop, quote, effective_price) in candidates {
            let inv = projected_stocks
                .iter_mut()
                .find(|x| x.stock_id == shop.stock_id)
                .ok_or(PlanError::MissingReference)?;
            if inv.quantity.value() == 0 {
                continue;
            }
            let route = route_capacity
                .iter_mut()
                .find(|x| x.0 == quote.route_source && x.1 == shop.good)
                .ok_or(PlanError::MissingReference)?;
            if route.2.value() == 0 {
                route_blocked = true;
                unmet_reason = RejectReason::RouteCapacity;
                continue;
            }

            // Demand is a price curve, not a one-time cap at the cheapest listed quote.
            // Each executable marginal offer tightens the cumulative purchase ceiling.
            let price_ceiling =
                demand_at_price(need, household.reference_price_per_q, effective_price)?;
            if group_sales.value() >= price_ceiling.value() {
                price_blocked = true;
                unmet_reason = RejectReason::NoDemand;
                break;
            }
            let cash = group_cash
                .iter_mut()
                .find(|x| x.0 == household.account)
                .ok_or(PlanError::MissingReference)?;
            if cash.1 <= 0 {
                cash_blocked = true;
                unmet_reason = RejectReason::NoCash;
                break;
            }
            let can_afford = cash
                .1
                .checked_div(effective_price)
                .ok_or(PlanError::Overflow)?;
            if can_afford == 0 {
                cash_blocked = true;
                unmet_reason = RejectReason::NoCash;
                break;
            }
            let allowed_at_price = price_ceiling
                .value()
                .checked_sub(group_sales.value())
                .ok_or(PlanError::Overflow)?;
            let qty = allowed_at_price
                .min(inv.quantity.value())
                .min(can_afford)
                .min(route.2.value());
            if qty <= 0 {
                return Err(PlanError::Unbalanced);
            }
            let quantity = Quantity::new(qty).map_err(|_| PlanError::Overflow)?;
            let goods = checked_mul_money(shop.shelf_price_per_q, quantity)?;
            let freight = checked_mul_money(quote.freight_per_q, quantity)?;
            let utility = checked_mul_money(quote.utility_surcharge_per_q, quantity)?;
            let total = add_money(add_money(goods, freight)?, utility)?;
            if total.value() > cash.1 {
                return Err(PlanError::Unbalanced);
            }
            cash.1 -= total.value();
            let goods_basis =
                basis_slice(inv.origin_basis, inv.origin_quantity, inv.offset, quantity)?;
            inv.quantity = inv
                .quantity
                .checked_sub(quantity)
                .map_err(|_| PlanError::Overflow)?;
            inv.basis = inv
                .basis
                .checked_sub(goods_basis)
                .map_err(|_| PlanError::Overflow)?;
            inv.offset = inv
                .offset
                .checked_add(quantity)
                .map_err(|_| PlanError::Overflow)?;
            route.2 = route
                .2
                .checked_sub(quantity)
                .map_err(|_| PlanError::Overflow)?;
            group_sales = add_qty(group_sales, quantity)?;
            if group_sales.value() >= price_ceiling.value() && group_sales.value() < need.value() {
                price_blocked = true;
                unmet_reason = RejectReason::NoDemand;
            }
            goods_total = add_money(goods_total, goods)?;
            freight_total = add_money(freight_total, freight)?;
            utility_total = add_money(utility_total, utility)?;
            push_line(
                &mut lines,
                PlanLine::Sale {
                    buyer_account: household.account,
                    shop_account: shop.account,
                    stock_id: shop.stock_id,
                    good: shop.good,
                    quantity,
                    goods_basis,
                    goods_payment: goods,
                    freight_payment: freight,
                    utility_payment: utility,
                    delivery_quote_id: quote.quote_id,
                    route_source: quote.route_source,
                    freight_account: quote.freight_account,
                    utility_account: quote.utility_account,
                },
            )?;
        }
        actual_sales_total = add_qty(actual_sales_total, group_sales)?;
        let reference_unmet = need
            .checked_sub(group_sales)
            .map_err(|_| PlanError::Overflow)?;
        let market_unmet = market_demand
            .checked_sub(group_sales.min(market_demand))
            .map_err(|_| PlanError::Overflow)?;
        unmet_market_demand_total = add_qty(unmet_market_demand_total, market_unmet)?;
        if reference_unmet.value() > 0 {
            unmet_total = add_qty(unmet_total, reference_unmet)?;
            push_line(
                &mut lines,
                PlanLine::Unmet {
                    household_group: household.id,
                    good: household.need_good,
                    quantity: reference_unmet,
                    market_demand,
                    unmet_market_demand: market_unmet,
                    reason: if cash_blocked {
                        RejectReason::NoCash
                    } else if price_blocked {
                        RejectReason::NoDemand
                    } else if route_blocked {
                        RejectReason::RouteCapacity
                    } else if group_sales == Quantity::ZERO
                        && candidates_empty_for(input, household.id, household.need_good)
                    {
                        RejectReason::MissingRoute
                    } else if market_demand.value() > 0
                        && market_demand.value() < need.value()
                        && group_sales >= market_demand
                    {
                        RejectReason::NoDemand
                    } else {
                        unmet_reason
                    },
                },
            )?;
        }
    }
    Ok(SettlementBatch {
        world: input.world,
        branch: input.branch,
        expected_revision: input.revision,
        day: input.day,
        operation_id: input.operation_id,
        lines,
        planned_goods_payments: goods_total,
        planned_freight: freight_total,
        planned_utilities: utility_total,
        planned_disposal: disposal_total,
        planned_labor_cost: labor_total,
        reference_need: reference_need_total,
        market_demand: market_demand_total,
        actual_sales: actual_sales_total,
        unmet_market_demand: unmet_market_demand_total,
        unmet_need: unmet_total,
        reason: None,
    })
}

fn candidates_empty_for(input: &MarketInput, group: u32, good: u8) -> bool {
    !input.shops.iter().filter(|s| s.good == good).any(|s| {
        input.delivery.iter().any(|q| {
            q.shop == s.shop_id && q.household_group == group && q.good == good && q.reachable
        })
    })
}

/// Validate a proposed batch against the exact read-only Engine snapshot from which it was
/// planned. Replanning is intentional: it checks every counterpart, capacity pool, stable ID,
/// lot basis slice and rounding remainder without maintaining a second ledger.
pub fn reconcile(
    input: &MarketInput,
    batch: &SettlementBatch,
) -> Result<Reconciliation, PlanError> {
    if !input_valid(input) {
        return Err(PlanError::InvalidInput);
    }
    if batch.world != input.world
        || batch.branch != input.branch
        || batch.expected_revision != input.revision
        || batch.day != input.day
        || batch.operation_id != input.operation_id
    {
        return Err(PlanError::SnapshotMismatch);
    }
    let expected = plan_day(input)?;
    if expected != *batch {
        return Err(PlanError::SnapshotMismatch);
    }

    let mut audit = Reconciliation::default();
    let mut goods = Money::ZERO;
    let mut freight = Money::ZERO;
    let mut utility = Money::ZERO;
    let mut disposal = Money::ZERO;
    for line in &batch.lines {
        match *line {
            PlanLine::Produce {
                recipe_id,
                employer,
                workers,
                input_stock_id,
                input_quantity,
                output_stock_id,
                output_good,
                output_quantity,
                waste_quantity,
                utility_quantity,
                input_basis,
                output_basis,
                labor_cost,
                utility_cost,
                freight_cost,
                disposal_cost,
                utility_account,
                freight_account,
                disposal_account,
            } => {
                let recipe = input
                    .recipes
                    .iter()
                    .find(|r| r.recipe_id == recipe_id)
                    .ok_or(PlanError::MissingReference)?;
                if recipe.employer != employer
                    || recipe.input_stock_id != input_stock_id
                    || recipe.output_stock_id != output_stock_id
                    || recipe.output_good != output_good
                    || utility_account != recipe.utility_account
                    || freight_account != recipe.freight_account
                    || disposal_account != recipe.disposal_account
                    || input_quantity.value()
                        != output_quantity
                            .value()
                            .checked_mul(recipe.input_per_output.value())
                            .ok_or(PlanError::Overflow)?
                    || waste_quantity.value()
                        != output_quantity
                            .value()
                            .checked_mul(recipe.waste_per_output_q.value())
                            .ok_or(PlanError::Overflow)?
                    || utility_quantity.value()
                        != output_quantity
                            .value()
                            .checked_mul(recipe.utility_per_output_q.value())
                            .ok_or(PlanError::Overflow)?
                {
                    return Err(PlanError::Unbalanced);
                }
                let expected_basis = add_money(
                    add_money(add_money(input_basis, utility_cost)?, labor_cost)?,
                    add_money(freight_cost, disposal_cost)?,
                )?;
                if expected_basis != output_basis {
                    return Err(PlanError::Unbalanced);
                }
                let production_cash =
                    add_money(add_money(utility_cost, freight_cost)?, disposal_cost)?;
                audit.cash_debits = add_money(audit.cash_debits, production_cash)?;
                audit.cash_credits = add_money(audit.cash_credits, production_cash)?;
                audit.input_quantity = add_qty(audit.input_quantity, input_quantity)?;
                audit.output_quantity = add_qty(audit.output_quantity, output_quantity)?;
                audit.waste_quantity = add_qty(audit.waste_quantity, waste_quantity)?;
                audit.consumed_basis = add_money(audit.consumed_basis, input_basis)?;
                audit.output_basis = add_money(audit.output_basis, output_basis)?;
                audit.utility_quantity = add_qty(audit.utility_quantity, utility_quantity)?;
                audit.labor_basis = add_money(audit.labor_basis, labor_cost)?;
                audit.worker_days = audit
                    .worker_days
                    .checked_add(workers)
                    .ok_or(PlanError::Overflow)?;
                utility = add_money(utility, utility_cost)?;
                freight = add_money(freight, freight_cost)?;
                disposal = add_money(disposal, disposal_cost)?;
            }
            PlanLine::Sale {
                buyer_account,
                shop_account,
                stock_id,
                good,
                quantity,
                goods_basis,
                goods_payment,
                freight_payment,
                utility_payment,
                delivery_quote_id,
                route_source,
                freight_account,
                utility_account,
            } => {
                let shop = input
                    .shops
                    .iter()
                    .find(|s| s.stock_id == stock_id && s.good == good && s.account == shop_account)
                    .ok_or(PlanError::MissingReference)?;
                if account(input, buyer_account).is_none()
                    || account(input, freight_account).is_none()
                    || account(input, utility_account).is_none()
                    || !input.delivery.iter().any(|q| {
                        q.shop == shop.shop_id
                            && q.good == good
                            && q.quote_id == delivery_quote_id
                            && q.route_source == route_source
                            && q.reachable
                            && q.freight_account == freight_account
                            && q.utility_account == utility_account
                    })
                {
                    return Err(PlanError::MissingReference);
                }
                let debit = add_money(add_money(goods_payment, freight_payment)?, utility_payment)?;
                audit.cash_debits = add_money(audit.cash_debits, debit)?;
                audit.cash_credits = add_money(
                    audit.cash_credits,
                    add_money(goods_payment, add_money(freight_payment, utility_payment)?)?,
                )?;
                audit.sold_quantity = add_qty(audit.sold_quantity, quantity)?;
                audit.route_quantity = add_qty(audit.route_quantity, quantity)?;
                audit.consumed_basis = add_money(audit.consumed_basis, goods_basis)?;
                goods = add_money(goods, goods_payment)?;
                freight = add_money(freight, freight_payment)?;
                utility = add_money(utility, utility_payment)?;
            }
            PlanLine::Unmet {
                quantity,
                unmet_market_demand,
                ..
            } => {
                audit.unmet_quantity = add_qty(audit.unmet_quantity, quantity)?;
                audit.unmet_market_demand =
                    add_qty(audit.unmet_market_demand, unmet_market_demand)?;
            }
        }
    }
    for household in &input.households {
        audit.reference_need = add_qty(
            audit.reference_need,
            checked_mul_qty(household.need_per_household, household.households)?,
        )?;
    }
    audit.market_demand = batch.market_demand;
    if audit.cash_debits != audit.cash_credits
        || goods != batch.planned_goods_payments
        || freight != batch.planned_freight
        || utility != batch.planned_utilities
        || disposal != batch.planned_disposal
        || audit.labor_basis != batch.planned_labor_cost
        || audit.reference_need != batch.reference_need
        || audit.sold_quantity != batch.actual_sales
        || audit.market_demand != batch.market_demand
        || audit.unmet_market_demand != batch.unmet_market_demand
        || audit.unmet_quantity != batch.unmet_need
    {
        return Err(PlanError::Unbalanced);
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> MarketInput {
        MarketInput {
            schema: "mycity-town-market-v0.1".to_owned(),
            world: 1,
            branch: 1,
            revision: 0,
            day: 1,
            operation_id: 1,
            households: vec![HouseholdGroup {
                id: 1,
                account: 10,
                households: 2,
                need_good: 0,
                need_per_household: Quantity::new(100).unwrap(),
                reference_price_per_q: Money::new(100).unwrap(),
            }],
            employers: vec![EmployerSnapshot {
                account: 20,
                available_workers: 1,
                wage_cost_per_worker_day: Money::ZERO,
                payroll_paid_today: Money::ZERO,
            }],
            accounts: vec![
                AccountSnapshot {
                    account: 10,
                    cash: Money::new(100_000).unwrap(),
                    protected: Money::ZERO,
                },
                AccountSnapshot {
                    account: 20,
                    cash: Money::new(100_000).unwrap(),
                    protected: Money::ZERO,
                },
            ],
            inventories: vec![
                InventorySnapshot {
                    stock_id: 30,
                    owner: 20,
                    good: 1,
                    quantity: Quantity::new(1000).unwrap(),
                    spendable_basis: Money::ZERO,
                    origin_quantity: Quantity::new(1000).unwrap(),
                    origin_basis: Money::ZERO,
                    offset: Quantity::ZERO,
                },
                InventorySnapshot {
                    stock_id: 31,
                    owner: 20,
                    good: 0,
                    quantity: Quantity::new(0).unwrap(),
                    spendable_basis: Money::ZERO,
                    origin_quantity: Quantity::ZERO,
                    origin_basis: Money::ZERO,
                    offset: Quantity::ZERO,
                },
            ],
            shops: vec![ShopSnapshot {
                shop_id: 1,
                account: 20,
                stock_id: 31,
                good: 0,
                shelf_price_per_q: Money::new(100).unwrap(),
            }],
            recipes: vec![Recipe {
                recipe_id: 1,
                employer: 20,
                input_stock_id: 30,
                assigned_workers: 1,
                input_per_output: Quantity::new(1).unwrap(),
                output_good: 0,
                output_stock_id: 31,
                output_per_worker_day: Quantity::new(100).unwrap(),
                waste_per_output_q: Quantity::ZERO,
                available_utility_per_day: Quantity::new(1000).unwrap(),
                utility_per_output_q: Quantity::new(1).unwrap(),
                utility_cost_per_output_q: Money::new(1).unwrap(),
                freight_cost_per_input_q: Money::ZERO,
                disposal_cost_per_input_q: Money::ZERO,
                utility_account: 20,
                freight_account: 20,
                disposal_account: 20,
            }],
            delivery: vec![DeliveryQuote {
                quote_id: 7001,
                shop: 1,
                household_group: 1,
                good: 0,
                reachable: true,
                route_source: 7,
                freight_account: 20,
                utility_account: 20,
                available_capacity: Quantity::new(1000).unwrap(),
                freight_per_q: Money::ZERO,
                utility_surcharge_per_q: Money::ZERO,
            }],
        }
    }
    #[test]
    fn plan_is_deterministic_bounded_and_does_not_mutate_authoritative_input() {
        let x = input();
        let before = x.clone();
        let a = plan_day(&x).unwrap();
        let b = plan_day(&x).unwrap();
        assert_eq!(a, b);
        assert_eq!(x, before);
        assert_eq!(a.day, 1);
    }
    #[test]
    fn unreachable_route_and_empty_stock_never_create_sales() {
        let mut x = input();
        x.delivery[0].reachable = false;
        let a = plan_day(&x).unwrap();
        assert_eq!(a.planned_goods_payments, Money::ZERO);
        assert!(matches!(
            a.lines.last(),
            Some(PlanLine::Unmet {
                reason: RejectReason::MissingRoute,
                ..
            })
        ));
    }
    #[test]
    fn sales_are_limited_by_actual_stock_and_cash() {
        let mut x = input();
        x.recipes.clear();
        x.inventories[1].quantity = Quantity::new(40).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(40).unwrap();
        x.accounts[0].cash = Money::new(500).unwrap();
        let a = plan_day(&x).unwrap();
        assert_eq!(a.planned_goods_payments.value(), 500);
    }
    #[test]
    fn producer_cash_and_recipe_input_limit_output() {
        let mut x = input();
        x.accounts[1].cash = Money::new(12_000).unwrap();
        x.accounts[1].protected = Money::new(12_000).unwrap();
        let a = plan_day(&x).unwrap();
        assert!(a
            .lines
            .iter()
            .all(|l| !matches!(l, PlanLine::Produce { .. })));
        x.accounts[1].cash = Money::new(100_000).unwrap();
        x.inventories[0].quantity = Quantity::new(2).unwrap();
        let a = plan_day(&x).unwrap();
        assert!(a.lines.iter().any(
            |l| matches!(l, PlanLine::Produce { input_quantity, output_quantity, .. }
                if input_quantity.value()==2 && output_quantity.value()==2)
        ));
    }

    #[test]
    fn duplicate_recipe_or_quote_identity_is_rejected() {
        let mut x = input();
        let mut duplicate_recipe = x.recipes[0];
        duplicate_recipe.input_stock_id = 30;
        duplicate_recipe.output_stock_id = 31;
        x.inventories.push(InventorySnapshot {
            stock_id: 32,
            owner: 20,
            good: 0,
            quantity: Quantity::ZERO,
            spendable_basis: Money::ZERO,
            origin_quantity: Quantity::ZERO,
            origin_basis: Money::ZERO,
            offset: Quantity::ZERO,
        });
        duplicate_recipe.output_stock_id = 32;
        x.recipes.push(duplicate_recipe);
        assert_eq!(plan_day(&x), Err(PlanError::InvalidInput));

        x = input();
        x.delivery[0].quote_id = 7002;
        // Distinct quote IDs are legal only for distinct consumer/store/good pairs.
        x.delivery.push(x.delivery[0]);
        assert_eq!(plan_day(&x), Err(PlanError::InvalidInput));
    }

    #[test]
    fn recipes_compete_for_one_shared_utility_pool() {
        let mut x = input();
        x.employers[0].available_workers = 2;
        x.recipes[0].available_utility_per_day = Quantity::new(100).unwrap();
        let mut second = x.recipes[0];
        second.recipe_id = 2;
        second.output_good = 2;
        second.output_stock_id = 32;
        x.recipes.push(second);
        x.inventories.push(InventorySnapshot {
            stock_id: 32,
            owner: 20,
            good: 2,
            quantity: Quantity::ZERO,
            spendable_basis: Money::ZERO,
            origin_quantity: Quantity::ZERO,
            origin_basis: Money::ZERO,
            offset: Quantity::ZERO,
        });
        let batch = plan_day(&x).unwrap();
        let production: Vec<_> = batch
            .lines
            .iter()
            .filter(|line| matches!(line, PlanLine::Produce { .. }))
            .collect();
        assert_eq!(production.len(), 1);
        assert_eq!(reconcile(&x, &batch).unwrap().utility_quantity.value(), 100);
    }

    #[test]
    fn route_capacity_is_consumed_once_per_route_and_good() {
        let mut x = input();
        x.recipes.clear();
        x.inventories[1].quantity = Quantity::new(1000).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(1000).unwrap();
        x.delivery[0].available_capacity = Quantity::new(25).unwrap();
        let plan = plan_day(&x).unwrap();
        let sold: i64 = plan
            .lines
            .iter()
            .map(|l| match l {
                PlanLine::Sale { quantity, .. } => quantity.value(),
                _ => 0,
            })
            .sum();
        assert_eq!(sold, 25);
        assert_eq!(plan.unmet_need.value(), 175);
    }

    #[test]
    fn reconciliation_balances_counterparties_and_rejects_stale_or_tampered_batch() {
        let x = input();
        let mut batch = plan_day(&x).unwrap();
        let audit = reconcile(&x, &batch).unwrap();
        assert_eq!(audit.cash_debits, audit.cash_credits);
        assert_eq!(audit.worker_days, 1);
        assert_eq!(audit.input_quantity.value(), 100);
        assert_eq!(audit.output_quantity.value(), 100);
        assert_eq!(audit.sold_quantity.value(), 100);
        assert_eq!(audit.utility_quantity.value(), 100);
        batch.day += 1;
        assert_eq!(reconcile(&x, &batch), Err(PlanError::SnapshotMismatch));
        batch.day -= 1;
        if let Some(PlanLine::Sale {
            freight_account, ..
        }) = batch
            .lines
            .iter_mut()
            .find(|line| matches!(line, PlanLine::Sale { .. }))
        {
            *freight_account = 999;
        }
        assert_eq!(reconcile(&x, &batch), Err(PlanError::SnapshotMismatch));
    }

    #[test]
    fn stockout_no_cash_and_closed_road_propagate_as_unmet_demand() {
        let mut x = input();
        x.recipes.clear();
        x.inventories[1].quantity = Quantity::ZERO;
        let stockout = plan_day(&x).unwrap();
        assert_eq!(stockout.planned_goods_payments, Money::ZERO);
        assert!(stockout.lines.iter().any(|line| matches!(
            line,
            PlanLine::Unmet {
                reason: RejectReason::NoStock,
                ..
            }
        )));

        x.inventories[1].quantity = Quantity::new(1000).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(1000).unwrap();
        x.accounts[0].cash = Money::new(1_000).unwrap();
        x.accounts[0].protected = Money::new(1_000).unwrap();
        let no_cash = plan_day(&x).unwrap();
        assert_eq!(no_cash.planned_goods_payments, Money::ZERO);
        assert!(no_cash.lines.iter().any(|line| matches!(
            line,
            PlanLine::Unmet {
                reason: RejectReason::NoCash,
                ..
            }
        )));

        x.accounts[0].cash = Money::new(100_000).unwrap();
        x.delivery[0].reachable = false;
        let closed_road = plan_day(&x).unwrap();
        assert_eq!(closed_road.planned_goods_payments, Money::ZERO);
        assert!(closed_road.lines.iter().any(|line| matches!(
            line,
            PlanLine::Unmet {
                reason: RejectReason::MissingRoute,
                ..
            }
        )));
    }

    #[test]
    fn energy_price_and_falling_demand_reduce_production_and_sales() {
        let mut x = input();
        x.accounts[1].cash = Money::new(100).unwrap();
        let baseline = plan_day(&x).unwrap();
        x.recipes[0].utility_cost_per_output_q = Money::new(2).unwrap();
        let energy_shock = plan_day(&x).unwrap();
        assert!(baseline
            .lines
            .iter()
            .any(|l| matches!(l, PlanLine::Produce { .. })));
        assert!(!energy_shock
            .lines
            .iter()
            .any(|l| matches!(l, PlanLine::Produce { .. })));

        x.recipes.clear();
        x.accounts[1].cash = Money::new(100_000).unwrap();
        x.inventories[1].quantity = Quantity::new(1000).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(1000).unwrap();
        let normal_demand = plan_day(&x).unwrap();
        x.households[0].households = 1;
        let lower_demand = plan_day(&x).unwrap();
        assert!(
            normal_demand.planned_goods_payments.value()
                > lower_demand.planned_goods_payments.value()
        );
        assert!(normal_demand
            .lines
            .iter()
            .any(|l| matches!(l, PlanLine::Sale { .. })));
    }

    fn add_expensive_offer(input: &mut MarketInput, inventory_quantity: i64) {
        input.inventories.push(InventorySnapshot {
            stock_id: 32,
            owner: 20,
            good: 0,
            quantity: Quantity::new(inventory_quantity).unwrap(),
            spendable_basis: Money::ZERO,
            origin_quantity: Quantity::new(inventory_quantity).unwrap(),
            origin_basis: Money::ZERO,
            offset: Quantity::ZERO,
        });
        input.shops.push(ShopSnapshot {
            shop_id: 2,
            account: 20,
            stock_id: 32,
            good: 0,
            shelf_price_per_q: Money::new(100).unwrap(),
        });
        input.delivery.push(DeliveryQuote {
            quote_id: 7002,
            shop: 2,
            household_group: 1,
            good: 0,
            reachable: true,
            route_source: 8,
            freight_account: 20,
            utility_account: 20,
            available_capacity: Quantity::new(100).unwrap(),
            freight_per_q: Money::ZERO,
            utility_surcharge_per_q: Money::ZERO,
        });
    }

    fn sales_for_group(batch: &SettlementBatch, buyer: u32) -> i64 {
        batch
            .lines
            .iter()
            .map(|line| match line {
                PlanLine::Sale {
                    buyer_account,
                    quantity,
                    ..
                } if *buyer_account == buyer => quantity.value(),
                _ => 0,
            })
            .sum()
    }

    #[test]
    fn empty_cheap_offer_does_not_set_demand_price_for_expensive_stock() {
        let mut x = input();
        x.recipes.clear();
        x.households[0].households = 1;
        x.households[0].reference_price_per_q = Money::new(10).unwrap();
        x.shops[0].shelf_price_per_q = Money::new(10).unwrap();
        add_expensive_offer(&mut x, 100);

        let batch = plan_day(&x).unwrap();
        assert_eq!(sales_for_group(&batch, 10), 10);
        assert_eq!(batch.market_demand.value(), 10);
        assert_eq!(batch.actual_sales.value(), 10);
        assert_eq!(batch.unmet_need.value(), 90);
    }

    #[test]
    fn remaining_cheap_inventory_then_expensive_offer_reprices_only_marginal_units() {
        let mut x = input();
        x.recipes.clear();
        x.households[0].households = 1;
        x.households[0].reference_price_per_q = Money::new(10).unwrap();
        x.shops[0].shelf_price_per_q = Money::new(10).unwrap();
        x.inventories[1].quantity = Quantity::new(5).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(5).unwrap();
        add_expensive_offer(&mut x, 100);

        let batch = plan_day(&x).unwrap();
        assert_eq!(sales_for_group(&batch, 10), 10);
        assert_eq!(batch.market_demand.value(), 100);
        assert_eq!(batch.unmet_market_demand.value(), 90);
    }

    #[test]
    fn exhausted_cheap_route_then_expensive_route_reprices_remaining_demand() {
        let mut x = input();
        x.recipes.clear();
        x.households[0].households = 1;
        x.households[0].reference_price_per_q = Money::new(10).unwrap();
        x.shops[0].shelf_price_per_q = Money::new(10).unwrap();
        x.inventories[1].quantity = Quantity::new(100).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(100).unwrap();
        x.delivery[0].available_capacity = Quantity::new(5).unwrap();
        add_expensive_offer(&mut x, 100);

        let batch = plan_day(&x).unwrap();
        assert_eq!(sales_for_group(&batch, 10), 10);
        assert_eq!(batch.market_demand.value(), 100);
        assert_eq!(batch.unmet_market_demand.value(), 90);
    }

    #[test]
    fn price_increase_keeps_reference_need_visible_as_unmet() {
        let mut x = input();
        x.recipes.clear();
        x.households[0].households = 1;
        x.households[0].reference_price_per_q = Money::new(10).unwrap();
        x.inventories[1].quantity = Quantity::new(100).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(100).unwrap();
        x.shops[0].shelf_price_per_q = Money::new(10).unwrap();
        x.delivery[0].utility_surcharge_per_q = Money::new(90).unwrap();

        let batch = plan_day(&x).unwrap();
        assert_eq!(sales_for_group(&batch, 10), 10);
        assert_eq!(batch.market_demand.value(), 10);
        assert_eq!(batch.unmet_need.value(), 90);
    }

    #[test]
    fn offer_iteration_order_does_not_change_the_plan() {
        let mut x = input();
        x.recipes.clear();
        x.households[0].households = 1;
        x.households[0].reference_price_per_q = Money::new(10).unwrap();
        x.shops[0].shelf_price_per_q = Money::new(10).unwrap();
        x.inventories[1].quantity = Quantity::new(5).unwrap();
        x.inventories[1].origin_quantity = Quantity::new(5).unwrap();
        add_expensive_offer(&mut x, 100);
        let expected = plan_day(&x).unwrap();

        x.shops.reverse();
        x.delivery.reverse();
        x.inventories.reverse();
        assert_eq!(plan_day(&x).unwrap(), expected);
    }
}
