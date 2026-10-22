# M0 T13A joint smoke/formal control 03

This is a new, independent attempt. It retains attempts 01 and 02 unchanged and uses unique roots `work/j3s-01` and `work/j3f-01`. The frozen input remains source `32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2`, archive SHA-256 `84c450c2c866578b02205030c7587df1dbb719660422fe64865ae62fe2ce908d`, and executable SHA-256 `59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0`. Preparation provenance is current main `v0.5.117` / `834d4a9c745d4b8e0cb952f66131528c93a039db`; the current Rust tree is not asserted equal to the frozen ZIP.

The reviewed acceptance boundary is recorded in [acceptance-review.md](acceptance-review.md). Incomplete process identity frames remain unknown and physical completeness stays unaccepted. They do not block functional smoke eligibility by themselves, and every full frame, including incomplete frames and compiler-interference frames, remains in the capacity analysis.

## Logging and capacity basis

The host sampler remains 4 Hz and writes every original JSON frame. `frame_codec.py` stores four successive frames in an independent raw-DEFLATE block with frame count, byte lengths, and CRC32. It does not discard fields, round values, sample less often, or replace unknowns. The writer tracks only a four-frame pending buffer and constant-space block counters. Before admitting a candidate or writing a block it checks committed host bytes plus the exact encoded candidate size (including header and any DEFLATE expansion) plus the latest scanned latency bytes against 256 MiB. Final partial-block flush repeats that check after a fresh latency scan; a rejected block marks the observer failed and reports any unpersisted tail. The protocol rejects frames above 8 MiB and raw blocks above 32 MiB; decode validates packed-size bounds and limits decompression to the advertised size plus one byte.

The fixed ZIP's `scale_metrics.rs` records each latency as 9 bytes (class byte plus little-endian `u64` nanoseconds); the per-file raw metric cap remains 128 MiB. `scale_endurance.rs` uses the same 720 game hours, 723 targets, 2,880 queries, and 96 saves in smoke and formal; cadence changes the wall clock from 100 ms to 40 seconds per game hour. The layout is different: smoke completes 600 scans and formal completes 28,800. Therefore business `compute-durations.bin` output is held at its measured fixed-work byte count in this screen, while layout metric bytes scale by scan count (48x). This is an evidence-based estimate, not proof of future runtime byte counts; the unchanged live 256 MiB cap remains mandatory.

Offline attempt-02 evidence is under `analysis/`. The raw 2,441-line log had 44 incomplete-identity rows and all rows parse. `attempt-02-lossless-host-samples.mcf` decodes byte-for-byte to the original log. The review preserves per-frame errors and PID correlations in `attempt-02-frame-diagnosis.json`; it does not modify the original 02 artifacts. The revised max-observed-block estimate includes every original frame and gives a 161,392,895-byte combined formal estimate, below the unchanged cap. This is a conservative smoke-derived screening estimate, not a claim that the 8-hour run has passed.

## Pure checks and static freeze verification

```sh
cd /Users/frankqin/dev/MyCity
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s docs/evidence/m0-t13a-joint-smoke-control-03 -p 'test_*.py' -v
PYTHONDONTWRITEBYTECODE=1 python3 docs/evidence/m0-t13a-joint-smoke-control-03/preflight.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/evidence/m0-t13a-joint-smoke-control-03/analyze_attempt02.py
```

The tests cover lossless block round-trip and corruption rejection, a peak-block spike that an average-based estimate hides, fixed business versus scan-scaled layout bytes, unchanged cap behavior, UTF-8 socket path boundaries, and existing process/hash helpers. These commands do not invoke Cargo, wait for admission, or launch the fixed executable. `support-manifest.json` records the exact 16 support-file hashes used by the runner. Storage preflight resolves the shared `work` path on the external HP P900 volume and retains the 190-original inventory and three-generation storage plan.

## Smoke and formal commands

Only start after the current shared Cargo/build window is released. The controller blocks if any `cargo` or `rustc` is active at preflight, after the 120-second continuous normal-pressure/compiler-free admission, or immediately before launch. It never kills another project. During execution, external compiler activity remains recorded per sample. The controller cleans inherited `MYCITY_*` environment keys, injects the expected heap/clock settings into the child, and execs the observer in place. Wait on that same observer PID to terminal status; do not restart after a timeout.

```sh
cd /Users/frankqin/dev/MyCity
python3 docs/evidence/m0-t13a-joint-smoke-control-03/run.py smoke 01
```

After a successful smoke, inspect and persist the capacity receipt:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-03/log_budget.py work/j3s-01
```

If the smoke wrapper and fixed-source checks pass and the estimate remains below 256 MiB, start the same-source formal run:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-03/run.py formal 01 01
```

This is permission to collect functional eight-hour endurance evidence. It does not accept physical identity completeness, normal steady-state behavior, physical performance, 16 GiB, or full M0. Preserve both complete snapshots and the protected 190 originals until actual cold restoration succeeds and their dependencies are released. Do not clean work or target as part of this attempt.
