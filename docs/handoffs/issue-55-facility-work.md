# Issue #55: bounded facility operation work — source handoff 04

2026-10-25, Asia/Shanghai. Author state: frozen source; ALL compile, test, fmt,
script, benchmark, stress, GPU, native-memory and runtime validation `not_run`.
This is not a facility-operation provider, original writer, Store integration,
real employment proof or full #55/#92 acceptance. No commit/push/tag/PR/Issue edit.

## Identity and exact owned files

Canonical checkout `/Users/frankqin/dev/MyCity`, branch main, initial HEAD
`60d557bd1252e198598cfae47e803654237bb8a5` (v0.5.188), matched root's assignment.
Read actual AGENTS.md first. Did not recreate the retired v177 tree or alter it.
Initial observer/M0 WIP already existed in:

- `docs/evidence/m0-model-lifecycle-observer-candidate/` (tracked and untracked).
- `docs/m0-implementation-status.md` (tracked change).
- `docs/evidence/m0-t13a-rare-control-01/` (untracked).

Those files, old source/evidence and all shared writers remain untouched by this
owner. Root remains their single integrator. This owner adds only:

- `crates/city-core/src/facility_work.rs`, full new source and eighteen inline tests;
  952 lines / 49305 bytes, SHA256
  `6773e22d14dfd5964a6b5c47e2c4ccdbf9b96fe07ea9f747311689c179122003`.
- `docs/handoffs/issue-55-facility-work.md`, this complete handoff.

Both are untracked at author freeze; tracked `git diff` alone misses them. There
are no separate fixtures. Root must inspect and archive full files and modes.

## Final API shape

Static `Role`, `Binding`, `Contract`, `Opening` and `Error` are public because the
original public assembly Opening exposes this registration. Runtime, source/event
refs, records, candidates, trusted evidence inputs and internal Command remain
crate-private. Public static data confers no admission or operation authority.

```text
Role = Electric | Water; required_base() reuses unchanged C48 qualification bits
Binding { scope, population_origin, member_version, people, epoch }
Opening { binding, contracts: [Option<Contract>; 4] }
Contract { id, version, device_id, role, members, employer_account: u32,
           employee_account: u32, venue_use_id, hourly_cents_per_member,
           start, end }
Contract::maximum_earned_cents() -> Result<u64, Error>
Source { binding, ordinal, operation_id, occurred_at }
Admission::from_root(contract, activity_id, qualification_source)
ObservedInterval::from_root(contract, activity_id, from, until,
                            ledger_operation_id, device_operation_id,
                            timeline_operation_id)
Payment::from_root(contract, cents, cash_operation_id)
Command = Begin(Admission) | Stop { contract_id, activity_id }
Runtime::new(Opening)
Runtime::prepare(Source, Command) -> Candidate
Runtime::record_interval(Source, ObservedInterval) -> Candidate
Runtime::confirm_payment(Source, Payment) -> Candidate
Runtime::validate_binding(&original_Opening, completed_tick)
Runtime::records(), opening(), next_ordinal(), last_source() -> Option<Source>
Candidate::next(), matches(&current), into_next(&current)
Record::due() -> earned - paid (read only after successful structural validation)
Runtime::{owned_bytes, candidate_bytes, prepare_extra_bytes, restore_bytes}()
Runtime::terminal_encoded_bound_bytes(&Opening) -> Result<usize, Error>
```

`Opening` has a packed prefix of at most four immutable, uniquely identified
contracts. Empty prefix is legal. Nonzero versions/IDs, nonempty exact half-open
members inside original population, positive rate, distinct nonzero payer/payee,
and explicit nonempty `[start,end)` are checked. The whole-window maximum wage
must fit original Money's nonnegative i64-cent range before admission. This is
an arithmetic projection, never a reserved fund or proof that work occurred.
Employer/employee accounts use original u32 indices, not a second account type.

Root explicitly accepted this bounded lifecycle: each contract may Begin ONCE,
then Stop ONCE. A stopped responsibility remains resident; it is not recycled.
Resuming later requires another explicit immutable contract within the four-slot
profile. This is not an indefinite facility lifecycle or arbitrary shifts system.
Electric and Water are explicit separate roles/contracts; nothing makes one
member qualified or available for both simultaneously.

Each record reserves its final responsibility slot at Opening. It stores phase,
original externally allocated activity_id, first Begin event and qualification
ReadSource, started/settled/work_seconds, cumulative earned/paid, denominator3600
remainder, last actual interval evidence, last cash payment reference and Stop
reference. There is no unbounded operation history/ID set. Original detailed
history remains in original Store receipts.

To avoid repeating population/scope bytes in every event slot, durable event refs
are `EventRef { ordinal, operation_id, occurred_at }`. Their binding is the single
immutable Runtime Opening binding. `EventRef::source(binding)` reconstructs the
full source when needed; root must use that original binding. Transient Source
inputs still carry and check the complete binding.

## Original-authority obligations — not implemented by this module

`Admission`, `ObservedInterval` and `Payment` fields are private, constructors
crate-private and not deserializable. They are TRUSTED root-adapter inputs, not
cryptographic capabilities or externally verified DTOs. A caller with crate
access can manufacture them. Root must never expose them as untrusted ingress.
Constructors do not perform authority work and accept no `verified` bool.

Before Begin, root must independently verify actual bilateral employment,
original membership-to-employee-account mapping, full exact qualification
coverage/Eligible (Unknown must refuse), current original source, venue actual
entry, and unique member-ledger admission. Root supplies the actual ledger's
allocated activity_id; this module never allocates IDs or claims member time.
ReadSource binding/as_of/root checks here are necessary only; matching source
does NOT certify that the job qualification gate passed. Unknown qualifications
cannot be resolved by a positive flag in this module because no such flag exists.
Root's future actual Unknown/education-conflict tests are mandatory.

At root's stated current adapter scope only original continuous_supply Resource301
genset Electric is to be supported. Unsupported Water/other devices must refuse
until real V2/plant providers are implemented and validated. These component
synthetic Water/role fixtures are not evidence of an actual Water adapter. Root
owns a separate public serde Begin/Stop ingress; internal Begin(Admission) is
never exposed externally. Root plans at most one payment/contract/P5 segment,
after interval recording, both on the same original operation with next local
ordinal. The real protected payment and ledger settlement stay in one Prepared.

Before record_interval, root must prove EXACT matching original ledger activity,
members, settlement interval, device actual RUN/operation coverage and completed
Timeline coverage, using original references. Numeric nonzero reference checks
here reject malformed data only; they do not prove real operation. Root must add
FacilityOperation to its original ledger and compare active responsibilities
one-for-one on Begin/settle/Stop/restore. This module only enforces same immutable
contract and activity ID. It introduces no second population, activity ledger,
calendar, device simulation, venue authority or 09–17 Payroll behavior.

Root owns the original production/funding/cash prepare/persist/apply transaction.
Use preflight arithmetic before admitting a device operation; a rejected candidate
must not leave physical work/output committed without its wage responsibility.
If work already happened, retain original facts and resolve the obligation; do
not use an overflow/retry failure to declare zero labor or delete arrears. Root
decides protection/funding for new production via its existing funds mechanism.
This module has no protected-money balance, asset, tax, transfer or wallet.

Before confirm_payment, root must have prepared the actual original debit/credit
for exactly this immutable contract's payer/payee, amount and original cash op.
Confirmation is an obligation cursor update only, atomic with the real payment.
Qualification, venue presence or paying cash alone never retrospectively proves
that labor occurred. Missing cash leaves earned/paid/carry unchanged and due live.

## Exact interval, arithmetic and sequence rules

Begin initializes settled=started at the original occurred_at, in the explicit
contract window; it accrues zero work. Intervals require `from == settled`,
`from < until <= contract.end` and `until <= original Source.occurred_at`. Thus no
zero, future, repeated, overlapping or skipped interval can be credited. Exact
actual coverage is required; working on part of a window and resting then working
again is outside this one-contiguous-lifecycle profile. Stop may occur after the
last settled time, even after contract expiry, but NEVER fills an unobserved gap.
Root must settle any actual work before Stop in the same atomic transaction.

Amounts are integer cents, rate is cents/member/hour, work_seconds is elapsed
seconds for the exact member interval, not pre-multiplied person-seconds:

```text
numerator = rate * (members.end - members.start) * actual_seconds + old_remainder
earned_delta = numerator / 3600
new_remainder = numerator % 3600
```

Production arithmetic uses checked u128 multiplication/addition and bounds each
earned result/cumulative amount to `i64::MAX` cents for original Money. Payment
amount must be positive and <=earned-paid. Partial payments preserve carry and
leave exact integer due; subcent remainder stays recorded through Stop and is
never rounded up, deleted or turned into paid cash. No wage rate reduction or
contract replacement is accepted after Opening. Split/merge exactness follows
from carrying the numerator remainder; authored oracle checks it independently.

Source binding must match Opening, operation_id nonzero, ordinal exactly next,
occurred_at nondecreasing. Same original operation_id/occurred_at with successive
local ordinals is LEGAL for one original Envelope's multiple atomic events
(e.g. interval then payment). Original retry/dedup is root Store's first step.
`last_source()` returns None for pristine state or reconstructs the last admitted
Source from the original Opening binding and saved watermark. It is a read-only
receipt-authentication input, not authentication itself or a new permission.
Local sequence prevents repeating an event; the last cash-operation ref also
rejects immediate duplicate payment confirmation. There is no claim that this
bounded module independently detects a very old cash op replay under a new local
ordinal: root must resolve original cash receipts, not relabel a past payment.

Every candidate retains complete expected+next fixed states. `into_next` compares
the actual current state, rejecting stale candidates. No mutation occurs during
prepare. All methods preserve one unused ordinal per active future Stop;
they ALSO preserve one unused ordinal per contract with integer `earned > paid`
for its final full arrears payment, independent of Active/Stopped status. Both
finish and restore use the SAME bounded formula:

```text
required_remaining = count(phase == Active) + count(earned > paid)
u64::MAX - last_ordinal >= required_remaining
```

Intermediate work/payment or a new Begin cannot consume these final ordinals.
New wages must reserve any newly arising payment responsibility as well as Stop.
A partial payment still leaving due cannot consume the final full-payment slot.
Stop requires no free record slot and reduces only the Stop reserve; integer
arrears retain their payment reserve through Stop and recovery. Pending/Stopped slots
cannot be reclaimed or used to bypass finite responsibility limits. At total
u64 sequence exhaustion no further events are supported; a future authenticated
stream migration would be separate, not an implicit ordinal reset.

A stopped fractional remainder is not a payable integer cent and adds no payment
ordinal. Root must retain that SAME stopped record, original rate/member/window,
earned/paid/carry and original receipts in checkpoint/WAL; neither due==0 nor Stop
means permission to delete its subcent responsibility. This profile never resumes
the stopped contract or converts its fraction into a fabricated integer wage.
Any future transfer/combination/migration of that fraction requires an explicitly
authenticated new profile preserving the exact numerator, not implicit rounding
or reset. At exhausted sequence, retention remains possible without a new event.

## Restore and budgets

Serde records have deny_unknown_fields; `validate_binding` must be called before
publishing restored state. It compares the full original Opening and immutable
rate/role/account/member/device/window fields, version/population/scope/epoch,
completed-time upper bound, fixed slot correspondence, valid phase, unique
activity IDs, retained event ordinals/tick ordering and final watermark. It
recomputes total work, exact earned and carry from the original rate and actual
continuous duration, checks paid<=earned, payment could not precede enough earned
work, and final Stop could not precede the final interval. It does NOT accrue,
advance a cursor or repair anything on load. Lost/corrupt original receipt data
must be refused by root; structural validity alone is not ready authentication.

Fixed Rust storage only: owned limit8KiB, prepare-extra limit32KiB. `new` checks
owned and prepare-extra planning interfaces AND the terminal encoded bound below.
`owned_bytes` includes all immutable
contracts/records, `candidate_bytes` includes expected+next, prepare-extra adds a
separate fixed working Runtime, restore_bytes is one extra Runtime. Root must add
original live owner, overlapping ledger/device candidates, parsing/serialization,
Store/WAL buffers and actual stack/native peaks to its shared4MiB/whole budgets.
No Vec/Box/heap history/cache is introduced. No byte-layout number or serialized
size was executed/measured by author.

Revision04 adds an ENCODING admission gate before any real labor:
`terminal_encoded_bound_bytes(&Opening)` validates registration, constructs a
fixed maximum-width sizing envelope, and counts the actual compact serde_json
serialization via `std::io::Write`. The counting writer discards bytes, uses
checked usize addition, and allocates no output Vec/file. Overflow is explicit.
`new` returns UnresolvedBudget if this bound exceeds the SAME original8192B cap;
it neither shrinks the four-slot table nor increases the cap. Callers can obtain
the bound even when it exceeds the cap, without constructing an admitted Runtime.

The envelope keeps immutable Opening fields exact. For each original contract
all optional record/source fields are Some; phase Stopped is as long as Pending
and longer than Active. Activity IDs, event ordinals/operation IDs/ticks, interval
references/endpoints/work_seconds and final Runtime watermarks use u64::MAX.
Earned/paid/payment cents use i64::MAX (original Money ceiling); carry is3599.
Each qualification ReadSource retains its immutable scope/population/version,
uses hash bytes255, revision u64::MAX and as_of i64::MAX. Thus every mutable
numeric field has a representation at least as wide as any structurally legal
state's encoding. No state can grow a new record/string/history beyond this shape.

These maxima are individually representable upper bounds, deliberately not a
causally consistent joint state (e.g. duplicate maximum ordinals); the envelope
is PRIVATE sizing data and must never be installed or treated as evidence. It
can over-reserve for a short window or an impossible combination. Every valid
state's compact JSON is covered, including initial/active/partial-payment/stopped
and fractional records. This is wire evidence from serializer counting code, not
the raw Rust layout, and it remains UNEXECUTED author code until root verifies.

The bound is for THIS Runtime's compact serde_json encoding, excluding original
root wrapper keys, escaping as a JSON string, formatting or another codec. Root
must separately budget outer structures and ensure its persistence encoding
matches this contract. Root should check the same Opening-time bound before
activating/restoring a registration. Counting uses one extra fixed Runtime plus
serializer/counting-writer stack at registration; include that restore/Opening
scratch and original overlapping state in joint budget. No runtime work/payment
event needs to run this serializer again after the Opening reservation.

No numeric bound was executed/reported by author. In particular there is no
claim that every possible four-contract JSON registration fits8192B; the authored
high-width four-contract fixture exposes the helper result and checks admission
against it. Root must report the actual conservative result for its actual
Opening and this worst-width fixture. If four contracts exceed the cap, stop
admission and coordinate compact tuples/arrays or explicit separate budgets with
root; this owner has not changed encoding or budget standards unilaterally.

## Authored tests and future root validation

Eighteen inline tests are authored, all `not_run`:

1. Independent repeated-addition integer oracle for fractions and multiple
   members; actual0..4 versus0..3..4 yields equal work/earned/remainder.
2. No prefilled wages; zero/gap/future/outside/overlap/duplicate intervals and
   stale candidate rejection, original unchanged on prepare failure.
3. Exact source, qualification-source binding, immutable rate/contract, actual
   externally allocated activity ID and nonzero evidence refs; no Eligible bool.
4. Stop leaves owed cents and fraction intact, never fills the gap, forbids
   resume; partial pay still works after Stop.
5. Partial payment/overpay/duplicate confirmation and same Envelope's interval
   plus payment with next local ordinal and same original operation/tick.
6. Unrepresentable full-window promise/u128 intermediate rejected, no state
   mutation, Stop does not compute hypothetical future wages.
7. Restore checks altered earned/paid/carry/work/activity/source/watermark and
   changed immutable rate/future time; loading causes no catchup.
8. All four reserved records can close; no module allocation of ledger IDs;
   duplicate supplied activity ID rejected; authored byte-budget assertions.
9. Invalid rate/member/window/duplicate or non-packed Opening rejected.
10. Hypothetical authenticated near-u64-limit prefix: reserved Stop ordinals
    reject intermediate work but allow all four remaining Stops. This fixture
    is synthetic structural state, not a recovered original Store history.
11. Near-limit integer arrears -> Stop -> serialized structural restore -> final
    full payment; Active/Stopped dead-end watermarks fail restore validation.
12. The final payment slot rejects a partial payment leaving due with no mutation,
    and accepts full remaining payment.
13. Four contracts each reserve independent Stop and final arrears payment through
    serialized structural restore; all eight final events can complete.
14. New integer wages cannot consume terminal payment reserve; a joint dry sequence
    of interval/full payment/Stop needs all three ordinals and preserves the owner
    on an insufficient-reserve interval proposal.
15. A stopped subcent remainder persists through serialized structural restore at
    exhausted sequence with no fabricated integer payment.
16. Counting writer matches independently retained compact JSON length for a
    full envelope; checked usize overflow returns error without wraparound.
17. Pristine/active/partial-paid/stopped/fully-paid-but-fractional serialized
    states fit the Opening's counted bound; enum name widths are covered.
18. Large numeric/hash-width four-contract registration retains all four slots;
    exact counted envelope length determines admission/UnresolvedBudget under the
    unchanged8192B cap. No unchecked claim of its numeric result is made.

Root declares the module (author did not edit lib/Cargo) and then alone executes,
serially with mounted HP P900 original target/work, for example:

```sh
CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test -p mycity-core facility_work::tests
```

Original advisor must review actual frozen files once, same author repairs any
blocker, then root validates actual entry/employment/Electric+Water qualification,
education conflicts, original ledger one-to-one activity, P5 device operation,
12→24kW/Stock changes, funding/payment failure, Store receipt retry/checkpoint/cold
restore and real combined budgets. This source has not performed any of those.
Root alone updates Issue #55/#92/design docs and release evidence as applicable.

## Retention and stop-writing state

Source plus this handoff is far below16MiB. No target/work/new worktree, generated
cache/binary/validation output or temporary scripts were created. No other process
was stopped and no existing WIP/evidence was changed or deleted. Keep complete
frozen originals until root full-byte/mode/hash archive and audit/repair handoff;
hash alone is not an original. No cleanup or retirement of old records occurred.
Author stops writing after freeze/report unless root requests a specific repair.

Revision02 implements root's requested static public vocabulary and last_source
accessor only, plus authored getter assertions/handoff updates. Immediately before
the edit, source01 SHA256 `2a5084aeffa62b72bf2706ed665062cc7e7cd69fee23aa869ae58c70a447e857`
and handoff01 `c806501042b5387841ea727818b534f6020914244447000ddda24e982f25f199`
were rechecked exactly. No shared World/Opening/lib/receipt file was edited by
this owner. All execution validation remains `not_run`.

Revision03 fixes the advisor's terminal arrears blocker: revision02 reserved only
Stop, allowing a final Stop to consume the last sequence number while unpaid
integer wages remained. Root confirmed full source02/handoff02/original01 archive
at `work/facility-operation-20261025/owner-source02` before this edit. Source02
`f0b40addc6db045df53692159aec8c3a80563d937bacd1795eb6e4b1b5c73ea9`, handoff02
`7a76b5542448a6374f778879d80de866b15d697a0a89cbea7a31fa91c6bb2d69` and historical
source01 were rechecked before editing; historical original01 is unchanged.
Root must joint dry-prepare interval + actual payment + close BEFORE real device
work is admitted, as root confirmed, and then commit the checked original whole
transaction. Source-only reserve checks do not grant permission for work already
performed without a payable terminal path. Same advisor narrow recheck and all
root execution validation remain pending/not_run.

Revision04 adds only the requested encoded admission interface, its fixed sizing
envelope/counting writer, new's pre-labor gate, three authored tests and this
handoff. Root confirmed source03 was fully archived and the same advisor cleared
the terminal-arrears blocker before this edit. Immediately pre-edit source03
SHA256 `e8e805ed0b29b50c577cdf5fe2ccd40fbc0b141b67331743cbcb98e5207a35b3` and
handoff03 `6220af2f7e8a1ebbe32871cd3e850bedeacbe4f143ca916108da6a5895367ff2`
matched exactly. Historical original01 remains untouched. All tests, scripts,
numeric serializer counts and actual budget/provenance checks remain `not_run`.
