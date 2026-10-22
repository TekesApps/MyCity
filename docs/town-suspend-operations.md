# Town business operational suspension

`TownSuspendOperations { profile_id, company_account }` is a System-only, one-way
operational stop for an account already configured as an employer or shop in the
world's town-market profile. It is accepted only at a fully completed day boundary
whose `MarketState::settled_day` matches the Timeline day. The action records only the
account and suspension time in bounded MarketState rows. An empty table is omitted from
serialization so legacy empty-state JSON remains byte-shape compatible.

For every Timeline payroll paid by that account, the transaction invokes the existing
`EndForExit` primitive in the same prepared operation. Any invalid payroll cursor or
unresolved running boot, construction, or exchange shipment blocks the entire stop. A
replay or a later repeated stop for the same account is a no-op. Existing reservations
explicitly owned by the account also block the stop. The action does not
transfer money or alter principal, inventory, accrued/paid wages, taxes, or the terms of
remaining liabilities. Existing claims and assessed but unpaid fees continue through
the existing settlement paths. No arrears receive priority and no assets are frozen.

Each later town settlement first validates the original configured shops, quotes,
recipes, and any immutable transport proof. It then excludes suspended operators' shops,
recipes, and quotes from the economic planner. Other employers and shops continue to
settle; the Store fixture uses a second independent shop and household, and checks that
the suspended shop's stock does not move while the other shop sells after the stop.
`AddPayroll` and identifiable new commitments involving a suspended account are
rejected: new Lend/Claims involving either party, Exchange Dispatch involving seller,
buyer, or carrier, Boot Start, Construction Start/Work, and account-authored capacity
reservations. Existing obligations remain payable. Capacity leases created by an unscoped
System action retain their existing municipal/citywide meaning; because the action carries
no beneficiary, suspension neither freezes them nor treats them as authority to restart the
suspended business. New construction entry including `BeginDemolition` is rejected for a
suspended owner/contractor so it cannot create work that is then blocked from advancing;
cleanup commands remain available. There is not yet a universal beneficiary-linked
reservation or asset-freeze model.

This is not a legal insolvency decision, bankruptcy process, liquidation, or `CompanyClose`.
There is no resume operation, asset transfer freeze, shareholder distribution, or final
debt-clearing gate. `Transfer` and `MoveStock` remain available under their existing
authorization rules. Claims do not encode their legal source, so future court-ordered or
statutory obligations cannot be distinguished from a new voluntary claim here. Likewise,
`ReserveCapacity` has no beneficiary field; an unscoped System reservation cannot be
associated with a suspended business and is outside this guard. These are explicit
follow-up integration limits, not guarantees of a complete business exit.

## Source verification status

The example keeps ordinary mode as the default and adds optional `--loss`, which changes
only the recipe's existing `utility_cost_per_output_q` from 1 to 10,000. It measures the
business projection from account cash, owned stock basis, unpaid Timeline payroll, and
unpaid Timeline claims before day one, after the completed first-day settlement, and after
the next day. The loss mode must observe a negative pre-suspension change before issuing
`TownSuspendOperations`; the high utility cost must suppress production while producing
positive accrued and idle-labor expense. It also checks no new labor or stock movement for
the suspended company, continuing sales at the second shop, an unchanged existing due-later
Timeline payable, and unchanged citywide cash. The fixture has no `Lend` or cash borrowing,
dividends, or asset transfers; it carries one fixed Timeline payable with no cash advance.
The projection is intentionally limited to those account and Timeline values, not a full
enterprise income statement. Reported amounts are cents. Stopping remains ordinary
operational suspension, not a legal closure.

Run60 ordinary and run61 `--loss` CLI both exited 0. Each checkpointed after the two-day run,
closed the Store, and reopened with the same rules; stdout reports `cold_reopen_verified=true`
and a persisted state hash/sequence. Ordinary mode recorded no equity change after stopping;
loss mode recorded the intended equity decline before stopping. In both, the suspended shop's
stock stayed unchanged, the other shop sold 10 units on day two, city cash was conserved, and
`legal_company_close=false`.

The added core tests cover real Timeline payroll and TownSettle behavior, preserving cash,
stock, and paid wages at suspension, rejection of new payroll, continuation of an unrelated
employer, cross-system rejection of future suspension markers and active/unbilled payroll,
and the construction-entry guard. The Store fixture now includes two independent shops and
checks day-2 sales from the continuing shop alongside unchanged suspended-shop inventory.
The Store loss/ordinary tests appeared in run23 and were included again in run72's final
core/storage all-target release test, which exited 0 with source unchanged. These are bounded
fixture checks; they do not imply legal closure, universal beneficiary-linked asset freezes,
or full financial liquidation. Run69's stale shared-cache compile failure, run70's targeted
mycity-core cache cleanup, and run71's epoch 3/3 result remain in the evidence history; run72
is the final all-target regression. Formal01 ended early under the pressure guard; 64MiB,
full M0 and actual 16GB device acceptance remain unverified.
