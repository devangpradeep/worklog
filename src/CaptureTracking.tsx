import type { TimeFormat, TrackingAction, TrackingState } from './api'
import { formatRelativeTime, localDay } from './time'

interface Props {
  tracking: TrackingState
  format: TimeFormat
  disabled: boolean
  onAction: (action: TrackingAction) => void
  onStartTask?: () => void
}

export default function CaptureTracking({ tracking, format, disabled, onAction, onStartTask }: Props) {
  const { session, entry } = tracking
  const time = (timestamp: number) => formatRelativeTime(timestamp, localDay(), format)
  const onBreak = entry?.kind === 'break'

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
      <div className="capture-tracking-row">
        <div className="capture-tracking-status" aria-live="polite">
          <h2>
            <span className="capture-status-dot" data-state={entry?.kind ?? 'idle'} aria-hidden="true" />
            {onBreak ? 'On break' : 'Current task'}
          </h2>
          <p title={entry && !onBreak ? entry.description : undefined}>
            {onBreak && entry ? 'Since ' + time(entry.activityAt) : entry?.description ?? 'No task running'}
          </p>
        </div>
        {entry ? (
          <button type="button" className="button secondary" disabled={disabled} onClick={() =>
            onAction({ kind: onBreak ? 'endBreak' : 'stopTask', id: entry.id })
          }>{onBreak ? 'End break' : 'Stop task'}</button>
        ) : onStartTask && (
          <div className="capture-tracking-actions">
            {session && <button type="button" className="button secondary" disabled={disabled} onClick={() =>
              onAction({ kind: 'startBreak' })
            }>Take break</button>}
            <button type="button" className="button secondary" disabled={disabled} onClick={onStartTask}>Start task</button>
          </div>
        )}
      </div>
    </section>
  )
}
