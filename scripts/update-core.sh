#!/bin/sh
# Points Cargo.toml at another core release (both git dependencies) and updates Cargo.lock.
#   scripts/update-core.sh 1.2.0
set -eu
version=${1:?usage: scripts/update-core.sh <version>}
cd "$(dirname "$0")/.."
git ls-remote --exit-code --tags https://github.com/openvireo/openvireo-core "v$version" >/dev/null \
  || { echo "no tag v$version in openvireo-core" >&2; exit 1; }
sed -i.bak -E "s|(git = \"https://github.com/openvireo/openvireo-core\", tag = )\"v[^\"]*\"|\1\"v$version\"|" Cargo.toml
rm Cargo.toml.bak
cargo update -p ov-core -p ov-share
cargo build
echo "OpenVireo core $version"
