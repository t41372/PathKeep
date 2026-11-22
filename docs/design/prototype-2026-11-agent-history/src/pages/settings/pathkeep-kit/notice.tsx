/** A tinted callout inside a settings dialog or row. Mirrors `src/features/settings/notice.tsx`. */
import { TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'

export function Notice({ tone, children }: { tone: 'warning' | 'info'; children: ReactNode }) {
  return (
    <div className={tone === 'warning' ? 'flex items-start gap-2.5 rounded-lg bg-brand-soft p-3 text-[13px]' : 'flex items-start gap-2.5 rounded-lg bg-muted p-3 text-[13px] text-muted-foreground'}>
      {tone === 'warning' && <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />}
      <span>{children}</span>
    </div>
  )
}

/** The red line a dialog shows when its step failed. */
export function ErrorLine({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <p id={id} role="alert" className="text-[13px] [overflow-wrap:anywhere] text-destructive">
      {children}
    </p>
  )
}
