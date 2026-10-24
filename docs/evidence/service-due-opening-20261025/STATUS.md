# Service P5 required continuation and OpeningInput publication

Status: finite functional gate passed; original advisor source and actual-result reviews accepted.
Release association: v0.5.183 (root direct integration; no PR).
Base: v0.5.182 / 424e05f874e49dd494184d7dab085dd53e258d1e.
Issues: #57, #101, #120; previous #118 capacity authority remains the dependency.

## Execution contract

The existing Background selector retains its v1 semantics. Only its genuine NoWork result
permits the typed service P5 continuation selector. Busy, backpressure and faults never fall
through. This branch does not dispatch another worker, accept a future target, allocate a
second reservation, or create a queue or clock. The original Store reservation funds a single
original Timeline Advance per RunDue call. P5 may finish the already accepted boundary;
`page_committed` and `boundary_completed` do not mean the entire target or service delivery completed.
The additional branch is reported as required_registry_v2. NoWork is limited to the
participating Background/service lanes; it does not certify that every other model or
future Timeline target is complete. Other existing CPU lanes still use their original owner.

A small DueView rejects unauthenticated restored state explicitly. A selected capability binds
scope, committed sequence, full Timeline progress, authenticated runtime/registration hashes,
and the entire original continuation (start, at, target, next_serial, page_limit, remaining_pages,
remaining_bytes). Missing, invalid or exhausted continuation must fail closed. The advisory
registry uses Development(city), the actual (last_complete, at) window, and distinct identities
for not-started, deadlines and occurrences. Missing hints cannot hide mandatory work.

Before writing the one transaction the server must successfully write the complete
service_continuation_selected event with the original Envelope and before sequence/serial.
A frame write failure executes nothing. Server write success does not prove caller receipt.
The server never generates a second operation to retry an uncertain result. A caller that
retained the selected event must query that original receipt and inspect the recovered outcome
and sequence/serial. ReceiptProof presence alone cannot distinguish success and failure tickets.
Without a complete event identity, the caller must report uncertainty and recover, never blindly
resubmit RunDue. The versioned service continuation namespace is C | checked(seq+1), with the original
low-bit limit. General Sim Execute rejects C/D/E; core Background D permissions remain
unchanged. Before the selected event, an existing C receipt causes explicit namespace
collision, never reuse or renumbering. Historical C use is possible; ReceiptProof still
queries such tickets. Collision blocks automatic RunDue only: original manual Timeline
Advance with the same target/page_limit and an explicitly unused ordinary operation remains
available through the original Store reservation, which must never be cleared to bypass it.

## Publication and recovery contract

The new supplied-bytes OpeningInput case exercises original workspace backing identity and
Store creation, actual HousingLease entry and partial P5 receipt authentication on cold open.
Separate EngineSlot transitions exercise legacy -> service -> legacy -> rejected damaged
candidate -> original service. A rejected candidate cannot publish or mutate the active slot.
Direct checkpoint restoration remains unauthenticated; only original Store receipt verification
can authenticate an entered service runtime. This does not claim Store exposes workspace reuse
across distinct open/close sessions.

## Verification and retention

Completed: workspace/all-target release check; fixed release/debug builds; focused serial
new selector/real Sim, OpeningInput/slot and existing dependency regressions;
default stacks and original Engine/role/normal budgets, frozen M0 workload unchanged.
Use only HP P900 target/work. Source snapshots, failed attempts, fixed executables, receipts and
worlds are retained under work/service-due-opening-20261025 (initial 512 MiB evidence budget).
The 24 unrelated protected WIP files remain outside this phase. No new worktree or automation.
This is functional validation, not real 16GB, GPU, long stress or fast-forward acceptance.

## Actual result

29 distinct release parent tests passed: capacity_read 5, venue_capacity 10, previous
Background RunDue 8, new service-needs RunDue 3, pure cursor encoding 1, previous OpeningInput
venue Store 1, new OpeningInput service Store 1. The new OpeningInput case also passed once
under default debug with RUST_MIN_STACK unset. Four new fresh reader PIDs (two release and
two debug) independently passed the WAL -> checkpoint recovery sequence without timeout or
wait error; child helpers are not extra parent tests. Existing helper counts are not included
in that new-reader count.

Attempt01 built successfully but its first ten venue tests all stopped at missing external
root environment configuration, before business assertions. Root corrected only the driver
environment; attempt02 reused the same frozen source and all 13 fixed programs without
rebuilding. All eight functional groups then passed. Initial failures and full binaries,
source archive, worlds, logs and receipts remain intact. Executed source/input/document set
9717 files ZIP SHA256 `ab28acafcf3170815e2afb658fe2302ff87cc33ec84073a21d20889e3f7b6140`;
source/program bytes and 24 protected WIP files remained unchanged during execution.

The lost-terminal case actually receives the selected event, deliberately does not consume
the terminal response, closes its owned Client, then reads the successful original Store
receipt and replays the exact Envelope. Sequence/serial advance once. This does not certify
arbitrary power loss or every transport failure. Deadlines row zero is only an encoding unit
case; canonical fixed 32-household/128-row pages exercise no-run -> Occurrences row zero.

Actual environment: Mac16,1 / 32 GiB, Rust 1.85.1, HP P900 USB/APFS external target/work.
No frozen M0 threshold, normal64MiB/role8MiB/Engine4MiB or stack limit changed. No native peak,
true16GB, GPU, long stress or throughput/speed acceptance. Full #57 dispatch/arrival/work/fees/
return, complete multi-model #101 coordination and full #120 typed profiles/migrations remain
open. Earlier stack fault issues retain their separate historical evidence and status.

Artifact planning deviation: initial 512 MiB was exceeded (about 651 MiB actual logical bytes)
because this snapshot included historical tracked document evidence. The explicit retention
revision to 768 MiB was made after discovering the overrun; it is not a retroactive claim that
the initial budget held, and does not change game runtime budgets. All original evidence is
retained; next phase must size the actual build/input read set before launching tests.
No new worktree or automation was created; shared target is retained for dependent integration.
