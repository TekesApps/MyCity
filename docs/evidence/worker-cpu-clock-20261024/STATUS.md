# v0.5.163 explicit air-v2 process CPU accounting — finite result

Root integrated the original #102 worker CPU helper after original-advisor source review and
original-worker test repairs. Air transport is explicitly `mycity-air-worker-v2`; old v1 Requests
are rejected rather than silently extended. The original air algorithm, 8MiB candidate ceiling,
64KiB control frame, Pool/Lease owner, actual wait/retirement and Store authority are unchanged.

## What is measured

The child starts safe, fallible POSIX ProcessCPUTime accounting after complete Request decode and
original Ticket validation, before workspace/input work. Each Event samples a checked cumulative
nanosecond difference from that Request baseline. It includes all process threads, input work,
prior event serialization and CPU used during intervening waits; it excludes earlier decode and
the final sample's following serialization. It is not pure solver time. Each new Request establishes
a new baseline. Sources/Cells/Published/Complete values are cumulative and must never be summed.

Parent elapsed uses the retained owner's Instant from send-start to Worker::next channel receive,
before candidate file validation. It includes transfer/wait and possibly buffered queue residence;
it is not child CPU or an instrumented queue-age measurement. Interval zero is the explicit
per-attempt host origin, not a cross-process epoch. Child CPU timestamps are never subtracted
from parent timestamps. Child accounting has source `ChildProcessAccounting`, distinct from host.

Full original Request/Ticket/PID checks and existing candidate read/hash/size checks precede
observations. The first valid application instance is pinned to the retained Child/Lease and
cannot be replaced during that ownership. This is the worker's application instance, not an OS
birth certificate. Core can refine Unknown to the bound instance without rewriting past reports.
Changed CPU baseline, regressed clock or invalid identity/version cannot become Known. Missing
or decodable but semantically invalid telemetry does not reject valid business. An entire Event
that fails deserialization still takes the original wire-error path. Clock failures are Unknown,
never zero; a transient sample-read failure may recover only against the same original baseline.
Work units, scratch peak, queue age, native peak and whole-game resource costs remain unknown.

## Frozen source and actual serial results

Mac16,1 /32GiB, HP P900 USB/APFS, existing target/work only. Full357-file build read-set source:
`3cfc368aff9be0786020cc16c8c9b1dc70df089425515fb080ddd803f3db2913`.
Three root #101 fixtures are separately retained and hash-checked; build SOURCE excludes them.
Release locked/offline builds succeeded. Eight serial fixed-executable commands passed:

| Group | Passed |
|---|---:|
| Core observation schema |11|
| Child CPU sampler |11|
| Host observation |5|
| Bounded IPC reader |3|
| Actual compute worker |13|
| Actual broker |23|
| Actual Sim host |6|
| Actual #101 dispatch |2|

Total74 selected tests. The omission helper and broker helper are explicitly invoked in child
processes by their parent tests; ignored helpers are not extra independent acceptance totals.
Real unpooled/direct workers each execute two Requests in the same instance with monotone
cumulative samples and new baselines; broker obtains actual CPU accounting too. Deliberate
telemetry omission is restricted to gated fault-lab mode: business completes with unchanged
numerical checkpoint while CPU/instance remain Unknown. A killed Request retains only its
last Cells observation, not a final total. Stop/wait and explicit owner retirement remain distinct.

21 retained JSON reports include duplicate Complete/observed copies; they are not21 tasks.
Root independently checked all clock delta arithmetic and provenance:18 Known CPU reports,
3 omitted/Unknown reports, all with separately measured host elapsed. All357 source files,
three ancillary fixtures and seven fixed executable bytes remained unchanged during the run.

## Failure and recovery retention

Source05's compute run was12passed/1failed/1ignored. The new test incorrectly asserted that
stop cleared the owner. Root changed only that test to assert retained owner after stop, then
explicit retire and cleared owner, with historical partial data unchanged. Original advisor
accepted this correction. Preserve original source05, binaries and failure; do not rewrite it.
Earlier check04 passed on its earlier source; final frozen source07 is proven by builds07/tests08,
not retroactively relabeled as check04. No global stack, workload or M0 acceptance change.

Complete originals, three frozen sources, owner originals/repairs, fixed executables, logs,
results and manifests are at `/Volumes/HP P900/dev/MyCity/work/issue102-wire-cpu-20261024/`.
Actual success root is `/Volumes/HP P900/dev/MyCity/work/p102cpu08-80966`; failed root is
`/Volumes/HP P900/dev/MyCity/work/p102cpu06-79211`. Their regular-file inventory retains639
success files; FIFO/socket paths are listed without reading them as file bytes. Combined logical
bytes were183,248,886 at accounting time, within the256MiB planning allowance, not a hard disk
quota or APFS reclamation claim. Retain until a newer complete recovery baseline is accepted;
no source/worktree/WIP/cache/original deletion in this stage. Original102 tree remains in use.

Original advisor read fixed logs and actual reports, accepted finite results with no new blocker.
Full #102 remains open: no general in-Sim protocol, all-model work accounting, dynamic grants,
GPU fences, total process memory, long stress, real16GiB or frozen M0 acceptance follows.
Next #103/#104 gate needs genuine page-boundary yielding and actual work/latency/cost evidence;
changing page size in a continuous loop alone does not prove resource sharing.
