#!/usr/bin/env bash
# Fail if Cargo.toml and package.json versions drift.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
npm_version="$(sed -n 's/^  "version": "\([^"]*\)".*/\1/p' "$root/package.json" | head -n 1)"

if [[ -z "$cargo_version" || -z "$npm_version" ]]; then
  echo "could not read versions" >&2
  exit 1
fi
if [[ "$cargo_version" != "$npm_version" ]]; then
  echo "version drift: Cargo.toml=$cargo_version package.json=$npm_version" >&2
  exit 1
fi
echo "versions match: $cargo_version"
