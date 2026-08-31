# Contributing

Contributions should preserve Resurrect's narrow purpose: permissionless root rendezvous without an owner, mandatory operator service, or application-specific semantics.

## Development setup

Install Rust 1.91+, Foundry 1.7.1, Node.js 22/24, pnpm 11.17, and `jq`. Then run:

```bash
corepack enable
pnpm install --frozen-lockfile
cargo build --workspace --locked
forge build --root contracts
```

## Change expectations

- Add regression tests for behavior changes and adversarial tests for parsers, providers, codecs, endpoint policy, or resource limits.
- Keep the descriptor strict and provider-neutral.
- Keep `ResurrectRegistryV1` stateless, immutable, and permissionless.
- Treat discovery input as hostile and preserve explicit bounds.
- Maintain Rust/TypeScript interoperability vectors when wire behavior changes.
- Update public docs and changelog for user-visible changes.
- Do not hand-edit only one copy of the canonical Solidity source; both copies must remain byte-identical.
- Do not commit generated build output, local databases, keys, RPC URLs, or tokens.

## Required checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings -W clippy::pedantic
cargo test --workspace --all-targets --locked
forge fmt --root contracts --check
FOUNDRY_PROFILE=ci forge test --root contracts
pnpm --recursive check
pnpm --recursive test
scripts/check-packages.sh
scripts/checklist-integration.sh
```

Run the optional fork suite with `MAINNET_RPC_URL` when a change depends on real EVM state. Never put the URL in source or logs.

## Protocol changes

A change to registry semantics, descriptor fields, namespace derivation, assigned codec numbers, signing domains, or validation requirements is a protocol change, not a routine implementation refactor. Update `docs/spec.md`, explain compatibility and migration, add cross-version tests, and choose a new Resurrect/application major version where interoperability would be unsafe.

## Commits and review

Keep commits focused by feature, test, documentation, or fix. Explain threat-model and liveness effects in the pull request. Reviewers should be able to map every normative behavior to code and tests. Releases are produced only by the documented CI workflow.

By contributing, you agree that contributions are licensed under the repository's applicable license: MIT or Apache-2.0 for Rust/TypeScript/tooling and CC0-1.0 for canonical registry contributions marked as such.
