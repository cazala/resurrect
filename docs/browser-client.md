# Browser and static client

`@resurrect-protocol/client` implements read-only Resurrect discovery for web and static applications. It accepts a caller-owned provider and does not contain a mandatory RPC hostname, wallet, hosted API, analytics endpoint, or account-access flow.

Use `ethereumMainnetDescriptor(deriveNamespace(application, majorVersion))` to pin the published Ethereum registry while retaining an application-specific namespace. Use `parseDescriptor` for another verified deployment. Neither path chooses or stores a provider URL.

## Provider choices

Use `jsonRpcProvider(url)` for a URL entered or configured by the user, or `injectedProvider(eip1193)` for a wallet/host provider. Both expose the same minimal `RegistryProvider` interface. A failing provider can be replaced at runtime with `client.setProvider(replacement)`.

```ts
const client = new ResurrectBrowserClient(
  parseDescriptor(descriptorJson),
  jsonRpcProvider(userRpcUrl)
)

try {
  const report = await client.scan()
  connectWithApplicationTransport(report.candidates)
} catch (error) {
  showProviderFailure(error)
  client.setProvider(injectedProvider(selectedWalletProvider))
}
```

The injected adapter only forwards methods requested by discovery. Scanning uses `eth_chainId`, `eth_call`, `eth_getBlockByNumber`, and `eth_getLogs`; it never calls `eth_requestAccounts`.

## URL privacy

`jsonRpcProvider` keeps its URL inside the provider instance. The package has no automatic storage or telemetry. Persist only after clear user action:

```ts
persistJsonRpcUrl(userRpcUrl, window.localStorage)
```

An RPC operator can still observe the user's IP, namespace, contract, and query timing. A malicious provider can omit logs or serve stale data. Let users replace providers and consider comparison or a local node for higher-assurance use. Browser CORS, TLS, and mixed-content errors surface as provider failures to the caller.

## Validation and scan behavior

Before logs are used the client verifies exact descriptor structure, chain ID, and the deployed registry constants. It selects a confirmed head, binary-searches the maximum-TTL timestamp window, requests filtered event logs in bounded chunks, and reduces the range after provider limit errors.

Each event is checked for source, topic, namespace, accepted codec, expiry against chain time, and bounded record size. Codec 2 is opened and certified in the standard libp2p Signed Envelope domain; the payload peer ID must match the signing public key. Candidates are sequence-deduplicated and retained under a deterministic cap.

## Browser dial context

The package accepts only secure browser-capable signed multiaddrs containing WebTransport, WSS, HTTPS, or TLS+WebSocket. Private and special-use IP literals are rejected unless `allowPrivateEndpoints` is set for an intentional local/private environment. A trailing `/p2p/<peer-id>`, when present, must match the signed record identity.

The scanner does not create a browser libp2p node or perform an application request. The application profile must translate returned endpoints into its chosen browser transport and must authenticate the expected peer plus application protocol after dialing.

## Reference explorer and live probe

The static reference application in `apps/explorer` is deployed at
[resurrect.caza.la](https://resurrect.caza.la). It uses the canonical Ethereum
deployment and the repository's demonstration namespace. The default public RPC
is `https://eth.drpc.org`; users can replace it in memory or select an injected
wallet provider. Selecting a wallet does not connect an account, request a
signature, or submit a transaction.

After a scan, the explorer reports four distinct values:

- matching registry announcements processed in the bounded scan;
- deduplicated, unexpired browser-compatible candidates;
- rejected or native-only records; and
- the confirmed head and scanned block window.

None is an authoritative network-size metric. Registry events persist after a
peer becomes unreachable, one peer may announce repeatedly, and participants
that joined through native discovery need not announce at all.

For each candidate, the explorer can create an ephemeral browser libp2p node,
dial the signed WSS endpoint, negotiate Noise and Yamux, compare the
authenticated remote peer ID with the signed record, run identify, and issue a
standard libp2p ping. It displays connection time, ping round-trip time, remote
agent/protocol versions, and supported protocols. Closing or navigating away
from the page stops the ephemeral node. A successful ping proves current
transport reachability and control of that libp2p identity; it does not prove
application membership, honest behavior, data availability, or authorization.

The reference WSS endpoint is
`/dns4/resurrect-ws.caza.la/tcp/443/wss`. TLS terminates at Cloudflare Tunnel;
Noise remains the end-to-end peer authentication layer.

## Resource options

`scan` accepts confirmations, initial/minimum chunk width, maximum raw logs, maximum retained candidates, maximum endpoints per signed record, and a private-endpoint opt-in. Counts must be positive safe integers and block quantities must be non-negative. Defaults follow the v1 resource recommendations.

## Error handling

Treat wrong chain, wrong registry constants, malformed provider data, and irreducible range failures as configuration/provider errors. Individual bad events are rejected without failing the entire scan. Report provider errors without logging secret URL components. Do not silently switch chains or request wallet accounts to repair discovery.
