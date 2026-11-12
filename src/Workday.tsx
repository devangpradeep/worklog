import { Coffee, Pencil, Play, Square, Sun, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api, errorText, type TimeFormat, type TrackingAction, type TrackingState, type WorkdaySummary, type WorkSession } from './api'
import { formatDuration, formatRelativeTime } from './time'
import { ErrorNotice } from './ui'
import WorkSessionDialog from './WorkSessionDialog'

interface Props {
  date: string
  today: string
  revision: number
  tracking?: TrackingState
  format: TimeFormat
  onStartTask: () => void
}

export default function Workday({ date, today, revision, tracking, format, onStartTask }: Props) {
  const [history, setHistory] = useState<{ date: string; summary: WorkdaySummary }>()
  const [readError, setReadError] = useState('')
  const [actionError, setActionError] = useState('')
  const [busy, setBusy] = useState(false)
  const [dialog, setDialog] = useState<{ session: WorkSession; mode: 'edit' | 'delete' }>()
  const isToday = date === today
  const session = tracking?.session
  const entry = tracking?.entry
  const summary = history?.date === date ? history.summary : undefined
  const sessions = summary?.sessions ?? []
  const totals = summary?.totals
  const time = (value: number) => formatRelativeTime(value, date, format)
  const sessionHistory = sessions.filter(item => !isToday || item.endedAt !== null)

  useEffect(() => {
    let active = true
    void api.workdaySummary(date).then(summary => {
      if (active) {
        setHistory({ date, summary })
        setReadError('')
      }
    }).catch(error => {
      if (active) setReadError(errorText(error))
    })
    return () => { active = false }
  }, [date, revision])

  async function track(action: TrackingAction) {
    if (busy) return
    setBusy(true)
    setActionError('')
    try {
      await api.track(action)
    } catch (error) {
      setActionError(errorText(error))
    } finally {
      setBusy(false)
    }
  }

  if (!isToday && sessions.length === 0 && !readError) return null

  return <section className="workday" aria-label="Workday">
    <div className="workday-main">
      <div className="workday-summary">
        <div className="workday-icon"><Sun size={19} /></div>
        <div className="min-w-0">
          <h2>Workday</h2>
          {isToday && session && <div className="workday-session-start">
            <p>Started at {time(session.startedAt)}</p>
            <button type="button" className="icon-button session-action" aria-label="Edit running work session" title="Edit start time"
              onClick={() => setDialog({ session, mode: 'edit' })}><Pencil size={14} /></button>
          </div>}
          {isToday && entry && <p className="workday-active" title={entry.description}>
            {entry.kind === 'break' ? 'On break' : entry.description} · Since {time(entry.activityAt)}
          </p>}
        </div>
      </div>
      {isToday && <div className="workday-actions">
        {session
          ? <button className="button secondary" disabled={busy} onClick={() => void track({ kind: 'endDay', id: session.id })}><Square size={14} /> End day</button>
          : <button className="button primary" disabled={busy || !tracking} onClick={() => void track({ kind: 'startDay' })}><Sun size={15} /> Start day</button>}
        <button className={'button ' + (session && !entry ? 'primary' : 'secondary')} disabled={busy || !tracking} onClick={onStartTask}><Play size={15} /> Start task</button>
        {entry?.kind === 'activity' && <button className="button secondary" disabled={busy} onClick={() => void track({ kind: 'stopTask', id: entry.id })}><Square size={14} /> Stop task</button>}
        {entry?.kind === 'break'
          ? <button className="button secondary" disabled={busy} onClick={() => void track({ kind: 'endBreak', id: entry.id })}><Coffee size={16} /> End break</button>
          : <button className="button secondary" disabled={busy || !tracking} onClick={() => void track({ kind: 'startBreak' })}><Coffee size={16} /> Take break</button>}
      </div>}
    </div>
    {totals && <dl className="workday-totals" aria-label="Completed workday totals">
      <div>
        <dt>Time worked</dt>
        <dd>{formatDuration(totals.workedMs)}</dd>
      </div>
      <div>
        <dt>Breaks</dt>
        <dd>{formatDuration(totals.breakMs)}</dd>
      </div>
    </dl>}
    {sessionHistory.length > 0 && <ul className="workday-sessions" aria-label="Work sessions">
      {sessionHistory.map(item => {
        const range = time(item.startedAt) + ' – ' + (item.endedAt === null ? 'Open' : time(item.endedAt))
        return <li key={item.id}>
          <span>
            <time dateTime={new Date(item.startedAt).toISOString()}>{time(item.startedAt)}</time>
            {' – '}
            {item.endedAt === null ? 'Open' : <time dateTime={new Date(item.endedAt).toISOString()}>{time(item.endedAt)}</time>}
          </span>
          <button type="button" className="icon-button session-action" aria-label={'Edit work session ' + range} title="Edit work session"
            onClick={() => setDialog({ session: item, mode: 'edit' })}><Pencil size={14} /></button>
          {item.endedAt !== null && <button type="button" className="icon-button session-action delete-button" aria-label={'Delete work session ' + range}
            title="Delete work session" onClick={() => setDialog({ session: item, mode: 'delete' })}><Trash2 size={14} /></button>}
        </li>
      })}
    </ul>}
    {(actionError || readError) && <ErrorNotice>{actionError || readError}</ErrorNotice>}
    {dialog && <WorkSessionDialog {...dialog} formatTime={time} onDismiss={() => setDialog(undefined)} />}
  </section>
}
