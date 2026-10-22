# T13-A normal budget attribution (source 32c6)

This note accounts for the original fixed normal threshold against the 32c6
L1 run. It does not change the workload, payload, threshold, or acceptance
result. `python3 docs/evidence/m0-t13a-budget-attribution/attribute.py`
recomputes the figures from the retained raw files and writes no files.

## Evidence inputs

- Frozen source identity and original threshold:
  `docs/evidence/m0-t13a-extents-joint-retry-02/source.json` and
  `docs/evidence/m0-t13a-extents-joint-retry-02/freeze.json`.
- Frozen Rust source archive:
  `docs/evidence/m0-t13a-extents-joint-retry-02/frozen-source.zip`
  (`crates/city-headless/src/layout_load.rs`,
  `crates/city-headless/src/layout_load/r07.rs`, and
  `crates/city-role-allocator/src/adapter.rs`). The archive records SHA-256
  source `32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2`.
- Raw process frames:
  `work/m0-t13a-extents-joint-l1-retry-02/observed/host-samples.jsonl`.
- Actual address-owner audit:
  `docs/evidence/m0-t13a-extents-joint-retry-02/address-audit.json`.
- Retained VM capture material:
  `docs/evidence/m0-t13a-extents-joint-retry-02/inflight-layout-49035.json`,
  `docs/evidence/m0-t13a-extents-joint-retry-02/inflight-pool-49034.json`,
  `docs/evidence/m0-t13a-extents-joint-retry-02/inflight-layout-49035.stdout`,
  and `docs/evidence/m0-t13a-extents-joint-retry-02/inflight-pool-49034.stdout`.
  These captures describe a reduced-R07 observation point, not the frame-104
  full-R07 peak; frame 104's R07 state comes from its own raw host frame.
- Original run totals and limitations:
  `docs/evidence/m0-t13a-extents-joint-retry-02/measurement-summary.json` and
  `docs/evidence/m0-t13a-extents-joint-retry-02/STATUS.md`.
- Script output checked into this evidence directory:
  `docs/evidence/m0-t13a-budget-attribution/derived-report.json`.

The host-sample frame index is zero-based; the JSONL line number is one-based.
The first complete frame above normal is index 5 (line 6). The maximum complete
frame with R07 partially reclaimed is index 55 (line 56). The overall complete
peak is index 104 (line 105). The report retains all three because the first
over-budget frame, the reduced-R07 peak, and the full-R07 peak answer different
questions.

## Accounting boundaries

`role_arena` is one address range in the layout process. Its measured range is
2,983,813,465 bytes. It does not contain the separate R07 mappings or the
normal-pool process mapping. The range's `grant_bytes` is 2,983,880,294 bytes;
the 66,829-byte difference is the allocator's bootstrap bytes plus adapter
header, which the frozen `Allocator::activate` subtracts before requesting its
arena from `System`.

The normal-pool owner range is a separate 67,108,864-byte mapping. At the
full-R07 frame, the actual R07 state reports 277,610,496 mapped bytes. Thus the
budget's fixed named allocations reconcile exactly:

```text
2,983,880,294 layout allocator grant
+ 277,610,496 full R07 mapped payload
+  67,108,864 normal-pool mapping
= 3,328,599,654 original normal threshold
```

The address audit establishes unique byte-address coverage, not additive
physical savings. Its VM region types, resident/dirty page summaries, page
table rows, and permitted helper credits are not subtracted from host physical
footprint. For example, frame 5's permitted helper value is 4,571,760 bytes
while the state reports 4,521,984 bytes replaced. Neither value is used as a
physical-footprint deduction; the raw group is summed as observed.

## Results and interpretation

The first complete-frame excess is 3,860,442 bytes. In the reduced-R07 peak
(index 55), the raw group exceeds normal by 3,893,210 bytes while the applied
R07 state reports 4,521,984 bytes actually replaced. The peak full-R07 frame
(index 104) is 3,337,031,256 bytes, 8,431,602 bytes over normal. In that frame,
R07 is fully mapped again; the restored 4,521,984 bytes explain almost all of
the increase from the reduced-R07 peak (the layout process rises by 4,538,392
bytes, a 16,408-byte remainder).

For the full-R07 peak, the observed process footprints beyond named payloads
are 2,763,023 bytes in the layout process, 1,147,264 bytes in the pool process,
and 4,588,144 bytes across observer and driver. Their sum is 8,498,431 bytes.
The 66,829-byte bootstrap/header is already inside the allocator grant, so it
is not additional to normal. Subtracting it gives the measured 8,431,602-byte
excess. The script asserts this exact identity.

This is evidence that the fixed logical grants leave no spare allocation for
the measured process/native footprint at the full payload. It does not identify
a safely removable allocation. The observed overhead should next be localized
with contemporaneous per-process VM maps and allocation/stack attribution for
layout, pool, observer, and driver at the complete-frame peak. Do not treat the
observer or driver as free measurement overhead, use map counts as byte
savings, or trim frozen payload to make the total pass.

The 2 MiB observer failure run is not included here: its older validator capped
the observer at 1 MiB, so it is a separate diagnostic and not a comparable
acceptance result.
