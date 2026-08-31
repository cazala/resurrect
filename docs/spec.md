# Resurrect v1

**Status:** Draft implementation specification  
**Version:** 1  
**Scope:** Generic, application-independent P2P cold-start and network-resurrection protocol  
**Primary anchor:** Ethereum / EVM event logs  

## 1. Abstract

Resurrect is a generic bootstrap layer for peer-to-peer networks that must remain recoverable even if every application-specific node, bootstrap server, domain, and original operator disappears.

Resurrect does **not** define an application messaging protocol, consensus protocol, DHT, pub/sub protocol, or data-availability layer. It defines only a permissionless **root rendezvous mechanism** that allows a node which knows no peers to discover recently announced peers, or to become the first peer of a new incarnation of the network.

The core mechanism is:

1. Each P2P application defines a Resurrect **network descriptor** containing an Ethereum registry address and a namespace.
2. Publicly reachable nodes publish self-authenticating peer records to a stateless Ethereum registry.
3. Announcements have bounded TTLs and exist only as Ethereum logs; the registry stores no peer set.
4. Nodes normally use their native P2P discovery mechanisms.
5. When native discovery fails, nodes query recent Resurrect announcements.
6. If no live peer can be found, an eligible node announces itself and becomes the first seed of the rebooted network.
7. Once connectivity is established, ordinary P2P discovery takes over and Resurrect leaves the hot path.

The intended failure property is:

```text
application network has zero live nodes
        +
all application-operated bootstrap infrastructure is gone
        +
original maintainers are gone
        +
Ethereum remains readable/writable
        ↓
a new participant can announce itself
        ↓
a second participant can discover it
        ↓
the P2P network can form again
```

Resurrect deliberately treats the Ethereum registry as **untrusted discovery information**. An onchain announcement is not an endorsement of a peer.

---

## 2. Goals

Resurrect v1 MUST provide:

- Permissionless peer announcement.
- No administrator, allowlist, owner, DAO, multisig, or upgrade key in the registry.
- Recovery from a state with zero live application peers.
- Application namespace isolation.
- Self-authenticating peer records.
- Bounded announcement lifetime.
- No unbounded registry contract state growth.
- Compatibility with existing peer discovery and transport stacks.
- A deterministic startup algorithm suitable for automated clients.
- A design in which Ethereum is used only as a cold-start/root-of-last-resort path during normal operation.
- A bootstrap path for static or immutable clients that does not require a protocol-controlled RPC hostname or application bootstrap hostname.

Resurrect SHOULD provide:

- Multiple peer-record codecs.
- Compatibility with EIP-778 ENRs.
- Compatibility with libp2p signed peer records.
- Partition healing when isolated network components later query Resurrect.
- Implementations that can run without any operator-controlled DNS name.
- Browser/static clients that can obtain registry-chain access from either a user-supplied JSON-RPC endpoint or an injected EIP-1193 provider.

---

## 3. Non-goals

Resurrect v1 does NOT attempt to provide:

- Application message dissemination.
- Historical application data recovery.
- Reliable message delivery.
- Application-level identity or reputation.
- Sybil resistance beyond the cost of Ethereum publication plus local peer policy.
- Proof that an announced endpoint is reachable.
- Proof that a peer provides any claimed application service.
- NAT traversal.
- Storage of P2P application state.
- A canonical global peer list.
- A token or economic incentive system.
- A canonical Ethereum JSON-RPC provider, RPC hostname, wallet, or gateway.

A rebooted network can recover **connectivity**, not application data that no participant retained.

---

## 4. Normative language

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT**, and **MAY** are to be interpreted as requirements for conforming implementations.

---

## 5. Terminology

### 5.1 Network

An application-specific P2P overlay that uses Resurrect only for bootstrap/recovery.

### 5.2 Namespace

A 32-byte identifier isolating one logical network from another.

### 5.3 Registry

An immutable Ethereum contract that accepts bounded-TTL peer announcements and emits them as logs.

### 5.4 Peer record

A self-authenticating serialized record describing a peer identity and one or more network endpoints.

### 5.5 Seed

A publicly reachable peer willing to be discovered by nodes that currently have no peers.

### 5.6 Native discovery

Any discovery mechanism used by the application outside Resurrect: discv5, TopDisc, Kademlia DHT, libp2p peer exchange, configured peers, mDNS, DNS discovery, etc.

### 5.7 Reboot

The process by which a network that currently has no reachable peers forms a connected component again using Resurrect.

---

## 6. Design principles

### 6.1 Discovery is not trust

Resurrect announcements MUST be treated as attacker-controlled input until the embedded peer record has been cryptographically validated.

### 6.2 Ethereum is the root rendezvous, not the data plane

Implementations SHOULD stop depending on Ethereum for peer discovery after they have enough peers to use native discovery safely.

### 6.3 Zero peers is a valid state

A node that finds no peer MUST NOT treat that condition as permanent network failure. If eligible to serve as a seed, it SHOULD announce itself.

### 6.4 Old announcements have no resurrection value

A peer record from years ago is unlikely to identify a reachable peer. Resurrect therefore bounds TTL and stores announcements only in logs.

### 6.5 No application-specific semantics in the registry

The registry knows only:

- namespace,
- peer-record codec,
- expiry,
- peer-record bytes.

It MUST NOT know application message types, capabilities, tokens, roles, or business logic.

---

## 7. Network descriptor

Every Resurrect-enabled application MUST define or embed a Resurrect Network Descriptor.

Canonical JSON representation:

```json
{
  "resurrectVersion": 1,
  "registry": {
    "chainId": 1,
    "address": "0x0000000000000000000000000000000000000000",
    "deploymentBlock": 0,
    "maxTtlSeconds": 7776000
  },
  "namespace": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "acceptedRecordTypes": [1, 2]
}
```

Fields:

| Field | Type | Meaning |
|---|---|---|
| `resurrectVersion` | integer | MUST be `1` for this specification. |
| `registry.chainId` | uint256 | EVM chain containing the Resurrect registry. |
| `registry.address` | address | Registry contract address. |
| `registry.deploymentBlock` | uint64 | First block that can contain registry logs. |
| `registry.maxTtlSeconds` | uint32 | MUST equal the deployed contract's `MAX_TTL`. |
| `namespace` | bytes32 | Application/network identifier. |
| `acceptedRecordTypes` | uint32[] | Peer-record codecs accepted by this network. |

The descriptor is application configuration. Resurrect does not specify how a participant learns its application's descriptor.

### 7.1 Recommended namespace derivation

Applications SHOULD derive namespaces as:

```text
namespace = keccak256(
    UTF8("resurrect:") ||
    UTF8(application_identifier) ||
    UTF8(":") ||
    UTF8(major_protocol_version)
)
```

Example:

```text
keccak256("resurrect:example-network:1")
```

The major protocol version SHOULD change when two node versions cannot safely participate in the same overlay.

### 7.2 Registry-chain access

The Resurrect Network Descriptor intentionally does **not** contain a required JSON-RPC hostname. The descriptor identifies the registry chain and contract; the client obtains an RPC/provider independently.

A Resurrect implementation MUST support a caller-supplied registry-chain transport/provider abstraction.

A browser or static/immutable Resurrect client SHOULD support both of the following access modes:

1. **User-supplied JSON-RPC URL** using the Ethereum JSON-RPC API.
2. **Injected EIP-1193 provider** supplied by a wallet or host environment.

A client using an injected provider for Resurrect discovery MUST NOT request account exposure merely to read registry state. In particular, discovery does not require `eth_requestAccounts`. Read-only calls such as `eth_chainId`, `eth_getBlockByNumber`, and `eth_getLogs` are sufficient.

Before using a provider, the client MUST verify:

```text
eth_chainId == descriptor.registry.chainId
```

If the chain differs, the client MUST stop or ask the user to select/configure the correct chain/provider. It MUST NOT silently scan a same-address contract on another chain.

A static or immutable client whose purpose includes long-term recovery MUST NOT make a single hard-coded RPC hostname its only registry-access path. It MAY ship optional convenience RPC fallbacks, but:

- the user MUST be able to override them,
- failure of every bundled fallback MUST NOT prevent use of a user-supplied provider,
- bundled providers are convenience infrastructure, not part of Resurrect consensus or identity.

When accepting a user-supplied RPC URL, browser implementations SHOULD:

- keep it in memory by default,
- avoid analytics/logging that disclose it,
- not send it to any service other than the selected RPC endpoint,
- persist it only after explicit user action,
- surface browser CORS/mixed-content failures clearly.

References:

- EIP-1193 Provider API: <https://eips.ethereum.org/EIPS/eip-1193>
- EIP-1474 Ethereum JSON-RPC: <https://eips.ethereum.org/EIPS/eip-1474>
- EIP-6963 injected-provider discovery: <https://eips.ethereum.org/EIPS/eip-6963>

---

## 8. Registry contract

### 8.1 Required interface

A conforming v1 registry MUST expose semantics equivalent to:

```solidity
// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.24;

contract ResurrectRegistryV1 {
    uint32 public constant VERSION = 1;
    uint32 public constant MAX_TTL = 90 days;
    uint32 public constant MAX_RECORD_BYTES = 4096;

    error InvalidTTL();
    error RecordTooLarge();

    event PeerAnnounced(
        bytes32 indexed namespace,
        uint32 indexed recordType,
        uint64 validUntil,
        bytes peerRecord
    );

    function announce(
        bytes32 namespace,
        uint32 recordType,
        uint32 ttl,
        bytes calldata peerRecord
    ) external {
        if (ttl == 0 || ttl > MAX_TTL) revert InvalidTTL();
        if (peerRecord.length == 0 || peerRecord.length > MAX_RECORD_BYTES) {
            revert RecordTooLarge();
        }

        emit PeerAnnounced(
            namespace,
            recordType,
            uint64(block.timestamp) + uint64(ttl),
            peerRecord
        );
    }
}
```

The final deployment SHOULD use the smallest audited bytecode that preserves these semantics.

### 8.2 Registry invariants

The canonical v1 registry MUST:

- Have no owner.
- Have no upgrade mechanism.
- Have no pause mechanism.
- Have no withdrawal function.
- Have no registration allowlist.
- Have no per-namespace administrator.
- Keep no enumerable peer storage.
- Permit any EOA or contract to call `announce`.
- Compute `validUntil` from the block timestamp rather than accepting an arbitrary absolute expiry.
- Enforce a global `MAX_TTL`.
- Enforce a maximum record byte length.

### 8.3 Why the registry is log-only

Permanent peer storage would introduce state growth and cleanup requirements without improving rebootability. A reboot only needs **recent** reachable peers.

The Ethereum log is the publication record. Consumers discard it after `validUntil`.

### 8.4 Recommended TTL

The registry maximum is 90 days.

Seeds SHOULD normally announce for 30 days and renew every 14-21 days.

Longer TTLs reduce renewal cost but retain dead/spam records longer. Individual networks MAY recommend a smaller TTL without changing the registry.

---

## 9. Peer record codecs

Resurrect separates the Ethereum announcement envelope from the peer identity/address format.

`recordType` is a uint32 codec identifier.

Resurrect v1 reserves:

| `recordType` | Codec | Status |
|---:|---|---|
| `1` | EIP-778 Ethereum Node Record (RLP bytes) | Standard codec |
| `2` | libp2p signed peer/address record envelope | Standard integration codec |
| `0x80000000`-`0xffffffff` | Private/experimental | Non-portable |

Future Resurrect revisions may assign additional codec IDs.

### 9.1 EIP-778 ENR (`recordType = 1`)

The `peerRecord` MUST contain the raw RLP encoding of an EIP-778 ENR, not the textual `enr:` base64 representation.

Consumers MUST:

1. Reject records larger than the EIP-778 maximum.
2. Decode the ENR.
3. Verify the ENR signature according to its identity scheme.
4. Validate monotonically increasing ENR sequence numbers when multiple records for the same node are observed.
5. Extract only endpoints/capabilities understood by the application.

Resurrect does not alter ENR signing or node-ID derivation.

Reference: <https://eips.ethereum.org/EIPS/eip-778>

### 9.2 libp2p signed peer record (`recordType = 2`)

The `peerRecord` MUST be a self-certified libp2p peer/address record serialized in a Signed Envelope according to the libp2p signed-envelope/routing-record specifications used by the implementation.

Consumers MUST verify the Signed Envelope before using embedded addresses, MUST verify that the peer ID is derived from/matches the signing key, and MUST retain only the highest valid record sequence observed for a peer.

References:

- <https://github.com/libp2p/specs/blob/master/RFC/0002-signed-envelopes.md>
- <https://github.com/libp2p/specs/blob/master/RFC/0003-routing-records.md>

### 9.3 Application profile

An application using Resurrect MUST specify at least one accepted record type and MUST define how a validated peer record is converted into a dial attempt.

A node MUST NOT attempt to interpret an unknown record type.

### 9.4 Dial contexts and browser reachability

Resurrect peer records carry signed endpoint information, but Resurrect does not define which transport an application must use. An application profile MUST define how endpoints are selected for each client environment it intends to support.

Typical dial contexts include:

```text
native-server
browser
mobile
restricted-egress
```

These names are descriptive only and are not encoded by the Resurrect registry.

If an application requires an immutable browser client to bootstrap from Resurrect, its application profile MUST identify at least one browser-dialable endpoint representation that can be carried inside an accepted signed peer-record codec. Examples include browser-capable libp2p WebTransport/WebSocket multiaddrs or an HTTPS/libp2p-HTTP multiaddr.

The endpoint used for bootstrap MUST be authenticated by the accepted peer record or by a subsequent cryptographic/application handshake specified by the application profile. A client MUST NOT take an unsigned HTTP endpoint from an unrelated registry side channel and treat it as belonging to a signed peer identity.

Resurrect itself defines no `HTTP`, `SOLVER`, `RELAY`, or other application capability bitset. Such capabilities belong to the application protocol or its peer handshake.

---

## 10. Announcement validation

For every `PeerAnnounced` event, a client MUST apply the following checks before dialing:

```text
1. event.namespace == configured namespace
2. event.recordType is accepted
3. event.validUntil > local_chain_time
4. peerRecord length within configured limits
5. peer-record decoder succeeds
6. peer-record cryptographic signature succeeds
7. peer identity is internally consistent
8. record sequence is not older than the latest accepted record for that peer
9. endpoint policy accepts at least one endpoint
```

`local_chain_time` SHOULD be derived from the timestamp of the latest sufficiently confirmed registry-chain block rather than the machine wall clock when practical.

A registry transaction sender has no special trust. The Ethereum account that pays to publish a peer record MAY be unrelated to the P2P identity in the record.

---

## 11. Registry scanning

### 11.1 Required query

Clients query `PeerAnnounced(namespace, ...)` logs for the registry address.

A fresh client MUST NOT blindly scan from genesis.

### 11.2 Determining the recent block window

Because all valid records expire within `MAX_TTL`, a client only needs blocks whose timestamps may be newer than:

```text
cutoff = latest_finalized_timestamp - MAX_TTL
```

The client SHOULD find an approximate `startBlock` by binary searching block timestamps between `deploymentBlock` and the latest finalized/safe block.

The client then calls `eth_getLogs` in bounded chunks from `startBlock` to the latest sufficiently confirmed block.

Suggested chunk size:

```text
10,000 to 50,000 blocks
```

Implementations SHOULD reduce chunk size automatically when an RPC provider returns range/response-size errors.

### 11.3 Finality and reorgs

A client MAY dial a peer from an unfinalized announcement because dialing is not a consensus decision.

However, clients SHOULD distinguish:

- `observed`: event seen in the latest chain,
- `safe`: event included in a safe block,
- `finalized`: event included in a finalized block.

A reorg MUST NOT create application correctness issues because Resurrect announcements are merely hints. If an event disappears in a reorg, the peer may remain in the local peer cache if independently reachable.

### 11.4 Deduplication

After decoding records, clients MUST deduplicate by the peer identity defined by the record codec, not by publisher address or event transaction hash.

For the same peer identity, retain the highest valid peer-record sequence number. If sequence numbers are equal, implementations MAY retain the newest onchain announcement.

---

## 12. Node roles

Resurrect defines behavioral roles only; there is no onchain role registry.

### 12.1 Light node

- Uses Resurrect to find peers.
- Does not publish itself.
- May be unreachable from the public Internet.

### 12.2 Peer

- Participates in the application network.
- MAY publish a Resurrect announcement when publicly reachable.

### 12.3 Seed

- Intentionally acts as a bootstrap target.
- SHOULD have stable, publicly reachable endpoints.
- SHOULD maintain a non-expired Resurrect announcement.
- SHOULD accept inbound connections from unknown conforming peers subject to anti-abuse limits.

A node can dynamically change behavior without changing identity.

---

## 13. Bootstrap algorithm

A conforming node SHOULD use the following order.

```text
A. Local cache
B. Native discovery
C. Resurrect registry
D. Self-announcement if isolated and eligible
E. Retry Resurrect/native discovery
F. Switch to native discovery once sufficiently connected
```

Reference pseudocode:

```pseudo
async function bootstrap(descriptor):
    peers = validateAndRank(loadLocalPeerCache())
    if await connectEnough(peers):
        startNativeDiscovery()
        return CONNECTED

    peers = await nativeDiscovery(timeout=NATIVE_DISCOVERY_TIMEOUT)
    if await connectEnough(peers):
        return CONNECTED

    announcements = await scanResurrectRegistry(descriptor)
    peers = validateDeduplicateAndRank(announcements)
    if await connectEnough(peers):
        startNativeDiscovery()
        return CONNECTED

    if isEligibleSeed():
        await publishResurrectAnnouncement(mySignedPeerRecord(), REBOOT_TTL)

    while not shutdown:
        peers = await nativeDiscovery(timeout=RETRY_INTERVAL)
        if not peers:
            peers = await scanResurrectRegistry(descriptor)

        if await connectEnough(peers):
            startNativeDiscovery()
            return CONNECTED

        sleep(backoffWithJitter())
```

### 13.1 `connectEnough`

The threshold is application policy.

Recommended defaults:

```text
minimum successful peers: 2
preferred bootstrap peers: 4
maximum simultaneous Resurrect dials: 8
```

A network with only one living node necessarily operates with one peer until another joins.

### 13.2 Isolated node promotion

If a publicly reachable node:

1. has no connected application peers,
2. has failed local/native/Resurrect discovery for a configured interval, and
3. does not have a sufficiently recent own announcement,

it SHOULD publish itself with a short reboot TTL.

Recommended reboot TTL:

```text
7 days
```

Once the network forms, normal seed renewal policy may apply.

---

## 14. Native discovery integration

Resurrect SHOULD be used as a fallback alongside existing P2P discovery.

### 14.1 discv5

EIP-778 ENRs are directly compatible with Ethereum Node Discovery v5.

Modern discv5 specifications also define topic-based service discovery (TopDisc) for discovering application/service participants on the shared discovery substrate. Applications MAY use TopDisc as the normal discovery path and keep Resurrect as the cold-start fallback.

Reference: <https://github.com/ethereum/devp2p/blob/master/discv5/discv5.md>

### 14.2 libp2p

libp2p applications SHOULD feed validated Resurrect records into their normal peer store and then rely on their normal discovery stack, such as:

- Kademlia DHT,
- GossipSub peer exchange,
- identify,
- rendezvous,
- configured peers.

Resurrect does not replace these mechanisms.

### 14.3 DNS

Applications MAY additionally publish DNS bootstrap information, including EIP-1459 ENR trees. DNS is convenience infrastructure and MUST NOT be the only recovery mechanism when rebootability is a requirement.

Reference: <https://eips.ethereum.org/EIPS/eip-1459>

---

## 15. Network resurrection scenarios

### 15.1 Total extinction

Initial state:

```text
connected peers = 0
valid Resurrect announcements = 0
```

Node A starts:

```text
native discovery → none
Resurrect discovery    → none
A announces itself
```

State:

```text
connected peers = 0
valid Resurrect announcements = {A}
```

Node B starts:

```text
Resurrect → A
B dials A
```

State:

```text
A ↔ B
```

The network has rebooted.

### 15.2 Simultaneous reboot

If A and B both observe an empty network and both announce, each will observe the other's announcement on the next scan and attempt connection.

No leader election or serialization is required.

### 15.3 Partition healing

If components `{A,B}` and `{C,D}` are disconnected, publicly announced seeds in each partition can cause later registry scans to reveal the other component. The application transport decides how topology converges afterward.

---

## 16. Announcement renewal

Seeds SHOULD renew before expiry.

Recommended policy:

```text
announcement TTL: 30 days
renew after:      14-21 days
renew jitter:     ±24 hours
```

Renewal SHOULD include the peer's newest signed record/sequence.

A peer whose endpoints change SHOULD publish a new signed peer record immediately rather than waiting for renewal.

A peer MUST NOT rely on an Ethereum transaction replacing/removing an old announcement. Expiry plus peer-record sequence numbers handle supersession.

---

## 17. Endpoint policy

Resurrect itself does not decide whether addresses are useful. Implementations MUST apply local policy.

Clients SHOULD reject or deprioritize endpoints that are:

- malformed,
- unspecified (`0.0.0.0`, `::`),
- loopback unless explicitly in test mode,
- unroutable private ranges unless explicitly allowed,
- unsupported transports,
- known abusive addresses,
- duplicate endpoints.

Seeds SHOULD advertise only addresses they reasonably believe are externally reachable.

If the application profile supports browser/static clients, seed operators SHOULD advertise at least one endpoint usable from that environment when practical. A browser-compatible endpoint SHOULD use authenticated encryption suitable for browser security requirements; plaintext endpoints SHOULD NOT be relied upon for public browser bootstrap.

Applications MAY define multiple endpoint classes and preference ordering. For example, a browser client may prefer direct WebTransport, then secure WebSocket, then HTTPS request/response ingress, while a native daemon may prefer QUIC/TCP.

Implementations SHOULD use normal reachability techniques (AutoNAT, dial-back checks, observed-address mechanisms, etc.) before publishing themselves as seeds.

---

## 18. Security model

### 18.1 Malicious announcements

Anyone can pay Ethereum gas to publish garbage under any namespace.

Therefore:

- peer-record signatures are mandatory,
- decoding must be resource bounded,
- dial concurrency must be bounded,
- a valid signature does not imply service correctness,
- application handshakes must authenticate protocol/version independently.

### 18.2 Sybil attacks

An attacker can generate many valid peer identities and publish them.

Resurrect's baseline Sybil cost is the cost of onchain publication. This is not sufficient for all threat models.

Clients SHOULD mitigate eclipse/Sybil risk by:

- randomizing selection,
- limiting peers per IP/prefix/ASN where appropriate,
- mixing peers from different discovery sources,
- preferring peers with successful historical connections,
- retaining a diverse local peer cache,
- never interpreting registry order as ranking,
- periodically searching for additional peers after bootstrap.

Applications with stronger adversarial requirements MAY add their own authenticated membership, staking, proof-of-work, reputation, or capability checks **after** Resurrect discovery. Such mechanisms are outside Resurrect.

### 18.3 Registry spam

Clients MUST cap:

- maximum log records processed per scan,
- maximum records per peer identity,
- maximum dial attempts per time window,
- maximum parallel dials.

If the number of valid candidate records exceeds local limits, clients SHOULD sample candidates using a deterministic-but-unpredictable seed such as:

```text
keccak256(local_node_id || latest_finalized_block_hash || namespace)
```

and rotate the sample over time.

### 18.4 Eclipse resistance

Resurrect cannot guarantee eclipse resistance if the only reachable peers are attacker-controlled.

Applications SHOULD preserve and use independent native discovery mechanisms. Resurrect is intended as one source of signed candidate endpoints, not as an authoritative membership list.

### 18.5 Privacy

Publishing a seed record on Ethereum permanently reveals that the peer identity/endpoints were associated with a namespace at a particular time.

Nodes that require endpoint privacy SHOULD NOT announce and should operate as light nodes, using other peers for discovery.

### 18.6 Ethereum censorship/outage

If the registry chain is unavailable or censoring announcements, Resurrect cannot create a new root rendezvous. Existing connected networks can continue operating using their native P2P protocols.

This is an explicit Resurrect trust/liveness assumption.

### 18.7 Registry-provider privacy and integrity

The registry RPC/provider is not trusted for application correctness, but it can affect discovery availability and privacy.

A malicious or faulty provider can:

- omit valid `PeerAnnounced` logs,
- return a stale chain head,
- claim the wrong chain,
- fingerprint the client's IP and query timing.

Clients SHOULD permit provider replacement and MAY compare results from multiple independent providers. Security-sensitive deployments MAY use a local Ethereum node.

A browser client using an EIP-1193 provider SHOULD perform read-only Resurrect discovery before requesting account access, so peer discovery does not unnecessarily reveal wallet addresses to the application.

---

## 19. Resource limits

Suggested v1 client defaults:

```text
max registry record bytes:      4096
max decoded endpoints/record:   16
max active candidate peers:     256
max parallel bootstrap dials:   8
per-dial timeout:               8 seconds
native discovery timeout:       10 seconds
registry retry minimum:         30 seconds
registry retry maximum:         15 minutes
```

Clients MUST use exponential backoff with jitter when isolated and MUST NOT continuously poll Ethereum.

---

## 20. Optional HTTP introspection API

A reference Resurrect implementation MAY expose a local/admin HTTP API. This API is not part of the network protocol and MUST NOT be required for interoperability.

Suggested endpoints:

```text
GET /v1/resurrect/status
GET /v1/resurrect/peers
POST /v1/resurrect/announce
POST /v1/resurrect/rescan
```

`POST /announce` SHOULD require local authorization because it spends registry-chain gas.

---

## 21. Reference implementation architecture

### 21.1 Recommended reference stack

The reference implementation SHOULD use the following stack unless a concrete compatibility reason requires otherwise:

| Component | Recommended stack |
|---|---|
| Registry contract | Solidity + Foundry |
| Reference native node | Rust |
| Async runtime | Tokio |
| P2P host / peer identity / transports | `rust-libp2p` |
| Ethereum RPC + contract interaction | Alloy |
| Optional discv5 backend | Sigma Prime `discv5` crate |
| CLI | `clap` |
| Serialization / config | `serde` |
| Optional local peer/cache DB | SQLite |
| Browser/static registry client | TypeScript + viem or direct EIP-1193/JSON-RPC |
| Local EVM testing | Anvil + Forge |

The reference implementation SHOULD NOT make Node.js a requirement for operating a full Resurrect seed. A browser/TypeScript client is valuable, but the long-lived reference seed should be a native process with predictable networking behavior and resource limits.

For new Rust code, Alloy is RECOMMENDED over legacy `ethers-rs` APIs. Resurrect itself does not require EIP-7702, but using Alloy keeps the Ethereum integration current and gives dependent protocols one modern provider/type stack.

### 21.2 Repository layout

A recommended standalone Resurrect repository layout is:

```text
resurrect/
  SPEC.md

  contracts/
    foundry.toml
    src/
      ResurrectRegistry.sol
    test/
      ResurrectRegistry.t.sol
    script/

  crates/
    resurrect-core/
      src/
        descriptor.rs
        namespace.rs
        peer_record.rs
        validation.rs

    resurrect-ethereum/
      src/
        abi.rs
        provider.rs
        log_scanner.rs
        block_timestamp_search.rs
        publisher.rs

    resurrect-libp2p/
      src/
        signed_peer_record.rs
        dial.rs
        peer_exchange.rs

    resurrect-node/
      src/
        bootstrap.rs
        peers.rs
        native_discovery.rs
        config.rs
        cli.rs

    resurrect-discv5/              # optional / later phase
      src/
        enr.rs
        discovery.rs
        topdisc.rs

  packages/
    ts/                      # optional lightweight browser/static client

  test-vectors/
    descriptors/
    peer-records/
    registry-events/

  integration-tests/
    reboot/
    simultaneous-reboot/
```

Resurrect is intended to be consumed by unrelated application protocols as released libraries/packages. Dependent projects SHOULD depend on released `resurrect-*` crates/packages rather than copying the source or using a git submodule that makes the two projects operationally inseparable.

### 21.3 Suggested native modules

```text
resurrect-node
├── DescriptorLoader
├── RegistryClient
│   ├── AlloyProviderAdapter
│   ├── LogScanner
│   ├── BlockTimestampSearch
│   └── AnnouncementPublisher
├── PeerRecordCodecRegistry
│   ├── EnrCodec
│   └── Libp2pSignedPeerRecordCodec
├── CandidateStore
│   ├── Dedupe
│   ├── Scoring
│   └── DialScheduler
├── BootstrapStateMachine
│   ├── Backoff
│   └── SeedPromotion
└── NativeDiscoveryAdapters
    ├── Libp2pPeerExchange
    └── Discv5TopDisc          # optional
```

Recommended discovery abstraction:

```rust
#[async_trait::async_trait]
trait DiscoverySource: Send + Sync {
    async fn discover(
        &self,
        namespace: Namespace,
    ) -> anyhow::Result<Vec<PeerCandidate>>;
}
```

Recommended native adapter abstraction:

```rust
#[async_trait::async_trait]
trait NativeDiscovery: Send + Sync {
    async fn discover(&self) -> anyhow::Result<Vec<PeerCandidate>>;
    async fn add_verified_peer(&self, peer: PeerCandidate) -> anyhow::Result<()>;
}
```

Recommended record abstraction:

```rust
struct PeerCandidate {
    record_type: u32,
    peer_id: Vec<u8>,
    sequence: u64,
    endpoints: Vec<Endpoint>,
    raw_signed_record: Vec<u8>,
    expires_at: u64,
    source: DiscoverySourceKind,
}
```

The bootstrap state machine MUST depend on discovery interfaces rather than concrete implementations. This makes the registry, local cache, libp2p peer exchange, DNS, discv5, and future TopDisc support replaceable discovery sources.

### 21.4 Native discovery profile for v1

The first reference implementation SHOULD keep native discovery deliberately simple:

```text
local cache
    ↓
libp2p known peers / peer exchange
    ↓
Resurrect Ethereum registry
```

Support for ENR/discv5/TopDisc SHOULD be implemented behind the discovery abstraction and MAY ship after the minimal reboot behavior is proven. The Resurrect correctness and resurrection properties MUST NOT depend on TopDisc or any other still-evolving discovery extension.

Once connected, applications SHOULD use their native P2P discovery mechanism and SHOULD avoid treating Ethereum as a hot-path peer lookup service.

### 21.5 Local persistence

Any local database is cache only. SQLite is RECOMMENDED for the reference implementation because it is embedded and operationally simple.

A seed MAY persist:

```text
verified peer records
last successful dial times
local peer scoring/reputation
last scanned registry block
own announcement metadata
```

Deleting the database MUST NOT make the node unable to rejoin or reboot a network. A clean node started from only the Network Descriptor and an Ethereum provider MUST be able to reconstruct enough state to operate.

### 21.6 Contracts and build tooling

The registry reference contract SHOULD use Solidity and Foundry. The repository SHOULD use:

```text
forge build
forge test
anvil
```

for deterministic contract builds and local integration tests.

The contract test suite SHOULD include unit, fuzz, and invariant tests for TTL bounds, record-size bounds, permissionless publication, and the absence of any privileged mutation path.

### 21.7 Cross-language test vectors

Protocol objects that have canonical byte encodings SHOULD have checked-in deterministic test vectors. At minimum, where applicable, vectors SHOULD cover:

```text
network descriptor normalization
namespace derivation
signed peer record samples
record identity / sequence extraction
registry event ABI encoding
expiry filtering
```

If both Rust and TypeScript implementations exist, CI SHOULD require them to derive byte-identical identifiers and parse the same vectors consistently.

---

## 22. Bootstrap state machine

Reference states:

```text
START
  ↓
CACHE_DISCOVERY
  ↓ fail
NATIVE_DISCOVERY
  ↓ fail
RESURRECT_SCAN
  ├─ candidates → DIALING
  └─ none       → ISOLATED

ISOLATED
  ├─ seed eligible → ANNOUNCE_SELF
  └─ otherwise     → BACKOFF

ANNOUNCE_SELF
  ↓
BACKOFF
  ↓
NATIVE_DISCOVERY / RESURRECT_SCAN

DIALING
  ├─ enough peers → CONNECTED
  └─ fail         → BACKOFF

CONNECTED
  ↓
NATIVE_OPERATION
```

A reference implementation SHOULD make transitions observable through structured logs/metrics.

---

## 23. Conformance requirements

### 23.1 Resurrect Registry v1

A registry implementation is conforming if:

- it implements Section 8 semantics,
- it has no privileged control path,
- TTL and record-size bounds cannot be bypassed,
- announcements are emitted exactly with contract-derived `validUntil`.

### 23.2 Resurrect client v1

A client is conforming if it:

- accepts a valid network descriptor,
- can scan recent registry logs,
- filters expired announcements,
- cryptographically validates at least one configured peer-record type,
- deduplicates by peer identity/sequence,
- attempts to bootstrap through Resurrect after native discovery failure,
- can operate normally without continuing to use Resurrect once connected.

A seed-capable client SHOULD additionally implement self-announcement and renewal.

### 23.3 Static/immutable browser client profile

A browser/static implementation claiming rebootable Resurrect discovery conformance SHOULD additionally:

- accept a user-supplied Ethereum JSON-RPC endpoint,
- support an injected EIP-1193 provider when available,
- verify the registry chain ID before scanning,
- perform registry discovery without requesting account access,
- avoid requiring any protocol-controlled RPC hostname,
- filter discovered records to endpoints supported by its browser dial context,
- let the user replace a failing or untrusted provider.

---

## 24. Required tests

Implementers SHOULD create deterministic tests for at least the following.

### 24.1 Registry contract

- `ttl == 0` reverts.
- `ttl > MAX_TTL` reverts.
- empty record reverts.
- oversized record reverts.
- valid announcement emits expected `validUntil`.
- any address can announce.
- no owner/admin methods exist.

### 24.2 Peer records

- valid ENR accepted.
- invalid ENR signature rejected.
- ENR > 300 bytes rejected by ENR codec.
- newer sequence supersedes older sequence.
- older sequence received later does not supersede newer record.
- malformed libp2p envelope rejected.
- signed peer ID/key mismatch rejected.

### 24.3 Registry scanner

- ignores wrong namespace.
- ignores unsupported record type.
- ignores expired events.
- handles RPC pagination/chunk reduction.
- handles duplicate announcements.
- handles a chain reorg without corrupting peer state.

### 24.4 Reboot behavior

Integration test with local EVM + two nodes:

```text
1. Start with no peers and empty registry.
2. Start node A.
3. A fails native/Resurrect discovery.
4. A announces itself.
5. Start node B.
6. B discovers A through registry.
7. B connects to A.
8. Disable registry access.
9. Start node C with A/B learnable through native discovery.
10. C joins without registry access.
```

### 24.5 Simultaneous reboot

- A and B both start with empty peer set.
- both announce.
- both rescan.
- at least one successful connection forms.

### 24.6 Dead records

- publish unreachable peer records.
- client attempts them under bounded concurrency.
- client eventually continues discovery rather than hanging.

### 24.7 Spam

- create more valid signed announcements than candidate cap.
- client memory stays bounded.
- client selects bounded diverse subset.

### 24.8 Static/browser registry access

- custom JSON-RPC provider successfully scans the configured registry.
- provider with wrong `eth_chainId` is rejected.
- injected EIP-1193 provider can scan Resurrect without `eth_requestAccounts`.
- failure of a bundled/default RPC does not prevent switching to a custom RPC.
- custom RPC URL is not persisted unless explicitly requested.
- browser client ignores valid peer records that contain no endpoint supported by its dial-context profile.
- browser client can bootstrap when the only live seed is reachable through the application's browser-compatible signed endpoint format.

---

## 25. Suggested implementation phases

### Phase 1 — Minimal rebootable network

Implement:

- Solidity/Foundry immutable registry contract,
- Rust `resurrect-core`, `resurrect-ethereum`, and `resurrect-node` crates,
- Tokio runtime,
- one peer-record codec,
- Alloy-based registry scanner/publisher,
- self-announcement,
- `rust-libp2p` static/native peer adapter,
- optional SQLite cache,
- two-node reboot integration test on Anvil.

### Phase 2 — Production hardening

Add:

- second peer-record codec,
- block timestamp binary search,
- RPC fallback providers,
- reachability checks,
- peer diversity/scoring,
- renewal scheduler,
- reorg handling,
- metrics.

### Phase 3 — Native discovery optimization

Integrate:

- discv5 / TopDisc and/or
- libp2p DHT / peer exchange.

The production goal is that healthy nodes rarely need Resurrect after first bootstrap.

---

## 26. Open implementation choices

The following are intentionally not consensus-critical and MAY differ between implementations:

- peer ranking algorithm,
- dial timeout,
- minimum connected peer target,
- native discovery mechanisms,
- seed promotion delay,
- renewal interval below expiry,
- RPC provider strategy,
- local reputation database,
- endpoint transport preference.

Implementations MUST NOT introduce behavior that makes a specific registry publisher, peer, domain, API server, or original maintainer mandatory for network recovery.

---

## 27. Failure model summary

| Failure | Expected behavior |
|---|---|
| One bootstrap peer dies | Native discovery / remaining peers. |
| All configured bootstrap peers die | Resurrect scan discovers other announced seeds. |
| DNS/domain disappears | Resurrect unaffected. |
| Original team disappears | Anyone can operate and announce a compatible seed. |
| All application peers disappear | First new public node announces; second finds it. |
| Old announcements point to dead hosts | Dial failures are bounded; new announcements supersede via record sequence/expiry. |
| Registry receives spam | Clients validate, cap, sample, and diversify. |
| Ethereum registry unavailable | Existing network continues; zero-peer resurrection is unavailable until registry access returns. |
| All peers and all copies of application data disappear | Connectivity can reboot; lost application data cannot be reconstructed by Resurrect. |

---

## 28. References

- EIP-778 — Ethereum Node Records: <https://eips.ethereum.org/EIPS/eip-778>
- Ethereum Node Discovery v5 / TopDisc: <https://github.com/ethereum/devp2p/blob/master/discv5/discv5.md>
- EIP-1459 — Node Discovery via DNS: <https://eips.ethereum.org/EIPS/eip-1459>
- EIP-1193 — Ethereum Provider API: <https://eips.ethereum.org/EIPS/eip-1193>
- EIP-1474 — Ethereum JSON-RPC: <https://eips.ethereum.org/EIPS/eip-1474>
- EIP-6963 — Multi Injected Provider Discovery: <https://eips.ethereum.org/EIPS/eip-6963>
- libp2p HTTP transport: <https://github.com/libp2p/specs/blob/master/http/README.md>
- libp2p Signed Envelopes: <https://github.com/libp2p/specs/blob/master/RFC/0002-signed-envelopes.md>
- libp2p Routing Records: <https://github.com/libp2p/specs/blob/master/RFC/0003-routing-records.md>
- libp2p Rendezvous: <https://github.com/libp2p/specs/blob/master/rendezvous/README.md>
- libp2p GossipSub v1.1: <https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md>

---

## 29. Implementer checklist

A new implementation can be considered MVP-complete when another agent can demonstrate all of the following without operator-owned bootstrap infrastructure:

- [ ] Deploy an immutable Resurrect registry to a local EVM network.
- [ ] Generate a self-authenticating peer record.
- [ ] Publish that record under an arbitrary namespace.
- [ ] Scan only the recent TTL window and recover the record.
- [ ] Reject invalid/expired records.
- [ ] Dial the recovered endpoint.
- [ ] Start node A against an empty registry/network and have A self-announce.
- [ ] Start node B with no knowledge of A and have B discover A only through Resurrect.
- [ ] After A/B connect, have node C join through native P2P discovery without consulting the registry.
- [ ] Implement a caller-supplied registry provider abstraction.
- [ ] For browser/static clients, support custom JSON-RPC and injected EIP-1193 discovery without requesting account access.
- [ ] Verify registry `eth_chainId` before scanning and support switching providers.
- [ ] Define application dial-context rules for browser-capable signed endpoints where required.
- [ ] Kill all nodes, then repeat the reboot process with unrelated node operators.
- [ ] Demonstrate that no contract owner, DNS name, hosted API, or original operator is required.
