# Full-tier unpaced timing diagnostic (v128-02, frozen for launch)

This one-shot diagnostic is statically bound to the post-retention descriptor at
`/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01/diagnostic-final-binary.json`.
It invokes the same fixed executable as observer and model:

```text
<exe> native-observe <root>/observed -- <exe> speed-run <root>/observed/model full unpaced
```

The workload remains full: 30 days, 720 hours, 723 targets, and 724 windows.
This is source-segment timing and functional diagnosis only. It is not clean
performance evidence, L1, M0, an eight-hour endurance result, or a 16 GB
acceptance result. The earlier v128-01 attempt is preserved as
`root-abort-prelaunch`; it did not start the observer or model.

The descriptor is pinned by `prepared-manifest.json` and the launch snapshot.
Static preparation verified source SHA-256
`db4577273a37fdbeeb95c092e20356ba2386bdb0dd311f2b6020de2bb89f0daf`, executable
SHA-256 `83b7a28460c8d82ae1b5c5c17ca154359648d6d3937103f973a8fa988fcc373a`,
source ZIP SHA-256
`ac4fed1cf8d4f915f7d69b2947e4795310f56e8aba2775a4eb386b01536cfdea`, and
record SHA-256 `f564922cf94d825522c3ffa64626969924480f84f4658c3d9633c14a99ac22f0`.
The runner recomputes the sorted, length-prefixed build-input digest from the
ZIP, checks its CRC and duplicate paths, verifies the executable embeds the
source fingerprint, checks fixture SHA-256
`876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508`, and
checks the full workload declarations. It does not compare the prepared source
to the currently changing checkout.

The runner imports only the pinned process, storage, socket, and readiness
helpers from `speed-full-v125-01/run_speed.py`; helper SHA-256 values are fixed
in the runner. It checks the external HP P900 USB/APFS volume and actual
`observed/model/normal-pool/pool.sock` path (90 UTF-8 bytes including NUL,
strictly below the 104-byte macOS limit). It requires continuous 120-second
normal-pressure/compiler-free admission, checks storage and process state again
before launch, strips inherited `MYCITY_*` settings, and preserves controller
peak RSS separately from the observer. Readiness helper errors are retained;
only tracked `Popen` children are terminated and waited in `finally`.

The observer owns `observed/` and `model/`; this wrapper does not precreate
either. The run root is one-shot and refuses existing intent, launch, or
terminal receipts. It samples external compiler/heavy-process activity and
sampling errors every five seconds. At the 3200-second outer deadline it sends
SIGTERM only to its still-live direct observer child after PID, PPID, start
identity, executable identity, and `Popen` relationship match; it waits without
escalation or restart. Missing or mismatched source, executable, report, child
PID, clock PID, nonce, or clock bounds cannot yield a successful terminal
receipt. Runtime success would still mean functional timing evidence only.

Before launch, the exact active runner, test, and prepared manifest bytes are
copied with SHA-256 into `controller-revisions/launch/`. Those launch copies
and the active runner, tests, and manifest are frozen and must not be modified
during or after execution. Earlier v128-01 seed versions remain under
`controller-revisions/seed-v128-01-final/`.

After the main agent releases the measurement window, launch exactly once:

```sh
cd "/Volumes/HP P900/dev/MyCity/work/timing-full-v128-02"
PYTHONDONTWRITEBYTECODE=1 python3 run_profile.py
```

Preparation verification used only Python unit tests and AST parsing. No
readiness helper, observer, model, Cargo command, or build was started by this
preparation step.
