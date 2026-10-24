# v0.5.165 actual air page execution sampling — finite result

Issue #104 follows the v164 actual exact-page control path. Original advisor recommends
actual page samples before bounded automatic Continue. This stage instruments the existing
successful page calls; it does not activate the earlier pure resource-controller candidate.

The independent original #104 owner supplies fixed-size air_page_metrics and literal tests;
root owns compute_worker/CPU-clock visibility/export/actual test integration. Existing complete
Ticket, Child/Lease instance, exact plan, turn/quota and source identity remain authoritative.
Wire4 explicitly carries optional summaries on SliceBoundary/Published/Complete only. The same
final numerical-page summary is reused for Published/Complete; candidate encoding, cache publish,
verification and Store submission occur outside its timing scope.

Each successful actual state.page call has same-child monotonic wall duration and process CPU
clock difference. CPU's bracket includes clock-read/capture overhead; wall excludes the first
CPU syscall but includes the page-call/capture boundary overhead. These are measured brackets,
not zero-overhead kernel time, hard deadlines or new generic work-cost units. Ordinary pages and
whole-grid segment integration pages retain separate counts, sums and maxima. Counts come from
actual original loops. Failed/no-op pages do not claim completed work. One bounded turn summary
replaces the previous one; no unbounded page history is kept in the game process.

Each missing/failed/backwards/overflowed clock remains Unknown independently. Invalid summary
shape or mismatched work becomes unavailable observation without rejecting otherwise valid
business, creating a grant, completing an obligation or inventing real shortage. Parent local
submit/Continue-send to boundary receipt includes IPC and scheduling; initial turn additionally
includes preparation. Parent boundary-to-Continue pause uses its own clock; cross-process
absolute clocks are never subtracted. Retirement stops the current-pause observation; the saved boundary-to-retirement interval
includes owner retirement wait and is labelled separately from boundary-to-Continue.
Continue clears the previous completed-page summary before advancing observations; a running
turn is explicitly not-yet-sampled. Original boundary Events retain the frozen old evidence.
Diagnostic Trace callbacks, when enabled, are inside the page bracket; calibration must report
whether diagnostics were enabled and cannot call that bracket pure kernel time.

## Fixed-source actual verification

Full362-file build read-set SHA256:
`231a4885e6c802cab7ca76a02beb1c408561b7297fa4785d744300f5ab71d1bc`.
Locked/offline release build and workspace all-target check passed. Eleven serial fixed-
executable commands passed91 tests: page metrics12, parent projection3, actual control5,
turn bookkeeping8, CPU11, observation5, IPC3, compute13, broker23, Sim6, registry2.
Subprocess helper rows remain separately ignored/invoked by their parent, not extra totals.
Seven exact executable bytes, complete source and ancillary root101 fixtures were verified
before and after. Full actual worlds remain:769 regular files/79,336,565 logical bytes at
inventory; sockets/FIFOs listed separately and not read as files.

The actual Sim path verifies that an intentional25ms pause increases parent current-pause
observation without changing the frozen numerical-page summary. Sources/Cells events contain
no old summary labelled with the new quota. Final Published/Complete summaries match. Original
wrong control/explicit retry, cancellation cold restore, full checkpoint equality and broker
no-refund-before-real-retirement cases also pass. Pure malformed/missing/mismatched sampling
becomes unavailable and does not mutate dispatch/completion responsibility.

Twelve additional raw actual worker processes compare retained v164/wire3 and current wire4
on one legal retained synthetic17-source/32x32/60-time input, two fixed quota profiles and
three repetitions. Every complete44,638-byte checkpoint is identical (SHA256
`2a8e90f8b6a207fd8ab82a39d85c52fb5a285204075d0aa1ce9645cccbeb6fac`).
All237 new turn summaries have known wall/CPU on this machine and include3 integration pages
per profile across three runs. Diagnostics/Trace are disabled for these paired runs.

| Profile | Old process wall ms (all three) | Sampled process wall ms (all three) |
|---|---|---|
| source1/cell17/pages1 |40.658,29.325,94.946|200.857,35.443,27.478|
| source8192/cell1024/pages16 |16.692,16.912,20.668|16.836,17.334,21.317|

These intervals include spawn, decoding, numerical work, serialization, IPC, waiting,
scheduling and capture bookkeeping; they are not per-page compute costs or an isolated causal
instrumentation-overhead estimate. All raw observations, including the200.857ms sample, remain.
Other projects were not stopped; three repetitions cannot establish a throughput/quantile.
Small-profile max-per-turn ordinary wall readings ranged0..14,042ns, integration1,500..3,375ns;
wide-profile ordinary2,209..2,625ns, integration12,542..31,542ns. These are observations of this
one tiny input, not page-tail bounds. A known0ns reading is timer resolution/measurement evidence,
never proof of free work; unknown values are not converted to0, and no controller may divide
by a zero reading or use it to increase admission. No ordinary-page average predicts the full
128x128 terminal integration exercised by the separate Sim test.

First build03 failed E0283 in root's new pause-duration TryInto expression; no tests ran on that
source. Only the target conversion changed to explicit u64::try_from. Failed source03 ZIP/log/
receipt remain; final source04/build04/tests05/check06 are separate immutable evidence.
Original advisor required clearing the previous summary at Continue and accepted that narrow
source correction before execution; final actual result review is recorded with the release.

Hardware Mac16,1/32GiB; original HP P900 USB/APFS target/work only. Complete source/executables,
failed and successful logs/worlds, raw samples and owner originals retained under
`/Volumes/HP P900/dev/MyCity/work/issue104-page-sampling-20261024/` (512MiB planning allowance,
not a hard memory/disk grant). No unrelated WIP, old restoration original or other project
process may be removed. Only this phase's stopped owner tree may retire after full archived
bytes/identity/occupancy/dependency checks.

Full #104/#103, automatic control, whole-game CPU/GPU/resident budget, page-tail guarantees,
16GiB, GPU, long stress and M0 acceptance remain unproven. New samples cannot increase admission,
concurrency or claim released memory. Next gate is bounded automatic Continue on the original
already-granted task; unknown costs retain conservative execution or explicit backpressure.
