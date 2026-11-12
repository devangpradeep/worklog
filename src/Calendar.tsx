import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api, type ReviewStatus } from './api'
import { dayDate, formatDay, localDay } from './time'

const weekdays = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

interface Props {
  date: string
  today: string
  revision: number
  onPick: (date: string) => void
}

export default function Calendar({ date, today, revision, onPick }: Props) {
  const selectedMonth = date.slice(0, 7)
  const [month, setMonth] = useState(selectedMonth)
  const [marks, setMarks] = useState<Record<string, ReviewStatus>>({})
  useEffect(() => setMonth(selectedMonth), [selectedMonth])
  useEffect(() => {
    let active = true
    void api.monthReviews(month).then(reviews => {
      if (active) setMarks(Object.fromEntries(reviews.map(review => [review.date, review.status])))
    }).catch(console.error)
    return () => { active = false }
  }, [month, revision])

  const start = dayDate(month + '-01')
  const offset = (start.getDay() + 6) % 7
  const end = new Date(start)
  end.setMonth(end.getMonth() + 1, 0)
  const days = Array.from({ length: end.getDate() }, (_, index) => {
    const day = new Date(start)
    day.setDate(index + 1)
    return localDay(day)
  })

  function shiftMonth(amount: number) {
    const changed = new Date(start)
    changed.setMonth(changed.getMonth() + amount)
    const next = localDay(changed).slice(0, 7)
    if (next.length === 7 && changed.getFullYear() >= 1) setMonth(next)
  }

  return (
    <section className="calendar" aria-label="Journal calendar">
      <div className="calendar-title">
        <h2>{formatDay(month + '-01', { month: 'long', year: 'numeric' })}</h2>
        <div className="flex shrink-0 gap-0.5">
          <button className="icon-button calendar-arrow" aria-label="Previous month" onClick={() => shiftMonth(-1)}><ChevronLeft size={15} /></button>
          <button className="icon-button calendar-arrow" aria-label="Next month" onClick={() => shiftMonth(1)}><ChevronRight size={15} /></button>
        </div>
      </div>
      <div className="calendar-grid">
        {weekdays.map((day, index) => <span className="calendar-weekday" key={index} aria-hidden="true">{day}</span>)}
        {Array.from({ length: offset }, (_, index) => <span key={'empty' + index} />)}
        {days.map(day => {
          const status = marks[day]
          const note = status === 'reviewed' ? ', reviewed' : status === 'needsReview' ? ', review needs updating' : ''
          return <button key={day} className="calendar-day" data-status={status} aria-pressed={day === date}
            aria-current={day === today ? 'date' : undefined}
            aria-label={formatDay(day, { dateStyle: 'full' }) + note}
            title={formatDay(day, { dateStyle: 'full' }) + note}
            onClick={() => onPick(day)}>
            {Number(day.slice(-2))}
          </button>
        })}
      </div>
    </section>
  )
}
