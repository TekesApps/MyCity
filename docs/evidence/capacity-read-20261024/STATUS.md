# #118 current scalar-capacity read — 2026-10-24

Finite read-only implementation, not full physical provider or #118 closure. The original
coordinator advisor independently accepted the frozen retention/budget/source-report fixes
before execution. Root added only storage module export and the previously reviewed narrow
World scope/revision getter; original transaction/receipt authority remains unchanged.

Actual validation: existing release/locked/offline/single-thread configuration, HP P900
shared target/work, five tests passed. Real Store hot/archive receipts, rejected/absent record,
released-target lookup, partial clock and deliberately interrupted checkpoint RecoveryRequired
were exercised. The printed interrupt panic is intentional and caught by its passing test.

The extra default-debug run aborted on first Store scenario; each other Store scenario also
aborted in an isolated process before any world file existed. Layout test alone passed in
debug. No stack override, M0 gate change, load reduction or debugger success claim was used.
Debug create-frame failure remains unresolved and is not repaired by release success.

All four release worlds are retained at rootrun-05-release, with 133247 logical file bytes
against an unmeasured-before-run32MiB planning cap. Failed debug directories remain separately
retained. These byte totals are not APFS recovered space or process memory. Layout sizes:
projection1896, stamp96, counter arrays256, selected components2352, full outer observation1936,
Box handle8, original JournalRecord3352 bytes. Selected sum is not an inline/stack upper bound;
record-return copies, decoder scratch, allocation overhead and native peaks remain unmeasured.

Unknown units/aliases/windows/rights/future admission remain explicit. Observations are local
to one exclusive Store call, not durable or cross-process authorization. No Reserve/Occupy/
Release is issued by the adapter. No new physical pool or receipt journal exists. Store receipt
rejections and missing values do not trigger re-execution. Real physical provider and formal
resource/joint/M0/true16GB acceptance remain pending; actual host is32GiB.

Evidence: /Volumes/HP P900/dev/MyCity/work/parallel-issue118-validation-20261024/ includes
complete owner source, root source delta/manifest, before-shared-file originals, debug/release
logs, fixed executables and retained worlds with per-file hashes. Keep all until relevant
recovery/integration dependencies are released and a newer complete baseline is accepted.
No WIP/worktree deletion, commit/tag/push or PR; Issue remains open.

## Later default-stack repair validation

The historical default-debug failures above are retained, but are no longer the latest
status: source09 of [the separate stack repair](../stack-faults-122-123-20261024/STATUS.md)
passed original equity/utility cases, release core209 and profile4, Store6parents with12 real
cold children, and default-debug capacity read5. Original advisor accepted these finite
results. Older failures and fixed executables stay recoverable. Native peak, full issue and
M0/16GB gates do not become passes; publication remains separately coordinated.
