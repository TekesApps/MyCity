# Market access validation, attempt 05

## Frozen inputs and commands

- Branch: `codex/core-market-access`; base commit: `14ba795d67ef6574e77a60a71314171fc1ec0cae`.
- Rust: `rustc 1.85.1`; Cargo flags: `--locked --offline --release`; shared target: `/Users/frankqin/dev/MyCity/target`.
- The exact source archive and SHA-256 receipts are under `source/`. No whole-workspace tests or scale-assembly workload was run.
- Attempt 03's initial compilation failure, intentional interruption (130), and two failing adapter expectations remain preserved in their original raw logs.

## Results

- Transport/path receipt unit tests: 14 passed, 0 failed. Raw output: `raw/transport-tests.txt`.
- Market access adapter integration tests: 12 passed, 0 failed. Raw output: `raw/market-access-tests.txt`.
- `cargo clippy -p mycity-core --all-targets --locked --offline --release -- -D warnings`: passed after fixing the CLI digest formatter. The initial clippy failure is retained at attempt 04.
- Six runs of the fixed CLI binary succeeded; JSON receipts are in `raw/cli-*.json`.

| Scenario | Served outbound/return | Boundary source IDs | Accessible and sold | Unmet reference need |
| --- | ---: | --- | ---: | ---: |
| normal | 10 / 10 | none | 200 | 0 |
| road closed | 10 / 0 | none | 0 | 200 |
| bottleneck | 2 / 2 | none | 40 | 160 |
| capacity expanded | 10 / 10 | none | 200 | 0 |
| legal cross-city | 10 / 10 | outbound 9100, return 9200 | 200 | 0 |
| unrelated cross-city OD | 10 / 10 | none on the market quote | 200 | 0 |

All CLI records report `pureplan_not_settled` and `engine_commit_required`. These checks prove bounded transport assignment feeds the planner's quote/access capacity and demand summaries; they do not prove Engine ledger commit, cash/inventory settlement, or full-city performance.

## Fixed binaries

`fixed-binaries/` contains hardlinks to the tested release CLI, transport unit-test executable, and adapter-test executable. Their hashes are in `fixed-binaries/SHA256SUMS`; `link-check.txt` records shared inode metadata.
