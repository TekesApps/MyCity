# Core economy validation

Source freeze: `0f2b1b6228869c20417280f218e1111758ea7bd14a1b197afa9b6203d190f7e3` (SHA-256 of `source.zip`). Git base `1f12916fe53512f6df74d6d1f12b01384f4d34ad`; the archive includes Cargo.lock, exact source/fixture/docs and binary git diff.

- Rust/Cargo 1.85.1, shared target `/Users/frankqin/dev/MyCity/target`, `--locked --offline`.
- `town_economy::tests`: 10 passed, 0 failed (68 unrelated tests filtered).
- Release example build: passed.
- `mycity-core --all-targets` strict Clippy with `-D warnings`: passed.
- Fixed release example ran baseline, missing cash, stockout, closed road, energy price shock, and demand drop fixture scenarios. Every output labels itself `pureplan_not_settled` and `fixture_supply_not_integrated`; details and hashes are in `scenario-report.json`.
- Candidate binary is a hardlink to the shared-target release example, SHA-256 `e074de5ec75335035c319995fafaa1dd5435eb0c5e2916a1b7dff0400277d7af`.
- The earlier source freeze `b9d9a33e...` has retained raw evidence: attempt 01 accidentally ran Cargo from the canonical checkout and is invalid for this branch; attempt 02 ran the economy branch (10 tests and example build passed) but Clippy found two diagnostics. Both are preserved.
- Production/consumption atomic Engine action remains unimplemented; these results do not establish a settled 28-day loop.
