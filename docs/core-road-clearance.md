# Never-published road clearance CLI scenario

`mycity-storage`'s `town_transport road-clearance <new-store-dir>` was executed as run59 against the
Store-level path where a road-capacity project is stopped before inspection and cleared without
ever publishing its added capacity. The recorded CLI exited 0; the details and limits below are
the measured fixture scope, not a broader legal or M0 acceptance claim.

The fixture binds one road effect to the existing fixed transport allocation: base capacity 2,
potential added capacity 8. It builds for 14,400 worker-seconds, consuming 500 units from the
existing material stock, pauses, and settles day one using the base route (40 sold). Owner and
permit gates are then explicitly approved for clearance. Clearance records its original work and
consumption, requires 2,400 more worker-seconds, accepts one waste-capacity unit for the fixed
250-cent fee, and closes the project. It does not manufacture replacement inventory or alter the
transport profile.

The example asserts the capacity remains 2 and both publication and withdrawal timestamps stay
absent through closure. It settles day two after closure (again 40 sold), checks finished-goods
debits and citywide cash conservation, checkpoints, cold-opens the same Store, and compares the
closed project and both journaled transport-proof hashes.

Run59 recorded day-one/day-two sales of 40 each, 2,400 worker-seconds of clearance, one accepted
waste-capacity unit at the 250-cent fee, retained material consumption of 500, capacity remaining
at the base 2 before and after closure, cash conservation, and cold recovery of the closed project
and historical proofs. No road capacity was published or withdrawn. The separate 58 road-withdraw
CLI exited 0: after expansion to capacity 10 and day-two sales 200, approved withdrawal at the
settled day-two boundary returned capacity to 2; day-three base sales were 40, the project closed,
and the site was released. These do not establish other interruption/retry paths.

Run57's 28-day road profile completed 28 daily settlements with 28 unique consumed transport
proofs, cash conservation and final cold reopen, but sales occurred on only six days; the final
sales day was day 6 and the opening raw stock of 1,000 was exhausted. It is not continuous sales,
self-funding, profitability or a calibrated balance-sheet result. The profile uses repeated fixed
OD and an explicit utility capacity/cost profile, not live transport or a live grid. The broader
road/town evidence index is reserved at [core-road-town-integration-122](evidence/core-road-town-integration-122/STATUS.md);
that version has not been published. 69's stale shared-cache compile failure is retained; 70
removed only mycity-core release cache artifacts, 71's road-epoch tests passed 3/3, and 72's
final core/storage all-target release test exited 0 with source unchanged. These do not establish
the 64MiB full-scale budget or physical performance acceptance. Formal01 ended early under the
pressure guard, and the 8-hour gate and actual 16GB device gate remain incomplete.
