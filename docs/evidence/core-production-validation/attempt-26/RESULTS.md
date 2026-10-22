# Attempt 26 — Store 28-day persistence and Engine perturbation tests

Source: `codex/core-production` at base commit `93e3058242feb9aa9f0ea84f2ed3814a43a38f60`; exact test source snapshots and patch are beside this file. No other worktree sources were included.

## Verified

- Engine business perturbation test: 1 passed. It runs actual Timeline payroll and atomic `TownSettle` for household cash exhaustion (normal wage then recorded transfer), missing raw material, closed route quote, higher delivery surcharge, reduced need, and higher producer utility cost.
- Store integration target: 3 passed. It includes Applied reply-loss cold recovery/idempotent retry, a 28-day run with checkpoint and cold reopen every seven days, and a partial paid shift while inventory remains (idle wage, no recipe remainder).
- `cargo clippy -p mycity-core -p mycity-storage --all-targets --release --locked --offline -- -D warnings`: passed.
- rustfmt check on the two changed Rust files: passed.
- Direct execution of the hard-linked acceptance test binaries: Engine 1/1, Store 3/3.

The 28-day fixture deliberately uses a 1,000,000-cent opening household balance and 100 units/day reference demand so each 100-unit daily lot sells out and the final 60-second worked interval can advance the recipe's arithmetic remainder. This is a test fixture, not evidence that the original 10-unit household economy is balanced or sustainable.

Fixed-binary stdout reports 703 contiguous Store operation receipts and 28 TownSettle receipt IDs; final payroll is 777,660 seconds, 21,601 accrued/paid cents, Timeline payroll fractional remainder 2,400, Town recipe arithmetic remainder 6,000. Final-day 60 seconds generate no product and accrue one cent as idle labor. The final snapshot shows raw stock 97,300 from 100,000, product inventory zero, and 27 full-day outputs/sales of 100 units each; the last day is the partial idle shift. The test adapter still uses genesis-bound delivery and utility-price substitutes; it does not prove live transport/utility integration, economic equilibrium, long-run sustainability, or performance.

## Preserved failed attempts

- attempt-20: low-cash fixture used an invalid supplier payroll payee; Timeline rejected it.
- attempt-21: Store snapshot typed lot ID as u64 instead of the actual u32.
- attempt-22: expected partial production remainder while unsold inventory correctly kept the recipe idle.
- attempt-24: strict Clippy rejected an eight-argument test helper.

Each failed source and raw stdout/stderr remains in its original attempt folder. Attempt 23 also preserves the first passing daily-sellout fixture before the idle-partial test was added. No failure evidence was overwritten.

## Temporary-file handling

Tests used the registered 256 MiB staging directory at `/Volumes/HP P900/dev/MyCity/work/.tmp/core-production-store28-20261022` through `TMPDIR`. It held only its intent marker after tests; no test process or open file held it. The directory has been removed. Cargo reused the existing shared target symlink `/Users/frankqin/dev/MyCity/target`; no new target was created. The attempt-26 executables under external `work/acceptance-binaries/core-production/attempt-26` are hard links to those target binaries, verified by matching inode and SHA-256.

This is functional verification only; no performance benchmark or scale claim is made.
