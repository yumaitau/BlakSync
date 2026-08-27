import { useCallback, useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import ConfirmDialog from '../components/ConfirmDialog'
import {
  apiGet,
  apiSend,
  formatBytes,
  type Capabilities,
  type FolderRow,
  type Overview,
  type RemoteDevice,
} from '../api'

type Dialog =
  | { kind: 'unshare'; folderId: string; deviceId: string; deviceName: string }
  | { kind: 'remove'; folderId: string }
  | null

export default function FoldersPage() {
  const [folders, setFolders] = useState<FolderRow[]>([])
  const [devices, setDevices] = useState<RemoteDevice[]>([])
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null)
  const [globalPaused, setGlobalPaused] = useState(false)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState('')
  const [shareTarget, setShareTarget] = useState<Record<string, string>>({})
  const [showAdd, setShowAdd] = useState(false)
  const [dialog, setDialog] = useState<Dialog>(null)
  const [typed, setTyped] = useState('')
  const [ignores, setIgnores] = useState<Record<string, string>>({})

  const refresh = useCallback(async () => {
    const data = await apiGet<Overview>('/api/overview')
    setFolders(data.folders)
    setDevices(data.remoteDevices)
    setCapabilities(data.capabilities ?? null)
    setGlobalPaused(Boolean(data.globalPaused))
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
        folderType: String(form.get('folderType') || 'sendreceive'),
      }),
    )
    setShowAdd(false)
    event.currentTarget.reset()
  }

  const canShare = capabilities?.share !== false
  const canWrite = capabilities?.writeFolder !== false

  return (
    <section className="page">
      <div className="page-head">
        <div>
          <h1>Folders</h1>
          <p className="lede">
            Named folders on this machine. Sharing is explicit. Unshare stops future copies; files already received stay
            on both disks.
          </p>
        </div>
        <div className="form-actions">
          {globalPaused ? (
            <button
              type="button"
              className="btn primary"
              disabled={Boolean(busy)}
              onClick={() => run('resume-all', () => apiSend('/api/folders/resume-all', 'POST'))}
            >
              Resume all
            </button>
          ) : (
            <button
              type="button"
              className="btn"
              disabled={Boolean(busy)}
              onClick={() => run('pause-all', () => apiSend('/api/folders/pause-all', 'POST'))}
            >
              Pause all
            </button>
          )}
          {canWrite ? (
            <button type="button" className="btn" onClick={() => setShowAdd((value) => !value)} aria-expanded={showAdd}>
              {showAdd ? 'Close' : 'Add folder'}
            </button>
          ) : null}
        </div>
      </div>

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
      {globalPaused ? (
        <p className="banner" role="status">
          All folders are paused. New transfers are stopped on this device.
        </p>
      ) : null}

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
          <label>
            Folder type
            <select name="folderType" defaultValue="sendreceive">
              <option value="sendreceive">Send and receive</option>
              <option value="receiveonly">Receive only</option>
            </select>
          </label>
          <label className="span-2">
            Access note
            <textarea
              name="accessNote"
              rows={3}
              required
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
                  <span>{folder.outOfSync} items out of sync</span>
                  <span>{formatBytes(folder.sizeBytes)}</span>
                  <span>{folder.folderType === 'receiveonly' ? 'Receive only' : 'Send and receive'}</span>
                </p>
                <p className="path">{folder.path}</p>
                <p className="shared-with">
                  Shared with:{' '}
                  {folder.sharedWith.length === 0
                    ? 'nobody yet'
                    : folder.sharedWith.map((device) => device.name).join(', ')}
                </p>
                {folder.conflicts && folder.conflicts.length > 0 ? (
                  <div className="access-note" role="status">
                    <span className="note-label">Conflicts</span>
                    <p>{folder.conflictAdvice}</p>
                    <ul className="note-list">
                      {folder.conflicts.map((path) => (
                        <li key={path}>
                          <code>{path}</code>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {folder.accessNote ? (
                  <p className="access-note">
                    <span className="note-label">Access note</span>
                    {folder.accessNote}
                  </p>
                ) : (
                  <p className="access-note muted">No access note on this folder.</p>
                )}
                {canWrite ? (
                  <label className="ignore-field">
                    Ignore patterns
                    <textarea
                      rows={2}
                      value={ignores[folder.id] ?? ''}
                      onChange={(event) => setIgnores((current) => ({ ...current, [folder.id]: event.target.value }))}
                      placeholder="*.tmp&#10;scratch/"
                    />
                    <button
                      type="button"
                      className="btn"
                      disabled={Boolean(busy)}
                      onClick={() =>
                        run(`ignores ${folder.id}`, () =>
                          apiSend(`/api/folders/${encodeURIComponent(folder.id)}/ignores`, 'POST', {
                            ignore: (ignores[folder.id] ?? '')
                              .split('\n')
                              .map((line) => line.trim())
                              .filter(Boolean),
                          }),
                        )
                      }
                    >
                      Save ignore patterns
                    </button>
                  </label>
                ) : null}
              </div>
              <div className="row-actions">
                {folder.status === 'Paused' ? (
                  <button
                    type="button"
                    className="btn"
                    disabled={Boolean(busy)}
                    onClick={() =>
                      run(`resume ${folder.id}`, () =>
                        apiSend(`/api/folders/${encodeURIComponent(folder.id)}/resume`, 'POST'),
                      )
                    }
                  >
                    Resume
                  </button>
                ) : (
                  <button
                    type="button"
                    className="btn"
                    disabled={Boolean(busy)}
                    onClick={() =>
                      run(`pause ${folder.id}`, () =>
                        apiSend(`/api/folders/${encodeURIComponent(folder.id)}/pause`, 'POST'),
                      )
                    }
                  >
                    Pause
                  </button>
                )}
                {canWrite && folder.folderType !== 'receiveonly' ? (
                  <button
                    type="button"
                    className="btn"
                    disabled={Boolean(busy)}
                    onClick={() =>
                      run(`type ${folder.id}`, () =>
                        apiSend(`/api/folders/${encodeURIComponent(folder.id)}/type`, 'PATCH', {
                          folderType: 'receiveonly',
                        }),
                      )
                    }
                  >
                    Make receive only
                  </button>
                ) : null}
                {canShare ? (
                  <>
                    <label className="inline-field">
                      <span className="sr-only">Share {folder.label} with device</span>
                      <select
                        aria-label={`Share ${folder.label} with device`}
                        value={shareTarget[folder.id] || ''}
                        onChange={(event) =>
                          setShareTarget((current) => ({ ...current, [folder.id]: event.target.value }))
                        }
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
                  </>
                ) : (
                  <p className="muted">Members cannot share a folder from this device.</p>
                )}
                {folder.sharedWith.map((device) => (
                  <button
                    key={device.deviceId}
                    type="button"
                    className="btn danger"
                    disabled={!canShare || Boolean(busy)}
                    onClick={() =>
                      setDialog({
                        kind: 'unshare',
                        folderId: folder.id,
                        deviceId: device.deviceId,
                        deviceName: device.name,
                      })
                    }
                  >
                    Unshare {device.name}
                  </button>
                ))}
                {canWrite ? (
                  <button
                    type="button"
                    className="btn danger"
                    disabled={Boolean(busy)}
                    onClick={() => {
                      setTyped('')
                      setDialog({ kind: 'remove', folderId: folder.id })
                    }}
                  >
                    Remove from this device
                  </button>
                ) : null}
              </div>
            </li>
          ))}
        </ul>
      )}

      {dialog?.kind === 'unshare' ? (
        <ConfirmDialog
          title="Unshare this folder"
          confirmLabel="Unshare"
          danger
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            const current = dialog
            setDialog(null)
            void run(`unshare ${current.folderId}`, () =>
              apiSend(`/api/folders/${encodeURIComponent(current.folderId)}/unshare`, 'POST', {
                deviceId: current.deviceId,
              }),
            )
          }}
        >
          <p>
            Unshare stops future copies with {dialog.deviceName}. Files already received stay on both disks. This does
            not wipe or erase files remotely.
          </p>
        </ConfirmDialog>
      ) : null}

      {dialog?.kind === 'remove' ? (
        <ConfirmDialog
          title="Remove this folder from this device"
          confirmLabel="Remove local folder"
          danger
          expected={dialog.folderId}
          typedValue={typed}
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            const current = dialog
            setDialog(null)
            void run(`remove ${current.folderId}`, () =>
              apiSend(`/api/folders/${encodeURIComponent(current.folderId)}/remove-local`, 'POST', {
                confirm: current.folderId,
              }),
            )
          }}
        >
          <p>
            This deletes the folder on this machine only. Other devices keep their copies. This is not a remote wipe.
          </p>
          <label>
            Type {dialog.folderId} to confirm
            <input value={typed} onChange={(event) => setTyped(event.target.value)} autoComplete="off" />
          </label>
        </ConfirmDialog>
      ) : null}
    </section>
  )
}

function slug(value: string) {
  return value.toLowerCase().replace(/\s+/g, '-')
}
