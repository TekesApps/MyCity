# Issue #101: service-needs P5 continuation handoff

## Frozen source boundary

- Canonical checkout: `/Users/frankqin/dev/MyCity`
- Branch / base HEAD: `main` / `424e05f874e49dd494184d7dab085dd53e258d1e` (v182)
- Common Git directory: `/Users/frankqin/dev/MyCity/.git`
- The canonical checkout contains other uncommitted WIP. This stage wrote only `crates/city-headless/src/required_dispatch.rs`, `crates/city-headless/tests/service_needs_due.rs`, `tests/fixtures/issue-101/service-needs-p5-due-v182.json`, and this handoff. No shared Store/Engine/Sim/Cargo/root module files were edited.
- The prior Background `required_dispatch` API and its selection/validation body remain in place. This stage appends service continuation types and adds four stable `DueError` codes. The older source snapshot supplied for comparison was `/tmp/mycity-issue101-service-due-20261025/initial/required_dispatch.rs`, SHA-256 `96f4a2307f185a9786199bdcf315ea29aac790c0a3bfe189dc2e51bf769a1df0`.

## Service continuation API and boundary

`required_dispatch.rs` adds `ServiceSelection::{NoWork, Busy, Selected(SelectedServiceContinuation)}`, `select_service_continuation`, `select_service_continuation_with_missing_hint`, and `validate_service_continuation(store, selected, worker_active_execution)`. The selected snapshot fields are private; the owner receives only `target()`, `page_limit()`, `key()`, `mode()`, `at()`, and `sequence()`. The selection is advisory about a real already-funded Store continuation. It never creates a future boundary, advances a clock, creates a worker, or makes a service-delivery claim.

The selector reads `Engine::service_needs_due_state()` and `Store::continuation_status()`. It recognizes due work only within P5: a pending service run, coverage behind the current P5 `at`, or `next_due == at`. P1–P4 `next_due == at` remains normal future work. An invalid authenticated view, invalid P5 snapshot, missing continuation, or exhausted reservation is an error; none becomes `NoWork`. No service view, or a validated view with no current P5 due, returns `NoWork`.

The task identity uses the actual World/branch and `city_id: Some(city_id)`, `ClockKind::Development { city_id }`, stable source ID `0x534e_0000_0000_0001`, source schema version `1`, and the real interval `[Timeline.progress.last_complete, Timeline.progress.at]`. Version 1 is the task-registration schema marker, not the service model version. Cursor `0` means the run has not started; started cursors encode `1 + row * 2 + subphase`, with Deadlines before Occurrences. A source-only unit test asserts `None=0`, Deadlines row zero `=1`, and Occurrences row zero `=2`. This is only a pure encoding check: with at most 32 households and a 128-household page cap, the current real Store fixture goes directly from no run to Occurrences row zero; it does not establish persisted Deadlines row zero. The clock input is the original Timeline boundary supplied by the owner; this module creates or converts no clock.

Normal selection evaluates the actual due source and its necessary consumer demand. An exact necessary candidate yields `RegistrySelected`; a missing/invalid candidate takes `MandatoryFallback` while the same Store obligation remains selected. The explicit missing-hint entrypoint exercises that fallback. Before the original `Advance`, validation rereads and compares committed sequence, scope, every `Timeline::Progress` field, the full authenticated DueView, the complete persisted continuation reservation, and task key. A changed snapshot returns `selection_changed`.

`DueError::as_code()` and `Display` preserve the old codes and add `invalid_due_view`, `missing_continuation`, `continuation_exhausted`, and `invalid_service_snapshot`. Root integration should use `select_required` first and call this service selector only for its `NoWork` branch. For `Selected`, validate once immediately before execution, write `service_continuation_selected` with the original complete `Advance` envelope and before-sequence/serial/key, then execute that envelope once through the existing CPU-log/Store writer. The service operation namespace is the checked `0xc000_0000_0000_0000 | (sequence + 1)` range. Before emitting the event, reject any existing receipt for that exact operation as `service_due_namespace_collision`; do not reuse its receipt or retry with a different ID. Generic Sim Execute rejects the reserved C/D/E namespaces. A user-initiated ordinary `Advance` remains the recovery path after a collision.

The original Store envelope, receipt, and recovery remain authoritative. A selected page is neither an ACK nor completion of a household service. A lost terminal response must recover the exact selected envelope and inspect its original outcome/Timeline serial; it must not submit a newly selected operation. No automatic advance to a future boundary occurs.

## Tests authored; not run by this writer

`service_needs_due.rs` has three bounded integration cases using the actual Store, service authority, Timeline, Sim `Client`, and RunDue path; `required_dispatch.rs` also contains one pure cursor-encoding unit test:

1. `p5_snapshot_missing_hint_stale_page_cold_resume_and_no_future_advance` starts from a real accepted HousingLease setup with the fixed seed-107 day-zero 08:00 occurrence. It checks no work before the boundary, the initial P5/no-run candidate and literal key/window/source values, missing-hint fallback, stale selection rejection after one original page, the real transition from no run to Occurrences row zero, cold Store reopen, completion of the current boundary, and no sequence/time change from selecting again.
2. `sim_rundue_lost_terminal_response_replays_only_the_original_page_receipt` opens that real persisted P5 world in the built Sim, retains the selected event's original C-namespace envelope, closes without consuming the terminal result, and reopens Store to check one committed sequence/serial advance and exact-envelope receipt replay without another page. This is an application that does not consume the final result frame; it does not simulate arbitrary TCP packet loss.
3. `sim_execute_c_namespace_is_reserved_and_an_occupied_due_operation_fails_closed` checks that generic Execute cannot preoccupy C and does not change status, then uses a real Store `Advance` receipt occupying the next selected C operation. RunDue must return the namespace collision within a 16-event bound, before its selected event, and preserve sequence, state hash, progress, and pending service work. The same accepted continuation then remains usable through an ordinary sequence+1 `Advance`. The collision event and before/after proofs are retained with the case.

Fixture `service-needs-p5-due-v182.json` is parsed by the test and supplies literal seed, target, page width, stable source bytes, schema version, and cursor oracle. Test worlds are created only below an existing `MYCITY_ISSUE101_SERVICE_DUE_ROOT` under `/Volumes/HP P900/dev/MyCity/work`; missing configuration, missing volume, or a non-external root fails closed. Each case uses a unique directory, and worlds/source openings remain in place. There is no temporary fallback or cleanup path. The Sim opens this real Store using the same `sim-w3:<MYCITY_SOURCE_SHA256>` identity as Profile::Base; no custom Sim profile or fake candidate DTO is used.

No `cargo`, `rustc`, formatter, test, script, or executable validation was run in this writing session. The source-only `git diff --check` completed without whitespace errors. Compile, the three integration tests and one pure cursor unit test, Sim operation collision behavior, CPU/memory/disk peaks, full Store recovery certification, and product acceptance are `not_run` here and remain for the root's serialized release/advisor review.

## Canonical source read set

Read from the dirty canonical checkout at v182; these hashes identify the source versions used for API alignment, not a claim that their WIP is part of this source contribution.

| Source | SHA-256 |
|---|---|
| `crates/city-core/src/service_needs_authority.rs` | `4180e8f36c972ea91ce7e4fc176f513b2a87646b5eed5907ff1b02beb346a340` |
| `crates/city-core/src/service_needs.rs` | `a7da9b901b38fbb7aa114e32589ff31eb2837ff885b1ec2e3845e2a3dccfd4b8` |
| `crates/city-core/src/transaction.rs` | `33cfe8b3aec7c329fd7c628808d52e65097c5f85b9ec87b1993e80c9996eb92f` |
| `crates/city-core/src/timeline_driver.rs` | `d2d118082889b4535a119dcbf5b33d3567458cf7e8e1fd8747216fdba05f23f5` |
| `crates/city-core/src/task_registry.rs` | `46416b1c6e7af291c2285c2ab70b2c9c74e819b9fd27221b2712ca93dcce724b` |
| `crates/city-storage/src/lib.rs` | `0841d20bd4b749f643b5db797236374ff79649c1b6162c9cb5fb9ecafa8fa309` |
| `crates/city-storage/src/continuation.rs` | `5c583b06df08c000514a4a534e29d23912619f7333c515e6b14b9ddb8aeaa45b` |
| `crates/city-storage/src/checkpoint.rs` | `9a03cd678ce52829aa00029da487a3f353862ca3bc65cf25928ae1e649b26d4e` |
| `crates/city-storage/tests/service_needs.rs` | `c5f2e2cd744e518906c2f299f6e70f744979dceecede1ed6c5e29d8be5b71151` |
| `crates/city-headless/src/sim_host.rs` | `bcfb13b3f4b18354aa3fa41cc4a065a8a3e257f7257f7dfc1858a55857e405ed` |
| `crates/city-headless/src/sim_client.rs` | `03f844d0e0a3903c7c2fa87eafe8a4be6444a9ba03d0224e94f1f9103475209a` |

## Serialized release request and limits

After the external directory below already exists and the root freezes the matching source/executable read set, the focused command is:

```sh
MYCITY_ISSUE101_SERVICE_DUE_ROOT="/Volumes/HP P900/dev/MyCity/work/service-due-opening-20261025" CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -p mycity-headless --test service_needs_due -- --test-threads=1
```

This is a request for the existing shared target `/Users/frankqin/dev/MyCity/target`; it is documentation only and was not run here. If the external root, volume, matching executable, or existing target is unavailable, stop instead of using a temporary directory, another cache, or an unpinned binary.

The tests use one small P5 household and at most one original Timeline page per command. No formal CPU, memory, parser, restore, filesystem, or peak-storage budget was measured or is guaranteed. This work adds no worker, provider, delivery/admission, durable token, service ledger, separate scheduler, M0 behavior, auto-advance, or #102/#103/#104 resource authorization. Retain all external worlds and logs until the root records their hashes and confirms recovery/audit no longer depends on them.
