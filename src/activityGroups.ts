import type { Entry } from './api'

export function entryEnd(entry: Entry, now: number) {
  return entry.endAt ?? (entry.isRunning ? Math.max(entry.activityAt, now) : null)
}

// Entries arrive in start-time order, including the entries within each group.
export function groupActivities(entries: Entry[], now: number) {
  const groups = new Map<string, Entry[]>()
  for (const entry of entries) {
    const description = entry.description.trim().replace(/\s+/g, ' ').toLowerCase()
    const key = JSON.stringify([entry.kind, description, entry.categoryId, entry.reference?.prefix, entry.reference?.number])
    const group = groups.get(key)
    if (group) group.push(entry)
    else groups.set(key, [entry])
  }

  return Array.from(groups, ([key, entries]) => {
    let duration: number | null = null
    let untimed = 0
    let coveredThrough = -Infinity
    for (const entry of entries) {
      const end = entryEnd(entry, now)
      if (end === null) {
        untimed += 1
        continue
      }
      duration = (duration ?? 0) + Math.max(0, end - Math.max(entry.activityAt, coveredThrough))
      coveredThrough = Math.max(coveredThrough, end)
    }
    return { key, entries, duration, untimed }
  })
}
