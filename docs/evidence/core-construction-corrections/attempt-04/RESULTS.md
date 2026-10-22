# Construction binding and authoritative material-basis corrections

Base commit: `aff775b72b3475bcb09a4866f78d9225fb378660`. Final correction source freeze SHA-256: `f123fe43f35b1ba0cd3df828be17639b5762a5e9e56b26f4f75403ca58b0f073`.

Runtime: Rust/Cargo 1.85.1, release, locked/offline, shared target `/Users/frankqin/dev/MyCity/target`.

- Pure construction model: 7 passed, 0 failed.
- Construction Engine integration: 7 passed, 0 failed. Includes generic ReleaseCapacity rejection with unchanged project/account/stock/resource/lease state and successful checkpoint restore followed by Work; high estimate/low actual lot basis succeeds under budget; low estimate/high actual lot basis fails atomically.
- Construction Store integration: 2 passed, 0 failed. Includes actual Work cash payment and inventory-lot consumption committed together, error injected at `WritePoint::Applied`, WAL cold reopen, same-operation retry, and no duplicated cash/material/work progress.
- Engine example: exit 0, project Operational.
- Strict Clippy: `mycity-core` and `mycity-storage`, all targets, `-D warnings`, passed.

The baseline construction acceptance and source bundle remain preserved under `docs/evidence/core-construction-validation/attempt-08/`; the compile failure from an earlier Store-test attempt remains in `attempt-02-failed/` with its exact source and output.
