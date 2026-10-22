# 28-day town market scenarios

`cargo run -p mycity-core --example town_market -- [options]` runs 28 game days through
the real Engine `Timeline` and `TownSettle` commands. With no options it keeps the original
base case. Scenario defaults can be overridden to support bounded parameter sweeps:

```text
--scenario base|cash-shortage|raw-shortage|no-route|capacity-cut|energy-price-rise|demand-drop
--household-cash N       --company-cash N       --raw-quantity N
--daily-need N           --route-reachable true|false
--route-capacity N       --production-utility-cost N
--debt-principal N       --debt-due-day 1..28
```

All money values are integer cents and market/inventory values are integer profile units.
The default profiles are encoded in the example source so the same invocation is repeatable.
`--scenario` chooses an initial profile for the whole run. `demand-drop` therefore compares
the same 28-day Engine loop at a lower fixed household need; it does not claim a mid-run
demand shock because the current genesis-bound MarketProfile has no update command.

The route reachability/capacity and utility availability/cost fields are explicit profile
substitutes. The output labels them `profile_substitutes` and reports `live_transport=false`
and `live_utility=false`. A route outage blocks delivery through the town-market planner;
it is not a live road graph. A utility cost increase changes the real settlement cash flows
under this fixed profile, but it does not model a utility network or its capacity.

Daily wages are accrued and paid by the existing Timeline payroll. The example also creates
one real Timeline loan from Treasury to the company and attempts to settle its due claim
daily from day 14 onward. Its disbursement, paid principal and outstanding balance are
reported. The current model has no tax policy/rate or tax assessment action, so output says
`tax_policy=not_configured`; generic Timeline claims are not presented as computed taxes.

Each run reports opening and closing cash by account, cash deltas, raw and product quantities
and bases, production, actual sales, reference need, market demand, unmet market demand,
payroll accrual/payment/arrears, debt, and settlement costs. It checks aggregate cash
conservation, raw input-to-output/waste reconciliation, product inventory/sales reconciliation,
and labor-cost reconciliation. An initial capital injection is visible in the opening cash
and is not evidence of sustainable operation. Operating profitability and parameter balance
remain uncalibrated.

The scenario envelope is bounded to one household group, one employer, one recipe, one
shop, one delivery quote, one payroll row and one debt claim over 28 settlements. The CLI
prints the Engine's existing owned-capacity measurement against its existing cap; this is not
a total process-memory measurement. Temporary JSON decoding allocations and stack space for
the aggregate counters are outside that counter and still require the M0 process-level
measurement before claiming an overall memory budget.

The source-level CLI regression checks invoke the built example executable directly:

```text
python3 crates/city-core/tests/town_market_scenarios.py /path/to/town_market
```

They compare scenario outputs and invariants from the real binary; they do not reimplement
the planner or settlement. On the frozen source in `docs/evidence/town-market-scenarios/attempt-03`,
the release executable passed all seven named cases plus one parameter override.

The measured outputs show the intended propagation. Base sold 280 units and ended with
household cash down 6,160 cents, while the company ended up 5,300 cents from its opening cash.
That household spend exceeded its 22,400-cent wages, so this is not a self-financing city
economy; it still draws on household opening cash. All runs paid the 100,000-cent Treasury
loan due on day 14 and reported zero wage arrears. No tax amount is assessed.

| Profile | Sold | Reference need | Market demand | Unmet market demand | Raw consumed | Utility payments total | Company cash change |
|---|---:|---:|---:|---:|---:|---:|---:|
| Base | 280 | 280 | 280 | 0 | 300 | 300 | +5,300 |
| Cash shortage | 219 | 280 | 280 | 61 | 300 | 300 | -800 |
| Raw shortage | 15 | 280 | 20 | 5 | 15 | 15 | -20,915 |
| No route | 0 | 280 | 0 | 0 | 100 | 100 | -22,500 |
| Capacity cut | 84 | 280 | 280 | 196 | 100 | 100 | -14,100 |
| Energy price rise | 280 | 280 | 280 | 0 | 300 | 300,000 | -294,400 |
| Demand drop | 56 | 56 | 56 | 0 | 100 | 100 | -16,900 |
| Parameter override: capacity 2, need 5 | 56 | 140 | 140 | 84 | 100 | 100 | -16,900 |

For a closed route, executable `market_demand` is zero even though natural/reference need is
280; therefore check the reference need and zero sales to observe inaccessible demand. The
reported market demand is the subset the planner can serve if cash, inventory and capacity
permit. The capacity-cut scenario exposes 196 units of executable market demand left unserved.

Every named profile reported `engine_owned_capacity_bytes=4,100,680` against the existing
4,194,304-byte Engine cap, a margin of 93,624 bytes. This counts Engine-owned capacity only;
the complete process footprint, temporary JSON decoding pages and allocator metadata remain
outside this measure. These results are functional examples, not the M0 scale/performance
acceptance.
