# v178 E housing regression

Root ran the existing housing chain on fixed published commit
`33ba60bbeb380f68c6366d2a864f4b746361eb1e` / v0.5.178. No game code, fixture,
business assertion or M0 standard changed. This supplements the finite
[v175 housing evidence](../housing-no-candidate-wait-20261024/STATUS.md), without
rewriting its results or closing the full parent #4.

Serial release results: housing_runtime2, housing_wage_window_runtime4,
housing_move_store_runtime4, housing_no_candidate_wait_store2 and
housing_equity_spatial1: **13 distinct parent tests passed**. The fixed
town_housing_move CLI and a separate fresh verify-open process also exit0;
they are two process executions, not two additional tests. Recovery issues
zero model commands and operates on a full copy; the original world remains
byte-identical. Literal independent checks retain total cash2885000, tenant0,
old claim383226 minus paid341000 =42226, new claim30000, moving fee10000,
provider344000 and day42 paid increment21000. Fresh state hash and sequence
match the original final cold state.

Original advisor reviewed actual results read-only: finite gate accepted,
no new blocker. Reviewer did not execute the commands. Source and fixed
executable hashes match before/after. Full source, external literal fixtures,
commands, launch/PID/exit records, stdout/stderr, original worlds, recovery
copy and independent oracle remain under
`/Volumes/HP P900/dev/MyCity/work/qualification-e-regression-20261025/`.
The seven historical ancillary inputs and four external test fixtures are
separate complete ZIPs, compared byte-for-byte to canonical inputs; hashes
do not replace their originals. Retention cap256MiB is an artifact planning
allowance, not a simulation grant. Existing target caches are retained for
the immediate dependency-chain regression; no recovery evidence is deleted.

Actual environment: Mac16,1/32GiB, Rust1.85.1, locked/offline release -j1,
existing HP P900 USB/APFS shared target/work. Functional command durations
are not sustainable-speed measurements. No true16GB, maximum population,
32-household load, housing-to-OD, GPU, native memory peak or M0 acceptance
follows. Full #54/#55 and parent #4 remain open; #26's finite closure is not
repeated. No PR; direct root evidence publication only. Protected observer,
clock and rare-control WIP remain unchanged.
