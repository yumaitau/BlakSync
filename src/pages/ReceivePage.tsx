import { useParams } from 'react-router-dom'

export default function ReceivePage() {
  const { room } = useParams()

  return (
    <section className="page" aria-labelledby="receive-heading">
      <h1 id="receive-heading">Receive transfer</h1>
      <p className="lede">
        Waiting room stub for peer handshakes. No signalling or data channel
        yet.
      </p>
      <p className="room-meta">
        Room code: <code>{room ?? 'unknown'}</code>
      </p>
    </section>
  )
}
