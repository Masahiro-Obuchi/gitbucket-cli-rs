#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release-tag.sh <tag>

Example:
  scripts/release-tag.sh v0.5.1

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

if [[ ! "${TAG}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z._-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  echo "error: tag must look like v0.5.1, v0.5.1-rc.1, or v0.5.1+build.5" >&2
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

MANIFEST_VERSION="$(
  cargo metadata --no-deps --format-version 1 | python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
packages = {pkg["id"]: pkg for pkg in metadata.get("packages", [])}
resolve = metadata.get("resolve") or {}
root_id = resolve.get("root")

if root_id and root_id in packages:
    print(packages[root_id]["version"])
elif len(packages) == 1:
    print(next(iter(packages.values()))["version"])
else:
    print("error: unable to determine root Cargo package version from cargo metadata", file=sys.stderr)
    sys.exit(1)
'
)"
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
