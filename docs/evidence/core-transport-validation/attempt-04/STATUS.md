# 0.1 Traffic model validation

Source SHA-256: `b87c6cdb369d96d0f59f65c887be7b553b5fef01f9c1894d4c2924c750775a78` (222 frozen source files; Rust/Cargo 1.85.1; lock SHA-256 `7da5c54a8499a07cbc71dcecce0c0095d4af3b8b6e5052783f20d188037b6dd6`). HEAD `1f12916fe53512f6df74d6d1f12b01384f4d34ad` plus the recorded worktree patch and untracked list. Shared target: `/Users/frankqin/dev/MyCity/target`. No Cargo process remains after this stage.

## Validation

- `cargo test --release --locked --offline -p mycity-core 'transport::' -- --nocapture`: exit 0; 13 transport tests ran and passed. Other crate test harnesses ran with 0 tests because the filter excluded them. Final attempt output: `transport-tests.stdout` and `.stderr`.
- `cargo clippy --release --locked --offline -p mycity-core --all-targets -- -D warnings`: exit 0, 3.99s. All-targets includes the example.
- Fixed release example binary: `/Users/frankqin/dev/MyCity/work/candidate-binaries/core-transport/b87c6cdb369d96d0f59f65c887be7b553b5fef01f9c1894d4c2924c750775a78/transport`, SHA-256 `52dc970850f7c2732d24b6a13f9c6b6bccf640b67bfc87e379ea507893472dec`.
- Four direct executable scenarios exited 0: normal served 8/8; closed road served 20/25 and labeled 5 `CapacityExhausted`; congested road served 20/20 and final cost rose 60→69 seconds; unreachable served 0/5 and labeled 5 `NoOpenRoute`. Full raw JSON is `cli-scenarios.json`.
- Source digest matched before/after tests, strict clippy, example build and direct CLI execution.

## Attempts retained

Attempt 1 ran 13 tests (10 pass, 3 fail): exposed the BPR fourth-power arithmetic defect and incorrect cross-city test labels. Attempt 2 ran 13 (12 pass, 1 fail): exposed a remaining inconsistent incoming-node city label in that test. Attempt 3 ran all 13 tests successfully; strict clippy exposed one test-style lint (`find().is_none()`). Attempt 4 fixes that lint and is the validated source above. Raw failures remain in each attempt directory.

## Bounds and limits

The model admits at most 128 nodes, 512 directed edges, 128 entrances, 512 OD groups and 512 aggregate path allocations. The code-bound estimate is at most about 67.2 million directed-edge scans in the extreme case, plus bounded node selection/index work and at most 65,024 edge-index references in final-cost recomputation. Scratch/returned-request estimate is below about 450 KiB, with a transient canonical fingerprint serialization below about 256 KiB; allocator overhead and caller-owned input are not included. These are structural estimates, not measured resident memory or performance acceptance. Assignment is stable ID-ordered loading, not a global equilibrium model. This result is not M0-T13-A, 16GB-device, formal performance, or real economy/traffic integration acceptance.
