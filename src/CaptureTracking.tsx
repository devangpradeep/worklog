import type { TimeFormat, TrackingAction, TrackingState, WorkdaySummary } from './api'
import { formatDuration, formatRelativeTime, localDay } from './time'

interface Props {
  tracking: TrackingState
  summary?: WorkdaySummary
  format: TimeFormat
  disabled: boolean
  onAction: (action: TrackingAction) => void
  onStartTask?: () => void
}

export default function CaptureTracking({ tracking, summary, format, disabled, onAction, onStartTask }: Props) {
  const { session, tasks, activeBreak } = tracking
  const today = localDay()
  const time = (timestamp: number) => formatRelativeTime(timestamp, today, format)
  const totals = summary?.totals

  return (
    <section className="capture-tracking" aria-label="Workday controls">
      <div className="capture-tracking-row">
        <div className="capture-tracking-status" aria-live="polite">
          <h2>Workday</h2>
          <p>{session ? 'Started at ' + time(session.startedAt) : 'Day not started'}</p>
        </div>
        <button type="button" className="button secondary" disabled={disabled} onClick={() =>
          onAction(session ? { kind: 'endDay', id: session.id } : { kind: 'startDay' })
        }>{session ? 'End day' : 'Start day'}</button>
      </div>
      {activeBreak && <div className="capture-tracking-row">
        <div className="capture-tracking-status">
          <h2><span className="tracking-dot" data-state="break" aria-hidden="true" />On break</h2>
          <p>Since {time(activeBreak.activityAt)}</p>
        </div>
        <button type="button" className="button secondary" disabled={disabled} onClick={() =>
          onAction({ kind: 'endBreak', id: activeBreak.id })
        }>End break</button>
      </div>}
      {tasks.map(task => <div className="capture-tracking-row" key={task.id}>
        <div className="capture-tracking-status">
          <h2>
            <span className="tracking-dot" data-state="activity" aria-hidden="true" />
            <span title={task.description}>{task.description}</span>
          </h2>
          <p>Running since {time(task.activityAt)}</p>
        </div>
        <div className="capture-tracking-actions">
          <button type="button" className="button secondary" disabled={disabled} aria-label={'Stop task: ' + task.description} onClick={() =>
            onAction({ kind: 'stopTask', id: task.id })
          }>Stop task</button>
          {tasks.length === 1 && onStartTask && <button type="button" className="button secondary" disabled={disabled}
            onClick={onStartTask}>Start second task</button>}
        </div>
      </div>)}
      {!activeBreak && tasks.length === 0 && <div className="capture-tracking-row">
        <div className="capture-tracking-status"><h2>No task running</h2></div>
        {onStartTask && <div className="capture-tracking-actions">
          {session && <button type="button" className="button secondary" disabled={disabled} onClick={() =>
            onAction({ kind: 'startBreak' })
          }>Take break</button>}
          <button type="button" className="button secondary" disabled={disabled} onClick={onStartTask}>Start task</button>
        </div>}
      </div>}
      {summary && <dl className="capture-totals" aria-label="Workday totals">
        <div>
          <dt>Time worked</dt>
          <dd>{formatDuration(totals?.workedMs ?? 0)}</dd>
        </div>
        <div>
          <dt>Breaks</dt>
          <dd>{totals?.completedBreaks ?? 0} completed · {formatDuration(totals?.breakMs ?? 0)} total</dd>
        </div>
      </dl>}
    </section>
  )
}
