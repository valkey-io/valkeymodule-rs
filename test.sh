#!/usr/bin/env sh
# TODO cargo test --all --all-targets --no-default-features
rm dump.rdb
cargo test --all --no-default-features
