# #102 persistent Client/Model owner — finite integration

v0.5.161 associates the production caller migration above v0.5.160. Complete #102 remains open.

## Implemented contract

Client is a persistent slot constructed before its fallible startup. All production Sim,
scale/steady/rare/speed/endurance/probe callers and Model/helper startup retain the same owner
through errors, panic and cold reopen. Legacy fallible Client/Supervisor factories and
construct-next/replace paths were removed. Child is retained before Bind; actual wait is cached
before original-capability retirement. Original Pool outlives scoped cleanup. Model reuses the
same StartupOwner; no new pool/scheduler or role credit is introduced.

First definite Register rejection is distinct from a possibly committed request. Only an
unsent or definite unallocated rejection can be discarded; a previous unknown attempt still
requires resolving and retiring that original responsibility. Repeated wait after successful
retirement remains valid; a new or invalid startup cannot infer wait from an old PID.

Outer CLI scopes catch errors/panic, attempt finite cleanup twice and preserve the original
result after successful cleanup. If original retirement remains unconfirmed, they emit a
bounded structured unresolved-owner receipt and exit2 while retaining owner state until exit.
The receipt reports actual Child/wait metadata, retired=false and new_start_forbidden=true;
this is terminal failure, never an acknowledgement, refund or successful recovery. OS pipe/
reader error cutpoints are controlled tests, not proof of actual OS allocation exhaustion.

## Fixed-source execution

SOURCE `d3ef78aaae6b978b9fdb95d04cb77e00f880862be8e065dc2d392421b6af6fd4`,347 files,
complete archived read set unchanged before/after execution. Three repository-root registry
fixtures outside build.rs SOURCE are archived separately. Fixed release binaries and their
SHA/commands/logs/worlds are retained; they are not relabeled as postcommit rebuilds.

All-targets check passed. Twelve serial focused commands passed76 tests: protocol3, owner API/
layout3, RPC/persistent-worker9, broker23, Sim6, role3, Model/helper8, registry dispatch2,
profiles2, joint1, compute10, pool6. The broker ignored helper is explicitly invoked by its
parent test; one native mapped-observer test was filtered and remains unrun this stage. The
layout record is not another business invariant. This is not a complete workspace suite.

Two additional real parent CLI commands passed: normal-pool-model-diagnostic(full Reference)
and sim-probe(eight finite scenarios). The Model parent traversed run_retry with its original
Pool, actual Child/wait and workspace retirement before Pool.finish; its report has one
workspace grant/return and full64MiB restored. It is a lifecycle diagnostic, not a performance
or native-memory result. Registry tests use the compiled default CLI path, checked byte-equal
to the retained fixed executable before/after; no concurrent build occurred.

The focused test artifacts contain46 balanced pool reports, one deliberately unbalanced
supervisor-SIGKILL report retaining6MiB without a returned event, and six ready roots with no
report from intentional dead/fatal cases. None of those seven cases counts as recovered.
The additional real Model diagnostic has a separate balanced report. The terminal-scope test
records actual wait=true, retirement unconfirmed and exit2 after deliberately killing its
known original Pool. Original advisor accepted the finite source/results with no new blocker.

## Limits and retained evidence

Real r03 task/report identity and measurements, versioned output-capability mapping, #103/#104
coordination, full issue closure, true16GB/native/GPU/long-stress and original M0 remain pending.
No budgets, workload, stacks or frozen criteria changed. Other long-time caller entry points
were compile-checked and migrated but not each run end-to-end. Missing measurements stay Unknown;
v1 output limits cannot reject otherwise-valid existing wire results.

Full originals, intermediate failed checks, source/executables, serial commands, raw artifacts,
results and advisor record are retained under:
`/Volumes/HP P900/dev/MyCity/work/issue102-client-slot-20261024/`.
Actual test roots are p102c9-13791 and p102-entry10 in the same work directory. Existing shared
HP P900 USB/APFS target/work, Mac16,1/32GiB;512MiB planning cap is not a measured memory budget.
All24 unrelated observer/clock/rare WIP files retain their previous exact hashes. No originals,
worktrees or recovery payloads were deleted; private #102 tree is still needed for r03.
No PR: direct root integration with separate commit/tag receipt. Full issue remains open.
