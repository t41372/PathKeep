/**
 * Gap indicator that sits between two consecutive sessions inside a single
 * day, telling the user "you stopped browsing for this long".
 *
 * ## Responsibilities
 * - Render a slim dashed-line separator centred on a duration label so the
 *   user can read at-a-glance "31 min" between sessions instead of having
 *   to subtract two clock times.
 *
 * ## Not responsible for
 * - Computing the gap duration — caller passes it pre-formatted via the
 *   shared `formatDuration` so a single i18n contract owns wording.
 * - Deciding when to render. The contact sheet only renders this between
 *   sessions whose break exceeds `SESSION_GAP_MINUTES` (currently 30).
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperSessionGapProps {
  label: ReactNode
  className?: string
  testId?: string
}

export function PaperSessionGap({
  label,
  className,
  testId,
}: PaperSessionGapProps) {
  return (
    <div
      className={cn('text-ink-faint my-4 flex items-center gap-3', className)}
      data-testid={testId}
    >
      <span
        aria-hidden="true"
        className="border-border-light flex-1 border-t border-dashed"
      />
      <span className="font-serif text-[11.5px] italic leading-none">
        {label}
      </span>
      <span
        aria-hidden="true"
        className="border-border-light flex-1 border-t border-dashed"
      />
    </div>
  )
}
