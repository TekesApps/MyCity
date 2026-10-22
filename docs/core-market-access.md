# Transport-backed town market access adapter

This adapter connects an exact `transport::Opening` + `transport::Assignment` to
`town_economy::plan_day`. It emits a proposal only: cash, stock and capacity are not
settled until an authoritative Engine accepts and commits the batch. The adapter does
not claim that the example's day-28 market plan has settled.

## Contract

- Reuse requires `Assignment::matches(Opening)`, including revision, topology, entrance
  permissions, capacities, modes and OD inputs, followed by `Assignment::verify(Opening)`.
  The latter performs one bounded deterministic recomputation and equality check so a caller
  cannot forge served counts or path receipts while retaining the input fingerprint. The
  adapter verifies once per plan, never once per quote. Same-revision graph edits invalidate it.
- Each quote explicitly binds a household-group account and shop account to authorized
  entrances whose `Entrance.owner` values match those accounts. Proximity gives no access.
- A quote has outbound and return OD `source_id`s. Both endpoints must describe the same
  household/shop pair, use the same single mode, and have actual served trips. A zero-trip
  partition yields zero accessible quantity; a missing outbound or return leg cannot be
  represented as completed shopping.
- `TripPartition.trip_count` is a requested upper bound. The adapter reserves actual served
  people/trips deterministically in quote-id order, across every quote and good for each OD
  source; `DeliveryAccess.allocated_trips` reports that actual allocation. It cannot exceed
  `served_people`. Remaining edge capacity is never reissued to quotes. A quote's commodity
  cap is only `allocated_trips * goods_per_trip` using `Quantity`; people and goods remain
  separate units. Short traffic service therefore lowers the quote cap and leaves the market
  need unmet instead of rejecting a valid partial assignment or faking a completed trip.
- Quotes use their own stable `quote_id` as town-economy route source, so the planner's
  per-route/per-good capacity is precisely the adapter's partition cap.
- The returned `SettlementBatch` summary is passed through unchanged. Reports must keep
  `reference_need` (natural/baseline need), price-responsive `market_demand`,
  `unmet_market_demand` (that market demand not purchased), and `unmet_need` (baseline need
  less actual sales) distinct. In particular, high prices may reduce market demand but do not
  erase natural/reference need. When route or stock constraints leave no executable offer,
  `market_demand` is zero while `unmet_need` still carries the baseline need; this must not be
  reported as price-elastic demand disappearing.
- Travel time is the final transport outcome mean for each leg. A caller-supplied
  `FeePolicyInput` supplies cents per travel-second and the provider account; the adapter
  applies it over both legs and conservatively rounds the aggregate tariff up per quantity
  tick. Time itself is not revenue. The output explicitly marks `fee_policy_integration` as
  `external_input_unintegrated`; this fee policy still needs provider/market validation
  and is not a simulator-wide tariff.
- `source_digest` records the opening fingerprint, complete verified assignment including its
  path receipts, full market input, partitions and fee-policy inputs. It is provenance, not a
  settlement idempotency key or persistence layer.

## Limits and known gap

The assignment now returns at most 512 aggregate `PathReceipt`s, one per allocated route slice,
with at most 127 stable edge IDs each. Same-node service has an empty edge list. Final OD travel
costs are still recomputed from final edge flows. The adapter reads boundary source IDs from
the exact outbound and return receipts, so legal cross-city shopping can proceed and unrelated
cross-city OD flow does not block a local quote. The one-time deterministic assignment
verification checks that receipts, per-OD served counts, edge totals, boundary totals and costs
all match the actual opening.

This version has a hard limit of 256 market delivery partitions, below the town planner's
1,024-line bound. Work is bounded by at most 512 transport demands plus 256 quote partitions.
At 512 paths × 127 edge IDs × 4 bytes, the new path payload is about 260 KiB plus receipt and
Vec headers; assignment verification temporarily creates one more bounded assignment. The
adapter also returns only quote-specific boundary ID lists. Estimated incremental peak memory
is below 2 MiB at the published row caps, including those copies, map/index data and bounded
digest serialization. This is a structural estimate, not measured resident memory or 16 GB
acceptance. CPU includes one complete deterministic traffic assignment recomputation per market
plan, then bounded lookups over at most 512 demand/outcome rows, 512 path receipts and 256
partitions (worst-case source lookup O(Q*D), Q<=256, D<=512), plus existing `plan_day` work.
No per-person or per-item expansion is performed. Benchmark and full-city memory evidence
remain pending.

## CLI proposal scenarios

`cargo run -p mycity-core --example market_access -- normal`
`cargo run -p mycity-core --example market_access -- road_closed`
`cargo run -p mycity-core --example market_access -- bottleneck`
`cargo run -p mycity-core --example market_access -- capacity_expanded`
`cargo run -p mycity-core --example market_access -- cross_city`
`cargo run -p mycity-core --example market_access -- unrelated_cross_city`

The CLI includes the served OD trip counts and exact boundary source IDs passed to the market
adapter, then reports the resulting accessible quantity and price/demand summary. Scenarios
cover a normal network, a closed return road, a narrow bottleneck, expanded capacity, a legal
cross-city shopping round trip, and unrelated cross-city flow alongside local shopping. All
output labels say `pureplan_not_settled` and include `engine_commit_required`: transport results
are consumed by the planner, but no Engine ledger commit or cash/inventory settlement occurs.
Fixture stock and utility availability are explicit market inputs; they are not generated by
the transport model.
