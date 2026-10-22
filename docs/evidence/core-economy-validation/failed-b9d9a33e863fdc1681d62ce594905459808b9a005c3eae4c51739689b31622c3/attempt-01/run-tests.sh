#!/bin/zsh
set -o pipefail
export CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target
cargo +1.85.1 test -p mycity-core --lib --release --locked --offline town_economy:: 2>&1 | tee cargo-test.raw
