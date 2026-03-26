#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./common.sh
source "${SCRIPT_DIR}/common.sh"

cleanup_runtime_dir() {
  if [[ ! -d "${GB_E2E_ROOT}" ]]; then
    return 0
  fi

  docker run --rm \
    -v "${GB_E2E_ROOT}:/cleanup" \
    --entrypoint sh \
    ghcr.io/gitbucket/gitbucket:4.44.0 \
    -c 'rm -rf /cleanup/* /cleanup/.[!.]* /cleanup/..?*' >/dev/null 2>&1 || true

  rm -rf "${GB_E2E_ROOT}"
}

compose down -v --remove-orphans
cleanup_runtime_dir
