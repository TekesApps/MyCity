# Town market scenario validation, attempt 03

## Frozen inputs and executable

- Git base: `6eb888d0e66cfc2862709c06cc0b309aea270bb8` (`v0.5.114`)
- Source bundle SHA-256: `6c9ada27e16aab015ed039be369d55eb6d7ef2b9237d28c58af6b50532553036`
- Rust: `rustc 1.85.1 (4eb161250 2025-03-15)`; Cargo 1.85.1
- Dependencies: `--locked --offline`
- Build: `cargo +1.85.1 build -p mycity-core --example town_market --release --locked --offline`
- Shared target: `/Users/frankqin/dev/MyCity/target` → `/Volumes/HP P900/dev/MyCity/target`
- Executable: `/Volumes/HP P900/dev/MyCity/work/candidate-binaries/town-market/6c9ada27e16aab015ed039be369d55eb6d7ef2b9237d28c58af6b50532553036/town_market`
- Executable SHA-256: `fb141f0c370ddd1dd0637e974b3cdfa4e7c286c8556ca9b8e7bc287e6d72313d`
- Executable format: Mach-O arm64, 1,639,360 bytes
- Cargo release build succeeded. Raw build stdout/stderr and fixed executable are retained.

## Actual CLI results

The Python regression script ran the fixed executable for each named profile and one
parameter override. All eight process exit codes were zero, all cross-scenario assertions
passed, and the CLI's own cash/material/wage/debt reconciliations passed.

| Scenario | Need | Market demand | Sold | Unmet market | Raw used | Utility payments | Company cash delta | Household cash delta |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Base | 280 | 280 | 280 | 0 | 300 | 580 | +5,300 | -6,160 |
| Cash shortage | 280 | 280 | 219 | 61 | 300 | 519 | -800 | +62 |
| Raw shortage | 280 | 20 | 15 | 5 | 15 | 30 | -20,915 | +20,870 |
| No route | 280 | 0 | 0 | 0 | 100 | 100 | -22,500 | +22,400 |
| Capacity cut (3/day) | 280 | 280 | 84 | 196 | 100 | 184 | -14,100 | +13,832 |
| Energy price rise | 280 | 280 | 280 | 0 | 300 | 300,280 | -294,400 | -6,160 |
| Demand drop (need 2/day) | 56 | 56 | 56 | 0 | 100 | 156 | -16,900 | +16,688 |
| Override (capacity 2, need 5) | 140 | 140 | 56 | 84 | 100 | 156 | -16,900 | +16,688 |

All eight runs conserve aggregate account cash. Every run accrues and pays 22,400 cents in
wages, with zero wage arrears. Each run advances the 100,000-cent Treasury loan to the
company, pays the claim in full on day 14, and ends with zero outstanding principal. Tax is
not assessed: this model has no tax policy/action and explicitly emits
`tax_policy=not_configured`.

Base case result: household cash falls from 200,000 to 193,840 cents despite receiving
22,400 cents in wages; household purchases plus route fees exceed that wage income by
6,160 cents. The company's cash rises by 5,300 cents. This example therefore depends on
household opening cash to support the observed spending and does not establish sustainable
household economics or calibrated profitability. The company starts with 600,000 cents and
also receives then repays a 100,000-cent loan.

In the closed-route case, natural/reference need is 280 but executable market demand and sales
are both zero: the planner excludes unreachable delivery from executable offers. In the
raw-shortage case only 20 units enter executable market demand even though natural need is
280; after 15 sales, five of that executable demand remain unmet.

The Engine reports `engine_owned_capacity_bytes=4,100,680` of the 4,194,304-byte cap for all
profiles (93,624 bytes of Engine-cap headroom). Its layout explicitly excludes allocator
metadata and temporary parse/view pages. This is not a whole-process memory observation and
does not satisfy the M0 performance gate.

## Retained records

- `freeze-manifest.json`, `source.zip`, `source.patch`: frozen source and lock inputs.
- `build.stdout`, `build.stderr`: original release build output.
- `scenario-tests.stdout`, `scenario-tests.stderr`: regression runner output.
- `scenario-results.json`: per-run command, exit code, output hashes and quantitative fields.
- External work `town-market-validation/<source-id>/attempt-03/*.stdout|*.stderr`: complete
  per-scenario raw process output, including the parameter override.
- Failed attempt 01/02 build/test logs, source ZIPs and manifests remain intact with their
  respective fixed executable copies outside the checkout.

## Scope boundary

This verifies the 28-day Engine market and Timeline CLI under the explicit profiles above.
Route/capacity and utility fields remain fixed profile substitutes, demand reduction applies
from day one, taxes are absent, and no transport-to-market Engine adapter is exercised. This
does not prove the full economy, a mid-run demand shock, tax policy, real transport/utility
integration, economy balance, M0 scale acceptance, 16-GB behavior, or the eight-hour run.
