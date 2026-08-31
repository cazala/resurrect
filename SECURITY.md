# Security policy

## Supported versions

Security fixes are applied to the latest stable release and the current `main` development line. This repository is currently pre-1.0; interfaces and security policy may evolve between minor releases. Development (`next`) packages are not recommended for production use.

## Reporting a vulnerability

Please use GitHub's private vulnerability-reporting feature for this repository. Include:

- affected commit, tag, crate/package, or contract source;
- impact and realistic threat scenario;
- minimal reproduction or failing test;
- whether keys, funds, peer identity, remote code execution, eclipse, denial of service, privacy, or supply-chain integrity are affected; and
- any suggested mitigation.

Do not open a public issue containing an unpatched exploit, secret, private RPC credential, or affected operator identity. If private reporting is unavailable, open a public issue requesting a private contact channel without exploit details.

Maintainers should acknowledge a complete report within seven days, provide status as investigation progresses, and coordinate disclosure after a fix or clear mitigation is available. Timelines may vary with severity and protocol/registry implications.

## Scope

In scope are the canonical Solidity source, Rust crates and native node, TypeScript package, peer-record validation, descriptor/provider binding, local cache, CI/release scripts, and checked-in interoperability vectors.

Expected protocol limitations—permissionless spam, Sybil identities, malicious but correctly signed peers, public endpoint metadata, inability to recover lost application data, and loss of resurrection while the configured EVM chain is unavailable—are documented risks rather than vulnerabilities unless an implementation flaw materially worsens them.

Never send live private keys, access tokens, or production credentials with a report. Use deterministic local keys and Anvil reproductions whenever possible.
