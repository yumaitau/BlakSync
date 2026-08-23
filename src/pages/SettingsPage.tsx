import { useEffect, useState } from 'react'
import { apiGet, type Settings } from '../api'

export default function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    apiGet<Settings>('/api/settings')
      .then(setSettings)
      .catch((err: Error) => setError(err.message))
  }, [])

  return (
    <section className="page">
      <h1>Settings</h1>
      <p className="lede">
        BlakSync talks to Syncthing on this machine only. The stock Syncthing GUI stays available if you need it.
      </p>
      {error ? <p className="banner error" role="alert">{error}</p> : null}
      {!settings && !error ? <p className="empty-state">Loading settings…</p> : null}
      {settings ? (
        <dl className="detail-grid panel">
          <div>
            <dt>BlakSync GUI bind</dt>
            <dd>
              <code>{settings.guiBind}</code> (localhost only by default)
            </dd>
          </div>
          <div>
            <dt>Syncthing API</dt>
            <dd>
              <code>{settings.syncthingUrl}</code>
            </dd>
          </div>
          <div>
            <dt>Stock Syncthing GUI</dt>
            <dd>
              <a href={settings.stockGuiFallback} rel="noreferrer">
                {settings.stockGuiFallback}
              </a>
            </dd>
          </div>
          <div>
            <dt>Access notes file</dt>
            <dd>
              <code>{settings.accessNotesPath}</code>
            </dd>
          </div>
          <div>
            <dt>This device ID</dt>
            <dd>
              <code className="device-id">{settings.thisDevice.deviceId}</code>
            </dd>
          </div>
        </dl>
      ) : null}
    </section>
  )
}
