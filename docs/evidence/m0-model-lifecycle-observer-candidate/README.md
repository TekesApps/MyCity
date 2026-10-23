# Model lifecycle native observer candidate

This is a read-only, external macOS observer for the bounded `normal_pool::model_lifecycle_diagnostic` candidate. It measures the actual pool child and Model helper while the separate CLI exercises the real `full` reference Model lifecycle. It does not start, stop, signal, or restart any process, and it does not run Cargo, L1 pressure, Sim, or an endurance workload.

The observer reuses the already-reviewed native support files from `/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-joint-smoke-control-03/`. It refuses to import them if any pinned hash changes:

| File | SHA-256 |
|---|---|
| `observe.py` | `0d7ddad946762bb3a9b683caa71dcf99cda6de08ff14c45c6877757e0d468d5d` |
| `native_metrics.py` | `4a672f7c711f30250ec493f63dad6ba95895e97ec5d10edcc99d034eb989051e` |
| `process_activity.py` | `1abbc0771180936fa660b34db4f6428f199fd467e0f866548e8a12bdb4c58aa9` |
| `frame_codec.py` | `005be0c42f1703e0aae4d6e7597d83e65654ffb805f22456873ae042e4a9b97f` |

It brackets each capture with the latest durable lifecycle event and verifies that the event did not advance during capture. For each sampled process it also brackets `proc_pid_rusage` with native parent, executable path, and second `proc_pid_rusage` reads. The OS process start is recorded as Mach absolute time from `ri_proc_start_abstime`; it is kept separate from the event's `application_instance_unix_ns`. PID reuse, changed parent/executable, permissions, missing metrics, or changed lifecycle events produce `null` metrics and a reason. No missing value is replaced by zero.

The sampled roles are the CLI supervisor, the normal-pool child, and the full Model helper. The Model helper must have the same PID and application instance in `filled`, `retiring`, and `returned`; retirement/return also confirms process absence when possible. The final diagnostic report must bind to the fresh source SHA and the helper's application identity. Other active `cargo`/`rustc` processes are recorded as interference context, without argv and without stopping them.

The terminal row keeps three results separate: the CLI exit code must be recorded by the shell's `wait`, the four durable lifecycle events are checked from `events.jsonl`, and the four native phase samples are counted only when identity/metrics are complete. A missing capture attempt never populates `captured_phase_names`. If the 256-row or 256-KiB bound is reached first, sampling stops with an explicit incomplete terminal row; already-written rows stay intact and the run continues independently. A complete four-phase capture is not an acceptance gate.

The reused `proc_pid_rusage` gives RSS and physical footprint. `PROC_PIDTASKINFO` gives virtual bytes, resident bytes, and thread count; thread count is not stack-resident memory. The reused bounded API has no per-thread stack-resident or VM-region-resident breakdown, so those fields remain explicitly unavailable. Missing VM-region data does not show that the historical 6,334,090-byte residual is reclaimable and does not authorize rerunning the original normal 8-hour workload. The observer does not run `vmmap` or spawn a sampler child.

Pool, helper, and supervisor captures occur sequentially. Each process is identity-bracketed, but the combined sample has a nonzero sampling span and must not be described as a simultaneous cross-process peak.

## Preconditions and run command

This directory contains only the observer script and this document. A root-owned fresh CLI integration is still required: the pinned executable must expose `normal-pool-model-diagnostic NEW_RUN_DIR`, dispatching to `normal_pool::model_lifecycle_diagnostic`. The current private source candidate has the method, but its `main.rs` does not yet expose that command. Do not use `normal-pool-host` alone; that would omit the actual Model helper.

Before a run, root must select a fresh executable/source pair from a build descriptor and choose a unique run directory on the external SSD. Keep the entire run directory, observer JSONL, command exit codes, source/executable descriptor, and stderr as evidence. The run directory and output path must not already exist. No L1 pressure input or snapshot corpus is needed for this diagnostic.

After replacing the three uppercase pin values from that fresh descriptor, run the following from a shell. The executable is launched directly so `$!` is its PID; the observer only attaches to that PID and the run path.

```sh
RUN='/Volumes/HP P900/dev/MyCity/work/model-lifecycle-<unique-id>'
OUT="${RUN}.native-observer.jsonl"
EXE='/Volumes/HP P900/dev/MyCity/work/<fresh-build>/mycity-headless'
EXE_SHA256='<fresh descriptor executable SHA-256>'
SOURCE_SHA256='<fresh descriptor source SHA-256>'
OBSERVER='/Users/frankqin/dev/MyCity/docs/evidence/m0-model-lifecycle-observer-candidate/observe_model_lifecycle.py'

set +e
MYCITY_NORMAL_POOL="$RUN/normal-pool" MYCITY_HEAP_LOG="$RUN/heaps.jsonl" \
  "$EXE" normal-pool-model-diagnostic "$RUN" &
CLI_PID=$!
python3 "$OBSERVER" \
  --run-root "$RUN" --output "$OUT" --supervisor-pid "$CLI_PID" \
  --exe "$EXE" --exe-sha256 "$EXE_SHA256" --source-sha256 "$SOURCE_SHA256" &
OBSERVER_PID=$!
wait "$CLI_PID"; CLI_RC=$?
wait "$OBSERVER_PID"; OBSERVER_RC=$?
printf 'cli_exit=%s observer_exit=%s\n' "$CLI_RC" "$OBSERVER_RC"
shasum -a 256 "$OBSERVER" "$EXE" "$OUT"
```

Use the script's exit status together with the CLI exit status and its terminal JSONL record. A successful observer process only means it collected the bounded diagnostic; it is not a memory acceptance result. `diagnostic_capture_incomplete` means one or more lifecycle phases were too brief or could not be sampled and must remain incomplete. A timeout, missing final report, identity mismatch, or failed native query is a failed diagnostic attempt; preserve its partial JSONL and all run files, and do not automatically rerun it.

## Fixed resource bounds

- Poll lifecycle bytes every 50 ms; sample a stable `filled` phase at most once per second by default.
- Stop after 620 seconds. The observer never sends a signal or kills the CLI.
- Read no more than 64 event rows, 2,048 bytes per row, or 128 KiB total.
- Emit at most 256 JSONL rows, 8 KiB per row, or 256 KiB total; output is a new sibling file, outside the run tree.
- Reuse the support provider's 8,192-PID enumeration capacity. It does not retain a full process table between captures.
- Record observer RSS/footprint and Python `tracemalloc` high-water data per sample, plus process-table/compiler presence, system pressure and swap context where readable. These are context about observer cost and interference, not acceptance thresholds.
- The actual lifecycle CLI creates its own small `model/`, `normal-pool/`, `heaps.jsonl`, and final diagnostic files. Observer JSONL maximum is 256 KiB. It creates no pressure files or snapshots.

## Initial candidate validation (historical)

Source-only checks permitted for this candidate are Python AST parsing and `git diff --check`; no model process, Cargo, or physical window has been run. The script has not been exercised against a fresh binary, real cross-process phase transitions, or live native identity/footprint calls. Those require root's scheduled quiet diagnostic window.

This diagnostic can attribute the pool and Model helper at their actual lifecycle phases. It cannot prove the original normal-load residual is fixed, cannot substitute for the frozen L1 or full business load, and cannot pass the 16 GB / 8-hour gate. Existing normal residual and the earlier R07 physical failures remain unchanged; the 16 GB and 8-hour test remains not run.

## Root integration (actual, current batch)

Root isolated this increment from canonical F WIP in a worktree based on v0.5.156,
added the minimal CLI dispatch and pinned the new release executable/source. The
initial445 retained-fixture test failed because its expected application timestamp was
one second wrong.446 exposed a separate validator mistake: initialized arena bytes
exclude actual bootstrap/header. The reader now requires initialized==arena bytes and
arena+bootstrap+adapter header==the unchanged20MiB Model grant, with checked arithmetic;
the real347 fixture was not changed. Counterexamples reject malformed accounting.
447 actual normal_pool suite16/16 passed.448 formatter check failed; after formatting
449fmt/450headless all-target strict/451release build passed. Source/binary identity
is independently compared using actual baseline JSON, not just filenames.

452/453 preflight attempts did not launch CLI/observer because another Cargo/test or
Cargo fmt shell was present. No process was stopped.454 is one actual diagnostic run;
its observer records external process/compiler context. It is explicitly not clean
performance evidence or a full original-load retry. Native short phase capture may
remain incomplete; CLI and observer outcomes must be reported separately.

Actual456 CLI/observer both exited0 in35.93s after correcting the support pin. Four events
are complete but native captures only35 filled samples: diagnostic_capture_incomplete,
native_four_phase_samples=false. Full evidence and limitations are in
[actual157 record](../m0-model-lifecycle-native-157/STATUS.md). Earlier source-only statements
above describe the initial candidate; they do not override this actual finite result.
