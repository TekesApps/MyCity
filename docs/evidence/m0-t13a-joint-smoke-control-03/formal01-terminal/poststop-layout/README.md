# Formal01 post-stop layout snapshot cold restore

This archive records a **functional layout cold-restore check only**. It does not complete or substitute for the missed 6-hour recovery, 8-hour endurance, 16 GB device, performance, or full M0 gates.

The complete original `snapshot-000057` and `snapshot-000058` remain at `/Volumes/HP P900/dev/MyCity/work/j3f-01/observed/model/layout/snapshots/`. Complete APFS CoW clones remain on the same external HP P900 volume at `/Volumes/HP P900/dev/MyCity/work/formal01-poststop-layout-01/inputs/`. No GB snapshot payload is copied into this Git evidence directory. The original source snapshots were not modified; SHA-256 manifests before/after match. `snapshot-000059.pending` is an incomplete interrupted state, was excluded, and is not claimed recoverable.

The frozen source-32c binary (`59aa...b4e0`) ran `layout-load-only` against each clone. Both reports show full payload hash/load and `cold_reopen_and_scan=passed`, with no next generation requested. The detailed run commands, executable hash, source/clone paths, timestamps, and storage scope are in [COMMANDS.md](COMMANDS.md). Preflight and execution receipts are alongside it; raw stdout/stderr, full reports, and file-level SHA lists are under `logs/` and `reports/`. [SHA256SUMS](SHA256SUMS) indexes every archived artifact except itself.
