import { Play, TriangleAlert } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api, errorText, type Category, type Entry, type TimeFormat } from './api'
import { formatRelativeTime, localDay } from './time'
import { ErrorNotice } from './ui'

interface Props {
  categories: Category[]
  format: TimeFormat
  revision: number
  disabled: boolean
  runningCount: number
  onBreak: boolean
  onStart: (task: Entry) => void
}

export default function RecentTasks({ categories, format, revision, disabled, runningCount, onBreak, onStart }: Props) {
  const [tasks, setTasks] = useState<Entry[]>()
  const [error, setError] = useState('')

  useEffect(() => {
    let active = true
    void api.recentTasks().then(tasks => {
      if (!active) return
      setTasks(tasks)
      setError('')
    }).catch(error => {
      if (active) setError(errorText(error))
    })
    return () => { active = false }
  }, [revision])

  const notice = runningCount > 1 ? 'Two tasks are running. Stop one to start another.'
    : runningCount === 1 ? 'Your current task will keep running.'
    : onBreak ? 'Starting a task will end your break.' : ''

  if (error) return <ErrorNotice>{error}</ErrorNotice>
  if (!tasks) return <p className="muted" role="status">Loading recent tasks…</p>
  if (tasks.length === 0) return <p className="muted">No finished tasks today.</p>

  return <>
    {notice && <p className="capture-task-notice" role="status"><TriangleAlert size={14} aria-hidden="true" />{notice}</p>}
    <ul className="recent-task-list">
      {tasks.map(task => {
        const category = categories.find(item => item.id === task.categoryId)
        const time = formatRelativeTime(task.endAt ?? task.activityAt, localDay(), format)
        const reference = task.reference && `${task.reference.prefix} ${task.reference.number}`
        return <li key={task.id}>
          <button className="recent-task" type="button" disabled={disabled || runningCount > 1} onClick={() => onStart(task)}>
            <span className="recent-task-details">
              <strong title={task.description}>{task.description}</strong>
              <small>{[category?.name, reference, time].filter(Boolean).join(' · ')}</small>
            </span>
            <span className="recent-task-action"><Play size={14} aria-hidden="true" /> Start again</span>
          </button>
        </li>
      })}
    </ul>
  </>
}
