#!/usr/bin/env zsh
set -eu

setopt NULL_GLOB

rm -f dump.rdb
rm -f **/*.profraw

cargo test-shims

integration_targets=(
    "Redis 7.0|min-redis-compatibility-version-7-0,use-redismodule-api"
    "Valkey 7.2|min-redis-compatibility-version-7-2"
    "Valkey 8.1|min-valkey-compatibility-version-8-0,min-redis-compatibility-version-7-2"
    "Valkey 9.1|min-valkey-compatibility-version-9-0,min-valkey-compatibility-version-8-0,min-redis-compatibility-version-7-2"
)

for target in "${integration_targets[@]}"; do
    server_name="${target%%|*}"
    features="${target#*|}"
    echo "Running integration tests on ${server_name}"
    cargo build --examples --release --no-default-features --features "${features}"
    cargo test --test integration --release --no-default-features --features "${features}" -- --test-threads=1
done

cargo test-docs
