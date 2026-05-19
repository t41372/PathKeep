/**
 * Vertical year rail — the right-edge mini-map of the full archive.
 *
 * Renders one cell per year between `bounds.firstYear` and `bounds.lastYear`
 * inclusive, top-to-bottom newest → oldest. Each year is density-tinted
 * (t0..t4) so a 60-year archive reads as a thermometer. The active year is
 * outlined, a tiny horizontal indicator marks where in that year the
 * currently-viewed day sits, and decade boundaries get a hairline tick + a
 * floating label so users can find "1990s" without counting.
 *
 * ## Responsibilities
 * - Render the year stack with density tints, decade ticks, and labels.
 * - Highlight the active year and mark the month-within-year position.
 * - Hide itself below the narrow breakpoint (caller-controlled via className
 *   override or the default `hidden md:flex`).
 *
 * ## Not responsible for
 * - The day-level density (that's the heatmap / calendar). The rail is a
 *   year-aggregate at-a-glance scrubber.
 * - Computing where to jump within a year. Caller chooses; helper
 *   `pickYearJumpIso` below offers a sensible default (mid-June or the
 *   archive's last day for the most recent year).
 */

import { useMemo } from 'react'
import { cn } from '@/lib/cn'
import { periodDensityTier } from '@/pages/explorer/paper/date-helpers'
import { pickYearJumpIso } from './paper-year-rail-helpers'

export interface PaperYearRailProps {
  /** Year-aggregate visit counts. */
  densityByYear: ReadonlyMap<number, number>
  bounds: { firstYear: number; lastYear: number; lastIso: string }
  /** Active day's ISO string — drives current-year highlight + month indicator. */
  currentDate: string
  onJump: (iso: string) => void
  /** Screen-reader label for the whole control. */
  ariaLabel?: string
  /** Pretty-format helper, e.g. (year, count) => "1990 · 12,345 pages". */
  titleFor?: (year: number, count: number) => string
  className?: string
  testId?: string
}

export function PaperYearRail({
  densityByYear,
  bounds,
  currentDate,
  onJump,
  ariaLabel,
  titleFor,
  className,
  testId,
}: PaperYearRailProps) {
  const { years, currentYear, currentMonthIdx } = useMemo(() => {
    const list: number[] = []
    for (let y = bounds.lastYear; y >= bounds.firstYear; y -= 1) list.push(y)
    const cy = currentDate
      ? Number.parseInt(currentDate.slice(0, 4), 10)
      : bounds.lastYear
    const cm = currentDate
      ? Number.parseInt(currentDate.slice(5, 7), 10) - 1
      : 0
    return {
      years: list,
      currentYear: Number.isFinite(cy) ? cy : bounds.lastYear,
      currentMonthIdx: Number.isFinite(cm) && cm >= 0 && cm <= 11 ? cm : 0,
    }
  }, [bounds.firstYear, bounds.lastYear, currentDate])

  return (
    <aside
      aria-label={ariaLabel ?? 'Year scrubber'}
      data-testid={testId}
      className={cn(
        'fixed bottom-[44px] right-3 top-[100px] z-[8] w-[30px]',
        'bg-card-paper border-border-light rounded-paper border',
        'hidden flex-col py-1 md:flex',
        'select-none',
        className,
      )}
    >
      <div className="border-border-light text-ink-faint flex flex-col items-center border-b border-dashed py-[2px] font-mono text-[9px] leading-[1.1] tracking-[0.04em]">
        <span>{bounds.lastYear}</span>
        <span className="text-ink-ghost mt-px text-[8px]">now</span>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-px px-1 py-1">
        {years.map((year) => {
          const count = densityByYear.get(year) ?? 0
          const tier = periodDensityTier(count)
          const isCurrent = year === currentYear
          const isDecade = year % 10 === 0
          const jumpIso = pickYearJumpIso(year, bounds)
          const title = titleFor
            ? titleFor(year, count)
            : `${year} · ${count.toLocaleString()} pages`

          return (
            <button
              type="button"
              key={year}
              title={title}
              onClick={() => onJump(jumpIso)}
              data-year={year}
              data-tier={`t${tier}`}
              data-decade={isDecade ? 'true' : undefined}
              data-current={isCurrent ? 'true' : undefined}
              aria-label={title}
              className={cn(
                'relative min-h-[4px] flex-1 cursor-pointer rounded-[1px]',
                'transition-[outline-color] duration-75',
                'outline outline-1 outline-transparent hover:outline-ink hover:z-[1]',
                yearTierClass(tier),
                isDecade &&
                  'border-t border-[color-mix(in_srgb,var(--ink)_18%,transparent)]',
                isCurrent &&
                  'z-[2] outline outline-[1.5px] outline-offset-[1px] outline-ink',
              )}
            >
              {isCurrent ? (
                <span
                  aria-hidden="true"
                  data-testid="year-month-indicator"
                  className="bg-ink pointer-events-none absolute -left-[2px] -right-[2px] h-[2px] rounded-[1px]"
                  style={{ top: `${(currentMonthIdx / 12) * 100}%` }}
                />
              ) : null}
              {isDecade ? (
                <span
                  aria-hidden="true"
                  className="text-ink-faint pointer-events-none absolute right-full top-1/2 mr-[6px] -translate-y-1/2 whitespace-nowrap font-mono text-[9px] tracking-[0.02em]"
                >
                  {year}
                </span>
              ) : null}
            </button>
          )
        })}
      </div>

      <div className="border-border-light text-ink-faint flex flex-col items-center border-t border-dashed py-[2px] font-mono text-[9px] leading-[1.1] tracking-[0.04em]">
        <span>{bounds.firstYear}</span>
        <span className="text-ink-ghost mt-px text-[8px]">first</span>
      </div>
    </aside>
  )
}

function yearTierClass(tier: 0 | 1 | 2 | 3 | 4): string {
  switch (tier) {
    case 0:
      return 'bg-page opacity-60'
    case 1:
      return 'bg-[color-mix(in_srgb,var(--accent)_22%,var(--bg-page))]'
    case 2:
      return 'bg-[color-mix(in_srgb,var(--accent)_45%,var(--bg-page))]'
    case 3:
      return 'bg-[color-mix(in_srgb,var(--accent)_70%,var(--bg-page))]'
    case 4:
      return 'bg-accent'
  }
}
