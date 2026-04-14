#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release-tag.sh <tag>

Example:
  scripts/release-tag.sh v0.1.0

This script:
  - validates the release tag format
  - checks Cargo package version matches the tag
  - ensures the git working tree is clean
  - runs release validation checks
  - creates the git tag locally
EOF
}

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 1
fi

TAG="$1"

if [[ ! "${TAG}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "error: tag must look like v0.1.0 or v0.1.0-rc.1" >&2
  exit 1
fi

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "error: this script must be run inside a git repository" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: git working tree is not clean; commit or stash changes first" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists locally" >&2
  exit 1
fi

MANIFEST_VERSION="$(cargo pkgid | sed -E 's/.*#.*@([^ ]+)$/\1/')"
TAG_VERSION="${TAG#v}"

if [[ "${MANIFEST_VERSION}" != "${TAG_VERSION}" ]]; then
  echo "error: Cargo package version (${MANIFEST_VERSION}) does not match tag (${TAG})" >&2
  exit 1
fi

echo "Running release validation checks..."
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings

git tag "${TAG}"
echo "Created local tag ${TAG}"
echo "Next step: git push origin ${TAG}"
