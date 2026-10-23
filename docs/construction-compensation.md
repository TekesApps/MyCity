# Construction compensation contract (0.1)

## Scope

Compensation is an optional fixed payment attached to exactly one gate in the project's initial application review. The contract records the gate ID, payee account, application round, and positive amount. It is configured with the opening Project; a player action cannot create or edit it. Projects without this contract keep the previous evidence-only gate behavior and omit the optional compensation fields from their serialized form.

This is a narrow payment rule. It does not model negotiation, appraisals, court proceedings, escrow, contingent claims, or a general compensation market.

## Authority and approval

The project owner files the application as `Caller::Account(owner)`. System callers and other accounts cannot file a contracted application. The bound compensation gate can be accepted only by its recorded payee account. The initial review must contain exactly one Compensation gate, with the contract's gate ID and payee. A mismatched gate, a second Compensation gate, or a direct projection-only acceptance without the Engine-recorded payment is rejected.

The transfer and gate decision are one Engine candidate transaction. The owner pays the contract amount to the payee from the owner's available cash; the transfer uses the existing cash-protection and account-balance rules. If authorization, available funds, protection, budget, or recipient-account validation fails, the transaction publishes neither the transfer nor the accepted gate. A retry with the same operation ID replays its recorded result; a different request cannot reuse that ID.

## Accounting and budget

The contract amount must be positive and no greater than the Project budget. At settlement, checked arithmetic enforces `spent + compensation <= budget`. The amount is added once to `Project.spent`, and the actual successful Envelope operation ID is retained with the contract. Construction materials, labor, utility charges, and later disposal remain separately accounted by their existing paths.

There is no escrow. Payment occurs when the payee accepts the bound initial gate. If another gate later rejects the application, or a later demolition review is rejected, the already-settled payment is not automatically refunded. The spent amount and payment receipt remain in the Project history. No new claim or collection process is implied.

## Demolition review and historical identity

The initial application has review round 1. The core transition supports a later demolition review with a different gate set, including new IDs or categories. The current compact Engine `BeginDemolition` command still reopens the Project's configured gate array with fresh decisions; it does not yet let a caller submit arbitrary replacement gates. In either case, the old payment contract remains historical: later gates do not need to contain its original ID or a Compensation gate. The old payee account must still resolve when the World is assembled or restored. A later gate acceptance never invokes the initial payment path and cannot pay the contract a second time.

A later Review state must have the persisted context produced by `BeginDemolition`: a configured waste receiver matching the request, a valid waste resource and nonzero contracted waste capacity, the occupied site lease, no pending effect, and a reachable previous phase. A settled contracted Project cannot return to Draft. Rejected demolition reviews may return to the prior operating or construction phase and be filed again; each refile advances the review round without changing or repaying the original contract.

The existing demolition workflow remains responsible for authorization, work, waste acceptance, capacity withdrawal, and final site release. Compensation does not bypass those steps or release a site.

## Persistence and recovery

Unbound legacy Projects serialize neither `compensation` nor the zero `review_round` field. An unpaid contract omits its zero settlement operation ID. These defaults preserve the old optional-field shape. Bound Projects persist the contract and settlement receipt in the existing Project/World checkpoint; restore validates the Project invariants and resolves the historical payee against the World account table even when demolition has replaced the initial gates.

The Engine's existing journal and Store recovery remain authoritative. A lost response is retried with the same Envelope; checkpoint or WAL recovery must not issue the transfer again. A forged checkpoint that changes the historic payee to a missing account, relabels a paid initial Review as demolition Review without a demolition context, or moves a paid contracted Project back to Draft is rejected.

## Bounds and current evidence

The contract is one fixed payment per Project, one initial Compensation gate, and a bounded scalar record; it adds no per-party or per-resident state. The existing 2 KiB Project state budget is unchanged. This rule does not estimate fair compensation, determine who is legally entitled to payment, or simulate the negotiation that led to the amount.

The frozen candidate's Engine, Store, strict Clippy, formatting, headless library tests, and seven joint CLI results are recorded in [the validation status](evidence/core-construction-compensation-135/STATUS.md). The independent receipt checks one owner-to-payee transfer of 50,000, conserved money, the persisted payment marker and `spent` amount, and exactly one successful receipt for operation `700006`. The CLI outputs matched the earlier release v0.5.133 baseline byte-for-byte. These are fixed-fixture functional results, not calibrated compensation policy or full-city economic balance.

The accompanying M0 residual attribution is a separate narrow diagnostic: it credits no bytes and establishes no memory, performance, or 16 GiB acceptance pass. No test result implies a real-world legal negotiation, appraisal, escrow, or refund mechanism.
