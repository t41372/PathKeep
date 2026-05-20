/**
 * GitHub-style year heatmap rendering the last 364 days as a 7-row × 52-column
 * grid. Built for the Dashboard's "A year in pages" card.
 *
 * Why this file exists:
 * - The design's heatmap is one of the most distinctive visual surfaces and
 *   gets reused across Dashboard, Intelligence, and possibly Explorer. Owning
 *   it in `src/components/heatmap/` lets each consumer pass real density data
 *   without rebuilding the layout.
 *
 * Not responsible for:
 * - Density generation (callers compute or fetch the data).
 * - Date math beyond rendering (consumer pre-computes ISO date strings).
 */

import { useMemo } from 'react'
import { cn } from '@/lib/cn'

export interface HeatmapCell {
  date: string
  count: number
  /** 0 (empty) … 4 (densest). */
  level: 0 | 1 | 2 | 3 | 4
}

export interface YearHeatmapProps {
  cells: HeatmapCell[]
  onSelectDate?: (date: string) => void
  monthLabels?: string[]
  dayLabels?: [string, string, string, string, string, string, string]
  legend?: { less: string; more: string }
  className?: string
}

const DEFAULT_DAY_LABELS: YearHeatmapProps['dayLabels'] = [
  '',
  'Mon',
  '',
  'Wed',
  '',
  'Fri',
  '',
]

const DEFAULT_LEGEND = { less: 'Less', more: 'More' }

export function YearHeatmap({
  cells,
  onSelectDate,
  monthLabels,
  dayLabels = DEFAULT_DAY_LABELS,
  legend = DEFAULT_LEGEND,
  className,
}: YearHeatmapProps) {
  const rows = useMemo<HeatmapCell[][]>(() => {
    const grouped: HeatmapCell[][] = [[], [], [], [], [], [], []]
    cells.forEach((cell, index) => {
      const dow = index % 7
      grouped[dow].push(cell)
    })
    return grouped
  }, [cells])

  const resolvedMonths = useMemo<string[]>(() => {
    if (monthLabels && monthLabels.length === 12) return monthLabels
    const months = [
      'Jan',
      'Feb',
      'Mar',
      'Apr',
      'May',
      'Jun',
      'Jul',
      'Aug',
      'Sep',
      'Oct',
      'Nov',
      'Dec',
    ]
    if (cells.length === 0) return months
    const first = cells[0]
    const firstDate = new Date(first.date)
    if (Number.isNaN(firstDate.getTime())) return months
    const ordered: string[] = []
    for (let i = 0; i < 12; i += 1) {
      const month = (firstDate.getMonth() + i) % 12
      ordered.push(months[month])
    }
    return ordered
  }, [monthLabels, cells])

  return (
    <div className={cn('pk-heatmap-wrap', className)}>
      <div className="grid grid-cols-[24px_repeat(52,minmax(0,1fr))] gap-x-[2px] gap-y-[2px] pb-1 text-[8.5px] text-ink-faint font-mono tracking-[0.04em] uppercase">
        <span />
        {resolvedMonths.map((label, index) => (
          <span key={`${label}-${index}`} className="col-span-4 truncate">
            {label}
          </span>
        ))}
      </div>
      <div
        className="grid grid-cols-[24px_repeat(52,minmax(0,1fr))] grid-rows-7 gap-x-[2px] gap-y-[2px]"
        role="grid"
        aria-label="365-day activity heatmap"
      >
        {rows.map((row, dayIndex) => (
          <DayRow
            key={dayIndex}
            label={dayLabels?.[dayIndex] ?? ''}
            row={row}
            onSelectDate={onSelectDate}
          />
        ))}
      </div>
      <div className="flex items-center justify-end gap-2 pt-2 font-mono text-[10px] text-ink-faint">
        <span>{legend.less}</span>
        {[0, 1, 2, 3, 4].map((level) => (
          <span
            key={level}
            className={cn(
              'inline-block h-[10px] w-[10px] rounded-[2px]',
              level === 0 && 'bg-hover border border-border-light',
              level === 1 && 'bg-accent-soft',
              level === 2 && 'bg-accent-medium',
              level === 3 && 'bg-accent-strong',
              level === 4 && 'bg-accent',
            )}
          />
        ))}
        <span>{legend.more}</span>
      </div>
    </div>
  )
}

interface DayRowProps {
  label: string
  row: HeatmapCell[]
  onSelectDate?: (date: string) => void
}

function DayRow({ label, row, onSelectDate }: DayRowProps) {
  return (
    <>
      <span className="font-mono text-[8.5px] text-ink-faint tracking-[0.04em]">
        {label}
      </span>
      {row.map((cell, index) => (
        <button
          key={`${cell.date}-${index}`}
          type="button"
          onClick={() => cell.count > 0 && onSelectDate?.(cell.date)}
          title={`${cell.date} · ${cell.count} pages`}
          aria-label={`${cell.date}: ${cell.count} pages`}
          className={cn(
            'aspect-square h-[10px] w-[10px] rounded-[2px] transition-transform hover:scale-110',
            cell.level === 0 &&
              'bg-hover border border-border-light cursor-default',
            cell.level === 1 && 'bg-accent-soft',
            cell.level === 2 && 'bg-accent-medium',
            cell.level === 3 && 'bg-accent-strong',
            cell.level === 4 && 'bg-accent',
            cell.count === 0 && 'cursor-default',
          )}
          disabled={cell.count === 0}
        />
      ))}
    </>
  )
}

/**
 * Generates a deterministic 364-day density distribution for the heatmap.
 *
 * Used by:
 * - Dashboard fallback when no real density is available.
 * - Browser preview / showcase fixtures.
 *
 * Real backend density (when wired) overrides this.
 */
// eslint-disable-next-line react-refresh/only-export-components
export function generateHeatmapCells(now: Date = new Date()): HeatmapCell[] {
  const cells: HeatmapCell[] = []
  const start = new Date(now)
  start.setDate(start.getDate() - 363)
  for (let i = 0; i < 364; i += 1) {
    const date = new Date(start)
    date.setDate(start.getDate() + i)
    const dow = date.getDay()
    const weekendBias = dow === 0 || dow === 6 ? 0.5 : 1.0
    const recencyBias = 0.4 + (i / 364) * 0.6
    const seed = pseudoRandom(date.getTime())
    const intensity = weekendBias * recencyBias * seed
    const count = Math.floor(intensity * 220)
    let level: HeatmapCell['level'] = 0
    if (count > 5) level = 1
    if (count > 30) level = 2
    if (count > 80) level = 3
    if (count > 140) level = 4
    cells.push({ date: date.toISOString().slice(0, 10), count, level })
  }
  return cells
}

function pseudoRandom(seed: number): number {
  const x = Math.sin(seed * 13.37) * 10000
  return Math.abs(x - Math.floor(x))
}
