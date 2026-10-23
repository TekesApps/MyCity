# D28 computation contract

This contract describes the exact bounded input and event order exercised by the D28 Store example. The executable and reports are identified in [STATUS.md](STATUS.md). Values are fixture parameters in cents and quantity units; they are not calibrated market recommendations.

## Fixed inputs

- One household, one employer/company, one worker payroll row, one recipe, one shop, one output stock, and one company tax rule.
- The company starts with 600,000¢ and 280q raw inventory with total basis 2,800¢. This is finite opening capital; the scenario does not replenish raw input.
- The base household starts with 200,000¢. The pressure variant changes only its opening cash to zero, so its total system cash is 2,800,000¢ instead of 3,000,000¢.
- A government/treasury contribution of 10,000¢ issues 100 shares. The person holds 6,028 opening shares backed by the company's 600,000¢ and 2,800¢ raw basis. The combined opening paid-in capital is 612,800¢.
- A 1,000¢ supplier loan is made to the company and is repaid once on or after day 14 when due. No interest is configured.
- Daily payroll is 100¢ per hour for an eight-hour day, and the payroll ends at day 28. Each daily worker accrual is 800¢.
- The recipe can produce at most 10q per service day. One output unit consumes one raw unit, incurs 1¢ recipe utility cost, and carries 80¢ labor basis when a full 10q shift is capitalized. With raw basis of 10¢ per unit, a full-run product basis is 91¢ per unit.
- The shelf price is 100¢/q. The fixed profile adds 1¢ freight and 1¢ buyer utility surcharge, making the household's landed price 102¢/q. Household reference need is 10q/day. The delivery quote has fixed source 77 and capacity 10q; it is not calculated from road geometry.
- The tax rule assesses 50% of positive realized daily margin. Recipe utility cost is fixed in the recipe/profile and is not supplied by a live Boot utility model.

## Authoritative ordering

The Store is created from the fixture, then the treasury makes the equity contribution, the Timeline is enabled, payroll is registered, and the supplier loan is issued. For each calendar day 1 through 28, the runner advances the existing Timeline to the day boundary, performs a due loan settlement on day 14 if needed, and invokes exactly one TownSettle for that completed day. It checkpoints and cold-opens after day 14 and day 28, comparing Store state hashes. It then advances to day 29 and settles once more so due tax claims are processed; payroll has ended, so day 29 accrues no new wage. After a further checkpoint/cold-open, the existing retained earnings determine whether a dividend is accepted. A government dividend receipt is transferred to the supplier as a real fiscal expenditure. Finally, the scenario issues BeginExit, SettleExit, and CloseExit through the Store, with checkpoint/cold-open boundaries between operations.

Each durable command uses the current Store revision and a distinct operation id derived from the committed sequence. Business state, money, stocks, timeline claims, tax cursor, equity register, and exit state remain in the same Engine/Store transaction path. The verifier opens a supplied copy and executes zero business commands; Store open itself may perform recovery, which is why it must not be pointed at the preserved original.

## Non-aggregatable daily boundaries

The result depends on daily scheduling, so a 28-day aggregate formula cannot replace the event sequence:

1. Timeline payroll accrual and payment occur at the day boundary before that day's market settlement. Day 29 is outside the payroll term.
2. Production uses a single output-stock lot. If any previous output remains unsold, production is skipped for that day; the stock must be completely cleared before another production run. Sales and next-day production therefore depend on the previous day's cash, purchases, and inventory.
3. TownSettle records actual sales proceeds, FIFO goods basis, produced labor basis, and accrued wages for that service day. Capitalized labor is not counted again as idle expense; accrued wage minus labor capitalized into produced goods is the idle labor expense.
4. For each service day, realized margin is `sales proceeds − sold goods basis − (accrued wages − capitalized production labor)`. The taxable base for that day is `max(realized margin, 0)`. The 50% assessment is floored in cents independently for each day (`taxable base × 5,000 / 10,000`), then recorded as a Timeline claim due on the following day. A negative day's margin does not offset an earlier or later positive day's daily tax base.
5. The tax cursor accumulates signed after-tax margin by day. A negative retained balance blocks positive dividend distribution. The day-14 loan maturity and day-29 tax payment also depend on actual calendar boundaries.

These are per-day settlement rules, not a claim that every underlying city subsystem must run at the same granularity. Any future batching must preserve the ordered daily state transitions and exact integer rounding; this scenario does not test such an optimization.

## Independent arithmetic checks

### Base

At full daily demand, 28 × 10q gives 280q sales and production. Gross seller receipts are 28,000¢. Full sold-goods basis is 2,800¢ raw + 280¢ recipe utility + 22,400¢ capitalized labor = 25,480¢. No idle wage remains, so pre-tax margin is 2,520¢ and daily tax sums to 1,260¢ (45¢ each day). At day 29, payroll is over; the tax claims are paid and retained earnings remain 1,260¢. Largest-remainder distribution allocates 1,239¢ to the person and 21¢ to the government. The latter is then actually transferred to the supplier. With the 1,000¢ loan repaid and inventory empty, exit returns 612,800¢ capital and records no liquidation surplus.

Observed base totals: all cash returns to 3,000,000¢ across accounts; produced/sold 280q; stocks end at zero; tax assessed/paid 1,260¢; dividend 1,260¢; government fiscal transfer 21¢; exit is Closed. The independent checkpoint oracle and cold-open reader agree on final state hash, sequence, balances, register, and checkpoint digest.

### Household cash pressure

With the household's opening cash at zero, the 800¢ daily wage and 102¢ landed unit price constrain purchases. The nonempty output slot blocks subsequent production until its prior lot sells out. The exact sequence in the run is 219q sold and 220q produced; 60q raw and 1q product remain. The 219 sold units carry 19,929¢ goods basis (91¢ per unit). Of the 22,400¢ accrued wages, 17,600¢ is capitalized into output and 4,800¢ is idle labor expense. Cumulative signed pre-tax margin is −2,829¢. Positive days still create 22 daily tax claims totalling 938¢ because tax bases are floored at zero independently each day; retained after tax is −3,767¢.

The household ends with 62¢, the loan has been repaid, no dividend is accepted, no capital is returned, and the company remains Liquidating because stock remains. Account cash remains conserved at 2,800,000¢. The fresh copy reader and independent checkpoint oracle agree on the terminal state.

## Explicit non-claims

The route and utility settings are fixed profile inputs. The run does not demonstrate live road graphs, capacity changes, neighborhood accessibility, Boot electricity/water, labor-market matching, replenishment of raw materials, changing prices, population movement, or long-horizon self-financing. The 16 GB performance gate is unrun. Per-process RSS and owned-capacity numbers are observations for this small fixture, not scale guarantees.

## Stable intervals, ownership, and reference path

Between durable commands, published Store revision and immutable fixture inputs remain authoritative; no background runner accrues money. Commands publish receipts with related account, inventory, tax, equity, and exit effects. The runner only schedules commands and owns report/checkpoint temporary buffers, never a second ledger. Engine owns Timeline/Town state; Store owns WAL/checkpoints/recovery. Report vectors and independent oracle rows are acceptance-tool memory, separate from model-owned capacity.

Future equivalent batching may skip only intervals proven free of calendar events, changes in inventory, cash affordability, resource capacity, ownership, tax, loan maturity, or exit/dividend commands. Crossing the 102-cent affordability threshold, empty output slot, debt maturity, payroll end, or retained-earnings sign requires ordered exact settlement. Cents floors and shareholder remainder order remain exact. No batching optimization is implemented or validated here.

Reference paths: `crates/city-storage/examples/town_equity_28d.rs`, its `support/town_equity_28d.rs`, corresponding integration tests, and this directory's independent `check_checkpoint.py`. `commands.json` and `fixed-cli.json` bind commands/source/executable. Complete recovery originals and cold-reader copies remain on external SSD at STATUS paths; digests do not replace restorable bytes.
