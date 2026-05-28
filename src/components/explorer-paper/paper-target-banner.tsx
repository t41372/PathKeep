/**
 * "Target landing" banner shown above the contact sheet when Browse is
 * reached from an external jump — On This Day, Search "See in context",
 * Intelligence drill-in.
 *
 * ## Responsibilities
 * - Render the kicker ("From 'On this day'" / "From search · 'query'"), the
 *   pretty date, and the status sentence (page count or "no archive" fallback).
 * - Render a "Clear ×" pill that drops the target filter back to default.
 * - Vary background tint by source (search vs on-this-day) so users can
 *   distinguish the entry point visually.
 *
 * ## Not responsible for
 * - Resolving the target date itself; caller passes ready-formatted strings.
 * - The pulse glow on the landed entry; that's `cs-pulse` applied to the
 *   row directly by the Browse view.
 *
 * ## Dependencies
 * - Paper tokens (`bg-accent-soft`, `text-accent`) via `src/styles/tokens.css`.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export type PaperTargetBannerSource = 'on-this-day' | 'search' | 'intelligence'

export interface PaperTargetBannerProps {
  source: PaperTargetBannerSource
  kicker: ReactNode
  date: ReactNode
  status: ReactNode
  onClear: () => void
  clearLabel: string
  className?: string
  testId?: string
}

export function PaperTargetBanner({
  source,
  kicker,
  date,
  status,
  onClear,
  clearLabel,
  className,
  testId,
}: PaperTargetBannerProps) {
  return (
    <div
      role="status"
      className={cn(
        'mb-4 flex items-center justify-between gap-3',
        'rounded-paper border border-accent border-l-[3px]',
        'py-[10px] px-[14px]',
        source === 'search'
          ? 'bg-[color-mix(in_srgb,var(--accent)_6%,var(--bg-paper))]'
          : 'bg-accent-soft',
        className,
      )}
      data-source={source}
      data-testid={testId}
    >
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="text-accent font-mono text-[9.5px] uppercase tracking-[0.1em] opacity-85">
          {kicker}
        </span>
        <div className="flex flex-wrap items-baseline gap-[10px] min-w-0">
          <span className="text-ink font-serif text-[16px] tracking-[-0.005em] whitespace-nowrap">
            {date}
          </span>
          <span className="text-ink-muted font-serif italic text-[12.5px]">
            {status}
          </span>
        </div>
      </div>
      <button
        type="button"
        onClick={onClear}
        aria-label={clearLabel}
        className={cn(
          'shrink-0 whitespace-nowrap font-mono text-[10px] tracking-[0.04em]',
          'border-border-default text-ink-muted bg-transparent',
          'rounded-pill border px-[10px] py-[3px]',
          'hover:text-ink hover:border-ink-muted hover:bg-card-paper',
          'transition-colors duration-150',
        )}
      >
        {clearLabel} ×
      </button>
    </div>
  )
}
