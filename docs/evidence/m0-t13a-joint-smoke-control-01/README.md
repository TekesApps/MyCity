# M0 T13A joint L1+L2 smoke/formal control

This harness prepares one full joint run against immutable source `32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2` and its published executable (`59aa365e…`). The preparation checkout is separately identified as main `v0.5.115` / `d14eb41999fad8d2d3709b86ff10f4fbda33ecb6`; current Rust sources are never asserted byte-identical to the older fixed archive. The two canonical fixtures, lockfile, complete source archive, executable digest and embedded source fingerprint are checked before execution.

The fixed executable's archived `joint-endurance` contract is checked for 720 business hours, 723 drive targets, 2,880 queries, 96 saves, 725 pool grants/returns/fill acknowledgements, and the full layout scans and restore checks. The smoke runs the complete joint controller with a 600-second layout minimum; formal uses the same controller and source with 28,800 seconds. The business clock remains 72 seconds in smoke and 28,800 seconds in formal. The old endurance-controller runner is not used.

## Prepare-only verification

Run these from the canonical checkout. They read the frozen archive/executable, verify the fixed contract and run small pure Python checks. They do not invoke Cargo, create a run root, wait for normal pressure, or start the model.

```sh
cd /Users/frankqin/dev/MyCity
python3 -m unittest discover -s docs/evidence/m0-t13a-joint-smoke-control-01 -p 'test_*.py' -v
python3 docs/evidence/m0-t13a-joint-smoke-control-01/preflight.py
```

`preflight.py` resolves `work` and checks that it is on HP P900 volume UUID `4289CF6D-F09D-4831-8558-4AB223E4EE04`. Its capacity estimate uses 3 × 3,378,784,402-byte full generations, the existing 256 MiB combined log limit, and a 2 GiB free-space floor. It is an estimate; the live observer retains the hard 20 GiB world cap, 256 MiB combined log cap, 2 GiB floor, 60-second non-normal pressure limit, 256 MiB swap-growth limit, and 4 Hz sampling.

## Future execution commands

The current Cargo feature-validation window must have ended before using these commands. They refuse to begin admission while `cargo` or `rustc` is active, and readiness continuously requires 120 seconds with normal memory pressure and no such compiler process. They do not stop other processes. At launch the controller removes inherited `MYCITY_*` variables; the observer supplies `MYCITY_HEAP_LOG` and `MYCITY_OBSERVER_CLOCK`. The observer records any compiler processes visible in each full process-table sample after launch.

```sh
cd /Users/frankqin/dev/MyCity
python3 docs/evidence/m0-t13a-joint-smoke-control-01/run.py smoke 01
```

Wait for that same process to exit and inspect `work/m0-t13a-joint-smoke-control-01-smoke-600-01/observed/observer-report.json` and `model/report.json`. Do not restart it on a timeout or interrupted connection. A new attempt requires a new run ID and preserves the previous root. Only after a successful smoke, estimate whether full 8-hour host and latency logs fit the unchanged cap:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-01/log_budget.py work/m0-t13a-joint-smoke-control-01-smoke-600-01
```

The estimate uses the maximum full smoke frame length × 115,200 formal frames, plus measured `compute-durations.bin` bytes scaled by 48. It never drops fields or changes a budget. A conservative projection above the cap blocks the formal gate, but it is only an upper-bound screen and does not prove an actual run would exceed the cap; preserve that smoke for a later, evidence-based estimate. A projection under the cap remains an estimate until the formal run completes. The formal launcher recomputes and compares the saved receipt, and verifies that every fixed support-file hash matches the successful smoke.

After successful smoke review and an `under_cap` estimate, the same fixed source and executable can be run formally:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-01/run.py formal 01 01
```

Each invocation creates a unique work parent and leaves its `observed` child absent for the observer and Rust controller to create. The runner `exec`s the observer in place after readiness, so the measured observer keeps the launch PID and there is no parked orchestration parent. There is no timeout or automatic restart. Keep the process and all outputs until terminal status is captured.

All original protected migration files listed in `docs/evidence/work-migration-20261022/attempt03/protected-after.json` remain retained. Keep both complete snapshots produced by the new joint run until each has been cold-restored successfully and its recovery dependencies are explicitly released. Do not clean work or target during a run. Record any compiler activity seen in frames; a clear admission/launch window alone does not establish a compiler-free eight-hour run.

This directory contains preparation assets only. No admission, model, 600-second smoke, or formal run has been started here.
