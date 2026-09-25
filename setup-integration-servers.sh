#!/usr/bin/env bash

# Prepare the Redis and Valkey binaries used by the SDK integration tests.
# Usage: ./setup-integration-servers.sh
# Requires Git, Make, a C build toolchain, and OpenSSL development files.
#
# Reads repository/branch entries from integration-servers.conf and clones them
# into tmp/integration-servers/<directory> relative to this script. Builds use
# make noopt BUILD_TLS=yes MALLOC=jemalloc (unoptimized, with TLS and jemalloc).
# Existing checkouts must be on the configured branch and are updated with
# git pull --ff-only. A build is reused when its commit matches the build marker
# and its server executable exists; otherwise the engine is rebuilt.
#
# Add versions in integration-servers.conf, then rerun this script. Setup needs
# network access to clone or update engines; it does not build SDK examples or
# run tests. Run ./test.sh afterward to build and test the configured matrix.

set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly ENGINE_ROOT="${SCRIPT_DIR}/tmp/integration-servers"
readonly BUILD_MARKER=".valkeymodule-rs-build-noopt-tls-jemalloc"

clone_and_build() {
    local name="$1"
    local repository="$2"
    local branch="$3"
    local server_binary="$4"
    local checkout_dir="${ENGINE_ROOT}/${name}"
    local marker_path="${checkout_dir}/${BUILD_MARKER}"

    if [[ ! -d "${checkout_dir}/.git" ]]; then
        if [[ -e "${checkout_dir}" ]]; then
            echo "error: ${checkout_dir} exists but is not a Git checkout" >&2
            return 1
        fi

        git clone --depth 1 --branch "${branch}" "${repository}" "${checkout_dir}"
    else
        local current_branch
        current_branch="$(git -C "${checkout_dir}" branch --show-current)"
        if [[ "${current_branch}" != "${branch}" ]]; then
            echo "error: ${checkout_dir} is on branch ${current_branch}, expected ${branch}" >&2
            return 1
        fi

        echo "${name}: checking branch ${branch} for updates"
        git -C "${checkout_dir}" pull --ff-only origin "${branch}"
    fi

    local current_commit
    current_commit="$(git -C "${checkout_dir}" rev-parse HEAD)"

    local built_commit=""
    if [[ -f "${marker_path}" ]]; then
        built_commit="$(<"${marker_path}")"
    fi

    if [[ "${built_commit}" == "${current_commit}" && -x "${checkout_dir}/${server_binary}" ]]; then
        echo "${name}: commit ${current_commit} is already built (${checkout_dir}/${server_binary})"
        return
    fi

    echo "${name}: building branch ${branch}"
    make -C "${checkout_dir}" noopt BUILD_TLS=yes MALLOC=jemalloc

    if [[ ! -x "${checkout_dir}/${server_binary}" ]]; then
        echo "error: build did not produce ${checkout_dir}/${server_binary}" >&2
        return 1
    fi

    printf '%s\n' "${current_commit}" > "${marker_path}"
    echo "${name}: built commit ${current_commit} (${checkout_dir}/${server_binary})"
}

mkdir -p "${ENGINE_ROOT}"

while IFS='|' read -r name repository branch features; do
    [[ -z "$name" || "$name" == \#* ]] && continue
    clone_and_build "$name" "$repository" "$branch" "src/${name%%-*}-server"
done < "${SCRIPT_DIR}/integration-servers.conf"

echo
echo "Integration servers are ready under ${ENGINE_ROOT}"
