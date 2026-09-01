#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATES_IO_USER_AGENT="Resurrect release automation (https://github.com/cazala/resurrect)"

crate_version_exists() {
  local crate="$1"
  curl --fail --silent --show-error \
    --header "User-Agent: ${CRATES_IO_USER_AGENT}" \
    "https://crates.io/api/v1/crates/${crate}/${VERSION}" \
    >/dev/null 2>&1
}

npm_package_version_exists() {
  local package="$1"
  local published_version
  if ! published_version="$(npm view "${package}@${VERSION}" version --json 2>/dev/null)"; then
    return 1
  fi
  published_version="${published_version//\"/}"
  [[ "${published_version}" == "${VERSION}" ]]
}

publish_crate() {
  local crate="$1"
  local attempt

  if crate_version_exists "${crate}"; then
    echo "${crate}@${VERSION} already exists on crates.io; skipping"
    return 0
  fi

  for attempt in $(seq 1 12); do
    if cargo publish -p "${crate}" --locked --allow-dirty; then
      return 0
    fi
    if crate_version_exists "${crate}"; then
      echo "${crate}@${VERSION} is visible on crates.io after the publish error; continuing"
      return 0
    fi
    if [[ "${attempt}" -eq 12 ]]; then
      echo "could not publish ${crate} after registry propagation retries" >&2
      return 1
    fi
    sleep 15
  done
}

publish_npm_package() {
  local directory="$1"
  local package="$2"

  if npm_package_version_exists "${package}"; then
    echo "${package}@${VERSION} already exists on npm; skipping"
    return 0
  fi

  if (cd "${directory}" && npm publish --tag "${NPM_TAG}" --access public --provenance); then
    return 0
  fi
  if npm_package_version_exists "${package}"; then
    echo "${package}@${VERSION} is visible on npm after the publish error; continuing"
    return 0
  fi
  return 1
}

main() {
  if [[ $# -ne 2 ]]; then
    echo "usage: $0 <semver> <next|latest>" >&2
    exit 2
  fi

  VERSION="$1"
  NPM_TAG="$2"
  if [[ "${NPM_TAG}" != "next" && "${NPM_TAG}" != "latest" ]]; then
    echo "npm tag must be next or latest" >&2
    exit 2
  fi

  cd "${REPOSITORY_ROOT}"
  NPM_CONFIG_USERCONFIG="$(mktemp "${TMPDIR:-/tmp}/resurrect-npmrc.XXXXXX")"
  export NPM_CONFIG_USERCONFIG
  trap 'rm -f "${NPM_CONFIG_USERCONFIG}"' EXIT
  npm config set registry https://registry.npmjs.org/ --location=user
  "${REPOSITORY_ROOT}/scripts/set-version.sh" "${VERSION}"
  cargo check --workspace --all-targets --locked
  pnpm install --frozen-lockfile
  pnpm --recursive build
  pnpm --recursive test
  scripts/check-packages.sh

  publish_crate resurrect-core
  publish_crate resurrect-libp2p
  publish_crate resurrect-ethereum
  publish_crate resurrect-node

  if [[ -n "${NODE_AUTH_TOKEN:-}" ]]; then
    npm config set //registry.npmjs.org/:_authToken "${NODE_AUTH_TOKEN}" --location=user
  else
    unset NODE_AUTH_TOKEN
  fi
  publish_npm_package packages/contracts @resurrect-protocol/contracts
  publish_npm_package packages/ts @resurrect-protocol/client
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
