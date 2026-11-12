import { listen } from '@tauri-apps/api/event'
import { useEffect, useRef, useState } from 'react'
import { api, errorText, type Snapshot, type WorkdaySummary } from './api'
import { localDay } from './time'

export function useNativeEvent<T = unknown>(event: string, callback: (payload: T) => void, ready?: () => void) {
  const latest = useRef({ callback, ready })
  useEffect(() => {
    latest.current = { callback, ready }
  })
  useEffect(() => {
    let cancelled = false
    const subscription = listen<T>(event, ({ payload }) => latest.current.callback(payload))
    void subscription.then(() => {
      if (!cancelled) latest.current.ready?.()
    }).catch(console.error)
    return () => {
      cancelled = true
      void subscription.then(unlisten => unlisten()).catch(console.error)
    }
  }, [event])
}

export function useData() {
  const [snapshot, setSnapshot] = useState<Snapshot>()
  const [error, setError] = useState('')
  const [revision, setRevision] = useState(0)
  const refresh = () => setRevision(value => value + 1)

  useNativeEvent('data-changed', refresh, refresh)
  useEffect(() => {
    let cancelled = false
    void api.snapshot().then(value => {
      if (!cancelled) {
        setSnapshot(value)
        setError('')
      }
    }).catch(error => {
      if (!cancelled) setError(errorText(error))
    })
    return () => { cancelled = true }
  }, [revision])

  const theme = snapshot?.settings.theme ?? 'system'
  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  return { snapshot, revision, error, refresh }
}

export function useToday() {
  const [today, setToday] = useState(localDay)
  useEffect(() => {
    const refresh = () => setToday(localDay())
    window.addEventListener('focus', refresh)
    const timer = window.setInterval(refresh, 30_000)
    return () => {
      window.removeEventListener('focus', refresh)
      window.clearInterval(timer)
    }
  }, [])
  return today
}

export function useWorkdaySummary(date: string, revision: number, enabled: boolean, live: boolean) {
  const [result, setResult] = useState<{ date: string; summary?: WorkdaySummary; error?: string }>()

  useEffect(() => {
    if (!enabled) return
    let active = true
    let pending = false

    async function load() {
      if (pending) return
      pending = true
      try {
        const summary = await api.workdaySummary(date)
        if (active) setResult({ date, summary })
      } catch (error) {
        if (active) setResult({ date, error: errorText(error) })
      } finally {
        pending = false
      }
    }

    void load()
    const timer = live ? window.setInterval(() => {
      if (!document.hidden) void load()
    }, 5_000) : undefined
    if (live) window.addEventListener('focus', load)

    return () => {
      active = false
      window.clearInterval(timer)
      window.removeEventListener('focus', load)
    }
  }, [date, revision, enabled, live])

  return result?.date === date ? result : undefined
}
