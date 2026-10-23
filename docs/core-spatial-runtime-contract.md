# F runtime computation contract (candidate)

This contract records the finite executable boundary. Run 237 actually passed the 28 storage
library tests and 5 spatial Store tests; standalone and joint CLIs are still pending at this
revision. Only Rust model and CLI are in scope.

## Authority and causal output

Immutable genesis binds the validated Scene, exact raw allocation bytes/hash, explicit asset
owner/permit accounts, managed edges and allowed close/restore operations. A request binds
its parent global profile hash, asset revision, effective service day and exact candidate hash.
Owner consent and administrative permit are independent authenticated Engine events. A staged
file or caller-supplied evidence ID does not authorize publication.

The same Engine candidate commits permission changes, operation receipt and Store root catalog
pointer. The sidecar chain retains the request and approvals required to replay each applied
version from genesis. Recovery must compare all immutable authority fields and derived edge
states with the Engine state, not merely version count or final hash. Failed or uncertain writes
retain originals; reopen the same world and replay the same operation before retrying.

The selected service-day profile is the authoritative input to actual transport assignment,
proof and TownSettle capacity/fees. Its output is served/unserved trips, realized sales and
actual account/inventory changes. Old proof verification selects its original version. A new
profile cannot retroactively reinterpret an old settlement.

## Events, stable intervals and batching

Events are request/approval/veto/withdraw/apply, service-day settlement, durable checkpoint and
recovery. Geometry and permissions remain stable between authorized updates. Reuse immutable
verified input where safe; no per-building daily geometry simulation is required. Existing
assignment remains the reproducible deterministic reference path and is recomputed on changed
inputs. This does not claim that an incremental route cache has been implemented.

Batch only immutable reads or independent validation without changing event ordering. Do not
collapse Apply across a service-day boundary, reuse a proof from another profile, bypass either
approval, or multiply one day's trade across inventory depletion, capacity thresholds, wages,
tax rounding or changed demand. Transport/market integer and identity invariants remain exact.
Game speed changes wall-clock pacing, not the ordered city events or calendar quantities.

## Resource ownership and limits

Engine owns active permission state; the retained preparation slot and restore slot own their
separate bounded backing. Account the simultaneously live buffers, inner edge vectors and
parser scratch, including failed decode/retry. CommitContext borrows the candidate and does
not own another state copy. Final retained owned bytes alone are not a peak measurement.

Store owns immutable allocation files, catalog versions and proofs. The prototype admits at
most 8 assets, 8 active requests, 16 approvals, 8 edges per asset and 128 historical versions,
subject to the existing encoded blob/WAL/disk admission limits. Include stage/orphan files,
full catalog rewrite amplification and old recoverable roots in disk accounting. Full tables
must return observable backpressure without consuming approvals, cash or the city event.
The lifetime history cap is an explicit prototype limitation, not evidence of unlimited play.

## Verification and limits

Reference fixtures retain finite funded stock: five-day F sales are expected to be 40/0/40/0/20
from 100 initial units; the final balance must be zero. A separate single-Store D+F candidate
joins funding, wages, roads, trade, source-backed costs, tax and dividends. Both need actual
receipts, negative cases, exact replay and full recovery originals before acceptance. Run 237
completed those F Store cases, including the three permission mutations with recalculated
outer hashes and 1,025 actual commits spanning two automatic 512-row hot windows. It does
not demonstrate 1,024 simultaneously resident Store receipts; rotation remains at 512.

Current adapter scope is ground-level WALK and a bounded managed edge toggle. Bridges/tunnels,
other modes, geometry relocation, widening and construction-driven capacity change remain
explicit unsupported operations. This contract does not certify whole-city throughput, the
original M0 gates, an eight-hour run or real-16GB hardware; those remain independent tests.

The compact Engine hot journal retains 1,024 entries and at most 16 action identities per
entry. Full actions remain in durable WAL/checkpoint/archive receipts. Store rehydrates them
into its existing fixed receipt backing and hashes serialized envelopes directly into SHA-256,
without an additional complete encoded JSON Vec. Legacy WAL bytes and maximum-action hash
equivalence were tested in 237. Resident Engine owned bytes and Store normal owned capacity
are separate from native process footprint, transient allocation high-water marks and common
8 MiB diagnostics; these results cannot certify the latter gates.


## Next geometry activation seam (private implementation, not accepted)

A geometric Scene update must not reinterpret `current_profile_sha256` as a generic target hash.
Existing profile identity, Close/Restore request digest, grants and old recovery bytes retain
their meanings. A separate compact geometry selector refers to an immutable bounded descriptor:
world/branch/asset, parent Scene SHA and geometry revision, parent profile SHA and transport
revision, candidate Scene/profile SHAs and their next revisions. Approval uses a distinct domain
and includes the descriptor SHA with existing actor/role context. Two different Scenes that
happen to produce the same transport profile cannot reuse each other's approval.

Store must bound/read/hash the complete descriptor and referenced originals, derive the candidate
from the active Scene/profile, and admit only the authorized geometry delta before the Engine
receives verified facts. Apply publishes Scene and profile identities atomically with the existing
receipt/WAL/root flow. Old service-day proofs resolve against their original complete Scene/profile
witnesses. Stale geometry parent, even with a correct profile parent, is rejected. Missing or changed
originals fail closed; hashes alone are not recoverable geometry witnesses.

The first pending slice is one supported WALK Road control-point change with unchanged endpoints,
permissions, capacity, OD and speed. Door relocation, widening and multi-mode geometry remain
separate scopes. The independent proposed fixture distinguishes71→72 seconds one-way and142→144
round trip; using an explicitly labeled10c/s tariff makes40Q freight differ by80c. These are
candidate integer expectations, not actual Store acceptance. Real assignment, trade, version history,
lost reply/cold replay, stale approval and descriptor-tamper counterexamples must run before passing.

New optional request/descriptor/version backing must be bounded, count headers and actual capacity,
and preserve legacy absent-state cost/identity. Action-size and combined Engine/World/Store ownership
and transient restore cost need actual verification; existing M0 and16GB budgets do not automatically
cover new Scene history. Root integrates only exact fresh-baseline deltas from the sole core writer.


Geometry derivation also requires a persisted immutable adapter basis (speed, capacity,
direction and other conversion settings). A rounded edge time cannot recover its original
speed uniquely. The new explicit genesis path must retain bounded complete adapter input/config
and its hash, rederive the genesis profile and compare it with the supplied profile. Catalog/root
and geometry approval descriptor bind this basis hash. Every subsequent geometry candidate uses
the same basis with the current Scene. A100cm/s test fixture is explicit input, not an implicit
city-wide default. Existing profile-only worlds remain readable and keep Close/Restore support,
but Geometry requests without this trustworthy basis reject; migration requires a separate
explicit provenance design. Include basis original/decode/restore bytes in resource accounting.
