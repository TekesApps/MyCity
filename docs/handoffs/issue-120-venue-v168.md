# Issue #120 v168 managed venue Store boundary

Status: test-source and retained synthetic fixture only. The source author did not run a compiler,
test, formatter, script, benchmark, stress run, or GPU work. The coordinator's `--release` source
check/build passed, then its first runtime run failed at the AccountView `opening_source_id`
expectation described below; the corrected test source has not been rerun. Runtime acceptance
remains unverified and all later test cases remain `not_run`.
This is a candidate acceptance source for the v168 `managed_people_v1` venue input through the
existing Store factory. It does not add a second Store, Engine slot, opening parser, or writer.

## Fixed source and ownership

- Repository: `/Users/frankqin/dev/MyCity`.
- New isolated worktree: `/Users/frankqin/.codex/worktrees/issue120-venue-v168/MyCity`.
- Branch: `codex/issue120-venue-v168`.
- Source baseline: `14d4526692b4326e5d4fc81b331cfc3935820b20` (v0.5.168).
- Source fixture: `crates/city-storage/tests/fixtures/opening-venue-store/managed-people-v1-opening.json`.
- Test source: `crates/city-storage/tests/opening_venue_store.rs`.
- Handoff: `docs/handoffs/issue-120-venue-v168.md`.
- Input was copied byte-for-byte from
  `/Volumes/HP P900/dev/MyCity/work/v118run14-76501/venue/lifecycle-76536-0/opening.json`.
  SHA-256: `eb8529e57b6b3af3550656a17275dc34b02a0674c31b9e13cbf8756d6dc812b8`; size: 3346 bytes.
  This is a synthetic, locally constructed legal opening used by a successful Store run, not a
  real-world source, actual venue permit, actual participant, or proof of service delivery.

Only the three listed files were added in this v168 worktree. The original `issue120-opening-loader`
tree and its prior work were not changed, rebased, or reset. If #101 publishes a newer base first,
the coordinator owns exact-source transplant and must refix the baseline before unified execution.

## Test API and artifacts

The dedicated Cargo integration target is `opening_venue_store`. The parent test is
`managed_people_v1_opening_same_slot_wal_checkpoint_and_no_overwrite`; its fresh-process helper is
`fresh_store_restore_probe_child`, ignored by default and started by the parent through
`current_exe` with a 60-second timeout.

The parent must set `MYCITY_VENUE_STORE_TEST_ROOT` to an existing unique directory below
`/Volumes/HP P900/dev/MyCity/work`. The test checks that the path is on the mounted external
volume, creates a unique child run directory, preserves all world files and logs, rejects symlink
entries, and enforces a 128 MiB file-byte ceiling. It has no temporary-directory fallback and no
Drop cleanup. The helper receives `MYCITY_VENUE_STORE_CHILD_PROBE` with the retained world path and
phase. The helper runs only in a child process, after the parent drops the Store lock. The parent
creates unique retained stdout/stderr files and directs the child streams to those files (no
unread pipes). Launch metadata is written before spawn, started metadata records the owned child
PID, and result metadata records exit code/success, timeout or wait error, log paths and byte
counts before the failure assertion. Each helper has a 60-second owned-child safety bound; on
timeout the parent kills and waits for that child only. This is a bounded-hang safeguard, not a
performance threshold or latency result.

The test uses `OpeningInput::from_json` and asserts `InputOrigin::SuppliedBytes`. It calls only
`Store::create_opening_input_in_workspace_with_observer`, `Store::open`, the existing Engine
views/resources, the existing `Store::receipt`, `Store::checkpoint`, and the existing
`Timeline::Enable` and `Venue::Accept/Enter` transaction actions. Resource inputs are explicit
`Resource::new(501, 4)`, `(502, 4)`, `(503, 5)`.

## Covered source oracles

- Exact supplied opening byte vector and literal SHA-256 remain in `genesis.json`; the same caller
  `EngineWorkspace` initial/world/engine/timeline/background/preparation identities are compared
  before and after the original Store factory. The actual Engine owned-capacity is compared with
  the existing `ENGINE_BYTE_BUDGET`; this is logical Engine capacity, not process RSS.
- Literal accounts/cash/source registrations, original inventory owner/quantity/basis/lot, ten
  registered people, and 2 venues/4 offers are checked from this same fixture. At initialization,
  resources 501/502/503 have exact capacities 4/4/5 with reserved=occupied=0. After the actual
  Enter, resource 501 remains capacity 4 with reserved=0/occupied=3, while 502 and 503 remain
  reserved=occupied=0. The immutable opening facts are checked separately from this transition
  state. These assertions provide a successful legal control before the malformed-reference
  candidate.
- The input's `accounts[].source` values 1/2/3/4 are checked with literal account ids and cash
  amounts, and the opening source table is checked against the corresponding literal cash and
  inventory capital rows. Runtime `AccountView.opening_source_id` is expected to be 0 for these
  accounts because this fixture has no town equity charter; that runtime field retains chartered
  company cash provenance, rather than the general input capital-source binding.
- Actual `Enable`, Account(3) `Accept`, and System `Enter` bind operation 3, event 100, time 0,
  token 1, one actual use of 3 people and Resource 501 occupancy 3. Operation 2's durable Accept
  receipt creates token 1; operation 3's Enter receipt creates no token because it reuses that
  accepted token. The original Enter receipt is compared with the identical-operation replay. The
  resource counters, one use, one activity, persisted receipt and replay are checked after a
  fresh-process WAL open and again after a separately started checkpoint recovery process.
- Recreating an already published venue world must fail and leave the complete recursive file
  manifest byte-identical.
- A candidate opening with venue owner 999 is attempted only after the canonical control has
  passed Store initialization; the expected error is `missing_account` and no CURRENT may publish.
  The canonical source can then be installed in a distinct new directory while the prior world
  remains unchanged. This is a synthetic malformed-reference control, not an entitlement check.
- The `CurrentRenamed` observer injects a lost response after publication. A fresh process reopens
  that same path and confirms the exact original balances. The original observer error text is
  retained. The parent Store then applies one literal transfer of 137 and checkpoints it. Another
  fresh process restores the checkpoint and replays only that same transfer receipt, proving cash
  remains 999863/1200137 without new business. A retry-create must leave every file unchanged. No
  initialization or capital issuance is repeated.

All population, capacity, source, venue and event facts come from the retained opening bytes and
the existing Store-owned Engine. The accepted raw host event remains an input supplied by the
trusted game process; the test does not certify travel, lawful access, education, permits or
real-world venue completion.

## Not covered / remaining work

- The coordinator's first retained runtime attempt failed in the legal-control assertion because
  the source test incorrectly expected the input capital-source id in runtime
  `AccountView.opening_source_id`. The existing assembler only copies that runtime field for a
  company covered by a town equity charter; the fixture has no such charter. This source correction
  now checks the input source-to-account/cash binding directly from the raw opening and expects the
  runtime charter-provenance field to remain zero. The failed run is preserved at
  `/Volumes/HP P900/dev/MyCity/work/p120venue09-33083`. The corrected
  assertions and later WAL/checkpoint/replay cases are pending the coordinator's serial rerun.

- This test does not wire the production `init-opening` CLI or prove the full CLI path from local
  opening/resources/spatial file reads into Store.
- The resource manifest and CSV/profile/schema migration path are not implemented by this test.
- No live filesystem fault outside the explicit `CurrentRenamed` observer cutpoint, device loss,
  process kill during sync, concurrency race, platform portability, native memory, or RSS was
  measured.
- File sizes in the artifact budget are bytes written, not Vec capacity, resident memory, or
  device-level allocation. Fixture plus test source are expected to remain under 16 MiB; runtime
  peak memory is unknown. Estimated artifact ceiling is 128 MiB; actual run output/retention cost is
  not measured until root schedules it.
- This handoff does not resolve receipt decoder, Store, Engine, CLI, resource ownership, or product
  authority questions and does not change M0 acceptance.

Future root-owned serial command after reconciling to the then-current main and reserving the
existing HP P900 target/work window:

```sh
MYCITY_VENUE_STORE_TEST_ROOT=/Volumes/HP\ P900/dev/MyCity/work/<precreated-unique-run-dir> \
  CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target \
  cargo test --release -j1 --locked --offline -p mycity-storage --test opening_venue_store -- --test-threads=1
```

The helper process is launched by the parent test. Keep the selected run directory, executable,
fixture provenance, logs and complete worlds until root archives the evidence; do not clean while
the validation window is active. No PR was created. Issue #120 remains open; this source is not
Store/CLI acceptance.
