# Market access resolver validation, attempt 01 (failed)

Frozen before validation on branch `codex/market-access-resolver`, based on `267554930f5906453f6d2fafe181deea12aba5a4` (`v0.5.112`). The source archive includes `Cargo.lock`, the Rust toolchain pin, relevant manifests, resolver/transport/economy source, the targeted test file, and the existing pure-plan CLI example. `SHA256SUMS` and `resolver.patch.gz` bind the exact pre-test worktree state.

Toolchain: Rust 1.85.1. The targeted command used shared canonical target `/Users/frankqin/dev/MyCity/target`, `--locked --offline --release`:

`cargo test -p mycity-core --test market_access --locked --offline --release -- --nocapture`

Result: 15 passed, 1 failed. The revoked-entrance stale-assignment case received `InvalidTransport` because validation happened before comparing the old assignment fingerprint; this failed attempt is retained in `raw/market-access-tests.log.gz`. No pressure or whole-city performance validation was run.
