# Creditor-before-equity liquidation residual candidate

Status: finite actual canonical verification in v0.5.154. See [source, failures, final tests, actual CLI and recovery evidence](evidence/core-parallel-154/STATUS.md).

## Existing coverage and the narrow gap

The existing D Store scenarios already exercise actual personal and treasury contributions, sales-derived tax assessment and payment, a dividend from retained earnings, an actual treasury transfer after receiving its dividend, source-backed lending, lost-`Applied` reply recovery, WAL replay, checkpoint reopen, and company closure. The tested dividend/liquidation scenario fully pays its loan before exit and leaves company cash equal to paid-in capital. It therefore does not independently show how a still-future creditor blocks shareholder settlement, then how a post-creditor balance above paid-in capital is classified and allocated.

This candidate adds only that missing path. It reuses the bounded `m0-e0-opening.json` fixture and the existing Store, Timeline, equity register, exit policy, and planner. It adds no ledger, clock, creditor model, or liquidation rule.

## Scenario and independent oracle

The Store starts with 3,000,000 cents across four accounts. The treasury and household each contribute 10,000 cents to the company, issuing 100 shares apiece; the existing person shareholder has 6,001 opening shares. The real first-day market settlement sells 10 units at the fixed fixture price, and the existing one-day payroll, utility charge, and 50% margin-tax fixture produce company cash of 620,145 cents by the fourth completed day. These are scenario parameters, not calibrated rates or an economic-balance claim.

At the fourth-day boundary, the Timeline records a real 1,000-cent loan from account 4 to company 3, due on day 5. Exit is requested by the registered requester and settled by the registered executor. A day-4 exit-settlement receipt must leave the company's 621,145 cents untouched because the source-backed loan remains future-due; shareholder capital-return counters remain zero. On day 5, ordinary market settlement marks the claim processed but does not pay it while the company is exiting: payment must go through the ranked exit settlement. The day-5 `SettleExit` transaction pays the now-due 1,000-cent creditor first, then distributes only the remaining 620,145 cents to shareholders.

The independent final arithmetic is:

- Company immediately before the day-5 exit settlement: `621,145` cents.
- The ranked day-5 exit transaction pays the creditor `1,000` cents and distributes `620,145` cents to shareholders.
- Paid-in capital: `600,100 + 10,000 + 10,000 = 620,100` cents.
- Liquidation surplus: `620,145 - 620,100 = 45` cents.
- Total distribution is allocated by 6,201 shares. The 100-share treasury receives 10,001 cents after the deterministic one-cent remainder; the 6,101-share person receives 610,144 cents.
- The claim remains unpaid after the ordinary day-5 settlement (`paid = 0`, `processed = true`, lender cash `1,199,000`); the ranked exit transaction then records `paid = 1,000` and restores lender cash to `1,200,000`. The loan advance and repayment are internal transfers; total cash remains 3,000,000 cents.

The test injects a lost response at the durable `Applied` point for the blocked day-4 settlement and the successful liquidation settlement, reopens the Store, checks each original receipt, and retries the exact operation IDs. It then checkpoints and cold-opens the closed world, comparing the state hash and replaying the exit receipts. The example prints the cash, capital-return and surplus counters, the cold-open hash, and initial/final owned-capacity counters. The example does not measure native peaks internally; the final root CLI wrapper observed RSS50,577,408B and footprint46,285,184B on the external-SSD32GiB Mac micro scenario. This is not M0/16GB acceptance.

## Scope limits

The delivery quote uses `route_source: 77` from the existing small fixture; this is not live road-network integration. The tax rate, payroll, utility charge, loan size, and share allocation are fixture inputs, not calibrated legal or economic policy. The example does not prove a general insolvency system, multiple creditor classes in a shortfall, market-wide profitability, or 16-GiB capacity. The finite Store/CLI and cold-recovery claims are actually verified in the linked154 evidence; general insolvency, real-road integration and scale/device gates remain unverified.

## Candidate commands

```sh
cargo test --locked --release -p mycity-storage --test equity_creditor_residual -- --exact future_creditor_blocks_equity_then_paid_in_and_surplus_are_distributed_once --nocapture
cargo run --locked --release -p mycity-storage --example town_equity_creditor_residual -- /Volumes/HP\ P900/dev/MyCity/work/equity-creditor-residual-world
```
