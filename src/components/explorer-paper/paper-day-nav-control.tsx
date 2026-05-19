/**
 * Day navigation pill — prev / current-day pill / next / Today.
 *
 * Anchors the Browse toolbar and exposes the keyboard-driven navigation
 * model from the design handoff (`pk-browse-nav.jsx` → `DayNavControl`):
 *
 *   ◀  [ FRI · May 16 · 2026  •  1,234p · 3w ago  ⌄ ]  ▶   |   Today
 *
 * ## Responsibilities
 * - Render the three navigation buttons (prev / pill / next) plus the
 *   secondary "Today" button after the separator.
 * - Decorate the pill with the day-of-week, formatted month/day, year,
 *   density-tier swatch, page count, and a relative-time hint.
 * - Reflect the open/closed state of the calendar via aria-expanded + a
 *   data-attribute consumers can hook for sticky chrome styling.
 *
 * ## Not responsible for
 * - Date math: callers pass already-formatted strings (dow, monthDay, year,
 *   relativeAgo, countLabel). Use `date-helpers.ts` to derive them.
 * - Owning the calendar popover; this component just toggles a flag.
 * - Disabling prev/next at archive bounds; pass `prevDisabled`/`nextDisabled`.
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperDayNavControlProps {
  /** Current day-of-week label, e.g. "FRI" (mono, small-caps). */
  dow: string
  /** Pretty month + day, e.g. "May 16" (serif). */
  monthDay: string
  /** Year, e.g. "2026" (mono). */
  year: string
  /** Density tier 0..4 — drives the small square swatch beside the count. */
  densityTier: 0 | 1 | 2 | 3 | 4
  /** Already-formatted page count, e.g. "1,234p" or "empty". */
  countLabel: string
  /** Relative-time hint, e.g. "yesterday" / "3w ago" / "today". */
  relativeAgo: string
  /** True when the calendar popover is currently open. */
  calOpen: boolean
  /** True when the displayed day is "today" (per the archive's anchor). */
  isToday: boolean
  prevDisabled?: boolean
  nextDisabled?: boolean
  onPrev: () => void
  onNext: () => void
  onToday: () => void
  onToggleCal: () => void
  /** i18n strings — caller owns translations. */
  copy: PaperDayNavControlCopy
  className?: string
  testId?: string
  /** Optional slot rendered after the pill (e.g. mounted calendar popover). */
  calendarSlot?: ReactNode
}

export interface PaperDayNavControlCopy {
  prev: string
  next: string
  today: string
  openCalendar: string
}

export function PaperDayNavControl({
  dow,
  monthDay,
  year,
  densityTier,
  countLabel,
  relativeAgo,
  calOpen,
  isToday,
  prevDisabled = false,
  nextDisabled = false,
  onPrev,
  onNext,
  onToday,
  onToggleCal,
  copy,
  className,
  testId,
  calendarSlot,
}: PaperDayNavControlProps) {
  return (
    <div
      className={cn('relative inline-flex items-center gap-[6px]', className)}
      data-testid={testId}
      data-cal-open={calOpen ? 'true' : undefined}
    >
      <button
        type="button"
        onClick={onPrev}
        disabled={prevDisabled}
        title={copy.prev}
        aria-label={copy.prev}
        className={cn(
          'border-border-default bg-card-paper text-ink-muted',
          'rounded-paper inline-flex h-7 w-7 items-center justify-center border',
          'transition-colors duration-150',
          'hover:border-ink-muted hover:text-ink',
          'disabled:cursor-not-allowed disabled:opacity-40',
        )}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M10 3l-5 5 5 5" />
        </svg>
      </button>

      <button
        type="button"
        onClick={onToggleCal}
        aria-haspopup="dialog"
        aria-expanded={calOpen}
        aria-label={copy.openCalendar}
        title={copy.openCalendar}
        data-density={`t${densityTier}`}
        className={cn(
          'inline-flex h-7 min-w-[260px] items-center gap-[10px] whitespace-nowrap px-2',
          'border-border-default bg-card-paper text-ink-secondary',
          'rounded-paper border transition-colors duration-150',
          'hover:border-ink-muted hover:text-ink',
          calOpen && 'border-accent bg-accent-soft text-ink',
        )}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
          className={cn('shrink-0', calOpen ? 'text-accent' : 'text-ink-muted')}
        >
          <rect x="2.2" y="3.2" width="11.6" height="10.6" rx="1.2" />
          <path d="M2.2 6.4h11.6" />
          <path d="M5.4 1.8v2.6" />
          <path d="M10.6 1.8v2.6" />
        </svg>
        <span className="inline-flex min-w-0 items-baseline gap-[6px]">
          <span className="text-ink-faint font-mono text-[9.5px] uppercase tracking-[0.08em]">
            {dow}
          </span>
          <span className="text-ink font-serif text-[13.5px] tracking-[-0.005em]">
            {monthDay}
          </span>
          <span className="text-ink-muted font-mono text-[11px] tracking-[0.02em]">
            {year}
          </span>
        </span>
        <span
          className={cn(
            'ml-auto inline-flex items-center gap-2 border-l pl-[10px]',
            'font-mono text-[10px] tracking-[0.02em]',
            calOpen ? 'border-accent-medium' : 'border-border-light',
          )}
        >
          <DensitySwatch tier={densityTier} />
          <span className="text-ink-secondary">{countLabel}</span>
          <span className="text-ink-faint">{relativeAgo}</span>
        </span>
        <svg
          width="9"
          height="9"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
          className="opacity-55"
        >
          <path d="M4 6l4 4 4-4" />
        </svg>
      </button>

      <button
        type="button"
        onClick={onNext}
        disabled={nextDisabled}
        title={copy.next}
        aria-label={copy.next}
        className={cn(
          'border-border-default bg-card-paper text-ink-muted',
          'rounded-paper inline-flex h-7 w-7 items-center justify-center border',
          'transition-colors duration-150',
          'hover:border-ink-muted hover:text-ink',
          'disabled:cursor-not-allowed disabled:opacity-40',
        )}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M6 3l5 5-5 5" />
        </svg>
      </button>

      <span
        aria-hidden="true"
        className="bg-border-default mx-1 inline-block h-[18px] w-px"
      />

      <button
        type="button"
        onClick={onToday}
        title={copy.today}
        className={cn(
          'border-border-default bg-card-paper text-ink-muted',
          'rounded-paper inline-flex h-7 items-center justify-center border px-[10px]',
          'font-sans text-[11.5px] font-medium tracking-[0.01em]',
          'transition-colors duration-150',
          'hover:border-ink-muted hover:text-ink',
          isToday && 'border-accent bg-accent-soft text-accent-text',
        )}
      >
        {copy.today}
      </button>

      {calendarSlot}
    </div>
  )
}

function DensitySwatch({ tier }: { tier: 0 | 1 | 2 | 3 | 4 }) {
  // Inline colour expressions mirror pk-tokens.css `.cs-daynav__pill-dot--t*`.
  const styles: Record<
    0 | 1 | 2 | 3 | 4,
    { background: string; border: string }
  > = {
    0: {
      background: 'var(--bg-page)',
      border: 'color-mix(in srgb, var(--ink-faint) 25%, transparent)',
    },
    1: {
      background: 'color-mix(in srgb, var(--accent) 22%, var(--bg-page))',
      border: 'color-mix(in srgb, var(--accent) 30%, transparent)',
    },
    2: {
      background: 'color-mix(in srgb, var(--accent) 45%, var(--bg-page))',
      border: 'color-mix(in srgb, var(--accent) 55%, transparent)',
    },
    3: {
      background: 'color-mix(in srgb, var(--accent) 70%, var(--bg-page))',
      border: 'color-mix(in srgb, var(--accent) 75%, transparent)',
    },
    4: {
      background: 'var(--accent)',
      border: 'var(--accent)',
    },
  }
  const { background, border } = styles[tier]
  return (
    <span
      aria-hidden="true"
      className="inline-block h-2 w-2 rounded-[2px] border"
      style={{ background, borderColor: border }}
      data-tier={`t${tier}`}
    />
  )
}
