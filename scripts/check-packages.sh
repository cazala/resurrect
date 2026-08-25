#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPOSITORY_ROOT}"

cmp contracts/src/RBPRegistryV1.sol packages/contracts/src/RBPRegistryV1.sol
node -e "JSON.parse(require('node:fs').readFileSync('packages/contracts/abi/RBPRegistryV1.json'))"

cargo package --workspace --locked --allow-dirty --no-verify

PACKAGE_DIRECTORY="$(mktemp -d "${TMPDIR:-/tmp}/rbp-packages.XXXXXX")"
pnpm --dir packages/contracts pack --pack-destination "${PACKAGE_DIRECTORY}"
pnpm --dir packages/ts pack --pack-destination "${PACKAGE_DIRECTORY}"
test "$(find "${PACKAGE_DIRECTORY}" -type f -name '*.tgz' | wc -l | tr -d ' ')" -eq 2

CONTRACT_ARCHIVE="$(find "${PACKAGE_DIRECTORY}" -type f -name 'rbp-protocol-contracts-*.tgz' -print -quit)"
CLIENT_ARCHIVE="$(find "${PACKAGE_DIRECTORY}" -type f -name 'rbp-protocol-client-*.tgz' -print -quit)"
test -n "${CONTRACT_ARCHIVE}"
test -n "${CLIENT_ARCHIVE}"
CONTRACT_LISTING="$(tar -tzf "${CONTRACT_ARCHIVE}")"
CLIENT_LISTING="$(tar -tzf "${CLIENT_ARCHIVE}")"
grep -qx 'package/src/RBPRegistryV1.sol' <<<"${CONTRACT_LISTING}"
grep -qx 'package/abi/RBPRegistryV1.json' <<<"${CONTRACT_LISTING}"
grep -qx 'package/dist/index.js' <<<"${CLIENT_LISTING}"
grep -qx 'package/dist/index.d.ts' <<<"${CLIENT_LISTING}"
