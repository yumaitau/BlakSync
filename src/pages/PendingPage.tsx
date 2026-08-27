import { useCallback, useEffect, useState } from 'react'
import { apiGet, apiSend, type Capabilities, type Overview } from '../api'

export default function PendingPage() {
  const [pending, setPending] = useState<Overview['pending']>({ devices: [], folders: [] })
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState('')
  const [acceptPath, setAcceptPath] = useState<Record<string, string>>({})

  const refresh = useCallback(async () => {
    const data = await apiGet<Overview>('/api/overview')
    setPending(data.pending)
    setCapabilities(data.capabilities ?? null)
  }, [])

  useEffect(() => {
    refresh().catch((err: Error) => setError(err.message))
  }, [refresh])

  async function run(label: string, action: () => Promise<unknown>) {
    setBusy(label)
    setError('')
    try {
      await action()
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy('')
    }
  }

  const canAccept = capabilities?.accept !== false

  return (
    <section className="page">
      <h1>Pending</h1>
      <p className="lede">
        Devices and folders waiting for a yes or no. Read the access note before you accept. Membership is deliberate.
      </p>

      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      {busy ? (
        <p className="banner" aria-live="polite">
          Working: {busy}
        </p>
      ) : null}

      <h2 className="section-title">Devices waiting</h2>
      {pending.devices.length === 0 ? (
        <p className="empty-state">No devices waiting to pair.</p>
      ) : (
        <ul className="row-list" aria-label="Pending devices">
          {pending.devices.map((device) => (
            <li key={device.deviceId} className="row-card">
              <div className="row-main">
                <h3>{device.name}</h3>
                <p className="path">
                  <code className="device-id">{device.deviceId}</code>
                </p>
                {device.shortCode ? <p className="meta">Short code {device.shortCode}</p> : null}
                {device.address ? <p className="meta">Seen from {device.address}</p> : null}
                <div className="access-note-block" role="region" aria-label="Access notes before accept">
                  <p className="note-label">Access notes on this org</p>
                  {device.accessNotes.length === 0 ? (
                    <p className="muted">No access notes are set on folders yet.</p>
                  ) : (
                    <ul className="note-list">
                      {device.accessNotes.map((entry) => (
                        <li key={entry.folderId}>
                          <strong>{entry.folderId}</strong>: {entry.note}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </div>
              <div className="row-actions">
                {canAccept ? (
                  <>
                    <button
                      type="button"
                      className="btn primary"
                      disabled={Boolean(busy)}
                      onClick={() =>
                        run(`accept ${device.deviceId}`, () =>
                          apiSend(`/api/pending/devices/${encodeURIComponent(device.deviceId)}/accept`, 'POST', {
                            name: device.name,
                          }),
                        )
                      }
                    >
                      Accept device
                    </button>
                    <button
                      type="button"
                      className="btn danger"
                      disabled={Boolean(busy)}
                      onClick={() =>
                        run(`deny ${device.deviceId}`, () =>
                          apiSend(`/api/pending/devices/${encodeURIComponent(device.deviceId)}/deny`, 'POST'),
                        )
                      }
                    >
                      Deny
                    </button>
                  </>
                ) : (
                  <p className="muted">Members cannot accept a new device. The access note is still here to read.</p>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}

      <h2 className="section-title">Folders offered</h2>
      {pending.folders.length === 0 ? (
        <p className="empty-state">No folder offers waiting.</p>
      ) : (
        <ul className="row-list" aria-label="Pending folders">
          {pending.folders.map((folder) => {
            const offeredId = Object.keys(folder.offeredBy ?? {})[0] ?? ''
            return (
              <li key={folder.folderId} className="row-card">
                <div className="row-main">
                  <h3>{folder.label}</h3>
                  <p className="meta">Folder ID: {folder.folderId}</p>
                  {folder.accessNote ? (
                    <p className="access-note">
                      <span className="note-label">Access note</span>
                      {folder.accessNote}
                    </p>
                  ) : (
                    <p className="muted">No access note attached.</p>
                  )}
                  <label>
                    Local path
                    <input
                      value={acceptPath[folder.folderId] ?? ''}
                      onChange={(event) =>
                        setAcceptPath((current) => ({ ...current, [folder.folderId]: event.target.value }))
                      }
                      autoComplete="off"
                      placeholder="/home/org/heritage-scans"
                    />
                  </label>
                </div>
                <div className="row-actions">
                  <button
                    type="button"
                    className="btn primary"
                    disabled={!acceptPath[folder.folderId] || !offeredId || Boolean(busy)}
                    onClick={() =>
                      run(`accept-folder ${folder.folderId}`, () =>
                        apiSend(`/api/pending/folders/${encodeURIComponent(folder.folderId)}/accept`, 'POST', {
                          path: acceptPath[folder.folderId],
                          deviceId: offeredId,
                          label: folder.label,
                        }),
                      )
                    }
                  >
                    Accept folder
                  </button>
                </div>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
