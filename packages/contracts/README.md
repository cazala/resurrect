# `@resurrect-protocol/contracts`

Canonical, permissionless Resurrect Registry v1 Solidity source and ABI. The contract is stateless and has no owner, administrator, allowlist, pause, upgrade, withdrawal, or peer-storage path. Anyone may publish a bounded signed peer record. Applications consume recent `PeerAnnounced` logs as untrusted discovery hints.

```bash
npm install @resurrect-protocol/contracts
```

```js
import registryAbi from '@resurrect-protocol/contracts/abi/ResurrectRegistryV1.json' with { type: 'json' }
import ethereumMainnet from '@resurrect-protocol/contracts/deployments/ethereum-mainnet.json' with { type: 'json' }
```

Solidity tools can import `@resurrect-protocol/contracts/src/ResurrectRegistryV1.sol`. The source exposes exactly `VERSION`, `MAX_TTL`, `MAX_RECORD_BYTES`, and `announce`. TTL must be between one second and 90 days; records must be between one and 4096 bytes. Expiry is computed from the block timestamp and emitted with the namespace, codec, and bytes.

The reference deployment is `0x6F33c332e8251dcd307D85A27fCcAbd85d578910` on Ethereum mainnet (chain ID `1`, receipt block `25882327`). Its exact transaction, deployer, compiler settings, runtime-bytecode hash, tagged source revision, and Etherscan and Sourcify verification URLs are published in the deployment JSON above. Consumers should independently verify those facts before relying on them.

The source is released under [CC0-1.0](https://creativecommons.org/publicdomain/zero/1.0/). Applications may use the reference deployment or deploy the exact source elsewhere, but must always pin the chosen chain, address, deployment block, namespace, and constants in their descriptor. A shared stateless registry does not create a shared peer list: namespaces remain application-specific, RPC providers remain caller-selected, and the deployer has no contract authority.

An onchain event is an untrusted discovery hint. Consumers must still verify the embedded signed peer record, sequence, expiry, endpoint policy, and application handshake.
