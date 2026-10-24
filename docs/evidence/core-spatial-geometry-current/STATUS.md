# Same-world geometry activation — work in progress

Canonical base v0.5.156, Rust/CLI only; not accepted/published. Root applied the exact
v155-resolved overlay df4a7e3c… preserving final housing Move/budget/historical rent changes.
No mixed private E whole-file rollback. The12 output files matched the handoff manifest.
The initial attempt to copy olda0bb test was correctly rejected because private author had
refactored it;443 therefore compiled only overlay's earlier test, not a0bb. The following
shell still ran Cargo after the copy guard failed; scope is explicitly recorded, with no
a0bb coverage claim. Root subsequently copied the frozen three support/test/CLI files in444.

443 actual check failed on request-row decode type and missing getter (source unchanged).
The sole core author supplied a narrow3-location patch.444 then reached storage and failed
on imports, changed constructor arguments, cross-crate grant visibility and optional witness
consumers. Complete source ZIPs and logs are retained; no Store or geometry-change behavior
is accepted. New Scene/profile/basis/descriptor/grant paths and resource inventory remain
uncompiled/unmeasured beyond those actual diagnostics.

F test/example owner later modified only the new CLI directly in canonical to add a
working-copy reader; this violated private-only handoff ownership. Root corrected the process,
confirmed444/445 had already ended and no source-frozen run was affected, and required the
owner to freeze the same cf0f9621… bytes in its private tree. All future canonical mutation
is root-only. This CLI mode is still not_run, not recovery proof.

Root runs only one Cargo task at a time using external HP P900 target. M0 diagnostic is being
built in a separate root-owned v156 worktree while F owner fixes concrete errors privately.
No formal M0 measurement/long rerun or 16GB pass is inferred.

## Actual integration attempts 468–484 (2026-10-24, not released)

468 workspace/all-target compilation passed. 469 release Store tests first ran:3passed/5failed. Root retained full failure source/executables/worlds. The geometry baseline oracle initially omitted BPR congestion: freeflow71→72 seconds becomes81→82 at flow/capacity1 with per-edge floor; actual freight6480→6560 cents (40q), unit162→164. Oracle corrected without changing roads or capacity. Geometry fixtures needed Region city1 registration; legacy no-region fixtures remain unchanged. Storage non-Apply candidate validation now checks exact durable Request/Approve/Veto/Withdraw effects while Apply still requires verified grant.

478 had an incorrect package name and ran no test.479 exposed stale shared-target core/map artifacts from another worktree, so no tests ran.480, after process checks, removed only MyCity core/map release package cache (Cargo estimates1691files/594.4MiB; no before/after allocated-space measurement and no attributed disk-free claim). Third-party target cache and archived actual executables remain.481 then compiled current source and found three missing local imports in the new validator; a narrow import patch fixed those.

482 ran8 Store tests:6passed/2failed. Old road/door Close/Restore, authorization/history and1025-commit hot/archive replay tests passed. New geometry Request failed because Engine only supplied prior_spatial_permissions when a permission candidate existed; geometry-only candidate also requires current permissions. A one-line CommitContext fix supplies existing active prior context without added state/allocation.

484 reran8 tests:6passed/2failed. New requests/approvals now proceed, but geometry Apply fails before injected Applied reply loss. Underlying error is under investigation; no complete geometry/CLI/fresh-reader/resource regression claim. Original failure worlds, logs, source ZIP and actual test executables remain under external `/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01/` matching round labels. No restore-original is replaced by a hash. F remains uncommitted WIP, preserved when D158 was fast-forward released.

## Takeover: 494 evidence and assertion-only correction (2026-10-24, not run)

The current canonical base is v0.5.158 (`ae28b8cda8041c7bd106a28cccdc3deab051b032`)
plus retained uncommitted F/E work. The previous coordinator has stopped; the current root
owns canonical integration and future serialized validation. No new execution window was
opened for this correction.

The retained `494-F-persistent-receipt-tests` record reports eight tests: six passed and two
failed. Both failures first reached the same shared fixture assertion at the former
`spatial_geometry_apply.rs:703`. That assertion expected an outer Store error for a changed
payload reusing a committed operation ID. The current Store replay contract instead returns
outer `Ok` with inner `TxError::OperationConflict` and no matching persistent receipt. The
fixture now checks that exact result, unchanged Store sequence/state, and unchanged original
receipt bytes. This is a test-contract correction, not a Store behavior change or a new pass.

Static review also found that the later stale-descriptor assertion expected a durable failed
model receipt, although the current Store rejects a descriptor with a changed active parent
before model execution. It now checks the exact outer
`Error::Invalid("spatial_geometry_descriptor_parent_mismatch")`, unchanged sequence/state,
`CURRENT.json` and Scene, and absence of a new receipt. Its diagnostic distinguishes Store
preflight rejection from model rejection. That later path was not reached by the two failed
494 tests and remains unexecuted after this correction.

The v154 private tree has eleven differing crate files, including older authorization-token,
independent revision, housing-history and budget-accounting paths. They were retained and
compared rather than copied over v158. Only the local replay assertion delta was reused;
the private `preview_profile_asset(&current)` change was rejected because the current API
requires `&VerifiedAllocationRef`, supplied by the canonical `&current.as_ref()`.

Complete before-originals, private differences and the narrow correction package are retained
under `/Volumes/HP P900/dev/MyCity/work/takeover-f-assertion-fix-20261024/`. The earlier 494
executables, source archives and failure worlds remain untouched. Compilation, tests, fmt,
CLI execution, recovery execution and resource validation for this correction are all
`not_run`; no commit, tag, release or F acceptance is implied. The original advisor's narrow
source review is recorded separately in that correction package.

## Unified validation after takeover (2026-10-24, not published)

The user authorized serialized execution on the existing external HP P900 target/work. The
fixed-source packages and complete run records are retained under
`/Volumes/HP P900/dev/MyCity/work/takeover-validation-20261024/`. Only the exact #54 candidate
source and its path-included test were overlaid into canonical for same-snapshot testing;
there is no education lib export, real provider or Store integration claim.

Three resumed F attempts each reported 7/8, with their source ZIPs, actual executables, logs
and full failure worlds preserved. They successively exposed the genesis structural-baseline
error after an authorized geometry, a remaining staging equality between asset/geometry
revision, and a newly added fixture's late Apply boundary. The last was a fixture error:
effective day 5 must commit at completed day 4. Production timing rules were retained. The
original advisor reviewed each narrow repair; its transient-parent ordering finding was also
fixed before execution, with final catalog validation retained.

The earlier source-only stale-error expectation above is superseded: the actual ordering
rejects closed access at `geometry_non_genesis_transport_state` before descriptor validation.
The fixture checks that exact preflight error, no new receipt, and unchanged state/sequence/
CURRENT/Scene. This is not a durable failed model transaction.

Final frozen source `v4-effective-boundary` passed:

- `04-F-original-eight`: all original eight test names, 8/8. The pending-geometry test now
  also covers geometry -> road/door Close/cold -> Restore -> second geometry -> Close/Restore
  and cold reopening, retaining the old proof bytes.
- `05-education`: #54 candidate tests, 14/14; this is synthetic module coverage only.
- `06-F-unit-counterexamples`: relevant storage units, 8/8, including structural/capacity
  mutation and genesis access-ceiling rejection.
- `08-recovery-summary.json`: three full-copy new-process cold opens passed; missing current
  catalog and altered referenced Scene were correctly rejected without fallback. Original
  worlds were unchanged. The old reader's `active_profile_sha256` field exposes genesis market
  configuration; it was explicitly excluded as active-profile evidence, with root/catalog/
  proof hashes checked independently instead.
- `09-E-regressions`: relocation/history/relet/budget 4/4, rent 2/2, paid-wage windows 4/4.
- `10-E-income-scenario` and `11-E-income-fresh-reader`: fixed CLI and a fresh process on a
  verified full copy passed. An independent integer-cent recurrence agrees with all accounts,
  the total 2,970,000 cents, low-income arrears 155,826 cents and retained old high-income debt
  30,000 cents. The scenario remains finite, uses supplied scores and does not prove housing OD.

All executed stages recorded unchanged frozen sources. The final advisor found no remaining
blocking repair and accepted only this functional/limited-recovery scope; the income scenario
and oracle completed after that advisor's evidence snapshot and were verified by the root.
This STATUS update is post-run documentation, not the archived executable source snapshot.

No all-power-loss-cutpoint, formal M0, native memory peak, performance, GPU or real-16GB pass
is claimed. The machine has 32 GiB; new bounded profile scratch in cold recovery has no native
peak evidence. M0 frozen workloads and limits remain unchanged. Builds retained an existing
unused-mut warning in the inspection example; no strict workspace lint pass is claimed.
No WIP, recovery originals, worktrees or shared target were removed; no commit/tag/release/PR.
