import { NavLink, Route, Routes } from 'react-router-dom'
import DropZonePage from './pages/DropZonePage'
import LibraryPage from './pages/LibraryPage'
import ReceivePage from './pages/ReceivePage'
import './App.css'

function App() {
  return (
    <div className="app-shell">
      <header className="app-header">
        <NavLink to="/" className="brand">
          BlakSync
        </NavLink>
        <nav className="app-nav" aria-label="Primary">
          <NavLink to="/" end>
            Send
          </NavLink>
          <NavLink to="/library">Library</NavLink>
        </nav>
      </header>
      <main className="app-main">
        <Routes>
          <Route path="/" element={<DropZonePage />} />
          <Route path="/receive/:room" element={<ReceivePage />} />
          <Route path="/library" element={<LibraryPage />} />
        </Routes>
      </main>
    </div>
  )
}

export default App
