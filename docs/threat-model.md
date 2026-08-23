# BlakSync threat model

BlakSync is a thin wrapper around [Syncthing](https://syncthing.net). There is no central file store, no BlakSync cloud, and no company copy of org folders. Files live on devices the org already owns. Transfers use TLS with perfect forward secrecy. Nothing moves until both sides accept a device **and** a folder is shared.

This note is the honest model for that design: device keys, discovery, relays, a lost laptop, and a malicious pending pair. It is not an IRAP or ISO certification.

Upstream security principles: <https://docs.syncthing.net/users/security.html>

## Honest limits

Three things this tool does **not** buy you:

1. **Metadata is visible.** File contents on the wire are encrypted. Folder names, file names, sizes, device IDs, and (if global discovery or public relays are on) approximate IPs are not a secret from the people and services that make pairing work. A relay operator cannot read files; they can see that two device IDs talked.
2. **Devices must be online to sync.** There is no Dropbox-in-the-middle. If the only copy of a folder is on a laptop that is off, nobody else can pull it. An always-on office node is the usual fix; it is also a high-value disk.
3. **Unlocked disk is plaintext.** Syncthing does not encrypt files at rest. `key.pem`, `config.xml`, and the folder bytes are whatever the filesystem holds. Full-disk encryption (or at least encrypting the config directory **and** the data) is the org's job. An unlocked office node, or a stolen laptop that was not locked, is a full compromise of that device's folders and identity.

## Assets

| Asset | What it is | If it leaks |
| --- | --- | --- |
| Shared folders | Photos, work docs, heritage scans on disk | Direct access to file contents on that device |
| `key.pem` + `cert.pem` | Device identity (Device ID is the SHA-256 of the cert) | Attacker impersonates the device, connects to already-paired peers, pulls every folder that device is allowed to see |
| `config.xml` | Folders, paired devices, listen addresses, **GUI API key** (`<apikey>`) | Control of the local GUI/REST API; map of what is shared with whom |
| `https-key.pem` | GUI TLS private key | Impersonate the local GUI if it is exposed beyond localhost |
| Device ID | Public identifier, safe to read out loud when pairing | With global discovery: lookup of current IP. Not enough to join a cluster |
| Office node | Always-on copy of chosen folders | Same as a lost laptop, but for every folder that node holds |

Device IDs are **not** secrets. `key.pem` is.

## Attackers

- **Opportunistic network** — café Wi-Fi, office LAN, a hostile relay. Can observe that Syncthing is in use; cannot read file bytes on the wire.
- **Discovery / relay operator** — default global discovery is run by the Syncthing project; **anyone** can run a public relay. They see device IDs (and discovery sees IP + port). They do not see file contents.
- **Someone with a device ID** — can *attempt* a connection (pending pair). They get files only if an operator accepts them **and** shares a folder.
- **Thief of an unlocked or unencrypted device** — has the folders and the keys on that disk. This is the realistic worst case.
- **Insider / mis-pair** — staff member accepts the wrong pending device, or shares a heritage folder with a personal laptop that later leaves the org.

## What is actually protected

- Device-to-device sync is TLS 1.2/1.3. Relays wrap the same TLS session: **relay ciphertext is not readable by the relay**, and perfect forward secrecy means a later theft of `key.pem` does not decrypt old captures.
- Uninvited devices cannot join. Both sides must add the other's Device ID. A third device that was never accepted cannot see the folder.
- Local GUI defaults to `127.0.0.1` only. Do not bind it to `0.0.0.0` on an office node unless you have another gate (Tailscale, firewall, GUI password).

## Discovery metadata

**Local discovery** (default on): every 30 seconds the device broadcasts Device ID + port on the LAN. Anyone on that LAN can see which machines run BlakSync. Turn it off only if you will set static addresses; otherwise office laptops will not find each other without the internet.

**Global discovery** (default on, org can turn off): the device announces Device ID + external port to discovery servers over TLS. The operator can map Device ID → IP and infer which devices look for each other. An internet eavesdropper can tell that a host runs Syncthing with global discovery. Disable this when devices only need LAN, or when they already have a private path (Tailscale, VPN, static addresses).

Knowing a Device ID plus global discovery **can expose an IP**. It still does not grant folder access.

## Relay ciphertext

Relaying defaults to on so two devices behind NAT can still sync. Public relays are run by volunteers. The chosen relay learns both Device IDs. **File data stays inside the TLS session** and is not inspectable by the relay.

Prefer a direct or Tailscale path for the office node. If the GUI connection type starts with `Relay`, you are on a volunteer relay. You can disable relays, or allow only a relay the org runs.

## Lost device

A lost or stolen laptop is not a remote-wipe situation. Revoke stops **future** sync. Bytes already on that disk stay there.

Do this immediately on **every remaining device**, office node first:

1. Remove the lost Device ID (that unshares every folder with it).
2. Ignore it if it tries to come back as a pending pair.
3. Treat every folder that device held as exposed **if the disk was unlocked or unencrypted**.
4. Rotate **this** device's identity only if its own `key.pem` leaked (see below). Other devices keep their IDs.

If the lost machine is the **office node**, assume every folder that node held is in the thief's hands. Staff laptops still have their copies. Stand up a new office node, pair it, re-share. Do not reuse the old `cert.pem` / `key.pem`.

Put the Syncthing/BlakSync config directory on the same encrypted volume as the data. Whole-disk encryption is the simple version of that.

## Malicious pending pair

Someone who has a Device ID (screenshot, global discovery, a port scan of `:22000`) can knock. The GUI shows a pending device. **Files do not move.**

Rules:

- Accept only Device IDs you compared out of band (call, Signal, a piece of paper in the office).
- If you did not invite it, click **Ignore**. Do not add it "to see who it is".
- Sharing a folder is a second step. A paired device with no folders still cannot pull data.
- Introducer mode spreads pairing automatically. Leave it off unless you understand that a compromised introducer can introduce further devices.

## Revoke a device (copy-paste)

Replace `LOST_ID` with the Syncthing Device ID of the lost or untrusted machine. Run this on **each remaining device** while BlakSync/Syncthing is running. Config dir is `~/.config/blaksync` if you used the BlakSync home; otherwise Syncthing's default (`~/.local/state/syncthing` on current Linux, `~/.config/syncthing` on older installs, `%LOCALAPPDATA%\Syncthing` on Windows).

### GUI

1. Open the local GUI: `http://127.0.0.1:8384`
2. Remote Devices → the lost device → **Edit** → **Remove** → confirm
3. If a "New Device" banner appears for that ID, click **Ignore**
4. Repeat on every remaining device, including the office node

### REST (Linux)

```bash
export CONFIG_DIR="${BLAKSYNC_HOME:-$HOME/.config/blaksync}"
export GUI="${BLAKSYNC_GUI:-http://127.0.0.1:8384}"
export LOST_ID='REPLACE-WITH-LOST-DEVICE-ID'
export API_KEY="$(sed -n 's:.*<apikey>\(.*\)</apikey>.*:\1:p' "$CONFIG_DIR/config.xml" | head -1)"

# Remove the device and unshare every folder with it
curl -sfS -X DELETE -H "X-API-Key: $API_KEY" \
  "$GUI/rest/config/devices/$LOST_ID"

# Drop a pending knock from the same ID (ignore it in the GUI if it returns)
curl -sfS -X DELETE -H "X-API-Key: $API_KEY" \
  "$GUI/rest/cluster/pending/devices?device=$LOST_ID"

echo "Revoked $LOST_ID on $(hostname)"
```

### CLI (if `syncthing` is on PATH)

```bash
export CONFIG_DIR="${BLAKSYNC_HOME:-$HOME/.config/blaksync}"
export LOST_ID='REPLACE-WITH-LOST-DEVICE-ID'

syncthing cli --home "$CONFIG_DIR" config devices "$LOST_ID" delete
```

Confirm the ID is gone: Remote Devices no longer lists it, and folder status is not "Syncing" with that name.

## Rotate identity

Do this when **this** device's `key.pem` leaked, or when you rebuild a compromised office node. It issues a **new Device ID**. Every peer must revoke the old ID and pair the new one.

```bash
export CONFIG_DIR="${BLAKSYNC_HOME:-$HOME/.config/blaksync}"

# 1. Stop BlakSync / Syncthing (GUI → Actions → Shutdown, or your systemd unit)
# 2. Destroy this device's identity only — do not delete config.xml or folder data
rm -f "$CONFIG_DIR/cert.pem" "$CONFIG_DIR/key.pem"

# 3. Start BlakSync / Syncthing again. It writes a new cert and key.
# 4. Print the new Device ID
syncthing --home "$CONFIG_DIR" device-id
```

Then, on every **other** device: revoke the old ID (commands above), add the new ID, re-share each folder.

If the GUI API key on this device was exposed (lost unlocked laptop with the GUI reachable, or `config.xml` committed by mistake):

```bash
export CONFIG_DIR="${BLAKSYNC_HOME:-$HOME/.config/blaksync}"
# Stop BlakSync / Syncthing first
# Replace the value inside <apikey>...</apikey> with a fresh 32+ character random string
# Example generator (prints a key; paste it into config.xml — do not commit it):
openssl rand -base64 32
# Start again. Update any scripts that stored the old API key.
```

A leaked API key without a leaked `key.pem` does not impersonate the device to peers; it does control the local REST API.

## What CI is for

CI on this repo runs Gitleaks **on the self-hosted runner**. The tree is not uploaded to a SaaS secret scanner. Every run also commits a dummy `key.pem` in a throwaway git repo and requires that scan to fail, so a broken detector cannot stay green. Real `cert.pem`, `key.pem`, and `config.xml` with API keys must never be committed — see the README.

To replay the dummy-key failure on a real branch:

```bash
git checkout -b ci/dummy-key-should-fail
cargo run --locked --bin blaksync-secret-scan -- --write-dummy-key key.pem
git add -f key.pem
git commit -m "test: dummy key.pem must fail secret scan"
git push -u origin HEAD
```

That push must go red on the Secret scan workflow. Do not merge it.
