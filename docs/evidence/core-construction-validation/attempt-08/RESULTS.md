# Construction Engine/Store integration validation

Source freeze: `source.zip`, SHA-256 `865bcbf282606e1a22a123855e76d2fb35ebddd2d255f8b8c6d2462d3490c211`.
Runtime: Rust/Cargo 1.85.1, release profile, locked and offline, shared target `/Users/frankqin/dev/MyCity/target`.

- `mycity-core --test construction`: 7 passed, 0 failed (explicit test target; not the earlier zero-test filtered invocation).
- `mycity-core --test construction_engine`: 5 passed, 0 failed.
- `mycity-storage --test construction`: 1 passed, 0 failed.
- Actual `mycity-core` Engine example: reached `Operational`; output recorded in `example.log`. The executable is hard-linked at `/Users/frankqin/dev/MyCity/work/candidate-binaries/core-construction/865bcbf282606e1a22a123855e76d2fb35ebddd2d255f8b8c6d2462d3490c211/construction` and its SHA is in `example-binary-SHA256SUMS`.
- Strict Clippy for all targets in core and storage: passed with `-D warnings`.

Atomicity coverage verifies no domain/account/stock/resource/lease mutation after insufficient site capacity, insufficient spendable cash, insufficient materials, or durable-hook rejection. Recovery coverage verifies durable commit/retry idempotency, non-Draft checkpoint resume through work and inspection, cold WAL reopen and typed construction receipt decoding, and exact occupied site lease restoration. Size assertions keep Action <=128 bytes, JournalRecord <=4096 bytes, Project <= its 2 KiB slot, and Engine owned capacity <= the existing 4 MiB budget.

The full demolition/waste lifecycle is still an explicit fail-closed gap: no authoritative waste quantity/receiver ledger is present, so Engine integration rejects demolition, waste acceptance and close. This validation does not claim the full construction line complete.

Failed attempts remain separately frozen with raw output in `attempt-01-failed`, `attempt-02` (initial compilation error), `attempt-03-failed`, and `attempt-05-failed` through `attempt-07-failed`; no failed result is overwritten or counted as a pass.
