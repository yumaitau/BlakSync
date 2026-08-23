import { useCallback, useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import {
  apiGet,
  apiSend,
  formatBytes,
  type FolderRow,
  type Overview,
  type RemoteDevice,
} from '../api'

export default function FoldersPage() {
  const [folders, setFolders] = useState<FolderRow[]>([])
  const [devices, setDevices] = useState<RemoteDevice[]>([])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState('')
  const [shareTarget, setShareTarget] = useState<Record<string, string>>({})
  const [showAdd, setShowAdd] = useState(false)

  const refresh = useCallback(async () => {
    const data = await apiGet<Overview>('/api/overview')
    setFolders(data.folders)
    setDevices(data.remoteDevices)
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

  async function onAdd(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    await run('add-folder', () =>
      apiSend('/api/folders', 'POST', {
        id: String(form.get('id') || '').trim(),
        path: String(form.get('path') || '').trim(),
        label: String(form.get('label') || '').trim(),
        accessNote: String(form.get('accessNote') || '').trim(),
      }),
    )
    setShowAdd(false)
    event.currentTarget.reset()
  }

  return (
    <section className="page">
      <div className="page-head">
        <div>
          <h1>Folders</h1>
          <p className="lede">Named folders on this machine. Sharing is explicit. Nothing moves until the other side accepts.</p>
        </div>
        <button type="button" className="btn" onClick={() => setShowAdd((value) => !value)} aria-expanded={showAdd}>
          {showAdd ? 'Close' : 'Add folder'}
        </button>
      </div>

      {error ? <p className="banner error" role="alert">{error}</p> : null}
      {busy ? <p className="banner" aria-live="polite">Working: {busy}</p> : null}

      {showAdd ? (
        <form className="panel form-grid" onSubmit={onAdd}>
          <label>
            Folder ID
            <input name="id" required autoComplete="off" placeholder="heritage-scans" />
          </label>
          <label>
            Label
            <input name="label" autoComplete="off" placeholder="Heritage scans" />
          </label>
          <label className="span-2">
            Path on this device
            <input name="path" required autoComplete="off" placeholder="/home/org/heritage-scans" />
          </label>
          <label className="span-2">
            Access note
            <textarea
              name="accessNote"
              rows={3}
              placeholder="Speak with the cultural officer before pairing a new device."
            />
          </label>
          <div className="form-actions span-2">
            <button type="submit" className="btn primary" disabled={Boolean(busy)}>
              Save folder
            </button>
          </div>
        </form>
      ) : null}

      {folders.length === 0 ? (
        <p className="empty-state">No folders yet. Add one, then share it with a paired device.</p>
      ) : (
        <ul className="row-list" aria-label="Folders">
          {folders.map((folder) => (
            <li key={folder.id} className="row-card">
              <div className="row-main">
                <h2>{folder.label}</h2>
                <p className="meta">
                  <span className={`status status-${slug(folder.status)}`}>{folder.status}</span>
                  <span>{folder.outOfSync} out of sync</span>
                  <span>{formatBytes(folder.sizeBytes)}</span>
                </p>
                <p className="path">{folder.path}</p>
                <p className="shared-with">
                  Shared with:{' '}
                  {folder.sharedWith.length === 0
                    ? 'nobody yet'
                    : folder.sharedWith.map((device) => device.name).join(', ')}
                </p>
                {folder.accessNote ? (
                  <p className="access-note">
                    <span className="note-label">Access note</span>
                    {folder.accessNote}
                  </p>
                ) : (
                  <p className="access-note muted">No access note on this folder.</p>
                )}
              </div>
              <div className="row-actions">
                {folder.status === 'Paused' ? (
                  <button
                    type="button"
                    className="btn"
                    disabled={Boolean(busy)}
                    onClick={() => run(`resume ${folder.id}`, () => apiSend(`/api/folders/${encodeURIComponent(folder.id)}/resume`, 'POST'))}
                  >
                    Resume
                  </button>
                ) : (
                  <button
                    type="button"
                    className="btn"
                    disabled={Boolean(busy)}
                    onClick={() => run(`pause ${folder.id}`, () => apiSend(`/api/folders/${encodeURIComponent(folder.id)}/pause`, 'POST'))}
                  >
                    Pause
                  </button>
                )}
                <label className="inline-field">
                  <span className="sr-only">Share {folder.label} with device</span>
                  <select
                    aria-label={`Share ${folder.label} with device`}
                    value={shareTarget[folder.id] || ''}
                    onChange={(event) => setShareTarget((current) => ({ ...current, [folder.id]: event.target.value }))}
                  >
                    <option value="">Choose device</option>
                    {devices.map((device) => (
                      <option key={device.deviceId} value={device.deviceId}>
                        {device.name}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  className="btn primary"
                  disabled={!shareTarget[folder.id] || Boolean(busy)}
                  onClick={() =>
                    run(`share ${folder.id}`, () =>
                      apiSend(`/api/folders/${encodeURIComponent(folder.id)}/share`, 'POST', {
                        deviceId: shareTarget[folder.id],
                      }),
                    )
                  }
                >
                  Share
                </button>
                {folder.sharedWith.map((device) => (
                  <button
                    key={device.deviceId}
                    type="button"
                    className="btn danger"
                    disabled={Boolean(busy)}
                    onClick={() =>
                      run(`unshare ${folder.id}`, () =>
                        apiSend(`/api/folders/${encodeURIComponent(folder.id)}/unshare`, 'POST', {
                          deviceId: device.deviceId,
                        }),
                      )
                    }
                  >
                    Unshare {device.name}
                  </button>
                ))}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function slug(value: string) {
  return value.toLowerCase().replace(/\s+/g, '-')
}
