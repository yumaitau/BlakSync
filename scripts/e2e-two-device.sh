#!/usr/bin/env bash
# Two isolated Syncthing homes on one self-hosted runner.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${BLAKSYNC_BIN:-$root/target/release/blaksync}"
syncthing_bin="${BLAKSYNC_SYNCTHING:-}"
workdir="${TMPDIR:-/tmp}/blaksync-e2e-$$"
mkdir -p "$workdir"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$workdir"' EXIT

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

"$bin" start --syncthing "$syncthing_bin" --home "$home_a" &
pid_a=$!
"$bin" start --syncthing "$syncthing_bin" --home "$home_b" &
pid_b=$!
sleep 4

export BLAKSYNC_URL=http://127.0.0.1:8384
# Isolated homes on one host cannot share 8384. Use generate+serve ports via env if needed.
# The launcher pins 8384, so run B on a second loopback by rewriting after generate.
# For CI we start sequentially on different homes only if we can change the GUI port.
# Fall back: use syncthing directly for B on 8386 while A uses the wrapper.

kill "$pid_a" "$pid_b" 2>/dev/null || true
wait "$pid_a" "$pid_b" 2>/dev/null || true

"$syncthing_bin" generate --home="$home_a" --no-port-probing
"$syncthing_bin" generate --home="$home_b" --no-port-probing
"$syncthing_bin" serve --home="$home_a" --gui-address=127.0.0.1:18384 --no-browser --no-port-probing &
"$syncthing_bin" serve --home="$home_b" --gui-address=127.0.0.1:18386 --no-browser --no-port-probing &
sleep 3

id_a="$("$syncthing_bin" device-id --home="$home_a")"
id_b="$("$syncthing_bin" device-id --home="$home_b")"

key_a="$(sed -n 's:.*<apikey>\(.*\)</apikey>.*:\1:p' "$home_a/config.xml" | head -n 1)"
key_b="$(sed -n 's:.*<apikey>\(.*\)</apikey>.*:\1:p' "$home_b/config.xml" | head -n 1)"

BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" add-device --device "$id_b" --name peer-b
BLAKSYNC_URL=http://127.0.0.1:18386 BLAKSYNC_API_KEY="$key_b" BLAKSYNC_CONFIG_DIR="$home_b" \
  "$bin" add-device --device "$id_a" --name peer-a

printf 'hello-blaksync\n' > "$folder_a/payload.txt"
sum_a="$(sha256sum "$folder_a/payload.txt" | awk '{print $1}')"

BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" add-folder --folder e2e --path "$folder_a" --label e2e --note 'e2e'
BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" share --folder e2e --device "$id_b"
BLAKSYNC_URL=http://127.0.0.1:18386 BLAKSYNC_API_KEY="$key_b" BLAKSYNC_CONFIG_DIR="$home_b" \
  "$bin" accept-folder --folder e2e --path "$folder_b" --device "$id_a" --label e2e

for _ in $(seq 1 40); do
  if [[ -f "$folder_b/payload.txt" ]]; then
    break
  fi
  sleep 1
done
sum_b="$(sha256sum "$folder_b/payload.txt" | awk '{print $1}')"
if [[ "$sum_a" != "$sum_b" ]]; then
  echo "checksum mismatch" >&2
  exit 1
fi

BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" pause --folder e2e
printf 'paused\n' > "$folder_a/paused.txt"
sleep 3
if [[ -f "$folder_b/paused.txt" ]]; then
  echo "pause still copied a file" >&2
  exit 1
fi
BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" resume --folder e2e
BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" unshare --folder e2e --device "$id_b"
printf 'after-unshare\n' > "$folder_a/after.txt"
sleep 3
if [[ -f "$folder_b/after.txt" ]]; then
  echo "unshare still copied a new file" >&2
  exit 1
fi

BLAKSYNC_URL=http://127.0.0.1:18384 BLAKSYNC_API_KEY="$key_a" BLAKSYNC_CONFIG_DIR="$home_a" \
  "$bin" health >/dev/null
echo "two-device e2e passed"
