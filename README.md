# Rebootable Bootstrap Protocol

RBP v1 is a permissionless cold-start and network-resurrection layer for peer-to-peer applications. It lets a network recover after every application node and every operator-controlled bootstrap service has disappeared, provided its configured EVM chain remains readable and writable.

The repository contains a complete reference implementation of [`docs/spec.md`](docs/spec.md):

- an immutable, stateless Solidity registry;
- reusable Rust protocol, Ethereum, and peer-record crates;
- a native Tokio/rust-libp2p seed and light-node process;
- a browser/static TypeScript registry client;
- canonical contract source and ABI packages;
- deterministic cross-language vectors; and
- unit, fuzz, invariant, fork, reboot, simultaneous-reboot, and packaging tests.

RBP is discovery, not trust. Every registry event is attacker-controlled input until the embedded peer record is cryptographically verified and accepted by local endpoint and application policy.

## How it works

```text
cache → native discovery → recent RBP events → self-announce if eligible → retry
                                │
                                └─ verify signature, identity, sequence,
                                   expiry, codec, namespace, and endpoints
```

The onchain contract stores no peer list. Anyone can emit a bounded announcement under any namespace. A client scans only the block window that can contain unexpired events, validates signed peer records, and dials a bounded candidate set. Once native connectivity exists, the registry leaves the hot path.

RBP does not recover lost application data, define application membership, prove endpoint reachability, provide NAT traversal, or replace a DHT, peer exchange, pub/sub, consensus, or application handshake.

## Repository map

| Path | Purpose | Release artifact |
|---|---|---|
| `contracts/` | Canonical Foundry project and registry tests | source via `@rbp-protocol/contracts` |
| `crates/rbp-core` | descriptors, namespaces, validation, bounded candidates | `rbp-core` |
| `crates/rbp-ethereum` | Alloy provider, scanner, publisher ABI | `rbp-ethereum` |
| `crates/rbp-libp2p` | EIP-778 ENR and libp2p signed-record codecs | `rbp-libp2p` |
| `crates/rbp-node` | native libp2p host, SQLite cache, supervisor, CLI | `rbp-node` crate and binaries |
| `packages/ts` | browser/static provider and registry scanner | `@rbp-protocol/client` |
| `packages/contracts` | canonical Solidity source and ABI | `@rbp-protocol/contracts` |
| `test-vectors/` | deterministic Rust/TypeScript interoperability data | repository data |
| `scripts/` | conformance, packaging, and release automation | CI tooling |

## Requirements

Development uses:

- Rust 1.91 or newer;
- Foundry 1.7.1 with Solidity 0.8.24;
- Node.js 22 or 24;
- pnpm 11.17; and
- `jq` for the end-to-end checklist test.

The native `rbp-node` binary has no Node.js runtime dependency.

## Build and test

```bash
cargo build --workspace --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings -W clippy::pedantic
forge test --root contracts
corepack enable
pnpm install --frozen-lockfile
pnpm --recursive check
pnpm --recursive test
```

Run the full implementer-checklist integration test with Anvil:

```bash
scripts/checklist-integration.sh
```

It deploys a fresh registry and proves empty-network self-promotion, RBP-only discovery, authenticated libp2p dialing, native discovery without registry access, total shutdown and unrelated-operator reboot, and simultaneous reboot. The machine-readable result is written to `artifacts/implementer-checklist.json`.

See [Testing](docs/testing.md) for suite boundaries and [Conformance](docs/conformance.md) for the checklist mapping.

## Network descriptor

Every application pins the chain, immutable registry, deployment block, namespace, and accepted codecs:

```json
{
  "rbpVersion": 1,
  "registry": {
    "chainId": 1,
    "address": "0x1111111111111111111111111111111111111111",
    "deploymentBlock": 21000000,
    "maxTtlSeconds": 7776000
  },
  "namespace": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "acceptedRecordTypes": [1, 2]
}
```

The JSON schema is intentionally closed: unknown fields are rejected. It never contains an RPC hostname. Applications derive a namespace with `keccak256("rbp:<application>:<major-version>")` and distribute the descriptor as ordinary versioned application configuration.

No canonical production registry is asserted by this repository. See [Deployments](docs/deployments.md).

## Run a native node

Build the process:

```bash
cargo build -p rbp-node --release --locked
```

Run a read-only light node with a caller-selected RPC endpoint:

```bash
target/release/rbp-node \
  --descriptor ./network.rbp.json \
  --rpc-url https://your-registry-chain-rpc.example \
  --listen /ip4/0.0.0.0/tcp/4001
```

Run a publicly reachable seed:

```bash
export RBP_ETHEREUM_PRIVATE_KEY=0x...
target/release/rbp-node \
  --descriptor ./network.rbp.json \
  --rpc-url https://your-registry-chain-rpc.example \
  --seed \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --advertise /dns4/seed.example/tcp/4001
```

Seed mode requires an Ethereum signing key and at least one explicitly advertised signed endpoint. The Ethereum payer need not match the peer identity. Keep the peer identity file stable, publish only externally reachable endpoints, and use a dedicated limited-balance payer key. Private and loopback endpoints are rejected unless `--allow-private-endpoints` is explicitly enabled.

The process writes no mandatory hosted API and needs no DNS name when an IP multiaddr is usable. `--status-file` enables an atomically replaced local JSON health snapshot. Use `--allow-unfinalized` only for development chains whose `safe` or `finalized` tag does not progress.

For deterministic native bootstrap without multicast, repeat `--native-peer` with a peer-ID-qualified multiaddr such as `/dns4/seed.example/tcp/4001/p2p/<peer-id>`. Configured peers, mDNS, and identify are attempted before RBP.

Operational details are in [Node operations](docs/node-operations.md).

## Use the Rust libraries

```toml
[dependencies]
rbp-core = "0.1"
rbp-ethereum = "0.1"
rbp-libp2p = "0.1"
```

The main abstractions accept caller-owned providers, codecs, discovery sources, native peer stores, connectors, and publishers. Applications can use the scanner/codecs without adopting the reference CLI or SQLite cache. See [Application integration](docs/application-integration.md).

## Use the browser/static client

```bash
pnpm add @rbp-protocol/client
```

```ts
import {
  RbpBrowserClient,
  injectedProvider,
  jsonRpcProvider,
  parseDescriptor
} from '@rbp-protocol/client'

const descriptor = parseDescriptor(applicationDescriptor)
const provider = window.ethereum
  ? injectedProvider(window.ethereum)
  : jsonRpcProvider(userEnteredRpcUrl)

const client = new RbpBrowserClient(descriptor, provider)
const { candidates } = await client.scan()
```

Discovery never invokes `eth_requestAccounts`. The client verifies the chain and contract constants before scanning, searches only the recent TTL window, validates libp2p signed envelopes, and retains secure browser-capable endpoints. RPC URLs remain in memory unless the application explicitly calls `persistJsonRpcUrl`.

The package returns signed, validated dial candidates; the host application still owns its browser transport and authenticated application handshake. See [Browser client](docs/browser-client.md).

## Contract

`RBPRegistryV1` has exactly four public function selectors: `VERSION()`, `MAX_TTL()`, `MAX_RECORD_BYTES()`, and `announce(bytes32,uint32,uint32,bytes)`. It has no owner, storage-backed peer set, upgrade, pause, allowlist, withdrawal, or namespace administrator.

The canonical source is [`contracts/src/RBPRegistryV1.sol`](contracts/src/RBPRegistryV1.sol). CI requires its npm package mirror to be byte-for-byte identical. Deployers should independently compile, verify, and pin the resulting address and deployment block.

## Security

- Treat all registry data and RPC responses as untrusted.
- Authenticate the signed record and then the application protocol.
- Bound decoding, log processing, retained candidates, concurrent dials, timeouts, and retry rate.
- Do not interpret publisher addresses, log ordering, or payment as reputation.
- Preserve native discovery and peer diversity to reduce eclipse risk.
- Do not announce private endpoints or stable identities when endpoint privacy is required.
- Replace or compare registry providers when omission or privacy threats matter.

Read the [security model](docs/security.md) and [security policy](SECURITY.md) before production deployment.

## Releases and publishing

All publishable artifacts are released by one CI workflow:

- a successful CI run for `main` publishes a unique `<workspace-base>-dev.<run>.<attempt>` version under npm's `next` tag and as matching crates.io prereleases;
- publishing a GitHub Release tagged `vMAJOR.MINOR.PATCH` publishes that exact stable version under npm's `latest` tag and crates.io's normal stable channel; and
- stable releases also attach versioned Linux, macOS, and Windows native binaries, checksums, and build attestations.

The release pipeline runs tests before publication and publishes dependency crates in topological order with registry propagation retries. See [Releasing](docs/releasing.md).

## Project status

The protocol specification is a draft. The implementation is conformance-oriented and comprehensively tested, but it has not been represented here as externally audited. Production users should review the contract, peer-record parsing, endpoint policy, application handshake, key handling, and chain/RPC assumptions for their threat model.

## Contributing and license

See [CONTRIBUTING.md](CONTRIBUTING.md). Rust and TypeScript code is available under MIT or Apache-2.0 at your option. The canonical registry contract is CC0-1.0 as declared in its source. Dependency licenses remain their own.
