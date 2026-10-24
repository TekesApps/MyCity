# v0.5.166 bounded actual automatic Continue — finite result

Issue #104 now has a production `adaptive_client::AdaptiveSession` API for **one already-
granted exact air task**. It reuses existing `Client`, RunRequiredSliced, safe page_boundary,
PageControl and the original Sim/Worker/Broker/Store owners. It does not enable the older
whole-game controller candidate, another scheduler, a new resource pool or an approximate model.
Selection is explicit in the production library API; frozen M0 and existing manual/default
commands do not silently select it. No UI, GPU or new stdin reader is introduced.

## Policy, sample and dependency contract

Original #104 independent owner supplies air_page_policy/tests/handoff from a fixed v165 tree.
Root alone integrates Client/adaptive session/export/actual tests. Fixed policy
`air-exact-auto-pages-v1`/version1 changes only pages1/2/4/8/16, keeping source/cell limits and
numerical strategy/iteration order unchanged. Target1..1e9ns is an execution-timeslice preference,
not a CPU entitlement or a hard deadline. Two qualified same-phase low(<half target) or
high(>target) turns and minimum hold2 qualified turns permit one rung change. This is a minimum
**turn count**, not a wall-clock holding period or completed global anti-oscillation contract.
Middle band/phase change/retarget resets trends as defined in the original handoff; bounded
counters saturate. Ordinary versus actual whole-grid integration remain distinct. Measured
single-page/integration overrun is explicit; reducing pages cannot subdivide that call.

Known positive page-call wall sums drive this local policy. Missing/invalid/empty/zero/unknown
wall samples reset trends and choose conservative pages1, with reason and unresolved status.
CPU stays independently Known/Unknown observation and is never substituted for wall or zero.
No extra admission/concurrency or presumed resident-memory release follows from any decision.
An unknown decision's false overrun flags mean no affirmative classified evidence, not proof
that the target was met. Unknown/0ns are never proof of free computation.

Session binds the full original Ticket and current build source, owned worker pid, plan,
instance, turn and unchanged source/cell quota. A verified wire SliceBoundary must match the
outer page_boundary. It clones a bounded policy to prepare exactly one pending decision,
returns that boundary/decision, and only the **next step** attempts Continue. An explicit
boundary retarget increments target version and rebuilds from the unconsumed base policy;
it does not count the same completed sample twice. Final short turns use read-only
preview_final; Published/Complete do not authorize a pointless Continue.

## Ownership, uncertainty and recovery

Running/BoundaryReady/QueryPending/CancelPending/ControlUncertain/Finished are distinct.
Observe and ReceiptProof must receive their matching response before automatic Continue;
cancel/retarget are rejected while a query is pending. Cancel consumes pending work and drains
only the original result. Continue consumes its saved decision and records attempted status
**before writing**; uncertain send cannot be retried, credited or treated as completion.

Adaptive initial submit reserves Client.outstanding before any write. A failed start therefore
keeps the original Client busy/not-empty until original stop/retirement. Existing normal submit
behavior is not silently replaced. Typed ReceiveError separates actual Timeout from decode,
identity and disconnected-channel Fault: only Timeout permits same-owner polling; Fault stops
automatic control and retains original ownership. No error-string matching makes this decision.

Final business Boundary/identity changes remain hard errors. Auxiliary final Summary missing,
invalid or changed becomes unavailable diagnostic and original result is still consumed; it
cannot turn valid necessary business into a sampling failure. Drop of Session only releases
a borrow: it is not cancel/retirement/refund, and original Client retains its outstanding id.
The caller must keep the original supervised Client scope or explicitly retire through it.

Wall samples/streaks/pending policy are not reconstructed across process death or replay.
An explicit fresh attempt re-observes; original durable input/checkpoint, Job obligation,
receipt and actual progress remain on the existing authority path. Numerical partitioning is
equivalent, so no wallet, random result or engineering progress is reset by a new policy.
A one-task timeslice preference does not complete whole-game fairness, calibrated switching/
resident cost, speed-target/backpressure or multidimensional budget contracts.

## Actual fixed-source verification

Mac16,1/32GiB; existing HP P900 USB/APFS target/work. Full366-file read-set SHA256:
`7964e9dd1caf28fb6d94067f0508e8b9c2dee88e32e4fc34d5625a3995ebc985`.
First locked/offline release build and final workspace/all-target check pass. Fourteen serial
commands pass111 tests: policy12, adaptive projection2, typed receive1, page metrics12,
page projection3, actual control10, turn bookkeeping8, CPU11, observation5, IPC3, compute13,
broker23, Sim6, registry2. Ignored subprocess helpers remain parent-invoked/separately reported.
Source, ancillary fixtures and seven fixed executable bytes match before/after execution.

Actual production API records one step-up and one step-down, with unchanged-quota holds and
bound/phase reasons. The test explicitly changes target from1e9ns to1ns after obtaining an
actual positive sample; this is **not evidence that one fixed target was calibrated**. Every
continued quota is recorded alongside its real sample, original identity and observed next
turn. Query/proof responses precede Continue; no query fabricates a receipt or completes work.

Fixed reference versus automatic complete checkpoint files are688,908bytes, byte identical,
SHA256 `e42c8e11f0c9202eaec5cbe1f9abee9ae268169dddf926ccfd1763890fcca4b8`.
The earlier fixed/alternating path regression also passes on this same fixed source. These
are actual process/Store execution paths over fixed legal synthetic inputs, not representative
city/population or real16GB performance evidence.

New actual cases also cover boundary cancel plus cold Store reopen/fresh automatic attempt,
dropping the borrow while the original Client/Job remain owned, and startup/Continue write
failure after deliberately killing **only each test's own known Sim child** and checking its
terminal OS state. Original owner retirement succeeds; no implicit resend/new attempt is used.
These are actual broken-pipe/error paths, not a claim of physically injecting every possible
partial kernel write. Literal final-sampling and typed-channel counterexamples remain separate
from actual process evidence. Existing wrong-control, retry, broker-no-early-refund, CPU,
registry and original Sim tests pass. No other project's process is stopped.

Original advisor found two source blockers (final auxiliary sampling causing business failure,
and receive errors conflated with timeout); both were repaired and narrow-reviewed before the
first compile. Actual result review accepts this finite stage with no new causal/budget/owner
blocker. Full #104 remains open; no M0/16GB/GPU/native/long-stress or hardware capacity acceptance.

Complete owner/shared originals, fixed source/executables, commands/logs/results and all849
regular actual-world files (88,037,151 logical bytes at inventory), plus separately listed
socket/FIFO paths, remain under the persistent package and root recorded in run-roots-05.json:
`/Volumes/HP P900/dev/MyCity/work/issue104-auto-continue-20261024/`.
Planning allowance512MiB for evidence/worlds, not a hard memory/disk grant. Preserve unrelated
WIP, older recovery originals, branches and target. Only the phase's frozen private tree may
retire after complete byte-matched archive, identity/occupancy and root dependency release.
