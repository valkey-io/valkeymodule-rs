#!/usr/bin/env sh
# TODO cargo test --all --all-targets --no-default-features
# cargo test --all --no-default-features
cargo build --all-targets && cargo test --all-targets --features enable-system-alloc

