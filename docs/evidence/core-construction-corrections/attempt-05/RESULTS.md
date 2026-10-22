# Authoritative construction estimate-overflow correction

Base commit: `91670b9a65596323ebe4018bb2dc907cf2de88c1`. Source bundle SHA-256: `1382d0de97d66cc04aa6723551dd9401478bbddf1333bdef367eb25d2383a001`.

Runtime: Rust/Cargo 1.85.1, release, locked/offline, shared target `/Users/frankqin/dev/MyCity/target`.

The Engine-authoritative `propose_work` path now skips estimate multiplication, conversion, and total-basis addition entirely. Engine reprices the candidate from the actual immutable source-lot offsets and performs the only authoritative budget check. The pure model estimation path remains unchanged.

- Pure construction model: 7 passed, 0 failed.
- Construction Engine: 8 passed, 0 failed. New counterexample uses `basis_per_unit = i64::MAX`, actual lot basis low enough for the project budget; work succeeds with the actual spend of 29,200 cents and actual remaining stock basis of 239,600 cents.
- Construction Store: 2 passed, 0 failed. Includes actual Work cash/material atomic commit, injected Applied lost response, WAL cold reopen and same-operation retry without duplicate settlement.
- Engine example: exit 0, project Operational. The executable is hardlinked under `/Users/frankqin/dev/MyCity/work/candidate-binaries/core-construction/1382d0de97d66cc04aa6723551dd9401478bbddf1333bdef367eb25d2383a001/`.
- Strict Clippy: `mycity-core` and `mycity-storage`, all targets, `-D warnings`, passed.

The earlier failed Store compile and earlier correction results remain unchanged in `attempt-02-failed/` and `attempt-04/`. This closes the estimate-overflow correction only; it does not claim the full construction lifecycle (including demolition and waste closure) is complete.
