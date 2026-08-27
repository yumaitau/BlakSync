#!/usr/bin/env bash
# Install the pinned Syncthing next to BlakSync or into a config bin dir.
set -euo pipefail

SYNCTHING_VERSION="${SYNCTHING_VERSION:-2.1.3}"
DEST="${1:-}"

if [[ -z "$DEST" ]]; then
  echo "usage: $0 DEST_DIR" >&2
  exit 2
fi

mkdir -p "$DEST"
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) os_name=linux ;;
  Darwin) os_name=macos ;;
  MINGW*|MSYS*|CYGWIN*) os_name=windows ;;
  *) echo "unsupported OS: $os" >&2; exit 1 ;;
esac
case "$arch" in
  x86_64|amd64) arch_name=amd64 ;;
  arm64|aarch64) arch_name=arm64 ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

asset="syncthing-${os_name}-${arch_name}-v${SYNCTHING_VERSION}"
url="https://github.com/syncthing/syncthing/releases/download/v${SYNCTHING_VERSION}/${asset}.tar.gz"
if [[ "$os_name" == "windows" ]]; then
  url="https://github.com/syncthing/syncthing/releases/download/v${SYNCTHING_VERSION}/${asset}.zip"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
archive="$tmp/syncthing.tgz"
curl -sSfL -o "$archive" "$url"
if [[ "$os_name" == "windows" ]]; then
  unzip -q "$archive" -d "$tmp"
else
  tar -xzf "$archive" -C "$tmp"
fi

# The release tarball also ships text helpers named `syncthing` (for example
# the UFW application profile). Prefer the documented binary path.
if [[ "$os_name" == "windows" ]]; then
  binary="$tmp/${asset}/syncthing.exe"
else
  binary="$tmp/${asset}/syncthing"
fi
if [[ ! -f "$binary" ]]; then
  binary="$(find "$tmp" -type f \( -name syncthing -o -name syncthing.exe \) ! -path '*/etc/*' | head -n 1)"
fi
if [[ -z "${binary:-}" || ! -f "$binary" ]]; then
  echo "extracted archive did not contain syncthing" >&2
  exit 1
fi
install -m 0755 "$binary" "$DEST/$(basename "$binary")"
"$DEST/$(basename "$binary")" --version | grep -F "$SYNCTHING_VERSION" >/dev/null
echo "Installed Syncthing ${SYNCTHING_VERSION} to $DEST"
