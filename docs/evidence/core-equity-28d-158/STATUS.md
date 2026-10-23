# D28 equity, tax, and business-exit evidence

Prepared 2026-10-24 from the frozen candidate and execution receipts dated 2026-10-23. This note records a small Store-backed scenario, not a city-scale or 16 GB performance result.

## Frozen executable and source

The release example was built with `cargo build --release -p mycity-storage --example town_equity_28d --locked`; attempt 473 exited 0. The frozen executable is `town_equity_28d`, SHA-256 `5be50b51e6f9f9047e6b978ccb7885ad095a5592031a9252ae8d29e514e8aaf7`. The coordinator's source identity is `24f5177b57a0b23058c314490614e4195eec5ad338ce915c9330a3bf8a66d7c9`. The source archive used by attempts 474–477 has SHA-256 `45ae50ad7a87ae6008b673ccadb0f428347a83752a8646fe2a1d189302ebe449`; its file manifest has SHA-256 `24c0ac8de635f6c4a0a91d2b8ae7cb13f0445cd1ace782f3ee4d9aaec9e7649f`.

## Actual runs

Attempts 474 and 475 ran the same release executable against fresh Stores, one `base` and one `household-cash-shortage`. Both exited 0 with `source_unchanged: true`.

| Observed result | Base | Household cash pressure |
|---|---:|---:|
| Initial and final cash | 3,000,000¢ | 2,800,000¢ |
| Produced / sold | 280q / 280q | 220q / 219q |
| Raw / product remaining | 0q / 0q | 60q / 1q |
| Accrued and paid wages | 22,400¢ / 22,400¢ | 22,400¢ / 22,400¢ |
| Tax assessed and paid | 1,260¢ / 1,260¢ across 28 claims | 938¢ / 938¢ across 22 claims |
| Retained earnings after day 29 | 1,260¢ | −3,767¢ |
| Dividend | 1,260¢ (household 1,239¢, treasury 21¢) | rejected; 0¢ |
| Fiscal transfer from government dividend | 21¢ | 0¢ |
| Exit result | `Closed`, 612,800¢ capital returned | `Liquidating`; close blocked, no capital returned |

The base case returned the remaining 612,800¢ capital after paying the claim and emptying stocks. The pressure case retained unsold inventory, had negative retained earnings, rejected the dividend, and remained liquidating. It did not convert the failed close into a successful exit.

The full-copy receipts in 476 and 477 state `full_copy_verified: true` and `original_unchanged: true`. A fresh `--verify-open` process opened each complete working copy with exit code 0 and no business commands. Its state hashes matched the corresponding run/oracle: base `5df06517821549485058129d1ac5340992789301f16a1dd69f572e45da9d2c3b`, sequence 734; pressure `9fc53fb65489fbeb9f4ea8ba2794529e262e6250af87bd236d1819781007283d`, sequence 733. Store open may recover/write, so only the copied working worlds were passed to that reader.

The independent checkpoint oracle reports `passed: true` for both cases and agrees on account balances, inventory, tax, retained earnings, capital return, dividend, exit phase, checkpoint digest, sequence, and state hash. Its declared scope is independent recurrence and checkpoint-state inspection; it does not replay the WAL. The separate fresh-process Store open and matching hash provide the cold-reader evidence. Do not describe the oracle alone as WAL replay.

Evidence is retained under `/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01/`: `473-D28-report-build/`, `474-D28-final-base/` and `474-D28-final-base-world/`, `475-D28-final-pressure/` and `475-D28-final-pressure-world/`, `476-D28-base-cold/`, `476-D28-base-cold-reader/`, `477-D28-pressure-cold/`, and `477-D28-pressure-cold-reader/`. These are source, executable, execution, world, full-copy, reader, and oracle records; this document does not duplicate the large Store worlds.

## Resource and model limits

The example's reported owned capacities are about 3.60 MB Engine, 41.15 MB Store, and 101,576 bytes World for this tiny fixture. `/usr/bin/time -l` recorded process maximum RSS of 56,246,272 bytes for base and 49,577,984 bytes for pressure. Those are individual small-process observations, not the model's exclusive heap, not a scale result, and not a 16 GB acceptance. The report explicitly leaves `full_16gb` as `not_run`.

The scenario starts with a finite raw lot of 280q and a fixed per-output utility charge. The delivery source is fixed quote/profile id 77, not a live spatial road graph; utility cost is a recipe/profile input, not live Boot electricity or water. It proves this bounded accounting path only. It does not prove continuous raw sourcing, real transport capacity changes, Boot supply integration, price equilibrium, or an open-ended self-financing business.

## Verification and retained failures

Final source passed 470 fmt, 471 workspace/all-target strict clippy, 472 two release Store tests, and 473 release example build. Commands and source-unchanged receipts are in `commands.json`. Fresh executable readers and final independent checkpoint oracles passed. Earlier macro recursion and missing mutable Store argument compile failures (457/459) remain under `failures/`. Initial oracle failures concerned omitted zero serde fields, snake-case exit phase, and genesis-v1 absence of spatial state; failed source and logs remain under `failures/`. Corrected oracle: `check_checkpoint.py`. Earlier 465/466 reports mislabeled combined utility payments; 474/475 separately name production utility and buyer surcharge, using unchanged business rules. Base requires actual Closed, not Closed-or-Waiting. This release adds bounded example/test/oracle/evidence only; production Engine, World, Timeline, and Store are unchanged.
