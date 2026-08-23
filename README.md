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

## What never goes in git

These are device identity or live config. They stay on the device. `.gitignore` already lists them; CI still scans so a force-add does not sneak through.

| File | Why |
| --- | --- |
| `cert.pem` / `key.pem` | Device identity. Anyone with `key.pem` can impersonate the device and pull every folder it can see. |
| `https-cert.pem` / `https-key.pem` | GUI TLS material. |
| `config.xml` | Folder paths, paired device IDs, and the GUI **API key** (`<apikey>`). |
| `.env` and anything with API keys or tokens | Same class of secret. Use `.env.example` with empty values if you must document names. |

Do not commit a copy "just for the office node backup". Copy those files onto the office disk with the same permissions you would give the live process, not into this repository.

If one of those files lands in git: treat it as leaked, revoke or rotate ([docs/threat-model.md](docs/threat-model.md)), and scrub history with a maintainer. Deleting it in a follow-up commit is not enough.

## Threat model and a lost laptop

Syncthing has no central file store. The realistic failures are a pairing mistake, a leaked `key.pem`, discovery/relay metadata, and an unlocked office node or laptop.

Read [docs/threat-model.md](docs/threat-model.md) before treating a device as gone. Copy-paste revoke and rotate steps live there. Honest limits in short:

- **Metadata** — file bytes on the wire are encrypted; device IDs, IPs (via global discovery), and folder/file names among peers are not a secret from the services that make pairing work.
- **Online requirement** — if every copy is offline, nobody can pull. An office node is an always-on disk, not a cloud.
- **Unlocked disk** — there is no at-rest encryption from Syncthing. Full-disk encryption is the org's job.

## Secret scan (self-hosted)

Pull requests and pushes run Gitleaks on `runs-on: [self-hosted]`. The runner scans the tree locally. Nothing is uploaded to a SaaS secret scanner.

A dummy `key.pem` committed on a test branch must fail that job. Locally:

```bash
python3 scripts/secret-scan.py --source .
bash scripts/assert-dummy-key-fails.sh
```

## Licence

Apache-2.0, same as Syncthing.
