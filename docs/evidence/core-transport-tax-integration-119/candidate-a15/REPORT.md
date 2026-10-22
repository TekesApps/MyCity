# Transport-settlement validation attempt 01

- Source branch: `codex/core-transport-settlement`
- Frozen source commit: `a15ca9745fd599be62b042b82310e7b8290974e5`
- Base: `d14eb41999fad8d2d3709b86ff10f4fbda33ecb6` (`v0.5.115`)
- Toolchain: Rust/Cargo 1.85.1; locked dependencies, offline mode
- Shared build target: `/Users/frankqin/dev/MyCity/target` -> `/Volumes/HP P900/dev/MyCity/target`
- Validation evidence and Store fixtures live on `/Volumes/HP P900` under `/Users/frankqin/dev/MyCity/work/transport-settlement-attempt-01/`.

## Final commands and results

- `cargo +1.85.1 fmt --all -- --check` and `git diff --check`: passed (`fmt-check.log`).
- `CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo +1.85.1 clippy --locked --offline --release -p mycity-core -p mycity-storage --all-targets -- -D warnings`: passed (`strict-clippy-retry-03.log`).
- `CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo +1.85.1 test --locked --offline --release -p mycity-core -p mycity-storage`: passed, 349 tests total, 0 failures (`final-core-storage-tests.log`).
- Four real Store CLI runs used `cargo +1.85.1 run --locked --offline --release -p mycity-storage --example town_transport -- <scenario> <new-store-dir>` and exited 0. Day-one actual sales: normal 100, closed 0, bottleneck 40, expanded 200. Logs: `cli-{normal,closed,bottleneck,expanded}-final.log`.
- Each final CLI world also exercised next-day issuance, same-day double-settlement rejection, proof mutation conflict, proof publication and Applied response-loss recovery, cold checkpoint restore and archived operation replay, missing/tampered historical proof rejection, and low-capacity rejection without state/cursor/proof-file/emergency-reserve changes.
- Retained release example executable: `town_transport-release`; SHA-256 `908732e4c3a004c0a3f4f034b97505f1dd09da4dbe8b07823a092150c4cce7e0`.

## Failed attempts retained

- `core-tests.log`: first compile found the newly added optional transport argument missing from one internal test call. Its initial tee pipeline did not use `pipefail`; do not treat the shell pipeline exit as a test pass. The raw compiler error is preserved; final combined test command above uses `pipefail`.
- `storage-tests.log`: first compile exposed implementation/API wiring omissions; preserved.
- `storage-tests-retry-02.log`: uncovered non-transport cold-open regression; fixed and final full package tests passed.
- `cli-normal.log`: negative CLI helper incorrectly treated the expected Store rejection (`town_transport_requires_verified_proof`) as scenario failure; the positive Store-owned proof path then passed on retry.
- `cli-closed.log`: fixture mutation assumed at least one served edge; changed to corrupt a scalar assignment total, and final closed run passed.
- Strict Clippy initial/retry logs are kept; final strict run passed.

## Limits

This is functional Store/CLI verification on the available host, not a performance or memory acceptance run. New transport-profile/proof/resolver simultaneous live heap and incremental process memory remain unmeasured (`not_run`); no 16 GiB-device claim is made. The profile is `SingleTownExclusive`, fixed daily demand, and not a global multi-city allocator. Utility and tariff inputs remain explicit fixture/profile substitutes. No UI, AgentKit, GPU, or runtime road-capacity construction effect was implemented here.
