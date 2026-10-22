# Core road/town validation controller

This directory is a staged validation harness, not acceptance evidence. It has not run
Cargo, a model command, or the helper tests.

Each future run retains the exact controller, helper-test source and README with a
per-file hash manifest under `controller-source/`, separate from the Rust source archive.

`python3 run_controller.py` defaults to a plan. `--run` requires an explicit full commit
hash and refuses to start while Cargo, rustc, cargo-clippy, clippy-driver, rustdoc, or a
detected MyCity M0 measurement process is active. The headless measurement gate recognizes
`joint-endurance`, `layout-session`, `scale-endurance`, `steady`, `speed`, and `rare`
subcommands, including when an observer wrapper remains after the measurement process exits.
Before every command it rechecks the live-process gate and exact clean source HEAD; after
every command it records and verifies the same. The controller also checks the HP P900
work and shared-target symlinks before starting.

The run reuses the existing external target, removes inherited `RUST_MIN_STACK` and all
`MYCITY_*` variables, then sets only this run's `MYCITY_TEST_ARTIFACT_ROOT`. Every command
gets a fresh evidence directory, separate stdout/stderr, an exit receipt, and monotonic
wall time. CLI commands are wrapped with macOS `/usr/bin/time -l`; recorded maximum RSS
covers the entire process and runtime. It does not measure model-only heap or count as an
M0 performance result. If a test command fails, the controller copies only the exact
executable named by that command's stderr `Running (...)` line, and only from the shared
target. If compilation failed before producing a test executable, it records `not_found`
and does not substitute an older binary.

The source snapshot writes `source-scope.json` with an exact path list and per-file SHA256,
plus archive and Cargo.lock hashes. It includes Cargo/toolchain inputs, `.cargo`, `crates`,
`fixtures`, and compile-time documentation fixtures used by storage construction and
headless economy-short tests. It excludes unrelated historical evidence. A failed command
stops the sequence and keeps logs, source snapshots, worlds, and captured binaries. The
controller never emits an overall passed/accepted result.

The v119 transport compatibility reader uses rules identity
`m0-town-transport-fixed-profile-v1` and an independently recorded v119 state hash.
Accepted-construction compatibility requires both fixture groups in one `--run`: one
accepted WAL Store and one accepted checkpoint Store. Supply a repeated world/manifest/seal/
rules/hash group for each; both are mandatory together. The source worlds are fixed at
`/Volumes/HP P900/dev/MyCity/work/legacy-accepted-project-119-run01/accepted-wal` and
`.../accepted-checkpoint`. They are copied under the run evidence directory and only the
copies are opened by the current reader.

The legacy source is pinned to
`/Users/frankqin/dev/MyCity/docs/evidence/core-transport-tax-integration-119/attempt02/source.zip`
with SHA256 `75d641c5696873580e8ef2a92606d49d9e6fa6dfe4489b7439c9886e5cda7bbe`; its
`freeze.json` SHA256 is
`3eaac12110dca5e07bd008fe31406eff03771b554faf040ef2cdb90b97c2c0f6`, and the business
source hash inside that manifest is
`717148eab1b00e5fc183ee240240fa9150cc839f9a94aa96aaa8bb8a22f67f82`. For each fixture,
the controller checks a full original-world file manifest, a root-issued seal, and hashes
for the frozen old writer's receipt, report, stdout, stderr, preserved executable, and
exporter harness. The seal also binds fixture kind, rules, state hash, source pins and the
world manifest. The current old-writer output and per-world seals do not yet exist, so both
compatibility checks remain `blocked/not_run` until root generates and reviews them.

After copying, the controller checks that the source world's full-file hash manifest still
matches before and after `Store::open`. It records the disposable copy's post-open manifest;
the copy may change during normal reader recovery. A symlink or path escaping either fixed
evidence root or the external work volume is rejected.

The suspension commands use the source-defined CLI order
`town_suspend <new-world-dir>` and `town_suspend <new-world-dir> --loss`.
The plan also includes `town_transport road-28 <new-world-dir>` for the untested
28-day joint scenario. Its finite opening stock means settlement through day 28 does
not establish continuous sales, economic balance, or self-funding.

`python3 -m unittest test_controller.py` is the future pure-helper test command. It has not
been run. Current changes are source-only; no Cargo, CLI, or model validation has run here.
