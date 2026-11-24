import { ArrowLeft, Check, CornerDownLeft, History, LayoutDashboard, TriangleAlert, X } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { api, errorText, type CaptureContext, type Entry, type TrackingAction } from './api'
import { useData, useNativeEvent, useToday, useWorkdaySummary } from './hooks'
import { dateTimeInput, localDay, timestampFromInput, type DateTimeInput } from './time'
import { Brand, CategoryOptions } from './ui'
import CaptureTracking from './CaptureTracking'
import RecentTasks from './RecentTasks'
import DateTimeFields from './DateTimeFields'

interface Draft {
  description: string
  categoryId?: number | null
  key?: string
  start?: 'now' | '15m' | 'custom'
  startedAt?: DateTimeInput
}

const storageKey = 'worklog.task.draft'
const emptyDraft: Draft = { description: '' }

function readDraft(): Draft {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(storageKey) ?? 'null')
    if (value && typeof value === 'object' && 'description' in value && typeof value.description === 'string') {
      return value as Draft
    }
  } catch (error) {
    console.error(error)
  }
  return emptyDraft
}

function createKey() {
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, '0')).join('')
}

export default function Capture() {
  const { snapshot, revision, error: dataError, refresh } = useData()
  const [draft, setDraft] = useState(readDraft)
  const [context, setContext] = useState<CaptureContext>()
  const latestContext = useRef<CaptureContext | undefined>(undefined)
  const taskMode = context?.mode === 'task'
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [saved, setSaved] = useState(false)
  const [changingTracking, setChangingTracking] = useState(false)
  const [showRecent, setShowRecent] = useState(false)
  const controlsDisabled = !context || busy || saved || changingTracking
  const running = useRef(false)
  const restartRequest = useRef<{ sourceId: number; key: string } | null>(null)
  const recentBack = useRef<HTMLButtonElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const timer = useRef<number | undefined>(undefined)

  const today = useToday()
  const session = snapshot?.tracking.session
  const date = session ? localDay(new Date(session.startedAt)) : today
  const workday = useWorkdaySummary(date, revision, !!snapshot, !!session)
  const taskCount = snapshot?.tracking.tasks.length ?? 0
  const taskLimit = taskMode && taskCount >= 2
  const showTaskNotice = taskMode && taskCount > 0 && !saved
  const startLabel = taskCount === 1 ? 'Start second task' : 'Start task'

  const defaultCategory = snapshot?.captureCategory ?? null
  const desiredCategory = draft.categoryId === undefined ? defaultCategory : draft.categoryId
  const categoryId = snapshot?.categories.some(category => category.isActive && category.id === desiredCategory)
    ? desiredCategory : null
  const settings = snapshot?.settings
  const startChoice = draft.start ?? 'now'

  function update(value: Draft) {
    setDraft(value)
    try {
      if (value.description) {
        localStorage.setItem(storageKey, JSON.stringify(value))
      } else {
        localStorage.removeItem(storageKey)
      }
    } catch (error) {
      console.error(error)
    }
  }

  function onOpened(next: CaptureContext) {
    if (latestContext.current && next.generation <= latestContext.current.generation) return
    latestContext.current = next
    setContext(next)
    window.clearTimeout(timer.current)
    setSaved(false)
    setError('')
    setShowRecent(false)
    input.current?.focus()
  }

  useNativeEvent<CaptureContext>('capture-opened', onOpened, () => {
    void api.captureContext().then(onOpened).catch(error => setError(errorText(error)))
  })
  useEffect(() => {
    const focus = () => input.current?.focus()
    window.addEventListener('focus', focus)
    focus()
    return () => {
      window.removeEventListener('focus', focus)
      window.clearTimeout(timer.current)
    }
  }, [])

  useEffect(() => {
    if (showRecent) recentBack.current?.focus()
  }, [showRecent])

  async function dismiss() {
    try {
      const { generation } = context ?? await api.captureContext()
      await api.hideCapture(generation)
    } catch (error) {
      setError(errorText(error))
    }
  }

  async function runControl(action: () => Promise<void>) {
    if (controlsDisabled || running.current) return
    running.current = true
    setChangingTracking(true)
    setError('')
    try {
      await action()
    } catch (error) {
      setError(errorText(error))
    } finally {
      running.current = false
      setChangingTracking(false)
      requestAnimationFrame(() => (input.current ?? recentBack.current)?.focus())
    }
  }

  function restartTask(task: Entry) {
    if (!context || taskCount >= 2) return
    const { generation } = context
    void runControl(async () => {
      const key = restartRequest.current?.sourceId === task.id ? restartRequest.current.key : createKey()
      restartRequest.current = { sourceId: task.id, key }
      await api.restartTask(task.id, key)
      restartRequest.current = null
      if (latestContext.current?.generation === generation) setShowRecent(false)
    })
  }

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!taskMode || !context || !settings || !draft.description.trim() || running.current || saved || changingTracking || taskLimit) return
    running.current = true
    setBusy(true)
    setError('')
    const captureKey = draft.key ?? createKey()
    update({ ...draft, categoryId, key: captureKey })
    try {
      const { generation } = context
      let startedAt: number | null = null
      if (startChoice === '15m') startedAt = Date.now() - 15 * 60_000
      if (startChoice === 'custom' && draft.startedAt) {
        startedAt = timestampFromInput(draft.startedAt, settings.timeFormat, 'start')
      }
      await api.startTask(draft.description, categoryId, captureKey, startedAt)
      update(emptyDraft)
      if (latestContext.current?.generation !== generation) return
      if (settings.showConfirmation) setSaved(true)
      const finish = () => {
        setSaved(false)
        const action = settings.hideAfterSave ? api.hideCapture(generation) : api.openCapture('quick', generation)
        void action.catch(error => setError(errorText(error)))
      }
      if (settings.showConfirmation) {
        timer.current = window.setTimeout(finish, settings.hideAfterSave ? 520 : 1600)
      } else {
        finish()
      }
    } catch (error) {
      setError(errorText(error))
    } finally {
      running.current = false
      setBusy(false)
      requestAnimationFrame(() => input.current?.focus())
    }
  }

  const recentTasksButton = <button type="button" className="button capture-header-button" disabled={controlsDisabled} onClick={() => {
    setError('')
    setShowRecent(true)
  }}><History size={14} aria-hidden="true" /> Recent tasks</button>

  return (
    <main className="capture-shell" onKeyDown={event => {
      if (event.key === 'Escape') {
        event.preventDefault()
        if (showRecent) setShowRecent(false)
        else void dismiss()
      }
      if (event.key === 'Enter' && event.repeat) event.preventDefault()
    }}>
      <header className="capture-header">
        {showRecent
          ? <button type="button" className="icon-button" ref={recentBack} aria-label="Back to capture" onClick={() => setShowRecent(false)}><ArrowLeft size={16} /></button>
          : <Brand small />}
        <h1>{showRecent ? 'Recent tasks' : taskMode ? 'Start task' : 'Quick Capture'}</h1>
        <div className="capture-header-actions">
          {taskMode && !showRecent && <button type="button" className="button capture-header-button" disabled={controlsDisabled}
            onClick={() => void runControl(() => api.openCapture('quick'))}><ArrowLeft size={14} /> Back</button>}
          {!taskMode && !showRecent && recentTasksButton}
          <button type="button" className="button capture-header-button" disabled={controlsDisabled}
            onClick={() => context && void runControl(() => api.openDashboard(context.generation))}>
            <LayoutDashboard size={14} aria-hidden="true" /> Open dashboard
          </button>
          <button type="button" className="icon-button" aria-label="Dismiss Quick Capture" title="Dismiss (Esc)" onClick={() => void dismiss()}><X size={17} /></button>
        </div>
      </header>
      {showRecent && <RecentTasks key={today} categories={snapshot?.categories ?? []} format={settings?.timeFormat ?? '24h'}
        revision={revision} disabled={controlsDisabled} runningCount={taskCount} onBreak={!!snapshot?.tracking.activeBreak} onStart={restartTask} />}
      {taskMode && !showRecent && <form className="capture-form" onSubmit={submit}>
        <div className="flex items-stretch gap-2">
          <input
            autoFocus ref={input} aria-label="Task" placeholder="What are you working on?"
            aria-describedby={showTaskNotice ? 'capture-task-notice' : undefined}
            value={draft.description} readOnly={busy || saved} maxLength={4000}
            onChange={event => {
              setError('')
              update({ ...draft, description: event.target.value, key: undefined })
            }}
          />
          <button className="button primary capture-save" disabled={controlsDisabled || taskLimit || !settings || !draft.description.trim()}>
            {saved ? <Check size={16} /> : <CornerDownLeft size={16} />}
            {saved ? 'Started' : busy ? 'Starting…' : startLabel}
          </button>
        </div>
        <div className="capture-options">
          <div className="capture-entry-options">
            <label htmlFor="capture-category" className="sr-only">Optional category</label>
            <select id="capture-category" value={categoryId ?? ''} disabled={!snapshot || busy} onChange={event =>
              update({ ...draft, categoryId: event.target.value ? Number(event.target.value) : null, key: undefined })
            }>
              <option value="">No category</option>
              <CategoryOptions categories={snapshot?.categories ?? []} />
            </select>
            {recentTasksButton}
          </div>
          <span className="capture-hint"><kbd>Enter</kbd> to start <span aria-hidden="true">·</span> <kbd>Esc</kbd> to hide</span>
        </div>
        <div className="capture-start-time">
          <label htmlFor="capture-start">Started</label>
          <select id="capture-start" value={startChoice} disabled={controlsDisabled} onChange={event => {
            const start = event.target.value as Draft['start']
            update({ ...draft, start, key: undefined, startedAt: draft.startedAt ?? dateTimeInput(new Date(), settings?.timeFormat ?? '24h') })
            setError('')
          }}>
            <option value="now">Now</option>
            <option value="15m">15 minutes ago</option>
            <option value="custom">Custom time…</option>
          </select>
        </div>
        {startChoice === 'custom' && draft.startedAt && <DateTimeFields label="Start"
          value={draft.startedAt} format={settings?.timeFormat ?? '24h'} onChange={startedAt => {
            update({ ...draft, startedAt, key: undefined })
            setError('')
          }} />}
        {showTaskNotice && <p className="capture-task-notice" id="capture-task-notice" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          {taskLimit ? 'Two tasks are running. Stop one to start another.' : 'Your current task will keep running.'}
        </p>}
        {saved && <span className="sr-only" role="status">Task started</span>}
      </form>}
      {!showRecent && snapshot && <CaptureTracking
        tracking={snapshot.tracking}
        summary={workday?.summary}
        format={snapshot.settings.timeFormat}
        disabled={controlsDisabled}
        onAction={(action: TrackingAction) => void runControl(() => api.track(action))}
        onStartTask={taskMode ? undefined : () => void runControl(() => api.openCapture('task'))}
      />}
      {(error || dataError || workday?.error) && <div className="capture-error" role="alert">
        {error || dataError || workday?.error} {(dataError || workday?.error) && <button type="button" onClick={refresh}>Retry</button>}
      </div>}
    </main>
  )
}
