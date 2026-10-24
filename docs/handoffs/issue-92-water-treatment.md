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
