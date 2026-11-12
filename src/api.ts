import { invoke } from '@tauri-apps/api/core'

export type Theme = 'system' | 'light' | 'dark'
export type TimeFormat = '12h' | '24h'
export type CaptureMode = 'quick' | 'task'
export type CaptureContext = { mode: CaptureMode; generation: number }
export type EntryKind = 'activity' | 'break'
export type TrackingAction = { kind: 'startDay' | 'startBreak' } | { kind: 'endDay' | 'endBreak' | 'stopTask'; id: number }
export type ReviewStatus = 'open' | 'reviewed' | 'needsReview'

export interface Settings {
  minimizeToTray: boolean
  theme: Theme
  timeFormat: TimeFormat
  hideAfterSave: boolean
  defaultCategory: number | null
  shortcut: string
  showConfirmation: boolean
  rememberCategory: boolean
  remindersEnabled: boolean
  reminderTime: string
  workingDays: number[]
  snoozeMinutes: number
  notificationSound: boolean
  detectReferences: boolean
  referencePrefixes: string[]
}

export interface Category {
  id: number
  name: string
  color: string
  sortOrder: number
  isActive: boolean
}

export interface Snapshot {
  settings: Settings
  categories: Category[]
  captureCategory: number | null
  databasePath: string
  launchAtLogin: boolean | null
  x11Shortcuts: boolean
  shortcutError: string | null
  tracking: TrackingState
}

export interface Reference {
  prefix: string
  number: string
}

export interface Entry {
  id: number
  description: string
  reference: Reference | null
  categoryId: number | null
  activityAt: number
  endAt: number | null
  kind: EntryKind
  isRunning: boolean
  createdAt: number
  updatedAt: number
}

export interface EntryInput {
  description: string
  categoryId: number | null
  activityAt: number | null
  endAt: number | null
  updateEnd: boolean
  kind: EntryKind
  reference: Reference | null
  autoReference: boolean
}

export interface WorkSession {
  id: number
  startedAt: number
  endedAt: number | null
}

export interface WorkdaySummary {
  sessions: WorkSession[]
  totals: { workedMs: number; breakMs: number } | null
}

export interface TrackingState {
  session: WorkSession | null
  entry: Entry | null
}

export interface EntryQuery {
  date: string | null
  search: string
  categoryId: number | null
  uncategorized: boolean
}

export interface Review {
  date: string
  notes: string
  status: ReviewStatus
  reviewedAt: number | null
}

export type ReviewAction = 'saveNotes' | 'markReviewed' | 'reopen'
export type DashboardRoute = { page: 'journal' | 'settings' } | { page: 'review'; date: string }
export type Transfer = 'backup' | 'restore' | 'export'

export const api = {
  snapshot: () => invoke<Snapshot>('snapshot'),
  captureContext: () => invoke<CaptureContext>('capture_context'),
  hideCapture: (generation: number) => invoke<void>('hide_capture', { generation }),
  openCapture: (mode: CaptureMode = 'quick') => invoke<void>('open_capture', { mode }),
  takeRoute: () => invoke<DashboardRoute | null>('take_dashboard_route'),
  capture: (description: string, categoryId: number | null, captureKey: string, mode: CaptureMode) =>
    invoke<Entry>('capture_entry', { description, categoryId, captureKey, mode }),
  track: (action: TrackingAction) => invoke<void>('track_work', { action }),
  workdaySummary: (date: string) => invoke<WorkdaySummary>('workday_summary', { date }),
  updateWorkSession: (session: WorkSession, startedAt: number, endedAt: number | null) =>
    invoke<void>('update_work_session', { session, startedAt, endedAt }),
  deleteWorkSession: (session: WorkSession) => invoke<void>('delete_work_session', { session }),
  entries: (query: EntryQuery) => invoke<Entry[]>('list_entries', { query }),
  saveEntry: (id: number | null, input: EntryInput) => invoke<Entry>('save_entry', { id, input }),
  deleteEntry: (id: number) => invoke<void>('delete_entry', { id }),
  review: (date: string) => invoke<Review>('day_review', { date }),
  monthReviews: (month: string) => invoke<Review[]>('month_reviews', { month }),
  saveReview: (date: string, notes: string, action: ReviewAction) =>
    invoke<Review>('save_review', { date, notes, action }),
  saveCategory: (input: Omit<Category, 'id' | 'sortOrder'> & { id: number | null }) =>
    invoke<void>('save_category', { input }),
  reorderCategories: (ids: number[]) => invoke<void>('reorder_categories', { ids }),
  savePreferences: (settings: Settings) => invoke<Settings>('save_preferences', { settings }),
  setLaunchAtLogin: (enabled: boolean) => invoke<void>('set_launch_at_login', { enabled }),
  transfer: (action: Transfer) => invoke<string | null>('transfer_data', { action }),
  testNotification: () => invoke<void>('test_notification'),
  quit: () => invoke<void>('quit_worklog'),
}

export function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
