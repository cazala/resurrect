# Releases and package publishing

## Artifacts

Every artifact that can be independently consumed is published:

- crates.io: `resurrect-core`, `resurrect-libp2p`, `resurrect-ethereum`, `resurrect-node`;
- npm: `@resurrect-protocol/contracts`, `@resurrect-protocol/client`; and
- GitHub Release: `resurrect-node` binaries for Linux, macOS, and Windows, SHA-256 checksums, and attestations.

The canonical Solidity source, ABI, and machine-readable reference Ethereum deployment manifest are distributed by npm rather than a separate contract binary channel. Rust and TypeScript packages expose the same address/block as typed constants and constructors; packaging tests fail if those values drift.

## Development releases from main

After CI succeeds for a `main` commit, `publish.yml` checks out the exact tested commit and creates a unique prerelease version of the form:

```text
<workspace-base>-dev.<CI-run>.<attempt>
```

It publishes all Rust crates with that prerelease version and both npm packages under the `next` dist-tag. A rerun cannot collide with the first attempt. The workflow publishes dependency crates in topological order and retries while crates.io indexes propagate.

npm scans every accepted upload before making it installable. The release script therefore polls the public registry for up to 20 minutes after each npm upload and fails if the exact version never becomes available under the requested dist-tag. Upload acceptance alone is not treated as publication success. The same availability gate protects stable releases.

## Stable releases

Create and publish a GitHub Release whose tag is exactly `vMAJOR.MINOR.PATCH`. The workflow rejects other tag forms, checks out that tag, applies the exact tag version to every manifest/dependency, reruns Rust, Foundry, npm, packaging, and full implementer-checklist tests, then publishes crates and npm packages under `latest`. Native artifacts use the same version in their filenames.

Do not point a release tag at unreviewed or failing code. GitHub Release publication is the stable-release authorization event.

## Required GitHub configuration

Create a GitHub Environment named `package-publishing`; environment reviewers are recommended for stable releases. Configure:

- `CARGO_REGISTRY_TOKEN` as an environment or repository secret with permission to publish all four crates.
- npm trusted publishers for both scoped packages, restricted to this repository, workflow filename `publish.yml`, and environment `package-publishing`.

The publishing identities must own or be allowed to create the `resurrect-*` crate names and the `@resurrect-protocol` npm scope. Confirm those names before enabling the first main publication; if any name is already controlled by another party, rename all manifests and documentation coherently rather than publishing through an unrelated owner.

The workflow has `id-token: write` and requests npm provenance. With trusted publishing configured, `NPM_TOKEN` should be omitted. If either npm package does not yet exist and npm cannot attach a trusted publisher before first publication, perform the one-time initial publish with a granular `NPM_TOKEN`, then configure trusted publishing and remove the token.

Optional `MAINNET_RPC_URL` enables the real-state fork suite, including live verification of the published Ethereum mainnet contract. It is not required for local EVM, unit, integration, packaging, or release tests; without it the live assertions return early.

GitHub's built-in token supplies release upload and attestation permissions. The asset job passes `GITHUB_REPOSITORY` to `gh release upload` explicitly because it intentionally does not check out source or rely on local Git metadata. No contract deployer key, production RPC URL, libp2p identity, hosted API key, DNS credential, or Ethereum announcement key is needed by CI.

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

Registry publication is not transactional. The publishing script queries each registry for the exact version before every upload, skips versions that already exist, and rechecks after an upload command fails in case the registry accepted the artifact but the response was lost. It is therefore safe to rerun the same workflow attempt after a partial publication.

The recovery checks are covered by `scripts/publish-packages.test.sh` in CI. They deliberately simulate an artifact that existed before the run, an artifact that became visible after a failed publish command, delayed npm scan visibility, and a scan delay that exceeds the configured deadline.

Already-published immutable versions are never overwritten. If a stable version contains the wrong source, publish a new patch release; never move or recreate a stable tag with different source.
