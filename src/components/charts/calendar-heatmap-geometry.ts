/**
 * @file calendar-heatmap-geometry.ts
 * @description Pure layout helpers for `CalendarHeatmap` — buckets a dense, pre-aggregated day-cell array into week columns and month-label markers, and publishes the fixed SVG grid constants the renderer positions `<rect>`s with.
 * @module components/charts
 *
 * ## Responsibilities
 * - Group a dense list of day cells into ISO-week columns keyed by
 *   day-of-week, so the visual grid lines up vertically regardless of which
 *   weekday the input window opens on (ported unchanged from the previous
 *   CSS-grid `year-heatmap.tsx`, including its column-increments-on-Sunday
 *   month-marker heuristic — this module is a geometry move, not a
 *   behavior change).
 * - Compute month-label column markers from date transitions in the cell
 *   list.
 * - Publish the fixed cell size/gap/padding/radius constants shared by
 *   `calendar-heatmap.tsx` and its tests, so the pixel grid and the "does
 *   this many cells render" assertions can't drift apart.
 *
 * ## Not responsible for
 * - Aggregating visit counts into cells — the caller's `useMemo` (e.g.
 *   `year-heatmap-card.tsx`'s `buildYearHeatmapCells`) owns that. This
 *   module only re-buckets an already-aggregated array into layout columns.
 * - Rendering or React — `calendar-heatmap.tsx` consumes this module's pure
 *   output.
 *
 * ## Dependencies
 * - None (pure TypeScript).
 *
 * ## Performance notes
 * - O(cells.length); safe on the ~365-off cell arrays `CalendarHeatmap`
 *   renders (see the 14.4M-row scale note in `chart-geometry.ts` — the
 *   render-path constraint is the same here: this never re-aggregates raw
 *   visits, only re-buckets already-small pre-aggregated cells).
 */

export interface CalendarHeatmapCell {
  /** YYYY-MM-DD ISO date string. */
  date: string
  /** Raw visit count for the day (0 if none). */
  count: number
  /** Display bucket 0-4: 0 = empty, 1-4 = ramped accent fill. */
  level: 0 | 1 | 2 | 3 | 4
  /** 0-6, Sunday-first day-of-week. */
  dayOfWeek: number
}

/** Cell edge length, in SVG user units (≈px at 1:1 scale). */
export const CELL_SIZE = 11
/** Gap between adjacent cells, in SVG user units. */
export const CELL_GAP = 2
/** Center-to-center distance between adjacent cells (size + gap). */
export const CELL_STRIDE = CELL_SIZE + CELL_GAP
/** Left inset reserved for the Mon/Wed/Fri day-of-week labels. */
export const GRID_LEFT_PAD = 24
/** Top inset reserved for the month labels. */
export const GRID_TOP_PAD = 14
/**
 * Cell corner radius, in SVG user units. SVG `rx` cannot consume a CSS
 * custom property directly (unlike the Tailwind `rounded-tight` utility
 * used for the HTML legend swatches), so this is a deliberate literal twin
 * of `--radius-tight` (2px) — keep it in lockstep with
 * `src/styles/tokens.css` if that token ever changes.
 */
export const CELL_RADIUS = 2

export interface MonthMarker {
  /** Week-column index (0-based) the label should sit above. */
  column: number
  label: string
}

export interface CalendarHeatmapLayout {
  /** Dense day cells re-bucketed into week columns, each a 7-tuple keyed by day-of-week (index 0=Sun..6=Sat); `null` marks a phantom slot with no data. */
  weeks: Array<Array<CalendarHeatmapCell | null>>
  monthMarkers: MonthMarker[]
}

/**
 * Buckets `cells` into week columns and derives month-label markers.
 *
 * @param cells Dense, already-aggregated day cells (caller-memoized).
 * @param monthLabels 12 localized month labels, Jan…Dec.
 */
export function buildCalendarHeatmapLayout(
  cells: CalendarHeatmapCell[],
  monthLabels: readonly string[],
): CalendarHeatmapLayout {
  return {
    weeks: groupIntoWeeks(cells),
    monthMarkers: collectMonthMarkers(cells, monthLabels),
  }
}

/** Total SVG viewBox width for a layout with `columns` week columns. */
export function gridWidth(columns: number): number {
  return GRID_LEFT_PAD + Math.max(columns, 0) * CELL_STRIDE
}

/** Total SVG viewBox height (always 7 day-of-week rows). */
export function gridHeight(): number {
  return GRID_TOP_PAD + 7 * CELL_STRIDE
}

function emptyWeek(): Array<CalendarHeatmapCell | null> {
  return [null, null, null, null, null, null, null]
}

/**
 * Groups the dense `cells` array into week columns. Each column is a
 * 7-element tuple keyed by day-of-week (0=Sun..6=Sat) so the grid lines up
 * vertically regardless of which weekday the window opens on. A new column
 * starts whenever the current column's slot for that day-of-week is already
 * filled.
 */
function groupIntoWeeks(
  cells: CalendarHeatmapCell[],
): Array<Array<CalendarHeatmapCell | null>> {
  if (cells.length === 0) return []
  const weeks: Array<Array<CalendarHeatmapCell | null>> = [emptyWeek()]
  for (const cell of cells) {
    const lastWeek = weeks[weeks.length - 1]
    if (lastWeek[cell.dayOfWeek] !== null) {
      weeks.push(emptyWeek())
    }
    weeks[weeks.length - 1][cell.dayOfWeek] = cell
  }
  return weeks
}

/**
 * Derives month-label column markers by walking `cells` in order and
 * recording a marker every time the month changes, incrementing the tracked
 * column on every Sunday cell.
 */
function collectMonthMarkers(
  cells: CalendarHeatmapCell[],
  monthLabels: readonly string[],
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
