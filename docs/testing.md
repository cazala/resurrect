# Testing and verification

The test strategy layers deterministic unit tests under real-process reboot scenarios. CI treats warnings, formatting drift, package drift, and lockfile drift as failures.

## Rust suites

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings -W clippy::pedantic
cargo test --workspace --all-targets --locked
cargo doc --workspace --no-deps --locked
```

Coverage includes strict descriptor parsing and chain binding, namespace vectors, announcement policy, sequence replacement, bounded deterministic candidate selection, valid and invalid ENRs, libp2p Signed Envelope tampering and identity mismatch, endpoint policy, block timestamp search, adaptive ranges, namespace/type/expiry rejection, duplicates, reorg checkpoints, provider conversion, cache tamper/expiry revalidation, bootstrap ordering, dial bounds, seed promotion, renewal, identity persistence, status serialization, and native Noise connection behavior.

## Contract suites

```bash
forge fmt --root contracts --check
forge test --root contracts -vv
FOUNDRY_PROFILE=ci forge test --root contracts -vv
```

The default suite contains example, fuzz, and stateful invariant tests. The CI profile raises fuzz runs and invariant depth. Assertions cover exact constants and event expiry, every input boundary, permissionless callers, lack of admin selectors, and absence of mutable storage.

The fork suite is safe to run without a credential—it skips live-state assertions when `MAINNET_RPC_URL` is empty. With a URL it creates a real-state fork, deploys a new registry, announces from an unrelated address, and rechecks permissionless/stateless behavior. It also checks the published Ethereum mainnet deployment block, runtime-bytecode hash, constants, and empty storage:

```bash
MAINNET_RPC_URL=https://... \
  forge test --root contracts --match-contract ResurrectRegistryV1ForkTest -vv
```

The RPC URL is supplied only at test time and is never embedded in deployment metadata or package defaults.

## TypeScript suites

```bash
pnpm --filter @resurrect-protocol/client check
pnpm --filter @resurrect-protocol/client build
pnpm --filter @resurrect-protocol/client test
```

Tests cover strict descriptors, namespace parity, custom JSON-RPC, injected EIP-1193 reads without account requests, explicit-only URL persistence, wrong-chain rejection, provider replacement, constant verification, adaptive log ranges, duplicate/expiry filtering, candidate bounds, secure browser endpoints, private/native-only rejection, and signed-envelope tampering.

CI runs the package on Node.js 22 and 24.

## Cross-language vectors

Checked-in descriptor, registry-event, and peer-record vectors are consumed independently across implementations. Rust and TypeScript agree on descriptor normalization, namespace derivation, `PeerAnnounced` topics/data decoding, peer ID, signed-envelope bytes, sequence, and endpoint filtering. `test-vectors/peer-records/libp2p-ed25519.json` is generated from a deterministic Ed25519 seed and contains both a secure browser endpoint record and a native-only record.

Regenerate for inspection with:

```bash
cargo run -p resurrect-libp2p --example generate_vector
```

Do not update the checked-in vector casually; a change should explain the intended wire-format difference and must pass both language suites.

## End-to-end implementer checklist

```bash
scripts/checklist-integration.sh
```

The script runs prerequisite suites, deploys the exact registry to a fresh Anvil chain, verifies its complete method-selector surface, and launches actual `resurrect-node` processes. It tests:

1. A starts with no peers/events and self-announces.
2. B has no cache, native discovery, DNS seed, or knowledge of A; it scans Resurrect and completes an authenticated Noise connection to A.
3. A/B restart; C starts with an authenticated configured libp2p peer and an unreachable RPC, then joins natively with zero registry scan attempts. This is deterministic on CI runners where multicast interfaces are absent or unroutable.
4. All processes stop; unrelated D/E identities and payer accounts reboot despite stale unreachable records.
5. F/G start simultaneously under a fresh namespace, both announce, and a connection forms.

The test uses private endpoints only under an explicit local-test flag. It has bounded waits and captures per-node logs/status on failure. CI uploads `artifacts/implementer-checklist.json` even when the job fails.

Ports default to Anvil `18545` and node TCP `42001` through `42007`; do not run conflicting listeners. The Anvil port can be changed with `RESURRECT_TEST_RPC_PORT`.

## Package verification

```bash
scripts/check-packages.sh
```

This compares canonical contract sources and deployment manifests, validates the public ABI and pinned Ethereum metadata, creates all four crates.io archives, and packs both npm packages. It detects missing package content, invalid metadata, dependency version drift, and unintended source or deployment-record divergence before publication.

## CI job map

| Job | Purpose |
|---|---|
| `rust` | formatting, strict lint, unit/integration tests, docs |
| `contracts` | formatting, high-run fuzz/invariant tests, sizes, source parity |
| `browser` | two supported Node versions, types, build, tests |
| `packages` | every publishable archive can be constructed |
| `fork` | optional real-state suite |
| `implementer-checklist` | real EVM and multi-process conformance |

A green unit suite alone is not release evidence. The stable release workflow reruns Rust, Foundry, and npm tests before publication.
