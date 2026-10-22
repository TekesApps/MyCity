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

The added core tests cover real Timeline payroll and TownSettle behavior, preserving cash,
stock, and paid wages at suspension, rejection of new payroll, continuation of an unrelated
employer, cross-system rejection of future suspension markers and active/unbilled payroll,
and the construction-entry guard. The Store fixture now includes two independent shops and
checks day-2 sales from the continuing shop alongside unchanged suspended-shop inventory.
MarketState empty serialization and manual receipt decoding have source tests. All are **not run** in this candidate because
the parent task owns the only Cargo/CLI validation window. Store checkpoint/reopen, WAL
replay, response-loss, and end-to-end CLI execution are also **not run** and remain required
before integration.
