# Issue 102 exact-pages test preparation

## Scope

Prepared `crates/city-headless/tests/task_execution_pages.rs` against the Issue 102 protocol contract and the root implementation now visible in `task_execution_protocol.rs` and `sim_host.rs`. The existing shared core files and existing integration tests were left untouched. The workspace contains unrelated pre-existing evidence and documentation changes; they were preserved.

## Test cases prepared

- The common bound-task entry selects `execute_exact_pages`, independently recomputes the accepted-execution digest from the typed request encoding, pauses at a real Air page boundary, observes and proves absence of a receipt, continues with a changed quota, and compares the final candidate with an ordinary `RunRequired` candidate from a deterministic equivalent opening. The normal-role grant is checked while paused and after process retirement.
- Cancellation leaves the original Air obligation pending, checkpoint/cold restore preserves that obligation, and a new explicit exact-pages binding resumes it. Cancellation itself is asserted to return an error rather than a fulfillment result.
- Capability version 1, plan version, quota bounds, and a non-Air execution request are rejected; stale binding is exercised after an intervening timeline advance. The tests compare business state around rejection.

Evidence directories use `MYCITY_TASK_REPORT_ROOT`; normal-pool files use the separate `MYCITY_TASK_PROTOCOL_POOL_ROOT`. The executable is supplied by `MYCITY_TASK_REPORT_EXE`. Test helpers retain event JSON under the evidence directory.

## Validation and limits

No test, build, formatter, or executable was run, as assigned. All validation is `not_run`. This is prepared test source, not evidence that it compiles or passes. The non-Air case changes the requested execution to `sim_timeline` while the current selected route is Air, to exercise the `task_exact_pages_requires_air` input guard; a separate integration fixture with a naturally selected non-Air route is not included. Root should compile/run this test after the shared implementation and frozen executable are ready, then resolve any API or fixture adjustments without weakening the assertions.

No PR was created; report and link implementation/verification status to the existing Issue 102 from the coordinating task.

## Root takeover before first build

The worker source was frozen and copied to `work/bound-air-pages-20261028/worker-originals/`. Root corrected the boundary loop so each new boundary receives an explicit Continue, using the original large quota after a smaller second turn. The stale case obtains real account references from a checkpoint copy. The first run showed that a pending P4 correctly records the transfer as Timeline busy; the corrected test expects that original refusal, then verifies that its Store sequence invalidates the old binding without a transfer or another business effect. Root also added a naturally selected zero-width Sim/Timeline exact-pages refusal to the existing task_execution_protocol test. No executed results are implied by this preparation note; current execution evidence is maintained by root separately.
