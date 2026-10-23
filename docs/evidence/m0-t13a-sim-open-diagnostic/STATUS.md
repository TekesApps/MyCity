# Bounded Sim-open diagnostic candidate (not M0 acceptance)

Linked work: [normal-memory #112](https://github.com/TekesApps/MyCity/issues/112),
[endurance #113](https://github.com/TekesApps/MyCity/issues/113).
This change observes the existing Store and Sim lifecycle; it does not reduce the frozen
business workload, change the 64MiB role pool, or repair the original normal-memory excess.

## Calculation and ownership contract

The authoritative input is the existing Sim runtime status. `sim_host` brackets create/open
with process-local ClockReading and role-arena counters. The pre-open reading occurs **after
StoreWorkspace allocation**. It is not a before-all-Store-allocation baseline. The runtime
reports active and spare model/prepare, codec and candidate backing alongside total Store
owned bytes. Workspace bytes are a subset of Store owned bytes; never add them again.
These are requested/owned allocations, not native footprint or resident memory.

`scale_sim::Session` validates identity, clock order and required capacities before persisting
one receipt per (PID, process-instance timestamp) in the existing transcript. The timestamp
is an application process-instance marker, **not a native process birth certificate**. Native
observer linkage requires its independent birth/parent records and clock synchronization.
No nearest-neighbor native sample proves an open-time instantaneous peak.

Stable interval: repeated statuses from an already recorded instance produce no new receipt.
The bounded identity set has eight slots. The current frozen successful endurance script uses
four Sim lifecycles; a worker restart alone does not create a new Sim identity. This does not
promise arbitrary future restart capacity. A ninth distinct instance fails explicitly.
Missing legacy diagnostic data is omitted; present malformed data fails, never becomes zero.
Each serialized row is bounded to4096 bytes with1024-byte initial buffer; eight rows add at
most32KiB to the existing bounded transcript. The typed intermediate/JSON clone transient peak
has not been isolated. All observations belong to the Sim/driver resources, not free overhead.
No per-subject or per-hour diagnostic log is introduced. The reference path remains full
720-hour business execution, existing CPU arithmetic oracle and complete cold checkpoint.
No approximate batching of money, obligations or arrivals is authorized by this change.

## Actual results and evidence retention

Base: `/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01`.
All source archives, exact executables, stdout/stderr, original worlds and checkpoints stay
there. `target` and `work` are shared HP P900 USB/APFS external SSD paths. This Mac has32GiB;
real16GB remains `not_run`.

-334: eight diagnostic/reader unit tests passed.
-335: actual release binary built and retained with full source archive.
-336: fixed335 binary ran full30-day real business,720 hour targets/723 verified drive targets;
  final business/air, retry without effects and complete cold image passed. Two persisted rows
  (create/open) passed `check_receipts.py`. Child exited0 after207.73s. The wrapper reported
  `source_unchanged=false`: an agent temporarily created an untracked future test in canonical
  source. That WIP was hash-verified into its private tree and removed from main. The fixed
  binary/source335 evidence remains valid, but336 is **not clean source-freeze evidence**.
-337: fresh workspace/all-target release clippy with `-D warnings` passed, source unchanged.
-338: fresh release build passed, source unchanged; exact source/binary are in its fixed pin.
-339: final-source eight diagnostic/reader tests passed, source unchanged; actual test executable retained.
-340: fresh fixed338 binary completed the same full720-hour/723-target functional run in208.06s, child exit0 and source unchanged. Final business/air, complete cold image and oldest receipt retry passed. Two bounded persisted lifecycle rows passed the independent receipt-shape/identity/clock/capacity verifier. All original worlds remain external. This is not a native concurrent performance gate.

The one production warning fix between335 and338 is `#[cfg(test)]` on the test-only
`LimitedBytes::new`; no Store/decoder/model/allocator mutation is part of this batch.
The next600s L1 concurrent run is a diagnostic, not the original normal,8h or real16GB gate.
No physical memory saving is claimed. Original j3q failure and full recovery files are retained.

Final formatting failed in341 (four style-only hunks), was corrected with pinned Rust1.85.1,
and342 fmt passed.343 reran all eight affected tests;344 workspace/all-target strict and345
release build passed.346 ran the final345 executable through the complete720-hour/723-target
functional path in196.91s (wrapper;196.89s time-l), source unchanged and exit0. The final
receipt verifier, business/air comparison, retry and complete cold recovery passed. This final
source/executable supersedes the pre-format pin for release; both full originals remain.
Final source `aa552fd1f408a19147823fc84b1f8a31863c111792f2ab8b89078e69e108c9ad`,
binary `6f3015ad1e88220afa505b1579fbf39d9aac16b823578aec55ab405d9aa15741`.
The wrapper's actual duration in commands.json is authoritative if rounded values differ.

Advisor independently accepted the finite diagnostic scope subject to final format success;
that condition is met by342. This says nothing about original M0 physical/endurance gates.

Stage-end retention: exact duplicate source archive allocations were replaced with hardlinks
only after all selected runs were terminal and lsof showed no archive handles. All archive
paths and complete contents remain. `retention.json` lists each round, reason and evidence.
Unique-inode file allocation decreased54,026,240B (51.52MiB); disk available changed by
53,923,840B observationally, not all attributed to this task. Remaining selected source
archive allocation is16,207,872B; worlds/executables/target remain required and untouched.
No whole-work volume scan or unrelated cleanup was performed.
