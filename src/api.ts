/** Shared API helpers for the BlakSync GUI. */

export type SharedDevice = {
  deviceId: string
  name: string
}

export type FolderRow = {
  id: string
  label: string
  path: string
  status: string
  folderType?: string
  outOfSync: number
  sizeBytes: number
  accessNote: string
  sharedWith: SharedDevice[]
  devices: string[]
  conflicts?: string[]
  conflictAdvice?: string
  unshareLeavesFiles?: boolean
}

export type RemoteDevice = {
  deviceId: string
  name: string
  connected: boolean
  completion: number
  sharedFolders: string[]
  paused: boolean
  address: string
}

export type PendingDevice = {
  deviceId: string
  name: string
  address: string
  shortCode?: string
  accessNotes: { folderId: string; note: string }[]
}

export type ThisDevice = {
  deviceId: string
  name: string
  version: string
  syncthingGuiAddress: string
  uptimeSeconds: number
  inboundBytes: number
  outboundBytes: number
  shortCode?: string
  qrSvg?: string
  discoveryPreset?: string
  discoveryLabel?: string
  sendLimitKib?: number
  receiveLimitKib?: number
  syncthingPin?: string
  blaksyncVersion?: string
}

export type Capabilities = {
  actorId: string
  actorName: string
  role: 'owner' | 'admin' | 'member' | null
  accept: boolean
  share: boolean
  revoke: boolean
  assignRoles: boolean
  exportAudit: boolean
  writeFolder: boolean
  editSettings: boolean
}

export type Overview = {
  thisDevice: ThisDevice
  folders: FolderRow[]
  remoteDevices: RemoteDevice[]
  pending: {
    devices: PendingDevice[]
    folders: {
      folderId: string
      label: string
      accessNote: string
      offeredBy?: Record<string, { name?: string }>
    }[]
  }
  syncthingUrl: string
  capabilities?: Capabilities
  setupNeeded?: boolean
  discoveryPreset?: string
  globalPaused?: boolean
  versionCheck?: boolean
  tls?: boolean
}

export type Settings = {
  thisDevice: ThisDevice
  syncthingUrl: string
  guiBind: string
  accessNotesPath: string
  stockGuiFallback: string
  tls?: boolean
  versionCheck?: boolean
  startAtLogin?: boolean
  discoveryPreset?: string
  discoveryLabel?: string
  tailscaleListen?: string
  sendLimitKib?: number
  receiveLimitKib?: number
  syncthingPin?: string
  blaksyncVersion?: string
  capabilities?: Capabilities
  organisation?: { name: string; timezone: string; contact: string }
  roleRule?: string
  privacyPath?: string
  support?: string
}

export type AuditRow = {
  timestamp: string
  event: string
  actor: string
  role: string
  device_id: string
  folder_label: string
}

export type OfficeHealth = {
  folders: { id: string; status: string; outOfSyncItems: number; freeDiskBytes: number }[]
  devices: { deviceId: string; connected: boolean; lastSeen?: string | null }[]
}

export type UpdateInfo = {
  enabled: boolean
  current: string
  latest?: string | null
  newer: boolean
  releaseUrl?: string | null
}

export type Member = {
  id: string
  name: string
  role: string
  device_id?: string | null
}

export async function apiGet<T>(path: string): Promise<T> {
  const response = await fetch(path)
  const data = await response.json()
  if (!response.ok) throw new Error(data.error || `Request failed (${response.status})`)
  return data as T
}

export async function apiSend<T>(path: string, method: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method,
    headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const data = await response.json()
  if (!response.ok) throw new Error(data.error || `Request failed (${response.status})`)
  return data as T
}

export function formatBytes(value: number): string {
  if (!value) return '0 B'
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB']
  let size = value
  let unit = 0
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit += 1
  }
  return `${size.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`
}
