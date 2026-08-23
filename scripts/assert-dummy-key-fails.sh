#!/usr/bin/env bash
# Commit a dummy key.pem in a throwaway git repo and require the scanner to fail.
# Used by CI so a broken detector cannot stay green. The dummy is built at
# runtime; it is never a real device key and is not left in this repository.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cargo run --locked --quiet --manifest-path "$ROOT/Cargo.toml" \
  --bin blaksync-secret-scan -- --self-test

if [[ -n "${GITLEAKS_BIN:-}" && -x "${GITLEAKS_BIN}" ]]; then
  work="$(mktemp -d "${TMPDIR:-/tmp}/blaksync-gitleaks-XXXXXX")"
  trap 'rm -rf "$work"' EXIT
  git init -q "$work"
  git -C "$work" config user.email "ci@blaksync.example"
  git -C "$work" config user.name "BlakSync CI"
  cargo run --locked --quiet --manifest-path "$ROOT/Cargo.toml" \
    --bin blaksync-secret-scan -- --write-dummy-key "$work/key.pem"
  git -C "$work" add -f key.pem
  git -C "$work" commit -qm "test: dummy key.pem"
  set +e
  "$GITLEAKS_BIN" detect --source "$work" --config "$ROOT/.gitleaks.toml" --redact --no-banner --verbose
  status=$?
  set -e
  if [[ "$status" -ne 1 ]]; then
    echo "FAIL: gitleaks exit $status, expected 1 (dummy key.pem detected)" >&2
    exit 1
  fi
  echo "ok: gitleaks rejected dummy key.pem (exit $status)"
fi
