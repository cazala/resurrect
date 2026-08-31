#!/usr/bin/env bash
set -euo pipefail

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

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

publish_crate() {
  local crate="$1"
  local attempt
  for attempt in $(seq 1 12); do
    if cargo publish -p "${crate}" --locked --allow-dirty; then
      return 0
    fi
    if [[ "${attempt}" -eq 12 ]]; then
      echo "could not publish ${crate} after registry propagation retries" >&2
      return 1
    fi
    sleep 15
  done
}

publish_crate resurrect-core
publish_crate resurrect-libp2p
publish_crate resurrect-ethereum
publish_crate resurrect-node

if [[ -n "${NODE_AUTH_TOKEN:-}" ]]; then
  npm config set //registry.npmjs.org/:_authToken "${NODE_AUTH_TOKEN}" --location=user
else
  unset NODE_AUTH_TOKEN
fi
(cd packages/contracts && npm publish --tag "${NPM_TAG}" --access public --provenance)
(cd packages/ts && npm publish --tag "${NPM_TAG}" --access public --provenance)
