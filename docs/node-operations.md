# Native node operations

## Roles

A light node scans and dials but never writes. A seed adds `--seed`, an Ethereum payer key, and one or more explicitly advertised endpoints. Both roles run the same native rust-libp2p host, peer-record validators, SQLite cache, and retry supervisor.

## Files and secrets

| Item | Sensitivity | Lifecycle |
|---|---|---|
| descriptor JSON | public, integrity-critical | distribute with the application release |
| libp2p identity file | secret, identity-critical | back up securely; stable across restarts |
| Ethereum payer key | secret, spend-capable | environment/secret manager; limited balance |
| SQLite peer cache | untrusted, disposable | may be removed; records are revalidated |
| status JSON | operational metadata | optional, atomically replaced |

Never commit payer or identity keys. Prefer a dedicated payer account with only enough funds for renewal. The payer address is not the libp2p identity and receives no protocol authority.

The reference node can derive a namespace from an application identifier and major version, or accept an already-derived namespace, and combine it with the published Ethereum mainnet registry. Use `--descriptor` instead for another verified registry or codec profile. `--application`, `--namespace`, and `--descriptor` are mutually exclusive descriptor sources. None selects an RPC provider.

## Starting a light node

```bash
resurrect-node \
  --application your-application \
  --major-version 1 \
  --rpc-url https://caller-selected-ethereum-mainnet-rpc.example \
  --identity /var/lib/resurrect/identity.key \
  --cache /var/lib/resurrect/peers.sqlite3 \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --status-file /run/resurrect/status.json
```

The node first tries its verified cache and native observations from configured peers, mDNS, and identify. It touches the provider only when Resurrect discovery becomes necessary. An unreachable RPC therefore does not prevent a healthy native component from accepting or finding peers. Supply a repeatable `--native-peer /dns4/seed.example/tcp/4001/p2p/<peer-id>` when deterministic native bootstrap is preferable to multicast discovery; the Noise handshake authenticates the configured peer ID.

## Starting a seed

```bash
export RESURRECT_ETHEREUM_PRIVATE_KEY=0x...
resurrect-node \
  --application your-application \
  --major-version 1 \
  --rpc-url https://caller-selected-ethereum-mainnet-rpc.example \
  --identity /var/lib/resurrect/identity.key \
  --cache /var/lib/resurrect/peers.sqlite3 \
  --seed \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --listen /ip4/127.0.0.1/tcp/4002/ws \
  --advertise /dns4/seed.example/tcp/4001 \
  --advertise /dns4/seed-ws.example/tcp/443/wss
```

The documentation name above is illustrative and should not be used in production. Use an actually reachable address. Seed startup fails if the signing key or advertised endpoint is absent. Before every announcement the node verifies that its provider's chain ID and registry constants match the descriptor.

The two structured flags construct and hash the exact UTF-8 preimage `resurrect:<application>:<major-version>`. Use `--namespace 0x...` when the derived value is already distributed as integrity-critical application configuration. For a custom deployment, use `--descriptor /etc/resurrect/network.json` instead. Do not reuse another application's identifier or namespace.

## Endpoint policy

The advertised address is signed and permanently visible in an event. Confirm inbound reachability from outside the host and advertise the public address, not `0.0.0.0`. Production defaults reject private, loopback, unspecified, multicast, and documentation ranges. `--allow-private-endpoints` exists only for local tests and intentionally private overlays.

The built-in host supports TCP and WebSocket with Noise authentication and
Yamux multiplexing. Repeat `--listen` to serve both transports. Browsers require
a secure `wss` address; terminate TLS at a reverse proxy or tunnel and forward
WebSocket upgrades to a loopback `/ws` listener. Do not expose that plain
listener publicly when the TLS edge is the intended path.

The signed peer record should contain the public address, such as
`/dns4/seed-ws.example/tcp/443/wss`, not the loopback origin. A peer record may
contain both the native TCP and WSS endpoints for the same identity. Configured
peers, mDNS, and identify are the included native mechanisms. Applications
requiring QUIC, WebTransport, discv5, DHT, or peer exchange should compose the
libraries into their own host or extend the adapter.

## TLS and WebSocket edge

The node intentionally serves libp2p-over-WebSocket, not a JSON API. An HTTPS
edge must preserve WebSocket upgrade headers and stream bytes without request
body transformation or caching. End-to-end peer authentication does not depend
on the edge certificate: the browser first validates TLS, then Noise proves the
libp2p identity contained in the signed registry record.

When using Cloudflare Tunnel, publish an HTTP ingress to the local plain
WebSocket listener. Cloudflare accepts public HTTPS/WSS and performs the upgrade
through the tunnel. Keep the connector token root-only, use a dedicated tunnel,
and retain a final `http_status:404` ingress rule. See [Hosted services](hosted-services.md)
for the reference deployment.

## Finality and development chains

Production scanning prefers `finalized`, then `safe`, then latest-minus-confirmations if those tags are unsupported. Some local chains expose finality tags that remain on the deployment block. Use `--allow-unfinalized --fallback-confirmations 0` for Anvil and similar controlled development networks. Do not disable finality casually on public chains.

## Resource controls

Important defaults are two required peers, eight parallel dials, eight seconds per dial, 12 fallback confirmations, a seven-day reboot TTL, 30-day maintenance TTL, 14-day renewal, and jittered exponential retry backoff capped at five minutes. Set the minimum to one for a two-node recovery demonstration. The scanner independently caps log records, endpoints, candidates, and provider range width.

## Observability

Set `RUST_LOG` for tracing filters and `--log-format json` for structured output. The optional status file includes:

- peer ID and state-machine state;
- authenticated connection count and latest successful discovery source;
- ordered transitions from the last cycle;
- registry scan, failure, and announcement counters;
- the most recent recoverable error; and
- update time.

Protect status and log output according to your privacy model because peer IDs, endpoints, provider failures, and timing can be sensitive.

## Failure handling

- **Wrong chain or wrong contract:** scanning and publishing fail closed; correct the provider or descriptor.
- **RPC unavailable:** native connectivity continues; isolated recovery retries with backoff.
- **Dead announced peers:** dials time out under bounded concurrency and later sources/cycles continue.
- **Corrupt cache:** invalid rows are ignored after cryptographic revalidation; the cache can be deleted.
- **Lost libp2p identity:** the node becomes a new peer and must publish a fresh signed record.
- **Lost payer key:** use any other funded Ethereum account; no ownership transfer is required.
- **All nodes stopped:** start an eligible seed, wait for its announcement, then start another participant.

## Upgrade and shutdown

Graceful termination waits for Ctrl-C and shuts down the native host. Old announcements cannot be revoked; they expire, and higher signed-record sequences supersede them. During upgrades preserve the identity, verify descriptor compatibility, and avoid advertising an endpoint until the replacement listener is reachable.

Adding or removing an advertised endpoint produces a higher-sequence signed
record on the next announcement. Because an older record remains onchain until
expiry, clients must continue to prefer the newest valid sequence for a peer.
