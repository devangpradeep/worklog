import {
  BookOpen, CalendarDays, CheckCircle2, ChevronLeft, ChevronRight, Clock3, Coffee,
  Database, ListFilter, NotebookPen, Pencil, Plus, Search, Settings2, Trash2, X,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { api, errorText, type CaptureMode, type Category, type Entry, type Review, type ReviewAction, type TimeFormat } from './api'
import Calendar from './Calendar'
import EntryEditor, { DeleteEntry } from './EntryEditor'
import { useData, useNativeEvent, useToday } from './hooks'
import ReviewPanel from './ReviewPanel'
import SettingsScreen from './SettingsScreen'
import { formatDay, formatRelativeTime, formatTime, localDay, shiftDay } from './time'
import Workday from './Workday'
import { Brand, ErrorNotice, SuccessNotice } from './ui'

export default function Dashboard() {
  const { snapshot, revision, error: dataError, refresh } = useData()
  const today = useToday()
  const [date, setDate] = useState(today)
  const [page, setPage] = useState<'journal' | 'settings'>('journal')
  const [search, setSearch] = useState('')
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('all')
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
  const changedDay = date !== today
  const queryKey = JSON.stringify([query ? null : date, query, category])
  const entries = result?.key === queryKey ? result.entries : []

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
    <div className="app-layout">
      <aside className="sidebar">
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
          <div className="toolbar-location">{page === 'journal' ? <BookOpen size={16} /> : <Settings2 size={16} />} {page === 'journal' ? 'Journal' : 'Settings'}</div>
          {page === 'journal' && <div className="search-field">
            <Search size={17} />
            <input ref={searchInput} type="search" placeholder="Search activities…" aria-label="Search activities" value={search}
              onChange={event => setSearch(event.target.value)}
              onKeyDown={event => { if (event.key === 'Escape') { setSearch(''); setQuery('') } }} />
            {search ? <button className="icon-button" aria-label="Clear search" onClick={() => { setSearch(''); setQuery(''); searchInput.current?.focus() }}><X size={15} /></button> : <kbd>Ctrl F</kbd>}
          </div>}
          <button className="button primary toolbar-add" onClick={() => void openCapture()} title="Quick Add (Ctrl+N)"><Plus size={18} /> Quick Add</button>
        </header>
        <main className="workspace-scroll" ref={scroll}>
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

              {!searchActive && <Workday date={date} today={today} revision={revision}
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
                  {showEntries && entries.length > 0 && <ol className="timeline-list">
                    {entries.map((entry, index) => {
                      const entryDay = localDay(new Date(entry.activityAt))
                      const previous = entries[index - 1]
                      const group = searchActive && (!previous || localDay(new Date(previous.activityAt)) !== entryDay)
                      return <li key={entry.id} className="entry-item">
                        {group && <button className="entry-day" onClick={() => pickDate(entryDay)}>{formatDay(entryDay, { dateStyle: 'full' })}<ChevronRight size={14} /></button>}
                        <EntryRow entry={entry} category={categories.find(item => item.id === entry.categoryId)} format={timeFormat}
                          onEdit={() => setEditor({ date: entryDay, entry })} onDelete={() => setDeleteEntry(entry)} />
                      </li>
                    })}
                  </ol>}
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

      {snapshot && editor && <EntryEditor date={editor.date} entry={editor.entry} categories={categories}
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

function EntryRow({ entry, category, format, onEdit, onDelete }: {
  entry: Entry
  category?: Category
  format: TimeFormat
  onEdit: () => void
  onDelete: () => void
}) {
  return (
    <article className="entry-row" data-kind={entry.kind}>
      <div className="entry-times">
        <time dateTime={new Date(entry.activityAt).toISOString()}>{formatTime(entry.activityAt, format)}</time>
        {entry.endAt !== null && <time className="entry-end" dateTime={new Date(entry.endAt).toISOString()}>
          <span className="sr-only">Ends at </span>{formatRelativeTime(entry.endAt, localDay(new Date(entry.activityAt)), format)}
        </time>}
      </div>
      <div className="entry-marker" aria-hidden="true"><span style={category ? { backgroundColor: category.color } : undefined} /></div>
      <div className="entry-content">
        <p className="entry-description">{entry.kind === 'break' && <Coffee size={15} aria-label="Break" />}{entry.description}</p>
        {(category || entry.reference || entry.isRunning) && <div className="entry-tags">
          {entry.isRunning && <span className="reference-tag">{entry.kind === 'break' ? 'On break' : 'Running'}</span>}
          {category && <span className="category-tag"><span style={{ backgroundColor: category.color }} />{category.name}</span>}
          {entry.reference && <span className="reference-tag">{entry.reference.prefix} {entry.reference.number}</span>}
        </div>}
      </div>
      <div className="entry-actions">
        <button className="icon-button" aria-label="Edit activity" title="Edit activity" onClick={onEdit}><Pencil size={15} /></button>
        <button className="icon-button delete-button" aria-label="Delete activity" title="Delete activity" onClick={onDelete}><Trash2 size={15} /></button>
      </div>
    </article>
  )
}
