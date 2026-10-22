# Formal01 post-stop layout restore

Run date: 2026-10-23 01:11 CST. Storage: `/Volumes/HP P900` (`/dev/disk7s1`, APFS, USB volume), 1.8 TiB free at preflight. Target is `work/formal01-poststop-layout-01` on that volume.

Preflight: no matching `mycity-headless`, `j3f-01`, or `formal01-poststop-layout` processes; `lsof` returned no open handles under original attempt/snapshot paths; target directory was absent. The 000059.pending directory was excluded and not read/copied.

Original inputs:
- `/Volumes/HP P900/dev/MyCity/work/j3f-01/observed/model/layout/snapshots/snapshot-000057`
- `/Volumes/HP P900/dev/MyCity/work/j3f-01/observed/model/layout/snapshots/snapshot-000058`

Copies were made serially with APFS clonefile mode and metadata preservation:
```sh
cp -cRp work/j3f-01/observed/model/layout/snapshots/snapshot-000057 work/formal01-poststop-layout-01/inputs/snapshot-000057
cp -cRp work/j3f-01/observed/model/layout/snapshots/snapshot-000058 work/formal01-poststop-layout-01/inputs/snapshot-000058
```

SHA-256 file manifests (64 regular payload files total per manifest) are in `logs/original-before.sha256`, `logs/copy-before.sha256`, and `logs/original-after.sha256`. Full comparisons: original-before equals copy-before; original-before equals original-after. `cmp` results are in this record. The restore command reads only the cloned snapshot and writes a separate report directory; it requested no next snapshot generation.

Frozen executable:
`/Users/frankqin/dev/MyCity/work/candidate-binaries/r07-extents/32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2/mycity-headless`
SHA-256: `59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0` (matches frozen formal01 receipt).

Exact invocations, serially:
```sh
B=/Users/frankqin/dev/MyCity/work/candidate-binaries/r07-extents/32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2/mycity-headless
"$B" layout-load-only /Users/frankqin/dev/MyCity/work/formal01-poststop-layout-01/inputs/snapshot-000057 /Users/frankqin/dev/MyCity/work/formal01-poststop-layout-01/reports/057
"$B" layout-load-only /Users/frankqin/dev/MyCity/work/formal01-poststop-layout-01/inputs/snapshot-000058 /Users/frankqin/dev/MyCity/work/formal01-poststop-layout-01/reports/058
```

For each report, `cold_reopen_and_scan=passed`, producer and reader source SHA are the frozen `32c6e61b...`, `same_source=true`, payload files were hashed and loaded, and `next_generation=null` / `next_generation_requested=false`. Snapshot 000057 restored generation 57, mutation iteration 16800, 3,378,784,402 payload bytes, cold restore 5.805139 s; resumed scan iteration 16801. Snapshot 000058 restored generation 58, mutation iteration 17100, same payload byte count, cold restore 5.849192167 s; resumed scan iteration 17101. Reports explicitly retain `steady_gate`, `eight_hour_gate`, `memory_16gb_gate` as `not_run` and `full_M0=not_accepted`.

CLI stdout/stderr are preserved at `logs/057.stdout.json`, `logs/057.stderr.log`, `logs/058.stdout.json`, `logs/058.stderr.log`; normalized reports are `reports/057/report.json` and `reports/058/report.json`. Original and cloned snapshot bytes remain present. This is functional post-stop cold restore evidence only; it does not reach or substitute for the missed 6-hour/8-hour checkpoint, nor the 16 GB gate.
