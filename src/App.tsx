import { useEffect, useState } from 'react'
import { NavLink, Route, Routes } from 'react-router-dom'
import FoldersPage from './pages/FoldersPage'
import ThisDevicePage from './pages/ThisDevicePage'
import RemoteDevicesPage from './pages/RemoteDevicesPage'
import PendingPage from './pages/PendingPage'
import SettingsPage from './pages/SettingsPage'
import AuditPage from './pages/AuditPage'
import HealthPage from './pages/HealthPage'
import PrivacyPage from './pages/PrivacyPage'
import Wizard from './components/Wizard'
import { apiGet, type Overview, type UpdateInfo } from './api'
import './App.css'

function App() {
  const [setupNeeded, setSetupNeeded] = useState<boolean | null>(null)
  const [pendingCount, setPendingCount] = useState(0)
  const [update, setUpdate] = useState<UpdateInfo | null>(null)

  async function loadShell() {
    const setup = await apiGet<{ needed: boolean }>('/api/setup')
    setSetupNeeded(setup.needed)
    if (setup.needed) return
    const overview = await apiGet<Overview>('/api/overview')
    setPendingCount(overview.pending.devices.length)
    if (overview.versionCheck) {
      const info = await apiGet<UpdateInfo>('/api/updates')
      setUpdate(info)
    } else {
      setUpdate(null)
    }
  }

  useEffect(() => {
    loadShell().catch(() => setSetupNeeded(false))
  }, [])

  if (setupNeeded) {
    return (
      <div className="app-shell">
        <main id="main-content" tabIndex={-1} className="app-main">
          <Wizard onDone={() => setSetupNeeded(false)} />
        </main>
      </div>
    )
  }

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <header className="app-header">
        <div className="app-header-inner">
          <div className="brand-block">
            <NavLink to="/" className="brand" aria-label="BlakSync folders">
              <span className="brand-mark" aria-hidden="true" />
              <span className="brand-name">
                <span>Blak</span>Sync
              </span>
            </NavLink>
            <p className="brand-tag">Local folder sync. No cloud in the middle.</p>
          </div>
          <div className="app-context" aria-label="Connection model">
            <span className="local-status">
              <span aria-hidden="true" />
              Local only
            </span>
            <span className="context-copy">Community-controlled sync</span>
          </div>
        </div>
        <nav className="app-nav" aria-label="Primary">
          <NavLink to="/" end>
            Folders
          </NavLink>
          <NavLink to="/this-device">This device</NavLink>
          <NavLink to="/devices">Remote devices</NavLink>
          <NavLink to="/pending">
            Pending{pendingCount ? ` (${pendingCount})` : ''}
          </NavLink>
          <NavLink to="/health">Health</NavLink>
          <NavLink to="/audit">Audit</NavLink>
          <NavLink to="/settings">Settings</NavLink>
        </nav>
      </header>
      {update?.newer ? (
        <p className="banner update-banner" role="status">
          A newer BlakSync is available: {update.current} → {update.latest}. See the release SOP. No files or analytics
          leave this check.
          {update.releaseUrl ? (
            <>
              {' '}
              <a href={update.releaseUrl} rel="noreferrer">
                Open the release
              </a>
            </>
          ) : null}
        </p>
      ) : null}
      <main id="main-content" tabIndex={-1} className="app-main">
        <Routes>
          <Route path="/" element={<FoldersPage />} />
          <Route path="/this-device" element={<ThisDevicePage />} />
          <Route path="/devices" element={<RemoteDevicesPage />} />
          <Route path="/pending" element={<PendingPage />} />
          <Route path="/health" element={<HealthPage />} />
          <Route path="/audit" element={<AuditPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/privacy" element={<PrivacyPage />} />
        </Routes>
      </main>
    </div>
  )
}

export default App
