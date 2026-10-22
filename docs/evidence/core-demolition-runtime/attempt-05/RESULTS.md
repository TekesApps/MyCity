# Construction Engine/Store demolition lifecycle validation

## Frozen source and runtime

- Git base: `531192db5329b7485202e699c16844a1c8d11b90` (`codex/core-demolition-runtime`).
- Apply `working-tree.diff` to `source-base.tar.gz` to reproduce this exact validation source. Cargo dependencies are locked by the included `Cargo.lock`; all invocations used Rust/Cargo 1.85.1, `--release` where stated, `--locked --offline`, and shared `CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target`, a symlink to `/Volumes/HP P900/dev/MyCity/target` on the HP P900 external SSD.
- `construction-release` is the actual example executable invoked by Cargo in attempt 05; SHA-256 and file mode/size are recorded alongside it.
- No pressure/performance run was made. The test fixture and CLI example are functional evidence only.

## Actual results

- `mycity-core --test construction`: 7 passed, 0 failed, release.
- `mycity-core --test construction_engine`: 9 passed, 0 failed, release. Debug with default test thread stack failed with stack overflow/SIGABRT; the same debug harness passed 9/9 with `RUST_MIN_STACK=16777216`. Logs and exact commands are retained under attempt 02.
- `mycity-storage --test construction`: 4 passed, 0 failed, release. The tests include WAL lost-response retry/cold reopen and the operational-capacity, demolition, waste and close replay path.
- Release example exited 0 and printed `Engine construction closed: project=7 owner_cash=1171200 stock_qty=599000 site_occupied=0 waste_occupied=2`.
- The example exercises filing and approval, game-calendar work and inspection, publication of operating capacity, a consumer lease that makes demolition Start return `Capacity`, release of the consumer lease, approved demolition Start withdrawing the published capacity, demolition work, paid waste receipt occupying receiver capacity, then Close releasing the site lease.
- Strict Clippy passed for `mycity-core` and `mycity-storage` all targets with `-D warnings`.

## Failure history retained

- Attempt 01 linked a stale shared-target library from a different checkout and failed to compile due to old crate metadata. It is retained as an environment/cache mismatch, not counted as source validation. Touching the tracked construction module caused Cargo to rebuild that package; no target cache was deleted.
- Attempt 02 records the debug stack overflow at default thread stack and the successful 16 MiB retry, plus the first release focused test run.
- Attempt 03 records the CLI `AcceptInspection` `InvalidOperation`: its request used an ordinary operation ID instead of the pending effect ID.
- Attempt 04 records `Start` `OperationConflict`: demolition review was incorrectly assigned an effect ID although the review request has no pending effect.
- Attempt 05 is the successful release CLI run. These failures remain intact and are not counted as passes.

## Resource and functional boundary

Construction `Project` remains within its existing 2 KiB state slot and the Engine within the existing 4 MiB owned-capacity assertion in the focused Engine test. The new example uses fixture-owned in-memory objects and bounded lease/resource collections; no new persistent worker, ledger, clock, or secondary transaction system was added. This is a model-contract bound, not an OS process RSS measurement.

This closes the selected Engine/Store demolition and waste acceptance path only. It does not implement resident/owner compensation processes, permits beyond the explicit gates in this fixture, a general construction CLI parser, traffic/economy integration, or a city-wide performance claim.
