#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
COMPOSE_FILE="${REPO_ROOT}/docker/e2e/compose.yaml"
COMPOSE_PROJECT_NAME="${GB_E2E_COMPOSE_PROJECT:-gb-e2e}"
GB_E2E_ROOT="${GB_E2E_ROOT:-${REPO_ROOT}/.tmp/e2e}"
GB_E2E_HTTP_PORT="${GB_E2E_HTTP_PORT:-18080}"
normalize_path_prefix() {
  local value=${1:-}

  if [[ -z "${value}" || "${value}" == "/" ]]; then
    printf '%s' ""
    return 0
  fi

  if [[ "${value}" != /* ]]; then
    value="/${value}"
  fi

  value="${value%/}"
  printf '%s' "${value}"
}

GB_E2E_PATH_PREFIX="$(normalize_path_prefix "${GB_E2E_PATH_PREFIX:-}")"
GB_E2E_HOST="${GB_E2E_HOST:-127.0.0.1:${GB_E2E_HTTP_PORT}${GB_E2E_PATH_PREFIX}}"
GB_E2E_BASE_URL="${GB_E2E_BASE_URL:-http://${GB_E2E_HOST}}"
GB_E2E_DATA_DIR="${GB_E2E_DATA_DIR:-${GB_E2E_ROOT}/gitbucket-data}"
GB_E2E_ENV_FILE="${GB_E2E_ENV_FILE:-${GB_E2E_ROOT}/runtime.env}"
GB_E2E_USER="${GB_E2E_USER:-gb-e2e-user}"
GB_E2E_PASSWORD="${GB_E2E_PASSWORD:-gb-e2e-pass}"
GB_E2E_REPO_OWNER="${GB_E2E_REPO_OWNER:-${GB_E2E_USER}}"
GB_E2E_REPO_NAME="${GB_E2E_REPO_NAME:-e2e-smoke}"
GB_E2E_REPO="${GB_E2E_REPO_OWNER}/${GB_E2E_REPO_NAME}"
GB_E2E_FORK_SOURCE_OWNER="${GB_E2E_FORK_SOURCE_OWNER:-root}"
GB_E2E_FORK_SOURCE_NAME="${GB_E2E_FORK_SOURCE_NAME:-e2e-fork-source}"
GB_E2E_FORK_SOURCE="${GB_E2E_FORK_SOURCE_OWNER}/${GB_E2E_FORK_SOURCE_NAME}"

export REPO_ROOT
export COMPOSE_FILE
export COMPOSE_PROJECT_NAME
export GB_E2E_ROOT
export GB_E2E_HTTP_PORT
export GB_E2E_PATH_PREFIX
export GB_E2E_HOST
export GB_E2E_BASE_URL
export GB_E2E_DATA_DIR
export GB_E2E_ENV_FILE
export GB_E2E_USER
export GB_E2E_PASSWORD
export GB_E2E_REPO_OWNER
export GB_E2E_REPO_NAME
export GB_E2E_REPO
export GB_E2E_FORK_SOURCE_OWNER
export GB_E2E_FORK_SOURCE_NAME
export GB_E2E_FORK_SOURCE

compose() {
  docker compose -p "${COMPOSE_PROJECT_NAME}" -f "${COMPOSE_FILE}" "$@"
}

ensure_runtime_dir() {
  mkdir -p "${GB_E2E_ROOT}"
  mkdir -p "${GB_E2E_DATA_DIR}"
}

wait_for_gitbucket() {
  local attempt

  for attempt in $(seq 1 90); do
    if curl -fsS "${GB_E2E_BASE_URL}/signin" >/dev/null 2>&1 || curl -fsS "${GB_E2E_BASE_URL}/" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done

  echo "GitBucket did not become ready at ${GB_E2E_BASE_URL}" >&2
  return 1
}

curl_json() {
  local output_file=$1
  shift
  curl -sS -o "${output_file}" -w "%{http_code}" "$@"
}

extract_json_string() {
  local key=$1
  local file=$2
  python3 -c 'import json,sys; data=json.load(open(sys.argv[2], encoding="utf-8")); value=data.get(sys.argv[1], ""); print("" if value is None else value)' "${key}" "${file}"
}

extract_html_clipboard_token() {
  local file=$1
  python3 -c 'import pathlib,re,sys; body=pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"); match=re.search(r"data-clipboard-text=\"([0-9a-f]{40})\"", body); print(match.group(1) if match else "")' "${file}"
}
