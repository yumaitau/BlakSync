import { useEffect, useState } from 'react'
import type { FormEvent, KeyboardEvent } from 'react'
import { apiGet, apiSend } from '../api'

const STEPS = ['Organisation', 'This device', 'Who may pair', 'Discovery']

type SetupInfo = {
  needed: boolean
  timezones: string[]
}

export default function Wizard({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState(0)
  const [timezones, setTimezones] = useState<string[]>(['Australia/Darwin'])
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [name, setName] = useState('')
  const [timezone, setTimezone] = useState('Australia/Darwin')
  const [contact, setContact] = useState('')
  const [deviceName, setDeviceName] = useState('')
  const [preset, setPreset] = useState('global')
  const [tailscaleListen, setTailscaleListen] = useState('')

  useEffect(() => {
    apiGet<SetupInfo>('/api/setup')
      .then((data) => {
        if (data.timezones?.length) setTimezones(data.timezones)
      })
      .catch((err: Error) => setError(err.message))
  }, [])

  function onKey(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
    }
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault()
    if (step < STEPS.length - 1) {
      setStep((value) => value + 1)
      return
    }
    setBusy(true)
    setError('')
    try {
      await apiSend('/api/setup', 'POST', {
        name,
        timezone,
        contact,
        deviceName,
        discoveryPreset: preset,
        tailscaleListen: preset === 'tailscale' ? tailscaleListen : undefined,
        actorId: 'owner',
        actorName: 'Owner',
      })
      onDone()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="page wizard" onKeyDown={onKey}>
      <h1>Set up this device</h1>
      <p className="lede">
        Name the organisation, this machine, and how other machines may find it. Nothing leaves this computer except
        the pairing choices you make next.
      </p>
      <ol className="wizard-steps" aria-label="Setup steps">
        {STEPS.map((label, index) => (
          <li key={label} aria-current={index === step ? 'step' : undefined}>
            {index + 1}. {label}
          </li>
        ))}
      </ol>
      {error ? (
        <p className="banner error" role="alert">
          {error}
        </p>
      ) : null}
      <form className="panel form-grid" onSubmit={onSubmit}>
        {step === 0 ? (
          <>
            <label className="span-2">
              Organisation name
              <input value={name} onChange={(event) => setName(event.target.value)} required autoComplete="organization" />
            </label>
            <label>
              Timezone
              <select value={timezone} onChange={(event) => setTimezone(event.target.value)} required>
                {timezones.map((zone) => (
                  <option key={zone} value={zone}>
                    {zone}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Support contact
              <input
                value={contact}
                onChange={(event) => setContact(event.target.value)}
                required
                autoComplete="email"
                placeholder="it@example.org.au"
              />
            </label>
          </>
        ) : null}
        {step === 1 ? (
          <label className="span-2">
            Name for this device
            <input
              value={deviceName}
              onChange={(event) => setDeviceName(event.target.value)}
              required
              autoComplete="off"
              placeholder="Office node"
            />
          </label>
        ) : null}
        {step === 2 ? (
          <p className="span-2 lede">
            Pairing a new device stays with the owner and admins. Members can sync folders that have already been
            shared. An admin cannot create a new owner.
          </p>
        ) : null}
        {step === 3 ? (
          <>
            <fieldset className="span-2">
              <legend>Discovery</legend>
              <label className="choice">
                <input type="radio" name="preset" checked={preset === 'lan'} onChange={() => setPreset('lan')} />
                LAN only — no global discovery or relays
              </label>
              <label className="choice">
                <input type="radio" name="preset" checked={preset === 'global'} onChange={() => setPreset('global')} />
                Defaults — LAN plus Syncthing global discovery
              </label>
              <label className="choice">
                <input
                  type="radio"
                  name="preset"
                  checked={preset === 'tailscale'}
                  onChange={() => setPreset('tailscale')}
                />
                Tailscale only — listen on the Tailscale address you type
              </label>
            </fieldset>
            {preset === 'tailscale' ? (
              <label className="span-2">
                Tailscale listen address
                <input
                  value={tailscaleListen}
                  onChange={(event) => setTailscaleListen(event.target.value)}
                  required
                  placeholder="tcp://100.64.0.1:22000"
                />
              </label>
            ) : null}
          </>
        ) : null}
        <div className="form-actions span-2">
          {step > 0 ? (
            <button type="button" className="btn" onClick={() => setStep((value) => value - 1)}>
              Back
            </button>
          ) : null}
          <button type="submit" className="btn primary" disabled={busy}>
            {step === STEPS.length - 1 ? 'Finish setup' : 'Continue'}
          </button>
        </div>
      </form>
    </section>
  )
}
