# `@resurrect-protocol/contracts`

Canonical, permissionless Resurrect Registry v1 Solidity source and ABI. The contract is stateless and has no owner, administrator, allowlist, pause, upgrade, withdrawal, or peer-storage path. Anyone may publish a bounded signed peer record. Applications consume recent `PeerAnnounced` logs as untrusted discovery hints.

```bash
npm install @resurrect-protocol/contracts
```

```js
import registryAbi from '@resurrect-protocol/contracts/abi/ResurrectRegistryV1.json' with { type: 'json' }
```

Solidity tools can import `@resurrect-protocol/contracts/src/ResurrectRegistryV1.sol`. The source exposes exactly `VERSION`, `MAX_TTL`, `MAX_RECORD_BYTES`, and `announce`. TTL must be between one second and 90 days; records must be between one and 4096 bytes. Expiry is computed from the block timestamp and emitted with the namespace, codec, and bytes.

The source is released under [CC0-1.0](https://creativecommons.org/publicdomain/zero/1.0/). Consumers should deploy and verify the exact source themselves, then pin the chain ID, address, deployment block, and constants in their application descriptor. This package does not designate or endorse a production deployment.

An onchain event is an untrusted discovery hint. Consumers must still verify the embedded signed peer record, sequence, expiry, endpoint policy, and application handshake.
