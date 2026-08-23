# BlakSync

Peer-to-peer folder sync and sharing for Australian Indigenous organisations.
Devices talk to each other. There is no Dropbox, iCloud, or OneDrive in the middle. Built for Country, not the cloud.

Public repo: https://github.com/jusso-dev/BlakSync

## Why it exists

Louis Gleeson posted about [Syncthing](https://syncthing.net) as a $0 replacement for cloud drive subscriptions. That model is right: devices connect directly, transfers are TLS with perfect forward secrecy, each device has a certificate, nothing moves without permission.

BlakSync is that idea aimed at Indigenous orgs. Same engine (Syncthing, Apache-2.0), plus org tenancy, cultural access notes, and a quieter UI. We do not reimplement the Block Exchange Protocol in v1.

Source for the tweet: https://x.com/aigleeson/status/2091102436438188479
Upstream: https://github.com/syncthing/syncthing

## What v1 does

- Install on Windows, macOS, Linux, and Android. Pair devices with a device ID and an explicit share.
- Sync named folders (photos, work docs, heritage scans) continuously when devices are online.
- No account, no subscription, no copy of the files on a company server.
- Org profile, roles, and an access note on each shared folder.
- Local web GUI plus a CLI. Discovery via local LAN and optional global discovery the org can turn off.
- Self-hosted CI on `runs-on: [self-hosted]`.

## What v1 does not do

- Not a browser-only drop-a-file link (that was a wrong first cut).
- Not a public cloud bucket.
- No claim this replaces a legal cultural heritage register.

## Stack (locked for first cut)

- Syncthing as the sync engine (vendored or spawned, not forked unless we must)
- Thin wrapper for org config, access notes, and audit log
- Local web GUI (existing Syncthing GUI is the fallback; BlakSync UI is a later ticket)
- Apache-2.0 to match Syncthing

## Org overlay

Syncthing has no organisation concept. BlakSync adds a local overlay in one config directory: org profile, roles, a free-text access note on each folder, a pending-device prompt, and a local audit log. The organisation writes the note. BlakSync does not invent ceremony rules.

Each config directory is one organisation. Two config directories cannot see each other's folders. There is no social login. Government ID is not stored.

Roles:

- **owner** — set the org profile, assign roles, accept devices, share folders
- **admin** — accept devices, share folders, write access notes
- **member** — sync accepted folders; cannot accept a new device

Default config directory: `--config-dir` or `BLAKSYNC_CONFIG_DIR`, otherwise `~/.config/blaksync` on Linux.

```
python3 -m blaksync init \
  --name "Example Land Council" \
  --timezone Australia/Darwin \
  --contact it@example.org.au \
  --actor-id office-node \
  --actor-name "Office node"

python3 -m blaksync folder-add \
  --id heritage-scans \
  --label "Heritage scans" \
  --note "Speak with the cultural officer before pairing a new device."

python3 -m blaksync pending
python3 -m blaksync accept --device DEVICEID
python3 -m blaksync share --folder heritage-scans --device DEVICEID
python3 -m blaksync unshare --folder heritage-scans --device DEVICEID
python3 -m blaksync audit-export
```

Timezone must be an `Australia/*` IANA name. The pending prompt shows the org name, folder labels, who may pair, and the access note before Accept. Members who run `accept` still see the note, then the command fails.

The audit CSV columns are `timestamp,event,actor,role,device_id,folder_label`. Events are `device_accepted`, `folder_shared`, and `folder_unshared`. Paths and file contents are not recorded.

```
python3 -m unittest discover -s tests -v
```

## Licence

Apache-2.0, same as Syncthing.
