/**
 * Session label that introduces each clustered reading session inside a day.
 *
 * ## Responsibilities
 * - Render the mono time-range token followed by the italic-serif label that
 *   names the session (e.g. "Rust async runtime deep dive").
 * - Provide the subtle border-bottom that separates the label from the cards
 *   beneath it, matching `.cs-session-header` from the handoff.
 *
 * ## Not responsible for
 * - Naming the session — caller supplies the label (often a topic synthesis
 *   from local intelligence or "Session" as a deterministic fallback).
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperSessionHeaderProps {
  timeRange: ReactNode
  label?: ReactNode
  className?: string
  testId?: string
}

export function PaperSessionHeader({
  timeRange,
  label,
  className,
  testId,
}: PaperSessionHeaderProps) {
  return (
    <div
      className={cn(
        'border-border-light flex items-baseline gap-3 border-b pb-2 mb-[14px]',
        className,
      )}
      data-testid={testId}
    >
      <span className="text-ink-muted font-mono text-[11px] tracking-[0.01em]">
        {timeRange}
      </span>
      {label ? (
        <span className="text-ink-faint font-serif italic text-[14px]">
          {label}
        </span>
      ) : null}
    </div>
  )
}
