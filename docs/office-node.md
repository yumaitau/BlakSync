# Office node

An office node is an ordinary Syncthing device with a clear role and an org-owned disk. It keeps a `sendreceive` copy of chosen folders, so laptops do not have to overlap online. Files remain in `/srv/blaksync/folders`; BlakSync does not use an object store or add another file format.

## Linux installation

Install Syncthing and Node.js 22 or newer, then run as root once:

```sh
useradd --system --home-dir /srv/blaksync --shell /usr/sbin/nologin blaksync
install -d -o blaksync -g blaksync -m 0700 /srv/blaksync/config /srv/blaksync/folders
install -o root -g root -m 0644 deploy/blaksync-office.service /etc/systemd/system/blaksync-office.service
systemctl daemon-reload
systemctl enable --now blaksync-office.service
```

The unit binds the management GUI to localhost, runs as the dedicated `blaksync` user, restarts after failures, and is enabled for reboot. Open `http://127.0.0.1:8384` through a local browser or SSH tunnel, set a GUI password and API key, then name the Syncthing device **Office node**. Never put the API key on a command line or in a world-readable file.

From this repository, add each accepted folder to the standard disk layout:

```sh
export BLAKSYNC_API_KEY='local-api-key'
npm run blaksync -- office-folder --folder shared-work --label 'Shared work'
npm run blaksync -- share --folder shared-work --device LAPTOP_DEVICE_ID
npm run blaksync -- health
```

`office-folder` always creates a `sendreceive` Syncthing configuration at `/srv/blaksync/folders/<folder-id>`. Set `BLAKSYNC_ORG_ROOT` to another absolute, org-owned mount before running it if `/srv` is not on the intended disk. Create and ownership-adjust that root first. The `health` output contains only folder IDs, state, aggregate out-of-sync item counts, free bytes, device IDs, connection state, and last-seen times—never file names.

## Tailscale-only transport

To keep the sync port off public interfaces, give the office node a stable Tailscale IP and in Syncthing **Settings → Connections**:

1. Set **Sync Protocol Listen Addresses** to `tcp://TAILSCALE_IP:22000` (and optionally `quic://TAILSCALE_IP:22000`).
2. Disable Global Discovery, Enable Relaying, and NAT traversal. Local Discovery may remain enabled only if LAN peers are wanted.
3. Set each laptop's address for the office node to `tcp://TAILSCALE_IP:22000`, or allow Tailscale DNS and use that stable name.
4. Restrict port 22000 with the host firewall and Tailscale ACLs. Keep the GUI at `127.0.0.1:8384`.

This is optional. Syncthing still provides authenticated, encrypted transport; Tailscale limits how the node is reachable.

## Windows service note

Use the official Syncthing Windows package or a service wrapper such as WinSW. Run it under a dedicated local service account with a command equivalent to:

```text
syncthing.exe serve --no-browser --no-restart --home=D:\BlakSync\config --gui-address=127.0.0.1:8384
```

Set the service startup type to **Automatic**, recovery to restart on failure, and grant only that account and org administrators access to `D:\BlakSync`. Set `BLAKSYNC_ORG_ROOT=D:\BlakSync` when using the CLI. Do not store the API key in the service command line.

## Reboot validation

1. Share a folder from laptop A to the office node and laptop B; accept it on the office node with `office-folder` and on B normally.
2. Wait for `health` and both laptops to report `Up to Date`, then turn laptop A off.
3. Add a test file on B and confirm the office node receives it. Bring a third accepted laptop online and confirm it can pull the file while A remains off.
4. Reboot the office node. Confirm `systemctl is-active blaksync-office` reports `active`, devices reconnect, and `health` returns zero `outOfSyncItems` after convergence.

Do not log raw Syncthing database/file-detail endpoints during this check. The aggregate `health` command is safe for operational logs, provided device and folder IDs are acceptable in the org's logging policy.
