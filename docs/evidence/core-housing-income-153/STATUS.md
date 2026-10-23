# Actual-paid-wage observation and canonical integration

Issues: #26 (housing) and #115 (same persistent world). Rust model/CLI only.
This finite stage does not implement Move/Wait, all household income or full M0.

## Authority and behavior

The existing HousingRuntime owns an optional fixed32-day ring for at most32 registered
households. It observes actual positive Timeline Payroll.paid changes in the same Prepared
transaction as payroll and cash. Automatic payment, explicit SettlePayroll, termination and
exit settlements use the same hook. It adds no balance, debt ledger, clock or daily event queue.

A completed midnight boundary queries31 complete days `[end-31*DAY,end)`. A midnight payment
belongs to the new day. Missing historical coverage is unavailable; instrumented zero is
valid zero. No qualifying contract, incomplete coverage and billed wage arrears are distinct
from complete observations. A stopped contract can retain actual historical receipts without
being an active future contract. Accrued-but-unbilled wages do not become unpaid wage arrears.
Legacy absent windows stay unobserved; restore does not fabricate lifetime payment dates.

The actual fixed CLI reaches day31: window[0,31),0c Complete (midnight payment excluded),
then day32: window[1,32),198400c Complete, one qualifying/active contract. The actual account
cash is opening200000+received198400=398400c; employer600000-198400=401600c. Total3000000c
is conserved. It checkpoints and cold opens with identical view and Engine state hash.
The income-only CLI does not accept a lease or claim Move behavior. The separate existing
joint test still exercises actual rent/transport/trade/equity in one Store under this source.

## Actual verification and provenance

Private income-only source:349 four component tests;363 four real Store tests;367 core/storage
libraries and integration targets533pass;368 workspace strict. Failed348/350 compile attempts
and357(3pass/1fail) remain recorded.357 terminated a contract at the window start, making it
unqualified; the corrected test retains that counterexample and separately uses StopWork to
prove qualified zero. Source and complete retained worlds are not replaced by hashes.

Canonical exact8-file integration preserved the152 reader fix.369 fmt,370 all5 Store tests,
371 core201/headless102/map5,372 strict and373 build passed with unchanged source.374 ran
the preserved fixed CLI (4.33s wall; app3521ms), actual cold recovery passed. The tests include
daily-vs-batched windows, lost Applied reply/exact retry, pending P2 checkpoint/cold open,
explicit and termination payments, no-contract vs observed zero. Executables and full worlds
are retained under external work/construction-utilities-current-01; commands.json pins paths.

Final source e43d8ef264617f3ce3a297e30bc00af36e55d6357a12db0dcb1a820a20ac403c;
CLI6b25632a4700b2b24b064aa679047c424a6026080fb2394606287f53253c5701.
Example binary has no embedded source field: ZIP framing/archive hashes and actual executable
are pinned by fixed-cli.json, not an invented embedded identity claim. Read-only literal
checkpoint and historical profile oracles pass; these are not a second WAL replay engine.
The first independent oracle omitted opening cash; oracle-correction.json preserves that
failed expectation and its correction. Production/CLI code was unchanged by that correction.

## Computation and resource contract

Authoritative inputs are Payroll.paid deltas at their internal event timestamp, immutable
household/account registration and the existing Timeline cursor. Output is a derived completed
window/status read model. Only actual payments mutate buckets; coverage is initialized once.
Stable intervals with no payment need no per-household daily work. Same-day actual payments
may sum with checked integer arithmetic, but crossing midnight requires original event times;
reference daily/large Advance paths must agree. Overflow, incomplete target/P2 and unavailable
coverage fail closed; neither promised wage nor a current partial day is extrapolated.

Single ring8456B (8+32*264), optional pointer8B. Active/candidate copies require up to16912B
ring backing; restoration can add another8456B during overlap, not measured as physical peak.
No record count, original pool, common8MiB or normal limit is enlarged. Actual374 enable
Engine3609920->3618376B and Store41160831->41169287B both +8456B. Cold Store41169351B includes
an extra64B not attributed to the ring. Native micro RSS48365568B, peak footprint44401000B.
These are observed absolute micro values, not isolated production peaks or memory savings.
32-household maximum workload and cold transient peak remain not_run. Work and target are
external HP P900 USB/APFS SSD on32GiB Mac hardware; real16GB remains not_run.

This input is actual wages received, not tax-net income, all income or future affordability.
Move consumption, old rent claim/history, city binding, wait, refunds and escrow remain queued
for the sole shared writer. Full normal/common8MiB/original speeds/8h gates remain unmet.
No UI/GPU/voice/AgentKit is introduced. Issue scope remains open beyond this finite stage.

## Stage retention

After all validation processes exited, exact363-374 source ZIP handles/dependencies were
checked. Full byte-identical archives were coalesced by hardlink, preserving every full path
and complete contents. Unique selected-file allocation fell65617920->10936320B, a54681600B
reduction; observed disk availability increased54681600B but is not exclusively attributed
to this task. This is not a full work inventory. Recovery originals, evidence, executable files,
source/WIP and shared target remain for subsequent validation; nothing needed for direct
recovery was deleted. Exact round/reason/retained paths are in retention.json.
