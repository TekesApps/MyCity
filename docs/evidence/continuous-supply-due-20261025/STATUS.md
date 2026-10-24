# Continuous supply RunDue and OpeningInput recovery gate

v0.5.185; actual base v0.5.184 `539f09f9c48e8010f38f9db341648624ecfc9cbc`.

27 different release parent tests pass, plus one additional default-debug execution of the same OpeningInput parent; six independently launched Store recovery children exit0 without timeout/wait error. Original advisor reviewed source, narrow repairs and actual results with no remaining finite gate blocker. This does not complete full #101/#120/#85/#86/#57 or M0.

| Gate | Actual release parents |
| --- | ---: |
| Existing service cursor unit | 1 |
| New original supply selector / real Sim | 4 |
| Existing required dispatch | 8 |
| Existing service needs due | 3 |
| Original supply / raw restore guard | 9 |
| Original raw Store supply recovery | 1 |
| New OpeningInput same-slot supply publication/recovery | 1 |

The six fresh Store recovery children are four release (two old raw Store, two new OpeningInput) and two default-debug new OpeningInput. Ignored helper functions are actually invoked by their parent; do not add them again to the distinct parent count. Sim Client exit followed by a parent Store reopen is separate proof, not an additional fresh Store helper process.

## Actual behavior

Original Background true NoWork -> service true NoWork -> authenticated continuous supply. Busy/error/unknown/backpressure in an earlier lane does not fall through. An accepted target remains necessary for Running, Stopped and Faulted profiles. The fixed legal Machine input exposes P1-P5; P0 is accepted by code but not observed as a persisted test phase.

FundedContinuation binds every original continuation field and preserves its width1. NextBoundaryAdmission binds the existing target/sequence/complete Progress/authenticated DueView with no reservation and phase=None; policy v1 width32 is only an admission intention. Original Store quote/checkpoint/admit-with-reserve remains sole funding authority. Admission failure is not NoWork or genuine business shortage; an injected budget-rejection scenario was not exercised here.

A zero-width current observation uses key=None and explicit reason, not a fabricated one-second TaskKey. For next admission this describes the completed current boundary, not the future Advance interval. One RunDue emits the exact C Envelope then submits at most one original Advance. A C collision fails before selection and preserves original target/reservation. After an unread terminal response, Client::stop closes input and waits for its owned process; cold Store finds and replays that exact original receipt without an additional page. This is not an arbitrary power-loss or network-fault experiment.

Running supply actually exhausts its registered raw-water input at second5000, stops/releases through the original path and retains the accepted7200 target until completion. Existing Stock/cash/fractions/costs are checked; no source status erases the time responsibility. No sanitary-water or actual service-delivery claim is added.

OpeningInput original supplied bytes are initialized into the caller EngineWorkspace initial slot and published to Store with all original slot/backing identities compared. Actual Account(1) Start has an original receipt; one-second uncheckpointed WAL prefix is opened by a fresh child and checkpointed, then another fresh child reads that checkpoint. Advance to3, Stop(3), checkpoint and parent reopen preserve the exact state and authentication. The final Stop reopen is not claimed as another independent process.

## Failures retained and repaired

Source01 workspace check failed on u32 source version versus u64 TaskKey; original writer repaired only the lossless widening, original advisor accepted. Source02 compiled; two Store tests passed but two Sim tests failed before RunDue. A fixed CLI on a complete copied failed world diagnosed exit2 `sim_spatial_missing`. Original writer added only the existing matching seed7/world1/branch1 spatial JSON and original synthetic terrain through Store::create_spatial for the two Sim worlds. Production preconditions, two Store-only paths and M0 standards were retained. Source03 then passed all stated checks. Complete old sources, fixed programs, failure worlds/logs and diagnosis copy remain.

## Identity, limits and retention

433 actual build inputs, fixed parent/CLI programs and24 unrelated WIP files were hashed before/after execution. Original target/work resolve to HP P900 USB/APFS. Actual machine is Mac16,1/32GiB, Rust1.85.1; this is not real16GiB acceptance. Original normal64MiB/role8MiB/Engine4MiB/default stacks and M0 frozen gates stay unchanged. The due digest checks original owned+96KiB scratch before computation; no new ledger/clock/queue/grant manager.

Full persistent originals: `/Volumes/HP P900/dev/MyCity/work/continuous-supply-due-20261025/`. Retain within planned768MiB; keep all source attempts, fixed programs, complete worlds, receipts and failure evidence until the dependent integration and recovery gates are released by root. No new tree, target or automation; no cleanup of unrelated WIP. No long stress, GPU, native peak, sustainable speed or population ceiling measured. The two historical stack faults remain separately tracked; this small default-debug pass does not close them.

[Results](results.json), [debug](debug-result.json), [children](recovery-children.json), [source identity](source-manifest.json), [summary](summary.json).
