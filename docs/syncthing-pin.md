# Syncthing version pin

BlakSync wraps Syncthing. It does not fork it and does not reimplement the Block Exchange Protocol.

## Current pin

- **2.1.3**

`blaksync --version` prints this pin. `blaksync start` refuses a Syncthing binary whose major version is not 2.

## Where the binary comes from

1. `--syncthing PATH` or `BLAKSYNC_SYNCTHING`
2. `<config>/bin/syncthing` (installer layout; `%APPDATA%\blaksync\bin\syncthing.exe` on Windows)
3. A `syncthing` next to the `blaksync` executable
4. `syncthing` on `PATH` (not required if a bundled binary exists)

On Windows, `scripts/install-syncthing.ps1` installs the pin into the config `bin` folder after checking it against the release's `sha256sum.txt.asc`. That folder is user-writable, so Syncthing's own auto-upgrade keeps it on the newest release of the same major version; the pin is the install baseline, not a ceiling.

## How to bump

1. Change `PINNED_SYNCTHING` in `backend/version.rs` and `LONG_VERSION`.
2. Change `SYNCTHING_VERSION` in `scripts/install-syncthing.sh` and the default `-Version` in `scripts/install-syncthing.ps1`.
3. Update this file and `CHANGELOG.md`.
4. Run `scripts/install-syncthing.sh` (and, on Windows, `scripts/install-syncthing.ps1 -Destination DIR`) on a throwaway directory.
5. Run `scripts/e2e-two-device.sh` (device-id, add-folder, pair, health).
6. Confirm `blaksync start` still refuses a 1.x binary.

Keep CI on `runs-on: [self-hosted, linux]`.
