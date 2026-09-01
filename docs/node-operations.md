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

The reference node can build its descriptor from an explicit application namespace and the published Ethereum mainnet registry. This is the shortest production configuration. Use `--descriptor` instead for another verified registry or codec profile; `--namespace` and `--descriptor` are mutually exclusive. Neither form selects an RPC provider.

## Starting a light node

```bash
resurrect-node \
  --namespace 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
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
  --namespace 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --rpc-url https://caller-selected-ethereum-mainnet-rpc.example \
  --identity /var/lib/resurrect/identity.key \
  --cache /var/lib/resurrect/peers.sqlite3 \
  --seed \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --advertise /dns4/seed.example/tcp/4001
```

The documentation name above is illustrative and should not be used in production. Use an actually reachable address. Seed startup fails if the signing key or advertised endpoint is absent. Before every announcement the node verifies that its provider's chain ID and registry constants match the descriptor.

For a custom deployment, replace `--namespace ...` with `--descriptor /etc/resurrect/network.json`. Derive the namespace as `keccak256("resurrect:<application>:<major-version>")` and distribute it as integrity-critical application configuration; do not reuse the placeholder value above.

## Endpoint policy

The advertised address is signed and permanently visible in an event. Confirm inbound reachability from outside the host and advertise the public address, not `0.0.0.0`. Production defaults reject private, loopback, unspecified, multicast, and documentation ranges. `--allow-private-endpoints` exists only for local tests and intentionally private overlays.

The built-in host currently supports TCP with Noise authentication and Yamux multiplexing. Configured peers, mDNS, and identify are the included native mechanisms. Applications requiring QUIC, WebTransport, WebSocket, discv5, DHT, or peer exchange should compose the libraries into their own host or extend the adapter.

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
