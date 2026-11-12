import { ArrowLeft, Check, CornerDownLeft, X } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { api, errorText, type CaptureContext, type CaptureMode, type TrackingAction } from './api'
import { useData, useNativeEvent } from './hooks'
import { Brand, CategoryOptions } from './ui'
import CaptureTracking from './CaptureTracking'

interface Draft {
  description: string
  categoryId?: number | null
  key?: string
}

const storageKey = (mode: CaptureMode) => mode === 'task' ? 'worklog.task.draft' : 'worklog.capture.draft'
const emptyDraft: Draft = { description: '' }

function readDraft(mode: CaptureMode): Draft {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(storageKey(mode)) ?? 'null')
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
  const { snapshot, error: dataError, refresh } = useData()
  const [drafts, setDrafts] = useState(() => ({ quick: readDraft('quick'), task: readDraft('task') }))
  const [context, setContext] = useState<CaptureContext>()
  const latestContext = useRef<CaptureContext | undefined>(undefined)
  const mode = context?.mode ?? 'quick'
  const taskMode = mode === 'task'
  const draft = drafts[mode]
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [saved, setSaved] = useState(false)
  const [changingTracking, setChangingTracking] = useState(false)
  const controlsDisabled = !context || busy || saved || changingTracking
  const running = useRef(false)
  const input = useRef<HTMLInputElement>(null)
  const timer = useRef<number | undefined>(undefined)

  const defaultCategory = snapshot?.captureCategory ?? null
  const desiredCategory = draft.categoryId === undefined ? defaultCategory : draft.categoryId
  const categoryId = snapshot?.categories.some(category => category.isActive && category.id === desiredCategory)
    ? desiredCategory : null
  const settings = snapshot?.settings

  function update(value: Draft, target = mode) {
    setDrafts(drafts => ({ ...drafts, [target]: value }))
    try {
      if (value.description) {
        localStorage.setItem(storageKey(target), JSON.stringify(value))
      } else {
        localStorage.removeItem(storageKey(target))
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

  async function dismiss() {
    try {
      await api.hideCapture((context ?? await api.captureContext()).generation)
    } catch (error) {
      setError(errorText(error))
    }
  }

  async function runControl(action: () => Promise<void>) {
    if (controlsDisabled || running.current) return
    setChangingTracking(true)
    setError('')
    try {
      await action()
    } catch (error) {
      setError(errorText(error))
    } finally {
      setChangingTracking(false)
      input.current?.focus()
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (!context || !settings || !draft.description.trim() || running.current || saved || changingTracking) return
    running.current = true
    setBusy(true)
    setError('')
    const captureKey = draft.key ?? createKey()
    update({ description: draft.description, categoryId, key: captureKey })
    try {
      const { generation, mode } = context
      await api.capture(draft.description, categoryId, captureKey, mode)
      update(emptyDraft, mode)
      if (latestContext.current?.generation !== generation) return
      if (settings.showConfirmation) setSaved(true)
      const finish = () => {
        setSaved(false)
        if (settings.hideAfterSave) {
          void api.hideCapture(generation).catch(error => setError(errorText(error)))
        }
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

  return (
    <main className="capture-shell" onKeyDown={event => {
      if (event.key === 'Escape') { event.preventDefault(); void dismiss() }
      if (event.key === 'Enter' && event.repeat) event.preventDefault()
    }}>
      <header className="capture-header">
        <Brand small />
        <h1>{taskMode ? 'Start task' : 'Quick Capture'}</h1>
        <div className="capture-header-actions">
          {taskMode && <button type="button" className="button capture-back" disabled={controlsDisabled}
            onClick={() => void runControl(() => api.openCapture('quick'))}><ArrowLeft size={14} /> Log activity</button>}
          <button type="button" className="icon-button" aria-label="Dismiss Quick Capture" title="Dismiss (Esc)" onClick={() => void dismiss()}><X size={17} /></button>
        </div>
      </header>
      <form className="capture-form" onSubmit={submit}>
        <div className="flex items-stretch gap-2">
          <input
            autoFocus ref={input} aria-label={taskMode ? 'Task' : 'Activity'} placeholder={taskMode ? 'What are you working on?' : 'What did you work on?'}
            value={draft.description} readOnly={!context || busy || (saved && settings?.hideAfterSave)} maxLength={4000}
            onChange={event => {
              setError('')
              if (!settings?.hideAfterSave) {
                window.clearTimeout(timer.current)
                setSaved(false)
              }
              update({ description: event.target.value, categoryId: draft.categoryId })
            }}
          />
          <button className="button primary capture-save" disabled={controlsDisabled || !settings || !draft.description.trim()}>
            {saved ? <Check size={16} /> : <CornerDownLeft size={16} />}
            {saved ? (taskMode ? 'Started' : 'Saved') : busy ? 'Saving' : taskMode ? 'Start' : 'Save'}
          </button>
        </div>
        <div className="capture-options">
          <label htmlFor="capture-category" className="sr-only">Optional category</label>
          <select id="capture-category" value={categoryId ?? ''} disabled={!snapshot || busy} onChange={event =>
            update({ description: draft.description, categoryId: event.target.value ? Number(event.target.value) : null })
          }>
            <option value="">No category</option>
            <CategoryOptions categories={snapshot?.categories ?? []} />
          </select>
          <span className="capture-hint"><kbd>Enter</kbd> to {taskMode ? 'start' : 'save'} <span aria-hidden="true">·</span> <kbd>Esc</kbd> to hide</span>
        </div>
        {saved && <span className="sr-only" role="status">{taskMode ? 'Task started' : 'Activity saved'}</span>}
      </form>
      {snapshot && <CaptureTracking
        tracking={snapshot.tracking}
        format={snapshot.settings.timeFormat}
        disabled={controlsDisabled}
        onAction={(action: TrackingAction) => void runControl(() => api.track(action))}
        onStartTask={taskMode ? undefined : () => void runControl(() => api.openCapture('task'))}
      />}
      {(error || dataError) && <div className="capture-error" role="alert">
        {error || dataError} {dataError && <button type="button" onClick={refresh}>Retry</button>}
      </div>}
    </main>
  )
}
