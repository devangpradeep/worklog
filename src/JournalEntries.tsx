import { ChevronRight, Coffee, Pencil, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { Category, Entry, TimeFormat } from './api'
import { entryEnd, groupActivities } from './activityGroups'
import { formatDay, formatHoursMinutes, formatTime, localDay } from './time'

interface Props {
  entries: Entry[]
  categories: Category[]
  format: TimeFormat
  byActivity: boolean
  searchActive: boolean
  onPickDate: (day: string) => void
  onEdit: (entry: Entry) => void
  onDelete: (entry: Entry) => void
}

export default function JournalEntries({ entries, categories, format, byActivity, searchActive, onPickDate, onEdit, onDelete }: Props) {
  const [now, setNow] = useState(Date.now)
  const running = entries.some(entry => entry.isRunning)

  useEffect(() => {
    if (!running) return
    const update = () => setNow(Date.now())
    update()
    const timer = window.setInterval(() => {
      if (!document.hidden) update()
    }, 5_000)
    window.addEventListener('focus', update)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener('focus', update)
    }
  }, [running])

  const categoryFor = (entry: Entry) => categories.find(item => item.id === entry.categoryId)
  const renderEntry = (entry: Entry) => <EntryRow entry={entry} category={categoryFor(entry)} format={format} now={now}
    onEdit={() => onEdit(entry)} onDelete={() => onDelete(entry)} />

  if (byActivity) return <ul className="timeline-list" aria-label="Activities by name">
    {groupActivities(entries, now).map(group => {
      const first = group.entries[0]
      const count = group.entries.length
      return <li key={group.key} className="activity-group">
        <details>
          <summary className="activity-summary">
            <ChevronRight size={16} className="activity-chevron" aria-hidden="true" />
            <span className="activity-summary-text">
              <ActivityName entry={first} />
              <span className="entry-tags">
                <span className="activity-count">{count === 1 ? '1 entry' : `${count} entries`}
                  {group.untimed > 0 && group.untimed < count && ` · ${group.untimed} untimed`}</span>
                <EntryTags entry={first} category={categoryFor(first)} running={group.entries.some(entry => entry.isRunning)} />
              </span>
            </span>
            <Duration milliseconds={group.duration} />
          </summary>
          <ol className="timeline-list activity-entries" aria-label="Recorded time ranges">
            {group.entries.map(entry => <li key={entry.id} className="entry-item">{renderEntry(entry)}</li>)}
          </ol>
        </details>
      </li>
    })}
  </ul>

  return <ol className="timeline-list">
    {entries.map((entry, index) => {
      const day = localDay(new Date(entry.activityAt))
      const previous = entries[index - 1]
      const showDate = searchActive && (!previous || localDay(new Date(previous.activityAt)) !== day)
      return <li key={entry.id} className="entry-item">
        {showDate && <button className="entry-day" onClick={() => onPickDate(day)}>{formatDay(day, { dateStyle: 'full' })}<ChevronRight size={14} /></button>}
        {renderEntry(entry)}
      </li>
    })}
  </ol>
}

function ActivityName({ entry }: { entry: Entry }) {
  return <span className="entry-description">{entry.kind === 'break' && <Coffee size={15} aria-label="Break" />}{entry.description}</span>
}

function EntryTags({ entry, category, running }: { entry: Entry; category?: Category; running: boolean }) {
  return <>
    {running && <span className="reference-tag">{entry.kind === 'break' ? 'On break' : 'Running'}</span>}
    {category && <span className="category-tag"><span style={{ backgroundColor: category.color }} />{category.name}</span>}
    {entry.reference && <span className="reference-tag">{entry.reference.prefix} {entry.reference.number}</span>}
  </>
}

function Duration({ milliseconds }: { milliseconds: number | null }) {
  return <span className="entry-duration" data-untimed={milliseconds === null || undefined}
    title={milliseconds === null ? 'Add an end time to include this entry in the total' : 'Duration in hours and minutes'}>
    {milliseconds === null ? 'Untimed' : <><span className="sr-only">Duration (hours and minutes) </span>{formatHoursMinutes(milliseconds)}</>}
  </span>
}

function EntryRow({ entry, category, format, now, onEdit, onDelete }: {
  entry: Entry
  category?: Category
  format: TimeFormat
  now: number
  onEdit: () => void
  onDelete: () => void
}) {
  const start = new Date(entry.activityAt)
  const end = entry.endAt === null ? null : new Date(entry.endAt)
  const endDay = end ? localDay(end) : null
  const overnight = endDay !== null && endDay !== localDay(start)
  const effectiveEnd = entryEnd(entry, now)

  return <article className="entry-row" data-kind={entry.kind}>
    <div className="entry-times" data-overnight={overnight || undefined}>
      <time dateTime={start.toISOString()}>{formatTime(entry.activityAt, format)}</time>
      {end && <time className="entry-end" dateTime={end.toISOString()}>
        <span className="sr-only">Ends at </span>
        {overnight && <span>{formatDay(endDay, { month: 'short', day: 'numeric' })}, </span>}
        <span>{formatTime(end.getTime(), format)}</span>
      </time>}
    </div>
    <div className="entry-marker" aria-hidden="true"><span style={category ? { backgroundColor: category.color } : undefined} /></div>
    <div className="entry-content">
      <ActivityName entry={entry} />
      {(category || entry.reference || entry.isRunning) && <div className="entry-tags">
        <EntryTags entry={entry} category={category} running={entry.isRunning} />
      </div>}
    </div>
    <div className="entry-actions">
      <Duration milliseconds={effectiveEnd === null ? null : effectiveEnd - entry.activityAt} />
      <button className="icon-button" aria-label="Edit activity" title="Edit activity" onClick={onEdit}><Pencil size={15} /></button>
      <button className="icon-button delete-button" aria-label="Delete activity" title="Delete activity" onClick={onDelete}><Trash2 size={15} /></button>
    </div>
  </article>
}
