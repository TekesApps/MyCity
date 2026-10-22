# Dynamic price ceiling validation

Frozen source SHA-256: `c0b38ec624f6d4ab90460fd1551b8519250c6d3ad498a26b323127fd5cff9889` (`source.zip`). Base commit: `d6564ae8513080f34da14600c7c07fca3073bdca`; validation includes uncommitted planner price fix plus docs update, later committed as a standalone fix.

- Release unit tests: 15 passed, 0 failed; 68 unrelated tests filtered.
- Release example build: passed.
- `mycity-core` release all-target strict Clippy (`-D warnings`): passed.
- Fixed binary: `work/candidate-binaries/core-economy/c0b38ec624f6d4ab90460fd1551b8519250c6d3ad498a26b323127fd5cff9889/town_economy`, SHA-256 in `binary.sha256`. It is a hardlink to the shared release example, same inode, 2 links, 730736 bytes.
- Six fixed-binary JSON fixture scenarios ran successfully: baseline, missing cash, stockout, closed road, energy price shock, demand drop. Raw outputs explicitly say `pureplan_not_settled` and `fixture_supply_not_integrated`.
- Price counterexamples include empty cheap offer, limited cheap inventory before expensive offer, route capacity exhaustion, fee shock preserving natural reference need, and deterministic input ordering.
- Attempt `failed-6f7a.../attempt-01` is retained: the first test run found stockout misclassified as `NoDemand`; the correction was applied, a new source zip frozen, and the complete validation was rerun.

This validates only the deterministic read-only planner against fixtures. Engine production/consumption atomic settlement, live transport/shared route supply, 28-day settled operation, and economic calibration remain unverified.
