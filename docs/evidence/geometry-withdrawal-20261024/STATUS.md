# v0.5.174 — stale geometry withdrawal and finite #25 closure

This closes the original single-map WALK/Scene/profile/Store work package of #25,
not parent #3/#6, all transport modes, production providers, or city-scale acceptance.
Root remains the sole shared writer and serial verifier. No UI/GPU/AgentKit changes.

## Concrete defect and repair

The existing Store applied activation preflight to every SpatialGeometry command.
A pending request became impossible to withdraw after its parent transport changed,
or its effective day passed. The core already permitted an authorized withdrawal of
the original request, but Store rejected it before reaching that transaction path.
There was no automatic request/approval retirement substitute.

Withdraw now has a separate original-commitment preflight: read the original retained
request and bounded canonical/hash-checked descriptor; bind world/branch, asset and
original geometry revision. It does not require the current activation parent,
candidate rebuild, future service day or midnight. Original core caller/state-revision,
Store health, Envelope/receipt, WAL and commit validation remain. Request/Approve/Apply
retain their original strict activation checks. No stale grant is made valid.

Only the original request and its approvals retire. Scene, profile, historical proof,
accounts, stock and other World fields remain identical; World revision advances exactly
once for the successful withdrawal. Failed caller attempts keep their failed receipt;
reply loss is recovered with the original Envelope, never a replacement business ID.

## Actual fixed-source verification

Source02 contains 448 pinned files, SHA-256
`e14a14dce0cbf9dac0c5f465f0e6744336415c23734e7a3372ff88aa8f60b425`.
Release all-targets check and fixed-program build exited 0. Source and preserved executable
hashes were unchanged before/after the runs. Rust 1.85.1, default stack, locked/offline,
one Cargo job, original external HP P900 target/work on 32GiB Mac16,1.

- F Store suite: original eight plus two new tests, **10 passed**.
- Storage library: **34 passed**.
- **44 distinct tests**, zero failures/ignored. The two initial focused passes repeat
  inside the F suite and are not counted again.

The new tests cover target and other-asset Close, the old Apply still rejecting, original
applicant withdrawal, effective-day expiry at a completed non-midnight tick, wrong hash,
unauthorized executor, Applied response loss, cold recovery, checkpoint/archived receipt
retry, request/approval retirement, highwater retention and actual new-slot reuse.
The closed-access cases retain historical proof bytes. Existing F tests continue to cover
same-Store access changes, geometry costs, approvals, malformed input and archive replay.

Wrong-hash rejection happens at original-request binding. Missing/tampered descriptor-file
rejection is supported by the unchanged bounded canonical/hash decoder, but this phase did
not execute those additional file-fault injections. It does not claim that they ran.

## Preserved failures and precise oracle

Before the producer repair, source01 compiled and ran the two new tests: **both failed**.
The raw causes were `geometry_non_genesis_transport_state` and
`spatial_geometry_calendar_not_complete`. Complete failed worlds, WAL/root/catalog/Scene,
source archive, executable and output remain under the external package; neither failure
is rewritten as a pass.

Static pre-run review also corrected an unreachable new-test success assertion: a committed
withdrawal must advance World revision by exactly one, rather than keep the entire WorldView
byte-identical. Source02 asserts that precise increment and equality of every other field.
The original eight acceptance assertions were not relaxed. A test-support wrapper preserves
the old path unless the new withdrawal case is explicitly selected.

## Original #25 acceptance mapping

| Original criterion | Actual evidence |
|---|---|
| Bounded interface, fixed legal input and original authority | Original Scene/profile/dual-role grant and immutable source paths, fixed geometry microfixture; current withdrawal only retires the original request. Source manifest and full retained originals identify the exact implementation. |
| Same Store changes access/trade, historical proof, permissions/version rejection, lost reply/recovery | Historical [v159 fixed gate](../release-association-20261024/STATUS.md), current F 10/10 including original eight, stale Apply rejection and successful withdrawal after Close, cold/retry assertions. |
| Actual receipts, oracle, failure/boundary/recovery evidence | Current baseline two failures and fixed F/library logs, preserved source/bin/worlds; original business and full-copy cold-reader evidence retained rather than replaced by hashes. |
| Activation/clock/version/precision/writer/workspace/cache/IPC/GPU/disk/exit registration | [Existing F document, current registration](../../core-spatial-traffic.md#f-同步任务与资源登记25-有限工作包), original enforced table and logical budget gates, historical/current microfixture metrics with unknown peaks explicitly separate. No invented worker or new clock. |

The original advisor read the exact saved live Issue body, source change and actual final
logs; no remaining blocker for this limited closure. The earlier proposal to close after
documentation alone was superseded when root found the withdrawal defect; closure follows
its real reproduction, repair and 44-test verification.

## Resources and remaining scope

The synchronous CPU call uses the original 8-request/16-approval tables and budgets; no new
resident cache, IPC or GPU allocation. Descriptor decode remains bounded to 4KiB. The actual
geometry metrics here are whole-microfixture retained/logical observations, not the feature
delta or a measured transient/native/stack peak. The neighboring 1025-commit test is archive
correctness, not a geometry performance benchmark or new M0 pressure run.

Full sources, fixed executables, logs, failed/successful/recovery worlds and receipts are in
`/Volumes/HP P900/dev/MyCity/work/geometry-withdrawal-20261024/`, about 42.3MB logical files
at result collection within a 128MiB phase allowance. Keep them until a validated successor
and recovery-dependency audit permits exact-file retirement; do not delete the shared target,
unrelated WIP, or historical original recovery packages. Hashes do not replace complete data.

Native peaks, APFS allocation delta, all power-loss cutpoints, full-city/provider integration,
non-WALK/bridge/tunnel, GPU, formal M0 and real16GB remain unverified/outside this finite Issue.
Their parent/union Issues remain open. This phase does not change their loads or standards.
The unrelated 24 protected WIP files remain unchanged. No new worktree/timer was created for
the root repair. Other active Issue trees remain under their owners until safely archived.
