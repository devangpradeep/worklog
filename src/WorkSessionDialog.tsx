import { useState, type FormEvent } from 'react'
import { api, errorText, type WorkSession } from './api'
import { localDateTime, timestampFromInput } from './time'
import { ErrorNotice, Modal } from './ui'

interface Props {
  session: WorkSession
  mode: 'edit' | 'delete'
  formatTime: (value: number) => string
  onDismiss: () => void
}

export default function WorkSessionDialog({ session, mode, formatTime, onDismiss }: Props) {
  const [start, setStart] = useState(() => localDateTime(new Date(session.startedAt)))
  const [end, setEnd] = useState(() => session.endedAt === null ? '' : localDateTime(new Date(session.endedAt)))
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const deleting = mode === 'delete'

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (busy) return
    setBusy(true)
    setError('')
    try {
      if (deleting) {
        await api.deleteWorkSession(session)
      } else {
        const startAt = timestampFromInput(start, session.startedAt)
        const endAt = session.endedAt === null ? null : timestampFromInput(end, session.endedAt)
        await api.updateWorkSession(session, startAt, endAt)
      }
      onDismiss()
    } catch (error) {
      setError(errorText(error))
      setBusy(false)
    }
  }

  function dismiss() {
    if (!busy) onDismiss()
  }

  return <Modal title={deleting ? 'Delete work session?' : 'Edit work session'} onDismiss={dismiss}>
    <form className="stack-form" onSubmit={submit}>
      {deleting
        ? <p>{formatTime(session.startedAt)}{session.endedAt !== null && ' – ' + formatTime(session.endedAt)}</p>
        : <div className="form-grid">
          <label className="field">Start date and time
            <input autoFocus required type="datetime-local" step={1} value={start} onChange={event => setStart(event.target.value)} />
          </label>
          {session.endedAt !== null && <label className="field">End date and time
            <input required type="datetime-local" step={1} value={end} onChange={event => setEnd(event.target.value)} />
          </label>}
        </div>}
      {error && <ErrorNotice>{error}</ErrorNotice>}
      <div className="modal-actions">
        <button autoFocus={deleting} type="button" className="button secondary" disabled={busy} onClick={dismiss}>Cancel</button>
        <button type="submit" className={'button ' + (deleting ? 'danger' : 'primary')} disabled={busy}>
          {deleting ? 'Delete session' : 'Save changes'}
        </button>
      </div>
    </form>
  </Modal>
}
