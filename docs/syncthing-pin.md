# Syncthing version pin

BlakSync wraps Syncthing. It does not fork it and does not reimplement the Block Exchange Protocol.

## Current pin

- **2.1.3**

`blaksync --version` prints this pin. `blaksync start` refuses a Syncthing binary whose major version is not 2.

## Where the binary comes from

1. `--syncthing PATH` or `BLAKSYNC_SYNCTHING`
2. `<config>/bin/syncthing` (installer layout)
3. A `syncthing` next to the `blaksync` executable
4. `syncthing` on `PATH` (not required if a bundled binary exists)

## How to bump

1. Change `PINNED_SYNCTHING` in `backend/version.rs` and `LONG_VERSION`.
2. Change `SYNCTHING_VERSION` in `scripts/install-syncthing.sh`.
3. Update this file and `CHANGELOG.md`.
4. Run `scripts/install-syncthing.sh` on a throwaway directory.
5. Run `scripts/e2e-two-device.sh` (device-id, add-folder, pair, health).
6. Confirm `blaksync start` still refuses a 1.x binary.

Keep CI on `runs-on: [self-hosted, linux]`.
