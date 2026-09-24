#!/usr/bin/env zsh
set -eu

setopt NULL_GLOB

rm -f dump.rdb
rm -f **/*.profraw

cargo test-shims

while IFS='|' read -r server_name repository branch features; do
    [[ -z "$server_name" || "$server_name" == \#* ]] && continue
    echo "Running integration tests on ${server_name}"
    cargo build --examples --release --no-default-features --features "${features}"
    INTEGRATION_TEST_SERVER="$server_name" cargo test --test integration --release --no-default-features --features "${features}" -- --test-threads=1
done < "${0:A:h}/integration-servers.conf"

cargo test-docs
