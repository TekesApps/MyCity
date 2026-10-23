# Bounded persistent Move/Wait integration — v0.5.155

Issues #26 and #115. Rust model/CLI only. This finite stage has actual Store, CLI, cold-recovery and strict-check evidence. No full E, F or M0 completion claim.

## Behavior and authority

A housing portfolio has one immutable city binding validated against Region.cities; changing
active city never changes the binding. Legacy absent bindings cold-open/bill/repay but cannot
initiate Move. Existing household/account identity, immutable offers, actual capacity, complete
paid-wage evidence, sourced assessment and protected current cash authorize a request. First
Wait persists EvidenceIncomplete without reserving capacity or moving funds. Retrying with its
operation ID revalidates the inputs. A successful Move atomically vacates old unit, occupies
new unit, consumes offer, pays moving provider, retains old earned rent debt and its original
source/Claim, and installs the new rent Claim. Deposits/refunds/nonzero travel time remain outside
this finite slice; environment and service scores are explicit fixture sources, not real providers.

The original dormant-city Payroll rule remains: switching active city freezes new autonomous
labor, not already earned wages. The actual day38 basis is266000 accrued,245000 paid,304h work.
Day42 pays the remaining21000 with no invented4-day work. Tenant cash56774+21000=77774 is then
explicitly transferred to the old landlord, bringing historical Claim paid263226→341000 and
leaving383226−341000=42226 owed. New Claim30000 is not due until day59. Cash total2885000 stays
constant. Wage observations never add cash a second time. Accepted cross-city fulfillment labor
is a separate existing obligation contract; this case does not assert that such labor freezes.

## Actual checks and retained failures

414 actual Store cases:3pass, unchanged source. Cases cover Wait checkpoint/cold-open, stale
Wait/city/caller rejection, Move lost Applied reply/cold-open/exact retry, historical debt
settlement lost reply/cold-open/retry, legacy city-less denial, and near-limit budget rejection.
415 broad core/storage regression actually passed540 tests in564.40s with unchanged source.
This is the pre-final-restore-repair source; the final affected validation still must run.

Failed400 compile,404 Wait cold-open World budget,407 historical daily P2,412 incorrect wage
oracle and413 historical explicit settlement remain retained with full original worlds where
created, source ZIPs, logs and actual executables.399 only checked unchanged154 after the path
guard rejected integration, and is not Move evidence. A complete original404 copy actually
cold-opened under409; the8 original files remain intact and an independently reconstructed
canonical business-state digest matches. Hashes do not replace the recovery originals.

404 exposed eager unrelated history backing plus omitted Box-header accounting. The correction
uses bounded lazy Vecs (each32) with bounded decoding and actual capacities, plus48-byte header.
A one-Wait state costs96 bytes. The same World128KiB owned+2048-byte scratch admission now runs
before successful business publication. Near-limit18-unit fixture:World128984+2048=131032 is
legal; adding96 becomes131128 and rejects by56 bytes with WorkingLimit, unchanged business
hash/cash/housing/timeline and a durable failed receipt preserved across immediate/cold retry.
The unchanged primary case still has2 units; no cap was raised or original M0 load reduced.

407 exposed active-unit validation incorrectly applied to historical claims during daily P2.
413 then exposed the same ordering error in explicit SettleClaim synchronization: the original
op1018 had a durable invalid_operation result. The narrow repair matches registered historical
source first, without altering current occupancy; active-unit lookup also matches lease ID.
The test now inspects the exact replayed business result, not just an IO/lost-reply condition.

## Limits and computation contract

[Housing computation contract](../../core-housing-shared-runtime-handoff.md) specifies authority,
triggers, stable intervals, exact thresholds, equivalent batching and resource ownership. No
second clock, debt ledger or persistence subsystem is added. Active/candidate/restore payloads
and native process peak are different measurements. Maximum32-household/full32-history use,
cold transient peak and original scale performance remain unverified. M0 normal/common8MiB/
original speeds/8h remain unmet; real16GB stays not_run on this32GiB Mac. Work/target use the
external HP P900 USB/APFS SSD.

Root coordinates all Cargo/measurements/releases and preserves unrelated WIP. F Scene geometry
activation and the housing-location→OD bridge remain explicit dependencies, not hidden completion.


## Newly isolated snapshot and offer-cap boundaries

The414 cold reopens replayed Move from an earlier checkpoint; they do not prove a checkpoint
made after Move can decode. The advisor identified `validate_housing_rent` still locating an
old unit before historical ownership, affecting vacant and re-let units alike. The private
repair validates exact historical source first and preserves active XOR historical ownership.
Final CLI now creates a post-Move/post-settlement checkpoint and cold-opens it;428 actually
passed, and the independent re-let case passed421 and426.

Existing consumed-offer capacity is a lifetime `unit_limit` cap: after2 successful accepts,
a2-unit prototype cannot accept a third offer even for a vacated unit. The re-let counterexample
therefore uses a separately labeled3-unit/3-offer legal fixture with a third real dwelling,
leaving the primary2-unit scenario unchanged. This does not prove unlimited reuse or2-unit
third-lease capability; that bounded offer-lifetime design remains an explicit product gap.
No table or resource cap is silently expanded to fit the test.


421 actually passed45 checks across7 Store/library targets, including the new re-let final
checkpoint/cold-open case and all4 Move cases. Actual re-let World ownership: opening119448,
after Wait/Move/re-let/old settlement128000; each+2048 stays below131072. Its tenant5 keeps30000,
new rent Claim15000 paid0/due59 and current housing unchanged while old debt drops by77774;
combined cash2915000 stays exact. This is the separately labeled3-unit prototype.

416 CLI compilation exposed an id-width typo (u64 helper versus u32 PayrollSpec.id), corrected
in417 without financial changes.420 new re-let test compilation exposed an unnecessary copied()
on an already-owned MoveWaitRecord, corrected for421.422 strict caught a derivable Default and
a test module before production items. The sole writer supplied equivalent derive/module-order
corrections;423 whole-workspace fmt passes. Final424 strict/425 core204/426 Move4/427 build actually pass with unchanged source. All original failed source/logs remain retained.


## Final executable and independent recovery

Final423 fmt and424 workspace/all-target clippy -D warnings pass.425 core204 and426 Move4
pass after the equivalent lint-only changes;427 preserves the actual CLI executable. Source
`0fb53229d3f00c7342b9e4387e08055c8ce1c3637209c953358944fb4150f2ec`,
CLI `d5cf0dc683a8f23ab8fc61fd82279a5ff32d868df716ab1ca9f633e2a0ce1c4c`.
Example has no embedded source identity: fixed-cli.json pins framed source archive and actual
binary, not an invented embedded check. Full commands/results/old executables are in commands.json.

428 ran that fixed CLI through42 days, Wait→Move→old-debt payment, final checkpoint and cold-open.
429 ran the same reader in a fresh process on a complete byte-verified clone: sequence1015 and
Store hash `c3b0e6e7482f21a492619557211517794db8ca6d2570cdc86f1f2dc6aef209d1` match,
0 model commands. The independent read-only check_checkpoint.py reconstructs canonical borrowed
World/Engine/Store business digest and checks literal wages/Claims/cash; it is not another WAL
engine.430 also opens the retained complete404 working copy with the final reader: sequence340,
original state hash `1b5befa9f0921f698ccb74196348829caf82b51f67c89d3720da643b84797066`.
Every file in both original recovery worlds remains unchanged. Originals and clones remain complete
under external work; public JSON receipts are not substitutes for those raw recovery files.

428 observed5.85s real,0.25s user/1.24s sys, RSS57262080B and peak footprint52969856B. These are
absolute instrumented microfixture observations, not clean full-load performance or isolated
incremental cost. Existing32GiB Mac and external SSD remain explicit. Engine ownership opening
3619544→3628096(+8552); World119376→127928(+8552), comprising8456 wage ring+96 Move runtime.
Store41170448→41179064(+8616); extra64 is observed, not silently attributed to Move. Fresh-process
clone Store41179069 differs5 more bytes; no saving or additional model cost inferred. Final
World+2048 scratch=129976, headroom1096. Opening immutable move metadata is already included:
4352 assessment+5120 candidate backing, not96 bytes for the whole housing feature. Optional Box,
actual capacities and candidates/restore overlap must be counted separately. Layout ceilings do
not guarantee simultaneous admission at all maxima; full32-household/32-history workload and
transient peak remain unmeasured. Fixed active capacity can reject a new obligation before its
nominal row ceiling, as the18-unit near-limit test demonstrates.

Advisor independently accepts only this finite stage, preserving the listed limitations. D's next
wage→actual spatial trade→tax/dividend/fiscal scenario and F Scene activation are still private,
not_run candidates; M0 short Model lifecycle diagnostic is also unintegrated/not_run and is not a
memory repair. Original M0 loads/failures/thresholds,8h and real16GB device gates remain open.

## Stage retention

After terminal399–430 rounds and lsof/dependency checks,20 byte-identical source ZIP paths were
coalesced by hardlink; every original archive path and complete bytes still exist. Selected
unique-file allocated space179904512→67371008B, reduction112533504B (107.32MiB). Observed APFS
available-space increase113168384B is recorded separately and not exclusively credited to us.
No recovery snapshot, log, executable, WIP, lockfile or shared target was deleted. This is a
selected archive accounting, not a total work inventory. Full round/reason/retained evidence
locations are in retention.json; existing stage-end policy remains validation-artifact-retention.md.


Post-stage read-only du observed all shared work112.65GiB and target4.86GiB on the external SSD;
this is not the obsolete25GiB work figure or all attributable to this batch. This batch's parent
construction-utilities-current-01 is about4.50GiB; larger retained M0/other rounds dominate the
rest. Their owner/recovery dependencies are under separate lightweight review; no old pressure
or snapshot is retired by name alone. Shared target remains in use for upcoming verification.
