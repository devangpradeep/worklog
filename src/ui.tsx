import { AlertCircle, Check, NotebookText } from 'lucide-react'
import { useEffect, useRef, type ReactNode } from 'react'
import type { Category } from './api'

export function Brand({ small = false }: { small?: boolean }) {
  return <span className={small ? 'brand-icon brand-icon-small' : 'brand-icon'} aria-hidden="true"><NotebookText /></span>
}

export function ErrorNotice({ children }: { children: ReactNode }) {
  return <div className="notice notice-error" role="alert"><AlertCircle size={16} /><span>{children}</span></div>
}

export function SuccessNotice({ children }: { children: ReactNode }) {
  return <div className="notice notice-success" role="status"><Check size={16} /><span>{children}</span></div>
}

export function CategoryOptions({ categories, retained }: { categories: Category[]; retained?: number | null }) {
  return categories.filter(item => item.isActive || item.id === retained).map(item =>
    <option key={item.id} value={item.id}>{item.name}{item.isActive ? '' : ' (inactive)'}</option>,
  )
}

export function Modal({ title, onDismiss, children }: { title: string; onDismiss: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null)
  useEffect(() => {
    const dialog = ref.current
    dialog?.showModal()
    return () => dialog?.close()
  }, [])
  return (
    <dialog className="modal" ref={ref} aria-label={title} onCancel={event => { event.preventDefault(); onDismiss() }}>
      <h2>{title}</h2>
      {children}
    </dialog>
  )
}
