# Housing runtime handoff: shared D and F seams

Status: bounded first-period housing and second-month recurrence passed the canonical Store and
CLI checks on the tested source. The recurrence tests exercised same-Claim accrual,
outstanding-only P2 payment, arrears, cold reopen at the P1/P2 boundary, exact operation replay,
and cash conservation. This remains a limited prototype, not complete housing, the later spatial
joint acceptance, or a 16 GB performance pass.

## Inputs and fixed bounds

Housing's current candidate is `housing.rs`. It defines `OpeningHousing`, one `HousingUnit` row
per physical dwelling, `DwellingFact`, and `Command::{Lease,RentDue,Move,EndLease}`. The prototype
ceiling is 32 units; each persistent row is 64 bytes, so a fully occupied 32-row backing is
2,048 row bytes. The row vector/header and `HousingState` header are additional. Restore validation
can hold three `u64` key arrays per unit: at most 768 scratch bytes for 32 homes, plus vector
headers/allocator overhead. This is a prototype cap, not the full housing target or proof that
Engine's 4 MiB budget passes. Allocate only the admitted unit count; measure active, Prepared,
Store restore and decode peaks before claiming the budget.

The current D contract provides four company charters/registers, at most eight direct holders per
company (32 direct holder rows across four companies), and 32 opening-source bindings per company.
There is no 8,192-relationship runtime capacity in this implementation. `Claim` remains at most
48 bytes even across 20,000 rows. Trusted claim provenance is sparse and bounded to 256 records of
at most 64 bytes (at most 16 KiB row payload); full-table insertion must fail the candidate rather
than omit a rent source. Source kinds are `Tax`, `Loan` and Engine-created `Rent`; the Action cannot
supply a source tag or append provenance directly.

F's current spatial permission state is bounded to 8 assets, 8 active requests, 16 approvals and 8
edges per asset. The transport sidecar catalog is bounded to 8 asset baselines, 128 versions and
128 KiB encoded bytes. It has one profile-wide hash chain; each version records the changed asset
and edge. This is a transport access-control/immutable-profile mechanism, not a property-title or
dwelling registration system.

## Shared data wiring, in dependency order

1. **Genesis and World.** `OpeningHousing` is an optional immutable registration and `HousingRuntime`
   in `World` holds it with bounded mutable `HousingState`. Absent/present serialization preserves
   legacy world decoding. The public view returns bounded unit/consumed-offer summaries rather than
   cloning the immutable opening. Housing restore currently constructs one bounded Box; it has no
   old→new backing reuse proof, so transient overlap and identity reuse remain unmeasured. The
   first-period Store scenario observed one-unit opening-owned 7,936 bytes and mutable state-owned
   72 bytes, not whole-World or peak memory.

2. **Authoritative dwelling snapshot.** Current runtime validates immutable opening dwelling rows
   and household/account registrations, and cross-checks active leases against those records. It
   does not yet derive dwelling facts from immutable scene/building assets at Store genesis or every
   recovery. A future integration must bind `unit_id`, `building_id`, owner, delivered/legal status,
   person/unit capacity, access edge and source version to actual immutable spatial assets and
   reject stale, duplicate or missing rows.
   F's `SpatialAssetState` can confirm a registered Road or Building and its managed Door/Road
   edges, but cannot by itself prove title, legal occupancy, construction completion or resident
   capacity. Bind housing building IDs to exact immutable scene asset identities at genesis;
   do not infer ownership from `Entrance.owner` or from a road.

3. **Transaction/Action.** The implemented first action is the compact selector
   `Action::HousingLease { offer_id, offer_sha256 }`, kept within the existing Action-size ceiling.
   The Engine resolves full terms and household identity only from the immutable opening offer;
   caller-provided terms, household IDs and source tags cannot override it. It checks the
   authenticated tenant account, opening household registration, dwelling and offer hash, and
   completed calendar boundary. Affordability is unknown rather than inferred from a fabricated
   income record. Non-zero deposits are rejected until escrow is implemented. Housing state, the
   ordinary Timeline rent Claim and sparse `Rent` source are staged in one Prepared candidate.
   Other command variants in `housing.rs` remain domain/planner APIs, not executable Engine actions.

4. **Rent and D equity boundary.** Actual rent paid is an account transfer. Unpaid rent is an
   ordinary Timeline Claim keyed by the lease; the housing row's `arrears_ref` points to that
   Claim and carries no second debt balance. A sparse `ClaimSource::Rent` record is created with
   the Claim and identifies its lease, unit and parties. The occurrence boundary is checked against
   `HousingUnit.last_accounted_at`/`next_due_at`, not duplicated in the provenance row. Reconcile
   arrears and `source_ref` on recovery. Deposits will need actual escrow-account funds with a
   reconciled row summary, not free income or a hidden balance; the current compact Action rejects
   non-zero deposits until that mechanism is implemented.

   D's current recognized `DailyRealizedMarketMargin` is built from actual market goods proceeds,
   inventory basis and idle labor. It does not yet include rent receipts, rent expenses or
   housing-asset costs. Therefore housing cash receipts must not silently increase D's
   distributable retained earnings. The first safe integration may transfer/settle rent and track
   its sourced Claim while leaving rent outside ordinary distributable earnings. Enabling a
   company landlord to distribute rent-derived earnings requires a separately reviewed D income
   source/cost bridge; until then the system must not claim rent is distributable profit. Company
   exit/close also remains blocked for any unsupported unresolved housing lease, deposit, arrears
   or receivable.

5. **Timeline and driver calculation contract.** The authoritative rent inputs are the registered
   dwelling/household/lease facts, the active `HousingUnit` (`rent_per_cycle`, `billing_anchor`,
   `last_accounted_at`, `next_due_at`, `arrears_ref`), and its original Timeline `Claim` plus the
   sparse `RentClaimSource`. No independent rent clock, invoice ledger, or arrears balance is
   introduced. The existing Gregorian monthly anchor selects each occurrence. The driver includes
   due-unit times in its next-boundary calculation; no rent accrues in the stable interval before
   `next_due_at`. At the exact boundary, P1 computes the prorated invoice in integer cents and
   advances the unit's `last_accounted_at` and `next_due_at` together with the ordinary Claim.
   Every due unit is handled separately; rent is neither aggregated across units nor skipped when
   simulation advances over multiple months.

   If the Claim has an outstanding balance (`principal - paid > 0`), P1 adds the new invoice to its
   principal and retains the earliest unpaid `due`. If the old balance is zero, the new occurrence
   uses the current boundary as its due. P1 also clears the prior `processed` state for the new
   occurrence. P2 pays only the current outstanding amount; actual payment adds to cumulative
   `Claim.paid` and the current page's `Progress.page_paid`. A shortage leaves the remainder on that
   same Claim. The unit's arrears reference is reconciled from the Claim after P2. Thus exact cents,
   Claim identity, and earliest due date are mandatory invariants, not aggregate estimates.

   The narrow Rent validation exception is limited to the matching lease/unit/household and the
   current occurrence: in P1 the due unit must equal the current boundary; in P2 the Claim must
   remain in lane 0 at or after the persisted cursor, with `last_accounted_at == progress.at` and
   `next_due_at > progress.at`. Restore applies the same cross-check. Ordinary Claim validation and
   payment rules are unchanged. Timeline work quoting includes the bounded housing-unit scan
   (maximum 32 rows) alongside due claims, payroll rows, and existing phase work; this is a bounded
   per-boundary scan, not per-person simulation or a new queue.

6. **Store/WAL/recovery and measured recurrence evidence.** The compact housing Action has strict
   receipt decoding. The first-period Store scenario passed invoice payment, arrears, pre-commit
   rollback, pending Advance checkpoint reopen, and exact replay after a lost reply. The later
   canonical Store run passed the second-month occurrence and cold recovery after P1 but before P2;
   the same Rent source row and Claim were retained, and replay did not accrue or collect twice.
   The second-month arrears case ended with principal 743,226 cents, cumulative paid 358,400 cents,
   earliest due day 31, and arrears 384,826 cents. In the fully paid-prior-occurrence case, the next
   due moved to day 59; after the second occurrence paid 195,174 cents, cumulative paid was
   458,400 cents, the tenant retained 15,974 cents cash, and 284,826 cents remained outstanding.
   P1/P2 checkpoint reopen and same-operation replay passed.
   Canonical core tests passed 197/197; storage library tests passed 28/28; the housing Store tests
   passed in the subsequent run, and the fixed CLI build and run passed. The earlier 307 Store
   attempt had two failures (a page-limit mismatch in the test driver and an outdated cash
   expectation); those were corrected in later test/CLI source. Do not count the original attempt as
   passed, and do not infer the separate spatial joint acceptance or 16 GB performance from these
   results.

7. **F transport profile updates.** The current F candidate exposes
   `Store::create_spatial_with_transport_permissions(...)` for a spatial genesis with explicit
   `SpatialPermissionState`; it keeps the existing generic constructors unchanged. Housing
   validation should use the current validated spatial snapshot/profile version at genesis and
   recovery, and check a dwelling's access edge against that profile. A later road/door profile
   Apply is published with the same Engine receipt and Store sequence as its catalog pointer. The
   housing snapshot must then be reconstructed against the selected profile version for the
   service day, while old transport proofs continue to resolve against their exact historical
   profile hash. Do not let a lease command mutate road/door permissions; access changes remain
   their separate two-party request/approval/apply flow.

## Acceptance order and commands

The current measured evidence is functional and source-specific. The commands below identify the
relevant checks; compile success alone is not a Store or CLI pass:

```sh
cargo test -p mycity-core --lib
cargo test -p mycity-storage --test spatial_transport_updates
cargo test -p mycity-storage --test housing_runtime
cargo run -p mycity-storage --example town_housing -- --help
```

On a rerun, retain the first-period cases and later monthly occurrence. Check exact cash, cumulative
`Claim.paid`, `principal - paid`, earliest due date, the single Rent source row, `source_ref`, housing
`last_accounted_at`/`next_due_at`, P1/P2 cursor, and cold reopen after accrual but before P2. The
compact Engine Action supports lease acceptance only; deposit, move, and end-lease remain
unsupported.

Then run an integrated spatial test: close an authorized Road edge, show the corresponding
dwelling becomes inaccessible or relocation waits; restore it only with a separate permitted
Restore request and demonstrate the next relevant service day uses the new profile. Assert an
old proof remains bound to its original profile and cannot be applied to the new day. This is a
joint behavior test, not proof of full transportation, housing economics or the 16 GB budget.

Finally record actual Engine owned bytes, Prepared peak, Store cold-open peak, and housing
validation scratch separately. Existing D capacities (4 companies, 8 holders/company, 256 sparse
claim-source rows) and F capacities (8 managed spatial assets, 128 profile versions, 128 KiB
catalog) remain unchanged. The recurrence adds no persistent queue rows or debt fields; the bounded
boundary scan is at most 32 housing rows and is included in Timeline work quoting. The test suite
has not exercised a full 32-unit maximum load, measured the transient old/new Box restore overlap,
or established the 16 GB budget. Deposits/escrow, move, end-lease, and binding dwelling access to
current spatial assets remain unsupported or unverified. The later spatial joint acceptance was
not run as part of this evidence; do not infer it from the 312 workstream. No capacity or 16 GB gate
is passed by this update.


## Next paid-income observation slice (implementation WIP, not acceptance)

The next Move affordability input will use actual Timeline Payroll.paid deltas, never accrued
wages or a caller-provided income amount. One bounded ring belongs to the existing housing
runtime:32 registered household bindings by32 day slots (31 complete days plus current day).
At game time `now`, end is `floor(now / DAY) * DAY`; the query is `[end-31*DAY, end)`.
Payment at exactly midnight belongs to the new day and enters the query only after that day
completes. Multiple actual payments that day are added once. Lifetime paid is not backfilled.
Coverage begins when trustworthy instrumentation starts; incomplete coverage is unavailable,
whereas a fully observed zero-payment day is valid zero data. Account/household identities,
coverage and bucket positions must cross-check on restore.

Every existing payment mutation, including Advance phase payment, explicit SettlePayroll and
supported termination/exit settlement, must update the statistical ring in the same Prepared
transaction as the actual account and Payroll change. Cross-day Advance must attribute each
delta to its internal event date and agree with the daily reference; a sum stamped at final
target is invalid. Lost replies and exact retry must not duplicate statistics or cash.
The ring is rebuildable evidence, not another wage ledger or another balance.

Unbilled `accrued > billed` at a normal weekly/monthly cycle is not overdue wage debt and does
not erase prior complete observations. Actual `billed > paid` may conservatively block Move
as unpaid-wage risk. The returned reference is actual received wages, not a tax-net-income
claim or future income guarantee. Unknown tax treatment cannot be silently fabricated.
`Account.cash - protected` independently constrains the actual current move transaction;
window wages are never added to cash again. Lack of a qualifying contract, lack of observation,
and observed zero income require distinct status/evidence.

Old Rent Claims retained after Move remain original sourced debts. A processed old Claim will
not be picked again by the new active lease's monthly billing; explicit Timeline::SettleClaim
must actually pay it. Reducing preloaded unearned rent may not make principal less than paid:
that Move is rejected until refund/credit obligations exist. This is not automatic collection
or a refund implementation. The sole shared writer wires these small slices; private tests
and arithmetic oracle are not Store or resource acceptance. Layout, active/spare/candidate/
restore amplification, actual payment hooks, cold recovery and combined scenarios remain to
be measured by root before release.


## Actual paid-wage slice, v0.5.153

The finite actual-paid tracker is now integrated and actually tested on canonical source;
[exact evidence and computation/resource contract](evidence/core-housing-income-153/STATUS.md)
supersede the preceding WIP label for this tracker only. Move, Wait, historical rent ownership,
city binding and refunds remain unimplemented in published code. This is an actual-paid wages
input, not all household income or guaranteed affordability. Root preserves the daily reference,
all failed tests and complete cold-recovery worlds. Single ring8456B does not prove whole-process
or maximum32-household resource acceptance.

Next Move city boundary: Region can hold multiple cities, and active_city is switchable.
The next OpeningHousing registration needs explicit immutable city identity checked against
Region.cities, inherited by all of its units/offers. Existing missing bindings stay readable
and retain original lease/billing/repayment behavior but cannot authorize Move; no default to
current active city during restore. This is a design dependency, not implemented in v153.


## Pending Move/Wait integration: authoritative computation boundary

This section describes the currently integrated, unreleased candidate; it does not mark the
remaining Store/CLI/regression gates passed. It supersedes the earlier **768-byte** housing
validation scratch estimate: the actual current constant is 32 × (DwellingFact32 + four u64
keys32) = **2048 bytes**. World128KiB, Engine4MiB and all original M0 limits remain unchanged.

- **Authority:** immutable OpeningHousing city binding (validated against Region.cities),
  registered household/account, unit and lease offers, sourced movement assessment/version,
  completed 31-day actual-paid wage window, current cash/protection, existing Rent Claim and
  consumed offers. No income, city, permission, balance or offer terms come from caller text.
  The compact HousingMove selects an assessment/city and prior Wait operation ID; these are
  selectors/CAS guards, not replacements for authoritative facts. Current assessments and
  environment observations are explicit genesis fixtures, not live environmental providers.
- **Triggers/output:** an explicit request revalidates every fact. A successful Move atomically
  changes occupancy, consumes the new offer, transfers the real moving fee, accrues only earned
  old rent, keeps the old sourced Claim, creates the new lease Claim, records bounded history,
  and clears the matching Wait. Wait records its reason/version without reserving a home or
  moving money. Retrying the same durable operation returns its recorded result; a later retry
  with a new operation must supply the current Wait CAS and revalidate capacity/affordability.
- **Stable intervals:** absence of a request does not repeatedly search homes. Wage observation
  is a complete-day rolling window; incomplete coverage is unknown. Housing bills only at its
  calendar boundary. A processed historical rent debt stays an actual Claim and is collected
  by explicit SettleClaim, not by assuming the new lease's P2 will revisit it.
- **Exact boundaries:** caller/identity, unique home/capacity, offer reuse, wage coverage,
  stale assessment/Wait, improvement/cooldown, protected cash and principal>=paid must be checked
  separately. Nonzero deposit/refund/travel-time variants remain rejected in this finite slice.
  Legacy missing city bindings remain readable and billable but cannot authorize new Move.
  Changing active city never rebinds the immutable housing portfolio.
- **Equivalent batching:** retain existing event/calendar reference and integer prorating. Due
  sources may share a prepared page only when original order, rounding, source identity and
  boundary semantics remain identical. Do not multiply a month, wage cycle or improvement
  threshold across transitions. Do not add observed wages to cash again. Threshold crossings,
  moves, explicit settlement, city activation and missing data require event-level handling.
- **City activation:** ordinary Payroll accrues work only in its home active city (or its
  explicitly unbound home_city=0 case). R54 freezes autonomous inactive-city work; already earned
  wages still bill/pay on their scheduled boundaries. last_accrual advances during dormancy,
  so reopening does not invent missed labor. This does not authorize freezing accepted cross-city
  fulfillment labor; those explicit obligations retain their own existing bounded contract.
- **Resource ownership:** World owns the optional MoveRuntime Box and its two lazy bounded Vecs,
  each capped at32. Actual Vec capacities plus48-byte Box payload header are counted; a one-Wait
  state uses96 bytes, not a preallocated unrelated32-row history. The prepare gate includes
  projected World ownership plus the same2048 restore scratch before business publication.
  Allocation failure/budget denial leaves business state unchanged but may persist the ordinary
  failed receipt. Full32+32 payload/header3120 bytes is a ceiling, not an admission guarantee:
  combined World capacity can reject earlier. Prepared/active/restore copies and process native
  footprint remain separate measurements; neither lazy allocation nor a micro scenario proves
  the original M0 memory gate or real16GB device acceptance.

Reference execution stays existing Engine Prepared → Store WAL/receipt → checkpoint/cold-open,
with the daily Timeline path and original source/Claim IDs. The original404 failed Wait world
is fully retained; a complete copy cold-opened under the corrected candidate with96 Move bytes,
World127928 plus2048 scratch=129976 below131072, and independently reconstructed Store hash equal.
412's legal near-limit18-unit rejection scenario passed without raising the cap or shrinking the
primary two-unit scenario. Its main Move scenario still has a test wage oracle correction pending;
no full Move acceptance is inferred from those partial results.


## Actual finite Move/Wait stage, v0.5.155

The preceding pending label is superseded only for the now-verified finite Move/Wait, explicit
city binding and historical-claim settlement paths. [Actual source/executable, Store/CLI,
independent final-snapshot recovery and resource evidence](evidence/core-housing-move-155/STATUS.md)
include the separate3-unit re-let case and original404 recovery. A2-unit prototype still has a
lifetime2-consumed-offer bound; third acceptance is not proven or silently enabled. All household
income, live environment/service providers, deposits/refunds, nonzero relocation travel time,
residence→OD, maximum combined counts and original scale/16GB gates remain outside acceptance.
