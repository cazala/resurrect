# Changelog

All notable changes are documented here. The project follows Semantic Versioning for released artifacts; pre-1.0 minor versions may include intentional interface changes described in release notes.

## Unreleased

No unreleased changes.

## 0.4.0 - 2026-09-03

### Added

- Native libp2p-over-WebSocket listeners alongside the existing TCP transport,
  with Noise/Yamux authentication and Rust transport tests.
- A static browser explorer with editable or injected read-only Ethereum
  providers, bounded discovery metrics, authenticated WSS dialing, peer-ID
  verification, identify, and ping.
- A real Rust-to-JavaScript WebSocket interoperability suite included in the
  implementer-checklist integration test.
- Cloudflare Pages deployment automation for the tested `main` commit and
  production topology/runbook documentation.

### Fixed

- JSON-RPC range errors are decoded before HTTP status handling so providers
  such as dRPC can trigger the scanner's adaptive block-range reduction.

## 0.3.0 - 2026-09-02

### Added

- `--application` and `--major-version` namespace-preimage support in the native
  node, alongside the existing precomputed `--namespace` option.
- Release and CI coverage for namespace derivation and canonical deployment
  defaults.

## 0.2.0 - 2026-09-01

### Changed

- Package publication waits for npm registry scan visibility and safely resumes
  after partial registry publication.
- Release asset uploads explicitly target the canonical repository.

## 0.1.0 - 2026-09-01

### Added

- Immutable, stateless, permissionless `ResurrectRegistryV1` with Foundry
  example, fuzz, invariant, and optional fork suites.
- Rust core, Ethereum, libp2p, and native-node crates; browser/static TypeScript
  client; canonical contracts package; and cross-language vectors.
- End-to-end Anvil implementer-checklist coverage for empty-network promotion,
  Resurrect-only discovery, native joining, total shutdown, unrelated-operator
  reboot, and simultaneous reboot.
- Automated `next` and `latest` package publication plus native release binaries,
  checksums, attestations, and comprehensive protocol documentation.
