# Core road and town 28-day source scenario

`town_transport road-28 <new-store-dir>` reuses the existing Store-backed road-effect
scenario and keeps its existing `road-effect` and `road-suspend` modes unchanged. The mode
uses the same authored road project, construction gates and work, inspection boundary,
published capacity contribution, fixed OD allocation, verified transport proof, town-market
settlement, payroll, and Store receipt paths. It does not add runtime code or a second
transport or economic ledger.

## Fixed opening inputs

The opening cash comes from the existing fixture's capital sources: treasury account 1 has
1,000,000 cents, household account 2 has 1,000,000 cents in this scenario, factory account 3
has 600,000 cents, and supplier account 4 has 1,200,000 cents. The factory receives 1,000
quantity units of raw stock with 1,000 cents of opening basis and starts with no finished
goods. The supplier's existing raw-stock lot funds the road project, which consumes 1,000
units; the scenario does not replenish either stock later. The road contractor is a separate
account. The scenario emits actual opening balances and final balances so that reliance on
opening capital and stock is visible.

The factory's real Timeline payroll pays 100 cents per hour and runs from game day 0 through
day 28, with an explicit end at `28 * DAY`. The normal working-day accrual window and due
payments remain unchanged. The factory recipe uses its existing input/output, utility
capacity, unit utility cost, and waste fields. Utility supply remains the explicit profile
input (`available_utility_per_day=1000`, `utility_per_output_q=1`), not a live city grid.

The transport allocation repeats a fixed daily OD batch: ten trips in each direction,
twenty goods units per trip. The road starts with the existing bottleneck capacity, and the
accepted inspection publishes the same capacity effect. Daily goods sold still come from
the actual stock and proof-backed market settlement. OD demand does not respond to inventory
shortage or household behavior; that limitation is reported with the result.

## Twenty-eight-day execution and checks

After the day-one road inspection publication, the example settles every day through day
28. Every settlement must have a fresh proof for that service day and the active published
road generation, with no duplicate proof hash. Day 14 checkpoints, closes the Store, reopens
it under the same rules identity, and checks the state hash, sequence, and day-14 proof.
The final checkpoint and cold reopen recheck the full state hash, sequence, and all daily
market receipts.

The bounded report aggregates actual sales, production, unmet demand, goods payments,
freight, utilities, disposal, accrued and paid wages, idle labor, construction spending,
and remaining stock. The opening/final company cash and inventory basis, unpaid wages, and
their restricted book projection are reported. This projection is only cash plus inventory
basis less unpaid wages for this no-financing fixture; it is not a complete company balance
sheet and excludes unmodeled assets or liabilities. The report separately says opening
capital was present and opening inventory was consumed; it does not claim that either was
proven necessary to fund the run. Continuous self-funding is explicitly not demonstrated,
because the finite opening raw inventory is allowed to run down.

The planner defines `idle_labor_expense = wages_accrued - labor_basis`; the source asserts
the corresponding reconciliation `labor_basis + idle_labor_expense == wages_accrued` over
all 28 daily summaries. This prevents adding productive labor basis and idle payroll expense
on top of the same accrued wages. It reconciles raw-stock consumption to production, finished
inventory to production less sales, supplier tariff receipts to freight and configured utility
and disposal charges, and citywide cash before and after the run. It reports the payroll
contract end and unpaid balance, daily sales coverage, and whether factory raw stock was
consumed. Shortages propagate from the opening stock; there are no scheduled free refills or
fixed sales quantities.

This is a finite source-level integration scenario, not an economic calibration or proof
that the city is self-sustaining. A run with sales for fewer than 28 days must be reported
as a shortage/continuity limit, not as success. It is ordinary operation and road service,
not bankruptcy, insolvency, or a successful company exit.

## Verification status

The scenario and its checks are **not run** in this candidate. Only Rust 1.85.1 formatting
and static diff checks have been performed; Cargo, CLI, and model execution remain pending.
