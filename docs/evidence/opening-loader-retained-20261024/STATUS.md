# Retained opening input and bounded profile decoding — 2026-10-24

Finite implementation/validation, not full #120 or product acceptance. Canonical base is
v0.5.158 ae28b8c plus protected F/E/M0 WIP and the coordinator's minimal #101 export/#118
metadata getter. Original issue tree and initial/failed source versions remain recoverable.
No commit/tag/PR/push, UI/rendering/GPU or long-pressure task was performed.

## Final behavior

OpeningInput transports at most64KiB of exact original bytes and digest/reader description.
Loading does not accept the schema or grant permissions. Initialization borrows the caller's
existing EngineSlot and actual Resource slice, reuses its original assembler/resource checks,
and yields a read-only view only after success. Successful slots reject reinitialization
without mutation; failed original initialization resets the unpublished slot. Source bytes
survive failure/retry. No extra World, slot, physical capacity pool, or publication is created.

The original direct World-return API overflows the default test stack even on small inputs.
Retained initialization avoids that path. A separate town-utility overflow was isolated to
Opening serde decode. Changing only MarketProfile to an aggregate map visitor was insufficient.
The accepted shared fix uses bounded model_rows::Rows scratch for profile tables, completes
serde decoding, then converts once to the unchanged inline FixedRows representation. All original
field names, required/default/null/duplicate rules, row bounds/order, overflow token and semantic
validators remain. No unsafe, global/test stack enlargement or frozen M0 limit change is used.

## Actual finite results

| Verification | Result | Evidence in v8-profile-scratch |
|---|---|---|
| Six current loader oracles, including seven fixture direct/bytes/reader comparisons |6/6;14 historical diagnostics ignored|final-opening-loader.log|
| Old-derived wire mirror: defaults/nulls, required/unknown/duplicates, seven table N/N+1 and distinct row order |4/4|final-wire-order.log|
| Correctly selected town runtime cursor/legacy state tests |2/2|corrected-runtime.log|
| Existing business exit rules |13/13|final-business-exit.log|
| Existing utility binding rules |6/6|final-utility-binding.log|
| Bounded scratch type-layout diagnostic |1 diagnostic, not functional/native peak|scratch-layout.log|

31 functional tests above pass. The historical seven stack diagnostics and seven fixture
localization tests remain separately ignored; none replaced the six current loader oracles.
Original v1/v2/v4 aborts, v3/v5/v6 localization, v7 insufficient fix, and test-author compile
errors are preserved. The original final-runtime.log ran ZERO tests and provides no acceptance;
corrected-runtime.log is the actual2-test evidence. No full workspace/clippy pass is claimed.

Two broader regressions are unresolved: the equity contribution before-Timeline unit case and
the utility cross-city shared-pool/checkpoint integration case each abort on the default stack.
For each, replacing only town_runtime.rs with its exact pre-fix version under otherwise identical
WIP reproduces the same failure. This supports an existing unresolved source-state problem,
not full historical-release certification or proof every call path is safe. Original-file/current
executables and baseline logs remain. Further town transaction filters were not run after the
failed equity step; the rest of those suites must not be inferred passed.

## Resource accounting and remaining gates

On this actual 32GiB Mac16,1, bounded scratch requests39,680bytes for five mandatory table
backings, plus up to2,592bytes for two optional tables: max42,272bytes,5–7allocations.
ProfileDecode header is1,872bytes; unchanged inline MarketProfile is43,968bytes. These are
layout/request measurements, NOT RSS, native peak or allocator metadata. Conversion overlaps
remaining scratch with the final inline value/stack temporaries. Every MarketProfile Deserialize
caller, including applicable restore paths, pays this scratch cost; it is not opening-only.
No new budget grant,16GB pass or M0 performance pass follows from these numbers.

CLI/Store wiring must initialize and publish the SAME retained slot. Preinitializing through
adapter then calling old Store create would initialize twice and is not accepted. Actual user
permission/provider, non-laboratory input/#107 migration, genesis envelope hash/raw bytes,
create failure/retry, cold restore without capital reissue, workspace/allocator/native peaks,
and frozen joint/M0/true16GB gates remain pending. The raw opening digest is not Store's entire
Genesis-envelope digest. Current source/test-only inclusion is not production CLI integration.

## Evidence and retention

All evidence is under /Volumes/HP P900/dev/MyCity/work/parallel-issue120-validation-20261024/,
using the existing shared HP P900 USB/APFS target/work. Full starting canonical WIP source.zip
plus versioned deltas/manifests, original owner packages, fixed executables and all failures are
retained. This phase evidence planning cap is1GiB; observed allocated directory size was522MiB
before final small reports. No original worktree is released/deleted yet. Retain until same-slot
Store recovery and the recorded stack failures are resolved and a newer complete baseline has
been accepted; then review exact obsolete artifacts under the existing retention policy.
Do not treat file logical sizes as APFS recovered space or hash manifests as original bytes.

Separate defect tracking: [#122 equity](https://github.com/TekesApps/MyCity/issues/122) and [#123 utility](https://github.com/TekesApps/MyCity/issues/123). Neither is accepted or newly assigned by this increment.

## Later default-stack repair validation

The historical default-debug failures above are retained, but are no longer the latest
status: source09 of [the separate stack repair](../stack-faults-122-123-20261024/STATUS.md)
passed original equity/utility cases, release core209 and profile4, Store6parents with12 real
cold children, and default-debug capacity read5. Original advisor accepted these finite
results. Older failures and fixed executables stay recoverable. Native peak, full issue and
M0/16GB gates do not become passes; publication remains separately coordinated.
