import { CheckCircle2, ClipboardCheck, PencilLine, RotateCcw } from 'lucide-react'
import { useState } from 'react'
import { api, errorText, type Review, type ReviewAction } from './api'
import { ErrorNotice } from './ui'

interface Props {
  date: string
  review?: Review
  expanded: boolean
  notes: string
  onExpand: () => void
  onNotes: (notes: string) => void
  onSave: (review: Review, action: ReviewAction) => void
}

export default function ReviewPanel({ date, review, expanded, notes, onExpand, onNotes, onSave }: Props) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const status = review?.status ?? 'open'
  const reviewed = status === 'reviewed'
  const outdated = status === 'needsReview'
  const changed = notes !== (review?.notes ?? '')

  async function save(action: ReviewAction) {
    if (busy || !review) return
    setBusy(true)
    setError('')
    try {
      onSave(await api.saveReview(date, notes, action), action)
    } catch (error) {
      setError(errorText(error))
    } finally {
      setBusy(false)
    }
  }

  return (
    <aside className="review-panel" aria-labelledby="review-title">
      <div className="review-icon" data-status={status}>
        {reviewed ? <CheckCircle2 size={19} /> : <ClipboardCheck size={19} />}
      </div>
      <h2 id="review-title">Daily review</h2>
      <p className="review-copy">
        {reviewed ? 'This day has been reviewed.' : outdated ? 'Activities changed since your last review.' : 'Check your activities and add any notes for the day.'}
      </p>
      {expanded ? (
        <div className="review-form">
          <label className="field" htmlFor="review-notes">Review notes <span className="muted">(optional)</span></label>
          <textarea id="review-notes" rows={5} value={notes} maxLength={10000} disabled={busy || !review}
            placeholder="Add a note…" onChange={event => onNotes(event.target.value)} />
          <div className="review-actions">
            {reviewed
              ? <button className="button secondary" disabled={busy || !review} onClick={() => void save('reopen')}><RotateCcw size={15} /> Reopen day</button>
              : <button className="button primary" disabled={busy || !review} onClick={() => void save('markReviewed')}><CheckCircle2 size={16} /> Mark reviewed</button>}
            {changed && <button className="button secondary" disabled={busy || !review} onClick={() => void save('saveNotes')}>Save notes</button>}
          </div>
        </div>
      ) : (
        <>
          {notes && <p className="review-note-preview">{notes}</p>}
          <button className="button secondary review-start" onClick={onExpand} disabled={!review}>
            {reviewed ? <PencilLine size={15} /> : <ClipboardCheck size={16} />}
            {reviewed ? 'View review' : 'Review day'}
          </button>
        </>
      )}
      {error && <ErrorNotice>{error}</ErrorNotice>}
    </aside>
  )
}
