export default function LibraryPage() {
  return (
    <section className="page" aria-labelledby="library-heading">
      <h1 id="library-heading">Organisation library</h1>
      <p className="lede">
        Encrypted store on a device the organisation owns. Empty for now; no
        storage backend in this scaffold.
      </p>
      <p className="empty-state">No items yet.</p>
    </section>
  )
}
