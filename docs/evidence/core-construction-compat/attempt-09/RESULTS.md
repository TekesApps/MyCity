# Construction legacy compatibility validation

Source bundle: `source.zip`, SHA-256 `d8731055a3e4bc4130a906d5360a98e9cba32e0df9c7d06efb98d572b51fb906`.

The source was frozen before Cargo execution. The authentic v106 producer artifacts and probe are embedded unchanged from attempt-01. Rust/Cargo were 1.85.1 on aarch64-apple-darwin; all commands used the shared `/Users/frankqin/dev/MyCity/target`, `--release --locked --offline`.

Results:

- `cargo test -p mycity-core -p mycity-storage --all-targets`: 273 passed, 0 failed across 30 test suites. Raw output: `core-storage-all-targets.stdout`.
- Targeted reused restore-slot test: 1 passed. Raw output: `reused-slot.stdout`.
- Targeted authentic v106 cold-open test: 1 passed. It verifies Engine hash, exact legacy serialized world view, balances, stock lot, capacity occupancy, journal hash, sequence and replayed receipt. Raw output: `storage-v106.stdout`.
- Strict Clippy for core and storage all targets: passed with `-D warnings`. Raw output: `clippy.stdout`.
- `git diff --check`: passed before freeze.

Attempt-08 records the initial bad assertion: the probe's top-level `state_sha256` was the genesis world hash, not the post-transaction World hash. The Engine hash was already exact. Attempt-09 compares the complete recovered WorldView to the authentic post-transaction `storage_probe.world` payload instead. This correction changed only the test oracle; historical bytes were not synthesized or modified.

The v106 probe includes null checkpoint fields, so this validates authentic v106 genesis/root/WAL cold-open. Reused restore-slot handling is validated separately by the core unit test using a prior-shape object with the optional field absent; it does not claim the v106 producer emitted a non-null checkpoint.
