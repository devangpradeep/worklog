import { useState, type FormEvent } from 'react'
import { api, errorText, type TimeFormat, type WorkSession } from './api'
import DateTimeFields from './DateTimeFields'
import { dateTimeInput, timestampFromInput } from './time'
import { ErrorNotice, Modal } from './ui'

interface Props {
  session: WorkSession
  mode: 'edit' | 'delete'
  format: TimeFormat
  formatTime: (value: number) => string
  onDismiss: () => void
}

export default function WorkSessionDialog({ session, mode, format, formatTime, onDismiss }: Props) {
  const [start, setStart] = useState(() => dateTimeInput(new Date(session.startedAt), format))
  const [end, setEnd] = useState(() => session.endedAt === null ? null : dateTimeInput(new Date(session.endedAt), format))
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
        const startAt = timestampFromInput(start, format, 'start', session.startedAt)
        const endAt = end ? timestampFromInput(end, format, 'end', session.endedAt ?? undefined) : null
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
        : <>
          <DateTimeFields label="Start" value={start} format={format} focusTime onChange={value => {
            setStart(value)
            setError('')
          }} />
          {end && <DateTimeFields label="End" value={end} format={format} onChange={value => {
            setEnd(value)
            setError('')
          }} />}
        </>}
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
