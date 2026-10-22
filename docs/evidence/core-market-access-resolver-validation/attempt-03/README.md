# Market access resolver validation, attempt 03 (final formatted source)

This attempt starts from the attempt-02 passing implementation and applies `rustfmt` to the targeted test imports. The resolver behavior is unchanged. The complete frozen source/locked dependency bundle and hashes are in `source/`.

Toolchain: Rust 1.85.1. Tests and strict Clippy used the shared `/Users/frankqin/dev/MyCity/target` with locked offline release dependencies. No independent target was created.

- `cargo test -p mycity-core --test market_access --locked --offline --release -- --nocapture`: 16 passed, 0 failed (`raw/market-access-tests.log.gz`).
- `cargo clippy -p mycity-core --all-targets --locked --offline --release -- -D warnings`: passed (`raw/clippy.log`).
- The six existing CLI scenarios were run against the fixed example binary recorded in attempt-02. The example source and binary were unchanged by the formatting-only test import edit; its outputs remain in attempt-02 and explicitly identify themselves as pureplan proposals, not settlements.

This is resolver and proposal-path validation only. Town Engine settlement, whole-city performance, pressure testing, and physical 16 GB device acceptance remain untested.
