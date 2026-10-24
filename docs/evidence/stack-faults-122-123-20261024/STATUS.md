# #122/#123 short default-stack diagnosis — 2026-10-24

Current status: **both defects were repaired, verified and closed in v0.5.159**
(`1bee5b4c9ec49ebd41e0b555fb8bcbe7e8efaca5`); live Issue states were rechecked
on2026-10-25. The chronological failures and pending statements below describe
their original diagnostic stages. They are retained, not current blockers.
The later [qualification checkpoint defect #124](../qualification-authority-20261025/STATUS.md)
has its own evidence and v0.5.178 repair. None establishes universal stack safety
or changes the frozen M0 standards.

Both original exact cases still fail with SIGABRT on the unchanged default test stack.
Root ran only short serialized debug builds/exact tests on the existing HP P900 target/work.
No M0 load, assertion, stack allowance or production budget was changed.

Two initial marker rounds reached assemble → restore_empty → assemble_into → Opening
serde decode start but not decode completion. This is concrete localization, not proof that
WorldView is the root cause. #122 uses e0 without town, so a town-only fix is insufficient.

Original advisor approved testing a private OpeningDecode aggregate with boxed bounded
optional fields while preserving public Opening and original wire/semantics. Candidate01
compiled, but both unmarked exact tests still aborted. A marked #122 follow-up proved this
candidate does complete Opening decode, assemble_into, Engine construction and the equity
helper; it then aborts during the first EquityContribute execute. Thus reducing decode frames
makes real progress through initialization but is not a complete fix. No corresponding later
stage is yet proven for #123. #118 debug Store creation remains separately unresolved.

Diagnostic size_of (bytes): public Opening48,576; candidate decode header232; World74,184;
town profile43,968; region128; economy296; boot136; exchange2,032; construction1,664;
housing176. Boxing every present optional field requests48,400 transient bytes at most across
seven allocations, plus allocator overhead; absent fields allocate no box. Conversion overlaps
with final Opening and existing profile scratch. These are requested/layout counts, not native
peak or total-process measurements. Every Opening deserialize caller would need accounting.

The speculative candidate and every marker were reverted to exact complete pre-diagnostic
assembly.rs/transaction.rs/town_utility_engine.rs originals. Existing WIP was not reverted.
Both originals and candidate/marked versions, fixed executables, logs and manifests remain at
`/Volumes/HP P900/dev/MyCity/work/stack-faults-122-123-20261024/` with a256MiB planning cap.
Retain until root-cause resolution and a newer complete recovery baseline; no cleanup now.

Next: inspect first transaction execute frame/call chain, then validate a combined minimum
fix under both original exact cases, wire-equivalence/retained loader and Store restore tests.
No source-only or partial progress closes #122/#123. No PR/commit/release in this diagnostic.

## Candidate08/09 actual increment (not final acceptance)

Original advisor accepted a combined minimum repair: borrow TownMarket in prepare, private
small decode carriers for Opening/World/TownMarket with consuming finishes and original
Rust type names for exact derived diagnostics, and decode Opening before entering the
large assembly/validation frame. Runtime layout/fields/validation order/capital remain.

- #122 original clock case passed default debug; candidate04 made #123 actual shared-pool
  transaction/checkpoint case pass default debug.
- Old-derive wire mirrors passed2/2 in release;223 Opening variants plus non-initial World/
  TownMarket preserve paid wages, production remainder, tax cursors and distributed equity.
  The initial mirror failure exposed diagnostic type-name differences; private namespaces
  retain original names rather than normalizing error text. Failed tests remain recorded.
- Full core default-debug07 aborted after162 successful completions in an additional retained
  earnings case. Markers locate initialization; splitting decode from assembly then passed
  that unchanged exact case. All markers are removed.
- Full default-debug08 reached terminal206passed/1failed/1ignored with no stack overflow.
  Its only failure was a stale F fixture supplying dummy descriptor hash[9;32]. Root fixed
  only that test input to use its actual descriptor hash, preserving revision assertions and
  adding rejection of changed adapter basis with old witness; targeted09 passed. Original
  advisor accepted this fixture correction. Do not rewrite the original full-run failure.
- Current debug integration: retained OpeningInput6passed/14 historical diagnostics ignored;
  actual utility Engine9passed, including the original #123 case and checkpoint restoration.

Final frozen342-file snapshots and distinct binaries/logs are retained. Final optimized
wire/profile/Store cold-recovery regression is scheduled serially; no full issue closure yet.
Evidence cap is now512MiB for distinct failed and successful fixed executables, not a product
resource grant. No source tree, WIP, recovery original or old failed executable was removed.

## Final finite review, source09

Original advisor reviewed actual results and accepted the concrete #122/#123 stack, wire
and Store recovery gates with no remaining causal/authority/recovery blocker. Final serial
release results: core209passed/1ignored, profile4passed, Store6parents passed and12 actual
fresh child probes independently checked (119files293,175logical bytes). Default-debug
capacity read5/5 also passes; the original caught interrupt remains expected and preserved.
342 source files and release executable bytes were unchanged before/after execution.

The default full-run result remains206pass/1fail/1ignored; the corrected F fixture's separate
1pass completes the stated coverage, not a rewritten all-green command. No test assertion,
workload, default stack, runtime layout, authority gate or M0 limit was weakened. The correct
F fixture now binds the actual descriptor hash and also rejects a changed adapter basis.

Related issues receive actual implementation/result/limits/no-PR evidence. Defect validation
is complete within this finite scope; publication/commit/tag association remains in the next
coordinated integration stage to protect mixed WIP. No complete product/M0/16GB/physical
peak or universal all-input stack-safety claim follows. #118 remains a read-only scalar
projection, not an admission/physical-capacity provider.
