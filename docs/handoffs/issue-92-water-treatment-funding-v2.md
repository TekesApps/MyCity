# #92 funding V2 interface for the actual #133 bridge

Date: 2026-10-25. Original owner, canonical `/Users/frankqin/dev/MyCity`.
Related existing Issues: [#92](https://github.com/TekesApps/MyCity/issues/92),
[#133](https://github.com/TekesApps/MyCity/issues/133). This is a **source-only candidate**.
Root owns shared integration, actual original receipts, serial validation, Issue updates,
commit/tag/release and any PR. No new Issue or alternate water source was created.

## Starting source and file ownership

Before editing, owner read actual AGENTS.md and verified `git diff 715758a --` the original
module and tests was empty. Original module SHA256 was
`eac29b08ded2c885b266a6561e627d80b58e3f70ff23ba8fb5259b30d8e60fa7`;
original test SHA256 was
`c2b3bc213ebcf0780bfe7ee0b3351270116290d4b0c186ed4bd8c687d7833507`.
Root reported actual game v0.5.190 `ea310a0` and documentation v0.5.191 `715758a`;
these reports do not establish a new root-bridge acceptance for this patch.

Only these four authorized paths belong to this delivery:

- `crates/city-core/src/water_treatment.rs` (existing kernel)
- `crates/city-core/tests/water_treatment.rs` (existing 24 tests retained, appended tests)
- `tests/fixtures/water-treatment/funding-v2.json` (new small independent arithmetic oracle)
- this new handoff

Original `raw-v1.json` and `issue-92-water-treatment.md` remain unchanged. Shared World,
Engine, assembly, continuous_supply/cost_curve, Stock, Timeline, receipt, Store, headless,
Cargo manifests/lock/lib root, global design/versions/ledgers, 24 observer/M0 WIP and
root's #133 edits were not written. No worktree, branch, target, cleanup or reset.
Root informed the owner that the earlier labor-helper run was terminal and would not
compile concurrently during this source edit. All current compilation belongs to root.

## Concrete blocker and version boundary

V1 computes electricity basis + personnel + residual cost, then requires that sum in
`cash_available_cents`. Actual #133 self-supply instead converts original already funded
fuel/cooling Stock; actual earned wages can already have been paid in the same Prepared.
Requiring the remaining cash to fund both again is incorrect for that new profile.
Inventory basis must not be added to cash, electricity basis must not be set to zero,
and rich-cash fixtures must not conceal the low-cash case.

Global `VERSION=1`, Profile, Config, Checkpoint, RootFacts, Candidate, IdleFacts and their
serde fields remain V1. Their serialized wire and config/checkpoint digest inputs are
unchanged. V1 still checks positive production input, original root verification and the
original full-three-cost cash gate; original idle default-deny behavior is retained.
There is no silent V1 migration or reinterpretation. A private material-arithmetic helper
is shared after version-specific authorization; no funding adapter creates fake V1 tickets,
augments cash, zeroes electricity basis or calls the V1 gate with invented availability.

`FUNDING_VERSION=2` and `FundingProfileV2::ActualFundingV2` introduce an independent
`FundingConfigV2 { version, profile, material:Config, accrual_policy }` wrapper.
Methods: `validate`, `open`, `restore`, `meters`, `prepare`, `revalidate`, `cover_idle`,
`revalidate_idle`. State is `FundingStateV2`, persisted through its explicit
`FundingCheckpointV2 { version, profile, config_digest, physical:Checkpoint, funding }`.
The outer digest binds original material config and explicit accrual policy. An old
Checkpoint cannot deserialize as this new envelope; no default historical coverage is
invented. Root must explicitly register a new wrapped source/opening and authenticate
original source and latest production/idle/coverage receipts on restore.

## Physical facts and complete costs

`FundingFactsV2` contains version, `physical:MaterialFactsV2`, `basis:IntervalBasisV2`,
a fixed `[Option<FundingItemV2>;16]`, real `cash_available_cents` and `funding_revision`.
MaterialFactsV2 carries original binding/from/until/read version, actual remaining raw and
reagent Stock, exact received electricity, next actual StreamBasis, finite three sinks
and source receipt. No fake V1 cost receipt or cash DTO is needed.

The six finite interval cost kinds are electricity input, personnel, equipment operation,
maintenance/wear use, residual acceptance and residual custody. Raw-input and reagent
**actual converted** exact basis come from the existing material result, making eight
reconciliation categories total. Additional raw/reagent integer prefetch stays in existing
input WIP; it is not all expensed as converted production. The existing original raw/
reagent/stream cost rules, all chemical proportions and Exact precision policy are unchanged.
The pure kernel creates no rates, recipient or fee obligation. The root new opening fixes
actual earned/converted rates, scope, purpose, trigger and lawful receiving/custody terms.
Zero in a category means this explicit root contract has no such cost; it does not prove
repair, legal disposal, equipment attendance or free labor.

Each positive `FundingItemV2 { kind, basis:Exact, coverage }` has exactly one coverage:

| Coverage | Meaning | Pure cash check |
| --- | --- | --- |
| OriginalStockConversion | Original already-funded input converted now; raw/reagent/electricity only | none |
| SamePreparedPaid | Original same-Prepared payment already staged and authenticated | none again |
| PriorReceiptPaid | Original prior payment/prepayment actually covers this cost | none again |
| CashDueNow | Still requires original writer's real current payment | sum must fit actual available cash |
| AccruedPayable | Original policy explicitly permits this earned unpaid obligation | two explicit policy/authority gates |

Raw/reagent converted basis must use OriginalStockConversion. Non-stock payments and
accruals must be whole cents; fractional source entitlement remains exact input/curve WIP,
not a new subcent cash payment or debt. A category's coverage sum must equal its full
expected basis, neither missing nor extra. Array entries must be contiguous, nonzero and
bounded; it is a single candidate, not a growing invoice table.

`FundingTotalsV2` contains complete `total_basis`, `original_stock`, `same_prepared_paid`,
`prior_receipt_paid`, `cash_due_now`, `accrued_payable`, all Exact. The five mutually
exclusive coverage totals must sum to full basis. Full basis includes actual raw/reagent
conversion and **all six** interval categories, including non-cash electricity and paid wages.
Only CashDueNow is compared to actual uncommitted cash, after other real payments and
reservations. Rich stock or paid invoices cannot cover a cash fee without an explicit,
real permitted funding source. Pure code never deducts money or issues receipts.

`FundingDeltaV2 { materials:MaterialDeltaV2, costs:FundingTotalsV2 }` deliberately has no
ambiguous V1 `root_interval_cost_cents` field. MaterialDeltaV2 retains exact inputs,
three compositions, original new take amounts/basis and converted raw/reagent basis.
`FundingCandidateV2` binds version/profile/base digest/facts/request/next/delta.
Revalidation uses fresh facts and fully recomputes the result before root's original
Prepared is allowed to commit any Stock, cost, actual work or payment.

## Evidence spans, authority and no duplicate funding

Every coverage carries `FundingEvidenceV2 { source_ref, source_revision, from, until,
purpose_ref, allocation_ref, cost_from, cost_until }`. The source is an original receipt/
Envelope; allocation_ref identifies its immutable specific actual input/payment allocation.
`from/until` bind the **current application interval**. Actual payment/withdrawal history
may be earlier; root separately authenticates that original history and remaining WIP/right.
`[cost_from,cost_until)` is the exact cumulative cents span of the original allocation;
its length must equal item basis.

One original P5 Envelope can legitimately have several purposes and allocations. The
module rejects **overlapping spans on the same source_ref + allocation_ref**, independent
of purpose or source-revision changes; it does not ban every repeated operation ID.
Disjoint spans and remaining original ceil-prefetch WIP may be converted later without
another input take or another payment. Local overlap checking is bounded to at most
120 pairs; no receipt pool, scheduling service or global funding wallet is added.

This local check does not prove that a mathematical span was funded, that a newly named
allocation is genuine, that another allocation does not duplicate its underlying input,
or that an earlier accepted candidate has not already consumed it. Root must authenticate
original source/owner/payer/purpose/amount/interval, fixed production/wage/fee curves,
actual take/paid status, root epoch/read revision/CAS, original remaining allocation budgets
and all prior claims in its actual Stock/WIP/journal. New IDs, nonzero digests and a
`covered=true` equivalent cannot replace those checks. Keeping this cross-receipt registry
in the original writer is deliberate; the pure kernel must not create a second authority.

`RootVerifier::verifies_funding_v2` defaults to false. Existing V1 production or idle
verifiers do not authorize any new V2 coverage. `accrual_policy=None` forbids accrual.
An explicit nonzero original policy/version must also match each accrual item, and
`RootVerifier::verifies_accrual_v2` separately defaults to false. Even an implementation
accepting ordinary V2 funding cannot implicitly grant debt. These gates confer no new
credit policy; root verifies the exact pre-existing permission and records any real debt.

Coverage totals in a saved checkpoint are **historical production-cost classifications**,
not a wallet, current paid status, spendable cash or current outstanding-debt balance.
Later payment updates only root's original settlement/paid/household/cash/debt receipts.
It must not add the earned cost to production again, reprice old water, mint another
coverage allocation or alter the already recorded historical accrued production cost.
The kernel offers no late-payment command. Actual permitted late-pay and original
Envelope replay/exactly-once acceptance remain root's real integration tests.

## Funding V2 idle and persistence

`FundingIdleFactsV2 { version, material:IdleFacts, funding_revision }` and
`FundingIdleCandidateV2 { version, profile, base_digest, facts, next, delta }` are distinct.
`RootVerifier::verifies_funding_idle_v2` independently defaults to false: neither the old
V1 idle verifier nor a new funding-production verifier implicitly grants this capability.
Root must prove the entire original [from,until) Timeline interval idle, no skipped accepted
work, original receipts and all funding/coverage status unchanged. Current Stopped or
NoWork is insufficient. No fake electric/cost tickets or automatic load-time gap filling.

Only physical cursor and checked sequence advance. Every quantity, material/input/output
fraction, input take, raw StreamBasis, WIP and all historical cost/coverage totals are
unchanged; the full material and funding Delta is zero. Idle renews no source/use permit,
releases no original labor/disposal/legal obligations and does not settle debt.
Revalidation compares version/profile, complete base digest and fresh facts, repeats the
V2-only idle authorization and recomputes the entire candidate. Structural restore retains
V1 physical validation plus exact coverage-total closure, no accrual without explicit
policy, zero costs at zero processed volume, and original-stock coverage sufficient for
original raw/reagent converted basis. Source authentication and receipt readiness stay root.

## Fixed budget and source-only verification

One funding state/candidate, eight cost kinds, sixteen coverage items maximum, five
coverage alternatives and one optional original accrual policy. Arrays/fields have no
unbounded strings, maps, history or persisted claims list. Saved funding adds six Exact
historical totals and an explicit wrapper, not a financial ledger. No GPU/UI/AgentKit/new
clock/worker/daemon is introduced. Existing caps remain World128KiB, Engine4MiB,
role8MiB, normal64MiB and default stack; original M0 acceptance is unchanged.

The existing 16KiB owned/96KiB prepare+restore planning bounds are **not new grants**.
Fixed-envelope planning estimates remain saved12KiB/candidate32KiB (two plus two =88KiB
for prior/new/revalidation/serde/restore overlap). Adding sixteen evidence spans increases
candidate DTO/serialization size: root must measure actual complete wrapper, old/new
states, facts, all source/stock/payment receipts, original workspaces, serializer capacities,
allocator and stack before accepting the shared bridge. Over-limit/precision exhaustion
means unresolved/backpressure, preserve original state/WIP/debt, never erase obligations.
Original HP P900 target/work and existing stage768MiB artifact plan stay root-owned;
no cleanup or parallel execution was performed here.

**All current compilation/testing/executable verification is not_run.** Owner performed
source edits, read review and `git diff --check` only, plus requested source SHA inspection.
Original 24 test bodies remain and thirteen new source tests bring the file to 37 functions:
V1 rich-cash/cash-gate/wire/default-deny compatibility, zero-cash actually covered full
basis, real cash-fee deficit, source/interval/amount/missing/duplicate coverage rejection,
explicit-policy accrual, candidate CAS/version/sequence/overflow, stream fractional WIP,
same-Envelope purposes, disjoint/overlapping spans, later prefetch conversion, V2-only idle
and funding/accepted-work tampering, and local fixed-envelope layout/serde checks. Synthetic
verifiers are payload/history fixtures, **not** actual Stock/Store/payment proof. Test-only
fee numbers in funding-v2.json do not establish runtime terms. Raw chemistry oracle is
unchanged. No component pass, actual #133 success or #92 completion is claimed.

Root's smallest later serial command after original-advisor narrow source review:

```sh
cargo test -p mycity-core --test water_treatment -- --test-threads=1
```

Then root must validate actual low-cash original self-supply, actual P5 wages/fees/source
curves, original Stock net/output fractions/Water work, real cash shortfall and all-or-none
rollback, actual permitted late-pay without repeated cost, original Envelope retry and cold
restore/full overlap budgets. No PR, commit, tag, release or Issue mutation from owner.
Complete changes are the canonical tracked module/test diff plus the two new full files
listed above; root can read all four before freezing actual build inputs. Final SHA values
are reported separately so this document is not self-referential. Owner freezes after report.
