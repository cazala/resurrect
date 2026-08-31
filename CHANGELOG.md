# Changelog

All notable changes are documented here. The project follows Semantic Versioning for released artifacts; pre-1.0 minor versions may include intentional interface changes described in release notes.

## Unreleased

### Added

- Immutable, stateless, permissionless `ResurrectRegistryV1` with Foundry example, fuzz, invariant, and optional fork suites.
- Strict Resurrect v1 descriptor and namespace primitives, signed announcement validation, sequence deduplication, deterministic bounded candidate selection, and caller-owned interfaces.
- Alloy registry constant checks, provider abstraction, finality-aware recent-window scanning, block timestamp binary search, adaptive log ranges, reorg checkpoints, and signed publication.
- EIP-778 ENR and standard libp2p Signed Envelope codecs with native/browser endpoint policies and cross-language descriptor, event, and peer-record vectors.
- Native Tokio/rust-libp2p node with Noise/Yamux, configured-peer/mDNS/identify discovery, disposable revalidated SQLite cache, self-promotion, renewal, retry supervision, telemetry, and CLI.
- Browser/static TypeScript client supporting custom JSON-RPC and injected EIP-1193 discovery without wallet account exposure.
- End-to-end Anvil implementer-checklist test covering total extinction, Resurrect-only recovery, registry-independent native joining, unrelated-operator reboot, and simultaneous reboot.
- Automated next/latest publication for every Rust crate, both npm packages, and versioned native release binaries with checksums and attestations.
- Architecture, integration, operations, browser, security, testing, conformance, deployment, and release documentation.
