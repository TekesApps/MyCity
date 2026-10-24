# #49 local operator observation adapter — source handoff

Date: 2026-10-24 (Asia/Shanghai). Fixed baseline: `v0.5.173`,
`3f48b3243eafb495faffd7a39ec9e73d4a65e935`.
Branch: `codex/issue49-observation-v173`; standard dedicated worktree:
`/Users/frankqin/.codex/worktrees/issue49-observation-v173/MyCity`.

## Files and integration API

- `crates/city-headless/src/query_observation.rs`
- `crates/city-headless/src/query_observation_tests.rs`
- `docs/handoffs/issue-49-operator-observation-v173.md`

`pub(crate) observe(&mut Store, Query, Option<&str>, usize) -> Result<ObservationPage, mycity_storage::Error>`.
The output schema is `mycity-operator-observation-v1`; it owns the unaltered original
`mycity-m0-query-v1` Page. There is exactly one real Store::query call, followed by
an immutable borrow of the same Store's Region. There is no QuerySource, mock,
copy of pagination, clone of WorldView, full row JSON duplicate, second query,
cache, clock, permission pool, persistent record or Sim IPC implementation.

Root must add `mod query_observation;` to the binary module root and wire the
separate `observe-query` / session `ObserveQuery` entrances. The module includes
its own sibling test file under cfg(test). Existing main/session/rootlib/Cargo,
storage/core/allocator/protocol and shared design files were not edited.
Root owns trusted argv Caller, System-only gates for every session Query path
(including old raw branches), stable Account `query_operator_required`, receipt
policy, process tests, issue/design updates and release. Do not infer player or
account read isolation from this adapter. The legacy standalone query output
must retain its original Page shape.

## Meaning and evidence

The page preserves schema, sequence, state/snapshot hashes, rows, next_cursor,
timeline Progress and complete_boundary. Errors use `?` without remapping.
Timeline target/working at remain distinct from last_complete; a committed
processing image is observable without promoting it to a completed boundary.
Query rows can be business records or storage/source diagnostic status. In
particular disk_capacity, source_sync and checkpoint need not be committed
business records. The generic label states only that Store::query returned a
record or diagnostic status, without predicting an outcome or asserting business
success. Leases, plans, shipments and retained claims retain their raw meanings.

Source is `store.query`; source_version names this exact Page's wire schema,
not a rules identity/model version. Coverage is only the requested section's
current page. These are literal adapter contract statements, not inferred
provider versions, global source completeness or product coverage.
Each page row has only its page index plus typed Known/Unknown fields:

- actual_or_forecast: Known StoreQueryRecordOrDiagnosticStatusNotPredictionOrBusinessSuccess,
  without classifying storage/source diagnostics as committed business records;
- observed_at and age_seconds: Unknown(NoGeneralObservationTimeProvider);
- conditional_eta: Unknown(NoConditionalEtaProvider), including paused transport;
- block_reason: Known only for raw paused_transport (cause unspecified) or the
  exact normal fact working capacity exhausted row/reason; otherwise
  Unknown(NoGeneralBlockReasonProvider), never "unblocked" from absent evidence.

Regional provenance is a separate narrow field: same Store Region's interaction
clock and existing population snapshots (city/version/observed_at/applied_sequence).
Age is checked interaction_time minus observed_at; underflow is typed Unknown,
not zero. No Region produces Unknown(NoRegionalSnapshotProvider). These are
regional population snapshot times, never general query-row timestamps or a wall
clock. No background forecast/Goal/Monitor provider exists here; no ETA is derived
from remaining_seconds, arrives_at, planned occupancy, or capacity reservations.

This finite contract elaborates the implementation gap in
`docs/m0-cli-session.md`, `docs/evidence/query-page-projection-20261024/STATUS.md`
and the existing full #49 scope; it does not close #49. Root must synchronize any
adopted entry/permission contract with shared design and the issue.

## Test source and actual validation status

All author compilation, test execution, fmt, rustc, scripts, benchmark, pressure,
GPU and process validation: **not_run**, per delegation. No commit/push/tag/PR,
issue mutation or publication. Source reading and git status/diff/hash inspection
only. In this author's dedicated tree the files remain unregistered in main;
root owns registration and validation in its integration tree.

Five test functions construct legal Stores using existing laboratory fixture
bytes and public Store APIs. They are experimental data, not production inputs:

1. Eight sections, page sizes 1/2/64, full original Page JSON equality, raw rows,
   cursors, hashes/sequences, metadata shape/Unknown meanings; paused transport,
   full inbox, checkpoint/reopen and actual physical delivery states.
2. Separate e0 Timeline Store with 20 scheduled claims and consistent legal
   Advance page width 2; processing target/phase/last_complete, full boundary
   completion, all 20 claims actually paid, and all-section Page comparison.
3. RegionAdvance and RegionActivate establish snapshot versions/times/sequence;
   compare to the same immutable Region, without transferring freshness to rows.
4. Paused transport's positive remaining_seconds remains Unknown ETA and a
   recorded state rather than delivery.
5. Invalid limits, malformed/oversized cursor, beyond-end offset, wrong-section,
   checkpoint-generation and post-commit stale cursors, and broken real producer
   outbox errors retain original Store error variants.

No forced RecoveryRequired, IO fault, native allocator/RSS or CPU acceptance is
claimed. Underflow handling is defensive source logic; no illegal Store fixture
is manufactured to make that branch executable.

Narrow review correction: the original advisor identified that the previous
generic committed-record label overstated disk/source/checkpoint diagnostics.
Only the common label, its test assertions and this handoff were revised. The
previous frozen package remains separately archived by root at
`/Volumes/HP P900/dev/MyCity/work/operator-observation-20261024/author-frozen-01`.
At that freeze all five tests were not_run; no Page/error/cursor or provider
behavior changed.

Root subsequently reported release check/build exit 0 for fixed source452 SHA
`d992d5b065458e418892de2302454fb830bda142ee7e7f6543b48256f177e7b7`.
Actual module execution was **4 passed, 1 failed**: Timeline fixture's first
Advance width 1 failed with `Invalid("timeline_page_limit_too_small")` before
processing assertions. The failed source/executable/worlds/logs remain at root's
external evidence directory `operator-observation-20261024` (`source-01.zip`,
`module-test-01.log`, `worlds-module-01`). The driver stopped after this group;
CLI/session permission groups were not_run, not implicitly accepted.

Author's minimal fixture correction keeps the same 20 claims and original
processing/full-boundary assertions. `Engine::timeline_quote_diagnosed` in
`crates/city-core/src/transaction.rs` quotes 20 due rows plus fixed
16+16+2+4+16+32 = 86 steps, so 106 steps require width >= 2 for the unchanged
`CONTINUATION_MAX_PAGES=96` gate. Width 2 reserves ceil(106/2)=53 pages and executes
only P0 and P1 transitions initially, leaving phase P2, target 3600, at 3600 and
last_complete 0. `Store::prepare_continuation` also rejects changing width while
a ticket exists, so both initial and subsequent Advances use width 2 (replacing
the previous subsequent width 64). A ticket-width assertion and final 20 paid
claim assertion strengthen the fixture without changing a model/storage gate.
This correction's compilation, execution and fmt remain **not_run** by author;
the prior failure remains a failure, pending root's serial rerun and advisor review.

## Resource and retention plan

At most 64 row-observation records, indexed into the moved Page; no repeated hash,
rawValue or unbounded copied type strings. Separately at most MAX_CITIES (8)
existing regional snapshot provenance records. The Store Page still bounds row
count only: encoded bytes per row, total scan CPU, hashing/status IO, native stack
and process RSS have no new general hard bound. Metadata CPU is O(page rows + 8),
which does not make Store query itself O(page). No allocator or budget claim.

Author source package budget is 16 MiB; three small text files, no new data fixture,
build outputs, caches, target, logs or generated world. Root's future finite
validation uses existing shared target and external HP P900 work, within its
128 MiB evidence stage budget. Preserve complete created worlds, failure logs,
fixed source/executable and receipts; no author cleanup of evidence or worktree.
Tests require an existing `MYCITY_OBSERVATION_TEST_WORK` evidence parent and use
exclusive new child directories, retaining both successful and failed worlds.

After root registration and review, suggested minimal serial commands (not run):

```sh
CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -j 1 -p mycity-headless --bin mycity-headless query_observation::tests:: --no-run
MYCITY_OBSERVATION_TEST_WORK='/Volumes/HP P900/dev/MyCity/work/issue49-observation-v173/worlds' CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -j 1 -p mycity-headless --bin mycity-headless query_observation::tests:: -- --test-threads=1
```

Root must first verify external volume availability, create its approved evidence
parent, pin/archive source and select actual toolchain. No alternate local target,
RUST_MIN_STACK override or unbounded reruns. Root adds real CLI/session process
checks, including permission failures and legacy Page compatibility; future Sim
query transport is still unimplemented. Source is frozen on delivery; root may
request blocking fixes. Only after full persistent archive, no active handles or
dependencies and no measurement may root retire this dedicated worktree while
retaining its branch and evidence.
