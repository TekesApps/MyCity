# Actual bounded Model lifecycle diagnostic — v0.5.157

Issues #112/#113 and original M0 #20. Rust/CLI only. This is instrumentation plus an actual
full-reference Model helper diagnostic, not a memory fix or original M0 acceptance.
Root isolated it from uncompiled F work in a v156-based worktree; original canonical WIP stays.

## Contract and real execution

`normal-pool-model-diagnostic NEW_ABSOLUTE_DIR` invokes the existing Process pool and actual
scale_model_task full/reference helper, then joins the pool. Authority is the fixed source/
executable pair, pool ready identity, actual helper ResultRecord and four original lifecycle
events. Application instance time is distinct from independently observed OS start_abstime.
No new clock/ledger/market simulator, no L1 pressure and no Sim. Exact64MiB pool,20MiB Model
grant and production1536B event-row cap remain. Diagnostic readers bound rows64×2048B; this
does not enlarge production rows. CLI rejects existing/noncanonical roots or wrong env paths.

447 actually passes16 normal_pool tests, including the unchanged retained347 real fixture,
missing/changed identity, truncated sequence and broken accounting.449fmt/450headless all-
target clippy-Dwarnings/451release build pass. `fixed-cli.json` pins framed source3d30cb89…
and executabledda0d9b2…; actual baseline JSON verifies embedded source matches the archive.

454 actual CLI exits0 but observer exits2 before any sample: native_metrics support pin was
transcribed incorrectly. Full originals and the463-byte fail-closed observer log remain.
Root verified all four actual helper bytes, corrected only that pin and README, then456
uses the same unchanged Rust executable. It actually exits CLI0/observer0 in35.93s.
Four prepared/filled/retiring/returned events are validated, pool grant fully returned after
owned wait, report balance true. Helper executes full CPU reference720 hour targets/30 game
days with64 accounts/8cities/20000claims/64payroll/128stocks/8quotes. This has no original L1/
Sim contention and cannot be compared to old joint runtime as a speed improvement.

Native outcome is **diagnostic_capture_incomplete**. Only35 identity-complete filled samples
exist. A retiring sample crossed into returned and was rejected; returned sampling found
supervisor gone. Missing metrics stay null. Prepared was not captured. Exit0 is not native
four-phase completion and there is no before/after reclamation measurement.

Filled pool footprint47464832B, helper23118160B, supervisor1425720B; threads2/2/1. These are
individual observations with stable OS identities, not synchronized cross-process peak.
Each capture group spans5.42–6.12ms. Observer itself reaches16154936B footprint/20840448B RSS,
separate from model roles. Captured external compiler count0 does not establish globally clean
full-performance coverage;452/453 preflight blocked on other Cargo/test or fmt processes, no
other process was stopped, and all system/swap context remains. VM-region/per-stack residency
is unavailable. Thread counts or heap/footprint gaps are not reclaimable-stack proof.

## Failures retained and correctness boundary

445 exposed a test expected application timestamp1second wrong; original347 fixture unchanged.
446 exposed validator treating initialized arena as whole grant. Real allocator partitions
grant into arena+bootstrap+adapter header; now checked exact sum, initialized==arena, peak≤
arena and grant==original20MiB. A one-byte corruption counterexample rejects.448 fmt failed,
then formatted-only449 passed. Preserve failed source ZIPs/logs/test binaries; do not relabel
these earlier source versions as passed. Corrected native helper hash is in the support-pin
receipt; former private claim that all pins matched was wrong.

The raw456 original directory, complete helper request/result/CPU artifacts, event/pool logs
and observer JSONL remain on external work; manifest lists every retained file.454 original
also remains, and455 was an attach precheck that refused an already-exited PID without
launching a new observer/helper. No hash replaces a recovery or diagnostic original.

## Computation and resource contract

Diagnostic trigger is explicit CLI invocation; steady normal operation gains no timer.
Existing event sequence is authoritative; reader/identity validation is bounded and cannot
batch past a phase/PID/generation change. Native observer polls50ms and samples filled at
most1Hz, max620s,256rows/256KiB output,8KiB/row, fixed8192-PID provider enumeration; terminal
incomplete on any output cap. Observer and diagnostic files are caller-owned evidence, not
new city/entity resident history. Model heap, assigned arena, actual native footprint and
observer resources remain separate. Full original frozen load/thresholds are unchanged.

Original347 peak3341357208B exceeds3328599654B by12757554B. Same-frame6423464B theoretical
role subtraction leaves6334090B arithmetic residual, but applied state still replaced0 at
that frame; neither subtraction nor this new diagnostic proves physical reclaimability.
Original normal/common8MiB/speeds/8h remain unmet. Actual16GB remains not_run on32GiB Mac16,1.
All work/shared target use external HP P900 USB/APFS SSD. Advisor accepts this finite scope
only, with no originalM0/A–F/full-product completion claim. Next memory work needs a specific
resident allocation/VM-region attribution and real fix before an original-load long retry.

Retention: after terminal-round handle checks, byte-identical445–451 immutable source ZIPs
share full hardlink content at every original path: allocated39960576→22831104B, reduction
17129472B(16.336MiB). Separately observed volume free+17088512B is not exclusively attributed.
Actual failed445/446 and passing447 debug test executables plus451release binary are retained.
No original world/result/snapshot/lockfile/WIP was removed; shared target remains for F.
No new full-work scan or current total-size assertion is made.
