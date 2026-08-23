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

  getSystemStatus() { return this.request("GET", "/rest/system/status"); }
  getDeviceId() { return this.getSystemStatus().then((status) => status.myID); }
  getConnections() { return this.request("GET", "/rest/system/connections"); }
  listDevices() { return this.request("GET", "/rest/config/devices"); }
  listFolders() { return this.request("GET", "/rest/config/folders"); }
  listPendingDevices() { return this.request("GET", "/rest/cluster/pending/devices"); }
  listPendingFolders() { return this.request("GET", "/rest/cluster/pending/folders"); }

  async getConfigOptions() {
    return this.request("GET", "/rest/config/options");
  }

  async getGuiConfig() {
    return this.request("GET", "/rest/config/gui");
  }

  async addDevice(deviceId, name = "") {
    assertDeviceId(deviceId);
    const template = await this.request("GET", "/rest/config/defaults/device");
    const device = { ...template, deviceID: deviceId, name, addresses: ["dynamic"] };
    await this.request("POST", "/rest/config/devices", device);
    return device;
  }

  async denyPendingDevice(deviceId) {
    assertDeviceId(deviceId);
    await this.request("DELETE", `/rest/cluster/pending/devices?device=${encodeURIComponent(deviceId)}`);
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

  async getCompletion(deviceId, folderId) {
    const query = new URLSearchParams();
    if (deviceId) query.set("device", deviceId);
    if (folderId) query.set("folder", folderId);
    return this.request("GET", `/rest/db/completion?${query}`);
  }

  async listFolderStatus() {
    const [folders, ownId] = await Promise.all([this.listFolders(), this.getDeviceId()]);
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

  async listRemoteDeviceStatus() {
    const [devices, connections, ownId, folders] = await Promise.all([
      this.listDevices(),
      this.getConnections(),
      this.getDeviceId(),
      this.listFolders(),
    ]);
    const connectionMap = connections.connections ?? {};
    return Promise.all(
      devices
        .filter((device) => device.deviceID !== ownId)
        .map(async (device) => {
          const connection = connectionMap[device.deviceID] ?? {};
          let completion = 0;
          try {
            const result = await this.getCompletion(device.deviceID);
            completion = Number(result.completion ?? 0);
          } catch {
            completion = connection.connected ? 0 : 0;
          }
          const sharedFolders = folders
            .filter((folder) => (folder.devices ?? []).some((entry) => entry.deviceID === device.deviceID))
            .map((folder) => folder.label || folder.id);
          return {
            deviceId: device.deviceID,
            name: device.name || device.deviceID,
            connected: Boolean(connection.connected),
            completion,
            sharedFolders,
            paused: Boolean(device.paused),
            address: connection.address || "",
          };
        }),
    );
  }

  async getThisDevice() {
    const [status, devices, connections, gui] = await Promise.all([
      this.getSystemStatus(),
      this.listDevices(),
      this.getConnections(),
      this.getGuiConfig(),
    ]);
    const self = devices.find((device) => device.deviceID === status.myID) ?? { deviceID: status.myID, name: "" };
    const total = connections.total ?? {};
    return {
      deviceId: status.myID,
      name: self.name || "This device",
      version: status.version || "",
      syncthingGuiAddress: gui.address || "127.0.0.1:8384",
      uptimeSeconds: Number(status.uptime ?? 0),
      inboundBytes: Number(total.inBytesTotal ?? 0),
      outboundBytes: Number(total.outBytesTotal ?? 0),
    };
  }
}

function folderStatus(folder, status, remoteDevices, runtime = {}) {
  return {
    id: folder.id,
    label: folder.label || folder.id,
    path: folder.path,
    status,
    devices: remoteDevices.map((device) => device.deviceID),
    needBytes: runtime.needBytes ?? 0,
    needFiles: runtime.needFiles ?? 0,
    outOfSync: runtime.needTotalItems ?? runtime.needFiles ?? 0,
    sizeBytes: runtime.globalBytes ?? runtime.localBytes ?? 0,
  };
}

export function assertDeviceId(deviceId) {
  if (!DEVICE_ID.test(deviceId)) throw new Error("Device ID must be a full Syncthing device ID (eight groups of seven A-Z/2-7 characters)");
}
