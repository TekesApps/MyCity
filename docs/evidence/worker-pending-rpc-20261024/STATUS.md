# #102 pending Prepare and original RPC finite gate

Root integrated an original PendingGrant before any Prepare RPC. The original request,
Ready/full u128 boot, parent process identity and secret key survive resolve errors; success
transfers the original Grant once. No second pool, wait proof, synthetic credit or changed
64MiB normal / 8MiB common budget is introduced. The first gate below was API-only;
the subsequent stage08 migrates the actual callers and validates finite CPU recovery.

## Actual validation

- API `cargo check -p mycity-headless --lib --locked --offline`: passed. Initial command
  used the wrong package name (`city-headless`), failed before compilation and is retained.
- Serial release test build, locked/offline, existing shared target: passed.
- Fixed test executable, `normal_pool::pending_tests --test-threads=1 --nocapture`:
  **4 passed, 0 failed, 0 ignored**, 104 unrelated tests filtered out.
- Same Prepare after lost reply recovered the original issued id, no second grant;
  original unspawned Abort replay returned once.
- Attach lost reply retained the actual live shell Child and charged 5MiB; actual wait
  preceded original Reap. Shell is a transport fixture, not real air fulfilment.
- Before-Reap explicit rejection made no mutation. This is not a dropped socket request.
- Successful Reap with lost reply left server returns=1 and local acknowledgement=false;
  same original Grant and actual cached exit retried without double credit.
- Independent report/phase checks: all four final grants=returns=1, available=67108864,
  granted=0; distinguishing pre/post-action rows matched. Source files unchanged after run.

Original advisor approved concrete API and test source before execution, then independently
reviewed actual results and accepted this finite gate without a new blocker. Result review is
recorded in the retained package; this gate does not establish complete #102 acceptance.
Duplicate after_wait calls after local acknowledgement can short-circuit locally; only the
lost-Reap-reply retry proves an additional real same-capability RPC in that scenario.

## Source, ownership and evidence

Source SHA: `9aa34dfeea90c7773feea70e5e653678c6a86ec67dea8b2d8ab1e22a01b34dd1`.
Full 343-file source archive, hashes, initial shared originals, owner test originals,
build logs, fixed executable, commands and independent checks:
`/Volumes/HP P900/dev/MyCity/work/issue102-integration-20261024/`.
`run-01.json` lists the four unique short `work/p102-*` roots, each retaining actual
ready/events/report/fixture commands. No data directories or recovery originals deleted.
Root package is about 4.8MB logical so far, below its 256MiB planning cap. Small RPC roots
are below their aggregate 16MiB log planning cap; these are not measured native budgets.

Actual host: Mac16,1, 32GiB; external HP P900 USB/APFS shared target/work. No true16GiB,
performance, GPU, long pressure or formal M0 acceptance claim. Four services were sequential
threads in one test process. Their Ready boot is that process instance, not four independently
restarted hosts. Cross-restart recovery was not tested. Failed-owner RETAINED slot lasts only
while the test process lives; it is not persisted capability recovery.

Root owns shared normal_pool/compute_worker/Sim/broker; existing issue owner supplied only
the private test file and handoff. Private r03 protocol adapter remains source-only.

## Dependency gate after the initial API-only result

Migrate all real spawn callers to a persistent owner slot before claiming #102 activation.
Retain original PendingGrant before RPC, original Child/Grant immediately after spawn,
and original broker Lease before accept/clone/reader setup. Nonempty/unconfirmed state blocks
new spawn/Rebind. Clear only after real wait plus original Reap acknowledgement, or original
unspawned Abort acknowledgement. Broker unconfirmed Start must retain session/endpoint;
original supervisor wait/revoke handles terminal recovery without a second local refund.
Then validate actual Worker/Sim/broker fault paths before #103/#104 activation. #101 remains
advisory; business pending and receipts retain original Store authority. No PR, commit/tag or
issue closure is claimed by this finite gate; protected mixed WIP release association pending.

Implementation, actual tests, advisor conclusion, unverified items and no-PR status were
backfilled to [#102 comment](https://github.com/TekesApps/MyCity/issues/102#issuecomment-6379140114).
GitHub connector write returned403; authenticated gh fallback succeeded and its response
and independent comment readback are retained. No issue state changed.

## Stage08: persistent actual Worker/Sim/broker ownership

Root replaced all spawn-returning-String constructors with an empty persistent Worker plus
`start_*(&mut self)`. Original PendingGrant precedes Prepare RPC; actual Child/Grant precede
Attach/pipes/readers. Broker PendingStart owns the original request/listener before RPC;
Started Lease enters the same Worker before accept/clone/reader setup. Nonempty owners reject
new start. Sim retires before Rebind/cache reset. No failed-owner take() remains there.

Real wait is cached before Reap. Only original Reap acknowledgement, broker's original waited
receipt, or original unspawned Abort acknowledgement permits Empty. A failed kill is not a
wait: recheck actual exit or retain the owner and return an error. Cancel/EOF/local Empty do
not create resource credit. `with_worker_on` keeps an externally held owner across normal
errors and caught unwind; it retries retirement finitely, then resumes the original panic.
`with_worker` is the outer session/probe wrapper. Drop is best effort, not reliable recovery.
Persistent final failure reports unrecovered responsibility; an independent direct probe
has no invented external recovery owner. Forced abort/SIGKILL/OOM remain external contracts.

Original advisor identified one concrete integration blocker: Start-unconfirmed initially
returned an ordinary result but Sim kept reading commands. Root fixed this with structured
`requires_session_exit()`: after sending that result, Sim exits immediately. No blind Start
retry, fake Lease or Sim-local refund is allowed. Original supervisor must actually wait Sim
then revoke; broker alone owns/waits the worker and returns its capacity. Narrow source
review accepted the fix; actual no-second-command exit is covered below.

### Actual serialized results

Frozen source SHA: `be142b7b957e48acedbe7fa52f44eed13400b83dccfe6a9d44349e5afd57a0ca`.
Serial release build passed. **37 selected tests passed, all exit0**:

| Selected group | Actual result | Meaning |
|---|---:|---|
| Original RPC4 plus persistent Worker4 and unwind1 | 9/9 | Same pending/Child/Grant and real cached exit survive faults; repeated start blocked; unwind retries one rejected Reap then resumes the same panic |
| New broker Started/Stopped lost-reply cases | 2/2 | No Stop/kill/second command needed for unconfirmed Start exit; original supervisor wait/revoke and cold retry; Stop lost reply keeps original receipt for cold replay |
| Existing compute_worker regression | 10/10 | Actual numerical candidate verification, freight/Store/receipt path, EOF/kill and cache ownership retained |
| Existing normal_pool protocol regression | 6/6 | Original64MiB/5MiB ownership, actual wait, explicit failed-spawn Abort, identity/wire and fatal fail-closed cases |
| Existing broker regression | 8/8 | Original wait/revoke, killed/paused workers, retry, lost bind/revoke and intentional fatal paths |
| #101 actual dispatch regression | 2/2 | Required/cancel/retry/cold receipt observation remains correct; still advisory |

The single `mapped_pool_unmaps_before_grant_and_rebuilds_only_after_owned_wait` native observer
case was explicitly filtered out, not passed. No formal M0, long pressure, GPU, physical peak
or true16GiB acceptance ran. Unwind's printed panic is intentional and is caught/asserted;
it is not a test failure. First all-targets check found old constructor calls in tests; root
migrated those calls, preserved assertions and added explicit owner/Abort assertions. All
subsequent all-targets checks passed. Failed check logs remain unchanged.

Root independently read nine original thread-pool reports: each grants=returns=1, available
67108864, granted=0. Fifteen actual broker fixtures contain thirteen balanced reports. The
two without reports are deliberate broker-death (`after-broker-spawn-pause`) and partial
workspace-return (`workspace-return-page-2`) failures; neither is claimed balanced. The new
Start fixture records returns=0 before supervisor wait and full return after that actual wait.
Cold world/receipt recovery assertions use the original Store path, not reconstructed facts.

Original advisor reviewed the actual stage records/logs/reports and accepted this finite CPU
lifecycle gate without a new blocker. Scope is not complete #102: private r03 schema/report
binding remains source-only; actual work/scratch counters, its2MiB-versus8MiB compatibility,
GPU/fences and #103/#104 control remain pending. No arbitrary counters activate the protocol.

### Retained evidence and next work

Same HP P900 package contains `effective-source-08.zip` (343 full files), per-file manifest,
fixed CLI and five fixed test executables in `bin-08`, build/check logs, full execution
commands/exit records and independent reports. `artifacts-08.json` lists the nine short
`work/p102-*` roots and four short `work/p108-*` regression evidence roots. Full worlds,
genesis/receipts/candidates/transcripts and intentional failure originals remain there.
Fixtures use explicit retain-root variables, and process tests accept the fixed CLI override
so later target rebuilds cannot silently replace their source identity. All source and fixed
binary hashes remained unchanged during the serialized runs.

Logical evidence size at verification: 83,939,322 bytes including the listed external fixture
files; below256MiB planning retention. This is neither native memory nor hard disk quota nor
APFS released space. No evidence/source/tree/cache deletion or release publication occurred.
Next: coherent commit/tag/version association audit of protected mixed WIP, then actual r03
identity/report integration before strategy/controller activation. Full issues stay open.

### Supplementary test input provenance

The build-script SOURCE digest covers343 files in total: crates/fixtures and four root
build files. It does not cover the repository-root `tests/fixtures/issue-101` inputs.
Complete bytes for those three additional files are retained in
`ancillary-root-test-fixtures-08.zip` with hashes. The198-byte `air-empty-sources-30.json`
used by actual dispatch tests was independently found verbatim in the fixed dispatch test
executable. The other two fixtures are inputs to different registry/adapter tests, not claimed
embedded in that executable. Release/read-set assembly must include these dependencies;
SOURCE alone is not a whole-repository or all-test-input identity. No game code or measured
build script was changed to retrofit that claim.

Stage08 implementation, advisor result and unverified scope were
[backfilled to #102](https://github.com/TekesApps/MyCity/issues/102#issuecomment-6379366621),
and the actual advisory regression was
[backfilled to #101](https://github.com/TekesApps/MyCity/issues/101#issuecomment-6379368301).
Both exact bodies were independently read back. No full issue closed.
