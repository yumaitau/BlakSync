import { useEffect, useState } from 'react'
import { apiGet, formatBytes, type ThisDevice } from '../api'

export default function ThisDevicePage() {
  const [device, setDevice] = useState<ThisDevice | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    apiGet<ThisDevice>('/api/this-device')
      .then(setDevice)
      .catch((err: Error) => setError(err.message))
  }, [])

  return (
    <section className="page">
      <h1>This device</h1>
      <p className="lede">Your Syncthing identity on this machine. Pairing uses this device ID, not a BlakSync account.</p>
      {error ? <p className="banner error" role="alert">{error}</p> : null}
      {!device && !error ? <p className="empty-state">Loading this device…</p> : null}
      {device ? (
        <dl className="detail-grid panel">
          <div>
            <dt>Name</dt>
            <dd>{device.name}</dd>
          </div>
          <div>
            <dt>Device ID</dt>
            <dd>
              <code className="device-id">{device.deviceId}</code>
            </dd>
          </div>
          <div>
            <dt>Syncthing version</dt>
            <dd>{device.version || 'Unknown'}</dd>
          </div>
          <div>
            <dt>Uptime</dt>
            <dd>{formatUptime(device.uptimeSeconds)}</dd>
          </div>
          <div>
            <dt>Transfer totals</dt>
            <dd>
              In {formatBytes(device.inboundBytes)} · Out {formatBytes(device.outboundBytes)}
            </dd>
          </div>
        </dl>
      ) : null}
    </section>
  )
}

function formatUptime(seconds: number) {
  if (!seconds) return 'Just started'
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (hours > 0) return `${hours} h ${minutes} min`
  return `${minutes} min`
}
