# v0.5.134 transport capacity probe

Status: **functional fixed-limit probe passed; performance acceptance not established**.

This evidence covers a single pure-transport CLI probe at the existing hard limits. It does not establish the complete A transport implementation, M0 completion, a 16 GB memory budget, clean performance, or full-city throughput.

## Frozen input and run

The final source identity is `cc6ac1a25ad5590df10fc6a5a549907b23e08c96725fbaea57b4f65a971080ab` (265 source files; 277 fixed inputs). The fixed executable SHA-256 is `44cc3a963310fa8fd8e3a937b7832f2e4d84affbc927e814a4e5041fa2869455`. The source archive remains on the external SSD; this documentation bundle deliberately does not copy the archive or executable. Hashes identify them but do not replace the original bytes. See `transport-capacity-final-binary.json` and the per-run manifests.

The measured run used an Apple M4/32 GiB Mac, with work and the shared Cargo target on an HP P900 USB/APFS SSD; source checkouts remain on the internal disk. During the measurement, another workspace's Cargo tests were active (observed process 20154); the probe did not have a quiet-performance admission. Consequently, the timings and whole-process memory numbers below are observations only and cannot be called clean or representative performance results.

Run 129 is retained as a failed probe attempt: its congestion assertion expected travel cost above free flow, but the fixture did not create that condition. This was a probe-fixture assumption error, not a demonstrated engine failure. The final source corrected the congestion case without changing the transport algorithm or its configured limits. Runs 130–132 and 135 use the final source identity; 130 strict checks, 131 example build, 132 measured CLI, and 135 formatting check exited successfully. Run 132's source-unchanged receipt is retained. The independent report `transport-capacity-independent.json` records 13 arithmetic/status checks as passed; it explicitly excludes allocator-peak and M0-performance claims.

## Fixed maximum workload and outcomes

Each assignment was rebuilt twice with the same input fingerprint and produced byte-identical result hashes on the repeat. The fixed graph contains 128 nodes, 512 directed edges, 128 authorized entrances, and 512 unique, authorized group OD demands of one person each. A reachable demand traverses a 127-edge route. These are bounded group demands, not individual resident agents.

| Case | Assigned | Unserved | Max people on one edge | Max/mean travel seconds | Assignment result SHA-256 | First assign / repeat |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| normal | 512 | 0 | 512 | 127000 / 127000 | `b78cbab1affbfb1aca364109ef48b0c568d27674f5b382cd3dd3ca2a66756c71` | 42,427,750 / 35,069,792 ns |
| preferred lane closed | 512 | 0 | 512 | 127000 / 127000 | `5c5c1528038c65c93d2213563a7f8ca261381357a5678498f9fea2693ba85094` | 33,056,792 / 32,910,542 ns |
| congested | 512 | 0 | 128 | 146050 / 146050 | `e6d3295e6b4245516e16340eb18f76ef7d2af35c60f9adff882b1422f93d49bb` | 34,659,750 / 35,555,750 ns |
| no open road | 0 | 512 (`NoOpenRoute`) | 0 | n/a | `a728f0a946fb5fcd13a458a216ce6b3e487e25b20334bfac4a40356766ab1a43` | 903,333 / 874,667 ns |

The congested case respects per-edge capacity: 512 people spread over the four fallback lanes yields at most 128 on an edge. Closing the preferred lane routes through the open fallback. With every road closed, all demands are explicitly unserved. Every case reports deterministic repeat equality. Full per-case fingerprints, flow counters, paths, timings, and receipts are in `runs/132-transport-capacity-measured/`.

## Memory and timing interpretation

The maximum path payload is 65,024 edge indices × 4-byte `u32` = **260,096 bytes**. The previous 130,048-byte estimate incorrectly assumed two-byte indices and is superseded. In the final run, the complete Assignment's owned `Vec` capacities were 313,344 bytes for normal, closed-lane, and congested cases, and 61,440 bytes for no-open-road; the Opening input Vec capacities summed to 39,936 bytes. The inline Assignment value was 192 bytes. These are `Vec.capacity × size_of(element)` measurements, not total allocations. Allocator metadata, caller copies, temporary scratch, simultaneous lifetimes, and peak allocation were not measured.

The four first assignments totaled 111,047,625 ns; deterministic repeats totaled 104,410,751 ns. The suite's internal wall time was 217,445,792 ns. The wrapper recorded 0.752079792 seconds including process startup/reaping; `/usr/bin/time -l` printed 0.73 seconds for the complete CLI, RSS 3,686,400 bytes, and peak footprint 2,949,432 bytes. This whole-process measurement is neither an incremental transport allocation measurement nor a clean performance gate. It cannot be extrapolated to a full city or used to validate 16 GB.

## Separate withheld decision

`scoped-index-withheld/decision.json` records the separate scoped-index candidate and runs 124–126. That candidate is withheld and not merged. Its full-workload physical saving was not measured; the recorded reduction in one native-owned counter is offset by increased padding, and the decode workspace remains 64 MiB. The old full L1 compatibility contract also differs and was not demonstrated by a successful restore. This decision is context only and is not part of the transport probe or evidence for its memory results.

## Evidence retained

The `runs/` directory holds compact command, exit, source-manifest, stdout, and stderr records for runs 127–132 and 135. The independent arithmetic/status report and binary descriptors are included. Original source archives, executables, and any full restore inputs remain at their original external-SSD locations; none were deleted or represented here by hashes alone.

## Stage retention

Four terminal source archive paths were verified byte-identical and unopened, then consolidated as hard links. Complete original paths and bytes remain available. File allocation fell by 12,828,672 bytes; observed SSD free space rose by 12,787,712 bytes, which is not claimed as exclusive APFS reclamation. No restore input, failure evidence, executable, source or shared target was deleted. See `stage-retention.json`.
