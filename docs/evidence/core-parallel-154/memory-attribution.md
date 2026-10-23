# j3q/r07 physical peak attribution candidate

Status: bounded diagnostic integrated for focused verification; no new physical measurement. Final commands/results are in this stage STATUS.md.
This does not fix the observed peak and claims no memory savings. The original
`r07-sim-open-01` failed cold restore remains unchanged and fully retained.

## Exact observed frame

The authoritative run is `/Volumes/HP P900/dev/MyCity/work/r07-sim-open-01`,
source `aa552fd1f408a19147823fc84b1f8a31863c111792f2ab8b89078e69e108c9ad`.
`observer-report.json` names raw frame index 6 as the simultaneous footprint peak:
`t=1.533054667s`, complete capture, six process identities, no missing footprints,
group footprint `3,341,357,208 B`. The exact frame in `host-samples.jsonl` is:

| Role / PID | Physical footprint | Authoritative ownership evidence |
|---|---:|---|
| observer 98209 | 3,490,104 B | native observer report and source-bound R07 reading |
| supervisor 98210 | 1,622,328 B | native observer report and source-bound R07 reading |
| normal-pool-host 98211 | 47,432,016 B | `normal-pool/ready.json`, `events.jsonl`, and 44 MiB remaining touched pool pages |
| layout-session 98214 | 3,264,383,592 B | layout manifest, process identity, and R07 applied receipt |
| scale-endurance driver 98236 | 1,311,032 B | `common-events.jsonl` spawn lifecycle |
| scale-model-task 98237 | 23,118,136 B | pool event binds the model helper; `heaps.jsonl` records 20 MiB arena and 15,824,672 B Rust requested peak |

The identities are PID plus birth/start time in the original sample. At this frame,
the R07 applied receipt allowed `5,046,896 B` but reports `replaced_bytes=0`;
`observer-report.json` reports zero accepted memory credit. A later run maximum of
`8,060,928 B` replacement is not credit for this earlier peak.

## Byte reconciliation

The frozen whole-group target is `NORMAL_BYTES = 3,328,599,654 B`. The L1 process's
intended share is that target less the fixed 64 MiB external pool reservation:
`3,328,599,654 - 67,108,864 = 3,261,490,790 B`. Its measured footprint exceeds that
owned backing by `2,892,802 B`.

At the same frame, the pool-host footprint and model-helper footprint sum to
`47,432,016 + 23,118,136 = 70,550,152 B`, or `3,441,288 B` above the fixed 64 MiB
pool partition. That pool capacity is accounted once: pool events show 44 MiB of
remaining pool pages and one 20 MiB model workspace, totaling exactly 64 MiB. The
extra `3,441,288 B` is process footprint beyond those logical backing capacities;
the evidence does not split it into native heap, stack, loader, or other VM regions.

The three theoretically creditable role processes total `3,490,104 + 1,622,328 +
1,311,032 = 6,423,464 B`. Thus the raw excess is exactly `12,757,554 B`, decomposed
as `6,423,464 B` role-process footprint plus `2,892,802 B` layout-process overhead
and `3,441,288 B` pool/model-process overhead. The last `6,334,090 B` is an observed
accounting difference, not a confirmed leak or an identified reclaimable allocation.
No common 8 MiB allowance is added or subtracted.

The sample occurs during startup of the fixed full L1 layout and the active model
helper. It is not a Sim peak: no Sim process is present in the complete six-member
capture. The pressure pages, 64 MiB pool ownership, active model helper, and 3.1 GiB
normal gate are all part of the frozen workload. The evidence gives no safe basis to
remove any of them or reduce their capacities.

## Bounded attribution candidate

The source patch in `crates/city-headless/src/normal_pool.rs` appends the pool
service's actual Rust heap counters and optional role-arena counters to its existing
bounded `events.jsonl` rows. Each sample stays attached to the run's existing
`ready.json` PID/process-instance identity and event monotonic timestamp. Missing
role-arena activation serializes as `null`, not a fabricated zero. This can establish
whether the pool-host's Rust allocator contributes materially to its `1,294,672 B`
footprint above the measured 44 MiB pool pages; it cannot explain native stacks,
mapped pages, or loader/runtime regions and does not itself reduce memory.

The compact event key is `mem_v1`; the suffix identifies wire version 1. Its wire contract is:

- `heap_bytes = [current_requested, lifetime_peak_requested]`.
- `arena_bytes = null` when no role arena is active; otherwise
  `[grant, initialized, current_requested, lifetime_peak_requested,
  bootstrap_quarantined]`.
- `arena_failures = null` with a missing arena; otherwise the allocator failure
  count. The two `*_bytes` arrays are byte values; failures are a count.

The key carries the schema version, and the field names carry the units. The
existing max-row test exercises all values at their largest encodable values and
prints the observed encoded length and unchanged 1536-byte limit on failure. The
actual event-path test checks the pair/vector lengths and preserves null for absent
arena counters.

Resource contract: capture is a stack-only copy of existing atomic counters; no new
buffer, thread, process, map, or allocator is introduced. Every existing event row
remains subject to `EVENT_BYTES=1536`; the existing total event bound remains 8 MiB.
The maximal-row source test now asserts the added fields still fit and that an absent
arena remains null. The unmeasured real log growth is bounded by the unchanged row
and total caps; it must be reported after the root-owned run. The private author did not run Cargo; root verification is recorded separately in STATUS.md.

## Handoff and next oracle

Only `crates/city-headless/src/normal_pool.rs` and this report are new in this batch;
all pre-existing reader, R07, Sim diagnostic, and root WIP remain untouched. The exact
source diff is the uncommitted change in that one Rust file. Root should first review
that diff and run its focused existing event-bound tests in an approved Cargo window.
For an attribution run, pin a fresh source/binary, preserve the full original L1 and
business load, and sample this same startup phase. Join the new pool heap counters to
observer footprint by PID/birth and event time. If the pool's heap counters do not
account for the observed residual, the next diagnostic must identify native VM region
ownership for the layout/pool/model PIDs before proposing a reclamation. Do not use
requested stack size, Rust heap peak, or logical page counts as physical savings.

347 remains a failed original run. Its reader error was repaired in152 and the original
full snapshot cold-loaded; this does not turn347 into a pass. Normal/common8MiB and fullM0
remain unmet, while real16GiB and eight-hour gates remain not_run. No old snapshot was modified or removed.
