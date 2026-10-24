# Issue #120 local opening source provider candidate

Status: source-only handoff. All Cargo, rustc, test, formatting, script, benchmark, and GPU
validation is `not_run`. This adds a provider API for future CLI wiring; it does not implement or
claim an `init-opening` command.

## Worktree and ownership

- Repository: `/Users/frankqin/dev/MyCity`
- Reused worktree: `/Users/frankqin/.codex/worktrees/issue120-opening-loader/MyCity`
- Branch: `codex/issue120-opening-loader`
- Worktree base remains `037f3cf89755430bd8cc56922d8f673497bdf6c3` (v0.5.149). Main had advanced
  to v0.5.161 while this isolated source candidate was authored; root owns reconciliation against
  current main and CLI wiring.
- New files for this provider increment:
  `crates/city-headless/src/opening_source_provider.rs`,
  `crates/city-headless/src/opening_source_provider_tests.rs`, this handoff.
- The earlier #120 opening-loader candidate files remain unchanged. No CLI/main/lib root, Cargo,
  city-core, city-storage, Store, workspace, or existing shared module was modified.

## API

- `read_local_file(path) -> Result<OpenedOpeningInput, String>` opens the explicitly selected path
  with safe `rustix::fs::open` using `RDONLY | NONBLOCK | CLOEXEC` on Unix, then requires the
  opened descriptor to be a regular file and reads through the existing
  `OpeningInput::read_named_json` under `INPUT_LIMIT`. It returns the original `OpeningInput` plus
  `OpenedFileProvenance` (constant label `local_file_os_read`, canonical path, opened-file
  identity stamp, actual byte count and SHA-256 of those bytes).
- `load_resources_file(path) -> Result<LoadedResourceManifest, String>` reads through the same
  bounded provider path with a 4096-byte limit. The new candidate JSON schema is
  `mycity-resources-v1` with `resources: [{id, capacity}]`, at most `MAX_RESOURCES` entries,
  `deny_unknown_fields` at each schema layer, nonzero unique IDs, and `Resource::new` for output.
  Explicit `resources: []` is representable. The manifest cannot specify `reserved` or `occupied`;
  initial resource capacity does not itself authorize either state. Existing slot cross-reference
  validation remains authoritative.
- `read_local_bytes(path, limit) -> Result<OpenedLocalBytes, String>` returns one original byte
  buffer with the same actual-file provenance. The explicit limit must be in `1..=16 MiB`.
  CLI callers should pass the existing 64 KiB limit for spatial JSON and the existing terrain
  `SOURCE_BYTE_LIMIT` for PNG. This does not change normal spatial/terrain acceptance limits.
- `OpenedLocalBytes::bytes()` and `provenance()` provide borrowed access so the CLI can pass
  original byte slices directly into the existing Store spatial constructor without another
  input Vec.
- `OpenedFileProvenance::to_json()` provides diagnostic JSON. Provenance labels OS-read origin
  only; they do not prove permission, ownership, asset rights, business qualification, or legal
  authority.

All paths and canonical paths are bounded at 4096 OS-string bytes. Unix FIFO inputs are opened
nonblocking and rejected by the regular-file gate without waiting for a writer. Non-Unix builds
fail closed with `opening_source_platform_unsupported`; equivalent nonblocking-open behavior is
not claimed there. The provider compares handle
metadata and canonical-path metadata before/after reading and rejects an observed change. On Unix,
file identity includes device and inode. Other platforms report canonical path plus size/time and
therefore have weaker identity evidence. Same-length writes that restore the observed timestamp
can evade metadata comparison; the recorded SHA-256 still describes exactly the bytes returned by
the read and is not a claim that all concurrent mutation is detected.

## Contract and unverified integration

The provider reads only an explicitly supplied path and never falls back to fixture/default data,
stdin, sockets, network, or implicit resource values. Open/read/size/metadata errors return before
the CLI can invoke Store. `OpeningInput` preserves the exact opening bytes; malformed JSON,
unsupported schema, and model/typed-reference errors remain for the established initialization
gate to report. A missing required spatial bundle must continue to be rejected by the existing
Store gate: this source provider does not produce provider permissions or waive that gate. There
are no charges, licenses, leases, or capital side effects here.

The resource manifest schema is a new candidate contract. Root must synchronize the design
document and issue text before treating this format as adopted. It is not an existing file format
or evidence that resource rights/occupancy are satisfied.

The main owner is expected to wire a positional `init-opening` CLI command, pass the returned
`OpeningInput`/resources/optional spatial byte slices to the existing `Store::create_opening_input`
or `create_opening_input_spatial` API, and preserve all errors. This worker did not touch that
command path. Store single-slot publication/recovery gate was already validated by the main owner;
the provider has not been joined to it here.

## Tests and future validation

The dedicated test source lives under `src/` and is included by `#[cfg(test)]` from the provider
module, so Cargo does not discover it as a separate integration-test target. The Unix FIFO case
uses `/usr/bin/mkfifo` and runs the provider probe in a child process with a five-second parent
timeout, so a regression cannot hang the main test process. Authored cases cover actual
regular-file bytes/hash and identity, empty/nonregular/missing/oversized paths, FIFO rejection
without a writer, exact-byte preservation for malformed/future
JSON, bounded raw-byte reads, metadata-change rejection, and strict resource schema/ID/count rules.
Every case is `not_run`.

Future validation after root wires the module and schedules the shared target:

```sh
CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --locked --offline -p mycity-headless --bin mycity-headless opening_source_provider
```

Then root owns sequential end-to-end `init-opening` validation using a real explicit file, one
Store creation, optional spatial bundle, and fresh recovery. Preserve the source fixture/artifacts
until that unified evidence is archived. Provider source/test/handoff are small; exact incremental
disk and runtime memory costs are unmeasured. No separate target, benchmark, or integration
artifact was created.

PR: none created. Issue #120 remains open. This is not runtime, Store, CLI, or product acceptance.
