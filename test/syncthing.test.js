import assert from "node:assert/strict";
import { after, before, beforeEach, describe, test } from "node:test";
import { createServer } from "node:http";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { SyncthingClient } from "../lib/syncthing.js";
import { AccessNotesStore } from "../lib/access-notes.js";
import { createGuiServer } from "../lib/gui-server.js";

const OWN_ID = "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH";
const PEER_ID = "IIIIIII-JJJJJJJ-KKKKKKK-LLLLLLL-MMMMMMM-NNNNNNN-OOOOOOO-PPPPPPP";
let server;
let baseUrl;
let requests;
let folders;
let devices;
let pendingDevices;
let deniedDevices;
let configDir;

before(async () => {
  server = createServer(async (request, response) => {
    const body = await readBody(request);
    requests.push({ method: request.method, url: request.url, headers: request.headers, body });
    const url = new URL(request.url, "http://localhost");
    if (request.headers["x-api-key"] !== "test-key") return send(response, 403, "forbidden");
    if (url.pathname === "/rest/system/status") return send(response, 200, { myID: OWN_ID, version: "v1.27.0", uptime: 120 });
    if (url.pathname === "/rest/system/connections") {
      return send(response, 200, {
        connections: { [PEER_ID]: { connected: true, address: "tcp://192.168.1.20:22000" } },
        total: { inBytesTotal: 2048, outBytesTotal: 4096 },
      });
    }
    if (url.pathname === "/rest/config/gui") return send(response, 200, { address: "127.0.0.1:8384" });
    if (url.pathname === "/rest/config/options") return send(response, 200, {});
    if (url.pathname === "/rest/config/defaults/device") return send(response, 200, { addresses: ["dynamic"], paused: false });
    if (url.pathname === "/rest/config/defaults/folder") return send(response, 200, { devices: [{ deviceID: OWN_ID }], type: "sendreceive" });
    if (url.pathname === "/rest/config/devices" && request.method === "GET") return send(response, 200, devices);
    if (url.pathname === "/rest/config/devices") {
      devices.push(body);
      pendingDevices = Object.fromEntries(Object.entries(pendingDevices).filter(([id]) => id !== body.deviceID));
      return send(response, 200, {});
    }
    if (url.pathname === "/rest/cluster/pending/devices" && request.method === "DELETE") {
      const device = url.searchParams.get("device");
      deniedDevices.push(device);
      delete pendingDevices[device];
      return send(response, 200, {});
    }
    if (url.pathname === "/rest/cluster/pending/devices") return send(response, 200, pendingDevices);
    if (url.pathname === "/rest/cluster/pending/folders") return send(response, 200, {});
    if (url.pathname === "/rest/db/completion") return send(response, 200, { completion: 87.5 });
    if (url.pathname === "/rest/config/folders" && request.method === "GET") return send(response, 200, folders);
    if (url.pathname === "/rest/config/folders") { folders.push(body); return send(response, 200, {}); }
    if (url.pathname.startsWith("/rest/config/folders/")) {
      const id = decodeURIComponent(url.pathname.split("/").at(-1));
      const folder = folders.find((item) => item.id === id);
      if (request.method === "GET") return send(response, 200, folder);
      Object.assign(folder, body);
      return send(response, 200, {});
    }
    if (url.pathname === "/rest/db/status") {
      return send(response, 200, {
        state: "idle",
        needBytes: 0,
        needFiles: 0,
        needTotalItems: 0,
        globalBytes: 1048576,
        localBytes: 1048576,
      });
    }
    return send(response, 404, "not found");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  baseUrl = `http://127.0.0.1:${server.address().port}`;
});

after(async () => {
  await new Promise((resolve) => server.close(resolve));
  if (configDir) await rm(configDir, { recursive: true, force: true });
});

beforeEach(async () => {
  requests = [];
  folders = [];
  devices = [{ deviceID: OWN_ID, name: "Office node" }];
  pendingDevices = { [PEER_ID]: { name: "Field laptop", address: "tcp://192.168.1.20:22000" } };
  deniedDevices = [];
  if (configDir) await rm(configDir, { recursive: true, force: true });
  configDir = await mkdtemp(path.join(tmpdir(), "blaksync-notes-"));
});

describe("Syncthing REST wrapper", () => {
  test("uses Syncthing's device ID and accepts a peer with dynamic discovery", async () => {
    const client = makeClient();
    assert.equal(await client.getDeviceId(), OWN_ID);
    const device = await client.addDevice(PEER_ID, "Peer laptop");
    assert.equal(device.deviceID, PEER_ID);
    assert.deepEqual(device.addresses, ["dynamic"]);
    assert.equal(requests.at(-1).body.deviceID, PEER_ID);
  });

  test("requires an explicit folder share and can unshare", async () => {
    const client = makeClient();
    await client.addFolder("country-docs", "/tmp/country-docs", "Country docs");
    assert.deepEqual((await client.listFolderStatus())[0].devices, []);
    assert.equal((await client.listFolderStatus())[0].status, "Unshared");
    await client.shareFolder("country-docs", PEER_ID);
    assert.deepEqual(folders[0].devices.map((item) => item.deviceID), [OWN_ID, PEER_ID]);
    assert.equal((await client.listFolderStatus())[0].status, "Up to Date");
    await client.unshareFolder("country-docs", PEER_ID);
    assert.deepEqual(folders[0].devices.map((item) => item.deviceID), [OWN_ID]);
  });

  test("pauses and resumes one folder", async () => {
    const client = makeClient();
    await client.addFolder("country-docs", "/tmp/country-docs");
    await client.setFolderPaused("country-docs", true);
    assert.equal(folders[0].paused, true);
    await client.setFolderPaused("country-docs", false);
    assert.equal(folders[0].paused, false);
  });

  test("reports remote device connection and completion", async () => {
    const client = makeClient();
    devices.push({ deviceID: PEER_ID, name: "Field laptop", paused: false });
    const remotes = await client.listRemoteDeviceStatus();
    assert.equal(remotes.length, 1);
    assert.equal(remotes[0].connected, true);
    assert.equal(remotes[0].completion, 87.5);
  });

  test("denies a pending device", async () => {
    const client = makeClient();
    await client.denyPendingDevice(PEER_ID);
    assert.deepEqual(deniedDevices, [PEER_ID]);
    assert.equal(Object.keys(pendingDevices).length, 0);
  });

  test("does not include the API key in an error", async () => {
    const client = new SyncthingClient({ baseUrl, apiKey: "wrong-secret" });
    await assert.rejects(client.getDeviceId(), (error) => !error.message.includes("wrong-secret"));
  });
});

describe("Access notes store", () => {
  test("stores and clears a folder access note", async () => {
    const store = new AccessNotesStore(configDir);
    assert.equal(await store.get("heritage"), "");
    await store.set("heritage", "Speak with the cultural officer first.");
    assert.equal(await store.get("heritage"), "Speak with the cultural officer first.");
    await store.set("heritage", "");
    assert.equal(await store.get("heritage"), "");
  });
});

describe("BlakSync GUI API", () => {
  test("shows access notes on pending accept and can pair then share", async () => {
    const notes = new AccessNotesStore(configDir);
    await notes.set("heritage-scans", "Speak with the cultural officer before pairing a new device.");
    const gui = createGuiServer({
      host: "127.0.0.1",
      port: 0,
      configDir,
      staticRoot: path.join(configDir, "empty-static"),
      clientFactory: () => makeClient(),
    });
    await new Promise((resolve, reject) => {
      gui.server.listen(0, "127.0.0.1", resolve);
      gui.server.once("error", reject);
    });
    const port = gui.server.address().port;
    const root = `http://127.0.0.1:${port}`;

    try {
      const overview = await jsonFetch(`${root}/api/overview`);
      assert.equal(overview.pending.devices.length, 1);
      assert.equal(overview.pending.devices[0].accessNotes[0].note, "Speak with the cultural officer before pairing a new device.");

      await jsonFetch(`${root}/api/pending/devices/${PEER_ID}/accept`, {
        method: "POST",
        body: { name: "Field laptop" },
      });
      assert.ok(devices.some((device) => device.deviceID === PEER_ID));

      await jsonFetch(`${root}/api/folders`, {
        method: "POST",
        body: {
          id: "heritage-scans",
          path: "/tmp/heritage-scans",
          label: "Heritage scans",
          accessNote: "Speak with the cultural officer before pairing a new device.",
        },
      });
      await jsonFetch(`${root}/api/folders/heritage-scans/share`, {
        method: "POST",
        body: { deviceId: PEER_ID },
      });

      const foldersView = await jsonFetch(`${root}/api/folders`);
      assert.equal(foldersView[0].accessNote, "Speak with the cultural officer before pairing a new device.");
      assert.equal(foldersView[0].sharedWith[0].deviceId, PEER_ID);

      await jsonFetch(`${root}/api/folders/heritage-scans/pause`, { method: "POST" });
      assert.equal(folders[0].paused, true);
      await jsonFetch(`${root}/api/folders/heritage-scans/unshare`, {
        method: "POST",
        body: { deviceId: PEER_ID },
      });
      assert.equal(folders[0].devices.some((device) => device.deviceID === PEER_ID), false);
    } finally {
      await gui.close();
    }
  });

  test("binds the listen address to loopback by default", async () => {
    const gui = createGuiServer({
      configDir,
      staticRoot: path.join(configDir, "empty-static"),
      clientFactory: () => makeClient(),
    });
    assert.equal(gui.host, "127.0.0.1");
    assert.equal(gui.port, 8385);
  });
});

function makeClient() { return new SyncthingClient({ baseUrl, apiKey: "test-key" }); }
function send(response, status, value) {
  const body = typeof value === "string" ? value : JSON.stringify(value);
  response.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(body) });
  response.end(body);
}
async function readBody(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  const text = Buffer.concat(chunks).toString();
  return text ? JSON.parse(text) : undefined;
}
async function jsonFetch(url, { method = "GET", body } = {}) {
  const response = await fetch(url, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await response.json();
  assert.ok(response.ok, data.error || `HTTP ${response.status}`);
  return data;
}
