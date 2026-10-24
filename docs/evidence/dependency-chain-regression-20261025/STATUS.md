# v178 capacity → dispatch → same-slot recovery regression

This is a new regression on fixed published v0.5.178 commit
`33ba60bbeb380f68c6366d2a864f4b746361eb1e`, not a rewrite of the original
v168/v169/v170 results. Root changed no game code, test, fixture, business
assertion, default stack or M0 frozen load/acceptance standard.

The original advisor first rechecked evidence retention: no new blocker.
Each test receives its dedicated existing HP P900 work subdirectory. Full
worlds are retained; no temporary-root fallback or Drop deletion. Child
stdout/stderr, PID and terminal metadata are persisted; only an owned
child may be handled by its existing timeout safeguard.

Serial locked/offline release -j1 builds exit0. Fixed executables then run
in dependency order with one test thread:

| Issue | Existing actual path | Passed parent tests |
| --- | --- | ---: |
| #118 | capacity_read, then original-pool venue_capacity | 5 +10 |
| #101 | task_registry_due, original Store/Sim/Worker obligations | 8 |
| #120 | opening_venue_store, original same-slot Store publication | 1 |

**24 distinct parent tests pass**, with no failed command or timeout.
The ignored #120 helper is actually parent-invoked four times: distinct
PIDs50311/50312/50313/50314, all exit0, success=true, timed_out=false and
wait_error=null. They cover WAL recovery, checkpoint recovery, post-CURRENT
lost-response recovery and subsequent checkpoint recovery. They are not
four additional test passes. The expected caught #118 interruption panic
remains part of its original passing RecoveryRequired test.

The actual #101 tests preserve mandatory fallback, full binding/clock guards,
P4 acceptance without implicit Advance, idle child reuse, cold running reopen,
cancelled attempts and hard backpressure. They do not prove universal city
scheduling. #120 retains exact genesis money/resources, original receipts,
no duplicate capital and no overwrite; it does not certify an external provider.

Full408-file crate/root source and fixed executable bytes match before/after;
seven external historical literal inputs and four tests/fixtures originals
are also retained in separate ZIPs and compared against canonical bytes.
Actual #101 CLI compiled path and a complete matching copy are fixed; no
later target binary is substituted. All24 unrelated protected WIP remain
byte-identical. Evidence is under
`/Volumes/HP P900/dev/MyCity/work/dependency-chain-regression-20261025/`:
source/fixtures, executable/CLI originals, commands/environment/PIDs/results,
raw logs, event transcripts, original complete worlds and per-file hashes.
Keep originals while recovery dependencies remain; no source or evidence
is removed to fit the256MiB artifact planning allowance.

Actual environment is Mac16,1/32GiB, Rust1.85.1 and existing HP P900 USB/APFS
shared target/work. Build and functional durations are not speed or memory
calibration. Original4MiB Engine, role8MiB/normal64MiB and M0 gates stay
unchanged. No true16GB, GPU, longstress, native peak or full M0 acceptance.
Full #118/#101/#120 remain open with the prior unimplemented provider,
multi-domain, migration and budget scope; no PR, direct evidence publication.

Original #122/#123 are separately repaired and closed in v159, confirmed
again via live GitHub; their complete failure originals remain. New #124
has its separate v178 repair. No finite regression proves all-input stack
safety or silently reopens those defects.
