#!/usr/bin/env sh
set -eu

cleanup_profraw() {
    rm -f *.profraw
}

# Raw LLVM profiles are intermediate artifacts. Remove them whether coverage
# succeeds, fails, or is interrupted.
trap cleanup_profraw EXIT HUP INT TERM

# Example modules execute inside valkey-server processes, outside Cargo's test
# process. Export LLVM coverage settings before building and launching them.
eval "$(cargo llvm-cov show-env --sh)"

cargo llvm-cov clean --workspace
cargo test-shims
cargo build-examples --release
cargo test-integration --release

# show-env uses the project's existing target directory instead of
# cargo-llvm-cov's default report directory.
env CARGO_LLVM_COV_TARGET_DIR="$PWD/target" CARGO_LLVM_COV_BUILD_DIR="$PWD/target" \
    cargo llvm-cov report --open
