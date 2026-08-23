const DEVICE_ID = /^(?:[A-Z2-7]{7}-){7}[A-Z2-7]{7}$/;

export class SyncthingError extends Error {
  constructor(message, status) {
    super(message);
    this.name = "SyncthingError";
    this.status = status;
  }
}

export class SyncthingClient {
  constructor({ baseUrl = "http://127.0.0.1:8384", apiKey, fetchImpl = fetch } = {}) {
    if (!apiKey) throw new Error("A Syncthing API key is required");
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.apiKey = apiKey;
    this.fetch = fetchImpl;
  }

  async request(method, path, body) {
    const response = await this.fetch(`${this.baseUrl}${path}`, {
      method,
      headers: { "X-API-Key": this.apiKey, ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!response.ok) {
      const detail = (await response.text()).trim();
      throw new SyncthingError(`Syncthing ${method} ${path} failed (${response.status})${detail ? `: ${detail}` : ""}`, response.status);
    }
    if (response.status === 204 || response.headers.get("content-length") === "0") return undefined;
    const text = await response.text();
    return text ? JSON.parse(text) : undefined;
  }

  getDeviceId() { return this.request("GET", "/rest/system/status").then((status) => status.myID); }
  listDevices() { return this.request("GET", "/rest/config/devices"); }
  listPendingDevices() { return this.request("GET", "/rest/cluster/pending/devices"); }

  async addDevice(deviceId, name = "") {
    assertDeviceId(deviceId);
    const template = await this.request("GET", "/rest/config/defaults/device");
    const device = { ...template, deviceID: deviceId, name, addresses: ["dynamic"] };
    await this.request("POST", "/rest/config/devices", device);
    return device;
  }

  async addFolder(folderId, path, label = folderId) {
    if (!folderId || !path) throw new Error("Folder ID and path are required");
    const template = await this.request("GET", "/rest/config/defaults/folder");
    const folder = { ...template, id: folderId, path, label, paused: false };
    await this.request("POST", "/rest/config/folders", folder);
    return folder;
  }

  async shareFolder(folderId, deviceId) {
    assertDeviceId(deviceId);
    const path = `/rest/config/folders/${encodeURIComponent(folderId)}`;
    const folder = await this.request("GET", path);
    const devices = folder.devices ?? [];
    if (!devices.some((device) => device.deviceID === deviceId)) {
      await this.request("PATCH", path, { devices: [...devices, { deviceID: deviceId }] });
    }
  }

  async unshareFolder(folderId, deviceId) {
    assertDeviceId(deviceId);
    const path = `/rest/config/folders/${encodeURIComponent(folderId)}`;
    const folder = await this.request("GET", path);
    await this.request("PATCH", path, { devices: (folder.devices ?? []).filter((device) => device.deviceID !== deviceId) });
  }

  setFolderPaused(folderId, paused) {
    return this.request("PATCH", `/rest/config/folders/${encodeURIComponent(folderId)}`, { paused });
  }

  async listFolderStatus() {
    const [folders, ownId] = await Promise.all([this.request("GET", "/rest/config/folders"), this.getDeviceId()]);
    return Promise.all(folders.map(async (folder) => {
      const remoteDevices = (folder.devices ?? []).filter((device) => device.deviceID !== ownId);
      if (folder.paused) return folderStatus(folder, "Paused", remoteDevices);
      if (remoteDevices.length === 0) return folderStatus(folder, "Unshared", remoteDevices);
      const runtime = await this.request("GET", `/rest/db/status?folder=${encodeURIComponent(folder.id)}`);
      let status = "Up to Date";
      if (["scanning", "scan-wait", "cleaning"].includes(runtime.state)) status = "Preparing";
      else if (runtime.state !== "idle" || runtime.needTotalItems > 0 || runtime.needBytes > 0) status = "Syncing";
      return folderStatus(folder, status, remoteDevices, runtime);
    }));
  }
}

function folderStatus(folder, status, remoteDevices, runtime = {}) {
  return { id: folder.id, label: folder.label, path: folder.path, status, devices: remoteDevices.map((device) => device.deviceID), needBytes: runtime.needBytes ?? 0, needFiles: runtime.needFiles ?? 0 };
}

export function assertDeviceId(deviceId) {
  if (!DEVICE_ID.test(deviceId)) throw new Error("Device ID must be a full Syncthing device ID (eight groups of seven A-Z/2-7 characters)");
}
