# M0 T13A joint L1+L2 smoke/formal control 02

This independent retry preserves attempt 01, including its short failure and long-path evidence, and uses fresh short work roots `work/j2s-01` and `work/j2f-01`. The controller derives these roots before admission and never reuses or overwrites an earlier run.

The preparation checkout is main `v0.5.116` at `6d25acf4dd54dda707ffee94fa2dd8e95a693a53`. That repository revision is provenance only. The run remains bound to the original frozen source `32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2`, its 231-entry published archive, fixtures, lockfile, and fixed executable (`59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0`). Current Rust files are not compared byte-for-byte with that archive.

## Socket path proof

`preflight.py` inventories all pathname `UnixListener::bind` and `UnixStream::connect` sites in the fixed ZIP. The joint controller derives the pool root as `model/normal-pool`, and both business/layout workers use `pool.sock` there. The worker broker binds its `/tmp/mc-w-…sock` endpoint; compute-worker transports use inherited anonymous Unix descriptors and add no filesystem socket path. The inventory checks both the work symlink spelling passed to the executable and its resolved HP P900 path. Each pathname is measured as UTF-8 bytes plus its terminating NUL and must be strictly shorter than macOS's 104-byte `sun_path` limit. The complete inventory and measured candidate paths are saved in `unix-socket-paths.json`.

The retry retains the 4 Hz whole-tree observer, 120-second continuous normal-pressure and compiler-free admission, and the existing pressure, swap, disk, and log limits. Admission blocks on any active `cargo` or `rustc` process and never stops other projects. After launch, visible compiler processes are recorded per frame. The observer is exec'd in the runner's PID; runs have no timeout or automatic restart.

## Pure checks and static freeze verification

```sh
cd /Users/frankqin/dev/MyCity
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s docs/evidence/m0-t13a-joint-smoke-control-02 -p 'test_*.py' -v
PYTHONDONTWRITEBYTECODE=1 python3 docs/evidence/m0-t13a-joint-smoke-control-02/preflight.py
```

These checks do not invoke Cargo, wait for admission, or start the model. Storage preflight resolves `work` to the external HP P900 USB/APFS SSD and validates its volume UUID. Its estimate reserves three 3,378,784,402-byte generations, the 256 MiB combined-log cap, and a 2 GiB free-space floor; live 20 GiB world, 256 MiB logs, and 2 GiB free-space gates remain unchanged.

## Execution

Use a new run ID if a future attempt reaches a terminal failure. The first smoke command is:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-02/run.py smoke 01
```

The fixed executable runs the full joint controller: smoke covers 720 business hours, 723 drive targets, 2,880 queries, 96 saves, 725 pool grant/return/fill acknowledgements, and at least 600 seconds of layout scans and restores. Formal uses the same source and executable with 28,800 seconds of layout work. Smoke's business clock remains 72 seconds; formal's remains 28,800 seconds.

Wait on the same process until terminal status. Do not restart on a timeout or lost connection. After a successful smoke, project the actual full frame sizes onto the formal interval:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-02/log_budget.py work/j2s-01
```

This writes `work/j2s-01/log-budget.json`. The formal launcher requires that receipt, recomputes it, checks the conservative projection under the unchanged 256 MiB cap, and compares all support hashes with the successful smoke. A projection above the cap is an upper-bound screening result, not proof that a real run would exceed the cap; it blocks this formal launch while preserving the evidence.

Only after successful smoke and a reviewed under-cap estimate:

```sh
python3 docs/evidence/m0-t13a-joint-smoke-control-02/run.py formal 01 01
```

The new `observed` root is left absent until the observer creates it. Preserve the protected 190 originals and both new complete snapshots until actual cold restoration succeeds and recovery dependencies are explicitly released. No work or target cleanup is part of this run.

Attempt 01 ended before the smoke workload because the longer path exceeded the Unix socket path limit. It is retained as a failed short attempt; it is not evidence of a 600-second smoke or eight-hour acceptance.
