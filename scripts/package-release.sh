#!/usr/bin/env bash
# Build release artefacts and SHA256SUMS. Signing secrets stay on the runner.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
dist="$root/dist-release"
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"
name="blaksync-${version}-${os}-${arch}"

rm -rf "$dist"
mkdir -p "$dist/$name"
(cd "$root" && cargo build --release --locked --bin blaksync)
if command -v npm >/dev/null; then
  (cd "$root" && npm ci && npm run build)
  if [[ -d "$root/dist" ]]; then
    cp -R "$root/dist" "$dist/$name/gui"
  fi
fi
cp "$root/target/release/blaksync" "$dist/$name/" 2>/dev/null || cp "$root/target/release/blaksync.exe" "$dist/$name/"
cp "$root/LICENSE" "$root/README.md" "$dist/$name/"
bash "$root/scripts/install-syncthing.sh" "$dist/$name"
(cd "$dist" && tar -czf "${name}.tar.gz" "$name")
(cd "$dist" && shasum -a 256 "${name}.tar.gz" > SHA256SUMS)
echo "Wrote $dist/${name}.tar.gz"
