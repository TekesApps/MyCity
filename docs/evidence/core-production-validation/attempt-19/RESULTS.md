# Core production market validation

Source freeze: `source.zip` SHA-256 `ee0ed35dc2747b9593bfcdfc00553863404d9d4f0b064622f463b2662a7a15f5`.

This is a bounded Engine/Store prototype for one genesis-bound town profile. It uses the current Engine accounts, stock lots, Timeline wages, checkpoint/WAL and receipt archive. The market/route inputs are fixed test-profile substitutes; they do not yet consume the real transport market-access adapter. It demonstrates actual transactions, not a calibrated or sustainable economy.

## Verified behavior

- `town_economy::tests`: 17 passed; actual-time worker scaling, payroll split, dynamic prices, stock/route/utility constraints, and planner reconciliation.
- Engine town settlement tests: 4 passed; actual lot basis, partial worker seconds and 100/101 wage allocation, one atomic settlement/retry, and durable-hook failure leaving no published mutation.
- Weekly payroll catch-up: 1 passed; daily settlement accepts accumulated weekly payment under the cumulative paid/accrued invariant.
- Store town market recovery: 1 passed; actual WAL `Applied` lost-reply recovery, same-operation retry, checkpoint cold open, archived receipt decode and replay without duplicate settlement.
- Existing construction storage tests: 3 passed, including fixed v106 Store/WAL/checkpoint/receipt reopening. Existing v106 fixture bytes are copied byte-for-byte from `../construction/docs/evidence/core-construction-compat/attempt-01/`; matching source and copied hashes are in `attempt-16/raw/compat-evidence-*.sha256`.
- Optional-field reused-slot checks: the construction and town restore tests each passed 1/1. `None` town configuration is omitted from legacy views and restoring a checkpoint without the town field clears a reused slot.
- Actual Engine 28-day CLI: passed all cash, stock quantity, output sales and labor-cost conservation assertions. Exact output is in `attempt-19/raw/town_market_28_day_cli.log`.
- Strict Clippy across all targets for `mycity-core` and `mycity-storage`, release/locked/offline, `-D warnings`: passed.

## 28-day fixture totals

The fixture starts with cash `[treasury 1,000,000; household 200,000; producer 600,000; supplier 1,200,000]`, total 3,000,000, and a raw inventory lot of 100,000 quantity/basis. Ending cash is `[1,000,860; 193,840; 605,300; 1,200,000]`, with the same total 3,000,000. Deltas are `+860, -6,160, +5,300, 0`.

The household paid 28,560 (`28,000` goods, `280` freight, `280` delivery utility) and received 22,400 wages. Its 6,160 cash decline is funded by initial household cash, so this fixture does not prove long-term purchasing-power sustainability. The producer's cash increase is not a claim of balance or profitability after full real-world costs.

Production was 300 units with zero waste; raw use was 300, leaving 99,700. Sales were 280 and final product inventory 20. Raw basis went from 100,000 to 99,700; remaining product basis is 200. Capitalized labor is 2,400 and idle labor expense is 20,000, exactly reconciling 22,400 accrued wages. The actual lot-basis conservation implies sold COGS basis of 2,500 (`100,000 + 2,400 - 99,700 - 200`).

## Bounded capacity

The exercised Engine image reports 4,100,680 owned bytes against the unchanged 4,194,304-byte Engine cap. The world reports 90,864 bytes against 131,072 bytes. The fixed town profile contributes 41,856 owned bytes; its inline world slot is 42,048 bytes. `Action` measured 80 bytes against the pre-existing 128-byte limit; no cap was increased. These are bounded model capacities, not a full 16 GB workload measurement.

## Remaining gaps

This finite phase does not complete the requested economic system. It has one fixed profile, limited lot slots, no multi-company bankruptcy/exit, and a test delivery/utility profile rather than the verified real transport adapter. The 28-day loop is a model CLI, not a 28-day persisted Store run. Loss/insolvency, multi-recipe and multi-company interaction, full waste ledgers, and production-to-real-route integration remain unverified. The fixture parameters are not calibrated and cannot establish sustained city solvency.

The initial checkpoint cold-open failure and later fixes are retained in attempts 08, 10, 12, and 13. The complete original failing checkpoint directory (68 KB) remains at `attempt-08/raw/checkpoint-failure-world`; later passing evidence does not replace or erase it.
