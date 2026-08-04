#!/usr/bin/env sh
set -eu

rm -f dump.rdb
rm -f *.profraw

cargo test-shims
cargo build-examples --release
cargo test-integration --release
cargo test-docs
