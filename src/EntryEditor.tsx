import { useState, type FormEvent } from 'react'
import { api, errorText, type Category, type Entry, type EntryKind, type TimeFormat } from './api'
import DateTimeFields from './DateTimeFields'
import { dateTimeInput, dateTimeInputChanged, timestampFromInput } from './time'
import { CategoryOptions, ErrorNotice, Modal } from './ui'

interface Props {
  date: string
  entry?: Entry
  categories: Category[]
  format: TimeFormat
  detectReferences: boolean
  onDismiss: () => void
  onSaved: (entry: Entry) => void
}

export default function EntryEditor({ date, entry, categories, format, detectReferences, onDismiss, onSaved }: Props) {
  const [originalTime] = useState(() => dateTimeInput(new Date(entry?.activityAt ?? Date.now()), format, entry ? undefined : date))
  const originalEndAt = entry?.endAt ?? null
  const originalEnd = originalEndAt === null ? null : dateTimeInput(new Date(originalEndAt), format)
  const [endAt, setEndAt] = useState(originalEnd)
  const [hasEnd, setHasEnd] = useState(!!originalEnd)
  const [kind, setKind] = useState<EntryKind>(entry?.kind ?? 'activity')
  const isBreak = kind === 'break'
  const [description, setDescription] = useState(entry?.description ?? '')
  const [categoryId, setCategoryId] = useState(entry?.categoryId ?? null)
  const [activityAt, setActivityAt] = useState(originalTime)
  const [prefix, setPrefix] = useState(entry?.reference?.prefix ?? '')
  const [number, setNumber] = useState(entry?.reference?.number ?? '')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [discard, setDiscard] = useState(false)
  const changed = description !== (entry?.description ?? '') || categoryId !== (entry?.categoryId ?? null)
    || kind !== (entry?.kind ?? 'activity') || hasEnd !== !!originalEnd
    || (hasEnd && endAt && originalEnd && dateTimeInputChanged(endAt, originalEnd))
    || dateTimeInputChanged(activityAt, originalTime) || prefix !== (entry?.reference?.prefix ?? '') || number !== (entry?.reference?.number ?? '')

  function dismiss() {
    if (busy) return
    if (changed) setDiscard(true)
    else onDismiss()
  }

  async function save(event: FormEvent) {
    event.preventDefault()
    if (busy || discard) return
    setError('')
    setBusy(true)
    try {
      const timestamp = timestampFromInput(activityAt, format, 'start', entry?.activityAt)
      const reference = !isBreak && (prefix.trim() || number.trim()) ? { prefix, number } : null
      const endTimestamp = hasEnd && endAt
        ? timestampFromInput(endAt, format, 'end', originalEndAt ?? undefined)
        : null
      const saved = await api.saveEntry(entry?.id ?? null, {
        description, categoryId: isBreak ? null : categoryId, activityAt: timestamp, endAt: endTimestamp,
        kind, updateEnd: endTimestamp !== originalEndAt, reference, autoReference: !isBreak && !entry && !reference,
      })
      onSaved(saved)
    } catch (error) {
      setError(errorText(error))
      setBusy(false)
    }
  }

  return (
    <Modal title={entry ? 'Edit activity' : 'Add activity'} onDismiss={dismiss}>
      <form onSubmit={save} className="stack-form">
        <div className="form-grid">
          <label className="field">Type
            <select value={kind} onChange={event => setKind(event.target.value as EntryKind)}>
              <option value="activity">Task / activity</option>
              <option value="break">Break</option>
            </select>
          </label>
          {!isBreak && <label className="field">Category
            <select value={categoryId ?? ''} onChange={event => setCategoryId(event.target.value ? Number(event.target.value) : null)}>
              <option value="">No category</option>
              <CategoryOptions categories={categories} retained={entry?.categoryId} />
            </select>
          </label>}
        </div>
        <label className="field">{isBreak ? 'Break note (optional)' : 'Activity'}
          <textarea required={!isBreak} autoFocus value={description} rows={2} maxLength={4000} onChange={event => setDescription(event.target.value)} />
        </label>
        <DateTimeFields label="Start" value={activityAt} format={format} onChange={value => {
          setActivityAt(value)
          setError('')
        }} />
        <label className="end-time-toggle"><input type="checkbox" checked={hasEnd} onChange={event => {
          setHasEnd(event.target.checked)
          setError('')
          if (event.target.checked && !endAt) setEndAt(activityAt)
        }} /> Set end time</label>
        {hasEnd && endAt && <DateTimeFields label="End" value={endAt} format={format} onChange={value => {
          setEndAt(value)
          setError('')
        }} />}
        {!isBreak && <fieldset>
          <legend>Reference <span className="muted">(optional)</span></legend>
          <div className="form-grid">
            <label className="field">Prefix<input value={prefix} maxLength={16} placeholder="NCR" onChange={event => setPrefix(event.target.value)} /></label>
            <label className="field">Number<input value={number} inputMode="numeric" maxLength={24} placeholder="2456" onChange={event => setNumber(event.target.value)} /></label>
          </div>
          {!entry && detectReferences && <p className="help">Leave both fields blank to detect a reference in the activity.</p>}
        </fieldset>}
        {error && <ErrorNotice>{error}</ErrorNotice>}
        {discard ? <div className="discard-bar">
          <span>Discard unsaved changes?</span>
          <button type="button" className="button secondary" onClick={() => setDiscard(false)}>Keep editing</button>
          <button type="button" className="button danger" onClick={onDismiss}>Discard</button>
        </div> : <div className="modal-actions">
          <button type="button" className="button secondary" disabled={busy} onClick={dismiss}>Cancel</button>
          <button className="button primary" disabled={busy || (!isBreak && !description.trim())}>{busy ? 'Saving…' : 'Save activity'}</button>
        </div>}
      </form>
    </Modal>
  )
}

export function DeleteEntry({ entry, onDismiss, onDeleted }: { entry: Entry; onDismiss: () => void; onDeleted: () => void }) {
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  async function remove() {
    if (busy) return
    setBusy(true)
    try {
      await api.deleteEntry(entry.id)
      onDeleted()
    } catch (error) {
      setError(errorText(error))
      setBusy(false)
    }
  }

  return <Modal title="Delete activity?" onDismiss={() => { if (!busy) onDismiss() }}>
    <p className="delete-description">{entry.description}</p>
    {error && <ErrorNotice>{error}</ErrorNotice>}
    <div className="modal-actions">
      <button autoFocus type="button" className="button secondary" disabled={busy} onClick={onDismiss}>Cancel</button>
      <button type="button" className="button danger" disabled={busy} onClick={() => void remove()}>{busy ? 'Deleting…' : 'Delete activity'}</button>
    </div>
  </Modal>
}
