import assert from "node:assert/strict";
import { after, before, beforeEach, describe, test } from "node:test";
import { createServer } from "node:http";
import { SyncthingClient, officeFolderPath } from "../lib/syncthing.js";

const OWN_ID = "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH";
const PEER_ID = "IIIIIII-JJJJJJJ-KKKKKKK-LLLLLLL-MMMMMMM-NNNNNNN-OOOOOOO-PPPPPPP";
let server;
let baseUrl;
let requests;
let folders;
let needTotalItems;

before(async () => {
  server = createServer(async (request, response) => {
    const body = await readBody(request);
    requests.push({ method: request.method, url: request.url, headers: request.headers, body });
    const url = new URL(request.url, "http://localhost");
    if (request.headers["x-api-key"] !== "test-key") return send(response, 403, "forbidden");
    if (url.pathname === "/rest/system/status") return send(response, 200, { myID: OWN_ID });
    if (url.pathname === "/rest/stats/device") return send(response, 200, { [PEER_ID]: { lastSeen: "2026-08-23T03:00:00Z" } });
    if (url.pathname === "/rest/system/connections") return send(response, 200, { connections: { [PEER_ID]: { connected: true } } });
    if (url.pathname === "/rest/config/defaults/device") return send(response, 200, { addresses: ["dynamic"], paused: false });
    if (url.pathname === "/rest/config/defaults/folder") return send(response, 200, { devices: [{ deviceID: OWN_ID }], type: "sendreceive" });
    if (url.pathname === "/rest/config/devices" && request.method === "GET") return send(response, 200, []);
    if (url.pathname === "/rest/config/devices") return send(response, 200, {});
    if (url.pathname === "/rest/cluster/pending/devices") return send(response, 200, { [PEER_ID]: { name: "Peer" } });
    if (url.pathname === "/rest/config/folders" && request.method === "GET") return send(response, 200, folders);
    if (url.pathname === "/rest/config/folders") { folders.push(body); return send(response, 200, {}); }
    if (url.pathname.startsWith("/rest/config/folders/")) {
      const id = decodeURIComponent(url.pathname.split("/").at(-1));
      const folder = folders.find((item) => item.id === id);
      if (request.method === "GET") return send(response, 200, folder);
      Object.assign(folder, body);
      return send(response, 200, {});
    }
    if (url.pathname === "/rest/db/status") return send(response, 200, { state: "idle", needBytes: 0, needFiles: 0, needTotalItems });
    return send(response, 404, "not found");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  baseUrl = `http://127.0.0.1:${server.address().port}`;
});

after(() => new Promise((resolve) => server.close(resolve)));
beforeEach(() => { requests = []; folders = []; needTotalItems = 0; });

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

  test("office folders use the org disk and send-receive mode", async () => {
    const client = makeClient();
    const folder = await client.addOfficeFolder("country-docs", "/org-owned", "Country docs");
    assert.equal(folder.path, "/org-owned/folders/country-docs");
    assert.equal(folder.type, "sendreceive");
    assert.throws(() => officeFolderPath("../escape", "/org-owned"));
    assert.throws(() => officeFolderPath("valid", "relative"));
  });

  test("health reports aggregate counts, last seen, and free disk without file names", async () => {
    const client = new SyncthingClient({ baseUrl, apiKey: "test-key", statfsImpl: async () => ({ bavail: 25n, bsize: 4096n }) });
    await client.addOfficeFolder("country-docs", "/org-owned");
    needTotalItems = 3;
    await client.shareFolder("country-docs", PEER_ID);
    const health = await client.getOfficeHealth();
    assert.deepEqual(health.folders, [{ id: "country-docs", status: "Syncing", outOfSyncItems: 3, freeDiskBytes: 102400 }]);
    assert.deepEqual(health.devices, [{ deviceId: PEER_ID, connected: true, lastSeen: "2026-08-23T03:00:00Z" }]);
    assert.equal(JSON.stringify(health).includes("path"), false);
    assert.equal(JSON.stringify(health).includes("name"), false);
  });

  test("does not include the API key in an error", async () => {
    const client = new SyncthingClient({ baseUrl, apiKey: "wrong-secret" });
    await assert.rejects(client.getDeviceId(), (error) => !error.message.includes("wrong-secret"));
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
