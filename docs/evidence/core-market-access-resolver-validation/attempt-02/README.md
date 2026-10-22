# Market access resolver validation, attempt 02 (passed)

Frozen after the attempt-01 failure was diagnosed and corrected. The resolver now classifies an assignment as stale before validating the edited opening, so an authorization revocation after assignment yields `StaleAssignment`. `source/frozen-source.tar.gz`, `SHA256SUMS`, `resolver.patch.gz`, `git-status.txt`, and `base-head.txt` bind the source and locked dependency snapshot for this attempt.

Toolchain: Rust 1.85.1. All commands used `/Users/frankqin/dev/MyCity/target`, `--locked --offline --release`; no independent target was created.

- Targeted integration tests: `cargo test -p mycity-core --test market_access --locked --offline --release -- --nocapture` — 16 passed, 0 failed (`raw/market-access-tests.log.gz`).
- Strict crate checks: `cargo clippy -p mycity-core --all-targets --locked --offline --release -- -D warnings` — passed (`raw/clippy.log`).
- Existing market access example: built once, SHA-256 and canonical hardlink recorded under `fixed-binaries/` and `/Users/frankqin/dev/MyCity/work/candidate-binaries/core-market-access-resolver/attempt-02/market_access`.
- CLI scenarios (`raw/cli-scenarios.jsonl`): normal and cross-city each sold 200; road closed sold 0; bottleneck sold 40 and left 160 market demand unmet; expanded capacity sold 200; unrelated cross-city OD did not block local sales (200).

All example outputs say `pureplan_not_settled` and `engine_commit_required=true`. This validates the resolver and proposal path only; it is not actual Town Engine settlement, whole-city performance, pressure testing, or 16 GB device acceptance.
