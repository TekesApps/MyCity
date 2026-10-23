# Geometry candidate component: verified finite stage

Associated Issue: [#25](https://github.com/TekesApps/MyCity/issues/25).
This is an intermediate F component, not completion of F or the shared persistent-world chain.

293 ran 17 release tests (5 terrain library, 7 geometry revision, 5 traffic adapter), all passed.
294 all-target mycity-map strict clippy, 295 release CLI build and 298 workspace fmt passed.
296 and 297 actually executed the fixed CLI for a curve control-point edit and a door-width edit.
See commands.json, fixed-binary.json, curve-report.json and door-report.json.
The final fixed source is 5fe7039ef09aa8b9f59d5d094a7e602537ceab1633b506cb69e21c009fbde18b and binary is 642ca4757660f9ae40014bff334e4466152166e7236c462b6ba31c364f9aea76.
Example source identity was verified from its complete source archive/manifest and executable hash;
there is no embedded example source-identity check. Older 285-290 results are not proof of this revision.

The curve changed road free-flow time 41 to 42 seconds, served people remained 4.
Door widening left time, capacity and served people unchanged. Both counters increment
from their own parent, need not equal each other; either overflow rejects without parent mutation.
The independent check_oracle.py and oracle.json use dense planar Bezier sampling only for
the pinned flat fixture, exact profile-byte hash, serialized revisions, endpoints, IDs and capacity.
This is not a universal terrain numerical proof. Reproduce with:

```sh
python3 docs/evidence/geometry-candidate-148/check_oracle.py . \
  '/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01/296-geometry-curve-version-world' \
  '/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01/297-geometry-door-version-world'
```

## Computation contract

Authoritative input is a validated prior Scene, terrain, explicit adapter configuration and
prior profile including its exact raw bytes; output is an unactivated candidate Scene and
exact new profile bytes/hash plus bounded diagnostics. One effective control-point or port-width
edit triggers full finite Scene validation and WALK adaptation. Typed vector/string/graph/raw-byte
bounds are checked before clone/decode. IDs, OD, speed and facility capacities are preserved.
There is no ongoing tick work: unchanged authoritative geometry/profile/config is the stable
interval. Ordered edits cannot be equivalently merged across authorization, terrain, parent
revision or history changes. Support, obstacle/edge clearance, layer and endpoint checks remain
exact gates. The existing CPU Scene and adapter are the reproducible reference path.

Candidate retained owned capacity is 20,161 B; terrain is 594,968 B, combined 615,129 B
after prior baseline release. This estimate excludes allocator metadata, temporary assembly,
serialization, diagnostics and output buffers. External-SSD microfixture time-l observations:
curve maxRSS 4,308,992 B, footprint 2,998,584 B; door maxRSS 4,259,840 B, footprint 2,949,432 B.
These are not isolated city-scale peaks or the common8MiB/16GB gate. Both use external
HP P900 USB/APFS SSD work/target and the same shared target.

## Authority, persistence and limits

No owner consent, legal permission, construction progress, calendar advance, Store write or
active-pointer switch is implemented here. Complete output files remain externally retained
with the full source ZIPs, actual executable and logs under construction-utilities-current-01.
They can be inspected as candidates; they are not Store recovery worlds. Existing v147
Store history/recovery evidence remains separately authoritative and is not re-certified here.
Only finite WALK, one quadratic control point or an existing door/anchor/terminal clear width
is supported. No endpoint relocation, full widening, bridge/tunnel or non-WALK activation.
The existing runtime geometry activation gap is specified in geometry-revision-candidate-handoff.md.
Original M0 normal/common8MiB gates remain unmet; 8h and real16GB remain not_run.

Advisor independently reviewed the new actual evidence and accepted this finite component
for publication with these limitations; no new blocker and no request for another general review.
