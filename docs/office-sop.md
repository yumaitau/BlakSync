# Office SOP

Two pages for a non-technical office. You do not need GitHub.

## Start

1. On Windows or macOS, turn on **Start at login** in Settings, or use the tray icon.
2. On Linux, enable the user service: `systemctl --user enable --now blaksync.service` (see `deploy/blaksync-user.service`).
3. Open http://127.0.0.1:8385. The first run asks for the organisation name, an `Australia/…` timezone, this device's name, and a discovery choice (LAN, defaults, or Tailscale).
4. You should not need a Syncthing API key. If the GUI cannot start, run `blaksync start` once so the config exists, then `blaksync gui`.

## Pair

Follow [docs/pairing-card.md](pairing-card.md). Pair both ways. A third machine that was never accepted cannot see folders.

## Share

On Folders, choose a device and click Share. Write an access note. The other machine accepts the folder and picks a local path. Receive-only folders do not send local extras back.

## Pause

Use **Pause all** on Folders before a satellite or phone link. Status says Paused, not only a colour change. Resume continues the copies. Optional KiB/s limits live on This device.

## Revoke (lost laptop)

1. On every remaining device, Remote devices → Revoke.
2. Files already on the lost disk stay there. This is not a remote wipe.
3. If this machine's own key leaked, follow the rotate steps on Settings: delete `cert.pem` and `key.pem` only, start again, pair the new ID.

## Backup

```sh
blaksync backup --out /media/org-usb/blaksync
```

Use org-owned disk or a USB drive. The tool refuses to write inside this git repository. Restore on a new machine with `blaksync restore --from …`. The device ID comes back with the certificate.

## Honest limits

- No remote wipe.
- No at-rest encryption from BlakSync. Full-disk encryption is the organisation's job.
- File bytes are not stored by Yuma. Discovery and relay services can see device IDs.

## Who to call

The contact on the organisation profile, or hello@yumait.com.au.
