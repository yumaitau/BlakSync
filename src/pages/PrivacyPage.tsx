import { useEffect, useState } from 'react'
import { apiGet } from '../api'

type Privacy = {
  title: string
  notice: string
  support: string
  fileBytesStoredByYuma: boolean
  discoveryIsNotSecret: boolean
}

export default function PrivacyPage() {
  const [privacy, setPrivacy] = useState<Privacy | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    apiGet<Privacy>('/api/privacy')
      .then(setPrivacy)
      .catch((err: Error) => setError(err.message))
  }, [])

  return (
    <section className="page">
      <h1>Privacy and support</h1>
      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      {privacy ? (
        <article className="panel privacy">
          <p className="lede">{privacy.notice}</p>
          <p>
            File bytes stored by Yuma: {privacy.fileBytesStoredByYuma ? 'yes' : 'no'}.
          </p>
          <p>
            Discovery and relay metadata is a secret from those services:{' '}
            {privacy.discoveryIsNotSecret ? 'no' : 'yes'}.
          </p>
          <p>
            Help: <a href={`mailto:${privacy.support}`}>{privacy.support}</a>
          </p>
        </article>
      ) : null}
    </section>
  )
}
