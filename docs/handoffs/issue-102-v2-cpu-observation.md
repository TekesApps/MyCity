# Issue #102: v2 CPU worker observation candidate

## Boundary and files

This is an unregistered, source-only v2 observation projection. It adds:

- crates/city-core/src/worker_protocol_observation.rs
- crates/city-core/src/worker_protocol_observation_tests.rs
- crates/city-headless/src/worker_protocol_observation.rs
- crates/city-headless/src/worker_protocol_observation_tests.rs
- docs/handoffs/issue-102-v2-cpu-observation.md

The core schema is pure data validation. The headless file is designed as a
private child module of compute_worker and reads its existing Worker, Request,
Event, Stage, and PROTOCOL. Root owns all module inclusion, submit/next wiring,
Sim reporting, and tests in the canonical checkout. No lib root, Cargo file,
worker wire, Worker lifecycle, broker, Store, scheduler, DirectChild, or v1
adapter was modified. Existing r03/v1 source, tests, and handoffs remain intact.

The candidate was drafted in the original private worktree at baseline
037f3cf89755430bd8cc56922d8f673497bdf6c3. Canonical v0.5.161 at
b860c36a9d3747e6c63ff20b96daaa7d3fd1a75f was inspected read-only for the
current Worker/Grant/Lease interfaces. It includes root-owned dirty additions
for Worker owner identity and Lease observation identity that are required by
the later integration.

## Observation contract

The core v2 request identity preserves the exact wire protocol and strategy,
plus every original Air Ticket field: world, branch, job, incarnation,
generation, attempt, base revision, field version, start/end, and rules/source/
input/checkpoint hashes. Hashes are fixed 32-byte values after strict lowercase
hex decoding. Zero base revision and field version remain valid. A bounded
RequestIdentity is retained for one attempt and compared in full for every
observation.

Pool/execution identity distinguishes Unpooled, Unknown, DirectGrant, and
BrokerLease. DirectGrant contains the complete pool boot u128 and grant ID;
BrokerLease carries distinct pool boot, generation, and lease ID fields. Every
u128 is serialized as a canonical bounded decimal string so serde_json::Value
round-trips values such as u128::MAX without truncation. No secret key is
included. Missing process birth-instance remains None; it is not inferred from
PID, a cursor, a diagnostic log, or a nonzero identifier. The owner instance
comes from the actual Worker owner. Pending or inconsistent pool ownership
projects as Unknown, never as Unpooled.

The headless Attempt stores the full original Request and one latest
CpuObservation. It verifies protocol, full Event Ticket, actual PID, owner
instance, and current pool identity. Call observe_after_next only after the
existing Worker::next identity and candidate read/hash/size checks. Published
and Complete then mean only host-read/hash-validated candidate evidence;
there is no Store-accepted state or acceptance flag. A new submit must create a
new Attempt and replace the previous observation; start replacement must clear
it. If a completed observation is retained after retirement, Sim output must
label it historical/previous-attempt data rather than a current active owner.
It cannot postpone or suppress required business consequences.

Measurements have explicit meaning, unit, provenance, interval, and Known or
Unknown value. The current worker wire and owner provide no actual process
birth-instance, CPU time, elapsed interval, completed-work counter, or
per-attempt scratch peak, so this projection emits Unknown for those metrics.
Each fixed slot is validated against its required meaning and unit; swapping
otherwise-valid measurement objects between slots is rejected. Known completed
work and scratch peak values are prohibited until real collectors exist. A
wall-clock interval cannot stand in for work or scratch. Cursor is only a worker
claim, not work units. Capacity is not attempt scratch.
The schema can represent host monotonic submit-to-receive elapsed time with an
explicit meaning and matching interval, but this candidate does not synthesize
or attach such a measurement. It is not CPU time, queue age, or kernel runtime.

The 8 MiB bound is only an observation/path acceptance ceiling for a candidate
file. It does not grant memory, change the existing air wire, alter frozen
standards, or amend admission policy.

## Source tests and limits

Core test source covers full Ticket field changes, zero base/field values,
full-width u128 serde_json::Value round-trip and invalid decimal forms,
Unpooled versus Unknown and DirectGrant versus BrokerLease distinction, stale
process rejection, exact 8 MiB versus over-limit candidate, and Unknown versus
measured-zero values, measurement-slot swaps, and rejection of wall-clock values
masquerading as work or scratch. A structurally valid Known CPU or elapsed value
only passes schema validation; it is not evidence that a real collector ran.
Headless tests cover exact Request projection,
candidate origin, cursor provenance, bound behavior, and Unknown metrics.
Fixtures are structure examples only; they do not assert a real process, grant,
wait, capacity return, or Store permission.

No compile, test, rustfmt, script, benchmark, pressure, GPU, process, or network
work was performed. All checks are not_run. Source tests are not pass evidence.
The API remains a proposal until root wires it into the canonical Worker path
and exposes an explicitly attempt-scoped observation.
