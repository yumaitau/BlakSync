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

## Licence

Apache-2.0, same as Syncthing.
