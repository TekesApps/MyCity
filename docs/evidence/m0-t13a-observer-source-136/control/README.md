# M0 T13-A joint control 05 observer-source-key repair

This fresh external control binds the final integrated source `a6290a562362b7ea21379726f901a61cf54d32e8d9fe2059bccf3fe44449d3d8`, exact 276-entry ZIP, executable, fixtures, Cargo.lock, and fixed support bytes. The preceding `j3n-01` observer failed at startup because `observe.py` requested `source.json["source_sha256"]` while the frozen document correctly exposes `fixed_source_sha256`. Its admission and child termination remain preserved as a failed attempt; no samples or performance credit are inherited.

The observer now validates the required `fixed_source_sha256` key against the exact expected hash passed by `run.py`, before creating a run root or spawning the model. It explicitly rejects the legacy/wrong key, missing key, malformed hash, and mismatching source hash. `run.py`, `preflight.py`, `log_budget.py`, and the observer all use `source_identity.py`, a small shared parser, so the observer does not import the much heavier static-preflight stack. The runner/support manifest contains 17 files (the original 16 plus this helper).

Workload and acceptance guards are unchanged: 720 game hours, 723 targets, 2,880 queries, 96 saves, 725 normal-pool grants/returns/acks, 600 smoke or 28,800 formal layout scans, 4 Hz observer sampling, 120 seconds continuously normal with no Cargo/rustc before launch, 64 MiB whole-role budget, 8 MiB shared diagnostic boundary, 60-second non-normal pressure guard, 256 MiB combined logs, 20 GiB world cap, 2 GiB free-space floor, two actual snapshots, worker kill at/after 2h, committed-reply loss at/after 4h, cold reopen at/after 6h, and the 8h due boundary. The 8h boundary is completion, not another fault.

Pure checks and static validation do not run Cargo, wait for admission, or execute MyCity:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s /Volumes/HP\ P900/dev/MyCity/work/j3p-control -p 'test_*.py' -v
PYTHONDONTWRITEBYTECODE=1 python3 /Volumes/HP\ P900/dev/MyCity/work/j3p-control/preflight.py
```

After a fresh same-source smoke passes and its log-capacity receipt is reviewed, formal may be prepared with that matching smoke receipt only. Commands are deliberately not run during this repair:

```sh
python3 /Volumes/HP\ P900/dev/MyCity/work/j3p-control/run.py smoke 01
python3 /Volumes/HP\ P900/dev/MyCity/work/j3p-control/log_budget.py /Volumes/HP\ P900/dev/MyCity/work/j3p-01
python3 /Volumes/HP\ P900/dev/MyCity/work/j3p-control/run.py formal 01 01
```

The new pathname roots `j3p-01` and `j3q-01` retain the six-character component length used by the frozen Unix-socket proof. The proof must be regenerated from the source ZIP for these paths. This preparation is not admission, smoke, formal, performance, 8-hour, or real-16GB acceptance.
