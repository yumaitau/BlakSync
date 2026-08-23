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

## Pair and share from the CLI

Requires Node.js 22 or newer, npm, and a running Syncthing instance. Install JavaScript dependencies with `npm ci`. BlakSync controls Syncthing through its local REST API; it does not proxy or store files. Create an API key under **Actions → Settings → GUI**, and keep the GUI bound to localhost. The key is read only from the environment and is never printed:

```sh
export BLAKSYNC_API_KEY='your-local-syncthing-api-key'
# Optional when the local GUI is not at the default address:
export BLAKSYNC_URL='http://127.0.0.1:8384'
npm run blaksync -- device-id
```

The reported ID is the certificate-derived Syncthing device ID. BlakSync does not create another identity. Device-to-device data uses Syncthing's authenticated TLS transport with perfect forward secrecy.

Pairing is mutual. On machine A, show the ID and add B; on machine B, show the ID and add A:

```sh
npm run blaksync -- device-id
npm run blaksync -- add-device --device DEVICE_ID_FROM_THE_OTHER_MACHINE --name 'Other machine'
```

`pending-devices` shows devices that have contacted this instance but have not been accepted. Adding a device does not share any folder. The default `dynamic` address uses both LAN discovery (which works without internet access) and, when enabled in Syncthing, global discovery. Syncthing's optional relay remains available when NAT prevents a direct connection; relays carry end-to-end encrypted traffic and cannot read folder contents.

Create and explicitly offer a folder on A:

```sh
mkdir -p "$PWD/sync-test"
npm run blaksync -- add-folder --folder sync-test --path "$PWD/sync-test" --label 'Sync test'
npm run blaksync -- share --folder sync-test --device DEVICE_ID_OF_B
```

Nothing is copied to B until B accepts the offered folder ID and chooses a local path:

```sh
mkdir -p "$PWD/sync-test"
npm run blaksync -- accept-folder --folder sync-test --path "$PWD/sync-test" --device DEVICE_ID_OF_A --label 'Sync test'
npm run blaksync -- status
```

Status is reported as `Preparing`, `Syncing`, `Up to Date`, `Unshared`, or `Paused`. Folder controls are:

```sh
npm run blaksync -- pause --folder sync-test
npm run blaksync -- resume --folder sync-test
npm run blaksync -- unshare --folder sync-test --device DEVICE_ID_OF_B
```

Unsharing removes that device from the folder and stops subsequent updates. It does not erase files already received.

## Manual two-device acceptance test

1. Start Syncthing and configure the environment variables above on two Linux machines (or Linux and Android with access to the Syncthing REST GUI endpoint). Confirm `device-id` returns the same ID shown by Syncthing.
2. Run `add-device` on both machines. Confirm a third, unaccepted Syncthing device is absent from `devices` and cannot list or request `sync-test`.
3. Add and share `sync-test` on A. Confirm B reports the offer but receives no files before `accept-folder` is run on B.
4. On A, create a 100 MiB test file without uploading it anywhere: `dd if=/dev/urandom of=sync-test/payload.bin bs=1M count=100 status=progress`.
5. Wait until `npm run blaksync -- status` says `Up to Date` on both machines. Run `sha256sum sync-test/payload.bin` on both and confirm the checksums match.
6. Run `unshare` on A, then create another file on A. Confirm it does not appear on B. Re-share, test `pause` and `resume`, and confirm updates stop while paused and continue after resume.

Automated REST wrapper tests use a local fake Syncthing HTTP instance and never publish folder contents or API keys:

```sh
npm test
```

## Licence

Apache License 2.0, same as Syncthing. See [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Never commit Syncthing API keys, device private keys, or `.env` files containing real secrets.
