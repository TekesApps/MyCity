# v128 runtime residual attribution (read-only)

**Result:** the available evidence does not attribute the residual bytes to the proposed native stack or control buffer. No bytes are credited or subtracted. This is a diagnostic accounting note, not a memory/performance pass.

## Frozen-window arithmetic

Source: `work/v128-native-01/combined-control/full-restored-window.json`, three complete frames with the same PID/birth identities. Each frame reports session footprint `3,264,318,056 B`, driver `68,485,480 B`, observer `3,424,544 B`; the group is `3,336,228,080 B` and remains `7,628,426 B` above the unchanged normal threshold.

The session arithmetic is exact:

- role-arena grant: `2,983,880,294 B`
- R07 map: `277,610,496 B`
- unexplained process-footprint remainder: `2,827,266 B`
- sum: `3,264,318,056 B`

The driver arithmetic is also exact:

- normal-pool backing: `67,108,864 B`
- unexplained process-footprint remainder: `1,376,616 B`
- sum: `68,485,480 B`

These equalities identify residual totals only; they do not identify their owners or address ranges. The v128 session `vm-regions-*.jsonl` files enumerate the role-arena range and R07 mappings (262/266/262 mapped regions across the three frames), but contain no thread-stack or driver-control range. The v128 heap/manifest's `thread_stack: original 2MiB native stack` and `tail_reserved_bytes: 65536` are declarations/accounting metadata, not address-bound physical-footprint measurements. `tail_reserved_bytes` is not evidence of a distinct 64KiB driver-control allocation.

## Historical vmmap limitation

The requested v123 vmmap file was not present under the inspected canonical evidence/work paths. The available historical capture is `docs/evidence/m0-t13a-full-r07-native-attribution/attempt03/captures/*.stdout` (source `32c6e61b…`, not the v128 source `db457727…`). It classifies regions and shows stack entries, but those are a different process generation/source and cannot attribute v128 residual bytes. Its layout capture reports thread 0 stack reservation 510 × 16KiB (3 resident pages) and thread 1 reservation 130 × 16KiB (1 resident page); reservation size is not resident footprint, and thread 1 is not identified there as the claimed 2MiB model stack. Do not carry those addresses or totals into v128.

## Smallest useful next observation (not run)

During a future authorized reproduction, hold the same full-restored phase and exact PID/birth identities, then capture each target's complete region map once:

```sh
/usr/bin/vmmap -wide -pages -interleaved "$SESSION_PID" > session.vmmap
/usr/bin/vmmap -wide -pages -interleaved "$DRIVER_PID" > driver.vmmap
```

Record target PID, process birth/start identity, parent identity, capture monotonic time, and the matching v128-style `vm-regions` frame immediately before and after each capture. Join the emitted address/length ranges against vmmap's address intervals; report resident/dirty/swap per matched region without treating virtual reservation as physical bytes. To attribute a specifically claimed 64KiB control buffer, the driver must also emit its exact owner-tagged address/length (or an equivalent allocator-level range record); vmmap category alone cannot prove that a generic region is that buffer.

This diagnostic capture invokes helper processes and perturbs the target. Keep its helper footprint separately visible; do not subtract it, alter thresholds, or reuse the observation as the normal-memory, speed, endurance, or 16GiB acceptance run. The v128 full-restored result remains over threshold and unresolved.
