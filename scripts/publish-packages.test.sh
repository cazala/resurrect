#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=publish-packages.sh
source "${REPOSITORY_ROOT}/scripts/publish-packages.sh"

TEST_DIRECTORY="$(mktemp -d "${TMPDIR:-/tmp}/resurrect-publish-test.XXXXXX")"
trap 'rm -rf "${TEST_DIRECTORY}"' EXIT
CRATE_STATE="${TEST_DIRECTORY}/crate-exists"
NPM_STATE="${TEST_DIRECTORY}/npm-exists"
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

rm -f "${NPM_STATE}"
MOCK_NPM_RESULT=lost-response publish_npm_package . @resurrect-protocol/contracts
test "$(grep -c '^npm ' "${COMMAND_LOG}")" -eq 2

echo "publish recovery tests passed"
