# Issue #92 water treatment source handoff

Date: 2026-10-25. Issue: https://github.com/TekesApps/MyCity/issues/92.
Baseline assigned: main v0.5.185, `9720dad54a30f4f1cc6f3a76a674bac4a0e72c72`.
This is a bounded **source candidate**, not integrated/compiled/tested or accepted water supply.
Root is the sole shared writer and serial validation, publishing and Issue-report owner.

## Ownership and delivered files

Only these owner files were created; they did not exist at dispatch:

- `crates/city-core/src/water_treatment.rs`
- `crates/city-core/tests/water_treatment.rs`
- `tests/fixtures/water-treatment/raw-v1.json`
- this handoff

No shared lib export, Cargo file, Engine, World, assembly, Stock, Timeline, receipt,
Store, supply, headless, design/version/dispatch file, branch, worktree or target was changed
by this owner. Existing unrelated observer/M0 WIP is preserved. Concurrent root shared edits
are outside this owner's delivery. No commit/push/PR/tag/Issue mutation was performed.

Read sources: actual AGENTS.md, U03 in product-utilities-environment-and-safety.md,
physical-economy-runtime-contract.md, units.rs, utility_network/rational_v3.rs,
electric_dispatch/continuous_genset.rs and continuous_supply.rs; live #92/#85/#96/#94/#86.
The source #96 volume-only candidate does not certify water chemistry or source custody.

## Finite policy

RawPurificationV1 consumes 100m3 raw + 0.1Q reagent + 20kWh to produce 98m3 water +
2m3 concentrate. Existing harmful mass goes 1/99 to product/concentrate; **actual known
source suspended mass**, including zero/lower loads, all goes to sludge. No suspension
is created to fit the sample. Original allowed dissolved mass follows 98/2 water; new
reagent goes 10% product allowed dissolved, 20% concentrate, 70% sludge. Subtraction for
last outlets retains exact conservation. The sample source has 0.01Q suspended per
100m3, but that is not an eligibility requirement or invented universal concentration.

All four source composition components must be known. Nonzero other components are
unsupported because their allocation rule is undefined. Unknown/unsupported refuses
this proposal, keeps base state, and does not prove real physical shortage. Sewage is
explicitly unsupported until other-component allocations are frozen. No silent migration.

Ordinary game chemical harmful threshold is 1mQ/50,000,000mL; reclaimed industrial/green
is 1mQ/10,000,000mL. `judge` also requires known composition, no suspended/other mass,
and positive water. Allowed dissolved mass is tracked separately and cannot dilute harmful
mass in the denominator. Results are CompositionUnknown, KnownOutOfSpec or
MeetsGameChemicalStandard(versioned standard). These are synthetic game chemical
classifications. Actual use qualification/permits/arrival/consumer compatibility must be
separately authenticated by root; they are not health or drinking-water certificates.

General Composition split/mix/evaporate does not create spendable water. Homogeneous
split/leak moves water and all known masses together. Unknown stays Unknown on both
sides. Evaporation transfers only water and retains all nonvolatile masses, including at
zero volume. Root must apply both sides to real stocks/natural endpoints atomically.
LotQuality mixing preserves up to four distinct SourceRefs or returns ReferenceLimit.

## Root-facing API and exact units

`Exact::{integer,fraction,restore,parts,ceil,floor,add,sub,ratio,cmp_exact}`.
All amounts are field-typed mL, mWh, mQ or cents; no floating point. Canonical reduced
whole/rem/den, precision policy v1, whole <= i64::MAX, denominator <= 10^18. Multiplication,
cumulative conversion, cross-comparison and addition use cancellation and checked u128.
Precision/overflow means unsupported current calculation, not zero, rounded material or
physical shortage. Config admission preflights the maximum endpoint supported by its
frozen raw/reagent lots; individual fractional requests and cumulative endpoints are
validated again before returning a candidate. The denominator policy is independent of
and does not extend #85's electric precision contract.

`Config` binds version, precision, profile, Binding, raw LotQuality, original raw and reagent
InputLots, explicit RawBasisMode and three distinct original output/concentrate/sludge Stock IDs. InputLot binds
stock_id, lot_ref, original quantity_milli and original basis_cents. In
RootConfirmedStream mode raw/raw_origin describe the common original chemistry source
(e.g. raw103), never an extra consumable Stock. RawBasisMode explicitly names the
original process ports [104,105], maximum two, which cannot alias source/reagent/output
Stocks or each other. Binding includes
world/root epoch/source version/city/operation/origin cursor. The raw composition must
match that original lot's mL projection. There is one quality source lot, at most two original production ports, and one reagent lot;
there is no unbounded source list, arbitrary recipe or history.

`Config::open` creates a zero cursor checkpoint structurally; it is not an opening receipt.
`Config::prepare(verifier, &State, Request{until,input_ml}, &RootFacts, work)` returns one
Candidate or a non-mutating error. Request amounts can be fractional; at 20L, reagent
is exactly 1/50mQ. A source with all 72m3/h pumped cannot ignore the shared 24kW source:
after two 6kW pumps, root's 12kW treatment allocation limits raw input to 60m3/h. The
module does not choose a conveniently integral flow or reuse already dissipated Heat.

RootFacts requires fresh Binding/from/until/read_version, raw_stock_available_ml and
reagent_stock_available_mq **excluding already prefetched WIP**, exact available mWh,
finite output/concentrate mL and sludge mQ capacity, raw_stream_basis_after (None for
FrozenLot; cumulative root-confirmed StreamBasis for stream), original interval electricity/personnel/
residual cost facts, payer cash, cost and actual-supply references. `RootVerifier::verifies`
has no default implementation. Root must authenticate source chemistry coverage/age,
opening/last commit, actual same-Prepared interval supply, physical peaks, machine/person
occupation, permissions, endpoints and actual payment; nonzero IDs are not authentication.
Missing work returns WorkBudget without creating shortages, products or new obligations.

Candidate contains version, base_digest, copied fresh facts/request, next Checkpoint and
Delta. Delta exposes exact input/reagent/electricity, output compositions, new integer
Stock takes, original take-basis hints, exact converted-basis curve deltas and root interval
cost. `revalidate(verifier,base,candidate,freshFacts)` compares freshness, recomputes
candidate and rejects tampering/old cursor/root/source/epoch/operation. Root alone applies
Stock/side-quality/WIP/basis/cash/meters/Timeline/journal changes in one original Prepared.

## WIP, cost and restore boundaries

Checkpoint saves version/config digest/cursor/sequence/cumulative exact processed mL,
and optional root-confirmed cumulative StreamBasis.
`State::checkpoint`, `Config::restore`, `Config::meters` expose structural restoration and
deterministically reconstruct cumulative outputs and WIP. Integer Stock prefetch uses
**ceil of cumulative exact consumption minus previous ceil**, never per-call ceil.
Unconsumed prefetched raw retains its proportional source composition; reagent WIP
retains its exact remaining mQ. No WIP is available to a second consumer. Failure leaves
base state and all existing residual responsibility unchanged.

In FrozenLot mode, meters retain raw/reagent exact converted cost, exact unused WIP cost entitlement, and
integer original Stock held/take basis hints computed from the immutable original lot.
`converted exact + WIP exact` equals the **exact original-lot prefetched entitlement**;
integer original Stock basis is its floor. The fractional cent entitlement remains bound to
the source curve; it is not an unfunded extra asset. Root must independently recheck its
original FrozenLotDelta and recognize each whole-cent cost once. Module cost numbers
are hints/requirements, not wallet transactions, sale prices or proof of payment.

RootConfirmedStream is the actual growing process-water seam: root provides cumulative
StreamBasis{taken_cents,converted,wip} **after** the requested interval, authenticated by
RootVerifier against the original production-cost cumulative endpoints and deterministic
[104,105] take sequence. No current average, future final basis or raw103 cost curve prices
that stream. `converted + wip == integer(taken_cents)` must hold; no new raw take means
no new take basis, taken/converted cannot decrease, and zero remaining raw WIP water
requires zero WIP cost. These cumulative cost facts persist in Checkpoint through restore.
Delta uses cumulative cost differences, not per-query repricing. Source-curve fractional
cent rights still on original port lots remain root-owned and cannot become unfunded
extra WIP assets. Reagent remains the immutable FrozenLot curve in both modes.

The common source composition is homogeneous and fixed within one Config. Root verifies
both process ports actually share that source and compatible quality; incompatible sources
cannot be silently merged. `raw_origin.quantity_milli` is the finite original source-volume
bound in stream mode, not available future pumped output. Fresh actual port Stock and
prior WIP alone authorize a take; the module does not add the source quantity to them.
The root original cumulative gross/outflow/net bookkeeping, production-cost endpoint
curve, real source consumption and transaction rollback remain mandatory. No third pool.

Recreating Config each window must not reset reagent WIP, source composition, costs or
previous consumed quantity. Changing source, dynamic lot merge/replenish, stop/refund,
profile migration, arbitrary chemistry and multiple simultaneous treatment runs are not
implemented. Unconsumed fractional material cannot be rounded into a refund. Exact
cumulative product/concentrate/sludge meters retain output fractions; root materializes
integer endpoint differences and retains every remaining output mass/volume fraction in
its original accounting, rather than flooring each Delta and dropping the difference.

Restore validates canonical Exact values, versions, config binding, cursor/sequence and
finite cumulative lot bounds; malformed or out-of-range checkpoints fail. A forged but
structurally valid checkpoint is still possible. Restore alone does **not** authenticate
Store custody or original journal source. The original root journal opening/last-commit/
exactly-once gates must remain false until actual source authentication succeeds.

## Activation and resources

CPU only, one exact strategy, no RNG, GPU, UI, AgentKit, new scheduler or clock. Root calls
on original Development intervals and dirty source/quality/right/sink/payment boundaries.
Necessary receiving/disposal/legal consequences must remain active even if a view is
closed; work exhaustion/backpressure cannot discard water, residues or obligations.

Fixed domain bound: one original quality source + up to two real process ports + one
reagent lot, four chemistry mass fields plus
water, four quality side records (input plus product/concentrate/sludge), four source refs
per quality record, one candidate, one checkpoint, zero retained candidate history.
This module itself retains only its input quality record and checkpoint; root owns outputs.

Planning caps remain **16KiB persistent / 96KiB proposal+restore workspace**, not new grants
or measured performance. Conservative fixed-JSON envelope estimates in source are 12KiB
for Config+Checkpoint and 32KiB per candidate. Two candidate envelopes plus two saved
envelopes reserve 88KiB within that planning workspace for old/new/revalidation/restore
serialization overlap. The actual fixed struct layout, allocator overhead, serializer buffers,
root DTO copies, root receipt/Stock/WIP/basis state, stack and original shared total admission
remain to be measured. Constants do not enforce root's total budget. Original Engine4MiB,
role8MiB and normal64MiB cannot grow. Failure to admit must preserve original state and
residual obligations and reject new commitments. No unbounded IPC/cache/disk history is
introduced here. Existing root WAL/save history retention stays root-owned.

## Verification status and minimal future validation

**not_run:** cargo/rustc/rustfmt, executable scripts, all 18 test functions, benchmarks,
pressure/CPU/GPU/device/UI/performance, serde peak, actual Store restore, actual existing
pump source/shared electricity/personnel/payment/finite residual integration and #57 use.
Only source reading/editing and Git whitespace checks were performed: tracked
`git diff --check`, plus `git diff --no-index --check /dev/null <owner-file>` for the new
module, tests and handoff. No whitespace diagnostics were printed; no-index exit 1 means
new-file differences, not compilation or tests. The owner performed source review as well.
Fixture is a small independent synthetic arithmetic oracle, never actual source/permit proof.

Suggested smallest future root-only serial command after source/advisor review:

```sh
cargo test -p mycity-core --test water_treatment -- --test-threads=1
```

It reuses existing external-SSD shared target; no new target/worktree/branch. Then root must
validate real same-Prepared existing process-water classified consumption, fractional 12kW
prefix, unavailable reagent/electricity, full lawful sinks, rollback/retry, stale facts, exact
basis, Store source authentication and restore against original receipts. Passing standalone
arithmetic tests would not complete #92, sanitary qualification or actual service performance.
No PR exists for this owner delivery. Root supplies actual result/evidence/tag/PR if any to
Issue #92 and keeps unsupported/full scope open. Hashes are reported separately after
freeze so a self-referential handoff hash is not embedded here.


## Root actual component gate - v0.5.186

The writer not_run history above remains. Root froze436 build inputs, added only public module export, passed release workspace check/build and18 actual serial component tests under synthetic RootVerifier, with24 unrelated WIP files unchanged. Original advisor reviewed actual logs/results. [Evidence and limits](../evidence/water-treatment-component-20261025/STATUS.md). No real source/Stock/World/Timeline/Store adapter or use qualification is accepted by this gate. Full #92 remains open. Root must continue the existing generated-water path, not replace it with initial certified water.

## Follow-up owner source: explicit idle coverage seam (after v186)

Root reported publication of the limited component gate at v0.5.186,
`eb018559dae59f75d863110313022c3d597c2eae`, and #92 result comment 6390430767.
Those previous 18 component tests and the original owner's not_run history are retained;
they do not verify this new idle source or the actual growing-water bridge. Root preserved
v186 original source/tests/program/archive before requesting this bounded follow-up.
No new Issue, writer, worktree, target, release, PR or independent verifier was created.

The actual bridge needs to represent processing through second 3, no purification during
[3,1000), then actual supply during [1000,1001). It must not relabel that last second as
supply for [3,1001), accept a zero-input production prepare, or auto-fill a cursor on load.
Root's architecture scope is `work/water-treatment-20261025/NEXT-ROOT-BRIDGE.md`.
Global design, actual runtime, Envelope/receipts and Issue updates remain root-owned.

New API, with original positive-input production prepare unchanged:

- `IDLE_POLICY_VERSION = 1`, separate from chemistry/production policy.
- `IdleFacts { policy_version, binding, from, until, read_version, timeline_revision,
  idle_receipt_ref }`. All versions/references must be nonzero and binding must match;
  `from == base.cursor`, `until > from`. References are not authenticity by themselves.
- `RootVerifier::verifies_idle(&Config, &Checkpoint, &IdleFacts) -> bool` defaults to
  **false**. Existing production-only verifiers cannot silently authorize idle. Root must
  prove complete original Timeline history for the entire [from,until), no purification
  and no skipped already accepted work, plus source receipt/CAS/read-version identity.
  Current Stopped or an arbitrary NoWork result is insufficient authority. The independent
  idle proof requires no fabricated electricity or cost ticket.
- `Config::cover_idle(verifier, base, facts, work) -> IdleCandidate` clones base, changes
  only cursor and checked sequence, validates structure and returns zero Delta.
- `IdleCandidate { policy_version, base_digest, facts, next, delta }` is a separate type.
  `Config::revalidate_idle(verifier, base, candidate, fresh_facts) -> State` checks policy,
  CAS/base digest and exact fresh facts, rechecks historical authorization and recomputes
  the complete candidate. Old receipts, changed facts, altered cost/material delta,
  StreamBasis/processed/sequence tampering and a stale base fail without mutation.

All existing processed quantities, original Stock takes, fractional input/output masses,
reagent WIP, confirmed StreamBasis and every cost field remain unchanged by idle. Delta
includes zero electricity, root interval cost, take basis and converted basis, and zero
compositions on every outlet. Idle neither renews source or use permits nor releases
labor, disposal, money or legal obligations. Root's other original systems must retain any
such obligations during idle; no elapsed-time exemption is granted by this component.

Restore now permits positive sequence with zero processed **only** at a cursor later than
origin, within the original sequence/elapsed bound and with initial raw StreamBasis
(None for FrozenLot, zero cumulative basis for stream). Original config digest, precision,
finite source/reagent/cumulative material and stream basis validation remain. Structural
admission is not historical idle/Store proof: root must persist and authenticate original
Idle Envelope/receipt before readiness. No restore call moves the cursor or invents work.
A later production request still must use exactly that restored cursor as its `from`.

Six additional test functions were written (24 total source functions): initial idle and
restore in both modes; production to3/idle to1000/actual next interval; merged versus split
idle with nonzero fractional stream/reagent WIP and all cost meters unchanged; stale/
nonzero delta/StreamBasis tampering; complete-history/default-deny/accepted-work refusal;
policy/epoch/zero-work/zero-reference/terminal-u64 cursor bounds. The new idle verifier is
only a synthetic historical interval fixture. Equality of zero idle deltas and unchanged
Meters says nothing about arbitrary external production interval charges: original
production fixtures charge 31 cents per segment and do **not** prove fee partition
equivalence. No prior production test is relabelled as a fee-invariance test.

Resource shape stays fixed: one idle candidate uses fixed-size fields, one cloned
Checkpoint and zero Delta; no list/history, scheduler, new clock or growing allocation.
Idle revalidation/old-new state/serde/restore overlap remains inside root's unchanged total
admission obligation; 16/96KiB planning caps are not grants or new measured maxima.

**Current follow-up verification: not_run.** Owner performed source edits/read review and
`git diff --check` only; no cargo, rustc, fmt, test/executable script, benchmark, pressure,
GPU or actual Stock/Timeline/Store/fee/use verification was run. The small raw-v1 fixture
is unchanged. Only original module, tests and this handoff changed. Minimal future root-only
serial command remains `cargo test -p mycity-core --test water_treatment -- --test-threads=1`,
after advisor source review and root's own frozen-input/target/resource admission gate.

## Root actual idle component gate, v0.5.187

Original advisor source narrow review accepted; workspace release all-target check and24 release component tests passed serially under synthetic verifiers.436 source inputs/fixed executable/24 protected WIP unchanged. Six Idle tests were actually run; this is not original Timeline history/Store authentication, actual power or generated-stock/cost integration. Full #92 open, no PR. Root evidence: docs/evidence/water-treatment-idle-20261025/STATUS.md and work/water-treatment-20261025 source02/programs02/attempt02. Original18 gate and all originals retained.
