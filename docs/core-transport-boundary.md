# Fixed-profile cross-boundary transport settlement

The new CLI mode is `town_transport cross-boundary <new-store-dir>`. It uses the existing
Store-backed two-day town settlement path with a fixed transport graph split across city IDs
1 and 2. The household entrance is authorized at node 1 in city 1; the factory/shop entrance
is authorized at node 2 in city 2. The outbound and return OD sources each request ten people
and use their own directed boundary edge, with immutable source IDs 9101 and 9102.
Source 9101 identifies edge 10 (city 1 to city 2); source 9102 identifies edge 11
(city 2 to city 1).

`SingleTownExclusive` remains the existing allocation-scope value. It constrains the exclusive
market allocation/consumption for this profile; it does not require all graph nodes to belong
to one city. The fixed profile still binds the complete opening graph and assignment by hash.
The test reads the exact proof bytes emitted by Store, checks their hash and decoded opening
and assignment, and verifies each OD path, served/unserved people, and boundary flow. The
five-person outbound and return flows produce five round trips in the one market partition,
not ten trips by adding the two directional flows together; the corresponding accessible and
actually sold quantity is at most 100 goods units per day. Missing or duplicate boundary
source IDs are rejected by the existing graph validator.

The mode performs actual day-one and day-two market settlement, checks fresh distinct proofs,
provider freight receipts and citywide cash conservation, and consumes the existing 200-unit
opening finished-goods stock without refilling it. It checkpoints, closes and cold-reopens the
Store, then checks state identity, historical receipts, and both immutable proof files.

This fixture demonstrates cross-boundary graph provenance and fulfillment into the existing
single-town exclusive market. The OD and boundary-source IDs come from the immutable fixed
fixture. It does not model neighboring-city snapshots, migration, regional synchronization,
or cross-city contracts, and it does not expand the meaning or authority of
`SingleTownExclusive`.

## Verification status

Run54 executed `town_transport cross-boundary` against a fresh Store and exited 0. It recorded
day-one and day-two actual sales of 100 each, five people served per direction over the fixed
OD pair, immutable boundary sources 9101/9102 on edges 10/11, matching provider freight receipts,
cash conservation, cold reopen and historical proof verification. Missing or duplicate boundary
sources were rejected. This validates the fixed fixture only; it does not model neighboring-city
snapshots, migration, regional synchronization, or cross-city contracts. Run72's final core/storage
all-target release test exited 0 with source unchanged. That result does not establish the 64MiB
process budget, full M0, or actual 16GB device acceptance; formal01 ended early under the pressure
guard. Evidence is retained in
`/Volumes/HP P900/dev/MyCity/work/core-road-town-precommit-01/54-town-transport-cross-boundary/`.
