# R07 physical extent experiment

## Purpose and frozen contract

This branch changes one variable: the physical allocation granularity for R07
immutable padding. Physical anonymous mappings hold at most 16 adjacent logical
quanta (1 MiB); the final mapping may be shorter. Logical accounting remains
64 KiB per quantum. The existing SplitMix64 generator seed reset for every
logical index, global logical index, total payload, `State.schema = 1`,
`quantum_bytes = 65536`, `mapped_bytes` / `replaced_bytes` rules, and R07 payload
bytes within v3 snapshots are unchanged. No pressure load, threshold or credit
rule is changed. The extent-list capacity retains the conservative former bound
of one slot per logical quantum, so metadata capacity is not reduced to the
usual sixteen-quanta grouping.

Maps are shared by `Arc` with snapshots. A pinned mapping cannot be split or
credited as released. On an unpinned interior shrink, the retained prefix is
allocated and copied first; only after the replacement exists is the prior map
dropped. The diagnostic high-water `r07_transition_peak_bytes` records the
replacement map length and is bounded by 1 MiB per transition. This is not an
aggregate payload or OS-footprint upper bound: old and new maps coexist while a
replacement is prepared, and every committed partial-refill payload remains in
`State.mapped_bytes`. On growth,
an unpinned partial final extent is replaced by a larger extent with its prefix
copied and only the newly added logical suffix generated. If the prior extent is
pinned, the missing suffix is appended without copying its prefix. A generation
is allocated per physical map. VM range records use physical address and length,
plus `logical_start` and `logical_len` byte intervals; these addresses are
diagnostic only and do not create physical-memory credit.

R07 payload serialization and verification iterate the same 64 KiB logical
sequence, and prefix restore compares each logical quantum. The R07 payload
bytes, `State` fields, and v3 layout format remain compatible; the top-level
layout manifest adds `r07_transition_peak_bytes`. The typed restore-index
manifest reader ignores this unknown diagnostic field. The experiment does not
claim the complete serialized metadata or full snapshot bytes are unchanged.

## Focused validation

The focused test passed against source fingerprint
`9d8e5f2d8d04ff05ff68454a52e47df0b247edb2cac5beabe10ad3f283c7f7e1`:

```sh
cargo test -p mycity-headless layout_load::r07::tests --lib
```

Result: 9 passed, 0 failed. It covers exact byte equality against the prior generator for every logical
quantum, physical extent count, interior split and partial refill, pinned
snapshot retention without false replacement credit, the 32-to-31 and 20-to-17
interior-shrink regressions with per-quantum byte comparison, old snapshot
retention and cold prefix restore, generation changes, payload byte counts,
schema-1 state, v3 R07 payload stream verification and malformed state
rejection. Allocation failure injection also covers a 35-to-31 shrink after
the trailing 3-quanta extent is dropped and a multi-extent refill that commits
one extent before the next allocation fails. Both assert actual mapped bytes,
replaced bytes, epoch and sequence; a retry resumes from that reported state.
Trace interval assertions check contiguous non-overlapping logical byte ranges
whose lengths sum to `mapped_bytes`; pinned snapshots assert their physical map
addresses and allocation generations remain unchanged.
The `refresh_r07` caller revokes the old `r07-state.json` acknowledgement before
attempting a map change, then propagates errors before publishing a replacement
acknowledgement. Strict package clippy also passed:

```sh
cargo clippy --release --locked --offline -p mycity-headless --lib -- -D warnings
```

The first clippy attempt exposed a dead-code warning for the extent byte limit;
the production allocator now checks the checked-multiply length against that
limit. The failed attempt log is retained beside the passing rerun. Follow with
the existing crate tests
that exercise layout v3 persistence and VM range diagnostics, then the frozen
M0-T13-A workloads and whole-group measurement. Those broader checks remain
owned by the primary build window; no performance result is inferred from unit
tests or source inspection.

## Build and disk discipline

Rust 1.85.1 and the fixed `Cargo.lock` were used in release, locked, offline
mode. Builds reused `/Users/frankqin/dev/MyCity/target`; its measured allocated
size grew from 2,209,460 KiB to 2,209,532 KiB (+72 KiB). The workspace-local
verification receipts and four attempt logs occupy 24 KiB. No independent
target/cache was created. The shared target is retained because other workspace
validation depends on it; no cleanup was done during this functional-validation
window. Current filesystem availability after validation was 89,043,396 KiB;
an exact pre-run available-space sample was not captured, so this is not reported
as reclaimed space.

Receipts and logs are in `verification/r07-extent-build/`: source freezes for
both the initial and corrected source, two focused-test attempts, and both
strict-clippy attempts. The initial clippy failure is retained for the audit
trail. No large L1 pressure/performance run was started. These results do not
claim 16 GiB-device, formal M0, or eight-hour acceptance.
