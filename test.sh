#!/usr/bin/env sh
rm dump.rdb
cargo test --all --all-targets --no-default-features --features enable-system-alloc
