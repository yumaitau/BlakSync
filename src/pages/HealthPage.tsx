import { useEffect, useState } from 'react'
import { apiGet, formatBytes, type OfficeHealth } from '../api'

export default function HealthPage() {
  const [health, setHealth] = useState<OfficeHealth | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    apiGet<OfficeHealth>('/api/office-health')
      .then(setHealth)
      .catch((err: Error) => setError(err.message))
  }, [])

  return (
    <section className="page">
      <h1>Office-node health</h1>
      <p className="lede">
        Folder IDs, out-of-sync counts, free disk, and last seen. File names are not shown. Replace the binary and
        restart the service to update without losing pairing.
      </p>
      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      {!health && !error ? <p className="empty-state">Loading health…</p> : null}
      {health ? (
        <>
          <h2 className="section-title">Folders</h2>
          {health.folders.length === 0 ? (
            <p className="empty-state">No folders on this node.</p>
          ) : (
            <ul className="row-list" aria-label="Folder health">
              {health.folders.map((folder) => {
                const lowDisk = folder.freeDiskBytes > 0 && folder.freeDiskBytes < 512 * 1024 * 1024
                return (
                  <li key={folder.id} className="row-card">
                    <div className="row-main">
                      <h3>{folder.id}</h3>
                      <p className="meta">
                        <span className={`status status-${folder.status.toLowerCase().replace(/\s+/g, '-')}`}>
                          {folder.status}
                        </span>
                        <span>{folder.outOfSyncItems} items out of sync</span>
                        <span>
                          {formatBytes(folder.freeDiskBytes)} free
                          {lowDisk ? ' — disk is low' : ''}
                        </span>
                      </p>
                    </div>
                  </li>
                )
              })}
            </ul>
          )}
          <h2 className="section-title">Devices</h2>
          {health.devices.length === 0 ? (
            <p className="empty-state">No remote devices recorded.</p>
          ) : (
            <ul className="row-list" aria-label="Device health">
              {health.devices.map((device) => (
                <li key={device.deviceId} className="row-card">
                  <div className="row-main">
                    <h3>
                      <code className="device-id">{device.deviceId}</code>
                    </h3>
                    <p className="meta">
                      <span className={`status ${device.connected ? 'status-up-to-date' : 'status-unshared'}`}>
                        {device.connected ? 'Connected' : 'Disconnected'}
                      </span>
                      <span>Last seen {device.lastSeen || 'unknown'}</span>
                    </p>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </>
      ) : null}
    </section>
  )
}
