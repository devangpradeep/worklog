import type { TimeFormat } from './api'
import type { DateTimeInput } from './time'

interface Props {
  label: 'Start' | 'End'
  value: DateTimeInput
  format: TimeFormat
  onChange: (value: DateTimeInput) => void
  focusTime?: boolean
}

export default function DateTimeFields({ label, value, format, onChange, focusTime }: Props) {
  return <div className="form-grid">
    <label className="field">{label} date
      <input required type="date" min="0001-01-01" max="9999-12-31" value={value.date}
        onChange={event => onChange({ ...value, date: event.target.value })} />
    </label>
    <div className="time-fields">
      <label className="field">{label} time
        <input required type="text" autoFocus={focusTime} autoComplete="off" spellCheck={false}
          placeholder={format === '12h' ? '9:30' : '14:30'} value={value.time}
          onChange={event => onChange({ ...value, time: event.target.value })} />
      </label>
      {format === '12h' && <select aria-label={label + ' time AM or PM'} value={value.period}
        onChange={event => onChange({ ...value, period: event.target.value === 'PM' ? 'PM' : 'AM' })}>
        <option value="AM">AM</option>
        <option value="PM">PM</option>
      </select>}
    </div>
  </div>
}
