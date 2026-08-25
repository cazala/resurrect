# `@rbp-protocol/client`

Browser/static RBP v1 registry discovery with caller-supplied JSON-RPC or EIP-1193 providers. The package verifies descriptors, provider chain and contract constants, recent registry logs, standard libp2p Signed Envelopes, peer identity, record sequence, expiry, and secure browser endpoint policy.

It does not request wallet accounts, ship a mandatory RPC hostname, persist URLs automatically, dial a transport, or authenticate your application protocol.

## Install

```bash
npm install @rbp-protocol/client
```

Node.js 22 or newer is required for the supported server-side toolchain. The emitted ESM targets modern browsers and ES2022.

## Create a descriptor and provider

```ts
import {
  RbpBrowserClient,
  deriveNamespace,
  injectedProvider,
  jsonRpcProvider,
  parseDescriptor
} from '@rbp-protocol/client'

const descriptor = parseDescriptor({
  rbpVersion: 1,
  registry: {
    chainId: 1,
    address: '0x1111111111111111111111111111111111111111',
    deploymentBlock: 21_000_000,
    maxTtlSeconds: 7_776_000
  },
  namespace: deriveNamespace('your-application', 1),
  acceptedRecordTypes: [2]
})

const provider = selectedEip1193Provider
  ? injectedProvider(selectedEip1193Provider)
  : jsonRpcProvider(userEnteredRpcUrl)

const client = new RbpBrowserClient(descriptor, provider)
const report = await client.scan()
for (const candidate of report.candidates) {
  await yourAuthenticatedBrowserTransport.dial(candidate.peerId, candidate.endpoints)
}
```

Replace placeholder deployment values with independently verified application configuration. The descriptor must not contain an RPC URL.

## Provider behavior

The provider abstraction exposes `request(method, params)`. Discovery calls only read methods and rejects a provider whose `eth_chainId` or registry constants differ from the descriptor. The injected adapter does not call `eth_requestAccounts`.

Replace a failed or untrusted provider without reconstructing the client:

```ts
client.setProvider(jsonRpcProvider(replacementUrl))
await client.verifyProvider()
```

Custom URLs remain in memory by default. Persistence requires an explicit call:

```ts
persistJsonRpcUrl(userApprovedUrl, window.localStorage)
```

Do not log URLs containing credentials. Surface CORS, TLS, mixed-content, wrong-chain, and provider-limit errors to the user.

## Scan options

```ts
const report = await client.scan({
  confirmations: 12n,
  initialChunkSize: 20_000n,
  minimumChunkSize: 64n,
  maxLogs: 50_000,
  maxCandidates: 256,
  maxEndpointsPerRecord: 16,
  allowPrivateEndpoints: false
})
```

The scanner binary-searches block timestamps to avoid genesis scans and automatically reduces log ranges after common provider limit errors. Individual invalid events are counted in `recordsRejected`; provider and configuration failures reject the scan.

`candidates` contain the peer ID, signed-record sequence, secure browser endpoints, raw signed envelope, registry expiry, block number, and log index. Duplicate peers retain the highest signed sequence, with onchain position as a tie-breaker.

## Browser endpoint profile

Codec 2 records must contain at least one signed multiaddr usable by a browser: WebTransport, secure WebSocket, HTTPS, or TLS+WebSocket. A present `/p2p` component must agree with the signed identity. Private/special IP literals and plaintext native-only TCP endpoints are rejected by default.

RBP discovery is not application authorization. After dialing, authenticate the expected transport peer and perform your application's normal version/capability/membership handshake.

## Public API

- `parseDescriptor`, `parseDescriptorJson`, `deriveNamespace`
- `jsonRpcProvider`, `injectedProvider`, `persistJsonRpcUrl`
- `RbpBrowserClient`, `scanRegistry`, `verifyProvider`
- `decodeBrowserPeerRecord`
- TypeScript interfaces for descriptors, providers, scan options/reports, and candidates

## Security

RPC results and registry events are untrusted. Keep log, candidate, endpoint, and dial limits bounded. Permit provider replacement. Do not infer trust from the transaction sender or registry ordering. See the repository [security model](https://github.com/cazala/rbp/blob/main/docs/security.md) and specification for the complete threat model.

Licensed under [MIT](https://opensource.org/license/mit) or [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) at your option.
