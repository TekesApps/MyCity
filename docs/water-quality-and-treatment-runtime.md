# Water quality and treatment runtime contract

Design revision1,2026-10-25. This extends [U03](product-utilities-environment-and-safety.md) and the existing single-writer/physical ledger contract; it does not replace budget or model rules. Issue [#92](https://github.com/TekesApps/MyCity/issues/92), upstream #85/#86/#96/#94, downstream #57. Baseline v0.5.185. All new implementation/acceptance below is pending unless its own evidence says otherwise.

## Scope, authority and unknown

Complete #92 includes finite water rights, original water stores, multiple quality networks, raw treatment, sewage and discharge. This vertical implementation starts from existing continuously produced process water and actual same-window generation. It must not substitute an easier initial qualified-water source. Existing v184/v185 unknown water remains unknown; an old save cannot acquire quality by loading new code. A new versioned opening must explicitly register original source composition and evidence; a later source observation needs its own original authority/receipt, never a guessed history.

Only original Stock/lot owns spendable amounts. Composition is a bounded side record of exact mL and distinct harmful/suspended/allowed-dissolved/other mQ, each Known or Unknown, with original provenance, coverage and use references. Unknown is never zero. Mixing merges all original references within a fixed bound or refuses the operation. Transfers/leaks carry volume and masses together; evaporation removes water, retains nonvolatile mass, and needs an actual receiving endpoint. Dry residue remains; zero water is never classified as compatible.

Classification distinguishes composition unknown, known outside the game chemical standard, meeting a named/versioned game chemical standard, and independently verified intended use. The last condition requires root's original permissions, receiver and use facts. Numeric concentration, quality ID or a digest is not a drinking/medical certificate; no real-world safety claim follows. Chemical compatibility does not prove #57 staff employment, arrival, actual inputs, work or return.

## Frozen purification and precision

Use U03's100m3 raw+0.1Q reagent+20kWh ->98m3 output+2m3 concentrate. Source harmful mass splits1%/99% output/concentrate; actual source suspended mass all moves to sludge. The sample0.01Q suspended/100m3 is a declared source amount, never created by processing or required as an exact universal concentration. Original allowed-dissolved mass follows water98/2; added reagent goes10% to output allowed-dissolved,20% concentrate,70% sludge. Final outlets take exact conservation remainder.

Unknown chemistry/undefined nonzero other components cannot be processed under this profile unless an explicit supported allocation and finite endpoints are supplied. Their unchanged source remains; refusal is not evidence of a real physical shortage. U03 sewage volumes and harmful95/5 are defined, but other sewage mass allocation is not; that path remains unsupported until separately versioned. No automatic migration or arbitrary average split.

Ordinary harmful threshold0.00002Q/m3 equals1mQ per50,000,000mL; reclaimed industrial/green0.0001Q/m3 equals1mQ per10,000,000mL. Compare exact mass and volume by checked cross products; do not round concentration first. Q is a game equivalent, not mg/L.20L consumes0.02mQ reagent: integer mQ truncation would create free treatment.

Precision v1 uses canonical reduced whole/rem/den and checked-u128 operations with cancellation; a proposed maximum denominator10^18 and whole<=i64::MAX is independent of the electric network limit. Validate supported configured endpoints before admission and each request again. Precision/overflow/work exhaustion returns unresolved/backpressure, not rounded material, zero or genuine shortage. Ordinary Stock units are integer projections: ceil-prewithdrawn original input is held as exact WIP; outputs credited only to their actual integer projection with residual fractions retained and counted against capacity. No ceil electric inventory credit. Costs are original cents; fractional mathematical entitlement is not a newly funded cent.

## Root bridge: actual process output and costs

The original continuous supply owner keeps gross production and gross cost, classified outflow and outflow cost; original Stock projects net=gross-outflow. Generic MoveStock on governed inputs/outputs remains forbidden. A controlled output pool's origin fields may describe its current net projection, but are not an external immutable input lot. Actual fuel/cooling/raw/reagent opening lots retain their original frozen identity and cost helper.

Consumption binds original port/run/segment and cumulative quantity, in a fixed deterministic port/production order. Generated-stream cost is C(b)-C(a) from a production curve determined before consumption; never current grossCost/grossProduced or unknown eventual production. Record where the start fee enters that curve, fractional prefix positions, every segment's physical inputs and its cost destination. Immediate and delayed consumption of the same produced prefix must agree. RootConfirmedStream cost observations are actual already-transferred integer cents with converted+WIP closure; root separately authenticates the curve and original debit. Pure module cannot self-sign them. Do not reset a Config each window or lose previous reagent WIP.

Both process ports104/105 are real original stocks, with one ultimate raw103 composition in the initial rig. A bounded list of these same-owner ports can be read jointly; it is not a third spendable water pool. Root decomposes takes deterministically, records actual original cost and source component movement, and commits source debit, treatment materialization and receiver/quality changes in one original Prepared. Other sources cannot be merged just because their quality IDs match.

After pump stop, later generator fuel/cooling cost belongs to actual treatment or explicit loss, never reprices already produced/transferred process water. The old fixed-mask run-duration invariant must remain for old profiles; the new profile records actual generator seconds, pump-mask segments and purifier conversion independently. New state and receipt/schema are explicitly versioned; no silent hot mutation of existing V1 roots.

## Joint power, devices and hard boundaries

Original generator24kW; two pumps6kW each produce72m3/h together. U03 treatment needs0.2kWh/m3, so with pumps running only12kW remains and at most60m3/h raw input can be processed. Source, pump and treatment constraints must be solved jointly before original materialization. Do not reuse electricity already assigned to Heat, spend future generation to start the loop, duplicate a meter or lower the flow simply to avoid fractional math.

The initial raw100m3 lasts5000s with both pumps. At that hard boundary, end/release pump obligations atomically, retain the generator's existing run/lease/fuel remainder/start fee and process backlog; mask0 is not synonymous with generator stopped. Purifier power after this point follows an explicit nominal/profile and allocation policy. The planned V2 bridge uses original available surplus up to U03 nominal1000m3/h(200kW):12kW while both pumps run, up to24kW after they stop, subject to actual material and finite receivers. This implies completion near5500s for the stated sample, not a hardcoded endpoint. Holding12kW instead would require a different explicit allocation cap and complete near6000s; the two policies cannot be silently interchanged. Derive every endpoint from actual events and preserve policy identity through recovery.

Earliest source/fuel/cooling/output/concentrate/sludge/permission event cuts the original Timeline boundary. No legal compatible residual endpoint means reduce or stop accepting, preserve all prior residues and obligations. Safe stop retains exact WIP, legal debts and original inputs/refunds. Fixed run/segment tables reserve representation for necessary terminal transitions; a full table may refuse new optional switching, never trap safe exit. Released resources only return through original actual lease retirement.

All electric input has a declared final heat/other sink; source native heat, pump heat and treatment heat cannot be double-counted. Root allocates original source input/start costs and actual personnel/residual costs once, using frozen per-purpose segments. A zero nominal fee is not proof of free actual labor. Missing employment, actual cost, occupancy or use qualification stays unknown/pending and cannot be filled with a boolean.

## Execution, persistence and budget

#92 owner may write only water_treatment.rs, its test source/small fixtures/handoff: bounded exact composition/transfer/purification proposals, revalidation, structure-only restore and source/precision tests. Root alone changes Opening/World/Stock/supply, device/joint power, resources/personnel/payment, Action/receipt/Store authentication, exports and headless scheduling. Pure candidates are not actual delivered water.

Activation uses original accepted Development intervals, source dirtiness, incoming/outgoing use and hard thresholds. No separate clock, dispatcher or per-drop actors. Inactivity never cancels mandatory storage, residual custody, legal deadlines or old work. Until approximation qualification exists, use one credible exact policy; reduce actual advancement or backpressure when it cannot fit.

Planning hard caps16KiB saved state and96KiB candidate workspace are reverse-design limits, not resource grants. Fix container/reference/component/candidate/run-segment bounds. Jointly count old/new states, original Stock buffers, serialization/restore and other staged domain state inside existing Engine4MiB, role8MiB and normal64MiB. The Mac32GiB prototype cannot accept real16GiB by extrapolation. No UI/render/GPU/AgentKit/long pressure test is authorized by this contract.

Raw restore is not an authenticated provider. Original opening registration and committed source/treatment/consumption receipts authenticate facts after save/load; preserve policy version, exact remainders, source/port cursor, gross/outflow costs and event/random identities. No unknown history is invented. Retain fixed actual inputs/programs/all failure/full recovery originals on the existing HP P900 target/work; no extra target or deletion of WIP. Stage artifact plan768MiB, preflight before execution, no measurement-time cleanup.

### Idle coverage policy1

Production requests continue to require strictly positive input. A separate `IdleFacts`/`IdleCandidate` path covers a continuous `[base.cursor,until)` interval only when root authenticates its entire original Timeline and accepted-work history. A currently stopped machine, nonzero receipt ID or checkpoint digest does not prove that history. The production verifier gains no idle authority: `verifies_idle` defaults to false.

An accepted idle candidate changes only the cursor and checked sequence; processed quantities, prefetched inputs, StreamBasis, every WIP and cost, obligations and resource occupancy remain unchanged, with a full zero Delta. Revalidation compares fresh facts and the original base digest and recomputes the complete candidate. It neither manufactures electric/cost receipts nor renews a source/use permission. Subsequent production must independently authenticate its true new window, e.g. production to3, idle3..1000, production1000..1001.

Structural restore may accept a later cursor and positive sequence with zero processed quantity only while the initial StreamBasis and all previous configuration, amount and time bounds hold. This is not historical authentication or `ready`; root still needs original idle receipts, including recovery and exact original-envelope retry. Split and merged idle intervals preserve material and costs, while their sequence counts may differ. No load-time cursor repair is permitted. The current component interface does not supply that root history verifier or Store persistence.

## Acceptance sequence and limits

1. Component independent oracle:100m3 mass/volume/reagent/power closure,20L fractional dose, exact harmful threshold, unknown/other rejection, mix/leak/evaporation, full finite sinks, precision budget, stale/tampered restore, equivalent split/merged prefixes and WIP/cost closure.
2. Root classified output: actual existing generated water, both ports and frozen production basis, immediate versus delayed export, no generic bypass, exact material/financial invariants and partial-prefix rollback.
3. Root joint generator/pumps/treatment: true12kW limitation while pumps run, actual raw hard stop and continued backlog treatment, real later-power policy, finite chemical/residual/heat constraints, original leases/fees/personnel facts, no future-start loop.
4. Original Store/OpeningInput/WAL/checkpoint and cold process recovery, exact receipt retry/no duplicate side effects; then original #101 due and #120 regressions serially. #57 only consumes water after original use, staff, occupancy and real work/return adapters are present.

Report each D/A/I/V layer independently. Component or finite root gates never close full #92. Finite rights/natural sources, complete multi-quality networks/stores, sewage/usage closure, actual authority and hardware/resource acceptance remain visible requirements.


## v0.5.186 actual component status

[Component evidence](evidence/water-treatment-component-20261025/STATUS.md):18 synthetic-RootVerifier release tests pass after original advisor source/result reviews. Root only exported the module. All actual classified output/cost curve, joint supply, permissions/personnel/residual facts, Stock/World/Store authentication and #57 use gates above remain pending; full #92 remains open. No inference from candidate IDs or sample memory estimates.
