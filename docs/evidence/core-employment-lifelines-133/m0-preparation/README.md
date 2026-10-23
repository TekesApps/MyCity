# M0 T13-A joint control 04 preparation

Prepared at v0.5.132 commit `61587dcc0297924d014ce3c6eb7e47fd6a6503dc`, this external control binds the final integrated headless source `a6290a562362b7ea21379726f901a61cf54d32e8d9fe2059bccf3fe44449d3d8`, its exact 276-entry source ZIP, 264 build-fingerprint entries, Cargo.lock, two acceptance fixtures, executable, fixed-input manifest, and every runner/support file. Its original v1 provenance files are preserved byte-for-byte under `preparation-v1-original/`; the corrected static verifier compares every one of the 276 ZIP entries to v132 git bytes. It does not inherit the prior source's successful smoke or capacity receipt.

The runner preserves the original workload and guards: full 720 game hours, 723 targets, 2,880 queries, 96 saves, 725 normal-pool grants/returns/acks, 600 smoke or 28,800 formal layout scans, 4 Hz observer sampling, 120 seconds continuously normal with no Cargo/rustc before launch, 64 MiB whole-role budget, 8 MiB shared diagnostic boundary, 60-second non-normal pressure guard, 256 MiB combined logs, 20 GiB world cap, 2 GiB free-space floor, and two actual snapshots. Existing paths `j3n-01` and `j3o-01` have the same six-character run-root component length as the prior roots, preserving the Unix-domain socket boundary.

The frozen acceptance fixture specifies a worker-cell kill no earlier than 2 hours, committed simulation-reply loss no earlier than 4 hours, normal cold reopen no earlier than 6 hours, then the 8-hour due boundary and final business boundary spanning the run. The 8-hour boundary is completion, not an added fault. The runner keeps that original contract unchanged.

Pure checks and static source validation, which do not call Cargo, wait for quiet admission, or execute MyCity:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s /Volumes/HP\ P900/dev/MyCity/work/j3n-control -p 'test_*.py' -v
PYTHONDONTWRITEBYTECODE=1 python3 /Volumes/HP\ P900/dev/MyCity/work/j3n-control/preflight.py
```

After a future same-source smoke passes, persist and review its log-capacity receipt before authorizing the matching formal run. The formal command requires that receipt and checks it against all full smoke frames. Do not reuse smoke evidence from another source.

```sh
python3 /Volumes/HP\ P900/dev/MyCity/work/j3n-control/run.py smoke 01
python3 /Volumes/HP\ P900/dev/MyCity/work/j3n-control/log_budget.py /Volumes/HP\ P900/dev/MyCity/work/j3n-01
python3 /Volumes/HP\ P900/dev/MyCity/work/j3n-control/run.py formal 01 01
```

Both run roots are new and are not created during preparation. All performance results must name the external HP P900 USB/APFS SSD. This preparation is not admission, smoke, formal, performance, 8-hour, or real-16GB acceptance.
