/**
 * Calendar popover that hangs off the day pill — month grid with density
 * tinting, an in-place year picker, and a footer with quick jumps.
 *
 * The visual contract follows `pk-browse-nav.jsx` → `CalendarPopover`:
 *
 *   ┌─────────────────────────┐
 *   │ ◀  May ▾ 2026  ▶        │
 *   │  M T W T F S S          │
 *   │  …  density-tinted grid │
 *   │ 2026-05-16 · 1,234p     │
 *   │ [T] Today  [1y] 1y ago  │
 *   └─────────────────────────┘
 *
 * ## Responsibilities
 * - Render the month grid with Monday-first columns and density tints t0..t4.
 * - Mark today, the selected day, days already loaded in the archive, and
 *   greyed-out future days.
 * - Provide hover-preview of any cell that updates the footer label.
 * - Toggle between day grid and year picker via the title button.
 * - Footer "Today" and "1 year ago" shortcuts.
 *
 * ## Not responsible for
 * - Density data; callers pass a `densityByDate` Map for the month plus a
 *   `loadedDates` set and bounds. See `date-helpers.ts` + the route data.
 * - Dismissing the popover on outside-click / Escape — owner controls that.
 */

import { useMemo, useState } from 'react'
import { cn } from '@/lib/cn'
import {
  addDaysIso,
  dateFromIso,
  dayDensityTier,
} from '@/pages/explorer/paper/date-helpers'

export interface PaperCalendarPopoverCopy {
  prevMonth: string
  nextMonth: string
  /** Month names in calendar order: January..December. */
  months: [
    string,
    string,
    string,
    string,
    string,
    string,
    string,
    string,
    string,
    string,
    string,
    string,
  ]
  /** Day-of-week single-letter labels in Monday-first order. */
  dowLabels: [string, string, string, string, string, string, string]
  today: string
  oneYearAgo: string
  /** Template, e.g. "{count} pages archived". */
  pagesArchived: string
  /** Template, e.g. "{active} active days · {total} pages". */
  monthSummary: string
  /** Template for the meta block, e.g. "{firstYear}–{lastYear} · {totalDays} days". */
  boundsMeta: string
}

export interface PaperCalendarPopoverBounds {
  firstIso: string
  lastIso: string
  firstYear: number
  lastYear: number
  totalDays: number
}

export interface PaperCalendarPopoverProps {
  /** Currently selected ISO date (drives initial month/year + selection ring). */
  value: string
  /** Today's anchor ISO — anything after this is considered "future". */
  todayIso: string
  /** Per-day visit count map; missing keys treated as zero. */
  densityByDate: ReadonlyMap<string, number>
  /** Per-year aggregate visit counts; missing keys treated as zero. */
  densityByYear: ReadonlyMap<number, number>
  /** Days already loaded in the route's BROWSE_DAYS set — get a green dot. */
  loadedDates: ReadonlySet<string>
  /** Archive bounds + summary used by the footer. */
  bounds: PaperCalendarPopoverBounds
  /** Peak per-day count, used to scale the preview density bar. */
  peakDailyCount: number
  onSelect: (iso: string) => void
  copy: PaperCalendarPopoverCopy
  className?: string
  testId?: string
}

export function PaperCalendarPopover({
  value,
  todayIso,
  densityByDate,
  densityByYear,
  loadedDates,
  bounds,
  peakDailyCount,
  onSelect,
  copy,
  className,
  testId,
}: PaperCalendarPopoverProps) {
  const initial = useMemo(() => dateFromIso(value), [value])
  const safeInitial = Number.isNaN(initial.getTime())
    ? dateFromIso(bounds.lastIso)
    : initial

  const [viewYear, setViewYear] = useState(safeInitial.getFullYear())
  const [viewMonth, setViewMonth] = useState(safeInitial.getMonth())
  const [hover, setHover] = useState<{ iso: string; count: number } | null>(
    null,
  )
  const [showYearPicker, setShowYearPicker] = useState(false)

  const { cells, monthTotal, monthActiveDays } = useMemo(() => {
    const firstDow = new Date(viewYear, viewMonth, 1).getDay()
    const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate()
    // Monday-first leading blanks.
    const leading = (firstDow + 6) % 7
    const list: ({ iso: string; day: number; count: number } | null)[] = []
    for (let i = 0; i < leading; i += 1) list.push(null)
    let monthTotalAccum = 0
    let monthActiveAccum = 0
    for (let day = 1; day <= daysInMonth; day += 1) {
      const iso = `${viewYear}-${String(viewMonth + 1).padStart(2, '0')}-${String(day).padStart(2, '0')}`
      const count = densityByDate.get(iso) ?? 0
      monthTotalAccum += count
      if (count > 0) monthActiveAccum += 1
      list.push({ iso, day, count })
    }
    while (list.length % 7 !== 0) list.push(null)
    return {
      cells: list,
      monthTotal: monthTotalAccum,
      monthActiveDays: monthActiveAccum,
    }
  }, [viewYear, viewMonth, densityByDate])

  const stepMonth = (delta: number) => {
    let nm = viewMonth + delta
    let ny = viewYear
    while (nm < 0) {
      nm += 12
      ny -= 1
    }
    while (nm > 11) {
      nm -= 12
      ny += 1
    }
    if (ny < bounds.firstYear || ny > bounds.lastYear) return
    setViewYear(ny)
    setViewMonth(nm)
  }

  const previewCount = hover ? hover.count : monthTotal
  const previewLabel = hover
    ? hover.iso
    : `${copy.months[viewMonth]} ${viewYear}`
  const previewSub = hover
    ? copy.pagesArchived.replace('{count}', hover.count.toLocaleString())
    : copy.monthSummary
        .replace('{active}', String(monthActiveDays))
        .replace('{total}', monthTotal.toLocaleString())

  return (
    <div
      role="dialog"
      aria-label="Calendar"
      data-testid={testId}
      className={cn(
        'absolute left-0 top-[calc(100%+8px)] z-[1000] w-[340px] overflow-hidden',
        'bg-card-paper border-border-default rounded-paper border',
        'shadow-frame-strong',
        className,
      )}
      onClick={(event) => event.stopPropagation()}
    >
      <div className="border-border-light grid grid-cols-[28px_1fr_28px] items-center gap-[6px] border-b p-[10px]">
        <button
          type="button"
          onClick={() => stepMonth(-1)}
          aria-label={copy.prevMonth}
          className="border-border-light text-ink-muted hover:border-ink-muted hover:text-ink hover:bg-hover rounded-paper inline-flex h-7 w-7 items-center justify-center border transition-colors duration-150"
        >
          <svg
            width="12"
            height="12"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M10 3l-5 5 5 5" />
          </svg>
        </button>
        <button
          type="button"
          onClick={() => setShowYearPicker((value) => !value)}
          aria-expanded={showYearPicker}
          aria-label={`${copy.months[viewMonth]} ${viewYear}`}
          className={cn(
            'rounded-paper inline-flex h-7 items-center justify-center gap-[6px] border border-transparent px-3',
            'transition-colors duration-150 hover:bg-hover',
            showYearPicker && 'bg-accent-soft border-accent-medium',
          )}
        >
          <span className="text-ink font-serif text-[15px] tracking-[-0.01em]">
            {copy.months[viewMonth]}
          </span>
          <span className="text-ink-muted font-mono text-[12px]">
            {viewYear}
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
            className="opacity-70"
          >
            <path d="M4 6l4 4 4-4" />
          </svg>
        </button>
        <button
          type="button"
          onClick={() => stepMonth(1)}
          aria-label={copy.nextMonth}
          className="border-border-light text-ink-muted hover:border-ink-muted hover:text-ink hover:bg-hover rounded-paper inline-flex h-7 w-7 items-center justify-center border transition-colors duration-150"
        >
          <svg
            width="12"
            height="12"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M6 3l5 5-5 5" />
          </svg>
        </button>
      </div>

      {showYearPicker ? (
        <YearPicker
          firstYear={bounds.firstYear}
          lastYear={bounds.lastYear}
          densityByYear={densityByYear}
          currentYear={viewYear}
          onSelect={(year) => {
            setViewYear(year)
            setShowYearPicker(false)
          }}
        />
      ) : (
        <>
          <div className="text-ink-faint grid grid-cols-7 gap-[2px] px-3 pb-1 pt-[10px] text-center font-mono text-[9.5px] uppercase tracking-[0.08em]">
            {copy.dowLabels.map((label, index) => (
              <span key={index}>{label}</span>
            ))}
          </div>

          <div className="grid grid-cols-7 gap-[2px] px-3 pb-[10px]">
            {cells.map((cell, index) => {
              if (!cell) {
                return (
                  <span
                    key={`empty-${index}`}
                    aria-hidden="true"
                    className="aspect-[1/0.9]"
                  />
                )
              }
              const tier = dayDensityTier(cell.count)
              const isToday = cell.iso === todayIso
              const isSelected = cell.iso === value
              const isLoaded = loadedDates.has(cell.iso)
              const isFuture = cell.iso > todayIso
              return (
                <button
                  key={cell.iso}
                  type="button"
                  disabled={isFuture}
                  onMouseEnter={() =>
                    setHover({ iso: cell.iso, count: cell.count })
                  }
                  onMouseLeave={() => setHover(null)}
                  onClick={() => onSelect(cell.iso)}
                  data-iso={cell.iso}
                  data-tier={`t${tier}`}
                  data-today={isToday ? 'true' : undefined}
                  data-selected={isSelected ? 'true' : undefined}
                  data-loaded={isLoaded ? 'true' : undefined}
                  className={cn(
                    'relative aspect-[1/0.9] rounded-[3px] font-mono text-[11px]',
                    'transition-[outline-color,transform] duration-75',
                    'border border-transparent',
                    densityClass(tier),
                    isToday && 'outline outline-[1.5px] outline-ink',
                    isSelected &&
                      'outline outline-[1.5px] outline-accent shadow-[0_0_0_2px_color-mix(in_srgb,var(--accent)_25%,transparent)]',
                    isFuture && 'cursor-not-allowed opacity-25',
                    'disabled:cursor-not-allowed',
                  )}
                >
                  <span className="leading-none">{cell.day}</span>
                  {isLoaded ? (
                    <span
                      aria-hidden="true"
                      className="bg-success absolute bottom-[2px] right-[2px] h-1 w-1 rounded-full"
                    />
                  ) : null}
                </button>
              )
            })}
          </div>

          <div className="border-border-light bg-paper flex items-center gap-3 border-t px-[14px] py-[10px]">
            <div className="flex min-w-0 flex-1 flex-col">
              <span className="text-ink font-serif text-[13px] tracking-[-0.005em]">
                {previewLabel}
              </span>
              <span className="text-ink-faint mt-px font-mono text-[10px] tracking-[0.01em]">
                {previewSub}
              </span>
            </div>
            <DensitySpark
              count={previewCount}
              max={Math.max(peakDailyCount, 1)}
            />
          </div>
        </>
      )}

      <div className="border-border-light bg-page flex items-center gap-[6px] border-t px-[10px] py-2">
        <button
          type="button"
          onClick={() => onSelect(todayIso)}
          className="border-border-light bg-card-paper text-ink-muted hover:border-ink-muted hover:text-ink rounded-paper inline-flex items-center gap-[6px] border px-[10px] py-1 font-sans text-[11px] transition-colors duration-150"
        >
          <kbd className="border-border-light bg-page rounded-[2px] border px-1 py-px font-mono text-[9px] tracking-[0.04em]">
            T
          </kbd>
          <span>{copy.today}</span>
        </button>
        <button
          type="button"
          onClick={() => onSelect(addDaysIso(value || bounds.lastIso, -365))}
          className="border-border-light bg-card-paper text-ink-muted hover:border-ink-muted hover:text-ink rounded-paper inline-flex items-center gap-[6px] border px-[10px] py-1 font-sans text-[11px] transition-colors duration-150"
        >
          <span>{copy.oneYearAgo}</span>
        </button>
        <span className="text-ink-faint ml-auto font-mono text-[9.5px] tracking-[0.02em]">
          {copy.boundsMeta
            .replace('{firstYear}', String(bounds.firstYear))
            .replace('{lastYear}', String(bounds.lastYear))
            .replace('{totalDays}', bounds.totalDays.toLocaleString())}
        </span>
      </div>
    </div>
  )
}

function densityClass(tier: 0 | 1 | 2 | 3 | 4): string {
  switch (tier) {
    case 0:
      return 'bg-page text-ink-ghost'
    case 1:
      return 'bg-[color-mix(in_srgb,var(--accent)_14%,var(--bg-page))] text-ink-secondary'
    case 2:
      return 'bg-[color-mix(in_srgb,var(--accent)_32%,var(--bg-page))] text-ink'
    case 3:
      return 'bg-[color-mix(in_srgb,var(--accent)_58%,var(--bg-page))] text-paper'
    case 4:
      return 'bg-accent text-paper'
  }
}

function DensitySpark({ count, max }: { count: number; max: number }) {
  const tier = dayDensityTier(count)
  const pct = Math.min(
    100,
    Math.sqrt(Math.max(count, 0) / Math.max(max, 1)) * 100,
  )
  const opacity =
    tier === 0
      ? 0
      : tier === 1
        ? 0.35
        : tier === 2
          ? 0.55
          : tier === 3
            ? 0.75
            : 1
  return (
    <span
      aria-hidden="true"
      className="bg-page block h-[6px] w-[80px] shrink-0 overflow-hidden rounded-[1px]"
    >
      <span
        className="bg-accent block h-full transition-[width] duration-200"
        style={{ width: `${pct}%`, opacity }}
      />
    </span>
  )
}

interface YearPickerProps {
  firstYear: number
  lastYear: number
  densityByYear: ReadonlyMap<number, number>
  currentYear: number
  onSelect: (year: number) => void
}

function YearPicker({
  firstYear,
  lastYear,
  densityByYear,
  currentYear,
  onSelect,
}: YearPickerProps) {
  const years: number[] = []
  for (let year = lastYear; year >= firstYear; year -= 1) years.push(year)
  const maxYearCount = years.reduce(
    (acc, year) => Math.max(acc, densityByYear.get(year) ?? 0),
    1,
  )

  return (
    <div className="pk-scrollbar max-h-[270px] overflow-y-auto px-[6px] py-1">
      {years.map((year) => {
        const count = densityByYear.get(year) ?? 0
        const tier = dayDensityTier(Math.min(count, 500))
        const pct = (count / maxYearCount) * 100
        const label =
          count === 0
            ? '—'
            : count >= 1000
              ? `${(count / 1000).toFixed(count >= 10_000 ? 0 : 1)}k`
              : count.toLocaleString()
        const isCurrent = year === currentYear
        return (
          <button
            type="button"
            key={year}
            onClick={() => onSelect(year)}
            className={cn(
              'grid w-full grid-cols-[44px_1fr_46px] items-center gap-[10px] rounded-[2px] px-2 py-1 text-left transition-colors duration-150',
              'hover:bg-hover',
              isCurrent &&
                'bg-accent-soft border-accent border-l-[2px] pl-[6px]',
            )}
            data-current={isCurrent ? 'true' : undefined}
          >
            <span className="text-ink font-mono text-[12px] tracking-[0.01em]">
              {year}
            </span>
            <span className="bg-page relative h-[6px] overflow-hidden rounded-[1px]">
              <span
                className="bg-accent absolute inset-y-0 left-0"
                style={{ width: `${pct}%`, opacity: opacityForTier(tier) }}
              />
            </span>
            <span className="text-ink-faint text-right font-mono text-[10px]">
              {label}
            </span>
          </button>
        )
      })}
    </div>
  )
}

function opacityForTier(tier: 0 | 1 | 2 | 3 | 4): number {
  if (tier === 0) return 0
  if (tier === 1) return 0.35
  if (tier === 2) return 0.55
  if (tier === 3) return 0.75
  return 1
}
