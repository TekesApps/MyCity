# Core economy validation evidence

`final-0f2b1b.../attempt-01` records the initial accepted planner source, 10 tests, release example, strict all-target Clippy and six fixture pureplan scenarios.

`final-c0b38e.../attempt-01` validates the dynamic price ceiling fix: 15 tests, release example, strict all-target Clippy, and six fixed-binary fixture scenarios. Frozen source zip SHA-256 is `c0b38ec624f6d4ab90460fd1551b8519250c6d3ad498a26b323127fd5cff9889`. The fixed binary is a hardlink in `work/candidate-binaries/core-economy/<source>/town_economy`; its file hash is recorded with inode/size evidence.

`failed-6f7a.../attempt-01` preserves the first run of the price fix. It exposed a stockout reason regression (`NoDemand` instead of `NoStock`); the fix was applied, frozen again, and the complete validation rerun. Earlier invalid-scope and strict-Clippy failure attempts remain preserved under their original ids.

All CLI scenarios say `pureplan_not_settled` and use fixture supply. Engine atomic production/consumption settlement, live shared transport supply, a settled 28-day operation, and market calibration are not validated by these results.
