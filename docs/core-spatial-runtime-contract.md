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
