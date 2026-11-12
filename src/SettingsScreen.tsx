import {
  ArrowDown, ArrowUp, BellRing, Download, FolderOpen, HardDriveDownload, Pencil, Plus, Power,
} from 'lucide-react'
import { useState, type FormEvent, type ReactNode } from 'react'
import { api, errorText, type Category, type Settings, type Snapshot, type Transfer } from './api'
import { CategoryOptions, ErrorNotice, Modal, SuccessNotice } from './ui'

const sections = ['General', 'Quick Capture', 'Reminders', 'References', 'Categories', 'Data'] as const
const weekdays = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']

interface Props {
  snapshot: Snapshot
  onSaved: () => void
}

export default function SettingsScreen({ snapshot, onSaved }: Props) {
  const [section, setSection] = useState<typeof sections[number]>('General')
  const [patch, setPatch] = useState<Partial<Settings>>({})
  const [prefixes, setPrefixes] = useState<string>()
  const [busy, setBusy] = useState('')
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [category, setCategory] = useState<Category | 'new'>()
  const [quit, setQuit] = useState(false)
  const settings = { ...snapshot.settings, ...patch }
  const originalPrefixes = snapshot.settings.referencePrefixes.join(', ')
  const dirty = Object.entries(patch).some(([key, value]) =>
    JSON.stringify(value) !== JSON.stringify(snapshot.settings[key as keyof Settings]),
  ) || (prefixes !== undefined && prefixes !== originalPrefixes)
  const defaultCategory = snapshot.categories.some(category => category.isActive && category.id === settings.defaultCategory)
    ? settings.defaultCategory : null

  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    setPatch(previous => ({ ...previous, [key]: value }))
    setNotice('')
    setError('')
  }

  async function perform<T>(task: string, action: () => Promise<T>, success: (value: T) => string = () => '') {
    if (busy) return
    setBusy(task)
    setError('')
    setNotice('')
    try {
      setNotice(success(await action()))
    } catch (error) {
      setError(errorText(error))
    } finally {
      setBusy('')
    }
  }

  function save(event: FormEvent) {
    event.preventDefault()
    if (!dirty) return
    void perform('save', async () => {
      const referencePrefixes = (prefixes ?? originalPrefixes).split(/[\s,]+/).filter(Boolean)
      await api.savePreferences({ ...settings, defaultCategory, referencePrefixes })
      setPatch({})
      setPrefixes(undefined)
      onSaved()
    }, () => 'Settings saved')
  }

  function move(index: number, offset: number) {
    const ids = snapshot.categories.map(category => category.id)
    const next = index + offset
    if (next < 0 || next >= ids.length) return
    const [moved] = ids.splice(index, 1)
    ids.splice(next, 0, moved)
    void perform('category-order', () => api.reorderCategories(ids), () => {
      onSaved()
      return 'Category order saved'
    })
  }

  function transfer(action: Transfer) {
    void perform(action, () => api.transfer(action), path => {
      if (!path || action === 'restore') return ''
      return (action === 'backup' ? 'Backup saved to ' : 'Entries exported to ') + path
    })
  }

  return (
    <div className="settings-page">
      <h1>Settings</h1>
      <nav className="settings-tabs" aria-label="Settings sections">
        {sections.map(item => <button key={item} aria-current={section === item ? 'page' : undefined} onClick={() => {
          setSection(item)
          setNotice('')
          setError('')
        }}>{item}</button>)}
      </nav>
      <form className="settings-card" onSubmit={save}>
        <h2>{section}</h2>

        {section === 'General' && <>
          <SettingRow title="Launch at login" detail="Start WorkLog in the background when you sign in.">
            <Toggle label="Launch at login" checked={snapshot.launchAtLogin === true} disabled={!!busy || snapshot.launchAtLogin === null}
              onChange={enabled => void perform('login', () => api.setLaunchAtLogin(enabled), () => {
                onSaved()
                return enabled ? 'Launch at login enabled' : 'Launch at login disabled'
              })} />
          </SettingRow>
          {snapshot.launchAtLogin === null && <p className="help">Could not read your desktop’s launch-at-login setting.</p>}
          <SettingRow title="Minimize to tray" detail="Hide the dashboard when you close it. Turning this off minimizes it to the dock.">
            <Toggle label="Minimize to tray" checked={settings.minimizeToTray} onChange={value => update('minimizeToTray', value)} />
          </SettingRow>
          <SettingRow title="Appearance">
            <select aria-label="Appearance" value={settings.theme} onChange={event => update('theme', event.target.value as Settings['theme'])}>
              <option value="system">Follow system</option><option value="light">Light</option><option value="dark">Dark</option>
            </select>
          </SettingRow>
          <SettingRow title="Time format">
            <select aria-label="Time format" value={settings.timeFormat} onChange={event => update('timeFormat', event.target.value as Settings['timeFormat'])}>
              <option value="24h">24-hour</option><option value="12h">12-hour</option>
            </select>
          </SettingRow>
          <SettingRow title="Quit WorkLog" detail="Close both windows and stop background reminders.">
            <button type="button" className="button secondary" onClick={() => setQuit(true)} disabled={!!busy}><Power size={15} /> Quit WorkLog</button>
          </SettingRow>
        </>}

        {section === 'Quick Capture' && <>
          <SettingRow title="Hide after saving">
            <Toggle label="Hide after saving" checked={settings.hideAfterSave} onChange={value => update('hideAfterSave', value)} />
          </SettingRow>
          <SettingRow title="Show save confirmation">
            <Toggle label="Show save confirmation" checked={settings.showConfirmation} onChange={value => update('showConfirmation', value)} />
          </SettingRow>
          <SettingRow title="Default category">
            <select aria-label="Default category" value={defaultCategory ?? ''} onChange={event => update('defaultCategory', event.target.value ? Number(event.target.value) : null)}>
              <option value="">No category</option>
              <CategoryOptions categories={snapshot.categories} />
            </select>
          </SettingRow>
          <SettingRow title="Remember last category" detail="Start with the category used for the last quick entry.">
            <Toggle label="Remember last category" checked={settings.rememberCategory} onChange={value => update('rememberCategory', value)} />
          </SettingRow>
          {snapshot.x11Shortcuts && <SettingRow title="Global shortcut" detail="Enter a combination such as Ctrl+Shift+Space. Leave it blank to disable.">
            <input aria-label="Global shortcut" value={settings.shortcut} placeholder="Ctrl+Shift+Space" maxLength={80}
              onChange={event => update('shortcut', event.target.value)} />
          </SettingRow>}
          {snapshot.shortcutError && <ErrorNotice>{snapshot.shortcutError}</ErrorNotice>}
          <div className="shortcut-help">
            <h3>{snapshot.x11Shortcuts ? 'Ubuntu keyboard shortcut' : 'Keyboard shortcut on Wayland'}</h3>
            <p>In Ubuntu Settings, open Keyboard → View and Customize Shortcuts → Custom Shortcuts. Add a shortcut with this command after installing WorkLog.</p>
            <code>/usr/bin/worklog --quick-add</code>
          </div>
        </>}

        {section === 'Reminders' && <>
          <SettingRow title="Daily review reminder" detail="Show a desktop notification when your day is ready to review.">
            <Toggle label="Daily review reminder" checked={settings.remindersEnabled} onChange={value => update('remindersEnabled', value)} />
          </SettingRow>
          <SettingRow title="Reminder time">
            <input type="time" required aria-label="Reminder time" value={settings.reminderTime} onChange={event => update('reminderTime', event.target.value)} />
          </SettingRow>
          <SettingRow title="Working days">
            <div className="weekday-picker" role="group" aria-label="Working days">
              {weekdays.map((day, index) => <button key={day} type="button" aria-label={day} title={day}
                aria-pressed={settings.workingDays.includes(index + 1)} onClick={() => {
                  const number = index + 1
                  update('workingDays', settings.workingDays.includes(number)
                    ? settings.workingDays.filter(value => value !== number)
                    : [...settings.workingDays, number].sort((a, b) => a - b))
                }}>{day.slice(0, 2)}</button>)}
            </div>
          </SettingRow>
          <SettingRow title="Snooze duration">
            <label className="inline-field">
              <input type="number" min={1} max={180} required aria-label="Snooze duration in minutes"
                value={settings.snoozeMinutes || ''} onChange={event => update('snoozeMinutes', Number(event.target.value))} /> minutes
            </label>
          </SettingRow>
          <SettingRow title="Notification sound" detail="Use the desktop’s notification sound when available.">
            <Toggle label="Notification sound" checked={settings.notificationSound} onChange={value => update('notificationSound', value)} />
          </SettingRow>
          <SettingRow title="Test notifications">
            <button type="button" className="button secondary" disabled={!!busy}
              onClick={() => void perform('notification', api.testNotification, () => 'Test notification sent')}><BellRing size={15} /> Send test</button>
          </SettingRow>
        </>}

        {section === 'References' && <>
          <SettingRow title="Detect references automatically" detail="Recognize references such as NCR 2456 and NCR-2456 in new activities.">
            <Toggle label="Detect references automatically" checked={settings.detectReferences} onChange={value => update('detectReferences', value)} />
          </SettingRow>
          <SettingRow title="Reference prefixes" detail="Separate prefixes with commas.">
            <input aria-label="Reference prefixes" value={prefixes ?? originalPrefixes} placeholder="NCR, OPS" maxLength={576}
              onChange={event => { setPrefixes(event.target.value); setNotice(''); setError('') }} />
          </SettingRow>
        </>}

        {section === 'Categories' && <>
          <div className="category-list">
            {snapshot.categories.map((item, index) => (
              <div className="category-row" key={item.id}>
                <span className="color-dot" style={{ backgroundColor: item.color }} />
                <span className="category-name">{item.name}</span>
                {!item.isActive && <span className="inactive-tag">Inactive</span>}
                <div className="category-actions">
                  <button type="button" className="icon-button" aria-label={'Move ' + item.name + ' up'} title="Move up"
                    disabled={!!busy || index === 0} onClick={() => move(index, -1)}><ArrowUp size={15} /></button>
                  <button type="button" className="icon-button" aria-label={'Move ' + item.name + ' down'} title="Move down"
                    disabled={!!busy || index === snapshot.categories.length - 1} onClick={() => move(index, 1)}><ArrowDown size={15} /></button>
                  <button type="button" className="icon-button" aria-label={'Edit ' + item.name} title="Edit category"
                    disabled={!!busy} onClick={() => setCategory(item)}><Pencil size={15} /></button>
                </div>
              </div>
            ))}
          </div>
          <button type="button" className="button secondary settings-add-category" disabled={!!busy}
            onClick={() => setCategory('new')}><Plus size={16} /> Add category</button>
        </>}

        {section === 'Data' && <>
          <div className="database-location"><h3>Database location</h3><code>{snapshot.databasePath}</code></div>
          <SettingRow title="Back up journal" detail="Save a SQLite copy of your entries, categories, reviews, and preferences.">
            <button type="button" className="button secondary" disabled={!!busy} onClick={() => transfer('backup')}>
              <HardDriveDownload size={16} /> Back up
            </button>
          </SettingRow>
          <SettingRow title="Restore from backup" detail="Replace this journal with a WorkLog backup.">
            <button type="button" className="button secondary" disabled={!!busy} onClick={() => transfer('restore')}>
              <FolderOpen size={16} /> Choose backup
            </button>
          </SettingRow>
          <SettingRow title="Export activities" detail="Save all entries as a CSV with UTC timestamps.">
            <button type="button" className="button secondary" disabled={!!busy} onClick={() => transfer('export')}>
              <Download size={16} /> Export CSV
            </button>
          </SettingRow>
        </>}

        {error && <ErrorNotice>{error}</ErrorNotice>}
        {notice && <SuccessNotice>{notice}</SuccessNotice>}
        {dirty && <div className="settings-footer">
          <span className="muted">Unsaved changes</span>
          <button type="button" className="button secondary" disabled={!!busy} onClick={() => { setPatch({}); setPrefixes(undefined); setError('') }}>Discard</button>
          <button type="submit" className="button primary" disabled={!!busy}>{busy === 'save' ? 'Saving…' : 'Save changes'}</button>
        </div>}
      </form>
      {category && <CategoryEditor category={category === 'new' ? undefined : category} onDismiss={() => setCategory(undefined)}
        onSaved={() => { setCategory(undefined); onSaved(); setNotice('Category saved') }} />}
      {quit && <Modal title="Quit WorkLog?" onDismiss={() => setQuit(false)}>
        <p>{dirty ? 'You have unsaved settings. Quitting will discard them and stop reminders.' : 'Background reminders will stop until you reopen WorkLog.'}</p>
        {error && <ErrorNotice>{error}</ErrorNotice>}
        <div className="modal-actions">
          <button autoFocus className="button secondary" onClick={() => setQuit(false)}>Cancel</button>
          <button className="button primary" onClick={() => void perform('quit', api.quit)}>Quit WorkLog</button>
        </div>
      </Modal>}
    </div>
  )
}

function SettingRow({ title, detail, children }: { title: string; detail?: string; children: ReactNode }) {
  return <div className="setting-row">
    <div><h3>{title}</h3>{detail && <p className="help">{detail}</p>}</div>
    <div className="setting-control">{children}</div>
  </div>
}

function Toggle({ label, checked, onChange, disabled }: { label: string; checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean }) {
  return <label className="toggle"><input type="checkbox" role="switch" aria-label={label} checked={checked} disabled={disabled}
    onChange={event => onChange(event.target.checked)} /><span aria-hidden="true" /></label>
}

function CategoryEditor({ category, onDismiss, onSaved }: { category?: Category; onDismiss: () => void; onSaved: () => void }) {
  const [name, setName] = useState(category?.name ?? '')
  const [color, setColor] = useState(category?.color ?? '#6755d5')
  const [isActive, setActive] = useState(category?.isActive ?? true)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  async function save(event: FormEvent) {
    event.preventDefault()
    if (busy) return
    setError('')
    setBusy(true)
    try {
      await api.saveCategory({ id: category?.id ?? null, name, color, isActive })
      onSaved()
    } catch (error) {
      setError(errorText(error))
      setBusy(false)
    }
  }

  return <Modal title={category ? 'Edit category' : 'Add category'} onDismiss={() => { if (!busy) onDismiss() }}>
    <form className="stack-form" onSubmit={save}>
      <label className="field">Name<input autoFocus required maxLength={60} value={name} onChange={event => setName(event.target.value)} /></label>
      <label className="field">Color<input type="color" value={color} onChange={event => setColor(event.target.value)} /></label>
      <SettingRow title="Active" detail="Inactive categories remain visible on existing activities.">
        <Toggle label="Active category" checked={isActive} onChange={setActive} />
      </SettingRow>
      {error && <ErrorNotice>{error}</ErrorNotice>}
      <div className="modal-actions">
        <button type="button" className="button secondary" disabled={busy} onClick={onDismiss}>Cancel</button>
        <button type="submit" className="button primary" disabled={busy || !name.trim()}>{busy ? 'Saving…' : 'Save category'}</button>
      </div>
    </form>
  </Modal>
}
