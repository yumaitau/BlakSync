import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { apiGet, apiSend, formatBytes, type ThisDevice } from '../api'

export default function ThisDevicePage() {
  const [device, setDevice] = useState<ThisDevice | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [send, setSend] = useState('0')
  const [receive, setReceive] = useState('0')

  useEffect(() => {
    apiGet<ThisDevice>('/api/this-device')
      .then((data) => {
        setDevice(data)
        setSend(String(data.sendLimitKib ?? 0))
        setReceive(String(data.receiveLimitKib ?? 0))
      })
      .catch((err: Error) => setError(err.message))
  }, [])

  async function onLimits(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    setError('')
    try {
      await apiSend('/api/settings/bandwidth', 'POST', {
        sendLimitKib: Number(send) || 0,
        receiveLimitKib: Number(receive) || 0,
      })
      setDevice(await apiGet<ThisDevice>('/api/this-device'))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="page">
      <h1>This device</h1>
      <p className="lede">
        Your Syncthing identity on this machine. Pair by showing the QR or the six-character code. The stored identity
        is still the full certificate device ID.
      </p>
      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      {!device && !error ? <p className="empty-state">Loading this device…</p> : null}
      {device ? (
        <>
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
              <dt>Short pairing code</dt>
              <dd>
                <code>{device.shortCode}</code>
              </dd>
            </div>
            <div>
              <dt>Discovery</dt>
              <dd>{device.discoveryLabel || device.discoveryPreset || 'Defaults'}</dd>
            </div>
            <div>
              <dt>Syncthing version</dt>
              <dd>
                {device.version || 'Unknown'} (pin {device.syncthingPin})
              </dd>
            </div>
            <div>
              <dt>BlakSync</dt>
              <dd>{device.blaksyncVersion}</dd>
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
            <div>
              <dt>Bandwidth cap</dt>
              <dd>
                Send {device.sendLimitKib || 0} KiB/s · Receive {device.receiveLimitKib || 0} KiB/s (0 means no cap)
              </dd>
            </div>
          </dl>
          {device.qrSvg ? (
            <figure className="panel qr-panel">
              <div className="qr" aria-hidden="true" dangerouslySetInnerHTML={{ __html: device.qrSvg }} />
              <figcaption>Show this QR on one screen. The other machine can add the device without typing the full ID.</figcaption>
            </figure>
          ) : null}
          <form className="panel form-grid" onSubmit={onLimits}>
            <label>
              Send limit (KiB/s)
              <input
                type="number"
                min={0}
                value={send}
                onChange={(event) => setSend(event.target.value)}
                aria-describedby="limit-help"
              />
            </label>
            <label>
              Receive limit (KiB/s)
              <input type="number" min={0} value={receive} onChange={(event) => setReceive(event.target.value)} />
            </label>
            <p id="limit-help" className="muted span-2">
              Use a low cap on a satellite or phone link. Zero means Syncthing&apos;s usual unrestricted rate.
            </p>
            <div className="form-actions span-2">
              <button type="submit" className="btn primary" disabled={busy}>
                Save bandwidth limit
              </button>
            </div>
          </form>
        </>
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
