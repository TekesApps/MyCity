# v0.5.164 exact air pages and production pause/continue — finite result

The original #103 independent child supplied only bounded pure control module/tests/handoff
from its dedicated v163 tree. Root integrated actual air loop counters, worker and sole-writer
Sim paths. Original advisor read source, required the control-failure retry correction, then
accepted final actual results without a new causal/authority/recovery blocker. Full103 stays open.

## Exact execution and ownership contract

The numerical strategy remains `run36-air-f32-v1`. Fixed plan `air-exact-pages-v1`/version1
controls only equivalent execution partitions, not approximation levels or a second scheduler.
`State::page_counted` counts successful Sources/Cells loop bodies separately. Segment-end whole
grid integration is additional `integrated_cells` plus `segments`; `pages` counts executed,
nonterminal-or-final pages, while a completed no-op returns all zeros. Failed pages return no
successful count. Existing public page APIs retain their business behavior. Workspace diagnostic
pages now omit completed no-ops; different partitions legitimately have different page counts.
Counters change neither checkpoint layout nor floating operation order/time segmentation.

Explicit `mycity-air-worker-v3` adds optional controlled quota, SliceBoundary and a distinct
ContinueFrame. Old wire v1/v2 Requests reject. Quota bounds: Sources1..8192, Cells1..1024,
Pages1..16 per turn. These are execution bounds, not calibrated hardware capacity claims.
The child pauses after a real nonterminal page boundary, retains scratch/cursor/grant, emits
one boundary and waits for one Continue. Final completion requires no pointless Continue.
Full Ticket, actual retained Child/Lease application instance, plan version, turn, quota and
bounded work shape are checked. Turn starts at1. Duplicate/old/skip/cross-instance/invalid
control cannot authorize another page. Probe ACK is a different message and cannot resume it.

The parent stores the exact prepared in-flight Continue before write; write success is not
execution/retirement acknowledgement, and send uncertainty is never blindly resent. Published
adds final turn delta once; Complete must match that same final envelope. Generic CPU work-cost
measurement remains Unknown: source/cell/integration/page units are not combined or converted
into the earlier planner's ResourceCost. Request process CPU begins before new turn bookkeeping;
owner elapsed remains a separate clock. Reports include current turn/quota and fixed plan.

`RunRequiredSliced{job,plan,quota}` selects the only trusted exact plan and reuses the original
required-job path. Existing RunRequired/probe/frozen M0 parameters do not select controlled mode.
Sim consumes events one at a time; original uncontrolled mode retains its four-event cap. At a
real boundary the existing parent may Observe, request an original receipt proof, Continue or
Cancel under the same outstanding command id. It can answer queries while paused; unrelated
business mutation is not opened by this channel. There is no unconditional automatic Continue.
This is production controllable execution, not automatic reaction to whole-game pressure.

Any sliced attempt error (including control EOF, parsing/binding or uncertain send) first uses
original actual worker retirement and records Failure, then ends that command without implicit
attempt restart. Necessary Job/interval remains; explicit new command and execution identity
are required to retry. The original uncontrolled transient retry policy remains. Paused/quota-
exhausted is unfinished, never genuine shortage, completion, partial publication or resource
refund. Actual wait/retirement and original broker/grant remain sole return authority.

## Frozen source and actual serial verification

Mac16,1/32GiB, existing HP P900 USB/APFS target/work. Full360-file build read-set SHA:
`04ee4b01efa33931ede9af59a4af6e83f7a17d846a0d9c276ad511cfd5d630f5`.
Three root101 fixtures are separately retained/hash-checked (not inside build SOURCE).
Final release locked/offline builds passed;12 serial fixed-executable commands passed:

| Group | Passed |
|---|---:|
| Air fixed workspace/equivalent quotas/partial restore |4|
| Actual production control/Sim/broker |5|
| Existing air model |13|
| Full core |220 (1 ignored)|
| Pure control state machines |8|
| CPU sampling |11|
| Host observation |5|
| Bounded IPC |3|
| Existing compute worker |13|
| Existing broker |23|
| Existing Sim |6|
| Existing registry dispatch |2|

Total313 passed. Ignored subprocess helpers are invoked by their parents where specified;
they are not additional independent totals. Fixed source, fixtures and10 executable bytes
were unchanged throughout. All success/negative worlds remain,769 regular files/79,058,352
logical bytes at inventory time; socket/FIFO paths are listed without reading them as files.

Actual tests prove pause with stable counters/no receipt, original owner held, Observe/proof
responses, changed quotas and exact final checkpoint versus uncontrolled reference. Wrong turn,
instance and duplicate controls end the old command with no implicit new worker/extra pages,
then explicit new command/identity succeeds. Cancel leaves required Job through actual cold
Store reopen. Real broker grant/return/available counts stay unchanged during pause; after actual
retirement all original credit returns. Seven raw negative variants include wrong Ticket/quota,
probe ACK and stdin EOF; no half candidate or extra unauthorized page is published.

An additional pair of actual processes compares the preserved v163/wire2 worker and current
wire3 with alternating quotas on one legal synthetic17-source/32x32/60-time input. Both complete
checkpoints are44,638bytes and SHA256
`2a8e90f8b6a207fd8ab82a39d85c52fb5a285204075d0aa1ce9645cccbeb6fac`.
This cross-version numerical reference is not real-city authority or performance acceptance.

## Failures, recovery and remaining scope

Check03 failed because root put the new module export before the crate inner doc comment;
only export placement was fixed. Checks04/05 passed on their own preserved source revisions.
Source05 actual control run was3passed/1failed: root's new cancel test expected PascalCase
instead of the established serde value `cancelled`. Cold recovery was not reached in that failed
case. Corrected test, added broker/EOF cases and final source07 produced the passing results.
Keep failed source, fixed bin06, logs and worlds; do not rewrite either failure as passing.

Complete owner originals/handoff, shared originals, frozen source revisions, fixed binaries,
commands/logs/results, cross-version original/executable and file hashes are retained at
`/Volumes/HP P900/dev/MyCity/work/issue103-page-control-20261024/`.
Success root: `/Volumes/HP P900/dev/MyCity/work/p103r08-7693` (canonical location is also recorded
in `run-roots-08.json`; use that manifest if local PID-derived directory differs).
Failed root: `/Volumes/HP P900/dev/MyCity/work/p103r06-3883`.
Planning allowance512MiB, not a hard disk quota or measured memory grant. No recovery original,
protected WIP, shared target or other project's process is deleted/stopped by this stage.
Source-only tree may retire only after complete originals match and all owner/occupancy gates.

Private page cursor is not durable across Sim death: original frozen input is explicitly retried
under original obligation/identity gates. This does not reset authoritative engineering/game
progress or invent completed facts. No different approximate strategy, compatibility dependency
graph activation, work-cost calibration, page-tail bound, resident peak/switch cost, automatic
whole-game budget, GPU, real16GiB, long stress or M0 acceptance is established. A tiny cell quota
still has full-grid final integration overhead. Unknown cost never becomes zero or new admission.
