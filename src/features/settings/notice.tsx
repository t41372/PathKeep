/** A tinted callout inside a settings dialog or row: a warning or a plain note. */
import { TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'

export function Notice({
  tone,
  children,
}: {
  tone: 'warning' | 'info'
  children: ReactNode
}) {
  return (
    <div
      className={
        tone === 'warning'
          ? 'flex items-start gap-2.5 rounded-lg bg-brand-soft p-3 text-[13px]'
          : 'flex items-start gap-2.5 rounded-lg bg-muted p-3 text-[13px] text-muted-foreground'
      }
    >
      {tone === 'warning' && (
        <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
      )}
      <span>{children}</span>
    </div>
  )
}
