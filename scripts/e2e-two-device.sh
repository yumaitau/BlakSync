#!/usr/bin/env bash
# Two isolated Syncthing homes on one self-hosted runner.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${BLAKSYNC_BIN:-$root/target/release/blaksync}"
syncthing_bin="${BLAKSYNC_SYNCTHING:-}"
workdir="${TMPDIR:-/tmp}/blaksync-e2e-$$"
mkdir -p "$workdir"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$workdir"' EXIT

checksum() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

wait_api() {
  local url="$1" key="$2"
  local _
  for _ in $(seq 1 50); do
    if curl -sf -H "X-API-Key: $key" "$url/rest/system/status" >/dev/null; then
      return 0
    fi
    sleep 0.2
  done
  echo "timed out waiting for $url" >&2
  return 1
}

patch_json() {
  local url="$1" key="$2" path="$3" body="$4"
  curl -sf -X PATCH \
    -H "X-API-Key: $key" \
    -H "Content-Type: application/json" \
    -d "$body" \
    "$url$path" >/dev/null
}

if [[ ! -x "$bin" ]]; then
  (cd "$root" && cargo build --release --locked --bin blaksync)
  bin="$root/target/release/blaksync"
fi

if [[ -z "$syncthing_bin" ]]; then
  bash "$root/scripts/install-syncthing.sh" "$workdir/bin"
  syncthing_bin="$workdir/bin/syncthing"
fi

home_a="$workdir/a"
home_b="$workdir/b"
folder_a="$workdir/share-a"
folder_b="$workdir/share-b"
mkdir -p "$home_a" "$home_b" "$folder_a" "$folder_b"

"$syncthing_bin" generate --home="$home_a" --no-port-probing
"$syncthing_bin" generate --home="$home_b" --no-port-probing

url_a="http://127.0.0.1:18384"
url_b="http://127.0.0.1:18386"
listen_a="tcp://127.0.0.1:22011"
listen_b="tcp://127.0.0.1:22012"

"$syncthing_bin" serve --home="$home_a" --gui-address=127.0.0.1:18384 --no-browser --no-port-probing &
"$syncthing_bin" serve --home="$home_b" --gui-address=127.0.0.1:18386 --no-browser --no-port-probing &

key_a="$(sed -n 's:.*<apikey>\(.*\)</apikey>.*:\1:p' "$home_a/config.xml" | head -n 1)"
key_b="$(sed -n 's:.*<apikey>\(.*\)</apikey>.*:\1:p' "$home_b/config.xml" | head -n 1)"
wait_api "$url_a" "$key_a"
wait_api "$url_b" "$key_b"

# Keep both instances off the default 22000 listener so they can run together.
patch_json "$url_a" "$key_a" "/rest/config/options" \
  "{\"listenAddresses\":[\"$listen_a\"],\"globalAnnounceEnabled\":false,\"relaysEnabled\":false,\"natEnabled\":false,\"localAnnounceEnabled\":false}"
patch_json "$url_b" "$key_b" "/rest/config/options" \
  "{\"listenAddresses\":[\"$listen_b\"],\"globalAnnounceEnabled\":false,\"relaysEnabled\":false,\"natEnabled\":false,\"localAnnounceEnabled\":false}"

id_a="$("$syncthing_bin" device-id --home="$home_a")"
id_b="$("$syncthing_bin" device-id --home="$home_b")"

BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" add-device --device "$id_b" --name peer-b >/dev/null
BLAKSYNC_URL="$url_b" BLAKSYNC_API_KEY="$key_b" BLAKSYNC_CONFIG_DIR="$home_b" \
  "$bin" add-device --device "$id_a" --name peer-a >/dev/null

patch_json "$url_a" "$key_a" "/rest/config/devices/$id_b" "{\"addresses\":[\"$listen_b\"]}"
patch_json "$url_b" "$key_b" "/rest/config/devices/$id_a" "{\"addresses\":[\"$listen_a\"]}"

printf 'hello-blaksync\n' > "$folder_a/payload.txt"
sum_a="$(checksum "$folder_a/payload.txt")"

BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" add-folder --folder e2e --path "$folder_a" --label e2e --note 'e2e' >/dev/null
BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" share --folder e2e --device "$id_b" >/dev/null
BLAKSYNC_URL="$url_b" BLAKSYNC_API_KEY="$key_b" BLAKSYNC_CONFIG_DIR="$home_b" \
  "$bin" accept-folder --folder e2e --path "$folder_b" --device "$id_a" --label e2e >/dev/null

for _ in $(seq 1 60); do
  if [[ -f "$folder_b/payload.txt" ]]; then
    break
  fi
  sleep 1
done
sum_b="$(checksum "$folder_b/payload.txt")"
if [[ "$sum_a" != "$sum_b" ]]; then
  echo "checksum mismatch" >&2
  exit 1
fi

BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" pause --folder e2e >/dev/null
printf 'paused\n' > "$folder_a/paused.txt"
sleep 3
if [[ -f "$folder_b/paused.txt" ]]; then
  echo "pause still copied a file" >&2
  exit 1
fi
BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" resume --folder e2e >/dev/null
BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" unshare --folder e2e --device "$id_b" >/dev/null
printf 'after-unshare\n' > "$folder_a/after.txt"
sleep 3
if [[ -f "$folder_b/after.txt" ]]; then
  echo "unshare still copied a new file" >&2
  exit 1
fi

BLAKSYNC_URL="$url_a" BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" health >/dev/null
echo "two-device e2e passed"
