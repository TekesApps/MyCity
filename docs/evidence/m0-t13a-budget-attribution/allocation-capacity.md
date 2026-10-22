# Allocation capacity and lifetime evidence

This note checks the fixed 32c6 T13-A source against the retained raw heap and
snapshot allocation traces. It narrows the allocation-lifetime question; it
does not authorize a capacity reduction or change the original workload,
payload, metadata reservation, or normal threshold. The reproducible checker is
`python3 docs/evidence/m0-t13a-budget-attribution/allocation-capacity.py`; its
recomputed receipt is `allocation-capacity.json`.

## Frozen source and evidence identity

The inspected source is the exact 231-entry
`docs/evidence/m0-t13a-extents-joint-retry-02/frozen-source.zip`. The checker
recomputed its sorted-name/LE-u64-length source stream as
`32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2`, and
verified archive SHA-256
`84c450c2c866578b02205030c7587df1dbb719660422fe64865ae62fe2ce908d`.
The receipt records SHA-256 and byte size for each cited frozen source file.
Line references below refer to those archived bytes, not an assumed current
checkout revision.

The preserved inputs are the original raw files under
`/Users/frankqin/dev/MyCity/work/m0-t13a-extents-joint-l1-retry-02/observed/model/`:

| Evidence | Size | SHA-256 | Recomputed measurement |
| --- | ---: | --- | --- |
| `heaps.jsonl` | 3,468 B | `43cfe1410867d1ddb091d97285b1184e913ac885867d772829477586a01dd1dc` | Layout process PID 49035, lifetime requested-heap peak 2,982,633,942 B |
| generation 1 `allocation-phases.bin` | 7,808 B | `ffae9641a37257e477b4da681ce11956f6dbf25b9db4837c26a84e2442ba1b14` | Minimum free 1,351,912 B at sequence 42, table decode |
| generation 2 `allocation-phases.bin` | 7,808 B | `648710427eee653ab3bd02fff15e5775254914b7ddcf822671a5b71ef90d7c2d` | Minimum free 1,327,024 B at sequence 57, table decode |

The original raw heap and both binary traces are present and retained. This
report uses those raw files, not a retired/compressed inventory, and does not
describe either snapshot as a directly restorable archive. The JSON receipt
contains their absolute paths, relative paths, sizes, and SHA values.

## What the allocator records show

The layout role receives a fixed 2,983,880,294-byte grant. The allocator
initializes and retains a 2,983,813,465-byte arena for the process lifetime;
the 66,829-byte difference comprises 66,669 bootstrap bytes and a 160-byte
adapter header. The Rust-heap lifetime peak is 2,982,633,942 bytes, leaving
1,179,523 bytes below the initialized arena size. The heap record explicitly
excludes native allocations, stacks, mappings, and foreign allocations, so
this margin is not spare whole-process memory.

Both 128-byte little-endian traces contain 61 records. Their lowest-free
records occur during phase 5 (table decode), which the frozen source marks at
`layout_load.rs:981-987`. Generation 1 has 58 reserve pages available, 70 pages
loaned in total, and 64 decoder loans; the other six are COW loans. Generation
2 has 56 available, 72 total loans, and the same 64 decoder loans; the other
eight are COW loans. Neither minimum-free record shows an allocation failure.

The 64-MiB decode workspace is initialized once and reused; validated decoded
tables are copied back to the primary tables (`decode_workspace.rs:3-39,
54-69`). The two 1-MiB snapshot buffers are reused (`layout_load.rs:796-806`).
The 128-MiB work-page image is streamed by reusing a reserve placeholder rather
than allocating another copy (`layout_load.rs:936-945`). Snapshot cloning
shares `Arc`-backed pressure payloads (`layout_load.rs:722-750`), and COW pages
are returned only after the snapshot joins and the old backing is uniquely
owned (`layout_load.rs:361-392`). These facts do not establish that those
buffers or pages can be removed while retaining the same overlapping restore,
snapshot, and COW behavior.

The budget path also reserves 128 MiB for work pages, 4 MiB for metadata, and
the external charge before deriving the R07 logical capacity
(`layout_load.rs:426-449, 471-493`). R07's extent vector keeps capacity for the
logical-quantum slot count even when the physical mappings are coalesced
(`r07.rs:129-145, 274-281`), so fewer extents do not automatically reduce that
metadata allocation. The adapter's arena is initialized and deliberately kept
for the allocator role's lifetime (`adapter.rs:105-165`).

## Finding

These measurements do not reveal a dead or duplicate backing that can be
removed without changing the workload or failure/restore semantics. The named
allocations already consume their fixed grants, and the observed heap margin
excludes native and mapping costs. In particular, this evidence does not
justify trimming the 4-MiB metadata charge, work-page reserve, COW reserve,
R07 logical payload, or original normal threshold. The defensible safe
reduction identified here is **0 bytes**.

To identify a candidate, collect synchronized per-process VM maps and
allocation/stack attribution at a complete-frame peak, then establish that one
specific backing is both accounted for and unnecessary at every overlapping
lifecycle point. Until that evidence exists, retain the capacities and treat
the over-budget frame as still over budget.
