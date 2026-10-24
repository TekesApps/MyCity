# #120 Store same-slot publication and recovery — 2026-10-24

## Actual finite result

Canonical HEAD remains ae28b8cda8041c7bd106a28cccdc3deab051b032 with protected uncommitted WIP. Root integrated OpeningInput through the original single Store factory and original initial EngineSlot, preserving raw opening bytes, original genesis envelope, Scene validation, WAL/CURRENT and restore. The town fixture requires the original spatial bundle; the spatial entry point supplies it without bypassing validation. No second initial workspace or second capitalization pass is introduced.

Serial release validation on Mac16,1, 32GiB, external HP P900 APFS/USB used the existing target/work:

```sh
cargo test --release --locked --offline -p mycity-storage --test opening_input_store --no-run
MYCITY_OPENING_INPUT_STORE_ROOT="/Volumes/HP P900/dev/MyCity/work/parallel-issue120-store-20261024/rootrun-01" \
  "/Volumes/HP P900/dev/MyCity/work/parallel-issue120-store-20261024/opening_input_store-fixed-01" \
  --nocapture --test-threads=1
```

Build exit0; six parent tests passed, zero failed. The normally ignored cold-open helper was explicitly invoked in 12 actual fresh child processes; all exited0 and each reported one passing helper test. Root independently checked their different PIDs, literal accounts/opening sources/stock ownership/lot/origin/offset values, transaction delta and root sequence against the static fixture expectations. E0 retains the actual 137-cent transfer after WAL replay, checkpoint and rejected recreation; capital is not reissued. Boot/town checkpoint and cold open, raw-byte legacy entry, exact whitespace-preserving genesis bytes, root hash and prepared backing-pointer continuity are covered.

GenesisSynced/RootSynced injection leaves no CURRENT; retries use new directories. CurrentRenamed error is uncertain publication and is resolved by opening that existing directory, without blind recreation. Invalid schema/resources and absent required spatial bundle cannot publish. These injected failures are not physical power-loss or full filesystem fault testing.

## Fixed source and evidence

Durable directory: `/Volumes/HP P900/dev/MyCity/work/parallel-issue120-store-20261024/`.

- `effective-source-01.zip` and manifest retain all 341 effective input files; source SHA256 `41304bbc7149cd03eee59141f3b538e1153b66a1fc6e9b979549c5d176081fdd`.
- `build-01.log`, `test-01.log`, `result-01.json` retain actual commands/results and all child statuses/logs, ledger checks and per-file hashes.
- Fixed executable SHA256 `39290148e2bde4c446c040ceeb7bc60d28b23e2d922697736887e91e8d651742`; actual and archived bytes remain equal after execution. All 341 source files also remain unchanged.
- Six unique case directories retain 119 files, 293,121 logical bytes. No world, failed attempt, original WIP, old executable or recovery evidence was deleted.

The 64MiB artifact allowance is post-write logical accounting, not a hard quota. Child output collection and full genesis decoding do not prove bounded peak memory. No CPU/GPU throughput, 16GB hardware, production provider, CLI input selection, real authorization, complete migration or formal M0 pass is claimed.

## Review and remaining work

Original advisor reviewed the actual build, six parent results, 12 child logs/PIDs and retained artifacts: finite same-slot Store handoff accepted, no new blocker. This review was read-only and ran no tests. Issue remains open; no PR, commit, push or release in this phase. Independent default-stack defects #122/#123 and the #118 debug Store-creation abort remain unresolved. Frozen M0 load and standards are unchanged. Historical #101 worlds require their matching archived CLI/source identity; a newer executable must not silently replace it for restore.

## Later default-stack repair validation

The historical default-debug failures above are retained, but are no longer the latest
status: source09 of [the separate stack repair](../stack-faults-122-123-20261024/STATUS.md)
passed original equity/utility cases, release core209 and profile4, Store6parents with12 real
cold children, and default-debug capacity read5. Original advisor accepted these finite
results. Older failures and fixed executables stay recoverable. Native peak, full issue and
M0/16GB gates do not become passes; publication remains separately coordinated.
