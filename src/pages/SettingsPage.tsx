import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { apiGet, apiSend, type Member, type Settings } from '../api'

export default function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [members, setMembers] = useState<Member[]>([])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [preset, setPreset] = useState('global')
  const [tailscale, setTailscale] = useState('')
  const [newId, setNewId] = useState('')
  const [newName, setNewName] = useState('')
  const [newRole, setNewRole] = useState('member')

  async function refresh() {
    const data = await apiGet<Settings>('/api/settings')
    setSettings(data)
    setPreset(data.discoveryPreset ?? 'global')
    setTailscale(data.tailscaleListen ?? '')
    if (data.capabilities?.assignRoles) {
      const list = await apiGet<{ members: Member[] }>('/api/org/members')
      setMembers(list.members)
    }
  }

  useEffect(() => {
    refresh().catch((err: Error) => setError(err.message))
  }, [])

  async function run(action: () => Promise<unknown>) {
    setBusy(true)
    setError('')
    try {
      await action()
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  async function onDiscovery(event: FormEvent) {
    event.preventDefault()
    await run(() =>
      apiSend('/api/settings/discovery', 'POST', {
        preset,
        tailscaleListen: preset === 'tailscale' ? tailscale : undefined,
      }),
    )
  }

  async function onAddMember(event: FormEvent) {
    event.preventDefault()
    await run(() => apiSend('/api/org/members', 'POST', { id: newId, name: newName, role: newRole }))
    setNewId('')
    setNewName('')
  }

  return (
    <section className="page">
      <h1>Settings</h1>
      <p className="lede">
        BlakSync talks to Syncthing on this machine only. The stock Syncthing GUI stays available if you need it.
      </p>
      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      {!settings && !error ? <p className="empty-state">Loading settings…</p> : null}
      {settings ? (
        <>
          <dl className="detail-grid panel">
            <div>
              <dt>BlakSync GUI bind</dt>
              <dd>
                <code>{settings.guiBind}</code> (localhost only)
              </dd>
            </div>
            <div>
              <dt>TLS</dt>
              <dd>{settings.tls ? 'HTTPS on this port. HTTP is refused.' : 'HTTP on 127.0.0.1. Enable with `gui --tls`.'}</dd>
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
              <dt>Versions</dt>
              <dd>
                BlakSync {settings.blaksyncVersion} · Syncthing pin {settings.syncthingPin}
              </dd>
            </div>
            <div>
              <dt>This device ID</dt>
              <dd>
                <code className="device-id">{settings.thisDevice.deviceId}</code>
              </dd>
            </div>
            <div>
              <dt>Support</dt>
              <dd>
                <a href="mailto:hello@yumait.com.au">hello@yumait.com.au</a>
              </dd>
            </div>
            <div>
              <dt>Privacy</dt>
              <dd>
                <a href="/privacy">Privacy notice and honest limits</a>
              </dd>
            </div>
          </dl>

          <form className="panel form-grid" onSubmit={onDiscovery}>
            <fieldset className="span-2">
              <legend>Discovery preset</legend>
              <label className="choice">
                <input type="radio" name="preset" checked={preset === 'lan'} onChange={() => setPreset('lan')} />
                LAN only
              </label>
              <label className="choice">
                <input type="radio" name="preset" checked={preset === 'global'} onChange={() => setPreset('global')} />
                Defaults (LAN plus global discovery)
              </label>
              <label className="choice">
                <input
                  type="radio"
                  name="preset"
                  checked={preset === 'tailscale'}
                  onChange={() => setPreset('tailscale')}
                />
                Tailscale only
              </label>
            </fieldset>
            {preset === 'tailscale' ? (
              <label className="span-2">
                Tailscale listen address
                <input value={tailscale} onChange={(event) => setTailscale(event.target.value)} required />
              </label>
            ) : null}
            <div className="form-actions span-2">
              <button type="submit" className="btn primary" disabled={busy || settings.capabilities?.editSettings === false}>
                Apply discovery
              </button>
            </div>
          </form>

          <div className="panel form-grid">
            <label className="choice span-2">
              <input
                type="checkbox"
                checked={Boolean(settings.versionCheck)}
                onChange={(event) =>
                  void run(() => apiSend('/api/settings', 'PATCH', { versionCheck: event.target.checked }))
                }
              />
              Check yumaitau/BlakSync releases for a newer version (off by default)
            </label>
            <label className="choice span-2">
              <input
                type="checkbox"
                checked={Boolean(settings.startAtLogin)}
                onChange={(event) =>
                  void run(() => apiSend('/api/settings/autostart', 'POST', { enabled: event.target.checked }))
                }
              />
              Start at login (tray on Windows and macOS; user systemd unit on Linux)
            </label>
          </div>

          <section className="panel">
            <h2 className="section-title">If this device leaked</h2>
            <ol>
              <li>Stop BlakSync. Delete cert.pem and key.pem in the config directory only if this identity leaked.</li>
              <li>Start again. The new device ID must be paired on every remaining machine.</li>
              <li>If only the GUI API key leaked, replace the value inside &lt;apikey&gt; in config.xml and restart.</li>
              <li>Files already on a lost disk stay there. Full-disk encryption is the organisation&apos;s job.</li>
            </ol>
          </section>

          {settings.capabilities?.assignRoles ? (
            <section className="panel">
              <h2 className="section-title">Organisation roles</h2>
              <p className="lede">{settings.roleRule}</p>
              <ul className="row-list" aria-label="Members">
                {members.map((member) => (
                  <li key={member.id} className="row-card">
                    <div className="row-main">
                      <h3>
                        {member.name} <span className="muted">({member.id})</span>
                      </h3>
                      <p className="meta">Role: {member.role}</p>
                    </div>
                    <label className="inline-field">
                      <span className="sr-only">Role for {member.name}</span>
                      <select
                        value={member.role}
                        aria-label={`Role for ${member.name}`}
                        onChange={(event) =>
                          void run(() =>
                            apiSend(`/api/org/members/${encodeURIComponent(member.id)}`, 'PATCH', {
                              role: event.target.value,
                            }),
                          )
                        }
                      >
                        <option value="owner">owner</option>
                        <option value="admin">admin</option>
                        <option value="member">member</option>
                      </select>
                    </label>
                  </li>
                ))}
              </ul>
              <form className="form-grid" onSubmit={onAddMember}>
                <label>
                  Member ID
                  <input value={newId} onChange={(event) => setNewId(event.target.value)} required autoComplete="off" />
                </label>
                <label>
                  Name
                  <input value={newName} onChange={(event) => setNewName(event.target.value)} required />
                </label>
                <label>
                  Role
                  <select value={newRole} onChange={(event) => setNewRole(event.target.value)}>
                    <option value="admin">admin</option>
                    <option value="member">member</option>
                    <option value="owner">owner</option>
                  </select>
                </label>
                <div className="form-actions span-2">
                  <button type="submit" className="btn primary" disabled={busy}>
                    Add member
                  </button>
                </div>
              </form>
            </section>
          ) : (
            <p className="empty-state">Members cannot open the role editor.</p>
          )}
        </>
      ) : null}
    </section>
  )
}
