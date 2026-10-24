# Issue #101: continuous-supply required RunDue lane (v184)

## Frozen boundary

- Canonical checkout: `/Users/frankqin/dev/MyCity`
- Branch / base HEAD: `main` / `539f09f9c48e8010f38f9db341648624ecfc9cbc` (v184)
- Common Git directory: `/Users/frankqin/dev/MyCity/.git`
- This source contribution owns only `crates/city-headless/src/required_dispatch.rs`, `crates/city-headless/src/required_dispatch/supply.rs`, `crates/city-headless/tests/continuous_supply_due.rs`, `tests/fixtures/issue-101/continuous-supply-due-v184.json`, and this handoff. The canonical checkout contains other user/root WIP; shared Core, Store, Sim, Cargo, and root integration files remain outside this ownership.
- The pre-existing Background and service-needs selectors retain their behavior. Supply is a lower-priority lane reached only after both prior required lanes return true `NoWork`. `Busy`, invalid authenticated state, and errors stop dispatch instead of falling through.

## Selection contract

`required_dispatch/supply.rs` adds `SupplySelection::{NoWork, Busy, Selected}`, `SupplyPlanKind::{FundedContinuation, NextBoundaryAdmission}`, a supply-specific selection mode including `MandatoryZeroBoundary`, and the three agreed select/validate entrypoints. The selected token has private fields and revalidates committed sequence, current World/branch, every `Timeline::Progress` field, the full authenticated `DueView`, and the full persisted `Continuation` when present.

The selector reads `Engine::continuous_supply_due_state()` and `Store::continuation_status()`. It requires an authenticated profile and registration, exact source version 1, nonzero city/source/fingerprints, and the view's scope to match the actual Engine scope. The continuous-supply opening fixes this source to `Development { city_id }`; the authenticated registration/fingerprint bind the opening's clock and namespace. Genset `Stopped`, `Running`, and `Faulted` status is retained in the binding and never used to suppress an already accepted Timeline responsibility.

An active timeline phase with a valid original continuation returns `FundedContinuation` and carries that reservation's exact page width, target, phase state, and all continuation fields. The task key uses the real `[last_complete, at)` interval, authenticated source ID/version/cursor, actual scope, and Development clock. Only a nonzero interval is evaluated by `task_registry`; missing or invalid candidate state becomes `MandatoryFallback`, while a zero-width interval is selected with `key=None` and `MandatoryZeroBoundary`.

`NextBoundaryAdmission` is selected only when phase is `None`, the original Timeline still has a target strictly beyond `at`, and there is no reservation. It is explicitly an intent to try the original Store path with policy version 1 and page limit 32. It does not fund or create an obligation. The caller must submit one original Timeline `Advance`; the existing Store quote/checkpoint/admit-with-reserve path remains the only admission authority. A rejected quote or budget reservation must leave the original target available for the owner's normal recovery path. This handoff does not claim a budget-failure run.

Neither selection mode advances a clock, adds a separate worker, creates an ACK/receipt/ledger, retries a selected operation, or claims delivered supply. The root's Sim integration owns the C-namespace selected event, collision check, original-envelope execution, and lost-response recovery. After an uncertain result, it must recover the exact original receipt/outcome before issuing any later RunDue; it must not blindly retry a new operation.

## Authored checks; not run by this writer

The new `continuous_supply_due.rs` contains four integration tests using the actual `Store`, the accepted opening fixture, and the production selector/validator:

1. `accepted_supply_target_survives_all_genset_statuses_and_next_boundary_admits_originally` covers Running, Stopped, and Faulted profiles. It checks that a newly accepted target is selected in the actual phases observed by this fixture, that funded pages keep their persisted page width, that a missing hint remains mandatory fallback, that cold Store reopen recovers the exact prior receipt, and that a phase-less retained target yields the width-32 admission intent before a single original Store `Advance`.
2. `equal_sequence_scope_and_timeline_still_reject_a_different_authenticated_supply_binding` compares legal stopped and faulted Stores with equal committed sequence, scope, and complete Timeline progress. Their authenticated DueViews differ; a selection from one must fail validation against the other.
3. `sim_rundue_admits_the_next_boundary_once_and_recovers_the_original_c_receipt_without_consuming_terminal` first exercises a real funded continuation through `RunDue` and asserts its C-envelope preserves the stored width-1 reservation with one serial increment. After the original Store drains that same reservation to the next boundary, a Running source exercises `NextBoundaryAdmission` through a second real `RunDue`. The test closes its owned Sim input and waits for ordinary process exit without the test application consuming that terminal event; a cold Store reads and replays the exact C receipt without a second commit. `Client::stop` normally exits and reaps its owned child before the cold open; this does not simulate arbitrary power loss or network failure. The real raw-water boundary stops production before the accepted target, so the next Sim session issues bounded single-page RunDue commands until the original target completes. It checks each C-envelope and serial increment, retained target, final source ledger cursor/raw-water use/produced quantities/production seconds, and stopped genset state.
4. `sim_rundue_c_namespace_collision_preserves_the_original_target_for_manual_advance` occupies the exact next C operation with an actual original Store Advance. RunDue must fail closed before publishing a selection event, with sequence, state hash, progress, authenticated source view, and reservation unchanged. A subsequent ordinary Store Advance remains able to continue the same target.

P0 is permitted by the selector but is not asserted as an observed phase: the current legal Machine opening has no payroll workload, so the first committed Advance may already leave P0. The Store test records only phases actually seen and requires P1 through P5 for this fixture. The Running profile stops at its real raw-water hard boundary at second 5000; the Sim test verifies that RunDue preserves the accepted target across that boundary and continues the stopped source through second 7200, rather than treating the first boundary as completion.

All test cases require `MYCITY_ISSUE101_CONTINUOUS_SUPPLY_DUE_ROOT` to name an already existing retained directory below `/Volumes/HP P900/dev/MyCity/work`. Missing SSD/configuration fails; there is no temporary fallback and no cleanup path. Each case creates a unique child directory and retains its Store world, executable identity, selected event/envelope, and before/after proof. The fixture uses width 1 for Store phase-by-phase inspection and for the real funded Sim continuation. In this empty-payroll Machine opening, one boundary requires at most seven P0-P5 steps; a width-8 Advance can consume the whole boundary and leave no funded reservation to continue, so width 8 is explicitly not claimed as covered. Sim cases create the world with the original `Store::create_spatial` using the existing spatial input and terrain fixture; both spatial and supply openings declare seed 7. The retained spatial opening and terrain bytes preserve exact inputs. This `laboratory_only`, synthetic terrain exists only to satisfy Sim's original spatial-opening prerequisite; it grants no additional continuous-supply qualification. Sim rules identity remains `sim-w3:<MYCITY_SOURCE_SHA256>` to match `Profile::Base`; no shadow DTO or fake Store state is used.

No `cargo`, `rustc`, formatter, script, test, or executable validation was run in this writer session. `git diff --check` is the only source-level check requested before handoff. The root reported that its first workspace release check found E0308 (`DueView.version: u32` versus `TaskKey.source_version: u64`); the conversion was changed to `u64::from(due.version)`. The root's next release check/build passed and its first test run passed both Store-only cases, but the two Sim cases exited before RunDue with `sim_spatial_missing` at readiness. This writer changed only those Sim worlds to use `Store::create_spatial` with the existing seed-7 spatial input and matching terrain bytes, preserving the two Store-only cases. The spatial repair awaits root revalidation; no RunDue or post-fix test result is claimed. Resource peaks and product acceptance remain `not_run`.

## API read set

These hashes identify the actual dirty-worktree sources read for alignment; they are not a claim that all upstream WIP belongs to this change.

| Source | SHA-256 |
|---|---|
| `crates/city-core/src/continuous_supply.rs` | `ef326cfee09350ddc2935a95b963cd73d33bc36e256e76c4d6f9a2a8e5d02db0` |
| `crates/city-core/src/transaction.rs` | `6aa335940a44c472d853e7f9994136c94cceb81e412fcda76f953d62d428653c` |
| `crates/city-core/src/electric_dispatch/continuous_genset.rs` | `6abafe357451afcb50980e2391d579723a8941d8fc7a21d7d82271e6aff05e1b` |
| `crates/city-core/src/timeline.rs` | `a01e5f7b770dd39d07f525b3df48bf5b0be352542a9bea7003a1809008cf263d` |
| `crates/city-core/src/timeline_driver.rs` | `2258671c984e7171170025ecf7a98a2cbb02a23ea7744089a6dcd5770def5c05` |
| `crates/city-core/src/task_registry.rs` | `46416b1c6e7af291c2285c2ab70b2c9c74e819b9fd27221b2712ca93dcce724b` |
| `crates/city-storage/src/continuation.rs` | `5c583b06df08c000514a4a534e29d23912619f7333c515e6b14b9ddb8aeaa45b` |
| `crates/city-storage/src/lib.rs` | `e75ee504506104bb7a1cf2733308b7457984577d13ad0385da9844b65330a7e3` |
| `crates/city-storage/tests/continuous_supply_store.rs` | `6a4a187e2a5eac58b582eefc868d28e23ee818caf5ee6aff74b916fa44a3cdbe` |
| `tests/fixtures/continuous-supply/opening.rs` | `d024b1b6ecc3e4aed18ea73ddbf113c9200d1eaf6e023e036d149a7061bf93b5` |
| `fixtures/m0-exchange-spatial.json` | `1891b68e1190fac29e3dc83a1d5d96ba3fa972cc7594fb8106ccbe179cdd29d6` |
| `fixtures/terrain/m0-flat16.png` | `cfc5e674bb900da72162e4d7a9f6225a4497d8d31ed5f7ea09695ffac6a6fad8` |
| `crates/city-headless/src/task_registry_adapter.rs` | `d83fc8fad10df1416e24c3dcb4607686441d70eb11080d07058c8631b327d131` |
| `crates/city-headless/src/sim_host.rs` | `8bb5d9e88df5366841d27cfc01f6707ce0f161cfae2cac92aba56b3b1a181a8c` |
| `crates/city-headless/src/sim_client.rs` | `c1dd1ba0dcbff94ba724843620e14b4489567f832590ba2906bb2964972571cd` |

## Serialized release request

After the retained external directory exists and the root freezes the matching source/CLI read set, run only:

```sh
MYCITY_ISSUE101_CONTINUOUS_SUPPLY_DUE_ROOT="/Volumes/HP P900/dev/MyCity/work/continuous-supply-due-v184" CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -p mycity-headless --test continuous_supply_due -- --test-threads=1
```

This text reuses the existing shared target at `/Users/frankqin/dev/MyCity/target`; it does not request a new cache and was not run here. If the HP P900 path or existing target is unavailable, stop rather than falling back to a local work directory or a different cache. No formal CPU, stack, memory, parser, recovery, disk, or 16-GB joint budget was measured or is guaranteed. Existing M0 behavior and the separate physical/service-ledger boundaries remain unchanged.


## Root actual integration result - v0.5.185

Writer not_run above describes source handoff history. Root subsequently fixed source inputs, serially built and ran all four new tests, original required8/service3/cursor1, supply9 and Store gates. Original advisor reviewed the actual results. [Evidence](../evidence/continuous-supply-due-20261025/STATUS.md) distinguishes27 different release parents, one repeated default-debug OpeningInput parent and six fresh Store helpers; full issue remains open. Source01 type failure and source02 Sim spatial-precondition failure remain complete in persistent work. No production budget, stack or M0 relaxation.
