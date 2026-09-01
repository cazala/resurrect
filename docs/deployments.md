# Registry deployments

## Reference Ethereum mainnet deployment

The reference packages default to this immutable `ResurrectRegistryV1` deployment:

| Field | Value |
|---|---|
| Network | Ethereum mainnet |
| EIP-155 chain ID | `1` |
| Contract | `0x6F33c332e8251dcd307D85A27fCcAbd85d578910` |
| Deployment block | `25882327` |
| Deployment time | `2026-09-01T12:07:23Z` |
| Transaction | `0x41f8b9e49265c5796c627eb8e32bd0d366f9408dfc0c75861189758f638440ab` |
| Deployer | `0x318027A00a3A3eB6A7d6F45C832e47c126B4F2C2` |
| Runtime bytecode hash | `0x0024244f6ad881009b5726d2c1644a3c2aff178852c4d01b1066cd7d9967c109` |
| Tagged source | `v0.1.0`, commit `3298158d0e86959a05434495eb28335808e7964a` |
| Compiler | Solidity `0.8.24`, optimizer enabled, `20000` runs, CBOR metadata disabled, bytecode hash `none` |

Inspect the [transaction on Etherscan](https://etherscan.io/tx/0x41f8b9e49265c5796c627eb8e32bd0d366f9408dfc0c75861189758f638440ab), the [contract on Etherscan](https://etherscan.io/address/0x6F33c332e8251dcd307D85A27fCcAbd85d578910), or the [verified source on Sourcify](https://repo.sourcify.dev/1/0x6F33c332e8251dcd307D85A27fCcAbd85d578910). The complete record is machine-readable at [`deployments/ethereum-mainnet.json`](../deployments/ethereum-mainnet.json) and is also published by `@resurrect-protocol/contracts/deployments/ethereum-mainnet.json`.

Deployment verification established all of the following:

- the receipt succeeded at the pinned block;
- the onchain runtime bytecode is byte-for-byte equal to the local build;
- Sourcify reports matching creation and runtime code for the published source;
- `VERSION() == 1`, `MAX_TTL() == 7776000`, and `MAX_RECORD_BYTES() == 4096`;
- the contract exposes only the three constant getters and `announce(bytes32,uint32,uint32,bytes)`;
- sampled storage remains empty; and
- the deployer has no owner, upgrade, pause, allowlist, withdrawal, or namespace authority.

The deployment is a convenient shared log contract, not a canonical peer list or control plane. Applications still choose their own namespace and signed-record codecs. Users still choose their own RPC provider. Any account may publish under any namespace, and clients treat every event as untrusted until its signed peer record and application handshake are verified.

## Reference descriptor

Replace the namespace below with the application-derived value:

```json
{
  "resurrectVersion": 1,
  "registry": {
    "chainId": 1,
    "address": "0x6F33c332e8251dcd307D85A27fCcAbd85d578910",
    "deploymentBlock": 25882327,
    "maxTtlSeconds": 7776000
  },
  "namespace": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "acceptedRecordTypes": [2]
}
```

A descriptor never includes an RPC URL. The Rust and TypeScript packages expose constructors and constants for the reference deployment, while the native node accepts `--namespace` as its Ethereum-mainnet shortcut.

## Independent verification

Do not trust documentation alone. A production integrator should:

1. obtain the transaction, receipt, code, and block from a caller-selected Ethereum mainnet provider;
2. build tagged source commit `3298158d0e86959a05434495eb28335808e7964a` with the pinned Foundry settings;
3. compare the complete deployed runtime bytecode and its hash;
4. inspect the verified source and compiler input through an independent explorer;
5. call all three constants and inspect the four-selector surface; and
6. pin the address and receipt block in the application release.

With Foundry, the repository's optional live fork test performs the code-hash, constant, deployment-block, and empty-storage checks:

```bash
MAINNET_RPC_URL=https://your-ethereum-mainnet-rpc.example \
  forge test --root contracts --match-contract ResurrectRegistryV1ForkTest -vv
```

## Deploying another exact registry

Applications may deploy the same immutable contract on another EVM chain when its availability, finality, censorship resistance, cost, or RPC ecosystem better fits their recovery assumptions:

```bash
forge create \
  --root contracts \
  src/ResurrectRegistryV1.sol:ResurrectRegistryV1 \
  --rpc-url https://caller-selected-rpc.example \
  --private-key 0x... \
  --broadcast
```

Record the chain ID, deployed address, receipt block, transaction, exact source revision, compiler settings, runtime bytecode hash, and public verification link. The deployer has no special permission after construction because the contract has no constructor state or administrative surface.

Do not add a proxy, owner, pause, allowlist, fee withdrawal, mutable namespace mapping, or peer storage. Such a deployment would not implement the canonical v1 registry semantics.

## Deployment-listing policy

Future community deployment entries must provide the same reproducible evidence as the Ethereum mainnet entry. A listing is informational: it does not grant a deployer governance, make one namespace global, bundle an RPC provider, or prevent applications from choosing another exact deployment.
