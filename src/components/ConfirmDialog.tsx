import type { FormEvent, ReactNode } from 'react'

type ConfirmDialogProps = {
  title: string
  children: ReactNode
  confirmLabel: string
  danger?: boolean
  typedValue?: string
  expected?: string
  onCancel: () => void
  onConfirm: () => void
}

export default function ConfirmDialog({
  title,
  children,
  confirmLabel,
  danger,
  typedValue,
  expected,
  onCancel,
  onConfirm,
}: ConfirmDialogProps) {
  const typedOk = expected === undefined || typedValue?.trim() === expected

  function onSubmit(event: FormEvent) {
    event.preventDefault()
    if (typedOk) onConfirm()
  }

  return (
    <div className="dialog-backdrop" role="presentation">
      <form className="dialog panel" role="dialog" aria-modal="true" aria-labelledby="dialog-title" onSubmit={onSubmit}>
        <h2 id="dialog-title">{title}</h2>
        <div className="dialog-body">{children}</div>
        <div className="form-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className={`btn ${danger ? 'danger' : 'primary'}`} disabled={!typedOk}>
            {confirmLabel}
          </button>
        </div>
      </form>
    </div>
  )
}
