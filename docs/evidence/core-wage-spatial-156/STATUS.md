# Wage, spatial trade and shareholder/fiscal joint scenario — v0.5.156

Issues #10 and #115. This adds three scenario/helper/test files over155; no production
Engine/World/Timeline/Store change. Rust model/CLI only. Existing actual transactions, stock
costs, map-derived transport proof, tax cursor and shareholder register remain authoritative.

## Actual behavior

Both worlds have zero opening household cash, the same fixed OD, eight served people per
service day, the same original1600c seller payroll, and real government100000c contribution.
The high case adds actual480c wages from a separately funded5000c employer. Actual sales
are[7,3] versus[10,10], with household day1 balances93 versus30. Final control cash590;
high2080−2020+77 personal dividend=137. Shares6003 personal+1000 government=7003; personal
opening ownership is explicitly endowed, not historical proof of a real opening purchase.

Sale price100c, production basis91c, buyer utility1c. Control day1 margin63/tax31; day2
27−800 idle wages=−773, yielding retained−741. It rejects an actual1-cent dividend with
InvalidOperation; balances/register/business hash are unchanged but a failed receipt advances
sequence. The exact rejected result survives cold retry. High has90c margin/45c tax per day,
retained90, actual payout77 personal+13 government. Government then actually spends13c to the
supplier. Fractional dividend remainder uses the existing stable largest-remainder rule.

Final cash is exactly2805000c in each world. Canonical account IDs1..5: control
[900051,590,699359,1200000,5000]; high[900130,137,700200,1200013,4520]. No income is credited
a second time from an observation. Original paid wages, trusted costs and tax claims govern
the same existing transaction candidate; no new clock, ledger or persistence layer.

## Actual verification and originals

436fmt,437 workspace/all-target strict clippy,438 five checks across four Store/library
targets,439 example build,440 fixed executable CLI and441/442 independent-process working-copy
readers pass with unchanged framed source. `commands.json` records exact invocations, source
archives and child exit codes. `fixed-cli.json` pins source5c1ddbdd… / executable0cda6575…;
the example has no embedded source check, so source archive plus build record is the binding.
Both complete440 worlds and all failed432 worlds remain on external work.441/442 full-copy
manifests cover every file before fresh Store open; zero model commands executed, originals
unchanged. Final sequence79/81 and state hashes match440. A hash never substitutes for a
recoverable original. The final independent checkpoint oracle is reported separately.

431 compile exposed import/id-width mistakes.432 runtime exposed a mistaken financial oracle
that omitted77c personal dividend; actual core cash137 was correct.433 compilation caught an
incomplete helper type correction. Original source ZIPs, logs, actual executables and worlds
remain retained;434 then passed before resource/negative additions,435 passed those additions.
Final436–442 supersede those source snapshots, without erasing failures.

## Computation contract and resource boundary

Authority: immutable Opening/profile/OD plus existing accounts, original Payroll, actual
TownSettle lines, inventory basis, sourced Tax claims, shareholder register and persisted receipts.
Triggers: game-day boundary, actual wage payment, explicit contribution/dividend/fiscal transfer.
Stable intervals have no extra wage/market/equity update. Equivalent batching may combine
validated sums inside one candidate, retaining per-day rounding, FIFO stock-basis slicing,
shareholder remainder order and debt protection. Affordability, partial stock, daily positive
tax, idle labor, protected cash and zero/negative retained are exact thresholds; crossing them
cannot be approximated by multiplying a multi-day average. The preserved reference path is
this event/day transaction sequence. No per-person/per-device simulation or high-frequency log.

No production resident structure is added. In440 both Engine owned3608544B and World owned
101576B remain unchanged from opening to final. Store41787258→41790722B control and
41787262→41790726B high, each+3464B. Different working-copy path strings change Store accounting
by a few bytes; these observations are not wholly attributable model cost. Actual micro CLI
2.23s real/0.33s user/0.15s sys, RSS55361536B, peak physical footprint50250088B from time-l.
Model-owned capacities, full Store backing and native peak are different measurements; this
is not full-scale p95, fast-forward, or cold transient peak acceptance.

Mac16,1 has32GiB; work/target use external HP P900 USB/APFS SSD. Original normal/common8MiB/
four-speed/8h M0 requirements remain unmet; physical16GB remains not_run. Economic balance,
ongoing raw procurement and general public-service provider costs remain outside this fixture.
OD is unchanged[[4,4],[4,4],[4,4]], not a housing-location→trip-origin bridge. F same-world
geometry activation and that later bridge remain explicit dependencies. No full A–F claim.

## Retention

Checked terminal431–442 source archives for open handles; byte-identical immutable copies now
share hardlinks while every path keeps full content. Unique file-allocated bytes68173824→
34062336, reduction34111488B(32.53125MiB). Volume free changed34054144B during observation,
reported separately and not wholly attributed to this task/APFS. No recovery world, snapshot,
executable, source, lockfile or WIP was removed. Current work total was not rescanned here;
the last155 observation was112.65GiB and is dated, not a current-size assertion. Continue
shared-target reuse and inspect handles/dependencies before each finite-stage cleanup.

Root then actually ran the copied independent oracle8740ca0a… with exit0: it reconstructed
v7 Engine and Store business hashes from both final checkpoint payloads, verified actual
cash/inventory, sourced tax claims/register/dividend and all three raw transport proofs,
then matched441/442 fresh-reader receipts. `checkpoint-oracle.stdout.json` preserves the
result and original checkpoint/proof hashes. It executes zero model commands and is not
a substitute for the already executed Store recovery. The two cases in fact have identical
raw proof bytes; this reinforces the fixed-OD limitation.
