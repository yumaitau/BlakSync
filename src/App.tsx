import { NavLink, Route, Routes } from 'react-router-dom'
import FoldersPage from './pages/FoldersPage'
import ThisDevicePage from './pages/ThisDevicePage'
import RemoteDevicesPage from './pages/RemoteDevicesPage'
import PendingPage from './pages/PendingPage'
import SettingsPage from './pages/SettingsPage'
import './App.css'

function App() {
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
          <NavLink to="/pending">Pending</NavLink>
          <NavLink to="/settings">Settings</NavLink>
        </nav>
      </header>
      <main id="main-content" tabIndex={-1} className="app-main">
        <Routes>
          <Route path="/" element={<FoldersPage />} />
          <Route path="/this-device" element={<ThisDevicePage />} />
          <Route path="/devices" element={<RemoteDevicesPage />} />
          <Route path="/pending" element={<PendingPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </main>
    </div>
  )
}

export default App
