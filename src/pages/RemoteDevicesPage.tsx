import { useCallback, useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import ConfirmDialog from '../components/ConfirmDialog'
import { apiGet, apiSend, type Capabilities, type Overview, type RemoteDevice } from '../api'

export default function RemoteDevicesPage() {
  const [devices, setDevices] = useState<RemoteDevice[]>([])
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [showAdd, setShowAdd] = useState(false)
  const [revokeId, setRevokeId] = useState<RemoteDevice | null>(null)

  const refresh = useCallback(async () => {
    const overview = await apiGet<Overview>('/api/overview')
    setDevices(overview.remoteDevices)
    setCapabilities(overview.capabilities ?? null)
  }, [])

  useEffect(() => {
    refresh().catch((err: Error) => setError(err.message))
  }, [refresh])

  async function onAdd(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    setBusy(true)
    setError('')
    try {
      await apiSend('/api/devices', 'POST', {
        deviceId: String(form.get('deviceId') || '').trim(),
        name: String(form.get('name') || '').trim(),
      })
      await refresh()
      setShowAdd(false)
      event.currentTarget.reset()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const canAdd = capabilities?.accept !== false
  const canRevoke = capabilities?.revoke !== false

  return (
    <section className="page">
      <div className="page-head">
        <div>
          <h1>Remote devices</h1>
          <p className="lede">
            Machines you have accepted. Connection and completion come from Syncthing. Revoke stops future copies; it
            does not wipe a lost disk.
          </p>
        </div>
        {canAdd ? (
          <button type="button" className="btn" onClick={() => setShowAdd((value) => !value)} aria-expanded={showAdd}>
            {showAdd ? 'Close' : 'Add device'}
          </button>
        ) : null}
      </div>

      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}

      {showAdd ? (
        <form className="panel form-grid" onSubmit={onAdd}>
          <label className="span-2">
            Device ID or six-character short code
            <input
              name="deviceId"
              required
              autoComplete="off"
              spellCheck={false}
              placeholder="XXXXXXX-XXXXXXX-… or AAAAAA"
            />
          </label>
          <label className="span-2">
            Name
            <input name="name" autoComplete="off" placeholder="Office laptop" />
          </label>
          <div className="form-actions span-2">
            <button type="submit" className="btn primary" disabled={busy}>
              Add device
            </button>
          </div>
        </form>
      ) : null}

      {devices.length === 0 ? (
        <p className="empty-state">
          No remote devices yet. Add a device ID or short code from the other machine, or accept one under Pending.
        </p>
      ) : (
        <ul className="row-list" aria-label="Remote devices">
          {devices.map((device) => (
            <li key={device.deviceId} className="row-card">
              <div className="row-main">
                <h2>{device.name}</h2>
                <p className="meta">
                  <span className={`status ${device.connected ? 'status-up-to-date' : 'status-unshared'}`}>
                    {device.connected ? 'Connected' : 'Disconnected'}
                  </span>
                  <span>{device.completion.toFixed(0)}% complete</span>
                </p>
                <p className="path">
                  <code className="device-id">{device.deviceId}</code>
                </p>
                <p className="shared-with">
                  Folders: {device.sharedFolders.length === 0 ? 'none shared' : device.sharedFolders.join(', ')}
                </p>
              </div>
              <div className="row-actions">
                {canRevoke ? (
                  <button type="button" className="btn danger" disabled={busy} onClick={() => setRevokeId(device)}>
                    Revoke device
                  </button>
                ) : (
                  <p className="muted">Members cannot revoke a device.</p>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}

      {revokeId ? (
        <ConfirmDialog
          title={`Revoke ${revokeId.name}`}
          confirmLabel="Revoke"
          danger
          onCancel={() => setRevokeId(null)}
          onConfirm={() => {
            const device = revokeId
            setRevokeId(null)
            setBusy(true)
            apiSend(`/api/devices/${encodeURIComponent(device.deviceId)}/revoke`, 'POST')
              .then(refresh)
              .catch((err: Error) => setError(err.message))
              .finally(() => setBusy(false))
          }}
        >
          <p>
            This unshares every folder with {revokeId.name} and removes it from this machine. Files already on the lost
            disk stay there. This is not a remote wipe.
          </p>
          <p>If this device&apos;s own key leaked, use the rotate steps on Settings after you revoke.</p>
        </ConfirmDialog>
      ) : null}
    </section>
  )
}
