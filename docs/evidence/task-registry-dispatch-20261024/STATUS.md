# #101 real Sim registry observation — 2026-10-24

Original advisor accepted shared hook and frozen tests. Release compilation succeeded;
fixed CLI/test executables and exact332-file build-script read-set were archived before
execution. Both real-process tests passed in1.12s; actual worker tickets all bind source
`9754108afa7c82c606406663a742ec04e8a3692014fc5fafbfd99ba072e52f35`. CLI and test bytes remained unchanged throughout execution.

The hook observes actual pending air Job before RebindLease using World scope and Timeline
progress.at; missing clock is Unknown, never fabricated zero. After rebind it rereads actual
execution identity. A bounded registry summary is added to existing worker_started payload.
No new event, clock, worker, queue or error propagation path is introduced. Receipt replay,
original hard gates, Ticket/import/ACK/Accept/commit and cancellation retain their authority.
Neither Candidate, NoHint nor Unknown controls mandatory dispatch. This is real advisory
wiring, not a new dynamic/multi-model scheduler or CPU/GPU budget controller.

Actual tests prove zero-source positive interval dispatch/commit, post-rebind
NoHint(AlreadyRunning) without losing dispatch, matching rebound/Ticket identities, WorkerCells
cancellation leaving P4/job/window/due intact, fresh generation/attempt/incarnation retry,
and cold-process receipt replay without new worker or sequence increment. The independently
literal30-second empty Input identity matches real tickets. Current valid protocol cannot
inject a missing-clock pending world or arbitrary old candidate; those cases remain uncovered.
Pure adapter matching tests are not substituted for real candidate-injection evidence.

3 worker starts were retained across two test cases, including the cancelled attempt.
Two append-only transcripts and complete worlds remain under rootrun-01: 4951875 logical bytes.
Transcripts are limited to512records/8MiB per case,256KiB per serialized record; these checks
occur after parsing/serialization and do not establish parser/native peaks. Added source binding,
registry allocations and payload output have real CPU/memory costs, unmeasured against formal
M0 gates. Original normal64MiB/common8MiB/speed/load standards are unchanged. No performance,
GPU, long stress or true16GB pass; actual host remains32GiB.

Evidence: /Volumes/HP P900/dev/MyCity/work/parallel-issue101-dispatch-20261024/ including original
shared files, owner originals, effective-source.zip/manifest, executables-before.json,
build/test logs and result-01.json. Existing HP P900 target/work were used serially. Keep complete
originals until recovery/integration dependencies release and a newer accepted baseline exists.
No cleanup, PR/commit/tag/push or full Issue closure. The unused result_matches_pending library
warning is retained; this hook deliberately does not turn that helper into commit authority.
# Later CPU-lifecycle regression (2026-10-24)

After root's persistent Worker/Sim/broker ownership integration, these original actual
dispatch tests passed again2/2 on frozen source
`be142b7b957e48acedbe7fa52f44eed13400b83dccfe6a9d44349e5afd57a0ca`.
See [Stage08](../worker-pending-rpc-20261024/STATUS.md#stage08-persistent-actual-workersimbroker-ownership)
for fixed binaries, original worlds/transcripts and actual advisor result review. The
historical evidence below remains tied to its own earlier fixed source. Observation is
still advisory, not a new scheduler/grant authority or complete #101/16GiB/M0 acceptance.
