# r07-sim-open-01 same-frame attribution at 1.533 s

This read-only attribution uses the preserved small `observer-report.json`, line 6 (zero-based) of `host-samples.jsonl`, and the layout manifest. It does not read pressure payloads, start processes, or alter original evidence. The compact fields are in [347-1533ms-attribution.json](347-1533ms-attribution.json).

At observer elapsed `1.533054667 s`, the complete native capture measured `3,341,357,208 B`; the manifest's `normal_joint_budget_bytes` is `3,328,599,654 B`. The same sample contains the R07 reading sequence 7. Its observer, supervisor, and driver footprints sum to `6,423,464 B`. The same-frame arithmetic is therefore:

`3,341,357,208 - 3,328,599,654 - 6,423,464 = 6,334,090 B`.

This identifies the run and frame from which the 6,334,090-byte residual was derived. It is candidate arithmetic, not measured reclamation. In that same sample, the applied provider state is still epoch 0 with `replaced_bytes: 0`, reading sequence 5, and `permitted_bytes: 5,046,896`; it does not reflect the sequence-7 component sum. No physical unmapping is demonstrated by the residual calculation.

This run is distinct from `j3q-01` (source `a6290a...`) whose 910.510 s frame measured `3,355,234,768 B` and whose separate hypothetical three-role remainder is `8,660,930 B`. Those values must not be combined or used to challenge the r07-sim-open-01 arithmetic. The original j3q evidence remains unchanged.

The 16 GB and eight-hour gates remain unrun; full M0 remains unaccepted.
