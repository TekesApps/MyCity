# #36 finite asset-maintenance opening and independent expectations

2026-10-29; actual source checkout `/Users/frankqin/dev/MyCity`, HEAD observed as
`1938fa3` while preparing this input. The parent reports current release v226 and
owns the shared contract/integration. This handoff is based on the source files
listed below; no maintenance runtime or future maintenance ingress fields were
added. All runtime, compile, test, formatting, and script checks are `not_run`.

## New profile and source facts

`tests/fixtures/finite-city/maintenance_opening.rs::input/resources` is a
separate variant of the existing finite input. `asset_opening.rs`, its base
opening, M0 inputs, and existing stock/cash/population amounts remain unchanged.

- Project 7 remains the real finite construction profile and carries its two
  configured operating units. The new variant sets its `operating_base_capacity`
  to 0 and gives resource 81 opening capacity 0. Its completed build can therefore
  contribute exactly the two newly constructed units; it does not withdraw or
  replace an older shared contribution. All other resources are copied unchanged.
- The only registered population remains the existing household 7 / account 2,
  with 10 people and its original source 7001. The new profile explicitly records
  Electric base qualification for members `[0,4)` and leaves `[4,10)` at the old
  mask. It extends this new profile's immutable authority and baseline-source
  validity through `30*DAY + 20h`, preserves the old Driver/Police bits, and does
  not create or certify any people. Those are synthetic opening facts, not a
  real credential. Root still must verify the qualification read from its actual
  writer at dispatch time.
- Existing payroll binding 21 stays intact. The preparation creates no unused
  Payroll22 promise. The future actual maintenance contract must cover four
  members for a 3,600-second interval at 2,000 cents/person-hour; only that
  actual order can generate 8,000 cents of wages. The earliest 30DAY due is
  commissioned_at 61,200 plus 30DAY, so [30DAY+17h,30DAY+18h) is the example
  work interval, not 09:00 before it is due. No work or payment exists yet.
- Existing Stock 15 is the only proposed finite Raw supply: opening quantity
  1,000 / actual basis 1,000 cents from source 21002. The original construction
  consumes 100 quantity / 100 cents, leaving 900 / 900 on the previously
  established finite-city path. No stock is added or replenished.

The fixture intentionally adds no venue offer. The service-delivery fixture
removes its inherited `venue` object. The other available example,
`tests/fixtures/facility-work/electric-v1-opening.json`, only has venue 1001 owned
by account 1 and operated by account 3, purpose 1, with a genesis offer for
household source 7001/member `[0,1)` during `[0,7200)`. Its venue purpose, owner,
members, and validity window do not establish a legal maintenance entrance for
project 7 (owner 7) at day 30. Extending or relabeling that offer would invent the
site/access fact. Root must supply or identify the actual authorized maintenance
venue/entrance and member access before dispatch can be represented.

## Calendar-only maintenance oracle

The intended bounded case is one ordinary 30-day cycle for the newly completed
project-7 electrical-support asset. It is a passive shell: operating wear is
configured as 0, and no operation profile, accident, or condition damage is
invented. The cycle may become due from calendar time only. Resource 81's two
units are temporarily unavailable while work is physically in progress and
return only through the owning resource writer after successful completion; do
not use construction `Withdraw`, which is a permanent end-of-day withdrawal.

Using the existing actual delivered asset basis `K = 28,916` cents as the
component scope, the P53 reference rule gives:

- `0.001K = 28.916` cents of compatible repair material value.
- Minimum net work is `max(1, 0.001K/80) = 1` crew-hour. Four exact Electric-
  qualified members for 3,600 seconds each are **14,400 person-seconds**, not
  3,600 person-seconds. Construction worker-seconds and project labor quote do
  not satisfy this maintenance quantity.
- At the explicit new binding's 2,000 cents per person-hour, four people for one
  hour cost **8,000 cents**. The construction quote (3,600 cents/team-hour) is
  not substituted for maintenance wages.
- The P53 ordinary reference-material price is 12 currency units/Q = 1,200
  cents/Q. At Quantity's milli-native coordinate, the least whole quantity that
  meets the 28.916-cent reference value is `ceil(28.916*1000/1200) = 25` units
  (0.025 Q). At the reference guide this is 30 cents. On the existing Raw Stock
  15 lot, its actual frozen basis is instead 25 cents (`900 basis * 25 / 900`);
  reference price is guidance and must not overwrite actual stock basis. The
  25-unit amount is available after construction from the existing 900 units.

The quantity calculation assumes root fixes project 7's delivered 28,916-cent
asset as the maintained component's K and authorizes Raw as compatible for this
part. If the actual electrical component has a distinct K or a stricter material
class, recompute quantity from that source/profile and do not claim Stock 15 is
compatible until its catalog contract confirms it. No fee, quote, or cash entry
can substitute for the physical stock debit.

Condition improvement, inspected fraction, maintenance charge policy, actual
venue/access source, and the exact stop/resume receipt remain unresolved inputs
for the root-owned contract. This handoff sets no condition delta and claims no
work completion. It intentionally does not define maintenance JSON fields.

## Validation and remaining work

- Source-derived opening edits and arithmetic: prepared, not executed.
- Resource/member/qualification/payroll/Stock/venue acceptance: `not_run`.
- Compile, tests, formatting, scripts, real issuer/root reads, and actual venue
  entrance evidence: `not_run`.
- Root owns integration and must resolve the venue/access gap and any
  component-specific K/material compatibility before serial validation. Preserve
  the candidate as incomplete until then.

## Root审查更正（准备不等于已验收）

1. 第一版把[0,4)旧Driver/Police mask33替换为Electric mask2，会使旧Police业务失去资格；当前改为mask35=33|2，[4,10)保留33。不是删除原资格换新资格。
2. 维护due从实际投运61200秒（17h）起算30DAY，不能在第30日09:00提前维护。候选工作窗应为30DAY+17h至+18h，当前新资格期限留到30DAY+20h。原存档/原fixture期限不变。
3. 未用到的issuer3和Payroll binding22已去除：原Opening baseline source是实际初始资格来源；维护须自己的显式工单雇佣合同，并沿原FacilityWork工资内核结算，不增加一个不会启动的Payroll承诺或把整组工资当每人报价。
4. 新variant显式使用已验LOCAL_EXACT_PROFILE，避免新工作仍落到已知逐票舍入旧语义；原fixture保持v1。
5. 所有改动仍待root实际Engine装配/资格/来源检查。venue实际Accept/Enter与停机/材料/维护回执尚未接线，不称现场或维护已完成。
