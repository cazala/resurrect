#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <semver>" >&2
  exit 2
fi

VERSION="$1"
if [[ ! "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid semantic version: ${VERSION}" >&2
  exit 2
fi

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPOSITORY_ROOT}"

perl -0pi -e "s/(\[workspace\.package\]\nversion = \x22)[^\x22]+/\${1}${VERSION}/" Cargo.toml
perl -0pi -e "s/(resurrect-(?:core|ethereum|libp2p|node) = \{ version = \x22)[^\x22]+/\${1}${VERSION}/g" crates/*/Cargo.toml

node - "${VERSION}" <<'NODE'
const fs = require('node:fs')
const version = process.argv[2]
for (const file of ['packages/ts/package.json', 'packages/contracts/package.json']) {
  const manifest = JSON.parse(fs.readFileSync(file, 'utf8'))
  manifest.version = version
  fs.writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`)
}
NODE

# Resolve the complete workspace so Cargo.lock records the new workspace package
# versions before every subsequent command runs with --locked.
cargo metadata --format-version 1 >/dev/null
for manifest in packages/ts/package.json packages/contracts/package.json; do
  test "$(node -p "require('./${manifest}').version")" = "${VERSION}"
done
