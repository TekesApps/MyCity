# Store 28-day and Engine scenario WIP freeze

Frozen without Cargo at the pre-SSD-migration point. This is unverified source, not a passing test result.

- Worktree: `/Users/frankqin/dev/MyCity-core-work/production`
- Branch: `codex/core-production`
- Base HEAD: `93e3058242feb9aa9f0ea84f2ed3814a43a38f60`
- Target comparison: `v0.5.109` (`99a1fcd589bd6429b04f5c36391c53b3257ec496`)
- The two touched base blobs are identical between `v0.5.109` and current `main`; WIP patch targets only `crates/city-core/src/transaction.rs` and `crates/city-storage/tests/town_market.rs`.
- Read-only context check confirmed `productive_market_engine()` and `issue()` surrounding lines in `transaction.rs`, and the existing Store test in `town_market.rs`, are present in both target revisions. The WIP hunks add a parameterized fixture/scenarios and a new Store test without changing other line files.
- No canonical `work`, shared `target`, pressure data, or other worktree was modified by this freeze.

## Verification state

Only `rustfmt +1.85.1 --edition 2021 --config skip_children=true` and `git diff --check` were run before this freeze. No Cargo commands or runtime validation were run. The patch must be validated after migration using the migrated shared target.
