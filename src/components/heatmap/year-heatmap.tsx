/**
 * @file year-heatmap.tsx
 * @description Paper-aesthetic GitHub-style 7×N year-in-pages heatmap.
 * @module components/heatmap
 *
 * ## Responsibilities
 * - Adapt the dashboard/year-review `YearHeatmapCell` shape onto the shared
 *   `CalendarHeatmap` SVG chart primitive (`src/components/charts`) and
 *   forward `(date, count)` to the optional click handler when the user
 *   picks a day with non-zero visits — by mouse or keyboard.
 * - Preserve the public `YearHeatmapProps`/`YearHeatmapCopy` contract so
 *   existing callers (`year-heatmap-card.tsx`, `year-review.tsx`) keep
 *   working unchanged apart from adding the new required `ariaLabel` copy
 *   field the shared primitive needs for its `role="grid"` label.
 *
 * ## Not responsible for
 * - Fetching or bucketing the input — callers pass cells from
 *   `buildYearHeatmapCells` or a sibling helper.
 * - Date-range computation (callers own that).
 * - Grid geometry, ramp-token mapping, or a11y wiring — all of that now
 *   lives in `CalendarHeatmap` / `calendar-heatmap-geometry.ts` so it's
 *   shared with any future calendar-shaped chart.
 *
 * ## Dependencies
 * - `@/components/charts` (`CalendarHeatmap`) for rendering.
 *
 * ## Performance notes
 * - Renders ~365 cells via `CalendarHeatmap`, which is fine at that size —
 *   no virtualization needed. This wrapper adds no extra render-path work
 *   beyond passing `cells` through.
 */

import {
  CalendarHeatmap,
  type CalendarHeatmapCopy,
} from '@/components/charts/calendar-heatmap'
import type { YearHeatmapCell } from './year-heatmap-helpers'

export interface YearHeatmapCopy {
  /** Describes the whole chart for screen readers, e.g. "Calendar heatmap of daily page visits over the past year". */
  ariaLabel: string
  legendLess: string
  legendMore: string
  /** Per-cell accessible name (and previously hover-only tooltip) — receives placeholders {date} {count}. Now also drives the SVG cell's `aria-label`. */
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

export function YearHeatmap({
  cells,
  copy,
  onSelectDate,
  testId,
}: YearHeatmapProps) {
  const calendarCopy: CalendarHeatmapCopy = {
    ariaLabel: copy.ariaLabel,
    legendLess: copy.legendLess,
    legendMore: copy.legendMore,
    cellAccessibleName: copy.cellTooltip,
    monthLabels: copy.monthLabels,
    dayLabels: copy.dayLabels,
  }

  return (
    <CalendarHeatmap
      cells={cells}
      copy={calendarCopy}
      onSelectDay={onSelectDate}
      testId={testId}
    />
  )
}
