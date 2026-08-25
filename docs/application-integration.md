# Application integration

## Define the application profile

An application adopting RBP must decide and document:

- its stable application identifier and major protocol version;
- the derived namespace;
- an independently verified immutable registry deployment;
- accepted record codecs;
- supported native and browser dial contexts;
- the authenticated application handshake performed after transport connection;
- connection target, dial limits, timeouts, cache policy, and seed eligibility; and
- how users replace or supply registry-chain providers.

Derive the namespace as `keccak256(UTF8("rbp:<application>:<major>"))`. Change the major component when versions cannot safely share the same overlay. Do not derive it from a maintainer domain that may disappear.

## Distribute a descriptor

Embed or ship the descriptor with application releases. Pin the chain ID, contract address, deployment block, `7776000` maximum TTL, namespace, and accepted codecs. Do not put an RPC URL, DNS bootstrap name, token, owner, or mutable control plane into the descriptor.

Use JSON integers within the safe-integer range for the portable descriptor form. Both reference implementations accept canonical unsigned decimal strings for larger chain IDs and deployment blocks; the Rust serializer emits that form when necessary. Both parsers enforce the on-chain `uint256` chain-ID and `uint64` block-number widths. Unknown fields, duplicate codecs, wrong constants, and malformed addresses/namespaces are rejected.

## Compose the Rust libraries

Applications can consume the released crates independently:

- `rbp-core` for descriptors, namespace derivation, codec registration, validation, and candidate bounds;
- `rbp-ethereum` for the provider abstraction, Alloy HTTP adapter, scanner, and generated contract calls;
- `rbp-libp2p` for ENR and libp2p Signed Envelope verification; and
- `rbp-node` for reusable bootstrap traits, SQLite cache, native libp2p host, announcer, and supervisor.

The bootstrap controller depends on `DiscoverySource`, `NativeDiscovery`, `PeerConnector`, and `AnnouncementPublisher`. An application may implement these traits around its existing DHT, discv5, peer exchange, transport, or metrics system. It should pass registry-validated peers into its ordinary peer store and return to native discovery after connectivity forms.

## Dial and handshake sequence

For every candidate:

1. use only endpoints authenticated inside the accepted signed record;
2. apply environment, transport, network-range, and diversity policy;
3. bound parallel dials and per-dial time;
4. authenticate the expected peer identity in the transport; and
5. perform the application's own protocol/version/membership handshake.

Do not grant privileges based on the transaction sender, namespace presence, gas expenditure, event age, or registry ordering.

## Seed operation

A seed should publish only after it has a stable identity and externally verified inbound reachability. The reference policy announces for seven days when rebooting an empty network, uses a 30-day healthy TTL, and renews every 14 days. Applications may tune those values below the registry maximum.

When endpoints change, increment the signed-record sequence and publish immediately. Retain the same peer identity only when doing so matches the application's privacy and identity model.

## Browser applications

If immutable browser recovery is a goal, require seeds to advertise at least one signed endpoint usable by the browser transport. The TypeScript package validates and returns these candidates but deliberately does not choose a libp2p browser runtime for the application. See [Browser client](browser-client.md).

## Adoption checklist

- Descriptor is immutable application configuration and has no RPC hostname.
- Registry bytecode/source and deployment block were independently verified.
- At least one signed-record codec and dial context are specified.
- Post-dial application authentication is explicit.
- Candidate, log, dial, retry, and cache resources are bounded.
- Native discovery remains active and can operate during registry outages.
- Users can supply/replace the EVM provider.
- A clean-room reboot has been tested with unrelated identities and operators.
