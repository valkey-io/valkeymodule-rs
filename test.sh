#!/usr/bin/env sh
set -eu

rm -f dump.rdb

cargo test-shims
cargo build-examples --release
cargo test-integration --release
cargo test-docs
