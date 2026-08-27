# Changelog

User-facing notes in Australian English. Not a commit dump.

## [Unreleased]

- Point public URLs at yumaitau/BlakSync. The older jusso-dev clone is archive-only.
- Read the Syncthing API key from `config.xml` so operators do not export `BLAKSYNC_API_KEY`.
- Pin Syncthing 2.1.3 and refuse an unsupported major version at start.
- Installer script that places a pinned Syncthing next to BlakSync.
- First-run wizard for organisation, device name, and discovery.
- GUI roles: members cannot accept, share, or revoke. Owners assign roles in Settings.
- Pair with a QR or a six-character short code.
- Revoke a lost device from the GUI, with honest leftover-file copy.
- Windows office-node WinSW config and a reboot-safe update path.
- Office health page, discovery presets, pause-all, and bandwidth caps.
- Conflict paths, receive-only folders, and ignore patterns.
- Audit page and CSV export. Role changes are recorded.
- Optional local HTTPS for the GUI. Optional in-app version check against yumaitau/BlakSync only.
- Config backup and restore that refuse the git work tree.
- Start-at-login and a tray status icon (Windows and macOS). Linux user systemd unit.
- Printed pairing card, office SOP, and privacy notice.

## [0.1.0] — 2026-08-27

- First public crate: Rust wrapper, local GUI, org overlay, and access notes.
