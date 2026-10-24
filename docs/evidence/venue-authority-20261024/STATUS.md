# #118 original-pool managed venue authority


Current-state correction (v0.5.171): live #122/#123 are closed after the v0.5.159 repair and validation (commit `1bee5b4c9ec49ebd41e0b555fb8bcbe7e8efaca5`). Any later “open” wording below was stale bookkeeping, not a newly observed failure. Historical failures and complete recovery originals remain retained; actual results in this report are unchanged.
This phase extends v0.5.167 with an explicit optional `managed_people_v1` profile.
Full #118 remains open. The original Resource/Lease pool alone holds capacity;
venue registration is immutable provenance, and use metadata is not a second pool.
Root alone integrated shared state and scheduled every compiler/test process.

## Authority, activation and recovery

The initial unit is simultaneous people present, including learners, teachers, staff and
visitors; enrollment is independent. Venue purposes1/2 are supported. Original immutable
Housing genesis supplies a bounded sorted household population namespace, not per-person
resident objects, live attendance or inferred location. Prefix ranges and exact source IDs
are bound to the original genesis through a population fingerprint. Same-group partitions
must agree and cannot be actually present at two venues; different groups cannot overlap.
Aliases, registered limits, purpose, owner/operator/permit and original resource references
are checked. Boot/exchange/construction resources cannot be reinterpreted as venue people.
Staged internal metadata and final token/counter bijections are validated before commit.

One compact original Action selects one immutable use offer. Operator acceptance reserves
original capacity immediately; conservative reservation refusal is not physical shortage.
Only the trusted System game host supplies raw actual enter/safe-exit facts, at the original
completed Timeline tick. These raw facts are not authenticated transport/teaching/service
provider evidence. Whole-group entry is atomic; no partial entry support is claimed.
Cancellation is original actor/System only. Actual responsibility remains after the window,
closure intent, cold restore and inactive venue tasks until explicit actual safe exit.
`venue_exit_due` derives due responsibility from the persisted offer and original clock;
no second scheduler, automatic exit, expiry release or reset of obligations is introduced.

Raw activity IDs and full original transaction sources remain in bounded tombstones;
retry under a new operation cannot reuse an old activity. Original Store prepare/WAL/commit
and durable receipt identity remain authoritative, including lost-response replay.
Checkpoint receipt decoding explicitly handles Venue through the existing bounded typed
reader; its raw duplicate-key/overflow/sequence parity probes remain intact. A byte-exact
copy of the previously failing checkpoint is a regression fixture; the failed original
world remains separately recoverable. Reused EngineSlot restores clear the optional venue
backing when legacy input omits it. This is finite old-format compatibility, not a general
migration, dynamic permit/population change or live upstream provider certification.

## Explicit budget partition

The initial combined128KiB World admission really failed. It is retained as a failed
configuration, not fixed by dropping counters or reporting allocation as free. The new
optional profile keeps base World+Housing+original Housing validation scratch <=128KiB;
its separately named venue backing is still charged to full World/Engine and preparation.
Original Engine4MiB, role8MiB and normal64MiB limits are unchanged. Full Engine validation
and projected dynamic preparation gates continue to count venue bytes. No-venue legacy
input keeps the original base budget behavior. New profile is a configuration contract,
not evidence that the old joint128KiB configuration passed.

Actual ABI: base World+Housing119312B, venue51040B, total World170352B;
Engine3670608B <=4194304B, Store41221767B <=its original64MiB bound. Venue header224B,
immutable directory backing23680B, policy512B, use15872B and activity10752B sum to51040B.
Dynamic preparation payload26624B plus64B Row headers is additional; Box handle8B is
separate. Opening decode header96B and23680B catalog backing are reported, not omitted.
The decode catalog transfers into Runtime; it is not a second permanently live directory.
Active/restore roots and original EngineWorkspace stay distinct, not shared-page savings.
Ordinary venue Store cases run unpooled; Store own64MiB is not the process common8MiB acceptance gate.
Logical owned capacities are not allocator/native/stack/RSS or a physical peak. Active,
restore and temporary workspaces require distinct accounting; future total16GB support,
GPU, concurrency/speed and original M0 acceptance remain unmeasured here.

## Failures and immutable evidence

Preserve build05 optional-backing Copy failure, build06 owner test-helper shadowing,
diagnostic07 base-budget rejection (two invalid-input results were masked), pure09 hand-
computed JSON-bound literal failure, diagnostic10 archived Venue receipt `invalid_json`,
and check11 const-generic parity-harness type failure, plus the run14 missing-root runner failure. Original authors repaired their
pure test source; root repaired shared integration. Original advisor narrowly reviewed
those causal fixes without executing tests. Failed worlds, exact fixture originals,
fixed source ZIPs, manifests, compiler logs and executable copies remain on HP P900.
Current successful run roots are `v118run14-76501` and `v118run15-77150` under HP work:
1170 regular files/101265483 logical bytes retained, special entries listed separately.
Some legacy tests perform their own successful temporary cleanup; this inventory describes
remaining originals, not a claim that every temporary file from every legacy test survives.
The original diagnostic10 lifecycle world is separately retained, not overwritten by fixture tests.

Canonical source13 is390 files, SHA256
`b34682dd8f3aac091a6ad12775a00de99b9bf93e9b030d67af1c2f92757ba8cc`.
Workspace/all-target release check13 and selected release no-run build13 pass. Eight
successful run14 groups pass58 tests: receipt decoder2, pure venue10, actual venue10,
capacity read5, original resource2, construction core10/Store4 and recovery15. Run14 then
stopped because root omitted MYCITY_OPENING_INPUT_STORE_ROOT: six opening tests failed
before constructing worlds. Preserve this runner failure. Only the remaining groups were
rerun with the required unique external root, using the SAME fixed executables/source:
run15 opening6, housing2, actual air control13, compute13, broker23, Sim6 and registry2
all pass65. Total15 successful groups/123 distinct tests; three ignored subprocess helpers
are invoked by parent tests and not counted again. Source/ancillary fixtures/executable
bytes match before/after. Original v106 recovery is covered by construction Store tests.
Original advisor accepts this finite source/result gate with no new blocker; review was read-only and did not rerun tests.
Mac16,1/32GiB; Rust1.85.1 release locked/offline, serial `-j1`, existing HP P900 USB/APFS
target/work. Functional wall durations are not a performance measurement. Two existing
debug-stack faults (#122/#123) remain separate; no RUST_MIN_STACK override or M0 change.

Full retained package is `/Volumes/HP P900/dev/MyCity/work/issue118-venue-authority-20261024`.
The512MiB artifact planning cap is not a runtime grant; original recovery evidence cannot
be replaced by hashes. All24 unrelated observer/clock/rare-control WIP files stay intact.
No PR: direct root integration follows validation and scoped version/tag publication.
Next gates are real #101 scheduling binding, #120 venue-aware same-slot Store publication/
recovery, then #54/#57 truthful capacity/activity adapters; full issue closure is not implied.
