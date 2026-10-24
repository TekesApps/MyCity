# Issue #101: finite RunDue selection handoff

## Frozen source boundary

- Worktree: `/Users/frankqin/.codex/worktrees/issue101-due-v168/MyCity`
- Branch: `codex/issue101-due-v168`
- Base HEAD: `14d4526692b4326e5d4fc81b331cfc3935820b20`
- Common Git directory: `/Users/frankqin/dev/MyCity/.git`
- This stage wrote only `crates/city-headless/src/required_dispatch.rs`, `crates/city-headless/tests/task_registry_due.rs`, and this handoff. It adds no fixture. Tests consume the existing 30-second and 60-second empty-source inputs at `tests/fixtures/issue-101/air-empty-sources-30.json` and `tests/fixtures/issue-101/air-empty-sources.json` as exact input identity oracles plus existing `fixtures/m0-exchange-opening.json`, `fixtures/m0-exchange-spatial.json`, and `fixtures/terrain/m0-flat16.png`; the six `Resource` capacities match `crates/city-storage/tests/background.rs` (101–104 at 1, 201/202 at 2000). They do not dispatch freight, preserving the empty-source air input.
- `crates/city-headless/src/lib.rs`, `sim_host.rs`, `compute_worker.rs`, `task_registry_adapter.rs`, Store/Engine, Cargo files, and all other shared sources remain outside this writer's ownership. The root's current integration adds `pub mod required_dispatch`, `Command::RunDue`, the explicit `Worker::has_active_execution()` query, and the original required-path validation call. Those shared changes were read for API alignment, not edited here.

## Implemented API and contract

`required_dispatch.rs` exposes:

- `select_required(&Store, worker_active_execution) -> Result<Selection, DueError>` reads the current Store's real `Background::pending()`, `World::scope_revision()`, Timeline progress, committed sequence, and adapter observation. No pending job, including a disabled Background, returns `NoWork` without a write. An active worker execution returns `Busy`; the bool must come from the owner’s live `has_active_execution()`, never from persisted `Job.running` or `Worker::is_empty()`.
- `select_required_with_missing_hint(&Store, worker_active_execution)` is the explicit missing-hint/fallback path used to prove advisory absence cannot suppress an existing required job. The normal `select_required` path obtains its advisory observation from the real Store adapter. Candidate selection requires exact scope, immutable source key, full `ActualJobBinding`, candidate key, World clock, due time, necessary flag, expected `committed_sequence.checked_add(1)` token, and `snapshot_token_durable == false`. A missing or mismatching observation selects `MandatoryFallback`; advisory data cannot produce backpressure. The adapter's `Observation` remains private to this crate boundary rather than leaking a partially public DTO through the selector API.
- A pending job is accepted only with P1–P4, a valid positive Timeline interval, `job.start == progress.last_complete`, `job.end == progress.at`, and `job.input.target == job.end`. P0/P5/None or an inconsistent snapshot is rejected. The current single Background air job is World-scoped with no city; the adapter's binding carries real world/branch scope and the complete Job fields. No clock is created or converted.
- `Selection::{NoWork, Busy, Backpressure(RequiredBlock), Selected(SelectedRequired)}` and the fixed errors `DueError::{StoreUnavailable, MissingTimeline, InvalidPendingSnapshot, SelectionChanged}` are stable. `DueError::as_code()`/Display return `store_unavailable`, `missing_timeline`, `invalid_pending_snapshot`, or `selection_changed`; `RequiredBlock::as_code()` returns `attempt_limit_reached` or `invalid_input_failure`.
- `SelectedRequired` keeps the chosen job, scope, committed sequence, Timeline `at`, `last_complete`, phase, business key, and full actual attempt binding private. Root obtains only the actual job id through `job()`. `validate_required` rereads and compares the same Store snapshot immediately before the original RebindLease; stale scope/sequence/time/phase/job/key fails as `SelectionChanged`. It deliberately does not recompute or compare diagnostic mode, because a valid missing-hint fallback must remain valid even if a later advisory observation differs.
- Attempt limit and actual `InvalidInput` failure remain hard backpressure with the real pending obligation retained. No queue, durable cursor/token, worker, lease, receipt, ACK, auto-advance, clock, or budget controller is added. The existing required owner retains retirement, RebindLease, dispatch, candidate validation, Accept, and receipt authority.

## Tests authored; not run in this writer session

`task_registry_due.rs` covers a real Store with no Background (NoWork, unchanged committed sequence), a real P4 pending job (normal candidate and missing-hint fallback), the same production `validate_required` helper after (a) `Background::Restart`, which commits a new sequence while leaving every `Job` field identical, and (b) a real lease that changes the binding, plus an equal-sequence/equal-scope/equal-window pair whose full binding differs only by its real `Transient` versus `Cancelled` failure. It also covers an active-execution Busy input, attempt-limit and `InvalidInput` backpressure after real Background commands, and a persisted `running == true` job reopened through `Store::open` with no live owner; the latter must select as `MandatoryFallback` and validate against that same Store. These are real Store transactions and recovery reads, not a simulated live worker recovery protocol. The input fixture is parsed and its literal target, empty source set, and input identity are compared with the actual pending job.

The Sim cases use `Client::start_probe`, `Command::RunDue`, and the actual worker event protocol. They cover NoWork without a worker or sequence change; a P4 required run accepted without an Advance before acceptance; explicit bounded continuation of the original target 30 to `target == None`/`last_complete == 30`, then a second P4 window at [30,60] while an idle child remains; cancellation at `WorkerCells`, cold process reopen, and a new current ticket with a fresh attempt/generation/incarnation; and exactly three real cancelled attempts followed by RunDue attempt-limit backpressure with unchanged sequence, phase, pending job, and time and no fourth worker. The second window compares against the existing 60-second empty-source fixture. `InvalidInput` hard blocking is covered only through the real Store API test; there is no public Sim injection path and no fake invalid Sim state. Tickets' empty-source input hashes are compared with their matching fixture identities. Cancellation/reopen proves the replacement ticket differs from the canceled attempt; it does not inject an old candidate into Sim or claim that every stale-result rejection path was exercised. Each run requires `MYCITY_ISSUE101_DUE_ROOT` to name an already-existing dedicated subdirectory under the external HP P900 work volume; there is no temporary-directory fallback or evidence deletion. Each case has a unique directory, executable-path manifest, retained world, and JSONL transcript with 512-event, 256-KiB/event, and 8-MiB/transcript bounds.

No `cargo`, `rustc`, `fmt`, test, script, or executable verification was run by this worker. Only source reads, Git status/diff inspection, and `git diff --check` were used. All compile, test, real Sim execution, resource peak, and product acceptance remain `not_run` for this stage until the root's serialized release window.

## Canonical source read set

The canonical checkout was read with its uncommitted WIP at HEAD `14d4526692b4326e5d4fc81b331cfc3935820b20`; these hashes were read from `/Users/frankqin/dev/MyCity` during this stage and are not claims that the sources are part of the frozen worktree base:

| Source | SHA-256 |
|---|---|
| `crates/city-headless/src/sim_host.rs` | `b6e3905144d74180cb9c7f1000cd3849c078b8a4543d97281fdfc2feca5db69b` |
| `crates/city-headless/src/task_registry_adapter.rs` | `d83fc8fad10df1416e24c3dcb4607686441d70eb11080d07058c8631b327d131` |
| `crates/city-headless/src/compute_worker.rs` | `1ac20bd7d16cef1aabc4dc47fc3fe58618ddfda87b0e170db4c7614ae3a90156` |
| `crates/city-headless/src/sim_client.rs` | `523e0fbe7bbf9d5b2795efbf55f68225123e288213966422256128544c39cc29` |
| `crates/city-storage/src/lib.rs` | `ef8ffa0417969f935a6652c358099912af723b13799c7a9c3112c76323f087ea` |
| `crates/city-core/src/background.rs` | `57e19b584f5196cbbfdd68f20fb32dd9280c8f6ade473379a2b2fe49832d9d4d` |

## Root integration seam

Keep the root's shared integration minimal: `RunDue` reads `select_required(store, worker.has_active_execution())`; `NoWork` returns `{dispatch:"required_registry_v1",state:"no_work",required_preserved:true}`; `Busy` returns the existing busy error; `Backpressure` uses `RequiredBlock::as_code()`. For `Selected`, pass its actual `job()` and the private selection context only through the existing `required` call. Keep the existing receipt replay and job-id/attempt/InvalidInput gates. After retiring/recovering the old worker and immediately before the existing `RebindLease`, call `validate_required(store, selected, worker.has_active_execution())` once; after that consume the context and continue the original path. Do not make advisory mode a gate, do not infer activity from `Job.running`, and do not make NoWork a statement that no future obligation can arise.

Root's reported result envelope for completion is `dispatch:"required_registry_v1"`, `dispatch_state:"completed"`, `selection_mode:"registry_selected"|"mandatory_fallback"`, and `selected_job`; the original `replayed` and receipt record remain authoritative. Keep `has_active_execution` as `outstanding.is_some() || requires_session_exit()` so an idle retained Child does not block the next window while the existing required path still retires it.

## Serialized release request and limits

After the test root below already exists on the mounted HP P900, the exact focused request is:

```sh
MYCITY_ISSUE101_DUE_ROOT="/Volumes/HP P900/dev/MyCity/work/issue101-due-v168-rootrun" CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -p mycity-headless --test task_registry_due -- --test-threads=1
```

The designated `CARGO_TARGET_DIR` is the existing shared `/Users/frankqin/dev/MyCity/target` symlink, not a new target. If the external volume, existing run-root directory, or target is unavailable, stop; do not substitute a local cache. The command is documentation only and was not run here. The root coordinates it with the locked source read set and fixed CLI executable before execution.

Proposed test bounds are one serialized test process, at most 512 Sim events per case, 256 KiB per event, and 8 MiB per retained event transcript. These are source limits only. CPU, peak memory, parser cost, checkpoint recovery, disk use, and suitability for any shared workload are unknown and unmeasured. The test covers the one existing Background air worker path; it does not grant worker capacity, multi-model scheduling, #102/#103/#104 budgets, durable selection tokens, stale-candidate injection into Sim, missing-clock behavior for a valid pending job, full Store recovery, or 16-GiB product capacity. Keep worlds and transcripts until the root records hashes/retention decisions and confirms no recovery or audit depends on them; do not clean them as part of this source-only stage.
