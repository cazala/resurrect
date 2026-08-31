# Resurrect v1 conformance evidence

This map connects every item in section 29 of the specification to implementation and CI evidence. The single `implementer-checklist` job runs `scripts/checklist-integration.sh`, which first runs all language/contract prerequisites and then exercises the runnable system on Anvil.

| Implementer checklist item | Implementation | CI evidence |
|---|---|---|
| Deploy immutable registry locally | `contracts/src/ResurrectRegistryV1.sol` | fresh `forge create`; selector surface and contract suites |
| Generate self-authenticating record | `resurrect-libp2p::sign_peer_record` | cross-language vectors and live seed announcement |
| Publish under arbitrary namespace | Alloy publisher and `RegistryAnnouncer` | fresh `keccak` namespaces for reboot scenarios |
| Scan only recent TTL window | `RegistryScanner` timestamp binary search | scanner unit tests plus live recovery |
| Reject invalid/expired records | core policy and both codecs | Rust/TypeScript rejection suites invoked by checklist |
| Dial recovered endpoint | `Libp2pHost` connector | B reports `connectedVia=RESURRECT_SCAN` after Noise connection |
| A self-announces from empty state | bootstrap controller and announcer | empty Anvil/network A scenario |
| B discovers A only through Resurrect | scanner, codec, connector | B has separate empty identity/cache and mDNS disabled |
| C joins natively without registry | configured native libp2p adapter | C uses A's peer-ID-qualified multiaddr, an unreachable RPC, and reports zero scan/failure counters |
| Caller-supplied provider | Rust trait and TypeScript interface | provider doubles plus CLI/user URL scenarios |
| Browser custom JSON-RPC and injected provider without accounts | `jsonRpcProvider` and `injectedProvider` | full scan tests assert no `eth_requestAccounts` |
| Chain verification and provider switching | descriptor/provider checks and `setProvider` | wrong-chain and failed-provider replacement tests |
| Browser dial-context rules | secure signed multiaddr policy | shared browser/native-only vectors and filtering tests |
| Unrelated operators reboot after shutdown | no owner plus independent peer/payer identities | all A/B/C stop; D/E reboot; identities differ |
| No owner, DNS, hosted API, or original operator | minimal contract and caller-owned configuration | selector proof; loopback IP multiaddrs; fresh arbitrary keys |

Additional required-test evidence from section 24:

| Area | Evidence |
|---|---|
| Registry bounds and statelessness | example, fuzz, and invariant Foundry suites |
| ENR validity, invalid signature, maximum size, sequences | `resurrect-libp2p` unit tests |
| libp2p malformed/signature/identity failures | codec unit and interoperability tests |
| scanner namespace, type, expiry, chunking, duplicate, reorg | `resurrect-ethereum` deterministic provider suite |
| simultaneous reboot | live F/G scenario under a fresh namespace |
| dead records and continuing discovery | stale A/B records precede unrelated D/E live records; bounded connector tests |
| spam/candidate cap | core candidate-store and scanner cap tests |
| browser provider/privacy/endpoints | TypeScript provider, scanner, and record suites |

The output artifact contains one boolean for every checklist claim plus the temporary registry address. A false claim or missing artifact fails CI. This is reference-implementation evidence, not a third-party audit or proof about an external deployment.
