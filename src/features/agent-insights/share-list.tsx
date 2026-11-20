/**
 * Rows that compare one amount: a label, a number and a bar under them
 * as long as the row's share of the largest. Used by the cards that rank
 * projects, tools, models and tool calls.
 */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface ShareRow {
  key: string
  label: ReactNode
  /** What the bar measures; `null` draws no bar (nothing on record). */
  value: number | null
  display: ReactNode
  sub?: ReactNode
  color?: string
}

export function ShareList({
  rows,
  className,
}: {
  rows: ShareRow[]
  className?: string
}) {
  const max = Math.max(1e-9, ...rows.map((row) => row.value ?? 0))
  return (
    <ul className={cn('flex flex-col gap-2.5', className)}>
      {rows.map((row) => (
        <li key={row.key} className="flex min-w-0 flex-col gap-1">
          <div className="flex min-w-0 items-center gap-2 text-[13px]">
            <span className="flex min-w-0 flex-1 items-center gap-1.5">
              {row.label}
            </span>
            {row.sub && (
              <span className="shrink-0 text-xs text-muted-foreground tabular">
                {row.sub}
              </span>
            )}
            <span className="w-[76px] shrink-0 text-right font-medium tabular">
              {row.display}
            </span>
          </div>
          <div
            aria-hidden
            className="h-1.5 overflow-hidden rounded-full bg-muted"
          >
            <span
              className="block h-full origin-left rounded-full transition-transform duration-500 ease-out"
              style={{
                transform: `scaleX(${(row.value ?? 0) / max})`,
                background: row.color ?? 'var(--brand)',
              }}
            />
          </div>
        </li>
      ))}
    </ul>
  )
}
