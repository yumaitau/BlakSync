import { useEffect, useState } from 'react'
import { apiGet, type AuditRow } from '../api'

export default function AuditPage() {
  const [rows, setRows] = useState<AuditRow[]>([])
  const [error, setError] = useState('')

  useEffect(() => {
    apiGet<AuditRow[]>('/api/audit')
      .then(setRows)
      .catch((err: Error) => setError(err.message))
  }, [])

  return (
    <section className="page">
      <div className="page-head">
        <div>
          <h1>Audit</h1>
          <p className="lede">
            Local events only: accepts, shares, unshares, revokes, and role changes. No file paths or file contents.
          </p>
        </div>
        <a className="btn" href="/api/audit.csv">
          Export CSV
        </a>
      </div>
      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      {rows.length === 0 && !error ? <p className="empty-state">No audit events yet.</p> : null}
      {rows.length > 0 ? (
        <div className="table-wrap panel">
          <table>
            <caption className="sr-only">Audit events</caption>
            <thead>
              <tr>
                <th scope="col">Time</th>
                <th scope="col">Event</th>
                <th scope="col">Actor</th>
                <th scope="col">Role</th>
                <th scope="col">Device</th>
                <th scope="col">Folder</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={`${row.timestamp}-${index}`}>
                  <td>{row.timestamp}</td>
                  <td>{row.event}</td>
                  <td>{row.actor}</td>
                  <td>{row.role}</td>
                  <td>{row.device_id || '—'}</td>
                  <td>{row.folder_label || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  )
}
