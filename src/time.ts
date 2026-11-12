import type { TimeFormat } from './api'

const pad = (value: number) => String(value).padStart(2, '0')

export function localDay(date = new Date()) {
  return [String(date.getFullYear()).padStart(4, '0'), pad(date.getMonth() + 1), pad(date.getDate())].join('-')
}

export function dayDate(day: string) {
  return new Date(day + 'T12:00:00')
}

export function shiftDay(day: string, amount: number) {
  const date = dayDate(day)
  date.setDate(date.getDate() + amount)
  return localDay(date)
}

export function formatDay(day: string, options: Intl.DateTimeFormatOptions) {
  return dayDate(day).toLocaleDateString(undefined, options)
}

export function formatTime(value: number, format: TimeFormat) {
  return new Date(value).toLocaleTimeString(undefined, {
    hour: '2-digit', minute: '2-digit', hourCycle: format === '12h' ? 'h12' : 'h23',
  })
}

export function formatDuration(milliseconds: number) {
  const totalMinutes = Math.floor(milliseconds / 60_000)
  if (totalMinutes === 0 && milliseconds > 0) {
    const seconds = Math.floor(milliseconds / 1_000)
    return seconds > 0 ? seconds + 's' : '<1s'
  }
  const hours = Math.floor(totalMinutes / 60)
  const minutes = totalMinutes % 60
  return hours ? hours + 'h' + (minutes ? ' ' + minutes + 'm' : '') : minutes + 'm'
}

export function formatHoursMinutes(milliseconds: number) {
  const minutes = Math.floor(milliseconds / 60_000)
  return Math.floor(minutes / 60) + ':' + pad(minutes % 60)
}

export function formatRelativeTime(value: number, date: string, format: TimeFormat) {
  const day = localDay(new Date(value))
  const time = formatTime(value, format)
  return day === date ? time : formatDay(day, { month: 'short', day: 'numeric' }) + ', ' + time
}

export interface DateTimeInput {
  date: string
  time: string
  period: 'AM' | 'PM'
}

export function dateTimeInput(date: Date, format: TimeFormat, day = localDay(date)): DateTimeInput {
  const hour = date.getHours()
  const seconds = date.getSeconds()
  const displayHour = format === '12h' ? hour % 12 || 12 : hour
  return {
    date: day,
    time: pad(displayHour) + ':' + pad(date.getMinutes()) + (seconds ? ':' + pad(seconds) : ''),
    period: hour < 12 ? 'AM' : 'PM',
  }
}

export function dateTimeInputChanged(value: DateTimeInput, original: DateTimeInput) {
  return value.date !== original.date || value.time !== original.time || value.period !== original.period
}

function localDateTime(date: Date) {
  return localDay(date) + 'T' + [date.getHours(), date.getMinutes(), date.getSeconds()].map(pad).join(':')
}

export function timestampFromInput(input: DateTimeInput, format: TimeFormat, field: 'start' | 'end', previous?: number) {
  const match = /^(\d{1,2}):([0-5]\d)(?::([0-5]\d))?$/.exec(input.time.trim())
  const hour = Number(match?.[1])
  const twelveHour = format === '12h'
  if (!match || (twelveHour ? hour < 1 || hour > 12 : hour > 23)) {
    const example = twelveHour ? '9:30' : '14:30'
    throw new Error(`Enter a valid ${field} time, such as ${example}.`)
  }
  const hour24 = twelveHour ? hour % 12 + (input.period === 'PM' ? 12 : 0) : hour
  const expected = `${input.date}T${pad(hour24)}:${match[2]}:${match[3] ?? '00'}`
  if (previous !== undefined && localDateTime(new Date(previous)) === expected) return previous
  const date = new Date(expected)
  if (!Number.isFinite(date.getTime()) || localDateTime(date) !== expected) {
    throw new Error(`Choose a valid local ${field} date and time.`)
  }
  return date.getTime()
}
