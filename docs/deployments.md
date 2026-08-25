# Registry deployments

This repository does not claim a canonical production RBPRegistryV1 address. Applications choose and verify their own immutable deployment, then pin it in their descriptor. Publishing an address here without independent verification would create exactly the operator dependency RBP is designed to avoid.

## Deploy locally

```bash
anvil --chain-id 31337
forge create \
  --root contracts \
  src/RBPRegistryV1.sol:RBPRegistryV1 \
  --rpc-url http://127.0.0.1:8545 \
  --private-key 0x... \
  --broadcast
```

Record the chain ID, deployed address, and receipt block. The deployer has no special permission after construction because the contract has no constructor state or administrative surface.

## Production verification procedure

1. Choose an EVM chain whose longevity, availability, finality, censorship resistance, cost, and RPC ecosystem fit the application's recovery assumptions.
2. Build the exact tagged `contracts/src/RBPRegistryV1.sol` with the pinned Solidity/Foundry settings.
3. Review the creation and runtime bytecode and publish verified source through the chain's normal explorer tooling.
4. Confirm `VERSION() == 1`, `MAX_TTL() == 7776000`, and `MAX_RECORD_BYTES() == 4096`.
5. Confirm the only callable function selectors are the three constants and `announce(bytes32,uint32,uint32,bytes)`.
6. Confirm an unrelated address can announce and sampled storage remains empty.
7. Pin the deployment receipt block—not zero/genesis unless that is truly correct—in the application descriptor.
8. Reproduce verification independently and distribute the descriptor with a signed application release.

Do not add a proxy, owner, pause, allowlist, fee withdrawal, mutable namespace mapping, or peer storage. Those deployments would not be the canonical v1 contract described by this repository.

## Descriptor example

```json
{
  "rbpVersion": 1,
  "registry": {
    "chainId": "1",
    "address": "0x1111111111111111111111111111111111111111",
    "deploymentBlock": "21000000",
    "maxTtlSeconds": 7776000
  },
  "namespace": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "acceptedRecordTypes": [2]
}
```

Addresses and numbers above are placeholders. A descriptor never includes an RPC URL. Users supply a chain provider independently and clients verify its chain ID and registry constants before scanning.

## Deployment registry policy

If this project later documents community deployments, each entry should include chain ID, address, deployment block, source tag/commit, runtime bytecode hash, transaction hash, explorer verification link, and at least two independent reproduction reports. Listing is informational, not endorsement, governance, or a protocol default.
