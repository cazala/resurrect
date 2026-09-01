#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPOSITORY_ROOT}"

cmp contracts/src/ResurrectRegistryV1.sol packages/contracts/src/ResurrectRegistryV1.sol
cmp deployments/ethereum-mainnet.json packages/contracts/deployments/ethereum-mainnet.json
node -e "const fs=require('node:fs'); JSON.parse(fs.readFileSync('packages/contracts/abi/ResurrectRegistryV1.json')); const deployment=JSON.parse(fs.readFileSync('deployments/ethereum-mainnet.json')); if (deployment.chainId !== 1 || deployment.address !== '0x6F33c332e8251dcd307D85A27fCcAbd85d578910' || deployment.deploymentBlock !== 25882327 || deployment.runtimeBytecodeHash !== '0x0024244f6ad881009b5726d2c1644a3c2aff178852c4d01b1066cd7d9967c109') throw new Error('canonical Ethereum deployment metadata drift')"

cargo package --workspace --locked --allow-dirty --no-verify

PACKAGE_DIRECTORY="$(mktemp -d "${TMPDIR:-/tmp}/resurrect-packages.XXXXXX")"
pnpm --dir packages/contracts pack --pack-destination "${PACKAGE_DIRECTORY}"
pnpm --dir packages/ts pack --pack-destination "${PACKAGE_DIRECTORY}"
test "$(find "${PACKAGE_DIRECTORY}" -type f -name '*.tgz' | wc -l | tr -d ' ')" -eq 2

CONTRACT_ARCHIVE="$(find "${PACKAGE_DIRECTORY}" -type f -name 'resurrect-protocol-contracts-*.tgz' -print -quit)"
CLIENT_ARCHIVE="$(find "${PACKAGE_DIRECTORY}" -type f -name 'resurrect-protocol-client-*.tgz' -print -quit)"
test -n "${CONTRACT_ARCHIVE}"
test -n "${CLIENT_ARCHIVE}"
CONTRACT_LISTING="$(tar -tzf "${CONTRACT_ARCHIVE}")"
CLIENT_LISTING="$(tar -tzf "${CLIENT_ARCHIVE}")"
grep -qx 'package/src/ResurrectRegistryV1.sol' <<<"${CONTRACT_LISTING}"
grep -qx 'package/abi/ResurrectRegistryV1.json' <<<"${CONTRACT_LISTING}"
grep -qx 'package/deployments/ethereum-mainnet.json' <<<"${CONTRACT_LISTING}"
grep -qx 'package/dist/index.js' <<<"${CLIENT_LISTING}"
grep -qx 'package/dist/index.d.ts' <<<"${CLIENT_LISTING}"
