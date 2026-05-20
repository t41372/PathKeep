/**
 * @file year-heatmap.tsx
 * @description Paper-aesthetic GitHub-style 7×N year-in-pages heatmap.
 * @module components/heatmap
 *
 * ## Responsibilities
 * - Render a dense 7×N (7 rows = Sun..Sat; N columns = weeks in window) grid
 *   of cells, with quartile-bucketed accent fills and the design's month
 *   labels above and "less / more" legend below.
 * - Forward `(date, count)` to the optional click handler when the user picks
 *   a day with non-zero visits.
 *
 * ## Not responsible for
 * - Fetching or bucketing the input — callers pass cells from
 *   `buildYearHeatmapCells` or a sibling helper.
 * - Date-range computation (callers own that).
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 *
 * ## Performance notes
 * - Renders ~365 cells. At that size React reconciliation is fine; no
 *   virtualization needed.
 */

import { useMemo } from 'react'
import type { YearHeatmapCell } from './year-heatmap-helpers'

export interface YearHeatmapCopy {
  legendLess: string
  legendMore: string
  /** Format used in the cell tooltip — receives placeholders {date} {count}. */
  cellTooltip: (date: string, count: number) => string
  /** Month labels, ordered Jan…Dec in the active locale. */
  monthLabels: [
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
  /** Three-letter day labels for Sunday..Saturday. */
  dayLabels: [string, string, string, string, string, string, string]
}

export interface YearHeatmapProps {
  cells: YearHeatmapCell[]
  copy: YearHeatmapCopy
  onSelectDate?: (date: string, count: number) => void
  testId?: string
}

const LEVEL_CLASS: Record<YearHeatmapCell['level'], string> = {
  0: 'bg-paper border-border-light',
  1: 'border-accent/30 bg-accent/15',
  2: 'border-accent/45 bg-accent/35',
  3: 'border-accent/65 bg-accent/60',
  4: 'border-accent bg-accent',
}

export function YearHeatmap({
  cells,
  copy,
  onSelectDate,
  testId,
}: YearHeatmapProps) {
  const weeks = useMemo(() => groupIntoWeeks(cells), [cells])
  const monthMarkers = useMemo(
    () => collectMonthMarkers(cells, copy.monthLabels),
    [cells, copy.monthLabels],
  )

  return (
    <div className="flex flex-col gap-2" data-testid={testId}>
      <div
        className="grid items-end font-mono text-[9px] text-ink-faint"
        style={{ gridTemplateColumns: `28px repeat(${weeks.length}, 1fr)` }}
      >
        <span />
        {monthMarkers.map((marker, index) => (
          <span
            key={index}
            className="tracking-[0.04em]"
            style={{ gridColumnStart: marker.column + 1, gridColumnEnd: 'span 1' }}
          >
            {marker.label ?? ''}
          </span>
        ))}
      </div>
      <div
        className="grid gap-[2px]"
        style={{ gridTemplateColumns: `28px repeat(${weeks.length}, 1fr)` }}
      >
        {[0, 1, 2, 3, 4, 5, 6].map((dow) => (
          <ColumnRow
            key={dow}
            label={dow === 1 || dow === 3 || dow === 5 ? copy.dayLabels[dow] : ''}
            cells={weeks.map((week) => week[dow] ?? null)}
            onSelectDate={onSelectDate}
            tooltip={copy.cellTooltip}
          />
        ))}
      </div>
      <div className="flex items-center justify-end gap-2 font-mono text-[9.5px] text-ink-faint">
        <span>{copy.legendLess}</span>
        <span className="flex items-center gap-[2px]">
          {[0, 1, 2, 3, 4].map((level) => (
            <span
              key={level}
              className={`h-[10px] w-[10px] rounded-[2px] border ${
                LEVEL_CLASS[level as YearHeatmapCell['level']]
              }`}
              aria-hidden="true"
            />
          ))}
        </span>
        <span>{copy.legendMore}</span>
      </div>
    </div>
  )
}

function ColumnRow({
  label,
  cells,
  onSelectDate,
  tooltip,
}: {
  label: string
  cells: (YearHeatmapCell | null)[]
  onSelectDate?: (date: string, count: number) => void
  tooltip: YearHeatmapCopy['cellTooltip']
}) {
  return (
    <>
      <span className="self-center pr-1 text-right font-mono text-[9px] text-ink-faint">
        {label}
      </span>
      {cells.map((cell, index) => {
        if (!cell) {
          return (
            <span
              key={index}
              className="aspect-square min-w-[10px] rounded-[2px] border border-transparent"
              aria-hidden="true"
            />
          )
        }
        const clickable = Boolean(onSelectDate && cell.count > 0)
        return (
          <button
            key={index}
            type="button"
            title={tooltip(cell.date, cell.count)}
            data-date={cell.date}
            data-level={cell.level}
            disabled={!clickable}
            onClick={() => {
              if (clickable && onSelectDate) onSelectDate(cell.date, cell.count)
            }}
            className={`aspect-square min-w-[10px] rounded-[2px] border transition-[outline-color,transform] duration-100 ${
              LEVEL_CLASS[cell.level]
            } ${
              clickable
                ? 'hover:outline hover:outline-1 hover:outline-ink cursor-pointer'
                : 'cursor-default'
            }`}
          />
        )
      })}
    </>
  )
}

/**
 * Groups the dense `cells` array into ISO-week columns. Each column is a
 * 7-element tuple keyed by day-of-week (0=Sun..6=Sat) so the grid lines up
 * vertically regardless of which weekday the window opens on.
 */
function groupIntoWeeks(
  cells: YearHeatmapCell[],
): Array<Array<YearHeatmapCell | null>> {
  if (cells.length === 0) return []
  const weeks: Array<Array<YearHeatmapCell | null>> = [
    [null, null, null, null, null, null, null],
  ]
  for (const cell of cells) {
    const lastWeek = weeks[weeks.length - 1]
    if (lastWeek[cell.dayOfWeek] !== null) {
      weeks.push([null, null, null, null, null, null, null])
    }
    weeks[weeks.length - 1][cell.dayOfWeek] = cell
  }
  return weeks
}

interface MonthMarker {
  column: number
  label: string
}

function collectMonthMarkers(
  cells: YearHeatmapCell[],
  monthLabels: YearHeatmapCopy['monthLabels'],
): MonthMarker[] {
  const markers: MonthMarker[] = []
  let seenMonth = -1
  let column = 0
  for (const cell of cells) {
    if (cell.dayOfWeek === 0) column += 1
    const month = Number(cell.date.slice(5, 7)) - 1
    if (month !== seenMonth) {
      markers.push({ column, label: monthLabels[month] })
      seenMonth = month
    }
  }
  return markers
}
