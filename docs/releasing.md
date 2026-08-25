# Releases and package publishing

## Artifacts

Every artifact that can be independently consumed is published:

- crates.io: `rbp-core`, `rbp-libp2p`, `rbp-ethereum`, `rbp-node`;
- npm: `@rbp-protocol/contracts`, `@rbp-protocol/client`; and
- GitHub Release: `rbp-node` binaries for Linux, macOS, and Windows, SHA-256 checksums, and attestations.

The canonical Solidity source and ABI are distributed by npm rather than a separate contract binary channel.

## Development releases from main

After CI succeeds for a `main` commit, `publish.yml` checks out the exact tested commit and creates a unique prerelease version of the form:

```text
<workspace-base>-dev.<CI-run>.<attempt>
```

It publishes all Rust crates with that prerelease version and both npm packages under the `next` dist-tag. A rerun cannot collide with the first attempt. The workflow publishes dependency crates in topological order and retries while crates.io indexes propagate.

## Stable releases

Create and publish a GitHub Release whose tag is exactly `vMAJOR.MINOR.PATCH`. The workflow rejects other tag forms, checks out that tag, applies the exact tag version to every manifest/dependency, reruns Rust, Foundry, npm, packaging, and full implementer-checklist tests, then publishes crates and npm packages under `latest`. Native artifacts use the same version in their filenames.

Do not point a release tag at unreviewed or failing code. GitHub Release publication is the stable-release authorization event.

## Required GitHub configuration

Create a GitHub Environment named `package-publishing`; environment reviewers are recommended for stable releases. Configure:

- `CARGO_REGISTRY_TOKEN` as an environment or repository secret with permission to publish all four crates.
- npm trusted publishers for both scoped packages, restricted to this repository, workflow file `.github/workflows/publish.yml`, and environment `package-publishing`.

The publishing identities must own or be allowed to create the `rbp-*` crate names and the `@rbp-protocol` npm scope. Confirm those names before enabling the first main publication; if any name is already controlled by another party, rename all manifests and documentation coherently rather than publishing through an unrelated owner.

The workflow has `id-token: write` and requests npm provenance. With trusted publishing configured, `NPM_TOKEN` should be omitted. If either npm package does not yet exist and npm cannot attach a trusted publisher before first publication, perform the one-time initial publish with a granular `NPM_TOKEN`, then configure trusted publishing and remove the token.

Optional `MAINNET_RPC_URL` enables the real-state fork suite. It is not required for local EVM, unit, integration, packaging, or release tests; without it the fork test returns early.

GitHub's built-in token supplies release upload and attestation permissions. No contract deployer key, production RPC URL, libp2p identity, hosted API key, DNS credential, or Ethereum announcement key is needed by CI.

Every third-party GitHub Action is pinned to a verified commit SHA. Update those pins deliberately after reviewing upstream release notes and resolving the corresponding signed major-version tag.

## Preflight

Before merging or publishing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings -W clippy::pedantic
cargo test --workspace --all-targets --locked
forge test --root contracts
pnpm --recursive check
pnpm --recursive test
scripts/check-packages.sh
scripts/checklist-integration.sh
```

Review packed archives and confirm the contract source mirror is exact. Confirm the base workspace version represents the next intended development line before merging version changes.

## Versioning mechanics

`scripts/set-version.sh` updates the workspace version, all internal Rust dependency requirements, and both npm manifests. `scripts/publish-packages.sh` runs checks, builds packages, publishes crates in dependency order, and finally publishes npm packages with the requested `next` or `latest` tag.

Publication mutates manifests only in the ephemeral CI checkout. Release versions need not be committed merely for the workflow to publish them, although maintainers should keep the base development version meaningful in source.

## Recovery from partial publication

Registry publication is not transactional. If a network failure leaves only some artifacts published, rerun the same workflow attempt only after inspecting registry state. Already-published immutable versions cannot be overwritten; use the next unique development run/attempt or a new patch release. Never retag a stable release to different source.
