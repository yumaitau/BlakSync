# BlakSync

Peer-to-peer file copy and sharing for Australian Indigenous organisations.
Encrypted transfers stay between devices. Files can also sit in an encrypted org store that the org controls. Built for Country, not the cloud.

Public repo: https://github.com/jusso-dev/BlakSync

## Why it exists

Most file tools upload everything to someone else's datacentre. That is a poor fit for land, heritage, and community records. BlakSync is a browser-first share tool in the same family as serverless WebRTC drop-and-link apps, plus an optional encrypted store that never leaves the org's machines unless they choose to send a copy.

Inspired by the drop-a-file, share-a-link pattern (see [this post](https://x.com/aigleeson/status/2091102436438188479)). BlakSync is not a clone of that product. It is a sovereign share and store tool for Indigenous orgs in Australia.

## What v1 does

- Drop a file or folder in the browser, get a link or QR, send it to another person or another org device.
- Bytes travel on an end-to-end encrypted WebRTC data channel. Signalling only carries handshake metadata.
- Optional encrypted store on a device the org owns (homelab, office NAS, or a small VM). Peers can pull or push against that store when the owner is online.
- Org tenancy, roles, and consent notes so cultural access rules can be recorded on a share.
- Works on poor links: pause, resume, retry, TURN fallback, and a clear "still waiting for the other device" state.

## What v1 does not do

- No public internet upload of file contents as the primary path.
- No analytics or third-party identity beyond what the org configures.
- No claim that this replaces legal cultural heritage systems. It is a share and store tool.

## Stack (locked for first cut)

- TypeScript, Vite, React
- WebRTC data channels, STUN plus optional TURN
- Signalling: a small self-hosted service the org runs (WebSocket). Nostr is allowed as a later option, not v1.
- Encrypted store: libsodium / age-style file encryption, keys held by the org
- CI on self-hosted runners (`runs-on: [self-hosted]`)

## Build and run

Requires Node.js 22 or newer and npm.

```bash
git clone https://github.com/jusso-dev/BlakSync.git
cd BlakSync
npm ci
npm run dev
```

Open the printed localhost URL (Vite defaults to http://localhost:5173). The home page is a drop zone stub. Placeholder routes:

- `/` — send / drop zone
- `/receive/:room` — receive stub for a room code
- `/library` — organisation library stub

Other scripts:

```bash
npm run lint
npm run typecheck
npm run build
npm run preview
```

`npm ci && npm run build` must succeed locally and in CI.

## Licence

Apache License 2.0. See [LICENSE](LICENSE).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Agents must not commit secrets, private keys, or `.env` files with real values.
