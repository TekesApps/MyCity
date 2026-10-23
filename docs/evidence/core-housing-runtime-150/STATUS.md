# Finite housing lease/recurrence and D/F/E persistent integration

Issues: [#26](https://github.com/TekesApps/MyCity/issues/26),
[#115](https://github.com/TekesApps/MyCity/issues/115), and construction compatibility under
[#2](https://github.com/TekesApps/MyCity/issues/2). Rust model/CLI only. This is a finite
stage, not completion of E, F, the full common lifecycle, or M0.

## Actual implementation and behavior

Trusted immutable genesis offers resolve household/account, unit, terms and source. The
compact HousingLease selector authenticates tenant and exact offer hash, consumes the offer
once, enforces legal occupancy/capacity/unique residence and rejects nonzero deposits.
No caller-supplied household/score or money becomes authority. Housing state, ordinary
Timeline Claim and sparse Rent source commit in the same Prepared transaction.

Gregorian rent recurrence uses existing HousingUnit.next_due_at, P1 accrual and P2 cash
settlement. It reuses one Claim/source, accumulates principal and paid, pays only outstanding
balance, and preserves oldest unpaid due. The Rent-specific cursor exception is bound to
unit/lease/parties/source and validated again on restore. No second clock or debt ledger.

The actual two-period test/CLI bills263226c initially, pays179200c from real payroll and
leaves84026c. Month2 principal743226c, cumulative paid358400c and arrears384826c retain
due day31. A second Store branch with100000c opening household cash pays the first period
fully, retains15974c, then pays195174c in month2; cumulative paid458400c, arrears284826c
and oldest due moves to day59. P1-before-P2 checkpoint/open and exact receipt replay do not
bill or collect twice. Precommit rollback, lost Applied reply, wrong tenant/hash and one-shot
offer rejection remain tested. Pagination width stays fixed for each persisted continuation;
last-hour single-step observation is a separate completed-target transition, not skipped work.

## Joint causal evidence

One Store combines capital contributions, payroll, real transport-backed sales/tax, authorized
outbound edge close/restore, lease and rent arrears. Days1/2/3 sales10/0/10 follow outbound
arrivals4/0/4; return arrivals stay4, so total served is8/4/8. Capacity is configured, not
invented from geometry. Three immutable profile versions and original proofs remain present.

Actual transport fee policy is0, with consumer utility surcharge1c/q. Household cash before
rent is200000-10000+1600-2*(1000+10)=189580c. Rent263226c pays189580c, leaves73646c.
Unresolved rent blocks dividend and exit; the fixture does not treat rent as market profit.
Final accounts [990130,0,809870,1200000] total3000000c. Cold reopen and original receipts
replay without repeating cash, offer consumption, profiles or debt. The independent
check_joint_checkpoint.py checks retained checkpoint, literal accounts and raw proof/profile
hashes; it is not a second WAL engine. Full originals remain in external runs328/330.

## Verification provenance

regression-summary.json accounts for525 different tests across321/322/323.321 had418pass/1fail;
the entire failed storage construction suite is excluded from the final count and replaced
by323 all4passed (the distinct core construction10 remains counted).322 passed106 across
25 remaining storage targets. Core and storage previously completed suites are not rerun
without a relevant change.321-to-final source deltas are recorded: only construction test
compatibility correction and pinned formatting of lib exports/CLI/housing tests. Final330
reran all7 affected construction/housing/joint tests;329 fmt and333 full workspace strict
passed.331 built the final CLI,332 actually ran it against a fresh full Store.

The v106 failure was comparing compressed HotJournalRecord JSON to the old full JournalRecord
hash. The corrected test still checks the original Engine identity and WorldView, loads
complete records from Store WAL, preserves the original fixed durable hash and checks every
hot record against the durable record. This does not weaken old history verification.
All failed attempts306-318/321/327, actual exes, exact source archives and full recovery
worlds remain external; commands.json exposes the later failure/verification sequence.

Final fixed source: 9c32380c22a2653856226cd847ce528384e07645856964c7353efb802a8942b9
Final CLI: 8940eb5590408ea5cbb58c9c8f21def0a5da5a74b6b5f333b1e92e5bf1f14713
See fixed-cli.json. Archive and binary binding is checked; example has no embedded source
identity field. Do not substitute older313/325 binaries as the final source proof.

## Computation/resource contract and limits

Detailed authoritative IO/events/stable intervals/batching/thresholds/ownership are in
../../core-housing-shared-runtime-handoff.md. Only monthly occurrences change rent; original
payroll/credit/phase order still executes. Boundaries, arrears and cash shortages cannot
be averaged away. The existing integer CPU Timeline is the reproducible reference path.
Scan is bounded by32 configured units; Claim stays48B, Action120B, sparse provenance stays
within the existing256-row ceiling. Actual scenarios configure1 unit;32-unit maximum load
and restore transient peak remain not_run.

One-unit opening owned7936B, mutable state72B, validation scratch budget2048B. Joint final330
Engine owned3616712B, Store41795484->41798948B (+3464B within this initialized fixture).
This is not an isolated total E increment. Native micro-run328 observed maxRSS56082432B and
peakfootprint50626920B; final CLI332 maxRSS48988160B/footprint44777856B. These external-SSD
32GiB Mac observations are not isolated original-scale gates. Box restore can overlap old/new
bounded housing allocations; backing reuse and whole-process transient peaks are unproved.

Move, EndLease, escrow/deposits, population relocation and residential access changes are
not implemented. Housing legal access is still an explicit genesis fact; road changes here
affect market paths, not dwelling access. Company rent profit/cost bridge is not implemented,
so the conservative distribution/exit gate remains. Real16GB, common8MiB, original normal
load and8h are not passed. M0 frozen quantities/thresholds and original failure evidence remain.

Only shared external HP P900 USB/APFS SSD target/work are used. All required recovery files
remain complete. Two precise archive coalescing receipts retain every original full path and
byte while reducing unique file allocation by57712640B (11 duplicate physical source archives);
observed filesystem free-space movements are recorded separately, not all attributed here.
Shared target is retained for the next M0/E validation.
