#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./common.sh
source "${SCRIPT_DIR}/common.sh"

validate_saved_token() {
  local token=$1
  local response_file status

  response_file=$(mktemp)
  status=$(curl_json "${response_file}" \
    -H "Accept: application/json" \
    -H "Authorization: token ${token}" \
    "${GB_E2E_BASE_URL}/api/v3/user")

  rm -f "${response_file}"
  [[ "${status}" == "200" ]]
}

create_token() {
  local response_file status token

  response_file=$(mktemp)
  status=$(curl_json "${response_file}" \
    -u "root:root" \
    -H "Accept: application/json" \
    -H "Content-Type: application/json" \
    -X POST \
    -d '{"scopes":["repo"],"note":"gb-e2e-smoke"}' \
    "${GB_E2E_BASE_URL}/api/v3/authorizations")

  if [[ "${status}" != "200" && "${status}" != "201" ]]; then
    echo "failed to create GitBucket access token via ${GB_E2E_BASE_URL}/api/v3/authorizations (HTTP ${status})" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    return 1
  fi

  token=$(extract_json_string "token" "${response_file}")
  rm -f "${response_file}"

  if [[ -z "${token}" ]]; then
    echo "failed to extract token from GitBucket authorization response" >&2
    return 1
  fi

  printf '%s\n' "${token}"
}

ensure_repo() {
  local token=$1
  local response_file status

  response_file=$(mktemp)
  status=$(curl_json "${response_file}" \
    -H "Accept: application/json" \
    -H "Authorization: token ${token}" \
    "${GB_E2E_BASE_URL}/api/v3/repos/${GB_E2E_REPO}")

  case "${status}" in
    200)
      rm -f "${response_file}"
      return 0
      ;;
    404)
      ;;
    *)
      echo "failed to check seeded repo ${GB_E2E_REPO} (HTTP ${status})" >&2
      cat "${response_file}" >&2
      rm -f "${response_file}"
      return 1
      ;;
  esac

  status=$(curl_json "${response_file}" \
    -H "Accept: application/json" \
    -H "Authorization: token ${token}" \
    -H "Content-Type: application/json" \
    -X POST \
    -d "{\"name\":\"${GB_E2E_REPO_NAME}\",\"private\":false}" \
    "${GB_E2E_BASE_URL}/api/v3/user/repos")

  if [[ "${status}" != "200" && "${status}" != "201" ]]; then
    echo "failed to create seeded repo ${GB_E2E_REPO} (HTTP ${status})" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    return 1
  fi

  rm -f "${response_file}"
}

write_runtime_env() {
  local token=$1

  cat > "${GB_E2E_ENV_FILE}" <<EOF
GB_E2E_HOST=127.0.0.1:${GB_E2E_HTTP_PORT}
GB_E2E_TOKEN=${token}
GB_E2E_REPO=${GB_E2E_REPO}
GB_E2E_PROTOCOL=http
GB_E2E_BASE_URL=${GB_E2E_BASE_URL}
EOF
}

main() {
  local token=""

  ensure_runtime_dir
  compose up -d
  wait_for_gitbucket

  if [[ -f "${GB_E2E_ENV_FILE}" ]]; then
    # shellcheck disable=SC1090
    source "${GB_E2E_ENV_FILE}"
    if [[ -n "${GB_E2E_TOKEN:-}" ]] && validate_saved_token "${GB_E2E_TOKEN}"; then
      token="${GB_E2E_TOKEN}"
    fi
  fi

  if [[ -z "${token}" ]]; then
    token=$(create_token)
  fi

  ensure_repo "${token}"
  write_runtime_env "${token}"

  echo "Docker GitBucket E2E environment is ready."
  echo "Env file: ${GB_E2E_ENV_FILE}"
}

main "$@"
