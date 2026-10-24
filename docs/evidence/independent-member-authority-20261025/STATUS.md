# One member ledger, independent registration and original Store recovery

Root implements the #57 shared-ledger prerequisite. Optional Opening.member_activity
explicitly registers version1, original Housing scope/population hash/total and epoch1.
It creates the existing member_activity::Ledger, not another population or activity pool.
Legacy education still implicitly creates that same ledger. With both registrations,
the ledger is established once and education borrows it; no ordinal reset or second copy
is installed. World has no new field. Only the new ledger-without-education shape uses
state-v12; original education schema and optional legacy field serialization remain.

Shared validation checks original namespace, completed-clock watermark, ranges,
exclusion and reserved close ordinals. The education runtime claims its exact active
rows. No consumer permits only a pristine empty ledger, not invented past history.
ServiceWork is rejected until a real service runtime can claim it. No public generic
Begin/End bypass is introduced. Old education settlement and exit do not require the
new optional field. A small read summary avoids constructing a whole WorldView.

Store compares presence and immutable ledger registration fingerprint against original
genesis after checkpoint load. Dynamic ordinal/active/settled cursors are excluded from
that fingerprint and recovered as recorded; they are not compared to or reset from
genesis zero. Original receipt and WAL/checkpoint authority remains the only publication
path. These checks do not constitute universal tamper or all-input security acceptance.

## Actual fixed-source results

Frozen416 source/input files, ZIP SHA256
`11ab0ff9aacfed894d5ac89840731bb79b93adcd893b2195ed8911f9ecf3a1ec`.
Workspace/all-target release check and all three selected build commands exit0.
Serial release core270 +member Store3 +education Store6 +qualification Store6 =
**285 different parent tests pass**. The15 Store tests also pass separately in default
debug with RUST_MIN_STACK unset. They are15 executions, not15 new distinct tests.
All32 distinct fresh child processes succeed without timeout: new member12,
existing education6 and qualification14; ignored helpers are actually invoked by
their parents and are not32 extra test passes. Fixed source, seven executables and
24 unrelated WIP hashes match before/after execution.

New tests prove valid standalone registration before negative cases, classic/spatial
WAL and checkpoint fresh-process recovery, original-receipt replay, old EngineSlot
clearing, and rejection of orphan ServiceWork, unowned cursor history and wrong epoch.
The explicit-registration education input and original input produce exactly equal
Engine checkpoint bytes. Actual preparation/nonclosing settlement retains active1 and
next_ordinal3 across cold reopen, then closes to active0/next_ordinal4 with unchanged
immutable registration. Existing education and qualification v2/v3 paths also pass.
Core mirrors compare direct-derived current Opening decoding, including new registration,
while the frozen legacy World wire oracle remains unchanged.

Original advisor accepted concrete source before execution and actual results afterward,
read-only, without a new causal/authority/recovery blocker. Reviewer did not execute or
modify files. Existing unused-import/dead-code warnings remain in raw logs. A metadata
audit initially assumed every child used timed_out; older education uses timeout.
The KeyError stopped that audit before a summary. Corrected extraction requires the
actual key and preserves original JSON; missing keys are never defaulted to success.
No build or game-runtime failure occurred in this phase.

## Budgets, provenance and incomplete scope

Mac16,1/32GiB, Rust1.85.1 release/default debug locked/offline -j1, existing HP P900
USB/APFS target/work. Standalone fixture World logical owned130984B includes the one
3192B ledger; registration decode backing72B is transient and Box/parent pointers are
not counted as another ledger. Original initialization, candidate and restore overlap
remain charged to original bounds. This explicit member profile is not proof that every
joint configuration fits the old base128KiB contract. Original32 activity slots,
Engine4MiB/role8MiB/normal64MiB and M0 frozen loads/standards are unchanged.
Logical counts and functional durations are not native/RSS/stack peaks or speed calibration.
No16GB, GPU, longstress, full M0, production CLI or universal stack-safety claim.

Full before-source originals, fixed source/fixtures/executables, commands/environment,
stdout/stderr/PIDs, all complete worlds and recovery records remain under
`/Volumes/HP P900/dev/MyCity/work/independent-member-authority-20261025/`.
About214.4MB logical bytes before final receipts, within the256MiB retention planning cap;
this is not a game resource grant or APFS released-space count. No private worktree was
created or evidence deleted. Shared target dependencies are retained for next integration.

Full #57 stays open. Natural needs, dispatch, arrival, actual utility/work and safe return
are still pending; registering an empty ledger is not service fulfillment. Original #85
owner's read-only preflight confirms that Boot/Town/construction/Exchange quantities
cannot directly certify this consumer's same-window electric/water delivery and one-time
use. Required temporary equipment remains part of joint utility/network solving; a stock
buffer is not silently relabelled as non-stored grid supply. Both original owners retain
their boundaries. Original #122/#123/#124 remain separately repaired; no old defect is
reopened by these results. PR none; scoped direct root commit/tag publication.
