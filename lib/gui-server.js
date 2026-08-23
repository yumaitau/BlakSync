import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { AccessNotesStore } from "./access-notes.js";
import { SyncthingClient, SyncthingError } from "./syncthing.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const DIST = path.join(ROOT, "dist");

const MIME = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
};

export function createGuiServer({
  host = "127.0.0.1",
  port = 8385,
  syncthingUrl = process.env.BLAKSYNC_URL || "http://127.0.0.1:8384",
  apiKey = process.env.BLAKSYNC_API_KEY,
  configDir,
  staticRoot = DIST,
  clientFactory,
} = {}) {
  const notes = new AccessNotesStore(configDir);
  const makeClient = clientFactory || (() => new SyncthingClient({ baseUrl: syncthingUrl, apiKey }));

  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url || "/", `http://${host}:${port}`);
      if (url.pathname.startsWith("/api/")) {
        if (!allowBrowserRequest(request, response, host)) return;
        await handleApi(request, response, url, makeClient, notes, syncthingUrl);
        return;
      }
      await serveStatic(response, url.pathname, staticRoot);
    } catch (error) {
      sendError(response, error);
    }
  });

  return {
    host,
    port,
    server,
    listen() {
      return new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen(port, host, () => {
          server.off("error", reject);
          resolve(`http://${host}:${port}`);
        });
      });
    },
    close() {
      return new Promise((resolve, reject) => {
        server.close((error) => (error ? reject(error) : resolve()));
      });
    },
  };
}

async function handleApi(request, response, url, makeClient, notes, syncthingUrl) {
  if (request.method === "OPTIONS") {
    response.writeHead(204, corsHeaders());
    response.end();
    return;
  }

  const client = makeClient();
  const parts = url.pathname.replace(/^\/api\//, "").split("/").filter(Boolean);
  const body = ["POST", "PATCH", "PUT"].includes(request.method || "") ? await readJson(request) : undefined;

  if (parts[0] === "health" && request.method === "GET") {
    sendJson(response, 200, { ok: true });
    return;
  }

  if (parts[0] === "overview" && request.method === "GET") {
    const [thisDevice, folders, remoteDevices, pendingDevices, pendingFolders, accessNotes] = await Promise.all([
      client.getThisDevice(),
      client.listFolderStatus(),
      client.listRemoteDeviceStatus(),
      client.listPendingDevices(),
      client.listPendingFolders(),
      notes.readAll(),
    ]);
    const deviceNames = Object.fromEntries(
      (await client.listDevices()).map((device) => [device.deviceID, device.name || device.deviceID]),
    );
    sendJson(response, 200, {
      thisDevice,
      folders: folders.map((folder) => enrichFolder(folder, accessNotes, deviceNames)),
      remoteDevices,
      pending: {
        devices: Object.entries(pendingDevices || {}).map(([deviceId, info]) => ({
          deviceId,
          name: info.name || deviceId,
          address: info.address || "",
          accessNotes: Object.entries(accessNotes).map(([folderId, note]) => ({ folderId, note })),
        })),
        folders: Object.entries(pendingFolders || {}).map(([folderId, info]) => ({
          folderId,
          label: info.label || folderId,
          offeredBy: info.offeredBy || {},
          accessNote: accessNotes[folderId] || "",
        })),
      },
      syncthingUrl,
    });
    return;
  }

  if (parts[0] === "folders" && parts.length === 1 && request.method === "GET") {
    const [folders, accessNotes, devices] = await Promise.all([
      client.listFolderStatus(),
      notes.readAll(),
      client.listDevices(),
    ]);
    const deviceNames = Object.fromEntries(devices.map((device) => [device.deviceID, device.name || device.deviceID]));
    sendJson(response, 200, folders.map((folder) => enrichFolder(folder, accessNotes, deviceNames)));
    return;
  }

  if (parts[0] === "folders" && parts.length === 1 && request.method === "POST") {
    const folder = await client.addFolder(required(body, "id"), required(body, "path"), body.label || body.id);
    if (body.accessNote) await notes.set(folder.id, body.accessNote);
    sendJson(response, 201, { id: folder.id, accessNote: await notes.get(folder.id) });
    return;
  }

  if (parts[0] === "folders" && parts[1] && parts[2] === "share" && request.method === "POST") {
    await client.shareFolder(parts[1], required(body, "deviceId"));
    sendJson(response, 200, { shared: true });
    return;
  }

  if (parts[0] === "folders" && parts[1] && parts[2] === "unshare" && request.method === "POST") {
    await client.unshareFolder(parts[1], required(body, "deviceId"));
    sendJson(response, 200, { shared: false });
    return;
  }

  if (parts[0] === "folders" && parts[1] && parts[2] === "pause" && request.method === "POST") {
    await client.setFolderPaused(parts[1], true);
    sendJson(response, 200, { paused: true });
    return;
  }

  if (parts[0] === "folders" && parts[1] && parts[2] === "resume" && request.method === "POST") {
    await client.setFolderPaused(parts[1], false);
    sendJson(response, 200, { paused: false });
    return;
  }

  if (parts[0] === "folders" && parts[1] && parts[2] === "note" && request.method === "PATCH") {
    const accessNote = await notes.set(parts[1], body?.accessNote ?? "");
    sendJson(response, 200, { id: parts[1], accessNote });
    return;
  }

  if (parts[0] === "devices" && parts.length === 1 && request.method === "GET") {
    sendJson(response, 200, await client.listRemoteDeviceStatus());
    return;
  }

  if (parts[0] === "devices" && parts.length === 1 && request.method === "POST") {
    const device = await client.addDevice(required(body, "deviceId"), body.name || "");
    sendJson(response, 201, { deviceId: device.deviceID, name: device.name });
    return;
  }

  if (parts[0] === "pending" && parts[1] === "devices" && parts[2] && request.method === "POST" && parts[3] === "accept") {
    const device = await client.addDevice(parts[2], body?.name || "");
    sendJson(response, 200, { accepted: true, deviceId: device.deviceID });
    return;
  }

  if (parts[0] === "pending" && parts[1] === "devices" && parts[2] && request.method === "POST" && parts[3] === "deny") {
    await client.denyPendingDevice(parts[2]);
    sendJson(response, 200, { denied: true });
    return;
  }

  if (parts[0] === "this-device" && request.method === "GET") {
    sendJson(response, 200, await client.getThisDevice());
    return;
  }

  if (parts[0] === "settings" && request.method === "GET") {
    const thisDevice = await client.getThisDevice();
    sendJson(response, 200, {
      thisDevice,
      syncthingUrl,
      guiBind: "127.0.0.1",
      accessNotesPath: notes.filePath,
      stockGuiFallback: `http://${thisDevice.syncthingGuiAddress}`,
    });
    return;
  }

  sendJson(response, 404, { error: "Not found" });
}

function enrichFolder(folder, accessNotes, deviceNames) {
  return {
    ...folder,
    accessNote: accessNotes[folder.id] || "",
    sharedWith: (folder.devices || []).map((deviceId) => ({
      deviceId,
      name: deviceNames[deviceId] || deviceId,
    })),
  };
}

async function serveStatic(response, pathname, staticRoot) {
  const safePath = pathname === "/" ? "/index.html" : pathname;
  const root = path.resolve(staticRoot);
  const resolved = path.resolve(root, `.${safePath}`);
  const relativePath = path.relative(root, resolved);
  if (relativePath.startsWith("..") || path.isAbsolute(relativePath)) {
    sendJson(response, 403, { error: "Forbidden" });
    return;
  }
  try {
    const data = await readFile(resolved);
    response.writeHead(200, { "content-type": MIME[path.extname(resolved)] || "application/octet-stream" });
    response.end(data);
  } catch {
    if (path.extname(safePath) === "") {
      const index = await readFile(path.join(staticRoot, "index.html"));
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(index);
      return;
    }
    sendJson(response, 404, { error: "Not found" });
  }
}

function required(body, key) {
  if (!body || body[key] === undefined || body[key] === "") throw new Error(`Missing ${key}`);
  return body[key];
}

async function readJson(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  const text = Buffer.concat(chunks).toString("utf8");
  if (!text) return {};
  return JSON.parse(text);
}

function sendJson(response, status, value) {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    ...corsHeaders(),
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
  });
  response.end(body);
}

function sendError(response, error) {
  const status = error instanceof SyncthingError ? error.status || 502 : 400;
  const message = error instanceof Error ? error.message : String(error);
  sendJson(response, status >= 400 ? status : 500, { error: message });
}

function allowBrowserRequest(request, response, bindHost) {
  const requestHost = request.headers.host || "";
  let requestHostname;
  try {
    requestHostname = new URL(`http://${requestHost}`).hostname;
  } catch {
    sendJson(response, 400, { error: "Invalid Host header" });
    return false;
  }

  const hostAllowed = isLoopback(bindHost) ? isLoopback(requestHostname) : requestHostname === bindHost;
  if (!hostAllowed) {
    sendJson(response, 403, { error: "Host is not allowed" });
    return false;
  }

  const origin = request.headers.origin;
  if (!origin) return true;
  let parsedOrigin;
  try {
    parsedOrigin = new URL(origin);
  } catch {
    sendJson(response, 403, { error: "Origin is not allowed" });
    return false;
  }

  const sameOrigin = parsedOrigin.protocol === "http:" && parsedOrigin.host === requestHost;
  const viteDevelopmentOrigin = parsedOrigin.protocol === "http:" && isLoopback(parsedOrigin.hostname) && parsedOrigin.port === "5173";
  if (!sameOrigin && !viteDevelopmentOrigin) {
    sendJson(response, 403, { error: "Origin is not allowed" });
    return false;
  }
  response.setHeader("access-control-allow-origin", origin);
  response.setHeader("vary", "Origin");
  return true;
}

function isLoopback(hostname) {
  return hostname === "127.0.0.1" || hostname === "localhost" || hostname === "[::1]" || hostname === "::1";
}

function corsHeaders() {
  return {
    "cache-control": "no-store",
    "access-control-allow-methods": "GET,POST,PATCH,DELETE,OPTIONS",
    "access-control-allow-headers": "content-type",
  };
}
