/** A failed backend call, shown where it happened: what failed, why, and what to do next. */
import { CircleAlert } from 'lucide-react'
import type { ReactNode } from 'react'

export function InlineError({
  title,
  detail,
  actions,
}: {
  title: string
  detail?: string
  actions?: ReactNode
}) {
  return (
    <div
      role="alert"
      className="flex animate-rise items-start gap-2.5 rounded-lg border border-destructive/25 bg-destructive/[0.06] px-3.5 py-3 text-[13px]"
    >
      <CircleAlert className="mt-0.5 size-4 shrink-0 text-destructive" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="font-medium text-destructive">{title}</span>
        {detail && (
          <span className="break-words text-muted-foreground">{detail}</span>
        )}
        {actions && (
          <div className="mt-1.5 flex flex-wrap gap-2">{actions}</div>
        )}
      </div>
    </div>
  )
}
