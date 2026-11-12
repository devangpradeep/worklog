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

export function formatRelativeTime(value: number, date: string, format: TimeFormat) {
  const day = localDay(new Date(value))
  const time = formatTime(value, format)
  return day === date ? time : formatDay(day, { month: 'short', day: 'numeric' }) + ', ' + time
}

export function localDateTime(date = new Date(), day = localDay(date)) {
  return day + 'T' + [date.getHours(), date.getMinutes(), date.getSeconds()].map(pad).join(':')
}

export function timestampFromInput(input: string, previous?: number) {
  const expected = input.length === 16 ? input + ':00' : input
  if (previous !== undefined && localDateTime(new Date(previous)) === expected) return previous
  const date = new Date(input)
  if (!Number.isFinite(date.getTime()) || localDateTime(date) !== expected) {
    throw new Error('Choose a valid local date and time.')
  }
  return date.getTime()
}
