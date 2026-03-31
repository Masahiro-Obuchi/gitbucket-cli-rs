#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./common.sh
source "${SCRIPT_DIR}/common.sh"

validate_saved_token() {
  local token=$1
  local response_file status

  response_file=$(mktemp)
  status=$(curl_json     "${response_file}"     -H "Accept: application/json"     -H "Authorization: token ${token}"     "${GB_E2E_BASE_URL}/api/v3/user")

  rm -f "${response_file}"
  [[ "${status}" == "200" ]]
}

ensure_validation_user() {
  local create_payload response_file status

  response_file=$(mktemp)
  status=$(curl_json     "${response_file}"     -u "root:root"     -H "Accept: application/json"     "${GB_E2E_BASE_URL}/api/v3/users/${GB_E2E_USER}")

  case "${status}" in
    200)
      rm -f "${response_file}"
      return 0
      ;;
    404)
      ;;
    *)
      echo "failed to check validation user ${GB_E2E_USER} (HTTP ${status})" >&2
      cat "${response_file}" >&2
      rm -f "${response_file}"
      return 1
      ;;
  esac

  create_payload=$(cat <<EOF
{"login":"${GB_E2E_USER}","password":"${GB_E2E_PASSWORD}","email":"${GB_E2E_USER}@example.test","fullName":"${GB_E2E_USER}","isAdmin":false,"description":"gb e2e validation user","url":null}
EOF
)
  status=$(curl_json     "${response_file}"     -u "root:root"     -H "Accept: application/json"     -H "Content-Type: application/json"     -X POST     -d "${create_payload}"     "${GB_E2E_BASE_URL}/api/v3/admin/users")

  if [[ "${status}" != "200" && "${status}" != "201" ]]; then
    echo "failed to create validation user ${GB_E2E_USER} (HTTP ${status})" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    return 1
  fi

  rm -f "${response_file}"
}

create_token_via_web() {
  local user=$1
  local password=$2
  local note=$3
  local cookie_file page_file token

  cookie_file=$(mktemp)
  page_file=$(mktemp)

  curl -sS -L     -c "${cookie_file}"     -b "${cookie_file}"     --data-urlencode "userName=${user}"     --data-urlencode "password=${password}"     --data-urlencode "hash="     -o /dev/null     "${GB_E2E_BASE_URL}/signin"

  curl -sS -L     -c "${cookie_file}"     -b "${cookie_file}"     --data-urlencode "note=${note}"     -o "${page_file}"     "${GB_E2E_BASE_URL}/${user}/_personalToken"

  token=$(extract_html_clipboard_token "${page_file}")
  rm -f "${cookie_file}" "${page_file}"

  if [[ -z "${token}" ]]; then
    echo "failed to extract token from GitBucket application page for ${user}" >&2
    return 1
  fi

  printf '%s\n' "${token}"
}

ensure_repo_with_token() {
  local token=$1
  local repo_full_name=$2
  local repo_name=${repo_full_name#*/}
  local create_payload response_file status

  response_file=$(mktemp)
  status=$(curl_json     "${response_file}"     -H "Accept: application/json"     -H "Authorization: token ${token}"     "${GB_E2E_BASE_URL}/api/v3/repos/${repo_full_name}")

  case "${status}" in
    200)
      rm -f "${response_file}"
      return 0
      ;;
    404)
      ;;
    *)
      echo "failed to check repo ${repo_full_name} (HTTP ${status})" >&2
      cat "${response_file}" >&2
      rm -f "${response_file}"
      return 1
      ;;
  esac

  create_payload=$(cat <<EOF
{"name":"${repo_name}","private":false}
EOF
)
  status=$(curl_json     "${response_file}"     -H "Accept: application/json"     -H "Authorization: token ${token}"     -H "Content-Type: application/json"     -X POST     -d "${create_payload}"     "${GB_E2E_BASE_URL}/api/v3/user/repos")

  if [[ "${status}" != "200" && "${status}" != "201" ]]; then
    echo "failed to create repo ${repo_full_name} (HTTP ${status})" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    return 1
  fi

  rm -f "${response_file}"
}

ensure_repo_deleted_with_token() {
  local token=$1
  local repo_full_name=$2
  local response_file status

  response_file=$(mktemp)
  status=$(curl_json     "${response_file}"     -H "Accept: application/json"     -H "Authorization: token ${token}"     "${GB_E2E_BASE_URL}/api/v3/repos/${repo_full_name}")

  case "${status}" in
    404)
      rm -f "${response_file}"
      return 0
      ;;
    200)
      ;;
    *)
      echo "failed to check repo ${repo_full_name} before cleanup (HTTP ${status})" >&2
      cat "${response_file}" >&2
      rm -f "${response_file}"
      return 1
      ;;
  esac

  status=$(curl_json     "${response_file}"     -H "Accept: application/json"     -H "Authorization: token ${token}"     -X DELETE     "${GB_E2E_BASE_URL}/api/v3/repos/${repo_full_name}")

  if [[ "${status}" == "204" || "${status}" == "404" ]]; then
    rm -f "${response_file}"
    return 0
  fi

  if [[ "${status}" != "204" ]]; then
    echo "failed to delete existing repo ${repo_full_name} (HTTP ${status})" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    return 1
  fi

  rm -f "${response_file}"
}

ensure_root_repo() {
  local repo_full_name=$1
  local repo_name=${repo_full_name#*/}
  local create_payload response_file status

  response_file=$(mktemp)
  status=$(curl_json     "${response_file}"     -u "root:root"     -H "Accept: application/json"     "${GB_E2E_BASE_URL}/api/v3/repos/${repo_full_name}")

  case "${status}" in
    200)
      rm -f "${response_file}"
      return 0
      ;;
    404)
      ;;
    *)
      echo "failed to check root repo ${repo_full_name} (HTTP ${status})" >&2
      cat "${response_file}" >&2
      rm -f "${response_file}"
      return 1
      ;;
  esac

  create_payload=$(cat <<EOF
{"name":"${repo_name}","private":false}
EOF
)
  status=$(curl_json     "${response_file}"     -u "root:root"     -H "Accept: application/json"     -H "Content-Type: application/json"     -X POST     -d "${create_payload}"     "${GB_E2E_BASE_URL}/api/v3/user/repos")

  if [[ "${status}" != "200" && "${status}" != "201" ]]; then
    echo "failed to create root repo ${repo_full_name} (HTTP ${status})" >&2
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
GB_E2E_USER=${GB_E2E_USER}
GB_E2E_PASSWORD=${GB_E2E_PASSWORD}
GB_E2E_TOKEN=${token}
GB_E2E_REPO=${GB_E2E_REPO}
GB_E2E_FORK_SOURCE=${GB_E2E_FORK_SOURCE}
GB_E2E_PROTOCOL=http
GB_E2E_BASE_URL=${GB_E2E_BASE_URL}
EOF
}

main() {
  local fork_target token=""

  ensure_runtime_dir
  compose up -d
  wait_for_gitbucket
  ensure_validation_user

  if [[ -f "${GB_E2E_ENV_FILE}" ]]; then
    # shellcheck disable=SC1090
    source "${GB_E2E_ENV_FILE}"
    if [[ -n "${GB_E2E_TOKEN:-}" ]] && validate_saved_token "${GB_E2E_TOKEN}"; then
      token="${GB_E2E_TOKEN}"
    fi
  fi

  if [[ -z "${token}" ]]; then
    token=$(create_token_via_web "${GB_E2E_USER}" "${GB_E2E_PASSWORD}" "gb-e2e-smoke")
  fi

  ensure_repo_with_token "${token}" "${GB_E2E_REPO}"
  ensure_root_repo "${GB_E2E_FORK_SOURCE}"
  fork_target="${GB_E2E_USER}/${GB_E2E_FORK_SOURCE_NAME}"
  ensure_repo_deleted_with_token "${token}" "${fork_target}"
  write_runtime_env "${token}"

  echo "Docker GitBucket E2E environment is ready."
  echo "Env file: ${GB_E2E_ENV_FILE}"
}

main "$@"
