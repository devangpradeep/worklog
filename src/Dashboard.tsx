import {
  BookOpen, CalendarDays, CheckCircle2, ChevronLeft, ChevronRight, Clock3,
  Database, ListFilter, NotebookPen, PanelLeftClose, PanelLeftOpen, Plus, Search, Settings2, X,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { api, errorText, type CaptureMode, type Entry, type Review, type ReviewAction } from './api'
import Calendar from './Calendar'
import EntryEditor, { DeleteEntry } from './EntryEditor'
import { useData, useNativeEvent, useToday, useWorkdaySummary } from './hooks'
import JournalEntries from './JournalEntries'
import ReviewPanel from './ReviewPanel'
import SettingsScreen from './SettingsScreen'
import { formatDay, formatDuration, localDay, shiftDay } from './time'
import Workday from './Workday'
import { Brand, ErrorNotice, SuccessNotice } from './ui'

// Keep the saved preference from the calendar toggle.
const sidebarCollapsedKey = 'worklog.calendar.collapsed'

export default function Dashboard() {
  const { snapshot, revision, error: dataError, refresh } = useData()
  const today = useToday()
  const [date, setDate] = useState(today)
  const [page, setPage] = useState<'journal' | 'settings'>('journal')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    try {
      return localStorage.getItem(sidebarCollapsedKey) === 'true'
    } catch {
      return false
    }
  })
  const [search, setSearch] = useState('')
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('all')
  const [byActivity, setByActivity] = useState(false)
  const [result, setResult] = useState<{ key: string; entries: Entry[] }>()
  const [review, setReview] = useState<Review>()
  const [reviewMode, setReviewMode] = useState(false)
  const [reviewDrafts, setReviewDrafts] = useState<Record<string, string>>({})
  const [editor, setEditor] = useState<{ date: string; entry?: Entry }>()
  const [deleteEntry, setDeleteEntry] = useState<Entry>()
  const [entryError, setEntryError] = useState('')
  const [reviewError, setReviewError] = useState('')
  const [actionError, setActionError] = useState('')
  const [notice, setNotice] = useState('')
  const [restoreVersion, setRestoreVersion] = useState(0)
  const searchInput = useRef<HTMLInputElement>(null)
  const previousToday = useRef(today)
  const scroll = useRef<HTMLElement>(null)

  const searchActive = !!search.trim()
  const showWorkday = page === 'journal' && !searchActive
  const runningSession = snapshot?.tracking.session
  const liveWorkday = !!runningSession && localDay(new Date(runningSession.startedAt)) === date
  const workday = useWorkdaySummary(date, revision, showWorkday, liveWorkday)
  const workedMs = workday?.summary?.totals?.workedMs
  const changedDay = date !== today
  const queryKey = JSON.stringify([query ? null : date, query, category])
  const entries = result?.key === queryKey ? result.entries : []

  function toggleSidebar() {
    const collapsed = !sidebarCollapsed
    setSidebarCollapsed(collapsed)
    try {
      localStorage.setItem(sidebarCollapsedKey, String(collapsed))
    } catch {
      // The sidebar remains collapsible for the current window.
    }
  }

  function pickDate(day: string, expand = false) {
    setPage('journal')
    setDate(day)
    setSearch('')
    setQuery('')
    setReviewMode(expand)
    if (expand) setCategory('all')
    scroll.current?.scrollTo({ top: 0 })
  }

  async function openCapture(mode: CaptureMode = 'quick') {
    try {
      await api.openCapture(mode)
    } catch (error) {
      setActionError(errorText(error))
    }
  }

  async function route() {
    try {
      const route = await api.takeRoute()
      if (route?.page === 'review') pickDate(route.date, true)
      if (route?.page === 'settings') setPage('settings')
      if (route?.page === 'journal') setPage('journal')
    } catch (error) {
      setActionError(errorText(error))
    }
  }

  useNativeEvent('dashboard-route', () => void route(), () => void route())
  useNativeEvent('data-restored', () => {
    setReviewDrafts({})
    setEditor(undefined)
    setDeleteEntry(undefined)
    setCategory('all')
    setRestoreVersion(version => version + 1)
    setNotice('Backup restored')
  })

  useEffect(() => {
    if (previousToday.current === today) return
    const previous = previousToday.current
    previousToday.current = today
    setDate(selected => selected === previous ? today : selected)
  }, [today])

  useEffect(() => {
    scroll.current?.scrollTo({ top: 0 })
  }, [page])

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(search.trim()), 160)
    return () => window.clearTimeout(timer)
  }, [search])

  useEffect(() => {
    let active = true
    setEntryError('')
    void api.entries({
      date: query ? null : date,
      search: query,
      categoryId: category === 'all' || category === 'none' ? null : Number(category),
      uncategorized: category === 'none',
    }).then(value => {
      if (!active) return
      setResult({ key: queryKey, entries: value })
      setEntryError('')
    }).catch(error => {
      if (active) setEntryError(errorText(error))
    })
    return () => { active = false }
  }, [date, query, category, revision, queryKey])

  useEffect(() => {
    let active = true
    void api.review(date).then(review => {
      if (active) {
        setReview(review)
        setReviewError('')
      }
    }).catch(error => {
      if (active) setReviewError(errorText(error))
    })
    return () => { active = false }
  }, [date, revision])

  useEffect(() => {
    if (!notice) return
    const timer = window.setTimeout(() => setNotice(''), 2700)
    return () => window.clearTimeout(timer)
  }, [notice])

  useEffect(() => {
    function keyboard(event: KeyboardEvent) {
      if (document.querySelector('dialog[open]')) return
      if (event.ctrlKey && !event.altKey && !event.shiftKey) {
        if (event.key.toLowerCase() === 'n') {
          event.preventDefault()
          if (!event.repeat) void openCapture()
        }
        if (event.key.toLowerCase() === 'f') {
          event.preventDefault()
          setPage('journal')
          requestAnimationFrame(() => searchInput.current?.focus())
        }
      }
    }
    window.addEventListener('keydown', keyboard)
    return () => window.removeEventListener('keydown', keyboard)
  }, [])

  function reviewSaved(saved: Review, action: ReviewAction) {
    setReview(saved)
    setReviewDrafts(drafts => {
      const updated = { ...drafts }
      delete updated[saved.date]
      return updated
    })
    if (action === 'markReviewed') setReviewMode(false)
    setNotice(action === 'markReviewed' ? 'Day marked reviewed' : action === 'reopen' ? 'Day reopened' : 'Review notes saved')
  }

  const categories = snapshot?.categories ?? []
  const currentReview = review?.date === date ? review : undefined
  const reviewNotes = reviewDrafts[date] ?? currentReview?.notes ?? ''
  const timeFormat = snapshot?.settings.timeFormat ?? '24h'
  const showEntries = !entryError && result?.key === queryKey && searchActive === !!query

  return (
    <div className="app-layout" data-sidebar-collapsed={sidebarCollapsed || undefined}>
      <aside id="dashboard-sidebar" className="sidebar" hidden={sidebarCollapsed}>
        <div className="wordmark"><Brand /><span>WorkLog</span></div>
        <nav aria-label="Workspace" className="side-navigation">
          <button className="nav-button" aria-current={page === 'journal' ? 'page' : undefined} onClick={() => setPage('journal')}><BookOpen size={17} /> Journal</button>
          <button className="nav-button" aria-current={page === 'settings' ? 'page' : undefined} onClick={() => setPage('settings')}><Settings2 size={17} /> Settings</button>
        </nav>
        <div className="side-divider" />
        <Calendar date={date} today={today} revision={revision} onPick={pickDate} />
        <button className="button side-today" onClick={() => pickDate(today)}><CalendarDays size={15} /> Go to today</button>
        <div className="sidebar-bottom"><Database size={15} /><span>Stored on this device</span></div>
      </aside>

      <div className="workspace">
        <header className="toolbar">
          <div className="toolbar-location">
            <button type="button" className="icon-button" aria-controls="dashboard-sidebar" aria-expanded={!sidebarCollapsed}
              aria-label={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
              title={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'} onClick={toggleSidebar}>
              {sidebarCollapsed ? <PanelLeftOpen size={17} /> : <PanelLeftClose size={17} />}
            </button>
            <span>{page === 'journal' ? 'Journal' : 'Settings'}</span>
          </div>
          {showWorkday && workedMs !== undefined && <div className="toolbar-worked" title="Work sessions minus recorded breaks">
            <Clock3 size={15} aria-hidden="true" />
            <span><strong>{formatDuration(workedMs)}</strong> worked</span>
          </div>}
          {page === 'journal' && <div className="search-field">
            <Search size={17} />
            <input ref={searchInput} type="search" placeholder="Search activities…" aria-label="Search activities" value={search}
              onChange={event => setSearch(event.target.value)}
              onKeyDown={event => { if (event.key === 'Escape') { setSearch(''); setQuery('') } }} />
            {search ? <button className="icon-button" aria-label="Clear search" onClick={() => { setSearch(''); setQuery(''); searchInput.current?.focus() }}><X size={15} /></button> : <kbd>Ctrl F</kbd>}
          </div>}
          <button className="button primary toolbar-add" onClick={() => void openCapture()} title="Quick Add (Ctrl+N)"><Plus size={18} /> Quick Add</button>
        </header>
        <main className="workspace-scroll" ref={scroll} tabIndex={0} aria-label={page === 'journal' ? 'Journal' : 'Settings'}>
          <div className="page-content">
            {(dataError || actionError) && <ErrorNotice>
              {dataError || actionError} <button className="text-button" onClick={() => { refresh(); setActionError('') }}>Retry</button>
            </ErrorNotice>}
            <div hidden={page !== 'settings'}>
              {snapshot && <SettingsScreen key={restoreVersion} snapshot={snapshot} onSaved={refresh} />}
            </div>
            <div hidden={page !== 'journal'}>
              <div className="day-header">
                <div>
                  <div className="date-eyebrow">
                    {searchActive ? 'All dates' : formatDay(date, { weekday: 'long' }) + ' · ' + date.slice(0, 4)}
                    {!searchActive && !changedDay && <span className="today-tag">Today</span>}
                  </div>
                  <h1>{searchActive ? 'Search activities' : formatDay(date, { month: 'long', day: 'numeric' })}</h1>
                </div>
                {!searchActive && <div className="day-navigation">
                  <button className="icon-button" aria-label="Previous day" title="Previous day" onClick={() => pickDate(shiftDay(date, -1))}><ChevronLeft size={18} /></button>
                  <input type="date" aria-label="Go to date" value={date} min="0001-01-01" max="9999-12-31"
                    onChange={event => { if (event.target.validity.valid && event.target.value) pickDate(event.target.value) }} />
                  <button className="icon-button" aria-label="Next day" title="Next day" onClick={() => pickDate(shiftDay(date, 1))}><ChevronRight size={18} /></button>
                </div>}
              </div>

              {!searchActive && <Workday date={date} today={today} summary={workday?.summary} readError={workday?.error}
                format={timeFormat} tracking={snapshot?.tracking} onStartTask={() => void openCapture('task')} />}

              <div className={'journal-grid' + (searchActive ? ' search-view' : '')}>
                <section className="timeline-card" aria-labelledby="timeline-title">
                  <div className="timeline-header">
                    <div className="timeline-title">
                      <Clock3 size={17} />
                      <h2 id="timeline-title">{searchActive ? 'Results' : 'Timeline'}</h2>
                      {showEntries && <span className="count">{entries.length}</span>}
                    </div>
                    <div className="timeline-controls">
                      {!searchActive && <div className="timeline-view" role="group" aria-label="Timeline view">
                        <button type="button" aria-pressed={!byActivity} onClick={() => setByActivity(false)}>Time order</button>
                        <button type="button" aria-pressed={byActivity} onClick={() => setByActivity(true)}>By activity</button>
                      </div>}
                      <label className="filter-control">
                        <ListFilter size={15} />
                        <span className="sr-only">Filter by category</span>
                        <select value={category} onChange={event => setCategory(event.target.value)}>
                          <option value="all">All categories</option>
                          <option value="none">Uncategorized</option>
                          {categories.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}
                        </select>
                      </label>
                      {!searchActive && <button className="icon-button timeline-add" title="Add activity to selected day"
                        aria-label="Add activity to selected day" disabled={!snapshot} onClick={() => setEditor({ date })}><Plus size={18} /></button>}
                    </div>
                  </div>
                  {entryError && <div className="timeline-error"><ErrorNotice>{entryError} <button className="text-button" onClick={refresh}>Retry</button></ErrorNotice></div>}
                  {!entryError && !showEntries && <div className="timeline-loading" role="status">Loading activities…</div>}
                  {showEntries && entries.length === 0 && (
                    <div className="empty-state">
                      <div className="empty-icon"><NotebookPen size={25} /></div>
                      <h3>{searchActive || category !== 'all' ? 'No matching activities' : 'No activities yet'}</h3>
                      <p>{searchActive ? 'Try another description or reference.' : category !== 'all' ? 'Try a different category.' : 'Add an activity to start this day’s timeline.'}</p>
                      {!searchActive && category === 'all' && <button className="button secondary" disabled={!snapshot} onClick={() => setEditor({ date })}><Plus size={16} /> Add activity</button>}
                    </div>
                  )}
                  {showEntries && entries.length > 0 && <JournalEntries key={date} entries={entries} categories={categories} format={timeFormat}
                    byActivity={byActivity && !searchActive} searchActive={searchActive} onPickDate={pickDate}
                    onEdit={entry => setEditor({ date: localDay(new Date(entry.activityAt)), entry })} onDelete={setDeleteEntry} />}
                </section>
                {!searchActive && <div className="review-column">
                  {currentReview?.status === 'reviewed' && <span className="status-tag reviewed"><CheckCircle2 size={14} /> Reviewed</span>}
                  {currentReview?.status === 'needsReview' && <span className="status-tag needs-review"><Clock3 size={14} /> Review needs updating</span>}
                  <ReviewPanel key={date} date={date} review={currentReview} notes={reviewNotes} expanded={reviewMode}
                    onExpand={() => { setReviewMode(true); setCategory('all') }}
                    onNotes={notes => setReviewDrafts(drafts => ({ ...drafts, [date]: notes }))}
                    onSave={reviewSaved} />
                  {reviewError && <ErrorNotice>{reviewError} <button className="text-button" onClick={refresh}>Retry</button></ErrorNotice>}
                </div>}
              </div>
            </div>
          </div>
        </main>
      </div>

      {snapshot && editor && <EntryEditor date={editor.date} entry={editor.entry} categories={categories} format={timeFormat}
        detectReferences={snapshot.settings.detectReferences} onDismiss={() => setEditor(undefined)}
        onSaved={saved => {
          setEditor(undefined)
          setNotice(editor.entry ? 'Activity updated' : 'Activity added')
          const savedDay = localDay(new Date(saved.activityAt))
          if (!searchActive && savedDay !== date) pickDate(savedDay)
          refresh()
        }} />}
      {deleteEntry && <DeleteEntry entry={deleteEntry} onDismiss={() => setDeleteEntry(undefined)} onDeleted={() => {
        setDeleteEntry(undefined)
        setNotice('Activity deleted')
        refresh()
      }} />}
      {notice && <div className="toast"><SuccessNotice>{notice}</SuccessNotice></div>}
    </div>
  )
}
