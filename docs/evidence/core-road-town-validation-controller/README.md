# Core road/town validation controller

This directory contains the clean-commit validation controller. The controller itself has
not run its complete sequence: root performed precommit verification with frozen sources,
preserved executables and separate commands instead; see
[the actual stage evidence](../core-road-town-integration-122/STATUS.md). Its nine pure helper
tests have now run and passed after normalizing the test temporary root with `resolve()`;
the initial `/var` versus `/private/var` failure is retained. These helper tests do not run
Cargo or prove a model or performance result.

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
world manifest. Root has now executed the frozen v119 writer and sealed its accepted WAL/checkpoint
outputs. In run 68 the preserved new reader opened full copies of both, plus the historical
v119 transport world; all expected hashes matched and originals stayed unchanged. The
complete controller sequence remains unexecuted; these are separately recorded runs.

After copying, the controller checks that the source world's full-file hash manifest still
matches before and after `Store::open`. It records the disposable copy's post-open manifest;
the copy may change during normal reader recovery. A symlink or path escaping either fixed
evidence root or the external work volume is rejected.

The suspension commands use the source-defined CLI order
`town_suspend <new-world-dir>` and `town_suspend <new-world-dir> --loss`.
The plan also includes `town_transport road-28 <new-world-dir>` for the untested
28-day joint scenario. Its finite opening stock means settlement through day 28 does
not establish continuous sales, economic balance, or self-funding.

`python3 -m unittest test_controller.py` is the pure-helper test command. The actual 9/9
helper result and preceding failure are archived separately. Cargo/CLI results belong to
the linked precommit evidence, not to a purported complete controller invocation.

The controller also builds the real `mycity-core` release examples explicitly with
`cargo +1.85.1 build --locked --offline --release --examples -p mycity-core`; the
all-targets test command alone is not treated as producing a runnable example binary.
It preserves and hashes the built `town_market` executable, then runs its actual CLI
serially for the seven documented 28-day profiles: `base`, `cash-shortage`,
`raw-shortage`, `no-route`, `capacity-cut`, `energy-price-rise`, and `demand-drop`.
These Engine examples take no world-directory argument. Each invocation gets the same
stdout/stderr, monotonic wall-time, and whole-process `/usr/bin/time -l` RSS evidence as
the other CLI stages. The first summary line is parsed into an exclusive
`town-market-result.json`; the original output logs remain authoritative evidence.

Two existing turnover-tax pressure combinations are also included with their documented
arguments: base at 500 bps, and base at 500 bps with company cash 0, a 1,000,000-cent loan
due on day 2, and production utility cost 50. The latter is a bounded cash-pressure
substitute, not a calibrated cost/revenue equilibrium. Results explicitly label route and
utility as profile substitutes (`live_transport=false`, `live_utility=false`), keep
`tax_policy=not_configured`, and state that opening-capital dependence, profitability, and
sustainable self-funding are not proven. Engine-owned capacity remains distinct from
whole-process RSS and does not establish the 16 GB performance gate.

The candidate also preserves and runs the two no-directory Engine examples,
`construction` and `unfinished_clearance`, serially after the Store scenarios.
The latter follows partial construction, explicit review and consent, additional
clearance labor, actual waste capacity/fees, and final site release without first
commissioning the project. The release storage test suite separately exercises
precommit failure, committed-but-lost responses and cold reopening; an Engine-only
CLI is not a substitute for those persistence checks. These additions remain
unexecuted source candidates while the original formal 8-hour run is live.

`town_transport road-clearance` is also planned as a separate fresh Store world:
it checks that clearing a never-commissioned road project preserves its base route,
including post-closure market service and historical proof recovery. This differs
from `road-withdraw`, which removes a previously commissioned increment. The
candidate electricity bridge adds core tests for local Boot consumption alongside
Exchange and plans two actual Engine CLI invocations, `town_utility` and
`town_utility --generator-failure`. The executable is preserved with its SHA; the
two commands take no world-directory argument. Neither CLI has run yet. DTO unit
tests and an unchanged transport-only CLI cannot establish this additional integration.

The finite joint candidate `town_lifelines` gets three independent preserved Store
worlds: normal, held road inspection and generator failure. Its original fixed
utility fixture is only a source of registered initial capital and stocks; actual
electricity comes through Boot, road capacity through accepted construction, and
sales through Store-owned transport proofs and Town settlement. See
`docs/town-lifelines-joint.md` for the exact scope and pending checks. These two-day
joint scenarios do not replace the seven original 28-day economic profiles or any
frozen M0 workload/performance gate.

Construction `recoverable_v2` candidates use independent Envelope identities so a
new request can retry after a durable negative receipt. Missing policy remains
`legacy_v1`; old WAL rejection outcomes must not be reinterpreted. The core/storage
test stages include source candidates for this distinction, and the real old-writer
compatibility stages remain required. No migration or acceptance is inferred from
the optional policy field alone.

The native memory parser matches the actual macOS `time -l` layout, with the
numeric byte value preceding `maximum resident set size`. A tiny `/usr/bin/true`
format probe is retained in `native-time-format-probe.txt`; it is not a MyCity
performance run. CLI logs must contain one positive unambiguous RSS value or the
controller refuses acceptance after preserving the logs. If present, peak memory
footprint is recorded separately; RSS is not silently treated as footprint or as
an M0/16GB gate. Parser regression tests ran only after the formal run was terminal. They passed as part
of the nine helper tests; they do not change the failed formal result.
