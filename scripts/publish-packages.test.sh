#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=publish-packages.sh
source "${REPOSITORY_ROOT}/scripts/publish-packages.sh"

TEST_DIRECTORY="$(mktemp -d "${TMPDIR:-/tmp}/resurrect-publish-test.XXXXXX")"
trap 'rm -rf "${TEST_DIRECTORY}"' EXIT
CRATE_STATE="${TEST_DIRECTORY}/crate-exists"
NPM_STATE="${TEST_DIRECTORY}/npm-exists"
NPM_VIEW_COUNT="${TEST_DIRECTORY}/npm-view-count"
COMMAND_LOG="${TEST_DIRECTORY}/commands"
VERSION="0.1.0-test.1"
NPM_TAG="next"

curl() {
  [[ -f "${CRATE_STATE}" ]]
}

cargo() {
  printf 'cargo %s\n' "$*" >>"${COMMAND_LOG}"
  touch "${CRATE_STATE}"
  [[ "${MOCK_CARGO_RESULT:-success}" != "lost-response" ]]
}

npm() {
  if [[ "${1:-}" == "view" ]]; then
    if [[ -f "${NPM_STATE}" ]]; then
      local view_count=0
      if [[ -f "${NPM_VIEW_COUNT}" ]]; then
        view_count="$(<"${NPM_VIEW_COUNT}")"
      fi
      view_count=$((view_count + 1))
      printf '%s\n' "${view_count}" >"${NPM_VIEW_COUNT}"
      if [[ "${view_count}" -le "${MOCK_NPM_PENDING_VIEWS:-0}" ]]; then
        return 1
      fi
      printf '"%s"\n' "${VERSION}"
      return 0
    fi
    return 1
  fi
  if [[ "${1:-}" == "publish" ]]; then
    printf 'npm %s\n' "$*" >>"${COMMAND_LOG}"
    touch "${NPM_STATE}"
    [[ "${MOCK_NPM_RESULT:-success}" != "lost-response" ]]
    return
  fi
  return 2
}

touch "${CRATE_STATE}"
publish_crate resurrect-core
! grep -q '^cargo ' "${COMMAND_LOG}" 2>/dev/null

rm -f "${CRATE_STATE}"
MOCK_CARGO_RESULT=success publish_crate resurrect-core
test "$(grep -c '^cargo ' "${COMMAND_LOG}")" -eq 1

rm -f "${CRATE_STATE}"
MOCK_CARGO_RESULT=lost-response publish_crate resurrect-core
test "$(grep -c '^cargo ' "${COMMAND_LOG}")" -eq 2

touch "${NPM_STATE}"
publish_npm_package . @resurrect-protocol/contracts
! grep -q '^npm ' "${COMMAND_LOG}" 2>/dev/null

rm -f "${NPM_STATE}"
MOCK_NPM_RESULT=success publish_npm_package . @resurrect-protocol/contracts
test "$(grep -c '^npm ' "${COMMAND_LOG}")" -eq 1

rm -f "${NPM_STATE}" "${NPM_VIEW_COUNT}"
NPM_AVAILABILITY_ATTEMPTS=4 \
NPM_AVAILABILITY_INTERVAL_SECONDS=0 \
MOCK_NPM_PENDING_VIEWS=2 \
MOCK_NPM_RESULT=success \
  publish_npm_package . @resurrect-protocol/contracts
test "$(<"${NPM_VIEW_COUNT}")" -eq 4

rm -f "${NPM_STATE}" "${NPM_VIEW_COUNT}"
if NPM_AVAILABILITY_ATTEMPTS=2 \
  NPM_AVAILABILITY_INTERVAL_SECONDS=0 \
  MOCK_NPM_PENDING_VIEWS=2 \
  MOCK_NPM_RESULT=success \
  publish_npm_package . @resurrect-protocol/contracts; then
  echo "pending npm publication unexpectedly passed its availability deadline" >&2
  exit 1
fi

rm -f "${NPM_STATE}" "${NPM_VIEW_COUNT}"
MOCK_NPM_RESULT=lost-response publish_npm_package . @resurrect-protocol/contracts
test "$(grep -c '^npm ' "${COMMAND_LOG}")" -eq 4

echo "publish recovery tests passed"
