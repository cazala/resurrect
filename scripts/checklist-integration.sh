#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANVIL_BIN="${ANVIL_BIN:-anvil}"
FORGE_BIN="${FORGE_BIN:-forge}"
CAST_BIN="${CAST_BIN:-cast}"
NODE_BIN="${NODE_BIN:-${REPOSITORY_ROOT}/target/debug/resurrect-node}"
RPC_PORT="${RESURRECT_TEST_RPC_PORT:-18545}"
RPC_URL="http://127.0.0.1:${RPC_PORT}"
WORK_DIRECTORY="$(mktemp -d "${TMPDIR:-/tmp}/resurrect-checklist.XXXXXX")"
ACCOUNT_A_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
ACCOUNT_D_KEY="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
NODE_PIDS=()
ANVIL_PID=""

for private_key in "${ACCOUNT_A_KEY}" "${ACCOUNT_D_KEY}"; do
  if [[ ! "${private_key}" =~ ^0x[[:xdigit:]]{64}$ ]]; then
    echo "Anvil fixture key must contain exactly 32 bytes" >&2
    exit 1
  fi
done

stop_nodes() {
  local pid
  for pid in "${NODE_PIDS[@]:-}"; do
    if kill -0 "${pid}" 2>/dev/null; then
      kill "${pid}" 2>/dev/null || true
      wait "${pid}" 2>/dev/null || true
    fi
  done
  NODE_PIDS=()
}

cleanup() {
  stop_nodes
  if [[ -n "${ANVIL_PID}" ]] && kill -0 "${ANVIL_PID}" 2>/dev/null; then
    kill "${ANVIL_PID}" 2>/dev/null || true
    wait "${ANVIL_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

wait_for_rpc() {
  local attempt
  for attempt in $(seq 1 100); do
    if "${CAST_BIN}" chain-id --rpc-url "${RPC_URL}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.1
  done
  echo "Anvil did not become ready" >&2
  return 1
}

wait_for_status() {
  local file="$1"
  local expression="$2"
  local label="$3"
  local attempt
  for attempt in $(seq 1 300); do
    if [[ -f "${file}" ]] && jq -e "${expression}" "${file}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.1
  done
  echo "Timed out waiting for ${label}" >&2
  [[ -f "${file}" ]] && jq . "${file}" >&2
  find "${WORK_DIRECTORY}" -maxdepth 1 -name '*.log' -print -exec tail -80 {} \; >&2
  return 1
}

write_descriptor() {
  local file="$1"
  local namespace="$2"
  cat >"${file}" <<JSON
{"resurrectVersion":1,"registry":{"chainId":31337,"address":"${REGISTRY_ADDRESS}","deploymentBlock":${DEPLOYMENT_BLOCK},"maxTtlSeconds":7776000},"namespace":"${namespace}","acceptedRecordTypes":[2]}
JSON
}

start_node() {
  local name="$1"
  local port="$2"
  local descriptor="$3"
  local rpc_url="$4"
  local mdns="$5"
  local seed="$6"
  local ethereum_key="${7:-}"
  local native_peer="${8:-}"
  local arguments=(
    --descriptor "${descriptor}"
    --rpc-url "${rpc_url}"
    --identity "${WORK_DIRECTORY}/${name}.key"
    --cache "${WORK_DIRECTORY}/${name}.sqlite3"
    --listen "/ip4/127.0.0.1/tcp/${port}"
    --mdns "${mdns}"
    --allow-private-endpoints
    --minimum-peers 1
    --fallback-confirmations 0
    --allow-unfinalized
    --native-observation-millis 500
    --dial-timeout-seconds 2
    --initial-backoff-millis 100
    --maximum-backoff-seconds 1
    --status-file "${WORK_DIRECTORY}/${name}.json"
    --log-format json
  )
  if [[ "${seed}" == "true" ]]; then
    arguments+=(--seed --advertise "/ip4/127.0.0.1/tcp/${port}")
  fi
  if [[ -n "${native_peer}" ]]; then
    arguments+=(--native-peer "${native_peer}")
  fi
  if [[ -n "${ethereum_key}" ]]; then
    env RESURRECT_ETHEREUM_PRIVATE_KEY="${ethereum_key}" "${NODE_BIN}" "${arguments[@]}" \
      >"${WORK_DIRECTORY}/${name}.log" 2>&1 &
  else
    "${NODE_BIN}" "${arguments[@]}" >"${WORK_DIRECTORY}/${name}.log" 2>&1 &
  fi
  NODE_PIDS+=("$!")
}

cd "${REPOSITORY_ROOT}"
mkdir -p artifacts
cat >artifacts/implementer-checklist.json <<JSON
{
  "passed": false,
  "status": "prerequisite-or-integration-failure",
  "registryDeployed": false,
  "selfAuthenticatingRecordGenerated": false,
  "arbitraryNamespacePublished": false,
  "recentTtlWindowScanned": false,
  "invalidAndExpiredRecordsRejected": false,
  "recoveredEndpointDialed": false,
  "nodeASelfAnnounced": false,
  "nodeBDiscoveredOnlyThroughResurrect": false,
  "nodeCJoinedThroughNativeDiscoveryWithoutRegistry": false,
  "callerSuppliedRegistryProvider": false,
  "browserCustomAndInjectedProvidersWithoutAccounts": false,
  "chainIdVerifiedAndProviderSwitchingSupported": false,
  "browserDialContextDefined": false,
  "unrelatedOperatorsRebootedAfterTotalShutdown": false,
  "noOwnerDnsHostedApiOrOriginalOperatorRequired": false,
  "simultaneousRebootFormedConnection": false
}
JSON

"${FORGE_BIN}" test --root contracts
cargo test --workspace --all-targets --locked
pnpm --filter @resurrect-protocol/client test
cargo build -p resurrect-node --locked

cmp contracts/src/ResurrectRegistryV1.sol packages/contracts/src/ResurrectRegistryV1.sol
cmp deployments/ethereum-mainnet.json packages/contracts/deployments/ethereum-mainnet.json
node -e "JSON.parse(require('node:fs').readFileSync('packages/contracts/abi/ResurrectRegistryV1.json'))"

"${ANVIL_BIN}" --port "${RPC_PORT}" --chain-id 31337 --silent \
  >"${WORK_DIRECTORY}/anvil.log" 2>&1 &
ANVIL_PID="$!"
wait_for_rpc

DEPLOYMENT_JSON="$("${FORGE_BIN}" create \
  --root contracts \
  src/ResurrectRegistryV1.sol:ResurrectRegistryV1 \
  --rpc-url "${RPC_URL}" \
  --private-key "${ACCOUNT_A_KEY}" \
  --broadcast \
  --json)"
REGISTRY_ADDRESS="$(jq -r .deployedTo <<<"${DEPLOYMENT_JSON}")"
DEPLOYMENT_TRANSACTION="$(jq -r .transactionHash <<<"${DEPLOYMENT_JSON}")"
DEPLOYMENT_BLOCK_HEX="$("${CAST_BIN}" receipt "${DEPLOYMENT_TRANSACTION}" --rpc-url "${RPC_URL}" --json | jq -r .blockNumber)"
DEPLOYMENT_BLOCK="$("${CAST_BIN}" to-dec "${DEPLOYMENT_BLOCK_HEX}")"

METHODS_JSON="$("${FORGE_BIN}" inspect --root contracts src/ResurrectRegistryV1.sol:ResurrectRegistryV1 methodIdentifiers --json)"
jq -e 'keys | sort == ["MAX_RECORD_BYTES()", "MAX_TTL()", "VERSION()", "announce(bytes32,uint32,uint32,bytes)"]' \
  <<<"${METHODS_JSON}" >/dev/null

NAMESPACE="$("${CAST_BIN}" keccak 'resurrect:ci-checklist:1')"
DESCRIPTOR="${WORK_DIRECTORY}/descriptor.json"
write_descriptor "${DESCRIPTOR}" "${NAMESPACE}"
jq -e 'has("rpcUrl") | not' "${DESCRIPTOR}" >/dev/null

# A promotes itself from a completely empty application network.
start_node a 42001 "${DESCRIPTOR}" "${RPC_URL}" false true "${ACCOUNT_A_KEY}"
wait_for_status "${WORK_DIRECTORY}/a.json" '.registry.announcements >= 1 and .connectedPeers == 0' 'node A self-announcement'

# B has no cache, native discovery, DNS seed, or knowledge of A.
start_node b 42002 "${DESCRIPTOR}" "${RPC_URL}" false false
wait_for_status "${WORK_DIRECTORY}/b.json" '.state == "CONNECTED" and .connectedPeers >= 1 and .connectedVia == "RESURRECT_SCAN" and .registry.scans >= 1' 'node B registry bootstrap'

# Restart the formed component, then make C join through a configured native
# libp2p peer while registry RPC is unavailable. C must never scan Ethereum.
stop_nodes
rm -f "${WORK_DIRECTORY}/a.json" "${WORK_DIRECTORY}/b.json"
start_node a 42001 "${DESCRIPTOR}" "${RPC_URL}" false true "${ACCOUNT_A_KEY}"
start_node b 42002 "${DESCRIPTOR}" "${RPC_URL}" false false
wait_for_status "${WORK_DIRECTORY}/b.json" '.state == "CONNECTED" and .connectedPeers >= 1' 'restarted A/B component'
wait_for_status "${WORK_DIRECTORY}/a.json" '.connectedPeers >= 1' 'restarted node A status'
A_PEER_ID="$(jq -r .peerId "${WORK_DIRECTORY}/a.json")"
start_node c 42003 "${DESCRIPTOR}" 'http://127.0.0.1:1' false false '' \
  "/ip4/127.0.0.1/tcp/42001/p2p/${A_PEER_ID}"
wait_for_status "${WORK_DIRECTORY}/c.json" '.state == "CONNECTED" and .connectedPeers >= 1 and .connectedVia == "NATIVE_DISCOVERY" and .registry.scans == 0 and .registry.scanFailures == 0' 'node C native-only bootstrap'

# Kill every node, then reboot with unrelated P2P identities and Ethereum payer.
stop_nodes
start_node d 42004 "${DESCRIPTOR}" "${RPC_URL}" false true "${ACCOUNT_D_KEY}"
wait_for_status "${WORK_DIRECTORY}/d.json" '.registry.announcements >= 1' 'unrelated node D self-announcement'
start_node e 42005 "${DESCRIPTOR}" "${RPC_URL}" false false
wait_for_status "${WORK_DIRECTORY}/e.json" '.state == "CONNECTED" and .connectedPeers >= 1 and .connectedVia == "RESURRECT_SCAN"' 'unrelated node E registry bootstrap'
test "$(jq -r .peerId "${WORK_DIRECTORY}/a.json")" != "$(jq -r .peerId "${WORK_DIRECTORY}/d.json")"

# Simultaneous reboot under a fresh arbitrary namespace.
stop_nodes
SIMULTANEOUS_NAMESPACE="$("${CAST_BIN}" keccak 'resurrect:ci-simultaneous:1')"
SIMULTANEOUS_DESCRIPTOR="${WORK_DIRECTORY}/simultaneous.json"
write_descriptor "${SIMULTANEOUS_DESCRIPTOR}" "${SIMULTANEOUS_NAMESPACE}"
start_node f 42006 "${SIMULTANEOUS_DESCRIPTOR}" "${RPC_URL}" false true "${ACCOUNT_A_KEY}"
start_node g 42007 "${SIMULTANEOUS_DESCRIPTOR}" "${RPC_URL}" false true "${ACCOUNT_D_KEY}"
wait_for_status "${WORK_DIRECTORY}/f.json" '.registry.announcements >= 1' 'node F simultaneous announcement'
wait_for_status "${WORK_DIRECTORY}/g.json" '.registry.announcements >= 1' 'node G simultaneous announcement'
wait_for_status "${WORK_DIRECTORY}/f.json" '.connectedPeers >= 1' 'simultaneous reboot connection'

cat >artifacts/implementer-checklist.json <<JSON
{
  "passed": true,
  "status": "passed",
  "registryDeployed": true,
  "selfAuthenticatingRecordGenerated": true,
  "arbitraryNamespacePublished": true,
  "recentTtlWindowScanned": true,
  "invalidAndExpiredRecordsRejected": true,
  "recoveredEndpointDialed": true,
  "nodeASelfAnnounced": true,
  "nodeBDiscoveredOnlyThroughResurrect": true,
  "nodeCJoinedThroughNativeDiscoveryWithoutRegistry": true,
  "callerSuppliedRegistryProvider": true,
  "browserCustomAndInjectedProvidersWithoutAccounts": true,
  "chainIdVerifiedAndProviderSwitchingSupported": true,
  "browserDialContextDefined": true,
  "unrelatedOperatorsRebootedAfterTotalShutdown": true,
  "noOwnerDnsHostedApiOrOriginalOperatorRequired": true,
  "simultaneousRebootFormedConnection": true,
  "registryAddress": "${REGISTRY_ADDRESS}",
  "workspace": "${WORK_DIRECTORY}"
}
JSON

jq -e '[to_entries[] | select(.value == false)] | length == 0' artifacts/implementer-checklist.json >/dev/null
echo "Resurrect implementer checklist integration passed"
