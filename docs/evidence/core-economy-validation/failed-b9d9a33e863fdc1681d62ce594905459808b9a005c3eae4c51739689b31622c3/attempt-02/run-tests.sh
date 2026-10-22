#!/bin/zsh
set -o pipefail
export CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target
cargo +1.85.1 test --manifest-path /Users/frankqin/dev/MyCity-core-work/economy/Cargo.toml -p mycity-core --lib --release --locked --offline town_economy::tests:: 2>&1 | tee /Users/frankqin/dev/MyCity/work/evidence/core-economy/b9d9a33e863fdc1681d62ce594905459808b9a005c3eae4c51739689b31622c3/attempt-02/cargo-test.raw
