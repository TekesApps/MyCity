# R07 extent full-L1 joint diagnostic preparation

This directory contains a prepared, **not run** external diagnostic for the
original M0-T13-A full L1 workload. It does not relax workload, coverage,
physical-memory, or normal thresholds and does not claim performance
acceptance. The diagnostic is limited to collecting range ownership and
checking R07 logical-to-physical extent topology while the existing test runs.

`topology.py` and `test_topology.py` validate continuous logical intervals,
total mapped bytes, non-overlapping physical addresses, stable address and
generation identity while pinned, and legal refill behavior for old full
extents plus at most one short tail replacement with a fresh generation. The
six synthetic tests pass. `prepare.py`, `run.py`, and `analyze.py` passed Python
syntax compilation; `prepare.py` and the live diagnostic were not executed.

Before any live run, the exact final source must be frozen and built by the
primary build owner. `prepare.py` requires a source fingerprint and an already
built executable that embeds that fingerprint. It records the executable SHA
and hard-links it (same device/inode, no copy) to
`work/candidate-binaries/r07-extents/<source-sha256>/mycity-headless` before
any later headless build can replace the shared `target` executable. The
currently observed shared executable did not contain the R07 source fingerprint
and was not pinned as R07 evidence: at inspection it was 13,545,104 bytes with
SHA-256 `6bb55b3a9bc7413a307e8aeb89b969cf80b2ee46b78285c7b54c1b4ca3ebff6e`.
The exact focused-test executable was found at
`target/release/deps/mycity_headless-bd191120ee0d5aca`; its embedded R07
fingerprint was verified and it was hard-linked, with matching device/inode,
to `work/candidate-binaries/r07-extents/<source-sha256>/focused-test-executable`.
Its SHA-256 is `21325dd90f3414bfe456a9b9b441e9f08430482fcdf11db54a759da2b804e24f`.
The receipt is `verification/r07-extent-build/focused-test-executable.json`.
This is strictly a `focused_test_executable_only` retention artifact, not a CLI
or pressure executable. Do not run `prepare.py` until the exact full-L1 CLI
source and executable are confirmed together.

The future diagnostic uses its unique `work/m0-t13a-extents-joint-l1-01`
workspace and retains up to 8 GiB of new pressure and snapshot data. It does not
modify the protected 190-file recovery set. Do not retire run-owned snapshots
until both generations pass independent fresh-process cold restore and scan,
restore receipts are durable, all run/observer/native-watch processes have
exited, and open-file checks show no dependent handles. Then retire only
path-inventoried run-owned pressure/reserve/snapshot files; retain source,
executable, dependencies, logs, raw VM captures, topology lists, identity
receipts, restore reports, and failure evidence. Hashes do not substitute for
snapshots while restore validation depends on the original bytes.

Root-owned native-watch/provider wiring is intentionally not copied here; it
has not yet been tested as part of the frozen joint source. No Cargo build,
L1 pressure run, or formal performance measurement was started in preparing
this harness.

The isolated extent source and a later canonical integrated source are different candidates. Freeze the actual executable and report every included change. The canonical run includes independently integrated core models and endurance-role wiring; unchanged workload is not proof of a single-variable source experiment. Raw group measurements include all their costs, with no inferred subtraction.
