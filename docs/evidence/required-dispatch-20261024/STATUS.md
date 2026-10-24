# #101 real required-task selection


Current-state correction (v0.5.171): live #122/#123 are closed after the v0.5.159 repair and validation (commit `1bee5b4c9ec49ebd41e0b555fb8bcbe7e8efaca5`). Any later “open” wording below was stale bookkeeping, not a newly observed failure. Historical failures and complete recovery originals remain retained; actual results in this report are unchanged.
v0.5.169 integrates the original101 owner's finite module/tests above v0.5.168. Root alone
changed shared Sim/Worker/exports and ran the serialized verification. Full101 remains open.
This is real selection for one existing mandatory air Job, not universal city scheduling.

## Behavior and authority

`Command::RunDue` takes no job ID and selects at most one actual Background.pending from the
same Store. The registry candidate must match full ActualJobBinding, World scope/clock,
business key, due/necessary flag and ephemeral sequence token. Missing/invalid hint or cache
never deletes the real obligation; a valid pending uses mandatory fallback. Actual InvalidInput,
attempt limit or invalid/missing clock/binding explicitly backpressures with pending intact.
No pending returns no_work only; it does not forecast future obligations or replay an old
RunDue result. The original receipt interface remains the recovery path for old results.

P1–P4 pending uses original Timeline.progress.at == job.end and start == last_complete.
Waiting for last_complete >= end or a fully completed Advance would deadlock P4, which waits
for this very computation. RunDue does not Advance the world, finish the old target or begin
a new window. A caller explicitly continues the original Advance after the required result.

SelectedRequired holds a private ephemeral scope/sequence/time/phase/full-binding snapshot.
Immediately before original Rebind, after the SAME owner successfully retires the old worker,
the actual Store snapshot is reread; changes reject selection. No second clock/queue/receipt
namespace, registry ACK, dispatcher process or grant owner is added. The first selection is
consumed once; original same-job failure/retry handling retains all original hard gates.
Original full Ticket/import/Accept/durable receipt checks and lost-response semantics remain.

Live outstanding request ownership differs from idle retained Child/Lease and persisted Job.running.
Worker.has_active_execution reads original outstanding/unconfirmed-start ownership. An idle
completed child does not block the next window, but the original retire still runs before
reuse and no query returns credits. A reopened running record with an empty live owner can
recover through original retirement/Rebind; a real active execution reports busy.

## Actual finite verification

Workspace/all-target release check03 and selected release no-run build03 exit0. Fixed source
and executable bytes match before/after.8 serial groups pass86 distinct tests: core registry13,
adapter8, new RunDue8, original actual dispatch2, actual controls13, compute13, broker23 and
Sim6. Ignored subprocess helpers are parent-invoked and not counted as extra main passes.
Original advisor accepted both source and actual-result finite gates without a new blocker;
review was read-only and did not rerun tests. No new build/runtime failure occurred this phase;
all earlier118 failures and two default-debug stack faults remain separately preserved.

New RunDue8 =5 real Store helper tests and3 real Sim tests. They cover candidate and absent-hint
fallback, no-work/no-write, stale sequence despite unchanged Job, full failure binding despite
equal scope/sequence/window, actual attempt/InvalidInput hard gates, and persisted running
reopen. Store guards are not public stale-candidate injection into a live Sim. Actual Sim
cases prove P4 acceptance without extra Advance, explicit completion then a second window
with an idle retained child, WorkerCells cancellation/cold reopen/fresh current Ticket, and
three cancelled attempts followed by hard backpressure without a fourth worker or lost job.
7 actual worker_started Tickets match the fixed build-script SOURCE. Missing-clock valid
pending and arbitrary old-result injection are not publicly constructible/tested here.

Full385-file source package SHA256:
`9aa32c9f1baf5b71709cdce5118d4322a9b662411ebac356a2efa970f57f23b6`.
Build-script SOURCE:
`31a429a2ad11cbaf18e7168853a7108cafbf4dbdc96f2415bb0ce8830915c455`.
They deliberately describe different read sets/hash encodings.7 additional literal include/
check fixtures outside build.rs SOURCE are preserved in a separate complete ZIP/manifest,
unchanged from the preceding118 fixed source. Default target CLI bytes are checked against
the preserved fixed CLI before/after; some existing test callers use that compiled path.
Future recovery uses the retained matching CLI, not whichever source later occupies target.

## Resources, retention and remaining work

Selection snapshots and repeated original Input identity/registry evaluations have CPU and
heap cost. They are not zero-cost observations; this phase does not calibrate that cost or
change original grant/admission limits. Only bounded current selection state is held; no
new unbounded history or cache is introduced. Existing normal64MiB/role8MiB/M0 frozen criteria
are unchanged. Parser/native/stack/RSS/queue-age/CPU overhead and true16GB/GPU/full-budget
acceptance remain unknown or not_run. Functional test durations are not performance evidence.

Mac16,1/32GiB, Rust1.85.1 release locked/offline -j1, existing HP P900 USB/APFS target/work.
All source originals/fixed exes/logs/Tickets and worlds remain in
`/Volumes/HP P900/dev/MyCity/work/issue101-required-dispatch-20261024/` and
`/Volumes/HP P900/dev/MyCity/work/p101due04-6866/`.
Actual retained worlds contain1026 regular files/105944651 logical bytes, special entries
separately inventoried.256MiB artifact planning allowance is not a runtime grant. All24
unrelated observer/clock/rare-control WIP files remain hash-identical; no stopped-main contact.
Private source tree retires only after full-original/identity/stopped-writer/occupancy checks;
branches and recovery evidence remain. PR none; direct root scoped commit/tag publication.

Full101 multi-domain periodic/provider/dependency coverage, broad concurrency/global budget,
physical peak and M0 remain open. #122/#123 default-debug faults stay separate. Next is120's
new managed-venue same-slot Store recovery bridge, then truthful54/57 upstream adapters.
