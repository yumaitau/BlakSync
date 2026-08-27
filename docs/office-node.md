# Office node

An office node is an ordinary Syncthing device with a clear role and an org-owned disk. It keeps a `sendreceive` copy of chosen folders, so laptops do not have to overlap online. Files remain in `/srv/blaksync/folders`; BlakSync does not use an object store or add another file format.

## Linux installation

Build the release binary on a trusted Rust 1.85+ build host, install Syncthing on the office node, then run as root once. Node.js is needed only if that build host also compiles the React GUI.

```sh
cargo build --release --locked
install -o root -g root -m 0755 target/release/blaksync /usr/local/bin/blaksync
useradd --system --home-dir /srv/blaksync --shell /usr/sbin/nologin blaksync
install -d -o blaksync -g blaksync -m 0700 /srv/blaksync/config /srv/blaksync/folders
install -o root -g root -m 0644 deploy/blaksync-office.service /etc/systemd/system/blaksync-office.service
systemctl daemon-reload
systemctl enable --now blaksync-office.service
```

The unit binds the management GUI to localhost, runs as the dedicated `blaksync` user, restarts after failures, and is enabled for reboot. BlakSync reads the GUI API key from `config.xml` after `start`. Never put an API key on a command line or in a world-readable file. Open `http://127.0.0.1:8385` for the BlakSync GUI or `http://127.0.0.1:8384` for the stock Syncthing fallback, then name this device **Office node**.

From this repository, add each accepted folder to the standard disk layout:

```sh
/usr/local/bin/blaksync office-folder --folder shared-work --label 'Shared work'
/usr/local/bin/blaksync share --folder shared-work --device LAPTOP_DEVICE_ID
/usr/local/bin/blaksync health
```

`office-folder` always creates a `sendreceive` Syncthing configuration at `/srv/blaksync/folders/<folder-id>`. Set `BLAKSYNC_ORG_ROOT` to another absolute, org-owned mount before running it if `/srv` is not on the intended disk. Create and ownership-adjust that root first. The `health` output contains only folder IDs, state, aggregate out-of-sync item counts, free bytes, device IDs, connection state, and last-seen times—never file names.

## Tailscale-only transport

To keep the sync port off public interfaces, give the office node a stable Tailscale IP and apply the **Tailscale only** preset in the BlakSync Settings page, or in Syncthing **Settings → Connections**:

1. Set **Sync Protocol Listen Addresses** to `tcp://TAILSCALE_IP:22000` (and optionally `quic://TAILSCALE_IP:22000`).
2. Disable Global Discovery, Relaying, and NAT traversal. Local Discovery may remain enabled only if LAN peers are wanted.
3. Set each laptop's address for the office node to `tcp://TAILSCALE_IP:22000`, or allow Tailscale DNS and use that stable name.
4. Restrict port 22000 with the host firewall and Tailscale ACLs. Keep the GUI at `127.0.0.1:8384`.

This is optional. Syncthing still provides authenticated, encrypted transport; Tailscale limits how the node is reachable.

## Windows office-node service

`deploy/blaksync-office.xml` is a WinSW configuration. Install [WinSW](https://github.com/winsw/winsw), place `blaksync.exe` under `C:\Program Files\BlakSync`, and keep config and folders on org-owned disk such as `D:\BlakSync`.

1. Create a dedicated local account `blaksync` with *Log on as a service*. Do not use a daily-driver login.
2. Grant that account and org administrators only on `D:\BlakSync` (config `0700` equivalent, no Users write).
3. `winsw install deploy\blaksync-office.xml` then start the service. Startup type is **Automatic**. Recovery restarts on failure.
4. The command line is `blaksync start --home D:\BlakSync\config`. The GUI stays on `127.0.0.1`. There is no API key on the command line.
5. After reboot, `blaksync health` (with `BLAKSYNC_CONFIG_DIR=D:\BlakSync\config` if needed) and the Health page should work. A field laptop can still sync.

## Reboot-safe binary update

1. Copy the new `blaksync` over `/usr/local/bin/blaksync` or `C:\Program Files\BlakSync\blaksync.exe`. Do not replace `config.xml`, `cert.pem`, or `key.pem`.
2. Restart the service: `systemctl restart blaksync-office` or the Windows Services console.
3. Confirm `blaksync health` and the Health page. Devices reconnect. Folder IDs and free-byte counts are visible; file names are not.

## Reboot validation

1. Share a folder from laptop A to the office node and laptop B; accept it on the office node with `office-folder` and on B normally.
2. Wait for `health` and both laptops to report `Up to Date`, then turn laptop A off.
3. Add a test file on B and confirm the office node receives it. Bring a third accepted laptop online and confirm it can pull the file while A remains off.
4. Reboot the office node. Confirm `systemctl is-active blaksync-office` reports `active`, devices reconnect, and `health` returns zero `outOfSyncItems` after convergence.

Do not log raw Syncthing database/file-detail endpoints during this check. The aggregate `health` command is safe for operational logs, provided device and folder IDs are acceptable in the org's logging policy.
