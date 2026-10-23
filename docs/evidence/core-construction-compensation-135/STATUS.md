# Construction compensation validation status

Date: 2026-10-23

The compensation contract and its limits are described in [`docs/construction-compensation.md`](../../construction-compensation.md). Validation is terminal for the frozen candidate below; earlier failed attempts are retained for provenance and are not counted as passing runs.

## Frozen source and final validation

The final fixed-source identity is `e7743bfdc17de3fbd640085f2c2af65ecad993f010d2ce9c83a6873636e49baf` (266 source files, 278 fixed input files). The seven final validation steps, 142–148, report exit code 0 and `source_unchanged: true`; their command, source manifest, stdout, stderr, and exit receipts are retained under [`runs/`](runs/), with per-step manifests; the complete shared source archive remains at the external path in `compensation-joint-binary.json`. [`copy-index.json`](copy-index.json) binds the copied evidence to its original external-work paths and hashes.

- **142 Store tests:** 48 passed (21 library and 27 integration tests), including the compensation cold WAL/checkpoint and retry cases.
- **143 strict Clippy:** all workspace targets passed.
- **144 release libraries:** 142 core tests and 93 headless tests passed.
- **145 Engine tests:** 16 `construction_engine` tests passed, including checkpoint tampering that rejects an unresolved historic payee, an initial paid Review forged as demolition round 2 without `BeginDemolition` context, and a paid Project forged back to Draft.
- **146 formatting:** passed.
- **147 joint CLI build:** `town_lifelines` built successfully; the retained executable SHA-256 is `c48b6378b77cb3847abf46a930cf96f380295881fbb187040947c6ed5d775e03`.
- **148 joint CLI:** all seven cases exited 0; stdout matched the run 120 baseline (release v0.5.133) byte-for-byte. The copied case logs and [results.json](runs/148-compensation-joint-cases/cases/results.json) are retained.

The independent validator [`compensation-independent.json`](compensation-independent.json) reports `passed` across 299 distinct final tests, exact owner/payee cash transfer, conserved total money, one successful operation receipt, the paid marker and `Project.spent`. Its independent compensation checkpoint confirms owner account 4 paid 50,000 to payee account 2, records `spent=50000` and operation `700006`, and contains exactly one successful receipt for that operation. The validator, its script, binary manifest, and joint runner are retained beside the report.

The two original Store worlds (14 files) remain preserved under [`business-originals/`](business-originals/) with the file-level SHA-256 inventory in [`business-originals.json`](business-originals.json); their originals remain at the recorded external HP P900 work paths. Generated CLI world directories were intentionally not duplicated into this evidence package.

Historical test-only corrections before that terminal result were:

- Run 133: an eight-element Gate test fixture had used an incorrect array length.
- Run 136: a test used a private account-handle helper instead of the public reference.
- Run 137: a pure Project fixture marked a project Operational without the corresponding occupied-site fields.
- Run 138: an Engine-only checkpoint test expected Store-archive replay behavior; the Engine correctly returned `StaleRevision` for that old request.
- The protected-cash scenario's insufficient available balance correctly returned `InsufficientFunds`, not `Protection`; only the test expectation changed.

These were test setup/oracle corrections; they did not relax core authorization, accounting, or recovery checks.

## Earlier failed attempts retained

Runs 140 and 141 are retained but are not counted as validation passes. Run 140 selected `construction_engine` in `mycity-storage`, where the target is named `construction`; run 141 stopped at compilation because two test call sites used a private account-handle API. Those test-only issues were corrected before frozen-source run 142. Runs 133, 136, 137, and 138 also remain available under `runs/` to show their fixture/oracle corrections.

The joint build and seven CLI outputs use the same frozen source identity. The CLI outputs were independently compared with the earlier run 120 baseline (release v0.5.133); all seven stdout files were byte-identical.

## Separate diagnostics and limits

[`runtime-residual-attribution/report.md`](runtime-residual-attribution/report.md) is included as a narrow M0 memory-accounting diagnostic. It attributes no bytes and credits no savings; the measured group remains 7,628,426 B above its unchanged normal threshold and unresolved. This is not a performance or acceptance pass.

The validation proves the bounded construction compensation contract and tested Store/Engine recovery paths only. It does not establish production legal negotiations, compensation appraisal, escrow, automatic refunds, market fairness, or a memory/performance pass. The external-storage test evidence describes one fixed fixture; it is not a calibrated economic result for a complete city.

## Independent review and stage retention

The advisor's final focused code review confirmed both reported restore gaps are fixed and found no further release blocker: the payee is always resolved, later Review requires demolition context, and the real Engine checkpoint positive case still restores. This was a read-only review; the execution counts above come from actual test receipts. Pure Project fixtures were corrected to include their occupied site/token prerequisites rather than weakening production validation.

Seven terminal final-source ZIP paths were checked byte-identical and unopened, then hard-linked to one complete external archive. File allocation decreased by 25,927,680 bytes; observed SSD free space increased by 25,853,952 bytes, not claimed as exclusive APFS reclamation. A redundant archive copy newly created in this source checkout was removed after byte comparison and an open-file check; its complete external original remains. That separate internal-disk allocation reduction was 4,321,280 bytes, with observed available-space increase 4,313,088 bytes. See `stage-retention.json` and `copy-index.json`.

All actual business/restore worlds, source/dependency bytes, executable binaries and failed-run evidence remain retained. The existing external shared target was reused. No M0 pressure payload or acceptance input was removed in this stage, and no retention operation ran during a formal measurement.
