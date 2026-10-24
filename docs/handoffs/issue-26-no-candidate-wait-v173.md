# Issue #26 — no eligible housing candidate: Store Wait source handoff

Date: 2026-10-24 (Asia/Shanghai). Status: source candidate; author validation **not_run**.

Owner scope: this dedicated checkout only, branch `codex/issue26-wait-store-v173`,
fixed HEAD `3f48b3243eafb495faffd7a39ec9e73d4a65e935` (v173), common repository
`/Users/frankqin/dev/MyCity/.git`. No shared implementation, existing test/fixture,
Cargo, lock, dispatch or version file changed. No commit, push, tag, PR, Issue write,
publication, target access, process termination or build/test/fmt/rustc/script run.
Root owns adoption, serial execution, Issue result and closure judgment.

## Delivered source

`crates/city-storage/tests/housing_no_candidate_wait_store.rs` contains exactly two
new tests. Necessary fixture constructors and Store/Timeline helpers were copied
from `housing_move_store_runtime.rs` at the fixed HEAD; none of its four tests is
included or counted as new evidence. The existing `fixtures/m0-region-opening.json`
is read through `include_bytes!`; there is no extra fixture file. The local fixture
retains its two units, source identities, offer hashes, original day-14 lease,
day-38 destination lease, original score intervals, version 1 and bounded limits.
The unrelated region routes/obligations are omitted at initial fixture assembly,
as in the reference; this is not a transit/product acceptance claim.

1. `unreachable_candidate_wait_survives_cold_open_and_original_token_retry`:
   set the candidate's `reachable=false` **before** Store creation. At day 37 a
   complete real paid-wage window precedes `NoReachableCandidate`. Checkpoint,
   close and open the same world, replay original envelope and reject missing CAS;
   advance Timeline to day 38 and reevaluate with the recovered original Wait
   token. The immutable unreachable candidate must remain Wait with a new CAS
   token. No dynamic reachability update is claimed.
2. `not_yet_eligible_candidate_wait_moves_with_original_token_when_lease_start_arrives`:
   keep the reference candidate unchanged. Day 37 has complete wages but its
   immutable lease starts at day 38. Current `plan_move` requires exact equality
   between lease start and effective time; with no qualifying candidate its broad
   rejection reason is `CandidateNotBetter`. After cold recovery, legal Timeline
   advancement alone reaches day 38; reevaluation uses the recovered original
   Wait operation ID, same assessment/version/household/city and original offer.
   It should move once. This is a time-eligibility case, not score improvement or
   an update of the immutable source catalog.

No `EvidenceIncomplete` Wait is accepted by either test. The original request
operation ID is replayed intact; fresh reevaluation uses a new request operation
ID with the original Wait CAS as required by the actual API, not a replacement
household/assessment/Opening. Both cases compare `genesis.json` bytes across cold
opens. No direct mutation of Engine/World/Timeline authoritative state is used.

## API/source facts and expected oracles (not execution evidence)

- `transaction::prepare_housing_move` resolves immutable Opening assessment,
  candidates and offers, checks the household/city/prior Wait CAS and complete
  Timeline boundary, then uses actual paid-wage history before calling `plan_move`.
- `housing::plan_move` checks `candidate.terms.starts_at == effective_at`; if all
  candidates are unreachable it returns `NoReachableCandidate`, otherwise no
  qualifying plan returns `CandidateNotBetter`. This admits the day-37/day-38
  transition without a new API or authority edit. It does not establish general
  mutable housing supply or scoring support.
- `housing_runtime_move::MoveWaitRecord` preserves operation ID, household, city,
  assessment ID/version, observed time and reason. Both full records and bounded
  one-row Wait cardinality are asserted. Timeline serialization preserves cursor,
  payroll and Claims across Wait and first cold open; engine/world hashes cover
  exact post-Wait and final-state recovery/replay.
- `Store::execute` returns the **current Store committed sequence** for replay,
  while preserving the original model receipt. The tests separately assert new
  Wait/failed-operation sequence increments, failed-CAS model hash stability,
  original receipt equality and final sequence/hash stability across all three
  original envelopes after the second checkpoint/cold open. A new successful
  Wait is expected to change model hash; it is not falsely treated as a no-op.
- Literal cash oracle at days 37/38 before Move: total 2,885,000; treasury
  1,000,000; tenant 66,774; employer/provider 355,000; landlord 1,463,226.
  Five real weekly payments total 245,000, January invoice is
  ceil(480,000 × 17 / 31) = 263,226. Day-37 window is [6,37).
- After Move only the 10,000 moving fee transfers: tenant 56,774,
  employer/provider 365,000; total and landlord unchanged. Old Claim ID 9,001,
  payer 2/payee 4, due day 31, paid 263,226, original trusted lease/unit source
  remain; principal becomes 383,226, retaining exactly 120,000 arrears. New Claim
  principal 30,000, paid zero, due day 59. Exactly one household occupancy moves
  from unit 101/lease 9,001 to unit 102/lease 9,002; old unit clears its lease and
  arrears reference, one historical lease retains old debt identity. Wait keeps
  original occupancy, lease and fully paid January Claim unchanged.

## Coverage and remaining unknowns

All of the above describes authored assertions and read source, **not a passed
test**. Compilation, fmt, actual branch outcomes, checkpoint/cold recovery,
receipt replay, budget behavior and runtime/disk costs remain unverified here.
Existing v159 actual Move/rent/wage/cold evidence supplied by root is neither
rerun nor newly certified by this package. Neither test proves dynamic source
publication, general reachability changes, score updates, repeated re-letting,
product acceptance, performance, GPU/device acceptance or the complete Issue #26.
No shared implementation/API change was necessary for this narrow proposed
time-eligibility transition. The unreachable fixture has no legal mutable
reachability API demonstrated; it therefore asserts continued Wait only.

A separate advisor completed one read-only source review against this fixed HEAD:
no concrete blockers were found in the time-eligibility path, literal oracles,
receipt/current-sequence handling, authority boundary or obvious API types.
This is static review only. The coordinating root's original advisor/adoption gate
is still root-owned; this review does not claim to replace a required review by a
particular pre-existing advisor. After review, only explicit retry sequence and
continued-Wait Timeline-equality assertions were added.

## Budget, retention and root-only future commands

Source package is approximately 30 KiB, below the 16 MiB author cap. No runtime
artifacts were created and no target/cache was occupied. Each future test requires
`MYCITY_TEST_ARTIFACT_ROOT` to point to an existing absolute directory on mounted
`/Volumes/HP P900`; exclusive `create_dir` gives each world a distinct directory.
It prints the preserved path before Store creation and never removes artifacts,
including on failure. Root must inspect free space and bound/monitor the common
128 MiB stage evidence budget before running; the test does not claim to enforce
a combined disk quota. Maximum input is the original two-unit fixture and two
worlds, with two checkpoints each. No load or frozen limit is reduced.

After root's normal mount/free-space and serial-target checks, the minimal command
from the adopted v173-based source checkout (not run by author) is:

```sh
MYCITY_TEST_ARTIFACT_ROOT='/Volumes/HP P900/dev/MyCity/work/issue26-no-candidate-wait-v173' CARGO_TARGET_DIR='/Users/frankqin/dev/MyCity/target' cargo test -p mycity-storage --test housing_no_candidate_wait_store -- --test-threads=1 --nocapture
```

Root creates the exclusive evidence stage directory first and captures full
command/output/exit code externally within its stage budget. A root-side source
format check can target this new file alone before adoption; do not format shared
WIP. Do not rerun the reference test target as purported new coverage. Stop on
budget/semantic failure and retain the complete failed world and logs.

This checkout remains a recovery dependency until root has archived the untracked
source, handoff and full diff and completed adoption/review. It must not be removed
before that. After durable adoption/archive, with no validation, handle, process
or recovery dependency remaining, root may retire this standard Git worktree
while retaining the branch and original evidence. No cleanup was attempted by
the author because the only source copy is still untracked in this tree.

## Fixed dependency SHA-256

| Read-only dependency | SHA-256 |
| --- | --- |
| `crates/city-storage/tests/housing_move_store_runtime.rs` | `a4372afc1590cb9cb41b556b07f6c033d8725bb36c0631a79e80040e49fd385c` |
| `crates/city-core/src/housing.rs` | `5f5ebceb061da37a6c290ce9c8d41e13256bdceebb8df07f07edd80e3599c446` |
| `crates/city-core/src/transaction.rs` | `65ee9d63526595da9f4105b9deea0c1b714149f2e26d57fc8cd597ddb698810c` |
| `crates/city-core/src/housing_runtime_move.rs` | `a2cad6b1a9d9a3bf116e7f939fccf9fa9691823e6d997395698dc1e3780aab93` |
| `crates/city-core/src/housing_income.rs` | `b9260cca5ced96e6b26f24b207f57b1c4a4479386d37bc84ba127595aba22faf` |
| `crates/city-storage/src/lib.rs` | `ef8ffa0417969f935a6652c358099912af723b13799c7a9c3112c76323f087ea` |
| `fixtures/m0-region-opening.json` | `b654f042638d2efd3b058b87590a1c0446dcec2112faa99f10f6eefafed91544` |

Final new-file hashes and complete untracked-file diffs are supplied in the final
handoff after review; self-hashing this document is intentionally avoided.
