import { useCallback, useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { apiGet, apiSend, type RemoteDevice } from '../api'

export default function RemoteDevicesPage() {
  const [devices, setDevices] = useState<RemoteDevice[]>([])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [showAdd, setShowAdd] = useState(false)

  const refresh = useCallback(async () => {
    setDevices(await apiGet<RemoteDevice[]>('/api/devices'))
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

  return (
    <section className="page">
      <div className="page-head">
        <div>
          <h1>Remote devices</h1>
          <p className="lede">Machines you have accepted. Connection and completion come from Syncthing.</p>
        </div>
        <button type="button" className="btn" onClick={() => setShowAdd((value) => !value)} aria-expanded={showAdd}>
          {showAdd ? 'Close' : 'Add device'}
        </button>
      </div>

      {error ? <p className="banner error" role="alert">{error}</p> : null}

      {showAdd ? (
        <form className="panel form-grid" onSubmit={onAdd}>
          <label className="span-2">
            Device ID
            <input
              name="deviceId"
              required
              autoComplete="off"
              spellCheck={false}
              placeholder="XXXXXXX-XXXXXXX-XXXXXXX-XXXXXXX-XXXXXXX-XXXXXXX-XXXXXXX-XXXXXXX"
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
        <p className="empty-state">No remote devices yet. Add a device ID from the other machine, or accept one under Pending.</p>
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
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
