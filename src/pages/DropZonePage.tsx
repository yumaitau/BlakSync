export default function DropZonePage() {
  return (
    <section className="page" aria-labelledby="drop-heading">
      <h1 id="drop-heading">Drop a file to share</h1>
      <p className="lede">
        Transfers stay between devices. Signalling and encryption land in later
        tickets; this screen is the send stub.
      </p>
      <div
        className="drop-zone"
        role="region"
        aria-label="File drop zone"
        tabIndex={0}
      >
        <p className="drop-zone-label">Drop files or folders here</p>
        <p className="drop-zone-hint">
          Or choose a file when pickers are wired up. WebRTC is not implemented
          yet.
        </p>
      </div>
    </section>
  )
}
